//! interfaces to communicate with JS world

// allow for whole file because [napi(object)] attribite on struct blocks allow(non_snake_case)
#![allow(non_snake_case)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::{GuardClippingMode, IdChValueVec, IdChVec, SpecSetting};

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
    pub px_per_sec: f64,
    pub left_margin: f64,
    pub right_margin: f64,
    pub top_margin: f64,
    pub bottom_margin: f64,
}

#[napi(object)]
pub struct WavImage {
    pub buf: Buffer,
    pub width: u32,
    pub height: u32,
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

#[derive(Default)]
pub struct IdChWavImages(pub IdChValueVec<WavImage>);

impl TypeName for IdChWavImages {
    fn type_name() -> &'static str {
        "HashMap"
    }

    fn value_type() -> ValueType {
        ValueType::Object
    }
}

impl ValidateNapiValue for IdChWavImages {}

impl ToNapiValue for IdChWavImages {
    unsafe fn to_napi_value(raw_env: sys::napi_env, val: Self) -> Result<sys::napi_value> {
        let env = Env::from(raw_env);
        let mut obj = env.create_object()?;
        for ((id, ch), waveform) in val.0.into_iter() {
            obj.set(format_id_ch(id, ch), waveform)?;
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
        _ => {
            return Err(Error::new(
                Status::Unknown,
                "The array element should be \"{unsigned_int}_{unsigned_int}\".",
            ));
        }
    }
}

pub fn parse_id_ch_tuples(id_ch_strs: Vec<String>) -> Result<IdChVec> {
    let mut result = IdChVec::with_capacity(id_ch_strs.len());
    for s in id_ch_strs {
        match parse_id_ch_str(&s) {
            Ok(id_ch) => {
                result.push(id_ch);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
    Ok(result)
}
