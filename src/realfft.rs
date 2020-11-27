use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::{FFTnum,FFT};
use rustfft::algorithm::Radix4;
use std::error;
use std::fmt;
use rustfft::num_traits::Float;
use std::f64::consts::PI as PIf64;

type Res<T> = Result<T, Box<dyn error::Error>>;

/// Custom error returned by FFTs
#[derive(Debug)]
pub struct FftError {
    desc: String,
}

impl fmt::Display for FftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.desc)
    }
}

impl error::Error for FftError {
    fn description(&self) -> &str {
        &self.desc
    }
}

impl FftError {
    pub fn new(desc: &str) -> Self {
        FftError {
            desc: desc.to_owned(),
        }
    }
}

/// An FFT that takes a real-valued input vector of length 2*N and transforms it to a complex
/// spectrum of length N+1.
pub struct RealToComplex<T: FFTnum + Float> {
    sin_cos: Vec<(T, T)>,
    length: usize,
    fft: Radix4<T>,
    buffer_out: Vec<Complex<T>>,
}

/// An FFT that takes a real-valued input vector of length 2*N and transforms it to a complex
/// spectrum of length N+1.
pub struct ComplexToReal<T: FFTnum + Float> {
    sin_cos: Vec<(T, T)>,
    length: usize,
    fft: Radix4<T>,
    buffer_in: Vec<Complex<T>>,
}

fn zip4<A, B, C, D>(
    a: A,
    b: B,
    c: C,
    d: D,
) -> impl Iterator<Item = (A::Item, B::Item, C::Item, D::Item)>
where
    A: IntoIterator,
    B: IntoIterator,
    C: IntoIterator,
    D: IntoIterator,
{
    a.into_iter()
        .zip(b.into_iter().zip(c.into_iter().zip(d)))
        .map(|(w, (x, (y, z)))| (w, x, y, z))
}


impl<T> RealToComplex<T> where T: FFTnum + Float {
    /// Create a new RealToComplex FFT for input data of a given length. Returns an error if the length is not even.
    pub fn new(length: usize) -> Res<Self> {
        if length % 2 > 0 {
            return Err(Box::new(FftError::new("Length must be even")));
        }
        let buffer_out = vec![Complex::zero(); length / 2 + 1];
        let mut sin_cos = Vec::with_capacity(length / 2);
        let pi = T::from_f64(PIf64).unwrap();
        let halflength = T::from_usize(length / 2).unwrap();
        for k in 0..length / 2 {
            let k = T::from_usize(k).unwrap();
            let sin = (k * pi / halflength).sin();
            let cos = (k * pi / halflength).cos();
            sin_cos.push((sin, cos));
        }
        let fft = Radix4::<T>::new(length / 2, false);
        Ok(RealToComplex {
            sin_cos,
            length,
            fft,
            buffer_out,
        })
    }

    /// Transform a vector of 2*N real-valued samples, storing the result in the N+1 element long complex output vector.
    /// The input buffer is used as scratch space, so the contents of input should be considered garbage after calling.
    pub fn process(&mut self, input: &mut [T], output: &mut [Complex<T>]) -> Res<()> {
        if input.len() != self.length {
            return Err(Box::new(FftError::new(
                format!(
                    "Wrong length of input, expected {}, got {}",
                    self.length,
                    input.len()
                )
                .as_str(),
            )));
        }
        if output.len() != (self.length / 2 + 1) {
            return Err(Box::new(FftError::new(
                format!(
                    "Wrong length of output, expected {}, got {}",
                    self.length / 2 + 1,
                    input.len()
                )
                .as_str(),
            )));
        }
        let fftlen = self.length / 2;
        //for (val, buf) in input.chunks(2).take(fftlen).zip(self.buffer_in.iter_mut()) {
        //    *buf = Complex::new(val[0], val[1]);
        //}
        let mut buf_in = unsafe {
            let ptr = input.as_mut_ptr() as *mut Complex<T>;
            let len = input.len();
            std::slice::from_raw_parts_mut(ptr, len / 2)
        };

        // FFT and store result in buffer_out
        self.fft.process(&mut buf_in, &mut self.buffer_out[0..fftlen]);

        self.buffer_out[fftlen] = self.buffer_out[0];

        for (&buf, &buf_rev, &(sin, cos), out) in zip4(
            &self.buffer_out,
            self.buffer_out.iter().rev(),
            &self.sin_cos,
            &mut output[..],
        ) {
            let xr = T::from_f32(0.5).unwrap() * (
                (buf.re + buf_rev.re) + cos * (buf.im + buf_rev.im) - sin * (buf.re - buf_rev.re)
            );
            let xi = T::from_f32(0.5).unwrap() * (
                (buf.im - buf_rev.im) - sin * (buf.im + buf_rev.im) - cos * (buf.re - buf_rev.re)
            );
            *out = Complex::new(xr, xi);
        }
        output[fftlen] = Complex::new(self.buffer_out[0].re - self.buffer_out[0].im, T::zero());
        Ok(())
    }
}

