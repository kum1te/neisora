"""Real UI reproduction: skip must never restart a duplicate radio recommendation."""
import json,time
from webdriver_client import execute,invoke

def wait(script):
 end=time.monotonic()+12
 while time.monotonic()<end:
  if execute(script):return
  time.sleep(.05)
 raise AssertionError(script+'\n'+str(execute('return {title:document.querySelector(".now-title")?.textContent,queue:[...document.querySelectorAll(".queue-item strong")].map(e=>e.textContent),calls:radioCalls}')))
def click(selector):execute('document.querySelector('+json.dumps(selector)+').click()')
execute('''window.radioBase=window.__VESPER_TEST_INVOKE__;window.radioSavedQueue=structuredClone(testQueue);window.radioCalls=[];window.radioPlays=[];window.radioReads=[];
testQueue={title:'Duplicate radio regression',items:[0,0,1,1,2,2,3,4,5,6].map(makeTrack),selected:0,dynamic:true,context:{playlistId:'RDtest',queueContextParams:'radio-start'},continuation:null};
window.__VESPER_TEST_INVOKE__=async(cmd,args,...rest)=>{
 if(cmd==='queue'){
  radioReads.push(JSON.parse(JSON.stringify(args)));
  if(args.endpoint.continuation==='radio-tail')return {...testQueue,items:[10,9,11,11].map(makeTrack),selected:0,context:{playlistId:'RDtest',queueContextParams:'radio-after-tail'},continuation:null};
 }
 if(cmd==='play')radioPlays.push(args.id);
 if(cmd==='refresh_queue'){
  radioCalls.push(JSON.parse(JSON.stringify(args)));await new Promise(r=>setTimeout(r,70));
  const n=Number(args.endpoint.watchEndpoint.videoId.slice(4)),serial=radioCalls.length;
  return {...testQueue,items:[n,n-1,n+1,n+1,...Array.from({length:6},(_,i)=>n+2+i)].map(makeTrack),updateSection:'QUEUE_UPDATE_SECTION_QUEUE',context:{playlistId:'RDtest',queueContextParams:'radio-skip-'+serial},continuation:{continuation:'radio-tail',queueContextParams:'radio-skip-'+serial}};
 }
 return radioBase(cmd,args,...rest);
};document.querySelector('nav button').click();''')
wait("return !!document.querySelector('.hero .primary')")
for _ in range(3):
 if execute("return document.querySelector('.playback .repeat-button').getAttribute('aria-label')==='Повтор выключен'"):break
 click('.playback .repeat-button')
click('.hero .primary')
wait("return document.querySelector('.now-title').textContent==='Track 0'&&!document.querySelector('.main-play').disabled")
if not execute("return !!document.querySelector('.queue-panel')"):click('[aria-label="Очередь воспроизведения"]')
wait("return document.querySelectorAll('.queue-item').length===7")
for n in [1,2]:
 click('.playback [aria-label="Следующий трек"]')
 wait(f"return document.querySelector('.now-title').textContent==='Track {n}'&&!document.querySelector('.main-play').disabled&&radioCalls.length==={n}")
 time.sleep(.15)
 assert execute("const ids=[...document.querySelectorAll('.queue-item strong')].map(e=>e.textContent);return new Set(ids).size===ids.length")
assert execute("return radioPlays.join(',')==='test0000000,test0000001,test0000002'")
assert execute("return radioCalls[0].context.queueContextParams==='radio-start'&&radioCalls[1].context.queueContextParams==='radio-skip-1'&&radioCalls[0].signals.videoInteraction.some(e=>e.videoId==='test0000000'&&e.playbackSkip)")
for n in [3,4,5]:
 invoke('plugin:event|emit',{'event':'track-ended','payload':None})
 wait(f"return document.querySelector('.now-title').textContent==='Track {n}'&&!document.querySelector('.main-play').disabled")
wait("return [...document.querySelectorAll('.queue-item strong')].some(e=>e.textContent==='Track 11')")
assert execute("return radioCalls.length===2"), 'Natural EOF must not act as a skip'
assert execute("const names=[...document.querySelectorAll('.queue-item strong')].map(e=>e.textContent);return names.join(',')===Array.from({length:12},(_,i)=>'Track '+i).join(',')"), 'Continuation must keep new items before an overlapping old tail'
click('.playback [aria-label="Следующий трек"]')
wait("return document.querySelector('.now-title').textContent==='Track 6'&&!document.querySelector('.main-play').disabled&&radioCalls.length===3")
assert execute("return radioCalls[2].context.queueContextParams==='radio-after-tail'&&radioReads.length===2"), 'Skip must retain the latest server context instead of recreating the radio'
click('[aria-label="Закрыть очередь"]')
execute("testQueue=radioSavedQueue;window.__VESPER_TEST_INVOKE__=radioBase;document.querySelector('nav button').click()")
wait("return !!document.querySelector('.hero .primary')")
click('.hero .primary');wait("return !document.querySelector('.main-play').disabled")
print('radio: initial duplicates, current/skipped echoes in inline refresh, successive skips, overlapping continuation, natural EOF and server-context continuity: PASS',flush=True)
