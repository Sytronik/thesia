use std::sync::atomic::Ordering;

use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::wav::{
    DEVICE_PIXEL_RATIO, WAV_CACHES, WAV_CLIPPING_COLOR, WAV_COLOR, WavDrawingOptions,
    draw_wav_internal,
};

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
) -> Result<(), JsValue> {
    let dpr = DEVICE_PIXEL_RATIO.load(Ordering::Acquire);
    let width = css_width as f32 * dpr;
    let height = css_height as f32 * dpr;
    let gap = OVERVIEW_CH_GAP_HEIGHT * dpr;
    let overview_heights =
        OverviewHeights::new(height, gap, id_ch_arr.len(), OVERVIEW_GAIN_HEIGHT_RATIO);

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
        .map(|id_ch| wav_caches.get(id_ch).unwrap().cache_amp_range)
        .fold((-1.0f32, 1.0f32), |acc_amp_range, amp_range| {
            (
                acc_amp_range.0.min(amp_range.0),
                acc_amp_range.1.max(amp_range.1),
            )
        });
    let mut options = WavDrawingOptions {
        px_per_sec: css_width as f32 / max_track_sec,
        amp_range,
        scale: 1.0,
        line_width: OVERVIEW_LINE_WIDTH,
        need_border_for_envelope: false,
        need_border_for_line: false,
        ..Default::default()
    };
    for (i, id_ch) in id_ch_arr.iter().enumerate() {
        options.offset_y = i as f32 * (overview_heights.ch + overview_heights.gap);
        options.clip_values = None;

        if wav_caches.get(id_ch).unwrap().is_clipped {
            draw_wav_internal(
                ctx,
                id_ch,
                width,
                overview_heights.ch,
                &options,
                WAV_CLIPPING_COLOR,
            )?;
            options.clip_values = Some((-1., 1.));
        }

        draw_wav_internal(ctx, id_ch, width, overview_heights.ch, &options, WAV_COLOR)?;
    }
    Ok(())
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
