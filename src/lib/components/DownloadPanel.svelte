<script lang="ts">
  import { CheckCircle2, CircleStop, Download, LoaderCircle, X } from 'lucide-svelte';
  import type { DownloadTask } from '../types';
  export let tasks: DownloadTask[] = [];
  export let onCancel: (id: string) => void;
  export let full = false;
  const formatSize = (bytes: number) => bytes < 1024 * 1024 ? `${Math.round(bytes / 1024)} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  const statusText = (status: DownloadTask['status']) => ({ queued: '排队中', downloading: '下载中', paused: '已暂停', completed: '已完成', failed: '失败', cancelled: '已取消' })[status];
</script>

<section class:full-panel={full} class="side-section"><h3><Download size={full ? 17 : 15} />{full ? '下载任务' : '下载中'} <span>{tasks.filter((task) => ['queued', 'downloading', 'paused'].includes(task.status)).length}</span></h3>
  {#each (full ? tasks : tasks.slice(0, 4)) as task (task.id)}
    <div class="task"><div class="task-line"><span class:success={task.status === 'completed'} class="task-status">{#if task.status === 'completed'}<CheckCircle2 size={14} />{:else if task.status === 'downloading'}<LoaderCircle class="spin" size={14} />{:else}<Download size={14} />{/if}</span><span class="task-name">{task.assetName}</span>{#if !['completed', 'failed', 'cancelled'].includes(task.status)}<button type="button" class="icon-button" title="取消下载" aria-label={`取消 ${task.assetName}`} on:click={() => onCancel(task.id)}><X size={14} /></button>{/if}</div><div class="task-meta">{statusText(task.status)}{task.total ? ` · ${formatSize(task.downloaded)} / ${formatSize(task.total)}` : ''}{task.speed ? ` · ${formatSize(task.speed)}/s` : ''}{#if task.error} · {task.error}{/if}</div>{#if task.total && task.status === 'downloading'}<div class="progress" role="progressbar" aria-label={`${task.assetName} 下载进度`} aria-valuemin="0" aria-valuemax={task.total} aria-valuenow={Math.min(task.total, task.downloaded)}><i style={`width: ${Math.min(100, task.downloaded / task.total * 100)}%`}></i></div>{/if}</div>
  {:else}<div class="side-empty"><CircleStop size={17} />还没有下载任务</div>{/each}
</section>
