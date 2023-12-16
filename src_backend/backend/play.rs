// #![feature(c_variadic)]
#![allow(dead_code)]

use std::collections::{HashSet, VecDeque};
use std::num::Wrapping;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use color_eyre::eyre::{ensure, Report, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    Device, FrameCount, FromSample, OutputCallbackInfo, Sample, SampleFormat, SizedSample, Stream,
    StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
};
use log::{debug, error, info, warn};
use ndarray::prelude::*;
use parking_lot::{Mutex, RwLock};
use rubato::{InterpolationParameters, InterpolationType, Resampler, SincFixedOut, WindowFunction};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::default;

pub use symphonia::core::probe::Hint;

#[derive(Debug, Clone, Copy, PartialEq)]
struct SampleRequest {
    frame: Option<(Duration, Wrapping<u8>)>,
    speed: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct SampleResult {
    samples: Vec<f32>,
    end_pos: Duration,
    skip_count: Wrapping<u8>,
    done: bool,
}

#[derive(Debug)]
struct DecodingSong {
    song_length: Duration,
    channel_count: usize,

    requests_channel: SyncSender<SampleRequest>,
    samples_channel: Mutex<Receiver<SampleResult>>,
    frames_per_resample: usize,

    buffer: VecDeque<f32>,
    pending_requests: usize,
    done: bool,
    had_output: bool,
    expected_pos: Duration,
    skip_count: Wrapping<u8>,
}

const MAXIMUM_SPEED_ADJUSTMENT_FACTOR: f64 = 2.0;
const MINIMUM_PLAYBACK_SPEED: f64 = 1.0 / MAXIMUM_SPEED_ADJUSTMENT_FACTOR;
const MAXIMUM_PLAYBACK_SPEED: f64 = 1.0 * MAXIMUM_SPEED_ADJUSTMENT_FACTOR;

impl DecodingSong {
    fn new(
        song: &Song,
        initial_pos: Duration,
        player_sample_rate: usize,
        player_channel_count: usize,
        expected_buffer_size: usize,
        initial_playback_speed: f64,
    ) -> Result<DecodingSong> {
        let frames = song.samples.clone();
        let song_channel_count = song.channel_count;
        if player_channel_count != song_channel_count {
            warn!("Playing song with {song_channel_count} channels while the player has {player_channel_count} channels");
        }
        let total_frames = frames[0].len();
        let frames_per_resample = expected_buffer_size / player_channel_count;
        let volume_adjustment = song.volume_adjustment;

        let (rtx, rrx) = mpsc::sync_channel::<SampleRequest>(10);
        let (stx, srx) = mpsc::channel();
        let song_sample_rate = song.sample_rate as u64;
        let song_length = Self::frame_to_duration(total_frames, song_sample_rate);
        let resample_ratio = player_sample_rate as f64 / song.sample_rate as f64;
        let (etx, erx) = mpsc::channel();
        thread::spawn(move || {
            let sinc_len = 128;
            let f_cutoff = 0.925_914_65;
            let params = InterpolationParameters {
                sinc_len,
                f_cutoff,
                interpolation: InterpolationType::Linear,
                oversampling_factor: 2048,
                window: WindowFunction::Blackman2,
            };
            let mut resampler = match SincFixedOut::<f32>::new(
                resample_ratio,
                MAXIMUM_SPEED_ADJUSTMENT_FACTOR,
                params,
                frames_per_resample, // SincFixedOut theoretically always gives us this much each time we process
                player_channel_count,
            ) {
                Ok(resampler) => {
                    etx.send(Ok(())).unwrap();
                    resampler
                }
                Err(e) => {
                    etx.send(Err(e)).unwrap();
                    return;
                }
            };
            let mut input_buffer = resampler.input_buffer_allocate();
            let mut output_buffer = resampler.output_buffer_allocate();

            let mut current_frame = 0;
            let mut skip_count = Wrapping(0);
            let mut last_request_speed = 1.0;
            loop {
                let request = match rrx.recv() {
                    Ok(request) => request,
                    Err(_) => {
                        debug!("Ending resampling thread.");
                        break;
                    }
                };

                // adjust position based on seek
                if let Some((new_pos, new_skip_count)) = request.frame {
                    let new_frame = (song_sample_rate * new_pos.as_secs()
                        + song_sample_rate * new_pos.subsec_nanos() as u64 / 1_000_000_000)
                        as usize;
                    current_frame = new_frame.min(total_frames);
                    skip_count = new_skip_count;
                }

                // adjust the speed if it has changed
                if request.speed != last_request_speed {
                    resampler
                        .set_resample_ratio_relative(1.0 / request.speed)
                        .unwrap();
                    last_request_speed = request.speed;
                }

                // determine which samples to pass in to the converter
                let frames_wanted_by_resampler = resampler.input_frames_next();
                let last_frame = (current_frame + frames_wanted_by_resampler).min(total_frames);
                let frames_we_have = last_frame - current_frame;
                for i in 0..player_channel_count {
                    input_buffer[i].clear();
                    for j in 0..frames_wanted_by_resampler {
                        if current_frame + j < total_frames {
                            input_buffer[i].push(frames[i % song_channel_count][current_frame + j]);
                        } else {
                            input_buffer[i].push(0.0);
                        }
                    }
                }
                current_frame = last_frame;
                let end_pos = Self::frame_to_duration(current_frame, song_sample_rate);

                // resample the frames and convert into interleaved samples
                let processed_samples =
                    match resampler.process_into_buffer(&input_buffer, &mut output_buffer, None) {
                        Ok(()) => {
                            let frame_count = if frames_we_have < frames_wanted_by_resampler {
                                frames_per_resample * frames_we_have / frames_wanted_by_resampler
                            } else {
                                frames_per_resample
                            };
                            let mut samples = vec![0.0; player_channel_count * frame_count];
                            for chan in 0..player_channel_count {
                                if chan < 2 || chan < output_buffer.len() {
                                    for sample in 0..frame_count {
                                        samples[sample * player_channel_count + chan] =
                                            output_buffer[chan % output_buffer.len()][sample]
                                                * volume_adjustment
                                    }
                                };
                            }
                            samples
                        }
                        Err(e) => {
                            error!("Error converting sample rate: {e}");
                            vec![0.0; expected_buffer_size]
                        }
                    };

                // send the data out over the channel
                // Dropping the other end of the channel will cause this to error, which will stop decoding.
                if stx
                    .send(SampleResult {
                        samples: processed_samples,
                        skip_count,
                        end_pos,
                        done: frames_we_have < frames_wanted_by_resampler,
                    })
                    .is_err()
                {
                    debug!("Ending resampling thread.");
                    break;
                }
            }
        });
        erx.recv()??;
        let skip_count = Wrapping(0);
        rtx.send(SampleRequest {
            speed: initial_playback_speed,
            frame: Some((initial_pos, skip_count)),
        })?;
        Ok(DecodingSong {
            song_length,
            channel_count: player_channel_count,
            requests_channel: rtx,
            samples_channel: Mutex::new(srx),
            frames_per_resample,
            buffer: VecDeque::new(),
            pending_requests: 1,
            done: false,
            had_output: false,
            expected_pos: initial_pos,
            skip_count,
        })
    }

