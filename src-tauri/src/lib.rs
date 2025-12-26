// need to statically link OpenBLAS on Windows
extern crate blas_src;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::LazyLock;

use parking_lot::RwLock;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_store::StoreExt;

mod backend;
mod context_menu;
mod interface;
mod menu;
mod os;
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

// TODO: prevent making mistake not to update the values below. Maybe sth like auto-sync?
static SPEC_SETTING: RwLock<SpecSetting> = RwLock::new(SpecSetting::new());

const OPEN_FILES_DIALOG_PATH_KEY: &str = "openFilesDialogPath";

#[tauri::command]
fn is_dev() -> bool {
    tauri::is_dev()
}

#[tauri::command]
fn init(app: AppHandle, colormap_length: u32) -> tauri::Result<ConstsAndUserSettings> {
    let user_settings = get_user_settings(&app)?;
    let user_settings = {
        let mut tracklist = TRACK_LIST.write();
        let mut tm = TM.write();
        if !tracklist.is_empty() {
            *tracklist = TrackList::new();
            *tm = TrackManager::new();
        }
        tm.set_colormap_length(&tracklist, colormap_length);
        if let Some(setting) = &user_settings.spec_setting {
            tm.set_setting(&tracklist, setting);
        }
        #[allow(non_snake_case)]
        if let Some(dB_range) = &user_settings.dB_range {
            tm.set_dB_range(&tracklist, *dB_range as f32);
        }
        if let Some(mode) = &user_settings.common_guard_clipping {
            tracklist.set_common_guard_clipping(*mode);
        }
        if let Some(target) = &user_settings.common_normalize {
            tracklist.set_common_normalize(*target);
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

    let settings_json = json!(user_settings);
    set_user_settings(app, settings_json)?;

    player::spawn_task();
    Ok(ConstsAndUserSettings {
        constants: Default::default(),
        user_settings,
    })
}

fn get_user_settings(app: &AppHandle) -> tauri::Result<UserSettingsOptionals> {
    let store = app.store("settings.json").unwrap();
    let user_settings_entries = store.entries();
    let default_json = json!(UserSettingsOptionals::default());
    let keys = default_json
        .as_object()
        .unwrap()
        .keys()
        .collect::<HashSet<_>>();
    let user_settings = user_settings_entries
        .into_iter()
        .filter_map(|(key, value)| {
            if keys.contains(&key) {
                Some((key, value))
            } else {
                log::warn!("Store has unexpected key: {}", key);
                None
            }
        })
        .collect::<HashMap<_, _>>();
    let user_settings: UserSettingsOptionals = serde_json::from_value(json!(user_settings))?;

    Ok(user_settings)
}

#[tauri::command]
fn set_user_settings(app: AppHandle, settings: serde_json::Value) -> tauri::Result<()> {
    let store = app.store("settings.json").unwrap();
    for (key, value) in settings.as_object().unwrap() {
        if value.is_null() {
            continue;
        }
        if !store.has(key) {
            return Err(tauri::Error::Anyhow(anyhow::anyhow!(
                "Key {} not found in store",
                key
            )));
        }
        store.set(key, value.clone());
    }
    Ok(())
}

#[tauri::command]
async fn get_open_files_dialog_path(app: AppHandle) -> String {
    let store = app.store("paths.json").unwrap();
    if !store.has(OPEN_FILES_DIALOG_PATH_KEY) {
        let path = if tauri::is_dev() {
            get_project_root().join("samples")
        } else {
            std::env::home_dir().unwrap_or_default()
        };
        store.set(
            OPEN_FILES_DIALOG_PATH_KEY,
            path.to_string_lossy().to_owned(),
        );
    }
    let path = store.get(OPEN_FILES_DIALOG_PATH_KEY).unwrap();
    path.to_string()
}

#[tauri::command]
async fn set_open_files_dialog_path(app: AppHandle, path: String) {
    let store = app.store("paths.json").unwrap();
    store.set(OPEN_FILES_DIALOG_PATH_KEY, path);
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

    let track_sec = {
        let tracklist = TRACK_LIST.read();
        match tracklist.get(id) {
            Some(track) => track.sec(),
            None => return Ok(serde_json::Value::Null),
        }
    };

    let tm = TM.read();
    tm.get_spectrogram((id, ch))
        .map_or(Ok(serde_json::Value::Null), |spec| {
            let (height, width) = (spec.shape()[0], spec.shape()[1]);
            let arr = spec.as_slice().unwrap();
            Ok(json!(Spectrogram {
                arr,
                width: width as u32,
                height: height as u32,
                track_sec,
            }))
        })
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
async fn refresh_track_player() {
    player::send(PlayerCommand::SetTrack((None, None))).await;
}

fn get_project_root() -> PathBuf {
    let curr_dir = std::env::current_dir().unwrap();
    curr_dir.parent().unwrap().to_owned()
}

#[cfg(any(windows, target_os = "linux"))]
fn parse_args(args: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // NOTICE: `args` may include URL protocol (`your-app-protocol://`)
    // or arguments (`--`) if your app supports them.
    // files may also be passed as `file://path/to/file`
    for maybe_file in args.into_iter().skip(1) {
        // skip flags like -f or --flag
        if maybe_file.starts_with('-') {
            continue;
        }

        // handle `file://` path urls and skip other urls
        if let Ok(url) = tauri::Url::parse(&maybe_file) {
            if let Ok(path) = url.to_file_path() {
                files.push(path);
            }
        } else {
            files.push(PathBuf::from(maybe_file))
        }
    }
    files
}

fn handle_file_associations(app: &AppHandle, files: Vec<PathBuf>, by_event: bool) {
    if files.is_empty() {
        return;
    }
    // -- Scope handling start --

    // You can remove this block if you only want to know about the paths, but not actually "use" them in the frontend.

    // This requires the `fs` tauri plugin and is required to make the plugin's frontend work:
    // use tauri_plugin_fs::FsExt;
    // let fs_scope = app.fs_scope();

    // This is for the `asset:` protocol to work:
    let asset_protocol_scope = app.asset_protocol_scope();

    for file in &files {
        // This requires the `fs` plugin:
        // let _ = fs_scope.allow_file(file);

        // This is for the `asset:` protocol:
        let _ = asset_protocol_scope.allow_file(file);
    }

    // -- Scope handling end --

    let window = app.get_webview_window("main").unwrap();
    if by_event {
        let _ = window.emit("open-files", files.as_slice());
    } else {
        let files = files
            .iter()
            .map(|f| {
                let file = f.to_string_lossy().replace('\\', "\\\\"); // escape backslash
                format!("\"{file}\"",) // wrap in quotes for JS array
            })
            .collect::<Vec<_>>()
            .join(",");

        let _ = window.eval(format!("window.openedFiles=[{files}]"));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut _dev_default_open_path: Option<PathBuf> = None;

    #[cfg(debug_assertions)]
    {
        _dev_default_open_path = Some(PathBuf::from(
            get_project_root().join("samples/stereo/sample_48k.wav"),
        ));
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus::get_physical())
        .build_global()
        .unwrap();
    // console_subscriber::init();

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let window = app.get_webview_window("main").expect("no main window");
            // TODO: need to check
            #[cfg(any(windows, target_os = "linux"))]
            {
                let mut files = parse_args(_args);
                handle_file_associations(app.handle(), files, true);
            }
            let _ = window.set_focus();
        }));
    }

    #[cfg(debug_assertions)]
    {
        let devtools = tauri_plugin_devtools::init();
        builder = builder.plugin(devtools);
    }

    builder = builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .menu(|app| menu::build(app))
        .on_menu_event(|app, event| menu::handle_menu_event(app, event))
        .setup(|app| {
            let handle = app.handle().clone();
            menu::init(&handle)?;

            // just put the store in the app's resource table
            let _ = app.store("settings.json")?;
            let _ = app.store("paths.json")?;

            log::info!(
                "settings store path: {}",
                tauri_plugin_store::resolve_store_path(&handle, "settings.json")
                    .unwrap()
                    .display()
            );

            // TODO: need to check
            #[cfg(any(windows, target_os = "linux"))]
            {
                let mut files = parse_args(std::env::args());
                #[cfg(debug_assertions)]
                {
                    if let Some(default_open_path) = _dev_default_open_path
                        && files.is_empty()
                    {
                        files.push(default_open_path);
                    }
                }
                handle_file_associations(&handle, files, false);
            }

            #[cfg(target_os = "macos")]
            {
                // Note that get_current's return value will also get updated every time on_open_url gets triggered.
                let start_urls = app.deep_link().get_current()?;
                let mut files = start_urls.map_or_else(Vec::new, |urls| {
                    // app was likely started by a deep link
                    urls.into_iter()
                        .filter_map(|url| url.to_file_path().ok())
                        .collect::<Vec<_>>()
                });

                #[cfg(debug_assertions)]
                {
                    if let Some(default_open_path) = _dev_default_open_path
                        && files.is_empty()
                    {
                        files.push(default_open_path);
                    }
                }
                handle_file_associations(&handle, files, false);

                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    let files = event
                        .urls()
                        .into_iter()
                        .filter_map(|url| url.to_file_path().ok())
                        .collect::<Vec<_>>();

                    handle_file_associations(&handle, files, true);
                });
            }

            #[cfg(debug_assertions)]
            {
                if let Some(window) = handle.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            is_dev,
            init,
            set_user_settings,
            get_open_files_dialog_path,
            set_open_files_dialog_path,
            add_tracks,
            reload_tracks,
            remove_tracks,
            apply_track_list_changes,
            get_dB_range,
            set_dB_range,
            get_spec_setting,
            set_spec_setting,
            get_common_guard_clipping,
            set_common_guard_clipping,
            get_common_normalize,
            set_common_normalize,
            get_spectrogram,
            get_wav,
            find_id_by_path,
            get_limiter_gain,
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
            menu::show_pause_menu,
            context_menu::show_edit_context_menu,
            context_menu::show_axis_context_menu,
            context_menu::show_track_context_menu,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
