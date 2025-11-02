use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, RwLock};

use atomic_float::AtomicF32;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::line_envelope::{
    CalcLineEnvelopePointsResult, TransformParams, WavEnvelope, WavLinePoints,
    calc_line_envelope_points,
};
use crate::mem::WasmFloat32Array;
use crate::simd::find_min_max;

pub(crate) const WAV_MARGIN_PX: f32 = 10.0;

pub(crate) const WAV_COLOR: &str = "rgb(19, 137, 235)";
pub(crate) const WAV_CLIPPING_COLOR: &str = "rgb(196, 34, 50)";
const WAV_BORDER_COLOR: &str = "rgb(0, 0, 0)";

const WAV_IMG_SCALE: f32 = 2.0;
const BORDER_WIDTH_CSS_PX: f32 = 1.5;
const LINE_WIDTH_CSS_PX: f32 = 1.75;

const CACHE_PX_PER_SEC: f32 = 2. / (1. / 20.); // 2px per period of 20Hz sine wave
const CACHE_HEIGHT: f32 = 10000.0;

pub(crate) static WAV_CACHES: LazyLock<RwLock<HashMap<String, WavCache>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub(crate) static DEVICE_PIXEL_RATIO: AtomicF32 = AtomicF32::new(1.0);

#[wasm_bindgen(js_name = setDevicePixelRatio)]
pub fn set_device_pixel_ratio(device_pixel_ratio: f32) {
    DEVICE_PIXEL_RATIO.store(device_pixel_ratio, Ordering::Release);
    for wav_cache in WAV_CACHES.write().unwrap().values_mut() {
        wav_cache.update_cache();
    }
}

#[wasm_bindgen(js_name = setWav)]
pub fn set_wav(id_ch_str: &str, wav: WasmFloat32Array, sr: u32, is_clipped: bool) {
    let wav_cache = WavCache::new(wav.into(), sr, is_clipped);
    WAV_CACHES
        .write()
        .unwrap()
        .insert(id_ch_str.into(), wav_cache);
}

#[wasm_bindgen(js_name = removeWav)]
pub fn remove_wav(track_id: u32) {
    WAV_CACHES
        .write()
        .unwrap()
        .retain(|id_ch_str, _| !id_ch_str.starts_with(&format!("{}_", track_id)))
}

#[wasm_bindgen(js_name = drawWav)]
pub fn draw_wav(
    canvas: &HtmlCanvasElement,
    ctx: &CanvasRenderingContext2d,
    id_ch_str: &str,
    css_width: u32,
    css_height: u32,
    start_sec: f32,
    css_px_per_sec: f32,
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

    let upsampled_wav_sr = if is_clipped {
        let options = WavDrawingOptions {
            width,
            height,
            start_sec,
            css_px_per_sec,
            amp_range: (amp_range_min, amp_range_max),
            ..Default::default()
        };
        draw_wav_internal(ctx, id_ch_str, &options, WAV_CLIPPING_COLOR, None)?
    } else {
        ctx.clear_rect(0.0, 0.0, width as f64, height as f64);
        None
    };

    let options = WavDrawingOptions {
        width,
        height,
        start_sec,
        css_px_per_sec,
        amp_range: (amp_range_min, amp_range_max),
        clip_values: if is_clipped { Some((-1., 1.)) } else { None },
        need_border_for_envelope: !is_clipped,
        ..Default::default()
    };
    draw_wav_internal(ctx, id_ch_str, &options, WAV_COLOR, upsampled_wav_sr)?;

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
    options: &WavDrawingOptions,
    color: &str,
    upsampled_wav_sr: Option<(Vec<f32>, u32)>,
) -> Result<Option<(Vec<f32>, u32)>, JsValue> {
    let line_width = options.line_width();

    let result = {
        let wav_caches = WAV_CACHES.read().unwrap();
        let wav_cache = wav_caches.get(id_ch_str).unwrap();
        if options.px_per_sec() >= CACHE_PX_PER_SEC {
            let WavCache { wav, sr, .. } = wav_cache;
            calc_line_envelope_points(wav, *sr, options, upsampled_wav_sr)
        } else {
            let (line_points, envelopes) = wav_cache.transform_line_envelopes(options);
            CalcLineEnvelopePointsResult {
                line_points,
                envelopes,
                upsampled_wav_sr: None,
            }
        }
    };

    let line_path = result.line_points.try_into_path()?;
    let envelope_paths = match result.envelopes {
        Some(envelopes) => {
            let mut envelope_paths = Vec::with_capacity(envelopes.len());
            for envelope in envelopes {
                let path = envelope.try_into_path(line_width)?;
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
        ctx.set_line_width((line_width + 2.0 * BORDER_WIDTH_CSS_PX * dpr) as f64);
        ctx.stroke_with_path(&line_path);
    }

    // Draw borders for envelopes
    if options.need_border_for_envelope {
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        ctx.set_stroke_style_str(WAV_BORDER_COLOR);
        ctx.set_line_width((2.0 * BORDER_WIDTH_CSS_PX * dpr) as f64);
        for path in &envelope_paths {
            ctx.stroke_with_path(path);
        }
    }

    // Draw main line
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(line_width as f64);
    ctx.stroke_with_path(&line_path);

    // Fill envelopes
    ctx.set_fill_style_str(color);
    for path in &envelope_paths {
        ctx.fill_with_path_2d(path);
    }

    Ok(result.upsampled_wav_sr)
}

pub(crate) struct WavDrawingOptions {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) start_sec: f32,
    pub(crate) css_px_per_sec: f32,
    pub(crate) amp_range: (f32, f32), // [min, max]
    pub(crate) offset_y: f32,
    pub(crate) clip_values: Option<(f32, f32)>, // [min, max] or None
    pub(crate) scale: f32,
    pub(crate) line_width_css_px: f32,
    pub(crate) need_border_for_envelope: bool,
    pub(crate) need_border_for_line: bool,
}

impl Default for WavDrawingOptions {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            start_sec: 0.0,
            css_px_per_sec: 0.0,
            amp_range: (0.0, 0.0),
            offset_y: 0.0,
            clip_values: None,
            scale: WAV_IMG_SCALE,
            line_width_css_px: LINE_WIDTH_CSS_PX,
            need_border_for_envelope: true,
            need_border_for_line: true,
        }
    }
}