    fn read_samples(
        &mut self,
        pos: Duration,
        count: usize,
        playback_speed: f64,
    ) -> (Vec<f32>, Duration, bool) {
        // if they want another position, we're seeking, so reset the buffer
        if pos != self.expected_pos {
            self.had_output = false;
            self.done = false;
            self.buffer.clear();
            self.skip_count += 1;
            self.requests_channel
                .send(SampleRequest {
                    speed: playback_speed,
                    frame: Some((pos, self.skip_count)),
                })
                .unwrap(); // This shouldn't be able to fail unless the thread stops which shouldn't be able to happen.
            self.pending_requests = 1;
        }

        while count
            > self.buffer.len()
                + self.pending_requests * self.frames_per_resample * self.channel_count
        {
            if self
                .requests_channel
                .send(SampleRequest {
                    speed: playback_speed,
                    frame: None,
                })
                .is_err()
            {
                break;
            }

            self.pending_requests += 1;
        }
        let channel = self.samples_channel.lock();
        if !self.done {
            // Fetch samples until there are none left to fetch and we have enough.
            let mut sent_warning = !self.had_output;
            loop {
                let got = channel.try_recv();
                match got {
                    Ok(SampleResult {
                        samples,
                        skip_count,
                        end_pos,
                        done,
                    }) => {
                        if self.skip_count == skip_count {
                            self.pending_requests -= 1;
                            self.buffer.append(&mut (samples).into());
                            self.expected_pos = end_pos;
                            if done {
                                self.done = true;
                                break;
                            }
                            if self.buffer.len() >= count {
                                break;
                            }
                        }
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.done = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => {
                        if self.buffer.len() >= count {
                            break;
                        } else if !sent_warning {
                            warn!("Waiting on resampler, this could cause audio choppyness. If you are a developer and this happens repeatedly in release mode please file an issue on playback-rs.");
                            sent_warning = true;
                        }
                    }
                }
            }
        }
        let mut vec = Vec::new();
        let mut done = false;
        for _i in 0..count {
            if let Some(sample) = self.buffer.pop_front() {
                vec.push(sample);
            } else {
                done = true;
                break;
            }
        }

        (vec, self.expected_pos, done)
    }
    fn frame_to_duration(frame: usize, song_sample_rate: u64) -> Duration {
        let sub_second_samples = frame as u64 % song_sample_rate;
        Duration::new(
            frame as u64 / song_sample_rate,
            (1_000_000_000 * sub_second_samples / song_sample_rate) as u32,
        )
    }
}

type PlaybackState = (DecodingSong, Duration);

#[derive(Clone)]
struct PlayerState {
    playback: Arc<RwLock<Option<PlaybackState>>>,
    next_samples: Arc<RwLock<Option<PlaybackState>>>,
    playing: Arc<RwLock<bool>>,
    channel_count: usize,
    sample_rate: usize,
    buffer_size: u32,
    playback_speed: Arc<RwLock<f64>>,
}

impl PlayerState {
    fn new(channel_count: u32, sample_rate: u32, buffer_size: FrameCount) -> Result<PlayerState> {
        Ok(PlayerState {
            playback: Arc::new(RwLock::new(None)),
            next_samples: Arc::new(RwLock::new(None)),
            playing: Arc::new(RwLock::new(false)),
            channel_count: channel_count as usize,
            sample_rate: sample_rate as usize,
            buffer_size,
            playback_speed: Arc::new(RwLock::new(1.0)),
        })
    }
    fn write_samples<T>(&self, data: &mut [T], _info: &OutputCallbackInfo)
    where
        T: Sample + FromSample<f32>,
    {
        for sample in data.iter_mut() {
            *sample = Sample::EQUILIBRIUM;
        }
        if *self.playing.read() {
            let playback_speed = *self.playback_speed.read();
            let mut playback = self.playback.write();
            if playback.is_none() {
                if let Some((new_samples, new_pos)) = self.next_samples.write().take() {
                    *playback = Some((new_samples, new_pos));
                }
            }
            let mut done = false;
            if let Some((decoding_song, sample_pos)) = playback.as_mut() {
                let mut neg_offset = 0;
                let data_len = data.len();
                let (mut samples, mut new_pos, mut is_final) =
                    decoding_song.read_samples(*sample_pos, data_len, playback_speed);
                for (i, sample) in data.iter_mut().enumerate() {
                    if i >= samples.len() {
                        if let Some((next_samples, next_pos)) = self.next_samples.write().take() {
                            *decoding_song = next_samples;
                            neg_offset = i;
                            *sample_pos = next_pos;
                            (samples, new_pos, is_final) = decoding_song.read_samples(
                                *sample_pos,
                                data_len - neg_offset,
                                playback_speed,
                            );
                        } else {
                            break;
                        }
                    }
                    *sample = T::from_sample(samples[i - neg_offset]);
                }
                *sample_pos = new_pos;
                done = is_final;
            }
            if done {
                *playback = None;
            }
        }
    }
    fn decode_song(&self, song: &Song, initial_pos: Duration) -> Result<DecodingSong> {
        DecodingSong::new(
            song,
            initial_pos,
            self.sample_rate,
            self.channel_count,
            self.buffer_size as usize,
            *self.playback_speed.read(),
        )
    }
    fn set_playback_speed(&self, speed: f64) {
        *self.playback_speed.write() = speed.clamp(MINIMUM_PLAYBACK_SPEED, MAXIMUM_PLAYBACK_SPEED);
    }
    fn stop(&self) {
        *self.next_samples.write() = None;
        *self.playback.write() = None;
    }
    fn skip(&self) {
        *self.playback.write() = None;
    }
    fn play_song(&self, song: &Song, time: Option<Duration>) -> Result<()> {
        let initial_pos = time.unwrap_or_default();
        let samples = self.decode_song(song, initial_pos)?;
        *self.next_samples.write() = Some((samples, initial_pos));
        Ok(())
    }
    fn set_playing(&self, playing: bool) {
        *self.playing.write() = playing;
    }
    fn get_position(&self) -> Option<(Duration, Duration)> {
        self.playback
            .read()
            .as_ref()
            .map(|(samples, pos)| (*pos, samples.song_length))
    }
    fn seek(&self, time: Duration) -> bool {
        let (mut playback, mut next_song) = (self.playback.write(), self.next_samples.write());
        if let Some((_, pos)) = playback.as_mut() {
            *pos = time;
            true
        } else if let Some((_, pos)) = next_song.as_mut() {
            *pos = time;
            true
        } else {
            false
        }
    }
    fn force_remove_next_song(&self) {
        let (mut playback, mut next_song) = (self.playback.write(), self.next_samples.write());
        if next_song.is_some() {
            *next_song = None;
        } else {
            *playback = None;
        }
    }
}

/// Manages playback of [Song]s through [cpal] and sample conversion through [rubato].
#[readonly::make]
pub struct Player {
    _stream: Box<dyn StreamTrait>,
    player_state: PlayerState,
    pub supported_sample_rates: HashSet<u32>,
}

impl Player {
    /// Creates a new [Player] to play [Song]s. If specified, the player will attempt to use one of
    /// the specified sampling rates. If not specified or the list is empty, the preferred rates
    /// are 48000 and 44100.
    ///
    /// If none of the preferred sampling rates are available, the closest available rate to the
    /// first preferred rate will be selected.
    ///
    /// On Linux, this prefers `pipewire`, `jack`, and `pulseaudio` devices over `alsa`.
    pub fn new(preferred_sampling_rates: Option<Vec<u32>>) -> Result<Player> {
        let device = {
            let mut selected_host = cpal::default_host();
            for host in cpal::available_hosts() {
                if host.name().to_lowercase().contains("jack") {
                    selected_host = cpal::host_from_id(host)?;
                }
            }
            info!("Selected Host: {:?}", selected_host.id());
            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd"
            ))]
            {
                if selected_host.id() == HostId::Alsa {
                    block_alsa_output();
                }
            }
            let mut selected_device = selected_host
                .default_output_device()
                .ok_or_else(|| Report::msg("No output device found."))?;
            for device in selected_host.output_devices()? {
                if let Ok(name) = device.name().map(|s| s.to_lowercase()) {
                    if name.contains("pipewire") || name.contains("pulse") || name.contains("jack")
                    {
                        selected_device = device;
                    }
                }
            }
            info!(
                "Selected Device: {}",
                selected_device
                    .name()
                    .unwrap_or_else(|_| "Unknown".to_string())
            );
            selected_device
        };
        let mut supported_configs = device.supported_output_configs()?.collect::<Vec<_>>();
        let supported_sample_rates = supported_configs
            .iter()
            .flat_map(|c| (c.min_sample_rate().0..=c.max_sample_rate().0))
            .collect();

