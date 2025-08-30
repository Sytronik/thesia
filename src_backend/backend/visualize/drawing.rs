use std::iter;
use std::ops::Neg;
use std::slice::SliceIndex;

use approx::relative_ne;
use cached::proc_macro::cached;
use fast_image_resize::pixels;
use ndarray::prelude::*;
use ndarray::{ErrorKind, ShapeError};
use rayon::prelude::*;

use super::super::dynamics::{GuardClippingResult, MaxPeak};
use super::super::simd::{find_max, find_min, find_min_max};
use super::super::track::AudioTrack;

use super::WavSliceArgs;
use super::resample::FftResampler;
use super::slice_args::{OverviewHeights, WavDrawingInfoSliceArgs};

const RESAMPLE_TAIL: usize = 500;
const THR_TOPBOTTOM_PERCENT: usize = 70;

const OVERVIEW_MAX_CH: usize = 4;
const MAX_WAV_LINE_LEN: usize = 2_usize.pow(20);

#[allow(non_snake_case)]
pub fn convert_spectrogram_to_img(
    spec: ArrayView2<f32>,
    i_freq_range: (usize, usize),
    dB_range: (f32, f32),
    colormap_length: Option<u32>,
) -> Array2<pixels::U16> {
    // spec: T x F
    // return: image with F x T
    let (i_freq_start, i_freq_end) = i_freq_range;
    let dB_span = dB_range.1 - dB_range.0;
    let width = spec.shape()[0];
    let height = i_freq_end - i_freq_start;
    let min_value =
        colormap_length.map_or(1, |l| ((u16::MAX as f64 / l as f64).round() as u16).max(1));
    let u16_span = (u16::MAX - min_value) as f32;
    Array2::from_shape_fn((height, width), |(i, j)| {
        let i_freq = i_freq_start + i;
        if i_freq < spec.raw_dim()[1] {
            let zero_to_one = (spec[[j, i_freq]] - dB_range.0) / dB_span;
            let u16_min_to_max = zero_to_one * u16_span + min_value as f32;
            pixels::U16::new(u16_min_to_max.round().clamp(0., u16::MAX as f32) as u16)
        } else {
            pixels::U16::new(0)
        }
    })
}

#[derive(Default)]
pub enum WavDrawingInfoKind {
    #[default]
    FillRect,
    Line(Vec<f32>, Option<(f32, f32)>),
    TopBottomEnvelope(Vec<f32>, Vec<f32>, Option<(f32, f32)>),
}

impl WavDrawingInfoKind {
    pub fn len(&self) -> usize {
        match self {
            Self::FillRect => 0,
            Self::Line(line, ..) => line.len(),
            Self::TopBottomEnvelope(top, ..) => top.len(),
        }
    }

    pub fn slice(&self, range: impl SliceIndex<[f32], Output = [f32]> + Clone) -> Self {
        match self {
            Self::FillRect => Self::FillRect,
            Self::Line(..) => unimplemented!(),
            Self::TopBottomEnvelope(top, btm, clip_values) => Self::TopBottomEnvelope(
                top[range.clone()].to_owned(),
                btm[range].to_owned(),
                *clip_values,
            ),
        }
    }

    pub fn downsample(&mut self, ratio: usize, offset: usize) {
        match self {
            WavDrawingInfoKind::FillRect => (),
            WavDrawingInfoKind::Line(..) => unimplemented!(),
            WavDrawingInfoKind::TopBottomEnvelope(top, btm, _) => {
                rayon::join(
                    || {
                        if offset > 0 {
                            let (pre, main) = top.split_at(offset);
                            *top = iter::once(pre)
                                .chain(main.chunks(ratio))
                                .map(find_min)
                                .collect();
                        } else {
                            *top = top.chunks(ratio).map(find_min).collect();
                        }
                    },
                    || {
                        if offset > 0 {
                            let (pre, main) = btm.split_at(offset);
                            *btm = iter::once(pre)
                                .chain(main.chunks(ratio))
                                .map(find_max)
                                .collect()
                        } else {
                            *btm = btm.chunks(ratio).map(find_max).collect();
                        }
                    },
                );
            }
        }
    }

