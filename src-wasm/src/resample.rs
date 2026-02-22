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

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn clear_resamplers() {
        RESAMPLERS.with_borrow_mut(|resamplers| resamplers.clear());
    }

    fn impulse(len: usize, index: usize) -> Vec<f32> {
        let mut signal = vec![0.0; len];
        signal[index] = 1.0;
        signal
    }

    fn peak_index(signal: &[f32]) -> usize {
        signal
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(index, _)| index)
            .unwrap()
    }

    fn expected_index(input_index: usize, sr: u32, output_sr: u32) -> usize {
        (input_index as f64 * output_sr as f64 / sr as f64).round() as usize
    }

    fn make_test_signal(len: usize, sr: u32) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let t = i as f32 / sr as f32;
                // Keep energy away from Nyquist so round-trip error reflects resampler quality.
                0.55 * (2.0 * PI * 220.0 * t).sin()
                    + 0.30 * (2.0 * PI * 880.0 * t).sin()
                    + 0.15 * (2.0 * PI * 3200.0 * t).sin()
            })
            .collect()
    }

    #[test]
    fn resample_has_no_extra_lag_for_impulse() {
        clear_resamplers();

        let input_len = UPSAMPLE_CHUNK_SIZE * 4 + 77;
        let input_impulse_index = UPSAMPLE_CHUNK_SIZE + 37;
        let sample_rate_pairs = [
            (44_100, 44_100),
            (44_100, 48_000),
            (48_000, 44_100),
            (22_050, 48_000),
            (96_000, 44_100),
        ];

        for (sr, output_sr) in sample_rate_pairs {
            let output = resample(&impulse(input_len, input_impulse_index), sr, output_sr);
            let actual_peak = peak_index(&output);
            let expected_peak = expected_index(input_impulse_index, sr, output_sr);

            assert!(
                actual_peak.abs_diff(expected_peak) <= 1,
                "unexpected lag for {sr}->{output_sr}: expected impulse near {expected_peak}, got {actual_peak}"
            );
        }
    }

    #[test]
    fn resample_keeps_zero_index_impulse_at_zero() {
        clear_resamplers();

        let input = impulse(UPSAMPLE_CHUNK_SIZE * 4, 0);
        let output = resample(&input, 44_100, 48_000);
        let peak = peak_index(&output);

        assert_eq!(peak, 0, "resampler introduced leading lag at signal start");
    }

    #[test]
    fn round_trip_up_and_down_matches_original_closely() {
        clear_resamplers();

        let source_sr = 44_100;
        let upsampled_sr = 48_000;
        let source = make_test_signal(UPSAMPLE_CHUNK_SIZE * 8 + 73, source_sr);

        let upsampled = resample(&source, source_sr, upsampled_sr);
        let round_tripped = resample(&upsampled, upsampled_sr, source_sr);

        assert_eq!(
            round_tripped.len(),
            source.len(),
            "round-trip output length changed unexpectedly"
        );

        // Ignore boundaries where zero-padding/flush effects are strongest.
        let edge = 256usize;
        let compare_len = source.len().saturating_sub(edge * 2);
        assert!(
            compare_len > 0,
            "test signal too short for round-trip check"
        );

        let source_mid = &source[edge..edge + compare_len];
        let round_tripped_mid = &round_tripped[edge..edge + compare_len];

        let mut abs_error_sum = 0.0f32;
        let mut max_abs_error = 0.0f32;

        for (&orig, &rt) in source_mid.iter().zip(round_tripped_mid.iter()) {
            let abs_err = (orig - rt).abs();
            abs_error_sum += abs_err;
            max_abs_error = max_abs_error.max(abs_err);
        }

        let mae = abs_error_sum / compare_len as f32;

        assert!(
            mae < 1.0e-4,
            "round-trip mean absolute error too high: mae={mae}, max_abs_error={max_abs_error}"
        );
        assert!(
            max_abs_error < 1.0e-3,
            "round-trip max absolute error too high: mae={mae}, max_abs_error={max_abs_error}"
        );
    }

    #[test]
    fn resample_empty_input_returns_empty_output() {
        clear_resamplers();

        let output = resample(&[], 44_100, 48_000);
        assert!(
            output.is_empty(),
            "empty input should produce empty output, got len={}",
            output.len()
        );
    }
}
