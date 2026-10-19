<script lang="ts">
  import {onMount} from 'svelte';
  import {Moon, X, LoaderCircle} from 'lucide-svelte';
  import {invoke} from './backend';
  import type {SleepTimerState} from './types';
  let {timer,hasTrack,onclose,onchanged}:{timer:SleepTimerState;hasTrack:boolean;onclose:()=>void;onchanged:(state:SleepTimerState)=>void}=$props();
  let dialog:HTMLDialogElement,minutes=$state(30),busy=$state(false),error=$state(''),now=$state(Date.now());
  let alive=true;
  const remaining=$derived(timer.deadlineMs?Math.max(0,Math.ceil((timer.deadlineMs-now)/1000)):0);
  onMount(()=>{dialog.showModal();const timer=setInterval(()=>now=Date.now(),1000);return()=>{alive=false;clearInterval(timer);};});
  async function set(mode:'off'|'track'|'minutes',duration?:number){
    if(busy)return;busy=true;error='';
    try{const next=await invoke<SleepTimerState>('set_sleep_timer',{request:mode==='minutes'?{mode,minutes:duration}:{mode}});if(alive){onchanged(next);onclose();}}
    catch(e){if(alive)error=String(e);}
    finally{if(alive)busy=false;}
  }
</script>
<dialog class="sleep-dialog" bind:this={dialog} aria-labelledby="sleep-title" oncancel={event=>{event.preventDefault();if(!busy)onclose();}}>
  <header><div><span class="eyebrow">ОТДЫХАЙ ПОД МУЗЫКУ</span><h2 id="sleep-title"><Moon size={22}/>Таймер сна</h2></div><button class="icon" aria-label="Закрыть таймер сна" disabled={busy} onclick={onclose}><X size={20}/></button></header>
  <p class="sleep-hint">Музыка встанет на паузу. Громкость останется прежней.</p>
  {#if timer.mode!=='off'}<p class="sleep-current" role="status">{timer.mode==='track'?'Остановимся после ближайшего завершившегося трека':`Осталось ${Math.floor(remaining/60)}:${String(remaining%60).padStart(2,'0')}`}</p>{/if}
  {#if error}<p class="sleep-error" role="alert">{error}</p>{/if}
  <div class="sleep-presets">{#each [15,30,45,60,90,120] as value}<button class="secondary" disabled={busy} onclick={()=>set('minutes',value)}>{value} мин</button>{/each}</div>
  <form onsubmit={event=>{event.preventDefault();void set('minutes',minutes);}}><label>Своё время, минут<input type="number" aria-label="Время таймера в минутах" bind:value={minutes} min="1" max="1440" step="1" required disabled={busy}/></label><button class="primary" disabled={busy||!Number.isInteger(minutes)||minutes<1||minutes>1440}>{#if busy}<LoaderCircle size={16} class="spin"/>{/if}Включить</button></form>
  <button class="secondary sleep-after-track" disabled={busy||!hasTrack} onclick={()=>set('track')}>После текущего трека</button>
  <p class="sleep-hint">Если пропустишь трек, таймер дождётся естественного окончания следующего. После перезапуска приложения таймер выключен.</p>
  {#if timer.mode!=='off'}<button class="secondary sleep-cancel" disabled={busy} onclick={()=>set('off')}>Выключить таймер</button>{/if}
</dialog>
<style>
 .sleep-dialog{margin:auto;width:min(450px,calc(100vw - 32px));max-height:calc(100dvh - 32px);overflow:auto;padding:26px;border:1px solid var(--line);border-radius:22px;background:var(--surface);color:var(--text);box-shadow:0 24px 100px #0009}.sleep-dialog::backdrop{background:#0009;backdrop-filter:blur(8px)}header{display:flex;align-items:flex-start;justify-content:space-between;gap:12px}h2{display:flex;align-items:center;gap:10px;font-size:23px;margin:9px 0 18px}.sleep-hint{font-size:12px;color:var(--muted);line-height:1.7;margin:10px 0 18px}.sleep-current{color:var(--accent);font-size:14px;line-height:1.5;margin:12px 0}.sleep-error{font-size:12px;color:#ffaaaa}.sleep-presets{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px}form{display:flex;gap:12px;align-items:flex-end;margin:20px 0}label{flex:1;min-width:0;font-size:12px;color:var(--muted)}input{display:block;width:100%;background:var(--bg);color:var(--text);border:1px solid var(--line);border-radius:10px;padding:11px;margin-top:8px;font:inherit;color-scheme:dark}form .primary{flex-shrink:0;min-height:40px}.sleep-after-track,.sleep-cancel{width:100%;justify-content:center}.sleep-cancel{color:var(--accent)}
</style>
