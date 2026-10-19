"""Behavior checks in a real WebKit window with deterministic music/IPC fixtures.

Run against tauri-driver started with isolated XDG directories and VESPER_MPV_AO=null.
Does not import or modify the user's YouTube session.
"""
import json
import sys
import time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from webdriver_client import request, SESSION, execute, invoke

def wait(script, seconds=8):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = execute(script)
        if value:
            return value
        time.sleep(.05)
    raise AssertionError('Timed out: ' + script)

def check(name, script):
    result = execute(script)
    assert result, name
    print(name, result, flush=True)

if '--create' in sys.argv:
    import os
    binary = Path(os.environ.get('NEISORA_TEST_BINARY', 'src-tauri/target/debug/neisora')).resolve()
    value = request('/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': str(binary)}}}})
    SESSION.write_text(json.dumps(value))
    wait('return !!document.querySelector(".search input")', seconds=20)

execute('''
window.testCalls = [];window.refreshActive=0;window.refreshOverlap=false;window.refreshSerial=0;
localStorage.removeItem("lyrics-offset:test0000002");
window.nativeInvoke = window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
window.makeTrack = n => ({id:'test'+String(n).padStart(7,'0'),title:'Track '+n,subtitle:'Test Artist • Test Album',image:'',kind:'song',duration:'3:20',selected:n===0,endpoint:{watchEndpoint:{videoId:'test'+String(n).padStart(7,'0'),playlistId:'RDtest',index:n}},play:null});
window.testQueue = {title:'Test Supermix',items:Array.from({length:14},(_,n)=>makeTrack(n)),selected:0,dynamic:true,context:{playlistId:'RDtest',queueContextParams:'original-session',params:'wAEB'},continuation:null};
window.testFeed = {title:'Для тебя',authenticated:true,continuation:null,sections:Array.from({length:30},(_,n)=>({title:'Section '+n,subtitle:'',items:Array.from({length:8},(_,i)=>({...makeTrack(n*8+i),kind:i===0?'playlist':'song',endpoint:{watchPlaylistEndpoint:{playlistId:'RDtest'}}}))}))};
window.testWallpaper={id:'wallpaper',name:'Fixture wallpaper',accent:'#d4b4f2',background:'#111018',surface:'#191720',text:'#f3f2f5'};
window.emitState = (extra={}) => nativeInvoke('plugin:event|emit',{event:'player-state',payload:{position:10,duration:200,paused:false,volume:60,buffering:false,active:true,...extra}});
window.__VESPER_TEST_INVOKE__ = async (cmd,args,...rest) => {
  testCalls.push({cmd,args});
  if(cmd==='browse'||cmd==='search') return structuredClone(testFeed);
  if(cmd==='get_appearance') return {...await nativeInvoke(cmd,args,...rest),wallpaper:structuredClone(testWallpaper)};
  if(cmd==='wallpaper_theme') return structuredClone(testWallpaper);
  if(cmd==='queue') return structuredClone(testQueue);
  if(cmd==='get_recording') return null;
  if(cmd==='prefetch'||cmd==='get_word_lyrics'||cmd==='set_visualizer') return null;
  if(cmd==='play') {await new Promise(r=>setTimeout(r,30)); await emitState({position:0}); return null;}
  if(cmd==='refresh_queue') {
    const serial=++refreshSerial;refreshOverlap ||= ++refreshActive>1;
    await new Promise(r=>setTimeout(r,60));
    refreshActive--;
    const id=args.endpoint.watchEndpoint.videoId;
    const n=Number(id.slice(4));
    return {...structuredClone(testQueue),selected:0,items:[makeTrack(n),...Array.from({length:12},(_,i)=>makeTrack(n+1+i))],context:{playlistId:'RDtest',queueContextParams:'continued-session-'+serial}};
  }
  if(cmd==='control') return null;
  if(cmd==='get_lyrics') return {synced:true,source:'Test timestamps',plain:'',lines:Array.from({length:45},(_,n)=>({time:n*4,text:'Test lyric line '+n,words:n===10?[{text:'Test ',time:40,end:41},{text:'lyric ',time:41.5,end:42.5},{text:'line 10',time:42.5,end:44}]:[]}))};
  return nativeInvoke(cmd,args,...rest);
};
document.querySelector('button[aria-label="Обновить подборки"]').click();
''')
wait("return document.querySelectorAll('.music-section').length===30")
execute("document.querySelector('.hero .primary').click()")
wait("return document.querySelector('.now-title').textContent==='Track 0'")
time.sleep(.15)
invoke('plugin:event|emit', {'event':'track-ended','payload':None})
wait("return document.querySelector('.now-title').textContent==='Track 1'")
check('natural end retains queue', "return testCalls.filter(c=>c.cmd==='refresh_queue').length===0")
execute("document.querySelector('.playback button[aria-label=\"Следующий трек\"]').click()")
wait("return testCalls.filter(c=>c.cmd==='refresh_queue').length===1")
time.sleep(.2)
check('manual skip sends session and skipped track', '''const c=testCalls.find(c=>c.cmd==='refresh_queue');return c.args.context.queueContextParams==='original-session' && c.args.signals.videoInteraction.some(v=>v.videoId==='test0000001' && v.playbackSkip) && document.querySelector('.now-title').textContent==='Track 2';''')
# Pump a deterministic wheel clock: test real WebKit scroll geometry without compositor focus.
execute("""
window.savedRAF=requestAnimationFrame;window.savedCancel=cancelAnimationFrame;
window.pendingFrames=new Map();window.frameSerial=0;window.frameCount=0;
window.requestAnimationFrame=f=>{const id=++frameSerial;pendingFrames.set(id,f);return id};
window.cancelAnimationFrame=id=>pendingFrames.delete(id);
window.wheelResults=[];document.querySelector('main').scrollTop=0;
for(const target of [document.querySelector('main'),document.querySelector('.card-grid')]){
 const e=new WheelEvent('wheel',{deltaY:120.75,bubbles:true,cancelable:true});target.dispatchEvent(e);wheelResults.push(e.defaultPrevented);
}
window.wheelPositions=[];let now=performance.now();
for(let i=0;i<90 && pendingFrames.size;i++){const frames=[...pendingFrames.values()];pendingFrames.clear();now+=16.67;for(const f of frames){frameCount++;f(now)}wheelPositions.push(document.querySelector('main').scrollTop)}
""")
check('wheel is smoothed on both the page and shelf', 'return wheelResults.every(Boolean) && wheelPositions.length>5 && Math.abs(document.querySelector("main").scrollTop-241.5)<1 && wheelPositions.every((v,i,a)=>!i||v>=a[i-1])')
check('fractional wheel target stops its animation', 'return pendingFrames.size===0 && frameCount<50')
execute("""document.querySelector('main').dispatchEvent(new WheelEvent('wheel',{deltaY:200,bubbles:true,cancelable:true}));document.querySelector('main').dispatchEvent(new PointerEvent('pointerdown'));window.pointerCancelled=pendingFrames.size===0;window.requestAnimationFrame=savedRAF;window.cancelAnimationFrame=savedCancel;""")
check('scrollbar interaction cancels wheel momentum', 'return pointerCancelled')
if '--wheel' in sys.argv:
    try:
        pos = execute("document.querySelector('main').scrollTop=0;const r=document.querySelector('.card-grid').getBoundingClientRect();return {x:Math.round(r.left+r.width/2),y:Math.round(r.top+40)}")
        before = execute('return document.querySelector("main").scrollTop')
        time.sleep(.1)
        action_path = '/session/' + json.loads(SESSION.read_text())['sessionId'] + '/actions'
        request(action_path, {'actions':[{'type':'pointer','id':'mouse','parameters':{'pointerType':'mouse'},'actions':[{'type':'pointerMove','origin':'viewport',**pos,'duration':100}]}]})
        request(action_path, {'actions':[{'type':'wheel','id':'wheel','actions':[{'type':'scroll','origin':'viewport',**pos,'deltaX':0,'deltaY':400,'duration':300}]}]})
        time.sleep(.4)
        check('real mouse wheel scrolls over shelf', f'return document.querySelector("main").scrollTop>{before}')
    except RuntimeError as error:
        print('Native driver wheel actions unavailable:', str(error)[:100], flush=True)
