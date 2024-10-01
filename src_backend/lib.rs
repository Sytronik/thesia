#![allow(dead_code)]

// need to statically link OpenBLAS on Windows
extern crate blas_src;

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{
    atomic::{self, AtomicBool},
    Arc,
};

use itertools::Itertools;
use lazy_static::lazy_static;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::json;
use tokio::sync::RwLock;

#[warn(dead_code)]
mod backend;
#[warn(dead_code)]
mod img_mgr;
#[warn(dead_code)]
mod objects;
#[warn(dead_code)]
mod player;

use backend::*;
use img_mgr::{DrawParams, ImgMsg};
use objects::*;
use player::{PlayerCommand, PlayerNotification};

#[cfg(all(
    any(windows, unix),
    target_arch = "x86_64",
    not(target_env = "musl"),
    not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

lazy_static! {
    static ref INITIALIZED_ONCE: AtomicBool = AtomicBool::new(false);

    static ref TRACK_LIST: Arc<RwLock<TrackList>> = Arc::new(RwLock::new(TrackList::new()));
    static ref TM: Arc<RwLock<TrackManager>> = Arc::new(RwLock::new(TrackManager::new()));

    // TODO: prevent making mistake not to update the values below. Maybe sth like auto-sync?
    static ref HZ_RANGE: Arc<RwLock<(f32, f32)>> = Arc::new(RwLock::new((0., f32::INFINITY)));
    static ref SPEC_SETTING: Arc<RwLock<SpecSetting>> = Arc::new(RwLock::new(Default::default()));
}

#[napi]
fn init(user_settings: UserSettingsOptionals) -> Result<UserSettings> {
    if !INITIALIZED_ONCE.load(atomic::Ordering::Acquire) {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_cpus::get_physical())
            .build_global()
            .map_err(|e| napi::Error::new(napi::Status::Unknown, e))?;
    }
    INITIALIZED_ONCE.store(true, atomic::Ordering::Release);

    let user_settings = {
        let mut tracklist = TRACK_LIST.blocking_write();
        let mut tm = TM.blocking_write();
        if !tracklist.is_empty() {
            *tracklist = TrackList::new();
            *tm = TrackManager::new();
        }
        if let Some(setting) = user_settings.spec_setting {
            tm.set_setting(&mut tracklist, setting.clone());
        }
        #[allow(non_snake_case)]
        if let Some(dB_range) = user_settings.dB_range {
            tm.set_dB_range(&mut tracklist, dB_range as f32);
        }
        if let Some(mode) = user_settings.common_guard_clipping {
            tm.set_common_guard_clipping(&mut tracklist, mode);
        }
        if let Some(target) = user_settings.common_normalize {
            let target = serde_json::from_value(target)?;
            tm.set_common_normalize(&mut tracklist, target);
        }
        UserSettings {
            spec_setting: tm.setting.clone(),
            blend: user_settings.blend.unwrap_or(0.5),
            dB_range: tm.dB_range as f64,
            common_guard_clipping: tracklist.common_guard_clipping,
            common_normalize: serde_json::to_value(tracklist.common_normalize).unwrap(),
        }
    };
    *HZ_RANGE.blocking_write() = (0., f32::INFINITY);
    *SPEC_SETTING.blocking_write() = user_settings.spec_setting.clone();

    img_mgr::spawn_runtime();
    player::spawn_task();
    Ok(user_settings)
}

#[napi]
async fn add_tracks(id_list: Vec<u32>, path_list: Vec<String>) -> Vec<u32> {
    assert!(!id_list.is_empty() && id_list.len() == path_list.len());

    let added_ids = TM.write().await.add_tracks(
        TRACK_LIST.write().await.deref_mut(),
        id_list.into_iter().map(|x| x as usize).collect(),
        path_list,
    );
    added_ids.into_iter().map(|x| x as u32).collect()
}

#[napi]
async fn reload_tracks(track_ids: Vec<u32>) -> Vec<u32> {
    assert!(!track_ids.is_empty());

    let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
    let no_err_ids = TM
        .write()
        .await
        .reload_tracks(&mut TRACK_LIST.write().await.deref_mut(), &track_ids);
    no_err_ids.into_iter().map(|x| x as u32).collect()
}

#[napi]
async fn remove_tracks(track_ids: Vec<u32>) {
    assert!(!track_ids.is_empty());

    let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
    let mut tracklist = TRACK_LIST.write().await;
    img_mgr::send(ImgMsg::Remove(tracklist.id_ch_tuples_from(&track_ids))).await;
    let hz_range = TM.write().await.remove_tracks(&mut tracklist, &track_ids);
    if let Some(hz_range) = hz_range {
        *HZ_RANGE.write().await = hz_range;
    }
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
    img_mgr::send(ImgMsg::Remove(id_ch_tuples)).await;
    player::send(PlayerCommand::SetSr(sr)).await;
    id_ch_strs
}

