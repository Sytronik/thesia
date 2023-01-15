use std::mem::MaybeUninit;
use std::ops::{AddAssign, DivAssign};

use ndarray::{prelude::*, ScalarOperand};
use rustfft::num_traits::{Float, FloatConst, FromPrimitive};

use super::windows::{calc_normalized_win, WindowType};

/// Helper function: sinc(x) = sin(pi*x)/(pi*x)
pub fn sinc<T>(value: T) -> T
where
    T: Float + FloatConst,
{
    if value == T::zero() {
        T::one()
    } else {
        (value * T::PI()).sin() / (value * T::PI())
    }
}

/// Helper function. Make a set of windowed sincs.
pub fn calc_windowed_sincs<T>(
    npoints: usize,
    factor: usize,
    f_cutoff: f32,
    win_type: WindowType,
) -> Array2<T>
where
    T: Float + FloatConst + FromPrimitive + ScalarOperand + AddAssign + DivAssign,
{
    let totpoints = npoints * factor;
    let mut y = Vec::with_capacity(totpoints);
    let window: Array1<T> = calc_normalized_win(win_type, totpoints, 1);
    let mut sum = T::zero();
    for (x, w) in window.iter().enumerate().take(totpoints) {
        let val = *w
            * sinc(
                (T::from(x).unwrap() - T::from(totpoints / 2).unwrap())
                    * T::from(f_cutoff).unwrap()
                    / T::from(factor).unwrap(),
            );
        sum += val;
        y.push(val);
    }
    sum /= T::from(factor).unwrap();
    let mut sincs = Array2::uninit((factor, npoints));
    for p in 0..npoints {
        for n in 0..factor {
            sincs[[factor - n - 1, p]] = MaybeUninit::new(y[factor * p + n] / sum);
        }
    }
    unsafe { sincs.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sincs() {
        let sincs = calc_windowed_sincs::<f64>(32, 8, 0.9, WindowType::Blackman);
        assert!((sincs[[7, 16]] - 1.0).abs() < 0.2);
        let sum: f64 = sincs.sum();
        assert!((sum - 8.0).abs() < 0.00001);
    }
}
