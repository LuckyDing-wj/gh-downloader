export type ProxyMode = 'none' | 'system' | 'custom';

export interface Asset {
  id: number;
  name: string;
  size: number;
  downloadUrl: string;
  contentType: string;
  downloadCount: number;
}

export interface Release {
  id: number;
  tagName: string;
  name: string;
  body: string;
  publishedAt: string | null;
  createdAt: string;
  prerelease: boolean;
  draft: boolean;
  isLatest: boolean;
  assets: Asset[];
}

export interface Repository {
  fullName: string;
  name: string;
  owner: string;
  description: string | null;
  htmlUrl: string;
  stargazersCount: number;
  forksCount: number;
  updatedAt: string;
  releases: Release[];
  hasMoreReleases: boolean;
}

export interface SearchHistoryEntry {
  owner: string;
  repo: string;
  url: string;
  queriedAt: string;
}

export interface AppConfig {
  downloadDir: string;
  proxyMode: ProxyMode;
  proxyUrl: string;
}

export type DownloadStatus = 'queued' | 'downloading' | 'paused' | 'completed' | 'failed' | 'cancelled';

export interface DownloadTask {
  id: string;
  revision: number;
  assetName: string;
  destination: string;
  downloaded: number;
  total: number;
  speed: number;
  status: DownloadStatus;
  error?: string;
}
