use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, RwLock};

use atomic_float::AtomicF32;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Path2d};

use crate::mem::WasmFloat32Array;
use crate::simd::{add_scalar_inplace, clamp_inplace, find_min_max, fused_mul_add};

const WAV_BORDER_COLOR: &str = "rgb(0, 0, 0)";
const WAV_IMG_SCALE: f32 = 2.0;
const WAV_BORDER_WIDTH: f32 = 1.5;
const WAV_LINE_WIDTH: f32 = 1.75 * WAV_IMG_SCALE;
const WAV_MARGIN_PX: f32 = 10.0;

const CACHE_CANVAS_PX_PER_SEC: f32 = 2. / (1. / 20.); // 2px per period of 20Hz sine wave
const CACHE_HEIGHT: f32 = 10000.0;

static WAV_CACHES: LazyLock<RwLock<HashMap<String, WavCache>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static DEVICE_PIXEL_RATIO: LazyLock<AtomicF32> = LazyLock::new(|| AtomicF32::new(1.0));

#[wasm_bindgen]
pub struct WavDrawingOptions {
    start_sec: f32,
    px_per_sec: f32,       // css pixels per second
    amp_range: (f32, f32), // [min, max]
    color: String,
    offset_y: f32,
    clip_values: Option<(f32, f32)>, // [min, max] or None
    need_border_for_envelope: bool,
    need_border_for_line: bool,
    do_clear: bool,
}

#[wasm_bindgen]
impl WavDrawingOptions {
    #[wasm_bindgen(constructor)]
    pub fn new(
        start_sec: f32,
        px_per_sec: f32,
        amp_range_min: f32,
        amp_range_max: f32,
        color: String,
    ) -> Self {
        Self {
            start_sec,
            px_per_sec,
            amp_range: (amp_range_min, amp_range_max),
            color,
            offset_y: 0.0,
            clip_values: None,
            need_border_for_envelope: true,
            need_border_for_line: true,
            do_clear: true,
        }
    }

    fn new_for_cache(amp_range: (f32, f32)) -> Self {
        Self::new(
            0.0,
            CACHE_CANVAS_PX_PER_SEC / WAV_IMG_SCALE / DEVICE_PIXEL_RATIO.load(Ordering::Acquire),
            amp_range.0,
            amp_range.1,
            "".into(),
        )
    }

    #[wasm_bindgen(setter)]
    pub fn set_offset_y(&mut self, offset_y: f32) {
        self.offset_y = offset_y;
    }

    #[wasm_bindgen(setter)]
    pub fn set_clip_values(&mut self, clip_values: Option<Box<[f32]>>) {
        self.clip_values = clip_values.map(|values| (values[0], values[1]));
    }

    #[wasm_bindgen(setter)]
    pub fn set_need_border_for_envelope(&mut self, need_border: bool) {
        self.need_border_for_envelope = need_border;
    }

    #[wasm_bindgen(setter)]
    pub fn set_need_border_for_line(&mut self, need_border: bool) {
        self.need_border_for_line = need_border;
    }

    #[wasm_bindgen(setter)]
    pub fn set_do_clear(&mut self, do_clear: bool) {
        self.do_clear = do_clear;
    }

    pub fn stroke_width(&self) -> f32 {
        WAV_LINE_WIDTH * DEVICE_PIXEL_RATIO.load(Ordering::Acquire)
    }

