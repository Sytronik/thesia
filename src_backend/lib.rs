#![allow(dead_code)]
#![allow(unexpected_cfgs)] // napi-rs issue

// need to statically link OpenBLAS on Windows
extern crate blas_src;

use std::num::Wrapping;
use std::sync::LazyLock;

use dashmap::DashMap;
use log::LevelFilter;
use napi::bindgen_prelude::*;
use napi::tokio;
use napi::tokio::sync::RwLock as AsyncRwLock;
use napi_derive::napi;
use parking_lot::RwLock as SyncRwLock;
use serde_json::json;
use simple_logger::SimpleLogger;

#[warn(dead_code)]
mod backend;
#[warn(dead_code)]
mod interface;
#[warn(dead_code)]
mod player;

use backend::*;
use interface::*;
use player::{PlayerCommand, PlayerNotification};

#[cfg(all(
    any(windows, unix),
    target_arch = "x86_64",
    not(target_env = "musl"),
    not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

static TRACK_LIST: LazyLock<AsyncRwLock<TrackList>> =
    LazyLock::new(|| AsyncRwLock::new(TrackList::new()));
static TM: LazyLock<AsyncRwLock<TrackManager>> =
    LazyLock::new(|| AsyncRwLock::new(TrackManager::new()));

static DRAW_WAV_TASK_ID_MAP: LazyLock<DashMap<String, Wrapping<u64>>> = LazyLock::new(DashMap::new);

// TODO: prevent making mistake not to update the values below. Maybe sth like auto-sync?
static SPEC_SETTING: SyncRwLock<SpecSetting> = SyncRwLock::new(SpecSetting::new());

fn _init_once() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus::get_physical())
        .build_global()
        .unwrap();
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();
    // console_subscriber::init();
}

// On Windows, this cause hanging.
#[cfg(not(windows))]
#[napi::module_init]
fn _napi_init() {
    _init_once();
}

#[napi]
fn init(user_settings: UserSettingsOptionals, max_spectrogram_size: u32) -> Result<UserSettings> {
    // On Windows, reloading cause restarting of renderer process.
    // (See killAndReload in src/main/menu.ts)
    // So INIT may not be needed, but use it for defensive purpose.
    #[cfg(windows)]
    {
        use parking_lot::Once;
        static INIT: Once = Once::new();
        INIT.call_once(_init_once);
    }

    let user_settings = {
        let mut tracklist = TRACK_LIST.blocking_write();
        let mut tm = TM.blocking_write();
        if !tracklist.is_empty() {
            *tracklist = TrackList::new();
            *tm = TrackManager::with_max_spectrogram_size(max_spectrogram_size);
        }
        if let Some(setting) = user_settings.spec_setting {
            tm.set_setting(&tracklist, setting.clone());
        }
        #[allow(non_snake_case)]
        if let Some(dB_range) = user_settings.dB_range {
            tm.set_dB_range(&tracklist, dB_range as f32);
        }
        if let Some(mode) = user_settings.common_guard_clipping {
            tracklist.set_common_guard_clipping(mode);
        }
        if let Some(target) = user_settings.common_normalize {
            let target = serde_json::from_value(target)?;
            tracklist.set_common_normalize(target);
        }
        UserSettings {
            spec_setting: tm.setting.clone(),
            blend: user_settings.blend.unwrap_or(0.5),
            dB_range: tm.dB_range as f64,
            common_guard_clipping: tracklist.common_guard_clipping,
            common_normalize: serde_json::to_value(tracklist.common_normalize).unwrap(),
        }
    };
    *SPEC_SETTING.write() = user_settings.spec_setting.clone();

    player::spawn_task();
    Ok(user_settings)
}

#[napi]
async fn add_tracks(id_list: Vec<u32>, path_list: Vec<String>) -> Vec<u32> {
    assert!(!id_list.is_empty() && id_list.len() == path_list.len());

    let added_ids = spawn_blocking(move || {
        TRACK_LIST
            .blocking_write()
            .add_tracks(id_list.into_iter().map(|x| x as usize).collect(), path_list)
    })
    .await
    .unwrap();
    let added_ids_u32 = added_ids.iter().map(|&x| x as u32).collect();
    spawn_blocking(move || {
        TM.blocking_write()
            .add_tracks(&TRACK_LIST.blocking_read(), &added_ids);
    });
    added_ids_u32
}

