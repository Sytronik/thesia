use std::sync::Arc;

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction, calculate_cutoff,
};

use super::device::{choose_stream_config, default_output_device, device_name};
use super::state::{PlaybackData, SharedPlayback, set_error};

const RUBATO_CHUNK_SIZE: usize = 1024;
const RUBATO_SINC_LEN: usize = 256;
const RUBATO_OVERSAMPLING: usize = 128;
const RUBATO_MAX_GENERATE_ATTEMPTS: usize = 16;
const RUBATO_QUEUE_COMPACT_THRESHOLD: usize = 8192;
const RUBATO_WINDOW: WindowFunction = WindowFunction::BlackmanHarris2;

pub(super) struct OutputStreamState {
    _stream: Stream,
    pub(super) device_name: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RubatoConfigKey {
    input_sample_rate: u32,
    output_sample_rate: u32,
    output_channels: usize,
}

struct RubatoStreamResampler {
    key: RubatoConfigKey,
    resampler: Async<f32>,
    input_buffer: Vec<Vec<f32>>,
    output_buffer: Vec<Vec<f32>>,
    queued_interleaved: Vec<f32>,
    queued_offset: usize,
    pending_delay_drop: usize,
    input_cursor_frame: usize,
    drain_chunks_left: usize,
}

impl RubatoStreamResampler {
    fn new(
        input_sample_rate: u32,
        output_sample_rate: u32,
        output_channels: usize,
    ) -> Result<Self, String> {
        if input_sample_rate == 0 || output_sample_rate == 0 {
            return Err("Invalid sample rate for resampler".to_string());
        }
        if output_channels == 0 {
            return Err("Invalid output channel count for resampler".to_string());
        }

        let ratio = output_sample_rate as f64 / input_sample_rate as f64;
        let params = SincInterpolationParameters {
            sinc_len: RUBATO_SINC_LEN,
            f_cutoff: calculate_cutoff(RUBATO_SINC_LEN, RUBATO_WINDOW),
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: RUBATO_OVERSAMPLING,
            window: RUBATO_WINDOW,
        };
        let resampler = Async::<f32>::new_sinc(
            ratio,
            1.0,
            &params,
            RUBATO_CHUNK_SIZE,
            output_channels,
            FixedAsync::Output,
        )
        .map_err(|e| format!("Failed to build rubato resampler: {}", e))?;
        let pending_delay_drop = resampler.output_delay();
        let input_buffer = vec![vec![0.; resampler.input_frames_max()]; output_channels];
        let output_buffer = vec![vec![0.; resampler.output_frames_max()]; output_channels];

        Ok(Self {
            key: RubatoConfigKey {
                input_sample_rate,
                output_sample_rate,
                output_channels,
            },
            resampler,
            input_buffer,
            output_buffer,
            queued_interleaved: Vec::with_capacity(output_channels * RUBATO_CHUNK_SIZE * 4),
            queued_offset: 0,
            pending_delay_drop,
            input_cursor_frame: 0,
            drain_chunks_left: 0,
        })
    }

    fn matches(
        &self,
        input_sample_rate: u32,
        output_sample_rate: u32,
        output_channels: usize,
    ) -> bool {
        self.key
            == RubatoConfigKey {
                input_sample_rate,
                output_sample_rate,
                output_channels,
            }
    }

    fn reset_for_cursor(&mut self, position_frame: f64) {
        self.resampler.reset();
        self.pending_delay_drop = self.resampler.output_delay();
        self.input_cursor_frame = position_frame.max(0.).floor() as usize;
        self.drain_chunks_left = 0;
        self.queued_interleaved.clear();
        self.queued_offset = 0;
    }

    fn output_frames_available(&self) -> usize {
        let available_samples = self
            .queued_interleaved
            .len()
            .saturating_sub(self.queued_offset);
        available_samples / self.key.output_channels
    }

    fn read_frame(&mut self, dst: &mut [f32]) -> bool {
        if dst.len() < self.key.output_channels || self.output_frames_available() == 0 {
            return false;
        }
        let start = self.queued_offset;
        let end = start + self.key.output_channels;
        dst[..self.key.output_channels].copy_from_slice(&self.queued_interleaved[start..end]);
        self.queued_offset = end;

        if self.queued_offset == self.queued_interleaved.len() {
            self.queued_interleaved.clear();
            self.queued_offset = 0;
        } else if self.queued_offset >= RUBATO_QUEUE_COMPACT_THRESHOLD
            && self.queued_offset * 2 >= self.queued_interleaved.len()
        {
            self.queued_interleaved.drain(..self.queued_offset);
            self.queued_offset = 0;
        }
        true
    }

