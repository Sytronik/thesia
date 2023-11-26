use std::iter;
use std::mem::MaybeUninit;
use std::ops::Neg;
// use std::time::Instant;

use cached::proc_macro::cached;
use napi_derive::napi;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use rayon::prelude::*;
use resize::{self, formats::Gray, Pixel::GrayF32, Resizer};
use rgb::FromSlice;
use serde::{Deserialize, Serialize};
use tiny_skia::{
    BlendMode, FillRule, IntRect, LineCap, Paint, PathBuilder, PixmapMut, PixmapPaint, PixmapRef,
    Rect, Stroke, Transform,
};

use super::img_slice::{ArrWithSliceInfo, CalcWidth, OverviewHeights, PartGreyInfo};
use super::resample::FftResampler;
use crate::backend::dynamics::{GuardClippingResult, MaxPeak};
use crate::backend::utils::Pad;
use crate::backend::{IdChArr, IdChMap, TrackManager};

pub type ResizeType = resize::Type;

const BLACK: [u8; 3] = [0; 3];
const WHITE: [u8; 3] = [255; 3];
pub const COLORMAP: [[u8; 3]; 10] = [
    [0, 0, 4],
    [27, 12, 65],
    [74, 12, 107],
    [120, 28, 109],
    [165, 44, 96],
    [207, 68, 70],
    [237, 105, 37],
    [251, 155, 6],
    [247, 209, 61],
    [252, 255, 164],
];
const WAV_COLOR: [u8; 3] = [120, 150, 210];
const LIMITER_GAIN_COLOR: [u8; 3] = [210, 150, 120];
const CLIPPING_COLOR: [u8; 3] = [255, 0, 0];
const RESAMPLE_TAIL: usize = 500;
const THR_TOPBOTTOM_PERCENT: u32 = 70;
const OVERVIEW_CH_GAP_HEIGHT: f32 = 1.;
const LIMITER_GAIN_HEIGHT_DENOM: usize = 5; // 1/5 of the height will be used for draw limiter gain

pub struct DprDependentConstants {
    thr_long_height: f32,
    topbottom_context_size: f32,
    wav_stroke_width: f32,
}

impl DprDependentConstants {
    fn calc(dpr: f32) -> Self {
        DprDependentConstants {
            thr_long_height: 2. * dpr,
            topbottom_context_size: 2. * dpr,
            wav_stroke_width: 1.75 * dpr,
        }
    }
}

#[napi(object)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawOption {
    pub px_per_sec: f64,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct DrawOptionForWav {
    pub amp_range: (f32, f32),
    pub dpr: f32,
}

impl DrawOptionForWav {
    pub fn with_dpr(dpr: f32) -> Self {
        DrawOptionForWav {
            dpr,
            ..Default::default()
        }
    }
}

impl Default for DrawOptionForWav {
    fn default() -> Self {
        DrawOptionForWav {
            amp_range: (-1., 1.),
            dpr: 1.,
        }
    }
}

pub enum ImageKind {
    Spec,
    Wav(DrawOptionForWav),
}

pub trait TrackDrawer {
    fn draw_entire_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChMap<Array3<u8>>;

    fn draw_part_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
        fast_resize_vec: Option<Vec<bool>>,
    ) -> IdChMap<Vec<u8>>;

    fn draw_overview(&self, id: usize, width: u32, height: u32, dpr: f32) -> Vec<u8>;
}

