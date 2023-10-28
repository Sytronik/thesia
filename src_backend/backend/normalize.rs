use napi::bindgen_prelude::{FromNapiValue, ToNapiValue};
use napi_derive::napi;
use ndarray::prelude::*;
use ndarray::{Data, DataMut};
use serde::{Deserialize, Serialize};

use super::stats::AudioStats;

#[napi(string_enum)]
#[derive(Default)]
pub enum GuardClippingMode {
    #[default]
    None,
    ReduceGlobalLevel,
    Limiter,
}

pub trait MaxPeak {
    fn max_peak(&self) -> f32;
}

impl<S, D> MaxPeak for ArrayBase<S, D>
where
    S: Data<Elem = f32>,
    D: Dimension,
{
    fn max_peak(&self) -> f32 {
        // TODO: better one?
        // f32::max(self.max_skipnan().abs(), self.min_skipnan().abs())
        self.iter()
            .map(|x| x.abs())
            .reduce(|max, x| f32::max(max, x))
            .unwrap_or_default()
    }
}

pub trait GuardClipping: MaxPeak + Sized {
    fn guard_clipping(&mut self, mode: GuardClippingMode);
    fn mutate_with_guard_clipping<F>(&mut self, f: F, mode: GuardClippingMode)
    where
        F: Fn(&mut Self);
}

impl<S, D> GuardClipping for ArrayBase<S, D>
where
    S: DataMut<Elem = f32>,
    D: Dimension,
{
    fn guard_clipping(&mut self, mode: GuardClippingMode) {
        match mode {
            GuardClippingMode::None => {
                self.mapv_inplace(|x| x.clamp(-1., 1.));
            }
            GuardClippingMode::ReduceGlobalLevel => {
                let peak = self.max_peak();
                if peak > 1. {
                    self.mapv_inplace(|x| (x / peak).clamp(-1., 1.));
                }
            }
            GuardClippingMode::Limiter => {
                unimplemented!();
            }
        }
    }

    fn mutate_with_guard_clipping<F>(&mut self, f: F, mode: GuardClippingMode)
    where
        F: Fn(&mut Self),
    {
        f(self);
        self.guard_clipping(mode);
    }
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(tag = "type", content = "target")]
pub enum NormalizeTarget {
    #[default]
    None,
    LUFS(f32),
    RMSdB(f32),
    PeakdB(f32),
}

pub trait Normalize {
    fn normalize(&mut self, target: NormalizeTarget, guard_clipping_mode: GuardClippingMode) {
        self.normalize_default(target, guard_clipping_mode);
    }

    fn normalize_default(
        &mut self,
        target: NormalizeTarget,
        guard_clipping_mode: GuardClippingMode,
    ) {
        // TODO: guard clipping can make rms different from target_db
        let gain = match target {
            NormalizeTarget::LUFS(target_lufs) => {
                10f32.powf((target_lufs - self.stats_for_normalize().global_lufs as f32) / 20.)
            }
            NormalizeTarget::RMSdB(target_db) => {
                10f32.powf((target_db - self.stats_for_normalize().rms_db) / 20.)
            }
            NormalizeTarget::PeakdB(target_peak_db) => {
                assert!(target_peak_db <= 0.);
                10f32.powf((target_peak_db - self.stats_for_normalize().max_peak_db) / 20.)
            }
            NormalizeTarget::None => 1.,
        };
        self.apply_gain(gain, guard_clipping_mode);
    }

    fn stats_for_normalize(&self) -> &AudioStats;

    fn apply_gain(&mut self, gain: f32, guard_clipping_mode: GuardClippingMode);
}
