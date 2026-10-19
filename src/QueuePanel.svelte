<script lang="ts">
  import { tick, untrack, onDestroy } from 'svelte';
  import { X, ListMusic, Search, GripVertical, Ellipsis, Trash2, Undo2, CheckSquare, ListPlus, Moon } from 'lucide-svelte';
  import LocalArtwork from './LocalArtwork.svelte';
  import { smoothScroll } from './scroll';
  import type { Queue, Item } from './types';
  let { queue, index, covers = {}, enabled, onselect, onclose, onextend, onremove, onmove, onclear, onremoveMany, onundo, canUndo, onsave, onsleep }: {
    queue: Queue | null; covers?: Record<string,string>; index: number; enabled: boolean;
    onselect: (n: number) => void; onclose: () => void; onextend: () => void;
    onremove:(n:number)=>void; onmove:(from:number,to:number)=>void; onclear:()=>void; onremoveMany:(positions:number[])=>void; onundo:()=>void; canUndo:boolean; onsave:()=>void;onsleep:()=>void;
  } = $props();
  const pitch = 60, overscan = 6;
  let list = $state<HTMLDivElement>(), top = $state(0), height = $state(600), query=$state(''), dragging=$state<number|null>(null), dropTarget=$state<number|null>(null);
  let selecting=$state(false), selected=$state<Item[]>([]);
  let actions=$state<HTMLDetailsElement>();
  const selectedItems=$derived(new Set(selected));
  const selectedPositions=$derived((queue?.items??[]).flatMap((item,n)=>n!==index&&selectedItems.has(item)?[n]:[]));
  $effect(()=>{const items=new Set(queue?.items);untrack(()=>{selected=selected.filter(item=>items.has(item));});});
  function choose(item:Item){selected=selected.includes(item)?selected.filter(i=>i!==item):[...selected,item];}
  let cancelDrag=()=>{};
  onDestroy(()=>cancelDrag());
  const filtered=$derived((queue?.items ?? []).map((item,n)=>({item,n})).filter(({item})=>`${item.title} ${item.subtitle}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())));
  const start = $derived(Math.max(0, Math.floor(top / pitch) - overscan));
  const end = $derived(Math.min(filtered.length, Math.ceil((top + height) / pitch) + overscan));
  const rows = $derived(filtered.slice(start,end));
  const currentKey = $derived(queue?.items[index]?.id);
  async function navigate(event: KeyboardEvent, at: number) {
    if (event.ctrlKey || event.metaKey || event.altKey || !queue || !list) return;
    const page = Math.max(1, Math.floor(height / pitch)), position=filtered.findIndex(row=>row.n===at);
    const targets: Record<string, number> = { ArrowDown: position + 1, ArrowUp: position - 1, PageDown: position + page, PageUp: position - page, Home: 0, End: filtered.length - 1 };
    if (!(event.key in targets)) return;
    event.preventDefault(); event.stopPropagation();
    const wanted = Math.max(0, Math.min(filtered.length - 1, targets[event.key]));
    const y = wanted * pitch;
    if (y < list.scrollTop || y + pitch > list.scrollTop + list.clientHeight) list.scrollTop = Math.max(0, y - list.clientHeight / 2 + pitch / 2);
    top = list.scrollTop; await tick();
    list?.querySelector<HTMLButtonElement>(`[data-queue-index="${filtered[wanted]?.n}"]`)?.focus({ preventScroll: true });
  }
  async function moveWithKeyboard(event:KeyboardEvent,n:number){
    if(event.key!=='ArrowUp'&&event.key!=='ArrowDown')return;
    event.preventDefault();event.stopPropagation();
    const to=n+(event.key==='ArrowUp'?-1:1);
    if(!queue||to<=index||to>=queue.items.length)return;
    onmove(n,to);await tick();
    list?.querySelector<HTMLButtonElement>(`[data-queue-drag="${to}"]`)?.focus({preventScroll:true});
  }
  function startDrag(event:PointerEvent,from:number){
    if(event.button!==0||!queue||from<=index||!list)return;
    event.preventDefault();cancelDrag();
    (event.currentTarget as HTMLButtonElement).focus({preventScroll:true});
    const item=queue.items[from],originX=event.clientX,originY=event.clientY,pointer=event.pointerId,element=list;
    element.setPointerCapture(pointer);
    let x=originX,y=originY,frame=0,last=0;
    function inside(){const r=list!.getBoundingClientRect();return x>=r.left&&x<=r.right&&y>=r.top&&y<=r.bottom;}
    function target(){
      if(!list||!inside()){dropTarget=null;return;}
      const r=list.getBoundingClientRect(),at=Math.max(0,Math.min(filtered.length-1,Math.floor((y-r.top+list.scrollTop)/pitch)));
      const n=filtered[at]?.n;dropTarget=n!==undefined&&n>index?n:null;
    }
    function scroll(now:number){
      frame=0;if(dragging===null||!list)return;
      const r=list.getBoundingClientRect(),dt=last?Math.min(32,now-last):16;last=now;
      if(x>=r.left&&x<=r.right){
        const speed=y<r.top+42?-Math.min(1,(r.top+42-y)/42):y>r.bottom-42?Math.min(1,(y-r.bottom+42)/42):0;
        if(speed){list.scrollTop+=speed*dt*.65;top=list.scrollTop;}
      }
      target();frame=requestAnimationFrame(scroll);
    }
    function move(e:PointerEvent){
      if(e.pointerId!==pointer)return;x=e.clientX;y=e.clientY;
      if(dragging===null&&Math.hypot(x-originX,y-originY)<5)return;
      e.preventDefault();dragging=from;target();if(!frame)frame=requestAnimationFrame(scroll);
    }
    function cleanup(){
      if(element.hasPointerCapture(pointer))element.releasePointerCapture(pointer);
      cancelAnimationFrame(frame);window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',finish);window.removeEventListener('pointercancel',abort);window.removeEventListener('blur',abort);
      dragging=null;dropTarget=null;cancelDrag=()=>{};
    }
    function finish(e:PointerEvent){
      if(e.pointerId!==pointer)return;
      x=e.clientX;y=e.clientY;target();
      const to=dragging!==null?dropTarget:null,valid=queue?.items[from]===item;
      cleanup();if(valid&&to!==null&&to!==from)onmove(from,to);
    }
    function abort(){cleanup();}
    window.addEventListener('pointermove',move,{passive:false});window.addEventListener('pointerup',finish);window.addEventListener('pointercancel',abort);window.addEventListener('blur',abort);cancelDrag=cleanup;
  }
  $effect(() => {
    const element = list;if (!element) return;
    const observer = new ResizeObserver(() => height = element.clientHeight);
    observer.observe(element); height = element.clientHeight;return () => observer.disconnect();
  });
  $effect(()=>{query;if(list){list.scrollTop=0;top=0;}});
  $effect(() => {
    const active = index, key = currentKey, element = list;
    untrack(() => {
      if (!element || !key) return;
      const at=filtered.findIndex(row=>row.n===active);if(at<0)return;
      const y = at * pitch;
      if (y < element.scrollTop || y + pitch > element.scrollTop + element.clientHeight) element.scrollTop = Math.max(0, y - element.clientHeight / 2 + pitch / 2);
      top = element.scrollTop;
    });
  });
</script>

<aside class="queue-panel" class:reordering={dragging!==null}>
  <div class="queue-heading"><div><span class="eyebrow">СЛЕДОМ ЗА ТОБОЙ</span><h2>Очередь</h2></div><button class="icon" aria-label="Закрыть очередь" onclick={onclose}><X size={19}/></button></div>
  {#if queue}
    <p class="queue-title">{queue.title || 'YouTube Music'}{#if queue.dynamic}<span> · живая подборка</span>{/if}</p>
    <label class="queue-search"><Search size={15}/><input bind:value={query} placeholder="Найти в очереди" aria-label="Поиск в очереди"/>{#if query}<button class="icon" aria-label="Очистить поиск в очереди" onclick={()=>query=''}><X size={13}/></button>{/if}</label>
    <div class="queue-tools">
      {#if selecting}<span>{selectedPositions.length} выбрано</span><button class="icon" aria-label="Удалить выбранные треки" disabled={!selectedPositions.length} onclick={()=>{onremoveMany(selectedPositions);selected=[];selecting=false;}}><Trash2 size={17}/></button><button class="icon" aria-label="Отменить выбор треков" onclick={()=>{selecting=false;selected=[];}}><X size={17}/></button>
      {:else}<span>{query ? `Найдено ${filtered.length} из ${queue.items.length}` : `${queue.items.length} треков`}</span>{#if canUndo}<button class="queue-undo" onclick={onundo}><Undo2 size={15}/>Отменить</button>{/if}<details class="queue-tools-menu" bind:this={actions}><summary aria-label="Управление очередью"><Ellipsis size={19}/></summary><div><button onclick={()=>{onsave();if(actions)actions.open=false;}}><ListPlus size={16}/>Сохранить как плейлист</button><button onclick={()=>{onsleep();if(actions)actions.open=false;}}><Moon size={16}/>Таймер сна</button><button onclick={()=>{selecting=true;if(actions)actions.open=false;}}><CheckSquare size={16}/>Выбрать треки</button><button disabled={index+1>=queue.items.length&&!queue.continuation} onclick={()=>{onclear();if(actions)actions.open=false;}}><Trash2 size={16}/>Очистить следующие</button></div></details>{/if}
    </div>
    <div class="queue-list virtual-queue" role="region" aria-label="Очередь треков" use:smoothScroll={enabled} bind:this={list} onscroll={() => { if (list) top = list.scrollTop; }}>
      <div class="queue-rows" style:height={`${filtered.length * pitch}px`}>
        {#each rows as row, local (row.item)}
          {@const item=row.item}{@const n=row.n}
          <div class="queue-edit-row" class:selecting role="group" aria-label={item.title} class:drag-target={dropTarget===n&&dragging!==n} class:drag-source={dragging===n} style:top={`${(start+local)*pitch}px`}>
            {#if selecting&&n!==index}<input class="queue-select" type="checkbox" checked={selected.includes(item)} aria-label={`Выбрать: ${item.title}`} onchange={()=>choose(item)}/>{/if}
            <button class="queue-item" class:playing={n === index} data-queue-index={n} aria-label={`${n + 1} из ${queue.items.length}: ${item.title}`} aria-current={n === index ? 'true' : undefined} onkeydown={event => void navigate(event, n)} onclick={() => onselect(n)}>
              {#if item.source==='local'}<span class="queue-local-cover"><LocalArtwork {item}/></span>{:else if item.image}<img src={covers[item.id] || item.image} alt="" loading="lazy"/>{/if}<span><strong>{item.title}</strong><small>{#if item.source==='local'}На устройстве · {/if}{item.subtitle}{#if item.manual} · Добавлено тобой{/if}</small></span>{#if n === index}<span class="now-dot"></span>{/if}
            </button>
            {#if n!==index&&!selecting}
              <div class="queue-edit-actions">
                <button aria-label={`Убрать из очереди: ${item.title}`} title="Убрать из очереди" onclick={()=>onremove(n)}><X size={15}/></button>
                {#if n>index}<button class="queue-drag" data-queue-drag={n} onpointerdown={event=>startDrag(event,n)} onkeydown={event=>void moveWithKeyboard(event,n)} aria-label={`Перетащить: ${item.title}`} title="Перетащи для перестановки · ↑ / ↓ с клавиатуры"><GripVertical size={16}/></button>{/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
      {#if !filtered.length}<p class="queue-empty-search">В очереди ничего не нашлось</p>{/if}
      {#if queue.continuation}<button class="secondary" onclick={onextend}>Продолжить очередь</button>{/if}
    </div>
  {:else}<div class="queue-empty"><ListMusic size={34}/><p>Включи трек или микс.<br/>Дальше музыка подхватит.</p></div>{/if}
</aside>

<style>
  .queue-tools{display:flex;align-items:center;gap:8px;min-height:32px;margin:0 0 9px;color:var(--muted);font-size:11px}.queue-tools>span{flex:1}.queue-tools .icon{width:27px;height:27px}.queue-undo{display:flex;align-items:center;gap:5px;font-size:11px;color:var(--accent)}.queue-tools-menu{position:relative}.queue-tools-menu summary{display:grid;place-items:center;cursor:pointer;list-style:none;width:28px;height:28px}.queue-tools-menu summary::-webkit-details-marker{display:none}.queue-tools-menu>div{position:absolute;right:0;top:30px;z-index:5;min-width:220px;padding:6px;background:var(--surface);border:1px solid var(--line);border-radius:12px;box-shadow:0 10px 30px #0008}.queue-tools-menu button{display:flex;align-items:center;gap:10px;width:100%;padding:11px;text-align:left;border-radius:7px}.queue-tools-menu button:hover{background:#ffffff08}.queue-edit-row.selecting{display:flex;align-items:center;gap:5px}.queue-edit-row.selecting .queue-item{flex:1;min-width:0}.queue-select{flex-shrink:0;accent-color:var(--accent);width:17px;height:17px;margin:0 4px;cursor:pointer}
  .virtual-queue{display:block;flex:1;min-height:0}.queue-rows{position:relative}
  .queue-edit-row{position:absolute;left:0;right:0;height:57px;border-radius:9px}
  .queue-edit-row .queue-item{position:relative;inset:auto;height:57px;width:100%;padding-right:62px}
  .queue-edit-row .queue-item.playing{padding-right:8px}
  .queue-edit-actions{position:absolute;right:4px;top:0;height:57px;display:flex;align-items:center;gap:2px;opacity:0;pointer-events:none;transition:opacity .12s}
  .queue-edit-row:hover .queue-edit-actions,.queue-edit-row:has(:focus-visible) .queue-edit-actions,.reordering .drag-source .queue-edit-actions{opacity:1;pointer-events:auto}
  .queue-edit-actions button{height:30px;width:24px;padding:0;display:grid;place-items:center;background:none;border:0;border-radius:4px;color:var(--muted);opacity:.6;transition:color .15s,opacity .15s}
  .queue-edit-actions button:hover,.queue-edit-actions button:focus-visible{color:var(--text);opacity:1}
  .queue-edit-actions button:focus-visible{outline:1px solid var(--accent);outline-offset:1px}
  .queue-drag{cursor:grab;touch-action:none;user-select:none}.queue-drag:active{cursor:grabbing}.drag-source{opacity:.45}.drag-target{box-shadow:inset 0 2px var(--accent);background:color-mix(in srgb,var(--accent) 7%,transparent)}
  .queue-search{display:flex;align-items:center;gap:8px;padding:9px 12px;border:1px solid var(--line);border-radius:10px;margin:0 0 14px;color:var(--muted)}.queue-search input{width:100%;background:none;border:0;outline:none;font:inherit;font-size:11px;color:var(--text)}.queue-search .icon{height:18px;width:18px}.queue-edit-row .queue-item>.queue-local-cover{height:38px;width:38px;flex:0 0 38px;border-radius:5px;overflow:hidden}.queue-empty-search{color:var(--muted);font-size:12px;padding:20px 0}
</style>
