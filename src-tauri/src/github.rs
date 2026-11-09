use crate::models::{AppConfig, Asset, GithubTokenEntry, ProxyMode, Release, Repository};
use reqwest::{header, redirect::Policy, Client, ClientBuilder, Response};
use serde::Deserialize;
use std::time::Duration;
use url::Url;

pub const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const USER_AGENT: &str = "gh-downloader/0.1";
const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
    "github-releases.githubusercontent.com",
];

#[derive(Debug, Deserialize)]
struct ApiRepo {
    full_name: String,
    name: String,
    owner: ApiOwner,
    description: Option<String>,
    html_url: String,
    stargazers_count: i64,
    forks_count: i64,
    updated_at: String,
}
#[derive(Debug, Deserialize)]
struct ApiOwner {
    login: String,
}
#[derive(Debug, Deserialize)]
struct ApiRelease {
    id: i64,
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    created_at: String,
    prerelease: bool,
    draft: bool,
    assets: Vec<ApiAsset>,
}
#[derive(Debug, Deserialize)]
struct ApiAsset {
    id: i64,
    name: String,
    size: i64,
    browser_download_url: String,
    content_type: String,
    download_count: i64,
}
#[derive(Debug, Deserialize)]
struct ApiUser {
    login: String,
}

pub fn parse_repo_url(input: &str) -> Result<(String, String), String> {
    let normalized = if input.starts_with("https://") || input.starts_with("http://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = Url::parse(&normalized).map_err(|_| "请输入有效的 GitHub 仓库地址".to_owned())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url
            .as_str()
            .split_once("://")
            .and_then(|(_, rest)| rest.split('/').next())
            .is_some_and(|authority| authority.contains('@'))
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("只支持标准 HTTPS GitHub 仓库地址".to_owned());
    }
    let parts: Vec<_> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err("仓库地址应为 github.com/owner/repository".to_owned());
    }
    if parts.len() > 2 && !(parts.len() == 3 && parts[2] == "releases") {
        return Err("请输入仓库根地址或仓库 Releases 地址".to_owned());
    }
    let repo = parts[1].strip_suffix(".git").unwrap_or(parts[1]);
    if repo.is_empty() || parts[0].contains('%') || repo.contains('%') {
        return Err("仓库地址格式无效".to_owned());
    }
    Ok((parts[0].to_owned(), repo.to_owned()))
}

pub fn validate_asset_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "GitHub Asset 地址无效".to_owned())?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let allowed = ALLOWED_HOSTS.contains(&host.as_str());
    if url.scheme() != "https"
        || !allowed
        || url.port().is_some()
        || !url.username().is_empty()
        || url
            .as_str()
            .split_once("://")
            .and_then(|(_, rest)| rest.split('/').next())
            .is_some_and(|authority| authority.contains('@'))
    {
        return Err("只允许从 HTTPS GitHub 官方下载域名获取文件".to_owned());
    }
    Ok(url)
}

pub fn client(proxy: &ProxyMode, proxy_url: &str) -> Result<Client, String> {
    let redirect = Policy::custom(|attempt| {
        if attempt.previous().len() >= 10 || validate_asset_url(attempt.url().as_str()).is_err() {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });
    let mut builder = ClientBuilder::new()
        .user_agent(USER_AGENT)
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .redirect(redirect);
    match proxy {
        ProxyMode::Custom => {
            if proxy_url.trim().is_empty() {
                return Err("已选择自定义代理，请填写代理地址".to_owned());
            }
            let proxy = reqwest::Proxy::all(proxy_url).map_err(|_| "代理地址无效".to_owned())?;
            builder = builder.proxy(proxy);
        }
        ProxyMode::None => builder = builder.no_proxy(),
        ProxyMode::System => {}
    }
    builder.build().map_err(|e| e.to_string())
}

/// 由本地配置的 Token 生成 Bearer 请求头；Token 为空时返回 None（匿名访问）。
fn bearer_value(token: Option<&str>) -> Option<header::HeaderValue> {
    let token = token?.trim();
    if token.is_empty() {
        return None;
    }
    header::HeaderValue::from_str(&format!("Bearer {token}")).ok()
}

/// 选择用于请求的 Token：优先匹配仓库 owner 对应账号，否则用第一个可用 Token。
fn pick_token_entry<'a>(
    entries: &'a [GithubTokenEntry],
    owner: Option<&str>,
) -> Option<&'a GithubTokenEntry> {
    if let Some(owner) = owner {
        if let Some(entry) = entries.iter().find(|entry| {
            !entry.token.trim().is_empty()
                && !entry.login.trim().is_empty()
                && entry.login.eq_ignore_ascii_case(owner)
        }) {
            return Some(entry);
        }
    }
    entries.iter().find(|entry| !entry.token.trim().is_empty())
}

