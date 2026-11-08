import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, Asset, DownloadTask, Repository } from './types';
import { parseRepoUrl } from './frontend';

const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url, { headers: { Accept: 'application/vnd.github+json' } });
  if (!response.ok) {
    if (response.status === 403) throw new Error('GitHub API 请求受限，请稍后重试');
    if (response.status === 404) throw new Error('找不到这个 GitHub 仓库');
    throw new Error(`GitHub 请求失败（${response.status}）`);
  }
  return response.json() as Promise<T>;
}

async function fetchReleasesInBrowser(input: string, page: number): Promise<Repository> {
  const { owner, repo } = parseRepoUrl(input);
  const [repository, releases] = await Promise.all([
    fetchJson<{ full_name: string; name: string; owner: { login: string }; description: string | null; html_url: string; stargazers_count: number; forks_count: number; updated_at: string }>(`https://api.github.com/repos/${owner}/${repo}`),
    fetch(`https://api.github.com/repos/${owner}/${repo}/releases?per_page=30&page=${page}`, { headers: { Accept: 'application/vnd.github+json' } })
  ]);
  if (!releases.ok) throw new Error(releases.status === 404 ? '找不到这个 GitHub 仓库' : `GitHub 请求失败（${releases.status}）`);
  const hasMoreReleases = (releases.headers.get('Link') || '').split(',').some((link) => link.includes('rel=\"next\"'));
  const releaseData = await releases.json() as Array<{ id: number; tag_name: string; name: string | null; body: string | null; published_at: string | null; created_at: string; prerelease: boolean; draft: boolean; assets: Array<{ id: number; name: string; size: number; browser_download_url: string; content_type: string; download_count: number }> }>;
  const latestId = page === 1 ? releaseData.find((release) => !release.draft && !release.prerelease)?.id : undefined;
  return {
    fullName: repository.full_name,
    name: repository.name,
    owner: repository.owner.login,
    description: repository.description,
    htmlUrl: repository.html_url,
    stargazersCount: repository.stargazers_count,
    forksCount: repository.forks_count,
    updatedAt: repository.updated_at,
    hasMoreReleases,
    releases: releaseData.map((release) => ({
      id: release.id,
      tagName: release.tag_name,
      name: release.name || release.tag_name,
      body: release.body || '',
      publishedAt: release.published_at,
      createdAt: release.created_at,
      prerelease: release.prerelease,
      draft: release.draft,
      isLatest: release.id === latestId,
      assets: release.assets.map((asset): Asset => ({
        id: asset.id,
        name: asset.name,
        size: asset.size,
        downloadUrl: asset.browser_download_url,
        contentType: asset.content_type,
        downloadCount: asset.download_count
      }))
    }))
  };
}

export const fetchReleases = (url: string, page = 1) => isTauri() ? invoke<Repository>('fetch_releases', { url, page }) : fetchReleasesInBrowser(url, page);

const defaultConfig = (): AppConfig => ({ downloadDir: '浏览器默认下载目录', proxyMode: 'none', proxyUrl: '' });

export async function getConfig() {
  if (isTauri()) return invoke<AppConfig>('get_config');
  try { return { ...defaultConfig(), ...JSON.parse(localStorage.getItem('gh-downloader-config') || '{}') } as AppConfig; } catch { return defaultConfig(); }
}

export async function saveConfig(config: AppConfig) {
  if (isTauri()) return invoke<AppConfig>('save_config', { config });
  localStorage.setItem('gh-downloader-config', JSON.stringify(config));
  return config;
}

export const chooseDownloadDir = () => isTauri() ? invoke<string | null>('choose_download_dir') : Promise.resolve(null);

export async function startDownload(owner: string, repo: string, assetId: number, assetName: string, downloadUrl: string) {
  if (isTauri()) return invoke<DownloadTask>('start_download', { owner, repo, assetId });
  window.open(downloadUrl, '_blank', 'noopener,noreferrer');
  return null;
}

export const cancelDownload = (taskId: string) => isTauri() ? invoke<void>('cancel_download', { taskId }) : Promise.resolve();
export const getDownloadTasks = () => isTauri() ? invoke<DownloadTask[]>('get_download_tasks') : Promise.resolve([]);
