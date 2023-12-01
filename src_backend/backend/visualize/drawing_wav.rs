// use std::time::Instant;

use cached::proc_macro::cached;
use ndarray::prelude::*;
use ndarray_stats::QuantileExt;
use serde::{Deserialize, Serialize};
use tiny_skia::{
    BlendMode, FillRule, LineCap, Paint, PathBuilder, PixmapMut, Rect, Stroke, Transform,
};

use super::img_slice::ArrWithSliceInfo;
use super::resample::FftResampler;

const WAV_COLOR: [u8; 3] = [120, 150, 210];
const LIMITER_GAIN_COLOR: [u8; 3] = [210, 150, 120];
const CLIPPING_COLOR: [u8; 3] = [255, 0, 0];

const RESAMPLE_TAIL: usize = 500;
const THR_TOPBOTTOM_PERCENT: usize = 70;

const WAV_STROKE_BORDER_WIDTH: f32 = 1.5; // this doesn't depend on dpr

struct DprDependentConstants {
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

pub fn draw_wav_to(
    output: &mut [u8],
    wav: ArrWithSliceInfo<f32, Ix1>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    show_clipping: bool,
    need_border: bool,
) {
    // let start = Instant::now();
    let &DrawOptionForWav { amp_range, dpr } = opt_for_wav;
    let DprDependentConstants {
        thr_long_height,
        topbottom_context_size,
        wav_stroke_width,
    } = DprDependentConstants::calc(dpr);
    let stroke_border_width = if need_border {
        WAV_STROKE_BORDER_WIDTH
    } else {
        0.
    };
    let amp_to_px = get_amp_to_px_fn(amp_range, height as f32);
    let px_per_samples = width as f64 / wav.length as f64;
    let resample_ratio = quantize_px_per_samples(px_per_samples);
    let outline_len = (wav.length as f32 * resample_ratio).round() as usize;
    let clip_values = (show_clipping && (amp_range.0 < -1. || amp_range.1 > 1.))
        .then_some((amp_to_px(-1.), amp_to_px(1.)));

    let width_usize = width as usize;
    let mut out_arr = ArrayViewMut3::from_shape((height as usize, width_usize, 4), output).unwrap();
    let mut pixmap = PixmapMut::from_bytes(out_arr.as_slice_mut().unwrap(), width, height).unwrap();

    if amp_range.1 - amp_range.0 < 1e-16 {
        // over-zoomed
        let rect = Rect::from_xywh(0., 0., width as f32, height as f32).unwrap();
        let path = PathBuilder::from_rect(rect);
        let paint_wav = get_wav_paint(&WAV_COLOR);
        pixmap.fill_path(
            &path,
            &paint_wav,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
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
        stroke_line_with_clipping_to(
            &mut pixmap,
            &mut wav.slice(s![..outline_len]).iter().map(|&x| amp_to_px(x)),
            wav_stroke_width,
            clip_values,
            stroke_border_width,
        );
    } else {
        let wav = wav.as_sliced();
        let half_context_size = topbottom_context_size / 2.;
        let mean_px = amp_to_px(wav.mean().unwrap_or(0.));
        let mut top_envlop = Vec::with_capacity(outline_len);
        let mut btm_envlop = Vec::with_capacity(outline_len);
        let mut n_mean_crossing = 0;
        for i_envlop in 0..outline_len {
            let i_envlop = i_envlop as f32;
            let i_start = ((i_envlop - half_context_size) / resample_ratio)
                .round()
                .max(0.) as usize;
            let i_end =
                (((i_envlop + half_context_size) / resample_ratio).round() as usize).min(wav.len());
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
        if n_mean_crossing > outline_len * THR_TOPBOTTOM_PERCENT / 100 {
            fill_topbottom_envelope_with_clipping_to(
                &mut pixmap,
                &mut top_envlop.into_iter(),
                &mut btm_envlop.into_iter(),
                outline_len,
                clip_values,
                need_border,
            );
        } else {
            stroke_line_with_clipping_to(
                &mut pixmap,
                &mut wav.iter().map(|&x| amp_to_px(x)),
                wav_stroke_width,
                clip_values,
                stroke_border_width,
            );
        }
    }

    // println!("drawing wav: {:?}", start.elapsed());
}

pub fn draw_limiter_gain_to(
    output: &mut [u8],
    gain: ArrayView1<f32>,
    width: u32,
    height: u32,
    opt_for_wav: &DrawOptionForWav,
    draw_bottom: bool,
) {
    let &DrawOptionForWav { amp_range, dpr } = opt_for_wav;
    let half_context_size = DprDependentConstants::calc(dpr).topbottom_context_size / 2.;
    let amp_to_px = get_amp_to_px_fn(amp_range, height as f32);
    let samples_per_px = gain.len() as f32 / width as f32;

    let mut envlop_iter = (0..width).map(|i_px| {
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
            let i_end =
                (((i_px + half_context_size) * samples_per_px).round() as usize).min(gain.len());
            amp_to_px(gain.slice(s![i_start..i_end]).mean().unwrap())
        }
    });

    let width_usize = width as usize;
    let mut out_arr = ArrayViewMut3::from_shape((height as usize, width_usize, 4), output).unwrap();
    let mut pixmap = PixmapMut::from_bytes(out_arr.as_slice_mut().unwrap(), width, height).unwrap();
    let paint = get_wav_paint(&LIMITER_GAIN_COLOR);
    if draw_bottom {
        let top_px = amp_to_px(amp_range.1);
        fill_topbottom_envelope_to(
            &mut pixmap,
            &mut (0..width).map(|_| top_px),
            &mut envlop_iter,
            width_usize,
            &paint,
        );
    } else {
        let btm_px = amp_to_px(amp_range.0);
        fill_topbottom_envelope_to(
            &mut pixmap,
            &mut envlop_iter,
            &mut (0..width).map(|_| btm_px),
            width_usize,
            &paint,
        );
    }
}

fn stroke_line_with_clipping_to(
    pixmap: &mut PixmapMut,
    y_px_iter: &mut dyn ExactSizeIterator<Item = f32>,
    stroke_width: f32,
    clip_values: Option<(f32, f32)>,
    stroke_border_width: f32,
) {
    let paint = get_wav_paint(&WAV_COLOR);
    match clip_values {
        Some((bottom_clip, top_clip)) => {
            let paint_clipping = get_wav_paint(&CLIPPING_COLOR);
            let y_px_vec: Vec<_> = y_px_iter.collect();
            stroke_line_to(
                pixmap,
                &mut y_px_vec.iter().cloned(),
                stroke_width,
                &paint_clipping,
                stroke_border_width,
            );

            let mut clipped = y_px_vec.into_iter().map(|x| x.clamp(top_clip, bottom_clip));
            stroke_line_to(
                pixmap,
                &mut clipped,
                stroke_width,
                &paint,
                stroke_border_width,
            );
        }
        None => {
            stroke_line_to(pixmap, y_px_iter, stroke_width, &paint, stroke_border_width);
        }
    };
}

fn stroke_line_to(
    pixmap: &mut PixmapMut,
    y_px_iter: &mut dyn ExactSizeIterator<Item = f32>,
    stroke_width: f32,
    paint: &Paint,
    stroke_border_width: f32,
) {
    let path = {
        let len = y_px_iter.len();
        let mut pb = PathBuilder::with_capacity(len + 1, len + 1);
        let first_elem = y_px_iter.next().unwrap();
        pb.move_to(0., first_elem);
        let point_per_px = pixmap.width() as f32 / len as f32;
        for (i, y) in (1..len).zip(y_px_iter) {
            pb.line_to(i as f32 * point_per_px, y);
        }
        if len == 1 {
            pb.line_to((pixmap.width().min(2) - 1) as f32, first_elem);
        }
        pb.finish().unwrap()
    };

    let stroke = Stroke {
        width: stroke_width,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    if stroke_border_width > 0. {
        let border_stroke = Stroke {
            width: stroke_width + stroke_border_width,
            ..stroke.clone()
        };
        pixmap.stroke_path(
            &path,
            &Paint::default(),
            &border_stroke,
            Transform::identity(),
            None,
        );
    }
    pixmap.stroke_path(&path, paint, &stroke, Transform::identity(), None);
}

fn fill_topbottom_envelope_with_clipping_to(
    pixmap: &mut PixmapMut,
    top_envlop_iter: &mut dyn DoubleEndedIterator<Item = f32>,
    btm_envlop_iter: &mut dyn DoubleEndedIterator<Item = f32>,
    envlop_len: usize,
    clip_values: Option<(f32, f32)>,
    need_border: bool,
) {
    let mut paint = get_wav_paint(&WAV_COLOR);
    let path = match clip_values {
        Some((bottom_clip, top_clip)) => {
            let paint_clipping = get_wav_paint(&CLIPPING_COLOR);
            let path = fill_topbottom_envelope_to(
                pixmap,
                top_envlop_iter,
                btm_envlop_iter,
                envlop_len,
                &paint_clipping,
            );
            paint.blend_mode = BlendMode::SourceAtop;
            let rect = Rect::from_xywh(0., top_clip, pixmap.width() as f32, bottom_clip - top_clip)
                .unwrap();
            let path_rect = PathBuilder::from_rect(rect);
            pixmap.fill_path(
                &path_rect,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
            paint.set_color_rgba8(0, 0, 0, u8::MAX);
            pixmap.stroke_path(
                &path_rect,
                &paint,
                &Default::default(),
                Transform::identity(),
                None,
            );
            path
        }
        None => {
            fill_topbottom_envelope_to(pixmap, top_envlop_iter, btm_envlop_iter, envlop_len, &paint)
        }
    };

    if need_border {
        let stroke = Stroke {
            width: 0.,
            ..Default::default()
        };
        pixmap.stroke_path(
            &path,
            &Paint::default(),
            &stroke,
            Transform::identity(),
            None,
        );
    }
}

fn fill_topbottom_envelope_to(
    pixmap: &mut PixmapMut,
    top_envlop_iter: &mut dyn DoubleEndedIterator<Item = f32>,
    btm_envlop_iter: &mut dyn DoubleEndedIterator<Item = f32>,
    envlop_len: usize,
    paint: &Paint,
) -> tiny_skia::Path {
    let path = {
        let len = envlop_len * 2 + 2;
        let mut pb = PathBuilder::with_capacity(len, len);
        pb.move_to(0., top_envlop_iter.next().unwrap());
        let point_per_px = pixmap.width() as f32 / envlop_len as f32;
        for (x, y) in (1..envlop_len).zip(top_envlop_iter) {
            pb.line_to(x as f32 * point_per_px, y);
        }
        for (x, y) in (0..envlop_len).rev().zip(btm_envlop_iter.rev()) {
            pb.line_to(x as f32 * point_per_px, y);
        }
        pb.close();
        pb.finish().unwrap()
    };

    pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    path
}

#[inline]
fn get_amp_to_px_fn(amp_range: (f32, f32), height: f32) -> impl Fn(f32) -> f32 {
    let scale_factor = height / (amp_range.1 - amp_range.0);
    move |x: f32| (amp_range.1 - x) * scale_factor
}

fn quantize_px_per_samples(px_per_samples: f64) -> f32 {
    if px_per_samples > 0.75 {
        px_per_samples.round() as f32
    } else if 0.5 < px_per_samples && px_per_samples <= 0.75 {
        0.75
    } else {
        1. / (1. / px_per_samples).round() as f32
    }
}

#[inline]
fn get_wav_paint(color: &[u8; 3]) -> Paint {
    let mut paint = Paint::default();
    let &[r, g, b] = color;
    paint.set_color_rgba8(r, g, b, u8::MAX);
    paint
}

#[cached(size = 64)]
fn create_resampler(input_size: usize, output_size: usize) -> FftResampler<f32> {
    FftResampler::new(input_size, output_size)
}
