use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::anyhow;
use serde::Serialize;
use tauri::menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewWindow, Wry};
use tauri_plugin_opener::OpenerExt;

use crate::os::os_label;
use crate::player::{PLAY_BIG_JUMP_SEC, PLAY_JUMP_SEC};

pub mod ids {
    pub const FILE: &str = "file-menu";

    #[cfg(target_os = "macos")]
    pub const EDIT: &str = "edit-menu";

    pub const VIEW: &str = "view-menu";
    pub const TRACKS: &str = "tracks-menu";
    pub const PLAY_MENU: &str = "play-menu";

    #[cfg(target_os = "macos")]
    pub const WINDOW: &str = "window-menu";

    pub const HELP: &str = "help-menu";

    pub const OPEN_AUDIO_TRACKS: &str = "open-audio-tracks";

    pub const EDIT_DELETE: &str = "edit-delete";

    pub const FREQ_ZOOM_IN: &str = "freq-zoom-in";
    pub const FREQ_ZOOM_OUT: &str = "freq-zoom-out";
    pub const TIME_ZOOM_IN: &str = "time-zoom-in";
    pub const TIME_ZOOM_OUT: &str = "time-zoom-out";

    pub const RELOAD: &str = "reload";
    pub const TOGGLE_DEVTOOLS: &str = "toggle-devtools";

    pub const REMOVE_SELECTED_TRACKS: &str = "remove-selected-tracks";
    pub const SELECT_ALL_TRACKS: &str = "select-all-tracks";

    pub const TOGGLE_PLAY: &str = "toggle-play";
    pub const REWIND: &str = "rewind";
    pub const FAST_FORWARD: &str = "fast-forward";
    pub const REWIND_BIG: &str = "rewind-big";
    pub const FAST_FORWARD_BIG: &str = "fast-forward-big";
    pub const REWIND_TO_FRONT: &str = "rewind-to-front";

    pub const HELP_LEARN_MORE: &str = "help-learn-more";
    pub const HELP_SEARCH_ISSUES: &str = "help-search-issues";

    pub const EDIT_AMP_RANGE: &str = "edit-amp-range";
    pub const EDIT_FREQ_UPPER_LIMIT: &str = "edit-freq-upper-limit";
    pub const EDIT_FREQ_LOWER_LIMIT: &str = "edit-freq-lower-limit";
    pub const RESET_AXIS_RANGE: &str = "reset-axis-range";
}

pub mod labels {
    pub const REMOVE_SELECTED_TRACKS: &str = "Remove Selected Tracks";
    pub const SELECT_ALL_TRACKS: &str = "Select All Tracks";
}

const AXIS_MENU_ITEM_IDS: &[&str] = &[
    ids::FREQ_ZOOM_IN,
    ids::FREQ_ZOOM_OUT,
    ids::TIME_ZOOM_IN,
    ids::TIME_ZOOM_OUT,
];

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    #[cfg(target_os = "macos")]
    let app_menu = build_app_menu(app)?;

    let file_menu = build_file_menu(app)?;

    #[cfg(target_os = "macos")]
    let edit_menu = build_edit_menu(app)?;

    let view_menu = build_view_menu(app)?;
    let tracks_menu = build_tracks_menu(app)?;
    let play_menu = build_play_menu(app)?;

    #[cfg(target_os = "macos")]
    let window_menu = build_window_menu(app)?;

    let help_menu = build_help_menu(app)?;

    let mut top_level: Vec<&dyn IsMenuItem<R>> = Vec::new();

    #[cfg(target_os = "macos")]
    top_level.push(&app_menu);

    top_level.push(&file_menu);

    #[cfg(target_os = "macos")]
    top_level.push(&edit_menu);

    top_level.push(&view_menu);
    top_level.push(&tracks_menu);
    top_level.push(&play_menu);

    #[cfg(target_os = "macos")]
    top_level.push(&window_menu);

    top_level.push(&help_menu);

    Menu::with_items(app, &top_level)
}

