import type { DownloadTask, SearchHistoryEntry } from './types';

export const SEARCH_HISTORY_LIMIT = 50;
export const PAGE_SIZES = [3, 5, 10, 20, 30] as const;
export const DEFAULT_PAGE_SIZE: number = PAGE_SIZES[0];

export function parseRepoUrl(input: string) {
  let url: URL;
  try {
    url = new URL(input.startsWith('http://') || input.startsWith('https://') ? input : `https://${input}`);
  } catch {
    throw new Error('请输入有效的 GitHub 仓库地址');
  }
  const authority = url.href.split('://')[1]?.split('/')[0] ?? '';
  if (url.protocol !== 'https:' || url.hostname !== 'github.com' || url.port || authority.includes('@') || url.search || url.hash) {
    throw new Error('只支持 HTTPS github.com 仓库地址');
  }
  const parts = url.pathname.split('/').filter(Boolean);
  if (parts.length < 2 || parts.length > 3 || (parts.length === 3 && parts[2] !== 'releases')) {
    throw new Error('仓库地址应为 github.com/owner/repository，可选 /releases');
  }
  const repo = parts[1].replace(/\.git$/, '');
  if (!parts[0] || !repo || parts.some((part) => part.includes('%'))) throw new Error('仓库地址格式无效');
  return { owner: parts[0], repo };
}

export function mergeTask(current: DownloadTask | undefined, incoming: DownloadTask): DownloadTask {
  if (!current || incoming.revision >= current.revision) return incoming;
  return current;
}

export function mergeTaskSnapshot(tasks: DownloadTask[], snapshot: DownloadTask[]): DownloadTask[] {
  const merged = new Map(tasks.map((task) => [task.id, task]));
  for (const task of snapshot) merged.set(task.id, mergeTask(merged.get(task.id), task));
  return [...merged.values()].sort((a, b) => b.id.localeCompare(a.id));
}

export function recordSearchHistory(history: SearchHistoryEntry[], entry: SearchHistoryEntry): SearchHistoryEntry[] {
  const deduped = history.filter((item) => !(item.owner === entry.owner && item.repo === entry.repo));
  return [entry, ...deduped].slice(0, SEARCH_HISTORY_LIMIT);
}

export function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
