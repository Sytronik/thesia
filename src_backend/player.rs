use std::cell::RefCell;
use std::sync::atomic::{self, AtomicU32, AtomicUsize};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use cpal::traits::DeviceTrait;
use cpal::SupportedStreamConfigsError;
use futures::task;
use kittyaudio::{Device, KaError, Mixer, Sound, SoundHandle, StreamSettings};
use lazy_static::{initialize, lazy_static};
use log::{error, info, LevelFilter, SetLoggerError};
use simple_logger::SimpleLogger;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
    sync::watch,
};

use crate::TM;

lazy_static! {
    static ref RUNTIME: Runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_time()
        .thread_name("thesia-tokio-audio")
        .build()
        .unwrap();
    static ref PLAYER_NOTI_INTERVAL: Duration = Duration::from_millis(100);
}

static mut COMMAND_TX: Option<mpsc::Sender<PlayerCommand>> = None;
static mut NOTI_RX: Option<watch::Receiver<PlayerNotification>> = None;

pub fn init_logger() -> Result<(), SetLoggerError> {
    SimpleLogger::new().with_level(LevelFilter::Info).init()
}

pub enum PlayerCommand {
    /// only caused by refreshing of frontend
    Initialize,
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

#[derive(Clone)]
pub struct PlayerState {
    /// if currently playing
    pub is_playing: bool,
    /// playing position (sec)
    pub position_sec: f64,
    /// timestamp when this state is created
    pub instant: Instant,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            is_playing: false,
            position_sec: 0.,
            instant: Instant::now(),
        }
    }
}

#[derive(Clone)]
pub enum PlayerNotification {
    Ok(PlayerState),
    Err(String),
}

pub async fn send(msg: PlayerCommand) {
    let msg_tx = unsafe { COMMAND_TX.clone().unwrap() };
    if let Err(e) = msg_tx.send(msg).await {
        panic!("PLAYER MSG_TX error: {}", e);
    }
}