#[cfg(target_os = "macos")]
fn build_app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let about = PredefinedMenuItem::about(app, None, None)?;
    let services = PredefinedMenuItem::services(app, None)?;
    let hide = PredefinedMenuItem::hide(app, None)?;
    let hide_others = PredefinedMenuItem::hide_others(app, None)?;
    let show_all = PredefinedMenuItem::show_all(app, None)?;
    let quit = PredefinedMenuItem::quit(app, None)?;

    Submenu::with_items(
        app,
        app.package_info().name.clone(),
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &services,
            &PredefinedMenuItem::separator(app)?,
            &hide,
            &hide_others,
            &show_all,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

fn build_file_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let open_audio_tracks = MenuItem::with_id(
        app,
        ids::OPEN_AUDIO_TRACKS,
        "Open Audio Tracks...",
        true,
        Some("CmdOrCtrl+O"),
    )?;

    let close_window = PredefinedMenuItem::close_window(app, None)?;
    let separator = PredefinedMenuItem::separator(app)?;

    Submenu::with_id_and_items(
        app,
        ids::FILE,
        "File",
        true,
        &[&open_audio_tracks, &separator, &close_window],
    )
}

#[cfg(target_os = "macos")]
fn build_edit_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let undo = PredefinedMenuItem::undo(app, None)?;
    let redo = PredefinedMenuItem::redo(app, None)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, None)?;
    let copy = PredefinedMenuItem::copy(app, None)?;
    let paste = PredefinedMenuItem::paste(app, None)?;
    let delete = MenuItem::with_id(app, ids::EDIT_DELETE, "&Delete", true, None::<&str>)?;
    // select_all is added/removed dynamically by MenuController

    Submenu::with_id_and_items(
        app,
        ids::EDIT,
        "Edit",
        false,
        &[&undo, &redo, &separator, &cut, &copy, &paste, &delete],
    )
}

fn build_view_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let menu = Submenu::with_id(app, ids::VIEW, "View", true)?;

    let freq_zoom_in = MenuItem::with_id(
        app,
        ids::FREQ_ZOOM_IN,
        "Frequency Zoom In",
        false,
        Some("CmdOrCtrl+Down"),
    )?;
    menu.append(&freq_zoom_in)?;

    let freq_zoom_out = MenuItem::with_id(
        app,
        ids::FREQ_ZOOM_OUT,
        "Frequency Zoom Out",
        false,
        Some("CmdOrCtrl+Up"),
    )?;
    menu.append(&freq_zoom_out)?;

    let time_zoom_in = MenuItem::with_id(
        app,
        ids::TIME_ZOOM_IN,
        "Time Zoom In",
        false,
        Some("CmdOrCtrl+Right"),
    )?;
    menu.append(&time_zoom_in)?;

    let time_zoom_out = MenuItem::with_id(
        app,
        ids::TIME_ZOOM_OUT,
        "Time Zoom Out",
        false,
        Some("CmdOrCtrl+Left"),
    )?;
    menu.append(&time_zoom_out)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if cfg!(debug_assertions) {
        let reload = MenuItem::with_id(app, ids::RELOAD, "Reload", true, Some("CmdOrCtrl+R"))?;
        menu.append(&reload)?;

        // menu.append(&build_fullscreen_menu_item(app)?)?;

        let toggle_devtools = MenuItem::with_id(
            app,
            ids::TOGGLE_DEVTOOLS,
            "Toggle Developer Tools",
            true,
            Some(os_label("Alt+Command+I", "Ctrl+Shift+I")),
        )?;
        menu.append(&toggle_devtools)?;
    } /* else {
    menu.append(&build_fullscreen_menu_item(app)?)?;
    } */

    Ok(menu)
}

fn build_tracks_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let remove_selected_tracks = MenuItem::with_id(
        app,
        ids::REMOVE_SELECTED_TRACKS,
        labels::REMOVE_SELECTED_TRACKS,
        false,
        Some("Backspace"),
    )?;

    let select_all_tracks = MenuItem::with_id(
        app,
        ids::SELECT_ALL_TRACKS,
        labels::SELECT_ALL_TRACKS,
        true,
        Some("CmdOrCtrl+A"),
    )?;

    Submenu::with_id_and_items(
        app,
        ids::TRACKS,
        "Tracks",
        true,
        &[&remove_selected_tracks, &select_all_tracks],
    )
}

