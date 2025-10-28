use std::sync::atomic::Ordering;

use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::mem::WasmFloat32Array;
use crate::wav::{
    DEVICE_PIXEL_RATIO, WAV_CACHES, WAV_CLIPPING_COLOR, WAV_COLOR, WavDrawingOptions,
    WavLinePoints, draw_wav_internal,
};

const LIMITER_GAIN_COLOR: &str = "rgb(218, 151, 46)";

const OVERVIEW_CH_GAP_HEIGHT: f32 = 1.;
const OVERVIEW_GAIN_HEIGHT_RATIO: f32 = 0.2;
const OVERVIEW_LINE_WIDTH: f32 = 1.;

#[wasm_bindgen(js_name = drawOverview)]
pub fn draw_overview(
    canvas: &HtmlCanvasElement,
    ctx: &CanvasRenderingContext2d,
    id_ch_arr: Vec<String>,
    css_width: u32,
    css_height: u32,
    max_track_sec: f32,
    limiter_gain_seq: Option<WasmFloat32Array>,
) -> Result<(), JsValue> {
    let dpr = DEVICE_PIXEL_RATIO.load(Ordering::Acquire);
    let width = css_width as f32 * dpr;
    let height = css_height as f32 * dpr;
    let gap = OVERVIEW_CH_GAP_HEIGHT * dpr;

    canvas.set_width(width.round() as u32);
    canvas.set_height(height.round() as u32);

    ctx.clear_rect(0.0, 0.0, width as f64, height as f64);

    let wav_caches = WAV_CACHES.read().unwrap();
    if id_ch_arr
        .iter()
        .any(|id_ch| wav_caches.get(id_ch).is_none())
    {
        return Ok(());
    }

    let amp_range = id_ch_arr
        .iter()
        .map(|id_ch| wav_caches.get(id_ch).unwrap().cache_amp_range())
        .fold((-1.0f32, 1.0f32), |acc_amp_range, amp_range| {
            (
                acc_amp_range.0.min(amp_range.0),
                acc_amp_range.1.max(amp_range.1),
            )
        });
    let overview_heights =
        OverviewHeights::new(height, gap, id_ch_arr.len(), OVERVIEW_GAIN_HEIGHT_RATIO);
    let mut options = WavDrawingOptions {
        px_per_sec: css_width as f32 / max_track_sec,
        amp_range,
        scale: 1.0,
        line_width: OVERVIEW_LINE_WIDTH,
        need_border_for_envelope: false,
        need_border_for_line: false,
        ..Default::default()
    };

    let limiter_gain_seq: Option<Vec<_>> = limiter_gain_seq.map(|seq| seq.into());
    let mut height_ch = overview_heights.ch;
    let mut limiter_gain_seq_iter = limiter_gain_seq.as_ref().map(|seq| {
        let length = seq.len() / id_ch_arr.len();
        height_ch = overview_heights.ch_wo_gain;
        seq.chunks_exact(length)
    });
    for (i, id_ch) in id_ch_arr.iter().enumerate() {
        options.offset_y = i as f32 * (overview_heights.ch + overview_heights.gap);
        options.clip_values = None;

        if let Some(gain_seq) = limiter_gain_seq_iter.as_mut().and_then(|iter| iter.next()) {
            draw_limiter_gain(
                ctx,
                gain_seq,
                width,
                overview_heights.gain,
                options.offset_y,
                options.offset_y + overview_heights.ch_wo_gain + overview_heights.gain,
                (0.5, 1.0),
            )?;
            options.offset_y += overview_heights.gain;
        }

        let upsampled_wav_sr = if wav_caches.get(id_ch).unwrap().is_clipped() {
            let upsampled_wav_sr = draw_wav_internal(
                ctx,
                id_ch,
                width,
                height_ch,
                &options,
                WAV_CLIPPING_COLOR,
                None,
            )?;
            options.clip_values = Some((-1., 1.));
            upsampled_wav_sr
        } else {
            None
        };

        draw_wav_internal(
            ctx,
            id_ch,
            width,
            height_ch,
            &options,
            WAV_COLOR,
            upsampled_wav_sr,
        )?;
    }
    Ok(())
}

