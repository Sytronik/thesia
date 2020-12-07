use std::collections::HashMap;
use std::io;
use std::ops::*;

use approx::abs_diff_ne;
use ndarray::{prelude::*, ScalarOperand};
use ndarray_stats::QuantileExt;
use rayon::prelude::*;
use rustfft::{num_complex::Complex, num_traits::Float, FFTnum};
use wasm_bindgen::prelude::*;

pub mod audio;
pub mod decibel;
pub mod display;
pub mod mel;
pub mod realfft;
pub mod utils;
pub mod windows;
use decibel::DeciBelInplace;
use realfft::RealFFT;
use utils::{calc_proper_n_fft, pad, PadMode};

pub enum FreqScale {
    Linear,
    Mel,
}

pub struct AudioTrack {
    path: String,
    wav: Array1<f32>,
    sr: u32,
    spec: Option<Array2<f32>>,
    fft_module: Option<RealFFT<f32>>,
}

impl AudioTrack {
    pub fn new(path: &str, setting: &SpecSetting) -> io::Result<Self> {
        let (wav, sr) = audio::open_audio_file(path)?;
        let wav = wav.sum_axis(Axis(0)); // TODO: stereo support
        let mut instance = AudioTrack {
            path: path.to_string(),
            wav,
            sr,
            spec: None,
            fft_module: None,
        };
        instance.get_or_calc_spec(setting);
        Ok(instance)
    }

    pub fn reload(&mut self, setting: &SpecSetting) -> io::Result<()> {
        let (wav, sr) = audio::open_audio_file(&self.path[..])?;
        self.wav = wav.sum_axis(Axis(0)); // TODO: stereo support
        self.sr = sr;
        self.get_or_calc_spec(setting);
        Ok(())
    }

    fn get_or_calc_spec(&mut self, setting: &SpecSetting) -> ArrayView2<f32> {
        if self.spec.is_none() {
            let win_length = setting.win_ms * self.sr as f32 / 1000.;
            let hop_length = (win_length / setting.t_overlap as f32).round() as usize;
            let win_length = hop_length * setting.t_overlap;
            let n_fft = calc_proper_n_fft(win_length) * setting.f_overlap;
            let need_new_module = match &self.fft_module {
                Some(m) => m.get_length() != n_fft,
                None => true,
            };
            if need_new_module {
                self.fft_module = Some(RealFFT::new(n_fft).unwrap());
            }
            let stft = perform_stft(
                self.wav.view(),
                win_length,
                hop_length,
                Some(self.fft_module.as_mut().unwrap()),
                false,
            );
            let mut linspec = stft.mapv(|x| x.norm());
            self.spec = Some(match setting.freq_scale {
                FreqScale::Linear => {
                    linspec.amp_to_db_default();
                    linspec
                }
                FreqScale::Mel => {
                    let mut melspec =
                        linspec.dot(&mel::mel_filterbanks(self.sr, n_fft, 128, 0f32, None));
                    melspec.amp_to_db_default();
                    melspec
                }
            });
        }
        self.spec.as_ref().unwrap().view()
    }
}

pub struct SpecSetting {
    win_ms: f32,
    t_overlap: usize,
    f_overlap: usize,
    freq_scale: FreqScale,
    db_range: f32,
}

#[wasm_bindgen]
pub struct MultiTrack {
    tracks: HashMap<usize, AudioTrack>,
    setting: SpecSetting,
    max_db: f32,
    min_db: f32,
    max_sec: f32,
    id_max_sec: usize,
}

