// reference: https://librosa.org/doc/0.8.0/_modules/librosa/core/spectrum.html
use ndarray::{prelude::*, DataMut};
use ndarray_stats::{MaybeNan, QuantileExt};
use rustfft::num_traits::Float;

const REF_DEFAULT: f32 = 1.0;
const AMIN_AMP_DEFAULT: f32 = 1e-18;
const AMIN_POWER_DEFAULT: f32 = 1e-36;

#[allow(dead_code)]
pub enum DeciBelRef<A: Float> {
    Value(A),
    Max,
}

pub trait DeciBelInplace<A: Float> {
    fn log_for_db(&mut self, reference: DeciBelRef<A>, amin: A);
    fn amp_to_db(&mut self, reference: DeciBelRef<A>, amin: A);
    fn power_to_db(&mut self, reference: DeciBelRef<A>, amin: A);
    fn amp_to_db_default(&mut self);
    fn power_to_db_default(&mut self);
    fn db_to_amp(&mut self, ref_value: A);
    fn db_to_power(&mut self, ref_value: A);
    fn db_to_amp_default(&mut self);
    fn db_to_power_default(&mut self);
}

impl<A, S, D> DeciBelInplace<A> for ArrayBase<S, D>
where
    A: Float + MaybeNan,
    <A as MaybeNan>::NotNan: Ord,
    S: DataMut<Elem = A>,
    D: Dimension,
{
    fn log_for_db(&mut self, reference: DeciBelRef<A>, amin: A) {
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

    fn power_to_db(&mut self, reference: DeciBelRef<A>, amin: A) {
        let factor = A::from(10.).unwrap();
        self.log_for_db(reference, amin);
        self.mapv_inplace(|x| factor * x);
    }

    fn amp_to_db(&mut self, reference: DeciBelRef<A>, amin: A) {
        let factor = A::from(20.).unwrap();
        self.log_for_db(reference, amin);
        self.mapv_inplace(|x| factor * x);
    }

    #[inline]
    fn amp_to_db_default(&mut self) {
        self.amp_to_db(
            DeciBelRef::Value(A::from(REF_DEFAULT).unwrap()),
            A::from(AMIN_AMP_DEFAULT).unwrap(),
        );
    }

    #[inline]
    fn power_to_db_default(&mut self) {
        self.power_to_db(
            DeciBelRef::Value(A::from(REF_DEFAULT).unwrap()),
            A::from(AMIN_POWER_DEFAULT).unwrap(),
        );
    }

    fn db_to_amp(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * A::from(10.).unwrap().powf(A::from(0.05).unwrap() * x));
    }

    fn db_to_power(&mut self, ref_value: A) {
        self.mapv_inplace(|x| ref_value * A::from(10.).unwrap().powf(A::from(0.1).unwrap() * x));
    }

    #[inline]
    fn db_to_amp_default(&mut self) {
        self.db_to_amp(A::from(REF_DEFAULT).unwrap());
    }

    #[inline]
    fn db_to_power_default(&mut self) {
        self.db_to_power(A::from(REF_DEFAULT).unwrap());
    }
}