fn build_play_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let toggle_play = MenuItem::with_id(app, ids::TOGGLE_PLAY, "Play", false, Some("Space"))?;
    let rewind = MenuItem::with_id(
        app,
        ids::REWIND,
        format!("Rewind {:.0}s", PLAY_JUMP_SEC),
        false,
        Some(","),
    )?;
    let fast_forward = MenuItem::with_id(
        app,
        ids::FAST_FORWARD,
        format!("Fast Forward {:.0}s", PLAY_JUMP_SEC),
        false,
        Some("."),
    )?;
    let rewind_big = MenuItem::with_id(
        app,
        ids::REWIND_BIG,
        format!("Rewind {:.0}s", PLAY_BIG_JUMP_SEC),
        false,
        Some("Shift+,"),
    )?;
    let fast_forward_big = MenuItem::with_id(
        app,
        ids::FAST_FORWARD_BIG,
        format!("Fast Forward {:.0}s", PLAY_BIG_JUMP_SEC),
        false,
        Some("Shift+."),
    )?;
    let rewind_to_front = MenuItem::with_id(
        app,
        ids::REWIND_TO_FRONT,
        "Rewind to the Front",
        false,
        Some("Enter"), // TODO: check if this is correct
    )?;

    Submenu::with_id_and_items(
        app,
        ids::PLAY_MENU,
        "Play",
        true,
        &[
            &toggle_play,
            &PredefinedMenuItem::separator(app)?,
            &rewind,
            &fast_forward,
            &rewind_big,
            &fast_forward_big,
            &PredefinedMenuItem::separator(app)?,
            &rewind_to_front,
        ],
    )
}

#[cfg(target_os = "macos")]
fn build_window_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let minimize = PredefinedMenuItem::minimize(app, None)?;
    let maximize = PredefinedMenuItem::maximize(app, None)?;
    let close = PredefinedMenuItem::close_window(app, None)?;

    Submenu::with_id_and_items(
        app,
        ids::WINDOW,
        "Window",
        true,
        &[
            &minimize,
            &maximize,
            &PredefinedMenuItem::separator(app)?,
            &close,
        ],
    )
}

fn build_help_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let learn_more = MenuItem::with_id(
        app,
        ids::HELP_LEARN_MORE,
        "Learn More...",
        true,
        None::<&str>,
    )?;
    let search_issues = MenuItem::with_id(
        app,
        ids::HELP_SEARCH_ISSUES,
        "Search Issues...",
        true,
        None::<&str>,
    )?;

    Submenu::with_id_and_items(app, ids::HELP, "Help", true, &[&learn_more, &search_issues])
}

pub fn init(app: &AppHandle<Wry>) -> tauri::Result<()> {
    let controller = MenuController::new(app)?;
    app.manage(controller);
    Ok(())
}

pub fn handle_menu_event(app: &AppHandle<Wry>, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        ids::OPEN_AUDIO_TRACKS
        | ids::EDIT_DELETE
        | ids::FREQ_ZOOM_IN
        | ids::FREQ_ZOOM_OUT
        | ids::TIME_ZOOM_IN
        | ids::TIME_ZOOM_OUT
        | ids::REMOVE_SELECTED_TRACKS
        | ids::SELECT_ALL_TRACKS
        | ids::TOGGLE_PLAY
        | ids::REWIND_TO_FRONT => emit_simple(app, id),

        ids::REWIND => emit_jump_event(app, JumpPlayerMode::Rewind),
        ids::FAST_FORWARD => emit_jump_event(app, JumpPlayerMode::FastForward),
        ids::REWIND_BIG => emit_jump_event(app, JumpPlayerMode::RewindBig),
        ids::FAST_FORWARD_BIG => emit_jump_event(app, JumpPlayerMode::FastForwardBig),

        ids::RELOAD => with_main_window(app, |window| {
            let _ = window.reload();
        }),

        ids::TOGGLE_DEVTOOLS => with_main_window(app, |_window| {
            #[cfg(debug_assertions)]
            {
                if _window.is_devtools_open() {
                    _window.close_devtools();
                } else {
                    _window.open_devtools();
                }
            }
        }),
        ids::HELP_LEARN_MORE => {
            let _ = app
                .opener()
                .open_url("https://github.com/Sytronik/thesia", None::<&str>);
        }

        ids::HELP_SEARCH_ISSUES => {
            let _ = app
                .opener()
                .open_url("https://github.com/Sytronik/thesia/issues", None::<&str>);
        }

        _ if id.starts_with(ids::EDIT_AMP_RANGE)
            || id.starts_with(ids::EDIT_FREQ_UPPER_LIMIT)
            || id.starts_with(ids::EDIT_FREQ_LOWER_LIMIT)
            || id.starts_with(ids::RESET_AXIS_RANGE) =>
        {
            emit_simple(app, id)
        }

        _ => log::warn!("unhandled menu event: {id}"),
    }
}

fn with_main_window<F: FnOnce(&WebviewWindow<Wry>)>(app: &AppHandle<Wry>, f: F) {
    if let Some(window) = app.get_webview_window("main") {
        f(&window);
    }
}

