use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub size: i64,
    pub download_url: String,
    pub content_type: String,
    pub download_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: i64,
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: Option<String>,
    pub created_at: String,
    pub prerelease: bool,
    pub draft: bool,
    pub is_latest: bool,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub full_name: String,
    pub name: String,
    pub owner: String,
    pub description: Option<String>,
    pub html_url: String,
    pub stargazers_count: i64,
    pub forks_count: i64,
    pub updated_at: String,
    pub releases: Vec<Release>,
    pub has_more_releases: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    #[default]
    None,
    System,
    Custom,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubTokenEntry {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub token: String,
    /// Token 所属账号登录名，首次使用时通过 GitHub /user 接口解析并缓存。
    #[serde(default)]
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub download_dir: String,
    #[serde(default)]
    pub proxy_mode: ProxyMode,
    #[serde(default)]
    pub proxy_url: String,
    /// 旧版单 Token 字段，仅用于读取历史配置后迁移到 github_tokens。
    #[serde(default)]
    pub github_token: String,
    #[serde(default)]
    pub github_tokens: Vec<GithubTokenEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: String,
    pub revision: u64,
    pub asset_name: String,
    pub destination: String,
    pub downloaded: i64,
    pub total: i64,
    pub speed: i64,
    pub status: DownloadStatus,
    pub error: Option<String>,
}
