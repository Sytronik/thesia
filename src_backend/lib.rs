#![allow(dead_code)]
#![allow(unexpected_cfgs)] // napi-rs issue

// need to statically link OpenBLAS on Windows
extern crate blas_src;

use std::collections::HashMap;
use std::sync::LazyLock;

use log::LevelFilter;
use napi::bindgen_prelude::*;
use napi::tokio::sync::RwLock as AsyncRwLock;
use napi_derive::napi;
use ndarray::{Array3, Axis, s};
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
}

// On Windows, this cause hanging.
#[cfg(not(windows))]
#[napi::module_init]
fn _napi_init() {
    _init_once();
}

#[napi]
fn init(user_settings: UserSettingsOptionals) -> Result<UserSettings> {
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
            *tm = TrackManager::new();
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
            .update_all_specs_greys(&TRACK_LIST.blocking_read());
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
            .update_all_specs_greys(&TRACK_LIST.blocking_read());
    })
    .await
    .unwrap();
    refresh_track_player().await;
    Ok(())
}

struct SpectrogramSliceArgs {
    px_per_sec: f64,
    left: usize,
    width: usize,
    height: usize,
    left_margin: f64,
    right_margin: f64,
}

impl SpectrogramSliceArgs {
    fn new(
        n_frames: usize,
        n_freqs: usize,
        track_sec: f64,
        sec_range: (f64, f64),
        margin_px: i32,
    ) -> Self {
        let px_per_sec = n_frames as f64 / track_sec;

        let left_f64 = sec_range.0 * px_per_sec;
        let width_f64 = ((sec_range.1 - sec_range.0) * px_per_sec).max(0.);

        let left_with_margin = left_f64 as i32 - margin_px;
        let width_with_margin =
            ((left_f64 + width_f64).ceil() as i32 + margin_px - left_with_margin).max(0);

        let left_clipped = left_with_margin.max(0) as usize;
        let width_clipped = width_with_margin.min(n_frames as i32 - left_clipped as i32) as usize;

        let left_margin = left_f64 - left_clipped as f64;
        let right_margin = width_clipped as f64 - width_f64;
        Self {
            px_per_sec,
            left: left_clipped,
            width: width_clipped,
            left_margin,
            right_margin,
            height: n_freqs,
        }
    }
}

#[napi]
async fn get_spectrogram(
    id_ch_str: String,
    sec_range: (f64, f64),
    margin_px: i32,
) -> Result<Option<Spectrogram>> {
    assert!(margin_px >= 0);
    let (id, ch) = parse_id_ch_str(&id_ch_str)?;
    let sec_range = (sec_range.0.max(0.), sec_range.1.max(sec_range.0));
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
        let tm = TM.blocking_read();
        let spec_grey = if let Some(spec_grey) = tm.spec_greys.get(&(id, ch)) {
            spec_grey
        } else {
            return Ok(None);
        };
        let args = SpectrogramSliceArgs::new(
            spec_grey.shape()[1],
            spec_grey.shape()[0],
            track_sec,
            sec_range,
            margin_px,
        );

        let sliced_spec = spec_grey
            .slice(s![.., args.left..args.left + args.width])
            .to_owned();
        let (sliced_vec, _) = sliced_spec.into_raw_vec_and_offset();

        Ok(Some(Spectrogram {
            arr: Float32Array::new(sliced_vec),
            width: args.width as u32,
            height: args.height as u32,
            px_per_sec: args.px_per_sec,
            left_margin: args.left_margin,
            right_margin: args.right_margin,
        }))
    })
    .await
    .unwrap()
}

#[napi]
async fn get_wav_image(
    id_ch_str: String,
    start_sec: f64,
    px_per_sec: f64,
    width: u32,
    height: u32,
    amp_range: (f64, f64),
    dpr: f64,
) -> Result<WavImage> {
    let (id, ch) = parse_id_ch_str(&id_ch_str)?;
    let px_per_sec = px_per_sec * dpr;
    let width = (width as f64 * dpr).round() as u32;
    let height = (height as f64 * dpr).round() as u32;
    let opt_for_wav = DrawOptionForWav {
        amp_range: (amp_range.0 as f32, amp_range.1 as f32),
        dpr: dpr as f32,
    };

    let arr = spawn_blocking(move || {
        let empty_img = || Array3::zeros((height as usize, width as usize, 4));
        let tracklist = TRACK_LIST.blocking_read();
        let track = if let Some(track) = tracklist.get(id) {
            track
        } else {
            return empty_img();
        };
        let (pad_left, drawing_width, pad_right) =
            track.decompose_width_of(start_sec, width, px_per_sec);
        let (wav, show_clipping) = track.channel_for_drawing(ch);
        let wav_part = ArrWithSliceInfo::new(
            wav,
            track.calc_part_wav_info(start_sec, drawing_width, px_per_sec),
        );
        if drawing_width == 0 || wav_part.is_empty() {
            return empty_img();
        }

        let mut canvas = Array3::zeros((height as usize, drawing_width as usize, 4));
        draw_wav_to(
            canvas.as_slice_mut().unwrap(),
            wav_part,
            drawing_width,
            height,
            &opt_for_wav,
            show_clipping,
            true,
        );
        canvas.pad(
            (pad_left as usize, pad_right as usize),
            Axis(1),
            PadMode::Constant(0),
        )
    })
    .await
    .unwrap();

    let buf = arr.as_slice_memory_order().unwrap().into();
    Ok(WavImage { buf, width, height })
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
async fn get_overview(track_id: u32, width: u32, height: u32, dpr: f64) -> WavImage {
    assert!(width >= 1 && height >= 1);
    let width = (width as f64 * dpr).round() as u32;
    let height = (height as f64 * dpr).round() as u32;

    let arr = spawn_blocking(move || {
        TM.blocking_read().draw_overview(
            &TRACK_LIST.blocking_read(),
            track_id as usize,
            width,
            height,
            dpr as f32,
        )
    })
    .await
    .unwrap();
    WavImage {
        buf: arr.into(),
        width,
        height,
    }
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
fn get_guard_clip_stats(track_id: u32) -> HashMap<String, String> {
    let tracklist = TRACK_LIST.blocking_read();
    let mode = tracklist.common_guard_clipping;
    let prefix = mode.to_string();
    tracklist
        .get(track_id as usize)
        .map_or_else(HashMap::new, |track| {
            let stat_to_string_if_not_empty = |(ch_str, stat): (String, &GuardClippingStats)| {
                let stat = stat.to_string();
                (!stat.is_empty()).then_some((ch_str, format!("{} by {}", &prefix, stat)))
            };
            if mode == GuardClippingMode::Clip {
                track
                    .guard_clip_stats()
                    .iter()
                    .enumerate()
                    .map(|(i, stat)| (i.to_string(), stat))
                    .filter_map(stat_to_string_if_not_empty)
                    .collect()
            } else {
                std::iter::once(("".to_string(), &track.guard_clip_stats()[0]))
                    .filter_map(stat_to_string_if_not_empty)
                    .collect()
            }
        })
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