/// Create a new ComplexToReal iFFT for output data of a given length. Returns an error if the length is not even.
impl<T> ComplexToReal<T> where T: FFTnum + Float {
    pub fn new(length: usize) -> Res<Self> {
        if length % 2 > 0 {
            return Err(Box::new(FftError::new("Length must be even")));
        }
        let buffer_in = vec![Complex::zero(); length / 2];
        let mut sin_cos = Vec::with_capacity(length / 2);
        let pi = T::from_f64(std::f64::consts::PI).unwrap();
        let halflength = T::from_usize(length / 2).unwrap();
        for k in 0..length / 2 {
            let k = T::from_usize(k).unwrap();
            let sin = (k * pi / halflength).sin();
            let cos = (k * pi / halflength).cos();
            sin_cos.push((sin, cos));
        }
        let fft = Radix4::<T>::new(length / 2, true);
        Ok(ComplexToReal {
            sin_cos,
            length,
            fft,
            buffer_in,
        })
    }

    /// Transform a complex spectrum of N+1 values and store the real result in the 2*N long output.
    pub fn process(&mut self, input: &[Complex<T>], output: &mut [T]) -> Res<()> {
        if input.len() != (self.length / 2 + 1) {
            return Err(Box::new(FftError::new(
                format!(
                    "Wrong length of input, expected {}, got {}",
                    self.length / 2 + 1,
                    input.len()
                )
                .as_str(),
            )));
        }
        if output.len() != self.length {
            return Err(Box::new(FftError::new(
                format!(
                    "Wrong length of output, expected {}, got {}",
                    self.length,
                    input.len()
                )
                .as_str(),
            )));
        }

        for (&buf, &buf_rev, &(sin, cos), fft_input) in zip4(
            input,
            input.iter().rev(),
            &self.sin_cos,
            &mut self.buffer_in[..],
        ) {
            let xr = T::from_f32(0.5).unwrap() * (
                (buf.re + buf_rev.re) - cos * (buf.im + buf_rev.im) - sin * (buf.re - buf_rev.re)
            );
            let xi = T::from_f32(0.5).unwrap() * (
                (buf.im - buf_rev.im) + cos * (buf.re - buf_rev.re) - sin * (buf.im + buf_rev.im)
            );
            *fft_input = Complex::new(xr, xi);
        }

        // FFT and store result in buffer_out
        let mut buf_out = unsafe {
            let ptr = output.as_mut_ptr() as *mut Complex<T>;
            let len = output.len();
            std::slice::from_raw_parts_mut(ptr, len / 2)
        };
        self.fft.process(&mut self.buffer_in, &mut buf_out);
        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::{ComplexToReal, RealToComplex};
//     use rustfft::num_complex::Complex;
//     use rustfft::num_traits::Zero;
//     use rustfft::FFTplanner;

//     fn compare_complex(a: &[Complex<f64>], b: &[Complex<f64>], tol: f64) -> bool {
//         a.iter().zip(b.iter()).fold(true, |eq, (val_a, val_b)| {
//             eq && (val_a.re - val_b.re).abs() < tol && (val_a.im - val_b.im).abs() < tol
//         })
//     }

//     fn compare_f64(a: &[f64], b: &[f64], tol: f64) -> bool {
//         a.iter()
//             .zip(b.iter())
//             .fold(true, |eq, (val_a, val_b)| eq && (val_a - val_b).abs() < tol)
//     }

//     // Compare RealToComplex with standard FFT
//     #[test]
//     fn real_to_complex() {
//         let mut indata = vec![0.0f64; 256];
//         indata[0] = 1.0;
//         indata[3] = 0.5;
//         let mut indata_c = indata
//             .iter()
//             .map(|val| Complex::from(val))
//             .collect::<Vec<Complex<f64>>>();
//         let mut fft_planner = FFTplanner::<f64>::new(false);
//         let fft = fft_planner.plan_fft(256);

//         let mut r2c = RealToComplex::<f64>::new(256).unwrap();
//         let mut out_a: Vec<Complex<f64>> = vec![Complex::zero(); 129];
//         let mut out_b: Vec<Complex<f64>> = vec![Complex::zero(); 256];

//         fft.process(&mut indata_c, &mut out_b);
//         r2c.process(&mut indata, &mut out_a).unwrap();
//         assert!(compare_complex(&out_a[0..129], &out_b[0..129], 1.0e-9));
//     }

//     // Compare ComplexToReal with standard iFFT
//     #[test]
//     fn complex_to_real() {
//         let mut indata = vec![Complex::<f64>::zero(); 256];
//         indata[0] = Complex::new(1.0, 0.0);
//         indata[1] = Complex::new(1.0, 0.4);
//         indata[255] = Complex::new(1.0, -0.4);
//         indata[3] = Complex::new(0.3, 0.2);
//         indata[253] = Complex::new(0.3, -0.2);

//         let mut fft_planner = FFTplanner::<f64>::new(true);
//         let fft = fft_planner.plan_fft(256);

//         let mut c2r = ComplexToReal::<f64>::new(256).unwrap();
//         let mut out_a: Vec<f64> = vec![0.0; 256];
//         let mut out_b: Vec<Complex<f64>> = vec![Complex::zero(); 256];

//         c2r.process(&indata[0..129], &mut out_a).unwrap();
//         fft.process(&mut indata, &mut out_b);

//         let out_b_r = out_b.iter().map(|val| 0.5 * val.re).collect::<Vec<f64>>();
//         assert!(compare_f64(&out_a, &out_b_r, 1.0e-9));
//     }

//     // Compare RealToComplex with standard FFT
//     #[test]
//     fn real_to_complex_odd() {
//         let mut indata = vec![0.0f64; 254];
//         indata[0] = 1.0;
//         indata[3] = 0.5;
//         let mut indata_c = indata
//             .iter()
//             .map(|val| Complex::from(val))
//             .collect::<Vec<Complex<f64>>>();
//         let mut fft_planner = FFTplanner::<f64>::new(false);
//         let fft = fft_planner.plan_fft(254);

//         let mut r2c = RealToComplex::<f64>::new(254).unwrap();
//         let mut out_a: Vec<Complex<f64>> = vec![Complex::zero(); 128];
//         let mut out_b: Vec<Complex<f64>> = vec![Complex::zero(); 254];

//         fft.process(&mut indata_c, &mut out_b);
//         r2c.process(&mut indata, &mut out_a).unwrap();
//         assert!(compare_complex(&out_a[0..128], &out_b[0..128], 1.0e-9));
//     }

//     // Compare ComplexToReal with standard iFFT
//     #[test]
//     fn complex_to_real_odd() {
//         let mut indata = vec![Complex::<f64>::zero(); 254];
//         indata[0] = Complex::new(1.0, 0.0);
//         indata[1] = Complex::new(1.0, 0.4);
//         indata[253] = Complex::new(1.0, -0.4);
//         indata[3] = Complex::new(0.3, 0.2);
//         indata[251] = Complex::new(0.3, -0.2);

//         let mut fft_planner = FFTplanner::<f64>::new(true);
//         let fft = fft_planner.plan_fft(254);

//         let mut c2r = ComplexToReal::<f64>::new(254).unwrap();
//         let mut out_a: Vec<f64> = vec![0.0; 254];
//         let mut out_b: Vec<Complex<f64>> = vec![Complex::zero(); 254];

//         c2r.process(&indata[0..128], &mut out_a).unwrap();
//         fft.process(&mut indata, &mut out_b);

//         let out_b_r = out_b.iter().map(|val| 0.5 * val.re).collect::<Vec<f64>>();
//         assert!(compare_f64(&out_a, &out_b_r, 1.0e-9));
//     }
// }