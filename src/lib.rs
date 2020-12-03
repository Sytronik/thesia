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
    use crate::realfft::InvRealFFT;
    use crate::*;

    use ndarray::{arr1, arr2, par_azip, stack, Array1};
    use ndarray_stats::QuantileExt;
    use rustfft::num_complex::Complex;
    use rustfft::num_traits::Zero;
    use rustfft::FFTplanner;
    use std::path::Path;
    use std::time::Instant;

    fn compare_complex<T: Float>(a: &[Complex<T>], b: &[Complex<T>], tol: T) -> bool {
        a.iter().zip(b.iter()).fold(true, |eq, (val_a, val_b)| {
            eq && (val_a.re - val_b.re).abs() < tol && (val_a.im - val_b.im).abs() < tol
        })
    }

    fn compare_float<T: Float>(a: &[T], b: &[T], tol: T) -> bool {
        a.iter().zip(b.iter()).fold(true, |eq, (val_a, val_b)| {
            eq && (*val_a - *val_b).abs() < tol
        })
    }

    // Compare RealToComplex with standard FFT
    #[test]
    fn real_to_complex() {
        let mut indata = vec![0.0f64; 256];
        indata[0] = 1.0;
        indata[3] = 0.5;
        let mut indata_c = indata
            .iter()
            .map(|val| Complex::from(val))
            .collect::<Vec<Complex<f64>>>();
        let mut fft_planner = FFTplanner::<f64>::new(false);
        let fft = fft_planner.plan_fft(256);

        let mut r2c = RealFFT::<f64>::new(256).unwrap();
        let mut out_a = vec![Complex::<f64>::zero(); 129];
        let mut out_b = vec![Complex::<f64>::zero(); 256];

        fft.process(&mut indata_c, &mut out_b);
        r2c.process(&mut indata, &mut out_a).unwrap();
        assert!(compare_complex(&out_a[0..129], &out_b[0..129], 1.0e-9));
    }

    // Compare ComplexToReal with standard iFFT
    #[test]
    fn complex_to_real() {
        let mut indata = vec![Complex::<f64>::zero(); 256];
        indata[0] = Complex::new(1.0, 0.0);
        indata[1] = Complex::new(1.0, 0.4);
        indata[255] = Complex::new(1.0, -0.4);
        indata[3] = Complex::new(0.3, 0.2);
        indata[253] = Complex::new(0.3, -0.2);

        let mut fft_planner = FFTplanner::<f64>::new(true);
        let fft = fft_planner.plan_fft(256);

        let mut c2r = InvRealFFT::<f64>::new(256).unwrap();
        let mut out_a = vec![0f64; 256];
        let mut out_b = vec![Complex::<f64>::zero(); 256];

        c2r.process(&indata[0..129], &mut out_a).unwrap();
        fft.process(&mut indata, &mut out_b);

        let out_b_r: Vec<f64> = out_b.iter().map(|val| 0.5 * val.re).collect();
        assert!(compare_float(&out_a, &out_b_r, 1.0e-9));
    }

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
    fn hann_window_works() {
        assert_eq!(windows::hann::<f32>(4, false), arr1(&[0., 0.5, 1., 0.5]));
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
    fn open_audio_works() {
        let (wav, sr) = audio::open_audio_file(Path::new("samples/sample.wav")).unwrap();
        let arr = arr2(&[[
            -1.919269561767578125e-05,
            2.510547637939453125e-04,
            2.177953720092773438e-04,
            8.809566497802734375e-05,
            1.561641693115234375e-05,
            1.788139343261718750e-05,
            1.298189163208007812e-04,
            1.105070114135742188e-04,
            -1.615285873413085938e-04,
            -4.312992095947265625e-04,
            -4.181861877441406250e-04,
            -1.516342163085937500e-04,
            -3.480911254882812500e-05,
            -2.431869506835937500e-05,
            -1.041889190673828125e-04,
            -1.143217086791992188e-04,
        ]]);
        assert_eq!(sr, 48000);
        assert_eq!(wav.shape(), &[1, 320911]);
        assert!(compare_float(
            &[wav.max().unwrap().clone()],
            &[0.1715821],
            f32::EPSILON,
        ));
        assert!(compare_float(
            wav.as_slice().unwrap(),
            arr.as_slice().unwrap(),
            f32::EPSILON,
        ));
    }

    #[test]
    fn stft_time() {
        let (wav, sr) = audio::open_audio_file(Path::new("samples/sample.wav")).unwrap();
        let wav = wav.sum_axis(Axis(0));
        // let wavs = stack![Axis(0), wav, wav, wav, wav, wav, wav];
        let N = 10;
        let mut sum_time = 0u128;
        let wav_length = wav.len() as f32 * 1000. / sr as f32;
        for _ in 0..N {
            let now = Instant::now();
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
        let mean_time = sum_time as f32 / N as f32;
        println!();
        println!("{} RT", mean_time / wav_length);
    }

    #[test]
    fn mel_hz_convert() {
        println!("{:?}", mel::hz_to_mel(42.09));
        assert_eq!(mel::hz_to_mel(100.), 1.5);
        assert_eq!(mel::hz_to_mel(1100.), 16.38629404765444);
        assert_eq!(mel::mel_to_hz(1.), 66.66666666666667);
        assert_eq!(mel::mel_to_hz(16.), 1071.1702874944676);
    }

    #[test]
    fn mel_works() {
        let answer = [
            0.000000000000000000e+00f64,
            6.613916251808404922e-03,
            1.322783250361680984e-02,
            1.984174735844135284e-02,
            2.105801925063133240e-02,
            1.444410253316164017e-02,
            7.830185815691947937e-03,
            1.216269447468221188e-03,
        ];
        let melf = mel::mel_filterbanks(24000, 2048, 128, 0f64, None);
        // println!("{:?}", melf);
        assert!(compare_float(
            &melf.t().as_slice().unwrap(),
            &answer[..],
            1e-8
        ));
    }

    #[test]
    fn colorbar_works() {
        let im = display::colorbar(500);
        im.save("colorbar.png").unwrap();
    }
}
