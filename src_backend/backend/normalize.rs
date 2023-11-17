use napi::bindgen_prelude::{FromNapiValue, ToNapiValue};
use napi_derive::napi;
use serde::{Deserialize, Serialize};

use super::stats::AudioStats;

#[napi(string_enum)]
#[derive(Default)]
pub enum GuardClippingMode {
    #[default]
    Clip,
    ReduceGlobalLevel,
    Limiter,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(tag = "type", content = "target")]
#[allow(clippy::upper_case_acronyms)]
pub enum NormalizeTarget {
    #[default]
    Off,
    LUFS(f32),
    RMSdB(f32),
    PeakdB(f32),
}

pub trait GuardClipping {
    type GainArray;

    fn guard_clipping(&mut self, mode: GuardClippingMode) -> Self::GainArray {
        match mode {
            GuardClippingMode::Clip => self.clip(),
            GuardClippingMode::ReduceGlobalLevel => self.reduce_global_level(),
            GuardClippingMode::Limiter => self.limit(),
        }
    }

    fn clip(&mut self) -> Self::GainArray;
    fn reduce_global_level(&mut self) -> Self::GainArray;
    fn limit(&mut self) -> Self::GainArray;
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
        // TODO: guard clipping can make lufs/rms different from target_db
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
            NormalizeTarget::Off => 1.,
        };
        self.apply_gain(gain, guard_clipping_mode);
    }

    fn stats_for_normalize(&self) -> &AudioStats;

    fn apply_gain(&mut self, gain: f32, guard_clipping_mode: GuardClippingMode);
}