    pub fn convert_amp_range(
        &mut self,
        orig_amp_range: (f32, f32),
        target_amp_range: (f32, f32),
        height: f64,
        wav_stroke_width: f64,
    ) {
        let amp_to_rel_y = get_amp_to_rel_y_fn(target_amp_range);
        let orig_scale = orig_amp_range.1 - orig_amp_range.0;
        let convert = |orig_rel_y: f32| amp_to_rel_y(orig_amp_range.1 - orig_rel_y * orig_scale);
        let need_convert = relative_ne!(orig_amp_range.0, target_amp_range.0)
            && relative_ne!(orig_amp_range.1, target_amp_range.1);

        match self {
            Self::FillRect => {}
            Self::Line(..) => unimplemented!(),
            Self::TopBottomEnvelope(top_envlop, btm_envlop, clip_values) => {
                if need_convert {
                    rayon::join(
                        || top_envlop.iter_mut().for_each(|y| *y = convert(*y)),
                        || btm_envlop.iter_mut().for_each(|y| *y = convert(*y)),
                    ); // TODO: use SIMD
                }
                let wav_stroke_width_div_height = (wav_stroke_width / height) as f32;
                top_envlop
                    .iter_mut()
                    .zip(btm_envlop.iter_mut())
                    .for_each(|(top, btm)| {
                        fill_topbottom_if_shorter_than_stroke_width(
                            (top, btm),
                            wav_stroke_width_div_height,
                        );
                    });
                *clip_values = clip_values.map(|(top, btm)| (convert(top), convert(btm)));
            }
        }
    }

    pub fn from_limiter_gain(
        gain: ArrayView1<f32>,
        width: u32,
        amp_range: (f32, f32),
        topbottom_context_size: f64,
    ) -> Self {
        let half_context_size = (topbottom_context_size / 2.) as f32;
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
                let i_end = (((i_px + half_context_size) * samples_per_px).round() as usize)
                    .min(gain.len());
                amp_to_rel_y(gain.slice(s![i_start..i_end]).mean().unwrap_or_default())
            }
        });

        let top_px = amp_to_rel_y(amp_range.1);
        let btm_px = amp_to_rel_y(amp_range.0);
        if amp_range.1 > 0. {
            Self::TopBottomEnvelope(
                vec![top_px; width as usize],
                envlop_iter.collect(),
                Some((top_px, btm_px)),
            )
        } else {
            Self::TopBottomEnvelope(
                envlop_iter.collect(),
                vec![btm_px; width as usize],
                Some((top_px, btm_px)),
            )
        }
    }
}

/// default value means no drawing (sliced wav is empty)
#[derive(Default)]
#[readonly::make]
pub struct WavDrawingInfoInternal {
    pub kind: WavDrawingInfoKind,
    pub drawing_sec: f64,
    pub pre_margin_sec: f64,
    pub post_margin_sec: f64,
}

impl WavDrawingInfoInternal {
    pub fn new(
        kind: WavDrawingInfoKind,
        drawing_sec: f64,
        pre_margin_sec: f64,
        post_margin_sec: f64,
    ) -> Self {
        Self {
            kind,
            drawing_sec,
            pre_margin_sec,
            post_margin_sec,
        }
    }

    pub fn from_wav(
        wav: ArrayView1<f32>,
        sr: u32,
        width: f64,
        height: f64,
        amp_range: (f32, f32),
        wav_stroke_width: f64,
        topbottom_context_size: f64,
        show_clipping: bool,
        force_topbottom: bool,
    ) -> Self {
        let wav_sec = wav.len() as f64 / sr as f64;
        Self::from_wav_with_slicing(
            wav,
            sr,
            (0., wav_sec),
            0.,
            width,
            height,
            amp_range,
            wav_stroke_width,
            topbottom_context_size,
            show_clipping,
            force_topbottom,
        )
        .unwrap()
    }

