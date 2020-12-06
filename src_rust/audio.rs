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
    use approx::assert_abs_diff_eq;
    use ndarray_stats::QuantileExt;

    #[test]
    fn open_audio_works() {
        let (wav, sr) = open_audio_file("samples/sample.wav").unwrap();
        let arr = arr2(&[[
            -1.919269561767578125e-05f32,
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
        assert_abs_diff_eq!(wav.max().unwrap().clone(), 0.1715821,);
        wav.iter()
            .zip(arr.iter())
            .for_each(|(&x, &y)| assert_abs_diff_eq!(x, y));
    }
}
