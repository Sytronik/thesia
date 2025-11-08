use tauri::Manager;

mod menu;

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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(|app| menu::build(app))
        .on_menu_event(|app, event| menu::handle_menu_event(app, event))
        .setup(|app| {
            let handle = app.handle();
            menu::init(&handle)?;

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            is_dev,
            get_project_root,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
