use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use napi_derive::napi;
use ndarray::prelude::*;
use rayon::prelude::*;
use realfft::{RealFftPlanner, RealToComplex};
use serde::{Deserialize, Serialize};

pub mod mel;
mod stft;

use super::windows::{calc_normalized_win, WindowType};
use crate::backend::dynamics::decibel::DeciBelInplace;
use stft::perform_stft;

const DEFAULT_WINTYPE: WindowType = WindowType::Hann;

type FramingParams = (usize, usize, usize); // hop, win, n_fft
type WinNfft = (usize, usize);
type SrNfft = (u32, usize);

#[napi(string_enum)]
#[derive(Debug, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum FreqScale {
    Linear,
    Mel,
}

impl FreqScale {
    #[inline]
    pub fn relative_freq_to_hz(&self, rel_freq: f32, hz_range: (f32, f32)) -> f32 {
        match self {
            FreqScale::Linear => (hz_range.1 - hz_range.0) * rel_freq + hz_range.0,
            FreqScale::Mel => {
                let mel_range = (mel::from_hz(hz_range.0), mel::from_hz(hz_range.1));
                mel::to_hz((mel_range.1 - mel_range.0) * rel_freq + mel_range.0)
            }
        }
    }

    #[inline]
    pub fn hz_to_relative_freq(&self, hz: f32, hz_range: (f32, f32)) -> f32 {
        match self {
            FreqScale::Linear => (hz - hz_range.0) / (hz_range.1 - hz_range.0),
            FreqScale::Mel => {
                let mel_range = (mel::from_hz(hz_range.0), mel::from_hz(hz_range.1));
                (mel::from_hz(hz) - mel_range.0) / (mel_range.1 - mel_range.0)
            }
        }
    }

    #[inline]
    fn calc_ratio_to_max_freq(&self, hz: f32, sr: u32) -> f32 {
        let half_sr = sr as f32 / 2.;
        match self {
            FreqScale::Linear => hz / half_sr,
            FreqScale::Mel => mel::from_hz(hz) / mel::from_hz(half_sr),
        }
    }

    #[inline]
    pub fn hz_range_to_idx(
        &self,
        hz_range: (f32, f32),
        sr: u32,
        n_freqs_or_mels: usize,
    ) -> (usize, usize) {
        if hz_range.0 >= hz_range.1 {
            return (0, 0);
        }
        let min_ratio = self.calc_ratio_to_max_freq(hz_range.0, sr);
        let max_ratio = self.calc_ratio_to_max_freq(hz_range.1, sr);
        let min_idx = ((min_ratio * n_freqs_or_mels as f32).floor() as usize).max(0);
        let max_idx = (max_ratio * n_freqs_or_mels as f32).ceil() as usize;

        (min_idx, max_idx)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SrWinNfft {
    pub sr: u32,
    pub win_length: usize,
    pub n_fft: usize,
}

#[napi(object)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpecSetting {
    #[napi(js_name = "winMillisec")]
    pub win_ms: f64,
    pub t_overlap: u32,
    pub f_overlap: u32,
    pub freq_scale: FreqScale,
}

impl Default for SpecSetting {
    fn default() -> Self {
        Self {
            win_ms: 40.,
            t_overlap: 4,
            f_overlap: 1,
            freq_scale: FreqScale::Mel,
        }
    }
}

impl SpecSetting {
    #[inline]
    pub fn calc_win_length(&self, sr: u32) -> usize {
        self.calc_hop_length(sr) * self.t_overlap as usize
    }

    #[inline]
    pub fn calc_hop_length(&self, sr: u32) -> usize {
        (self.calc_win_length_float(sr) / self.t_overlap as f64).round() as usize
    }

    #[inline]
    pub fn calc_framing_params(&self, sr: u32) -> FramingParams {
        let hop_length = self.calc_hop_length(sr);
        let win_length = self.calc_win_length_from_hop_length(hop_length);
        let n_fft = self.calc_n_fft_from_win_length(win_length);
        (hop_length, win_length, n_fft)
    }

    #[inline]
    pub fn calc_sr_win_nfft(&self, sr: u32) -> SrWinNfft {
        let win_length = self.calc_win_length(sr);
        let n_fft = self.calc_n_fft_from_win_length(win_length);
        SrWinNfft {
            sr,
            win_length,
            n_fft,
        }
    }

    #[inline]
    fn calc_win_length_from_hop_length(&self, hop_length: usize) -> usize {
        hop_length * self.t_overlap as usize
    }

    #[inline]
    fn calc_win_length_float(&self, sr: u32) -> f64 {
        self.win_ms * sr as f64 / 1000.
    }

