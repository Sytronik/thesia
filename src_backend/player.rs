use std::cell::RefCell;
use std::sync::OnceLock;
use std::sync::atomic::{self, AtomicU32, AtomicUsize};
use std::time::{Duration, Instant};

use atomic_float::AtomicF32;
use cpal::{SupportedStreamConfigsError, traits::DeviceTrait};
use kittyaudio::{Device, KaError, Mixer, Sound, SoundHandle, StreamSettings};
use log::{error, info};
use napi::bindgen_prelude::spawn_blocking;
use napi::tokio::sync::mpsc::{self, error::TryRecvError};
use napi::tokio::sync::watch;

use crate::{DeciBel, TRACK_LIST};

const PLAYER_NOTI_INTERVAL: Duration = Duration::from_millis(100);

static COMMAND_TX: OnceLock<mpsc::Sender<PlayerCommand>> = OnceLock::new();
static NOTI_RX: OnceLock<watch::Receiver<PlayerNotification>> = OnceLock::new();

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

#[derive(Clone, Debug)]
pub struct InternalPlayerState {
    /// if currently playing
    pub is_playing: bool,
    /// playing position (sec)
    pub position_sec: f64,
    /// timestamp when this state is created
    pub instant: Instant,
}

impl InternalPlayerState {
    pub fn position_sec_elapsed(&self) -> f64 {
        if self.is_playing {
            self.position_sec + self.instant.elapsed().as_secs_f64()
        } else {
            self.position_sec
        }
    }
}

