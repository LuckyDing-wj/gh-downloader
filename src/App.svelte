<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { Github, Download, Settings, Star, GitFork, ExternalLink, AlertCircle, LoaderCircle } from 'lucide-svelte';
  import SearchBar from './lib/components/SearchBar.svelte';
  import ReleaseList from './lib/components/ReleaseList.svelte';
  import DownloadPanel from './lib/components/DownloadPanel.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import HistoryPanel from './lib/components/HistoryPanel.svelte';
  import { cancelDownload, chooseDownloadDir, clearSearchHistory, fetchReleases, getSearchHistory, getConfig, getDownloadTasks, saveConfig, saveSearchHistory, startDownload } from './lib/api';
  import type { AppConfig, Asset, DownloadTask, Repository, SearchHistoryEntry } from './lib/types';
  import { DEFAULT_PAGE_SIZE, PAGE_SIZES, isDesktopRuntime, mergeTaskSnapshot, recordSearchHistory } from './lib/frontend';

  let url = '';
  let repository: Repository | null = null;
  let config: AppConfig = { downloadDir: '', proxyMode: 'none', proxyUrl: '', githubTokens: [] };
  let tasks: DownloadTask[] = [];
  let loading = false;
  let error = '';
  let notice = '';
  let activeNav = '仓库';
  let unlistenProgress: (() => void) | undefined;
  let taskPoll: ReturnType<typeof setInterval> | undefined;
  let disposed = false;
  let searchGeneration = 0;
  let repoUrl = '';
  let page = 1;
  let pageSize = DEFAULT_PAGE_SIZE;
  let pageLoading = false;
  let history: SearchHistoryEntry[] = [];
  const desktop = isDesktopRuntime();

  onMount(() => {
    void initialize();
    return () => {
      disposed = true;
      unlistenProgress?.();
      stopTaskPolling();
    };
  });

  async function initialize() {
    try {
      const cleanup = await listen<DownloadTask>('download-progress', (event) => mergeTask(event.payload));
      if (disposed) cleanup();
      else unlistenProgress = cleanup;
    } catch { /* 静态预览 */ }
    try { const value = await getConfig(); if (!disposed) config = value; } catch { /* 浏览器预览模式允许无后端运行 */ }
    try { const entries = await getSearchHistory(); if (!disposed) history = entries; } catch { /* 历史读取失败时从空列表开始 */ }
    const savedPageSize = Number(localStorage.getItem('gh-downloader-page-size'));
    if ((PAGE_SIZES as readonly number[]).includes(savedPageSize)) pageSize = savedPageSize;
    await refreshTasks();
  }

  function mergeTask(next: DownloadTask) {
    tasks = mergeTaskSnapshot(tasks, [next]);
  }

  async function refreshTasks() {
    try {
      const snapshot = await getDownloadTasks();
      if (!disposed) tasks = mergeTaskSnapshot(tasks, snapshot);
    } catch { /* 浏览器预览模式无本地任务表 */ }
  }

  function startTaskPolling() {
    if (!desktop) return;
    stopTaskPolling();
    void refreshTasks();
    taskPoll = setInterval(() => void refreshTasks(), 750);
  }

  function stopTaskPolling() {
    if (taskPoll) clearInterval(taskPoll);
    taskPoll = undefined;
  }

  function navigate(item: string) {
    activeNav = item;
    if (item === '下载') startTaskPolling();
    else stopTaskPolling();
  }

  async function search(value: string) {
    const generation = ++searchGeneration;
    pageLoading = false;
    page = 1;
    url = value.trim(); repoUrl = url; error = ''; notice = ''; repository = null; loading = true;
    try {
      const result = await fetchReleases(url, 1, pageSize);
      if (generation === searchGeneration) {
        repository = result;
        await rememberSearch(result);
      }
    }
    catch (e) { if (generation === searchGeneration) error = String(e).replace(/^Error: /, ''); }
    finally { if (generation === searchGeneration) loading = false; }
  }

  async function rememberSearch(repository: Repository) {
    const entry: SearchHistoryEntry = {
      owner: repository.owner,
      repo: repository.name,
      url: `${repository.owner}/${repository.name}`,
      queriedAt: new Date().toISOString()
    };
    history = recordSearchHistory(history, entry);
    try { await saveSearchHistory(history); } catch { /* 历史保存失败不影响查询 */ }
  }

  async function selectHistory(target: string) {
    await search(`https://github.com/${target}`);
  }

  async function wipeHistory() {
    history = [];
    try { await clearSearchHistory(); } catch { /* 清空失败下次启动仍会重载 */ }
  }

  async function goToPage(target: number, force = false) {
    if (!repository || pageLoading) return;
    if (!force && (target < 1 || target === page)) return;
    if (!force && target > page && !repository.hasMoreReleases) return;
    const generation = searchGeneration;
    const currentRepository = repository;
    pageLoading = true;
    try {
      const result = await fetchReleases(repoUrl, target, pageSize);
      if (generation !== searchGeneration || repository?.fullName !== currentRepository.fullName) return;
      if (result.releases.length === 0 && target > 1) { notice = '已经是最后一页'; return; }
      repository = { ...currentRepository, releases: result.releases, hasMoreReleases: result.hasMoreReleases };
      page = target;
    } catch (e) { if (generation === searchGeneration) error = String(e).replace(/^Error: /, ''); }
    finally { if (generation === searchGeneration) pageLoading = false; }
  }

  async function changePageSize(size: number) {
    if (size === pageSize) return;
    pageSize = size;
    localStorage.setItem('gh-downloader-page-size', String(size));
    if (!repository) { page = 1; return; }
    searchGeneration++; // 作废进行中的翻页请求，避免旧响应覆盖
    pageLoading = false;
    await goToPage(1, true);
  }

  async function download(asset: Asset) {
    notice = '';
    try {
      if (!repository) return;
      if (!desktop) {
        await startDownload(repository.owner, repository.name, asset.id, asset.name, asset.downloadUrl);
        notice = `${asset.name} 已交给浏览器下载`;
        return;
      }
      const task = await startDownload(repository.owner, repository.name, asset.id, asset.name, asset.downloadUrl);
      if (task) {
        tasks = tasks.some((current) => current.id === task.id) ? tasks : [task, ...tasks];
        notice = `${asset.name} 已加入下载队列`;
      } else {
        notice = `${asset.name} 已交给浏览器下载`;
      }
    } catch (e) { notice = String(e).replace(/^Error: /, ''); }
  }

  async function chooseDir() {
    try {
      if (!desktop) return;
      const path = await chooseDownloadDir();
      if (!path) return;
      const updated = await saveConfig({ ...config, downloadDir: path });
      config = updated;
      notice = '下载目录已保存';
    } catch (e) { notice = String(e).replace(/^Error: /, ''); }
  }

  async function persistConfig() {
    if (!desktop) return;
    try { config = await saveConfig(config); notice = '设置已保存'; }
    catch (e) { notice = String(e).replace(/^Error: /, ''); }
  }

  async function cancel(id: string) {
    try {
      await cancelDownload(id);
      notice = '正在取消下载…';
    } catch (e) { notice = String(e).replace(/^Error: /, ''); }
  }

  const formatCount = (value: number) => value > 999 ? `${(value / 1000).toFixed(1)}k` : String(value);