execute("""const n=document.querySelector('input[aria-label="Позиция воспроизведения"]');n.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true}));n.value='75';n.dispatchEvent(new Event('input',{bubbles:true}));emitState({position:11});""")
time.sleep(.05)
check('seek thumb keeps its drag position during player updates', '''return +document.querySelector('input[aria-label="Позиция воспроизведения"]').value===75''')
execute("document.querySelector('input[aria-label=\"Позиция воспроизведения\"]').dispatchEvent(new Event('change',{bubbles:true}));emitState({position:12})")
time.sleep(.05)
check('seek commits once and ignores stale position', '''return testCalls.filter(c=>c.cmd==="control"&&c.args.action==="seek").length===1 && testCalls.filter(c=>c.cmd==="control"&&c.args.action==="seek")[0].args.value===75 && +document.querySelector('input[aria-label="Позиция воспроизведения"]').value===75''')
execute('emitState({position:75})')
time.sleep(.05)
execute("document.querySelector('.expand-player').click()")
wait("return !!document.querySelector('.expanded-player')")
time.sleep(.3)
check('expanded player hides background work', "return getComputedStyle(document.querySelector('.workspace')).visibility==='hidden' && getComputedStyle(document.querySelector('.expanded-player')).visibility==='visible' && document.querySelectorAll('.lyric-line').length===45")
check('visualizer matches display resolution', "const c=document.querySelector('.audio-visualizer');return c instanceof HTMLCanvasElement && c.clientWidth>0 && Math.abs(c.width-c.clientWidth*devicePixelRatio)<=1 && Math.abs(c.height-c.clientHeight*devicePixelRatio)<=1")
execute("emitState({position:41})")
wait("return document.querySelector('.lyric-active')?.dataset.line==='10'")
execute("document.querySelector('[aria-label=\"Текст позже на полсекунды\"]').click()")
check('lyrics offset saved', "return document.querySelector('.offset-reset').textContent==='+0.5 с' && localStorage.getItem('lyrics-offset:test0000002')==='0.5'")
execute("document.querySelector('[data-line=\"12\"]').click()")
check('lyric click respects offset', "return testCalls.filter(c=>c.cmd==='control').at(-1).args.value===48.5")
execute('emitState({position:48.5})')
time.sleep(.05)
execute("document.querySelector('[aria-label=\"Свернуть плеер\"]').click()")
check('volume percent hidden at rest', "return !document.querySelector('.volume-percent')")
execute('''const n=document.querySelector('input[aria-label="Громкость"]');n.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true}));n.value='37';n.dispatchEvent(new Event('input',{bubbles:true}));''')
time.sleep(.08)
check('volume changes before release with percentage', "return document.querySelector('.volume-percent')?.textContent==='37%' && testCalls.some(c=>c.cmd==='control' && c.args.action==='volume' && c.args.value===37) && !testCalls.some(c=>c.cmd==='control' && c.args.action==='volume_commit')")
execute("window.dispatchEvent(new PointerEvent('pointerup',{bubbles:true}))")
time.sleep(.1)
check('volume commits and hides percentage on release', "return !document.querySelector('.volume-percent') && testCalls.some(c=>c.cmd==='control' && c.args.action==='volume_commit' && c.args.value===37)")
execute("emitState({position:42,paused:true});document.querySelector('.expand-player').click()")
wait("return !!document.querySelector('.lyric-word')")
execute("document.querySelector('.offset-reset').click()")
check('word timing is rendered only on the active line', 'return document.querySelectorAll(".word-line").length===1 && document.querySelectorAll(".lyric-word").length===3')
check('word fill follows source timing', '''const fill=document.querySelectorAll('.word-fill')[1];const clip=getComputedStyle(fill).clipPath;return clip.includes('50%') && document.querySelector('.lyric-active').classList.contains('word-timed');''')
execute("emitState({position:43,paused:true})")
wait("return Math.abs(parseFloat(document.querySelectorAll('.lyric-word')[2].style.getPropertyValue('--word-progress'))-100/3)<.01")
check('word fill advances on the next timestamp', "return document.querySelectorAll('.lyric-word')[1].style.getPropertyValue('--word-progress')==='100%'")
execute("emitState({position:46,paused:true})")
time.sleep(.3)
execute('document.getAnimations().filter(a=>a.effect?.target?.classList.contains("lyric-active")).forEach(a=>a.finish())')
check('active lyric is bright and glowing', "return +getComputedStyle(document.querySelector('.lyric-active')).opacity>.95 && getComputedStyle(document.querySelector('.lyric-active')).textShadow!=='none'")
execute("document.querySelector('[aria-label=\"Свернуть плеер\"]').click();document.querySelector('.sidebar-bottom nav button').click()")
wait("return !!document.querySelector('.settings-page')")
check('account and appearance share the Settings tab', 'return document.querySelectorAll(".settings-section").length===6 && !!document.querySelector(".audio-settings") && !!document.querySelector(".diagnostics-section") && !!document.querySelector(".sleep-settings") && document.querySelectorAll(".theme-preview").length>=6 && !document.querySelector(".account") && !document.querySelector(".appearance-shortcut")')
check('visualizer modes and controls are available', 'return document.querySelectorAll(".visualizer-modes button").length===4 && !!document.querySelector(".visualizer-sliders")')
check('wallpaper control is an accessible custom switch', 'return !!document.querySelector(".toggle-switch[role=switch] span") && !document.querySelector(".wallpaper-toggle input")')
wait("return !document.querySelector('.toggle-switch').disabled")
execute("window.switchBefore=document.querySelector('.toggle-switch').getAttribute('aria-checked');document.querySelector('.toggle-switch').click()")
time.sleep(.2)
check('wallpaper switch changes and animates', "return document.querySelector('.toggle-switch').getAttribute('aria-checked')!==switchBefore && getComputedStyle(document.querySelector('.toggle-switch span')).transitionDuration.includes('0.18')")
execute('''window.oldView=document.querySelector('.page-view');document.querySelector('nav button').click()''')
wait("return !document.querySelector('.settings-page')")
check('navigation replaces and animates the page', 'return document.querySelector(".page-view")!==oldView && getComputedStyle(document.querySelector(".page-view")).animationName==="page-enter"')
check('no visible error toast', "return !document.querySelector('.toast')")
execute("for(let i=0;i<3;i++) document.querySelector('.playback button[aria-label=\"Следующий трек\"]').click()")
wait("return document.querySelector('.now-title').textContent==='Track 5'")
time.sleep(.3)
check('rapid skips keep the latest track and session', "return document.querySelector('.now-title').textContent==='Track 5' && testCalls.filter(c=>c.cmd==='refresh_queue').length===4 && testCalls.filter(c=>c.cmd==='refresh_queue').at(-1).args.context.queueContextParams==='continued-session-3' && !refreshOverlap")
invoke('plugin:event|emit', {'event':'track-ended','payload':None})
wait("return document.querySelector('.now-title').textContent==='Track 6'")
check('natural end after skips does not rebuild queue', "return testCalls.filter(c=>c.cmd==='refresh_queue').length===4")
before = len(execute("return testCalls.filter(c=>c.cmd==='refresh_queue')"))
invoke('plugin:event|emit', {'event':'player-stream-error','payload':{'id':'test0000006','generation':42,'position':30}})
wait("return testCalls.some(c=>c.cmd==='recover_playback')")
check('stream recovery preserves the current queue and position', f"return document.querySelector('.now-title').textContent==='Track 6' && testCalls.filter(c=>c.cmd==='refresh_queue').length==={before} && testCalls.filter(c=>c.cmd==='recover_playback').at(-1).args.position===30")
check('visualizer starts and stops with the expanded player', 'return testCalls.some(c=>c.cmd==="set_visualizer"&&c.args.enabled) && testCalls.some(c=>c.cmd==="set_visualizer"&&!c.args.enabled)')
check('new branding is visible', 'return document.querySelector(".brand-full").textContent==="neisora."')
print('UI regression checks passed', flush=True)
