use std::cell::RefCell;
use std::ops::Neg;
// use std::time::Instant;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{__m128, __m128i, __m256, __m256i};

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::{float32x4_t, int32x4_t};

#[allow(unused_imports)]
use aligned::{Aligned, A16, A32};
use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::{pixels, FilterType, ImageView, ResizeAlg, ResizeOptions, Resizer};
use itertools::{multizip, Itertools};
use ndarray::prelude::*;
use rayon::prelude::*;
use tiny_skia::{
    FillRule, IntRect, Paint, PathBuilder, Pixmap, PixmapMut, PixmapPaint, PixmapRef, Transform,
};

use super::super::dynamics::{GuardClippingResult, MaxPeak};
use super::super::track::TrackList;
use super::super::utils::Pad;
use super::super::{IdChArr, IdChValueVec, TrackManager};
use super::drawing_wav::{draw_limiter_gain_to, draw_wav_to};
use super::img_slice::{ArrWithSliceInfo, CalcWidth, LeftWidth, OverviewHeights, PartGreyInfo};
use super::params::{DrawOptionForWav, DrawParams, ImageKind};

const BLACK: [u8; 3] = [000; 3];
const WHITE: [u8; 3] = [255; 3];
// const BLACK_F32: [f32; 3] = [0.; 3];
const WHITE_F32: [f32; 3] = [255.; 3];

const COLORMAP_R: [f32; 256] = [
    0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 4.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
    13.0, 14.0, 16.0, 17.0, 18.0, 20.0, 21.0, 22.0, 24.0, 25.0, 27.0, 28.0, 30.0, 31.0, 33.0, 35.0,
    36.0, 38.0, 40.0, 42.0, 43.0, 45.0, 47.0, 49.0, 51.0, 52.0, 54.0, 56.0, 58.0, 59.0, 61.0, 63.0,
    64.0, 66.0, 68.0, 69.0, 71.0, 73.0, 74.0, 76.0, 78.0, 79.0, 81.0, 83.0, 84.0, 86.0, 87.0, 89.0,
    91.0, 92.0, 94.0, 95.0, 97.0, 99.0, 100.0, 102.0, 103.0, 105.0, 107.0, 108.0, 110.0, 111.0,
    113.0, 115.0, 116.0, 118.0, 119.0, 121.0, 123.0, 124.0, 126.0, 127.0, 129.0, 130.0, 132.0,
    134.0, 135.0, 137.0, 138.0, 140.0, 142.0, 143.0, 145.0, 146.0, 148.0, 150.0, 151.0, 153.0,
    154.0, 156.0, 158.0, 159.0, 161.0, 162.0, 164.0, 165.0, 167.0, 169.0, 170.0, 172.0, 173.0,
    175.0, 176.0, 178.0, 179.0, 181.0, 182.0, 184.0, 185.0, 187.0, 188.0, 190.0, 191.0, 193.0,
    194.0, 196.0, 197.0, 198.0, 200.0, 201.0, 203.0, 204.0, 205.0, 207.0, 208.0, 209.0, 211.0,
    212.0, 213.0, 214.0, 216.0, 217.0, 218.0, 219.0, 220.0, 221.0, 223.0, 224.0, 225.0, 226.0,
    227.0, 228.0, 229.0, 230.0, 231.0, 232.0, 233.0, 234.0, 235.0, 235.0, 236.0, 237.0, 238.0,
    239.0, 240.0, 240.0, 241.0, 242.0, 242.0, 243.0, 244.0, 244.0, 245.0, 246.0, 246.0, 247.0,
    247.0, 248.0, 248.0, 249.0, 249.0, 249.0, 250.0, 250.0, 250.0, 251.0, 251.0, 251.0, 252.0,
    252.0, 252.0, 252.0, 252.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0, 253.0,
    253.0, 253.0, 253.0, 252.0, 252.0, 252.0, 252.0, 252.0, 251.0, 251.0, 251.0, 251.0, 250.0,
    250.0, 250.0, 249.0, 249.0, 248.0, 248.0, 247.0, 247.0, 246.0, 246.0, 245.0, 245.0, 244.0,
    244.0, 244.0, 243.0, 243.0, 243.0, 242.0, 242.0, 242.0, 242.0, 243.0, 243.0, 244.0, 244.0,
    245.0, 246.0, 247.0, 249.0, 250.0, 251.0, 253.0,
];
const COLORMAP_G: [f32; 256] = [
    0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 6.0, 7.0, 7.0, 8.0, 8.0,
    9.0, 9.0, 10.0, 10.0, 11.0, 11.0, 11.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0,
    12.0, 11.0, 11.0, 11.0, 11.0, 10.0, 10.0, 10.0, 10.0, 9.0, 9.0, 9.0, 9.0, 9.0, 9.0, 10.0, 10.0,
    10.0, 10.0, 11.0, 11.0, 12.0, 12.0, 13.0, 13.0, 14.0, 14.0, 15.0, 15.0, 16.0, 17.0, 17.0, 18.0,
    18.0, 19.0, 20.0, 20.0, 21.0, 21.0, 22.0, 23.0, 23.0, 24.0, 24.0, 25.0, 25.0, 26.0, 27.0, 27.0,
    28.0, 28.0, 29.0, 29.0, 30.0, 31.0, 31.0, 32.0, 32.0, 33.0, 33.0, 34.0, 34.0, 35.0, 36.0, 36.0,
    37.0, 37.0, 38.0, 38.0, 39.0, 40.0, 40.0, 41.0, 41.0, 42.0, 43.0, 43.0, 44.0, 45.0, 45.0, 46.0,
    46.0, 47.0, 48.0, 49.0, 49.0, 50.0, 51.0, 51.0, 52.0, 53.0, 54.0, 54.0, 55.0, 56.0, 57.0, 58.0,
    59.0, 60.0, 60.0, 61.0, 62.0, 63.0, 64.0, 65.0, 66.0, 67.0, 68.0, 69.0, 70.0, 72.0, 73.0, 74.0,
    75.0, 76.0, 77.0, 79.0, 80.0, 81.0, 82.0, 84.0, 85.0, 86.0, 88.0, 89.0, 90.0, 92.0, 93.0, 95.0,
    96.0, 98.0, 99.0, 101.0, 102.0, 104.0, 105.0, 107.0, 109.0, 110.0, 112.0, 113.0, 115.0, 117.0,
    118.0, 120.0, 122.0, 123.0, 125.0, 127.0, 129.0, 130.0, 132.0, 134.0, 136.0, 137.0, 139.0,
    141.0, 143.0, 145.0, 146.0, 148.0, 150.0, 152.0, 154.0, 156.0, 158.0, 160.0, 161.0, 163.0,
    165.0, 167.0, 169.0, 171.0, 173.0, 175.0, 177.0, 179.0, 181.0, 183.0, 185.0, 186.0, 188.0,
    190.0, 192.0, 194.0, 196.0, 198.0, 200.0, 202.0, 204.0, 206.0, 208.0, 210.0, 212.0, 214.0,
    216.0, 218.0, 220.0, 222.0, 224.0, 226.0, 228.0, 229.0, 231.0, 233.0, 235.0, 237.0, 238.0,
    240.0, 241.0, 243.0, 244.0, 246.0, 247.0, 249.0, 250.0, 251.0, 252.0, 253.0, 254.0, 255.0,
];
const COLORMAP_B: [f32; 256] = [
    4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 21.0, 23.0, 25.0, 27.0, 29.0, 32.0, 34.0,
    36.0, 38.0, 41.0, 43.0, 45.0, 48.0, 50.0, 53.0, 55.0, 58.0, 60.0, 62.0, 65.0, 67.0, 70.0, 72.0,
    74.0, 77.0, 79.0, 81.0, 83.0, 85.0, 87.0, 89.0, 91.0, 93.0, 94.0, 96.0, 97.0, 98.0, 99.0,
    100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 105.0, 106.0, 107.0, 107.0, 108.0, 108.0, 108.0,
    109.0, 109.0, 109.0, 110.0, 110.0, 110.0, 110.0, 110.0, 111.0, 111.0, 111.0, 111.0, 111.0,
    111.0, 111.0, 111.0, 111.0, 111.0, 111.0, 111.0, 110.0, 110.0, 110.0, 110.0, 110.0, 110.0,
    109.0, 109.0, 109.0, 109.0, 108.0, 108.0, 108.0, 107.0, 107.0, 107.0, 106.0, 106.0, 105.0,
    105.0, 105.0, 104.0, 104.0, 103.0, 102.0, 102.0, 101.0, 101.0, 100.0, 100.0, 99.0, 98.0, 98.0,
    97.0, 96.0, 95.0, 95.0, 94.0, 93.0, 92.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 85.0,
    84.0, 83.0, 82.0, 81.0, 80.0, 79.0, 78.0, 77.0, 76.0, 75.0, 74.0, 72.0, 71.0, 70.0, 69.0, 68.0,
    67.0, 66.0, 65.0, 64.0, 62.0, 61.0, 60.0, 59.0, 58.0, 57.0, 56.0, 54.0, 53.0, 52.0, 51.0, 50.0,
    48.0, 47.0, 46.0, 45.0, 43.0, 42.0, 41.0, 40.0, 38.0, 37.0, 36.0, 35.0, 33.0, 32.0, 31.0, 30.0,
    28.0, 27.0, 26.0, 24.0, 23.0, 22.0, 20.0, 19.0, 18.0, 16.0, 15.0, 14.0, 12.0, 11.0, 10.0, 9.0,
    8.0, 7.0, 7.0, 6.0, 6.0, 6.0, 6.0, 7.0, 7.0, 8.0, 9.0, 10.0, 12.0, 13.0, 15.0, 17.0, 19.0,
    20.0, 22.0, 24.0, 27.0, 29.0, 31.0, 33.0, 35.0, 38.0, 40.0, 43.0, 45.0, 48.0, 50.0, 53.0, 56.0,
    58.0, 61.0, 64.0, 67.0, 70.0, 73.0, 76.0, 80.0, 83.0, 86.0, 90.0, 94.0, 97.0, 101.0, 105.0,
    109.0, 113.0, 117.0, 122.0, 126.0, 130.0, 134.0, 138.0, 142.0, 146.0, 150.0, 154.0, 158.0,
    162.0, 165.0,
];
const GREY_TO_POS: f32 = COLORMAP_R.len() as f32 / (u16::MAX - 1) as f32;

