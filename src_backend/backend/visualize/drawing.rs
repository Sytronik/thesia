use std::cell::RefCell;
use std::ops::Neg;
// use std::time::Instant;

use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer, pixels};
use ndarray::prelude::*;
use rayon::prelude::*;

use super::super::TrackManager;
use super::super::dynamics::{GuardClippingResult, MaxPeak};
use super::super::track::TrackList;
use super::super::utils::Pad;
use super::drawing_wav::{draw_limiter_gain_to, draw_wav_to};
use super::img_slice::{CalcWidth, OverviewHeights};
use super::params::DrawOptionForWav;

const OVERVIEW_MAX_CH: usize = 4;
const OVERVIEW_CH_GAP_HEIGHT: f32 = 1.;
const LIMITER_GAIN_HEIGHT_DENOM: usize = 5; // 1/5 of the height will be used for draw limiter gain

pub trait TrackDrawer {
    fn draw_overview(
        &self,
        tracklist: &TrackList,
        id: usize,
        width: u32,
        height: u32,
        dpr: f32,
    ) -> Vec<u8>;
}

impl TrackDrawer for TrackManager {
    fn draw_overview(
        &self,
        tracklist: &TrackList,
        id: usize,
        width: u32,
        height: u32,
        dpr: f32,
    ) -> Vec<u8> {
        let track = if let Some(track) = tracklist.get(id) {
            track
        } else {
            return Vec::new();
        };
        let (pad_left, drawing_width, pad_right) =
            track.decompose_width_of(0., width, width as f64 / tracklist.max_sec);
        let (pad_left, drawing_width_usize, pad_right) = (
            pad_left as usize,
            drawing_width as usize,
            pad_right as usize,
        );
        let n_ch = track.n_ch().min(OVERVIEW_MAX_CH);
        let heights = OverviewHeights::new(height, n_ch, OVERVIEW_CH_GAP_HEIGHT, dpr);
        let (clipped_peak, draw_gain_heights) = match track.guard_clip_result() {
            GuardClippingResult::WavBeforeClip(before_clip) => {
                (before_clip.max_peak(), Default::default())
            }
            GuardClippingResult::GainSequence(gain_seq) if gain_seq.iter().any(|&x| x < 1.) => {
                (1., heights.decompose_by_gain(LIMITER_GAIN_HEIGHT_DENOM))
            }
            _ => (1., Default::default()),
        };

        let mut arr = Array3::zeros((heights.total, drawing_width_usize, 4));
        arr.slice_mut(s![heights.margin.., .., ..])
            .axis_chunks_iter_mut(Axis(0), heights.ch_and_gap())
            .into_par_iter()
            .enumerate()
            .for_each(|(ch, mut arr_ch)| {
                let mut draw_wav = |i_h, h| {
                    draw_wav_to(
                        arr_ch
                            .slice_mut(s![i_h..(i_h + h), .., ..])
                            .as_slice_mut()
                            .unwrap(),
                        track.channel(ch).into(),
                        drawing_width,
                        h as u32,
                        &DrawOptionForWav::with_dpr(dpr),
                        false,
                        false,
                    )
                };
                match track.guard_clip_result() {
                    GuardClippingResult::WavBeforeClip(before_clip) if clipped_peak > 1. => {
                        draw_wav_to(
                            arr_ch
                                .slice_mut(s![..heights.ch, .., ..])
                                .as_slice_mut()
                                .unwrap(),
                            before_clip.slice(s![ch, ..]).into(),
                            drawing_width,
                            heights.ch as u32,
                            &DrawOptionForWav {
                                amp_range: (-clipped_peak, clipped_peak),
                                dpr,
                            },
                            true,
                            false,
                        )
                    }
                    GuardClippingResult::GainSequence(gain_seq)
                        if draw_gain_heights != Default::default() =>
                    {
                        let (gain_h, wav_h) = draw_gain_heights;
                        draw_wav(gain_h, wav_h);
                        if ch > 0 {
                            return;
                        }
                        let gain_seq = gain_seq.slice(s![0, ..]);
                        let neg_gain_seq = gain_seq.neg();
                        let mut draw_gain = |i_h, gain: ArrayView1<f32>, amp_range, draw_bottom| {
                            draw_limiter_gain_to(
                                arr_ch
                                    .slice_mut(s![i_h..(i_h + gain_h), .., ..])
                                    .as_slice_mut()
                                    .unwrap(),
                                gain,
                                drawing_width,
                                gain_h as u32,
                                &DrawOptionForWav { amp_range, dpr },
                                draw_bottom,
                            );
                        };
                        draw_gain(0, gain_seq, (0.5, 1.), true);
                        draw_gain(gain_h + wav_h, neg_gain_seq.view(), (-1., -0.5), false);
                    }
                    _ => {
                        draw_wav(0, heights.ch);
                    }
                }
            });

        if draw_gain_heights != Default::default() {
            let (gain_h, wav_h) = draw_gain_heights;
            let gain_upper = arr
                .slice(s![heights.margin.., .., ..])
                .slice(s![..gain_h, .., ..])
                .to_owned();
            let gain_lower = arr
                .slice(s![heights.margin.., .., ..])
                .slice(s![(gain_h + wav_h)..heights.ch, .., ..])
                .to_owned();

            arr.slice_mut(s![heights.margin.., .., ..])
                .axis_chunks_iter_mut(Axis(0), heights.ch_and_gap())
                .into_par_iter()
                .enumerate()
                .filter(|(ch, _)| *ch > 0)
                .for_each(|(_, mut arr_ch)| {
                    arr_ch.slice_mut(s![..gain_h, .., ..]).assign(&gain_upper);
                    arr_ch
                        .slice_mut(s![(gain_h + wav_h)..heights.ch, .., ..])
                        .assign(&gain_lower);
                });
        }
        if width != drawing_width {
            arr = arr.pad((pad_left, pad_right), Axis(1), Default::default());
        }
        arr.into_raw_vec_and_offset().0
    }
}

