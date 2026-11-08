<script lang="ts">
  import { SlidersHorizontal } from 'lucide-svelte';
  import ReleaseCard from './ReleaseCard.svelte';
  import type { Asset, Release } from '../types';
  export let releases: Release[] = [];
  export let onDownload: (asset: Asset) => void;
  export let onLoadMore: () => void;
  export let loadingMore = false;
  export let hasMore = false;
  let sort = 'latest';
  $: visible = [...releases].sort((a, b) => sort === 'latest' ? Date.parse(b.publishedAt ?? b.createdAt) - Date.parse(a.publishedAt ?? a.createdAt) : Date.parse(a.publishedAt ?? a.createdAt) - Date.parse(b.publishedAt ?? b.createdAt));
</script>

<div class="section-heading"><h2>Releases <span>{releases.length}</span></h2><label class="sort"><SlidersHorizontal size={13} /><select bind:value={sort} aria-label="Release 排序"><option value="latest">最新优先</option><option value="oldest">最早优先</option></select></label></div>
{#each visible as release (release.id)}<ReleaseCard {release} onDownload={onDownload} />{:else}<div class="empty-state">这个仓库还没有公开 Release。</div>{/each}
{#if hasMore}<div class="load-more"><button type="button" class="load-more-button" disabled={loadingMore} aria-busy={loadingMore} on:click={onLoadMore}>{loadingMore ? '正在加载…' : '加载更多 Releases'}</button></div>{/if}
