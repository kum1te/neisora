import type { Item, Queue } from './types';

/** Listening position survives pruning and is independent of endpoint item IDs. */
export function queuePosition(item: Item, fallback = 0): number {
  if (item.queuePosition !== undefined) return item.queuePosition;
  const server = (item.endpoint.watchEndpoint as { index?: unknown } | undefined)?.index;
  const position = typeof server === 'number' || (typeof server === 'string' && /^\d+$/.test(server)) ? Number(server) : NaN;
  return Number.isSafeInteger(position) && position >= 0 ? position : item.queuePosition ?? fallback;
}
export function positioned(items: Item[], first = 0): Item[] {
  return items.map((item, n) => ({ ...item, queuePosition: queuePosition(item, first + n) }));
}
/** Previously played entries already live in the bounded back-button history. */
export function compactQueue(queue: Queue, index: number, keepPast = 40): { queue: Queue; index: number } {
  if (!queue.dynamic) return { queue, index };
  const removed = Math.max(0, index - keepPast);
  if (!removed) return { queue, index };
  return { queue: { ...queue, items: queue.items.slice(removed), selected: index - removed }, index: index - removed };
}

/** The web client assigns fresh endpoint IDs when it replaces a radio tail.
 * These IDs are independent of the listening position reported in skip signals. */
export function queueItemId(item: Item): number {
  return queuePosition({ ...item, queuePosition: undefined });
}
export function radioTail(items: Item[], position: number, firstId: number): Item[] {
  return items.map((item, n) => ({
    ...item, queuePosition: position + n,
    endpoint: { ...item.endpoint, watchEndpoint: { ...(item.endpoint.watchEndpoint as object), index: firstId + n } },
  }));
}
/** Radio recommendations are not explicit repeats. Keep user-added occurrences intact. */
export function radioRecommendations(items:Item[], blocked:Iterable<string> = []):Item[] {
  const seen=new Set([...blocked,...items.filter(i=>i.manual||i.source==='local').map(i=>i.id)]);
  return items.filter(item=>{
    if(item.manual||item.source==='local')return true;
    if(seen.has(item.id))return false;
    seen.add(item.id);return true;
  });
}
/** Keep the selected occurrence and server identity; clean only its automatic future. */
export function normalizeRadio(queue:Queue,index:number):Queue {
  if(!queue.dynamic||!queue.items[index])return queue;
  const tail=queue.items.slice(index+1);
  const clean=radioRecommendations(tail,queue.items.slice(Math.max(0,index-40),index+1).map(i=>i.id));
  return clean.length===tail.length?queue:{...queue,items:[...queue.items.slice(0,index+1),...clean]};
}
export function refreshRadio(queue: Queue, index: number, next: Queue, firstId: number) {
  const track = queue.items[index];
  const selected = next.items[next.selected]?.id === track.id ? next.selected : next.items.findIndex(i => i.id === track.id);
  // An inline QUEUE update contains only future items, even when a repeated ID
  // happens to match the current song. Never cut that response at the first ID.
  const tail = next.updateSection === 'QUEUE_UPDATE_SECTION_QUEUE' ? next.items
    : selected >= 0 ? next.items.slice(selected + 1) : next.items;
  const past = queue.items.slice(Math.max(0, index - 40), index);
  const manual: {item:Item;after:number}[] = [];
  let preceding = 0;
  for (const item of queue.items.slice(index + 1)) {
    if (item.manual || item.source === 'local') manual.push({item,after:preceding});
    else preceding++;
  }
  const excluded = new Set([...(queue.excluded ?? []),...manual.map(m=>m.item.id),...past.map(i=>i.id),track.id]);
  const server = radioTail(radioRecommendations(tail,excluded), queuePosition(track, index) + 1, firstId);
  const items = [...server];
  manual.forEach((entry,n) => items.splice(Math.min(entry.after,server.length)+n,0,entry.item));
  return { queue: { ...next, excluded:queue.excluded, items: [...past, track, ...items], selected: past.length }, index: past.length, nextId: firstId + server.length };
}

/** User entries are independent of the radio's endpoint IDs and positions. */
export function insertQueued(queue: Queue, index: number, item: Item, next: boolean): Queue {
  const entry = { ...item, manual: true, queueKey: crypto.randomUUID() };
  const at = next ? index + 1 : queue.items.length;
  return { ...queue, items: [...queue.items.slice(0,at),entry,...queue.items.slice(at)] };
}
export function removeQueued(queue: Queue, index: number, at: number) {
  if (at === index || at < 0 || at >= queue.items.length) return {queue,index};
  const item = queue.items[at];
  const excluded = !item.manual && item.source !== 'local' ? [...new Set([...(queue.excluded ?? []),item.id])].slice(-1000) : queue.excluded;
  return {queue:{...queue,excluded,items:queue.items.filter((_,n) => n !== at)},index:index-(at<index ? 1 : 0)};
}
export function moveQueued(queue: Queue, index: number, from: number, to: number): Queue {
  if (from <= index || to <= index || from >= queue.items.length || to >= queue.items.length || from === to) return queue;
  const items = [...queue.items], [item] = items.splice(from,1);
  items.splice(to,0,{...item,manual:true,queueKey:item.queueKey ?? crypto.randomUUID()});
  return {...queue,items};
}

/** Shuffle only the unplayed part; played entries and the current occurrence stay put. */
export function shuffleQueue(queue: Queue, index: number, random = Math.random): Queue {
  if (queue.dynamic || queue.continuation || index < 0 || !queue.items[index]) return queue;
  const items = queue.items.map((item,n)=>({...item,shuffleOrder:queue.shuffled ? item.shuffleOrder ?? n : n}));
  const tail=items.slice(index+1);
  if (queue.shuffled) tail.sort((a,b)=>a.shuffleOrder-b.shuffleOrder);
  else for(let n=tail.length-1;n>0;n--){const at=Math.floor(random()*(n+1));[tail[n],tail[at]]=[tail[at],tail[n]];}
  return {...queue,shuffled:!queue.shuffled,items:[...items.slice(0,index+1),...tail]};
}
export function removeManyQueued(queue: Queue, index: number, positions: number[]) {
  const removed=new Set(positions.filter(n=>Number.isInteger(n)&&n>=0&&n<queue.items.length&&n!==index));
  if(!removed.size)return {queue,index};
  const blocked=queue.items.filter((_,n)=>removed.has(n)).filter(i=>!i.manual&&i.source!=='local').map(i=>i.id);
  return {queue:{...queue,items:queue.items.filter((_,n)=>!removed.has(n)),excluded:[...new Set([...(queue.excluded??[]),...blocked])].slice(-1000)},index:index-[...removed].filter(n=>n<index).length};
}
