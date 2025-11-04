use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use web_sys::Path2d;

use crate::resample::resample;
use crate::simd::{
    add_scalar_inplace, clamp_inplace, find_max, find_min, find_min_max, fused_mul_add, negate,
};
use crate::wav::{WAV_MARGIN_PX, WavDrawingOptions};

const UPSAMPLE_MARGIN_SEC: f32 = 0.1;

#[derive(Debug)]
pub(crate) struct WavLinePoints {
    xs: Vec<f32>,
    ys: Vec<f32>,
}

impl WavLinePoints {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self {
            xs: Vec::new(),
            ys: Vec::new(),
        }
    }

    #[inline(always)]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            xs: Vec::with_capacity(capacity),
            ys: Vec::with_capacity(capacity),
        }
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, x: f32, y: f32) {
        self.xs.push(x);
        self.ys.push(y);
    }

    pub(crate) fn try_into_path(self) -> Result<Path2d, JsValue> {
        let path = Path2d::new()?;
        if self.is_empty() {
            return Ok(path);
        }
        path.move_to(self.xs[0] as f64, self.ys[0] as f64);
        for (x, y) in self.xs.into_iter().zip(self.ys.into_iter()).skip(1) {
            path.line_to(x as f64, y as f64);
        }
        Ok(path)
    }

    pub(crate) fn slice_transform(
        &self,
        start_x: f32,
        end_x: f32,
        params: &TransformParams,
    ) -> Self {
        thread_local! {
            static TMP_BUFFER: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
        }
        let mut i_start = self.len();
        let mut i_end = self.len();
        for (i, x) in self.xs.iter().enumerate() {
            if *x < start_x {
                continue;
            }
            if i_start == self.len() {
                i_start = i;
            }
            if *x >= end_x {
                i_end = i;
                break;
            }
        }
        let mut out = Self::with_capacity(i_end - i_start);
        fused_mul_add(
            &self.xs[i_start..i_end],
            params.x_scale,
            params.x_offset,
            &mut out.xs,
        );
        TMP_BUFFER.with_borrow_mut(|buf| {
            buf.clear();
            fused_mul_add(
                &self.ys[i_start..i_end],
                params.y2v_scale,
                params.y2v_offset,
                buf,
            );
            clamp_inplace(buf, params.v_clip_values.0, params.v_clip_values.1);
            fused_mul_add(buf, params.v2y_scale, params.v2y_offset, &mut out.ys);
            buf.clear();
        });
        out
    }

    #[inline(always)]
    pub(crate) fn shift_y_inplace(&mut self, offset_y: f32) {
        add_scalar_inplace(&mut self.ys, offset_y);
    }

    #[inline(always)]
    pub(crate) fn upside_down(&self) -> Self {
        let mut out = Self::with_capacity(self.len());
        out.xs.clone_from(&self.xs);
        negate(&self.ys, &mut out.ys);
        out
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.xs.is_empty()
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.xs.len()
    }
}

impl TryFrom<WavLinePoints> for Path2d {
    type Error = JsValue;

    #[inline(always)]
    fn try_from(value: WavLinePoints) -> Result<Self, Self::Error> {
        value.try_into_path()
    }
}

#[derive(Debug)]
pub(crate) struct WavEnvelope {
    xs: Vec<f32>,
    tops: Vec<f32>,
    bottoms: Vec<f32>,
}

