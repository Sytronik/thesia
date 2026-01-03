use std::sync::Arc;

use identity_hash::IntMap;
use ndarray::prelude::*;
use rayon::prelude::*;
use realfft::{RealFftPlanner, RealToComplex};
use serde::{Deserialize, Serialize};

mod stft;

use super::dynamics::decibel::DeciBelInplace;
use super::tuple_hasher::{TupleIntMap, TupleIntSet};
use super::windows::{WindowType, calc_normalized_win};
use stft::perform_stft;
use thesia_common::{self as mel, FreqScale};

const DEFAULT_WINTYPE: WindowType = WindowType::Hann;

type FramingParams = (usize, usize, usize); // hop, win, n_fft
type WinNfft = (usize, usize);
type SrNfft = (u32, usize);

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SrWinNfft {
    pub sr: u32,
    pub win_length: usize,
    pub n_fft: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpecSetting {
    #[serde(rename = "winMillisec")]
    pub win_ms: f64,
    pub t_overlap: u32,
    pub f_overlap: u32,
    pub freq_scale: FreqScale,
}

impl Default for SpecSetting {
    fn default() -> Self {
        Self::new()
    }
}

impl SpecSetting {
    pub const fn new() -> Self {
        Self {
            win_ms: 40.,
            t_overlap: 4,
            f_overlap: 1,
            freq_scale: FreqScale::Mel,
        }
    }

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
    windows: TupleIntMap<WinNfft, Array1<f32>>,
    fft_modules: IntMap<usize, Arc<dyn RealToComplex<f32>>>,
    mel_fbs: TupleIntMap<SrNfft, Array2<f32>>,
}

impl SpectrogramAnalyzer {
    pub fn new() -> Self {
        SpectrogramAnalyzer {
            windows: TupleIntMap::with_capacity_and_hasher(1, Default::default()),
            fft_modules: IntMap::with_capacity_and_hasher(1, Default::default()),
            mel_fbs: TupleIntMap::with_capacity_and_hasher(1, Default::default()),
        }
    }

    pub fn prepare(&mut self, params: &TupleIntSet<SrWinNfft>, freq_scale: FreqScale) {
        let mut real_fft_planner = RealFftPlanner::<f32>::new();
        let new_windows: Vec<_> = params
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
        self.windows.extend(new_windows);
        for param in params.iter() {
            self.fft_modules
                .entry(param.n_fft)
                .or_insert_with(|| real_fft_planner.plan_fft_forward(param.n_fft));
        }
        if let FreqScale::Mel = freq_scale {
            let new_mel_fbs: Vec<_> = params
                .par_iter()
                .filter_map(|param| {
                    let k = (param.sr, param.n_fft);
                    if !self.mel_fbs.contains_key(&k) {
                        let v = mel::calc_mel_fb_default(param.sr, param.n_fft);
                        Some((k, v))
                    } else {
                        None
                    }
                })
                .collect();
            self.mel_fbs.extend(new_mel_fbs);
        } else {
            self.mel_fbs.clear();
            self.mel_fbs.shrink_to_fit();
        }
    }

    pub fn retain(&mut self, params: &TupleIntSet<SrWinNfft>, freq_scale: FreqScale) {
        self.windows.retain(|&(win_length, n_fft), _| {
            params
                .iter()
                .any(|param| win_length == param.win_length && n_fft == param.n_fft)
        });
        if self.windows.capacity() > 2 * self.windows.len() {
            self.windows.shrink_to(1);
        }

        self.fft_modules
            .retain(|&n_fft, _| params.iter().any(|param| n_fft == param.n_fft));
        if self.fft_modules.capacity() > 2 * self.fft_modules.len() {
            self.fft_modules.shrink_to(1);
        }

        if freq_scale == FreqScale::Mel {
            self.mel_fbs.retain(|&(sr, n_fft), _| {
                params
                    .iter()
                    .any(|param| sr == param.sr && n_fft == param.n_fft)
            });
            if self.mel_fbs.capacity() > 2 * self.mel_fbs.len() {
                self.mel_fbs.shrink_to(1);
            }
        } else {
            self.mel_fbs.clear();
            self.mel_fbs.shrink_to_fit();
        }
    }

    pub fn calc_spec(
        &self,
        wav: &ArrayRef1<f32>,
        sr: u32,
        setting: &SpecSetting,
        parallel: bool,
    ) -> Array2<f32> {
        let (hop_length, win_length, n_fft) = setting.calc_framing_params(sr);
        let window = self.window(win_length, n_fft);
        let fft_module = self.fft_module(n_fft);
        let stft = perform_stft(
            wav, win_length, hop_length, n_fft, window, fft_module, parallel,
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

    fn window(&'_ self, win_length: usize, n_fft: usize) -> CowArray<'_, f32, Ix1> {
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

    fn mel_fb(&'_ self, sr: u32, n_fft: usize) -> CowArray<'_, f32, Ix2> {
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
