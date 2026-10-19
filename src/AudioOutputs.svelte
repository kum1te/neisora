<script lang="ts">
 import {onMount} from 'svelte';
 import {listen,type UnlistenFn} from '@tauri-apps/api/event';
 import {Headphones,ChevronDown,Check,RefreshCw,LoaderCircle} from 'lucide-svelte';
 import {invoke} from './backend';
 import type {AudioOutputState} from './types';
 let outputState=$state<AudioOutputState|null>(null),busy=$state(false),error=$state(''),menu:HTMLDetailsElement,alive=true;
 const label=$derived(outputState?.devices.find(d=>d.name===outputState?.selected)?.description||'Как в системе');
 onMount(()=>{let unlisten:UnlistenFn|undefined;void listen<AudioOutputState>('audio-output-state',e=>{if(alive)outputState=e.payload;}).then(off=>{if(alive)unlisten=off;else off();});void refresh();return()=>{alive=false;unlisten?.();};});
 async function refresh(){if(busy)return;busy=true;error='';try{const value=await invoke<AudioOutputState>('audio_outputs');if(alive)outputState=value;}catch(e){if(alive)error=String(e);}finally{if(alive)busy=false;}}
 async function choose(name:string,close=true){if(busy)return;busy=true;error='';try{const value=await invoke<AudioOutputState>('set_audio_output',{name});if(alive){outputState=value;if(close)menu.open=false;}}catch(e){if(alive)error=String(e);}finally{if(alive)busy=false;}}
 function navigate(event:KeyboardEvent){
  if(!['ArrowUp','ArrowDown','Home','End'].includes(event.key)||busy)return;
  const buttons=[...(event.currentTarget as HTMLDivElement).querySelectorAll<HTMLButtonElement>('button[data-output]')];
  const at=buttons.indexOf(document.activeElement as HTMLButtonElement);if(at<0)return;
  event.preventDefault();const next=event.key==='Home'?0:event.key==='End'?buttons.length-1:(at+(event.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length;
  buttons[next]?.focus();const name=buttons[next]?.dataset.output;if(name)void choose(name,false);
 }
</script>
<section class="settings-section audio-settings" aria-labelledby="audio-title">
 <div class="settings-section-heading"><span class="settings-symbol"><Headphones size={22}/></span><div><h2 id="audio-title">Аудиовыход</h2><p>Куда воспроизводить музыку.</p></div></div>
 <div class="output-control"><details class="output-selector" bind:this={menu}><summary aria-label="Выбрать аудиовыход"><span>{outputState?label:'Загружаем устройства…'}</span><ChevronDown size={17}/></summary><div class="output-options" role="radiogroup" tabindex="-1" aria-label="Аудиоустройства" onkeydown={navigate}>
 {#if outputState?.fallback}<button role="radio" aria-checked="true" disabled>Сохранённое устройство · отключено</button>{/if}
 {#each outputState?.devices??[] as device(device.name)}<button role="radio" data-output={device.name} aria-checked={outputState?.preferred===device.name} disabled={busy} onclick={()=>choose(device.name)}><span>{device.description}</span>{#if outputState?.preferred===device.name}<Check size={16}/>{/if}</button>{/each}
 </div></details><button class="icon" disabled={busy} aria-label="Обновить аудиоустройства" onclick={refresh}>{#if busy}<LoaderCircle class="spin" size={17}/>{:else}<RefreshCw size={17}/>{/if}</button></div>
 {#if error}<p class="output-error" role="alert">{error}</p>{/if}
 {#if outputState?.fallback}<p class="output-note" role="status">Выбранное устройство отключено. Пока используем системный выход; вернёмся к выбранному при подключении.</p>{:else}<p class="output-note">«Как в системе» следует системным настройкам звука. Выбор другого устройства действует только в neisora.</p>{/if}
 {#if outputState?.silent}<p class="output-note" role="status">Плеер работает без звука. Проверь подключение устройства или выбери другой выход.</p>{/if}
</section>
<style>
 .output-control{display:flex;align-items:flex-start;gap:10px}.output-selector{flex:1;min-width:0;max-width:560px}summary{display:flex;align-items:center;justify-content:space-between;gap:14px;cursor:pointer;list-style:none;padding:13px 15px;border:1px solid var(--line);border-radius:12px;background:var(--bg);color:var(--text);font-size:13px}summary::-webkit-details-marker{display:none}summary span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.output-options{margin-top:8px;max-height:300px;overflow:auto;border:1px solid var(--line);border-radius:12px;padding:5px;background:var(--bg)}.output-options button{width:100%;display:flex;justify-content:space-between;align-items:center;gap:12px;padding:12px;text-align:left;font-size:12px;border-radius:8px}.output-options button span{min-width:0;overflow-wrap:anywhere}.output-options button[aria-checked=true]{color:var(--accent);background:color-mix(in srgb,var(--accent) 8%,transparent)}.output-options button:hover{background:var(--surface)}.output-note{font-size:12px;line-height:1.8;color:var(--muted);margin-top:14px}.output-error{font-size:12px;line-height:1.7;color:#ffaaaa;margin-top:12px}
</style>
