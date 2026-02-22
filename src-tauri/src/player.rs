mod device;
mod state;
mod stream;

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use log::{info, warn};
use tauri::AppHandle;
use tauri::async_runtime::spawn_blocking;
use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::DeciBel;
use device::default_output_device_name;
use state::{
    SharedPlayback, StateEmitter, pause, position_sec, resume, seek, set_error, set_track,
};
use stream::{OutputStreamState, rebuild_stream};

pub const PLAY_JUMP_SEC: f64 = 1.0;
pub const PLAY_BIG_JUMP_SEC: f64 = 5.0;

const PLAYER_LOOP_INTERVAL: Duration = Duration::from_millis(20);
const DEVICE_CHECK_INTERVAL: Duration = Duration::from_millis(500);

static COMMAND_TX: OnceLock<mpsc::Sender<PlayerCommand>> = OnceLock::new();

pub enum PlayerCommand {
    /// only caused by refreshing of frontend
    Initialize,
    /// set volume
    SetVolumedB(f64),
    /// if zero, the default sr is used
    SetSr(u32),
    /// arg: (optional track_id, optional start_time (sec))
    /// if track_id is None, the current track is reloaded
    SetTrack((Option<usize>, Option<f64>)),
    /// arg: time (sec)
    Seek(f64),
    /// pause playing
    Pause,
    /// resume playing
    Resume,
}

pub async fn send(msg: PlayerCommand) {
    let msg_tx = COMMAND_TX.get().unwrap().clone();
    if let Err(e) = msg_tx.send(msg).await {
        panic!("PLAYER MSG_TX error: {}", e);
    }
}

fn main_loop(mut msg_rx: mpsc::Receiver<PlayerCommand>, app: AppHandle) {
    let shared = Arc::new(SharedPlayback::default());

    let mut requested_sr: Option<u32> = None;
    let mut stream_state: Option<OutputStreamState> = None;
    let mut current_error = String::new();
    let mut state_emitter = StateEmitter::default();
    let mut last_device_check = Instant::now();

    let mut should_emit =
        rebuild_stream(&shared, requested_sr, &mut stream_state, &mut current_error);

    loop {
        loop {
            match msg_rx.try_recv() {
                Ok(msg) => match msg {
                    PlayerCommand::Initialize => {
                        should_emit |= rebuild_stream(
                            &shared,
                            requested_sr,
                            &mut stream_state,
                            &mut current_error,
                        );
                    }
                    PlayerCommand::SetSr(sr) => {
                        let new_requested_sr = (sr != 0).then_some(sr);
                        if new_requested_sr != requested_sr || stream_state.is_none() {
                            requested_sr = new_requested_sr;
                            should_emit |= rebuild_stream(
                                &shared,
                                requested_sr,
                                &mut stream_state,
                                &mut current_error,
                            );
                        } else {
                            info!("sr no change");
                        }
                    }
                    #[allow(non_snake_case)]
                    PlayerCommand::SetVolumedB(volume_dB) => {
                        let volume = volume_dB.amp_from_dB_default() as f32;
                        shared.data.lock().volume = volume;
                    }
                    PlayerCommand::SetTrack((track_id, start_time)) => {
                        let (current_position, is_playing) = {
                            let playback = shared.data.lock();
                            (position_sec(&playback), playback.is_playing)
                        };
                        let start_time = start_time.unwrap_or(current_position);
                        set_track(&shared, track_id, start_time, is_playing);
                        shared.clear_track_end();
                        should_emit = true;
                    }
                    PlayerCommand::Seek(sec) => {
                        seek(&shared, sec);
                        shared.clear_track_end();
                        should_emit = true;
                    }
                    PlayerCommand::Pause => {
                        pause(&shared);
                        should_emit = true;
                    }
                    PlayerCommand::Resume => {
                        resume(&shared);
                        should_emit = true;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        if shared.take_track_end() {
            info!("track ended");
            should_emit = true;
        }

        if let Some(stream_error) = shared.take_stream_error() {
            should_emit |= set_error(&mut current_error, Some(stream_error));
            should_emit |=
                rebuild_stream(&shared, requested_sr, &mut stream_state, &mut current_error);
        }

        if last_device_check.elapsed() >= DEVICE_CHECK_INTERVAL {
            last_device_check = Instant::now();

            if let Some(stream) = &stream_state {
                match default_output_device_name() {
                    Some(device_name) if device_name != stream.device_name => {
                        warn!(
                            "default output device changed: {} -> {}",
                            stream.device_name, device_name
                        );
                        should_emit |= rebuild_stream(
                            &shared,
                            requested_sr,
                            &mut stream_state,
                            &mut current_error,
                        );
                    }
                    Some(_) => {}
                    None => {
                        should_emit |= set_error(
                            &mut current_error,
                            Some("No default output device available".to_string()),
                        );
                        stream_state = None;
                        shared.data.lock().is_playing = false;
                    }
                }
            } else {
                should_emit |=
                    rebuild_stream(&shared, requested_sr, &mut stream_state, &mut current_error);
            }
        }

        if should_emit {
            state_emitter.emit_if_changed(&app, &shared, &current_error);
            should_emit = false;
        }

        std::thread::sleep(PLAYER_LOOP_INTERVAL);
    }
}

pub fn spawn_task(app: AppHandle) {
    if COMMAND_TX.get().is_some_and(|cmd_tx| !cmd_tx.is_closed()) {
        COMMAND_TX
            .get()
            .unwrap()
            .blocking_send(PlayerCommand::Initialize)
            .unwrap();
        return;
    }

    let (command_tx, command_rx) = mpsc::channel::<PlayerCommand>(20);
    COMMAND_TX.set(command_tx).unwrap();
    spawn_blocking(move || main_loop(command_rx, app));
}
