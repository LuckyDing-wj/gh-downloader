use crate::{
    github,
    models::{AppConfig, DownloadStatus, DownloadTask},
    state::AppState,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use tauri::{AppHandle, Emitter, State};
use tokio::{
    fs::{self, File, OpenOptions},
    io::AsyncWriteExt,
    sync::{watch, Mutex},
};
use uuid::Uuid;

const MAX_TASK_HISTORY: usize = 100;
const PROGRESS_EVENT: &str = "download-progress";
type TaskMap = Arc<Mutex<HashMap<String, DownloadTask>>>;
struct DownloadJob {
    app: AppHandle,
    config: AppConfig,
    token: Option<String>,
    asset: crate::models::Asset,
    task: DownloadTask,
    part: PathBuf,
    lock: std::fs::File,
    downloaded: u64,
    file: File,
    cancel: watch::Receiver<bool>,
    tasks: TaskMap,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ResumeMetadata {
    asset_id: i64,
    asset_size: u64,
    asset_name: String,
    etag: Option<String>,
    last_modified: Option<String>,
}

pub async fn start(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
    owner: String,
    repo: String,
    asset_id: i64,
    token: Option<String>,
) -> Result<DownloadTask, String> {
    let asset = github::resolve_asset(&owner, &repo, asset_id, &config, token.as_deref()).await?;
    let safe_name = sanitize_filename(&asset.name)?;
    if asset.size as u64 > github::MAX_DOWNLOAD_BYTES {
        return Err("文件超过允许的最大下载大小（2 GiB）".to_owned());
    }

    let dir = PathBuf::from(&config.download_dir);
    fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let (destination, part, lock, file, offset) =
        reserve_destination(&dir, &safe_name, asset.size as u64).await?;
    let id = Uuid::new_v4().to_string();
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let task = DownloadTask {
        id: id.clone(),
        revision: 0,
        asset_name: safe_name,
        destination: destination.to_string_lossy().into_owned(),
        downloaded: offset as i64,
        total: asset.size,
        speed: 0,
        status: DownloadStatus::Downloading,
        error: None,
    };
    let tasks = state.tasks.clone();
    let cancellations = state.cancellations.clone();
    {
        let mut table = tasks.lock().await;
        table.insert(id.clone(), task.clone());
        prune_tasks(&mut table);
    }
    cancellations.lock().await.insert(id.clone(), cancel_tx);

    let returned_task = task.clone();
    let fallback_task = task.clone();
    let app_for_job = app.clone();
    let tasks_for_job = tasks.clone();
    tokio::spawn(async move {
        let result = run(DownloadJob {
            app,
            config,
            token,
            asset,
            task,
            part,
            lock,
            downloaded: offset,
            file,
            cancel: cancel_rx,
            tasks,
        })
        .await;
        if let Err(error) = result {
            let mut latest = tasks_for_job
                .lock()
                .await
                .get(&id)
                .cloned()
                .unwrap_or(fallback_task);
            latest.status = DownloadStatus::Failed;
            latest.error = Some(error);
            publish(&app_for_job, &tasks_for_job, latest).await;
        }
        cancellations.lock().await.remove(&id);
    });
    Ok(returned_task)
}

async fn run(job: DownloadJob) -> Result<(), String> {
    let DownloadJob {
        app,
        config,
        token,
        asset,
        mut task,
        part,
        lock,
        mut downloaded,
        mut file,
        mut cancel,
        tasks,
    } = job;
    let url = github::validate_asset_url(&asset.download_url)?;
    let client = github::client(&config.proxy_mode, &config.proxy_url)?;
    let metadata_path = metadata_path(&part);
    let mut metadata = read_resume_metadata(&metadata_path).await;
    if metadata.asset_id != asset.id
        || metadata.asset_size != asset.size as u64
        || metadata.asset_name != asset.name
    {
        downloaded = 0;
        file.set_len(0).await.map_err(|e| e.to_string())?;
        metadata = ResumeMetadata {
            asset_id: asset.id,
            asset_size: asset.size as u64,
            asset_name: asset.name.clone(),
            etag: None,
            last_modified: None,
        };
        save_resume_metadata(&metadata_path, &metadata).await?;
    }
    let validator = metadata
        .etag
        .as_deref()
        .or(metadata.last_modified.as_deref());
    let response = tokio::select! {
        changed = cancel.changed() => {
            if changed.is_ok() && *cancel.borrow() {
                file.flush().await.map_err(|e| e.to_string())?;
                return mark_cancelled(&app, &tasks, task).await;
            }
            return Err("下载取消信号异常".to_owned());
        }
        response = github::open_asset(&client, &url, (downloaded > 0).then_some(downloaded), validator, token.as_deref()) => response?,
    };

    if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && downloaded > 0 {
        let complete = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|value| value.strip_prefix("bytes */"))
            .and_then(|value| value.parse::<u64>().ok());
        if complete == Some(downloaded) && downloaded == asset.size as u64 {
            file.flush().await.map_err(|e| e.to_string())?;
            let destination = PathBuf::from(&task.destination);
            if fs::try_exists(&destination).await.unwrap_or(true) {
                return Err("下载目标文件已存在，为避免覆盖已取消写入".to_owned());
            }
            fs::hard_link(&part, &destination)
                .await
                .map_err(|e| format!("无法安全创建最终文件：{e}"))?;
            drop(file);
            lock.unlock().map_err(|e| e.to_string())?;
            fs::remove_file(part).await.map_err(|e| e.to_string())?;
            let _ = fs::remove_file(metadata_path).await;
            task.status = DownloadStatus::Completed;
            task.downloaded = downloaded as i64;
            task.total = downloaded as i64;
            publish(&app, &tasks, task).await;
            return Ok(());
        }
        return Err("服务器拒绝续传，且本地分片未达到预期完整大小".to_owned());
    }

    if response.status() != reqwest::StatusCode::OK
        && !(downloaded > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT)
    {
        return Err(format!("下载服务器返回意外状态码：{}", response.status()));
    }
    let (resume, total) =
        if downloaded > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            let (range_start, range_end, range_total) = parse_content_range(
                response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok()),
            )?;
            let range_length = range_end.saturating_sub(range_start).saturating_add(1);
            if range_start != downloaded
                || range_end < range_start
                || range_total != asset.size as u64
                || range_end.checked_add(1) != Some(range_total)
                || response
                    .content_length()
                    .is_some_and(|length| length != range_length)
            {
                return Err("服务器返回的续传范围与本地分片不一致".to_owned());
            }
            (true, range_total)
        } else {
            // A 200 response means the server ignored If-Range or Range; restart from byte zero.
            downloaded = 0;
            file.set_len(0).await.map_err(|e| e.to_string())?;
            (
                false,
                response.content_length().unwrap_or(asset.size as u64),
            )
        };
    if total > github::MAX_DOWNLOAD_BYTES || total != asset.size as u64 {
        return Err("服务器返回的文件大小与 GitHub Asset 信息不符".to_owned());
    }
    if !resume {
        metadata.etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        metadata.last_modified = response
            .headers()
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        save_resume_metadata(&metadata_path, &metadata).await?;
    }
    task.downloaded = downloaded as i64;
    task.total = total as i64;
    task.status = DownloadStatus::Downloading;
    publish(&app, &tasks, task.clone()).await;

    let mut stream = response.bytes_stream();
    let started = Instant::now();
    loop {
        let next = tokio::select! {
            changed = cancel.changed() => {
                if changed.is_ok() && *cancel.borrow() {
                file.flush().await.map_err(|e| e.to_string())?;
                return mark_cancelled(&app, &tasks, task).await;
            }
                return Err("下载取消信号异常".to_owned());
            }
            next = stream.next() => next,
        };
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|e| e.to_string())?;
        downloaded = downloaded
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| "下载大小溢出".to_owned())?;
        if downloaded > github::MAX_DOWNLOAD_BYTES || downloaded > total {
            return Err("服务器发送的数据超过允许大小".to_owned());
        }
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        task.downloaded = downloaded as i64;
        task.speed = (downloaded as f64 / started.elapsed().as_secs_f64().max(0.1)) as i64;
        publish(&app, &tasks, task.clone()).await;
    }
    if downloaded != total {
        return Err(format!(
            "下载不完整：收到 {downloaded} 字节，预期 {total} 字节"
        ));
    }
    file.flush().await.map_err(|e| e.to_string())?;
    let destination = PathBuf::from(&task.destination);
    if fs::try_exists(&destination).await.unwrap_or(true) {
        return Err("下载目标文件已存在，为避免覆盖已取消写入".to_owned());
    }
    fs::hard_link(&part, &destination)
        .await
        .map_err(|e| format!("无法安全创建最终文件：{e}"))?;
    drop(file);
    lock.unlock().map_err(|e| e.to_string())?;
    fs::remove_file(&part).await.map_err(|e| e.to_string())?;
    let _ = fs::remove_file(metadata_path).await;
    task.status = DownloadStatus::Completed;
    task.speed = 0;
    publish(&app, &tasks, task).await;
    Ok(())
}

