// reference: https://librosa.org/doc/0.8.0/_modules/librosa/core/spectrum.html
#![allow(non_snake_case)]

use ndarray::prelude::*;
use ndarray::DataMut;
use ndarray_stats::{MaybeNan, QuantileExt};
use rustfft::num_traits::Float;

const AMIN_AMP_DEFAULT: f32 = 1e-18;
const AMIN_POWER_DEFAULT: f32 = 1e-36;

pub enum DeciBelRef<A: Float> {
    Value(A),
    Max,
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
    fn into_value<D: Dimension>(self, data_for_max: ArrayView<A, D>) -> A {
        match self {
            DeciBelRef::Value(v) => {
                assert!(v >= A::zero());
                v
            }
            DeciBelRef::Max => *data_for_max.max_skipnan(),
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
    A: Float + MaybeNan,
    <A as MaybeNan>::NotNan: Ord,
{
    type A = A;
    fn log_for_dB(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self {
        assert!(amin >= A::zero());
        let temp = [*self];
        let ref_value = reference.into_value(temp[..].into());
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
        A::from(20.).unwrap() * self.log_for_dB(reference, amin)
    }

    #[inline]
    fn dB_from_power(&self, reference: DeciBelRef<Self::A>, amin: Self::A) -> Self {
        A::from(10.).unwrap() * self.log_for_dB(reference, amin)
    }

    #[inline]
    fn dB_from_amp_default(&self) -> Self {
        self.dB_from_amp(Default::default(), A::from(AMIN_AMP_DEFAULT).unwrap())
    }

    #[inline]
    fn dB_from_power_default(&self) -> Self {
        self.dB_from_power(Default::default(), A::from(AMIN_POWER_DEFAULT).unwrap())
    }

    #[inline]
    fn amp_from_dB(&self, ref_value: Self::A) -> Self {
        ref_value * A::from(10.).unwrap().powf(A::from(0.05).unwrap() * *self)
    }

    #[inline]
    fn power_from_dB(&self, ref_value: Self::A) -> Self {
        ref_value * A::from(10.).unwrap().powf(A::from(0.05).unwrap() * *self)
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
    fn dB_from_power_inplace(&mut self, reference: DeciBelRef<Self::A>, amin: Self::A);
    fn dB_from_amp_inplace_default(&mut self);
    fn dB_from_power_inplace_default(&mut self);
    fn amp_from_dB_inplace(&mut self, ref_value: Self::A);
    fn power_from_dB_inplace(&mut self, ref_value: Self::A);
    fn amp_from_dB_inplace_default(&mut self);
    fn power_from_dB_inplace_default(&mut self);
}

impl<A, S, D> DeciBelInplace for ArrayBase<S, D>
where
    A: Float + MaybeNan,
    <A as MaybeNan>::NotNan: Ord,
    S: DataMut<Elem = A>,
    D: Dimension,
{
    type A = A;
    fn log_for_dB_inplace(&mut self, reference: DeciBelRef<A>, amin: A) {
        assert!(amin >= A::zero());
        let ref_value = reference.into_value(self.view());
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
        let factor = A::from(20.).unwrap();
        self.log_for_dB_inplace(reference, amin);
        self.mapv_inplace(|x| factor * x);
    }

    #[inline]
    fn dB_from_power_inplace(&mut self, reference: DeciBelRef<A>, amin: A) {
        let factor = A::from(10.).unwrap();
        self.log_for_dB_inplace(reference, amin);
        self.mapv_inplace(|x| factor * x);
    }

    #[inline]
    fn dB_from_amp_inplace_default(&mut self) {
        self.dB_from_amp_inplace(Default::default(), A::from(AMIN_AMP_DEFAULT).unwrap());
    }

    #[inline]
    fn dB_from_power_inplace_default(&mut self) {
        self.dB_from_power_inplace(Default::default(), A::from(AMIN_POWER_DEFAULT).unwrap());
    }

    #[inline]
    fn amp_from_dB_inplace(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * A::from(10.).unwrap().powf(A::from(0.05).unwrap() * x));
    }

    #[inline]
    fn power_from_dB_inplace(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * A::from(10.).unwrap().powf(A::from(0.1).unwrap() * x));
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
