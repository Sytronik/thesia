// reference: https://librosa.org/doc/0.8.0/_modules/librosa/core/spectrum.html
#![allow(non_snake_case)]

use ndarray::DataMut;
use ndarray::prelude::*;
use ndarray_stats::{MaybeNan, QuantileExt};
use num_traits::{AsPrimitive, Float};

use super::super::simd::ScalarMulSIMDInplace;

const AMIN_AMP_DEFAULT: f32 = 0.0;
const AMIN_POWER_DEFAULT: f32 = 0.0;

pub enum DeciBelRef<A> {
    Value(A),
    _Max,
}

impl<A: Float> Default for DeciBelRef<A> {
    fn default() -> Self {
        DeciBelRef::Value(A::one())
    }
}

impl<A> DeciBelRef<A>
where
    A: Float + MaybeNan,
    <A as MaybeNan>::NotNan: Ord,
{
    fn into_value<D: Dimension>(self, data_for_max: &ArrayRef<A, D>) -> A {
        match self {
            DeciBelRef::Value(v) => {
                debug_assert!(v.is_sign_positive());
                v
            }
            DeciBelRef::_Max => *data_for_max.max_skipnan(),
        }
    }
}

pub trait DeciBel
where
    Self::A: Float,
{
    type A;

    fn log_for_dB(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self;
    fn dB_from_amp(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self;
    fn dB_from_power(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self;
    fn dB_from_amp_default(&self) -> Self;
    fn dB_from_power_default(&self) -> Self;
    fn amp_from_dB(&self, ref_value: Self::A) -> Self;
    fn power_from_dB(&self, ref_value: Self::A) -> Self;
    fn amp_from_dB_default(&self) -> Self;
    fn power_from_dB_default(&self) -> Self;
}

impl<A> DeciBel for A
where
    A: Float + MaybeNan + 'static,
    <A as MaybeNan>::NotNan: Ord,
    f32: AsPrimitive<A>,
{
    type A = A;

    fn log_for_dB(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self {
        debug_assert!(amin.is_sign_positive());
        let temp = [*self];
        let ref_value = reference.into_value(&ArrayView1::from(&temp[..]));
        if ref_value.is_nan() || ref_value.is_sign_negative() {
            return A::nan();
        }
        let log_amin = amin.log10();
        let log_ref = if ref_value > amin {
            ref_value.log10()
        } else {
            log_amin
        };
        let out_for_small = log_amin - log_ref;
        if self.is_nan() || self.is_sign_negative() {
            A::nan()
        } else if self > &amin {
            self.log10() - log_ref
        } else {
            out_for_small
        }
    }

    #[inline]
    fn dB_from_amp(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self {
        20.0.as_() * self.log_for_dB(reference, amin)
    }

    #[inline]
    fn dB_from_power(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self {
        10.0.as_() * self.log_for_dB(reference, amin)
    }

    #[inline]
    fn dB_from_amp_default(&self) -> Self {
        self.dB_from_amp(Default::default(), AMIN_AMP_DEFAULT.as_())
    }

    #[inline]
    fn dB_from_power_default(&self) -> Self {
        self.dB_from_power(Default::default(), AMIN_POWER_DEFAULT.as_())
    }

    #[inline]
    fn amp_from_dB(&self, ref_value: Self::A) -> Self {
        ref_value * 10.0.as_().powf(0.05.as_() * *self)
    }

    #[inline]
    fn power_from_dB(&self, ref_value: Self::A) -> Self {
        ref_value * 10.0.as_().powf(0.1.as_() * *self)
    }

    #[inline]
    fn amp_from_dB_default(&self) -> Self {
        if let DeciBelRef::Value(ref_value) = Default::default() {
            self.amp_from_dB(ref_value)
        } else {
            self.amp_from_dB(A::one())
        }
    }

    #[inline]
    fn power_from_dB_default(&self) -> Self {
        if let DeciBelRef::Value(ref_value) = Default::default() {
            self.power_from_dB(ref_value)
        } else {
            self.power_from_dB(A::one())
        }
    }
}

pub trait DeciBelInplace
where
    Self::A: Float,
{
    type A;
    fn log_for_dB_inplace(&mut self, reference: DeciBelRef<Self::A>, amin: Self::A);
    fn dB_from_amp_inplace(&mut self, reference: DeciBelRef<Self::A>, amin: Self::A);
    #[allow(dead_code)]
    fn dB_from_power_inplace(&mut self, reference: DeciBelRef<Self::A>, amin: Self::A);
    fn dB_from_amp_inplace_default(&mut self);
    #[allow(dead_code)]
    fn dB_from_power_inplace_default(&mut self);
    #[allow(dead_code)]
    fn amp_from_dB_inplace(&mut self, ref_value: Self::A);
    #[allow(dead_code)]
    fn power_from_dB_inplace(&mut self, ref_value: Self::A);
    #[allow(dead_code)]
    fn amp_from_dB_inplace_default(&mut self);
    #[allow(dead_code)]
    fn power_from_dB_inplace_default(&mut self);
}

impl<A, S, D> DeciBelInplace for ArrayBase<S, D>
where
    A: Float + MaybeNan + 'static,
    <A as MaybeNan>::NotNan: Ord,
    f32: AsPrimitive<A>,
    S: DataMut<Elem = A>,
    D: Dimension,
    ArrayBase<S, D>: ScalarMulSIMDInplace<A>,
{
    type A = A;
    fn log_for_dB_inplace(&mut self, reference: DeciBelRef<A>, amin: A) {
        debug_assert!(amin.is_sign_positive());
        let ref_value = reference.into_value(self);
        if ref_value.is_nan() {
            return;
        } else if ref_value.is_sign_negative() {
            self.fill(A::nan());
            return;
        }
        let log_amin = amin.log10();
        let log_ref = if ref_value > amin {
            ref_value.log10()
        } else {
            log_amin
        };
        let out_for_small = log_amin - log_ref;
        self.mapv_inplace(|x| {
            if x.is_nan() || x.is_sign_negative() {
                A::nan()
            } else if x > amin {
                x.log10() - log_ref
            } else {
                out_for_small
            }
        });
    }

    #[inline]
    fn dB_from_amp_inplace(&mut self, reference: DeciBelRef<A>, amin: A) {
        let factor = 20.0.as_();
        self.log_for_dB_inplace(reference, amin);
        self.scalar_mul_simd_inplace(factor);
    }

    #[inline]
    fn dB_from_power_inplace(&mut self, reference: DeciBelRef<A>, amin: A) {
        let factor = 10.0.as_();
        self.log_for_dB_inplace(reference, amin);
        self.scalar_mul_simd_inplace(factor);
    }

    #[inline]
    fn dB_from_amp_inplace_default(&mut self) {
        self.dB_from_amp_inplace(Default::default(), AMIN_AMP_DEFAULT.as_());
    }

    #[inline]
    fn dB_from_power_inplace_default(&mut self) {
        self.dB_from_power_inplace(Default::default(), AMIN_POWER_DEFAULT.as_());
    }

    #[inline]
    fn amp_from_dB_inplace(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * 10.0.as_().powf(0.05.as_() * x));
    }

    #[inline]
    fn power_from_dB_inplace(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * 10.0.as_().powf(0.1.as_() * x));
    }

    #[inline]
    fn amp_from_dB_inplace_default(&mut self) {
        if let DeciBelRef::Value(ref_value) = Default::default() {
            self.amp_from_dB_inplace(ref_value);
        } else {
            self.amp_from_dB_inplace(A::one());
        }
    }

    #[inline]
    fn power_from_dB_inplace_default(&mut self) {
        if let DeciBelRef::Value(ref_value) = Default::default() {
            self.power_from_dB_inplace(ref_value);
        } else {
            self.power_from_dB_inplace(A::one());
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;
    use ndarray::arr1;

    use super::*;

    #[test]
    #[allow(non_snake_case)]
    fn scalar_dB_conversions_round_trip() {
        let amp = 0.25f32;
        let amp_dB = amp.dB_from_amp_default();
        assert_abs_diff_eq!(amp_dB, -12.0412, epsilon = 1e-4);
        assert_abs_diff_eq!(amp_dB.amp_from_dB_default(), amp, epsilon = 1e-6);

        let power = 0.25f32;
        let power_dB = power.dB_from_power_default();
        assert_abs_diff_eq!(power_dB, -6.0206, epsilon = 1e-4);
        assert_abs_diff_eq!(power_dB.power_from_dB_default(), power, epsilon = 1e-6);
    }

    #[test]
    #[allow(non_snake_case)]
    fn scalar_dB_conversion_handles_floor_and_invalid_input() {
        assert_eq!(0.0f32.dB_from_amp_default(), f32::NEG_INFINITY);
        assert_eq!(0.0f32.dB_from_power_default(), f32::NEG_INFINITY);
        assert!((-1.0f32).dB_from_amp_default().is_nan());
        assert!(f32::NAN.dB_from_power_default().is_nan());

        let relative_to_two = 1.0f32.dB_from_amp(DeciBelRef::Value(2.0), AMIN_AMP_DEFAULT);
        assert_abs_diff_eq!(relative_to_two, -6.0206, epsilon = 1e-4);
    }

    #[test]
    #[allow(non_snake_case)]
    fn array_dB_inplace_conversion_matches_scalar_rules() {
        let mut amps = arr1(&[1.0f32, 0.5, 0.0, -1.0, f32::NAN]);
        amps.dB_from_amp_inplace(DeciBelRef::Value(1.0), 1e-3);

        assert_abs_diff_eq!(amps[0], 0.0);
        assert_abs_diff_eq!(amps[1], -6.0206, epsilon = 1e-4);
        assert_abs_diff_eq!(amps[2], -60.0);
        assert!(amps[3].is_nan());
        assert!(amps[4].is_nan());

        let mut powers = arr1(&[1.0f32, 0.25, 0.0]);
        powers.dB_from_power_inplace(DeciBelRef::_Max, 1e-6);

        assert_abs_diff_eq!(powers[0], 0.0);
        assert_abs_diff_eq!(powers[1], -6.0206, epsilon = 1e-4);
        assert_abs_diff_eq!(powers[2], -60.0);
    }
}