    #[inline]
    fn calc_n_fft_from_win_length(&self, win_length: usize) -> usize {
        win_length.next_power_of_two() * self.f_overlap as usize
    }
}

pub struct SpectrogramAnalyzer {
    windows: HashMap<WinNfft, Array1<f32>>,
    fft_modules: HashMap<usize, Arc<dyn RealToComplex<f32>>>,
    mel_fbs: HashMap<SrNfft, Array2<f32>>,
}

impl SpectrogramAnalyzer {
    pub fn new() -> Self {
        SpectrogramAnalyzer {
            windows: HashMap::new(),
            fft_modules: HashMap::new(),
            mel_fbs: HashMap::new(),
        }
    }

    pub fn prepare(&mut self, params: &HashSet<SrWinNfft>, freq_scale: FreqScale) {
        let mut real_fft_planner = RealFftPlanner::<f32>::new();
        let entries: Vec<_> = params
            .par_iter()
            .filter_map(|param| {
                let k = (param.win_length, param.n_fft);
                if !self.windows.contains_key(&k) {
                    let v = calc_normalized_win(DEFAULT_WINTYPE, param.win_length, param.n_fft);
                    Some((k, v))
                } else {
                    None
                }
            })
            .collect();
        self.windows.extend(entries);
        for &param in params.iter() {
            self.fft_modules
                .entry(param.n_fft)
                .or_insert_with(|| real_fft_planner.plan_fft_forward(param.n_fft));
        }
        if let FreqScale::Mel = freq_scale {
            let entries: Vec<_> = params
                .par_iter()
                .filter_map(|&param| {
                    let k = (param.sr, param.n_fft);
                    if !self.mel_fbs.contains_key(&k) {
                        let v = mel::calc_mel_fb_default(param.sr, param.n_fft);
                        Some((k, v))
                    } else {
                        None
                    }
                })
                .collect();
            self.mel_fbs.extend(entries);
        } else {
            self.mel_fbs.clear();
        }
    }

    pub fn retain(&mut self, params: &HashSet<SrWinNfft>, freq_scale: FreqScale) {
        self.windows.retain(|&(win_length, n_fft), _| {
            params
                .iter()
                .any(|&param| win_length == param.win_length && n_fft == param.n_fft)
        });
        self.fft_modules
            .retain(|&n_fft, _| params.iter().any(|&param| n_fft == param.n_fft));
        if freq_scale == FreqScale::Mel {
            self.mel_fbs.retain(|&(sr, n_fft), _| {
                params
                    .iter()
                    .any(|&param| sr == param.sr && n_fft == param.n_fft)
            });
        } else {
            self.mel_fbs.clear();
        }
    }

    pub fn calc_spec(
        &self,
        wav: ArrayView1<f32>,
        sr: u32,
        setting: &SpecSetting,
        parallel: bool,
    ) -> Array2<f32> {
        let (hop_length, win_length, n_fft) = setting.calc_framing_params(sr);
        let window = self.window(win_length, n_fft);
        let fft_module = self.fft_module(n_fft);
        let stft = perform_stft(
            wav,
            win_length,
            hop_length,
            n_fft,
            Some(window),
            Some(fft_module),
            parallel,
        );
        let mut linspec = stft.mapv(|x| x.norm());
        match setting.freq_scale {
            FreqScale::Linear => {
                linspec.dB_from_amp_inplace_default();
                linspec
            }
            FreqScale::Mel => {
                let mut melspec = linspec.dot(&self.mel_fb(sr, n_fft));
                melspec.dB_from_amp_inplace_default();
                melspec
            }
        }
    }

    fn window(&self, win_length: usize, n_fft: usize) -> CowArray<f32, Ix1> {
        self.windows.get(&(win_length, n_fft)).map_or_else(
            || {
                eprintln!(
                    "AnalysisParamManager hasn't prepare a window for ({}, {})!",
                    win_length, n_fft
                );
                calc_normalized_win(DEFAULT_WINTYPE, win_length, n_fft).into()
            },
            |a| a.view().into(),
        )
    }

    fn fft_module(&self, n_fft: usize) -> Arc<dyn RealToComplex<f32>> {
        self.fft_modules.get(&n_fft).map_or_else(
            || {
                eprintln!(
                    "AnalysisParamManager hasn't prepare a fft module for n_fft {}!",
                    n_fft
                );
                let mut real_fft_planner = RealFftPlanner::<f32>::new();
                real_fft_planner.plan_fft_forward(n_fft)
            },
            |m| Arc::clone(m),
        )
    }

    fn mel_fb(&self, sr: u32, n_fft: usize) -> CowArray<f32, Ix2> {
        self.mel_fbs.get(&(sr, n_fft)).map_or_else(
            || {
                eprintln!(
                    "AnalysisParamManager hasn't prepare a mel filterbank for ({}, {})!",
                    sr, n_fft
                );
                mel::calc_mel_fb_default(sr, n_fft).into()
            },
            |a| a.view().into(),
        )
    }
}
