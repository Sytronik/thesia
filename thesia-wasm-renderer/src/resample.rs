use std::cell::RefCell;
use std::collections::HashMap;

use rubato::{FftFixedInOut, Resampler};

const UPSAMPLE_CHUNK_SIZE: usize = 1024;

type ResamplerWithBuffers = (FftFixedInOut<f32>, Vec<f32>, Vec<f32>);

thread_local! {
    static RESAMPLERS: RefCell<HashMap<(u32, u32), ResamplerWithBuffers>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn resample(wav: &[f32], sr: u32, output_sr: u32) -> Vec<f32> {
    let output_len = (wav.len() as f64 * output_sr as f64 / sr as f64).round() as usize;

    RESAMPLERS.with_borrow_mut(|resamplers| {
        resamplers.retain(|key, _| !(key.0 == sr && key.1 > output_sr * 2));
        let (resampler, in_buffer, out_buffer) =
            resamplers.entry((sr, output_sr)).or_insert_with(|| {
                let resampler = FftFixedInOut::<f32>::new(
                    sr as usize,
                    output_sr as usize,
                    UPSAMPLE_CHUNK_SIZE,
                    1,
                )
                .unwrap();
                let in_buffer = Vec::with_capacity(resampler.input_frames_max());
                let out_buffer = vec![0.; resampler.output_frames_max()];
                (resampler, in_buffer, out_buffer)
            });
        let delay = resampler.output_delay();
        let mut resampled = Vec::with_capacity(delay + output_len + output_sr as usize / 100);

        {
            let mut wave_out = [out_buffer.as_mut_slice()];

            // process frames in chunks
            let mut i = 0;
            while i + resampler.input_frames_next() <= wav.len() {
                let wave_in = [&wav[i..i + resampler.input_frames_next()]];
                let (in_frame_len, out_frame_len) = resampler
                    .process_into_buffer(&wave_in, &mut wave_out, None)
                    .unwrap();
                resampled.extend_from_slice(&wave_out[0][..out_frame_len]);
                i += in_frame_len;
            }

            // process remaining frames
            if i < wav.len() {
                in_buffer.extend_from_slice(&wav[i..]);
                in_buffer.resize(resampler.input_frames_next(), 0.0);
                let wave_in = [in_buffer.as_slice()];
                let (_, out_frame_len) = resampler
                    .process_into_buffer(&wave_in, &mut wave_out, None)
                    .unwrap();
                resampled.extend_from_slice(&wave_out[0][..out_frame_len]);
            }

            // flush the last frame
            in_buffer.resize(resampler.input_frames_next(), 0.0);
            let wave_in = [in_buffer.as_slice()];
            let (_, out_frame_len) = resampler
                .process_into_buffer(&wave_in, &mut wave_out, None)
                .unwrap();
            resampled.extend_from_slice(&wave_out[0][..out_frame_len]);
        }

        resampler.reset();
        in_buffer.clear();
        out_buffer.fill(0.0);

        resampled.drain(..delay);
        resampled.truncate(output_len);
        resampled
    })
}
