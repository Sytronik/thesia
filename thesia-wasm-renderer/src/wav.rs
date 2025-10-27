use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, RwLock};

use atomic_float::AtomicF32;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Path2d};

use crate::mem::WasmFloat32Array;
use crate::simd::{add_scalar_inplace, clamp_inplace, find_min_max, fused_mul_add, negate};

pub(crate) const WAV_COLOR: &str = "rgb(19, 137, 235)";
pub(crate) const WAV_CLIPPING_COLOR: &str = "rgb(196, 34, 50)";
const WAV_BORDER_COLOR: &str = "rgb(0, 0, 0)";

const WAV_IMG_SCALE: f32 = 2.0;
const WAV_BORDER_WIDTH: f32 = 1.5;
const WAV_LINE_WIDTH: f32 = 1.75;
const WAV_MARGIN_PX: f32 = 10.0;

const CACHE_CANVAS_PX_PER_SEC: f32 = 2. / (1. / 20.); // 2px per period of 20Hz sine wave
const CACHE_HEIGHT: f32 = 10000.0;

pub(crate) static WAV_CACHES: LazyLock<RwLock<HashMap<String, WavCache>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub(crate) static DEVICE_PIXEL_RATIO: LazyLock<AtomicF32> = LazyLock::new(|| AtomicF32::new(1.0));

#[wasm_bindgen(js_name = setDevicePixelRatio)]
pub fn set_device_pixel_ratio(device_pixel_ratio: f32) {
    DEVICE_PIXEL_RATIO.store(device_pixel_ratio, Ordering::Release);
    for wav_cache in WAV_CACHES.write().unwrap().values_mut() {
        wav_cache.update_cache();
    }
}

#[wasm_bindgen(js_name = setWav)]
pub fn set_wav(id_ch_str: &str, wav: WasmFloat32Array, sr: u32, is_clipped: bool) {
    let wav: Vec<_> = wav.into();
    if let Some(wav_cache) = WAV_CACHES.read().unwrap().get(id_ch_str)
        && wav_cache.wav.len() == wav.len()
        && wav_cache.sr == sr
        && wav_cache.is_clipped == is_clipped
        && wav_cache.wav.iter().zip(wav.iter()).all(|(a, b)| a == b)
    {
        return; // TODO: remove duplicated calls at the first place
    }
    let wav_cache = WavCache::new(wav, sr, is_clipped);
    WAV_CACHES
        .write()
        .unwrap()
        .insert(id_ch_str.into(), wav_cache);
}

#[wasm_bindgen(js_name = drawWav)]
pub fn draw_wav(
    canvas: &HtmlCanvasElement,
    ctx: &CanvasRenderingContext2d,
    id_ch_str: &str,
    css_width: u32,
    css_height: u32,
    start_sec: f32,
    px_per_sec: f32,
    amp_range_min: f32,
    amp_range_max: f32,
) -> Result<(), JsValue> {
    let dpr = DEVICE_PIXEL_RATIO.load(Ordering::Acquire);
    canvas.set_width((css_width as f64 * dpr as f64).round() as u32);
    canvas.set_height((css_height as f64 * dpr as f64).round() as u32);

    ctx.scale(1. / WAV_IMG_SCALE as f64, 1. / WAV_IMG_SCALE as f64)?;

    let width = canvas.width() as f32 * WAV_IMG_SCALE;
    let height = canvas.height() as f32 * WAV_IMG_SCALE;

    let is_clipped = {
        let wav_caches = WAV_CACHES.read().unwrap();
        match wav_caches.get(id_ch_str) {
            Some(wav_cache) => wav_cache.is_clipped,
            None => {
                return Ok(());
            }
        }
    };

    if is_clipped {
        let options = WavDrawingOptions {
            start_sec,
            px_per_sec,
            amp_range: (amp_range_min, amp_range_max),
            ..Default::default()
        };
        draw_wav_internal(ctx, id_ch_str, width, height, &options, WAV_CLIPPING_COLOR)?;
    } else {
        ctx.clear_rect(0.0, 0.0, width as f64, height as f64);
    }

    let options = WavDrawingOptions {
        start_sec,
        px_per_sec,
        amp_range: (amp_range_min, amp_range_max),
        clip_values: if is_clipped { Some((-1., 1.)) } else { None },
        need_border_for_envelope: !is_clipped,
        ..Default::default()
    };
    draw_wav_internal(ctx, id_ch_str, width, height, &options, WAV_COLOR)?;

    Ok(())
}

