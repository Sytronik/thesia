use std::cell::RefCell;
use std::io;
use std::path::Path;

use kittyaudio::Frame;
use napi_derive::napi;
use ndarray::prelude::*;
use rayon::prelude::*;
use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::AudioCodecParameters;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::Track as SymphoniaTrack;
use symphonia::core::io::MediaSourceStream;

use super::dynamics::{
    AudioStats, GuardClipping, GuardClippingMode, GuardClippingResult, GuardClippingStats,
    LimiterManager, MaxPeak, StatCalculator,
};

#[readonly::make]
#[derive(PartialEq, Clone)]
pub struct Audio {
    wavs: Array2<f32>,
    pub sr: u32,
    pub stats: AudioStats,
    pub guard_clip_result: GuardClippingResult<Ix2>,
    pub guard_clip_stats: Array1<GuardClippingStats>,
}

impl Audio {
    pub fn new(wavs: Array2<f32>, sr: u32, stat_calculator: &mut StatCalculator) -> Self {
        let stats = stat_calculator.calc(wavs.view());
        let guard_clip_result = GuardClippingResult::GlobalGain((1., wavs.raw_dim()));
        let guard_clip_stats = Array1::default(wavs.shape()[0]);
        Self {
            wavs,
            sr,
            stats,
            guard_clip_result,
            guard_clip_stats,
        }
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
        let guard_clip_result = self.guard_clipping(guard_clipping_mode);
        self.guard_clip_stats = (&guard_clip_result).into();
        self.guard_clip_result = guard_clip_result;
        self.update_stats(stat_calculator);
    }

    #[inline]
    pub fn channel(&self, ch: usize) -> ArrayView1<f32> {
        self.wavs.slice(s![ch, ..])
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

impl GuardClipping<Ix2> for Audio {
    fn clip(&mut self) -> GuardClippingResult<Ix2> {
        let before_clip = self.wavs.clone();
        self.wavs
            .axis_iter_mut(Axis(0))
            .into_par_iter()
            .for_each(|mut channel| {
                channel.mapv_inplace(|x| x.clamp(-1., 1.));
            });
        GuardClippingResult::WavBeforeClip(before_clip)
    }

    fn reduce_global_level(&mut self) -> GuardClippingResult<Ix2> {
        let peak = self.wavs.max_peak() as f64;
        if peak > 1. {
            let gain = 1. / peak;
            self.wavs
                .axis_iter_mut(Axis(0))
                .into_par_iter()
                .for_each(|mut channel| {
                    channel.mapv_inplace(|x| ((x as f64 * gain) as f32).clamp(-1., 1.));
                });
            GuardClippingResult::GlobalGain((gain as f32, self.wavs.raw_dim()))
        } else {
            GuardClippingResult::GlobalGain((1., self.wavs.raw_dim()))
        }
    }

    fn limit(&mut self) -> GuardClippingResult<Ix2> {
        thread_local! {
            pub static LIMITER_MANAGER: RefCell<LimiterManager> = RefCell::new(LimiterManager::new());
        }

        let peak = self.wavs.max_peak();
        let gain_shape = (1, self.wavs.shape()[1]);
        let gain_seq = if peak > 1. {
            let gain_seq = LIMITER_MANAGER.with_borrow_mut(|manager| {
                let limiter = manager.get_or_insert(self.sr);
                limiter.process_inplace(self.wavs.view_mut())
            });
            gain_seq.into_shape_with_order(gain_shape).unwrap()
        } else {
            Array2::ones(gain_shape)
        };
        GuardClippingResult::GainSequence(gain_seq)
    }
}

impl From<&Audio> for Vec<Frame> {
    fn from(value: &Audio) -> Self {
        value
            .view()
            .axis_iter(Axis(1))
            .map(|frame| {
                match frame.len() {
                    1 => frame[0].into(),
                    2 => (frame[0], frame[1]).into(),
                    _ => unimplemented!(), // TODO
                }
            })
            .collect()
    }
}

#[napi(object)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioFormatInfo {
    pub name: String,
    #[napi(js_name = "sampleRate")]
    pub sr: u32,
    pub bit_depth: String,
    pub bitrate: String,
}

impl AudioFormatInfo {
    pub fn from_decoding_result(
        format_name: &str,
        codec_name: &str,
        codec_params: &AudioCodecParameters,
        found_sr: u32,
        found_sample_format: &str,
        total_packets_byte: usize,
        decoded_wav_len: usize,
    ) -> Self {
        let format_name = format_name.replace("wave", "wav");
        let name = if format_name == codec_name {
            format_name
        } else {
            format!("{} - {}", format_name, codec_name)
        };
        if codec_name == "alac" {
            return Self {
                name,
                sr: found_sr,
                bit_depth: found_sample_format.into(),
                bitrate: "".into(),
            };
        }
        if name.starts_with("wav") {
            return Self {
                name,
                sr: found_sr,
                ..Default::default()
            };
        }
        let (bit_depth, bitrate) = match (
            codec_params.sample_format,
            codec_params.bits_per_sample,
            codec_params.bits_per_coded_sample,
        ) {
            (Some(sample_format), _, _) => (format!("{:?}", sample_format), "".into()),
            (None, Some(bits_per_sample), _) => (format!("{} bit", bits_per_sample), "".into()),
            (None, None, Some(bits_per_coded_sample)) => {
                let kbps = (bits_per_coded_sample as usize * found_sr as usize) as f64 / 1000.0;
                ("".into(), format!("{} kbps", kbps.round() as usize))
            }
            (None, None, None) => {
                let kbps = (total_packets_byte * 8) as f64 * found_sr as f64
                    / decoded_wav_len as f64
                    / 1000.;
                ("".into(), format!("{} kbps", kbps.round() as usize))
            }
        };
        Self {
            name,
            sr: found_sr,
            bit_depth,
            bitrate,
        }
    }
}

pub fn open_audio_file(path: &str) -> Result<(Array2<f32>, AudioFormatInfo), SymphoniaError> {
    let src = std::fs::File::open(path)?;

    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    if let Some(ext) = Path::new(path).extension() {
        let ext = ext.to_string_lossy();
        hint.with_extension(&ext);
    }

    // Probe the media source.
    let mut format = symphonia::default::get_probe().probe(
        &hint,
        mss,
        Default::default(),
        Default::default(),
    )?;

    // Find the first audio track with a known (decodeable) codec.
    let SymphoniaTrack {
        id: track_id,
        codec_params,
        language: _,
        time_base,
        num_frames,
        ..
    } = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.as_ref().is_some_and(|p| p.audio().is_some()))
        .ok_or_else(|| {
            SymphoniaError::IoError(io::Error::new(
                io::ErrorKind::InvalidData,
                "no audio track found",
            ))
        })?
        .clone();
    let codec_params = codec_params.as_ref().unwrap().audio().unwrap();
    let mut n_ch = codec_params.channels.as_ref().map_or(0, |c| c.count());
    let mut sr = codec_params.sample_rate.unwrap_or_default();

