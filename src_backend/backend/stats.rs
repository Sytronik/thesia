use ebur128::{EbuR128, Mode as LoudnessMode};
use ndarray::prelude::*;
use readonly;

use super::normalize::MaxPeak;
use super::utils::Planes;

#[readonly::make]
#[derive(Clone, PartialEq)]
pub struct AudioStats {
    pub global_lufs: f64,
    pub rms_db: f32,
    pub max_peak: f32,
    pub max_peak_db: f32,
}

pub struct StatCalculator(EbuR128);

impl StatCalculator {
    pub fn new(n_ch: u32, sr: u32) -> Self {
        let loudness_analyzer = EbuR128::new(n_ch as u32, sr, LoudnessMode::all()).unwrap();
        StatCalculator(loudness_analyzer)
    }

    pub fn calc(&mut self, wavs: ArrayView2<f32>) -> AudioStats {
        self.0.reset();
        self.0.add_frames_planar_f32(&wavs.planes()).unwrap();
        let global_lufs = self.0.loudness_global().unwrap();

        let n_elem = wavs.len();
        let mean_squared = wavs.iter().map(|x| x.powi(2)).sum::<f32>() / n_elem as f32;
        let rms_db = 10. * mean_squared.log10(); // TODO: numerical stability
        let max_peak = wavs.max_peak();
        let max_peak_db = 20. * max_peak.log10();

        AudioStats {
            global_lufs,
            rms_db,
            max_peak,
            max_peak_db,
        }
    }
}