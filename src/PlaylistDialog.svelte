<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { X, Plus, LoaderCircle, ListMusic, Search, Check, Lock, Link, Globe } from 'lucide-svelte';
  import { invoke } from './backend';
  import type { Collection, Endpoint, Feed, Item } from './types';
  let { mode, collection = null, initialTitle = '', track = null, tracks = [], partial = false, onclose, onchanged }: {
    mode: 'create' | 'edit' | 'pick' | 'delete' | 'tracks' | 'queue'; collection?: Collection | null; initialTitle?: string; track?: Item | null; tracks?: Item[]; partial?: boolean;
    onclose: () => void; onchanged: (id: string | null, deleted: boolean) => void;
  } = $props();
  let dialog: HTMLDialogElement;
  const visibility=[{value:'PRIVATE',label:'Личный',hint:'Виден только тебе',icon:Lock},{value:'UNLISTED',label:'По ссылке',hint:'Доступен тем, у кого есть ссылка',icon:Link},{value:'PUBLIC',label:'Открытый',hint:'Доступен всем пользователям YouTube',icon:Globe}];
  const queueTracks=untrack(()=>tracks.filter(item=>item.source!=='local'&&/^[A-Za-z0-9_-]{11}$/.test(item.id)));
  const localCount=untrack(()=>tracks.filter(item=>item.source==='local').length);
  let view = $state(untrack(()=>mode)), title = $state(untrack(()=>initialTitle)), description = $state(untrack(()=>mode === 'edit' ? collection?.description || '' : '')), privacy = $state(untrack(()=>mode === 'edit' && visibility.some(v=>v.value===collection?.privacy)?collection!.privacy!:'PRIVATE'));
  let busy = $state(false), loading = $state(false), error = $state(''), choices = $state<Item[]>([]);
  let query = $state(''), results = $state<Item[]>([]), searched = $state(false), added = $state<string[]>([]);
  let searchVersion = 0;
  const targetPlaylist = untrack(() => collection?.playlistId);
  let alive = true;
  function close() {
    if (busy) return;
    if (view === 'tracks' && added.length) onchanged(null, false);
    onclose();
  }
  function invalidateSearch() {
    searchVersion++; loading = false; results = []; searched = false; error = '';
  }
  async function searchTracks() {
    const text = query.trim();
    if (!text) return;
    const version = ++searchVersion;
    loading = true; error = ''; results = []; searched = false;
    try {
      const feed = await invoke<Feed>('search', {query:text});
      if (alive && version === searchVersion) {
        const seen = new Set<string>();
        results = feed.sections.flatMap(s => s.items).filter(item => {
          if (item.kind !== 'song' || item.source === 'local' || !/^[A-Za-z0-9_-]{11}$/.test(item.id) || seen.has(item.id)) return false;
          seen.add(item.id); return true;
        });
        searched = true;
      }
    } catch (e) { if (alive && version === searchVersion) error = String(e); }
    finally { if (alive && version === searchVersion) loading = false; }
  }
  async function addTrack(item: Item) {
    if (busy || !targetPlaylist || added.includes(item.id)) return;
    busy = true; error = '';
    try {
      await invoke('playlist_change', {change:{action:'add',playlistId:targetPlaylist,videoId:item.id}});
      if (alive) added = [...added,item.id];
    } catch (e) { if (alive) error = String(e); }
    finally { if (alive) busy = false; }
  }
  let continuation = $state<Endpoint | null>(null);
  onMount(() => {
    dialog.showModal();
    if (view === 'tracks') dialog.querySelector<HTMLInputElement>('.playlist-search input')?.focus();
    if (view === 'pick') void loadChoices();
    return () => { alive = false; };
  });
  async function loadChoices(more = false) {
    if (loading) return;
    loading = true;
    try {
      const feed = more && continuation ? await invoke<Feed>('browse', {endpoint:continuation}) : await invoke<Feed>('library', { category: 'playlists' });
      if (alive) { continuation = feed.continuation; choices = [...(more ? choices : []), ...feed.sections.flatMap(s => s.items).filter(i => {
        const p = (i.endpoint.browseEndpoint as {browseId?:string})?.browseId || '';
        return p.startsWith('VLPL');
      })]; }
    } catch (e) { if (alive) error = String(e); }
    finally { if (alive) loading = false; }
  }
  async function submit(playlistId?: string) {
    if (busy) return;
    busy = true; error = '';
    try {
      const change = view === 'pick' ? { action: 'add', playlistId, videoId: track?.id }
        : view === 'delete' ? {action:'delete',playlistId:collection?.playlistId}
        : {action: view === 'edit' ? 'update' : 'create', playlistId: view === 'edit' ? collection?.playlistId : undefined, title: title.trim(), description, privacy, ...(view === 'queue' ? {videoIds:queueTracks.map(item=>item.id)} : view === 'create' && track ? {videoId:track.id} : {})};
      const created = await invoke<string | null>('playlist_change', { change });
      if (alive) { onchanged(created,view === 'delete'); onclose(); }
    } catch (e) { if (alive) error = String(e); }
    finally { if (alive) busy = false; }
  }