impl WavDrawingOptions {
    fn new_for_cache(wav_len: usize, sr: u32, amp_range: (f32, f32)) -> Self {
        let px_per_samples = (CACHE_PX_PER_SEC / sr as f32).min(0.1);
        let css_px_per_sec =
            CACHE_PX_PER_SEC / WAV_IMG_SCALE / DEVICE_PIXEL_RATIO.load(Ordering::Acquire);
        Self {
            width: wav_len as f32 * px_per_samples,
            height: CACHE_HEIGHT,
            css_px_per_sec,
            amp_range,
            ..Default::default()
        }
    }

    pub(crate) fn line_width(&self) -> f32 {
        self.line_width_css_px * self.scale * DEVICE_PIXEL_RATIO.load(Ordering::Acquire)
    }

    pub(crate) fn px_per_sec(&self) -> f32 {
        self.css_px_per_sec * self.scale * DEVICE_PIXEL_RATIO.load(Ordering::Acquire)
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

    pub(crate) fn is_clipped(&self) -> bool {
        self.is_clipped
    }

    pub(crate) fn cache_amp_range(&self) -> (f32, f32) {
        self.cache_amp_range
    }

    fn update_cache(&mut self) {
        let options =
            WavDrawingOptions::new_for_cache(self.wav.len(), self.sr, self.cache_amp_range);
        let result = calc_line_envelope_points(&self.wav, self.sr, &options, None);
        self.line_points_cache = result.line_points;
        self.envelopes_cache = result.envelopes.unwrap();
    }

    fn transform_line_envelopes(
        &self,
        options: &WavDrawingOptions,
    ) -> (WavLinePoints, Option<Vec<WavEnvelope>>) {
        let width = options.width;
        let height = options.height;
        let px_per_sec = options.px_per_sec();
        let x_scale = px_per_sec / CACHE_PX_PER_SEC;
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

        let xformed_line_points =
            self.line_points_cache
                .slice_transform(start_x, end_x, &transform_params);

        let mut xformed_envelopes = Vec::new();
        for envelope in self.envelopes_cache.iter() {
            if envelope.out_of_range(start_x, end_x) {
                continue;
            }
            let xformed_envelope =
                envelope.slice_transform(start_x, end_x, &transform_params, options.scale);
            xformed_envelopes.push(xformed_envelope);
        }

        (xformed_line_points, Some(xformed_envelopes))
    }
}
