use ndarray::prelude::*;
use ndarray::{AssignElem, ScalarOperand};
use num_traits::{AsPrimitive, Float, FloatConst, MulAdd, NumAssignOps};

use super::windows::{WindowType, calc_normalized_win};

/// Helper function: sinc(x) = sin(pi*x)/(pi*x)
pub fn sinc<A>(value: A) -> A
where
    A: Float + FloatConst,
{
    if value.is_zero() {
        A::one()
    } else {
        (value * A::PI()).sin() / (value * A::PI())
    }
}

/// Helper function. Make a set of windowed sincs.
pub fn calc_windowed_sincs<A>(
    npoints: usize,
    factor: usize,
    f_cutoff: f32,
    win_type: WindowType,
) -> Array2<A>
where
    A: Float + FloatConst + ScalarOperand + NumAssignOps,
    f32: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    let totpoints = npoints * factor;
    let mut y = Vec::with_capacity(totpoints);
    let window: Array1<A> = calc_normalized_win(win_type, totpoints, 1);
    let mut sum = A::zero();
    for (x, w) in window.iter().enumerate().take(totpoints) {
        let val = *w * sinc((x.as_() - (totpoints / 2).as_()) * f_cutoff.as_() / factor.as_());
        sum += val;
        y.push(val);
    }
    sum /= factor.as_();
    let mut sincs = Array2::uninit((factor, npoints));
    for p in 0..npoints {
        for n in 0..factor {
            sincs[[factor - n - 1, p]].assign_elem(y[factor.mul_add(p, n)] / sum);
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