impl TrackDrawer for TrackManager {
    fn draw_entire_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        option: DrawOption,
        kind: ImageKind,
    ) -> IdChMap<Array3<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = IdChMap::with_capacity(id_ch_tuples.len());
        result.par_extend(id_ch_tuples.par_iter().map(|&(id, ch)| {
            let out_for_not_exist = || ((id, ch), Array::zeros((0, 0, 0)));
            let track = if let Some(track) = self.track(id) {
                track
            } else {
                return out_for_not_exist();
            };
            let width = track.calc_width(px_per_sec);
            let shape = (height as usize, width as usize, 4);
            let arr = match kind {
                ImageKind::Spec => {
                    let grey = if let Some(grey) = self.spec_greys.get(&(id, ch)) {
                        grey.view()
                    } else {
                        return out_for_not_exist();
                    };
                    let vec = colorize_resize_grey(grey.into(), width, height, false);
                    Array3::from_shape_vec(shape, vec).unwrap()
                }
                ImageKind::Wav(opt_for_wav) => {
                    let mut arr = Array3::zeros(shape);
                    draw_wav_to(
                        arr.as_slice_mut().unwrap(),
                        track.channel(ch).into(),
                        width,
                        height,
                        &opt_for_wav,
                        None,
                        false,
                    );
                    arr
                }
            };
            ((id, ch), arr)
        }));
        // println!("draw entire: {:?}", start.elapsed());
        result
    }

    /// Draw part of images. if blend < 0, draw waveform with transparent background
    fn draw_part_imgs(
        &self,
        id_ch_tuples: &IdChArr,
        start_sec: f64,
        width: u32,
        option: DrawOption,
        opt_for_wav: DrawOptionForWav,
        blend: f64,
        fast_resize_vec: Option<Vec<bool>>,
    ) -> IdChMap<Vec<u8>> {
        // let start = Instant::now();
        let DrawOption { px_per_sec, height } = option;
        let mut result = IdChMap::with_capacity(id_ch_tuples.len());
        let par_iter = id_ch_tuples.par_iter().enumerate().map(|(i, &(id, ch))| {
            let out_for_not_exist = || ((id, ch), Vec::new());
            let track = if let Some(track) = self.track(id) {
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
            let wav_part = ArrWithSliceInfo::new(
                track.channel(ch),
                track.calc_part_wav_info(start_sec_with_margin, width_with_margin, px_per_sec),
            );
            let vec = draw_blended_spec_wav(
                spec_grey_part,
                wav_part,
                drawing_width_with_margin,
                height,
                &opt_for_wav,
                blend,
                fast_resize_vec.as_ref().map_or(false, |v| v[i]),
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
            let arr = if arr.is_standard_layout() {
                arr
            } else {
                arr.as_standard_layout().into_owned()
            };
            ((id, ch), arr.into_raw_vec())
        });
        result.par_extend(par_iter);

        // println!("draw: {:?}", start.elapsed());
        result
    }

    fn draw_overview(&self, id: usize, width: u32, height: u32, dpr: f32) -> Vec<u8> {
        let track = if let Some(track) = self.track(id) {
            track
        } else {
            return Vec::new();
        };
        let (pad_left, drawing_width, pad_right) =
            track.decompose_width_of(0., width, width as f64 / self.tracklist.max_sec);
        let (pad_left, drawing_width_usize, pad_right) = (
            pad_left as usize,
            drawing_width as usize,
            pad_right as usize,
        );
        let heights = OverviewHeights::new(height, track.n_ch(), OVERVIEW_CH_GAP_HEIGHT, dpr);
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
            .enumerate()
            .par_bridge()
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
                        None,
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
                            None,
                            true,
                        )
                    }
                    GuardClippingResult::GainSequence(gain_seq)
                        if draw_gain_heights != Default::default() =>
                    {
                        let gain_seq_ch = gain_seq.slice(s![ch, ..]);
                        let neg_gain_seq_ch = gain_seq_ch.neg();
                        let (gain_h, wav_h) = draw_gain_heights;
                        draw_wav(gain_h, wav_h);
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
                                None,
                            );
                        };
                        draw_gain(0, gain_seq_ch, (0.5, 1.), true);
                        draw_gain(gain_h + wav_h, neg_gain_seq_ch.view(), (-1., -0.5), false);
                    }
                    _ => {
                        draw_wav(0, heights.ch);
                    }
                }
            });

        if width != drawing_width {
            arr = arr.pad((pad_left, pad_right), Axis(1), Default::default());
        }
        arr.into_raw_vec()
    }
}