#[napi]
async fn reload_tracks(track_ids: Vec<u32>) -> Vec<u32> {
    assert!(!track_ids.is_empty());

    let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
    let (reloaded_ids, no_err_ids) =
        spawn_blocking(move || TRACK_LIST.blocking_write().reload_tracks(&track_ids))
            .await
            .unwrap();
    spawn_blocking(move || {
        TM.blocking_write()
            .reload_tracks(&TRACK_LIST.blocking_read(), &reloaded_ids);
    });
    no_err_ids.into_iter().map(|x| x as u32).collect()
}

#[napi]
fn remove_tracks(track_ids: Vec<u32>) {
    assert!(!track_ids.is_empty());

    let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
    let removed_id_ch_tuples = TRACK_LIST.blocking_write().remove_tracks(&track_ids);
    removed_id_ch_tuples.iter().for_each(|(id, ch)| {
        DRAW_WAV_TASK_ID_MAP.remove(&format_id_ch(*id, *ch));
    });
    spawn_blocking(move || {
        TM.blocking_write()
            .remove_tracks(&TRACK_LIST.blocking_read(), &removed_id_ch_tuples);
    });
}

#[napi]
async fn apply_track_list_changes() -> Vec<String> {
    let (id_ch_tuples, sr) = spawn_blocking(move || {
        let mut tm = TM.blocking_write();
        let tracklist = TRACK_LIST.blocking_read();
        let (updated_id_set, sr) = tm.apply_track_list_changes(&tracklist);
        let updated_ids: Vec<usize> = updated_id_set.into_iter().collect();
        (tracklist.id_ch_tuples_from(&updated_ids), sr)
    })
    .await
    .unwrap();
    let id_ch_strs = id_ch_tuples
        .iter()
        .map(|&(id, ch)| format_id_ch(id, ch))
        .collect();
    player::send(PlayerCommand::SetSr(sr)).await;
    id_ch_strs
}

#[napi(js_name = "getdBRange")]
#[allow(non_snake_case)]
async fn get_dB_range() -> f64 {
    TM.read().await.dB_range as f64
}

#[napi(js_name = "setdBRange")]
#[allow(non_snake_case)]
async fn set_dB_range(dB_range: f64) {
    assert!(dB_range > 0.);
    spawn_blocking(move || {
        TM.blocking_write()
            .set_dB_range(&TRACK_LIST.blocking_read(), dB_range as f32)
    })
    .await
    .unwrap();
}

#[napi]
async fn set_colormap_length(colormap_length: u32) {
    assert!(colormap_length > 0);
    spawn_blocking(move || {
        TM.blocking_write()
            .set_colormap_length(&TRACK_LIST.blocking_read(), colormap_length)
    })
    .await
    .unwrap();
}

#[napi]
fn get_spec_setting() -> SpecSetting {
    SPEC_SETTING.read().clone()
}

#[napi]
async fn set_spec_setting(spec_setting: SpecSetting) {
    assert!(spec_setting.win_ms > 0.);
    assert!(spec_setting.t_overlap >= 1);
    assert!(spec_setting.f_overlap >= 1);
    *SPEC_SETTING.write() = spec_setting.clone();
    spawn_blocking(move || {
        TM.blocking_write()
            .set_setting(&TRACK_LIST.blocking_read(), spec_setting)
    })
    .await
    .unwrap();
}

#[napi]
fn get_common_guard_clipping() -> GuardClippingMode {
    TRACK_LIST.blocking_read().common_guard_clipping
}

#[napi]
async fn set_common_guard_clipping(mode: GuardClippingMode) {
    spawn_blocking(move || TRACK_LIST.blocking_write().set_common_guard_clipping(mode))
        .await
        .unwrap();
    spawn_blocking(move || {
        TM.blocking_write()
            .update_all_specs_mipmaps(&TRACK_LIST.blocking_read());
    })
    .await
    .unwrap();
    refresh_track_player().await;
}

#[napi]
fn get_common_normalize() -> serde_json::Value {
    serde_json::to_value(TRACK_LIST.blocking_read().common_normalize).unwrap()
}

