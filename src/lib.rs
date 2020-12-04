use ndarray::prelude::*;
use ndarray::{stack, RemoveAxis, ScalarOperand, Slice};
use rayon::prelude::*;
use rustfft::num_complex::Complex;
use rustfft::num_traits::identities::*;
use rustfft::num_traits::{Float, Num};
use rustfft::FFTnum;
use std::ops::*;
use wasm_bindgen::prelude::*;

mod audio;
mod decibel;
mod display;
mod mel;
mod realfft;
mod windows;
use decibel::DeciBelInplace;
use realfft::RealFFT;

trait Impulse {
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

pub fn rfft<A>(input: &Array1<A>) -> Array1<Complex<A>>
where
    A: FFTnum + Float,
{
    let n_fft = input.shape()[0];
    let mut r2c = RealFFT::<A>::new(n_fft).unwrap();
    let mut output = Array1::<Complex<A>>::zeros(n_fft / 2 + 1);
    r2c.process(&mut input.to_vec(), output.as_slice_mut().unwrap())
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
            stack![axis, pad_left.view(), array, pad_right.view()]
        }
        PadMode::Reflect => {
            let s_left_reflect = Slice::new(1, Some(n_pad_left as isize + 1), -1);
            let s_right_reflect = Slice::new(-(n_pad_right as isize + 1), Some(-1), -1);
            let pad_left = array.slice_axis(axis, s_left_reflect);
            let pad_right = array.slice_axis(axis, s_right_reflect);
            stack![axis, pad_left, array, pad_right]
        }
    }
}

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
pub fn get_spectrogram(path: &str) -> Vec<u8> {
    let (wav, sr) = audio::open_audio_file(path).unwrap();
    let wav = wav.sum_axis(Axis(0));
    let spec = stft(wav.view(), 1920, 480, false);
    let mag = spec.mapv(|x| x.norm());
    let mut melspec = mag.dot(&mel::mel_filterbanks(sr, 2048, 128, 0f32, None));
    melspec.amp_to_db_default();
    let im = display::spec_to_image(&melspec, 1200, 800);
    im.into_raw()
}

#[cfg(test)]
mod tests {
    use crate::decibel::DeciBelInplace;
    use crate::*;

    use ndarray::{arr1, arr2, Array1};
    use rustfft::num_complex::Complex;
    use std::time;

    // Compare RealToComplex with standard FFT
    #[test]
    fn impulse_works() {
        assert_eq!(Array1::<f32>::impulse(4, 0), arr1(&[1., 0., 0., 0.]));
    }

    #[test]
    fn rfft_wrapper_works() {
        assert_eq!(
            rfft(&Array1::<f32>::impulse(4, 0)),
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

    #[test]
    fn stft_time() {
        let (wav, sr) = audio::open_audio_file("samples/sample.wav").unwrap();
        let wav = wav.sum_axis(Axis(0));
        // let wavs = stack![Axis(0), wav, wav, wav, wav, wav, wav];
        let n_experiments = 10;
        let mut sum_time = 0u128;
        let wav_length = wav.len() as f32 * 1000. / sr as f32;
        for _ in 0..n_experiments {
            let now = time::Instant::now();
            // par_azip!((wav in wavs.axis_iter(Axis(0))), {stft(wav.view(), 1920, 480)});
            let spec = stft(wav.view(), 1920, 480, false);
            let mut mag = spec.mapv(|x| x.norm());
            let mut melspec = mag.dot(&mel::mel_filterbanks(sr, 2048, 128, 0f32, None));
            melspec.amp_to_db_default();
            let im = display::spec_to_image(&melspec, 1200, 800);
            im.save("spec.png").unwrap();

            let time = now.elapsed().as_millis();
            println!("{}", time as f32 / wav_length);
            sum_time += time;
        }
        let mean_time = sum_time as f32 / n_experiments as f32;
        println!();
        println!("{} RT", mean_time / wav_length);
    }
}
