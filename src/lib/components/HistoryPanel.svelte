<script lang="ts">
  import { Clock, History, Trash2 } from 'lucide-svelte';
  import type { SearchHistoryEntry } from '../types';
  export let history: SearchHistoryEntry[] = [];
  export let onSelect: (url: string) => void;
  export let onClear: () => void;
  const relativeTime = (date: string) => {
    const elapsed = Date.now() - new Date(date).getTime();
    const minutes = Math.floor(elapsed / 60_000);
    if (minutes < 1) return '刚刚';
    if (minutes < 60) return `${minutes} 分钟前`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours} 小时前`;
    return `${Math.floor(hours / 24)} 天前`;
  };
</script>

<section class="side-section history-section" aria-label="查询历史">
  <h3><History size={15} />查询历史 <span>{history.length}</span>
    {#if history.length}<button type="button" class="history-clear" on:click={onClear}><Trash2 size={12} />清空</button>{/if}
  </h3>
  {#each history as entry (entry.owner + '/' + entry.repo)}
    <button type="button" class="history-item" title={entry.url} on:click={() => onSelect(entry.url)}>
      <span class="history-repo">{entry.owner}/{entry.repo}</span>
      <span class="history-time"><Clock size={11} />{relativeTime(entry.queriedAt)}</span>
    </button>
  {:else}
    <div class="side-empty"><History size={17} />还没有查询记录</div>
  {/each}
</section>
