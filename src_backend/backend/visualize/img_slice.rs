use ndarray::prelude::*;

use super::super::spectrogram::SpecSetting;

pub type IdxLen = (isize, usize);

#[readonly::make]
pub struct ArrWithSliceInfo<'a, A, D: Dimension> {
    pub arr: ArrayView<'a, A, D>,
    pub index: usize,
    pub length: usize,
}

impl<'a, A, D: Dimension> ArrWithSliceInfo<'a, A, D> {
    pub fn new(arr: ArrayView<'a, A, D>, (index, length): IdxLen) -> Self {
        let (index, length) =
            calc_effective_slice(index, length, arr.shape()[arr.ndim() - 1]).unwrap_or((0, 0));
        ArrWithSliceInfo { arr, index, length }
    }

    pub fn entire(arr: ArrayView<'a, A, D>) -> Self {
        let length = arr.shape()[arr.ndim() - 1];
        ArrWithSliceInfo {
            arr,
            index: 0,
            length,
        }
    }

    pub fn as_sliced(&self) -> ArrayView<A, D> {
        self.arr.slice_axis(
            Axis(self.arr.ndim() - 1),
            ((self.index as isize)..((self.index + self.length) as isize)).into(),
        )
    }

    pub fn as_sliced_with_tail(&self, tail: usize) -> ArrayView<A, D> {
        let end = (self.index + self.length + tail).min(self.arr.shape()[self.arr.ndim() - 1]);
        self.arr.slice_axis(
            Axis(self.arr.ndim() - 1),
            (self.index as isize..(end as isize)).into(),
        )
    }
}

impl<'a, A, D: Dimension> From<ArrayView<'a, A, D>> for ArrWithSliceInfo<'a, A, D> {
    fn from(value: ArrayView<'a, A, D>) -> Self {
        ArrWithSliceInfo::entire(value)
    }
}

#[inline]
pub fn calc_effective_slice(
    index: isize,
    length: usize,
    total_length: usize,
) -> Option<(usize, usize)> {
    if index >= total_length as isize {
        None
    } else if index < 0 {
        let i_right = length as isize + index;
        (i_right > 0).then_some((0, (i_right as usize).min(total_length)))
    } else {
        Some((index as usize, length.min(total_length - index as usize)))
    }
}

/// Heights of the overview
/// height (total) = ch + gap + ... + ch
/// ch = gain + ch_wo_gain + gain
#[readonly::make]
pub struct OverviewHeights {
    pub ch: f32,
    pub gap: f32,
    pub gain: f32,
    pub ch_wo_gain: f32,
}

impl OverviewHeights {
    pub fn new(height: f32, gap: f32, n_ch: usize, gain_height_ratio: f32) -> Self {
        let height_without_gap = height - gap * ((n_ch - 1) as f32);
        let ch = height_without_gap / n_ch as f32;
        let gain = ch * gain_height_ratio;
        let ch_wo_gain = ch - 2. * gain;
        OverviewHeights {
            ch,
            gap,
            gain,
            ch_wo_gain,
        }
    }
}

#[derive(Debug)]
#[readonly::make]
pub struct SpectrogramSliceArgs {
    pub px_per_sec: f64,
    pub left: usize,
    pub width: usize,
    pub top: usize,
    pub height: usize,
    pub left_margin: f64,
    pub right_margin: f64,
    pub top_margin: f64,
    pub bottom_margin: f64,
}

impl SpectrogramSliceArgs {
    pub fn new(
        n_frames: usize,
        n_freqs: usize,
        track_sec: f64,
        sec_range: (f64, f64),
        spec_hz_range: (f32, f32),
        hz_range: (f32, f32),
        margin_px: usize,
        spec_setting: &SpecSetting,
    ) -> Self {
        let px_per_sec = n_frames as f64 / track_sec;
        let left_f64 = sec_range.0 * px_per_sec;
        let width_f64 = ((sec_range.1 - sec_range.0) * px_per_sec).max(0.);

        let (left_w_margin_clipped, width_w_margin_clipped, left_margin, right_margin) =
            Self::calc_margin(left_f64, width_f64, n_frames, margin_px);

        let (top_f64, height_f64) = {
            let top_f64 = spec_setting
                .freq_scale
                .hz_to_relative_freq(hz_range.0, spec_hz_range) as f64
                * n_freqs as f64;
            let bottom_f64 = spec_setting
                .freq_scale
                .hz_to_relative_freq(hz_range.1, spec_hz_range) as f64
                * n_freqs as f64;
            (top_f64, bottom_f64 - top_f64)
        };

        let (top_w_margin_clipped, height_w_margin_clipped, top_margin, bottom_margin) =
            Self::calc_margin(top_f64, height_f64, n_freqs, margin_px);

        Self {
            px_per_sec,
            left: left_w_margin_clipped,
            width: width_w_margin_clipped,
            top: top_w_margin_clipped,
            height: height_w_margin_clipped,
            left_margin,
            right_margin,
            top_margin,
            bottom_margin,
        }
    }

    // TODO: refactor
    pub fn calc_margin(
        start: f64,
        length: f64,
        max_length: usize,
        margin: usize,
    ) -> (usize, usize, f64, f64) {
        let start_w_margin = start as isize - margin as isize;
        let len_w_margin =
            ((start + length).ceil() as isize + margin as isize - start_w_margin).max(0);

        let start_w_margin_clipped = start_w_margin.max(0) as usize;
        let len_w_margin_clipped =
            len_w_margin.min(max_length as isize - start_w_margin_clipped as isize) as usize;

        let pre_margin = start - start_w_margin_clipped as f64;
        let post_margin = len_w_margin_clipped as f64 - length;
        (
            start_w_margin_clipped,
            len_w_margin_clipped,
            pre_margin,
            post_margin,
        )
    }
}