fn parse_content_range(value: Option<&str>) -> Result<(u64, u64, u64), String> {
    let value = value.ok_or_else(|| "续传响应缺少 Content-Range".to_owned())?;
    let (range, total) = value
        .strip_prefix("bytes ")
        .and_then(|value| value.split_once('/'))
        .ok_or_else(|| "Content-Range 格式无效".to_owned())?;
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| "Content-Range 格式无效".to_owned())?;
    Ok((
        start.parse().map_err(|_| "Content-Range 起始位置无效")?,
        end.parse().map_err(|_| "Content-Range 结束位置无效")?,
        total.parse().map_err(|_| "Content-Range 总大小无效")?,
    ))
}

fn metadata_path(part: &Path) -> PathBuf {
    PathBuf::from(format!("{}.json", part.to_string_lossy()))
}

async fn read_resume_metadata(path: &Path) -> ResumeMetadata {
    fs::read(path)
        .await
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

async fn save_resume_metadata(path: &Path, metadata: &ResumeMetadata) -> Result<(), String> {
    let bytes = serde_json::to_vec(metadata).map_err(|e| e.to_string())?;
    fs::write(path, bytes).await.map_err(|e| e.to_string())
}

async fn mark_cancelled(
    app: &AppHandle,
    tasks: &TaskMap,
    mut task: DownloadTask,
) -> Result<(), String> {
    task.status = DownloadStatus::Cancelled;
    task.speed = 0;
    publish(app, tasks, task).await;
    Ok(())
}

async fn publish(app: &AppHandle, tasks: &TaskMap, mut task: DownloadTask) {
    {
        let mut table = tasks.lock().await;
        task.revision = table
            .get(&task.id)
            .map_or(1, |current| current.revision.saturating_add(1));
        table.insert(task.id.clone(), task.clone());
    }
    let _ = app.emit(PROGRESS_EVENT, task);
}

fn prune_tasks(tasks: &mut HashMap<String, DownloadTask>) {
    if tasks.len() <= MAX_TASK_HISTORY {
        return;
    }
    let mut terminal: Vec<_> = tasks
        .values()
        .filter(|task| {
            matches!(
                task.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            )
        })
        .map(|task| task.id.clone())
        .collect();
    terminal.sort();
    let remove = tasks.len().saturating_sub(MAX_TASK_HISTORY);
    for id in terminal.into_iter().take(remove) {
        tasks.remove(&id);
    }
}

fn sanitize_filename(name: &str) -> Result<String, String> {
    if name.is_empty()
        || name.len() > 240
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
        || Path::new(name)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || name.ends_with('.')
        || name.ends_with(' ')
    {
        return Err("GitHub Asset 文件名不安全".to_owned());
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&stem.as_str()) {
        return Err("GitHub Asset 文件名是 Windows 保留名称".to_owned());
    }
    Ok(name.to_owned())
}

async fn reserve_destination(
    dir: &Path,
    name: &str,
    total: u64,
) -> Result<(PathBuf, PathBuf, std::fs::File, File, u64), String> {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    for index in 0..10_000u32 {
        let filename = if index == 0 {
            name.to_owned()
        } else {
            format!("{stem} ({index}){extension}")
        };
        let destination = dir.join(filename);
        if fs::try_exists(&destination).await.unwrap_or(true) {
            continue;
        }
        let part = PathBuf::from(format!("{}.part", destination.to_string_lossy()));
        let opened = match OpenOptions::new().read(true).write(true).open(&part).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(&part)
                    .await
                {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(error.to_string()),
                }
            }
            Err(error) => return Err(error.to_string()),
        };
        let standard = opened.into_std().await;
        let lock_handle = standard.try_clone().map_err(|e| e.to_string())?;
        if fs2::FileExt::try_lock_exclusive(&lock_handle).is_err() {
            continue;
        }
        let existing = standard.metadata().map_err(|e| e.to_string())?.len();
        if existing > total {
            standard.set_len(0).map_err(|e| e.to_string())?;
        }
        if fs::try_exists(&destination).await.unwrap_or(true) {
            drop(lock_handle);
            drop(standard);
            continue;
        }
        let offset = if existing <= total { existing } else { 0 };
        let file = File::from_std(standard);
        return Ok((destination, part, lock_handle, file, offset));
    }
    Err("无法为下载文件分配唯一名称".to_owned())
}

pub async fn cancel(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let cancellations = state.cancellations.lock().await;
    if let Some(sender) = cancellations.get(&id) {
        let _ = sender.send(true);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_content_range, sanitize_filename};

    #[test]
    fn rejects_path_like_and_reserved_names() {
        for name in [
            "../escape",
            "..\\escape",
            "C:\\escape.exe",
            "\\\\host\\share",
            "CON.txt",
            "NUL",
        ] {
            assert!(sanitize_filename(name).is_err(), "accepted {name}");
        }
        assert_eq!(
            sanitize_filename("app-win-x64.zip").unwrap(),
            "app-win-x64.zip"
        );
    }

    #[test]
    fn parses_content_range_strictly() {
        assert_eq!(
            parse_content_range(Some("bytes 10-19/20")).unwrap(),
            (10, 19, 20)
        );
        assert!(parse_content_range(Some("bytes 9-19/20")).is_ok());
        assert!(parse_content_range(Some("bytes */20")).is_err());
        assert!(parse_content_range(None).is_err());
    }
}
