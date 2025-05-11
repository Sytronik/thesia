//! interfaces to communicate with JS world

// allow for whole file because [napi(object)] attribite on struct blocks allow(non_snake_case)
#![allow(non_snake_case)]

use fast_image_resize::pixels;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use ndarray::Array2;

use crate::{
    GuardClippingMode, IdChValueVec, OverviewDrawingInfoInternal, SlicedWavDrawingInfo,
    SpecSetting, SpectrogramSliceArgs, WavDrawingInfoInternal,
};

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

impl WavDrawingInfo {
    pub fn new(internal: SlicedWavDrawingInfo, start_sec: f64) -> Self {
        if internal.drawing_sec == 0. {
            return Self {
                line: Some(Default::default()),
                start_sec,
                ..Default::default()
            };
        }
        let base = Self {
            start_sec,
            ..Default::default()
        };
        match internal.drawing_info {
            WavDrawingInfoInternal::FillRect => base,
            WavDrawingInfoInternal::Line(line, clip_values) => {
                let buf: &[u8] = bytemuck::cast_slice(&line);

                let points_per_sec = line.len() as f64 / internal.drawing_sec;
                Self {
                    line: Some(buf.into()),
                    points_per_sec,
                    pre_margin: internal.pre_margin_sec * points_per_sec,
                    post_margin: internal.post_margin_sec * points_per_sec,
                    clip_values: clip_values.map(|(x, y)| vec![x as f64, y as f64]),
                    ..base
                }
            }
            WavDrawingInfoInternal::TopBottomEnvelope(
                top_envelope,
                bottom_envelope,
                clip_values,
            ) => {
                let top_buf: &[u8] = bytemuck::cast_slice(&top_envelope);
                let bottom_buf: &[u8] = bytemuck::cast_slice(&bottom_envelope);

                let points_per_sec = top_envelope.len() as f64 / internal.drawing_sec;
                Self {
                    top_envelope: Some(top_buf.into()),
                    bottom_envelope: Some(bottom_buf.into()),
                    points_per_sec,
                    pre_margin: internal.pre_margin_sec * points_per_sec,
                    post_margin: internal.post_margin_sec * points_per_sec,
                    clip_values: clip_values.map(|(x, y)| vec![x as f64, y as f64]),
                    ..base
                }
            }
        }
    }
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

impl OverviewDrawingInfo {
    pub fn new(internal: OverviewDrawingInfoInternal, track_sec: f64) -> Self {
        let OverviewDrawingInfoInternal {
            ch_drawing_infos,
            limiter_gain_infos,
            heights,
        } = internal;
        let convert = |drawing_info| {
            WavDrawingInfo::new(
                SlicedWavDrawingInfo {
                    drawing_info,
                    drawing_sec: track_sec,
                    pre_margin_sec: 0.,
                    post_margin_sec: 0.,
                },
                0.,
            )
        };
        let ch_drawing_infos = ch_drawing_infos.into_iter().map(convert).collect();
        let (top, bottom) = limiter_gain_infos.map_or((None, None), |(top, bottom)| {
            (Some(convert(top)), Some(convert(bottom)))
        });
        Self {
            ch_drawing_infos,
            limiter_gain_top_info: top,
            limiter_gain_bottom_info: bottom,
            ch_height: heights.ch as f64,
            gap_height: heights.gap as f64,
            limiter_gain_height: heights.gain as f64,
            ch_wo_gain_height: heights.ch_wo_gain as f64,
        }
    }
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
