use std::fs::File;
use std::io::{self, BufReader};

use ndarray::prelude::*;

use hound::{self, SampleFormat};
use rodio::{Decoder, Source};

pub fn open_audio_file(path: &str) -> io::Result<(Array2<f32>, u32)> {
    let (mut vec, sr, channels) = if let Ok(reader) = hound::WavReader::open(path) {
        let sr = reader.spec().sample_rate;
        let channels = reader.spec().channels;
        let bits = reader.spec().bits_per_sample;
        let vec: Vec<f32> = match reader.spec().sample_format {
            SampleFormat::Float => reader.into_samples::<f32>().map(|x| x.unwrap()).collect(),
            SampleFormat::Int => reader
                .into_samples::<i32>()
                .map(|x| (x.unwrap() as f32) / (2u32.pow(bits as u32 - 1) as f32))
                .collect(),
        };
        (vec, sr, channels)
    } else {
        let source = match Decoder::new(BufReader::new(File::open(path)?)) {
            Ok(decoder) => decoder,
            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        };
        let sr = source.sample_rate();
        let channels = source.channels();
        let vec: Vec<f32> = source.convert_samples::<f32>().into_iter().collect();
        (vec, sr, channels)
    };

    let shape = (channels as usize, vec.len() / channels as usize);
    vec.truncate(shape.0 * shape.1); // defensive code
    let wav = Array2::from_shape_vec(shape.strides((1, shape.0)), vec).unwrap();
    Ok((wav, sr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray_stats::QuantileExt;

    #[test]
    fn open_audio_works() {
        let (wav, sr) = open_audio_file("samples/sample_48k.wav").unwrap();
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
    }
}