    // Create a decoder for the track.
    // Use the default options for the decoder.
    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(codec_params, &Default::default())?;

    let total_duration = match (time_base, num_frames) {
        (Some(tb), Some(nf)) => tb.calc_time(nf),
        _ => Default::default(),
    };
    let n_samples = (sr as f64 * (total_duration.seconds as f64 + total_duration.frac)) as usize;
    let mut planes = vec![Vec::with_capacity(n_samples); n_ch];
    let mut found_sample_format = "";
    let mut total_packets_byte = 0;
    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => {
                // Reached the end of the stream.
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
                // A unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(_decoded) => {
                if found_sample_format.is_empty() {
                    found_sample_format = match _decoded {
                        GenericAudioBufferRef::U8(_) | GenericAudioBufferRef::S8(_) => "8 bit",
                        GenericAudioBufferRef::U16(_) | GenericAudioBufferRef::S16(_) => "16 bit",
                        GenericAudioBufferRef::U24(_) | GenericAudioBufferRef::S24(_) => "24 bit",
                        GenericAudioBufferRef::U32(_) | GenericAudioBufferRef::S32(_) => "32 bit",
                        GenericAudioBufferRef::F32(_) => "32 bit",
                        GenericAudioBufferRef::F64(_) => "64 bit",
                    };
                }
                let found_n_ch = _decoded.num_planes();
                if found_n_ch != n_ch {
                    planes.resize_with(found_n_ch, || Vec::with_capacity(n_samples));
                    n_ch = found_n_ch;
                }
                let found_sr = _decoded.spec().rate();
                if sr != found_sr {
                    sr = found_sr;
                }
                let mut slices: Vec<_> = planes
                    .iter_mut()
                    .map(|plane| {
                        let prev_len = plane.len();
                        plane.resize(prev_len + _decoded.samples_planar(), 0.);
                        &mut plane[prev_len..]
                    })
                    .collect();
                _decoded.copy_to_slice_planar(&mut slices);
                total_packets_byte += packet.buf().len();
            }
            Err(SymphoniaError::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                continue;
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

    let mut vec: Vec<_> = planes.into_iter().flatten().collect();
    if vec.len() < n_ch {
        (vec.len()..n_ch).for_each(|_| vec.push(0.));
    }

    if n_ch == 0 {
        return Err(SymphoniaError::IoError(io::Error::new(
            io::ErrorKind::InvalidData,
            "no audio channels found",
        )));
    }
    let shape = (n_ch, vec.len() / n_ch);
    vec.truncate(shape.0 * shape.1); // defensive code
    let wavs = Array2::from_shape_vec(shape, vec).unwrap();

    let format_info = AudioFormatInfo::from_decoding_result(
        format.format_info().short_name,
        decoder.codec_info().short_name,
        codec_params,
        sr,
        found_sample_format,
        total_packets_byte,
        wavs.shape()[1],
    );
    Ok((wavs, format_info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;

    #[test]
    fn open_audio_works() {
        let paths = [
            "samples/sample_48k.wav",
            "samples/sample_48k_wav_no_extension",
        ];
        let format_infos = [
            AudioFormatInfo {
                name: "wav - pcm_s16le".into(),
                sr: 48000,
                bit_depth: "".into(),
                bitrate: "".into(),
            },
            AudioFormatInfo {
                name: "wav - pcm_s16le".into(),
                sr: 48000,
                bit_depth: "".into(),
                bitrate: "".into(),
            },
        ];
        for (path, format_info_answer) in paths.into_iter().zip(format_infos.into_iter()) {
            let (wavs, format_info) = open_audio_file(path).unwrap();
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
            assert_eq!(wavs.shape(), &[1, 2113529]);
            let (&min, &max) = wavs.iter().minmax().into_option().unwrap();
            assert_eq!((min, max), (-0.20355224609375, 0.234344482421875));
            assert_eq!(wavs.slice(s![0, ..arr.len()]), arr);
            assert_eq!(format_info, format_info_answer);
        }
    }
}
