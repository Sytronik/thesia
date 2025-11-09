//! interfaces to communicate with JS world

use std::{sync::LazyLock, thread};

use crossbeam_channel::{Sender, unbounded};
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{GuardClippingMode, SpecSetting, SpectrogramSliceArgs};

static WRITE_LOCK_WORKER: LazyLock<WriteLockWorker> = LazyLock::new(WriteLockWorker::new);

type WriteLockJob = Box<dyn FnOnce() + Send + 'static>;

struct WriteLockWorker {
    sender: Sender<WriteLockJob>,
}

impl WriteLockWorker {
    fn new() -> Self {
        let (sender, receiver) = unbounded::<WriteLockJob>();
        thread::Builder::new()
            .name("write-lock-worker".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    job();
                }
                log::error!("write lock worker channel closed; exiting thread");
            })
            .expect("Failed to spawn write lock worker");
        Self { sender }
    }

    fn spawn<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if let Err(err) = self.sender.send(Box::new(job)) {
            log::error!("Failed to submit write lock job: {err}");
        }
    }
}

pub fn spawn_write_lock_task<F, R>(f: F) -> impl std::future::Future<Output = R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    WRITE_LOCK_WORKER.spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });
    async move { rx.await.expect("write lock worker terminated unexpectedly") }
}

#[derive(Default, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct UserSettingsOptionals {
    pub spec_setting: Option<SpecSetting>,
    pub blend: Option<f64>,
    pub dB_range: Option<f64>,
    pub common_guard_clipping: Option<GuardClippingMode>,
    pub common_normalize: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct UserSettings {
    pub spec_setting: SpecSetting,
    pub blend: f64,
    pub dB_range: f64,
    pub common_guard_clipping: GuardClippingMode,
    pub common_normalize: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct PlayerState {
    pub is_playing: bool,
    pub position_sec: f64,
    pub err: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Spectrogram {
    pub buf: Vec<f32>,
    pub width: u32,
    pub height: u32,
    pub start_sec: f64,
    pub px_per_sec: f64,
    pub left_margin: f64,
    pub right_margin: f64,
    pub top_margin: f64,
    pub bottom_margin: f64,
    pub is_low_quality: bool,
}

impl Spectrogram {
    pub fn new(
        args: SpectrogramSliceArgs,
        mipmap: Array2<f32>,
        start_sec: f64,
        is_low_quality: bool,
    ) -> Self {

        Self {
            buf: mipmap.into_raw_vec_and_offset().0,
            width: args.width as u32,
            height: args.height as u32,
            start_sec,
            px_per_sec: args.px_per_sec,
            left_margin: args.left_margin,
            right_margin: args.right_margin,
            top_margin: args.top_margin,
            bottom_margin: args.bottom_margin,
            is_low_quality,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct WavMetadata {
    pub length: u32,
    pub sr: u32,
    pub is_clipped: bool,
}

#[inline]
pub fn format_id_ch(id: usize, ch: usize) -> String {
    format!("{}_{}", id, ch)
}

#[inline]
pub fn parse_id_ch_str(id_ch_str: &str) -> anyhow::Result<(usize, usize)> {
    let mut iter = id_ch_str.split('_').map(|x| x.parse::<usize>());
    match (iter.next(), iter.next()) {
        (Some(Ok(id)), Some(Ok(ch))) => Ok((id, ch)),
        _ => Err(anyhow::anyhow!(
          "The array element should be \"{{unsigned_int}}_{{unsigned_int}}\".",
        )),
    }
}