#[wasm_bindgen(js_name = clearWav)]
pub fn clear_wav(
    canvas: Option<HtmlCanvasElement>,
    ctx: Option<CanvasRenderingContext2d>,
    css_width: u32,
    css_height: u32,
) {
    let dpr = DEVICE_PIXEL_RATIO.load(Ordering::Acquire);
    let width = css_width as f32 * dpr;
    let height = css_height as f32 * dpr;
    if let Some(canvas) = canvas {
        canvas.set_width(width.round() as u32);
        canvas.set_height(height.round() as u32);
    }
    if let Some(ctx) = ctx {
        ctx.clear_rect(
            0.,
            0.,
            (width * WAV_IMG_SCALE) as f64,
            (height * WAV_IMG_SCALE) as f64,
        );
    }
}

pub(crate) fn draw_wav_internal(
    ctx: &CanvasRenderingContext2d,
    id_ch_str: &str,
    width: f32,
    height: f32,
    options: &WavDrawingOptions,
    color: &str,
) -> Result<(), JsValue> {
    let stroke_width = options.stroke_width();

    let (line_points, envelopes) = {
        let wav_caches = WAV_CACHES.read().unwrap();
        let wav_cache = wav_caches.get(id_ch_str).unwrap();
        if options.canvas_px_per_sec() >= CACHE_CANVAS_PX_PER_SEC {
            let WavCache { wav, sr, .. } = wav_cache;
            calc_line_envelope_points(wav, *sr, width, height, options)
        } else {
            wav_cache.transform_line_envelopes(width, height, options)
        }
    };

    let line_path = line_points.try_into_path()?;
    let envelope_paths = match envelopes {
        Some(envelopes) => {
            let mut envelope_paths = Vec::with_capacity(envelopes.len());
            for envelope in envelopes {
                let path = envelope.try_into_path(stroke_width)?;
                envelope_paths.push(path);
            }
            envelope_paths
        }
        None => Vec::new(),
    };

    let dpr = DEVICE_PIXEL_RATIO.load(Ordering::Acquire);

    // Draw borders for line
    if options.need_border_for_line {
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        ctx.set_stroke_style_str(WAV_BORDER_COLOR);
        ctx.set_line_width((stroke_width + 2.0 * WAV_BORDER_WIDTH * dpr) as f64);
        ctx.stroke_with_path(&line_path);
    }

    // Draw borders for envelopes
    if options.need_border_for_envelope {
        for path in &envelope_paths {
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_stroke_style_str(WAV_BORDER_COLOR);
            ctx.set_line_width((2.0 * WAV_BORDER_WIDTH * dpr) as f64);
            ctx.stroke_with_path(path);
        }
    }

    // Draw main line
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(stroke_width as f64);
    ctx.stroke_with_path(&line_path);

    // Fill envelopes
    for path in &envelope_paths {
        ctx.set_fill_style_str(color);
        ctx.fill_with_path_2d(path);
    }

    Ok(())
}

pub(crate) struct WavDrawingOptions {
    pub(crate) start_sec: f32,
    pub(crate) px_per_sec: f32,       // css pixels per second
    pub(crate) amp_range: (f32, f32), // [min, max]
    pub(crate) offset_y: f32,
    pub(crate) clip_values: Option<(f32, f32)>, // [min, max] or None
    pub(crate) scale: f32,
    pub(crate) line_width: f32,
    pub(crate) need_border_for_envelope: bool,
    pub(crate) need_border_for_line: bool,
}

