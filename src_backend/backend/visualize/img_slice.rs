use ndarray::prelude::*;

use super::super::audio::Audio;

pub type LeftWidth = (u32, u32);
pub type IdxLen = (isize, usize);

#[derive(PartialEq, Debug)]
pub struct PartGreyInfo {
    pub i_w_and_width: IdxLen,
    pub start_sec_with_margin: f64,
    pub width_with_margin: u32,
}

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

pub trait CalcWidth {
    fn calc_width(&self, px_per_sec: f64) -> u32;
    fn calc_part_grey_info(
        &self,
        grey_width: u64,
        start_sec: f64,
        target_width: u32,
        px_per_sec: f64,
    ) -> PartGreyInfo;

    fn calc_part_wav_info(&self, start_sec: f64, width: u32, px_per_sec: f64) -> IdxLen;

    fn decompose_width_of(&self, start_sec: f64, width: u32, px_per_sec: f64) -> (u32, u32, u32);
}

impl CalcWidth for Audio {
    #[inline]
    fn calc_width(&self, px_per_sec: f64) -> u32 {
        ((px_per_sec * self.len() as f64 / self.sr as f64).round() as u32).max(1)
    }

    fn calc_part_grey_info(
        &self,
        grey_width: u64,
        start_sec: f64,
        target_width: u32,
        px_per_sec: f64,
    ) -> PartGreyInfo {
        let wavlen = self.len() as f64;
        let sr = self.sr as u64;
        let grey_px_per_sec = (grey_width * sr) as f64 / wavlen;
        let left = start_sec * grey_px_per_sec;
        let left_floor = left.floor();
        let i_w = left_floor as isize;
        let width_f64 = target_width as f64 * grey_px_per_sec / px_per_sec;
        let width = ((left + width_f64).ceil() as isize - i_w).max(1) as usize;
        let start_sec_with_margin = left_floor / grey_px_per_sec;
        let target_width_with_margin = (width as f64 / grey_px_per_sec * px_per_sec).round() as u32;
        PartGreyInfo {
            i_w_and_width: (i_w, width),
            start_sec_with_margin,
            width_with_margin: target_width_with_margin,
        }
    }

    fn calc_part_wav_info(&self, start_sec: f64, width: u32, px_per_sec: f64) -> IdxLen {
        let i = (start_sec * self.sr as f64).round() as isize;
        let length = ((self.sr as u64 * width as u64) as f64 / px_per_sec).round() as usize;
        (i, length)
    }

    fn decompose_width_of(&self, start_sec: f64, width: u32, px_per_sec: f64) -> (u32, u32, u32) {
        let total_width = (px_per_sec * self.len() as f64 / self.sr as f64).max(1.);
        let pad_left = ((-start_sec * px_per_sec).max(0.).round() as u32).min(width);
        let pad_right = ((start_sec.mul_add(px_per_sec, width as f64) - total_width)
            .max(0.)
            .round() as u32)
            .min(width - pad_left);

        (pad_left, width - pad_left - pad_right, pad_right)
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

#[readonly::make]
pub struct OverviewHeights {
    pub total: usize,
    pub ch: usize,
    pub gap: usize,
    pub margin: usize,
}

impl OverviewHeights {
    pub fn new(height: u32, n_ch: usize, gap: f32, dpr: f32) -> Self {
        let total = height as usize;
        let gap = (gap * dpr).round() as usize;
        let height_without_gap = total - gap * (n_ch - 1);
        let ch = height_without_gap / n_ch;
        let margin = height_without_gap % n_ch / 2;
        OverviewHeights {
            total,
            ch,
            gap,
            margin,
        }
    }

    #[inline]
    pub fn ch_and_gap(&self) -> usize {
        self.ch + self.gap
    }

    #[inline]
    pub fn decompose_by_gain(&self, gain_height_denom: usize) -> (usize, usize) {
        let gain_h = self.ch / gain_height_denom;
        (gain_h, self.ch - 2 * gain_h)
    }
}
