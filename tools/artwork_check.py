"""Artwork lookup must be asynchronous, coalesced and fall back on image failure."""
import time
from webdriver_client import execute


def wait(script):
    for _ in range(100):
        if execute(script): return
        time.sleep(.05)
    raise AssertionError(script)

execute('''
window.artworkBase=window.__VESPER_TEST_INVOKE__;
window.artworkQueue=structuredClone(testQueue);window.artworkCalls=0;window.artworkMode='delay';
window.artworkPixel='data:image/svg+xml,'+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>');
window.__VESPER_TEST_INVOKE__=(cmd,args,...rest)=>{
 if(cmd==='get_recording'){
  artworkCalls++;
  if(artworkMode==='success')return Promise.resolve({image:artworkPixel});
  return new Promise(resolve=>window.artworkResolve=resolve);
 }
 return artworkBase(cmd,args,...rest);
};
testQueue={...structuredClone(testQueue),items:testQueue.items.map(item=>({...item,image:'https://i.ytimg.com/vi/'+item.id+'/default.jpg'}))};
document.querySelector('.hero .primary').click();
''')
try:
    wait('return typeof artworkResolve==="function" && !document.querySelector(".main-play").disabled')
    execute('for(let i=0;i<12;i++)emitState({position:i})')
    time.sleep(.15)
    assert execute('return artworkCalls===1'), 'Player clock retriggers pending artwork lookup'
    execute("artworkResolve({image:'data:image/png;base64,invalid'})")
    time.sleep(.2)
    assert execute('return document.querySelector(".cover-button img").src.includes("ytimg.com")')
    print('Failed external image preserves the original without delaying playback: PASS',flush=True)
    execute("artworkMode='success';document.querySelector('.hero .primary').click()")
    wait('return document.querySelector(".cover-button img")?.src===artworkPixel')
    execute("document.querySelector('.expand-player').click()")
    wait('return document.querySelector(".expanded-cover img")?.src===artworkPixel')
    execute("document.querySelector('.expanded-header [aria-label=\"Свернуть плеер\"]').click()")
    print('Matching artwork is shared by the player bar and expanded player: PASS',flush=True)
finally:
    execute('window.__VESPER_TEST_INVOKE__=artworkBase;testQueue=artworkQueue;document.querySelector(".hero .primary").click()')
    wait('return !document.querySelector(".main-play").disabled')
