<script lang="ts">
 import {onMount,untrack} from 'svelte';
 import {X,Search,LoaderCircle,Check} from 'lucide-svelte';
 import {invoke} from './backend';
 import type {Item,LyricCandidate,LyricSelection} from './types';
 let {track,selectedId,onclose,onchanged}:{track:Item;selectedId?:number;onclose:()=>void;onchanged:(value:LyricSelection|null)=>void}=$props();
 let dialog:HTMLDialogElement,query=$state(''),results=$state<LyricCandidate[]>([]),preview=$state<LyricSelection|null>(null),loading=$state(false),previewing=$state<number|null>(null),saving=$state(false),error=$state(''),currentId=$state<number|undefined>(untrack(()=>selectedId)),alive=true,searchId=0,previewId=0;
 onMount(()=>{dialog.showModal();void search();void invoke<LyricSelection|null>('get_lyric_choice',{id:track.id}).then(value=>{if(alive)currentId=value?.record.id;}).catch(()=>{});return()=>{alive=false;searchId++;previewId++;};});
 async function search(){
  const version=++searchId;previewId++;results=[];preview=null;previewing=null;loading=true;error='';
  try{const values=await invoke<LyricCandidate[]>('lyric_versions',{title:track.title,artist:track.subtitle,query:query.trim()});if(alive&&version===searchId)results=values;}
  catch(e){if(alive&&version===searchId)error=String(e);}finally{if(alive&&version===searchId)loading=false;}
 }
 async function view(record:LyricCandidate){
  const version=++previewId;preview=null;previewing=record.id;error='';
  try{const value=await invoke<LyricSelection>('preview_lyric_version',{recordId:record.id});if(alive&&version===previewId)preview=value;}
  catch(e){if(alive&&version===previewId)error=String(e);}finally{if(alive&&version===previewId)previewing=null;}
 }
 async function apply(recordId:number|null){
  if(saving)return;saving=true;error='';
  try{const value=await invoke<LyricSelection|null>('select_lyric_version',{id:track.id,recordId});if(alive){onchanged(value);onclose();}}
  catch(e){if(alive)error=String(e);}finally{if(alive)saving=false;}
 }
 function duration(seconds:number){const n=Math.floor(seconds);return `${Math.floor(n/60)}:${String(n%60).padStart(2,'0')}`;}
</script>
<dialog class="lyric-versions" bind:this={dialog} aria-labelledby="versions-title" oncancel={e=>{e.preventDefault();if(!saving)onclose();}}>
 <header><div><span class="eyebrow">ТЕКСТ ПЕСНИ</span><h2 id="versions-title">Выбрать версию</h2></div><button class="icon" aria-label="Закрыть выбор текста" disabled={saving} onclick={onclose}><X size={20}/></button></header>
 <p class="version-track">{track.title}</p><p class="version-hint">Сверь исполнителя, длительность и начало текста. Выбор сохранится для этого трека.</p>
 <form onsubmit={e=>{e.preventDefault();void search();}}><input aria-label="Поиск версии текста" bind:value={query} maxlength="200" placeholder="Другое название или исполнитель" disabled={saving}/><button class="secondary" aria-label="Найти версии текста" disabled={loading||saving}>{#if loading}<LoaderCircle size={17} class="spin"/>{:else}<Search size={17}/>{/if}</button></form>
 {#if error}<p class="version-error" role="alert">{error}</p>{/if}
 <div class="version-results" aria-busy={loading}>{#each results as record(record.id)}<button class="version-result" class:previewed={preview?.record.id===record.id} disabled={saving||previewing!==null} aria-label={`Посмотреть текст: ${record.title} — ${record.artist}`} onclick={()=>view(record)}><span><strong>{record.title}</strong><small>{record.artist}{record.album?` · ${record.album}`:''}</small><small>{duration(record.duration)} · {record.wordTimed?'По словам':record.synced?'По строкам':'Без синхронизации'}</small></span>{#if currentId===record.id}<Check size={18}/>{:else if previewing===record.id}<LoaderCircle size={18} class="spin"/>{/if}</button>{:else}<p class="version-hint">{loading?'Ищем версии…':'Версии не найдены. Попробуй другое название или исполнителя.'}</p>{/each}</div>
 {#if preview?.lyrics}<section class="version-preview" aria-label="Предпросмотр текста"><p class="version-hint">{preview.record.wordTimed?'С таймкодами слов':preview.lyrics.synced?'С таймкодами строк':'Без таймкодов'}</p><pre>{preview.lyrics.plain||preview.lyrics.lines.map(l=>l.text).join('\n')}</pre></section>{/if}
 <footer><button class="secondary" disabled={saving||!currentId} onclick={()=>apply(null)}>Автоматический подбор</button><button class="primary" disabled={saving||!preview?.lyrics} onclick={()=>{if(preview)void apply(preview.record.id);}}>{#if saving}<LoaderCircle size={16} class="spin"/>{/if}Использовать</button></footer>
</dialog>
<style>
 .lyric-versions{margin:auto;width:min(620px,calc(100vw - 32px));max-height:calc(100dvh - 32px);overflow:auto;background:var(--surface);color:var(--text);border:1px solid var(--line);border-radius:22px;padding:26px;box-shadow:0 24px 100px #0009}.lyric-versions::backdrop{background:#0009;backdrop-filter:blur(8px)}header{display:flex;justify-content:space-between;gap:14px}h2{font-size:23px;margin-top:8px}.version-track{font-size:14px;line-height:1.6;margin-top:18px;overflow-wrap:anywhere}.version-hint{font-size:12px;line-height:1.7;color:var(--muted);margin:10px 0}form{display:flex;gap:8px;margin:16px 0}input{flex:1;width:100%;min-width:0;border:1px solid var(--line);border-radius:10px;background:var(--bg);color:var(--text);padding:12px;font-size:12px}.version-results{max-height:260px;overflow:auto;min-height:70px}.version-result{width:100%;text-align:left;display:flex;align-items:center;justify-content:space-between;gap:12px;padding:12px;border-radius:10px;margin-bottom:3px;border:1px solid transparent}.version-result:hover,.version-result.previewed{background:var(--bg);border-color:var(--line)}.version-result span{display:flex;flex-direction:column;gap:5px;min-width:0}.version-result strong,.version-result small{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.version-result strong{font-size:13px}.version-result small{font-size:11px;color:var(--muted)}.version-result :global(>svg){color:var(--accent)}.version-preview{margin-top:12px;border-top:1px solid var(--line)}pre{font:inherit;font-size:13px;line-height:1.8;white-space:pre-wrap;overflow-wrap:anywhere;max-height:180px;overflow:auto;margin:12px 0}.version-error{font-size:12px;color:#ffaaaa;line-height:1.6}footer{display:flex;justify-content:space-between;gap:10px;flex-wrap:wrap;margin-top:22px}
</style>