#[wasm_bindgen]
impl MultiTrack {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        MultiTrack {
            tracks: HashMap::new(),
            setting: SpecSetting {
                win_ms: 40.,
                t_overlap: 4,
                f_overlap: 1,
                freq_scale: FreqScale::Mel,
                db_range: 120.,
            },
            max_db: 0.,
            min_db: -120.,
            max_sec: 0.,
            id_max_sec: 0,
        }
    }

    pub fn add_tracks(&mut self, id_list: &[usize], path_list: &str) -> Result<bool, JsValue> {
        for (&id, path) in id_list.iter().zip(path_list.split("\n").into_iter()) {
            let track = match AudioTrack::new(path, &self.setting) {
                Ok(track) => track,
                Err(err) => return Err(JsValue::from(err.to_string())),
            };
            let sec = track.wav.len() as f32 / track.sr as f32;
            if sec > self.max_sec {
                self.max_sec = sec;
                self.id_max_sec = id;
            }
            self.tracks.insert(id, track);
        }
        Ok(self.update_db_scale())
    }

    fn update_db_scale(&mut self) -> bool {
        let (mut max, mut min) = self
            .tracks
            .par_iter()
            .map(|(_, track)| {
                let spec = if let Some(x) = track.spec.as_ref() {
                    x
                } else {
                    return (-f32::INFINITY, f32::INFINITY);
                };
                let max = *spec.max().unwrap_or(&-f32::INFINITY);
                let min = *spec.min().unwrap_or(&f32::INFINITY);
                (max, min)
            })
            .reduce(
                || (-f32::INFINITY, f32::INFINITY),
                |(max, min): (f32, f32), (current_max, current_min)| {
                    (max.max(current_max), min.min(current_min))
                },
            );
        if max.is_infinite() {
            max = 0.
        }
        if min < max - self.setting.db_range {
            min = max - self.setting.db_range;
        }
        let mut changed = false;
        if abs_diff_ne!(self.max_db, max, epsilon = 1e-3) {
            self.max_db = max;
            changed = true;
        }
        if abs_diff_ne!(self.min_db, min, epsilon = 1e-3) {
            self.min_db = min;
            changed = true;
        }
        changed
    }

    pub fn remove_track(&mut self, id: usize) -> bool {
        self.tracks.remove(&id);
        if self.id_max_sec == id {
            let (id, max_sec) = self.tracks
                .par_iter()
                .map(|(id, track)| (*id, track.wav.len() as f32 / track.sr as f32))
                .reduce(
                    || (0, 0.),
                    |(id_max, max), (id, sec)| {
                        if sec > max {
                            (id, sec)
                        } else {
                            (id_max, max)
                        }
                    },
                );
            self.id_max_sec = id;
            self.max_sec = max_sec;
        }
        self.update_db_scale()
    }

    pub fn get_spec_image(&mut self, id: usize, px_per_sec: f32, nheight: u32) -> Vec<u8> {
        let track = self.tracks.get_mut(&id).unwrap();
        let nwidth = (px_per_sec * track.wav.len() as f32 / track.sr as f32) as u32;
        let specview = track.get_or_calc_spec(&self.setting);
        display::spec_to_image(specview, nwidth, nheight, self.max_db, self.min_db).into_raw()
    }
}

fn to_windowed_frames<A: Float>(
    input: ArrayView1<A>,
    window: ArrayView1<A>,
    hop_length: usize,
    (n_pad_left, n_pad_right): (usize, usize),
) -> Vec<Array1<A>> {
    input
        .windows(window.len())
        .into_iter()
        .step_by(hop_length)
        .map(|x| {
            pad(
                (&x * &window).view(),
                (n_pad_left, n_pad_right),
                Axis(0),
                PadMode::Constant(A::zero()),
            )
        })
        .collect()
}