#[napi]
async fn set_common_normalize(target: serde_json::Value) -> Result<()> {
    let target = serde_json::from_value(target)?;

    spawn_blocking(move || {
        TRACK_LIST.blocking_write().set_common_normalize(target);
    })
    .await
    .unwrap();
    spawn_blocking(move || {
        TM.blocking_write()
            .update_all_specs_mipmaps(&TRACK_LIST.blocking_read());
    })
    .await
    .unwrap();
    refresh_track_player().await;
    Ok(())
}

#[napi]
async fn get_spectrogram(
    id_ch_str: String,
    sec_range: (f64, f64),
    hz_range: (f64, f64),
    margin_px: u32,
) -> Result<Option<Spectrogram>> {
    let (id, ch) = parse_id_ch_str(&id_ch_str)?;
    let sec_range = (sec_range.0.max(0.), sec_range.1);
    if sec_range.0 >= sec_range.1 {
        return Ok(None);
    }

    spawn_blocking(move || {
        let track_sec = {
            let tracklist = TRACK_LIST.blocking_read();
            if let Some(track) = tracklist.get(id) {
                track.sec()
            } else {
                return Ok(None);
            }
        };
        if sec_range.0 >= track_sec {
            return Ok(None);
        }
        let Some((args, sliced_mipmap)) = TM.blocking_read().get_sliced_spec_mipmap(
            (id, ch),
            track_sec,
            sec_range,
            (hz_range.0 as f32, hz_range.1 as f32),
            margin_px as usize,
        ) else {
            return Ok(None);
        };

        Ok(Some(Spectrogram::new(args, sliced_mipmap, sec_range.0)))
    })
    .await
    .unwrap()
}

#[napi]
async fn get_wav_drawing_info(
    id_ch_str: String,
    sec_range: (f64, f64),
    width: u32,
    height: u32,
    amp_range: (f64, f64),
    wav_stroke_width: f64,
    topbottom_context_size: f64,
    margin_ratio: f64,
) -> Result<Option<WavDrawingInfo>> {
    let (id, ch) = parse_id_ch_str(&id_ch_str)?;
    let sec_range = (sec_range.0.max(0.), sec_range.1);
    if sec_range.0 >= sec_range.1 {
        return Ok(None);
    }

    let task_id = {
        let mut entry = DRAW_WAV_TASK_ID_MAP
            .entry(id_ch_str.clone())
            .or_insert(Wrapping(0));
        *entry.value_mut() += 1;
        *entry.value()
    };

    let task = spawn_blocking(move || {
        let tracklist = TRACK_LIST.blocking_read();
        let track = tracklist.get(id)?;
        let internal = track.calc_wav_drawing_info(
            ch,
            sec_range,
            width as f32,
            height as f32,
            (amp_range.0 as f32, amp_range.1 as f32),
            wav_stroke_width as f32,
            topbottom_context_size as f32,
            margin_ratio,
        );
        Some(WavDrawingInfo::new(internal, sec_range.0))
    });

    loop {
        if task.is_finished() {
            return Ok(task.await.unwrap());
        }
        if DRAW_WAV_TASK_ID_MAP
            .get(&id_ch_str)
            .is_none_or(|id| *id != task_id)
        {
            return Ok(None); // if new task is started, return None
        }
        tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
    }
}

#[napi]
async fn find_id_by_path(path: String) -> i32 {
    TRACK_LIST
        .read()
        .await
        .find_id_by_path(&path)
        .map_or(-1, |id| id as i32)
}

#[napi]
async fn get_overview_drawing_info(
    track_id: u32,
    width: u32,
    height: u32,
    gap_height: f64,
    limiter_gain_height_ratio: f64,
    wav_stroke_width: f64,
    topbottom_context_size: f64,
) -> Option<OverviewDrawingInfo> {
    assert!(width >= 1 && height >= 1);
    let width = width as f32;
    let height = height as f32;
    let gap_height = gap_height as f32;
    let wav_stroke_width = wav_stroke_width as f32;
    let topbottom_context_size = topbottom_context_size as f32;

    let (internal, track_sec) = spawn_blocking(move || {
        let tracklist = TRACK_LIST.blocking_read();
        let track = tracklist.get(track_id as usize)?;
        let internal = OverviewDrawingInfoInternal::new(
            track,
            width,
            tracklist.max_sec,
            height,
            gap_height,
            limiter_gain_height_ratio as f32,
            wav_stroke_width,
            topbottom_context_size,
        );
        Some((internal, track.sec()))
    })
    .await
    .unwrap()?;

    Some(OverviewDrawingInfo::new(internal, track_sec))
}

