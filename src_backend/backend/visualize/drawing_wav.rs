// use std::time::Instant;
use std::ops::Neg;
use std::slice::SliceIndex;

use approx::relative_ne;
use cached::proc_macro::cached;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use rayon::prelude::*;

use super::super::dynamics::{GuardClippingResult, MaxPeak};
use super::super::track::AudioTrack;

use super::WavSliceArgs;
use super::resample::FftResampler;
use super::slice_args::{ArrWithSliceInfo, OverviewHeights};

const RESAMPLE_TAIL: usize = 500;
const THR_TOPBOTTOM_PERCENT: usize = 70;

const OVERVIEW_MAX_CH: usize = 4;

#[derive(Default)]
pub enum WavDrawingInfoInternal {
    #[default]
    FillRect,
    Line(Vec<f32>, Option<(f32, f32)>),
    TopBottomEnvelope(Vec<f32>, Vec<f32>, Option<(f32, f32)>),
}

impl WavDrawingInfoInternal {
    pub fn len(&self) -> usize {
        match self {
            WavDrawingInfoInternal::FillRect => 0,
            WavDrawingInfoInternal::Line(line, ..) => line.len(),
            WavDrawingInfoInternal::TopBottomEnvelope(top, ..) => top.len(),
        }
    }

    pub fn slice(
        &self,
        range: impl SliceIndex<[f32], Output = [f32]> + Clone,
    ) -> WavDrawingInfoInternal {
        match self {
            WavDrawingInfoInternal::FillRect => WavDrawingInfoInternal::FillRect,
            WavDrawingInfoInternal::Line(..) => {
                unimplemented!();
            }
            WavDrawingInfoInternal::TopBottomEnvelope(top, bottom, clip_values) => {
                WavDrawingInfoInternal::TopBottomEnvelope(
                    top[range.clone()].to_owned(),
                    bottom[range].to_owned(),
                    clip_values.clone(),
                )
            }
        }
    }

    pub fn convert_amp_range(
        &self,
        orig_amp_range: (f32, f32),
        target_amp_range: (f32, f32),
        height: f32,
        wav_stroke_width: f32,
    ) -> Self {
        let amp_to_rel_y = get_amp_to_rel_y_fn(target_amp_range);
        let orig_scale = orig_amp_range.1 - orig_amp_range.0;
        let convert = |orig_rel_y: f32| amp_to_rel_y(orig_amp_range.1 - orig_rel_y * orig_scale);
        let need_convert = relative_ne!(orig_amp_range.0, target_amp_range.0)
            && relative_ne!(orig_amp_range.1, target_amp_range.1);

        match self {
            WavDrawingInfoInternal::FillRect => WavDrawingInfoInternal::FillRect,
            WavDrawingInfoInternal::Line(..) => {
                unimplemented!();
            }
            WavDrawingInfoInternal::TopBottomEnvelope(
                orig_top_envlop,
                orig_btm_envlop,
                orig_clip_values,
            ) => {
                let zero_top = amp_to_rel_y(0.) - wav_stroke_width / height / 2.;
                let zero_btm = amp_to_rel_y(0.) + wav_stroke_width / height / 2.;

                let handle_zero = move |(top, btm)| {
                    let is_larger_than_stroke_width = btm - top >= wav_stroke_width / height;
                    if is_larger_than_stroke_width {
                        (top, btm)
                    } else {
                        (zero_top, zero_btm)
                    }
                };
                let (top_envlop, btm_envlop) = if need_convert {
                    orig_top_envlop
                        .iter()
                        .copied()
                        .map(convert)
                        .zip(orig_btm_envlop.iter().copied().map(convert))
                        .map(handle_zero)
                        .unzip() // TODO: use SIMD
                } else {
                    orig_top_envlop
                        .iter()
                        .copied()
                        .zip(orig_btm_envlop.iter().copied())
                        .map(handle_zero)
                        .unzip() // TODO: use SIMD
                };
                let clip_values = orig_clip_values.map(|(top, btm)| (convert(top), convert(btm)));

                WavDrawingInfoInternal::TopBottomEnvelope(top_envlop, btm_envlop, clip_values)
            }
        }
    }
}