impl Default for InternalPlayerState {
    fn default() -> Self {
        InternalPlayerState {
            is_playing: false,
            position_sec: 0.,
            instant: Instant::now(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum PlayerNotification {
    Ok(InternalPlayerState),
    Err(String),
}

pub async fn send(msg: PlayerCommand) {
    let msg_tx = COMMAND_TX.get().unwrap().clone();
    if let Err(e) = msg_tx.send(msg).await {
        panic!("PLAYER MSG_TX error: {}", e);
    }
}

pub fn recv() -> PlayerNotification {
    let noti_rx = NOTI_RX.get().unwrap();
    let noti = (noti_rx.borrow()).clone();
    match noti {
        PlayerNotification::Ok(mut state) if state.is_playing => {
            state.position_sec = state.position_sec_elapsed();
            PlayerNotification::Ok(state)
        }
        _ => noti,
    }
}

fn get_supported_sr_list(device_name: &str) -> Result<Vec<u32>, KaError> {
    if let Device::Custom(device) = Device::from_name(device_name)? {
        match device.supported_output_configs() {
            Ok(supported_configs) => Ok(supported_configs
                .flat_map(|c| (c.min_sample_rate().0..=c.max_sample_rate().0))
                .collect()),
            Err(SupportedStreamConfigsError::DeviceNotAvailable) => Err(KaError::BuildStreamError(
                cpal::BuildStreamError::DeviceNotAvailable,
            )),
            Err(SupportedStreamConfigsError::InvalidArgument) => Err(KaError::BuildStreamError(
                cpal::BuildStreamError::InvalidArgument,
            )),
            Err(SupportedStreamConfigsError::BackendSpecific { err }) => Err(
                KaError::BuildStreamError(cpal::BuildStreamError::BackendSpecific { err }),
            ),
        }
    } else {
        unreachable!();
    }
}

fn get_optimal_sr(device_name: &str, sr: u32) -> Result<Option<u32>, KaError> {
    if sr == 0 {
        return Ok(None);
    }
    let supported_sr_list = get_supported_sr_list(device_name)?;
    info!("supported sr: {:?}", supported_sr_list);
    if supported_sr_list.contains(&sr) {
        return Ok(Some(sr));
    }
    let (closest_greater, closest_less) = supported_sr_list.into_iter().fold(
        (u32::MAX, 0),
        |(closest_greater, closest_less), curr_sr| {
            if curr_sr >= sr {
                if curr_sr - sr < closest_greater - sr {
                    return (curr_sr, closest_less);
                }
                return (closest_greater, closest_less);
            }
            if sr - curr_sr < sr - closest_less {
                return (closest_greater, curr_sr);
            }
            (closest_greater, closest_less)
        },
    );
    if closest_greater < u32::MAX {
        Ok(Some(closest_greater))
    } else if closest_less > 0 {
        Ok(Some(closest_less))
    } else {
        Ok(None)
    }
}

fn calc_position_sec(sound_handle: &SoundHandle) -> f64 {
    sound_handle.index() as f64 / sound_handle.sample_rate() as f64
}

fn noti_err(noti_tx: &watch::Sender<PlayerNotification>, err: KaError) {
    error!("{}", err);
    noti_tx
        .send(PlayerNotification::Err(err.to_string()))
        .unwrap();
}

fn main_loop(
    mut msg_rx: mpsc::Receiver<PlayerCommand>,
    noti_tx: watch::Sender<PlayerNotification>,
) {
    let current_sr = AtomicU32::new(48000);
    let current_volume = AtomicF32::new(1.);
    let current_track_id = AtomicUsize::new(0);
    let get_device_name = || {
        Device::Default.name().unwrap_or_else(|err| {
            noti_err(&noti_tx, err);
            "".into()
        })
    };
    let device_name = RefCell::new(String::new());
    let init_mixer = |sr: Option<u32>, change_device: bool| {
        let sr = sr.unwrap_or(48000);
        let mixer = Mixer::new();
        if change_device {
            *device_name.borrow_mut() = get_device_name();
        }
        let device = loop {
            match Device::from_name(&device_name.borrow()) {
                Ok(d) => break d,
                Err(err) => {
                    noti_err(&noti_tx, err);
                    std::thread::sleep(Duration::from_secs(1));
                    *device_name.borrow_mut() = get_device_name();
                }
            };
        };
        mixer.init_ex(
            device,
            StreamSettings {
                sample_rate: Some(sr),
                ..Default::default()
            },
        );
        info!("device: {}, sr: {}", device_name.borrow(), sr);
        current_sr.store(sr, atomic::Ordering::Release);
        mixer
    };
    let mut mixer = init_mixer(None, true);
    let mut sound_handle = SoundHandle::new({
        let mut sound = Sound::default();
        sound.pause();
        sound
    });
    let set_track = |mixer: &mut Mixer,
                     sound_handle: &mut SoundHandle,
                     track_id: Option<usize>,
                     start_time_sec: f64,
                     is_playing: bool| {
        let track_id = track_id.unwrap_or(current_track_id.load(atomic::Ordering::Acquire));
        let sound = TRACK_LIST
            .read()
            .get(track_id)
            .map(|track| Sound::from_frames(track.sr(), track.interleaved_frames()));

        info!("sound created with track {}", track_id);
        match sound {
            Some(mut sound) => {
                sound.paused = !is_playing;
                sound.set_volume(current_volume.load(atomic::Ordering::Acquire));
                sound.seek_to(start_time_sec);
                mixer.renderer.guard().sounds.clear();
                info!("mixer clear");
                *sound_handle = mixer.play(sound);
                info!("sound added");
                current_track_id.store(track_id, atomic::Ordering::Release);
            }
            None => {
                mixer.renderer.guard().sounds.clear();
                info!("mixer clear");
            }
        }
    };

    loop {
        match msg_rx.try_recv() {
            Ok(msg) => match msg {
                PlayerCommand::Initialize => {
                    mixer = init_mixer(None, true);
                }
                PlayerCommand::SetSr(sr) if current_sr.load(atomic::Ordering::Acquire) != sr => {
                    let sr = match get_optimal_sr(&device_name.borrow(), sr) {
                        Ok(sr) => sr,
                        Err(err) => {
                            noti_err(&noti_tx, err);
                            continue;
                        }
                    };
                    if sr.is_some_and(|sr| sr != current_sr.load(atomic::Ordering::Acquire))
                        || sr.is_none()
                    {
                        mixer = init_mixer(sr, false);
                    } else {
                        info!("sr no change");
                    }
                }
                PlayerCommand::SetSr(_) => {
                    info!("sr no change");
                }
                #[allow(non_snake_case)]
                PlayerCommand::SetVolumedB(volume_dB) => {
                    let volume = volume_dB.amp_from_dB_default() as f32;
                    current_volume.store(volume, atomic::Ordering::Release);
                    sound_handle.set_volume(volume);
                }
                PlayerCommand::SetTrack((track_id, start_time)) => {
                    info!("set track");
                    let (start_time, is_playing) =
                        if let PlayerNotification::Ok(state) = &(*noti_tx.borrow()) {
                            (
                                start_time.unwrap_or(state.position_sec_elapsed()),
                                state.is_playing,
                            )
                        } else {
                            (0., false)
                        };
                    set_track(
                        &mut mixer,
                        &mut sound_handle,
                        track_id,
                        start_time,
                        is_playing,
                    );
                }
                PlayerCommand::Seek(sec) => {
                    let max_sec = TRACK_LIST.read().max_sec;
                    let sec = sec.min(max_sec);
                    noti_tx.send_modify(|noti| {
                        if let PlayerNotification::Ok(state) = noti {
                            if state.is_playing && mixer.is_finished() {
                                set_track(
                                    &mut mixer,
                                    &mut sound_handle,
                                    Some(current_track_id.load(atomic::Ordering::Acquire)),
                                    sec,
                                    state.is_playing,
                                );
                            } else {
                                sound_handle.seek_to(sec);
                            }
                            state.position_sec = sec;
                            state.instant = Instant::now();
                        }
                    });
                    info!("seek to {}", sec);
                }
                PlayerCommand::Pause => {
                    sound_handle.pause();
                    if matches!(*noti_tx.borrow(), PlayerNotification::Ok(_)) {
                        noti_tx
                            .send(PlayerNotification::Ok(InternalPlayerState {
                                is_playing: false,
                                position_sec: calc_position_sec(&sound_handle),
                                instant: Instant::now(),
                            }))
                            .unwrap();
                    }
                    info!("pause");
                }
                PlayerCommand::Resume => {
                    sound_handle.resume();

                    let position_sec = if let PlayerNotification::Ok(state) = &(*noti_tx.borrow()) {
                        state.position_sec
                    } else {
                        0.
                    };
                    if mixer.is_finished() {
                        set_track(&mut mixer, &mut sound_handle, None, position_sec, true);
                    }
                    if matches!(*noti_tx.borrow(), PlayerNotification::Ok(_)) {
                        noti_tx
                            .send(PlayerNotification::Ok(InternalPlayerState {
                                is_playing: true,
                                position_sec,
                                instant: Instant::now(),
                            }))
                            .unwrap();
                    }
                    info!("play");
                }
            },
            Err(TryRecvError::Empty) => {
                // TODO: error handling
                // if !mixer.backend.is_locked() {
                //     mixer.handle_errors(|err| {
                //         noti_err(&noti_tx, KaError::StreamError(err));
                //     });
                //     mixer = init_mixer(Some(current_sr.load(atomic::Ordering::Acquire)));
                // }
                // notification
                let prev_state = if let PlayerNotification::Ok(state) = &(*noti_tx.borrow()) {
                    Some(state.clone())
                } else {
                    None
                };
                if let Some(prev_state) = prev_state {
                    let mut state = InternalPlayerState {
                        is_playing: prev_state.is_playing,
                        position_sec: calc_position_sec(&sound_handle),
                        instant: Instant::now(),
                    };
                    if mixer.is_finished() {
                        // no current sound
                        {
                            // only for logging
                            if !sound_handle.paused() {
                                sound_handle.pause();
                                info!("track ended");
                            }
                        }
                        let position_sec = prev_state.position_sec
                            + (state.instant - prev_state.instant).as_secs_f64();
                        let max_sec = TRACK_LIST.read().max_sec;
                        if position_sec >= max_sec {
                            state.is_playing = false;
                            state.position_sec = max_sec;
                            if prev_state.position_sec != max_sec {
                                info!("reached max_sec {}", max_sec);
                            }
                        } else if prev_state.is_playing {
                            state.is_playing = true;
                            state.position_sec = position_sec;
                        } else {
                            state.is_playing = false;
                            state.position_sec = prev_state.position_sec;
                        }
                    }
                    noti_tx.send(PlayerNotification::Ok(state)).unwrap();
                }
                let new_device = Device::Default.name();
                if let Ok(new_device) = new_device {
                    if new_device != *device_name.borrow() {
                        let sr = match get_optimal_sr(
                            &new_device,
                            current_sr.load(atomic::Ordering::Acquire),
                        ) {
                            Ok(sr) => sr,
                            Err(err) => {
                                noti_err(&noti_tx, err);
                                continue;
                            }
                        };
                        mixer = init_mixer(sr, true);
                        sound_handle.pause();

                        let state = if let PlayerNotification::Ok(state) = &(*noti_tx.borrow()) {
                            Some(state.clone())
                        } else {
                            None
                        };
                        if let Some(state) = state {
                            set_track(
                                &mut mixer,
                                &mut sound_handle,
                                Some(current_track_id.load(atomic::Ordering::Acquire)),
                                state.position_sec_elapsed(),
                                state.is_playing,
                            );
                        }
                        continue;
                    }
                }
                std::thread::sleep(PLAYER_NOTI_INTERVAL);
            }
            Err(TryRecvError::Disconnected) => {
                break;
            }
        }
    }
}

pub fn spawn_task() {
    if COMMAND_TX.get().is_some_and(|cmd_tx| !cmd_tx.is_closed())
        && NOTI_RX
            .get()
            .is_some_and(|noti_rx| noti_rx.has_changed().is_ok())
    {
        COMMAND_TX
            .get()
            .unwrap()
            .blocking_send(PlayerCommand::Initialize)
            .unwrap();
        return;
    }

    let (command_tx, command_rx) = mpsc::channel::<PlayerCommand>(20);
    let (noti_tx, noti_rx) = watch::channel(PlayerNotification::Ok(Default::default()));
    COMMAND_TX.set(command_tx).unwrap();
    NOTI_RX.set(noti_rx).unwrap();
    spawn_blocking(|| main_loop(command_rx, noti_tx));
}
