//! Limiter Implementation motivated by https://signalsmith-audio.co.uk/writing/2022/limiter/

use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::sync::Arc;

use lazy_static::lazy_static;
use ndarray::prelude::*;
use num_traits::{Float, NumAssignOps, NumOps};
use parking_lot::RwLock;

use super::envelope::{BoxStackFilter, PeakHold};

#[derive(Clone)]
#[readonly::make]
pub struct ExponentialRelease<A: Float + NumOps> {
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
            release_slew: A::one() / (release_samples + A::one()),
            output: initial_value,
        }
    }

    pub fn reset(&mut self) {
        self.output = self.initial_value;
    }

    pub fn step(&mut self, input: A) -> A {
        let output = input.min(self.output + (input - self.output) * self.release_slew);
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
    buffer: Vec<f32>,
    i_buf: usize,
}

impl PerfectLimiter {
    pub fn new(sr: u32, threshold: f64, attack_ms: f64, hold_ms: f64, release_ms: f64) -> Self {
        assert!(threshold > f32::EPSILON as f64);
        assert!(attack_ms >= 0.);
        assert!(hold_ms >= 0.);
        assert!(release_ms >= 0.);
        let ms_to_samples = |x: f64| (x * sr as f64 / 1000.);
        let attack = ms_to_samples(attack_ms).round() as usize;
        let mut smoother = BoxStackFilter::with_num_layers(attack, 3);
        smoother.reset(1.);
        PerfectLimiter {
            threshold,
            attack,
            peakhold: PeakHold::new(sr, attack_ms + hold_ms),
            release: ExponentialRelease::new(ms_to_samples(release_ms)),
            smoother,
            buffer: vec![0.; attack],
            i_buf: 0,
        }
    }

    pub fn with_default(sr: u32) -> Self {
        Self::new(sr, 1., 5., 15., 40.)
    }

    pub fn reset(&mut self) {
        self.peakhold.reset_default();
        self.release.reset();
        self.smoother.reset(1.);
        self.buffer.fill(0.);
    }

    /// process one sample, and returns (delayed_output, gain)
    pub fn step(&mut self, value: f32) -> (f32, f32) {
        let delayed = self.buffer[self.i_buf] as f64;
        let gain = self.calc_gain(value);
        self.buffer[self.i_buf] = value;
        self.i_buf = (self.i_buf + 1) % self.buffer.len();
        // println!("i={} d={} g={}", value, delayed, gain);
        let out = (delayed * gain) as f32;
        debug_assert!(-1. - f32::EPSILON < out && out < 1. + f32::EPSILON);
        (out.clamp(-1., 1.), gain as f32)
    }

    /// apply limiter to wav inplace, return gain array
    pub fn process_inplace(&mut self, mut wav: ArrayViewMut1<f32>) -> Array1<f32> {
        self.reset();
        let mut gain_arr = Array1::uninit(wav.raw_dim());
        for i in 0..(wav.len() + self.buffer.len()) {
            let input = if i < wav.len() { wav[i] } else { 0. };
            let (output, gain) = self.step(input);
            if i >= self.buffer.len() {
                let j = i - self.buffer.len();
                wav[j] = output;
                gain_arr[j] = MaybeUninit::new(gain);
            }
        }
        unsafe { gain_arr.assume_init() }
    }

    /// apply limiter to wav, return (output, gain array)
    #[inline]
    pub fn _process(&mut self, wav: ArrayView1<f32>) -> (Array1<f32>, Array1<f32>) {
        let mut out = wav.to_owned();
        let gain_arr = self.process_inplace(out.view_mut());
        (out, gain_arr)
    }

    fn calc_gain(&mut self, value: f32) -> f64 {
        let v_abs = value.abs() as f64;
        let raw_gain = if v_abs > self.threshold {
            self.threshold / (v_abs + f64::EPSILON)
        } else {
            1.
        };
        let peak_holded = -self.peakhold.step(-raw_gain);
        let peak_holded_released = self.release.step(peak_holded);
        self.smoother.step(peak_holded_released)
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
        assert!((0.0..1.0).contains(&attack));
        assert!((0.0..1.0).contains(&release));
        assert!(lookahead_ms > 0.);
        assert!(threshold > 0.);
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

            gain = gain * self.attack + target_gain * (1. - self.attack);

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

struct LimiterManager(HashMap<u32, PerfectLimiter>);

impl LimiterManager {
    pub fn new() -> Self {
        LimiterManager(HashMap::new())
    }

    pub fn get(&self, sr: u32) -> Option<PerfectLimiter> {
        self.0.get(&sr).cloned()
    }

    pub fn insert(&mut self, sr: u32) -> PerfectLimiter {
        let limiter = PerfectLimiter::with_default(sr);
        self.0.insert(sr, limiter.clone());
        limiter
    }
}

pub fn get_cached_limiter(sr: u32) -> PerfectLimiter {
    lazy_static! {
        static ref LIMITER_MANAGER: Arc<RwLock<LimiterManager>> =
            Arc::new(RwLock::new(LimiterManager::new()));
    }

    let limiter_or_none = LIMITER_MANAGER.read().get(sr);
    match limiter_or_none {
        Some(limiter) => limiter,
        None => LIMITER_MANAGER.write().insert(sr),
    }
}
#[cfg(test)]
mod tests {
    use super::super::audio::open_audio_file;
    use super::*;

    #[test]
    fn limiter_works() {
        let path = "samples/sample_48k.wav";
        let (wavs, sr, _) = open_audio_file(path).unwrap();
        let mut wav = wavs.index_axis_move(Axis(0), 0);
        let mut limiter = PerfectLimiter::new(sr, 1., 5., 15., 40.);
        wav *= 8.;
        limiter.process_inplace(wav.view_mut());

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: sr,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer =
            hound::WavWriter::create("samples/sample_48k_plus18dB_limit.wav", spec).unwrap();
        for sample in wav.into_iter() {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }
}
