//! interfaces to communicate with JS world

// allow for whole file because [napi(object)] attribite on struct blocks allow(non_snake_case)
#![allow(non_snake_case)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::backend::{GuardClippingMode, IdChValueVec, IdChVec, SpecSetting};

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

#[derive(Default)]
pub struct IdChImages(pub IdChValueVec<Vec<u8>>);

impl TypeName for IdChImages {
    fn type_name() -> &'static str {
        "HashMap"
    }

    fn value_type() -> ValueType {
        ValueType::Object
    }
}

impl ValidateNapiValue for IdChImages {}

impl ToNapiValue for IdChImages {
    unsafe fn to_napi_value(raw_env: sys::napi_env, val: Self) -> Result<sys::napi_value> {
        let env = Env::from(raw_env);
        let mut obj = env.create_object()?;
        for ((id, ch), v) in val.0.into_iter() {
            obj.set(&format_id_ch(id, ch), Buffer::from(v))?;
        }

        unsafe { Object::to_napi_value(raw_env, obj) }
    }
}

#[inline]
pub fn format_id_ch(id: usize, ch: usize) -> String {
    format!("{}_{}", id, ch)
}

pub fn parse_id_ch_tuples(id_ch_strs: Vec<String>) -> Result<IdChVec> {
    let mut result = IdChVec::with_capacity(id_ch_strs.len());
    for s in id_ch_strs {
        let mut iter = s.split('_').map(|x| x.parse::<usize>());
        match (iter.next(), iter.next()) {
            (Some(Ok(id)), Some(Ok(ch))) => {
                result.push((id, ch));
            }
            _ => {
                return Err(Error::new(
                    Status::Unknown,
                    "The array element should be \"{unsigned_int}_{unsigned_int}\".",
                ));
            }
        }
    }
    Ok(result)
}
