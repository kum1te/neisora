import type { Item, Queue } from './types';
export const SESSION_KEY='playback:session:v1';
const LIMIT=2000, MAX_BYTES=3_000_000;
export interface SavedSession {version:1;queue:Queue;position:number;duration:number;radioId?:string;partial?:boolean}
function number(value:unknown,max:number){return typeof value==='number'&&Number.isFinite(value)&&value>=0&&value<=max?value:0;}
function text(value:unknown,max:number){return typeof value==='string'?value.slice(0,max):'';}
/** Persist IDs and display metadata only, never provider tokens, paths or stream URLs. */
function clean(value:unknown):Item|null {
  if(!value||typeof value!=='object')return null;
  const item=value as Partial<Item>,id=text(item.id,100),local=item.source==='local';
  if(!(local?/^local:[a-fA-F0-9]{40}$/:/^[\w-]{11}$/).test(id)||!item.title||typeof item.title!=='string')return null;
  let image='';
  try{const url=new URL(text(item.image,2000));if(url.protocol==='https:'&&!url.username&&!url.password&&(!url.port||url.port==='443')&&/^(i\.ytimg\.com|cdn-images\.dzcdn\.net|(?:[\w-]+\.)*(?:googleusercontent|ggpht)\.com)$/.test(url.hostname)){url.search='';url.hash='';image=url.href;}}catch{/* Missing covers are harmless. */}
  return {id,title:text(item.title,500),subtitle:text(item.subtitle,500),album:text(item.album,500),image:local?'':image,kind:'song',duration:text(item.duration,32),selected:false,source:local?'local':null,endpoint:local?{localId:id}:{watchEndpoint:{videoId:id}},play:null,manual:item.manual===true,queueKey:text(item.queueKey,80)||undefined,shuffleOrder:typeof item.shuffleOrder==='number'&&Number.isSafeInteger(item.shuffleOrder)&&item.shuffleOrder>=0&&item.shuffleOrder<=100_000?item.shuffleOrder:undefined,isMusicVideo:item.isMusicVideo===true};
}
export function snapshot(queue:Queue,index:number,position:number,duration:number):SavedSession|null {
  if(!queue.items[index])return null;
  const first=queue.dynamic?index:Math.max(0,Math.min(index-40,queue.items.length-LIMIT));
  const source=queue.dynamic?[queue.items[index],...queue.items.slice(index+1).filter(i=>i.manual||i.source==='local')]:queue.items.slice(first,first+LIMIT);
  const cleaned=source.slice(0,LIMIT).map(clean);
  if(!cleaned[queue.dynamic?0:index-first]||cleaned.some(i=>!i))return null;
  const items=cleaned as Item[],selected=queue.dynamic?0:index-first;
  const radioId=queue.dynamic&&typeof queue.context.playlistId==='string'&&/^RD[\w-]{1,198}$/.test(queue.context.playlistId)?queue.context.playlistId:undefined;
  return {version:1,queue:{title:text(queue.title,300),items,selected,dynamic:queue.dynamic===true,shuffled:queue.shuffled===true&&!queue.dynamic,context:{restored:true,playlistId:radioId,restoredPartial:!queue.dynamic&&(queue.items.length>LIMIT||!!queue.continuation||queue.context.restoredPartial===true)},continuation:null},position:number(position,604800),duration:number(duration,604800),radioId,partial:!queue.dynamic&&(queue.items.length>LIMIT||!!queue.continuation||queue.context.restoredPartial===true)};
}
export function encodeSession(value:SavedSession):string|null {const raw=JSON.stringify(value);return raw.length<=MAX_BYTES?raw:null;}
export function decodeSession(raw:string|null):SavedSession|null {
  if(!raw||raw.length>MAX_BYTES)return null;
  try{
    const value=JSON.parse(raw);
    if(value?.version!==1||!Array.isArray(value.queue?.items)||!value.queue.items.length||value.queue.items.length>LIMIT||!Number.isInteger(value.queue.selected)||value.queue.selected<0||value.queue.selected>=value.queue.items.length)return null;
    const q:Queue={...value.queue,context:{playlistId:value.radioId,restoredPartial:value.partial===true},dynamic:value.queue.dynamic===true};
    return snapshot(q,q.selected,value.position,value.duration);
  }catch{return null;}
}