#[napi]
async fn set_image_state(
    id_ch_strs: Vec<String>,
    start_sec: f64,
    width: u32,
    option: DrawOption,
    opt_for_wav: serde_json::Value,
    blend: f64,
) -> Result<()> {
    // let start = Instant::now();
    let opt_for_wav: DrawOptionForWav = serde_json::from_value(opt_for_wav)?;
    assert!(!id_ch_strs.is_empty());
    assert!(width >= 1);
    assert!(option.px_per_sec.is_finite());
    assert!(option.px_per_sec >= 0.);
    assert!(option.height >= 1);
    assert!(opt_for_wav.amp_range.0 <= opt_for_wav.amp_range.1);

    let id_ch_tuples = {
        let tm = TM.read().await;
        parse_id_ch_tuples(id_ch_strs)?
            .into_iter()
            .filter(|id_ch| tm.exists(id_ch))
            .collect()
    };
    img_mgr::send(ImgMsg::Draw((
        id_ch_tuples,
        DrawParams::new(start_sec, width, option, opt_for_wav, blend),
    )))
    .await;
    Ok(())
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
    let tracklist = TRACK_LIST.read().await;
    let all_id_ch_tuples = tracklist.id_ch_tuples();
    spawn_blocking(move || {
        TM.blocking_write()
            .set_dB_range(&tracklist, dB_range as f32)
    });
    img_mgr::send(ImgMsg::Remove(all_id_ch_tuples)).await;
}

#[napi]
fn get_hz_range(max_track_hz: f64) -> [f64; 2] {
    let hz_range = calc_valid_hz_range(max_track_hz as f32);
    [hz_range.0 as f64, hz_range.1 as f64]
}

#[napi]
async fn set_hz_range(min_hz: f64, max_hz: f64) -> bool {
    assert!(min_hz >= 0.);
    assert!(max_hz > 0.);
    assert!(min_hz < max_hz);
    let hz_range = (min_hz as f32, max_hz as f32);
    *HZ_RANGE.write().await = hz_range.clone();
    let tracklist = TRACK_LIST.read().await;
    let all_id_ch_tuples = tracklist.id_ch_tuples();
    let need_update =
        spawn_blocking(move || TM.blocking_write().set_hz_range(&tracklist, hz_range))
            .await
            .unwrap();
    if need_update {
        img_mgr::send(ImgMsg::Remove(all_id_ch_tuples)).await;
    }
    need_update
}

#[napi]
fn get_spec_setting() -> SpecSetting {
    SPEC_SETTING.blocking_read().clone()
}

#[napi]
async fn set_spec_setting(spec_setting: SpecSetting) {
    assert!(spec_setting.win_ms > 0.);
    assert!(spec_setting.t_overlap >= 1);
    assert!(spec_setting.f_overlap >= 1);
    *SPEC_SETTING.write().await = spec_setting.clone();
    let mut tracklist = TRACK_LIST.write().await;
    let all_id_ch_tuples = tracklist.id_ch_tuples();
    spawn_blocking(move || {
        TM.blocking_write()
            .set_setting(&mut tracklist, spec_setting)
    });
    img_mgr::send(ImgMsg::Remove(all_id_ch_tuples)).await;
}

#[napi]
async fn get_common_guard_clipping() -> GuardClippingMode {
    TRACK_LIST.read().await.common_guard_clipping
}

#[napi]
async fn set_common_guard_clipping(mode: GuardClippingMode) {
    spawn_blocking(move || {
        let mut tracklist = TRACK_LIST.blocking_write();
        TM.blocking_write()
            .set_common_guard_clipping(&mut tracklist, mode);
    });
    remove_all_imgs().await;
    refresh_track_player().await;
}

#[napi]
async fn get_common_normalize() -> serde_json::Value {
    serde_json::to_value(TRACK_LIST.read().await.common_normalize).unwrap()
}

#[napi]
async fn set_common_normalize(target: serde_json::Value) -> Result<()> {
    let target = serde_json::from_value(target)?;

    spawn_blocking(move || {
        let mut tracklist = TRACK_LIST.blocking_write();
        TM.blocking_write()
            .set_common_normalize(&mut tracklist, target);
    });
    remove_all_imgs().await;
    refresh_track_player().await;
    Ok(())
}

