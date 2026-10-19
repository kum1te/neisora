import assert from 'node:assert/strict';
import { test } from 'node:test';
import { compactQueue, positioned, queuePosition } from '../src/queue.ts';
import type { Item, Queue } from '../src/types.ts';
const track = (n: number): Item => ({ id: `track${n}`, title: `Track ${n}`, subtitle: '', image: '', kind: 'song', duration: '', selected: false, endpoint: { watchEndpoint: { videoId: `track${n}`, opaque: 'preserve' } }, play: null });
const queue = (items: Item[]): Queue => ({ title: 'Mix', items, selected: 0, context: { queueContextParams: 'server-session' }, continuation: { token: 'next-page' }, dynamic: true });
test('finite playlists keep earlier tracks available for direct selection', () => {
  const original = { ...queue(positioned(Array.from({length:200},(_, n)=>track(n)))), dynamic: false };
  const result=compactQueue(original, 190);
  assert.equal(result.queue, original);
  assert.equal(result.index, 190);
  assert.equal(result.queue.items[0].id, 'track0');
});

test('large queue keeps current track, future and opaque server context while pruning past', () => {
  const original = queue(positioned(Array.from({ length: 10_000 }, (_, n) => track(n))));
  const result = compactQueue(original, 9000);
  assert.equal(result.index, 40);
  assert.equal(result.queue.items.length, 1040);
  assert.equal(result.queue.items[result.index].id, 'track9000');
  assert.equal(queuePosition(result.queue.items[result.index]), 9000);
  assert.equal(result.queue.items.at(-1)?.id, 'track9999');
  assert.equal(result.queue.context, original.context);
  assert.equal(result.queue.continuation, original.continuation);
  assert.equal(original.items.length, 10_000);
});
test('server queue index wins over UI position and endpoints are never rewritten', () => {
  const item = track(0);
  (item.endpoint.watchEndpoint as Record<string, unknown>).index = '17007';
  const [copy] = positioned([item], 9000);
  assert.equal(queuePosition(copy), 17007);
  assert.equal(copy.endpoint, item.endpoint);
  assert.deepEqual(copy.endpoint, { watchEndpoint: { videoId: 'track0', opaque: 'preserve', index: '17007' } });
});
test('continuing radio retains a bounded past through thousands of natural transitions', () => {
  let current = queue(positioned(Array.from({ length: 20 }, (_, n) => track(n))));
  let index = 0, generated = 20;
  for (let n = 0; n < 2000; n++) {
    assert.equal(current.items[index].id, `track${n}`);
    assert.equal(queuePosition(current.items[index]), n);
    if (index + 1 === current.items.length) {
      const addition = positioned(Array.from({ length: 20 }, (_, n) => track(generated + n)), generated);
      generated += 20;
      current = { ...current, items: [...current.items, ...addition] };
    }
    ({ queue: current, index } = compactQueue(current, index + 1));
    assert.ok(current.items.length <= 61);
  }
});

test('radio tail resets cannot reuse endpoint IDs or reset listening position', async () => {
  const { refreshRadio, queueItemId } = await import('../src/queue.ts');
  let current = queue(positioned(Array.from({ length: 25 }, (_, n) => track(n))));
  let index = 0, nextId = 25;
  for (let skip = 1; skip <= 100; skip++) {
    index++;
    const playing = current.items[index];
    const response = { ...queue(Array.from({ length: 25 }, (_, n) => {
      const item = track(skip * 100 + n);
      (item.endpoint.watchEndpoint as Record<string, unknown>).index = n;
      return item;
    })), updateSection: 'QUEUE_UPDATE_SECTION_QUEUE' };
    const updated = refreshRadio(current, index, response, nextId);
    current = updated.queue; index = updated.index;
    assert.equal(current.items[index], playing);
    assert.equal(queuePosition(playing), skip);
    assert.equal(queuePosition(current.items[index + 1]), skip + 1);
    assert.equal(queueItemId(current.items[index + 1]), nextId);
    assert.equal((current.items[index + 1].endpoint.watchEndpoint as Record<string, unknown>).opaque, 'preserve');
    assert.equal((response.items[0].endpoint.watchEndpoint as Record<string, unknown>).index, 0);
    nextId = updated.nextId;
    assert.ok(current.items.length <= 66);
  }
});

