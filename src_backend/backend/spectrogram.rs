use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use napi::bindgen_prelude::{FromNapiValue, ToNapiValue};
use napi_derive::napi;
use ndarray::prelude::*;
use rayon::prelude::*;
use realfft::{RealFftPlanner, RealToComplex};
use serde::{Deserialize, Serialize};

pub mod mel;
mod stft;

pub use super::windows::{calc_normalized_win, WindowType};
pub use stft::perform_stft;

const DEFAULT_WINTYPE: WindowType = WindowType::Hann;

#[napi(string_enum)]
#[derive(Debug, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub enum FreqScale {
    Linear,
    Mel,
}

#[inline]
pub fn calc_up_ratio(sr: u32, max_sr: u32, freq_scale: FreqScale) -> f32 {
    match freq_scale {
        FreqScale::Linear => max_sr as f32 / sr as f32,
        FreqScale::Mel => mel::from_hz(max_sr as f32 / 2.) / mel::from_hz(sr as f32 / 2.),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SrWinNfft {
    pub sr: u32,
    pub win_length: usize,
    pub n_fft: usize,
}

pub struct AnalysisParamManager {
    windows: HashMap<(usize, usize), Array1<f32>>,
    fft_modules: HashMap<usize, Arc<dyn RealToComplex<f32>>>,
    mel_fbs: HashMap<(u32, usize), Array2<f32>>,
}

impl AnalysisParamManager {
    pub fn new() -> Self {
        AnalysisParamManager {
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

    pub fn get_window(&self, win_length: usize, n_fft: usize) -> CowArray<f32, Ix1> {
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

    pub fn get_fft_module(&self, n_fft: usize) -> Arc<dyn RealToComplex<f32>> {
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

    pub fn get_mel_fb(&self, sr: u32, n_fft: usize) -> CowArray<f32, Ix2> {
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
