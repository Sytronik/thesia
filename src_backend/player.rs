use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::task;
use lazy_static::{initialize, lazy_static};
use log::{LevelFilter, SetLoggerError};
use simple_logger::SimpleLogger;
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc,
    sync::watch,
};

use crate::{Player, Song, TM};

lazy_static! {
    static ref RUNTIME: Runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_time()
        .thread_name("thesia-tokio-audio")
        .build()
        .unwrap();
    static ref PLAYER_NOTI_INTERVAL: Duration = Duration::from_secs_f64(1. / 60.);
}

static mut COMMAND_TX: Option<mpsc::Sender<PlayerCommand>> = None;
static mut NOTI_RX: Option<watch::Receiver<PlayerNotification>> = None;

pub fn init_logger() -> Result<(), SetLoggerError> {
    SimpleLogger::new().with_level(LevelFilter::Info).init()
}

pub enum PlayerCommand {
    Initialize, // caused by refreshing of frontend
    SetSr(u32),
    SetTrack((usize, Duration)),
    Seek(Duration),
    Pause,
    Resume,
}

#[derive(Clone)]
pub struct PlayerStatus {
    pub is_playing: bool,
    pub play_pos: Duration,
    pub instant: Instant,
}

impl Default for PlayerStatus {
    fn default() -> Self {
        PlayerStatus {
            is_playing: false,
            play_pos: Default::default(),
            instant: Instant::now(),
        }
    }
}

#[derive(Clone)]
pub enum PlayerNotification {
    Ok(PlayerStatus),
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
        PlayerNotification::Ok(mut status) if status.is_playing => {
            let elapsed = status.instant.elapsed();
            status.play_pos += elapsed;
            PlayerNotification::Ok(status)
        }
        _ => noti,
    }
}

fn main_loop(
    mut msg_rx: mpsc::Receiver<PlayerCommand>,
    noti_tx: watch::Sender<PlayerNotification>,
) {
    init_logger().unwrap();
    let init_player = || Player::new(Some(vec![48000])).unwrap(); // TODO: handle error
    let mut player = init_player();
    let mut current_track_id = 0;
    let mut set_track = |player: &Player, track_id: Option<usize>, start_time: Duration| {
        let track_id = track_id.unwrap_or(current_track_id);
        let song = TM
            .blocking_read()
            .track(track_id)
            .map(|track| Song::new(track.view(), track.sr(), None));

        println!("song created");
        if let Some(song) = song {
            match player.play_song_now(&song, Some(start_time)) {
                Ok(()) => {
                    current_track_id = track_id;
                    println!("set song");
                }
                Err(e) => {
                    noti_tx
                        .send(PlayerNotification::Err(e.to_string()))
                        .unwrap();
                }
            }
        }
    };

    let waker = task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        match msg_rx.poll_recv(&mut cx) {
            Poll::Ready(Some(msg)) => match msg {
                PlayerCommand::Initialize => {
                    player = init_player();
                }
                PlayerCommand::SetSr(sr) => {
                    if player.sr() != sr as usize {
                        let new_player = match Player::new(Some(vec![sr])) {
                            Ok(p) => p,
                            Err(e) => {
                                noti_tx
                                    .send(PlayerNotification::Err(e.to_string()))
                                    .unwrap();
                                continue;
                            }
                        };
                        if player.sr() != new_player.sr() {
                            player = new_player;
                            println!("player is initialized with sr {}", player.sr());
                        }
                    }
                }
                PlayerCommand::SetTrack((track_id, start_time)) => {
                    set_track(&player, Some(track_id), start_time);
                }
                PlayerCommand::Seek(time) => {
                    let max_sec = TM.blocking_read().tracklist.max_sec;
                    let time = time.min(Duration::from_secs_f64(max_sec));
                    player.seek(time);
                    noti_tx.send_modify(|noti| {
                        if let PlayerNotification::Ok(status) = noti {
                            status.play_pos = time;
                            status.instant = Instant::now();
                        }
                    });
                    println!("seek to {:?}", time);
                }
                PlayerCommand::Pause => {
                    player.set_playing(false);
                    println!("pause");
                }
                PlayerCommand::Resume => {
                    player.set_playing(true);

                    let play_pos = if let PlayerNotification::Ok(status) = &(*noti_tx.borrow()) {
                        status.play_pos
                    } else {
                        Default::default()
                    };
                    if !player.has_current_song() {
                        set_track(&player, None, play_pos);
                    }
                    println!("play");
                }
            },
            Poll::Pending => {
                // notification
                let prev_pos = if let PlayerNotification::Ok(status) = &(*noti_tx.borrow()) {
                    Some(status.play_pos)
                } else {
                    None
                };
                if let Some(prev_pos) = prev_pos {
                    let status = if let Some((play_pos, _)) = player.get_playback_position() {
                        PlayerStatus {
                            is_playing: player.is_playing(),
                            play_pos,
                            instant: Instant::now(),
                        }
                    } else {
                        if !player.has_current_song() && player.is_playing() {
                            player.set_playing(false);
                            println!("song end");
                        }
                        PlayerStatus {
                            is_playing: false,
                            play_pos: prev_pos,
                            instant: Instant::now(),
                        }
                    };
                    noti_tx.send(PlayerNotification::Ok(status)).unwrap();
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
