<script lang="ts">
 import {onMount} from 'svelte';
 import {Activity,Copy,RefreshCw,Trash2,LoaderCircle} from 'lucide-svelte';
 import {invoke} from './backend';
 interface Report {schema:number;version:string;platform:string;architecture:string;components:Record<string,boolean>;graphics:{safeRenderer:boolean};player:{active:boolean;paused:boolean;buffering:boolean};events:{secondsAgo:number;stage:string;outcome:string;code:string|null}[]}
 let report=$state<Report|null>(null),busy=$state(false),error=$state(''),copied=$state(false),alive=true;
 const failure=$derived(report?.events.slice().reverse().find(e=>e.outcome==='failed'));
 const stages:Record<string,string>={resolve:'получение аудиопотока',open_audio:'открытие аудио',playback:'воспроизведение',recovery:'восстановление потока',control:'управление плеером'};
 const codes:Record<string,string>={timeout:'истекло время ожидания',access_denied:'сервис отказал в доступе',unavailable:'аудио недоступно',component:'проблема компонента',unknown:'причина не определена'};
 onMount(()=>{void refresh();return()=>{alive=false;};});
 async function refresh(action:'read'|'copy'|'clear'='read'){
  if(busy)return;busy=true;error='';copied=false;
  try{if(action==='clear')await invoke('clear_diagnostics');const value=await invoke<Report>(action==='copy'?'copy_diagnostic_report':'diagnostic_report');if(alive){report=value;copied=action==='copy';}}
  catch(e){if(alive)error=String(e);}finally{if(alive)busy=false;}
 }
</script>
<section class="settings-section diagnostics-section" aria-labelledby="diagnostics-title">
 <div class="settings-section-heading"><span class="settings-symbol"><Activity size={22}/></span><div><h2 id="diagnostics-title">Диагностика</h2><p>Поможет разобраться, почему музыка не запускается или прерывается.</p></div></div>
 {#if failure}<p class="diagnostic-status">Последний сбой: {stages[failure.stage]||'работа плеера'} — {failure.code?codes[failure.code]||'причина не определена':'причина не определена'}.</p>{:else if report}<p class="diagnostic-status">Сбоев воспроизведения в журнале этой сессии нет.</p>{/if}
 <p class="diagnostic-privacy">Только состояние приложения и до 32 последних событий плеера. Без названий треков, истории, cookies, адресов потоков и путей к файлам. Ничего не отправляется автоматически.</p>
 {#if error}<p class="diagnostic-error" role="alert">{error}</p>{/if}
 <div class="diagnostic-actions"><button class="secondary" disabled={busy||!report} onclick={()=>refresh('copy')}><Copy size={16}/>{copied?'Скопировано':'Скопировать отчёт'}</button><button class="icon" aria-label="Обновить диагностику" disabled={busy} onclick={()=>refresh()}>{#if busy}<LoaderCircle class="spin" size={17}/>{:else}<RefreshCw size={17}/>{/if}</button><button class="icon" aria-label="Очистить журнал диагностики" disabled={busy||!report?.events.length} onclick={()=>refresh('clear')}><Trash2 size={17}/></button></div>
 {#if report}<details class="diagnostic-preview"><summary>Посмотреть отчёт</summary><textarea readonly aria-label="Отчёт диагностики" value={JSON.stringify(report,null,2)} spellcheck="false"></textarea></details>{/if}
 <span class="diagnostic-status" role="status">{copied?'Отчёт скопирован в буфер обмена':''}</span>
</section>
<style>
 .diagnostic-status,.diagnostic-privacy{font-size:12px;line-height:1.8}.diagnostic-status{color:var(--text);display:block}.diagnostic-privacy{color:var(--muted);margin:10px 0 16px}.diagnostic-actions{display:flex;gap:10px;align-items:center;flex-wrap:wrap}.diagnostic-error{color:#ffaaaa;font-size:12px;line-height:1.7;margin-bottom:12px}.diagnostic-preview{margin-top:18px;font-size:12px;color:var(--muted)}summary{cursor:pointer}textarea{display:block;margin-top:12px;width:100%;min-height:250px;max-height:450px;resize:vertical;background:var(--bg);color:var(--text);border:1px solid var(--line);border-radius:12px;padding:14px;font:11px/1.7 monospace;overflow:auto;box-sizing:border-box}
</style>