fn emit_simple(app: &AppHandle<Wry>, event: &str) {
    with_main_window(app, |window| {
        let _ = window.emit(event, ());
    });
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
enum JumpPlayerMode {
    FastForward,
    Rewind,
    FastForwardBig,
    RewindBig,
}

fn emit_jump_event(app: &AppHandle<Wry>, mode: JumpPlayerMode) {
    with_main_window(app, |window| {
        let _ = window.emit("jump-player", mode);
    });
}

pub struct MenuController<R: Runtime> {
    #[cfg(target_os = "macos")]
    edit_menu: Submenu<R>,
    #[cfg(target_os = "macos")]
    select_all: PredefinedMenuItem<R>,
    #[cfg(target_os = "macos")]
    tracks_menu: Submenu<R>,
    #[cfg(target_os = "macos")]
    select_all_tracks: MenuItem<R>,
    axis_items: Vec<MenuItem<R>>,
    remove_selected_tracks: MenuItem<R>,
    play_items: Vec<MenuItem<R>>,
    toggle_play_item: MenuItem<R>,
    toggle_play_enabled: AtomicBool,
    play_menu_enabled: AtomicBool,
    is_playing: AtomicBool,
}

impl<R: Runtime> MenuController<R> {
    fn new(app: &AppHandle<R>) -> tauri::Result<Self> {
        let menu = app
            .menu()
            .ok_or_else(|| tauri::Error::Anyhow(anyhow!("application menu not initialized")))?;

        #[cfg(target_os = "macos")]
        let edit_menu = find_submenu(&menu, ids::EDIT)?;
        let axis_menu = find_submenu(&menu, ids::VIEW)?;
        let tracks_menu = find_submenu(&menu, ids::TRACKS)?;
        let play_menu = find_submenu(&menu, ids::PLAY_MENU)?;

        #[cfg(target_os = "macos")]
        // Create select_all PredefinedMenuItem for dynamic add/remove
        let select_all = PredefinedMenuItem::select_all(app, None)?;

        let mut axis_items = Vec::new();
        for id in AXIS_MENU_ITEM_IDS {
            axis_items.push(find_menu_item(&axis_menu, id)?);
        }

        let remove_selected_tracks = find_menu_item(&tracks_menu, ids::REMOVE_SELECTED_TRACKS)?;
        #[cfg(target_os = "macos")]
        let select_all_tracks = find_menu_item(&tracks_menu, ids::SELECT_ALL_TRACKS)?;

        let toggle_play_item = find_menu_item(&play_menu, ids::TOGGLE_PLAY)?;
        let play_items = collect_menu_items(&play_menu)?;

        Ok(Self {
            #[cfg(target_os = "macos")]
            edit_menu,
            #[cfg(target_os = "macos")]
            select_all,
            #[cfg(target_os = "macos")]
            tracks_menu,
            #[cfg(target_os = "macos")]
            select_all_tracks,
            axis_items,
            remove_selected_tracks,
            play_items,
            toggle_play_item,
            toggle_play_enabled: AtomicBool::new(false),
            play_menu_enabled: AtomicBool::new(false),
            is_playing: AtomicBool::new(false),
        })
    }

    #[cfg(target_os = "macos")]
    pub fn set_edit_menu_enabled(&self, enabled: bool) -> tauri::Result<()> {
        self.edit_menu.set_enabled(enabled)?;
        if enabled {
            // Show select_all in edit menu, hide select_all_tracks in tracks menu
            self.edit_menu.append(&self.select_all)?;
            self.tracks_menu.remove(&self.select_all_tracks)?;
        } else {
            // Hide select_all from edit menu, show select_all_tracks in tracks menu
            let _ = self.edit_menu.remove(&self.select_all); // ignore error if not present
            self.tracks_menu.append(&self.select_all_tracks)?;
        }
        Ok(())
    }

    pub fn set_axis_zoom_menu_enabled(&self, enabled: bool) -> tauri::Result<()> {
        for item in &self.axis_items {
            item.set_enabled(enabled)?;
        }
        Ok(())
    }

    pub fn set_remove_track_menu_enabled(&self, enabled: bool) -> tauri::Result<()> {
        self.remove_selected_tracks.set_enabled(enabled)
    }

    pub fn set_play_menu_enabled(&self, enabled: bool) -> tauri::Result<()> {
        self.play_menu_enabled.store(enabled, Ordering::SeqCst);
        for item in &self.play_items {
            let id = item.id();
            if id == ids::TOGGLE_PLAY {
                continue;
            }
            item.set_enabled(enabled)?;
        }
        self.apply_toggle_state()
    }

    pub fn set_toggle_play_menu_enabled(&self, enabled: bool) -> tauri::Result<()> {
        self.toggle_play_enabled.store(enabled, Ordering::SeqCst);
        self.apply_toggle_state()
    }

    pub fn show_play_menu(&self) -> tauri::Result<()> {
        self.is_playing.store(false, Ordering::SeqCst);
        self.apply_toggle_state()
    }

    pub fn show_pause_menu(&self) -> tauri::Result<()> {
        self.is_playing.store(true, Ordering::SeqCst);
        self.apply_toggle_state()
    }

    fn apply_toggle_state(&self) -> tauri::Result<()> {
        if !self.play_menu_enabled.load(Ordering::SeqCst) {
            self.toggle_play_item.set_enabled(false)?;
            self.toggle_play_item.set_text("Play")?;
            return Ok(());
        }

        if self.toggle_play_enabled.load(Ordering::SeqCst) {
            if self.is_playing.load(Ordering::SeqCst) {
                self.toggle_play_item.set_enabled(true)?;
                self.toggle_play_item.set_text("Pause")?;
            } else {
                self.toggle_play_item.set_enabled(true)?;
                self.toggle_play_item.set_text("Play")?;
            }
        } else {
            self.toggle_play_item.set_enabled(false)?;
        }

        Ok(())
    }
}

fn find_submenu<R: Runtime>(menu: &Menu<R>, id: &str) -> tauri::Result<Submenu<R>> {
    menu.items()? // Result<Vec<MenuItemKind>>
        .into_iter()
        .find_map(|item| {
            if item.id() == id {
                item.as_submenu().cloned()
            } else {
                None
            }
        })
        .ok_or_else(|| tauri::Error::Anyhow(anyhow!("submenu `{id}` not found")))
}

fn find_menu_item<R: Runtime>(submenu: &Submenu<R>, id: &str) -> tauri::Result<MenuItem<R>> {
    submenu
        .items()?
        .into_iter()
        .find_map(|item| {
            if item.id() == id {
                item.as_menuitem().cloned()
            } else {
                None
            }
        })
        .ok_or_else(|| tauri::Error::Anyhow(anyhow!("menu item `{id}` not found")))
}

fn collect_menu_items<R: Runtime>(submenu: &Submenu<R>) -> tauri::Result<Vec<MenuItem<R>>> {
    Ok(submenu
        .items()?
        .into_iter()
        .filter_map(|item| item.as_menuitem().cloned())
        .collect())
}

#[tauri::command]
pub fn enable_edit_menu(_controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return _controller
            .set_edit_menu_enabled(true)
            .map_err(|e| e.to_string());
    }
    #[cfg(not(target_os = "macos"))]
    Ok(())
}

