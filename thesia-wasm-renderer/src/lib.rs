use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use js_sys::Float32Array;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Path2d};

const WAV_BORDER_COLOR: &str = "rgb(0, 0, 0)";
const WAV_BORDER_WIDTH: f32 = 1.5;
const WAV_LINE_WIDTH_FACTOR: f32 = 1.75;
const WAV_MARGIN_PX: f32 = 10.0;

// Import the `console.log` function from the browser's console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Define a macro to provide `println!(..)`-style syntax for `console.log` logging.
#[allow(unused_macros)]
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        log(&format!( $( $t )* ))
    }
}

#[wasm_bindgen]
pub struct WavDrawingOptions {
    start_sec: f32,
    px_per_sec: f32,
    amp_range: (f32, f32), // [min, max]
    color: String,
    scale: f32,
    device_pixel_ratio: f32,
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
        scale: f32,
        device_pixel_ratio: f32,
    ) -> WavDrawingOptions {
        WavDrawingOptions {
            start_sec,
            px_per_sec,
            amp_range: (amp_range_min as f32, amp_range_max as f32),
            color,
            scale,
            device_pixel_ratio,
            offset_y: 0.0,
            clip_values: None,
            need_border_for_envelope: true,
            need_border_for_line: true,
            do_clear: true,
        }
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
}

struct Wav {
    wav: Vec<f32>,
    sr: u32,
}

static WAVS: LazyLock<RwLock<HashMap<String, Wav>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

