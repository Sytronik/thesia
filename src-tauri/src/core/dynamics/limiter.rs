//! Limiter Implementation motivated by https://signalsmith-audio.co.uk/writing/2022/limiter/

use identity_hash::IntMap;
use ndarray::prelude::*;
use num_traits::{Float, NumAssignOps, NumOps};
use rayon::prelude::*;

use super::envelope::{BoxStackFilter, PeakHold};

#[derive(Clone)]
#[readonly::make]
pub struct ExponentialRelease<A> {
    pub release_samples: A,
    pub initial_value: A,
    release_slew: A,
    output: A,
}

impl<A: Float + NumOps + NumAssignOps> ExponentialRelease<A> {
    pub fn new(release_samples: A) -> Self {
        ExponentialRelease::with_initial_value(release_samples, A::one())
    }

    pub fn with_initial_value(release_samples: A, initial_value: A) -> Self {
        ExponentialRelease {
            release_samples,
            initial_value,
            release_slew: (release_samples + A::one()).recip(),
            output: initial_value,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.output = self.initial_value;
    }

    pub fn step(&mut self, input: A) -> A {
        let output = input.min((input - self.output).mul_add(self.release_slew, self.output));
        self.output = output;
        output
    }
}

#[derive(Clone)]
#[readonly::make]
pub struct PerfectLimiter {
    pub threshold: f64,
    attack: usize,
    peakhold: PeakHold<f64>,
    release: ExponentialRelease<f64>,
    smoother: BoxStackFilter<f64>,
    /// buffer of interleaved samples
    buffer: Array2<f64>,
    i_buf: usize,
}

impl PerfectLimiter {
    pub fn new(sr: u32, threshold: f64, attack_ms: f64, hold_ms: f64, release_ms: f64) -> Self {
        debug_assert!(threshold > f32::EPSILON as f64);
        debug_assert!(attack_ms >= 0.);
        debug_assert!(hold_ms >= 0.);
        debug_assert!(release_ms >= 0.);
        let ms_to_samples = |x: f64| x * sr as f64 / 1000.;
        let attack = ms_to_samples(attack_ms).round() as usize;
        let mut smoother = BoxStackFilter::with_num_layers(attack, 3);
        smoother.reset(1.);
        PerfectLimiter {
            threshold,
            attack,
            peakhold: PeakHold::new(sr, attack_ms + hold_ms),
            release: ExponentialRelease::new(ms_to_samples(release_ms)),
            smoother,
            buffer: Array2::zeros((attack, 0)),
            i_buf: 0,
        }
    }

    #[inline]
    pub fn with_default(sr: u32) -> Self {
        Self::new(sr, 1., 5., 15., 40.)
    }

    pub fn reset(&mut self, n_ch: usize) {
        self.peakhold.reset_default();
        self.release.reset();
        self.smoother.reset(1.);
        self.buffer = Array2::zeros((self.attack, n_ch));
    }

    /// process one sample, and returns (delayed_output, gain)
    pub fn _step(&mut self, frame: ArrayView1<f32>) -> (Array1<f32>, f32) {
        let mut delayed = self.buffer.slice(s![self.i_buf, ..]).to_owned();
        let gain = self.calc_gain(frame);
        azip!((y in &mut self.buffer.slice_mut(s![self.i_buf, ..]), x in &frame) *y = *x as f64);
        self.i_buf = (self.i_buf + 1) % self.buffer.shape()[0];
        // println!("i={} d={} g={}", value, delayed, gain);
        delayed *= gain;
        for &y in &delayed {
            debug_assert!(-1. - (f32::EPSILON as f64) < y && y < 1. + (f32::EPSILON as f64));
        }
        let out = delayed
            .into_iter()
            .map(|y| y.clamp(-1., 1.) as f32)
            .collect();
        (out, gain as f32)
    }

    /// apply limiter to wav inplace , return gain array. parallel over channel axis (=Axis(0))
    pub fn process_inplace(&mut self, mut wavs: ArrayViewMut2<f32>) -> Array1<f32> {
        self.reset(0);

        let zero = Array1::zeros(wavs.shape()[0]);
        let attack = self.attack;
        let gain_seq: Array1<_> = itertools::chain(
            wavs.lanes(Axis(0)),
            itertools::repeat_n(zero.view(), attack),
        )
        .map(|x| self.calc_gain(x))
        .skip(attack)
        .collect();

        wavs.axis_iter_mut(Axis(0))
            .into_par_iter()
            .for_each(|mut ch| {
                for (x, &gain) in ch.iter_mut().zip(&gain_seq) {
                    let y = *x as f64 * gain;
                    debug_assert!(
                        -1. - (f32::EPSILON as f64) < y && y < 1. + (f32::EPSILON as f64)
                    );
                    *x = y.clamp(-1., 1.) as f32;
                }
            });
        gain_seq.mapv(|x| x as f32)
    }