    pub fn from_wav_with_slicing(
        wav: ArrayView1<f32>,
        sr: u32,
        sec_range: (f64, f64),
        margin_ratio: f64,
        width: f64,
        height: f64,
        amp_range: (f32, f32),
        wav_stroke_width: f64,
        topbottom_context_size: f64,
        show_clipping: bool,
        force_topbottom: bool,
    ) -> Result<WavDrawingInfoInternal, ShapeError> {
        let px_per_samples = width / (sec_range.1 - sec_range.0) / sr as f64;
        let resample_ratio = quantize_px_per_samples(px_per_samples);
        let args = WavSliceArgs::new(sr, sec_range, resample_ratio, wav.len(), margin_ratio);

        if args.start_w_margin >= wav.len() {
            return Err(ShapeError::from_kind(ErrorKind::OutOfBounds));
        }
        let end_w_margin = args.start_w_margin + args.length_w_margin;
        let thr_long_height = (wav_stroke_width / height) as f32;
        let amp_to_rel_y = get_amp_to_rel_y_fn(amp_range);
        let clip_values = (show_clipping && (amp_range.0 < -1. || amp_range.1 > 1.))
            .then_some((amp_to_rel_y(1.), amp_to_rel_y(-1.)));

        let kind = if amp_range.1 - amp_range.0 < 1e-16 {
            // over-zoomed
            WavDrawingInfoKind::FillRect
        } else if resample_ratio > 0.5 && !force_topbottom {
            // upsampling
            let mut resampler;
            let wav = if resample_ratio != 1. {
                let end_w_tail = (end_w_margin + RESAMPLE_TAIL).min(wav.len());
                let wav_w_tail = wav.slice(s![args.start_w_margin..end_w_tail]);
                let upsampled_len_tail = (wav_w_tail.len() as f64 * resample_ratio).round();
                resampler = create_resampler(wav_w_tail.len(), upsampled_len_tail as usize);
                resampler.resample(wav_w_tail)
            } else {
                wav.slice(s![args.start_w_margin..end_w_margin])
            };
            WavDrawingInfoKind::Line(
                wav.slice(s![..args.total_len])
                    .into_par_iter()
                    .with_min_len((args.total_len / rayon::current_num_threads()).max(1))
                    .map(|&x| amp_to_rel_y(x))
                    .collect(), // TODO: benchmark parallel iterator, use SIMD
                clip_values,
            )
        } else {
            let half_context_size = topbottom_context_size / 2.;
            let mean_rel_y = amp_to_rel_y(
                wav.slice(s![args.start_w_margin..end_w_margin])
                    .mean()
                    .unwrap_or(0.),
            );
            let zero_rel_y = amp_to_rel_y(0.);

            let (mut top_envlop, mut btm_envlop): (Vec<_>, Vec<_>) = (0..args.total_len)
                .into_par_iter()
                .with_min_len((args.total_len / rayon::current_num_threads()).max(1))
                .map(|i_envlop| {
                    let i_envlop = i_envlop as f64;
                    let i_start = (args.start_w_margin_f64
                        + (i_envlop - half_context_size) / resample_ratio)
                        .max(0.) as usize;
                    let i_end = ((args.start_w_margin_f64
                        + (i_envlop + half_context_size) / resample_ratio)
                        as usize)
                        .min(wav.len());

                    let wav_slice = &wav.as_slice().unwrap()[i_start..i_end];
                    let (min_val, max_val) = find_min_max(wav_slice);

                    (amp_to_rel_y(max_val), amp_to_rel_y(min_val))
                })
                .unzip();

            let wav_stroke_width_div_height = (wav_stroke_width / height) as f32;
            let n_mean_crossing = top_envlop
                .par_iter_mut()
                .with_min_len((args.total_len / rayon::current_num_threads()).max(1))
                .zip(
                    btm_envlop
                        .par_iter_mut()
                        .with_min_len((args.total_len / rayon::current_num_threads()).max(1)),
                )
                .filter_map(|(top, btm)| {
                    let top_val = *top;
                    let btm_val = *btm;

                    let is_mean_crossing = top_val < mean_rel_y + f32::EPSILON
                        && btm_val > mean_rel_y - thr_long_height
                        || top_val < mean_rel_y + thr_long_height
                            && btm_val > mean_rel_y - f32::EPSILON;

                    let was_shorter = fill_topbottom_if_shorter_than_stroke_width(
                        (top, btm),
                        wav_stroke_width_div_height,
                    );

                    if !was_shorter {
                        is_mean_crossing.then_some(())
                    } else {
                        let all_close_to_zero = top_val <= zero_rel_y && btm_val >= zero_rel_y;
                        all_close_to_zero.then_some(())
                    }
                })
                .count();
            if force_topbottom
                || args.length_w_margin > MAX_WAV_LINE_LEN
                || n_mean_crossing > args.total_len * THR_TOPBOTTOM_PERCENT / 100
            {
                WavDrawingInfoKind::TopBottomEnvelope(top_envlop, btm_envlop, clip_values)
            } else {
                WavDrawingInfoKind::Line(
                    wav.slice(s![args.start_w_margin..end_w_margin])
                        .into_par_iter()
                        .with_min_len((args.length_w_margin / rayon::current_num_threads()).max(1))
                        .map(|&x| amp_to_rel_y(x))
                        .collect(), // TODO: benchmark parallel iterator, use SIMD
                    clip_values,
                )
            }
        };

        Ok(WavDrawingInfoInternal::new(
            kind,
            args.drawing_sec,
            args.pre_margin_sec,
            args.post_margin_sec,
        ))
    }
}

