use std::fmt::Display;

use napi_derive::napi;
use ndarray::prelude::*;

#[napi(string_enum)]
#[derive(Default, Eq, PartialEq)]
pub enum GuardClippingMode {
    #[default]
    Clip,
    ReduceGlobalLevel,
    Limiter,
}

impl Display for GuardClippingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GuardClippingMode::Clip => write!(f, "clipped"),
            GuardClippingMode::ReduceGlobalLevel => write!(f, "globally reduced"),
            GuardClippingMode::Limiter => write!(f, "reduced"),
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum GuardClippingResult<D: Dimension> {
    WavBeforeClip(Array<f32, D>),
    GlobalGain((f32, D)),
    GainSequence(Array<f32, D>),
}

pub trait GuardClipping<D: Dimension> {
    #[inline]
    fn guard_clipping(&mut self, mode: GuardClippingMode) -> GuardClippingResult<D> {
        match mode {
            GuardClippingMode::Clip => self.clip(),
            GuardClippingMode::ReduceGlobalLevel => self.reduce_global_level(),
            GuardClippingMode::Limiter => self.limit(),
        }
    }

    fn clip(&mut self) -> GuardClippingResult<D>;
    fn reduce_global_level(&mut self) -> GuardClippingResult<D>;
    fn limit(&mut self) -> GuardClippingResult<D>;
}
