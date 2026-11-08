use crate::models::{Asset, ProxyMode, Release, Repository};
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

pub async fn fetch(
    url: &str,
    page: u32,
    config: &crate::models::AppConfig,
) -> Result<Repository, String> {
    let (owner, name) = parse_repo_url(url)?;
    let client = client(&config.proxy_mode, &config.proxy_url)?;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/vnd.github+json"),
    );
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
    let response = client
        .get(format!(
            "https://api.github.com/repos/{owner}/{repo}/releases/assets/{asset_id}"
        ))
        .header(header::ACCEPT, "application/vnd.github+json")
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
) -> Result<Response, String> {
    let mut request = client.get(url.clone());
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
    use super::{parse_repo_url, validate_asset_url};

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