const OVERVIEW_MAX_CH: usize = 4;
const OVERVIEW_CH_GAP_HEIGHT: f32 = 1.;
const LIMITER_GAIN_HEIGHT_DENOM: usize = 5; // 1/5 of the height will be used for draw limiter gain

pub trait TrackDrawer {
    fn draw_entire_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        height: u32,
        px_per_sec: f64,
        kind: ImageKind,
    ) -> IdChValueVec<Array3<u8>>;

    fn draw_part_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        params: &DrawParams,
        fast_resize_vec: impl Into<Option<Vec<bool>>>,
    ) -> IdChValueVec<Vec<u8>>;

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
    fn draw_entire_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        height: u32,
        px_per_sec: f64,
        kind: ImageKind,
    ) -> IdChValueVec<Array3<u8>> {
        // let start = Instant::now();
        let parallel = id_ch_tuples.len() < rayon::current_num_threads();
        id_ch_tuples
            .par_iter()
            .map(|&(id, ch)| {
                let out_for_not_exist = || ((id, ch), Array::zeros((0, 0, 0)));
                let track = if let Some(track) = tracklist.get(id) {
                    track
                } else {
                    return out_for_not_exist();
                };
                let width = track.calc_width(px_per_sec);
                let shape = (height as usize, width as usize, 4);
                let arr = match &kind {
                    ImageKind::Spec => {
                        let grey = if let Some(grey) = self.spec_greys.get(&(id, ch)) {
                            grey.view()
                        } else {
                            return out_for_not_exist();
                        };
                        let vec = colorize_resize_grey(grey.into(), width, height, false, parallel);
                        Array3::from_shape_vec(shape, vec).unwrap()
                    }
                    ImageKind::Wav(opt_for_wav) => {
                        let mut arr = Array3::zeros(shape);
                        let (wav, show_clipping) = track.channel_for_drawing(ch);
                        draw_wav_to(
                            arr.as_slice_mut().unwrap(),
                            wav.into(),
                            width,
                            height,
                            opt_for_wav,
                            show_clipping,
                            true,
                        );
                        arr
                    }
                };
                ((id, ch), arr)
            })
            .collect()
        // println!("draw entire: {:?}", start.elapsed());
    }

    /// Draw part of images. if blend < 0, draw waveform with transparent background
    fn draw_part_imgs(
        &self,
        tracklist: &TrackList,
        id_ch_tuples: &IdChArr,
        params: &DrawParams,
        fast_resize_vec: impl Into<Option<Vec<bool>>>,
    ) -> IdChValueVec<Vec<u8>> {
        // let start = Instant::now();
        let &DrawParams {
            start_sec,
            width,
            height,
            px_per_sec,
            ref opt_for_wav,
            blend,
        } = params;
        let fast_resize_vec = fast_resize_vec.into();
        let parallel = id_ch_tuples.len() < rayon::current_num_threads();
        id_ch_tuples
            .par_iter()
            .enumerate()
            .map(|(i, &(id, ch))| {
                let out_for_not_exist = || ((id, ch), Vec::new());
                let track = if let Some(track) = tracklist.get(id) {
                    track
                } else {
                    return out_for_not_exist();
                };
                let spec_grey = if let Some(grey) = self.spec_greys.get(&(id, ch)) {
                    grey
                } else {
                    return out_for_not_exist();
                };
                let PartGreyInfo {
                    i_w_and_width,
                    start_sec_with_margin,
                    width_with_margin,
                } = track.calc_part_grey_info(
                    spec_grey.shape()[1] as u64,
                    start_sec,
                    width,
                    px_per_sec,
                );

                let (pad_left, drawing_width_with_margin, pad_right) =
                    track.decompose_width_of(start_sec_with_margin, width_with_margin, px_per_sec);
                if drawing_width_with_margin == 0 {
                    return ((id, ch), vec![0u8; height as usize * width as usize * 4]);
                }

                let spec_grey_part = ArrWithSliceInfo::new(spec_grey.view(), i_w_and_width);
                let (wav, show_clipping) = track.channel_for_drawing(ch);
                let wav_part = ArrWithSliceInfo::new(
                    wav,
                    track.calc_part_wav_info(start_sec_with_margin, width_with_margin, px_per_sec),
                );
                let vec = draw_blended_spec_wav(
                    spec_grey_part,
                    wav_part,
                    drawing_width_with_margin,
                    height,
                    opt_for_wav,
                    blend,
                    fast_resize_vec.as_ref().map_or(false, |v| v[i]),
                    show_clipping,
                    parallel,
                );
                let mut arr = Array3::from_shape_vec(
                    (height as usize, drawing_width_with_margin as usize, 4),
                    vec,
                )
                .unwrap();

                if width_with_margin != drawing_width_with_margin {
                    arr = arr.pad(
                        (pad_left as usize, pad_right as usize),
                        Axis(1),
                        Default::default(),
                    );
                }
                let margin_l = ((start_sec - start_sec_with_margin) * px_per_sec).round() as isize;
                arr.slice_collapse(s![.., margin_l..(margin_l + width as isize), ..]);
                let (vec, _) = arr
                    .as_standard_layout()
                    .to_owned()
                    .into_raw_vec_and_offset();
                ((id, ch), vec)
            })
            .collect()
        // println!("draw: {:?}", start.elapsed());
    }

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