impl WavEnvelope {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self {
            xs: Vec::new(),
            tops: Vec::new(),
            bottoms: Vec::new(),
        }
    }

    #[inline(always)]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            xs: Vec::with_capacity(capacity),
            tops: Vec::with_capacity(capacity),
            bottoms: Vec::with_capacity(capacity),
        }
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, x: f32, top: f32, bottom: f32) {
        self.xs.push(x);
        self.tops.push(top);
        self.bottoms.push(bottom);
    }

    pub(crate) fn record_on_path(
        mut self,
        path: &Path2d,
        stroke_width: f32,
    ) -> Result<(), JsValue> {
        if self.is_empty() {
            return Ok(());
        }

        let half_stroke_width = stroke_width / 2.0;

        add_scalar_inplace(&mut self.tops, -half_stroke_width);
        add_scalar_inplace(&mut self.bottoms, half_stroke_width);

        // Move to first point
        path.move_to(self.xs[0] as f64, self.tops[0] as f64);

        // Draw top envelope
        for (x, y) in self.xs.iter().zip(self.tops.iter()).skip(1) {
            path.line_to(*x as f64, *y as f64);
        }

        // Draw bottom envelope (reversed)
        for (x, y) in self.xs.iter().zip(self.bottoms.iter()).rev() {
            path.line_to(*x as f64, *y as f64);
        }

        path.close_path();
        Ok(())
    }

    pub(crate) fn slice_transform(
        &self,
        start_x: f32,
        end_x: f32,
        params: &TransformParams,
        scale: f32,
    ) -> Self {
        thread_local! {
            static TMP_BUFFER: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
        }
        let mut i_start = self.len();
        let mut i_end = self.len();
        for (i, x) in self.xs.iter().enumerate() {
            if *x < start_x {
                continue;
            }
            if i_start == self.len() {
                i_start = i;
            }
            if *x >= end_x {
                i_end = i;
                break;
            }
        }

        let mut out = Self::with_capacity(i_end - i_start);
        TMP_BUFFER.with_borrow_mut(|buf| {
            buf.clear();
            fused_mul_add(
                &self.xs[i_start..i_end],
                params.x_scale,
                params.x_offset,
                buf,
            );

            // downsampling
            let floor_x = |x: f32| (x / scale).floor() * scale;
            if i_start == 0 {
                out.xs.push(floor_x(buf[0]) + scale / 2.0);
                out.tops.push(self.tops[0]);
                out.bottoms.push(self.bottoms[0]);
            }

            // exclude the first and last points
            let i_start2 = i_start.max(1);
            let i_end2 = i_end.min(self.len() - 1);

            let mut i = i_start2;
            while i < i_end2 {
                let x = buf[i - i_start];
                let x_floor = floor_x(x);
                let x_mid = x_floor + scale / 2.0;
                let mut i2 = i;
                while i2 < i_end2 {
                    let x2 = buf[i2 - i_start];
                    let x2_floor = floor_x(x2);
                    if x2_floor > x_floor {
                        break;
                    }
                    i2 += 1;
                }
                if i2 == i {
                    i2 = (i + 1).min(i_end2);
                }
                out.xs.push(x_mid);
                out.tops.push(find_min(&self.tops[i..i2]));
                out.bottoms.push(find_max(&self.bottoms[i..i2]));
                i = i2;
            }
            if i_end == self.len() {
                out.xs.push(floor_x(buf[i_end - i_start - 1]) + scale / 2.0);
                out.tops.push(self.tops[i_end - 1]);
                out.bottoms.push(self.bottoms[i_end - 1]);
            }
            buf.clear();

            fused_mul_add(&out.tops, params.y2v_scale, params.y2v_offset, buf);
            clamp_inplace(buf, params.v_clip_values.0, params.v_clip_values.1);
            out.tops.clear();
            fused_mul_add(buf, params.v2y_scale, params.v2y_offset, &mut out.tops);
            buf.clear();

            fused_mul_add(&out.bottoms, params.y2v_scale, params.y2v_offset, buf);
            clamp_inplace(buf, params.v_clip_values.0, params.v_clip_values.1);
            out.bottoms.clear();
            fused_mul_add(buf, params.v2y_scale, params.v2y_offset, &mut out.bottoms);
            buf.clear();
        });
        out
    }

    #[inline(always)]
    pub(crate) fn out_of_range(&self, start_x: f32, end_x: f32) -> bool {
        self.xs[0] >= end_x || self.xs[self.len() - 1] < start_x
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.xs.is_empty()
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.xs.len()
    }
}

pub(crate) struct CalcLineEnvelopePointsResult {
    pub(crate) line_points: WavLinePoints,
    pub(crate) envelopes: Option<Vec<WavEnvelope>>,
    pub(crate) upsampled_wav_sr: Option<(Vec<f32>, u32)>,
}

