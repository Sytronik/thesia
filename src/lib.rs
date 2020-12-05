use std::collections::HashMap;
use std::io;
use std::ops::*;

use ndarray::{prelude::*, ScalarOperand};
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
        let mut result = AudioTrack {
            path: path.to_string(),
            wav,
            sr,
            spec: None,
            fft_module: None,
        };
        result.get_or_calc_spec(setting);
        Ok(result)
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
}

#[wasm_bindgen]
pub struct MultiTrack {
    tracks: HashMap<usize, AudioTrack>,
    setting: SpecSetting,
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
            },
        }
    }

    pub fn add_tracks(&mut self, id_list: &[usize], path_list: &str) -> Result<(), JsValue> {
        for (&id, path) in id_list.iter().zip(path_list.split("\n").into_iter()) {
            self.tracks.insert(
                id,
                match AudioTrack::new(path, &self.setting) {
                    Ok(track) => track,
                    Err(err) => return Err(JsValue::from(err.to_string())),
                },
            );
        }
        Ok(())
    }

    pub fn get_spec_image(&mut self, id: usize, px_per_sec: f32, nheight: u32) -> Vec<u8> {
        let track = self.tracks.get_mut(&id).unwrap();
        let nwidth = (px_per_sec * track.wav.len() as f32 / track.sr as f32) as u32;
        let specview = track.get_or_calc_spec(&self.setting);
        display::spec_to_image(specview, nwidth, nheight).into_raw()
    }
}

pub fn perform_stft<A>(
    input: ArrayView1<A>,
    win_length: usize,
    hop_length: usize,
    fft_module: Option<&mut RealFFT<A>>,
    parallel: bool,
) -> Array2<Complex<A>>
where
    A: FFTnum + Float + MulAssign + ScalarOperand,
{
    let n_fft = 2usize.pow((win_length as f32).log2().ceil() as u32);
    let n_frames = (input.len() - win_length) / hop_length + 1;
    let n_pad_left = (n_fft - win_length) / 2;
    let n_pad_right = (((n_fft - win_length) as f32) / 2.).ceil() as usize;

    let mut window = windows::hann(win_length, false);
    // window *= A::from(1024 / win_length).unwrap();
    let mut frames: Vec<Array1<A>> = input
        .windows(win_length)
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
        .collect();

    let mut spec = Array2::<Complex<A>>::zeros((n_frames, n_fft / 2 + 1));
    let spec_view_mut: Vec<&mut [Complex<A>]> = spec
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
        frames.par_iter_mut().zip(spec_view_mut).for_each(|(x, y)| {
            let mut fft_module = RealFFT::<A>::new(n_fft).unwrap();
            let x = x.as_slice_mut().unwrap();
            fft_module.process(x, y).unwrap();
        });
    } else {
        frames.iter_mut().zip(spec_view_mut).for_each(|(x, y)| {
            let x = x.as_slice_mut().unwrap();
            fft_module.process(x, y).unwrap();
        });
    }

    spec
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
    let im = display::spec_to_image(melspec.view(), nwidth, nheight);
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
            arr2(&[[
                Complex::<f32>::new(1., 0.),
                Complex::<f32>::new(-1., 0.),
                Complex::<f32>::new(1., 0.)
            ]])
        );
    }
}