// #[readonly::make]
pub struct WavDrawingInfoCache {
    pub wav_stroke_width: f32,
    pub topbottom_context_size: f32,
    pub px_per_sec: f32,
    pub drawing_infos: Vec<WavDrawingInfoInternal>,
    pub amp_ranges: Vec<(f32, f32)>,
}

impl WavDrawingInfoCache {
    pub fn slice(
        &self,
        ch: usize,
        sec_range: (f64, f64),
        track_sec: f64,
        height: f32,
        amp_range: (f32, f32),
        wav_stroke_width: f32,
        margin_ratio: f64,
    ) -> SlicedWavDrawingInfo {
        let drawing_info = &self.drawing_infos[ch];
        let cache_len = drawing_info.len();

        let slice_args =
            WavSliceArgs::from_cache_len(cache_len, sec_range, track_sec, margin_ratio);

        if slice_args.start_w_margin >= cache_len {
            return Default::default();
        }
        let drawing_info_sliced = drawing_info.slice(
            slice_args.start_w_margin..(slice_args.start_w_margin + slice_args.length_w_margin),
        );

        // convert amp_range
        let drawing_info_sliced = drawing_info_sliced.convert_amp_range(
            self.amp_ranges[ch],
            amp_range,
            height,
            wav_stroke_width,
        );

        SlicedWavDrawingInfo {
            drawing_info: drawing_info_sliced,
            drawing_sec: slice_args.drawing_sec,
            pre_margin_sec: slice_args.pre_margin_sec,
            post_margin_sec: slice_args.post_margin_sec,
        } // return cache
    }
}

/// default value means no drawing (sliced wav is empty)
#[derive(Default)]
pub struct SlicedWavDrawingInfo {
    pub drawing_info: WavDrawingInfoInternal,
    pub drawing_sec: f64,
    pub pre_margin_sec: f64,
    pub post_margin_sec: f64,
}

