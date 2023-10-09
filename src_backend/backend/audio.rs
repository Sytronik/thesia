use std::borrow::Cow;
use std::io;
use std::path::Path;

use ndarray::prelude::*;
use symphonia::core::audio::{AudioBuffer, Signal};
use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, Track as SymphoniaTrack};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

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

    pub fn view(&self) -> ArrayView2<f32> {
        self.wavs.view()
    }

    pub fn planes(&self) -> Vec<&[f32]> {
        self.wavs
            .axis_iter(Axis(0))
            .map(|x| x.to_slice().unwrap())
            .collect()
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

pub fn open_audio_file(path: &str) -> Result<(Audio, String), SymphoniaError> {
    let src = std::fs::File::open(path)?;

    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    let (ext, hint) = if let Some(ext) = Path::new(path).extension() {
        let ext = ext.to_string_lossy();
        hint.with_extension(&ext);
        (ext, hint)
    } else {
        (Cow::Borrowed("unknown"), hint)
    };

    let mut probed = {
        let fmt_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };

        // Probe the media source.
        symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &Default::default())?
    };

    // Find the first audio track with a known (decodeable) codec.
    let SymphoniaTrack {
        id: track_id,
        codec_params,
        language: _,
    } = probed
        .format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| {
            SymphoniaError::IoError(io::Error::new(
                io::ErrorKind::InvalidData,
                "no audio track found",
            ))
        })?
        .clone();

    // Create a decoder for the track.
    // Use the default options for the decoder.
    let mut decoder = symphonia::default::get_codecs().make(&codec_params, &Default::default())?;

    let n_ch = codec_params.channels.unwrap_or_default().count();
    let mut temp_buf: Option<AudioBuffer<f32>> = None;
    let mut vec_channels = vec![Vec::new(); n_ch];
    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match probed.format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err)) if err.kind() == io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(err) => {
                // A unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !probed.format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            probed.format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(_decoded) => {
                if temp_buf.is_none()
                    || temp_buf
                        .as_ref()
                        .is_some_and(|x| x.capacity() < _decoded.capacity())
                {
                    temp_buf = Some(_decoded.make_equivalent::<f32>());
                };
                let buf_ref = temp_buf.as_mut().unwrap();
                _decoded.convert(buf_ref);
                for ch in 0..n_ch {
                    vec_channels[ch].extend(buf_ref.chan(ch));
                }
            }
            Err(SymphoniaError::DecodeError(err)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                println!(
                    "[Warning] DecodeError by wrong packet of audio file: {}",
                    err
                );
                continue;
            }
            Err(err) => {
                // An unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        }
    }

    let mut vec: Vec<_> = vec_channels.into_iter().flatten().collect();
    if vec.len() < n_ch {
        (vec.len()..n_ch).into_iter().for_each(|_| vec.push(0.));
    }

    let shape = (n_ch, vec.len() / n_ch);
    vec.truncate(shape.0 * shape.1); // defensive code
    let wav = Array2::from_shape_vec(shape, vec).unwrap();
    let sr = codec_params.sample_rate.unwrap_or_default();

    // TODO: format & codec description https://github.com/pdeljanov/Symphonia/issues/94
    let get_bit_depth_str = || {
        format!(
            "{} bit",
            codec_params
                .bits_per_sample
                .map_or(String::from("?"), |x| x.to_string())
        )
    };
    let format_desc = format!(
        "{} / {}",
        ext,
        codec_params
            .sample_format
            .map_or_else(get_bit_depth_str, |x| format!("{:?}", x))
    );
    Ok((Audio::new(wav, sr), format_desc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray_stats::QuantileExt;

    #[test]
    fn open_audio_works() {
        let paths = [
            "samples/sample_48k.wav",
            "samples/sample_48k_wav_no_extension",
        ];
        let format_descs = ["wav / 16 bit", "unknown / 16 bit"];
        for (path, format_desc_answer) in paths.into_iter().zip(format_descs.into_iter()) {
            let (audio, format_desc) = open_audio_file(path).unwrap();
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
            assert_eq!(format_desc, format_desc_answer);
        }
    }
}
