// need to statically link OpenBLAS on Windows
extern crate blas_src;

use std::num::Wrapping;
use std::sync::LazyLock;

use dashmap::DashMap;
use fast_image_resize::pixels;
use parking_lot::RwLock;
use serde_json::json;
use tauri::Manager;

mod backend;
mod interface;
mod menu;
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

static TRACK_LIST: LazyLock<RwLock<TrackList>> = LazyLock::new(|| RwLock::new(TrackList::new()));
static TM: LazyLock<RwLock<TrackManager>> = LazyLock::new(|| RwLock::new(TrackManager::new()));

type IdChStrToTaskIdMap = DashMap<String, Wrapping<u64>>;
static DRAW_SPEC_TASK_ID_MAP: LazyLock<IdChStrToTaskIdMap> = LazyLock::new(IdChStrToTaskIdMap::new);
static DRAW_WAV_TASK_ID_MAP: LazyLock<IdChStrToTaskIdMap> = LazyLock::new(IdChStrToTaskIdMap::new);

// TODO: prevent making mistake not to update the values below. Maybe sth like auto-sync?
static SPEC_SETTING: RwLock<SpecSetting> = RwLock::new(SpecSetting::new());

#[tauri::command]
fn init(
    user_settings: UserSettingsOptionals,
    max_spectrogram_size: u32,
    tmp_dir_path: String,
) -> tauri::Result<UserSettings> {
    let user_settings = {
        let mut tracklist = TRACK_LIST.write();
        let mut tm = TM.write();
        if !tracklist.is_empty() {
            *tracklist = TrackList::new();
            *tm =
                TrackManager::with_max_spec_size_tmp_dir(max_spectrogram_size, tmp_dir_path.into());
        } else {
            tm.set_tmp_dir_path(tmp_dir_path.into())?;
        }
        if let Some(setting) = user_settings.spec_setting {
            tm.set_setting(&tracklist, &setting);
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
    SPEC_SETTING.write().clone_from(&user_settings.spec_setting);

    player::spawn_task();
    Ok(user_settings)
}

#[tauri::command]
async fn add_tracks(track_ids: Vec<u32>, paths: Vec<String>) -> Vec<u32> {
    assert!(!track_ids.is_empty() && track_ids.len() == paths.len());

    spawn_write_lock_task(move || {
        let added_ids = TRACK_LIST
            .write()
            .add_tracks(track_ids.into_iter().map(|x| x as usize).collect(), paths);
        {
            let tracklist = TRACK_LIST.read();
            TM.write().add_tracks(&tracklist, &added_ids);
        }
        added_ids.iter().map(|&x| x as u32).collect()
    })
    .await
}

#[tauri::command]
async fn reload_tracks(track_ids: Vec<u32>) -> Vec<u32> {
    assert!(!track_ids.is_empty());

    spawn_write_lock_task(move || {
        let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
        let (reloaded_ids, no_err_ids) = TRACK_LIST.write().reload_tracks(&track_ids);
        let tracklist = TRACK_LIST.read();
        TM.write().reload_tracks(&tracklist, &reloaded_ids);
        no_err_ids.into_iter().map(|x| x as u32).collect()
    })
    .await
}

#[tauri::command]
fn remove_tracks(track_ids: Vec<u32>) {
    assert!(!track_ids.is_empty());

    let track_ids: Vec<_> = track_ids.into_iter().map(|x| x as usize).collect();
    let removed_id_ch_tuples = TRACK_LIST.write().remove_tracks(&track_ids);
    removed_id_ch_tuples.iter().for_each(|(id, ch)| {
        let id_ch_str = format_id_ch(*id, *ch);
        DRAW_SPEC_TASK_ID_MAP.remove(&id_ch_str);
        DRAW_WAV_TASK_ID_MAP.remove(&id_ch_str);
    });
    rayon::spawn_fifo(move || {
        let tracklist = TRACK_LIST.read();
        TM.write().remove_tracks(&tracklist, &removed_id_ch_tuples);
    });
}

#[tauri::command]
async fn apply_track_list_changes() -> Vec<String> {
    let (id_ch_tuples, sr) = spawn_write_lock_task(move || {
        let (updated_id_set, sr) = {
            let tracklist = TRACK_LIST.read();
            TM.write().apply_track_list_changes(&tracklist)
        };
        let updated_ids: Vec<usize> = updated_id_set.into_iter().collect();
        (TRACK_LIST.read().id_ch_tuples_from(&updated_ids), sr)
    })
    .await;
    let id_ch_strs = id_ch_tuples
        .iter()
        .map(|&(id, ch)| format_id_ch(id, ch))
        .collect();
    player::send(PlayerCommand::SetSr(sr)).await;
    id_ch_strs
}

#[tauri::command]
#[allow(non_snake_case)]
fn get_dB_range() -> f64 {
    TM.read().dB_range as f64
}

#[tauri::command]
#[allow(non_snake_case)]
async fn set_dB_range(dB_range: f64) {
    assert!(dB_range > 0.);
    spawn_write_lock_task(move || {
        let tracklist = TRACK_LIST.read();
        TM.write().set_dB_range(&tracklist, dB_range as f32)
    })
    .await;
}

#[tauri::command]
async fn set_colormap_length(colormap_length: u32) {
    assert!(colormap_length > 0);
    spawn_write_lock_task(move || {
        let tracklist = TRACK_LIST.read();
        TM.write().set_colormap_length(&tracklist, colormap_length)
    })
    .await;
}

#[tauri::command]
fn get_spec_setting() -> SpecSetting {
    SPEC_SETTING.read().clone()
}

#[tauri::command]
async fn set_spec_setting(spec_setting: SpecSetting) {
    assert!(spec_setting.win_ms > 0.);
    assert!(spec_setting.t_overlap >= 1);
    assert!(spec_setting.f_overlap >= 1);
    SPEC_SETTING.write().clone_from(&spec_setting);
    spawn_write_lock_task(move || {
        let tracklist = TRACK_LIST.read();
        TM.write().set_setting(&tracklist, &spec_setting)
    })
    .await;
}

#[tauri::command]
fn get_common_guard_clipping() -> GuardClippingMode {
    TRACK_LIST.read().common_guard_clipping
}

#[tauri::command]
async fn set_common_guard_clipping(mode: GuardClippingMode) {
    spawn_write_lock_task(move || {
        TRACK_LIST.write().set_common_guard_clipping(mode);
        let tracklist = TRACK_LIST.read();
        TM.write().update_all_specs_mipmaps(&tracklist);
    })
    .await;
    refresh_track_player().await;
}

#[tauri::command]
fn get_common_normalize() -> NormalizeTarget {
    TRACK_LIST.read().common_normalize
}

#[tauri::command]
async fn set_common_normalize(target: NormalizeTarget) {
    spawn_write_lock_task(move || {
        TRACK_LIST.write().set_common_normalize(target);
        let tracklist = TRACK_LIST.read();
        TM.write().update_all_specs_mipmaps(&tracklist);
    })
    .await;
    refresh_track_player().await;
}

#[tauri::command]
async fn get_spectrogram(id_ch_str: String) -> tauri::Result<serde_json::Value> {
    let (id, ch) = parse_id_ch_str(&id_ch_str)?;

    let out = {
        let tm = TM.read();
        tm.get_spectrogram((id, ch)).map(|spec| {
            let (height, width) = (spec.shape()[0], spec.shape()[1]);
            let arr =
                unsafe { std::mem::transmute::<&[pixels::U16], &[u16]>(spec.as_slice().unwrap()) };
            Spectrogram {
                arr,
                width: width as u32,
                height: height as u32,
            }
        })
    };
    Ok(json!(out))
}

#[tauri::command]
async fn get_mipmap_info(
    id_ch_str: String,
    sec_range: (f64, f64),
    hz_range: (f64, Option<f64>),
    margin_px: u32,
) -> tauri::Result<Option<MipmapInfo>> {
    let (id, ch) = parse_id_ch_str(&id_ch_str)?;
    let sec_range = (sec_range.0.max(0.), sec_range.1);
    if sec_range.0 >= sec_range.1 {
        return Ok(None);
    }

    let task_id = {
        let mut entry = DRAW_SPEC_TASK_ID_MAP
            .entry(id_ch_str.clone())
            .or_insert(Wrapping(0));
        *entry.value_mut() += 1;
        *entry.value()
    };

    let out = tokio_rayon::spawn(move || {
        let track_sec = {
            let tracklist = TRACK_LIST.read();
            tracklist.get(id)?.sec()
        };
        if sec_range.0 >= track_sec {
            return None;
        }
        TM.read().get_mipmap_info(
            (id, ch),
            track_sec,
            sec_range,
            (
                hz_range.0 as f32,
                hz_range.1.unwrap_or(f64::INFINITY) as f32,
            ),
            margin_px as usize,
        )
    })
    .await;

    if DRAW_SPEC_TASK_ID_MAP
        .get(&id_ch_str)
        .is_none_or(|id| *id != task_id)
    {
        return Ok(None); // if new task has been started, return None
    }
    Ok(out)
}

#[tauri::command]
fn get_wav(id_ch_str: String) -> tauri::Result<WavInfo> {
    let (id, ch) = parse_id_ch_str(&id_ch_str)?;
    match TRACK_LIST.read().get(id) {
        Some(track) => {
            let (wav, is_clipped) = track.channel_for_drawing(ch);
            Ok(WavInfo {
                wav: wav.to_vec(),
                sr: track.sr(),
                is_clipped,
            })
        }
        None => Ok(Default::default()),
    }
}

#[tauri::command]
fn find_id_by_path(path: String) -> i32 {
    TRACK_LIST
        .read()
        .find_id_by_path(&path)
        .map_or(-1, |id| id as i32)
}

#[tauri::command]
fn get_limiter_gain(track_id: u32) -> Option<Vec<f32>> {
    let tracklist = TRACK_LIST.read();
    tracklist
        .get(track_id as usize)
        .and_then(|track| track.guard_clipping_gain())
        .map(|gain| gain.to_owned().into_raw_vec_and_offset().0)
}

#[tauri::command]
fn freq_pos_to_hz(y: f64, height: u32, hz_range: (f64, Option<f64>)) -> f64 {
    assert!(height >= 1);

    let hz_range = (
        hz_range.0 as f32,
        hz_range.1.unwrap_or(f64::INFINITY) as f32,
    );
    convert_freq_pos_to_hz(y as f32, height, hz_range) as f64
}

#[tauri::command]
fn freq_hz_to_pos(hz: f64, height: u32, hz_range: (f64, Option<f64>)) -> f64 {
    assert!(height >= 1);

    let hz_range = (
        hz_range.0 as f32,
        hz_range.1.unwrap_or(f64::INFINITY) as f32,
    );
    convert_freq_hz_to_pos(hz as f32, height, hz_range) as f64
}

#[tauri::command]
fn seconds_to_label(sec: f64) -> String {
    convert_sec_to_label(sec)
}

#[tauri::command]
fn time_label_to_seconds(label: String) -> f64 {
    convert_time_label_to_sec(&label).unwrap_or(f64::NAN)
}

#[tauri::command]
fn hz_to_label(hz: f64) -> String {
    convert_hz_to_label(hz as f32)
}

#[tauri::command]
fn freq_label_to_hz(label: String) -> f64 {
    convert_freq_label_to_hz(&label).unwrap_or(f32::NAN) as f64
}

#[tauri::command]
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

#[tauri::command]
fn get_freq_axis_markers(
    max_num_ticks: u32,
    max_num_labels: u32,
    hz_range: (f64, Option<f64>),
    max_track_hz: f64,
) -> serde_json::Value {
    assert_axis_params(max_num_ticks, max_num_labels);

    json!(calc_freq_axis_markers(
        (hz_range.0 as f32, hz_range.1.unwrap_or(max_track_hz) as f32),
        SPEC_SETTING.read().freq_scale,
        max_num_ticks,
        max_num_labels
    ))
}

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
#[allow(non_snake_case)]
fn get_max_dB() -> f64 {
    TM.read().max_dB as f64
}

#[tauri::command]
#[allow(non_snake_case)]
fn get_min_dB() -> f64 {
    TM.read().min_dB as f64
}

#[tauri::command]
fn get_max_track_hz() -> f64 {
    TM.read().max_sr as f64 / 2.
}

#[tauri::command]
fn get_longest_track_length_sec() -> f64 {
    TRACK_LIST.read().max_sec
}

#[tauri::command]
fn get_channel_counts(track_id: u32) -> u32 {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or(0, |track| track.n_ch() as u32)
}

#[tauri::command]
fn get_length_sec(track_id: u32) -> f64 {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or(0., |track| track.sec())
}

#[tauri::command]
fn get_sample_rate(track_id: u32) -> u32 {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or(0, |track| track.sr())
}

#[tauri::command]
fn get_format_info(track_id: u32) -> AudioFormatInfo {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or_else(Default::default, |track| track.format_info.clone())
}

#[tauri::command]
fn get_global_lufs(track_id: u32) -> f64 {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().global_lufs)
}

#[tauri::command]
#[allow(non_snake_case)]
fn get_rms_dB(track_id: u32) -> f64 {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().rms_dB as f64)
}

