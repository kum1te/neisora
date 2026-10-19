"""Race regressions, after regression.py, in the isolated fixture window."""
import time
from webdriver_client import execute, invoke


def wait(script):
    end = time.monotonic() + 8
    while time.monotonic() < end:
        if execute(script): return
        time.sleep(.05)
    raise AssertionError(script)

assert execute('return typeof window.__VESPER_TEST_INVOKE__ === "function"')
execute('''
window.auditBase=window.__VESPER_TEST_INVOKE__;window.auditAuthenticated=true;window.auditDelayQueue=false;
window.__VESPER_TEST_INVOKE__=(cmd,args,...rest)=>{
 if(cmd==='status')return Promise.resolve({authenticated:auditAuthenticated,browser:[],mpv:true,extractor:true,volume:60});
 if(cmd==='like')return new Promise(resolve=>window.auditResolveLike=resolve);
 if(cmd==='logout'){auditAuthenticated=false;return Promise.resolve(null)}
 if(cmd==='queue'&&auditDelayQueue)return new Promise(resolve=>window.auditResolveQueue=resolve);
 return auditBase(cmd,args,...rest);
};
''')
invoke('plugin:event|emit', {'event':'auth-updated','payload':None})
time.sleep(.15)
execute('document.querySelector(".like-button").click()')
wait('return typeof auditResolveLike==="function"')
execute('window.auditExpectedNext="Track "+(Number(document.querySelector(".now-title").textContent.slice(6))+1);document.querySelector(".playback [aria-label=\\"Следующий трек\\"]").click()')
wait('return document.querySelector(".now-title").textContent===auditExpectedNext')
execute('auditResolveLike(null)');time.sleep(.1)
assert execute('return !document.querySelector(".like-button").classList.contains("liked")')
print('late like response does not mark the next track: PASS',flush=True)
time.sleep(.2)
execute('auditDelayQueue=true;window.auditPlaysBefore=testCalls.filter(c=>c.cmd==="play").length;document.querySelector(".hero .primary").click()')
wait('return typeof auditResolveQueue==="function"')
execute('document.querySelector(".sidebar-bottom nav button").click()')
wait('return !!document.querySelector(".disconnect-button")')
execute('document.querySelector(".disconnect-button").click()');time.sleep(.1)
execute('auditResolveQueue(structuredClone(testQueue))');time.sleep(.2)
assert execute('return !document.querySelector(".now-title").textContent.startsWith("Track ") && testCalls.filter(c=>c.cmd==="play").length===auditPlaysBefore')
print('late queue response cannot restart playback after logout: PASS',flush=True)

# Delay playback deliberately: the title changes before the play request settles.
execute('window.__VESPER_TEST_INVOKE__=async(cmd,args,...rest)=>{if(cmd==="play")await new Promise(r=>setTimeout(r,200));return auditBase(cmd,args,...rest)};document.querySelector("nav button").click()')
wait('return !!document.querySelector(".hero .primary")')
execute('document.querySelector(".hero .primary").click()')
wait('return document.querySelector(".now-title").textContent==="Track 0" && !document.querySelector(".main-play").disabled && !document.querySelector(".hero .primary").disabled')

# A pending continuation belongs to the queue and playback that requested it.
# Finishing it must not skip the first track in a playlist opened meanwhile.
execute('''
window.auditSavedQueue=structuredClone(testQueue);
window.__VESPER_TEST_INVOKE__=(cmd,args,...rest)=>{
 if(cmd==='queue'&&args.endpoint?.continuation==='audit-tail')return new Promise(resolve=>window.auditResolveTail=resolve);
 return auditBase(cmd,args,...rest);
};
testQueue={...structuredClone(auditSavedQueue),items:[makeTrack(10)],selected:0,dynamic:false,context:{playlistId:'old-playlist'},continuation:{continuation:'audit-tail'}};
document.querySelector('.hero .primary').click();
''')
wait('return document.querySelector(".now-title").textContent==="Track 10" && !document.querySelector(".main-play").disabled')
# Playback prefetch may have started the continuation already.
execute('document.querySelector(".playback [aria-label=\\"Следующий трек\\"]").click()')
wait('return typeof auditResolveTail==="function"')
execute('''testQueue={...structuredClone(auditSavedQueue),items:[makeTrack(100),makeTrack(101)],selected:0,dynamic:false,context:{playlistId:'new-playlist'},continuation:null};document.querySelector('.hero .primary').click();''')
wait('return document.querySelector(".now-title").textContent==="Track 100" && !document.querySelector(".main-play").disabled')
execute("auditResolveTail({...structuredClone(auditSavedQueue),items:[makeTrack(11)],context:{playlistId:'old-playlist'},continuation:null})")
time.sleep(.25)
assert execute('return document.querySelector(".now-title").textContent==="Track 100"'), 'Old Next advanced a newly opened playlist'
print('late Next continuation cannot advance a different playlist: PASS',flush=True)
execute('window.__VESPER_TEST_INVOKE__=auditBase;testQueue=auditSavedQueue;document.querySelector(".hero .primary").click()')
wait('return document.querySelector(".now-title").textContent==="Track 0" && !document.querySelector(".main-play").disabled && !document.querySelector(".hero .primary").disabled')
