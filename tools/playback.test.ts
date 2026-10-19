import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { readRepeat, cycleRepeat, repeatTarget, resumePosition } from '../src/playback.ts';
test('repeat cycles and invalid persistence defaults to off',()=>{
 assert.equal(readRepeat('garbage'),'off'); assert.equal(readRepeat(null),'off');
 assert.deepEqual(['off','all','one'].map(v=>cycleRepeat(readRepeat(v))),['all','one','off']);
});
test('repeat one replays at EOF but explicit Next advances',()=>{
 assert.equal(repeatTarget('one',true,2,4,false,false),2);
 assert.equal(repeatTarget('one',false,2,4,false,false),3);
 assert.equal(repeatTarget('one',false,3,4,false,false),null);
});
test('only finite fully loaded queues wrap, never live radio or pending continuations',()=>{
 assert.equal(repeatTarget('all',true,3,4,false,false),0);
 assert.equal(repeatTarget('all',false,3,4,false,false),0);
 assert.equal(repeatTarget('all',true,3,4,true,false),null);
 assert.equal(repeatTarget('all',true,3,4,false,true),null);
 assert.equal(repeatTarget('off',true,3,4,false,false),null);
 assert.equal(repeatTarget('all',true,0,0,false,false),null);
});

test('inactive Play resumes a restored position, but restarts after EOF or explicit stop',()=>{
 assert.equal(resumePosition(true,58.5,200),58.5);
 assert.equal(resumePosition(false,200,200),0);
 assert.equal(resumePosition(true,200,200),0);
 assert.equal(resumePosition(false,58.5,200),0);
 assert.equal(resumePosition(true,-1,200),0);
 assert.equal(resumePosition(true,NaN,200),0);
});
