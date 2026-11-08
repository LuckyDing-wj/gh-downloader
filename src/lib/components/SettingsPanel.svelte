<script lang="ts">
  import { Check, FolderOpen, Globe2, Settings2 } from 'lucide-svelte';
  import type { AppConfig } from '../types';
  export let config: AppConfig;
  export let onChooseDir: () => void;
  export let onSave: () => void;
  export let disabled = false;
</script>

<section class="side-section"><h3><Settings2 size={15} />下载设置</h3><fieldset disabled={disabled} class="settings-fields"><div class="setting-card"><div class="setting-label">保存到</div><button type="button" class="path-button" aria-label="选择下载目录" on:click={onChooseDir}><span>{config.downloadDir || '选择下载目录'}</span><FolderOpen size={14} /></button></div><fieldset class="setting-card proxy-fieldset"><legend class="setting-label"><Globe2 size={13} />代理</legend><div class="segmented" role="group" aria-label="代理模式"><button type="button" class:active={config.proxyMode === 'none'} aria-pressed={config.proxyMode === 'none'} on:click={() => config.proxyMode = 'none'}>关闭</button><button type="button" class:active={config.proxyMode === 'system'} aria-pressed={config.proxyMode === 'system'} on:click={() => config.proxyMode = 'system'}>系统</button><button type="button" class:active={config.proxyMode === 'custom'} aria-pressed={config.proxyMode === 'custom'} on:click={() => config.proxyMode = 'custom'}>自定义</button></div>{#if config.proxyMode === 'custom'}<input id="proxy-url" class="proxy-input" bind:value={config.proxyUrl} aria-label="自定义代理地址" placeholder="HTTP 或 SOCKS5 代理 URL" />{/if}<p class="hint">端口由你填写，不预设固定端口。</p></fieldset><button type="button" class="save-settings" on:click={onSave}><Check size={14} />保存设置</button></fieldset></section>