/// 选出本次请求使用的 Token；缺失账号归属信息时通过 GitHub /user 解析并缓存到本地配置。
pub async fn select_token(config: &mut AppConfig, owner: Option<&str>) -> Option<String> {
    if pick_token_entry(&config.github_tokens, owner)
        .map(|entry| !entry.login.trim().is_empty())
        .unwrap_or(false)
    {
        return pick_token_entry(&config.github_tokens, owner).map(|entry| entry.token.clone());
    }
    let pending: Vec<String> = config
        .github_tokens
        .iter()
        .filter(|entry| !entry.token.trim().is_empty() && entry.login.trim().is_empty())
        .map(|entry| entry.token.clone())
        .collect();
    if !pending.is_empty() {
        if let Ok(auth_client) = client(&config.proxy_mode, &config.proxy_url) {
            let resolved = futures_util::future::join_all(pending.iter().map(|token| {
                let auth_client = auth_client.clone();
                async move {
                    let login = resolve_login(&auth_client, token).await;
                    (token.clone(), login)
                }
            }))
            .await;
            let mut changed = false;
            for (token, login) in resolved {
                let Some(login) = login else { continue };
                if let Some(entry) = config
                    .github_tokens
                    .iter_mut()
                    .find(|entry| entry.token == token && entry.login.trim().is_empty())
                {
                    entry.login = login;
                    changed = true;
                }
            }
            if changed {
                let _ = crate::config::save(config);
            }
        }
    }
    pick_token_entry(&config.github_tokens, owner).map(|entry| entry.token.clone())
}

async fn resolve_login(auth_client: &Client, token: &str) -> Option<String> {
    auth_client
        .get("https://api.github.com/user")
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, bearer_value(Some(token))?)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<ApiUser>()
        .await
        .ok()
        .map(|user| user.login)
}

pub async fn fetch(
    url: &str,
    page: u32,
    config: &crate::models::AppConfig,
    token: Option<&str>,
) -> Result<Repository, String> {
    let (owner, name) = parse_repo_url(url)?;
    let client = client(&config.proxy_mode, &config.proxy_url)?;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/vnd.github+json"),
    );
    if let Some(value) = bearer_value(token) {
        headers.insert(header::AUTHORIZATION, value);
    }
    let repo: ApiRepo = client
        .get(format!("https://api.github.com/repos/{owner}/{name}"))
        .headers(headers.clone())
        .send()
        .await
        .map_err(|e| format!("请求 GitHub 失败：{e}"))?
        .error_for_status()
        .map_err(github_error)?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let release_response = client
        .get(format!(
            "https://api.github.com/repos/{owner}/{name}/releases?per_page=30&page={page}"
        ))
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("获取 Releases 失败：{e}"))?
        .error_for_status()
        .map_err(github_error)?;
    let has_more_releases = release_response
        .headers()
        .get(header::LINK)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|link| link.split(',').any(|part| part.contains("rel=\"next\"")));
    let releases: Vec<ApiRelease> = release_response.json().await.map_err(|e| e.to_string())?;
    let latest_id = if page == 1 {
        releases
            .iter()
            .find(|release| !release.draft && !release.prerelease)
            .map(|release| release.id)
    } else {
        None
    };
    let releases = releases
        .into_iter()
        .map(|release| {
            let assets = release
                .assets
                .into_iter()
                .map(map_asset)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Release {
                id: release.id,
                name: release
                    .name
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| release.tag_name.clone()),
                tag_name: release.tag_name,
                body: release.body.unwrap_or_default(),
                published_at: release.published_at,
                created_at: release.created_at,
                prerelease: release.prerelease,
                draft: release.draft,
                is_latest: Some(release.id) == latest_id,
                assets,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Repository {
        full_name: repo.full_name,
        name: repo.name,
        owner: repo.owner.login,
        description: repo.description,
        html_url: repo.html_url,
        stargazers_count: repo.stargazers_count,
        forks_count: repo.forks_count,
        updated_at: repo.updated_at,
        releases,
        has_more_releases,
    })
}