#[napi]
fn freq_pos_to_hz(y: f64, height: u32, hz_range: (f64, f64)) -> f64 {
    assert!(height >= 1);

    let hz_range = (hz_range.0 as f32, hz_range.1 as f32);
    convert_freq_pos_to_hz(y as f32, height, hz_range) as f64
}

#[napi]
fn freq_hz_to_pos(hz: f64, height: u32, hz_range: (f64, f64)) -> f64 {
    assert!(height >= 1);

    let hz_range = (hz_range.0 as f32, hz_range.1 as f32);
    convert_freq_hz_to_pos(hz as f32, height, hz_range) as f64
}

#[napi]
fn seconds_to_label(sec: f64) -> String {
    convert_sec_to_label(sec)
}

#[napi]
fn time_label_to_seconds(label: String) -> f64 {
    convert_time_label_to_sec(&label).unwrap_or(f64::NAN)
}

#[napi]
fn hz_to_label(hz: f64) -> String {
    convert_hz_to_label(hz as f32)
}

#[napi]
fn freq_label_to_hz(label: String) -> f64 {
    convert_freq_label_to_hz(&label).unwrap_or(f32::NAN) as f64
}

#[napi]
fn get_time_axis_markers(
    start_sec: f64,
    end_sec: f64,
    tick_unit: f64,
    label_interval: u32,
    max_sec: f64,
) -> serde_json::Value {
    assert!(start_sec <= end_sec);
    assert!(label_interval > 0);
    json!(calc_time_axis_markers(
        start_sec,
        end_sec,
        tick_unit,
        label_interval,
        max_sec
    ))
}

#[napi]
fn get_freq_axis_markers(
    max_num_ticks: u32,
    max_num_labels: u32,
    hz_range: (f64, f64),
    max_track_hz: f64,
) -> serde_json::Value {
    assert_axis_params(max_num_ticks, max_num_labels);

    json!(calc_freq_axis_markers(
        (hz_range.0 as f32, hz_range.1.min(max_track_hz) as f32),
        SPEC_SETTING.read().freq_scale,
        max_num_ticks,
        max_num_labels
    ))
}

#[napi]
fn get_amp_axis_markers(
    max_num_ticks: u32,
    max_num_labels: u32,
    amp_range: (f64, f64),
) -> serde_json::Value {
    assert_axis_params(max_num_ticks, max_num_labels);
    assert!(amp_range.0 < amp_range.1);

    json!(calc_amp_axis_markers(
        max_num_ticks,
        max_num_labels,
        (amp_range.0 as f32, amp_range.1 as f32),
    ))
}

#[napi(js_name = "getdBAxisMarkers")]
#[allow(non_snake_case)]
fn get_dB_axis_markers(
    max_num_ticks: u32,
    max_num_labels: u32,
    min_dB: f64,
    max_dB: f64,
) -> serde_json::Value {
    assert_axis_params(max_num_ticks, max_num_labels);

    json!(calc_dB_axis_markers(
        max_num_ticks,
        max_num_labels,
        (min_dB as f32, max_dB as f32)
    ))
}

#[napi(js_name = "getMaxdB")]
#[allow(non_snake_case)]
async fn get_max_dB() -> f64 {
    TM.read().await.max_dB as f64
}

#[napi(js_name = "getMindB")]
#[allow(non_snake_case)]
async fn get_min_dB() -> f64 {
    TM.read().await.min_dB as f64
}

#[napi]
fn get_max_track_hz() -> f64 {
    TM.blocking_read().max_sr as f64 / 2.
}

#[napi]
fn get_longest_track_length_sec() -> f64 {
    TRACK_LIST.blocking_read().max_sec
}

#[napi]
fn get_channel_counts(track_id: u32) -> u32 {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or(0, |track| track.n_ch() as u32)
}

#[napi]
fn get_length_sec(track_id: u32) -> f64 {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or(0., |track| track.sec())
}

#[napi]
fn get_sample_rate(track_id: u32) -> u32 {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or(0, |track| track.sr())
}

#[napi]
fn get_format_info(track_id: u32) -> AudioFormatInfo {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or_else(Default::default, |track| track.format_info.clone())
}

