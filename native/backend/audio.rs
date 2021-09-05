use std::fs::File;
use std::io::{self, BufReader};

use ndarray::prelude::*;

use rodio::{Decoder, Source};

pub fn open_audio_file(path: &str) -> io::Result<(Array2<f32>, u32, String)> {
    let source = match Decoder::new(BufReader::new(File::open(path)?)) {
        Ok(decoder) => decoder,
        Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
    };
    let sr = source.sample_rate();
    let channels = source.channels() as usize;
    let sample_format_str = source.sample_format_str();
    let mut vec: Vec<f32> = source.collect();
    if vec.len() < channels {
        (vec.len()..channels).into_iter().for_each(|_| vec.push(0.));
    }

    let shape = (channels, vec.len() / channels);
    vec.truncate(shape.0 * shape.1); // defensive code
    let wav = Array2::from_shape_vec(shape.strides((1, shape.0)), vec).unwrap();
    Ok((wav, sr, sample_format_str))
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