impl From<WavDrawingInfoInternal> for WavDrawingInfoKind {
    fn from(info: WavDrawingInfoInternal) -> Self {
        info.kind
    }
}

// #[readonly::make]
pub struct WavDrawingInfoCache {
    pub wav_stroke_width: f64,
    pub topbottom_context_size: f64,
    pub px_per_sec: f64,
    pub drawing_info_kinds: Vec<WavDrawingInfoKind>,
    pub amp_ranges: Vec<(f32, f32)>,
}

impl WavDrawingInfoCache {
    pub fn slice(
        &self,
        ch: usize,
        sec_range: (f64, f64),
        track_sec: f64,
        margin_ratio: f64,
        width: f64,
        height: f64,
        amp_range: (f32, f32),
        wav_stroke_width: f64,
    ) -> WavDrawingInfoInternal {
        let kind = &self.drawing_info_kinds[ch];
        let cache_len = kind.len();

        let args = WavDrawingInfoSliceArgs::new(cache_len, sec_range, track_sec, margin_ratio);

        if args.start_w_margin >= cache_len {
            return Default::default();
        }

        // slice
        let mut kind =
            kind.slice(args.start_w_margin..(args.start_w_margin + args.length_w_margin));

        let drawing_width = args.drawing_sec / (sec_range.1 - sec_range.0) * width;
        let ratio = (kind.len() as f64 / drawing_width).floor() as usize;
        debug_assert!(ratio >= 1);
        if ratio > 1 {
            kind.downsample(ratio, args.start_w_margin % ratio);
        }

        // convert amp_range
        kind.convert_amp_range(self.amp_ranges[ch], amp_range, height, wav_stroke_width);

        WavDrawingInfoInternal {
            kind,
            drawing_sec: args.drawing_sec,
            pre_margin_sec: args.pre_margin_sec,
            post_margin_sec: args.post_margin_sec,
        } // return cache
    }
}

