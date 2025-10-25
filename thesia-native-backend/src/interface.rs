//! interfaces to communicate with JS world

// allow for whole file because [napi(object)] attribite on struct blocks allow(non_snake_case)
#![allow(non_snake_case)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ndarray::Array2;

use crate::{GuardClippingMode, SpecSetting, SpectrogramSliceArgs};

#[napi(object)]
pub struct UserSettingsOptionals {
    pub spec_setting: Option<SpecSetting>,
    pub blend: Option<f64>,

    #[napi(js_name = "dBRange")]
    pub dB_range: Option<f64>,

    pub common_guard_clipping: Option<GuardClippingMode>,
    pub common_normalize: Option<serde_json::Value>,
}

#[napi(object)]
pub struct UserSettings {
    pub spec_setting: SpecSetting,
    pub blend: f64,

    #[napi(js_name = "dBRange")]
    pub dB_range: f64,

    pub common_guard_clipping: GuardClippingMode,
    pub common_normalize: serde_json::Value,
}

#[napi(object)]
pub struct PlayerState {
    pub is_playing: bool,
    pub position_sec: f64,
    pub err: String,
}

#[napi(object)]
#[derive(Default)]
pub struct Spectrogram {
    pub buf: Buffer,
    pub width: u32,
    pub height: u32,
    pub start_sec: f64,
    pub px_per_sec: f64,
    pub left_margin: f64,
    pub right_margin: f64,
    pub top_margin: f64,
    pub bottom_margin: f64,
    pub is_low_quality: bool,
}

impl Spectrogram {
    pub fn new(
        args: SpectrogramSliceArgs,
        mipmap: Array2<f32>,
        start_sec: f64,
        is_low_quality: bool,
    ) -> Self {
        let buf: &[u8] = bytemuck::cast_slice(mipmap.as_slice().unwrap());

        Self {
            buf: buf.into(),
            width: args.width as u32,
            height: args.height as u32,
            start_sec,
            px_per_sec: args.px_per_sec,
            left_margin: args.left_margin,
            right_margin: args.right_margin,
            top_margin: args.top_margin,
            bottom_margin: args.bottom_margin,
            is_low_quality,
        }
    }
}

#[napi(object)]
#[derive(Default)]
pub struct WavMetadata {
    pub length: u32,
    pub sr: u32,
    pub is_clipped: bool,
}

#[inline]
pub fn format_id_ch(id: usize, ch: usize) -> String {
    format!("{}_{}", id, ch)
}

#[inline]
pub fn parse_id_ch_str(id_ch_str: &str) -> Result<(usize, usize)> {
    let mut iter = id_ch_str.split('_').map(|x| x.parse::<usize>());
    match (iter.next(), iter.next()) {
        (Some(Ok(id)), Some(Ok(ch))) => Ok((id, ch)),
        _ => Err(Error::new(
            Status::Unknown,
            "The array element should be \"{unsigned_int}_{unsigned_int}\".",
        )),
    }
}
