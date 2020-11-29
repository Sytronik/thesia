use std::fs::File;
use std::io;
use std::io::{BufReader, Result};
use std::path::Path;

use ndarray::prelude::*;

use rodio::{Decoder, Source};
use hound::{self, SampleFormat};

pub fn open_audio_file(path: &Path) -> Result<(Array2<f32>, u32)> {
    let (mut vec, sr, channels) = if let Ok(reader) = hound::WavReader::open(path.as_os_str()) {
        let sr = reader.spec().sample_rate;
        let channels = reader.spec().channels;
        let bits = reader.spec().bits_per_sample;
        let vec: Vec<f32> = match reader.spec().sample_format {
            SampleFormat::Float 
                => reader.into_samples::<f32>().map(|x| x.unwrap()).collect(),
            SampleFormat::Int 
                => reader.into_samples::<i32>().map(|x| (x.unwrap() as f32) / (2i32.pow(bits as u32-1) as f32)).collect(),
        };
        (vec, sr, channels)
    }
    else {
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
    Ok((Array2::from_shape_vec(shape.strides((1, shape.0)), vec).unwrap(), sr))
}
