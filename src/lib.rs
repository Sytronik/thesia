use ndarray::prelude::*;
use rustfft::num_traits::identities::*;
use rustfft::num_traits::Float;
use rustfft::num_complex::Complex;
use rustfft::FFTnum;
use apodize::hanning_iter;
use std::iter::FromIterator;

mod realfft;
use realfft::RealToComplex;

trait Impulse {
    fn impulse(size: usize, location: usize) -> Self;
}

impl<A> Impulse for Array1<A>
where
    A: Zero + Clone + One,
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
    let mut r2c = RealToComplex::<A>::new(n_fft).unwrap();
    let mut output_vec: Vec<Complex<A>> = vec![Complex::zero(); n_fft/2+1];
    r2c.process(&mut input.to_vec(), &mut output_vec).unwrap();

    Array1::<Complex<A>>::from(output_vec)
}

pub fn stft<A>(input: &Array1<A>, win_length: usize, hop_length: usize) -> Array2<Complex<A>>
where
    A: FFTnum + Float + std::fmt::Debug,
{
    let n_fft: usize = 2usize.pow((win_length as f32).log2().ceil() as u32);
    let pad_left = (n_fft - win_length)/2;
    let pad_right = (((n_fft - win_length) as f32)/2f32).ceil() as usize;
    let window = Array::from_iter(
        hanning_iter(win_length+1)
        .map(|x| A::from_f64(x).unwrap())
        .enumerate()
        .filter(|&(i, _)| i < win_length)
        .map(|(_, x)| x)
    );
    let mut r2c = RealToComplex::<A>::new(n_fft).unwrap();
    let mut spec = Array2::<Complex<A>>::zeros(
        (n_fft / 2 + 1, (input.len() - win_length)/hop_length + 1)
    );
    let spec_view_mut:Vec<&mut [Complex<A>]> 
        = spec.axis_iter_mut(Axis(1)).map(|x| x.into_slice().unwrap()).collect();

    let mut input: Vec<Vec<A>> = input.windows(win_length)
        .into_iter()
        .enumerate()
        .filter(|&(i, _)| i % hop_length == 0)
        .map(|(_, x)| {
            let mut left = vec![A::zero(); pad_left];
            left.extend((&x*&window).iter());
            let right = vec![A::zero(); pad_right];
            left.extend(&right);
            left
        }).collect();

    input.iter_mut()
        .zip(spec_view_mut)
        .for_each(|(x, y)| r2c.process(x, y).unwrap());

    spec
}

#[cfg(test)]
mod tests {
    use crate::*;
    use crate::realfft::{ComplexToReal, RealToComplex};
    use rustfft::num_complex::Complex;
    use rustfft::num_traits::Zero;
    use rustfft::FFTplanner;
    use ndarray::{Array1,arr2};

    fn compare_complex(a: &[Complex<f64>], b: &[Complex<f64>], tol: f64) -> bool {
        a.iter().zip(b.iter()).fold(true, |eq, (val_a, val_b)| {
            eq && (val_a.re - val_b.re).abs() < tol && (val_a.im - val_b.im).abs() < tol
        })
    }

    fn compare_f64(a: &[f64], b: &[f64], tol: f64) -> bool {
        a.iter()
            .zip(b.iter())
            .fold(true, |eq, (val_a, val_b)| eq && (val_a - val_b).abs() < tol)
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

        let mut r2c = RealToComplex::<f64>::new(256).unwrap();
        let mut out_a: Vec<Complex<f64>> = vec![Complex::zero(); 129];
        let mut out_b: Vec<Complex<f64>> = vec![Complex::zero(); 256];

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

        let mut c2r = ComplexToReal::<f64>::new(256).unwrap();
        let mut out_a: Vec<f64> = vec![0.0; 256];
        let mut out_b: Vec<Complex<f64>> = vec![Complex::zero(); 256];

        c2r.process(&indata[0..129], &mut out_a).unwrap();
        fft.process(&mut indata, &mut out_b);

        let out_b_r = out_b.iter().map(|val| 0.5 * val.re).collect::<Vec<f64>>();
        assert!(compare_f64(&out_a, &out_b_r, 1.0e-9));
    }

    #[test]
    fn impulse_works() {
        assert_eq!(
            Array1::<f32>::impulse(4, 0).to_vec(),
            vec![1., 0., 0., 0.]
        );
        assert_eq!(
            rfft(&Array1::<f32>::impulse(4, 0)).to_vec(),
            vec![Complex::<f32>::new(1., 0.); 3]
        );
    }

    #[test]
    fn stft_works() {
        assert_eq!(
            stft(&Array1::<f32>::impulse(4, 2), 4, 2),
            arr2(&[
                [Complex::<f32>::new(1., 0.)],
                [Complex::<f32>::new(-1., 0.)],
                [Complex::<f32>::new(1., 0.)]
            ])
        )
    }
}