</script>

<svelte:head><title>gh-downloader</title></svelte:head>
<div class="app-shell">
  <header class="topbar">
    <div class="brand"><span class="brand-mark"><Github size={17} /></span><strong>gh-downloader</strong></div>
    <nav aria-label="主导航">
      {#each ['仓库', '下载', '设置'] as item (item)}
        <button class:active={activeNav === item} aria-current={activeNav === item ? 'page' : undefined} on:click={() => navigate(item)}>
          {#if item === '下载'}<Download size={14} />{:else if item === '设置'}<Settings size={14} />{/if}
          {item}{#if item === '下载' && tasks.length}<em>{tasks.length}</em>{/if}
        </button>
      {/each}
    </nav>
    <div class="user-mark">G</div>
  </header>

  {#if activeNav === '仓库'}
    <main class="main">
      <div class="eyebrow">GITHUB RELEASE FINDER</div><h1>找到你要的版本</h1>
      <p class="subtitle">粘贴 GitHub 仓库地址，浏览并下载 Release 文件。</p>
      <SearchBar value={url} {loading} onSearch={search} />
      {#if error}<div class="alert error" role="alert"><AlertCircle size={16} />{error}</div>{/if}
      {#if notice}<div class="alert notice"><AlertCircle size={16} />{notice}</div>{/if}
      {#if loading}
        <div class="loading-state" role="status" aria-live="polite"><LoaderCircle class="spin" size={24} />正在读取 GitHub Releases…</div>
      {:else}
        <div class="layout">
          <section class="release-column">
            {#if repository}
              <div class="repo-head"><div class="repo-logo"><Github size={20} /></div><div class="repo-title"><h2>{repository.name}</h2><span>{repository.fullName}</span></div><div class="repo-stats"><span><Star size={13} /> {formatCount(repository.stargazersCount)}</span><span><GitFork size={13} /> {formatCount(repository.forksCount)}</span><a href={repository.htmlUrl} target="_blank" rel="noreferrer" title="在 GitHub 打开"><ExternalLink size={14} /></a></div></div>
              <section class="repository-releases"><ReleaseList releases={repository.releases} onDownload={download} {page} {pageSize} onChangePage={goToPage} onChangePageSize={changePageSize} hasNext={repository.hasMoreReleases} loading={pageLoading} /></section>
            {:else}
              <div class="welcome"><Github size={36} /><strong>输入仓库地址开始</strong><span>支持公开 GitHub 仓库和自定义代理</span></div>
            {/if}
          </section>
          <aside><HistoryPanel {history} onSelect={selectHistory} onClear={wipeHistory} /></aside>
        </div>
      {/if}
    </main>
  {:else if activeNav === '下载'}
    <main class="main subpage">
      <div class="eyebrow">DOWNLOADS</div><h1>下载任务</h1><p class="subtitle">查看进度、速度和任务状态。</p>
      {#if notice}<div class="alert notice" role="status">{notice}</div>{/if}
      {#if !desktop}<div class="alert notice" role="status">浏览器下载由浏览器管理，此处不显示进度；桌面版可查看任务进度。</div>{/if}
      <div class="subpage-content"><DownloadPanel {tasks} onCancel={cancel} full /></div>
    </main>
  {:else}
    <main class="main subpage">
      <div class="eyebrow">PREFERENCES</div><h1>设置</h1><p class="subtitle">下载目录和网络代理设置。</p>
      {#if notice}<div class="alert notice"><AlertCircle size={16} />{notice}</div>{/if}
      {#if !desktop}<div class="alert notice" role="status">浏览器模式不支持保存下载目录或代理设置；请使用桌面版。</div>{/if}
      <div class="subpage-content settings-page"><SettingsPanel {config} onChooseDir={chooseDir} onSave={persistConfig} disabled={!desktop} /></div>
    </main>
  {/if}
</div>
