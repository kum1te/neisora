<script lang="ts">
 import {onMount} from 'svelte';
 import {Copy, ExternalLink, X, LoaderCircle} from 'lucide-svelte';
 import {invoke} from './backend';
 import type {ShareTarget} from './sharing';
 let {target,title,onclose}:{target:ShareTarget;title:string;onclose:()=>void}=$props();
 let dialog:HTMLDialogElement,url=$state(''),busy=$state(false),loading=$state(true),error=$state(''),copied=$state(false),alive=true;
 onMount(()=>{dialog.showModal();void invoke<string>('share_link',{target}).then(value=>{if(alive)url=value;}).catch(e=>{if(alive)error=String(e);}).finally(()=>{if(alive)loading=false;});return()=>{alive=false;};});
 async function action(open:boolean){
  if(busy||!url)return;busy=true;error='';
  try{await invoke(open?'open_share_link':'copy_share_link',{target});if(alive){copied=!open;if(open)onclose();}}
  catch(e){if(alive)error=String(e);}finally{if(alive)busy=false;}
 }
</script>
<dialog class="share-dialog" bind:this={dialog} aria-labelledby="share-title" oncancel={event=>{event.preventDefault();if(!busy)onclose();}}>
 <header><h2 id="share-title">Поделиться</h2><button class="icon" aria-label="Закрыть окно ссылки" disabled={busy} onclick={onclose}><X size={20}/></button></header>
 <p class="share-name">{title}</p>
 <p class="share-description">Ссылка на YouTube Music. Доступ к личному плейлисту определяется его настройками в YouTube.</p>
 {#if loading}<LoaderCircle class="spin" size={20}/>{:else if url}<input aria-label="Ссылка YouTube Music" readonly value={url} onclick={event=>event.currentTarget.select()}/>{/if}
 {#if error}<p class="share-error" role="alert">{error}</p>{/if}
 <div class="share-actions"><button class="primary" disabled={busy||!url} onclick={()=>action(false)}><Copy size={16}/>{copied?'Скопировано':'Скопировать'}</button><button class="secondary" disabled={busy||!url} onclick={()=>action(true)}><ExternalLink size={16}/>Открыть в браузере</button></div>
 <span class="sr-only" role="status">{copied?'Ссылка скопирована':''}</span>
</dialog>
<style>
 .share-dialog{margin:auto;width:min(480px,calc(100vw - 32px));max-height:calc(100dvh - 32px);overflow:auto;background:var(--surface);color:var(--text);border:1px solid var(--line);border-radius:22px;padding:26px;box-shadow:0 24px 100px #0009}.share-dialog::backdrop{background:#0009;backdrop-filter:blur(8px)}header{display:flex;align-items:center;justify-content:space-between;gap:12px}h2{font-size:23px}.share-name{font-size:15px;line-height:1.6;overflow-wrap:anywhere;margin:16px 0 10px}.share-description{font-size:12px;color:var(--muted);line-height:1.7;margin-bottom:18px}input{width:100%;padding:13px 12px;background:var(--bg);color:var(--text);border:1px solid var(--line);border-radius:10px;font-size:12px}.share-actions{display:flex;gap:10px;flex-wrap:wrap;margin-top:22px}.share-error{color:#ffaaaa;font-size:12px;line-height:1.6;margin-top:14px}.sr-only{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%)}
</style>