    /// apply limiter to wav, return (output, gain array)
    #[inline]
    pub fn _process(&mut self, wavs: ArrayView2<f32>) -> (Array2<f32>, Array1<f32>) {
        let mut out = wavs.to_owned();
        let gain_seq = self.process_inplace(out.view_mut());
        (out, gain_seq)
    }

    fn calc_gain(&mut self, frame: ArrayView1<f32>) -> f64 {
        // max abs over all channels
        let v_abs = frame.iter().map(|x| x.abs()).reduce(f32::max).unwrap() as f64;
        let raw_gain = if v_abs > self.threshold {
            self.threshold / (v_abs + f64::EPSILON)
        } else {
            1.
        };
        let peak_holded = -self.peakhold.step(-raw_gain);
        let peak_holded_released = self.release.step(peak_holded);
        self.smoother.step(peak_holded_released).min(1.)
    }

    #[inline]
    pub fn _latency_samples(&self) -> usize {
        self.attack
    }

    #[inline]
    pub fn _hold_samples(&self) -> usize {
        self.peakhold.hold_length() - self.attack
    }

    #[inline]
    pub fn _release_samples(&self) -> f64 {
        self.release.release_samples
    }
}

/// Limiter with imperfect anticipation.
/// This is a rust version of cylimiter https://github.com/pzelasko/cylimiter
#[allow(dead_code)]
#[derive(Clone)]
pub struct SimpleLimiter {
    attack: f64,
    release: f64,
    lookahead: usize,
    threshold: f64,
    lookahead_buf: Vec<f32>,
}

#[allow(dead_code)]
impl SimpleLimiter {
    pub fn new(
        sr: u32,
        attack: f64,
        release: f64,
        lookahead_ms: f64,
        threshold: f64,
    ) -> SimpleLimiter {
        debug_assert!((0.0..1.0).contains(&attack));
        debug_assert!((0.0..1.0).contains(&release));
        debug_assert!(lookahead_ms > 0.);
        debug_assert!(threshold > 0.);
        let sr = sr as f64;
        let lookahead = (sr * lookahead_ms / 1000.).round() as usize;
        let lookahead_buf = vec![0.; lookahead];
        SimpleLimiter {
            attack,
            release,
            lookahead,
            threshold,
            lookahead_buf,
        }
    }

    pub fn process_inplace(&mut self, mut wav: ArrayViewMut1<f32>) {
        let mut delay_index = 0;
        let mut gain = 1.;
        let mut envelope = 0.;

        self.lookahead_buf.fill(0.);
        for i_look in 0..(wav.len() + self.lookahead) {
            let look_sample = if i_look < wav.len() { wav[i_look] } else { 0. };
            self.lookahead_buf[delay_index] = look_sample;
            delay_index = (delay_index + 1) % self.lookahead;

            envelope = (look_sample.abs() as f64).max(envelope * self.release);

            let target_gain = if envelope > self.threshold {
                self.threshold / envelope
            } else {
                1.
            };

            gain = gain.mul_add(self.attack, target_gain.mul_add(-self.attack, target_gain));

            if i_look >= self.lookahead {
                let out = self.lookahead_buf[delay_index] as f64 * gain;
                wav[i_look - self.lookahead] = (out as f32).clamp(-1., 1.);
            }
        }
    }

    pub fn process(&mut self, wav: ArrayView1<f32>) -> Array1<f32> {
        let mut out = wav.to_owned();
        self.process_inplace(out.view_mut());
        out
    }
}

pub struct LimiterManager(IntMap<u32, PerfectLimiter>);

impl LimiterManager {
    pub fn new() -> Self {
        LimiterManager(Default::default())
    }

    pub fn get_or_insert(&mut self, sr: u32) -> &mut PerfectLimiter {
        self.0
            .entry(sr)
            .or_insert_with(|| PerfectLimiter::with_default(sr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::audio::open_audio_file;

    #[test]
    fn limiter_works() {
        let path = "../samples/sample_48k.wav";
        let (mut wavs, format_info) = open_audio_file(path).unwrap();
        let mut limiter = PerfectLimiter::new(format_info.sr, 1., 5., 15., 40.);
        wavs *= 8.;
        let gain_seq = limiter.process_inplace(wavs.view_mut());
        assert!(
            gain_seq.iter().all(|x| (0.0..=1.0).contains(x)),
            "cnt of gain>1: {}, cnt of gain<0: {}",
            gain_seq.iter().filter(|&&x| x > 1.).count(),
            gain_seq.iter().filter(|&&x| x < 0.).count(),
        );

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: format_info.sr,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer =
            hound::WavWriter::create("../samples/sample_48k_plus18dB_limit.wav", spec).unwrap();
        for sample in wavs.into_iter() {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }
}