        let preferred_sampling_rates = preferred_sampling_rates
            .filter(|given_rates| !given_rates.is_empty())
            .unwrap_or(vec![48000, 44100]);
        let preferred_sampling_rate = preferred_sampling_rates[0];
        let rank_supported_config = |config: &SupportedStreamConfigRange| {
            let chans = config.channels() as u32;
            let channel_rank = match chans {
                0 => 0,
                1 => 1,
                2 => 4,
                4 => 3,
                _ => 2,
            };
            let min_sample_rank = if config.min_sample_rate().0 <= preferred_sampling_rate {
                3
            } else {
                0
            };
            let max_sample_rank = if config.max_sample_rate().0 >= preferred_sampling_rate {
                3
            } else {
                0
            };
            let sample_format_rank = if config.sample_format() == SampleFormat::F32 {
                4
            } else {
                0
            };
            channel_rank + min_sample_rank + max_sample_rank + sample_format_rank
        };
        supported_configs.sort_by_key(|c_2| std::cmp::Reverse(rank_supported_config(c_2)));

        let supported_config = supported_configs
            .into_iter()
            .next()
            .ok_or_else(|| Report::msg("No supported output config."))?;

        let sample_rate_range =
            supported_config.min_sample_rate().0..supported_config.max_sample_rate().0;
        let supported_config = if let Some(selected_rate) = preferred_sampling_rates
            .into_iter()
            .find(|rate| sample_rate_range.contains(rate))
        {
            supported_config.with_sample_rate(cpal::SampleRate(selected_rate))
        } else if sample_rate_range.end <= preferred_sampling_rate {
            supported_config.with_sample_rate(cpal::SampleRate(sample_rate_range.end))
        } else {
            supported_config.with_sample_rate(cpal::SampleRate(sample_rate_range.start))
        };
        let sample_format = supported_config.sample_format();
        let sample_rate = supported_config.sample_rate().0;
        let channel_count = supported_config.channels();
        let buffer_size = match supported_config.buffer_size() {
            SupportedBufferSize::Range { min, .. } => (*min).max(1024) * 2,
            SupportedBufferSize::Unknown => 1024 * 2,
        };
        let config = supported_config.into();
        let player_state = PlayerState::new(channel_count as u32, sample_rate, buffer_size)?;
        info!(
            "SR, CC, SF: {}, {}, {:?}",
            sample_rate, channel_count, sample_format
        );
        fn build_stream<T>(
            device: &Device,
            config: &StreamConfig,
            player_state: PlayerState,
        ) -> Result<Stream>
        where
            T: SizedSample + FromSample<f32>,
        {
            let err_fn = |err| error!("A playback error has occurred! {}", err);
            let stream = device.build_output_stream(
                config,
                move |data, info| player_state.write_samples::<T>(data, info),
                err_fn,
                None,
            )?;
            // Not all platforms (*cough cough* windows *cough*) automatically run the stream upon creation, so do that here.
            stream.play()?;
            Ok(stream)
        }
        let stream = {
            let player_state = player_state.clone();
            match sample_format {
                SampleFormat::I8 => build_stream::<i8>(&device, &config, player_state)?,
                SampleFormat::I16 => build_stream::<i16>(&device, &config, player_state)?,
                SampleFormat::I32 => build_stream::<i32>(&device, &config, player_state)?,
                SampleFormat::I64 => build_stream::<i64>(&device, &config, player_state)?,
                SampleFormat::U8 => build_stream::<u8>(&device, &config, player_state)?,
                SampleFormat::U16 => build_stream::<u16>(&device, &config, player_state)?,
                SampleFormat::U32 => build_stream::<u32>(&device, &config, player_state)?,
                SampleFormat::U64 => build_stream::<u64>(&device, &config, player_state)?,
                SampleFormat::F32 => build_stream::<f32>(&device, &config, player_state)?,
                SampleFormat::F64 => build_stream::<f64>(&device, &config, player_state)?,
                sample_format => Err(Report::msg(format!(
                    "Unsupported sample format '{sample_format}'"
                )))?,
            }
        };

