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

use super::dynamics::{
    get_cached_limiter, AudioStats, GuardClipping, GuardClippingMode, MaxPeak, StatCalculator,
};

const FORMAT_DESC_DELIMITER: &str = "|";

#[readonly::make]
#[derive(PartialEq, Clone)]
pub struct Audio {
    wavs: Array2<f32>,
    pub sr: u32,
    pub stats: AudioStats,
}

impl Audio {
    pub fn new(wavs: Array2<f32>, sr: u32, stat_calculator: &mut StatCalculator) -> Self {
        let stats = stat_calculator.calc(wavs.view());
        Self { wavs, sr, stats }
    }

    pub fn view(&self) -> ArrayView2<f32> {
        self.wavs.view()
    }

    pub fn mutate<F>(
        &mut self,
        f: F,
        stat_calculator: &mut StatCalculator,
        guard_clipping_mode: GuardClippingMode,
    ) where
        F: Fn(ArrayViewMut2<f32>),
    {
        f(self.wavs.view_mut());
        self.guard_clipping(guard_clipping_mode);
        self.update_stats(stat_calculator);
    }

    #[inline]
    pub fn channel(&self, ch: usize) -> ArrayView1<f32> {
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

    fn update_stats(&mut self, stat_calculator: &mut StatCalculator) {
        self.stats = stat_calculator.calc(self.view());
    }
}

impl GuardClipping for Audio {
    type GainArray = Array2<f32>;

    fn clip(&mut self) -> Self::GainArray {
        // self.wavs.mapv_inplace(|x| x.clamp(-1., 1.));
        let mut gain_arr = Array2::ones(self.wavs.raw_dim());
        self.wavs.indexed_iter_mut().for_each(|(index, x)| {
            *x = x.clamp(-1., 1.);
            if *x > 1. {
                gain_arr[index] = 1. / *x;
                *x = 1.;
            } else if *x < -1. {
                gain_arr[index] = -1. / *x;
                *x = -1.;
            }
        });
        gain_arr
    }

    fn reduce_global_level(&mut self) -> Self::GainArray {
        let peak = self.wavs.max_peak() as f64;
        if peak > 1. {
            let gain = 1. / peak;
            self.wavs
                .mapv_inplace(|x| ((x as f64 * gain) as f32).clamp(-1., 1.));
            Array2::from_elem(self.wavs.raw_dim(), gain as f32)
        } else {
            Array2::ones(self.wavs.raw_dim())
        }
    }

    fn limit(&mut self) -> Array2<f32> {
        let mut limiter = get_cached_limiter(self.sr);
        let mut gain_arrs = Vec::with_capacity(self.n_ch());
        for wav in self.wavs.axis_iter_mut(Axis(0)) {
            gain_arrs.push(limiter.process_inplace(wav));
        }
        let gain_arr_views: Vec<_> = gain_arrs.iter().map(ArrayBase::view).collect();
        ndarray::stack(Axis(0), &gain_arr_views).unwrap_or_default()
    }
}

pub fn open_audio_file(path: &str) -> Result<(Array2<f32>, u32, String), SymphoniaError> {
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
                for (ch, vec) in vec_channels.iter_mut().enumerate() {
                    vec.extend(buf_ref.chan(ch));
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
        (vec.len()..n_ch).for_each(|_| vec.push(0.));
    }

    let shape = (n_ch, vec.len() / n_ch);
    vec.truncate(shape.0 * shape.1); // defensive code
    let wavs = Array2::from_shape_vec(shape, vec).unwrap();
    let sr = codec_params.sample_rate.unwrap_or_default();

    // TODO: format & codec description https://github.com/pdeljanov/Symphonia/issues/94
    let sample_format_str = match (codec_params.sample_format, codec_params.bits_per_sample) {
        (Some(sample_format), _) => {
            format!("{:?}", sample_format)
        }
        (None, Some(bits_per_sample)) => {
            format!("{} bit", bits_per_sample)
        }
        (None, None) => String::from("? bit"),
    };
    let format_desc = format!("{} {} {}", ext, FORMAT_DESC_DELIMITER, sample_format_str);
    Ok((wavs, sr, format_desc))
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
        let format_descs = [
            format!("wav {} 16 bit", FORMAT_DESC_DELIMITER),
            format!("unknown {} 16 bit", FORMAT_DESC_DELIMITER),
        ];
        for (path, format_desc_answer) in paths.into_iter().zip(format_descs.into_iter()) {
            let (wavs, sr, format_desc) = open_audio_file(path).unwrap();
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
            assert_eq!(sr, 48000);
            assert_eq!(wavs.shape(), &[1, 2113529]);
            assert_eq!(wavs.max().unwrap().clone(), 0.234344482421875);
            assert_eq!(wavs.min().unwrap().clone(), -0.20355224609375);
            assert_eq!(wavs.slice(s![0, ..arr.len()]), arr);
            assert_eq!(format_desc, format_desc_answer);
        }
    }
}
