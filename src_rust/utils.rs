use std::collections::HashMap;
use std::hash::Hash;

use ndarray::{concatenate, prelude::*, Data, RemoveAxis, Slice};
use rayon::prelude::*;
use rustfft::{
    num_complex::Complex,
    num_traits::{
        identities::{One, Zero},
        Float, Num,
    },
    FFTnum,
};

use crate::realfft::RealFFT;

pub fn calc_proper_n_fft(win_length: usize) -> usize {
    2usize.pow((win_length as f32).log2().ceil() as u32)
}

pub trait Impulse {
    fn impulse(size: usize, location: usize) -> Self;
}

impl<A> Impulse for Array1<A>
where
    A: Clone + Zero + One,
{
    fn impulse(size: usize, location: usize) -> Self {
        let mut new = Array1::<A>::zeros((size,));
        new[location] = A::one();
        new
    }
}

pub fn rfft<A, S, D>(input: ArrayBase<S, D>) -> Array1<Complex<A>>
where
    A: FFTnum + Float,
    S: Data<Elem = A>,
    D: Dimension,
{
    let n_fft = input.shape()[0];
    let mut r2c = RealFFT::<A>::new(n_fft).unwrap();
    let mut output = Array1::<Complex<A>>::zeros(n_fft / 2 + 1);
    r2c.process(
        input.into_owned().as_slice_mut().unwrap(),
        output.as_slice_mut().unwrap(),
    )
    .unwrap();

    output
}

pub enum PadMode<T> {
    Constant(T),
    Reflect,
}

pub fn pad<A, D>(
    array: ArrayView<A, D>,
    (n_pad_left, n_pad_right): (usize, usize),
    axis: Axis,
    mode: PadMode<A>,
) -> Array<A, D>
where
    A: Copy + Num,
    D: Dimension + RemoveAxis,
{
    match mode {
        PadMode::Constant(constant) => {
            let mut shape_left = array.raw_dim();
            let mut shape_right = array.raw_dim();
            shape_left[axis.index()] = n_pad_left;
            shape_right[axis.index()] = n_pad_right;
            let pad_left = Array::from_elem(shape_left, constant);
            let pad_right = Array::from_elem(shape_right, constant);
            concatenate![axis, pad_left.view(), array, pad_right.view()]
        }
        PadMode::Reflect => {
            let s_left_reflect = Slice::new(1, Some(n_pad_left as isize + 1), -1);
            let s_right_reflect = Slice::new(-(n_pad_right as isize + 1), Some(-1), -1);
            let pad_left = array.slice_axis(axis, s_left_reflect);
            let pad_right = array.slice_axis(axis, s_right_reflect);
            concatenate![axis, pad_left, array, pad_right]
        }
    }
}

pub fn par_collect_to_hashmap<A, K, V>(par_map: A) -> HashMap<K, V>
where
    A: ParallelIterator<Item = (K, V)>,
    K: Send + Eq + Hash,
    V: Send,
{
    let identity_spec_greys = || HashMap::<K, V>::new();
    par_map
        .fold(identity_spec_greys, |mut hm, (k, v)| {
            hm.insert(k, v);
            hm
        })
        .reduce(identity_spec_greys, |mut hm1, hm2| {
            hm1.extend(hm2);
            hm1
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    use ndarray::{arr1, arr2, Array1};
    use rustfft::num_complex::Complex;

    #[test]
    fn rfft_wrapper_works() {
        assert_eq!(
            rfft(Array1::<f32>::impulse(4, 0)),
            arr1(&[Complex::<f32>::new(1., 0.); 3])
        );
    }

    #[test]
    fn pad_works() {
        assert_eq!(
            pad(
                arr2(&[[1, 2, 3]]).view(),
                (1, 2),
                Axis(0),
                PadMode::Constant(10)
            ),
            arr2(&[[10, 10, 10], [1, 2, 3], [10, 10, 10], [10, 10, 10]])
        );
        assert_eq!(
            pad(arr2(&[[1, 2, 3]]).view(), (1, 2), Axis(1), PadMode::Reflect),
            arr2(&[[2, 1, 2, 3, 2, 1]])
        );
    }
}
