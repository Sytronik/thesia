use ndarray::ScalarOperand;
use ndarray::prelude::*;
use num_traits::{AsPrimitive, Float, FloatConst, NumOps};

pub enum WindowType {
    Hann,
    Blackman,
    _BoxCar,
}

#[inline]
pub fn calc_normalized_win<A>(
    win_type: WindowType,
    size: usize,
    norm_factor: impl AsPrimitive<A>,
) -> Array1<A>
where
    A: Float + FloatConst + NumOps + ScalarOperand,
    f32: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    let norm_factor = norm_factor.as_();
    match win_type {
        WindowType::Hann => hann(size, false) / norm_factor,
        WindowType::Blackman => blackman(size, false) / norm_factor,
        WindowType::_BoxCar => Array1::from_elem(size, norm_factor.recip()),
    }
}

#[inline]
pub fn hann<A>(size: usize, symmetric: bool) -> Array1<A>
where
    A: Float + FloatConst + 'static,
    f32: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    cosine_window(0.5.as_(), 0.5.as_(), A::zero(), A::zero(), size, symmetric)
}

// from rubato crate
pub fn blackman<A>(size: usize, symmetric: bool) -> Array1<A>
where
    A: Float + FloatConst + 'static,
    f32: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    debug_assert!(size > 1);
    let size2 = if symmetric { size + 1 } else { size };
    let pi2 = 2.as_() * A::PI();
    let pi4 = 4.as_() * A::PI();
    let np_f = (size2).as_();
    let a = (0.42).as_();
    let b = (0.5).as_();
    let c = (0.08).as_();
    (0..size2)
        .map(|x| {
            let x_float = x.as_();
            a - b.mul_add(
                (pi2 * x_float / np_f).cos(),
                c * (pi4 * x_float / np_f).cos(),
            )
        })
        .skip(if symmetric { 1 } else { 0 })
        .collect()
}

#[allow(clippy::many_single_char_names)]
fn cosine_window<A>(a: A, b: A, c: A, d: A, size: usize, symmetric: bool) -> Array1<A>
where
    A: Float + FloatConst + 'static,
    usize: AsPrimitive<A>,
{
    debug_assert!(size > 1);
    let size2 = if symmetric { size } else { size + 1 };
    let cos_fn = |i: usize| {
        let x = A::PI() * i.as_() / (size2 - 1).as_();
        let b_ = b * (2.as_() * x).cos();
        let c_ = c * (4.as_() * x).cos();
        let d_ = d * (6.as_() * x).cos();
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