fn draw_limiter_gain(
    ctx: &CanvasRenderingContext2d,
    gain_seq: &[f32],
    width: f32,
    height: f32,
    offset_y_above_part: f32,
    offset_y_below_part: f32,
    gain_range: (f32, f32),
) -> Result<(), JsValue> {
    let above_envelopes = calc_limiter_gain_envelopes(gain_seq, width, height, gain_range);
    for mut above_envelope in above_envelopes {
        let mut below_envelope = above_envelope.upside_down();
        below_envelope.shift_y_inplace(offset_y_below_part + height);
        above_envelope.shift_y_inplace(offset_y_above_part);

        let above_path = above_envelope.try_into_path()?;
        above_path.close_path();
        let below_path = below_envelope.try_into_path()?;
        below_path.close_path();

        ctx.set_fill_style_str(LIMITER_GAIN_COLOR);
        ctx.fill_with_path_2d(&above_path);
        ctx.fill_with_path_2d(&below_path);
    }
    Ok(())
}

fn calc_limiter_gain_envelopes(
    gain_seq: &[f32],
    width: f32,
    height: f32,
    gain_range: (f32, f32),
) -> Vec<WavLinePoints> {
    let x_scale = width / gain_seq.len() as f32;
    let idx_to_x = move |i: usize| i as f32 * x_scale;

    let y_scale = -height / (gain_range.1 - gain_range.0).max(1e-8);
    let y_offset = -gain_range.1 * y_scale;
    let wav_to_y = move |v: f32| v.mul_add(y_scale, y_offset);

    let y_unity_gain = wav_to_y(gain_range.1);
    let mut current_envlp = WavLinePoints::new();
    let mut envelopes = Vec::new();

    let mut i = 0;
    while i < gain_seq.len() {
        let x = idx_to_x(i);
        let x_floor = x.floor();
        let x_mid = x_floor + 0.5;

        let mut i2 = i;
        let mut i_next = gain_seq.len();
        while i2 < gain_seq.len() {
            let x2 = idx_to_x(i2);
            let x2_floor = x2.floor();
            if x2_floor > x_floor && i_next == gain_seq.len() {
                i_next = i2;
            }
            if x2_floor > x_floor + 1.0 {
                break;
            }
            i2 += 1;
        }
        if i2 == i {
            i2 = (i + 1).min(gain_seq.len());
        }

        let min_v = gain_seq[i..i2].iter().fold(gain_range.1, |a, &b| a.min(b));
        let bottom = wav_to_y(min_v);

        if bottom > y_unity_gain {
            if current_envlp.is_empty() {
                // new envelope starts
                current_envlp.push(x_floor, y_unity_gain);
            }

            // continue the envelope
            current_envlp.push(x_mid, bottom);
        } else {
            // no need to draw envelope
            if !current_envlp.is_empty() {
                // finish the recent envelope
                current_envlp.push(x_floor, y_unity_gain);

                envelopes.push(current_envlp);
                current_envlp = WavLinePoints::new();
            }
        }
        i = i_next;
    }
    // Handle remaining envelope
    if !current_envlp.is_empty() {
        let last_x = idx_to_x(gain_seq.len() - 1);
        let last_x_floor = last_x.floor();
        let last_x_ceil = last_x_floor + 1.0;
        let last_y = wav_to_y(gain_seq[gain_seq.len() - 1]);

        current_envlp.push(last_x_ceil, last_y);
        envelopes.push(current_envlp);
    }
    envelopes
}

/// Heights of the overview
/// height (total) = ch + gap + ... + ch
/// ch = gain + ch_wo_gain + gain
struct OverviewHeights {
    ch: f32,
    gap: f32,
    gain: f32,
    ch_wo_gain: f32,
}

impl OverviewHeights {
    fn new(height: f32, gap: f32, n_ch: usize, gain_height_ratio: f32) -> Self {
        let height_without_gap = height - gap * ((n_ch - 1) as f32);
        let ch = height_without_gap / n_ch as f32;
        let gain = ch * gain_height_ratio;
        let ch_wo_gain = ch - 2. * gain;
        OverviewHeights {
            ch,
            gap,
            gain,
            ch_wo_gain,
        }
    }
}