impl Default for WavDrawingOptions {
    fn default() -> Self {
        Self {
            start_sec: 0.0,
            px_per_sec: 0.0,
            amp_range: (0.0, 0.0),
            offset_y: 0.0,
            clip_values: None,
            scale: WAV_IMG_SCALE,
            line_width: WAV_LINE_WIDTH,
            need_border_for_envelope: true,
            need_border_for_line: true,
        }
    }
}

impl WavDrawingOptions {
    fn new_for_cache(amp_range: (f32, f32)) -> Self {
        let px_per_sec =
            CACHE_CANVAS_PX_PER_SEC / WAV_IMG_SCALE / DEVICE_PIXEL_RATIO.load(Ordering::Acquire);
        Self {
            px_per_sec,
            amp_range,
            ..Default::default()
        }
    }

    fn stroke_width(&self) -> f32 {
        self.line_width * self.scale * DEVICE_PIXEL_RATIO.load(Ordering::Acquire)
    }

    fn canvas_px_per_sec(&self) -> f32 {
        self.px_per_sec * self.scale * DEVICE_PIXEL_RATIO.load(Ordering::Acquire)
    }
}

pub(crate) struct WavCache {
    wav: Vec<f32>,
    sr: u32,
    is_clipped: bool,
    cache_amp_range: (f32, f32),
    line_points_cache: WavLinePoints,
    envelopes_cache: Vec<WavEnvelope>,
}

impl WavCache {
    fn new(wav: Vec<f32>, sr: u32, is_clipped: bool) -> Self {
        let cache_amp_range = {
            let (min, max) = find_min_max(&wav);
            (min.min(-1.), max.max(1.))
        };
        let mut wav_cache = Self {
            wav,
            sr,
            is_clipped,
            cache_amp_range,
            line_points_cache: WavLinePoints::new(),
            envelopes_cache: Vec::new(),
        };
        wav_cache.update_cache();
        wav_cache
    }

    fn update_cache(&mut self) {
        let options = WavDrawingOptions::new_for_cache(self.cache_amp_range);
        let px_per_samples = (options.canvas_px_per_sec() / self.sr as f32).min(0.1);
        let width = self.wav.len() as f32 * px_per_samples;

        let (line_points, envelopes) =
            calc_line_envelope_points(&self.wav, self.sr, width, CACHE_HEIGHT, &options);
        self.line_points_cache = line_points;
        self.envelopes_cache = envelopes.unwrap();
    }

    fn transform_line_envelopes(
        &self,
        width: f32,
        height: f32,
        options: &WavDrawingOptions,
    ) -> (WavLinePoints, Option<Vec<WavEnvelope>>) {
        let px_per_sec = options.canvas_px_per_sec();
        let x_scale = px_per_sec / CACHE_CANVAS_PX_PER_SEC;
        let x_offset = -options.start_sec * px_per_sec;

        let y2v_scale = -(self.cache_amp_range.1 - self.cache_amp_range.0) / CACHE_HEIGHT;
        let y2v_offset = self.cache_amp_range.1;
        let v_clip_values = options
            .clip_values
            .unwrap_or((-f32::INFINITY, f32::INFINITY));
        let v2y_scale = -height / (options.amp_range.1 - options.amp_range.0).max(1e-8);
        let v2y_offset = options.offset_y - options.amp_range.1 * v2y_scale;

        let transform_params = &TransformParams {
            x_scale,
            x_offset,
            y2v_scale,
            y2v_offset,
            v_clip_values,
            v2y_scale,
            v2y_offset,
        };

        let start_x = (-WAV_MARGIN_PX - x_offset) / x_scale;
        let end_x = (width + WAV_MARGIN_PX - x_offset) / x_scale;

        let line_len_hint =
            ((width / px_per_sec - options.start_sec) * CACHE_CANVAS_PX_PER_SEC).ceil() as usize;
        let xformed_line_points = self.line_points_cache.slice_transform(
            start_x,
            end_x,
            &transform_params,
            Some(line_len_hint),
        );

        let mut xformed_envelopes = Vec::new();
        for envelope in self.envelopes_cache.iter() {
            if envelope.xs[0] >= end_x || envelope.xs[envelope.len() - 1] < start_x {
                continue;
            }
            let xformed_envelope =
                envelope.slice_transform(start_x, end_x, &transform_params, Some(envelope.len()));
            xformed_envelopes.push(xformed_envelope);
        }

        (xformed_line_points, Some(xformed_envelopes))
    }