    pub fn canvas_px_per_sec(&self) -> f32 {
        self.px_per_sec * WAV_IMG_SCALE * DEVICE_PIXEL_RATIO.load(Ordering::Acquire)
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

struct WavLinePoints {
    xs: Vec<f32>,
    ys: Vec<f32>,
}

impl WavLinePoints {
    fn new() -> Self {
        Self {
            xs: Vec::new(),
            ys: Vec::new(),
        }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            xs: Vec::with_capacity(capacity),
            ys: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, x: f32, y: f32) {
        self.xs.push(x);
        self.ys.push(y);
    }

    fn is_empty(&self) -> bool {
        self.xs.is_empty()
    }

    fn len(&self) -> usize {
        self.xs.len()
    }

    fn try_into_path(self) -> Result<Path2d, JsValue> {
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

    fn slice_transform(
        &self,
        start_x: f32,
        end_x: f32,
        params: &TransformParams,
        capacity: Option<usize>,
    ) -> Self {
        let mut out = capacity.map_or_else(Self::new, Self::with_capacity);
        let mut tmp = Vec::with_capacity(self.len());
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
        capacity: Option<usize>,
    ) -> Self {
        let mut out = capacity.map_or_else(Self::new, Self::with_capacity);
        let mut i_start = self.len();
        let mut i_end = self.len();
        let mut tmp = Vec::with_capacity(self.len());
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

struct WavCache {
    wav: Vec<f32>,
    sr: u32,
    amp_range: (f32, f32),
    line_points_cache: WavLinePoints,
    envelopes_cache: Vec<WavEnvelope>,
}

impl WavCache {
    fn new(wav: Vec<f32>, sr: u32) -> Self {
        let amp_range = {
            let (min, max) = find_min_max(&wav);
            (min.min(-1.), max.max(1.))
        };
        let mut wav_cache = Self {
            wav,
            sr,
            amp_range,
            line_points_cache: WavLinePoints::new(),
            envelopes_cache: Vec::new(),
        };
        wav_cache.update_cache();
        wav_cache
    }

    fn update_cache(&mut self) {
        let options = WavDrawingOptions::new_for_cache(self.amp_range);
        let px_per_samples = (options.canvas_px_per_sec() / self.sr as f32).min(0.1);
        let width = self.wav.len() as f32 * px_per_samples;

        let (line_points, envelopes) =
            calc_line_envelope_points(&self.wav, self.sr, width, CACHE_HEIGHT, &options);
        self.line_points_cache = line_points;
        self.envelopes_cache = envelopes.unwrap();
    }
}

#[wasm_bindgen(js_name = getWavImgScale)]
pub fn get_wav_img_scale() -> f32 {
    WAV_IMG_SCALE
}

#[wasm_bindgen(js_name = setDevicePixelRatio)]
pub fn set_device_pixel_ratio(device_pixel_ratio: f32) {
    DEVICE_PIXEL_RATIO.store(device_pixel_ratio, Ordering::Release);
    for wav_cache in WAV_CACHES.write().unwrap().values_mut() {
        wav_cache.update_cache();
    }
}

#[wasm_bindgen(js_name = setWav)]
pub fn set_wav(id_ch_str: &str, wav: WasmFloat32Array, sr: u32) {
    let wav_cache = WavCache::new(wav.into(), sr);
    WAV_CACHES
        .write()
        .unwrap()
        .insert(id_ch_str.into(), wav_cache);
}

#[wasm_bindgen(js_name = drawWav)]
pub fn draw_wav(
    ctx: &CanvasRenderingContext2d,
    id_ch_str: &str,
    options: &WavDrawingOptions,
) -> Result<(), JsValue> {
    let width = ctx.canvas().unwrap().width() as f32 * WAV_IMG_SCALE;
    let height = ctx.canvas().unwrap().height() as f32 * WAV_IMG_SCALE;
    let stroke_width = options.stroke_width();

    let (line_points, envelopes) = {
        let wav_caches = WAV_CACHES.read().unwrap();
        let WavCache {
            wav,
            sr,
            amp_range,
            line_points_cache,
            envelopes_cache,
        } = wav_caches.get(id_ch_str).unwrap();
        if options.canvas_px_per_sec() >= CACHE_CANVAS_PX_PER_SEC {
            calc_line_envelope_points(wav, *sr, width, height, options)
        } else {
            transform_line_envelopes(
                line_points_cache,
                envelopes_cache,
                *amp_range,
                width,
                height,
                options,
            )
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

    // Clear canvas if needed
    if options.do_clear {
        ctx.clear_rect(0.0, 0.0, width as f64, height as f64);
    }

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
    ctx.set_stroke_style_str(&options.color);
    ctx.set_line_width(stroke_width as f64);
    ctx.stroke_with_path(&line_path);

    // Fill envelopes
    for path in &envelope_paths {
        ctx.set_fill_style_str(&options.color);
        ctx.fill_with_path_2d(path);
    }

    Ok(())
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
    let idx_to_x = |idx| (idx as f32).mul_add(x_scale, x_offset);
    let floor_x = |x: f32| ((x - x_offset) / WAV_IMG_SCALE).floor() * WAV_IMG_SCALE + x_offset;

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
    let i_end =
        (options.start_sec * sr_f32 + width / px_per_sec * sr_f32 + margin_samples).ceil() as usize;

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

    let wav_len = wav.len();
    let mut i = i_start;
    let mut i_prev = i;

    while i < i_end.min(wav_len) {
        let x = idx_to_x(i);
        let y = wav_to_y(wav[i]);

        // downsampling
        let x_floor = floor_x(x);
        let x_mid = x_floor + WAV_IMG_SCALE / 2.0;
        let mut i2 = i_prev;
        let mut i_next = i_end;

        while i2 < i_end.min(wav_len) {
            let x2 = idx_to_x(i2);
            let x2_floor = floor_x(x2);
            if x2_floor > x_floor + WAV_IMG_SCALE {
                break;
            }
            if x2_floor > x_floor && i_next == i_end {
                i_next = i2;
            }
            i2 += 1;
        }

        if i2 == i_prev {
            i2 = (i_prev + 1).min(i_end.min(wav_len));
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
        envelopes.push(current_envlp);

        let last_y = if i_end > 0 && (i_end - 1) < wav_len {
            wav_to_y(wav[i_end - 1])
        } else {
            0.0
        };
        line_points.push(floor_x(idx_to_x(i_end - 1)), last_y);
    }
    (line_points, Some(envelopes))
}

fn transform_line_envelopes(
    line_points: &WavLinePoints,
    envelopes: &[WavEnvelope],
    amp_range: (f32, f32),
    width: f32,
    height: f32,
    options: &WavDrawingOptions,
) -> (WavLinePoints, Option<Vec<WavEnvelope>>) {
    let px_per_sec = options.canvas_px_per_sec();
    let x_scale = px_per_sec / CACHE_CANVAS_PX_PER_SEC;
    let x_offset = -options.start_sec * px_per_sec;

    let y2v_scale = -(amp_range.1 - amp_range.0) / CACHE_HEIGHT;
    let y2v_offset = amp_range.1;
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

    let line_capacity =
        ((width / px_per_sec - options.start_sec) * CACHE_CANVAS_PX_PER_SEC).ceil() as usize;
    let xformed_line_points =
        line_points.slice_transform(start_x, end_x, &transform_params, Some(line_capacity));

    let mut xformed_envelopes = Vec::new();
    for envelope in envelopes {
        if envelope.xs[0] >= end_x || envelope.xs[envelope.len() - 1] < start_x {
            continue;
        }
        let xformed_envelope =
            envelope.slice_transform(start_x, end_x, &transform_params, Some(envelope.len()));
        xformed_envelopes.push(xformed_envelope);
    }

    (xformed_line_points, Some(xformed_envelopes))
}
