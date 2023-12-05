use std::ops::*;
use std::sync::Arc;

use ndarray::prelude::*;
use ndarray::ScalarOperand;
use num_traits::AsPrimitive;
use rayon::prelude::*;
use rustfft::{
    num_complex::Complex,
    num_traits::{Float, FloatConst},
    FftNum,
};

use super::super::utils::{Pad, PadMode};
use super::super::windows::{calc_normalized_win, WindowType};
use realfft::{RealFftPlanner, RealToComplex};

pub fn perform_stft<A>(
    input: ArrayView1<A>,
    win_length: usize,
    hop_length: usize,
    n_fft: usize,
    window: Option<CowArray<A, Ix1>>,
    fft_module: Option<Arc<dyn RealToComplex<A>>>,
    parallel: bool,
) -> Array2<Complex<A>>
where
    A: FftNum + Float + FloatConst + DivAssign + ScalarOperand,
    f32: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    let window = window.map_or_else(
        || calc_normalized_win(WindowType::Hann, win_length, n_fft).into(),
        |w| {
            assert_eq!(w.len(), win_length);
            w
        },
    );

    let to_frames_wrapper = move |x| {
        let n_pad_left = (n_fft - win_length) / 2;
        let n_pad_right = n_fft - win_length - n_pad_left;
        to_windowed_frames(x, window.view(), hop_length, (n_pad_left, n_pad_right))
    };

    let fft_module =
        fft_module.unwrap_or_else(|| RealFftPlanner::<A>::new().plan_fft_forward(n_fft));
    let do_fft = move |(x, y): (&mut Array1<A>, &mut [Complex<A>])| {
        let x = x.as_slice_mut().unwrap();
        fft_module.process(x, y).unwrap();
    };

    if input.len() < win_length {
        let padded = input.pad((win_length / 2, win_length / 2), Axis(0), PadMode::Reflect);
        let mut frames = to_frames_wrapper(padded.view());

        let n_frames = frames.len();
        let mut output = Array2::<Complex<A>>::zeros((n_frames, n_fft / 2 + 1));
        let out_frames: Vec<&mut [Complex<A>]> = output
            .axis_iter_mut(Axis(0))
            .map(|x| x.into_slice().unwrap())
            .collect();

        if parallel {
            frames.par_iter_mut().zip_eq(out_frames).for_each(do_fft);
        } else {
            frames.iter_mut().zip(out_frames).for_each(do_fft);
        }
        return output;
    }
    let front_wav =
        input
            .slice(s![..(win_length - 1)])
            .pad((win_length / 2, 0), Axis(0), PadMode::Reflect);
    let mut front_frames = to_frames_wrapper(front_wav.view());

    let mut first_i = front_frames.len() * hop_length - win_length / 2;
    let mut frames = to_frames_wrapper(input.slice(s![first_i..]));

    first_i += frames.len() * hop_length;
    let i_back_wav_start = first_i.min(input.len() - win_length / 2 - 1);

    let mut back_wav =
        input
            .slice(s![i_back_wav_start..])
            .pad((0, win_length / 2), Axis(0), PadMode::Reflect);
    back_wav.slice_collapse(s![(first_i - i_back_wav_start).max(0)..]);
    let mut back_frames = to_frames_wrapper(back_wav.view());

    let n_frames = front_frames.len() + frames.len() + back_frames.len();
    let mut output = Array2::<Complex<A>>::zeros((n_frames, n_fft / 2 + 1));
    let out_frames: Vec<&mut [Complex<A>]> = output
        .axis_iter_mut(Axis(0))
        .map(|x| x.into_slice().unwrap())
        .collect();

    if parallel {
        let in_frames = front_frames
            .par_iter_mut()
            .chain(frames.par_iter_mut())
            .chain(back_frames.par_iter_mut());
        in_frames.zip(out_frames).for_each(do_fft);
    } else {
        let in_frames = front_frames
            .iter_mut()
            .chain(frames.iter_mut())
            .chain(back_frames.iter_mut());
        in_frames.zip(out_frames).for_each(do_fft);
    }

    output
}

#[inline]
fn to_windowed_frames<A: Float>(
    input: ArrayView1<A>,
    window: ArrayView1<A>,
    hop_length: usize,
    (n_pad_left, n_pad_right): (usize, usize),
) -> Vec<Array1<A>> {
    input
        .windows(window.len())
        .into_iter()
        .step_by(hop_length)
        .map(|x| {
            (&x * &window).pad(
                (n_pad_left, n_pad_right),
                Axis(0),
                PadMode::Constant(A::zero()),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ndarray::{arr2, Array1};
    use rustfft::num_complex::Complex;
    use rustfft::num_traits::{One, Zero};

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

    use super::*;
    #[test]
    fn stft_works() {
        let impulse = Array1::<f32>::impulse(4, 2);
        assert_eq!(
            perform_stft(impulse.view(), 4, 2, 4, None, None, false),
            arr2(&[
                [
                    Complex::<f32>::new(0., 0.),
                    Complex::<f32>::new(0., 0.),
                    Complex::<f32>::new(0., 0.)
                ],
                [
                    Complex::<f32>::new(1. / 4., 0.),
                    Complex::<f32>::new(-1. / 4., 0.),
                    Complex::<f32>::new(1. / 4., 0.)
                ],
                [
                    Complex::<f32>::new(1. / 4., 0.),
                    Complex::<f32>::new(-1. / 4., 0.),
                    Complex::<f32>::new(1. / 4., 0.)
                ]
            ])
        );
    }

    #[test]
    fn stft_short_wav() {
        let impulse = Array1::<f32>::impulse(2, 1);
        let spec = perform_stft(impulse.view(), 8, 6, 8, None, None, false);
        dbg!(spec.shape());
    }
}
