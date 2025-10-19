use js_sys::Float32Array;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Path2d};

const WAV_BORDER_COLOR: &str = "rgb(0, 0, 0)";
const WAV_BORDER_WIDTH: f64 = 1.5;
const WAV_LINE_WIDTH_FACTOR: f64 = 1.75;
const WAV_MARGIN_PX: f64 = 10.0;

// Import the `console.log` function from the browser's console
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Define a macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        log(&format!( $( $t )* ))
    }
}

#[wasm_bindgen]
pub struct WavDrawingOptions {
    start_sec: f64,
    px_per_sec: f64,
    amp_range: (f32, f32), // [min, max]
    color: String,
    scale: f64,
    device_pixel_ratio: f64,
    offset_y: f64,
    clip_values: Option<(f32, f32)>, // [min, max] or None
    need_border_for_envelope: bool,
    need_border_for_line: bool,
    do_clear: bool,
}

#[wasm_bindgen]
impl WavDrawingOptions {
    #[wasm_bindgen(constructor)]
    pub fn new(
        start_sec: f64,
        px_per_sec: f64,
        amp_range_min: f64,
        amp_range_max: f64,
        color: String,
        scale: f64,
        device_pixel_ratio: f64,
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
    pub fn set_offset_y(&mut self, offset_y: f64) {
        self.offset_y = offset_y;
    }

    #[wasm_bindgen(setter)]
    pub fn set_clip_values(&mut self, clip_values: Option<Box<[f64]>>) {
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

fn envelope_to_path(
    top_envelope: &[(f64, f64)],
    bottom_envelope: &[(f64, f64)],
    stroke_width: f64,
) -> Result<Path2d, JsValue> {
    let path = Path2d::new()?;
    let half_stroke_width = stroke_width / 2.0;

    if top_envelope.is_empty() {
        return Ok(path);
    }

    // Move to first point
    path.move_to(top_envelope[0].0, top_envelope[0].1 - half_stroke_width);

    // Draw top envelope
    for (i, (x, y)) in top_envelope.iter().enumerate() {
        if i == 0 {
            continue;
        }
        path.line_to(*x, y - half_stroke_width);
    }

    // Draw bottom envelope (reversed)
    for (x, y) in bottom_envelope.iter().rev() {
        path.line_to(*x, y + half_stroke_width);
    }

    path.close_path();
    Ok(path)
}

#[wasm_bindgen]
pub fn draw_wav(
    ctx: &CanvasRenderingContext2d,
    wav: &Float32Array,
    sr: u32,
    options: &WavDrawingOptions,
) -> Result<(), JsValue> {
    let width = ctx.canvas().unwrap().width() as f64 * options.scale;
    let height = ctx.canvas().unwrap().height() as f64 * options.scale;
    let px_per_sec = options.px_per_sec * options.scale * options.device_pixel_ratio;
    let stroke_width = WAV_LINE_WIDTH_FACTOR * options.scale * options.device_pixel_ratio;
    let sr_f64 = sr as f64;

    let offset_x = -options.start_sec * px_per_sec;
    let idx_to_x = |idx| (idx as f64 * px_per_sec) / sr_f64 + offset_x;
    let floor_x = |x: f64| ((x - offset_x) / options.scale).floor() * options.scale + offset_x;

    let amp_range_scale = (options.amp_range.1 - options.amp_range.0).max(1e-8);
    let wav_to_y = match options.clip_values {
        Some((clip_min, clip_max)) => Box::new(move |v: f32| {
            ((options.amp_range.1 - (v.max(clip_min).min(clip_max))) / amp_range_scale) as f64
                * height
                + options.offset_y
        }) as Box<dyn Fn(f32) -> f64>,
        None => Box::new(|v: f32| {
            ((options.amp_range.1 - v) / amp_range_scale) as f64 * height + options.offset_y
        }) as Box<dyn Fn(f32) -> f64>,
    };

    let margin_samples = (WAV_MARGIN_PX / px_per_sec) * sr_f64;
    let i_start = (options.start_sec * sr_f64 - margin_samples).floor() as i32;
    let i_end =
        (options.start_sec * sr_f64 + width / px_per_sec * sr_f64 + margin_samples).ceil() as u32;

    let mut line_path: Option<Path2d> = None;
    let mut top_envelope: Vec<(f64, f64)> = Vec::new();
    let mut bottom_envelope: Vec<(f64, f64)> = Vec::new();
    let mut envelope_paths: Vec<Path2d> = Vec::new();

    let wav_len = wav.length();
    let mut i = i_start.max(0) as u32;
    let mut i_prev = i;

    while i < i_end.min(wav_len) {
        let wav_val = wav.get_index(i);
        let x = idx_to_x(i);
        let y = wav_to_y(wav_val);

        if px_per_sec < sr_f64 {
            // downsampling
            let x_floor = floor_x(x);
            let x_mid = x_floor + options.scale / 2.0;
            let mut top = y;
            let mut bottom = y;
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

                let y2 = wav_to_y(wav.get_index(i2));
                if y2 < top {
                    top = y2;
                }
                if y2 > bottom {
                    bottom = y2;
                }
                i2 += 1;
            }

            if bottom - top > stroke_width / 2.0 {
                // need to draw envelope
                if top_envelope.is_empty() {
                    // new envelope starts
                    let prev_y = if i > 0 {
                        wav_to_y(wav.get_index(i - 1))
                    } else {
                        y
                    };
                    top_envelope.push((x_floor, prev_y));
                    bottom_envelope.clear(); // defensive code
                    bottom_envelope.push((x_floor, prev_y));

                    match line_path {
                        None => {
                            let new_path = Path2d::new()?;
                            new_path.move_to(x_mid, y);
                            line_path = Some(new_path);
                        }
                        Some(ref path) => {
                            path.line_to(x_mid, y);
                        }
                    }
                }

                // continue the envelope
                top_envelope.push((x_mid, top));
                bottom_envelope.push((x_mid, bottom));
                if let Some(ref path) = line_path {
                    path.line_to(x_mid, (top + bottom) / 2.0);
                }
            } else {
                // no need to draw envelope
                if !top_envelope.is_empty() {
                    // the recent envelope is finished
                    top_envelope.push((x_floor, y));
                    bottom_envelope.push((x_floor, y));

                    envelope_paths.push(envelope_to_path(
                        &top_envelope,
                        &bottom_envelope,
                        stroke_width,
                    )?);
                    top_envelope.clear();
                    bottom_envelope.clear();

                    if let Some(ref path) = line_path {
                        let prev_y = if i > 0 {
                            wav_to_y(wav.get_index(i - 1))
                        } else {
                            y
                        };
                        path.line_to(x_mid - 1.0, prev_y);
                    }
                }

                // continue the line
                match line_path {
                    None => {
                        let new_path = Path2d::new()?;
                        new_path.move_to(x_mid, (top + bottom) / 2.0);
                        line_path = Some(new_path);
                    }
                    Some(ref path) => {
                        path.line_to(x_mid, (top + bottom) / 2.0);
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
                    new_path.move_to(x, y);
                    line_path = Some(new_path);
                }
                Some(ref path) => {
                    path.line_to(x, y);
                }
            }
            i += 1;
        }
    }

    // Handle remaining envelope
    if !top_envelope.is_empty() {
        envelope_paths.push(envelope_to_path(
            &top_envelope,
            &bottom_envelope,
            stroke_width,
        )?);
        if let Some(ref path) = line_path {
            let last_y = if i_end > 0 && (i_end - 1) < wav_len {
                wav_to_y(wav.get_index(i_end - 1))
            } else {
                0.0
            };
            path.line_to(floor_x(idx_to_x(i_end - 1)), last_y);
        }
    }

    // Clear canvas if needed
    if options.do_clear {
        ctx.clear_rect(0.0, 0.0, width, height);
    }

    // Draw borders for line
    if options.need_border_for_line {
        if let Some(ref path) = line_path {
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_stroke_style_str(&WAV_BORDER_COLOR);
            ctx.set_line_width(stroke_width + 2.0 * WAV_BORDER_WIDTH * options.device_pixel_ratio);
            ctx.stroke_with_path(path);
        }
    }

    // Draw borders for envelopes
    if options.need_border_for_envelope {
        for path in &envelope_paths {
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_stroke_style_str(&WAV_BORDER_COLOR);
            ctx.set_line_width(2.0 * WAV_BORDER_WIDTH * options.device_pixel_ratio);
            ctx.stroke_with_path(path);
        }
    }

    // Draw main line
    if let Some(ref path) = line_path {
        ctx.set_line_cap("round");
        ctx.set_line_join("round");
        ctx.set_stroke_style_str(&options.color);
        ctx.set_line_width(stroke_width);
        ctx.stroke_with_path(path);
    }

    // Fill envelopes
    for path in &envelope_paths {
        ctx.set_fill_style_str(&options.color);
        ctx.fill_with_path_2d(path);
    }

    Ok(())
}