#[inline]
pub fn get_colormap_rgb() -> Vec<u8> {
    multizip((COLORMAP_R.iter(), COLORMAP_G.iter(), COLORMAP_B.iter()))
        .flat_map(|(&r, &g, &b)| [r as u8, g as u8, b as u8].into_iter())
        .chain(WHITE.iter().copied())
        .collect()
}

#[allow(non_snake_case)]
pub fn convert_spec_to_grey(
    spec: ArrayView2<f32>,
    i_freq_range: (usize, usize),
    dB_range: (f32, f32),
) -> Array2<pixels::U16> {
    // spec: T x F
    // return: grey image with F(inverted) x T
    let (i_freq_start, i_freq_end) = i_freq_range;
    let dB_span = dB_range.1 - dB_range.0;
    let width = spec.shape()[0];
    let height = i_freq_end - i_freq_start;
    Array2::from_shape_fn((height, width), |(i, j)| {
        let i_freq = i_freq_start + height - 1 - i;
        if i_freq < spec.raw_dim()[1] {
            pixels::U16::new(
                (((spec[[j, i_freq]] - dB_range.0) / dB_span).mul_add((u16::MAX - 1) as f32, 1.))
                    .clamp(1., u16::MAX as f32)
                    .round() as u16,
            )
        } else {
            pixels::U16::new(0)
        }
    })
}

pub fn make_opaque(mut image: ArrayViewMut3<u8>, left: u32, width: u32) {
    image
        .slice_mut(s![.., left as isize..(left + width) as isize, 3])
        .fill(u8::MAX);
}

pub fn blend_img_to(
    spec_background: &mut [u8],
    wav_img: &[u8],
    width: u32,
    height: u32,
    blend: f64,
    eff_l_w: impl Into<Option<LeftWidth>>,
) {
    debug_assert!(0. < blend && blend < 1.);
    let mut pixmap = PixmapMut::from_bytes(spec_background, width, height).unwrap();

    let wav_pixmap = PixmapRef::from_bytes(wav_img, width, height).unwrap();
    blend_wav_img_to(&mut pixmap, wav_pixmap, blend, eff_l_w);
}

