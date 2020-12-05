use std::f64::consts::PI as PIf64;
use std::iter::FromIterator;

use ndarray::prelude::*;
use rustfft::num_traits::Float;

fn cosine_window<T: Float>(a: T, b: T, c: T, d: T, size: usize, symmetric: bool) -> Array1<T> {
    assert!(size > 1);
    let pi = T::from(PIf64).unwrap();
    let size2 = if symmetric { size } else { size + 1 };
    let cos_fn = |i| {
        let x = pi * T::from(i).unwrap() / T::from(size2 - 1).unwrap();
        let b_ = b * (T::from(2.).unwrap() * x).cos();
        let c_ = c * (T::from(4.).unwrap() * x).cos();
        let d_ = d * (T::from(6.).unwrap() * x).cos();
        (a - b_) + (c_ - d_)
    };
    Array::from_iter((0..size2).map(cos_fn).take(size))
}

pub fn hann<T: Float>(size: usize, symmetric: bool) -> Array1<T> {
    cosine_window(
        T::from(0.5).unwrap(),
        T::from(0.5).unwrap(),
        T::zero(),
        T::zero(),
        size,
        symmetric,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hann_window_works() {
        assert_eq!(hann::<f32>(4, false), arr1(&[0., 0.5, 1., 0.5]));
    }
}
