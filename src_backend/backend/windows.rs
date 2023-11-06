use std::ops::Div;

use ndarray::{prelude::*, ScalarOperand};
use rustfft::num_traits::{Float, FloatConst, FromPrimitive, ToPrimitive};

pub enum WindowType {
    Hann,
    Blackman,
    BoxCar,
}

#[inline]
pub fn calc_normalized_win<T>(
    win_type: WindowType,
    size: usize,
    norm_factor: impl ToPrimitive,
) -> Array1<T>
where
    T: Float + FloatConst + FromPrimitive + Div + ScalarOperand,
{
    let norm_factor = T::from(norm_factor).unwrap();
    match win_type {
        WindowType::Hann => hann(size, false) / norm_factor,
        WindowType::Blackman => blackman(size, false) / norm_factor,
        WindowType::BoxCar => Array1::from_elem(size, T::one() / norm_factor),
    }
}

#[inline]
pub fn hann<T>(size: usize, symmetric: bool) -> Array1<T>
where
    T: Float + FloatConst + FromPrimitive,
{
    cosine_window(
        T::from(0.5).unwrap(),
        T::from(0.5).unwrap(),
        T::zero(),
        T::zero(),
        size,
        symmetric,
    )
}

// from rubato crate
pub fn blackman<T>(size: usize, symmetric: bool) -> Array1<T>
where
    T: Float + FloatConst + FromPrimitive,
{
    assert!(size > 1);
    let size2 = if symmetric { size + 1 } else { size };
    let pi2 = T::from_u8(2).unwrap() * T::PI();
    let pi4 = T::from_u8(4).unwrap() * T::PI();
    let np_f = T::from(size2).unwrap();
    let a = T::from(0.42).unwrap();
    let b = T::from(0.5).unwrap();
    let c = T::from(0.08).unwrap();
    (0..size2)
        .map(|x| {
            let x_float = T::from_usize(x).unwrap();
            a - b * (pi2 * x_float / np_f).cos() + c * (pi4 * x_float / np_f).cos()
        })
        .skip(if symmetric { 1 } else { 0 })
        .collect()
}

#[allow(clippy::many_single_char_names)]
fn cosine_window<T>(a: T, b: T, c: T, d: T, size: usize, symmetric: bool) -> Array1<T>
where
    T: Float + FloatConst + FromPrimitive,
{
    assert!(size > 1);
    let size2 = if symmetric { size } else { size + 1 };
    let cos_fn = |i| {
        let x = T::PI() * T::from_usize(i).unwrap() / T::from_usize(size2 - 1).unwrap();
        let b_ = b * (T::from_u8(2).unwrap() * x).cos();
        let c_ = c * (T::from_u8(4).unwrap() * x).cos();
        let d_ = d * (T::from_u8(6).unwrap() * x).cos();
        (a - b_) + (c_ - d_)
    };
    (0..size2).map(cos_fn).take(size).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hann_window_works() {
        assert_eq!(hann::<f32>(4, false), arr1(&[0f32, 0.5, 1., 0.5]));
    }
}