#[wasm_bindgen]
pub fn set_wav(id_ch_str: &str, wav: &Float32Array, sr: u32) {
    WAVS.write().unwrap().insert(
        id_ch_str.into(),
        Wav {
            wav: wav.to_vec(),
            sr,
        },
    );
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

#[wasm_bindgen]
pub fn draw_wav(
    ctx: &CanvasRenderingContext2d,
    id_ch_str: &str,
    options: &WavDrawingOptions,
) -> Result<(), JsValue> {
    let width = ctx.canvas().unwrap().width() as f32 * options.scale;
    let height = ctx.canvas().unwrap().height() as f32 * options.scale;
    let stroke_width = WAV_LINE_WIDTH_FACTOR * options.scale * options.device_pixel_ratio;
    let (line_path, envelope_paths) = {
        let wavs = WAVS.read().unwrap();
        let Wav { wav, sr } = wavs.get(id_ch_str).unwrap();
        let px_per_sec = options.px_per_sec * options.scale * options.device_pixel_ratio;
        let sr_f32 = *sr as f32;

        let offset_x = -options.start_sec * px_per_sec;
        let idx_to_x = |idx| (idx as f32 * px_per_sec) / sr_f32 + offset_x;
        let floor_x = |x: f32| ((x - offset_x) / options.scale).floor() * options.scale + offset_x;

        let amp_range_scale = (options.amp_range.1 - options.amp_range.0).max(1e-8);
        let wav_to_y = match options.clip_values {
            Some((clip_min, clip_max)) => Box::new(move |v: f32| {
                ((options.amp_range.1 - (v.max(clip_min).min(clip_max))) / amp_range_scale) * height
                    + options.offset_y
            }) as Box<dyn Fn(f32) -> f32>,
            None => Box::new(|v: f32| {
                ((options.amp_range.1 - v) / amp_range_scale) * height + options.offset_y
            }) as Box<dyn Fn(f32) -> f32>,
        };

        let margin_samples = (WAV_MARGIN_PX / px_per_sec) * sr_f32;
        let i_start = (options.start_sec * sr_f32 - margin_samples)
            .floor()
            .max(0.0) as usize;
        let i_end = (options.start_sec * sr_f32 + width / px_per_sec * sr_f32 + margin_samples)
            .ceil() as usize;

        let mut line_path = None;
        let mut envelope_x = Vec::new();
        let mut top_envelope_y = Vec::new();
        let mut bottom_envelope_y = Vec::new();
        let mut envelope_paths = Vec::new();

        let wav_len = wav.len();
        let mut i = i_start;
        let mut i_prev = i;

        while i < i_end.min(wav_len) {
            let x = idx_to_x(i);
            let y = wav_to_y(wav[i]);

            if px_per_sec < sr_f32 {
                // downsampling
                let x_floor = floor_x(x);
                let x_mid = x_floor + options.scale / 2.0;
                let mut i2 = i_prev;
                let mut i_next = i_end;

                while i2 < i_end.min(wav_len) {
                    let x2 = idx_to_x(i2);
                    let x2_floor = floor_x(x2);
                    if x2_floor > x_floor + options.scale {
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

                        match line_path {
                            None => {
                                let new_path = Path2d::new()?;
                                new_path.move_to(x_mid as f64, y as f64);
                                line_path = Some(new_path);
                            }
                            Some(ref path) => {
                                path.line_to(x_mid as f64, y as f64);
                            }
                        }
                    }

                    // continue the envelope
                    envelope_x.push(x_mid);
                    top_envelope_y.push(top);
                    bottom_envelope_y.push(bottom);
                    if let Some(ref path) = line_path {
                        path.line_to(x_mid as f64, ((top + bottom) / 2.0) as f64);
                    }
                } else {
                    // no need to draw envelope
                    if !envelope_x.is_empty() {
                        // finish the recent envelope
                        envelope_x.push(x_floor);
                        top_envelope_y.push(y);
                        bottom_envelope_y.push(y);

                        envelope_paths.push(envelope_to_path(
                            &envelope_x,
                            &mut top_envelope_y,
                            &mut bottom_envelope_y,
                            stroke_width,
                        )?);
                        envelope_x.clear();
                        top_envelope_y.clear();
                        bottom_envelope_y.clear();

                        if let Some(ref path) = line_path {
                            let prev_y = if i > 0 { wav_to_y(wav[i - 1]) } else { y };
                            path.line_to((x_mid - 1.0) as f64, prev_y as f64);
                        }
                    }

                    // continue the line
                    match line_path {
                        None => {
                            let new_path = Path2d::new()?;
                            new_path.move_to(x_mid as f64, ((top + bottom) / 2.0) as f64);
                            line_path = Some(new_path);
                        }
                        Some(ref path) => {
                            path.line_to(x_mid as f64, ((top + bottom) / 2.0) as f64);
                        }
                    }
                }
                i_prev = i;
                i = i_next;
            } else {
                // no downsampling
                match line_path {
                    None => {
                        let new_path = Path2d::new()?;
                        new_path.move_to(x as f64, y as f64);
                        line_path = Some(new_path);
                    }
                    Some(ref path) => {
                        path.line_to(x as f64, y as f64);
                    }
                }
                i += 1;
            }
        }

        // Handle remaining envelope
        if !envelope_x.is_empty() {
            envelope_paths.push(envelope_to_path(
                &envelope_x,
                &mut top_envelope_y,
                &mut bottom_envelope_y,
                stroke_width,
            )?);
            if let Some(ref path) = line_path {
                let last_y = if i_end > 0 && (i_end - 1) < wav_len {
                    wav_to_y(wav[i_end - 1])
                } else {
                    0.0
                };
                path.line_to(floor_x(idx_to_x(i_end - 1)) as f64, last_y as f64);
            }
        }
        (line_path, envelope_paths)
    };

    // Clear canvas if needed
    if options.do_clear {
        ctx.clear_rect(0.0, 0.0, width as f64, height as f64);
    }

    // Draw borders for line
    if options.need_border_for_line {
        if let Some(ref path) = line_path {
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_stroke_style_str(&WAV_BORDER_COLOR);
            ctx.set_line_width(
                (stroke_width + 2.0 * WAV_BORDER_WIDTH * options.device_pixel_ratio) as f64,
            );
            ctx.stroke_with_path(path);
        }
    }

    // Draw borders for envelopes
    if options.need_border_for_envelope {
        for path in &envelope_paths {
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_stroke_style_str(&WAV_BORDER_COLOR);
            ctx.set_line_width((2.0 * WAV_BORDER_WIDTH * options.device_pixel_ratio) as f64);
            ctx.stroke_with_path(path);
        }
    }

    // Draw main line
    if let Some(ref path) = line_path {
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        ctx.set_stroke_style_str(&options.color);
        ctx.set_line_width(stroke_width as f64);
        ctx.stroke_with_path(path);
    }

    // Fill envelopes
    for path in &envelope_paths {
        ctx.set_fill_style_str(&options.color);
        ctx.fill_with_path_2d(path);
    }

    Ok(())
}

#[allow(unused)]
#[inline]
fn min_max_f32_scalar(values: &[f32]) -> (f32, f32) {
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    for &v in values {
        if v < min_v {
            min_v = v;
        }
        if v > max_v {
            max_v = v;
        }
    }
    (min_v, max_v)
}

#[allow(unused)]
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn min_max_f32_simd(values: &[f32]) -> (f32, f32) {
    use core::arch::wasm32::{f32x4_max, f32x4_min, f32x4_splat, v128_load, v128_store};

    let len = values.len();
    if len == 0 {
        return (0.0, 0.0);
    }

    let mut i = 0usize;
    let ptr = values.as_ptr();

    // Initialize vector mins/maxs
    let mut v_min = f32x4_splat(f32::INFINITY);
    let mut v_max = f32x4_splat(f32::NEG_INFINITY);

    while i + 4 <= len {
        let v = v128_load(ptr.add(i) as *const _);
        v_min = f32x4_min(v_min, v);
        v_max = f32x4_max(v_max, v);
        i += 4;
    }

    // Reduce lanes to scalars
    let mut tmp_min = [0.0f32; 4];
    let mut tmp_max = [0.0f32; 4];
    v128_store(tmp_min.as_mut_ptr() as *mut _, v_min);
    v128_store(tmp_max.as_mut_ptr() as *mut _, v_max);

    let mut min_v = tmp_min[0].min(tmp_min[1]).min(tmp_min[2]).min(tmp_min[3]);
    let mut max_v = tmp_max[0].max(tmp_max[1]).max(tmp_max[2]).max(tmp_max[3]);

    // Remainder
    while i < len {
        let v = *ptr.add(i);
        if v < min_v {
            min_v = v;
        }
        if v > max_v {
            max_v = v;
        }
        i += 1;
    }

    (min_v, max_v)
}

#[inline]
fn min_max_f32(values: &[f32]) -> (f32, f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        return min_max_f32_simd(values);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        return min_max_f32_scalar(values);
    }
}

#[inline]
fn add_scalar_to_slice_scalar(values: &mut [f32], scalar: f32) {
    for v in values.iter_mut() {
        *v += scalar;
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline]
unsafe fn add_scalar_to_slice_simd(values: &mut [f32], scalar: f32) {
    use core::arch::wasm32::{f32x4_add, f32x4_splat, v128_load, v128_store};

    let len = values.len();
    let mut i = 0;
    let ptr = values.as_mut_ptr();
    let splat_scalar = f32x4_splat(scalar);

    while i + 4 <= len {
        let v = v128_load(ptr.add(i) as *const _);
        let result = f32x4_add(v, splat_scalar);
        v128_store(ptr.add(i) as *mut _, result);
        i += 4;
    }

    // Remainder
    while i < len {
        *ptr.add(i) += scalar;
        i += 1;
    }
}

#[inline]
fn add_scalar_to_slice(values: &mut [f32], scalar: f32) {
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    unsafe {
        add_scalar_to_slice_simd(values, scalar);
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        add_scalar_to_slice_scalar(values, scalar);
    }
}