pub(crate) fn calc_line_envelope_points(
    wav: &[f32],
    sr: u32,
    options: &WavDrawingOptions,
    upsampled_wav_sr: Option<(Vec<f32>, u32)>,
) -> CalcLineEnvelopePointsResult {
    let px_per_sec = options.px_per_sec();
    let stroke_width = options.line_width();
    let sr_f32 = sr as f32;

    let x_scale = px_per_sec / sr_f32;
    let x_offset = -options.start_sec * px_per_sec;
    let idx_to_x = |i: f32| i.mul_add(x_scale, x_offset);
    let floor_x = |x: f32| ((x - x_offset) / options.scale).floor() * options.scale + x_offset;

    let (clip_min, clip_max) = options
        .clip_values
        .unwrap_or((-f32::INFINITY, f32::INFINITY));
    let y_scale = -options.height / (options.amp_range.1 - options.amp_range.0).max(1e-8);
    let y_offset = options.offset_y - options.amp_range.1 * y_scale;
    let wav_to_y = |v: f32| v.max(clip_min).min(clip_max).mul_add(y_scale, y_offset);

    let margin_sec = WAV_MARGIN_PX / px_per_sec;
    let i_start = ((options.start_sec - margin_sec) * sr_f32).floor().max(0.0) as usize;
    let i_end = wav.len().min(
        ((options.start_sec + options.width / px_per_sec + margin_sec) * sr_f32).ceil() as usize,
    );

    if px_per_sec > sr_f32 / 2. {
        let mut line_points = WavLinePoints::new();
        if i_start >= i_end {
            return CalcLineEnvelopePointsResult {
                line_points,
                envelopes: None,
                upsampled_wav_sr: None,
            };
        }

        let (upsampled_wav, upsampled_sr, factor) = match upsampled_wav_sr {
            Some((upsampled_wav, upsampled_sr)) => {
                let factor = (upsampled_sr / sr) as usize;
                (Some(upsampled_wav), upsampled_sr, factor)
            }
            None => {
                let factor = 2usize.pow((px_per_sec / sr_f32).log2().ceil().max(0.) as u32);
                let upsample_sr = sr * factor as u32;
                if upsample_sr > sr {
                    let upsample_margin = (UPSAMPLE_MARGIN_SEC * sr_f32).round() as usize;
                    let i_start_upsample = i_start.saturating_sub(upsample_margin);
                    let i_end_upsample = (i_end + upsample_margin).min(wav.len());
                    let mut upsampled_wav =
                        resample(&wav[i_start_upsample..i_end_upsample], sr, upsample_sr);
                    upsampled_wav.drain(..((i_start - i_start_upsample) * factor));
                    upsampled_wav.truncate((i_end - i_start) * factor);
                    (Some(upsampled_wav), upsample_sr, factor)
                } else {
                    (None, sr, 1)
                }
            }
        };

        let factor_f32 = factor as f32;
        let wav_slice = upsampled_wav
            .as_ref()
            .map_or_else(|| &wav[i_start..i_end], Vec::as_slice);
        for (i, v) in ((i_start * factor)..(i_end * factor)).zip(wav_slice) {
            let x = idx_to_x(i as f32 / factor_f32);
            let y = wav_to_y(*v);
            line_points.push(x, y);
        }

        let upsampled_wav_sr = upsampled_wav.map(|wav| (wav, upsampled_sr));
        return CalcLineEnvelopePointsResult {
            line_points,
            envelopes: None,
            upsampled_wav_sr,
        };
    }

    let mut line_points = WavLinePoints::new();
    let mut current_envlp = WavEnvelope::new();
    let mut envelopes = Vec::new();

    let mut i = i_start;
    let mut i_prev = i;

    while i < i_end {
        let x = idx_to_x(i as f32);
        let y = wav_to_y(wav[i]);

        // downsampling
        let x_floor = floor_x(x);
        let x_mid = x_floor + options.scale / 2.0;
        let mut i2 = i_prev;
        let mut i_next = i_end;

        while i2 < i_end {
            let x2 = idx_to_x(i2 as f32);
            let x2_floor = floor_x(x2);
            if x2_floor > x_floor && i_next == i_end {
                i_next = i2;
            }
            if x2_floor > x_floor + options.scale {
                break;
            }
            i2 += 1;
        }

        if i2 == i_prev {
            i2 = (i_prev + 1).min(i_end);
        }

        let (min_v, max_v) = find_min_max(&wav[i_prev..i2]);
        let top = wav_to_y(max_v);
        let bottom = wav_to_y(min_v);

        if bottom - top > stroke_width / 2.0 {
            // need to draw envelope
            if current_envlp.is_empty() {
                // new envelope starts
                let prev_y = if i > 0 { wav_to_y(wav[i - 1]) } else { y };
                current_envlp.push(x_floor, prev_y, prev_y);

                line_points.push(x_mid, y);
            }

            // continue the envelope
            current_envlp.push(x_mid, top, bottom);
            line_points.push(x_mid, (top + bottom) / 2.0);
        } else {
            // no need to draw envelope
            if !current_envlp.is_empty() {
                // finish the recent envelope
                current_envlp.push(x_floor, y, y);

                envelopes.push(current_envlp);
                current_envlp = WavEnvelope::new();

                let prev_y = if i > 0 { wav_to_y(wav[i - 1]) } else { y };
                line_points.push(x_mid - 1.0, prev_y);
            }

            // continue the line
            line_points.push(x_mid, (top + bottom) / 2.0);
        }
        i_prev = i;
        i = i_next;
    }

    // Handle remaining envelope
    if !current_envlp.is_empty() {
        let last_x = idx_to_x((i_end - 1) as f32);
        let last_x_floor = floor_x(last_x);
        let last_x_mid = last_x_floor + options.scale / 2.0;
        let last_x_ceil = last_x_floor + options.scale;
        let last_y = wav_to_y(if i_end > 0 { wav[i_end - 1] } else { 0.0 });

        current_envlp.push(last_x_ceil, last_y, last_y);
        envelopes.push(current_envlp);
        line_points.push(last_x_mid, last_y);
    }
    CalcLineEnvelopePointsResult {
        line_points,
        envelopes: (!envelopes.is_empty()).then_some(envelopes),
        upsampled_wav_sr: None,
    }
}

pub(crate) struct TransformParams {
    pub(crate) x_scale: f32,
    pub(crate) x_offset: f32,
    pub(crate) y2v_scale: f32,
    pub(crate) y2v_offset: f32,
    pub(crate) v_clip_values: (f32, f32),
    pub(crate) v2y_scale: f32,
    pub(crate) v2y_offset: f32,
}