impl WavDrawingInfoInternal {
    pub fn new(
        wav: ArrWithSliceInfo<f32, Ix1>,
        width: f32,
        height: f32,
        amp_range: (f32, f32),
        wav_stroke_width: f32,
        topbottom_context_size: f32,
        show_clipping: bool,
    ) -> WavDrawingInfoInternal {
        let thr_long_height = wav_stroke_width / height;
        let amp_to_rel_y = get_amp_to_rel_y_fn(amp_range);
        let px_per_samples = width / wav.length as f32;
        let resample_ratio = quantize_px_per_samples(px_per_samples);
        let outline_len = (wav.length as f32 * resample_ratio).round() as usize;
        let clip_values = (show_clipping && (amp_range.0 < -1. || amp_range.1 > 1.))
            .then_some((amp_to_rel_y(1.), amp_to_rel_y(-1.)));

        if amp_range.1 - amp_range.0 < 1e-16 {
            // over-zoomed
            WavDrawingInfoInternal::FillRect
        } else if resample_ratio > 0.5 {
            // upsampling
            let mut resampler;
            let wav = if resample_ratio != 1. {
                let wav_tail = wav.as_sliced_with_tail(RESAMPLE_TAIL);
                let upsampled_len_tail = (wav_tail.len() as f32 * resample_ratio).round();
                resampler = create_resampler(wav_tail.len(), upsampled_len_tail as usize);
                resampler.resample(wav_tail)
            } else {
                wav.as_sliced()
            };
            WavDrawingInfoInternal::Line(
                wav.slice(s![..outline_len])
                    .into_par_iter()
                    .with_min_len(outline_len / rayon::current_num_threads())
                    .map(|&x| amp_to_rel_y(x))
                    .collect(), // TODO: benchmark parallel iterator, use SIMD
                clip_values,
            )
        } else {
            let wav = wav.as_sliced();
            let half_context_size = topbottom_context_size / 2.;
            let mean_rel_y = amp_to_rel_y(wav.mean().unwrap_or(0.));
            let zero_rel_y = amp_to_rel_y(0.);
            let zero_top = zero_rel_y - wav_stroke_width / height / 2.;
            let zero_btm = zero_rel_y + wav_stroke_width / height / 2.;
            let result: Vec<_> = (0..outline_len)
                .into_par_iter()
                .with_min_len(outline_len / rayon::current_num_threads())
                .map(|i_envlop| {
                    let i_envlop = i_envlop as f32;
                    let i_start = ((i_envlop - half_context_size) / resample_ratio)
                        .round()
                        .max(0.) as usize;
                    let i_end = (((i_envlop + half_context_size) / resample_ratio).round()
                        as usize)
                        .min(wav.len());
                    let wav_slice = wav.slice(s![i_start..i_end]);
                    let top = amp_to_rel_y(*wav_slice.max_skipnan());
                    let bottom = amp_to_rel_y(*wav_slice.min_skipnan());
                    let is_mean_crossing = top < mean_rel_y + f32::EPSILON
                        && bottom > mean_rel_y - thr_long_height
                        || top < mean_rel_y + thr_long_height && bottom > mean_rel_y - f32::EPSILON;
                    let is_larger_than_stroke_width = bottom - top >= wav_stroke_width / height;
                    if is_larger_than_stroke_width {
                        (top, bottom, is_mean_crossing)
                    } else {
                        (zero_top, zero_btm, false)
                    }
                })
                .collect(); // TODO: benchmark parallel iterator, use SIMD
            let n_mean_crossing = result
                .iter()
                .filter(|(_, _, is_mean_crossing)| *is_mean_crossing)
                .count();
            if n_mean_crossing > outline_len * THR_TOPBOTTOM_PERCENT / 100 {
                let (top_envlop, btm_envlop) = result
                    .into_iter()
                    .map(|(top, bottom, _)| (top, bottom))
                    .unzip();
                WavDrawingInfoInternal::TopBottomEnvelope(top_envlop, btm_envlop, clip_values)
            } else {
                WavDrawingInfoInternal::Line(
                    wav.into_par_iter()
                        .with_min_len(wav.len() / rayon::current_num_threads())
                        .map(|&x| amp_to_rel_y(x))
                        .collect(), // TODO: benchmark parallel iterator, use SIMD
                    clip_values,
                )
            }
        }
    }
}

pub fn calc_limiter_gain_drawing_info(
    gain: ArrayView1<f32>,
    width: u32,
    amp_range: (f32, f32),
    topbottom_context_size: f32,
) -> WavDrawingInfoInternal {
    let half_context_size = topbottom_context_size / 2.;
    let amp_to_rel_y = get_amp_to_rel_y_fn(amp_range);
    let samples_per_px = gain.len() as f32 / width as f32;

    let envlop_iter = (0..width).map(|i_px| {
        let i_px = i_px as f32;
        let i_mid = ((i_px * samples_per_px).round() as usize).min(gain.len() - 1);
        if gain[i_mid.max(1) - 1] == gain[i_mid]
            || gain[i_mid] == gain[i_mid.min(gain.len() - 2) + 1]
        {
            amp_to_rel_y(gain[i_mid])
        } else {
            let i_start = ((i_px - half_context_size) * samples_per_px)
                .round()
                .max(0.) as usize;
            let i_end =
                (((i_px + half_context_size) * samples_per_px).round() as usize).min(gain.len());
            amp_to_rel_y(gain.slice(s![i_start..i_end]).mean().unwrap_or_default())
        }
    });

    let top_px = amp_to_rel_y(amp_range.1);
    let btm_px = amp_to_rel_y(amp_range.0);
    if amp_range.1 > 0. {
        WavDrawingInfoInternal::TopBottomEnvelope(
            (0..width).map(|_| top_px).collect(),
            envlop_iter.collect(),
            Some((top_px, btm_px)),
        )
    } else {
        WavDrawingInfoInternal::TopBottomEnvelope(
            envlop_iter.collect(),
            (0..width).map(|_| btm_px).collect(),
            Some((top_px, btm_px)),
        )
    }
}

