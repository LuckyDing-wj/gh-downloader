<script lang="ts">
  import { SlidersHorizontal } from 'lucide-svelte';
  import ReleaseCard from './ReleaseCard.svelte';
  import { DEFAULT_PAGE_SIZE, PAGE_SIZES } from '../frontend';
  import type { Asset, Release } from '../types';
  export let releases: Release[] = [];
  export let onDownload: (asset: Asset) => void;
  export let page = 1;
  export let pageSize = DEFAULT_PAGE_SIZE;
  export let onChangePage: (page: number) => void;
  export let onChangePageSize: (size: number) => void;
  export let hasNext = false;
  export let loading = false;
  let sort = 'latest';
  $: visible = [...releases].sort((a, b) => sort === 'latest' ? Date.parse(b.publishedAt ?? b.createdAt) - Date.parse(a.publishedAt ?? a.createdAt) : Date.parse(a.publishedAt ?? a.createdAt) - Date.parse(b.publishedAt ?? b.createdAt));

  const handleSize = (event: Event) => onChangePageSize(Number((event.currentTarget as HTMLSelectElement).value));
</script>

<div class="section-heading">
  <h2>Releases <span>第 {page} 页</span></h2>
  <label class="sort"><span>每页</span><select value={pageSize} on:change={handleSize} aria-label="每页 Release 数量">{#each PAGE_SIZES as size (size)}<option value={size}>{size}</option>{/each}</select></label>
  <label class="sort"><SlidersHorizontal size={13} /><select bind:value={sort} aria-label="本页 Release 排序"><option value="latest">本页最新优先</option><option value="oldest">本页最早优先</option></select></label>
</div>
{#each visible as release (release.id)}<ReleaseCard {release} onDownload={onDownload} />{:else}<div class="empty-state">{page > 1 ? '这一页没有更多 Release 了。' : '这个仓库还没有公开 Release。'}</div>{/each}
{#if page > 1 || hasNext || loading}
  <div class="load-more">
    <button type="button" class="load-more-button" disabled={loading || page <= 1} on:click={() => onChangePage(page - 1)}>上一页</button>
    <span class="sort" style="margin:0 8px;cursor:default">第 {page} 页</span>
    <button type="button" class="load-more-button" disabled={loading || !hasNext} on:click={() => onChangePage(page + 1)}>{loading ? '加载中…' : '下一页'}</button>
  </div>
{/if}
