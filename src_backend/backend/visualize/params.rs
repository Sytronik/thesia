use approx::{AbsDiffEq, RelativeEq};
use serde::{Deserialize, Serialize};

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