pub async fn resolve_asset(
    owner: &str,
    repo: &str,
    asset_id: i64,
    config: &crate::models::AppConfig,
    token: Option<&str>,
) -> Result<Asset, String> {
    if asset_id <= 0
        || owner.is_empty()
        || repo.is_empty()
        || owner.contains('/')
        || repo.contains('/')
    {
        return Err("仓库或 Asset 标识无效".to_owned());
    }
    let client = client(&config.proxy_mode, &config.proxy_url)?;
    let mut request = client
        .get(format!(
            "https://api.github.com/repos/{owner}/{repo}/releases/assets/{asset_id}"
        ))
        .header(header::ACCEPT, "application/vnd.github+json");
    if let Some(value) = bearer_value(token) {
        request = request.header(header::AUTHORIZATION, value);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("获取 Asset 信息失败：{e}"))?
        .error_for_status()
        .map_err(github_error)?;
    let asset: ApiAsset = response.json().await.map_err(|e| e.to_string())?;
    map_asset(asset)
}

pub async fn open_asset(
    client: &Client,
    url: &Url,
    range_start: Option<u64>,
    if_range: Option<&str>,
    token: Option<&str>,
) -> Result<Response, String> {
    let mut request = client.get(url.clone());
    if let Some(value) = bearer_value(token) {
        request = request.header(header::AUTHORIZATION, value);
    }
    if let Some(start) = range_start {
        request = request.header(header::RANGE, format!("bytes={start}-"));
        if let Some(validator) = if_range {
            request = request.header(header::IF_RANGE, validator);
        }
    }
    request.send().await.map_err(|e| e.to_string())
}

fn map_asset(asset: ApiAsset) -> Result<Asset, String> {
    validate_asset_url(&asset.browser_download_url)?;
    if asset.size < 0 || asset.size as u64 > MAX_DOWNLOAD_BYTES {
        return Err("Asset 超过允许的最大下载大小（2 GiB）".to_owned());
    }
    Ok(Asset {
        id: asset.id,
        name: asset.name,
        size: asset.size,
        download_url: asset.browser_download_url,
        content_type: asset.content_type,
        download_count: asset.download_count,
    })
}

fn github_error(error: reqwest::Error) -> String {
    if error.status() == Some(reqwest::StatusCode::FORBIDDEN) {
        "GitHub API 请求受限，请稍后重试".to_owned()
    } else {
        format!("GitHub 返回错误：{error}")
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_repo_url, pick_token_entry, validate_asset_url};
    use crate::models::GithubTokenEntry;

    fn entry(label: &str, token: &str, login: &str) -> GithubTokenEntry {
        GithubTokenEntry {
            label: label.to_owned(),
            token: token.to_owned(),
            login: login.to_owned(),
        }
    }

    #[test]
    fn picks_token_by_owner_then_default() {
        let entries = vec![
            entry("工作", "tok-a", "alice"),
            entry("个人", "", "bob"),
            entry("默认", "tok-c", ""),
        ];
        let picked = pick_token_entry(&entries, Some("Alice")).map(|e| e.token.as_str());
        assert_eq!(picked, Some("tok-a"));
        let default = pick_token_entry(&entries, Some("carol")).map(|e| e.token.as_str());
        assert_eq!(default, Some("tok-a"));
    }

    #[test]
    fn ignores_blank_tokens_when_picking() {
        let entries = vec![entry("空", "  ", "")];
        assert!(pick_token_entry(&entries, None).is_none());
    }

    #[test]
    fn accepts_canonical_repository_urls() {
        assert_eq!(
            parse_repo_url("https://github.com/prime-zt/pure-clash").unwrap(),
            ("prime-zt".into(), "pure-clash".into())
        );
        assert_eq!(
            parse_repo_url("github.com/prime-zt/pure-clash/releases").unwrap(),
            ("prime-zt".into(), "pure-clash".into())
        );
    }

    #[test]
    fn rejects_noncanonical_repository_urls() {
        for url in [
            "http://github.com/a/b",
            "https://github.com:444/a/b",
            "https://user@github.com/a/b",
            "https://github.com/a/b/issues",
            "https://evil.test/a/b",
        ] {
            assert!(parse_repo_url(url).is_err(), "accepted {url}");
        }
    }

    #[test]
    fn only_accepts_https_github_asset_hosts() {
        assert!(validate_asset_url("https://release-assets.githubusercontent.com/file").is_ok());
        for url in [
            "http://github.com/file",
            "https://127.0.0.1/file",
            "https://github.com.evil.test/file",
            "https://user@github.com/file",
        ] {
            assert!(validate_asset_url(url).is_err(), "accepted {url}");
        }
    }
}
