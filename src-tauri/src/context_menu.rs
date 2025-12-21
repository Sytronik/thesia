use const_format::{formatcp, str_repeat};
use tauri::menu::{ContextMenu, Menu, MenuBuilder, MenuItem};
use tauri::{Manager, Window, Wry};

use crate::menu::{ids, labels};
use crate::os::os_label;

macro_rules! label_and_accelerator {
    ($label:expr, $accelerator_macos:expr, $accelerator_other:expr, $n_tabs:expr) => {
        formatcp!(
            "{}{}({})",
            $label,
            str_repeat!("\t", $n_tabs),
            os_label($accelerator_macos, $accelerator_other)
        )
    };
}

#[tauri::command]
pub fn show_edit_context_menu(window: Window<Wry>) -> Result<(), String> {
    let app = window.app_handle();

    let menu = MenuBuilder::new(app)
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .separator()
        .select_all()
        .build()
        .map_err(|e| e.to_string())?;

    menu.popup(window).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn show_track_context_menu(window: Window<Wry>) -> Result<(), String> {
    let app = window.app_handle();

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(
                app,
                ids::REMOVE_SELECTED_TRACKS,
                label_and_accelerator!(
                    labels::REMOVE_SELECTED_TRACKS,
                    "⌫ | ⌦",
                    "Del | Backspace",
                    1
                ),
                true,
                None::<&str>,
            )
            .unwrap(),
            &MenuItem::with_id(
                app,
                ids::SELECT_ALL_TRACKS,
                label_and_accelerator!(labels::SELECT_ALL_TRACKS, "⌘ A", "Ctrl+A", 3),
                true,
                None::<&str>,
            )
            .unwrap(),
        ],
    )
    .map_err(|e| e.to_string())?;

    menu.popup(window).map_err(|e| e.to_string())
}
