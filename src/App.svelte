<script lang="ts">
  import { onMount } from 'svelte';
  import { version as appVersion } from '../package.json';
  import { invoke } from './backend';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { House, Compass, Library, Search, Play, Pause, SkipBack, SkipForward, ListMusic, Heart, Volume2, VolumeX, ChevronLeft, ChevronRight, ArrowUpRight, RefreshCw, Settings2, X, Minus, Maximize2, Headphones, LogIn, LogOut, LoaderCircle, Music2, Check, Radio, Palette, ChevronsUp, History as HistoryIcon, Bookmark, BookmarkCheck, FolderOpen, ListPlus, ListStart, Plus, Ellipsis, Pencil, Trash2, ArrowUp, ArrowDown, Share2 } from 'lucide-svelte';
  import SeekBar from './SeekBar.svelte';
  import RepeatButton from './RepeatButton.svelte';
  import ShuffleButton from './ShuffleButton.svelte';
  import { SESSION_KEY, snapshot, encodeSession, decodeSession } from './session';
  import PlaylistDialog from './PlaylistDialog.svelte';
  import ShareDialog from './ShareDialog.svelte';
  import {itemShare,type ShareTarget} from './sharing';
  import SleepTimerDialog from './SleepTimerDialog.svelte';
  import { readRepeat, cycleRepeat, repeatTarget, resumePosition } from './playback';
  import ExpandedPlayer from './ExpandedPlayer.svelte';
  import Settings from './Settings.svelte';
  import VolumeSlider from './VolumeSlider.svelte';
  import QueuePanel from './QueuePanel.svelte';
  import LocalLibrary from './LocalLibrary.svelte';
  import LocalArtwork from './LocalArtwork.svelte';
  import { localArtwork } from './localArtwork';
  import ListeningHistory from './ListeningHistory.svelte';
  import { normalizeRadio, radioRecommendations, compactQueue, positioned, queuePosition, queueItemId, radioTail, refreshRadio, insertQueued, moveQueued, removeManyQueued, shuffleQueue } from './queue';
  import { smoothScroll } from './scroll';
  import { updateAudioLevel } from './audio';
  import { presets, applyTheme } from './themes';
  import type { Endpoint, Feed, Item, Queue, Status, PlayerState, Preferences, Theme, AppearanceState, LocalPage, SleepTimerState } from './types';

  let feed = $state<Feed | null>(null);
  let status = $state<Status | null>(null);
  let page = $state('home');
  let pageTitle = $state('Для тебя');
  let query = $state('');
  let loading = $state(true);
  let moreLoading = $state(false);
  let error = $state('');
  let notice = $state('');
  let authBusy = $state(false);
  let preferences = $state<Preferences>({ selected: 'violet', custom: [], followWallpaper: false, visualizer: { mode: 'ribbon', intensity: .55, speed: 1 } });
  let wallpaper = $state<Theme | null>(null);
  let wallpaperJob: Promise<void> | null = null;
  function refreshWallpaper() {
    if (wallpaperJob) return wallpaperJob;
    const job = invoke<Theme>('wallpaper_theme').then(value => { wallpaper = value; }).catch(() => {})
      .finally(() => { if (wallpaperJob === job) wallpaperJob = null; });
    wallpaperJob = job;
    return job;
  }
  $effect(() => { if (page === 'settings' || preferences.followWallpaper) void refreshWallpaper(); });
  let playbackHistory: Item[] = [];
  let interactions: Record<string, unknown>[] = [];
  let covers = $state<Record<string,string>>({});
  let coverLookupKey = '';
  function coverFor(item: Item) { return covers[item.id] || item.image; }
  $effect(() => {
    const track = current, duration = player.duration;
    if (!track || resolving || !player.active || !duration || covers[track.id] || (track.source !== 'local' && !/ytimg\.com|youtube\.com|yt3\./.test(track.image))) return;
    const key = `${track.id}:${Math.round(duration)}:${playRequest}`, session = queueRequest;
    if (coverLookupKey === key) return;
    coverLookupKey = key;
    const lookup = track.source === 'local' ? localArtwork(track,true).then(image => image ? {image} : null) : invoke<{image:string} | null>('get_recording', {title:track.title,artist:track.subtitle,duration,musicVideo:track.isMusicVideo ?? /official music video/i.test(track.title)});
    void lookup.then(result => {
      if (session === queueRequest && result?.image) {
        const image = new Image(); image.referrerPolicy = 'no-referrer';
        image.onload = () => { if (session === queueRequest) { const entries = Object.entries(covers).slice(-127); covers = Object.fromEntries([...entries,[track.id,result.image]]); } };
        image.src = result.image;
      }
    }).catch(() => {});
  });
  let impressedIds = new Set<string>();
  let nextQueueItemId = 0, pendingRadioSkip = false;
  let radioContext: Endpoint = {};
  let refreshChain: Promise<void> = Promise.resolve();
  let repeatMode = $state(readRepeat(localStorage.getItem('playback:repeat')));
  let lastAudibleVolume = 70;
  function changeRepeat() { repeatMode = cycleRepeat(repeatMode); localStorage.setItem('playback:repeat', repeatMode); }
  function toggleMute() { if (player.volume > 0) { lastAudibleVolume = player.volume; void control('volume_commit', 0); } else void control('volume_commit', lastAudibleVolume); }
  let queueOpen = $state(false);
  let focus = $state(false);
  let queue = $state<Queue | null>(null);
  let index = $state(0);
  let shuffleBusy=$state(false), restored=$state(false);
  let restoredRadioId:string|undefined;
  let sessionReady=false, sessionSaveFailed=false;
  let queueUndo=$state<{before:Queue;after:Queue;index:number;session:number;play:number}|null>(null);

  let undoTimer:ReturnType<typeof setTimeout>;
  let sessionWrite:Promise<void>=Promise.resolve();
  function sessionError(){if(!sessionSaveFailed){sessionSaveFailed=true;toast('Не удалось сохранить очередь на этом устройстве');}}
  function saveSession() {
    if(!sessionReady)return;
    try {
      const saved=queue&&current?snapshot(queue,index,player.position,player.duration):null;
      const raw=saved?encodeSession(saved):null;
      if(saved&&!raw)throw new Error('size');
      try{if(raw)localStorage.setItem(SESSION_KEY,raw);else localStorage.removeItem(SESSION_KEY);}catch{/* The native checkpoint remains authoritative. */}
      sessionWrite=sessionWrite.then(()=>invoke<void>('save_playback_session',{value:raw})).catch(sessionError);
    }catch{sessionError();}
  }
  $effect(()=>{queue;index;current;const timer=setTimeout(saveSession,500);return()=>clearTimeout(timer);});
  function rememberQueue(before:Queue,at:number) {
    if(!queue||queue===before)return;
    queueUndo={before,after:queue,index:at,session:queueRequest,play:playRequest};
    clearTimeout(undoTimer);undoTimer=setTimeout(()=>queueUndo=null,30000);
  }
  function undoQueue() {
    if(!canUndoQueue||!queueUndo||queueUndo.session!==queueRequest||queueUndo.play!==playRequest)return;
    queue=queueUndo.before;index=queueUndo.index;radioContext=queue.context;queueUndo=null;warmNext();
  }
  function removeSelected(positions:number[]) {
    if(!queue)return;const before=queue,at=index;
    ({queue,index}=removeManyQueued(queue,index,positions));rememberQueue(before,at);warmNext();
  }
  function clearFollowing() {
    if(!queue||index+1>=queue.items.length&&!queue.continuation)return;
    const before=queue,at=index;
    ++queueRequest;extending=null;pendingRadioSkip=false;radioContext={};refreshChain=Promise.resolve();
    queue={...queue,items:queue.items.slice(0,index+1),continuation:null,dynamic:false,shuffled:false,context:{manual:true}};
    rememberQueue(before,at);warmNext();
  }
  async function changeShuffle() {
    if(!queue||queue.dynamic||shuffleBusy)return;
    const session=queueRequest;shuffleBusy=true;queueUndo=null;
    try {
      for(let page=0;queue?.continuation&&page<100;page++) {
        const before:Queue=queue;await extendQueue();
        if(session!==queueRequest)return;
        if(queue===before)break;
      }
      if(session!==queueRequest||!queue)return;
      if(queue.continuation){toast('Не удалось загрузить очередь целиком для перемешивания. Попробуй ещё раз.');return;}
      queue=shuffleQueue(queue,index);warmNext();
    }finally{shuffleBusy=false;}
  }
  async function hydrateRadio():Promise<boolean> {
    if(!restored||!queue?.dynamic||!current)return true;
    const session=queueRequest,version=playRequest,track=current,manual=queue.items.slice(index+1).filter(i=>i.manual||i.source==='local');
    const endpoint=track.source==='local'?{watchPlaylistEndpoint:{playlistId:restoredRadioId}}:{watchEndpoint:{videoId:track.id,...(restoredRadioId?{playlistId:restoredRadioId}:{})}};
    if(track.source==='local'&&!restoredRadioId){queue={...queue,dynamic:false};return true;}
    resolving=true;
    try {
      const result=await invoke<Queue>('queue',{endpoint});
      if(session!==queueRequest||version!==playRequest)return false;
      const selected=result.items[result.selected]?.id===track.id?result.selected:result.items.findIndex(i=>i.id===track.id),tail=selected>=0?result.items.slice(selected+1):result.items;
      const manualIds=new Set(manual.map(i=>i.id));
      const anchor=selected>=0&&track.source!=='local'?{...track,endpoint:result.items[selected].endpoint,queuePosition:queuePosition(result.items[selected],selected)}:track;
      current=anchor;queue={...result,items:positioned([anchor,...manual,...tail.filter(i=>!manualIds.has(i.id)&&i.id!==track.id)]),selected:0};index=0;
      radioContext=result.context;nextQueueItemId=queue.items.reduce((max,item)=>Math.max(max,queueItemId(item)+1),0);
      return true;
    }catch(e){if(session===queueRequest&&version===playRequest)toast(`Не удалось продолжить микс: ${e}`);return false;}
    finally{if(session===queueRequest&&version===playRequest)resolving=false;}
  }
  async function resumePlayback() {
    if(resolving||!current)return;
    const position=resumePosition(restored,player.position,player.duration);
    if(!await hydrateRadio())return;
    await playAt(index,'select',false,position);
  }
  let current = $state<Item | null>(null);
  let resolving = $state(false);
  let liked = $state(false);
  let likeBusy = $state(false), collectionBusy = $state(false);
  const likeCache = new Map<string, boolean>();
  let accountVersion = 0, likeRevision = 0;
  let libraryCategory = $state('playlists');
  let playlistDialog = $state<{mode:'create'|'edit'|'pick'|'delete'|'tracks'|'queue';track?:Item;tracks?:Item[];partial?:boolean;title?:string;version:number} | null>(null);
  let playlistWriting = $state(false);
  let sharing=$state<{target:ShareTarget;title:string}|null>(null);
  function shareItem(item:Item){const target=itemShare(item);if(target){sharing={target,title:item.title};closeTrackMenus();}}
  let sleepDialog=$state(false), sleepState=$state<SleepTimerState>({mode:'off',deadlineMs:null});
  function closeTrackMenus(event?:MouseEvent) {
    if (event?.target instanceof Element && event.target.closest('.track-menu')) return;
    document.querySelectorAll<HTMLDetailsElement>('.track-menu[open]').forEach(el=>el.open=false);
  }
  function placeTrackMenu(event:Event) {
    const details=event.currentTarget as HTMLDetailsElement;
    if (!details.open) return;
    document.querySelectorAll<HTMLDetailsElement>('.track-menu[open]').forEach(el=>{if(el!==details)el.open=false;});
    const menu=details.querySelector<HTMLElement>('.track-menu-items');
    if(!menu)return;
    const rect=details.getBoundingClientRect(), height=menu.getBoundingClientRect().height;
    menu.style.left=`${Math.max(8,Math.min(rect.right-260,innerWidth-268))}px`;
    menu.style.top=`${Math.max(8,Math.min(rect.bottom+5,innerHeight-height-8))}px`;
  }
  function showPlaylist(mode:'create'|'edit'|'pick'|'delete'|'tracks'|'queue', track?:Item) {
    if (!status?.authenticated) { focus = false; showSettings(); return; }
    playlistDialog = {mode,track,version:requestId,...(mode==='queue'?{tracks:[...(queue?.items??[])],partial:!!queue?.continuation,title:(queue?.title||'Твоя очередь').slice(0,150)}:{})};
    document.querySelectorAll<HTMLDetailsElement>('.track-menu[open]').forEach(el=>el.open=false);
  }
  async function playlistChanged(created:string|null, deleted:boolean) {
    const version=playlistDialog?.version;
    feedCache.clear();
    toast(deleted ? 'Плейлист удалён из YouTube Music' : 'Плейлист сохранён в YouTube Music');
    if (version !== requestId) return;
    if (created) await load({browseId:`VL${created}`},'Новый плейлист','browse');
    else if (deleted) openLibrary('playlists');
    else if (page === 'browse' || page === 'library') await load(lastEndpoint,pageTitle,page,false,true);
  }
  async function changePlaylistEntry(item:Item, before?:string|null) {
    const collection=feed?.collection, version=requestId;
    if (!collection?.editable || !item.setVideoId || playlistWriting) return;
    playlistWriting=true;
    document.querySelectorAll<HTMLDetailsElement>('.track-menu[open]').forEach(el=>el.open=false);
    try {
      await invoke('playlist_change',{change:before === undefined
        ? {action:'remove',playlistId:collection.playlistId,videoId:item.id,setVideoId:item.setVideoId}
        : {action:'move',playlistId:collection.playlistId,setVideoId:item.setVideoId,before}});
      feedCache.clear();
      if (version === requestId) await load(lastEndpoint,pageTitle,page,false,true);
    } catch(e) {toast(String(e));}
    finally {playlistWriting=false;}
  }
  let historyRevision = $state(0);
  const historySession = crypto.randomUUID();
  let heardSeconds = 0, lastHeardTick = 0, recordedPlayback = -1, historyReady = false;
  let mediaTimer: ReturnType<typeof setTimeout> | undefined;
  let mediaPending: Record<string, unknown> | null = null;
  let mediaIdentity='', mediaStatus='';
  let mediaWrite:Promise<void>=Promise.resolve();
  function sendMedia(snapshot:Record<string,unknown>) {
    mediaWrite=mediaWrite.then(()=>invoke('sync_media',{snapshot})).then(()=>{}).catch(()=>{});
  }
  function publishMedia() {
    const track = current;
    mediaPending = {id:track?.id || '',title:track?.title || '',artist:track?.subtitle.split(' • ')[0] || '',cover:track && track.source !== 'local' ? coverFor(track) : '',...player,active:player.active && !resolving,repeatMode,canNext:!!queue && (index+1<queue.items.length || !!queue.continuation || (repeatMode === 'all' && !queue.dynamic)),canPrevious:!!track};
    const identity=track?.id||'', state=mediaPending.active?(player.paused?'Paused':'Playing'):'Stopped';
    if(identity!==mediaIdentity||state!==mediaStatus){
      mediaIdentity=identity;mediaStatus=state;clearTimeout(mediaTimer);mediaTimer=undefined;sendMedia(mediaPending);return;
    }
    if (mediaTimer) return;
    mediaTimer = setTimeout(() => { mediaTimer = undefined; if (mediaPending) sendMedia(mediaPending); }, 300);
  }
  $effect(() => { current; player; resolving; queue; index; covers; repeatMode; publishMedia(); });
  function recordListening(s: PlayerState) {
    const now = performance.now(), delta = lastHeardTick ? Math.min(1,(now-lastHeardTick)/1000) : 0;
    lastHeardTick = now;
    if (!current || !historyReady || resolving || !s.active || s.paused || s.buffering) return;
    heardSeconds += delta;
    if (heardSeconds < 5 || recordedPlayback === playRequest) return;
    const version = playRequest; recordedPlayback = version;
    void invoke('record_history',{item:{...current,image:current.source==='local'?'':coverFor(current)},playback:`${historySession}:${version}`}).then(() => { historyRevision++; }).catch(() => {});
  }
  let player = $state<PlayerState>({ position: 0, duration: 0, paused: true, volume: 70, buffering: false, active: false });
  let lastEndpoint = $state<Endpoint>({ browseId: 'FEmusic_home' });
  let requestId = 0;
  let queueRequest = 0;
  let playRequest = 0;
  const canUndoQueue=$derived(!!queueUndo&&queueUndo.after===queue&&queueUndo.session===queueRequest&&queueUndo.play===playRequest);
  let extending: Promise<void> | null = null;
  let noticeTimer: ReturnType<typeof setTimeout>;
  let history = $state<{ endpoint: Endpoint; title: string; page: string }[]>([]);
  let historyIndex = $state(-1);
  let content: HTMLElement;
  const feedCache = new Map<string, Feed>();
  const featured = $derived(feed?.sections.flatMap(s => s.items).find(i => /супермикс|supermix/i.test(i.title))
    ?? feed?.sections.flatMap(s => s.items).find(i => i.kind === 'playlist' || i.kind === 'album')
    ?? feed?.sections[0]?.items[0]);
  const canBack = $derived(historyIndex > 0);
  const canForward = $derived(historyIndex < history.length - 1);

  function toast(message: string) { notice = message; clearTimeout(noticeTimer); noticeTimer = setTimeout(() => notice = '', 6500); }
  function formatTime(value: number) { const n = Math.max(0, Math.floor(value || 0)); return `${Math.floor(n / 60)}:${String(n % 60).padStart(2, '0')}`; }
  function isWatch(e: Endpoint) { return !!(e.watchEndpoint || e.watchPlaylistEndpoint); }
  async function refreshStatus() { status = await invoke<Status>('status'); player = { ...player, volume: status.volume }; }

  async function load(endpoint: Endpoint, title: string, tab = page, record = true, force = false) {
    playlistDialog = null; closeTrackMenus();
    const id = ++requestId;
    if (tab === 'library' && typeof endpoint.library === 'string') libraryCategory = endpoint.library;
    loading = true; moreLoading = false; error = ''; pageTitle = title; page = tab; lastEndpoint = endpoint;
    if (content) content.scrollTop = 0;
    if (record) {
      history = [...history.slice(0, historyIndex + 1), { endpoint, title, page: tab }].slice(-100);
      historyIndex = history.length - 1;
    }
    if (tab === 'settings' || tab === 'history' || tab === 'local') { if (tab === 'history') historyRevision++; if (tab === 'local' && force) void invoke('scan_local_music').catch(e=>toast(String(e))); loading = false; return; }
    const key = JSON.stringify(endpoint);
    const cached = force ? undefined : feedCache.get(key);
    if (cached) { feed = cached; loading = false; }
    try {
      const result = typeof endpoint.library === 'string'
        ? await invoke<Feed>('library', {category:endpoint.library})
        : endpoint.query !== undefined
        ? await invoke<Feed>('search', { query: endpoint.query })
        : await invoke<Feed>('browse', { endpoint });
      if (id !== requestId) return;
      if (feedCache.size >= 16) feedCache.delete(feedCache.keys().next().value!);
      feedCache.set(key, result);
      feed = result;
      if (result.title) pageTitle = result.title;
      if (status?.authenticated && !result.authenticated) toast('YouTube не подтвердил вход. Обнови сессию в настройках.');
    } catch (e) { if (id === requestId) { error = String(e); feed = null; void refreshStatus(); } }
    finally { if (id === requestId) loading = false; }
  }

  async function loadMore() {
    if (!feed?.continuation || moreLoading) return;
    const id = requestId;
    const endpoint = feed.continuation;
    moreLoading = true;
    try {
      const next = await invoke<Feed>('browse', { endpoint });
      if (id !== requestId || !feed) return;
      feed = { ...feed, sections: [...feed.sections, ...next.sections], continuation: next.continuation };
    } catch (e) { toast(String(e)); }
    finally { if (id === requestId) moreLoading = false; }
  }

  function showSettings() { void load({ settings: true }, 'Настройки', 'settings'); }
  function navigate(direction: number) {
    historyIndex += direction;
    const h = history[historyIndex]; if (h) void load(h.endpoint, h.title, h.page, false);
  }
  function showLocal() {void load({local:true},'На устройстве','local');}
  function open(item: Item) {
    if (item.source === 'local') { startLocalList(item,[item],null); return; }
    if (isWatch(item.endpoint)) void startQueue(item.play ?? item.endpoint);
    else void load(item.endpoint, item.title, 'browse');
  }
  async function playCard(item: Item) {
    if (item.source === 'local') {startLocalList(item,[item],null);return;}
    if (item.play || isWatch(item.endpoint)) { await startQueue(item.play ?? item.endpoint); return; }
    const id = ++queueRequest;
    resolving = true;
    try {
      const detail = await invoke<Feed>('browse', { endpoint: item.endpoint });
      if (id !== queueRequest) return;
      const track = detail.sections.flatMap(s => s.items).find(i => isWatch(i.play ?? i.endpoint));
      if (!track) throw new Error('YouTube не вернул треки для этой подборки');
      await startQueue(track.play ?? track.endpoint);
    } catch (e) { toast(String(e)); }
    finally { if (id === queueRequest) resolving = false; }
  }
  async function startQueue(endpoint: Endpoint) {
    const id = ++queueRequest;
    resolving = true;
    try {
      const result = await invoke<Queue>('queue', { endpoint });
      if (id !== queueRequest) return;
      if (!result.items.length) throw new Error('В этой очереди пока нет доступных треков');
      queue = { ...result, items: positioned(result.items) }; index = result.selected; extending = null; interactions = []; impressedIds = new Set();
      nextQueueItemId = queue.items.reduce((max, item) => Math.max(max, queueItemId(item) + 1), 0);
      radioContext = result.context; refreshChain = Promise.resolve(); pendingRadioSkip = false;
      await playAt(index);
    } catch (e) { if (id === queueRequest) { resolving = false; toast(String(e)); } }
  }
  function startLocalList(item: Item, items: Item[], continuation: Endpoint | null) {
    ++queueRequest; extending = null; interactions = []; impressedIds = new Set(); pendingRadioSkip = false; radioContext = {}; refreshChain = Promise.resolve();
    const at = Math.max(0,items.findIndex(i=>i.id===item.id));
    queue = {title:'На устройстве',items:[...items],selected:at,continuation,context:{localLibrary:true},dynamic:false};
    void playAt(at);
  }
  function enqueue(item: Item, next = false) {
    if (!queue || !current) {
      ++queueRequest; extending = null; interactions = []; impressedIds = new Set(); pendingRadioSkip = false; radioContext = {}; refreshChain = Promise.resolve();
      queue = {title:'Твоя очередь',items:[{...item,manual:true,queueKey:crypto.randomUUID()}],selected:0,continuation:null,context:{manual:true},dynamic:false};
      void playAt(0); return;
    }
    queueUndo=null;queue = insertQueued(queue,index,item,next);warmNext();
    toast(next ? 'Будет играть следующим' : 'Добавлено в очередь');
  }
  function removeFromQueue(at: number) {removeSelected([at]);}
  function moveInQueue(from: number,to: number) {queueUndo=null;if (queue) queue = moveQueued(queue,index,from,to);warmNext();}
  async function selectQueued(n:number) {
    const target=queue?.items[n];if(!target)return;
    if(restored&&queue?.dynamic){
      if(n===index){await resumePlayback();return;}
      if(!await hydrateRadio())return;
      const at=queue?.items.indexOf(target)??-1;
      if(at>=0)await playAt(at);
    }else await playAt(n);
  }
  function warmNext() {
    const track = queue?.items[index + 1];
    if (track && track.source !== 'local') void invoke('prefetch', { id: track.id }).catch(() => {});
  }
  function refreshLive(track: Item, version: number, signals: Record<string, unknown>[]) {
    const session = queueRequest;
    const job = refreshChain.then(async () => {
      if (session !== queueRequest) return;
      try {
        const next = await invoke<Queue | null>('refresh_queue', { context: radioContext, endpoint: track.endpoint, signals: { videoInteraction: signals } });
        if (session !== queueRequest || !next) return;
        // Even an older skip response advances the server session. Apply its
        // context before processing the next queued skip, without replacing UI.
        radioContext = next.context;
        if (version !== playRequest || current?.id !== track.id || !queue) return;
        const updated = refreshRadio(queue, index, next, nextQueueItemId);
        queue = updated.queue; index = updated.index; nextQueueItemId = updated.nextId; extending = null;
      } catch (e) { if (session === queueRequest && version === playRequest) toast(`Не удалось обновить микс: ${e}`); }
    });
    refreshChain = job;
    return job;
  }
  async function playAt(n: number, reason: 'select' | 'skip' | 'ended' | 'repeat' = 'select', remember = true, position = 0) {
    if (!queue?.items[n]) return;
    queueUndo=null;restored=false;
    const track = queue.items[n], dynamic = queue.dynamic;
    if (reason === 'skip' && current && current.source !== 'local' && !current.manual) {interactions.push({ videoId: current.id, queueIndex: queuePosition(current, index), playbackSkip: {} });if (dynamic) pendingRadioSkip = true;}
    if (track.source !== 'local' && !track.manual && !impressedIds.has(track.id)) {
      interactions.push({ videoId: track.id, queueIndex: queuePosition(track, n), queueImpress: {} });
      impressedIds.add(track.id);
    }
    interactions = interactions.slice(-80);
    const signals = [...interactions];
    if (remember && current && current.id !== track.id) playbackHistory = [...playbackHistory.slice(-39), current];
    const id = ++playRequest; pendingSeek = null; seekRequest++;
    index = n; current = track; liked = likeCache.get(track.id) ?? track.liked ?? false; likeBusy = false; resolving = true;
    heardSeconds = 0; lastHeardTick = 0; historyReady = false;
    if (track.source !== 'local' && status?.authenticated && track.liked == null && !likeCache.has(track.id)) {
      const account = accountVersion, revision = likeRevision;
      void invoke<boolean | null>('track_like', {id:track.id}).then(value => {
        if (account !== accountVersion || revision !== likeRevision || value === null) return;
        if (likeCache.size >= 1000) likeCache.delete(likeCache.keys().next().value!);
        likeCache.set(track.id,value);
        if (id === playRequest && !likeBusy) liked = value;
      }).catch(() => {});
    }
    ({ queue, index } = compactQueue(queue, index));
    queue = normalizeRadio(queue,index);
    player = { ...player, position, duration: position>0?player.duration:0, paused: true };
    // Refreshing a radio and starting its selected song are independent. Audio
    // never waits for the extra /next request needed to rebuild the mix.
    const shouldRefresh = dynamic && pendingRadioSkip && track.source !== 'local' && !track.manual;
    if (shouldRefresh) pendingRadioSkip = false;
    const refreshed = shouldRefresh ? refreshLive(track, id, signals) : null;
    try {
      await invoke(track.source === 'local' ? 'play_local' : 'play', { id: track.id, position, automatic:reason==='ended'||reason==='repeat' });
      if (id !== playRequest) return;
      historyReady = true;
      if (queue && queue.items.length - index < 6 && !refreshed) void extendQueue();
    } catch (e) { if (id === playRequest) toast(String(e)); }
    finally { if (id === playRequest) { resolving = false; if (refreshed) void refreshed.then(() => { if (id === playRequest) warmNext(); }); else warmNext(); } }
  }
  function extendQueue(): Promise<void> {
    if (extending) return extending;
    if (!queue?.continuation) return Promise.resolve();
    const original = queue;
    const job = Promise.resolve().then(async () => {
      try {
        if (original.continuation?.localQuery) {
          const filters = original.continuation.localQuery as {query:string;artist:string;album:string};
          const offset = Number(original.continuation.offset) || 0;
          const result = await invoke<LocalPage>('local_library',{...filters,offset});
          if (!queue || queue.context !== original.context) return;
          const known = new Set(queue.items.map(i=>i.id));
          queue = {...queue,items:[...queue.items,...result.items.filter(i=>!known.has(i.id))],continuation:offset+result.items.length<result.total ? {localQuery:filters,offset:offset+result.items.length} : null};
          return;
        }
        const next = await invoke<Queue>('queue', { endpoint: original.continuation });
        if (!queue || queue.context !== original.context) return;
        const lastServer = [...original.items].reverse().find(i=>!i.manual && i.source!=='local');
        const tail = lastServer?.id;
        const overlap = next.items.map(i => i.id).lastIndexOf(tail ?? '');
        const candidates = queue.dynamic ? next.items : overlap >= 0 ? next.items.slice(overlap + 1) : next.items;
        const blocked = new Set([...(queue.excluded ?? []),...queue.items.filter(i=>i.manual).map(i=>i.id)]);
        const addition = queue.dynamic ? radioRecommendations(candidates,[...blocked,...queue.items.map(i=>i.id)]) : candidates.filter(i=>!blocked.has(i.id));
        const repeated = JSON.stringify(next.continuation) === JSON.stringify(original.continuation);
        const first = lastServer ? queuePosition(lastServer, original.items.length - 1) + 1 : 0;
        const items = queue.dynamic ? radioTail(addition, first, nextQueueItemId) : positioned(addition, first);
        if (queue.dynamic) nextQueueItemId += items.length;
        radioContext = { ...radioContext, ...next.context };
        queue = { ...queue, context: { ...queue.context, ...next.context }, items: [...queue.items, ...items], continuation: repeated && !addition.length ? null : next.continuation };
      } catch (e) { toast(String(e)); }
      finally { if (extending === job) extending = null; }
    });
    extending = job;
    return job;
  }
  async function next(reason: 'skip' | 'ended' = 'skip') {
    if(restored&&!await hydrateRadio())return;
    if (!queue) return;
    if (reason === 'ended' && repeatMode === 'one') { await playAt(index, 'repeat', false); return; }
    const session = queueRequest, version = playRequest;
    if (index + 1 >= queue.items.length) await extendQueue();
    // A continuation may finish after a new playlist, manual selection or logout.
    if (session !== queueRequest || version !== playRequest) return;
    if (queue) {
      const at = repeatTarget(repeatMode, reason === 'ended', index, queue.items.length, queue.dynamic, !!queue.continuation);
      if (at !== null) await playAt(at, reason);
    }
  }
  function previous() {
    if (player.position > 3) void seek(0);
    else if (playbackHistory.length && queue) { const track = playbackHistory.pop()!; const n = queue.items.findIndex(i => i.id === track.id); if (n >= 0) void playAt(n, 'select', false); else { queue = { ...queue, items: [track, ...queue.items] }; void playAt(0, 'select', false); } }
    else if (index > 0) void playAt(index - 1, 'select', false);
  }
  let seekRequest = 0, pendingSeek: { target: number; until: number } | null = null;
  let seekBusy = false, queuedSeek: { target: number; version: number; track: number } | null = null;
  async function seek(value: number) {
    const version = ++seekRequest;
    const target = Math.max(0, Math.min(player.duration || value, value));
    pendingSeek = { target, until: performance.now() + 6000 };
    player = { ...player, position: target };
    if(restored){saveSession();return;}
    queuedSeek = { target, version, track: playRequest };
    if (seekBusy) return;
    seekBusy = true;
    try {
      while (queuedSeek) {
        const request = queuedSeek;
        queuedSeek = null;
        if (request.track !== playRequest || request.version !== seekRequest) continue;
        // Keep only the newest pending seek and preserve its order across async IPC.
        try { await invoke('control', { action: 'seek', value: request.target }); if(request.version===seekRequest)saveSession(); }
        catch (e) { if (request.version === seekRequest) { pendingSeek = null; toast(String(e)); } }
      }
    } finally { seekBusy = false; }
  }
  async function control(action: string, value: number | null = null) { if (action.startsWith('volume') && value !== null) player = { ...player, volume: value }; try { await invoke('control', { action, value }); } catch (e) { toast(String(e)); } }
  function togglePlayback() { if (current && !resolving) { if (!player.active) void resumePlayback(); else void control('toggle'); } }
  async function toggleLike() {
    if (!current || current.source === 'local' || likeBusy) return;
    if (!status?.authenticated) { showSettings(); return; }
    const track = current.id, version = playRequest, account = accountVersion, value = !liked;
    likeBusy = true; likeRevision++;
    try {
      await invoke('like', {id:track,liked:value});
      if (account !== accountVersion) return;
      if (likeCache.size >= 1000) likeCache.delete(likeCache.keys().next().value!);
      likeCache.set(track,value); feedCache.clear();
      if (current?.id === track && version === playRequest) liked = value;
      if (feed && page === 'liked' && !value) feed = {...feed,sections:feed.sections.map(s => ({...s,items:s.items.filter(i => i.id !== track)}))};
    } catch (e) { toast(String(e)); }
    finally { if (version === playRequest) likeBusy = false; }
  }
  async function saveCollection() {
    const collection = feed?.collection;
    if (!collection || collectionBusy) return;
    const version = requestId, value = !collection.saved;
    collectionBusy = true;
    try {
      await invoke('save_collection',{browseId:collection.browseId,saved:value});
      feedCache.clear();
      if (version === requestId && feed?.collection?.browseId === collection.browseId) feed = {...feed,collection:{...collection,saved:value}};
      toast(value ? 'Сохранено в медиатеку YouTube Music' : 'Убрано из медиатеки');
    } catch(e) { toast(String(e)); }
    finally { collectionBusy = false; }
  }
  function openLibrary(category = libraryCategory) {
    if (!status?.authenticated) { showSettings(); return; }
    libraryCategory = category;
    void load({library:category},'Медиатека','library');
  }
  function mediaCommand(command: {action:string;value:number|null;trackId:string}) {
    if (command.action === 'seek' && command.trackId !== (current?.id || '')) return;
    switch(command.action) {
      case 'repeat': if (command.value !== null && [0,1,2].includes(command.value)) { repeatMode = readRepeat(['off','all','one'][command.value]); localStorage.setItem('playback:repeat',repeatMode); } break;
      case 'toggle': togglePlayback(); break;
      case 'play': if (current && (!player.active || player.paused)) togglePlayback(); break;
      case 'pause': if (player.active && !player.paused) void control('pause'); break;
      case 'stop': if (player.active) void control('stop'); break;
      case 'next': void next(); break;
      case 'previous': previous(); break;
      case 'seek': if (command.value !== null) void seek(command.value); break;
      case 'volume': if (command.value !== null) void control('volume_commit',command.value); break;
    }
  }
  async function login() { try { await invoke('open_login'); } catch (e) { toast(String(e)); } }
  async function importBrowser(browser: string) {
    authBusy = true;
    try { await invoke('import_browser', { browser }); await connected(); }
    catch (e) { toast(String(e)); }
    finally { authBusy = false; }
  }
  async function connected() {
    accountVersion++; likeCache.clear(); playlistDialog = null;
    feedCache.clear();
    await refreshStatus();
    toast('Сессия подключена. Загружаем твою главную.');
    if (page !== 'settings') await load({ browseId: 'FEmusic_home' }, 'Для тебя', 'home');
  }
  async function logout() {
    ++requestId; ++queueRequest; ++playRequest; ++seekRequest; pendingSeek = null;
    accountVersion++; likeCache.clear(); liked = false; likeBusy = false; playlistDialog = null;
    restored=false;queueUndo=null;try{localStorage.removeItem(SESSION_KEY);}catch{/* Logout must work even with storage disabled. */}
    queue = null; current = null; focus = false; extending = null; playbackHistory = []; interactions = []; impressedIds.clear(); radioContext = {}; resolving = false; moreLoading = false; loading = false;
    try {
      saveSession();await sessionWrite;await invoke('logout');
      feedCache.clear();
      await refreshStatus(); if (page !== 'settings') await load({ browseId: 'FEmusic_home' }, 'Для тебя', 'home');
    } catch (e) { toast(String(e)); }
  }
  function keyboard(event: KeyboardEvent) {
    if (playlistDialog||sleepDialog||sharing||document.querySelector('dialog[open]')) return;
    if ((event.ctrlKey || event.metaKey) && event.code === 'KeyK') { event.preventDefault(); document.querySelector<HTMLInputElement>('.search input')?.focus(); return; }
    if (event.code === 'Escape') { focus = false; queueOpen = false; closeTrackMenus(); }
    if ((event.target instanceof Element && event.target.closest('button,input,textarea,select,summary,[contenteditable=true]'))) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (current && (event.code === 'ArrowLeft' || event.code === 'ArrowRight')) { event.preventDefault(); void seek(player.position + (event.code === 'ArrowLeft' ? -5 : 5)); }
    if (event.code === 'KeyM') { event.preventDefault(); toggleMute(); }
    if (event.code === 'KeyR') { event.preventDefault(); changeRepeat(); }
    if (event.code === 'Space' && current) { event.preventDefault(); togglePlayback(); }
  }
  $effect(() => {
    const theme = preferences.followWallpaper && wallpaper ? wallpaper
      : [...presets, ...preferences.custom].find(t => t.id === preferences.selected) ?? presets[0];
    applyTheme(theme);
  });
  let appearanceRequest = 0;
  async function saveAppearance(next: Preferences) {
    const version = ++appearanceRequest;
    const old = preferences; preferences = next;
    try { await invoke('set_appearance', { preferences: next }); }
    catch (e) { if (version === appearanceRequest) { preferences = old; toast(String(e)); } }
  }
  onMount(() => {
    const unlisteners: (() => void)[] = [];
    let disposed = false;
    const register = async <T,>(event: string, handler: (payload: T) => void) => {
      const unlisten = await listen<T>(event, e => handler(e.payload));
      if (disposed) unlisten(); else unlisteners.push(unlisten);
    };
    void register<{ rms: number; peak: number }>('audio-level', updateAudioLevel);
    void register<{action:string;value:number|null;trackId:string}>('media-command', mediaCommand);
    void register<PlayerState>('player-state', s => {
      recordListening(s);
      if (pendingSeek) {
        if (Math.abs(s.position - pendingSeek.target) < .8 || performance.now() > pendingSeek.until || !s.active) pendingSeek = null;
        else s = { ...s, position: pendingSeek.target };
      }
      const wasPaused=player.paused;
      player = restored&&!s.active?{...s,position:player.position,duration:player.duration}:s;
      if(s.paused&&!wasPaused&&!resolving)saveSession();
    });
    void register<SleepTimerState>('sleep-timer-state',value=>sleepState=value);
    void register<void>('sleep-timer-fired',()=>toast('Таймер сна: музыка остановлена'));
    void invoke<SleepTimerState>('get_sleep_timer').then(value=>{if(!disposed)sleepState=value;}).catch(()=>{});
    void register<void>('track-ended', () => { if (!resolving) void next('ended'); });
    void register<string>('player-error', message => { resolving = false; toast(message); });
    void register<{ id: string; generation: number; position: number }>('player-stream-error', failure => {
      if (current?.id !== failure.id) return;
      const version = playRequest; resolving = true;
      void invoke('recover_playback', failure).catch(e => { if (version === playRequest) toast(String(e)); })
        .finally(() => { if (version === playRequest) resolving = false; });
    });
    void register<void>('auth-updated', () => { void connected(); });
    void invoke<AppearanceState>('get_appearance').then(v => { preferences = v.preferences; if (v.wallpaper) wallpaper = v.wallpaper; }).catch(e => toast(String(e)));
    const wallpaperTimer = setInterval(() => { if (preferences.followWallpaper) void refreshWallpaper(); }, 4000);
    void invoke<string|null>('playback_session').then(raw=>{
      if(disposed||queueRequest!==0||current)return;
      const saved=decodeSession(raw);
      if(saved){queue=saved.queue;index=saved.queue.selected;current=queue.items[index];restored=true;restoredRadioId=saved.radioId;player={...player,position:saved.position,duration:saved.duration,paused:true,active:false};if(saved.partial)toast('Восстановлена загруженная часть очереди. Полный плейлист можно открыть из медиатеки.');}
    }).catch(sessionError).finally(()=>{sessionReady=true;if(!disposed)saveSession();});
    const sessionTimer=setInterval(saveSession,5000);
    const onHidden=()=>{if(document.hidden)saveSession();};
    document.addEventListener('visibilitychange',onHidden);window.addEventListener('pagehide',saveSession);
    void refreshStatus().catch(e => error = String(e));
    void load({ browseId: 'FEmusic_home' }, 'Для тебя', 'home');
    return () => { disposed = true;saveSession();clearInterval(sessionTimer);clearTimeout(undoTimer);document.removeEventListener('visibilitychange',onHidden);window.removeEventListener('pagehide',saveSession);unlisteners.forEach(f => f()); clearTimeout(noticeTimer); clearInterval(wallpaperTimer); clearTimeout(mediaTimer); };
  });