pub struct OverviewDrawingInfoInternal {
    pub ch_drawing_infos: Vec<WavDrawingInfoInternal>,
    pub limiter_gain_infos: Option<(WavDrawingInfoInternal, WavDrawingInfoInternal)>,
    pub heights: OverviewHeights,
}

impl OverviewDrawingInfoInternal {
    pub fn new(
        track: &AudioTrack,
        width: f32,
        max_sec: f64,
        height: f32,
        gap_height: f32,
        limiter_gain_height_ratio: f32,
        wav_stroke_width: f32,
        topbottom_context_size: f32,
    ) -> OverviewDrawingInfoInternal {
        let px_per_sec = width / max_sec as f32;
        let drawing_width = px_per_sec * track.sec() as f32;
        let n_ch = track.n_ch().min(OVERVIEW_MAX_CH);
        let heights = OverviewHeights::new(height, gap_height, n_ch, limiter_gain_height_ratio);

        let (wav_drawing_infos, mut gain_drawing_infos): (Vec<_>, Vec<_>) = (0..n_ch)
            .into_par_iter()
            .map(|ch| {
                let new_wav_drawing_info = |h| {
                    WavDrawingInfoInternal::new(
                        track.channel(ch).into(),
                        drawing_width,
                        h,
                        (-1., 1.),
                        wav_stroke_width,
                        topbottom_context_size,
                        false,
                    )
                };
                match track.guard_clip_result() {
                    GuardClippingResult::WavBeforeClip(before_clip) => {
                        let clipped_peak = before_clip.max_peak();
                        if clipped_peak > 1. {
                            // draw wav with clipping
                            (
                                WavDrawingInfoInternal::new(
                                    before_clip.slice(s![ch, ..]).into(),
                                    drawing_width,
                                    heights.ch,
                                    (-clipped_peak, clipped_peak),
                                    wav_stroke_width,
                                    topbottom_context_size,
                                    true,
                                ),
                                None,
                            )
                        } else {
                            (new_wav_drawing_info(heights.ch), None)
                        }
                    }
                    GuardClippingResult::GainSequence(gain_seq)
                        if gain_seq.iter().any(|&x| x < 1.) =>
                    {
                        let gain_drawing_infos = if ch < n_ch - 1 {
                            // calc gain only once
                            None
                        } else {
                            let gain_seq = gain_seq.slice(s![0, ..]);
                            let neg_gain_seq = gain_seq.neg();

                            Some((
                                calc_limiter_gain_drawing_info(
                                    gain_seq,
                                    drawing_width.round() as u32,
                                    (0.5, 1.),
                                    topbottom_context_size,
                                ),
                                calc_limiter_gain_drawing_info(
                                    neg_gain_seq.view(),
                                    drawing_width.round() as u32,
                                    (-1., -0.5),
                                    topbottom_context_size,
                                ),
                            ))
                        };

                        (new_wav_drawing_info(heights.ch_wo_gain), gain_drawing_infos)
                    }
                    _ => {
                        // draw wav only
                        (new_wav_drawing_info(heights.ch), None)
                    }
                }
            })
            .unzip();
        OverviewDrawingInfoInternal {
            ch_drawing_infos: wav_drawing_infos,
            limiter_gain_infos: gain_drawing_infos.pop().unwrap(),
            heights,
        }
    }
}

#[inline]
fn get_amp_to_rel_y_fn(amp_range: (f32, f32)) -> impl Fn(f32) -> f32 {
    let scale = amp_range.1 - amp_range.0;
    move |x: f32| (amp_range.1 - x) / scale
}

fn quantize_px_per_samples(px_per_samples: f32) -> f32 {
    if px_per_samples > 0.75 {
        px_per_samples.round()
    } else if 0.5 < px_per_samples && px_per_samples <= 0.75 {
        0.75
    } else {
        1. / (1. / px_per_samples).round()
    }
}

#[cached(size = 64)]
fn create_resampler(input_size: usize, output_size: usize) -> FftResampler<f32> {
    FftResampler::new(input_size, output_size)
}
