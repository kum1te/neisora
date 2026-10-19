"""10,000-track WebKit fixture regression, after regression.py/audit_regression.py."""
import time
from webdriver_client import execute

def wait(script):
    for _ in range(200):
        if execute(script): return
        time.sleep(.05)
    raise AssertionError(script)

assert execute('return typeof window.__VESPER_TEST_INVOKE__ === "function"')
execute('window.smallQueue=structuredClone(testQueue);testQueue={...testQueue,items:Array.from({length:10000},(_,n)=>makeTrack(n))};document.querySelector(".hero .primary").click()')
wait('return document.querySelector(".now-title").textContent==="Track 0"')
execute('if(!document.querySelector(".queue-panel"))document.querySelector("[aria-label=\\"Очередь воспроизведения\\"]").click()')
wait('return document.querySelector(".queue-rows")?.style.height==="600000px"')
assert execute('return document.querySelectorAll(".queue-item").length<50')
execute('const n=document.querySelector(".queue-list");n.scrollTop=9000*60;n.dispatchEvent(new Event("scroll"))')
wait('return !!document.querySelector("[data-queue-index=\\"9000\\"]")')
assert execute('return document.querySelectorAll(".queue-item").length<50')
print('10,000 tracks render fewer than 50 rows after deep scrolling: PASS', flush=True)
execute('const n=document.querySelector("[data-queue-index=\\"9000\\"]");n.focus();n.dispatchEvent(new KeyboardEvent("keydown",{key:"End",bubbles:true}))')
wait('return document.activeElement?.dataset.queueIndex==="9999"')
execute('document.activeElement.dispatchEvent(new KeyboardEvent("keydown",{key:"Home",bubbles:true}))')
wait('return document.activeElement?.dataset.queueIndex==="0"')
print('keyboard navigation reaches both ends of the virtual queue: PASS', flush=True)
execute('const n=document.querySelector(".queue-list");n.scrollTop=9000*60;n.dispatchEvent(new Event("scroll"))')
wait('return !!document.querySelector("[data-queue-index=\\"9000\\"]")')
execute('document.querySelector("[data-queue-index=\\"9000\\"]").click()')
wait('return document.querySelector(".now-title").textContent==="Track 9000"')
wait('return document.querySelector(".queue-item.playing")?.dataset.queueIndex==="40"')
execute('document.querySelector(".playback [aria-label=\\"Следующий трек\\"]").click()')
wait('return testCalls.some(c=>c.cmd==="refresh_queue"&&c.args.signals.videoInteraction.some(s=>s.videoId==="test0009000"&&s.playbackSkip&&s.queueIndex===9000))')
print('pruned queue still reports the original skipped server index: PASS', flush=True)
wait('return !document.querySelector(".hero .primary").disabled')
execute('testQueue=smallQueue;document.querySelector(".hero .primary").click()')
wait('return document.querySelector(".now-title").textContent==="Track 0" && !document.querySelector(".main-play").disabled')
execute('document.querySelector("[aria-label=\\"Закрыть очередь\\"]").click()')
