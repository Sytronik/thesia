use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{LazyLock, RwLock};

use atomic_float::AtomicF32;
use itertools::multizip;
use js_sys::Float32Array;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Path2d};

use crate::simd::{add_scalar_to_slice, min_max_f32};

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
    ) -> WavDrawingOptions {
        WavDrawingOptions {
            start_sec,
            px_per_sec,
            amp_range: (amp_range_min as f32, amp_range_max as f32),
            color,
            offset_y: 0.0,
            clip_values: None,
            need_border_for_envelope: true,
            need_border_for_line: true,
            do_clear: true,
        }
    }

    fn new_for_cache(amp_range: (f32, f32)) -> WavDrawingOptions {
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
        self.clip_values = clip_values.map(|values| (values[0] as f32, values[1] as f32));
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

struct WavCache {
    wav: Vec<f32>,
    sr: u32,
    amp_range: (f32, f32),
    line_points_cache: Vec<(f32, f32)>,
    envelopes_cache: Vec<(Vec<f32>, Vec<f32>, Vec<f32>)>,
}

impl WavCache {
    fn new(wav: Vec<f32>, sr: u32) -> Self {
        let amp_range = {
            let (min, max) = min_max_f32(&wav);
            (min.min(-1.), max.max(1.))
        };
        let mut wav_cache = Self {
            wav,
            sr,
            amp_range,
            line_points_cache: Vec::new(),
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
pub fn set_wav(id_ch_str: &str, wav: &Float32Array, sr: u32) {
    let wav_cache = WavCache::new(wav.to_vec(), sr);
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
            line_points_cache,
            envelopes_cache,
            ..
        } = wav_caches.get(id_ch_str).unwrap();
        if options.canvas_px_per_sec() >= CACHE_CANVAS_PX_PER_SEC {
            calc_line_envelope_points(wav, *sr, width, height, options)
        } else {
            transform_line_envelopes(line_points_cache, envelopes_cache, width, height, options)
        }
    };

    let line_path = line_to_path(&line_points)?;
    let envelope_paths = match envelopes {
        Some(envelopes) => {
            let mut envelope_paths = Vec::with_capacity(envelopes.len());
            for (envelope_x, mut top_envelope_y, mut bottom_envelope_y) in envelopes {
                let path = envelope_to_path(
                    &envelope_x,
                    &mut top_envelope_y,
                    &mut bottom_envelope_y,
                    stroke_width,
                )?;
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
        ctx.set_stroke_style_str(&WAV_BORDER_COLOR);
        ctx.set_line_width((stroke_width + 2.0 * WAV_BORDER_WIDTH * dpr) as f64);
        ctx.stroke_with_path(&line_path);
    }

    // Draw borders for envelopes
    if options.need_border_for_envelope {
        for path in &envelope_paths {
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_stroke_style_str(&WAV_BORDER_COLOR);
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
) -> (Vec<(f32, f32)>, Option<Vec<(Vec<f32>, Vec<f32>, Vec<f32>)>>) {
    let px_per_sec = options.canvas_px_per_sec();
    let stroke_width = options.stroke_width();
    let sr_f32 = sr as f32;

    let offset_x = -options.start_sec * px_per_sec;
    let idx_to_x = |idx| (idx as f32 * px_per_sec) / sr_f32 + offset_x;
    let floor_x = |x: f32| ((x - offset_x) / WAV_IMG_SCALE).floor() * WAV_IMG_SCALE + offset_x;

    let amp_range_scale = (options.amp_range.1 - options.amp_range.0).max(1e-8);
    let (clip_min, clip_max) = options
        .clip_values
        .unwrap_or((-f32::INFINITY, f32::INFINITY));
    let wav_to_y = |v: f32| {
        ((options.amp_range.1 - (v.max(clip_min).min(clip_max))) / amp_range_scale) * height
            + options.offset_y
    };

    let margin_samples = (WAV_MARGIN_PX / px_per_sec) * sr_f32;
    let i_start = (options.start_sec * sr_f32 - margin_samples)
        .floor()
        .max(0.0) as usize;
    let i_end =
        (options.start_sec * sr_f32 + width / px_per_sec * sr_f32 + margin_samples).ceil() as usize;

    if px_per_sec >= sr_f32 {
        let mut line_points = Vec::new();
        for i in i_start..i_end {
            let x = idx_to_x(i);
            let y = wav_to_y(wav[i]);
            line_points.push((x, y));
        }
        return (line_points, None);
    }

    let mut line_points = Vec::new();
    let mut envelope_x = Vec::new();
    let mut top_envelope_y = Vec::new();
    let mut bottom_envelope_y = Vec::new();
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

        let (min_v, max_v) = min_max_f32(&wav[i_prev..i2]);
        let top = wav_to_y(max_v);
        let bottom = wav_to_y(min_v);

        if bottom - top > stroke_width / 2.0 {
            // need to draw envelope
            if envelope_x.is_empty() {
                // new envelope starts
                let prev_y = if i > 0 { wav_to_y(wav[i - 1]) } else { y };
                envelope_x.push(x_floor);
                top_envelope_y.clear(); // defensive code
                top_envelope_y.push(prev_y);
                bottom_envelope_y.clear(); // defensive code
                bottom_envelope_y.push(prev_y);

                line_points.push((x_mid, y));
            }

            // continue the envelope
            envelope_x.push(x_mid);
            top_envelope_y.push(top);
            bottom_envelope_y.push(bottom);
            line_points.push((x_mid, ((top + bottom) / 2.0)));
        } else {
            // no need to draw envelope
            if !envelope_x.is_empty() {
                // finish the recent envelope
                envelope_x.push(x_floor);
                top_envelope_y.push(y);
                bottom_envelope_y.push(y);

                envelopes.push((envelope_x, top_envelope_y, bottom_envelope_y));
                envelope_x = Vec::new();
                top_envelope_y = Vec::new();
                bottom_envelope_y = Vec::new();

                let prev_y = if i > 0 { wav_to_y(wav[i - 1]) } else { y };
                line_points.push(((x_mid - 1.0), prev_y));
            }

            // continue the line
            line_points.push((x_mid, ((top + bottom) / 2.0)));
        }
        i_prev = i;
        i = i_next;
    }

    // Handle remaining envelope
    if !envelope_x.is_empty() {
        envelopes.push((envelope_x, top_envelope_y, bottom_envelope_y));

        let last_y = if i_end > 0 && (i_end - 1) < wav_len {
            wav_to_y(wav[i_end - 1])
        } else {
            0.0
        };
        line_points.push((floor_x(idx_to_x(i_end - 1)), last_y));
    }
    (line_points, Some(envelopes))
}

fn transform_line_envelopes(
    line_points: &[(f32, f32)],
    envelopes: &[(Vec<f32>, Vec<f32>, Vec<f32>)],
    width: f32,
    height: f32,
    options: &WavDrawingOptions,
) -> (Vec<(f32, f32)>, Option<Vec<(Vec<f32>, Vec<f32>, Vec<f32>)>>) {
    let px_per_sec = options.canvas_px_per_sec();
    let ratio_x = px_per_sec / CACHE_CANVAS_PX_PER_SEC;
    let offset_x = -options.start_sec * px_per_sec;
    let ratio_y = height / CACHE_HEIGHT;
    let offset_y = options.offset_y;

    let mut xformed_line_points = Vec::with_capacity(
        ((width / px_per_sec - options.start_sec) * CACHE_CANVAS_PX_PER_SEC).ceil() as usize,
    );
    let start_x = (-WAV_MARGIN_PX - offset_x) / ratio_x;
    let end_x = (width + WAV_MARGIN_PX - offset_x) / ratio_x;
    for (x, y) in line_points {
        if *x < start_x {
            continue;
        }
        if *x >= end_x {
            break;
        }
        xformed_line_points.push((*x * ratio_x + offset_x, *y * ratio_y + offset_y));
    }
    let mut xformed_envelopes = Vec::new();
    for (envelope_x, top_envelope_y, bottom_envelope_y) in envelopes {
        if envelope_x[0] >= end_x || envelope_x[envelope_x.len() - 1] < start_x {
            continue;
        }
        let mut xformed_envelope_x = Vec::with_capacity(envelope_x.len());
        let mut xformed_top_envelope_y = Vec::with_capacity(envelope_x.len());
        let mut xformed_bottom_envelope_y = Vec::with_capacity(envelope_x.len());
        for (x, top, bottom) in multizip((envelope_x, top_envelope_y, bottom_envelope_y)) {
            if *x < start_x {
                continue;
            }
            if *x >= end_x {
                break;
            }
            xformed_envelope_x.push(*x * ratio_x + offset_x);
            xformed_top_envelope_y.push(*top * ratio_y + offset_y);
            xformed_bottom_envelope_y.push(*bottom * ratio_y + offset_y);
        }
        xformed_envelopes.push((
            xformed_envelope_x,
            xformed_top_envelope_y,
            xformed_bottom_envelope_y,
        ));
    }

    (xformed_line_points, Some(xformed_envelopes))
}

fn line_to_path(points: &[(f32, f32)]) -> Result<Path2d, JsValue> {
    let path = Path2d::new()?;
    if points.is_empty() {
        return Ok(path);
    }
    path.move_to(points[0].0 as f64, points[0].1 as f64);
    for (x, y) in points.into_iter().skip(1) {
        path.line_to(*x as f64, *y as f64);
    }
    Ok(path)
}

fn envelope_to_path(
    envelope_x: &[f32],
    top_envelope_y: &mut [f32],
    bottom_envelope_y: &mut [f32],
    stroke_width: f32,
) -> Result<Path2d, JsValue> {
    let path = Path2d::new()?;

    if envelope_x.is_empty() {
        return Ok(path);
    }

    let half_stroke_width = stroke_width / 2.0;

    add_scalar_to_slice(top_envelope_y, -half_stroke_width);
    add_scalar_to_slice(bottom_envelope_y, half_stroke_width);

    // Move to first point
    path.move_to(envelope_x[0] as f64, top_envelope_y[0] as f64);

    // Draw top envelope
    for (x, y) in envelope_x.iter().zip(top_envelope_y.iter()).skip(1) {
        path.line_to(*x as f64, *y as f64);
    }

    // Draw bottom envelope (reversed)
    for (x, y) in envelope_x.iter().zip(bottom_envelope_y.iter()).rev() {
        path.line_to(*x as f64, *y as f64);
    }

    path.close_path();
    Ok(path)
}