</script>

<svelte:window onkeydown={keyboard} onclick={closeTrackMenus} onresize={()=>closeTrackMenus()}/>

<div class="app" class:player-expanded={focus}>
  <aside class="sidebar">
    <button class="brand" onclick={() => load({ browseId: 'FEmusic_home' }, 'Для тебя', 'home')} aria-label="neisora — главная"><span class="brand-full">neisora<span class="brand-dot">.</span></span><span class="brand-short" aria-hidden="true">n<span class="brand-dot">.</span></span></button>
    <div class="nav-label">ТВОЯ МУЗЫКА</div>
    <nav>
      <button aria-label="Для тебя" title="Для тебя" class:active={page === 'home'} onclick={() => load({ browseId: 'FEmusic_home' }, 'Для тебя', 'home')}><House size={19}/><span>Для тебя</span></button>
      <button aria-label="Обзор" title="Обзор" class:active={page === 'explore'} onclick={() => load({ browseId: 'FEmusic_explore' }, 'Открывай новое', 'explore')}><Compass size={19}/><span>Обзор</span></button>
      <button aria-label="Миксы" title="Миксы" class:active={page === 'mixes'} onclick={() => load({ browseId: 'FEmusic_mixed_for_you' }, 'Твои миксы', 'mixes')}><Radio size={19}/><span>Миксы</span></button>
      <button aria-label="Медиатека" title="Медиатека" class:active={page === 'library'} onclick={() => openLibrary()}><Library size={19}/><span>Медиатека</span></button>
    </nav>
    <div class="nav-divider"></div>
    <div class="nav-label">КОЛЛЕКЦИЯ</div>
    <nav>
      <button aria-label="Мне нравится" title="Мне нравится" onclick={() => status?.authenticated ? load({ browseId: 'VLLM' }, 'Мне нравится', 'liked') : showSettings()} class:active={page === 'liked'}><Heart size={19}/><span>Мне нравится</span></button>
      <button aria-label="История" title="История" onclick={() => load({history:true},'История','history')} class:active={page === 'history'}><HistoryIcon size={19}/><span>История</span></button>
      <button aria-label="На устройстве" title="На устройстве" onclick={showLocal} class:active={page === 'local'}><FolderOpen size={19}/><span>На устройстве</span></button>
      <button aria-label="Очередь" title="Очередь" onclick={() => queueOpen = !queueOpen} class:active={queueOpen}><ListMusic size={19}/><span>Очередь</span>{#if queue}<span class="count">{queue.items.length}</span>{/if}</button>
    </nav>
    <div class="sidebar-bottom"><nav><button aria-label="Настройки" title="Настройки" class:active={page === 'settings'} onclick={showSettings}><Settings2 size={19}/><span>Настройки</span></button></nav><span class="sidebar-version">neisora · {appVersion}</span></div>
  </aside>

  <div class="workspace">
    <header class="topbar">
      <div class="history"><button class="icon" disabled={!canBack} aria-label="Назад" onclick={() => navigate(-1)}><ChevronLeft size={19}/></button><button class="icon" disabled={!canForward} aria-label="Вперёд" onclick={() => navigate(1)}><ChevronRight size={19}/></button></div>
      <form class="search" onsubmit={e => { e.preventDefault(); if (query.trim()) void load({ query: query.trim() }, `Поиск: ${query.trim()}`, 'search'); }}><Search size={17}/><input bind:value={query} placeholder="Треки, исполнители, альбомы" aria-label="Поиск музыки"/><kbd>↵</kbd></form>
      <div class="drag-space" data-tauri-drag-region></div>
      <button class="icon" aria-label="Обновить подборки" onclick={() => load(lastEndpoint, pageTitle, page, false, true)}><RefreshCw size={16}/></button>
      <div class="window-controls"><button aria-label="Свернуть" onclick={() => getCurrentWindow().minimize()}><Minus size={14}/></button><button aria-label="Развернуть" onclick={() => getCurrentWindow().toggleMaximize()}><Maximize2 size={13}/></button><button aria-label="Закрыть" onclick={async() => {saveSession();await sessionWrite;await getCurrentWindow().close();}}><X size={16}/></button></div>
    </header>

    <main bind:this={content} use:smoothScroll={!focus} onscroll={()=>closeTrackMenus()} class="content" class:with-queue={queueOpen}>
      {#key `${page}:${historyIndex}:${loading ? 'pending' : 'ready'}`}<div class="page-view">
      {#if page === 'settings'}<Settings {sleepState} onsleep={()=>sleepDialog=true} {status} {authBusy} {preferences} {wallpaper} onlogin={login} onimport={importBrowser} onlogout={logout} onappearance={saveAppearance} onlocal={showLocal}/>{:else if page === 'local'}<LocalLibrary onplay={startLocalList} onqueue={enqueue} onsettings={showSettings} currentId={current?.id || ''}/>{:else if page === 'history'}<ListeningHistory onplay={open} currentId={current?.id || ''} revision={historyRevision}/>{:else}
      <div class="page-heading"><div><span class="eyebrow">{page === 'search' ? 'НАЙДИ СВОЁ ЗВУЧАНИЕ' : 'МУЗЫКА, КОТОРАЯ РЯДОМ'}</span><h1>{pageTitle}</h1></div>{#if !status?.authenticated}<button class="connect-button" onclick={() => showSettings()}><LogIn size={16}/>Подключить YouTube</button>{:else}<span class="source-badge"><span></span>YouTube Music</span>{/if}</div>

      {#if page === 'library'}<div class="library-tabs" role="group" aria-label="Раздел медиатеки">{#each [['playlists','Плейлисты'],['albums','Альбомы'],['songs','Треки']] as [category,label]}<button class:active={libraryCategory === category} onclick={() => openLibrary(category)}>{label}</button>{/each}<button onclick={showLocal}>На устройстве</button>{#if libraryCategory === 'playlists'}<button class="new-playlist" onclick={()=>showPlaylist('create')}><Plus size={16}/>Создать плейлист</button>{/if}</div>{/if}
      {#if !loading && feed?.collection?.browseId && status?.authenticated}<div class="collection-actions"><button class="secondary" disabled={collectionBusy} onclick={saveCollection}>{#if collectionBusy}<LoaderCircle size={16} class="spin"/>{:else if feed.collection.saved}<BookmarkCheck size={16}/>{:else}<Bookmark size={16}/>{/if}{feed.collection.saved ? 'В медиатеке' : 'Сохранить в медиатеку'}</button>{#if feed.collection.editable}<button class="secondary playlist-add-tracks" onclick={()=>showPlaylist('tracks')}><Plus size={16}/>Добавить треки</button><button class="secondary" onclick={()=>showPlaylist('edit')}><Pencil size={16}/>Изменить</button><button class="icon" aria-label="Удалить плейлист" onclick={()=>showPlaylist('delete')}><Trash2 size={17}/></button>{/if}</div>{/if}

      {#if loading}
        <div class="skeleton hero-skeleton"></div><div class="skeleton-heading skeleton"></div><div class="skeleton-grid">{#each [1,2,3,4,5] as n}<div><div class="skeleton square"></div><div class="skeleton line"></div><div class="skeleton short-line"></div></div>{/each}</div>
      {:else if error}
        <div class="empty-state"><Music2 size={38}/><h2>Музыка пока не загрузилась</h2><p>{error}</p><button class="primary" onclick={() => load(lastEndpoint, pageTitle, page, false, true)}><RefreshCw size={16}/>Попробовать снова</button></div>
      {:else if !feed?.sections.length}
        <div class="empty-state"><Search size={38}/><h2>{page === 'search' ? 'Ничего не нашлось' : 'Здесь пока пусто'}</h2><p>{page === 'search' ? 'Попробуй другое название или имя исполнителя.' : 'Подключи аккаунт, чтобы увидеть свою музыку и персональные миксы.'}</p>{#if !status?.authenticated}<button class="primary" onclick={() => showSettings()}>Подключить YouTube</button>{/if}</div>
      {:else}
        {#if featured && (page === 'home' || page === 'mixes')}
          <section class="hero">
            {#if featured.image}<img class="hero-background" src={featured.image} alt=""/>{/if}
            <div class="hero-copy"><div class="hero-label"><span class="spark">✦</span>{status?.authenticated ? 'ПОДОБРАНО ДЛЯ ТЕБЯ' : 'ОТКРОЙ СВОЮ МУЗЫКУ'}</div><h2>{featured.title}</h2><p>{featured.subtitle || 'Включай и оставайся в своём ритме.'}</p><div class="hero-actions"><button class="primary" disabled={resolving} onclick={() => playCard(featured!)}><Play size={17} fill="currentColor"/>Слушать</button><button class="secondary" onclick={() => open(featured!)}>Открыть подборку<ArrowUpRight size={15}/></button></div></div>
            <button class="hero-art" aria-label={`Открыть ${featured.title}`} onclick={() => open(featured!)}>{#if featured.image}<img src={featured.image} alt={featured.title}/>{:else}<Radio size={70}/>{/if}<span class="art-badge"><Headphones size={14}/>Только музыка</span></button>
          </section>
        {/if}
        {#each feed.sections as section, sectionIndex}
          <section class="music-section"><div class="section-heading"><div>{#if section.subtitle}<span class="section-subtitle">{section.subtitle}</span>{/if}<h2>{section.title || (page === 'search' ? 'Результаты' : 'Треки')}</h2></div><span class="section-number">{String(sectionIndex + 1).padStart(2, '0')}</span></div>
            {#if section.items.every(i => i.kind === 'song')}
              <div class="track-list">{#each section.items as item, n}<div class="track-actions-row"><button class="track-row" class:playing={current?.id === item.id} onclick={() => open(item)}><span class="track-number">{current?.id === item.id ? '♪' : n + 1}</span>{#if item.image}<img src={coverFor(item)} alt="" loading="lazy"/>{:else}<span class="track-placeholder"><Music2 size={18}/></span>{/if}<span class="track-details"><strong>{item.title}</strong><small>{item.subtitle}</small></span><span class="track-duration">{item.duration}</span><Play size={15}/></button><div class="track-extra"><details class="track-menu" ontoggle={placeTrackMenu}><summary aria-label={`Действия с треком: ${item.title}`} title="Действия с треком"><Ellipsis size={19}/></summary><div class="track-menu-items"><button onclick={()=>{enqueue(item,true);document.querySelectorAll<HTMLDetailsElement>('.track-menu[open]').forEach(e=>e.open=false);}}><ListStart size={16}/>Воспроизвести следующим</button><button onclick={()=>{enqueue(item,false);document.querySelectorAll<HTMLDetailsElement>('.track-menu[open]').forEach(e=>e.open=false);}}><ListPlus size={16}/>Добавить в очередь</button><button onclick={()=>showPlaylist('pick',item)}><Plus size={16}/>Добавить в плейлист</button>{#if feed.collection?.editable && item.setVideoId}<hr/>{#if n > 0 && section.items[n-1]?.setVideoId}<button disabled={playlistWriting} onclick={()=>changePlaylistEntry(item,section.items[n-1].setVideoId)}><ArrowUp size={16}/>Переместить выше</button>{/if}{#if n+1 < section.items.length && (!feed.continuation || section.items[n+2]?.setVideoId)}<button disabled={playlistWriting} onclick={()=>changePlaylistEntry(item,section.items[n+2]?.setVideoId || null)}><ArrowDown size={16}/>Переместить ниже</button>{/if}<button disabled={playlistWriting} onclick={()=>changePlaylistEntry(item)}><Trash2 size={16}/>Убрать из плейлиста</button>{/if}</div></details></div></div>{/each}</div>
            {:else}
              <div class="card-grid">{#each section.items as item}<article class="music-card"><div class="card-art" class:artist-art={item.kind === 'artist'}><button class="art-open" onclick={() => open(item)} aria-label={`Открыть ${item.title}`}>{#if item.image}<img src={coverFor(item)} alt="" loading="lazy" decoding="async"/>{:else}<span class="art-placeholder"><Music2 size={35}/></span>{/if}</button>{#if item.kind !== 'artist'}<button class="card-play" aria-label={`Слушать ${item.title}`} onclick={() => playCard(item)}><Play size={20} fill="currentColor"/></button>{/if}</div><button class="card-title" onclick={() => open(item)}>{item.title}</button><p>{item.subtitle}</p>{#if item.kind === 'song'}<div class="card-queue-actions"><button class="icon" aria-label={`Воспроизвести следующим: ${item.title}`} onclick={()=>enqueue(item,true)}><ListStart size={16}/></button><button class="icon" aria-label={`Добавить в очередь: ${item.title}`} onclick={()=>enqueue(item,false)}><ListPlus size={16}/></button></div>{/if}</article>{/each}</div>
            {/if}
          </section>
        {/each}
        {#if feed.continuation}<div class="load-more"><button class="secondary" disabled={moreLoading} onclick={loadMore}>{#if moreLoading}<LoaderCircle size={16} class="spin"/>{/if}Ещё музыка</button></div>{/if}
        <footer class="content-footer"><span class="footer-wordmark">neisora.</span><small>Музыка и подборки — YouTube Music</small></footer>
      {/if}
      {/if}
      </div>{/key}
    </main>

    {#if queueOpen}<QueuePanel {queue} {index} {covers} enabled={!focus} onclose={() => queueOpen = false} onselect={(n) => void selectQueued(n)} onextend={() => void extendQueue()} onremove={removeFromQueue} onmove={moveInQueue} onclear={clearFollowing} onremoveMany={removeSelected} onundo={undoQueue} canUndo={canUndoQueue} onsave={()=>showPlaylist('queue')} onsleep={()=>sleepDialog=true}/>{/if}
  </div>

  {#if focus && current}<ExpandedPlayer track={current} cover={coverFor(current)} {player} {resolving} onclose={() => focus = false} ontoggle={togglePlayback} onprevious={previous} onnext={() => void next()} onseek={seek} visualizer={preferences.visualizer} {repeatMode} onrepeat={changeRepeat} shuffled={queue?.shuffled??false} shuffleDisabled={!queue||queue.dynamic} {shuffleBusy} onshuffle={()=>void changeShuffle()} onplaylist={()=>{if(current)showPlaylist('pick',current);}} onqueue={() => { focus = false; queueOpen = true; }}/>{/if}

  <footer class="playerbar">
    <div class="current-track"><button class="cover-button" disabled={!current} onclick={() => focus = !focus} aria-label="Открыть экран трека">{#if current && coverFor(current)}<img src={coverFor(current)} alt=""/>{:else if current?.source === 'local'}<LocalArtwork item={current}/>{:else}<Music2 size={24}/>{/if}</button><div><button class="now-title" onclick={() => { if (current) focus = !focus; }}>{current?.title || 'Найди свой следующий трек'}</button><p>{current?.subtitle || 'Начни с подборки или поиска'}</p></div>{#if current && current.source !== 'local'}<button class="icon like-button" class:liked disabled={likeBusy} onclick={toggleLike} aria-label={liked ? 'Убрать отметку нравится' : 'Мне нравится'}><Heart size={18} fill={liked ? 'currentColor' : 'none'}/></button><button class="icon share-button" aria-label="Поделиться текущим треком" title="Поделиться" onclick={()=>{if(current)shareItem(current);}}><Share2 size={18}/></button>{/if}</div>
    <div class="playback"><div class="playback-buttons"><ShuffleButton active={queue?.shuffled??false} disabled={!current||!!queue?.dynamic} busy={shuffleBusy} onchange={()=>void changeShuffle()}/><button class="icon" disabled={!current} aria-label="Предыдущий трек" onclick={previous}><SkipBack size={19} fill="currentColor"/></button><button class="main-play" disabled={!current || resolving} aria-label={player.paused ? 'Воспроизвести' : 'Пауза'} onclick={togglePlayback}>{#if resolving || player.buffering}<LoaderCircle size={21} class="spin"/>{:else if !player.paused && player.active}<Pause size={19} fill="currentColor"/>{:else}<Play size={19} fill="currentColor"/>{/if}</button><button class="icon" disabled={!current} aria-label="Следующий трек" onclick={() => void next()}><SkipForward size={19} fill="currentColor"/></button><RepeatButton mode={repeatMode} disabled={!current} onchange={changeRepeat}/></div><div class="seek"><span>{formatTime(player.position)}</span><SeekBar position={player.position} duration={player.duration} label="Позиция воспроизведения" onseek={seek}/><span>{formatTime(player.duration)}</span></div></div>
    <div class="player-options"><button class="icon expand-player" disabled={!current} aria-label="Развернуть плеер" onclick={() => focus = true}><ChevronsUp size={19}/></button><button class="icon" class:enabled={queueOpen} aria-label="Очередь воспроизведения" onclick={() => queueOpen = !queueOpen}><ListMusic size={19}/></button><button class="icon mute-button" aria-label={player.volume > 0 ? 'Выключить звук' : 'Включить звук'} aria-pressed={player.volume === 0} onclick={toggleMute}>{#if player.volume > 0}<Volume2 size={18}/>{:else}<VolumeX size={18}/>{/if}</button><VolumeSlider value={player.volume} onvolume={(value, commit) => control(commit ? 'volume_commit' : 'volume', value)}/></div>
  </footer>
</div>

{#if notice}<div class="toast" role="status"><span>{notice}</span><button class="icon" aria-label="Закрыть уведомление" onclick={() => notice = ''}><X size={16}/></button></div>{/if}

{#if playlistDialog}<PlaylistDialog mode={playlistDialog.mode} track={playlistDialog.track} tracks={playlistDialog.tracks} partial={playlistDialog.partial} collection={feed?.collection} initialTitle={playlistDialog.mode==='queue'?playlistDialog.title||'Твоя очередь':playlistDialog.mode === 'edit' || playlistDialog.mode === 'delete' || playlistDialog.mode === 'tracks' ? pageTitle : ''} onclose={()=>playlistDialog=null} onchanged={(id,deleted)=>void playlistChanged(id,deleted)}/>{/if}

{#if sharing}<ShareDialog target={sharing.target} title={sharing.title} onclose={()=>sharing=null}/>{/if}

{#if sleepDialog}<SleepTimerDialog timer={sleepState} hasTrack={!!current} onclose={()=>sleepDialog=false} onchanged={value=>sleepState=value}/>{/if}