fn blend_wav_img_to(
    pixmap: &mut PixmapMut,
    wav_pixmap: PixmapRef,
    blend: f64,
    eff_l_w: impl Into<Option<LeftWidth>>,
) {
    // black
    if let Some((left, width)) = eff_l_w.into() {
        if (0.0..0.5).contains(&blend) && width > 0 {
            let rect = IntRect::from_xywh(left as i32, 0, width, pixmap.height())
                .unwrap()
                .to_rect();
            let path = PathBuilder::from_rect(rect);
            let mut paint = Paint::default();
            let alpha = (u8::MAX as f64 * (blend.mul_add(-2., 1.))).round() as u8;
            paint.set_color_rgba8(0, 0, 0, alpha);
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    let paint = PixmapPaint {
        opacity: blend.mul_add(-2., 2.).min(1.) as f32,
        ..Default::default()
    };
    pixmap.draw_pixmap(0, 0, wav_pixmap, &paint, Transform::identity(), None);
}

#[inline]
fn interpolate<const L: usize>(color1: &[f32; L], color2: &[f32; L], ratio: f32) -> [u8; L] {
    let mut iter = color1.iter().zip(color2).map(|(&a, &b)| {
        let out_f32 = ratio.mul_add(a, b.mul_add(-ratio, b));
        #[cfg(target_arch = "x86_64")]
        {
            out_f32.round_ties_even() as u8 // to match with AVX2 rounding
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            out_f32.round() as u8
        }
    });
    [(); L].map(|_| iter.next().unwrap())
}

/// Map u16 GRAY to u8x4 RGBA color
/// 0 -> COLORMAP[0]
/// u16::MAX -> WHITE
fn map_grey_to_color(x: u16) -> [u8; 3] {
    if x == 0 {
        return BLACK;
    }
    if x == u16::MAX {
        return WHITE;
    }
    let position = (x as f32).mul_add(GREY_TO_POS, -GREY_TO_POS);
    let idx2 = position.floor() as usize;
    let idx1 = idx2 + 1;
    let ratio = position.fract();
    // dbg!(idx2, idx1, ratio);
    let rgb1 = if idx2 >= COLORMAP_R.len() - 1 {
        &WHITE_F32
    } else {
        &[COLORMAP_R[idx1], COLORMAP_G[idx1], COLORMAP_B[idx1]]
    };
    let rgb2 = &[COLORMAP_R[idx2], COLORMAP_G[idx2], COLORMAP_B[idx2]];
    interpolate(rgb1, rgb2, ratio)
}

fn map_grey_to_color_iter_fallback(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    grey.iter()
        .flat_map(|&x| map_grey_to_color(x).into_iter().chain(Some(u8::MAX)))
}

/// slower than scalar version
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn _map_grey_to_color_sse41(
    chunk_f32: Aligned<A16, [f32; 4]>,
    grey_to_pos: __m128,
    colormap_len: __m128i,
) -> impl Iterator<Item = u8> {
    use std::arch::x86_64::*;
    use std::mem::{self, MaybeUninit};

    type AlignedUninitI32x4 = Aligned<A16, [MaybeUninit<i32>; 4]>;
    type AlignedI32x4 = Aligned<A16, [i32; 4]>;

    let chunk_simd = _mm_load_ps(chunk_f32.as_ptr());

    // position = chunk_simd * grey_to_pos - grey_to_pos
    let position = _mm_sub_ps(_mm_mul_ps(chunk_simd, grey_to_pos), grey_to_pos);

    // position_floor = floor(position)
    let position_floor = _mm_floor_ps(position);

    // idx2 = (int)position_floor
    let idx2 = _mm_cvtps_epi32(position_floor);

    // idx1 = idx2 + 1
    let idx1 = _mm_add_epi32(idx2, _mm_set1_epi32(1));

    // idx2 = min(idx2, colormap_len)
    let idx2 = _mm_min_epi32(idx2, colormap_len);

    // idx1 = max(idx1, 0)
    let idx1 = _mm_max_epi32(idx1, _mm_setzero_si128());

    // ratio = position - position_floor
    let ratio = _mm_sub_ps(position, position_floor);

    // into arr
    let mut idx1_arr: AlignedUninitI32x4 = Aligned([MaybeUninit::uninit(); 4]);
    _mm_store_si128(idx1_arr.as_mut_ptr() as *mut __m128i, idx1);
    let idx1_arr = mem::transmute::<_, AlignedI32x4>(idx1_arr);

    let mut idx2_arr: AlignedUninitI32x4 = Aligned([MaybeUninit::uninit(); 4]);
    _mm_store_si128(idx2_arr.as_mut_ptr() as *mut __m128i, idx2);
    let idx2_arr = mem::transmute::<_, AlignedI32x4>(idx2_arr);

    // mask1 = colormap_len > idx1
    let mask1 = _mm_cmpgt_epi32(colormap_len, idx1);
    let mut mask1_arr: AlignedUninitI32x4 = Aligned([MaybeUninit::uninit(); 4]);
    _mm_store_si128(mask1_arr.as_mut_ptr() as *mut __m128i, mask1);
    let mask1_arr = mem::transmute::<_, AlignedI32x4>(mask1_arr);

    // mask2 = idx2 >= 0
    let mask2 = _mm_cmpgt_epi32(idx2, _mm_set1_epi32(-1));
    let mut mask2_arr: AlignedUninitI32x4 = Aligned([MaybeUninit::uninit(); 4]);
    _mm_store_si128(mask2_arr.as_mut_ptr() as *mut __m128i, mask2);
    let mask2_arr = mem::transmute::<_, AlignedI32x4>(mask2_arr);

    let mut rgb1 = [MaybeUninit::<__m128>::uninit(); 3];
    let mut rgb2 = [MaybeUninit::<__m128>::uninit(); 3];

    for i in 0..3 {
        let mut values1 = Aligned::<A16, _>([0f32; 4]);
        let mut values2 = Aligned::<A16, _>([0f32; 4]);

        for j in 0..4 {
            let idx1_val = idx1_arr[j] as usize;
            let idx2_val = idx2_arr[j] as usize;

            if mask1_arr[j] != 0 && idx1_val < COLORMAP_R.len() {
                let colormap = match i {
                    0 => &COLORMAP_R,
                    1 => &COLORMAP_G,
                    2 => &COLORMAP_B,
                    _ => unreachable!(),
                };
                values1[j] = colormap[idx1_val];
            } else {
                values1[j] = u8::MAX as f32;
            }

            if mask2_arr[j] != 0 && idx2_val < COLORMAP_R.len() {
                let colormap = match i {
                    0 => &COLORMAP_R,
                    1 => &COLORMAP_G,
                    2 => &COLORMAP_B,
                    _ => unreachable!(),
                };
                values2[j] = colormap[idx2_val];
            } else {
                values2[j] = 0.;
            }
        }

        rgb1[i].write(_mm_load_ps(&values1[0]));
        rgb2[i].write(_mm_load_ps(&values2[0]));
    }
    let rgb1 = mem::transmute::<_, [__m128; 3]>(rgb1);
    let rgb2 = mem::transmute::<_, [__m128; 3]>(rgb2);

    let mut out_r4g4b4 = Aligned::<A16, _>([0u8; 12]); // 4 RGB pixels

    for (out_chunk, color1, color2) in multizip((out_r4g4b4.chunks_exact_mut(4), rgb1, rgb2)) {
        // x = -(color2 * ratio) + color2 = color2 * (1 - ratio)
        let one_minus_ratio = _mm_sub_ps(_mm_set1_ps(1.0), ratio);
        let x = _mm_mul_ps(color2, one_minus_ratio);

        // out_f32 = color1 * ratio + x
        let out_f32 = _mm_add_ps(_mm_mul_ps(color1, ratio), x);

        // round to integer
        let out = _mm_cvtps_epi32(_mm_round_ps(
            out_f32,
            _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC,
        ));

        // into arr
        let mut out_arr = Aligned::<A16, _>([MaybeUninit::<i32>::uninit(); 4]);
        _mm_store_si128(out_arr.as_mut_ptr() as *mut __m128i, out);
        let out_arr = mem::transmute::<_, AlignedI32x4>(out_arr);

        for i in 0..4 {
            if chunk_f32[i] == 0. {
                continue;
            }

            if chunk_f32[i] == u16::MAX as f32 {
                out_chunk[i] = u8::MAX;
            } else {
                out_chunk[i] = out_arr[i] as u8;
            }
        }
    }

    (0..4).cartesian_product(0..4).map(move |(i, j)| {
        if j < 3 {
            out_r4g4b4[j * 4 + i] as u8
        } else {
            u8::MAX
        }
    })
}

/// slower than scalar version
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn _map_grey_to_color_iter_sse41(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    use std::arch::x86_64::*;

    use aligned::{Aligned, A16};

    let grey_to_pos_sse41 = _mm_set1_ps(GREY_TO_POS);
    let colormap_len_sse41 = _mm_set1_epi32(COLORMAP_R.len() as i32);

    let grey_sse41 = grey.chunks_exact(4);
    let grey_fallback = grey_sse41.remainder();
    grey_sse41
        .flat_map(move |chunk| {
            let mut chunk_iter = chunk.iter().map(|&x| x as f32);
            let chunk_f32 = Aligned::<A16, _>([(); 4].map(|_| chunk_iter.next().unwrap()));
            _map_grey_to_color_sse41(chunk_f32, grey_to_pos_sse41, colormap_len_sse41)
        })
        .chain(map_grey_to_color_iter_fallback(grey_fallback))
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn map_grey_to_color_avx2(
    chunk_f32: Aligned<A32, [f32; 8]>,
    grey_to_pos: __m256,
    colormap_len: __m256i,
) -> impl Iterator<Item = u8> {
    use std::arch::x86_64::*;

    let chunk_simd = _mm256_load_ps(chunk_f32.as_ptr());
    let position = _mm256_fmsub_ps(chunk_simd, grey_to_pos, grey_to_pos);
    let position_floor = _mm256_floor_ps(position);
    let idx2 = _mm256_cvtps_epi32(position_floor);
    let idx1 = _mm256_add_epi32(idx2, _mm256_set1_epi32(1));
    let idx2 = _mm256_min_epi32(idx2, colormap_len);
    let idx1 = _mm256_max_epi32(idx1, _mm256_setzero_si256());
    let ratio = _mm256_sub_ps(position, position_floor);

    // dbg!(position_floor);
    // let mut tmp = [0i32; 8];
    // _mm256_storeu_si256(tmp.as_mut_ptr() as _, idx2);
    // println!("idx2: {:?}", tmp);
    // _mm256_storeu_si256(tmp.as_mut_ptr() as _, idx1);
    // println!("idx1: {:?}", tmp);
    // dbg!(ratio);

    let mask1 = _mm256_castsi256_ps(_mm256_cmpgt_epi32(colormap_len, idx1));
    let white = _mm256_set1_ps(u8::MAX as f32);
    let rgb1 = [
        _mm256_mask_i32gather_ps::<4>(white, COLORMAP_R.as_ptr(), idx1, mask1),
        _mm256_mask_i32gather_ps::<4>(white, COLORMAP_G.as_ptr(), idx1, mask1),
        _mm256_mask_i32gather_ps::<4>(white, COLORMAP_B.as_ptr(), idx1, mask1),
    ];

    let mask2 = _mm256_castsi256_ps(_mm256_cmpgt_epi32(idx2, _mm256_set1_epi32(-1)));
    let black = _mm256_setzero_ps();
    let rgb2 = [
        _mm256_mask_i32gather_ps::<4>(black, COLORMAP_R.as_ptr(), idx2, mask2),
        _mm256_mask_i32gather_ps::<4>(black, COLORMAP_G.as_ptr(), idx2, mask2),
        _mm256_mask_i32gather_ps::<4>(black, COLORMAP_B.as_ptr(), idx2, mask2),
    ];

    let mask = _mm256_castps_si256(_mm256_cmp_ps::<_CMP_NEQ_UQ>(
        chunk_simd,
        _mm256_setzero_ps(),
    ));
    let mask_white = _mm256_castps_si256(_mm256_cmp_ps::<_CMP_EQ_UQ>(
        chunk_simd,
        _mm256_set1_ps(u16::MAX as f32),
    ));
    let white = _mm256_set1_epi32(u8::MAX as i32);
    let mut out_r8g8b8 = Aligned::<A32, _>([0; 24]);
    for (out_chunk, color1, color2) in multizip((out_r8g8b8.chunks_exact_mut(8), rgb1, rgb2)) {
        let x = _mm256_fnmadd_ps(color2, ratio, color2);
        let out_f32 = _mm256_fmadd_ps(ratio, color1, x);
        // dbg!(out_f32);
        let out = _mm256_cvtps_epi32(_mm256_round_ps::<0>(out_f32));
        _mm256_maskstore_epi32(out_chunk.as_mut_ptr() as _, mask, out);
        _mm256_maskstore_epi32(out_chunk.as_mut_ptr() as _, mask_white, white);
    }
    (0..8).cartesian_product(0..4).map(move |(i, j)| {
        if j < 3 {
            out_r8g8b8[j * 8 + i] as u8
        } else {
            u8::MAX
        }
    })
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn map_grey_to_color_iter_avx2(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    use std::arch::x86_64::*;

    let grey_to_pos_avx2 = _mm256_set1_ps(GREY_TO_POS);
    let colormap_len_avx2 = _mm256_set1_epi32(COLORMAP_R.len() as i32);
    // let grey_to_pos_sse41 = _mm_set1_ps(GREY_TO_POS);
    // let colormap_len_sse41 = _mm_set1_epi32(COLORMAP_R.len() as i32);

    let grey_avx2 = grey.chunks_exact(8);
    let grey_remainder = grey_avx2.remainder();
    // let grey_sse41 = grey_remainder.chunks_exact(4);
    // let grey_fallback = grey_sse41.remainder();
    let grey_fallback = grey_remainder;
    grey_avx2
        .flat_map(move |chunk| {
            let mut chunk_iter = chunk.iter().map(|&x| x as f32);
            let chunk_f32 = Aligned::<A32, _>([(); 8].map(|_| chunk_iter.next().unwrap()));
            map_grey_to_color_avx2(chunk_f32, grey_to_pos_avx2, colormap_len_avx2)
        })
        // .chain(grey_sse41.flat_map(move |chunk| {
        //     let mut chunk_iter = chunk.iter().map(|&x| x as f32);
        //     let chunk_f32 = Aligned::<A16, _>([(); 4].map(|_| chunk_iter.next().unwrap()));
        //     map_grey_to_color_sse41(chunk_f32, grey_to_pos_sse41, colormap_len_sse41)
        // }))
        .chain(map_grey_to_color_iter_fallback(grey_fallback))
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn map_grey_to_color_neon(
    chunk_f32: Aligned<A16, [f32; 4]>,
    grey_to_pos: float32x4_t,
    colormap_len: int32x4_t,
) -> impl Iterator<Item = u8> {
    use std::arch::aarch64::*;

    // Load the chunk into a NEON vector
    let chunk_simd = vld1q_f32(chunk_f32.as_ptr());

    // Correct computation of position
    let position = vsubq_f32(vmulq_f32(chunk_simd, grey_to_pos), grey_to_pos);

    // Floor the position to get idx2 and add 1 to get idx1
    let position_floor = vrndmq_f32(position);
    let idx2 = vcvtq_s32_f32(position_floor);
    let idx1 = vaddq_s32(idx2, vdupq_n_s32(1));

    // Clamp indices to colormap bounds
    let zero = vdupq_n_s32(0);
    let max_idx = vsubq_s32(colormap_len, vdupq_n_s32(1));
    let idx2 = vmaxq_s32(vminq_s32(idx2, max_idx), zero);
    let idx1 = vmaxq_s32(vminq_s32(idx1, colormap_len), zero);

    // Compute the ratio
    let ratio = vsubq_f32(position, position_floor);

    // Masks for special cases
    let mask_zero = vceqq_f32(chunk_simd, vdupq_n_f32(0.0));
    let mask_max = vceqq_f32(chunk_simd, vdupq_n_f32(u16::MAX as f32));
    let mask_normal = vmvnq_u32(vorrq_u32(mask_zero, mask_max));

    // Prepare output arrays
    let mut out_r8g8b8 = [0u8; 12]; // 4 pixels x 3 color channels

    // Process each color channel
    for c in 0..3 {
        // Assuming COLORMAP_R, COLORMAP_G, COLORMAP_B are &[f32]
        let colormap = match c {
            0 => COLORMAP_R,
            1 => COLORMAP_G,
            _ => COLORMAP_B,
        };

        // Emulate gather operation for idx1 and idx2
        let idx1_array = [
            vgetq_lane_s32::<0>(idx1),
            vgetq_lane_s32::<1>(idx1),
            vgetq_lane_s32::<2>(idx1),
            vgetq_lane_s32::<3>(idx1),
        ];
        let idx2_array = [
            vgetq_lane_s32::<0>(idx2),
            vgetq_lane_s32::<1>(idx2),
            vgetq_lane_s32::<2>(idx2),
            vgetq_lane_s32::<3>(idx2),
        ];

        // Load colors for idx1 and idx2
        let mut color1 = vdupq_n_f32(255.0);
        if idx1_array[0] < 256 {
            color1 = vsetq_lane_f32(colormap[idx1_array[0] as usize], color1, 0);
        }
        if idx1_array[1] < 256 {
            color1 = vsetq_lane_f32(colormap[idx1_array[1] as usize], color1, 1);
        }
        if idx1_array[2] < 256 {
            color1 = vsetq_lane_f32(colormap[idx1_array[2] as usize], color1, 2);
        }
        if idx1_array[3] < 256 {
            color1 = vsetq_lane_f32(colormap[idx1_array[3] as usize], color1, 3);
        }

        let color2 = vsetq_lane_f32(colormap[idx2_array[0] as usize], vdupq_n_f32(0.0), 0);
        let color2 = vsetq_lane_f32(colormap[idx2_array[1] as usize], color2, 1);
        let color2 = vsetq_lane_f32(colormap[idx2_array[2] as usize], color2, 2);
        let color2 = vsetq_lane_f32(colormap[idx2_array[3] as usize], color2, 3);

        // Compute interpolated color
        let interpolated = vmlaq_f32(
            vmulq_f32(color1, ratio),
            color2,
            vsubq_f32(vdupq_n_f32(1.0), ratio),
        );

        // Apply masks
        let masked_color = vbslq_f32(mask_normal, interpolated, vdupq_n_f32(0.0));
        let masked_color = vbslq_f32(mask_max, vdupq_n_f32(255.0), masked_color);

        // Convert to u8 and store in the output array
        let color_u32 = vcvtq_u32_f32(vrndaq_f32(masked_color));
        let color_u16 = vmovn_u32(color_u32);
        let color_u8 = vmovn_u16(vcombine_u16(color_u16, vdup_n_u16(0)));

        // Store the color components
        vst1_lane_u8(&mut out_r8g8b8[c * 4 + 0] as *mut u8, color_u8, 0);
        vst1_lane_u8(&mut out_r8g8b8[c * 4 + 1] as *mut u8, color_u8, 1);
        vst1_lane_u8(&mut out_r8g8b8[c * 4 + 2] as *mut u8, color_u8, 2);
        vst1_lane_u8(&mut out_r8g8b8[c * 4 + 3] as *mut u8, color_u8, 3);
    }

    // Create an iterator over the output pixels
    (0..4).cartesian_product(0..4).map(move |(i, j)| {
        if j < 3 {
            out_r8g8b8[j * 4 + i]
        } else {
            u8::MAX
        }
    })
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn map_grey_to_color_iter_neon(grey: &[u16]) -> impl Iterator<Item = u8> + use<'_> {
    use std::arch::aarch64::*;

    let grey_to_pos_neon = vdupq_n_f32(GREY_TO_POS);
    let colormap_len_neon = vdupq_n_s32(COLORMAP_R.len() as i32);

    let grey_neon = grey.chunks_exact(4);
    let grey_remainder = grey_neon.remainder();
    let grey_fallback = grey_remainder;
    grey_neon
        .flat_map(move |chunk| {
            let mut chunk_iter = chunk.iter().map(|&x| x as f32);
            let chunk_f32 = Aligned::<A16, _>([(); 4].map(|_| chunk_iter.next().unwrap()));
            map_grey_to_color_neon(chunk_f32, grey_to_pos_neon, colormap_len_neon)
        })
        .chain(map_grey_to_color_iter_fallback(grey_fallback))
}

fn map_grey_to_color_iter(grey: &[u16]) -> Box<dyn Iterator<Item = u8> + '_> {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::is_x86_feature_detected;

        if is_x86_feature_detected!("avx2") {
            return unsafe { Box::new(map_grey_to_color_iter_avx2(grey)) };
        } /* else if is_x86_feature_detected!("sse4.1") {
              return unsafe { Box::new(map_grey_to_color_iter_sse41(grey)) };
          } */
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected;

        if is_aarch64_feature_detected!("neon") {
            return unsafe { Box::new(map_grey_to_color_iter_neon(grey)) };
        }
    }
    Box::new(map_grey_to_color_iter_fallback(grey))
}

fn colorize_resize_grey(
    grey: ArrWithSliceInfo<pixels::U16, Ix2>,
    width: u32,
    height: u32,
    fast_resize: bool,
    parallel: bool,
) -> Vec<u8> {
    thread_local! {
        static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
    }

    // let start = Instant::now();
    let (grey, trim_left, trim_width) = (grey.arr, grey.index, grey.length);
    let resized_buf = RESIZER.with_borrow_mut(|resizer| {
        let src_image = TypedImageRef::new(
            grey.shape()[1] as u32,
            grey.shape()[0] as u32,
            grey.as_slice().unwrap(),
        )
        .unwrap();
        let resize_opt = ResizeOptions::new()
            .crop(
                trim_left as f64,
                0.,
                trim_width as f64,
                src_image.height() as f64,
            )
            .resize_alg(ResizeAlg::Convolution(if fast_resize {
                FilterType::Bilinear
            } else {
                FilterType::Lanczos3
            }));

        let mut dst_buf = vec![0; width as usize * height as usize * 2];
        let mut dst_image =
            TypedImage::<pixels::U16>::from_buffer(width, height, &mut dst_buf).unwrap();
        resizer
            .resize_typed(&src_image, &mut dst_image, &resize_opt)
            .unwrap();
        dst_buf
    });
    let resized = unsafe {
        std::slice::from_raw_parts(resized_buf.as_ptr() as *const u16, resized_buf.len() / 2)
    };

    if parallel {
        resized
            .par_chunks(rayon::current_num_threads())
            .flat_map_iter(map_grey_to_color_iter)
            .collect()
    } else {
        map_grey_to_color_iter(resized).collect()
    }
    // println!("drawing spec: {:?}", start.elapsed());
}

/// blend can be < 0 for not drawing spec
fn draw_blended_spec_wav(
    spec_grey: ArrWithSliceInfo<pixels::U16, Ix2>,
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    blend: f64,
    fast_resize: bool,
    show_clipping: bool,
    parallel: bool,
) -> Vec<u8> {
    // spec
    if spec_grey.length == 0 || wav.length == 0 {
        return vec![0u8; height as usize * width as usize * 4];
    }
    let mut result = if blend > 0. {
        colorize_resize_grey(spec_grey, width, height, fast_resize, parallel)
    } else {
        vec![0u8; height as usize * width as usize * 4]
    };

    let mut pixmap = PixmapMut::from_bytes(&mut result, width, height).unwrap();

    if blend < 1. {
        // wave
        let mut wav_pixmap = Pixmap::new(width, height).unwrap();
        draw_wav_to(
            wav_pixmap.data_mut(),
            wav,
            width,
            height,
            opt_for_wav,
            show_clipping,
            blend != 0.,
        );
        blend_wav_img_to(&mut pixmap, wav_pixmap.as_ref(), blend, (0, width));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::{Duration, Instant};

    use image::RgbImage;
    use ndarray_rand::{rand_distr::Uniform, RandomExt};

    #[test]
    fn show_colorbar() {
        let (width, height) = (50, 500);
        let colormap: Vec<pixels::U8x3> = multizip((COLORMAP_R, COLORMAP_G, COLORMAP_B))
            .rev()
            .map(|(r, g, b)| [r as u8, g as u8, b as u8])
            .map(pixels::U8x3::new)
            .collect();
        let src_image = TypedImageRef::new(1, colormap.len() as u32, colormap.as_slice()).unwrap();
        let mut dst_image = TypedImage::new(width, height);
        let options =
            ResizeOptions::new().resize_alg(ResizeAlg::Interpolation(FilterType::Bilinear));
        Resizer::new()
            .resize_typed(&src_image, &mut dst_image, &options)
            .unwrap();
        let dst_raw_vec = dst_image
            .pixels()
            .into_iter()
            .map(|x| x.0)
            .flatten()
            .collect();
        RgbImage::from_raw(width, height, dst_raw_vec)
            .unwrap()
            .save("samples/colorbar.png")
            .unwrap();
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn grey_to_color_work_with_avx2() {
        use std::arch::is_x86_feature_detected;

        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let mut sum_elapsed = Duration::ZERO;
        let mut sum_elapsed_avx2 = Duration::ZERO;
        for _ in 0..10 {
            let grey_arr = Array::random((149, 110), Uniform::new_inclusive(0, u16::MAX));
            let (grey, _) = grey_arr.into_raw_vec_and_offset();
            let grey_len = grey.len();
            let start_time = Instant::now();
            let rgba_avx2: Vec<_> = unsafe { map_grey_to_color_iter_avx2(&grey).collect() };
            sum_elapsed_avx2 += start_time.elapsed();
            let start_time = Instant::now();
            let rgba: Vec<_> = map_grey_to_color_iter_fallback(&grey).collect();
            sum_elapsed += start_time.elapsed();
            multizip((grey, rgba_avx2.chunks(4), rgba.chunks(4))).enumerate().for_each(|(i, (x, y_avx2, y))| {
                assert_eq!(
                    y_avx2, y,
                    "the difference between avx2 output {:?} and the answer {:?} is too large for the {}-th grey value {} (grey len: {})",
                    y_avx2, y, i, x, grey_len
                );
            });
        }
        println!(
            "AVX2 operations reduced {:.2} % of the elapsed duration.",
            100. - sum_elapsed_avx2.as_secs_f64() / sum_elapsed.as_secs_f64() * 100.
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn _grey_to_color_work_with_sse41() {
        use std::arch::is_x86_feature_detected;

        if !is_x86_feature_detected!("sse4.1") {
            return;
        }
        let mut sum_elapsed = Duration::ZERO;
        let mut sum_elapsed_sse41 = Duration::ZERO;
        for i in 0..10 {
            let grey_arr = Array::random((149, 110), Uniform::new_inclusive(0, u16::MAX));
            let (grey, _) = grey_arr.into_raw_vec_and_offset();
            let grey_len = grey.len();
            let start_time = Instant::now();
            let rgba_sse41: Vec<_> = unsafe { _map_grey_to_color_iter_sse41(&grey).collect() };
            if i > 0 {
                sum_elapsed_sse41 += start_time.elapsed();
            }
            let start_time = Instant::now();
            let rgba: Vec<_> = map_grey_to_color_iter_fallback(&grey).collect();
            if i > 0 {
                sum_elapsed += start_time.elapsed();
            }
            multizip((grey, rgba_sse41.chunks(4), rgba.chunks(4))).enumerate().for_each(|(i, (x, y_avx2, y))| {
                assert_eq!(
                    y_avx2, y,
                    "the difference between sse4.1 output {:?} and the answer {:?} is too large for the {}-th grey value {} (grey len: {})",
                    y_avx2, y, i, x, grey_len
                );
            });
        }
        println!(
            "AVX2 operations reduced {:.2} % of the elapsed duration.",
            100. - sum_elapsed_sse41.as_secs_f64() / sum_elapsed.as_secs_f64() * 100.
        );
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn grey_to_color_work_with_neon() {
        use std::arch::is_aarch64_feature_detected;

        if !is_aarch64_feature_detected!("neon") {
            return;
        }
        let mut sum_elapsed = Duration::ZERO;
        let mut sum_elapsed_neon = Duration::ZERO;
        for _ in 0..10 {
            let grey_arr = Array::random((149, 110), Uniform::new_inclusive(0, u16::MAX));
            let (grey, _) = grey_arr.into_raw_vec_and_offset();
            let grey_len = grey.len();
            let start_time = Instant::now();
            let rgba_neon: Vec<_> = unsafe { map_grey_to_color_iter_neon(&grey).collect() };
            sum_elapsed_neon += start_time.elapsed();
            let start_time = Instant::now();
            let rgba: Vec<_> = map_grey_to_color_iter_fallback(&grey).collect();
            sum_elapsed += start_time.elapsed();
            multizip((grey, rgba_neon.chunks(4), rgba.chunks(4))).enumerate().for_each(|(i, (x, y_neon, y))| {
                assert_eq!(
                    y_neon, y,
                    "the difference between neon output {:?} and the answer {:?} is too large for the {}-th grey value {} (grey len: {})",
                    y_neon, y, i, x, grey_len
                );
            });
        }
        println!(
            "neon operations reduced {:.2} % of the elapsed duration.",
            100. - sum_elapsed_neon.as_secs_f64() / sum_elapsed.as_secs_f64() * 100.
        );
    }
}