    pub fn is_clipped(&self) -> bool {
        self.is_clipped
    }

    pub fn cache_amp_range(&self) -> (f32, f32) {
        self.cache_amp_range
    }
}

#[derive(Debug)]
pub(crate) struct WavLinePoints {
    xs: Vec<f32>,
    ys: Vec<f32>,
}

impl WavLinePoints {
    pub(crate) fn new() -> Self {
        Self {
            xs: Vec::new(),
            ys: Vec::new(),
        }
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            xs: Vec::with_capacity(capacity),
            ys: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn push(&mut self, x: f32, y: f32) {
        self.xs.push(x);
        self.ys.push(y);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.xs.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.xs.len()
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

    pub(crate) fn shift_y_inplace(&mut self, offset_y: f32) {
        add_scalar_inplace(&mut self.ys, offset_y);
    }

    pub(crate) fn upside_down(&self) -> Self {
        let mut out = Self::with_capacity(self.len());
        out.xs = self.xs.clone();
        negate(&self.ys, &mut out.ys);
        out
    }

    fn slice_transform(
        &self,
        start_x: f32,
        end_x: f32,
        params: &TransformParams,
        len_hint: Option<usize>,
    ) -> Self {
        let mut out = len_hint.map_or_else(Self::new, Self::with_capacity);
        let mut tmp = len_hint.map_or_else(Vec::new, Vec::with_capacity);
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
        fused_mul_add(
            &self.xs[i_start..i_end],
            params.x_scale,
            params.x_offset,
            &mut out.xs,
        );
        fused_mul_add(
            &self.ys[i_start..i_end],
            params.y2v_scale,
            params.y2v_offset,
            &mut tmp,
        );
        clamp_inplace(&mut tmp, params.v_clip_values.0, params.v_clip_values.1);
        fused_mul_add(&tmp, params.v2y_scale, params.v2y_offset, &mut out.ys);
        tmp.clear();
        out
    }
}

impl TryFrom<WavLinePoints> for Path2d {
    type Error = JsValue;

    fn try_from(value: WavLinePoints) -> Result<Self, Self::Error> {
        value.try_into_path()
    }
}

#[derive(Debug)]
struct WavEnvelope {
    xs: Vec<f32>,
    tops: Vec<f32>,
    bottoms: Vec<f32>,
}

impl WavEnvelope {
    fn new() -> Self {
        Self {
            xs: Vec::new(),
            tops: Vec::new(),
            bottoms: Vec::new(),
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            xs: Vec::with_capacity(capacity),
            tops: Vec::with_capacity(capacity),
            bottoms: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, x: f32, top: f32, bottom: f32) {
        self.xs.push(x);
        self.tops.push(top);
        self.bottoms.push(bottom);
    }

    fn is_empty(&self) -> bool {
        self.xs.is_empty()
    }

    fn len(&self) -> usize {
        self.xs.len()
    }

    fn try_into_path(mut self, stroke_width: f32) -> Result<Path2d, JsValue> {
        let path = Path2d::new()?;

        if self.is_empty() {
            return Ok(path);
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
        Ok(path)
    }

    fn slice_transform(
        &self,
        start_x: f32,
        end_x: f32,
        params: &TransformParams,
        len_hint: Option<usize>,
    ) -> Self {
        let mut out = len_hint.map_or_else(Self::new, Self::with_capacity);
        let mut tmp = len_hint.map_or_else(Vec::new, Vec::with_capacity);
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
        fused_mul_add(
            &self.xs[i_start..i_end],
            params.x_scale,
            params.x_offset,
            &mut out.xs,
        );
        fused_mul_add(
            &self.tops[i_start..i_end],
            params.y2v_scale,
            params.y2v_offset,
            &mut tmp,
        );
        clamp_inplace(&mut tmp, params.v_clip_values.0, params.v_clip_values.1);
        fused_mul_add(&tmp, params.v2y_scale, params.v2y_offset, &mut out.tops);
        tmp.clear();
        fused_mul_add(
            &self.bottoms[i_start..i_end],
            params.y2v_scale,
            params.y2v_offset,
            &mut tmp,
        );
        clamp_inplace(&mut tmp, params.v_clip_values.0, params.v_clip_values.1);
        fused_mul_add(&tmp, params.v2y_scale, params.v2y_offset, &mut out.bottoms);
        tmp.clear();
        out
    }
}

struct TransformParams {
    x_scale: f32,
    x_offset: f32,
    y2v_scale: f32,
    y2v_offset: f32,
    v_clip_values: (f32, f32),
    v2y_scale: f32,
    v2y_offset: f32,
}

fn calc_line_envelope_points(
    wav: &[f32],
    sr: u32,
    width: f32,
    height: f32,
    options: &WavDrawingOptions,
) -> (WavLinePoints, Option<Vec<WavEnvelope>>) {
    let px_per_sec = options.canvas_px_per_sec();
    let stroke_width = options.stroke_width();
    let sr_f32 = sr as f32;

    let x_scale = px_per_sec / sr_f32;
    let x_offset = -options.start_sec * px_per_sec;
    let idx_to_x = |i| (i as f32).mul_add(x_scale, x_offset);
    let floor_x = |x: f32| ((x - x_offset) / options.scale).floor() * options.scale + x_offset;

    let (clip_min, clip_max) = options
        .clip_values
        .unwrap_or((-f32::INFINITY, f32::INFINITY));
    let y_scale = -height / (options.amp_range.1 - options.amp_range.0).max(1e-8);
    let y_offset = options.offset_y - options.amp_range.1 * y_scale;
    let wav_to_y = |v: f32| v.max(clip_min).min(clip_max).mul_add(y_scale, y_offset);

    let margin_samples = (WAV_MARGIN_PX / px_per_sec) * sr_f32;
    let i_start = (options.start_sec * sr_f32 - margin_samples)
        .floor()
        .max(0.0) as usize;
    let i_end = wav.len().min(
        (options.start_sec * sr_f32 + width / px_per_sec * sr_f32 + margin_samples).ceil() as usize,
    );

    if px_per_sec >= sr_f32 {
        let mut line_points = WavLinePoints::new();
        for (i, v) in wav.iter().enumerate().take(i_end).skip(i_start) {
            let x = idx_to_x(i);
            let y = wav_to_y(*v);
            line_points.push(x, y);
        }
        return (line_points, None);
    }

    let mut line_points = WavLinePoints::new();
    let mut current_envlp = WavEnvelope::new();
    let mut envelopes = Vec::new();

    let mut i = i_start;
    let mut i_prev = i;

    while i < i_end {
        let x = idx_to_x(i);
        let y = wav_to_y(wav[i]);

        // downsampling
        let x_floor = floor_x(x);
        let x_mid = x_floor + options.scale / 2.0;
        let mut i2 = i_prev;
        let mut i_next = i_end;

        while i2 < i_end {
            let x2 = idx_to_x(i2);
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
        let last_x = idx_to_x(i_end - 1);
        let last_x_floor = floor_x(last_x);
        let last_x_mid = last_x_floor + options.scale / 2.0;
        let last_x_ceil = last_x_floor + options.scale;
        let last_y = wav_to_y(if i_end > 0 { wav[i_end - 1] } else { 0.0 });

        current_envlp.push(last_x_ceil, last_y, last_y);
        envelopes.push(current_envlp);
        line_points.push(last_x_mid, last_y);
    }
    (line_points, Some(envelopes))
}