</script>

<dialog bind:this={dialog} class="playlist-dialog" oncancel={(e) => { e.preventDefault(); if (!busy) close(); }} aria-labelledby="playlist-dialog-title">
  <header><div><span class="eyebrow">YOUTUBE MUSIC</span><h2 id="playlist-dialog-title">{view === 'queue' ? 'Сохранить очередь' : view === 'tracks' ? 'Добавить треки' : view === 'pick' ? 'Добавить в плейлист' : view === 'edit' ? 'Изменить плейлист' : view === 'delete' ? 'Удалить плейлист?' : 'Новый плейлист'}</h2></div><button class="icon" disabled={busy} aria-label="Закрыть" onclick={close}><X size={20}/></button></header>
  {#if error}<p class="playlist-error" role="alert">{error}</p>{/if}
  {#if view === 'tracks'}
    <p class="playlist-hint">В плейлист «{initialTitle}» · поиск в YouTube Music</p>
    <form class="playlist-search" onsubmit={(e)=>{e.preventDefault();void searchTracks();}}>
      <input aria-label="Найти треки для плейлиста" type="search" bind:value={query} oninput={invalidateSearch} placeholder="Трек или исполнитель" maxlength="200"/>
      <button class="secondary" type="submit" disabled={loading || !query.trim()} aria-label="Найти треки">{#if loading}<LoaderCircle size={18} class="spin"/>{:else}<Search size={18}/>{/if}</button>
    </form>
    <div class="playlist-results" aria-busy={loading}>
      {#each results as item (item.id)}
        <div class="playlist-result">
          {#if item.image}<img src={item.image} alt="" loading="lazy"/>{:else}<ListMusic size={28}/>{/if}
          <span><strong>{item.title}</strong><small>{item.subtitle}</small></span>
          <button class="icon" disabled={busy || added.includes(item.id)} aria-label={added.includes(item.id) ? `Добавлен: ${item.title}` : `Добавить: ${item.title}`} onclick={()=>addTrack(item)}>{#if added.includes(item.id)}<Check size={20}/>{:else}<Plus size={20}/>{/if}</button>
        </div>
      {:else}
        <p class="playlist-hint">{loading ? 'Ищем треки…' : searched ? 'Треки не найдены. Попробуй другое название или исполнителя.' : 'Найди музыку и добавляй треки кнопкой + справа.'}</p>
      {/each}
    </div>
    <div class="playlist-dialog-actions"><span class="playlist-hint" role="status">{added.length ? `Добавлено треков: ${added.length}` : ''}</span><button class="primary" disabled={busy} onclick={close}>Готово</button></div>
  {:else if view === 'pick'}
    <p class="playlist-hint">{track?.title}</p>
    <button class="secondary" disabled={busy} onclick={() => {view='create';title='';error='';}}><Plus size={17}/>Создать плейлист</button>
    <div class="playlist-choices" aria-busy={loading || busy}>
      {#if loading}<LoaderCircle size={24} class="spin"/>{:else if !choices.length}<p>Плейлистов пока нет. Создай первый здесь.</p>{/if}
      {#each choices as item}<button disabled={busy} onclick={() => submit((item.endpoint.browseEndpoint as {browseId:string}).browseId.slice(2))}><ListMusic size={22}/><span><strong>{item.title}</strong><small>{item.subtitle}</small></span></button>{/each}
    </div>
    {#if continuation}<button class="secondary" disabled={loading || busy} onclick={()=>loadChoices(true)}>Ещё плейлисты</button>{/if}
    <p class="playlist-hint">Добавление доступно в свои плейлисты. Общие и сохранённые подборки YouTube проверяет отдельно.</p>
  {:else if view === 'delete'}
    <p>«{initialTitle}» будет удалён из YouTube Music на всех устройствах. Вернуть плейлист через neisora не получится.</p>
    <div class="playlist-dialog-actions"><button class="secondary" disabled={busy} onclick={close}>Отмена</button><button class="primary danger" disabled={busy} onclick={() => submit()}>{#if busy}<LoaderCircle size={16} class="spin"/>{/if}Удалить</button></div>
  {:else}
    {#if view === 'queue'}<p class="playlist-hint">В новый плейлист YouTube Music: {queueTracks.length} треков из загруженной очереди, в текущем порядке.{#if localCount} Локальные файлы ({localCount}) останутся на компьютере.{/if}{#if partial} Продолжение очереди пока не загружено и не войдёт в плейлист.{/if}</p>{#if queueTracks.length>2000}<p class="playlist-error">За один раз можно сохранить до 2000 треков. Сократи очередь перед сохранением.</p>{:else if !queueTracks.length}<p class="playlist-error">В очереди нет треков YouTube Music для сохранения.</p>{/if}{/if}
    <form onsubmit={(e) => {e.preventDefault();void submit();}} >
      <label>Название<input aria-label="Название плейлиста" bind:value={title} required maxlength="150" placeholder="Музыка для…"/></label>
      <label>Описание<textarea aria-label="Описание плейлиста" bind:value={description} maxlength="5000" rows="3" placeholder="Необязательно"></textarea></label>
      <fieldset class="playlist-privacy" aria-label="Видимость плейлиста" disabled={busy}><legend>Доступ</legend><div class="privacy-options">{#each visibility as option}<label class="privacy-option" class:chosen={privacy===option.value}><input type="radio" name="playlist-privacy" value={option.value} bind:group={privacy} aria-label={option.label}/><option.icon size={18}/><span>{option.label}</span></label>{/each}</div><p>{visibility.find(option=>option.value===privacy)?.hint}</p></fieldset>
      <div class="playlist-dialog-actions"><button type="button" class="secondary" disabled={busy} onclick={close}>Отмена</button><button type="submit" class="primary" disabled={busy || !title.trim() || (view==='queue'&&(!queueTracks.length||queueTracks.length>2000))}>{#if busy}<LoaderCircle size={16} class="spin"/>{/if}{view === 'edit' ? 'Сохранить' : 'Создать'}</button></div>
    </form>
  {/if}
</dialog>

<style>
  .playlist-dialog { margin:auto; width:min(480px,calc(100vw - 32px)); max-height:calc(100dvh - 32px); overflow:auto; border:1px solid var(--line); border-radius:22px; padding:26px; background:var(--surface); color:var(--text); box-shadow:0 24px 100px #0009; }
  .playlist-dialog::backdrop {background:#0009;backdrop-filter:blur(8px)}
  header {display:flex;align-items:flex-start;justify-content:space-between;gap:12px;margin-bottom:20px} h2{font-size:23px;margin:8px 0 0} form,label{display:flex;flex-direction:column;gap:10px}form{gap:18px}label{font-size:13px;color:var(--muted)}
  input:not([type=radio]),textarea{width:100%;min-width:0;background:var(--bg);border:1px solid var(--line);border-radius:10px;color:var(--text);padding:12px;font:inherit}textarea{resize:vertical}
  .playlist-privacy{border:0;padding:0;min-width:0}.playlist-privacy legend{font-size:13px;color:var(--muted);margin-bottom:10px}.privacy-options{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px}.privacy-option{position:relative;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;min-height:72px;padding:12px 5px;border:1px solid var(--line);border-radius:12px;background:var(--bg);color:var(--muted);cursor:pointer;text-align:center;font-size:12px}.privacy-option.chosen{color:var(--accent);border-color:var(--accent);background:color-mix(in srgb,var(--accent) 9%,var(--bg))}.privacy-option:has(input:focus-visible){outline:2px solid var(--accent);outline-offset:3px}.privacy-option input{position:absolute;inset:0;width:100%;height:100%;margin:0;opacity:0;cursor:pointer}.playlist-privacy p{font-size:11px;color:var(--muted);line-height:1.5;margin:9px 0 0}.playlist-privacy:disabled{opacity:.6}
  .playlist-dialog-actions{display:flex;justify-content:flex-end;gap:10px;margin-top:10px}.playlist-error{color:#ffaaaa;font-size:13px;line-height:1.6}.playlist-hint{font-size:12px;color:var(--muted);line-height:1.6}.danger{background:#b9324b;color:white}
  .playlist-search{flex-direction:row;gap:8px}.playlist-search input{flex:1}.playlist-search button{flex-shrink:0}
  .playlist-results{margin:14px 0;max-height:40vh;overflow:auto}.playlist-result{display:flex;align-items:center;gap:12px;padding:9px 0}.playlist-result :global(>svg){flex-shrink:0}.playlist-result img{width:42px;height:42px;object-fit:cover;border-radius:7px}.playlist-result span{flex:1;min-width:0;display:flex;flex-direction:column;gap:4px}.playlist-result strong,.playlist-result small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.playlist-result small{color:var(--muted);font-size:12px}.playlist-result button{flex-shrink:0}.playlist-dialog-actions>span{margin-right:auto}
  .playlist-choices{display:flex;flex-direction:column;gap:4px;margin:18px 0;max-height:40vh;overflow:auto}.playlist-choices>button{display:flex;align-items:center;gap:12px;padding:12px;background:transparent;text-align:left;border-radius:10px}.playlist-choices>button:hover{background:#ffffff08}.playlist-choices span{display:flex;flex-direction:column;gap:4px;min-width:0}.playlist-choices strong,.playlist-choices small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.playlist-choices small{color:var(--muted)}
</style>
