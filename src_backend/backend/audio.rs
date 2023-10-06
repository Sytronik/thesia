use creak::{Decoder, DecoderError};
use ndarray::prelude::*;

#[readonly::make]
#[derive(PartialEq)]
pub struct Audio {
    wavs: Array2<f32>,
    pub sr: u32,
}

impl Audio {
    pub fn new(wavs: Array2<f32>, sr: u32) -> Self {
        Self { wavs, sr }
    }

    pub fn get_entire(&self) -> ArrayView2<f32> {
        self.wavs.view()
    }

    #[inline]
    pub fn get_ch<'a>(&'a self, ch: usize) -> ArrayView1<'a, f32> {
        self.wavs.index_axis(Axis(0), ch)
    }

    #[inline]
    pub fn n_ch(&self) -> usize {
        self.wavs.shape()[0]
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.wavs.shape()[1]
    }

    #[inline]
    pub fn sec(&self) -> f64 {
        self.wavs.shape()[1] as f64 / self.sr as f64
    }
}

pub fn open_audio_file(path: &str) -> Result<(Audio, String), DecoderError> {
    let decoder = Decoder::open(path)?;
    let info = decoder.info();
    let channels = info.channels() as usize;
    let sample_format_str = info.format().to_string(); // TODO: sample format
    let mut vec: Vec<f32> = Vec::with_capacity(channels);
    for sample in decoder.into_samples()? {
        vec.push(sample?);
    }
    if vec.len() < channels {
        (vec.len()..channels).into_iter().for_each(|_| vec.push(0.));
    }

    let shape = (channels, vec.len() / channels);
    vec.truncate(shape.0 * shape.1); // defensive code
    let wav = Array2::from_shape_vec(shape.strides((1, shape.0)), vec).unwrap();
    Ok((Audio::new(wav, info.sample_rate()), sample_format_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray_stats::QuantileExt;

    #[test]
    fn open_audio_works() {
        let (audio, sample_format_str) = open_audio_file("samples/sample_48k.wav").unwrap();
        let arr = arr1(&[
            0.00000000e+00f32,
            0.00000000e+00,
            0.00000000e+00,
            0.00000000e+00,
            0.00000000e+00,
            0.00000000e+00,
            0.00000000e+00,
            -3.05175781e-05,
            -3.05175781e-05,
            -3.05175781e-05,
            -3.05175781e-05,
            -3.05175781e-05,
            -3.05175781e-05,
            -3.05175781e-05,
            -3.05175781e-05,
            0.00000000e+00,
        ]);
        assert_eq!(audio.sr, 48000);
        assert_eq!((audio.n_ch(), audio.len()), (1, 2113529));
        assert_eq!(audio.get_ch(0).max().unwrap().clone(), 0.234344482421875);
        assert_eq!(audio.get_ch(0).min().unwrap().clone(), -0.20355224609375);
        assert_eq!(audio.get_ch(0).slice(s![..arr.len()]), arr);
        assert_eq!(sample_format_str, "PCM16");
    }
}
