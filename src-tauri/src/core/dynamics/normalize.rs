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

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;

    use super::*;

    struct NormalizeRecorder {
        stats: AudioStats,
        gain: f32,
        mode: GuardClippingMode,
    }

    impl NormalizeRecorder {
        fn new() -> Self {
            Self {
                stats: AudioStats::new_for_test(-23.0, -12.0, 0.5, -6.0),
                gain: f32::NAN,
                mode: GuardClippingMode::Clip,
            }
        }
    }

    impl Normalize for NormalizeRecorder {
        fn stats_for_normalize(&self) -> &AudioStats {
            &self.stats
        }

        fn apply_gain(&mut self, gain: f32, guard_clipping_mode: GuardClippingMode) {
            self.gain = gain;
            self.mode = guard_clipping_mode;
        }
    }

    #[test]
    fn normalize_off_applies_unity_gain_with_selected_guard_mode() {
        let mut recorder = NormalizeRecorder::new();
        recorder.normalize(NormalizeTarget::Off, GuardClippingMode::Limiter);

        assert_abs_diff_eq!(recorder.gain, 1.0);
        assert!(recorder.mode == GuardClippingMode::Limiter);
    }

    #[test]
    fn normalize_targets_use_original_stats_to_compute_gain() {
        let mut recorder = NormalizeRecorder::new();
        recorder.normalize(NormalizeTarget::LUFS(-20.0), GuardClippingMode::Clip);
        assert_abs_diff_eq!(recorder.gain, 10f32.powf(3.0 / 20.0), epsilon = 1e-6);

        recorder.normalize(
            NormalizeTarget::RMSdB(-18.0),
            GuardClippingMode::ReduceGlobalLevel,
        );
        assert_abs_diff_eq!(recorder.gain, 10f32.powf(-6.0 / 20.0), epsilon = 1e-6);
        assert!(recorder.mode == GuardClippingMode::ReduceGlobalLevel);

        recorder.normalize(NormalizeTarget::PeakdB(-1.0), GuardClippingMode::Limiter);
        assert_abs_diff_eq!(recorder.gain, 10f32.powf(5.0 / 20.0), epsilon = 1e-6);
        assert!(recorder.mode == GuardClippingMode::Limiter);
    }
}