#[tauri::command]
#[allow(non_snake_case)]
fn get_max_peak_dB(track_id: u32) -> f64 {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or(f64::NEG_INFINITY, |track| track.stats().max_peak_dB as f64)
}

#[tauri::command]
fn get_guard_clip_stats(track_id: u32) -> serde_json::Value {
    let tracklist = TRACK_LIST.read();
    let mode = tracklist.common_guard_clipping;
    let prefix = mode.to_string();
    match tracklist.get(track_id as usize) {
        Some(track) => {
            let format_ch_stat = move |(ch, stat): (isize, &GuardClippingStats)| {
                let stat_str = stat.to_string();
                (!stat_str.is_empty()).then_some((ch, format!("{} by {}", &prefix, stat_str)))
            };
            let vec = track.format_guard_clip_stats(mode, format_ch_stat);
            json!(vec)
        }
        None => json!([]),
    }
}

#[tauri::command]
fn get_path(track_id: u32) -> String {
    TRACK_LIST
        .read()
        .get(track_id as usize)
        .map_or_else(String::new, |track| track.path_string())
}

#[tauri::command]
fn get_file_name(track_id: u32) -> String {
    TRACK_LIST.read().filename(track_id as usize).to_owned()
}

#[tauri::command]
#[allow(non_snake_case)]
async fn set_volume_dB(volume_dB: f64) {
    player::send(PlayerCommand::SetVolumedB(volume_dB)).await;
}

