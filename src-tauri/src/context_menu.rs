use const_format::{concatcp, str_repeat};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::menu::{ContextMenu, IsMenuItem, Menu, MenuBuilder, MenuItem};
use tauri::{Manager, Window, Wry};

use crate::menu::{ids, labels};
use crate::os::os_label;

macro_rules! label_and_accelerator {
    ($label:expr, $accelerator_macos:expr, $accelerator_other:expr, $n_tabs:expr) => {
        concatcp!(
            $label,
            str_repeat!("\t", $n_tabs),
            "(",
            os_label($accelerator_macos, $accelerator_other),
            ")"
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
pub fn show_track_context_menu(window: Window<Wry>) -> Result<(), tauri::Error> {
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
            )?,
            &MenuItem::with_id(
                app,
                ids::SELECT_ALL_TRACKS,
                label_and_accelerator!(labels::SELECT_ALL_TRACKS, "⌘ A", "Ctrl+A", 3),
                true,
                None::<&str>,
            )?,
        ],
    )?;

    menu.popup(window)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AxisKind {
    AmpAxis,
    FreqAxis,
    TimeRuler,
    #[serde(rename = "dBAxis")]
    #[allow(non_camel_case_types)]
    dBAxis,
}

#[tauri::command]
pub fn show_axis_context_menu(
    window: Window<Wry>,
    axis_kind: AxisKind,
    id: usize,
) -> Result<(), tauri::Error> {
    let app = window.app_handle();
    let mut items = match axis_kind {
        AxisKind::AmpAxis => vec![MenuItem::with_id(
            app,
            format!("{}-{}", ids::EDIT_AMP_RANGE, id),
            label_and_accelerator!("Edit Range", "Double Click", "Double Click", 2),
            true,
            None::<&str>,
        )?],
        AxisKind::FreqAxis => vec![
            MenuItem::with_id(
                app,
                format!("{}-{}", ids::EDIT_FREQ_UPPER_LIMIT, id),
                label_and_accelerator!("Edit Upper Limit", "Double Click", "Double Click", 1),
                true,
                None::<&str>,
            )?,
            MenuItem::with_id(
                app,
                format!("{}-{}", ids::EDIT_FREQ_LOWER_LIMIT, id),
                label_and_accelerator!("Edit Lower Limit", "Double Click", "Double Click", 1),
                true,
                None::<&str>,
            )?,
        ],
        AxisKind::TimeRuler => vec![],
        AxisKind::dBAxis => unimplemented!(),
    };
    items.push(MenuItem::with_id(
        app,
        format!(
            "{}-{}",
            ids::RESET_AXIS_RANGE,
            json!(axis_kind).as_str().unwrap().replace("\"", "")
        ), // reset axis is processed in MaiViewer
        label_and_accelerator!("Reset Range", "⌥ Click", "Alt+Click", 2),
        true,
        None::<&str>,
    )?);

    let menu = Menu::with_items(
        app,
        &items
            .iter()
            .map(|item| item as &dyn IsMenuItem<Wry>)
            .collect::<Vec<_>>(),
    )?;

    menu.popup(window)
}