pub fn convert_spec_to_grey(
    spec: ArrayView2<f32>,
    up_ratio: f32,
    max: f32,
    min: f32,
) -> Array2<f32> {
    // spec: T x F
    // return: grey image with F(inverted) x T
    let width = spec.shape()[0];
    let height = (spec.shape()[1] as f32 * up_ratio).round() as usize;
    let mut grey = Array2::uninit((height, width));
    for ((i, j), x) in grey.indexed_iter_mut() {
        if height - 1 - i < spec.raw_dim()[1] {
            *x = MaybeUninit::new((spec[[j, height - 1 - i]] - min) / (max - min));
        } else {
            *x = MaybeUninit::new(0.);
        }
    }
    unsafe { grey.assume_init() }
}

pub fn make_opaque(mut image: ArrayViewMut3<u8>, left: u32, width: u32) {
    image
        .slice_mut(s![.., left as isize..(left + width) as isize, 3])
        .mapv_inplace(|_| u8::MAX);
}

pub fn blend_img(
    spec_img: &[u8],
    wav_img: &[u8],
    width: u32,
    height: u32,
    blend: f64,
    eff_l_w: Option<(u32, u32)>,
) -> Vec<u8> {
    assert!(0. < blend && blend < 1.);
    let mut result = spec_img.to_vec();
    let mut pixmap = PixmapMut::from_bytes(&mut result, width, height).unwrap();
    // black
    if let Some((left, width)) = eff_l_w {
        if blend < 0.5 && width > 0 {
            let rect = IntRect::from_xywh(left as i32, 0, width, height)
                .unwrap()
                .to_rect();
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, (u8::MAX as f64 * (1. - 2. * blend)).round() as u8);
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
    {
        let paint = PixmapPaint {
            opacity: (2. - 2. * blend).min(1.) as f32,
            ..Default::default()
        };
        pixmap.draw_pixmap(
            0,
            0,
            PixmapRef::from_bytes(wav_img, width, height).unwrap(),
            &paint,
            Transform::identity(),
            None,
        );
    }
    result
}

#[inline]
pub fn get_colormap_rgb() -> Vec<u8> {
    COLORMAP.iter().flat_map(|x| x.iter().cloned()).collect()
}

#[inline]
fn interpolate(rgba1: &[u8], rgba2: &[u8], ratio: f32) -> Vec<u8> {
    rgba1
        .iter()
        .zip(rgba2)
        .map(|(&a, &b)| (ratio * a as f32 + (1. - ratio) * b as f32).round() as u8)
        .collect()
}

fn map_grey_to_color(x: f32) -> Vec<u8> {
    if x < 0. {
        return BLACK.to_vec();
    }
    if x >= 1. {
        return WHITE.to_vec();
    }
    let position = x * COLORMAP.len() as f32;
    let index = position.floor() as usize;
    let rgba1 = if index >= COLORMAP.len() - 1 {
        &WHITE
    } else {
        &COLORMAP[index + 1]
    };
    interpolate(rgba1, &COLORMAP[index], position - index as f32)
}

fn colorize_resize_grey(
    grey: ArrWithSliceInfo<f32, Ix2>,
    width: u32,
    height: u32,
    fast_resize: bool,
) -> Vec<u8> {
    // let start = Instant::now();
    let (grey, trim_left, trim_width) = (grey.arr, grey.index, grey.length);
    let mut resizer = create_resizer(
        trim_width,
        grey.shape()[0],
        width as usize,
        height as usize,
        fast_resize,
    );
    let mut resized = vec![0f32; width as usize * height as usize];
    resizer
        .resize_stride(
            grey.as_slice().unwrap()[trim_left..].as_gray(),
            grey.shape()[1],
            resized.as_gray_mut(),
        )
        .unwrap();
    resized
        .into_iter()
        .flat_map(|x| map_grey_to_color(x).into_iter().chain(iter::once(u8::MAX)))
        .collect()
    // println!("drawing spec: {:?}", start.elapsed());
}

fn get_wav_paint(color: &[u8; 3], alpha: u8) -> Paint {
    let mut paint = Paint::default();
    let &[r, g, b] = color;
    paint.set_color_rgba8(r, g, b, alpha);
    paint.anti_alias = true;
    paint
}

fn stroke_line(data: &[f32], stroke_width: f32, pixmap: &mut PixmapMut, paint: &Paint) {
    let path = {
        let mut pb = PathBuilder::with_capacity(data.len() + 1, data.len() + 1);
        pb.move_to(0., data[0]);
        for (i, &y) in data.iter().enumerate().skip(1) {
            pb.line_to((i * pixmap.width() as usize) as f32 / data.len() as f32, y);
        }
        if data.len() == 1 {
            pb.line_to((pixmap.width().min(2) - 1) as f32, data[0]);
        }
        pb.finish().unwrap()
    };

    let stroke = Stroke {
        width: stroke_width,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    pixmap.stroke_path(&path, paint, &stroke, Transform::identity(), None);
}

fn stroke_line_with_clipping(
    data: &[f32],
    stroke_width: f32,
    pixmap: &mut PixmapMut,
    alpha: u8,
    clip_values: Option<(f32, f32)>,
) {
    let paint = get_wav_paint(&WAV_COLOR, alpha);
    match clip_values {
        Some((bottom_clip, top_clip)) => {
            let paint_clipping = get_wav_paint(&CLIPPING_COLOR, alpha);
            stroke_line(data, stroke_width, pixmap, &paint_clipping);

            let clipped: Vec<_> = data
                .iter()
                .map(|&x| x.clamp(top_clip, bottom_clip))
                .collect();
            stroke_line(&clipped, stroke_width, pixmap, &paint);
        }
        None => {
            stroke_line(data, stroke_width, pixmap, &paint);
        }
    };
}

fn fill_topbottom_envelope(
    top_envlop: &[f32],
    btm_envlop: &[f32],
    pixmap: &mut PixmapMut,
    paint: &Paint,
) {
    let path = {
        let len = top_envlop.len() + btm_envlop.len() + 2;
        let mut pb = PathBuilder::with_capacity(len, len);
        pb.move_to(0., top_envlop[0]);
        for (x, &y) in top_envlop.iter().enumerate().skip(1) {
            pb.line_to(x as f32, y);
        }
        for (x, &y) in btm_envlop.iter().enumerate().rev() {
            pb.line_to(x as f32, y);
        }
        pb.close();
        pb.finish().unwrap()
    };

    pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
}

fn fill_topbottom_envelope_with_clipping(
    top_envlop: &[f32],
    btm_envlop: &[f32],
    pixmap: &mut PixmapMut,
    alpha: u8,
    clip_values: Option<(f32, f32)>,
) {
    let mut paint = get_wav_paint(&WAV_COLOR, alpha);
    match clip_values {
        Some((bottom_clip, top_clip)) => {
            let paint_clipping = get_wav_paint(&CLIPPING_COLOR, alpha);
            fill_topbottom_envelope(top_envlop, btm_envlop, pixmap, &paint_clipping);
            paint.blend_mode = BlendMode::SourceAtop;
            let rect = Rect::from_xywh(0., top_clip, pixmap.width() as f32, bottom_clip - top_clip)
                .unwrap();
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
        None => {
            fill_topbottom_envelope(top_envlop, btm_envlop, pixmap, &paint);
        }
    };
}

fn get_amp_to_px_fn(amp_range: (f32, f32), height: f32) -> impl Fn(f32) -> f32 {
    let scale_factor = height / (amp_range.1 - amp_range.0);
    move |x: f32| (amp_range.1 - x) * scale_factor
}

fn draw_limiter_gain_to(
    output: &mut [u8],
    gain: ArrayView1<f32>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    draw_bottom: bool,
    alpha: Option<u8>,
) {
    let &DrawOptionForWav { amp_range, dpr } = opt_for_wav;
    let half_context_size = DprDependentConstants::calc(dpr).topbottom_context_size / 2.;
    let amp_to_px = get_amp_to_px_fn(amp_range, height as f32);
    let samples_per_px = gain.len() as f32 / width as f32;

    let envlop: Vec<_> = (0..width)
        .map(|i_px| {
            let i_px = i_px as f32;
            let i_mid = (i_px * samples_per_px).round() as usize;
            if gain[i_mid.max(1) - 1] == gain[i_mid]
                || gain[i_mid] == gain[i_mid.min(gain.len() - 2) + 1]
            {
                amp_to_px(gain[i_mid])
            } else {
                let i_start = ((i_px - half_context_size) * samples_per_px)
                    .round()
                    .max(0.) as usize;
                let i_end = (((i_px + half_context_size) * samples_per_px).round() as usize)
                    .min(gain.len());
                amp_to_px(gain.slice(s![i_start..i_end]).mean().unwrap())
            }
        })
        .collect();

    let mut out_arr =
        ArrayViewMut3::from_shape((height as usize, width as usize, 4), output).unwrap();
    let mut pixmap = PixmapMut::from_bytes(out_arr.as_slice_mut().unwrap(), width, height).unwrap();
    let paint = get_wav_paint(&LIMITER_GAIN_COLOR, alpha.unwrap_or(u8::MAX));
    if draw_bottom {
        let top_envlop = vec![amp_to_px(amp_range.1); width as usize];
        fill_topbottom_envelope(&top_envlop, &envlop, &mut pixmap, &paint);
    } else {
        let btm_envlop = vec![amp_to_px(amp_range.0); width as usize];
        fill_topbottom_envelope(&envlop, &btm_envlop, &mut pixmap, &paint);
    }
}

fn draw_wav_to(
    output: &mut [u8],
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    alpha: Option<u8>,
    show_clipping: bool,
) {
    // let start = Instant::now();
    let &DrawOptionForWav { amp_range, dpr } = opt_for_wav;
    let DprDependentConstants {
        thr_long_height,
        topbottom_context_size,
        wav_stroke_width,
    } = DprDependentConstants::calc(dpr);
    let amp_to_px = get_amp_to_px_fn(amp_range, height as f32);
    let samples_per_px = wav.length as f32 / width as f32;
    let alpha = alpha.unwrap_or(u8::MAX);
    let clip_values = show_clipping.then_some((amp_to_px(-1.), amp_to_px(1.)));

    let mut out_arr =
        ArrayViewMut3::from_shape((height as usize, width as usize, 4), output).unwrap();
    let mut pixmap = PixmapMut::from_bytes(out_arr.as_slice_mut().unwrap(), width, height).unwrap();

    if amp_range.1 - amp_range.0 < 1e-16 {
        // over-zoomed
        let rect = Rect::from_xywh(0., 0., width as f32, height as f32).unwrap();
        let paint_wav = get_wav_paint(&WAV_COLOR, alpha);
        pixmap.fill_rect(rect, &paint_wav, Transform::identity(), None);
    } else if samples_per_px < 2. {
        // upsampling
        let wav_tail = wav.as_sliced_with_tail(RESAMPLE_TAIL);
        let width_tail = (width as f32 * wav_tail.len() as f32 / wav.length as f32).round();
        let mut resampler = create_resampler(wav_tail.len(), width_tail as usize);
        let upsampled = resampler.resample(wav_tail).mapv(amp_to_px);
        let wav_px = upsampled.slice_move(s![..width as usize]);
        stroke_line_with_clipping(
            wav_px.as_slice().unwrap(),
            wav_stroke_width,
            &mut pixmap,
            alpha,
            clip_values,
        );
    } else {
        let wav = wav.as_sliced();
        let half_context_size = topbottom_context_size / 2.;
        let mean_px = amp_to_px(wav.mean().unwrap_or(0.));
        let mut top_envlop = Vec::with_capacity(width as usize);
        let mut btm_envlop = Vec::with_capacity(width as usize);
        let mut n_mean_crossing = 0u32;
        for i_px in 0..width {
            let i_px = i_px as f32;
            let i_start = ((i_px - half_context_size) * samples_per_px)
                .round()
                .max(0.) as usize;
            let i_end =
                (((i_px + half_context_size) * samples_per_px).round() as usize).min(wav.len());
            let wav_slice = wav.slice(s![i_start..i_end]);
            let top = amp_to_px(*wav_slice.max_skipnan()) - wav_stroke_width / 2.;
            let bottom = amp_to_px(*wav_slice.min_skipnan()) + wav_stroke_width / 2.;
            if top < mean_px + f32::EPSILON && bottom > mean_px - thr_long_height
                || top < mean_px + thr_long_height && bottom > mean_px - f32::EPSILON
            {
                n_mean_crossing += 1;
            }
            top_envlop.push(top);
            btm_envlop.push(bottom);
        }
        if n_mean_crossing > width * THR_TOPBOTTOM_PERCENT / 100 {
            fill_topbottom_envelope_with_clipping(
                &top_envlop,
                &btm_envlop,
                &mut pixmap,
                alpha,
                clip_values,
            );
        } else {
            let wav_px = wav.map(|&x| amp_to_px(x));
            stroke_line_with_clipping(
                wav_px.as_slice().unwrap(),
                wav_stroke_width,
                &mut pixmap,
                alpha,
                clip_values,
            );
        }
    }

    // println!("drawing wav: {:?}", start.elapsed());
}

fn draw_blended_spec_wav(
    spec_grey: ArrWithSliceInfo<f32, Ix2>,
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    blend: f64,
    fast_resize: bool,
) -> Vec<u8> {
    // spec
    if spec_grey.length == 0 || wav.length == 0 {
        return vec![0u8; height as usize * width as usize * 4];
    }
    let mut result = if blend > 0. {
        colorize_resize_grey(spec_grey, width, height, fast_resize)
    } else {
        vec![0u8; height as usize * width as usize * 4]
    };

    let mut pixmap = PixmapMut::from_bytes(&mut result, width, height).unwrap();

    if blend < 1. {
        // black
        if (0.0..0.5).contains(&blend) {
            let rect = IntRect::from_xywh(0, 0, width, height).unwrap().to_rect();
            let mut paint = Paint::default();
            let alpha = (u8::MAX as f64 * (1. - 2. * blend)).round() as u8;
            paint.set_color_rgba8(0, 0, 0, alpha);
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }

        let alpha = (u8::MAX as f64 * (2. - 2. * blend).min(1.)).round() as u8;
        // wave
        draw_wav_to(
            pixmap.data_mut(),
            wav,
            width,
            height,
            opt_for_wav,
            Some(alpha),
            false,
        );
    }
    result
}

#[cached(size = 64)]
fn create_resizer(
    src_width: usize,
    src_height: usize,
    dest_width: usize,
    dest_height: usize,
    fast_resize: bool,
) -> Resizer<Gray<f32, f32>> {
    resize::new(
        src_width,
        src_height,
        dest_width,
        dest_height,
        GrayF32,
        if fast_resize {
            ResizeType::Triangle
        } else {
            ResizeType::Lanczos3
        },
    )
    .unwrap()
}

#[cached(size = 64)]
fn create_resampler(input_size: usize, output_size: usize) -> FftResampler<f32> {
    FftResampler::new(input_size, output_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::RgbImage;
    use resize::Pixel::RGB8;

    #[test]
    fn show_colorbar() {
        let (width, height) = (50, 500);
        let colormap: Vec<u8> = COLORMAP.iter().rev().flatten().cloned().collect();
        let mut imvec = vec![0u8; width * height * 3];
        let mut resizer = resize::new(1, 10, width, height, RGB8, ResizeType::Triangle).unwrap();
        resizer
            .resize(&colormap.as_rgb(), imvec.as_rgb_mut())
            .unwrap();

        RgbImage::from_raw(width as u32, height as u32, imvec)
            .unwrap()
            .save("samples/colorbar.png")
            .unwrap();
    }
}
