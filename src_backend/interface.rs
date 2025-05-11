//! interfaces to communicate with JS world

// allow for whole file because [napi(object)] attribite on struct blocks allow(non_snake_case)
#![allow(non_snake_case)]

use fast_image_resize::pixels;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use ndarray::Array2;

use crate::{GuardClippingMode, IdChValueVec, SpecSetting, SpectrogramSliceArgs};

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
}

impl Spectrogram {
    pub fn new(args: SpectrogramSliceArgs, mipmap: Array2<pixels::F32>, start_sec: f64) -> Self {
        let (pixels_vec, _) = mipmap.into_raw_vec_and_offset();
        let f32_slice = unsafe {
            std::slice::from_raw_parts(pixels_vec.as_ptr() as *const f32, pixels_vec.len())
        };
        let buf: &[u8] = bytemuck::cast_slice(f32_slice);

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
        }
    }
}

#[napi(object)]
#[derive(Default)]
pub struct WavDrawingInfo {
    pub line: Option<Buffer>,
    pub top_envelope: Option<Buffer>,
    pub bottom_envelope: Option<Buffer>,
    pub start_sec: f64,
    pub points_per_sec: f64,
    pub pre_margin: f64,
    pub post_margin: f64,
    pub clip_values: Option<Vec<f64>>,
}

#[napi(object)]
pub struct OverviewDrawingInfo {
    pub ch_drawing_infos: Vec<WavDrawingInfo>,
    pub limiter_gain_top_info: Option<WavDrawingInfo>,
    pub limiter_gain_bottom_info: Option<WavDrawingInfo>,
    pub ch_height: f64,
    pub gap_height: f64,
    pub limiter_gain_height: f64,
    pub ch_wo_gain_height: f64,
}

#[derive(Default)]
pub struct IdChSpectrograms(pub IdChValueVec<Spectrogram>);

impl TypeName for IdChSpectrograms {
    fn type_name() -> &'static str {
        "HashMap"
    }

    fn value_type() -> ValueType {
        ValueType::Object
    }
}

impl ValidateNapiValue for IdChSpectrograms {}

impl ToNapiValue for IdChSpectrograms {
    unsafe fn to_napi_value(raw_env: sys::napi_env, val: Self) -> Result<sys::napi_value> {
        let env = Env::from(raw_env);
        let mut obj = env.create_object()?;
        for ((id, ch), spectrogram) in val.0.into_iter() {
            obj.set(format_id_ch(id, ch), spectrogram)?;
        }

        unsafe { Object::to_napi_value(raw_env, obj) }
    }
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
