<script lang="ts">
  import { ChevronDown, ChevronRight, Download, FileArchive, FileText } from 'lucide-svelte';
  import type { Release, Asset } from '../types';
  export let release: Release;
  export let expanded = true;
  export let onDownload: (asset: Asset) => void;
  const formatSize = (bytes: number) => bytes < 1024 * 1024 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  const formatDate = (date: string | null) => date ? new Date(date).toLocaleDateString('zh-CN', { year: 'numeric', month: 'short', day: 'numeric' }) : '未发布';
  const iconFor = (name: string) => /zip|tar|gz|dmg|pkg|exe|msi/i.test(name) ? FileArchive : FileText;
</script>

<article class="release-card">
  <button type="button" class="release-header" on:click={() => expanded = !expanded} aria-label={`${release.tagName} 文件列表`} aria-expanded={expanded}>
    <span class="release-dot"></span>
    <span class="release-info"><strong>{release.tagName}</strong>{#if release.isLatest}<span class="latest">最新</span>{/if}{#if release.prerelease}<span class="pre">预发布</span>{/if}<small>{formatDate(release.publishedAt ?? release.createdAt)}{release.name && release.name !== release.tagName ? ` · ${release.name}` : ''}</small></span>
    <span class="asset-count">{release.assets.length} 个文件 {#if expanded}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}</span>
  </button>
  {#if expanded}
    <div class="asset-list">
      {#each release.assets as asset (asset.id)}
        {@const Icon = iconFor(asset.name)}
        <div class="asset-row"><span class="file-icon"><Icon size={16} /></span><span class="file-name">{asset.name}</span><span class="file-size">{formatSize(asset.size)}</span><button type="button" class="download-button" aria-label={`下载 ${asset.name}`} on:click={() => onDownload(asset)}><Download size={14} />下载</button></div>
      {:else}<div class="empty-assets">该 Release 没有可下载文件</div>{/each}
    </div>
  {/if}
</article>
