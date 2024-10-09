use serde::{Deserialize, Serialize};

use super::guardclipping::GuardClippingMode;
use super::stats::AudioStats;

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

pub trait Normalize {
    #[inline]
    fn normalize(&mut self, target: NormalizeTarget, guard_clipping_mode: GuardClippingMode) {
        self.normalize_default(target, guard_clipping_mode);
    }

    fn normalize_default(
        &mut self,
        target: NormalizeTarget,
        guard_clipping_mode: GuardClippingMode,
    ) {
        // TODO: guard clipping can make lufs/rms different from target
        let gain = match target {
            NormalizeTarget::LUFS(target_lufs) => {
                10f32.powf((target_lufs - self.stats_for_normalize().global_lufs as f32) / 20.)
            }
            #[allow(non_snake_case)]
            NormalizeTarget::RMSdB(target_dB) => {
                10f32.powf((target_dB - self.stats_for_normalize().rms_dB) / 20.)
            }
            #[allow(non_snake_case)]
            NormalizeTarget::PeakdB(target_peak_dB) => {
                debug_assert!(target_peak_dB <= 0.);
                10f32.powf((target_peak_dB - self.stats_for_normalize().max_peak_dB) / 20.)
            }
            NormalizeTarget::Off => 1.,
        };
        self.apply_gain(gain, guard_clipping_mode);
    }

    fn stats_for_normalize(&self) -> &AudioStats;

    fn apply_gain(&mut self, gain: f32, guard_clipping_mode: GuardClippingMode);
}
