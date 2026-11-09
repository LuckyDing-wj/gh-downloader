use crate::models::{AppConfig, GithubTokenEntry, ProxyMode};
use directories::{BaseDirs, ProjectDirs};
use std::{fs, path::PathBuf};

fn config_path() -> PathBuf {
    ProjectDirs::from("", "", "gh-downloader")
        .map(|dirs| dirs.config_dir().join("config.json"))
        .unwrap_or_else(|| PathBuf::from("gh-downloader-config.json"))
}

pub fn default_config() -> AppConfig {
    let download_dir = BaseDirs::new()
        .map(|dirs| dirs.home_dir().join("Downloads"))
        .unwrap_or_else(|| PathBuf::from("."));
    AppConfig {
        download_dir: download_dir.to_string_lossy().into_owned(),
        proxy_mode: ProxyMode::None,
        proxy_url: String::new(),
        github_token: String::new(),
        github_tokens: Vec::new(),
    }
}

pub fn load() -> Result<AppConfig, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(default_config());
    }
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut config: AppConfig = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    migrate_legacy_token(&mut config);
    Ok(config)
}

/// 旧版单 Token 配置迁移为列表，避免升级后静默丢失。
fn migrate_legacy_token(config: &mut AppConfig) {
    let legacy = config.github_token.trim();
    if !legacy.is_empty()
        && !config
            .github_tokens
            .iter()
            .any(|e| e.token.trim() == legacy)
    {
        config.github_tokens.insert(
            0,
            GithubTokenEntry {
                label: "默认".to_owned(),
                token: legacy.to_owned(),
                login: String::new(),
            },
        );
    }
    config.github_token.clear();
}

pub fn save(config: &AppConfig) -> Result<AppConfig, String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())?;
    Ok(config.clone())
}
