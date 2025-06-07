use std::fmt::Display;

use ebur128::{EbuR128, Mode as LoudnessMode};
use ndarray::prelude::*;
use ndarray::{Data, RemoveAxis};
use ndarray_stats::{MaybeNan, QuantileExt};
use num_traits::{AsPrimitive, Float};
use rayon::prelude::*;

use super::super::simd::sum_squares;
use super::super::utils::Planes;

use super::decibel::DeciBel;
use super::guardclipping::GuardClippingResult;

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

    pub fn change_parameters(&mut self, n_ch: u32, sr: u32) {
        self.0.change_parameters(n_ch, sr).unwrap();
    }

    pub fn calc(&mut self, wavs: ArrayView2<f32>) -> AudioStats {
        self.0.reset();
        let (global_lufs, mean_squared) = rayon::join(
            || {
                self.0.add_frames_planar_f32(&wavs.planes()).unwrap();
                self.0.loudness_global().unwrap()
            },
            || {
                let n_elem = wavs.len();
                wavs.axis_iter(Axis(0))
                    .into_par_iter()
                    .map(|x| sum_squares(x.as_slice().unwrap()))
                    .sum::<f32>()
                    / n_elem as f32
            },
        );

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

impl Display for GuardClippingStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.max_reduction_gain_dB == 0. {
            write!(f, "")
        } else if self.reduction_cnt == 0 {
            write!(f, "{:.2} dB", self.max_reduction_gain_dB)
        } else {
            write!(
                f,
                "max {:.2} dB, total {} samples", // TODO: beter formatting
                self.max_reduction_gain_dB, self.reduction_cnt
            )
        }
    }
}

impl GuardClippingStats {
    pub fn from_wav_before_clip<A, D>(wav_before_clip: ArrayView<A, D>) -> Self
    where
        A: Float + MaybeNan + DeciBel + AsPrimitive<f32>,
        <A as MaybeNan>::NotNan: Ord,
        D: Dimension,
    {
        let max_peak = wav_before_clip.max_peak();
        if max_peak > A::one() {
            GuardClippingStats {
                max_reduction_gain_dB: max_peak.recip().dB_from_amp_default().as_(),
                reduction_cnt: wav_before_clip
                    .iter()
                    .filter(|x| x.abs() > A::one())
                    .count(),
            }
        } else {
            Default::default()
        }
    }

    pub fn from_global_gain(gain: f32) -> Self {
        GuardClippingStats {
            max_reduction_gain_dB: gain.dB_from_amp_default(),
            reduction_cnt: 0,
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
                    .axis_iter(Axis(0))
                    .into_par_iter()
                    .map(GuardClippingStats::from_wav_before_clip)
                    .collect();
                Array::from_shape_vec(raw_dim.remove_axis(Axis(raw_dim.ndim() - 1)), vec).unwrap()
            }
            GuardClippingResult::GlobalGain((gain, raw_dim)) => Array::from_elem(
                raw_dim.remove_axis(Axis(raw_dim.ndim() - 1)),
                GuardClippingStats::from_global_gain(*gain),
            ),
            GuardClippingResult::GainSequence(gain_seq) => {
                let raw_dim = gain_seq.raw_dim();
                let vec = gain_seq
                    .axis_iter(Axis(0))
                    .into_par_iter()
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
