import { describe, expect, it } from 'vitest';
import { isDesktopRuntime, mergeTask, mergeTaskSnapshot, parseRepoUrl, recordSearchHistory, SEARCH_HISTORY_LIMIT } from './frontend';
import type { DownloadTask, SearchHistoryEntry } from './types';

const task = (overrides: Partial<DownloadTask> = {}): DownloadTask => ({
  id: 'task-1', revision: 0, assetName: 'asset.zip', destination: 'Downloads/asset.zip',
  downloaded: 0, total: 100, speed: 0, status: 'queued', ...overrides
});

describe('parseRepoUrl', () => {
  it('accepts HTTPS repository and releases URLs', () => {
    expect(parseRepoUrl('https://github.com/owner/repo')).toEqual({ owner: 'owner', repo: 'repo' });
    expect(parseRepoUrl('github.com/owner/repo/releases')).toEqual({ owner: 'owner', repo: 'repo' });
  });

  it('rejects non-HTTPS, foreign hosts, credentials, ports and extra paths', () => {
    for (const input of [
      'http://github.com/owner/repo', 'https://github.com:444/owner/repo',
      'https://user@github.com/owner/repo', 'https://github.com.evil.test/owner/repo',
      'https://github.com/owner/repo/issues'
    ]) expect(() => parseRepoUrl(input)).toThrow();
  });
});

describe('mergeTask', () => {
  it('does not roll back progress when a stale snapshot arrives', () => {
    const latest = task({ revision: 5, status: 'downloading', downloaded: 75, speed: 20 });
    expect(mergeTask(latest, task({ revision: 2 }))).toMatchObject({ revision: 5, status: 'downloading', downloaded: 75, speed: 20 });
  });

  it('accepts explicit terminal states and preserves progress', () => {
    const current = task({ revision: 3, status: 'downloading', downloaded: 75 });
    expect(mergeTask(current, task({ revision: 4, status: 'completed', downloaded: 100 }))).toMatchObject({ status: 'completed', downloaded: 100 });
  });

  it('merges snapshots by task id', () => {
    expect(mergeTaskSnapshot([task({ downloaded: 50, status: 'downloading' })], [task()])).toHaveLength(1);
    expect(mergeTaskSnapshot([task({ revision: 1, downloaded: 50, status: 'downloading' })], [task({ revision: 0 })])[0].downloaded).toBe(50);
  });
});

describe('recordSearchHistory', () => {
  const entry = (owner: string, repo: string, index = 0): SearchHistoryEntry => ({
    owner, repo, url: `${owner}/${repo}`, queriedAt: new Date(2026, 0, 1, 12, index).toISOString()
  });

  it('puts the newest entry first', () => {
    const history = recordSearchHistory([], entry('a', 'one'));
    expect(recordSearchHistory(history, entry('b', 'two'))[0].repo).toBe('two');
  });

  it('deduplicates by owner/repo and moves the repeat to the top', () => {
    const history = recordSearchHistory([], entry('a', 'one', 0));
    const updated = recordSearchHistory(history, entry('a', 'one', 5));
    expect(updated).toHaveLength(1);
    expect(updated[0].queriedAt).toBe(entry('a', 'one', 5).queriedAt);
  });

  it('trims history to the 50-entry limit', () => {
    let history: SearchHistoryEntry[] = [];
    for (let index = 0; index < SEARCH_HISTORY_LIMIT + 10; index++) {
      history = recordSearchHistory(history, entry('owner', `repo-${index}`, index));
    }
    expect(history).toHaveLength(SEARCH_HISTORY_LIMIT);
    expect(history[0].repo).toBe(`repo-${SEARCH_HISTORY_LIMIT + 9}`);
    expect(history.at(-1)?.repo).toBe(`repo-10`);
  });

  it('does not mutate the input array', () => {
    const original = [entry('a', 'one')];
    recordSearchHistory(original, entry('b', 'two'));
    expect(original).toHaveLength(1);
  });
});

describe('runtime detection', () => {
  it('returns a boolean without depending on Tauri globals in Node', () => {
    expect(typeof isDesktopRuntime()).toBe('boolean');
  });
});
