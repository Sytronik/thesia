use ebur128::{EbuR128, Mode as LoudnessMode};
use ndarray::prelude::*;
use ndarray::{Data, RemoveAxis};
use ndarray_stats::{MaybeNan, QuantileExt};
use num_traits::{AsPrimitive, Float};

use super::decibel::DeciBel;
use super::normalize::GuardClippingResult;
use crate::backend::utils::Planes;

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

pub trait MaxPeak<A> {
    fn max_peak(&self) -> A;
}

impl<A, S, D> MaxPeak<A> for ArrayBase<S, D>
where
    A: Float + MaybeNan,
    <A as MaybeNan>::NotNan: Ord,
    S: Data<Elem = A>,
    D: Dimension,
{
    fn max_peak(&self) -> A {
        self.iter()
            .map(|x| x.abs())
            .reduce(Float::max)
            .unwrap_or(A::zero())
    }
}

#[derive(Clone, Default, PartialEq)]
#[allow(non_snake_case)]
pub struct GuardClippingStats {
    pub max_reduction_gain_dB: f32,
    pub reduction_cnt: usize,
}

impl GuardClippingStats {
    pub fn from_wav_before_clip<A, D>(wav_before_clip: ArrayView<A, D>) -> Self
    where
        A: Float + MaybeNan + DeciBel + AsPrimitive<f32>,
        <A as MaybeNan>::NotNan: Ord,
        D: Dimension,
    {
        let max_reduction_gain = wav_before_clip.max_peak().recip();
        GuardClippingStats {
            max_reduction_gain_dB: max_reduction_gain.dB_from_amp_default().as_(),
            reduction_cnt: wav_before_clip
                .iter()
                .filter(|x| x.abs() > A::one())
                .count(),
        }
    }

    pub fn from_global_gain(gain: f32, len: usize) -> Self {
        assert!(len > 0);
        GuardClippingStats {
            max_reduction_gain_dB: gain.dB_from_amp_default(),
            reduction_cnt: len,
        }
    }

    pub fn from_gain_seq<A, D>(gain_seq: ArrayView<A, D>) -> Self
    where
        A: Float + MaybeNan + DeciBel + AsPrimitive<f32>,
        <A as MaybeNan>::NotNan: Ord,
        D: Dimension,
    {
        GuardClippingStats {
            max_reduction_gain_dB: gain_seq.min_skipnan().dB_from_amp_default().as_(),
            reduction_cnt: gain_seq.iter().filter(|&&x| x != A::one()).count(),
        }
    }
}

impl<D: Dimension + RemoveAxis> From<&GuardClippingResult<D>>
    for Array<GuardClippingStats, D::Smaller>
{
    fn from(value: &GuardClippingResult<D>) -> Self {
        match value {
            GuardClippingResult::WavBeforeClip(before_clip) => {
                let raw_dim = before_clip.raw_dim();
                let vec = before_clip
                    .lanes(Axis(raw_dim.ndim() - 1))
                    .into_iter()
                    .map(GuardClippingStats::from_wav_before_clip)
                    .collect();
                Array::from_shape_vec(raw_dim.remove_axis(Axis(raw_dim.ndim() - 1)), vec).unwrap()
            }
            GuardClippingResult::GlobalGain((gain, raw_dim)) => Array::from_elem(
                raw_dim.remove_axis(Axis(raw_dim.ndim() - 1)),
                GuardClippingStats::from_global_gain(*gain, 1),
            ),
            GuardClippingResult::GainSequence(gain_seq) => {
                let raw_dim = gain_seq.raw_dim();
                let vec = gain_seq
                    .lanes(Axis(raw_dim.ndim() - 1))
                    .into_iter()
                    .map(GuardClippingStats::from_gain_seq)
                    .collect();
                Array::from_shape_vec(raw_dim.remove_axis(Axis(raw_dim.ndim() - 1)), vec).unwrap()
            }
        }
    }
}

impl<D: Dimension + RemoveAxis> From<GuardClippingResult<D>>
    for Array<GuardClippingStats, D::Smaller>
{
    fn from(value: GuardClippingResult<D>) -> Self {
        (&value).into()
    }
}