"""Terminal fixture check: reload UI and hydrate radio from fresh server context."""
import json,time,sys
from webdriver_client import execute,invoke,request,sid

def click(selector):execute('document.querySelector('+json.dumps(selector)+').click()')

def wait(script):
    end=time.monotonic()+15
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.1)
    raise AssertionError(script+'\n'+str(execute('return document.body.innerText.slice(-1200)')))
# Stop only the mpv instance in the disposable test profile.
wait('return !!document.querySelector(".search input")')
invoke('control',{'action':'stop'})
track={'id':'test0000400','title':'Restored radio song','subtitle':'Fixture','image':'','kind':'song','duration':'3:20','selected':False,'endpoint':{'watchEndpoint':{'videoId':'test0000400'}},'play':None}
manual={**track,'id':'test0000401','title':'Manual addition','manual':True}
saved={'version':1,'queue':{'title':'Saved supermix '+str(time.monotonic_ns()),'items':[track,manual],'selected':0,'dynamic':True,'context':{},'continuation':None},'position':58.5,'duration':200,'radioId':'RDfixture'}
# Populate the real UI first: pagehide must persist the active queue, rather
# than replacing a hand-written storage fixture with the old empty UI state.
execute('window.restoreInitial='+json.dumps(saved)+''';const native=window.__TAURI_INTERNALS__.invoke;
window.__VESPER_TEST_INVOKE__=async(cmd,args,...rest)=>{
 if(cmd==='browse')return {title:'Restore fixture',authenticated:false,continuation:null,sections:[{title:'Songs',subtitle:'',items:[restoreInitial.queue.items[0]]}]};
 if(cmd==='queue')return {...restoreInitial.queue,context:{playlistId:restoreInitial.radioId}};
 if(cmd==='play'){await native('plugin:event|emit',{event:'player-state',payload:{position:58.5,duration:200,paused:true,volume:60,buffering:false,active:true}});return null;}
 if(cmd==='prefetch'||cmd==='get_recording')return null;
 return native(cmd,args,...rest);
};document.querySelector('[aria-label="Обновить подборки"]').click();''')
wait('return !!document.querySelector(".hero .primary")')
execute('document.querySelector(".hero .primary").click()')
wait('return document.querySelector(".now-title")?.textContent==="Restored radio song"&&!document.querySelector(".main-play").disabled')
execute('window.dispatchEvent(new Event("pagehide"))')
for _ in range(100):
    result=invoke('playback_session')
    if result['ok'] and result['value'] and json.loads(result['value'])['queue']['title']==saved['queue']['title'] and abs(json.loads(result['value'])['position']-58.5)<.1:break
    time.sleep(.05)
else:raise AssertionError('Native checkpoint was not committed')
execute('location.reload()')
wait('return document.querySelector(".now-title")?.textContent==="Restored radio song"')
# mpv may report pause=false after stop even though it is idle. A late idle
# event must not turn a restored paused checkpoint into a playing UI.
assert invoke('plugin:event|emit',{'event':'player-state','payload':{'position':0,'duration':0,'paused':False,'volume':60,'buffering':False,'active':False}})['ok']
wait('return document.querySelector(".main-play").getAttribute("aria-label")==="Воспроизвести"&&Math.abs(+document.querySelector(".seek input").value-58.5)<.2')
assert execute('return document.querySelector(".playback .shuffle-button").disabled')
execute('''window.restoreCalls=[];const native=window.__TAURI_INTERNALS__.invoke;
window.__VESPER_TEST_INVOKE__=async(cmd,args,...rest)=>{
 restoreCalls.push({cmd,args});
 if(cmd==='queue')return {title:'Fresh supermix',items:[{id:'test0000400',title:'Restored radio song',subtitle:'Fixture',image:'',kind:'song',duration:'3:20',selected:false,endpoint:{watchEndpoint:{videoId:'test0000400',index:7}},play:null},{id:'test0000402',title:'Fresh recommendation',subtitle:'Fixture',image:'',kind:'song',duration:'3:20',selected:false,endpoint:{watchEndpoint:{videoId:'test0000402',index:8}},play:null}],selected:0,dynamic:true,continuation:null,context:{playlistId:'RDfixture',queueContextParams:'fresh-session'}};
 if(cmd==='play'){await native('plugin:event|emit',{event:'player-state',payload:{position:args.position,duration:200,paused:false,volume:60,buffering:false,active:true}});return null;}
 if(cmd==='prefetch'||cmd==='get_recording'||cmd==='track_like')return null;
 return native(cmd,args,...rest);
};''')
if '--queue' in sys.argv:
    click('[aria-label="Очередь воспроизведения"]')
    wait("return !!document.querySelector('.queue-item[aria-current=true]')")
    execute("document.querySelector('.queue-item[aria-current=true]').click()")
else:
    execute("document.querySelector('.main-play').click()")
wait("return restoreCalls.some(c=>c.cmd==='play')&&!document.querySelector('.main-play').disabled")
calls=execute('return restoreCalls')
queue_call=next(c for c in calls if c['cmd']=='queue')
assert queue_call['args']['endpoint']['watchEndpoint']=={'videoId':'test0000400','playlistId':'RDfixture'}
assert next(c for c in calls if c['cmd']=='play')['args']['position']==58.5
assert not any(c['cmd']=='refresh_queue' for c in calls), 'Resume is not a fake skip'
if '--queue' not in sys.argv:
    click('[aria-label="Очередь воспроизведения"]')
wait("return document.querySelectorAll('.queue-item').length===3")
assert execute("return [...document.querySelectorAll('.queue-item strong')].map(e=>e.textContent).join('|')==='Restored radio song|Manual addition|Fresh recommendation'")
click('.playback [aria-label="Следующий трек"]')
wait("return document.querySelector('.now-title').textContent==='Manual addition'")
print('session UI: paused restore, fresh radio queue/session on Play, preserved manual additions and resume position; no fake skip: PASS ('+('queue selection' if '--queue' in sys.argv else 'Play')+')',flush=True)
