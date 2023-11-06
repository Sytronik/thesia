// https://librosa.org/doc/0.8.0/_modules/librosa/filters.html#mel

use std::ops::*;

use ndarray::{prelude::*, ScalarOperand, Zip};
use rustfft::num_traits::Float;

#[allow(clippy::excessive_precision)]
pub const MEL_DIFF_2K_1K: f32 = 10.081880157308321; // from_hz(2000) - from_hz(1000)
pub const MIN_LOG_MEL: usize = 15;
const MIN_LOG_HZ: f64 = 1000.;
const LOGSTEP: f64 = 0.06875177742094912; // 6.4.ln() / 27.
const LINEARSCALE: f64 = 200. / 3.;

#[inline]
pub fn to_hz<A: Float>(mel: A) -> A {
    let min_log_mel = A::from(MIN_LOG_MEL).unwrap();
    if mel < min_log_mel {
        A::from(LINEARSCALE).unwrap() * mel
    } else {
        A::from(MIN_LOG_HZ).unwrap() * (A::from(LOGSTEP).unwrap() * (mel - min_log_mel)).exp()
    }
}

#[inline]
pub fn from_hz<A: Float>(freq: A) -> A {
    let min_log_hz = A::from(MIN_LOG_HZ).unwrap();
    if freq < min_log_hz {
        freq / A::from(LINEARSCALE).unwrap()
    } else {
        A::from(MIN_LOG_MEL).unwrap() + (freq / min_log_hz).ln() / A::from(LOGSTEP).unwrap()
    }
}

/// Returns size (n_fft / 2 + 1, n_mel) array
pub fn calc_mel_fb<A>(
    sr: u32,
    n_fft: usize,
    n_mel: usize,
    fmin: A,
    fmax: Option<A>,
    do_norm: bool,
) -> Array2<A>
where
    A: Float
        + ScalarOperand
        + AddAssign
        + SubAssign
        + MulAssign
        + DivAssign
        + Sync
        + Send
        + std::fmt::Debug,
{
    assert_eq!(n_fft % 2, 0);
    assert_ne!(n_mel, 0);
    let f_nyquist = A::from((sr as f32) / 2.).unwrap();
    let fmax = if let Some(f) = fmax { f } else { f_nyquist };
    let n_freq = n_fft / 2 + 1;

    let linear_freqs = Array::linspace(A::zero(), f_nyquist, n_freq);
    let mut mel_freqs = Array::linspace(from_hz(fmin), from_hz(fmax), n_mel + 2);
    mel_freqs.par_mapv_inplace(to_hz);

    let mut weights = Array2::<A>::zeros((n_freq, n_mel));
    Zip::indexed(weights.axis_iter_mut(Axis(1))).par_for_each(|i_m, mut w| {
        for (i_f, &f) in linear_freqs.indexed_iter() {
            if f <= mel_freqs[i_m] {
                continue;
            } else if mel_freqs[i_m] < f && f < mel_freqs[i_m + 1] {
                w[i_f] = (f - mel_freqs[i_m]) / (mel_freqs[i_m + 1] - mel_freqs[i_m]);
            } else if f == mel_freqs[i_m + 1] {
                w[i_f] = A::one();
            } else if mel_freqs[i_m + 1] < f && f < mel_freqs[i_m + 2] {
                w[i_f] = (mel_freqs[i_m + 2] - f) / (mel_freqs[i_m + 2] - mel_freqs[i_m + 1]);
            } else {
                break;
            }
        }
        if do_norm {
            w /= w.sum().max(A::epsilon());
        }
    });
    weights
}

pub fn calc_mel_fb_default(sr: u32, n_fft: usize) -> Array2<f32> {
    let mut n_mel =
        (2. * from_hz(sr as f32 / 2.) / from_hz(sr as f32 / n_fft as f32) - 1.) as usize;
    n_mel = n_mel.min(n_fft / 2 + 1);

    loop {
        let mel_fb = calc_mel_fb(sr, n_fft, n_mel, 0f32, None, true);
        if mel_fb.sum_axis(Axis(0)).iter().all(|&x| x > 0.) {
            break mel_fb;
        }
        n_mel -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use approx::assert_abs_diff_eq;

    #[test]
    fn mel_hz_convert() {
        assert_abs_diff_eq!(from_hz(100.), 1.5, epsilon = 1e-14);
        assert_abs_diff_eq!(from_hz(1100.), 16.38629404765444, epsilon = 1e-14);
        assert_abs_diff_eq!(to_hz(1.), 66.66666666666667, epsilon = 1e-14);
        assert_abs_diff_eq!(to_hz(16.), 1071.1702874944676, epsilon = 1e-14);
    }

    #[test]
    fn mel_works() {
        let (sr, n_fft, n_mel) = (24000, 2048, 80);
        let mel0_answer = [
            0.0f64,
            0.07852016499598029,
            0.15704032999196058,
            0.23556049498794085,
            0.25,
            0.17147983500401973,
            0.09295967000803942,
            0.014439505012059144,
            0.0,
        ];
        let mel_fb = calc_mel_fb(sr, n_fft, n_mel, 0f64, None, true);
        let mel_fb = mel_fb.t();
        assert_eq!(mel_fb.shape(), &[n_mel, n_fft / 2 + 1]);

        let mel0_answer_iter = mel0_answer
            .into_iter()
            .chain(std::iter::repeat(0.).take(mel_fb.shape()[1] - mel0_answer.len()));
        mel_fb
            .iter()
            .zip(mel0_answer_iter)
            .for_each(|(&x, y)| assert_abs_diff_eq!(x, y, epsilon = 1e-8));
    }

    #[test]
    fn mel_default_works() {
        for &sr in &[
            400, 800, 1000, 2000, 4000, 8000, 16000, 24000, 44100, 48000, 88200, 96000,
        ] {
            for n_fft_exp in 5..15 {
                let n_fft = 2usize.pow(n_fft_exp);
                let mel_fb = calc_mel_fb_default(sr, n_fft);
                assert!(
                    mel_fb.sum_axis(Axis(0)).iter().all(|&x| x > 0.),
                    "Empty mel filterbanks were found!\nsr: {}, n_fft: {}, n_mel: {}",
                    sr,
                    n_fft,
                    mel_fb.shape()[1]
                );
                if mel_fb.shape()[1] == mel_fb.shape()[0] {
                    continue;
                }
                let mel_fb_fail = calc_mel_fb(sr, n_fft, mel_fb.shape()[1] + 1, 0f32, None, true);
                assert!(
                    mel_fb_fail.sum_axis(Axis(0)).iter().any(|&x| x == 0.),
                    "More mel filterbanks are okay!\nsr: {}, n_fft: {}, n_mel: {}",
                    sr,
                    n_fft,
                    mel_fb_fail.shape()[1]
                );
            }
        }
    }
}
