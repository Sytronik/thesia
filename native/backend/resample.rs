// from rubato crate
use std::ops::{AddAssign, Div, DivAssign, MulAssign};
use std::sync::Arc;

use ndarray::{prelude::*, ScalarOperand};
use ndarray_stats::QuantileExt;
use realfft::{num_complex::Complex, ComplexToReal, FftNum, RealFftPlanner, RealToComplex};
use rustfft::num_traits::{Float, FloatConst, FromPrimitive, NumAssign};

use crate::backend::utils::{Pad, PadMode};

use super::sinc::calc_windowed_sincs;
use super::windows::WindowType;

#[derive(Clone)]
pub struct FftResampler<T> {
    input_size: usize,
    output_size: usize,
    latency: usize,
    fft: Arc<dyn RealToComplex<T>>,
    ifft: Arc<dyn ComplexToReal<T>>,
    filter_f: Array1<Complex<T>>,
    scratch_fw: Vec<Complex<T>>,
    scratch_inv: Vec<Complex<T>>,
    input_buf: Array1<T>,
    input_f: Array1<Complex<T>>,
    output_f: Array1<Complex<T>>,
    output_buf: Array1<T>,
}

impl<T> FftResampler<T>
where
    T: Float
        + FloatConst
        + FromPrimitive
        + ScalarOperand
        + FftNum
        + AddAssign
        + DivAssign
        + MulAssign
        + Div
        + NumAssign,
{
    pub fn new(input_size: usize, output_size: usize) -> Self {
        // calculate antialiasing cutoff
        let cutoff = if input_size > output_size {
            0.4f32.powf(16.0 / input_size as f32) * output_size as f32 / input_size as f32
        } else {
            0.4f32.powf(16.0 / input_size as f32)
        };

        let sinc = calc_windowed_sincs::<T>(input_size, 1, cutoff, WindowType::Blackman)
            .index_axis_move(Axis(0), 0);
        let latency =
            ((sinc.argmax().unwrap() * output_size) as f32 / input_size as f32).round() as usize;

        let mut planner = RealFftPlanner::<T>::new();
        let fft = planner.plan_fft_forward(2 * input_size);
        let ifft = planner.plan_fft_inverse(2 * output_size);

        let mut filter_t = {
            let x = &sinc / T::from(2 * input_size).unwrap();
            x.pad((0, input_size), Axis(0), PadMode::Constant(T::zero()))
        };
        let mut filter_f = Array1::zeros(input_size + 1);
        fft.process(
            filter_t.as_slice_mut().unwrap(),
            filter_f.as_slice_mut().unwrap(),
        )
        .unwrap();

        let scratch_fw = fft.make_scratch_vec();
        let scratch_inv = ifft.make_scratch_vec();
        let input_buf = Array1::zeros(2 * input_size);
        let input_f = Array1::zeros(input_size + 1);
        let output_f = Array1::zeros(output_size + 1);
        let output_buf = Array1::zeros(2 * output_size);

        FftResampler {
            input_size,
            output_size,
            latency,
            fft,
            ifft,
            filter_f,
            scratch_fw,
            scratch_inv,
            input_buf,
            input_f,
            output_f,
            output_buf,
        }
    }

    pub fn resample(&mut self, wav_in: ArrayView1<T>) -> ArrayView1<T> {
        assert_eq!(wav_in.len(), self.input_size);
        // Copy to input buffer and clear padding area
        self.input_buf
            .slice_mut(s![..self.input_size])
            .assign(&wav_in);

        // FFT and store result in history, update index
        self.fft
            .process_with_scratch(
                self.input_buf.as_slice_mut().unwrap(),
                self.input_f.as_slice_mut().unwrap(),
                &mut self.scratch_fw,
            )
            .unwrap();

        // multiply with filter FT
        self.input_f *= &self.filter_f;
        let new_len = if self.input_size < self.output_size {
            self.input_size + 1
        } else {
            self.output_size
        };

        // copy to modified spectrum
        self.output_f
            .slice_mut(s![..new_len])
            .assign(&self.input_f.slice(s![..new_len]));

        // IFFT result, store result and overlap
        self.ifft
            .process_with_scratch(
                self.output_f.as_slice_mut().unwrap(),
                self.output_buf.as_slice_mut().unwrap(),
                &mut self.scratch_inv,
            )
            .unwrap();
        self.output_buf
            .slice(s![self.latency..(self.latency + self.output_size)])
    }
}

#[cfg(test)]
mod tests {
    use ndarray_stats::QuantileExt;

    use super::*;

    #[test]
    fn resample_works() {
        let mut resampler = FftResampler::<f64>::new(147, 1000);
        let mut wav_in = Array1::zeros(147);

        wav_in[1] = 0.3;
        wav_in[2] = 0.7;
        wav_in[3] = 1.0;
        wav_in[4] = 0.7;
        wav_in[5] = 0.3;

        let wav_out = resampler.resample(wav_in.view());
        assert_eq!(
            wav_out.argmax().unwrap(),
            (3. * 1000. / 147.).round() as usize
        );
        assert!((wav_out.max().unwrap() - 1.0).abs() < 0.1);
        // dbg!(wav_out.slice(s![..41]));
    }
}
