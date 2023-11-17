use ebur128::{EbuR128, Mode as LoudnessMode};
use ndarray::prelude::*;
use ndarray::Data;

use super::decibel::DeciBel;
use super::utils::Planes;

#[readonly::make]
#[derive(Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct AudioStats {
    pub global_lufs: f64,
    pub rms_dB: f32,
    pub max_peak: f32,
    pub max_peak_dB: f32,
}

pub struct StatCalculator(EbuR128);

impl StatCalculator {
    pub fn new(n_ch: u32, sr: u32) -> Self {
        let loudness_analyzer = EbuR128::new(n_ch, sr, LoudnessMode::all()).unwrap();
        StatCalculator(loudness_analyzer)
    }

    pub fn calc(&mut self, wavs: ArrayView2<f32>) -> AudioStats {
        self.0.reset();
        self.0.add_frames_planar_f32(&wavs.planes()).unwrap();
        let global_lufs = self.0.loudness_global().unwrap();

        let n_elem = wavs.len();
        let mean_squared = wavs.iter().map(|x| x.powi(2)).sum::<f32>() / n_elem as f32;
        #[allow(non_snake_case)]
        let rms_dB = mean_squared.dB_from_power_default();
        let max_peak = wavs.max_peak();
        #[allow(non_snake_case)]
        let max_peak_dB = max_peak.dB_from_amp_default();

        AudioStats {
            global_lufs,
            rms_dB,
            max_peak,
            max_peak_dB,
        }
    }
}

pub trait MaxPeak {
    fn max_peak(&self) -> f32;
}

impl<S, D> MaxPeak for ArrayBase<S, D>
where
    S: Data<Elem = f32>,
    D: Dimension,
{
    fn max_peak(&self) -> f32 {
        self.iter()
            .map(|x| x.abs())
            .reduce(f32::max)
            .unwrap_or_default()
    }
}
