import assert from 'node:assert/strict';
import { test } from 'node:test';
import {snapshot,encodeSession,decodeSession} from '../src/session.ts';
import {shuffleQueue} from '../src/queue.ts';
import type {Item,Queue} from '../src/types.ts';
const item=(n:number):Item=>({id:'test'+String(n).padStart(7,'0'),title:'Song '+n,subtitle:'Artist',image:'https://i.ytimg.com/image.jpg?private=secret',kind:'song',endpoint:{watchEndpoint:{videoId:'test'+String(n).padStart(7,'0'),queueContextParams:'private-token'}},play:{url:'https://example.com/secret'},duration:'3:20',selected:false});
const queue=(dynamic=false):Queue=>({title:'Queue',items:[0,1,2,3].map(item),selected:1,dynamic,continuation:null,context:{playlistId:'RDtest',queueContextParams:'secret'}});
test('session round trip keeps duplicate occurrences, shuffle order, position and excludes tokens/URLs',()=>{
 const q=queue();q.items[2].id=q.items[1].id;
 const shuffled=shuffleQueue(q,1,()=>0), saved=snapshot(shuffled,1,72.5,200)!;
 const raw=encodeSession(saved)!;assert.ok(!/secret|private-token|queueContextParams|private=/.test(raw));
 const restored=decodeSession(raw)!;assert.equal(restored.position,72.5);assert.equal(restored.queue.shuffled,true);
 assert.deepEqual(shuffleQueue(restored.queue,1).items.map(i=>i.shuffleOrder),[0,1,2,3]);
 assert.equal(restored.queue.items.filter(i=>i.id===item(1).id).length,2);
});
test('radio persists only current and explicit additions, retaining no stale server recommendations',()=>{
 const q=queue(true);q.items[3].manual=true;
 const saved=decodeSession(encodeSession(snapshot(q,1,25,200)!))!;
 assert.deepEqual(saved.queue.items.map(i=>i.title),['Song 1','Song 3']);assert.equal(saved.radioId,'RDtest');
 assert.equal(decodeSession(encodeSession(snapshot(saved.queue,0,25,200)!))!.radioId,'RDtest');
});
test('session is bounded around the active occurrence and incomplete restores are marked',()=>{
 const q={...queue(),items:Array.from({length:10000},(_,n)=>item(n)),continuation:{private:'token'}};
 const saved=snapshot(q,9000,1,200)!;
 assert.equal(saved.queue.items.length,2000);assert.equal(saved.queue.items[saved.queue.selected].id,item(9000).id);assert.equal(saved.partial,true);
 const decoded=decodeSession(encodeSession(saved))!;assert.equal(decoded.partial,true);assert.equal(snapshot(decoded.queue,decoded.queue.selected,1,200)!.partial,true);
});
test('corrupt, oversized, invalid IDs and command injection are rejected or stripped',()=>{
 for(const raw of [null,'bad','null','{}','x'.repeat(3_000_001),JSON.stringify({version:1,queue:{items:[item(0)],selected:99}})])assert.equal(decodeSession(raw),null);
 const q=queue();q.items[0].id='../../etc/passwd';assert.equal(snapshot(q,0,0,0),null);
 q.items[0]=item(0);q.items[0].image='file:///etc/passwd';
 const saved=snapshot(q,0,NaN,Infinity)!;assert.equal(saved.queue.items[0].image,'');assert.equal(saved.position,0);assert.equal(saved.duration,0);
 q.items[0]={...item(0),source:'local',id:'local:'+'a'.repeat(40),image:'https://i.ytimg.com/local.jpg'};
 const local=snapshot(q,0,0,0)!;assert.deepEqual(local.queue.items[0].endpoint,{localId:q.items[0].id});assert.equal(local.queue.items[0].image,'');
});