#[allow(non_snake_case)]
pub fn convert_spec_to_img(
    spec: ArrayView2<f32>,
    i_freq_range: (usize, usize),
    dB_range: (f32, f32),
    colormap_length: Option<u32>,
) -> Array2<pixels::F32> {
    // spec: T x F
    // return: image with F x T
    let (i_freq_start, i_freq_end) = i_freq_range;
    let dB_span = dB_range.1 - dB_range.0;
    let width = spec.shape()[0];
    let height = i_freq_end - i_freq_start;
    Array2::from_shape_fn((height, width), |(i, j)| {
        let i_freq = i_freq_start + i;
        if i_freq < spec.raw_dim()[1] {
            let zero_to_one = (spec[[j, i_freq]] - dB_range.0) / dB_span;
            let eps_to_one = if let Some(colormap_length) = colormap_length {
                (zero_to_one * (colormap_length - 1) as f32 + 1.0) / colormap_length as f32
            } else {
                zero_to_one
            };
            pixels::F32::new(eps_to_one.clamp(0., 1.))
        } else {
            pixels::F32::new(0.)
        }
    })
}

pub fn resize(img: ArrayView2<pixels::F32>, width: u32, height: u32) -> Array2<pixels::F32> {
    thread_local! {
        static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
    }

    RESIZER.with_borrow_mut(|resizer| {
        let src_img = TypedImageRef::new(
            img.shape()[1] as u32,
            img.shape()[0] as u32,
            img.as_slice_memory_order().unwrap(),
        )
        .unwrap();
        let resize_opt =
            ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

        let mut dst_buf = vec![pixels::F32::new(0.); width as usize * height as usize];
        let mut dst_img =
            TypedImage::<pixels::F32>::from_pixels_slice(width, height, &mut dst_buf).unwrap();
        resizer
            .resize_typed(&src_img, &mut dst_img, &resize_opt)
            .unwrap();
        Array2::from_shape_vec((height as usize, width as usize), dst_buf).unwrap()
    })
}
