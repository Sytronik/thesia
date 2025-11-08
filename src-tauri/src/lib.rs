use tauri::Manager;

mod menu;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .menu(|app| menu::build(app))
        .on_menu_event(|app, event| menu::handle_menu_event(app, event))
        .setup(|app| {
            let handle = app.handle();
            menu::init(&handle)?;

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
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
