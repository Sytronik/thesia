use ndarray::prelude::*;
use num_traits::identities::*;
use num_traits::Num;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::Complex;
use rustfft::{FFTnum, FFT};

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

pub fn rfft<A>(input: Array1<A>) -> Array1<Complex<A>>
where
    A: Clone + Num + FFTnum,
{
    let n_fft = input.shape()[0];
    let mut input_vec: Vec<Complex<A>> = input
        .to_vec()
        .iter()
        .map(|x| Complex::<A>::new(*x, A::zero()))
        .collect();
    let mut output_vec: Vec<Complex<A>> = vec![Complex::zero(); n_fft];

    let fft = Radix4::new(n_fft, false);
    fft.process(&mut input_vec, &mut output_vec);
    output_vec.drain(n_fft / 2 + 1..);
    Array1::<Complex<A>>::from(output_vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        assert_eq!(
            Array1::<f32>::impulse(4, 0).to_vec(),
            vec![1., 0., 0., 0.]
        );
        assert_eq!(
            rfft(Array1::<f32>::impulse(4, 0)).to_vec(),
            vec![Complex::<f32>::new(1., 0.); 3]
        );
    }
}
