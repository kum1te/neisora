<script lang="ts">
  import { onMount } from 'svelte';
  import { History, Play, Trash2, Search, LoaderCircle, Music2 } from 'lucide-svelte';
  import LocalArtwork from './LocalArtwork.svelte';
  import { invoke } from './backend';
  import type { Item, HistoryEntry } from './types';
  let { onplay, currentId = '', revision = 0 }: { onplay: (item: Item) => void; currentId?: string; revision?: number } = $props();
  let entries = $state<HistoryEntry[]>([]), query = $state(''), loading = $state(true), error = $state(''), confirming = $state(false);
  let generation = 0;
  const filtered = $derived(entries.filter(e => `${e.item.title} ${e.item.subtitle}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())));
  function date(value: number) { return new Date(value * 1000).toLocaleDateString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' }); }
  async function refresh() {
    const version = ++generation;
    try { const value = await invoke<HistoryEntry[]>('listening_history'); if (version === generation) { entries = value; error = ''; } }
    catch (e) { if (version === generation) error = String(e); }
    finally { if (version === generation) loading = false; }
  }
  $effect(() => { revision; void refresh(); });
  async function clear() {
    if (!confirming) { confirming = true; return; }
    try { await invoke('clear_history'); generation++; entries = []; confirming = false; }
    catch (e) { error = String(e); }
  }
  onMount(() => () => { generation++; });
</script>
<div class="page-heading"><div><span class="eyebrow">ТВОЯ МУЗЫКАЛЬНАЯ ПАМЯТЬ</span><h1>История</h1></div>{#if entries.length}<div class="history-clear"><button class="secondary" onclick={clear}><Trash2 size={15}/>{confirming ? 'Да, очистить историю' : 'Очистить'}</button>{#if confirming}<button class="secondary" onclick={() => confirming = false}>Отмена</button>{/if}</div>{/if}</div>
<p class="history-description">Последние 500 прослушиваний в neisora. Хранятся только на этом устройстве.</p>
{#if loading}<div class="empty-state"><LoaderCircle class="spin" size={30}/></div>
{:else if error}<div class="empty-state"><p>{error}</p><button class="secondary" onclick={refresh}>Повторить</button></div>
{:else if !entries.length}<div class="empty-state"><History size={38}/><h2>Здесь будет твоя музыка</h2><p>Треки появятся после пяти секунд воспроизведения.</p></div>
{:else}
  <label class="history-search"><Search size={17}/><input bind:value={query} placeholder="Найти в истории" aria-label="Поиск в истории"/></label>
  {#if !filtered.length}<div class="empty-state"><Search size={30}/><h2>Ничего не нашлось</h2></div>{/if}
  {#each filtered as entry, n}
    {#if n === 0 || date(entry.playedAt) !== date(filtered[n - 1].playedAt)}<h2 class="history-date">{date(entry.playedAt)}</h2>{/if}
    <button class="track-row history-track" class:playing={currentId === entry.item.id} onclick={() => onplay(entry.item)}>
      <span class="track-number">{currentId === entry.item.id ? '♪' : n + 1}</span>
      {#if entry.item.source === 'local'}<span class="history-local-cover"><LocalArtwork item={entry.item}/></span>{:else if entry.item.image}<img src={entry.item.image} alt="" loading="lazy"/>{:else}<span class="track-placeholder"><Music2 size={18}/></span>{/if}
      <span class="track-details"><strong>{entry.item.title}</strong><small>{entry.item.subtitle}</small></span>
      <time datetime={new Date(entry.playedAt * 1000).toISOString()}>{new Date(entry.playedAt * 1000).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}</time><Play size={15}/>
    </button>
  {/each}
{/if}
<style>
  .history-local-cover{width:42px;height:42px;flex-shrink:0;border-radius:7px}
  .history-description{color:var(--muted);font-size:13px;margin:-12px 0 26px;line-height:1.6}
  .history-clear{display:flex;gap:8px}.history-search{display:flex;align-items:center;gap:10px;max-width:440px;padding:12px 16px;background:var(--surface);border:1px solid var(--line);border-radius:12px;color:var(--muted)}
  .history-search input{background:none;border:0;outline:0;color:var(--text);width:100%;font:inherit;font-size:13px}.history-date{font-size:16px;margin:30px 0 12px}.history-track{width:100%}.history-track time{font-size:12px;color:var(--muted);font-variant-numeric:tabular-nums}
</style>
