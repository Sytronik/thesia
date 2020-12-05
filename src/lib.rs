use std::ops::*;

use ndarray::{prelude::*, ScalarOperand};
use rayon::prelude::*;
use rustfft::{num_complex::Complex, num_traits::Float, FFTnum};
use wasm_bindgen::prelude::*;

pub mod audio;
pub mod decibel;
pub mod display;
pub mod mel;
pub mod realfft;
pub mod utils;
pub mod windows;
use decibel::DeciBelInplace;
use realfft::RealFFT;
use utils::{pad, PadMode};

pub fn stft<A>(
    input: ArrayView1<A>,
    win_length: usize,
    hop_length: usize,
    parallel: bool,
) -> Array2<Complex<A>>
where
    A: FFTnum + Float + MulAssign + ScalarOperand,
{
    let n_fft = 2usize.pow((win_length as f32).log2().ceil() as u32);
    let n_frames = (input.len() - win_length) / hop_length + 1;
    let n_pad_left = (n_fft - win_length) / 2;
    let n_pad_right = (((n_fft - win_length) as f32) / 2.).ceil() as usize;

    let mut window = windows::hann(win_length, false);
    // window *= A::from(1024 / win_length).unwrap();
    let mut frames: Vec<Array1<A>> = input
        .windows(win_length)
        .into_iter()
        .step_by(hop_length)
        .map(|x| {
            pad(
                (&x * &window).view(),
                (n_pad_left, n_pad_right),
                Axis(0),
                PadMode::Constant(A::zero()),
            )
        })
        .collect();

    let mut spec = Array2::<Complex<A>>::zeros((n_frames, n_fft / 2 + 1));
    let spec_view_mut: Vec<&mut [Complex<A>]> = spec
        .axis_iter_mut(Axis(0))
        .map(|x| x.into_slice().unwrap())
        .collect();

    if parallel {
        frames.par_iter_mut().zip(spec_view_mut).for_each(|(x, y)| {
            let mut r2c = RealFFT::<A>::new(n_fft).unwrap();
            let x = x.as_slice_mut().unwrap();
            r2c.process(x, y).unwrap();
        });
    } else {
        let mut r2c = RealFFT::<A>::new(n_fft).unwrap();
        frames.iter_mut().zip(spec_view_mut).for_each(|(x, y)| {
            let x = x.as_slice_mut().unwrap();
            r2c.process(x, y).unwrap();
        });
    }

    spec
}

#[wasm_bindgen]
pub fn get_spectrogram(path: &str, px_per_sec: f32, nheight: u32) -> Vec<u8> {
    let (wav, sr) = audio::open_audio_file(path).unwrap();
    let wav = wav.sum_axis(Axis(0));
    let nwidth = (px_per_sec * wav.len() as f32 / sr as f32) as u32;
    let spec = stft(wav.view(), 1920, 480, false);
    let mag = spec.mapv(|x| x.norm());
    let mut melspec = mag.dot(&mel::mel_filterbanks(sr, 2048, 128, 0f32, None));
    melspec.amp_to_db_default();
    let im = display::spec_to_image(melspec.view(), nwidth, nheight);
    im.into_raw()
}

#[cfg(test)]
mod tests {
    use ndarray::{arr2, Array1};
    use rustfft::num_complex::Complex;

    use super::utils::Impulse;
    use super::*;

    #[test]
    fn stft_works() {
        assert_eq!(
            stft(Array1::<f32>::impulse(4, 2).view(), 4, 2, false),
            arr2(&[[
                Complex::<f32>::new(1., 0.),
                Complex::<f32>::new(-1., 0.),
                Complex::<f32>::new(1., 0.)
            ]])
        );
    }
}
