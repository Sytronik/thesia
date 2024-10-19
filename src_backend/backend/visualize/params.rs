use approx::{relative_ne, AbsDiffEq, RelativeEq};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq)]
pub struct DrawParams {
    pub start_sec: f64,
    pub width: u32,
    pub height: u32,
    pub px_per_sec: f64,
    pub opt_for_wav: DrawOptionForWav,
    pub blend: f64,
}

impl DrawParams {
    pub fn is_params_for_different_img_cache(&self, other: &Self) -> bool {
        self.height != other.height || relative_ne!(self.px_per_sec, other.px_per_sec)
    }

    pub fn is_params_for_different_wav_cache(&self, other: &Self) -> bool {
        relative_ne!(self.opt_for_wav, other.opt_for_wav)
    }
}

impl Default for DrawParams {
    fn default() -> Self {
        DrawParams {
            start_sec: 0.,
            width: 1,
            height: 1,
            px_per_sec: 0.,
            opt_for_wav: Default::default(),
            blend: 1.,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DrawOptionForWav {
    pub amp_range: (f32, f32),
    pub dpr: f32,
}

impl DrawOptionForWav {
    pub fn with_dpr(dpr: f32) -> Self {
        DrawOptionForWav {
            dpr,
            ..Default::default()
        }
    }
}

impl Default for DrawOptionForWav {
    fn default() -> Self {
        DrawOptionForWav {
            amp_range: (-1., 1.),
            dpr: 1.,
        }
    }
}

impl AbsDiffEq for DrawOptionForWav {
    type Epsilon = f32;

    fn default_epsilon() -> Self::Epsilon {
        f32::EPSILON
    }

    fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.amp_range.0.abs_diff_eq(&other.amp_range.0, epsilon)
            && self.amp_range.1.abs_diff_eq(&other.amp_range.1, epsilon)
            && self.dpr.abs_diff_eq(&other.dpr, epsilon)
    }

    fn abs_diff_ne(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
        self.amp_range.0.abs_diff_ne(&other.amp_range.0, epsilon)
            || self.amp_range.1.abs_diff_ne(&other.amp_range.1, epsilon)
            || self.dpr.abs_diff_ne(&other.dpr, epsilon)
    }
}

impl RelativeEq for DrawOptionForWav {
    fn default_max_relative() -> Self::Epsilon {
        f32::EPSILON
    }

    fn relative_eq(
        &self,
        other: &Self,
        epsilon: Self::Epsilon,
        max_relative: Self::Epsilon,
    ) -> bool {
        self.amp_range
            .0
            .relative_eq(&other.amp_range.0, epsilon, max_relative)
            && self
                .amp_range
                .1
                .relative_eq(&other.amp_range.1, epsilon, max_relative)
            && self.dpr.relative_eq(&other.dpr, epsilon, max_relative)
    }

    fn relative_ne(
        &self,
        other: &Self,
        epsilon: Self::Epsilon,
        max_relative: Self::Epsilon,
    ) -> bool {
        self.amp_range
            .0
            .relative_ne(&other.amp_range.0, epsilon, max_relative)
            || self
                .amp_range
                .1
                .relative_ne(&other.amp_range.1, epsilon, max_relative)
            || self.dpr.relative_ne(&other.dpr, epsilon, max_relative)
    }
}

pub enum ImageKind<'a> {
    Spec,
    Wav(&'a DrawOptionForWav),
}
