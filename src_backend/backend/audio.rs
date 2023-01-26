use creak::{Decoder, DecoderError};
use ndarray::prelude::*;

pub fn open_audio_file(path: &str) -> Result<(Array2<f32>, u32, String), DecoderError> {
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
    Ok((wav, info.sample_rate(), sample_format_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray_stats::QuantileExt;

    #[test]
    fn open_audio_works() {
        let (wav, sr, sample_format_str) = open_audio_file("samples/sample_48k.wav").unwrap();
        let arr = arr2(&[[
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
        ]]);
        assert_eq!(sr, 48000);
        assert_eq!(wav.shape(), &[1, 2113529]);
        assert_eq!(wav.max().unwrap().clone(), 0.234344482421875);
        assert_eq!(wav.min().unwrap().clone(), -0.20355224609375);
        assert_eq!(wav.slice(s![.., ..arr.len()]), arr);
        assert_eq!(sample_format_str, "PCM16");
    }
}