test('manual local and YouTube entries survive a replacement radio tail in order', async()=>{
  const {insertQueued,refreshRadio}=await import('../src/queue.ts');
  const local={...track(90),id:'local:'+'a'.repeat(40),source:'local' as const,endpoint:{localId:'local:'+'a'.repeat(40)}};
  let q=queue(positioned([track(0),track(1),track(2),track(3)]));
  q=insertQueued(q,1,local,true);q=insertQueued(q,1,track(91),false);
  const result=refreshRadio(q,1,{...queue([track(20),track(21),track(22)]),updateSection:'QUEUE_UPDATE_SECTION_QUEUE'},100);
  assert.deepEqual(result.queue.items.map(i=>i.id),['track0','track1',local.id,'track20','track21','track91','track22']);
  assert.equal(result.queue.items[2].manual,true);assert.equal(result.nextId,103);
  assert.equal((result.queue.items[3].endpoint.watchEndpoint as {index:number}).index,100);
});
test('reordering preserves current playback and removes the server duplicate of the moved entry',async()=>{
  const {moveQueued,refreshRadio}=await import('../src/queue.ts');
  const q=queue(positioned([track(0),track(1),track(2),track(3)]));
  assert.equal(moveQueued(q,1,1,3),q);
  const moved=moveQueued(q,1,3,2);assert.equal(moved.items[1],q.items[1]);assert.equal(moved.items[2].manual,true);
  const result=refreshRadio(moved,1,{...queue([track(3),track(20),track(21)]),updateSection:'QUEUE_UPDATE_SECTION_QUEUE'},4);
  assert.deepEqual(result.queue.items.map(i=>i.id),['track0','track1','track3','track20','track21']);
});
test('removing a recommendation survives radio refresh and removing the current track is a no-op',async()=>{
  const {removeQueued,refreshRadio}=await import('../src/queue.ts');
  const q=queue(positioned([track(0),track(1),track(2),track(3)]));
  assert.equal(removeQueued(q,1,1).queue,q);
  const removed=removeQueued(q,1,2);assert.equal(removed.index,1);
  const result=refreshRadio(removed.queue,1,{...queue([track(2),track(20)]),updateSection:'QUEUE_UPDATE_SECTION_QUEUE'},4);
  assert.deepEqual(result.queue.items.map(i=>i.id),['track0','track1','track20']);
  const past=removeQueued(q,2,0);assert.equal(past.index,1);assert.equal(past.queue.items[past.index].id,'track2');
});

import { shuffleQueue, removeManyQueued } from '../src/queue.ts';
test('shuffle keeps played/current occurrences, preserves duplicates, and unshuffle restores future order after advancing',()=>{
 const q={...queue([track(0),track(1),track(2),track(2),track(4)]),dynamic:false,continuation:null};
 const shuffled=shuffleQueue(q,1,()=>0);
 assert.deepEqual(shuffled.items.slice(0,2).map(i=>i.id),['track0','track1']);
 assert.deepEqual(shuffled.items.map(i=>i.shuffleOrder),[0,1,3,4,2]);
 const restored=shuffleQueue(shuffled,2);
 assert.deepEqual(restored.items.map(i=>i.shuffleOrder),[0,1,3,2,4]);
 assert.equal(restored.shuffled,false);
});
test('shuffle refuses radio and incomplete playlists; bulk removal targets occurrences and preserves current',()=>{
 const q=queue([track(0),track(1),track(1),track(3)]);
 assert.equal(shuffleQueue(q,1),q);
 assert.equal(shuffleQueue({...q,dynamic:false},1).shuffled,undefined);
 const result=removeManyQueued(q,1,[0,1,2,2,99,-1]);
 assert.deepEqual(result.queue.items.map(i=>i.id),['track1','track3']);assert.equal(result.index,0);
 assert.deepEqual(result.queue.excluded,['track0','track1']);
});

test('a later shuffle restores the latest manual order rather than an earlier shuffle session',()=>{
 const q={...queue([track(0),track(1),track(2),track(3)]),dynamic:false,continuation:null};
 const once=shuffleQueue(shuffleQueue(q,0,()=>0),0);
 const manual={...once,items:[once.items[0],once.items[3],once.items[1],once.items[2]]};
 const again=shuffleQueue(shuffleQueue(manual,0,()=>0),0);
 assert.deepEqual(again.items.map(i=>i.id),manual.items.map(i=>i.id));
});

test('inline radio replacement cannot enqueue the current or just-skipped track again',async()=>{
 const {refreshRadio}=await import('../src/queue.ts');
 const q=queue(positioned([track(0),track(1),track(8)]));
 const response={...queue([track(1),track(2),track(0),track(2),track(3)]),updateSection:'QUEUE_UPDATE_SECTION_QUEUE'};
 const result=refreshRadio(q,1,response,40);
 assert.deepEqual(result.queue.items.map(i=>i.id),['track0','track1','track2','track3']);
 assert.equal(queuePosition(result.queue.items[2]),2);
 assert.equal(result.nextId,42);
 assert.equal(result.queue.context,response.context);
});

test('radio entry and continuation cleanup preserve manual repeats and original server indices',async()=>{
 const {normalizeRadio,radioRecommendations}=await import('../src/queue.ts');
 const items=positioned([track(0),track(0),track(2),track(2),track(3),{...track(0),manual:true}]);
 const initial=normalizeRadio(queue(items),0);
 assert.deepEqual(initial.items.map(i=>i.id),['track0','track2','track3','track0']);
 assert.equal(queuePosition(initial.items[1]),2);
 assert.equal(initial.items.at(-1)?.manual,true);
 assert.equal(initial.items[0],items[0]);
 const addition=radioRecommendations([track(2),track(4),track(4),track(0),track(5)],initial.items.map(i=>i.id));
 assert.deepEqual(addition.map(i=>i.id),['track4','track5']);
 const finite={...queue(items),dynamic:false};assert.equal(normalizeRadio(finite,0),finite);
});