pub struct OverviewDrawingInfoInternal {
    pub ch_drawing_infos: Vec<WavDrawingInfoKind>,
    pub limiter_gain_infos: Option<(WavDrawingInfoKind, WavDrawingInfoKind)>,
    pub heights: OverviewHeights,
}

impl OverviewDrawingInfoInternal {
    pub fn new(
        track: &AudioTrack,
        width: f64,
        max_sec: f64,
        height: f64,
        gap_height: f64,
        limiter_gain_height_ratio: f64,
        wav_stroke_width: f64,
        topbottom_context_size: f64,
    ) -> OverviewDrawingInfoInternal {
        let px_per_sec = width / max_sec;
        let drawing_width = px_per_sec * track.sec();
        let n_ch = track.n_ch().min(OVERVIEW_MAX_CH);
        let heights = OverviewHeights::new(height, gap_height, n_ch, limiter_gain_height_ratio);

        let (wav_drawing_infos, mut gain_drawing_infos): (Vec<_>, Vec<_>) = (0..n_ch)
            .into_par_iter()
            .map(|ch| {
                let new_wav_drawing_info_kind = |h| {
                    let wav = track.channel(ch);
                    WavDrawingInfoInternal::from_wav(
                        wav,
                        track.sr(),
                        drawing_width,
                        h,
                        (-1., 1.),
                        wav_stroke_width,
                        topbottom_context_size,
                        false,
                        false,
                    )
                    .into()
                };
                match track.guard_clip_result() {
                    GuardClippingResult::WavBeforeClip(before_clip) => {
                        let clipped_peak = before_clip.max_peak();
                        if clipped_peak > 1. {
                            // draw wav with clipping
                            let wav_drawing_info_kind = WavDrawingInfoInternal::from_wav(
                                before_clip.slice(s![ch, ..]),
                                track.sr(),
                                drawing_width,
                                heights.ch,
                                (-clipped_peak, clipped_peak),
                                wav_stroke_width,
                                topbottom_context_size,
                                true,
                                false,
                            )
                            .into();
                            (wav_drawing_info_kind, None)
                        } else {
                            (new_wav_drawing_info_kind(heights.ch), None)
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

                            Some(rayon::join(
                                || {
                                    WavDrawingInfoKind::from_limiter_gain(
                                        gain_seq,
                                        drawing_width.round() as u32,
                                        (0.5, 1.),
                                        topbottom_context_size,
                                    )
                                },
                                || {
                                    WavDrawingInfoKind::from_limiter_gain(
                                        neg_gain_seq.view(),
                                        drawing_width.round() as u32,
                                        (-1., -0.5),
                                        topbottom_context_size,
                                    )
                                },
                            ))
                        };

                        (
                            new_wav_drawing_info_kind(heights.ch_wo_gain),
                            gain_drawing_infos,
                        )
                    }
                    _ => {
                        // draw wav only
                        (new_wav_drawing_info_kind(heights.ch), None)
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

fn quantize_px_per_samples(px_per_samples: f64) -> f64 {
    if px_per_samples > 0.75 {
        px_per_samples.round()
    } else if 0.5 < px_per_samples && px_per_samples <= 0.75 {
        0.75
    } else {
        1. / (1. / px_per_samples).round()
    }
}

fn fill_topbottom_if_shorter_than_stroke_width(
    (top, btm): (&mut f32, &mut f32),
    wav_stroke_width_div_height: f32,
) -> bool {
    let len_to_fill_stroke_width = wav_stroke_width_div_height - (*btm - *top);

    let need_fill = len_to_fill_stroke_width > 0.;
    if need_fill {
        *top -= len_to_fill_stroke_width / 2.;
        *btm += len_to_fill_stroke_width / 2.;
    }
    need_fill
}

#[cached(size = 16)]
fn create_resampler(input_size: usize, output_size: usize) -> FftResampler<f32> {
    FftResampler::new(input_size, output_size)
}
