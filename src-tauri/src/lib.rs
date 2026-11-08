mod commands;
mod config;
mod download;
mod github;
mod models;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::fetch_releases,
            commands::get_config,
            commands::save_config,
            commands::choose_download_dir,
            commands::start_download,
            commands::cancel_download,
            commands::get_download_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gh-downloader");
}