#[napi]
fn get_images() -> HashMap<String, Buffer> {
    match img_mgr::recv() {
        Some(images) => images
            .into_iter()
            .map(|((id, ch), img)| (format_id_ch(id, ch), img.into()))
            .collect(),
        None => HashMap::new(),
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
async fn get_overview(track_id: u32, width: u32, height: u32, dpr: f64) -> Buffer {
    assert!(width >= 1 && height >= 1);

    let tracklist = TRACK_LIST.read().await;
    TM.read()
        .await
        .draw_overview(&tracklist, track_id as usize, width, height, dpr as f32)
        .into()
}

#[napi]
fn freq_pos_to_hz_on_current_range(y: f64, height: u32) -> f64 {
    assert!(height >= 1);

    convert_freq_pos_to_hz(y as f32, height, None) as f64
}

#[napi]
fn freq_pos_to_hz(y: f64, height: u32, hz_range: (f64, f64)) -> f64 {
    assert!(height >= 1);

    let hz_range = (hz_range.0 as f32, hz_range.1 as f32);
    convert_freq_pos_to_hz(y as f32, height, Some(hz_range)) as f64
}

#[napi]
fn freq_hz_to_pos(hz: f64, height: u32, hz_range: (f64, f64)) -> f64 {
    assert!(height >= 1);

    let hz_range = (hz_range.0 as f32, hz_range.1 as f32);
    convert_freq_hz_to_pos(hz as f32, height, Some(hz_range)) as f64
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
    max_track_hz: f64,
) -> serde_json::Value {
    assert_axis_params(max_num_ticks, max_num_labels);

    json!(calc_freq_axis_markers(
        calc_valid_hz_range(max_track_hz as f32),
        SPEC_SETTING.blocking_read().freq_scale,
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
async fn get_sample_rate(track_id: u32) -> u32 {
    TRACK_LIST
        .read()
        .await
        .get(track_id as usize)
        .map_or(0, |track| track.sr())
}

#[napi]
async fn get_format_info(track_id: u32) -> AudioFormatInfo {
    TRACK_LIST
        .read()
        .await
        .get(track_id as usize)
        .map_or_else(Default::default, |track| track.format_info.clone())
}

#[napi(js_name = "getGlobalLUFS")]
async fn get_global_lufs(track_id: u32) -> f64 {
    TRACK_LIST
        .read()
        .await
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().global_lufs)
}

#[napi(js_name = "getRMSdB")]
#[allow(non_snake_case)]
async fn get_rms_dB(track_id: u32) -> f64 {
    TRACK_LIST
        .read()
        .await
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().rms_dB as f64)
}

#[napi(js_name = "getMaxPeakdB")]
#[allow(non_snake_case)]
async fn get_max_peak_dB(track_id: u32) -> f64 {
    TRACK_LIST
        .read()
        .await
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().max_peak_dB as f64)
}

#[napi]
// TODO: currently, no use
async fn get_guard_clip_stats(track_id: u32) -> String {
    let tracklist = TRACK_LIST.read().await;
    let prefix = tracklist.common_guard_clipping.to_string();
    tracklist
        .get(track_id as usize)
        .map_or_else(String::new, |track| {
            Itertools::intersperse(
                track.guard_clip_stats().iter().map(|stat| {
                    let stat = stat.to_string();
                    if !stat.is_empty() {
                        format!("{} by {}", &prefix, stat)
                    } else {
                        stat
                    }
                }),
                "\n".to_string(),
            )
            .collect()
        })
}

#[napi]
async fn get_path(track_id: u32) -> String {
    TRACK_LIST
        .read()
        .await
        .get(track_id as usize)
        .map_or_else(String::new, |track| track.path_string())
}

#[napi]
async fn get_file_name(track_id: u32) -> String {
    TRACK_LIST
        .read()
        .await
        .filename(track_id as usize)
        .to_owned()
}

#[napi]
fn get_color_map() -> Buffer {
    visualize::get_colormap_rgb().into()
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
fn format_id_ch(id: usize, ch: usize) -> String {
    format!("{}_{}", id, ch)
}

fn parse_id_ch_tuples(id_ch_strs: Vec<String>) -> Result<IdChVec> {
    let mut result = IdChVec::with_capacity(id_ch_strs.len());
    for s in id_ch_strs {
        let mut iter = s.split('_').map(|x| x.parse::<usize>());
        match (iter.next(), iter.next()) {
            (Some(Ok(id)), Some(Ok(ch))) => {
                result.push((id, ch));
            }
            _ => {
                return Err(Error::new(
                    Status::Unknown,
                    "The array element should be \"{unsigned_int}_{unsigned_int}\".",
                ));
            }
        }
    }
    Ok(result)
}

#[inline]
async fn remove_all_imgs() {
    img_mgr::send(ImgMsg::Remove(TRACK_LIST.read().await.id_ch_tuples())).await;
}

#[inline]
async fn refresh_track_player() {
    player::send(PlayerCommand::SetTrack((None, None))).await;
}

#[inline]
fn calc_valid_hz_range(max_track_hz: f32) -> (f32, f32) {
    TrackManager::calc_valid_hz_range(&HZ_RANGE.blocking_read(), max_track_hz)
}

#[inline]
fn convert_freq_pos_to_hz(y: f32, height: u32, hz_range: Option<(f32, f32)>) -> f32 {
    let hz_range =
        hz_range.unwrap_or_else(|| calc_valid_hz_range(TM.blocking_read().max_sr as f32 / 2.));
    let rel_freq = 1. - y / height as f32;
    SPEC_SETTING
        .blocking_read()
        .freq_scale
        .relative_freq_to_hz(rel_freq, hz_range)
}

#[inline]
fn convert_freq_hz_to_pos(hz: f32, height: u32, hz_range: Option<(f32, f32)>) -> f32 {
    let hz_range =
        hz_range.unwrap_or_else(|| calc_valid_hz_range(TM.blocking_read().max_sr as f32 / 2.));
    let rel_freq = SPEC_SETTING
        .blocking_read()
        .freq_scale
        .hz_to_relative_freq(hz, hz_range);
    (1. - rel_freq) * height as f32
}
