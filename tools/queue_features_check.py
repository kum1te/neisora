"""Queue editing and shuffle in real WebKit, synthetic provider fixtures only."""
import json,time
from webdriver_client import execute

def wait(script):
    until=time.monotonic()+12
    while time.monotonic()<until:
        if execute(script):return
        time.sleep(.05)
    raise AssertionError(script+'\n'+str(execute('return document.body.innerText.slice(-1200)')))
def click(selector):execute('document.querySelector('+json.dumps(selector)+').click()')
execute("window.queueFeatureSaved=structuredClone(testQueue);testQueue={title:'Shuffle fixture',items:[0,1,2,3,4,5].map(makeTrack),selected:0,dynamic:false,context:{playlistId:'PLfixture'},continuation:null};document.querySelector('nav button').click()")
wait("return !!document.querySelector('.hero .primary')")
click('.hero .primary')
wait("return document.querySelector('.now-title').textContent==='Track 0'&&!document.querySelector('.main-play').disabled")
assert execute("return !document.querySelector('.shuffle-button').disabled")
click('.playback .shuffle-button')
wait("return document.querySelector('.playback .shuffle-button').getAttribute('aria-pressed')==='true'")
click('[aria-label="Очередь воспроизведения"]')
wait("return !!document.querySelector('.queue-panel')")
assert execute("return document.querySelector('[aria-current=true]').textContent.includes('Track 0')")
click('.playback .shuffle-button')
wait("return document.querySelector('.playback .shuffle-button').getAttribute('aria-pressed')==='false'")
assert execute("return [...document.querySelectorAll('.queue-item strong')].map(e=>e.textContent).join(',')==='Track 0,Track 1,Track 2,Track 3,Track 4,Track 5'")
click('[aria-label="Убрать из очереди: Track 1"]')
wait("return !document.querySelector('[aria-label=\"Убрать из очереди: Track 1\"]')&&!!document.querySelector('.queue-undo')")
click('.queue-undo')
wait("return !!document.querySelector('[aria-label=\"Убрать из очереди: Track 1\"]')")
click('.queue-tools-menu summary')
execute("[...document.querySelectorAll('.queue-tools-menu button')].find(e=>e.textContent.includes('Выбрать')).click()")
wait("return document.querySelectorAll('.queue-select').length===5")
click('[aria-label="Выбрать: Track 1"]');click('[aria-label="Выбрать: Track 3"]')
click('[aria-label="Удалить выбранные треки"]')
wait("return document.querySelectorAll('.queue-item').length===4&&!!document.querySelector('.queue-undo')")
assert execute("return [...document.querySelectorAll('.queue-item strong')].map(e=>e.textContent).join(',')==='Track 0,Track 2,Track 4,Track 5'")
click('.queue-undo')
wait("return document.querySelectorAll('.queue-item').length===6")
click('.queue-tools-menu summary')
execute("[...document.querySelectorAll('.queue-tools-menu button')].find(e=>e.textContent.includes('Очистить следующие')).click()")
wait("return document.querySelectorAll('.queue-item').length===1")
assert execute("return document.querySelector('.now-title').textContent==='Track 0'")
click('.queue-undo')
wait("return document.querySelectorAll('.queue-item').length===6")
click('[aria-label="Убрать из очереди: Track 1"]')
click('.playback [aria-label="Следующий трек"]')
wait("return document.querySelector('.now-title').textContent==='Track 2'&&!document.querySelector('.main-play').disabled")
assert execute("return !document.querySelector('.queue-undo')"), 'Undo must never rewind playback after a track change'
execute("window.dispatchEvent(new Event('pagehide'))")
saved=execute("return JSON.parse(localStorage.getItem('playback:session:v1'))")
assert saved['queue']['items'][saved['queue']['selected']]['title']=='Track 2'
assert 'queueContextParams' not in json.dumps(saved)
print('queue features: shuffle/unshuffle, current protection, bulk removal, clear/undo, playback invalidates undo and bounded session write: PASS',flush=True)
click('[aria-label="Закрыть очередь"]')
execute("testQueue=queueFeatureSaved;document.querySelector('nav button').click()")
wait("return !!document.querySelector('.hero .primary')")
click('.hero .primary')
wait("return !document.querySelector('.main-play').disabled")
assert execute("return document.querySelector('.playback .shuffle-button').disabled"), 'Live radio must not be locally shuffled'