#[napi(js_name = "getGlobalLUFS")]
fn get_global_lufs(track_id: u32) -> f64 {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().global_lufs)
}

#[napi(js_name = "getRMSdB")]
#[allow(non_snake_case)]
fn get_rms_dB(track_id: u32) -> f64 {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().rms_dB as f64)
}

#[napi(js_name = "getMaxPeakdB")]
#[allow(non_snake_case)]
fn get_max_peak_dB(track_id: u32) -> f64 {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().max_peak_dB as f64)
}

#[napi]
fn get_guard_clip_stats(track_id: u32) -> serde_json::Value {
    let tracklist = TRACK_LIST.blocking_read();
    let mode = tracklist.common_guard_clipping;
    let prefix = mode.to_string();
    match tracklist.get(track_id as usize) {
        Some(track) => {
            let format_ch_stat = |(ch, stat): (isize, String)| {
                (!stat.is_empty()).then_some((ch, format!("{} by {}", &prefix, stat)))
            };
            let vec: Vec<_> = match mode {
                GuardClippingMode::Clip => track
                    .guard_clip_stats()
                    .indexed_iter()
                    .map(|(ch, stat)| (ch as isize, stat.to_string()))
                    .filter_map(format_ch_stat)
                    .collect(),
                _ => std::iter::once((-1, track.guard_clip_stats()[0].to_string()))
                    .filter_map(format_ch_stat)
                    .collect(),
            };
            json!(vec)
        }
        None => json!([]),
    }
}

#[napi]
fn get_path(track_id: u32) -> String {
    TRACK_LIST
        .blocking_read()
        .get(track_id as usize)
        .map_or_else(String::new, |track| track.path_string())
}

#[napi]
fn get_file_name(track_id: u32) -> String {
    TRACK_LIST
        .blocking_read()
        .filename(track_id as usize)
        .to_owned()
}

#[napi(js_name = "setVolumedB")]
#[allow(non_snake_case)]
async fn set_volume_dB(volume_dB: f64) {
    player::send(PlayerCommand::SetVolumedB(volume_dB)).await;
}

#[napi]
async fn set_track_player(track_id: u32, sec: Option<f64>) {
    let track_id = track_id as usize;
    if TRACK_LIST.read().await.has(track_id) {
        player::send(PlayerCommand::SetTrack((Some(track_id), sec))).await;
    }
}

#[napi]
async fn seek_player(sec: f64) {
    player::send(PlayerCommand::Seek(sec)).await;
}

#[napi]
async fn pause_player() {
    player::send(PlayerCommand::Pause).await;
}

#[napi]
async fn resume_player() {
    player::send(PlayerCommand::Resume).await;
}

#[napi]
fn get_player_state() -> PlayerState {
    match player::recv() {
        PlayerNotification::Ok(state) => PlayerState {
            is_playing: state.is_playing,
            position_sec: state.position_sec,
            err: "".to_string(),
        },
        PlayerNotification::Err(e_str) => PlayerState {
            is_playing: false,
            position_sec: 0.,
            err: e_str,
        },
    }
}

#[inline]
pub fn assert_axis_params(max_num_ticks: u32, max_num_labels: u32) {
    assert!(max_num_ticks >= 2);
    assert!(max_num_labels >= 2);
    assert!(max_num_ticks >= max_num_labels);
}

#[inline]
async fn refresh_track_player() {
    player::send(PlayerCommand::SetTrack((None, None))).await;
}

#[inline]
fn convert_freq_pos_to_hz(y: f32, height: u32, hz_range: (f32, f32)) -> f32 {
    let hz_range = (
        hz_range.0,
        hz_range.1.min(TM.blocking_read().max_sr as f32 / 2.), // TODO: remove
    );
    let rel_freq = 1. - y / height as f32;
    SPEC_SETTING
        .read()
        .freq_scale
        .relative_freq_to_hz(rel_freq, hz_range)
}

#[inline]
fn convert_freq_hz_to_pos(hz: f32, height: u32, hz_range: (f32, f32)) -> f32 {
    let hz_range = (
        hz_range.0,
        hz_range.1.min(TM.blocking_read().max_sr as f32 / 2.), // TODO: remove
    );
    let rel_freq = SPEC_SETTING
        .read()
        .freq_scale
        .hz_to_relative_freq(hz, hz_range);
    (1. - rel_freq) * height as f32
}