#[tauri::command]
async fn set_track_player(track_id: u32, sec: Option<f64>) {
    let track_id = track_id as usize;
    if TRACK_LIST.read().has(track_id) {
        player::send(PlayerCommand::SetTrack((Some(track_id), sec))).await;
    }
}

#[tauri::command]
async fn seek_player(sec: f64) {
    player::send(PlayerCommand::Seek(sec)).await;
}

#[tauri::command]
async fn pause_player() {
    player::send(PlayerCommand::Pause).await;
}

#[tauri::command]
async fn resume_player() {
    player::send(PlayerCommand::Resume).await;
}

#[tauri::command]
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
        hz_range.1.min(TM.read().max_sr as f32 / 2.), // TODO: remove
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
        hz_range.1.min(TM.read().max_sr as f32 / 2.), // TODO: remove
    );
    let rel_freq = SPEC_SETTING
        .read()
        .freq_scale
        .hz_to_relative_freq(hz, hz_range);
    (1. - rel_freq) * height as f32
}

#[tauri::command]
fn is_dev() -> bool {
    tauri::is_dev()
}

#[tauri::command]
fn get_project_root() -> Option<String> {
    if !tauri::is_dev() {
        return None;
    }
    std::env::current_dir()
        .ok()
        .map(|path| path.parent().unwrap().to_str().unwrap().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus::get_physical())
        .build_global()
        .unwrap();
    // console_subscriber::init();

    let mut builder = tauri::Builder::default();
    #[cfg(debug_assertions)]
    {
        let devtools = tauri_plugin_devtools::init();
        builder = builder.plugin(devtools);
    }
    builder = builder
        .plugin(tauri_plugin_dialog::init())
        .menu(|app| menu::build(app))
        .on_menu_event(|app, event| menu::handle_menu_event(app, event))
        .setup(|app| {
            let handle = app.handle();
            menu::init(&handle)?;

            #[cfg(debug_assertions)]
            {
                if let Some(window) = handle.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            init,
            is_dev,
            get_project_root,
            add_tracks,
            reload_tracks,
            remove_tracks,
            apply_track_list_changes,
            get_dB_range,
            set_dB_range,
            set_colormap_length,
            get_spec_setting,
            set_spec_setting,
            get_common_guard_clipping,
            set_common_guard_clipping,
            get_common_normalize,
            set_common_normalize,
            get_spectrogram,
            get_mipmap_info,
            get_wav,
            find_id_by_path,
            get_limiter_gain,
            freq_pos_to_hz,
            freq_hz_to_pos,
            seconds_to_label,
            time_label_to_seconds,
            hz_to_label,
            freq_label_to_hz,
            get_time_axis_markers,
            get_freq_axis_markers,
            get_amp_axis_markers,
            get_dB_axis_markers,
            get_max_dB,
            get_min_dB,
            get_max_track_hz,
            get_longest_track_length_sec,
            get_channel_counts,
            get_length_sec,
            get_sample_rate,
            get_format_info,
            get_global_lufs,
            get_rms_dB,
            get_max_peak_dB,
            get_guard_clip_stats,
            get_path,
            get_file_name,
            set_volume_dB,
            set_track_player,
            seek_player,
            pause_player,
            resume_player,
            get_player_state,
            menu::enable_edit_menu,
            menu::disable_edit_menu,
            menu::enable_axis_zoom_menu,
            menu::disable_axis_zoom_menu,
            menu::enable_remove_track_menu,
            menu::disable_remove_track_menu,
            menu::enable_play_menu,
            menu::disable_play_menu,
            menu::enable_toggle_play_menu,
            menu::disable_toggle_play_menu,
            menu::show_play_menu,
            menu::show_pause_menu
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
