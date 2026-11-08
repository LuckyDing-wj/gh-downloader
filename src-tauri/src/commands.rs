use crate::{
    config, download, github,
    models::{AppConfig, DownloadTask, Repository},
    state::AppState,
};
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

#[tauri::command]
pub async fn fetch_releases(url: String, page: u32) -> Result<Repository, String> {
    let config = config::load()?;
    github::fetch(&url, page.max(1), &config).await
}

#[tauri::command]
pub fn get_config() -> Result<AppConfig, String> {
    config::load()
}

#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<AppConfig, String> {
    config::save(&config)
}

#[tauri::command]
pub async fn choose_download_dir(app: AppHandle) -> Result<Option<String>, String> {
    let (sender, receiver) = oneshot::channel();
    app.dialog()
        .file()
        .set_title("选择下载目录")
        .pick_folder(move |selected| {
            let _ = sender.send(selected);
        });
    let selected = receiver.await.map_err(|e| e.to_string())?;
    Ok(selected.map(|path| path.to_string()))
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    owner: String,
    repo: String,
    asset_id: i64,
) -> Result<DownloadTask, String> {
    let config = config::load()?;
    download::start(app, state, config, owner, repo, asset_id).await
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    download::cancel(state, task_id).await
}

#[tauri::command]
pub async fn get_download_tasks(state: State<'_, AppState>) -> Result<Vec<DownloadTask>, String> {
    let tasks: HashMap<_, _> = state.tasks.lock().await.clone();
    Ok(tasks.into_values().collect())
}
