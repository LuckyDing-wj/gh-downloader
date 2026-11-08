<script lang="ts">
  import { Search, ArrowRight, LoaderCircle } from 'lucide-svelte';
  export let value = '';
  export let loading = false;
  export let onSearch: (value: string) => void;
</script>

<div class="search">
  <Search size={17} strokeWidth={1.8} />
  <input bind:value placeholder="粘贴 GitHub 仓库地址，例如 github.com/owner/repository" aria-label="GitHub 仓库地址" aria-describedby="search-hint" on:keydown={(event) => event.key === 'Enter' && !loading && onSearch(value)} />
  <button type="button" class="primary" disabled={loading || !value.trim()} on:click={() => onSearch(value)}>{#if loading}<LoaderCircle class="spin" size={15} />{:else}<ArrowRight size={15} />{/if}查看仓库</button>
</div>
<p id="search-hint" class="sr-only">仅支持 https://github.com/owner/repository 或可选 /releases 路径。</p>