#[tauri::command]
pub fn disable_edit_menu(_controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return _controller
            .set_edit_menu_enabled(false)
            .map_err(|e| e.to_string());
    }
    #[cfg(not(target_os = "macos"))]
    Ok(())
}

#[tauri::command]
pub fn enable_axis_zoom_menu(controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    controller
        .set_axis_zoom_menu_enabled(true)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disable_axis_zoom_menu(controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    controller
        .set_axis_zoom_menu_enabled(false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn enable_remove_track_menu(
    controller: tauri::State<MenuController<Wry>>,
) -> Result<(), String> {
    controller
        .set_remove_track_menu_enabled(true)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disable_remove_track_menu(
    controller: tauri::State<MenuController<Wry>>,
) -> Result<(), String> {
    controller
        .set_remove_track_menu_enabled(false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn enable_play_menu(controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    controller
        .set_play_menu_enabled(true)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disable_play_menu(controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    controller
        .set_play_menu_enabled(false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn enable_toggle_play_menu(
    controller: tauri::State<MenuController<Wry>>,
) -> Result<(), String> {
    controller
        .set_toggle_play_menu_enabled(true)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disable_toggle_play_menu(
    controller: tauri::State<MenuController<Wry>>,
) -> Result<(), String> {
    controller
        .set_toggle_play_menu_enabled(false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn show_play_menu(controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    controller.show_play_menu().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn show_pause_menu(controller: tauri::State<MenuController<Wry>>) -> Result<(), String> {
    controller.show_pause_menu().map_err(|e| e.to_string())
}