pub fn perform_stft<A>(
    input: ArrayView1<A>,
    win_length: usize,
    hop_length: usize,
    fft_module: Option<&mut RealFFT<A>>,
    parallel: bool,
) -> Array2<Complex<A>>
where
    A: FFTnum + Float + DivAssign + ScalarOperand,
{
    let n_fft = 2usize.pow((win_length as f32).log2().ceil() as u32);
    let n_pad_left = (n_fft - win_length) / 2;
    let n_pad_right = (((n_fft - win_length) as f32) / 2.).ceil() as usize;

    let mut window = windows::hann(win_length, false);
    window /= A::from(n_fft).unwrap();

    let to_frames_wrapper =
        |x| to_windowed_frames(x, window.view(), hop_length, (n_pad_left, n_pad_right));
    let front_wav = pad(
        input.slice(s![..(win_length - 1)]),
        (win_length / 2, 0),
        Axis(0),
        PadMode::Reflect,
    );
    let mut front_frames = to_frames_wrapper(front_wav.view());

    let mut first_idx = front_frames.len() * hop_length - win_length / 2;
    let mut frames: Vec<Array1<A>> = to_frames_wrapper(input.slice(s![first_idx..]));

    first_idx += frames.len() * hop_length;
    let back_wav_start_idx = first_idx.min(input.len() - win_length / 2 - 1);

    let mut back_wav = pad(
        input.slice(s![back_wav_start_idx..]),
        (0, win_length / 2),
        Axis(0),
        PadMode::Reflect,
    );
    back_wav.slice_collapse(s![(first_idx - back_wav_start_idx).max(0)..]);
    let mut back_frames = to_frames_wrapper(back_wav.view());

    let n_frames = front_frames.len() + frames.len() + back_frames.len();
    let mut output = Array2::<Complex<A>>::zeros((n_frames, n_fft / 2 + 1));
    let out_frames: Vec<&mut [Complex<A>]> = output
        .axis_iter_mut(Axis(0))
        .map(|x| x.into_slice().unwrap())
        .collect();

    let mut new_module;
    let fft_module = if let Some(m) = fft_module {
        m
    } else {
        new_module = RealFFT::<A>::new(n_fft).unwrap();
        &mut new_module
    };
    if parallel {
        let in_frames = front_frames
            .par_iter_mut()
            .chain(frames.par_iter_mut())
            .chain(back_frames.par_iter_mut());
        in_frames.zip(out_frames).for_each(|(x, y)| {
            let mut fft_module = RealFFT::<A>::new(n_fft).unwrap();
            let x = x.as_slice_mut().unwrap();
            fft_module.process(x, y).unwrap();
        });
    } else {
        let in_frames = front_frames
            .iter_mut()
            .chain(frames.iter_mut())
            .chain(back_frames.iter_mut());
        in_frames.zip(out_frames).for_each(|(x, y)| {
            let x = x.as_slice_mut().unwrap();
            fft_module.process(x, y).unwrap();
        });
    }

    output
}

#[wasm_bindgen]
pub fn get_spectrogram(path: &str, px_per_sec: f32, nheight: u32) -> Vec<u8> {
    let (wav, sr) = audio::open_audio_file(path).unwrap();
    let wav = wav.sum_axis(Axis(0));
    let nwidth = (px_per_sec * wav.len() as f32 / sr as f32) as u32;
    let spec = perform_stft(wav.view(), 1920, 480, None, false);
    let mag = spec.mapv(|x| x.norm());
    let mut melspec = mag.dot(&mel::mel_filterbanks(sr, 2048, 128, 0f32, None));
    melspec.amp_to_db_default();
    let im = display::spec_to_image(
        melspec.view(),
        nwidth,
        nheight,
        *melspec.max().unwrap(),
        *melspec.min().unwrap(),
    );
    im.into_raw()
}

#[cfg(test)]
mod tests {
    use ndarray::{arr2, Array1};
    use rustfft::num_complex::Complex;

    use super::utils::Impulse;
    use super::*;

    #[test]
    fn stft_works() {
        assert_eq!(
            perform_stft(Array1::<f32>::impulse(4, 2).view(), 4, 2, None, false),
            arr2(&[
                [
                    Complex::<f32>::new(0., 0.),
                    Complex::<f32>::new(0., 0.),
                    Complex::<f32>::new(0., 0.)
                ],
                [
                    Complex::<f32>::new(1. / 4., 0.),
                    Complex::<f32>::new(-1. / 4., 0.),
                    Complex::<f32>::new(1. / 4., 0.)
                ],
                [
                    Complex::<f32>::new(1. / 4., 0.),
                    Complex::<f32>::new(-1. / 4., 0.),
                    Complex::<f32>::new(1. / 4., 0.)
                ]
            ])
        );
    }

    #[test]
    fn multitrack_works() {
        let mut multitrack = MultiTrack::new();
        multitrack.add_tracks(&[0], "samples/sample.wav").unwrap();
        multitrack.get_spec_image(0, 100., 300);
    }
}
