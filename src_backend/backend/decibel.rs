// reference: https://librosa.org/doc/0.8.0/_modules/librosa/core/spectrum.html
#![allow(non_snake_case)]

use ndarray::prelude::*;
use ndarray::DataMut;
use ndarray_stats::{MaybeNan, QuantileExt};
use rustfft::num_traits::Float;

const AMIN_AMP_DEFAULT: f32 = 1e-18;
const AMIN_POWER_DEFAULT: f32 = 1e-36;

#[allow(dead_code)]
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
    fn into_value<D: Dimension>(&self, data_for_max: ArrayView<A, D>) -> A {
        match self {
            &DeciBelRef::Value(v) => {
                assert!(v >= A::zero());
                v
            }
            &DeciBelRef::Max => *data_for_max.max_skipnan(),
        }
    }
}

pub trait DeciBelInplace
where
    Self::A: Float,
{
    type A;
    fn into_log_for_dB(&mut self, reference: DeciBelRef<Self::A>, amin: Self::A);
    fn into_dB_from_amp(&mut self, reference: DeciBelRef<Self::A>, amin: Self::A);
    fn into_dB_from_power(&mut self, reference: DeciBelRef<Self::A>, amin: Self::A);
    fn into_dB_from_amp_default(&mut self);
    fn into_dB_from_power_default(&mut self);
    fn into_amp_from_dB(&mut self, ref_value: Self::A);
    fn into_power_from_dB(&mut self, ref_value: Self::A);
    fn into_amp_from_dB_default(&mut self);
    fn into_power_from_dB_default(&mut self);
}

impl<A, S, D> DeciBelInplace for ArrayBase<S, D>
where
    A: Float + MaybeNan,
    <A as MaybeNan>::NotNan: Ord,
    S: DataMut<Elem = A>,
    D: Dimension,
{
    type A = A;
    fn into_log_for_dB(&mut self, reference: DeciBelRef<A>, amin: A) {
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

    fn into_dB_from_power(&mut self, reference: DeciBelRef<A>, amin: A) {
        let factor = A::from(10.).unwrap();
        self.into_log_for_dB(reference, amin);
        self.mapv_inplace(|x| factor * x);
    }

    fn into_dB_from_amp(&mut self, reference: DeciBelRef<A>, amin: A) {
        let factor = A::from(20.).unwrap();
        self.into_log_for_dB(reference, amin);
        self.mapv_inplace(|x| factor * x);
    }

    #[inline]
    fn into_dB_from_amp_default(&mut self) {
        self.into_dB_from_amp(Default::default(), A::from(AMIN_AMP_DEFAULT).unwrap());
    }

    #[inline]
    fn into_dB_from_power_default(&mut self) {
        self.into_dB_from_power(Default::default(), A::from(AMIN_POWER_DEFAULT).unwrap());
    }

    fn into_amp_from_dB(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * A::from(10.).unwrap().powf(A::from(0.05).unwrap() * x));
    }

    fn into_power_from_dB(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * A::from(10.).unwrap().powf(A::from(0.1).unwrap() * x));
    }

    #[inline]
    fn into_amp_from_dB_default(&mut self) {
        if let DeciBelRef::Value(ref_value) = Default::default() {
            self.into_amp_from_dB(ref_value);
        } else {
            self.into_amp_from_dB(A::one());
        }
    }

    #[inline]
    fn into_power_from_dB_default(&mut self) {
        if let DeciBelRef::Value(ref_value) = Default::default() {
            self.into_power_from_dB(ref_value);
        } else {
            self.into_power_from_dB(A::one());
        }
    }
}
