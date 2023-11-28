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
const THR_TOPBOTTOM_PERCENT: u32 = 70;

const WAV_STROKE_BORDER_WIDTH: f32 = 1.5; // this doesn't depend on dpr

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

#[inline]
fn get_wav_paint(color: &[u8; 3]) -> Paint {
    let mut paint = Paint::default();
    let &[r, g, b] = color;
    paint.set_color_rgba8(r, g, b, u8::MAX);
    paint
}

fn stroke_line(
    data: &[f32],
    stroke_width: f32,
    pixmap: &mut PixmapMut,
    paint: &Paint,
    stroke_border_width: f32,
) {
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

fn stroke_line_with_clipping(
    data: &[f32],
    stroke_width: f32,
    pixmap: &mut PixmapMut,
    clip_values: Option<(f32, f32)>,
    stroke_border_width: f32,
) {
    let paint = get_wav_paint(&WAV_COLOR);
    match clip_values {
        Some((bottom_clip, top_clip)) => {
            let paint_clipping = get_wav_paint(&CLIPPING_COLOR);
            stroke_line(
                data,
                stroke_width,
                pixmap,
                &paint_clipping,
                stroke_border_width,
            );

            let clipped: Vec<_> = data
                .iter()
                .map(|&x| x.clamp(top_clip, bottom_clip))
                .collect();
            stroke_line(&clipped, stroke_width, pixmap, &paint, stroke_border_width);
        }
        None => {
            stroke_line(data, stroke_width, pixmap, &paint, stroke_border_width);
        }
    };
}

fn fill_topbottom_envelope(
    top_envlop: &[f32],
    btm_envlop: &[f32],
    pixmap: &mut PixmapMut,
    paint: &Paint,
) -> tiny_skia::Path {
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
    path
}

fn fill_topbottom_envelope_with_clipping(
    top_envlop: &[f32],
    btm_envlop: &[f32],
    pixmap: &mut PixmapMut,
    clip_values: Option<(f32, f32)>,
    need_border: bool,
) {
    let mut paint = get_wav_paint(&WAV_COLOR);
    let path = match clip_values {
        Some((bottom_clip, top_clip)) => {
            let paint_clipping = get_wav_paint(&CLIPPING_COLOR);
            let path = fill_topbottom_envelope(top_envlop, btm_envlop, pixmap, &paint_clipping);
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
        None => fill_topbottom_envelope(top_envlop, btm_envlop, pixmap, &paint),
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

fn get_amp_to_px_fn(amp_range: (f32, f32), height: f32) -> impl Fn(f32) -> f32 {
    let scale_factor = height / (amp_range.1 - amp_range.0);
    move |x: f32| (amp_range.1 - x) * scale_factor
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
    let paint = get_wav_paint(&LIMITER_GAIN_COLOR);
    if draw_bottom {
        let top_envlop = vec![amp_to_px(amp_range.1); width as usize];
        fill_topbottom_envelope(&top_envlop, &envlop, &mut pixmap, &paint);
    } else {
        let btm_envlop = vec![amp_to_px(amp_range.0); width as usize];
        fill_topbottom_envelope(&envlop, &btm_envlop, &mut pixmap, &paint);
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
    let samples_per_px = wav.length as f32 / width as f32;
    let clip_values = (show_clipping && (amp_range.0 < -1. || amp_range.1 > 1.))
        .then_some((amp_to_px(-1.), amp_to_px(1.)));

    let mut out_arr =
        ArrayViewMut3::from_shape((height as usize, width as usize, 4), output).unwrap();
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
            clip_values,
            stroke_border_width,
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
                clip_values,
                need_border,
            );
        } else {
            let wav_px = wav.map(|&x| amp_to_px(x));
            stroke_line_with_clipping(
                wav_px.as_slice().unwrap(),
                wav_stroke_width,
                &mut pixmap,
                clip_values,
                stroke_border_width,
            );
        }
    }

    // println!("drawing wav: {:?}", start.elapsed());
}

#[cached(size = 64)]
fn create_resampler(input_size: usize, output_size: usize) -> FftResampler<f32> {
    FftResampler::new(input_size, output_size)
}