        Ok(Player {
            _stream: Box::new(stream),
            player_state,
            supported_sample_rates,
        })
    }
    pub fn sr(&self) -> usize {
        self.player_state.sample_rate
    }
    /// Set the playback speed (This will also affect song pitch)
    pub fn set_playback_speed(&self, speed: f64) {
        self.player_state.set_playback_speed(speed);
    }
    /// Set the song that will play after the current song is over (or immediately if no song is currently playing), optionally start playing in the middle of the song.
    pub fn play_song_next(&self, song: &Song, start_time: Option<Duration>) -> Result<()> {
        self.player_state.play_song(song, start_time)
    }
    /// Start playing a song immediately, while discarding any song that might have been queued to play next. Optionally start playing in the middle of the song.
    pub fn play_song_now(&self, song: &Song, start_time: Option<Duration>) -> Result<()> {
        self.player_state.stop();
        self.player_state.play_song(song, start_time)?;
        Ok(())
    }
    /// Used to replace the next song, or the current song if there is no next song. Optionally start playing in the middle of the song.
    ///
    /// This will remove the current song if no next song exists to avoid a race condition in case the current song ends after you have determined that the next song must be replaced but before you call this function.
    /// See also [`force_remove_next_song`](Player::force_remove_next_song)
    pub fn force_replace_next_song(&self, song: &Song, start_time: Option<Duration>) -> Result<()> {
        self.player_state.force_remove_next_song();
        self.player_state.play_song(song, start_time)?;
        Ok(())
    }
    /// Used to remove the next song, or the current song if there is no next song.
    ///
    /// This will remove the current song if no next song exists to avoid a race condition in case the current song ends after you have determined that the next song must be replaced but before you call this function.
    /// See also [`force_replace_next_song`](Player::force_replace_next_song)
    pub fn force_remove_next_song(&self) -> Result<()> {
        self.player_state.force_remove_next_song();
        Ok(())
    }
    /// Stop playing any songs and remove a next song if it has been queued.
    ///
    /// Note that this does not pause playback (use [`set_playing`](Player::set_playing)), meaning new songs will play upon adding them.
    pub fn stop(&self) {
        self.player_state.stop();
    }
    /// Skip the currently playing song (i.e. stop playing it immediately.
    ///
    /// This will immediately start playing the next song if it exists.
    pub fn skip(&self) {
        self.player_state.skip();
    }
    /// Return the current playback position, if there is currently a song playing (see [`has_current_song`](Player::has_current_song))
    ///
    /// See also [`seek`](Player::seek)
    pub fn get_playback_position(&self) -> Option<(Duration, Duration)> {
        self.player_state.get_position()
    }
    /// Set the current playback position if there is a song playing or a song queued to be played next.
    ///
    /// Returns whether the seek was successful (whether there was a song to seek).
    /// Note that seeking past the end of the song will be successful and will cause playback to begin at the _beginning_ of the next song.
    ///
    /// See also [`get_playback_position`](Player::get_playback_position)
    pub fn seek(&self, time: Duration) -> bool {
        self.player_state.seek(time)
    }
    /// Sets whether playback is enabled or not, without touching the song queue.
    ///
    /// See also [`is_playing`](Player::is_playing)
    pub fn set_playing(&self, playing: bool) {
        self.player_state.set_playing(playing);
    }
    /// Returns whether playback is currently paused.
    ///
    /// See also [`set_playing`](Player::set_playing)
    pub fn is_playing(&self) -> bool {
        *self.player_state.playing.read()
    }
    /// Returns whether there is a song queued to play next after the current song has finished
    ///
    /// If you want to check whether there is currently a song playing, use [`has_current_song`][Player::has_current_song] and [`is_playing`][Player::is_playing].
    /// This should always be queried before calling [`play_song_next`](Player::play_song_next) if you do not intend on replacing the song currently in the queue.
    pub fn has_next_song(&self) -> bool {
        self.player_state.next_samples.read().is_some()
    }
    /// Returns whether there is a song currently playing (or about to start playing next audio frame)
    ///
    /// Note that this **does not** indicate whether the current song is actively being played or paused, for that functionality you can use [is_playing](Self::is_playing).
    pub fn has_current_song(&self) -> bool {
        self.player_state.playback.read().is_some()
            || self.player_state.next_samples.read().is_some()
    }
}

