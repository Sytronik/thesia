use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::TRACK_LIST;

#[derive(Default)]
pub(super) struct SharedPlayback {
    pub(super) data: Mutex<PlaybackData>,
    reached_track_end: AtomicBool,
    stream_error: Mutex<Option<String>>,
}

impl SharedPlayback {
    pub(super) fn mark_stream_error(&self, err: String) {
        *self.stream_error.lock() = Some(err);
    }

    pub(super) fn take_stream_error(&self) -> Option<String> {
        self.stream_error.lock().take()
    }

    pub(super) fn mark_track_end(&self) {
        self.reached_track_end.store(true, Ordering::Release);
    }

    pub(super) fn take_track_end(&self) -> bool {
        self.reached_track_end.swap(false, Ordering::AcqRel)
    }

    pub(super) fn clear_track_end(&self) {
        self.reached_track_end.store(false, Ordering::Release);
    }
}

#[derive(Clone)]
pub(super) struct PlaybackData {
    pub(super) track_id: Option<usize>,
    pub(super) samples: Vec<f32>,
    pub(super) input_channels: usize,
    pub(super) sample_rate: u32,
    pub(super) position_frame: f64,
    pub(super) cursor_version: u64,
    pub(super) volume: f32,
    pub(super) is_playing: bool,
}

impl Default for PlaybackData {
    fn default() -> Self {
        Self {
            track_id: None,
            samples: Vec::new(),
            input_channels: 0,
            sample_rate: 0,
            position_frame: 0.,
            cursor_version: 0,
            volume: 1.,
            is_playing: false,
        }
    }
}

#[derive(Clone)]
struct PlayerStateSnapshot {
    is_playing: bool,
    position_sec: f64,
    track_id: Option<u32>,
    err: String,
}

impl PlayerStateSnapshot {
    fn is_same_state(&self, other: &Self, elapsed_since_self: Duration) -> bool {
        if self.is_playing != other.is_playing
            || self.track_id != other.track_id
            || self.err != other.err
        {
            return false;
        }

        let expected_position_sec = if self.is_playing {
            self.position_sec + elapsed_since_self.as_secs_f64()
        } else {
            self.position_sec
        };

        (expected_position_sec - other.position_sec).abs() < 1e-3
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayerStateEvent {
    is_playing: bool,
    position_sec: f64,
    event_time_ms: u64,
    track_id: Option<u32>,
    err: String,
}

impl From<PlayerStateSnapshot> for PlayerStateEvent {
    fn from(value: PlayerStateSnapshot) -> Self {
        Self {
            is_playing: value.is_playing,
            position_sec: value.position_sec,
            event_time_ms: now_millis(),
            track_id: value.track_id,
            err: value.err,
        }
    }
}

#[derive(Default)]
pub(super) struct StateEmitter {
    last_snapshot: Option<PlayerStateSnapshot>,
    last_emit_at: Option<Instant>,
}

impl StateEmitter {
    pub(super) fn emit_if_changed(&mut self, app: &AppHandle, shared: &SharedPlayback, err: &str) {
        let snapshot = snapshot_state(shared, err);
        if let Some(prev) = self.last_snapshot.as_ref() {
            let elapsed = self.last_emit_at.map_or(Duration::ZERO, |t| t.elapsed());
            if prev.is_same_state(&snapshot, elapsed) {
                return;
            }
        }
        self.last_snapshot = Some(snapshot.clone());
        self.last_emit_at = Some(Instant::now());

        let event = PlayerStateEvent::from(snapshot);
        if let Err(e) = app.emit("player-state-changed", event) {
            log::error!("failed to emit player-state-changed: {}", e);
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) fn position_sec(playback: &PlaybackData) -> f64 {
    if playback.samples.is_empty() || playback.input_channels == 0 || playback.sample_rate == 0 {
        return 0.;
    }

    let total_frames = playback.samples.len() / playback.input_channels;
    playback.position_frame.clamp(0., total_frames as f64) / playback.sample_rate as f64
}

fn snapshot_state(shared: &SharedPlayback, err: &str) -> PlayerStateSnapshot {
    let playback = shared.data.lock();
    PlayerStateSnapshot {
        is_playing: playback.is_playing,
        position_sec: position_sec(&playback),
        track_id: playback.track_id.map(|x| x as u32),
        err: err.to_string(),
    }
}

pub(super) fn set_error(current_error: &mut String, maybe_err: Option<String>) -> bool {
    match maybe_err {
        Some(err) => {
            if *current_error == err {
                false
            } else {
                *current_error = err;
                true
            }
        }
        None => {
            if current_error.is_empty() {
                false
            } else {
                current_error.clear();
                true
            }
        }
    }
}

pub(super) fn set_track(
    shared: &Arc<SharedPlayback>,
    track_id: Option<usize>,
    start_time_sec: f64,
    is_playing: bool,
) {
    let current_track_id = shared.data.lock().track_id;
    let target_track_id = track_id.or(current_track_id);

    let loaded_track = target_track_id.and_then(|id| {
        TRACK_LIST.read().get(id).map(|track| {
            (
                id,
                track.interleaved_samples(),
                track.n_ch(),
                track.sr(),
                track.sec(),
            )
        })
    });

    let mut playback = shared.data.lock();
    match loaded_track {
        Some((track_id, samples, channels, sample_rate, max_sec)) => {
            let start_sec = start_time_sec.clamp(0., max_sec.max(0.));
            playback.track_id = Some(track_id);
            playback.samples = samples;
            playback.input_channels = channels;
            playback.sample_rate = sample_rate;
            playback.position_frame = start_sec * sample_rate as f64;
            playback.cursor_version = playback.cursor_version.wrapping_add(1);
            playback.is_playing = is_playing;
            log::info!("set track {}", track_id);
        }
        None => {
            playback.track_id = None;
            playback.samples.clear();
            playback.input_channels = 0;
            playback.sample_rate = 0;
            playback.position_frame = 0.;
            playback.cursor_version = playback.cursor_version.wrapping_add(1);
            playback.is_playing = false;
            log::info!("clear track");
        }
    }
}

pub(super) fn seek(shared: &Arc<SharedPlayback>, sec: f64) {
    let sec = sec.clamp(0., TRACK_LIST.read().max_sec.max(0.));
    let mut playback = shared.data.lock();

    if playback.sample_rate == 0 || playback.input_channels == 0 || playback.samples.is_empty() {
        playback.position_frame = 0.;
        return;
    }

    let total_frames = playback.samples.len() / playback.input_channels;
    let max_sec = total_frames as f64 / playback.sample_rate as f64;
    let sec = sec.min(max_sec);
    playback.position_frame = sec * playback.sample_rate as f64;
    playback.cursor_version = playback.cursor_version.wrapping_add(1);

    log::info!("seek to {}", sec);
}

pub(super) fn pause(shared: &Arc<SharedPlayback>) {
    shared.data.lock().is_playing = false;
    log::info!("pause");
}

pub(super) fn resume(shared: &Arc<SharedPlayback>) {
    let mut playback = shared.data.lock();
    if playback.track_id.is_some() && !playback.samples.is_empty() {
        playback.is_playing = true;
    }
    log::info!("resume");
}
