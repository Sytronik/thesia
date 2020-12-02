// https://librosa.org/doc/0.8.0/_modules/librosa/filters.html#mel

use ndarray::prelude::*;
use ndarray::{azip, ScalarOperand};
use rustfft::num_traits::Float;
use std::ops::*;

const MIN_LOG_MEL: usize = 15;
const MIN_LOG_HZ: f64 = 1000.;
const LOGSTEP: f64 = 0.06875177742094912; // 6.4.ln() / 27.
const LINEARSCALE: f64 = 200. / 3.;

#[inline]
pub fn mel_to_hz<A: Float>(mel: A) -> A {
    let min_log_mel = A::from(MIN_LOG_MEL).unwrap();
    if mel < min_log_mel {
        A::from(LINEARSCALE).unwrap() * mel
    } else {
        A::from(MIN_LOG_HZ).unwrap() * (A::from(LOGSTEP).unwrap() * (mel - min_log_mel)).exp()
    }
}

#[inline]
pub fn hz_to_mel<A: Float>(freq: A) -> A {
    let min_log_hz = A::from(MIN_LOG_HZ).unwrap();
    if freq < min_log_hz {
        freq / A::from(LINEARSCALE).unwrap()
    } else {
        A::from(MIN_LOG_MEL).unwrap() + (freq / min_log_hz).ln() / A::from(LOGSTEP).unwrap()
    }
}

pub fn mel_filterbanks<A>(
    sr: u32,
    n_fft: usize,
    n_mel: usize,
    fmin: A,
    fmax: Option<A>,
) -> Array2<A>
where
    A: Float + ScalarOperand + AddAssign + Sub + SubAssign + MulAssign + DivAssign + Div, /* + std::fmt::Debug*/
{
    assert_eq!(n_fft % 2, 0);
    let fmax = match fmax {
        Some(f) => f,
        None => A::from((sr as f32) / 2.).unwrap(),
    };
    let norm = 1;
    let n_freq = n_fft / 2 + 1;
    let mut weights = Array2::<A>::zeros((n_freq, n_mel + 2));

    let min_mel = hz_to_mel(A::from(fmin).unwrap());
    let max_mel = hz_to_mel(A::from(fmax).unwrap());
    // println!("{:?}", min_mel);
    // println!("{:?}", max_mel);
    let mut mel_f = Array::linspace(min_mel, max_mel, n_mel + 2);
    mel_f.mapv_inplace(mel_to_hz);
    let fdiff = &mel_f.slice(s![1..]) - &mel_f.slice(s![0..-1]);
    weights -= &Array::linspace(A::zero(), A::from((sr as f32) / 2.).unwrap(), n_freq)
        .into_shape((n_freq, 1))
        .unwrap();
    weights += &mel_f;

    // println!("{:?}", weights);
    // println!("{:?}", mel_f);

    for i_mel in 0..n_mel {
        let mut upper = weights.index_axis(Axis(1), i_mel + 2).to_owned();
        upper /= fdiff[i_mel + 1];

        let mut w = weights.index_axis_mut(Axis(1), i_mel);
        w /= -fdiff[i_mel]; // lower
        azip!((x in &mut w, &u in &upper) {
            if *x > u {
                *x = u;
            }
            if *x <= A::zero() {
                *x = A::zero();
            }
        });
    }

    let mut weights = weights.slice_move(s![.., ..n_mel]);
    if norm == 1 {
        let mut enorm = &mel_f.slice(s![2..(n_mel + 2)]) - &mel_f.slice(s![..n_mel]);
        enorm.mapv_inplace(|x| A::from(2.).unwrap() / x);
        // println!("{:?}", enorm);
        weights *= &enorm;
    }
    weights
}
