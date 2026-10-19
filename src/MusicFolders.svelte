<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { FolderPlus, FolderOpen, RefreshCw, X, LoaderCircle } from 'lucide-svelte';
  import { invoke } from './backend';
  import type { LocalStatus } from './types';
  let {onopen}: {onopen:()=>void} = $props();
  let status=$state<LocalStatus>({folders:[],total:0,scanning:false,warning:''}),busy=$state(false),error=$state('');
  let version=0;
  async function refresh(){const id=++version;try{const value=await invoke<LocalStatus>('local_status');if(id===version)status=value;}catch(e){if(id===version)error=String(e);}}
  async function add(){busy=true;error='';try{await invoke('choose_music_folder');await refresh();}catch(e){error=String(e);}finally{busy=false;}}
  async function remove(id:string){try{await invoke('remove_music_folder',{id});await refresh();}catch(e){error=String(e);}}
  async function scan(){try{await invoke('scan_local_music');await refresh();}catch(e){error=String(e);}}
  onMount(()=>{let disposed=false,unlisten:(()=>void)|undefined;void refresh();void listen('local-library-updated',()=>void refresh()).then(f=>{if(disposed)f();else unlisten=f;});return()=>{disposed=true;unlisten?.();};});
</script>
<section class="settings-section local-settings" aria-labelledby="local-music-title">
  <div class="settings-section-heading"><span class="settings-symbol"><FolderOpen size={22}/></span><div><h2 id="local-music-title">Локальная музыка</h2><p>Твои файлы — на своих местах. Папки сканируются вместе с подпапками.</p></div></div>
  <div class="folder-actions"><button class="primary" disabled={busy} onclick={add}>{#if busy}<LoaderCircle size={16} class="spin"/>{:else}<FolderPlus size={16}/>{/if}Добавить папку</button><button class="secondary" onclick={onopen}>Открыть коллекцию</button>{#if status.folders.length}<button class="icon" aria-label="Пересканировать папки" disabled={status.scanning} onclick={scan}><RefreshCw size={17} class={status.scanning?'spin':''}/></button>{/if}</div>
  <div class="music-folders">{#each status.folders as folder (folder.id)}<div class="music-folder"><FolderOpen size={19}/><div><strong>{folder.path.split('/').filter(Boolean).at(-1) || folder.path}</strong><small>{folder.path}</small>{#if !folder.available}<span>Папка недоступна — проверь подключение диска</span>{/if}</div><button class="icon" aria-label={`Отключить папку ${folder.path}`} title="Убрать из медиатеки, сохранив файлы" onclick={()=>remove(folder.id)}><X size={16}/></button></div>{/each}</div>
  <p class="folder-note">{status.scanning?'Обновляем индекс…':`${status.total} треков в коллекции`}. Файлы не копируются и не загружаются в сеть. Отключение папки сохраняет файлы на диске.</p>
  {#if error || status.warning}<p class="settings-warning">{error || status.warning}</p>{/if}
</section>
<style>.folder-actions{display:flex;align-items:center;gap:10px;flex-wrap:wrap}.music-folders{margin-top:20px;display:grid;gap:8px}.music-folder{display:flex;align-items:center;gap:13px;padding:14px;border:1px solid var(--line);border-radius:12px;min-width:0}.music-folder>div{flex:1;min-width:0}.music-folder strong{display:block;font-size:13px;font-weight:500}.music-folder small{display:block;font-size:11px;color:var(--muted);margin-top:5px;overflow-wrap:anywhere}.music-folder span{display:block;color:#d8ae7e;font-size:11px;margin-top:6px}.folder-note{font-size:11px;line-height:1.8;color:var(--muted);margin-top:18px}</style>