pub fn recv() -> PlayerNotification {
    let noti = unsafe { (*NOTI_RX.as_ref().unwrap().borrow()).clone() };
    match noti {
        PlayerNotification::Ok(mut state) if state.is_playing => {
            let elapsed = state.instant.elapsed();
            state.position_sec += elapsed.as_secs_f64();
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
    println!("supported sr: {:?}", supported_sr_list);
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
    init_logger().unwrap();
    let current_sr = AtomicU32::new(48000);
    let current_track_id = AtomicUsize::new(0);
    let get_device_name = || match Device::Default.name() {
        Ok(n) => n,
        Err(err) => {
            noti_err(&noti_tx, err);
            "".into()
        }
    };
    let device_name = RefCell::new(get_device_name());
    let init_mixer = |sr: Option<u32>| {
        let sr = sr.unwrap_or(48000);
        let mixer = Mixer::new();
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
    let mut mixer = init_mixer(None);
    let mut sound_handle = SoundHandle::new({
        let mut sound = Sound::default();
        sound.paused = true;
        sound
    });
    let set_track = |mixer: &mut Mixer,
                     sound_handle: &mut SoundHandle,
                     track_id: Option<usize>,
                     start_time_sec: f64,
                     is_playing: bool| {
        let track_id = track_id.unwrap_or(current_track_id.load(atomic::Ordering::Acquire));
        let sound = TM
            .blocking_read()
            .track(track_id)
            .map(|track| Sound::from_frames(track.sr(), &track.interleaved_frames()));

        println!("sound created with track {}", track_id);
        match sound {
            Some(mut sound) => {
                sound.seek_to(start_time_sec);
                sound.paused = !is_playing;
                mixer.renderer.guard().sounds.clear();
                println!("mixer clear");
                *sound_handle = mixer.play(sound);
                println!("sound added");
                current_track_id.store(track_id, atomic::Ordering::Release);
            }
            None => {
                mixer.renderer.guard().sounds.clear();
                println!("mixer clear");
            }
        }
    };

    let waker = task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        match msg_rx.poll_recv(&mut cx) {
            Poll::Ready(Some(msg)) => match msg {
                PlayerCommand::Initialize => {
                    mixer = init_mixer(None);
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
                        mixer = init_mixer(sr);
                    } else {
                        println!("sr no change");
                    }
                }
                PlayerCommand::SetSr(_) => {}
                PlayerCommand::SetTrack((track_id, start_time)) => {
                    println!("set track");
                    let (start_time, is_playing) =
                        if let PlayerNotification::Ok(state) = &(*noti_tx.borrow()) {
                            (start_time.unwrap_or(state.position_sec), state.is_playing)
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
                    let max_sec = TM.blocking_read().tracklist.max_sec;
                    let sec = sec.min(max_sec);
                    sound_handle.seek_to(sec);
                    noti_tx.send_modify(|noti| {
                        if let PlayerNotification::Ok(state) = noti {
                            state.position_sec = sec;
                            state.instant = Instant::now();
                        }
                    });
                    println!("seek to {}", sec);
                }
                PlayerCommand::Pause => {
                    sound_handle.guard().paused = true;
                    if matches!(*noti_tx.borrow(), PlayerNotification::Ok(_)) {
                        noti_tx
                            .send(PlayerNotification::Ok(PlayerState {
                                is_playing: false,
                                position_sec: calc_position_sec(&sound_handle),
                                instant: Instant::now(),
                            }))
                            .unwrap();
                    }
                    println!("pause");
                }
                PlayerCommand::Resume => {
                    sound_handle.guard().paused = false;

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
                            .send(PlayerNotification::Ok(PlayerState {
                                is_playing: true,
                                position_sec,
                                instant: Instant::now(),
                            }))
                            .unwrap();
                    }
                    println!("play");
                }
            },
            Poll::Pending => {
                // TODO: error handling
                // mixer.handle_errors(|err| {
                //     noti_err(&noti_tx, KaError::StreamError(err));
                // });
                // notification
                let prev_state = if let PlayerNotification::Ok(state) = &(*noti_tx.borrow()) {
                    Some(state.clone())
                } else {
                    None
                };
                if let Some(prev_state) = prev_state {
                    let mut state = PlayerState {
                        is_playing: prev_state.is_playing,
                        position_sec: calc_position_sec(&sound_handle),
                        instant: Instant::now(),
                    };
                    if mixer.is_finished() {
                        // no current sound
                        {
                            // only for logging
                            let mut sound = sound_handle.guard();
                            if !sound.paused {
                                sound.paused = true;
                                println!("track ended");
                            }
                        }
                        let position_sec = prev_state.position_sec
                            + (state.instant - prev_state.instant).as_secs_f64();
                        let max_sec = TM.blocking_read().tracklist.max_sec;
                        if position_sec >= max_sec {
                            state.is_playing = false;
                            state.position_sec = max_sec;
                            if prev_state.position_sec != max_sec {
                                println!("reached max_sec {}", max_sec);
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
                std::thread::sleep(*PLAYER_NOTI_INTERVAL);
            }
            Poll::Ready(None) => {
                break;
            }
        }
    }
}

pub fn spawn_runtime() {
    unsafe {
        if COMMAND_TX
            .as_ref()
            .is_some_and(|cmd_tx| !cmd_tx.is_closed())
            && NOTI_RX
                .as_ref()
                .is_some_and(|noti_rx| noti_rx.has_changed().is_ok())
        {
            COMMAND_TX
                .as_ref()
                .unwrap()
                .blocking_send(PlayerCommand::Initialize)
                .unwrap();
            return;
        }
    }
    initialize(&RUNTIME);

    let (command_tx, command_rx) = mpsc::channel::<PlayerCommand>(10);
    let (noti_tx, noti_rx) = watch::channel(PlayerNotification::Ok(Default::default()));
    unsafe {
        COMMAND_TX = Some(command_tx);
        NOTI_RX = Some(noti_rx);
    }
    RUNTIME.spawn_blocking(|| main_loop(command_rx, noti_tx));
}