/// Represents a single song that has been decoded into memory, can be played in a <Player> struct.
///
/// The data in the song is stored in an <Arc> so cloning a song is a lightweight operation.
#[derive(Debug, Clone)]
pub struct Song {
    samples: Arc<Vec<Vec<f32>>>,
    sample_rate: u32,
    channel_count: usize,
    volume_adjustment: f32,
}

impl Song {
    pub fn new(samples: ArrayView2<f32>, sample_rate: u32, volume_adjustment: Option<f32>) -> Self {
        let channel_count = samples.shape()[0];
        let samples_vec = samples.axis_iter(Axis(0)).map(|x| x.to_vec()).collect();

        Song {
            samples: Arc::new(samples_vec),
            sample_rate,
            channel_count,
            volume_adjustment: volume_adjustment.unwrap_or(1.0),
        }
    }

    /// Creates a new song using a reader of some kind and a type hint (the Symphonia hint type has been reexported at the crate root for convenience), as well as an optional volume adjustment (used for e.g. replay gain).
    pub fn from_reader(
        reader: Box<dyn MediaSource>,
        hint: &Hint,
        volume_adjustment: Option<f32>,
    ) -> Result<Song> {
        let media_source_stream =
            MediaSourceStream::new(reader, MediaSourceStreamOptions::default());
        let mut probe_result = default::get_probe().format(
            hint,
            media_source_stream,
            &FormatOptions {
                enable_gapless: true,
                ..FormatOptions::default()
            },
            &MetadataOptions::default(),
        )?;
        let mut decoder = default::get_codecs().make(
            &probe_result
                .format
                .default_track()
                .ok_or_else(|| Report::msg("No default track in media file."))?
                .codec_params,
            &DecoderOptions::default(),
        )?;
        let mut song: Option<(Vec<Vec<f32>>, u32, usize)> = None;
        let mut bad_packet = false;
        loop {
            match probe_result.format.next_packet() {
                Ok(packet) => {
                    let decoded = match decoder.decode(&packet) {
                        Ok(decoded) => decoded,
                        Err(symphonia::core::errors::Error::DecodeError(err)) => {
                            // The example playback code doesn't treat decode errors as fatal errors,
                            // so just log this at most once per file.
                            if !bad_packet {
                                bad_packet = true;
                                warn!("Bad packet: {err:?}");
                            }
                            continue;
                        }
                        Err(err) => {
                            return Err(Report::new(err));
                        }
                    };
                    let spec = *decoded.spec();
                    let song_samples =
                        if let Some((samples, sample_rate, channel_count)) = &mut song {
                            ensure!(
                                spec.rate == *sample_rate,
                                "Sample rate of decoded does not match previous sample rate."
                            );
                            ensure!(
                                spec.channels.count() == *channel_count,
                                "Channel count of decoded does not match previous channel count."
                            );
                            samples
                        } else {
                            song = Some((
                                vec![Vec::new(); spec.channels.count()],
                                spec.rate,
                                spec.channels.count(),
                            ));
                            &mut song.as_mut().unwrap().0
                        };
                    if decoded.frames() > 0 {
                        let mut samples = SampleBuffer::new(decoded.frames() as u64, spec);
                        samples.copy_interleaved_ref(decoded);
                        for frame in samples.samples().chunks(spec.channels.count()) {
                            for (chan, sample) in frame.iter().enumerate() {
                                song_samples[chan].push(*sample)
                            }
                        }
                    } else {
                        warn!("Empty packet encountered while loading song!");
                    }
                }
                Err(SymphoniaError::IoError(_)) => break,
                Err(e) => return Err(e.into()),
            }
        }
        song.map(|(samples, sample_rate, channel_count)| Song {
            samples: Arc::new(samples),
            sample_rate,
            channel_count,
            volume_adjustment: volume_adjustment.unwrap_or(1.0),
        })
        .ok_or_else(|| Report::msg("No song data decoded."))
    }
    /// Creates a [Song] by reading data from a file and using the file's extension as a format type hint. Takes an optional volume adjustment (used for e.g. replay gain)
    pub fn from_file<P: AsRef<std::path::Path>>(
        path: P,
        volume_adjustment: Option<f32>,
    ) -> Result<Song> {
        let mut hint = Hint::new();
        if let Some(extension) = path.as_ref().extension().and_then(|s| s.to_str()) {
            hint.with_extension(extension);
        }
        Self::from_reader(
            Box::new(std::fs::File::open(path)?),
            &hint,
            volume_adjustment,
        )
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd"
))]
fn block_alsa_output() {
    use std::os::raw::{c_char, c_int};

    use alsa_sys::snd_lib_error_set_handler;
    use log::trace;

    unsafe extern "C" fn error_handler(
        file: *const c_char,
        line: c_int,
        function: *const c_char,
        err: c_int,
        format: *const c_char,
        mut format_args: ...
    ) {
        use std::ffi::CStr;
        let file = String::from_utf8_lossy(CStr::from_ptr(file).to_bytes());
        let function = String::from_utf8_lossy(CStr::from_ptr(function).to_bytes());
        let format = String::from_utf8_lossy(CStr::from_ptr(format).to_bytes());
        // FIXME: This should really be better, but it works for alsa so
        let mut last_m = 0;
        let formatted: String = format
            .match_indices("%s")
            .flat_map(|(m, s)| {
                let res = [
                    format[last_m..m].to_string(),
                    String::from_utf8_lossy(
                        CStr::from_ptr(format_args.arg::<*const c_char>()).to_bytes(),
                    )
                    .to_string(),
                ];
                last_m = m + s.len();
                res
            })
            .collect();
        trace!("ALSA Error: {err}: {file} ({line}): {function}: {formatted}");
    }

    unsafe {
        snd_lib_error_set_handler(Some(error_handler));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use approx::assert_abs_diff_eq;

    use super::super::audio::open_audio_file;
    use super::*;

    #[test]
    fn play_works() {
        let (wavs1, sr, _) = open_audio_file("samples/sample_48k.wav").unwrap();
        let (wavs2, _, _) = open_audio_file("samples/stereo/sample_48k.wav").unwrap();
        // wav.slice_collapse(s![.., ..2 * 48000]);
        let duration = wavs1.shape()[1] as f32 / sr as f32;
        let player = Player::new(Some(vec![sr])).unwrap();
        let song1 = Song::new(wavs1.view(), sr, None);
        let song2 = Song::new(wavs2.view(), sr, None);
        player.play_song_now(&song1, None).unwrap();
        player.set_playing(true);
        player
            .play_song_next(&song2, Some(Duration::new(5, 0)))
            .unwrap();
        assert_eq!(player.has_current_song(), true);
        while player.has_current_song() {
            if let Some(playback_pos) = player.get_playback_position() {
                println!("{:?}", playback_pos);
                assert!(playback_pos.0 <= playback_pos.1);
                // assert_abs_diff_eq!(duration, playback_pos.1.as_secs_f32());
                if playback_pos.0 > Duration::new(10, 0) {
                    player.skip();
                }
            } else {
                println!("None");
            }
            std::thread::sleep(Duration::new(0, 1000_000_000 / 60));
        }
        assert_eq!(player.is_playing(), true);
        player.set_playing(false);
        assert_eq!(player.is_playing(), false);
    }
}