    fn append_output(&mut self, out_frames: usize) {
        if out_frames == 0 {
            return;
        }

        let dropped = self.pending_delay_drop.min(out_frames);
        self.pending_delay_drop -= dropped;
        let kept_frames = out_frames - dropped;
        if kept_frames == 0 {
            return;
        }

        self.queued_interleaved
            .reserve(kept_frames * self.key.output_channels);
        for frame_idx in dropped..out_frames {
            for channel in 0..self.key.output_channels {
                self.queued_interleaved
                    .push(self.output_buffer[channel][frame_idx]);
            }
        }
    }

    fn process_chunk(
        &mut self,
        playback: &PlaybackData,
        total_frames: usize,
    ) -> Result<bool, String> {
        if self.input_cursor_frame >= total_frames && self.drain_chunks_left == 0 {
            return Ok(false);
        }
        let output_channels = self.key.output_channels;
        let needed_input_frames = self.resampler.input_frames_next();

        for channel in 0..output_channels {
            let input_channel = &mut self.input_buffer[channel];
            if input_channel.len() < needed_input_frames {
                input_channel.resize(needed_input_frames, 0.);
            }
            input_channel[..needed_input_frames].fill(0.);
        }

        let available_input_frames = total_frames.saturating_sub(self.input_cursor_frame);
        let copied_input_frames = available_input_frames.min(needed_input_frames);
        for frame_offset in 0..copied_input_frames {
            let source_frame_idx = self.input_cursor_frame + frame_offset;
            for output_channel in 0..output_channels {
                self.input_buffer[output_channel][frame_offset] = source_sample_for_output(
                    playback,
                    source_frame_idx,
                    output_channel,
                    output_channels,
                );
            }
        }
        self.input_cursor_frame += copied_input_frames;
        if self.input_cursor_frame >= total_frames && self.drain_chunks_left == 0 {
            self.drain_chunks_left = 1;
        }
        if copied_input_frames == 0 && self.drain_chunks_left > 0 {
            self.drain_chunks_left -= 1;
        }

        let output_frames_capacity = self.output_buffer.first().map_or(0, Vec::len);
        let input_adapter = SequentialSliceOfVecs::new(
            self.input_buffer.as_slice(),
            output_channels,
            needed_input_frames,
        )
        .map_err(|e| format!("rubato input adapter error: {}", e))?;
        let mut output_adapter = SequentialSliceOfVecs::new_mut(
            self.output_buffer.as_mut_slice(),
            output_channels,
            output_frames_capacity,
        )
        .map_err(|e| format!("rubato output adapter error: {}", e))?;
        let (_, out_frames) = self
            .resampler
            .process_into_buffer(&input_adapter, &mut output_adapter, None)
            .map_err(|e| format!("rubato process error: {}", e))?;
        self.append_output(out_frames);

        Ok(self.output_frames_available() > 0
            || self.input_cursor_frame < total_frames
            || self.drain_chunks_left > 0)
    }
}

#[derive(Default)]
struct RenderState {
    rubato_resampler: Option<RubatoStreamResampler>,
    observed_cursor_version: Option<u64>,
    frame_buffer: Vec<f32>,
}

fn source_sample_for_output(
    playback: &PlaybackData,
    source_frame_idx: usize,
    output_channel: usize,
    output_channels: usize,
) -> f32 {
    if playback.input_channels == 0 {
        return 0.;
    }
    let source_offset = source_frame_idx * playback.input_channels;
    if source_offset >= playback.samples.len() {
        return 0.;
    }

    if playback.input_channels == 1 {
        return playback.samples[source_offset];
    }

    if output_channels == 1 {
        let left = playback.samples[source_offset];
        let right = playback.samples[source_offset + 1.min(playback.input_channels - 1)];
        return (left + right) * 0.5;
    }

    let source_channel = if playback.input_channels == 2 {
        output_channel % 2
    } else {
        output_channel.min(playback.input_channels - 1)
    };
    playback.samples[source_offset + source_channel]
}

fn fill_silence<T, F>(data: &mut [T], convert: &F)
where
    T: Copy,
    F: Fn(f32) -> T,
{
    for sample in data {
        *sample = convert(0.);
    }
}

fn fill_output_without_resampler<T, F>(
    data: &mut [T],
    output_channels: usize,
    playback: &mut PlaybackData,
    total_frames: usize,
    shared: &SharedPlayback,
    convert: &F,
) where
    T: Copy,
    F: Fn(f32) -> T,
{
    let mut reached_end = false;
    for frame in data.chunks_mut(output_channels) {
        let source_frame_idx = playback.position_frame.max(0.).floor() as usize;
        if source_frame_idx >= total_frames {
            reached_end = true;
            for sample in frame {
                *sample = convert(0.);
            }
            continue;
        }

        for (output_channel, sample) in frame.iter_mut().enumerate() {
            let value = source_sample_for_output(
                playback,
                source_frame_idx,
                output_channel,
                output_channels,
            );
            *sample = convert((value * playback.volume).clamp(-1., 1.));
        }
        playback.position_frame = (playback.position_frame + 1.).min(total_frames as f64);
    }

    if reached_end {
        playback.position_frame = total_frames as f64;
        if playback.is_playing {
            playback.is_playing = false;
            shared.mark_track_end();
        }
    }
}

fn fill_output_with_rubato<T, F>(
    data: &mut [T],
    output_channels: usize,
    output_sample_rate: u32,
    playback: &mut PlaybackData,
    total_frames: usize,
    render_state: &mut RenderState,
    shared: &SharedPlayback,
    convert: &F,
) where
    T: Copy,
    F: Fn(f32) -> T,
{
    let mut cursor_changed = render_state.observed_cursor_version != Some(playback.cursor_version);
    render_state.observed_cursor_version = Some(playback.cursor_version);

    let needs_new_resampler = match render_state.rubato_resampler.as_ref() {
        Some(resampler) => {
            !resampler.matches(playback.sample_rate, output_sample_rate, output_channels)
        }
        None => true,
    };
    if needs_new_resampler {
        match RubatoStreamResampler::new(playback.sample_rate, output_sample_rate, output_channels)
        {
            Ok(resampler) => {
                render_state.rubato_resampler = Some(resampler);
                cursor_changed = true;
            }
            Err(err) => {
                log::error!("{}", err);
                playback.is_playing = false;
                shared.mark_stream_error(err);
                fill_silence(data, convert);
                return;
            }
        }
    }

    let resampler = render_state.rubato_resampler.as_mut().unwrap();
    if cursor_changed {
        resampler.reset_for_cursor(playback.position_frame);
    }
    if render_state.frame_buffer.len() < output_channels {
        render_state.frame_buffer.resize(output_channels, 0.);
    }

    let step = playback.sample_rate as f64 / output_sample_rate as f64;
    let mut reached_end = false;
    let mut process_error: Option<(usize, String)> = None;

    for (frame_idx, frame) in data.chunks_mut(output_channels).enumerate() {
        let mut has_frame = resampler.read_frame(&mut render_state.frame_buffer[..output_channels]);
        let mut can_generate = true;
        let mut attempts = 0;
        while !has_frame && can_generate && attempts < RUBATO_MAX_GENERATE_ATTEMPTS {
            attempts += 1;
            match resampler.process_chunk(playback, total_frames) {
                Ok(has_more) => {
                    can_generate = has_more;
                }
                Err(err) => {
                    process_error = Some((frame_idx, err));
                    break;
                }
            }
            has_frame = resampler.read_frame(&mut render_state.frame_buffer[..output_channels]);
        }

        if process_error.is_some() {
            break;
        }

        if !has_frame {
            reached_end = true;
            for sample in frame {
                *sample = convert(0.);
            }
            continue;
        }

        for (output_channel, sample) in frame.iter_mut().enumerate() {
            let value =
                (render_state.frame_buffer[output_channel] * playback.volume).clamp(-1., 1.);
            *sample = convert(value);
        }
        playback.position_frame = (playback.position_frame + step).min(total_frames as f64);
    }

    if let Some((failed_frame_idx, err)) = process_error {
        log::error!("{}", err);
        playback.is_playing = false;
        shared.mark_stream_error(err);
        let start = failed_frame_idx * output_channels;
        fill_silence(&mut data[start..], convert);
        return;
    }

    if reached_end {
        playback.position_frame = total_frames as f64;
        if playback.is_playing {
            playback.is_playing = false;
            shared.mark_track_end();
        }
    }
}

fn fill_output<T, F>(
    data: &mut [T],
    output_channels: usize,
    output_sample_rate: u32,
    shared: &SharedPlayback,
    render_state: &mut RenderState,
    convert: F,
) where
    T: Copy,
    F: Fn(f32) -> T,
{
    let mut playback = shared.data.lock();

    if output_channels == 0 || output_sample_rate == 0 {
        return;
    }

    if !playback.is_playing
        || playback.samples.is_empty()
        || playback.input_channels == 0
        || playback.sample_rate == 0
    {
        fill_silence(data, &convert);
        return;
    }

    let total_frames = playback.samples.len() / playback.input_channels;
    if total_frames == 0 {
        playback.is_playing = false;
        playback.position_frame = 0.;
        shared.mark_track_end();
        fill_silence(data, &convert);
        return;
    }

    if playback.sample_rate == output_sample_rate {
        render_state.rubato_resampler = None;
        fill_output_without_resampler(
            data,
            output_channels,
            &mut playback,
            total_frames,
            shared,
            &convert,
        );
        return;
    }

    fill_output_with_rubato(
        data,
        output_channels,
        output_sample_rate,
        &mut playback,
        total_frames,
        render_state,
        shared,
        &convert,
    );
}

fn build_output_stream(
    shared: &Arc<SharedPlayback>,
    requested_sr: Option<u32>,
) -> Result<OutputStreamState, String> {
    let device = default_output_device()?;
    let current_device_name =
        device_name(&device).unwrap_or_else(|| "<unknown-device>".to_string());

    let (config, sample_format, selected_sr) = choose_stream_config(&device, requested_sr)?;
    let output_channels = config.channels as usize;

    let shared_for_error = Arc::clone(shared);
    let err_fn = move |err: cpal::StreamError| {
        log::error!("audio stream error: {}", err);
        shared_for_error.mark_stream_error(err.to_string());
    };

    let stream = match sample_format {
        SampleFormat::F32 => {
            let shared = Arc::clone(shared);
            let mut render_state = RenderState::default();
            device.build_output_stream(
                &config,
                move |data: &mut [f32], _| {
                    fill_output(
                        data,
                        output_channels,
                        selected_sr,
                        &shared,
                        &mut render_state,
                        |x| x,
                    );
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let shared = Arc::clone(shared);
            let mut render_state = RenderState::default();
            device.build_output_stream(
                &config,
                move |data: &mut [i16], _| {
                    fill_output(
                        data,
                        output_channels,
                        selected_sr,
                        &shared,
                        &mut render_state,
                        |x| <i16 as Sample>::from_sample(x),
                    );
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let shared = Arc::clone(shared);
            let mut render_state = RenderState::default();
            device.build_output_stream(
                &config,
                move |data: &mut [u16], _| {
                    fill_output(
                        data,
                        output_channels,
                        selected_sr,
                        &shared,
                        &mut render_state,
                        |x| <u16 as Sample>::from_sample(x),
                    );
                },
                err_fn,
                None,
            )
        }
        SampleFormat::I24 => {
            let shared = Arc::clone(shared);
            let mut render_state = RenderState::default();
            device.build_output_stream(
                &config,
                move |data: &mut [cpal::I24], _| {
                    fill_output(
                        data,
                        output_channels,
                        selected_sr,
                        &shared,
                        &mut render_state,
                        |x| <cpal::I24 as Sample>::from_sample(x),
                    );
                },
                err_fn,
                None,
            )
        }
        SampleFormat::U24 => {
            let shared = Arc::clone(shared);
            let mut render_state = RenderState::default();
            device.build_output_stream(
                &config,
                move |data: &mut [cpal::U24], _| {
                    fill_output(
                        data,
                        output_channels,
                        selected_sr,
                        &shared,
                        &mut render_state,
                        |x| <cpal::U24 as Sample>::from_sample(x),
                    );
                },
                err_fn,
                None,
            )
        }
        _ => {
            return Err(format!(
                "Unsupported output sample format from device: {:?}",
                sample_format
            ));
        }
    }
    .map_err(|e| format!("Failed to build output stream: {}", e))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start output stream: {}", e))?;

    log::info!(
        "device: {}, sr: {}, channels: {}, format: {:?}",
        current_device_name,
        selected_sr,
        output_channels,
        sample_format
    );

    Ok(OutputStreamState {
        _stream: stream,
        device_name: current_device_name,
    })
}

pub(super) fn rebuild_stream(
    shared: &Arc<SharedPlayback>,
    requested_sr: Option<u32>,
    stream_state: &mut Option<OutputStreamState>,
    current_error: &mut String,
) -> bool {
    match build_output_stream(shared, requested_sr) {
        Ok(new_stream_state) => {
            *stream_state = Some(new_stream_state);
            set_error(current_error, None)
        }
        Err(err_str) => {
            log::error!("{}", err_str);
            *stream_state = None;
            let mut playback = shared.data.lock();
            playback.is_playing = false;
            set_error(current_error, Some(err_str))
        }
    }
}
