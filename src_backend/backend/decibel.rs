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

pub trait DeciBelInplace<A: Float> {
    fn into_log_for_dB(&mut self, reference: DeciBelRef<A>, amin: A);
    fn into_dB_from_amp(&mut self, reference: DeciBelRef<A>, amin: A);
    fn into_dB_from_power(&mut self, reference: DeciBelRef<A>, amin: A);
    fn into_dB_from_amp_default(&mut self);
    fn into_dB_from_power_default(&mut self);
    fn into_amp_from_dB(&mut self, ref_value: A);
    fn into_power_from_dB(&mut self, ref_value: A);
    fn into_amp_from_dB_default(&mut self);
    fn into_power_from_dB_default(&mut self);
}

impl<A, S, D> DeciBelInplace<A> for ArrayBase<S, D>
where
    A: Float + MaybeNan,
    <A as MaybeNan>::NotNan: Ord,
    S: DataMut<Elem = A>,
    D: Dimension,
{
    fn into_log_for_dB(&mut self, reference: DeciBelRef<A>, amin: A) {
        assert!(self.iter().all(|&x| x >= A::zero()));
        assert!(amin >= A::zero());
        let ref_value = match reference {
            DeciBelRef::Value(v) => {
                assert!(v >= A::zero());
                v.abs()
            }
            DeciBelRef::Max => *self.view().max_skipnan(),
        };
        let log_amin = amin.log10();
        let log_ref = if ref_value > amin {
            ref_value.log10()
        } else {
            log_amin
        };
        self.mapv_inplace(|x| {
            if x > amin {
                x.log10() - log_ref
            } else {
                log_amin - log_ref
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
