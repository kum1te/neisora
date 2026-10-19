"""Native folder picker, indexed WAV/ID3/LRC, actual mpv and mixed-radio queue checks.
Requires the isolated GUI fixture session and xdotool inside its X server.
"""
import json, os, shutil, subprocess, time
from pathlib import Path
from webdriver_client import execute, invoke

assert os.environ['XDG_CONFIG_HOME'].startswith('/tmp/neisora-')

def wait(script, seconds=12):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.08)
    raise AssertionError(script)

def native_wait(test,seconds=12):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        value=invoke('local_library',{})
        if value['ok'] and test(value['value']):return value['value']
        time.sleep(.12)
    print(execute('return {body:document.body.innerText.slice(-2000),notice:document.querySelector(".toast")?.textContent}'),flush=True)
    print(subprocess.run(['xdotool','search','--onlyvisible','--name','Папка'],capture_output=True,text=True).stdout,flush=True)
    subprocess.run(['import','-window','root','/tmp/neisora-picker-debug.png'],check=False)
    raise AssertionError(value)

from local_fixtures import write_track

root=Path(os.environ['XDG_CONFIG_HOME']).parent/'music-fixtures'
write_track(root/'Alpha.wav','Local Alpha');write_track(root/'sub/Beta.wav','Local Beta')
Path(os.environ['XDG_CONFIG_HOME']).joinpath('user-dirs.dirs').write_text('XDG_MUSIC_DIR='+json.dumps(str(root))+'\n')
execute("window.localBase=window.__VESPER_TEST_INVOKE__;window.localSavedFeed=structuredClone(testFeed);window.localSavedQueue=structuredClone(testQueue);[...document.querySelectorAll('.sidebar nav button')].find(b=>b.textContent==='На устройстве').click()")
wait("return document.querySelector('h1')?.textContent==='На устройстве'")
execute("document.querySelector('.local-actions .primary').click()")
window=None
for _ in range(100):
    p=subprocess.run(['xdotool','search','--onlyvisible','--name','Папка с музыкой'],capture_output=True,text=True)
    if p.returncode==0:window=p.stdout.splitlines()[-1];break
    time.sleep(.08)
assert window, 'Native folder picker did not open'
subprocess.run(['xdotool','windowfocus','--sync',window],check=True)
time.sleep(.3)
geometry=subprocess.check_output(['xdotool','getwindowgeometry','--shell',window],text=True)
values=dict(line.split('=',1) for line in geometry.splitlines())
subprocess.run(['xdotool','mousemove','--window',window,str(int(values['WIDTH'])-65),str(int(values['HEIGHT'])-23),'click','1'],check=True)
page=native_wait(lambda p:p['total']==2 and not p['status']['scanning'])
assert page['artists']==['Local Artist'] and page['albums']==['Local Album'],page
assert all(i['source']=='local' and i['available'] for i in page['items'])
wait("return document.querySelectorAll('.local-tracks .track-row').length===2")
wait("return document.querySelector('.local-cover img')?.naturalWidth===240")
print('native folder picker, recursive tags, filtered index and lazy 240px covers: PASS',flush=True)
execute("window.localOnlineBefore=testCalls.filter(c=>c.cmd==='play'||c.cmd==='get_lyrics'||c.cmd==='get_word_lyrics'||c.cmd==='get_recording').length;document.querySelector('.local-tracks .track-row').click()")
wait("return document.querySelector('.now-title').textContent==='Local Alpha' && !document.querySelector('.main-play').disabled && document.querySelector('.main-play').getAttribute('aria-label')==='Пауза'")
wait("return document.querySelector('.cover-button img')?.naturalWidth===800")
execute("document.querySelector('.expand-player').click()")
wait("return document.querySelectorAll('.lyric-line').length===3 && document.querySelector('.lyrics-source').textContent.includes('Локальный LRC')")
assert execute("return testCalls.filter(c=>c.cmd==='play'||c.cmd==='get_lyrics'||c.cmd==='get_word_lyrics'||c.cmd==='get_recording').length===localOnlineBefore"), 'Local playback contacted an online provider'
assert invoke('control',{'action':'pause'})['ok']
assert invoke('control',{'action':'seek','value':2})['ok']
wait("return Math.abs(+document.querySelector('input[aria-label=\"Позиция трека\"]').value-2)<.8")
assert invoke('control',{'action':'toggle'})['ok']
for _ in range(60):
    history=invoke('listening_history')
    if history['ok'] and any(e['item'].get('source')=='local' for e in history['value']):break
    time.sleep(.15)
else:raise AssertionError('Local listening did not persist in history')
assert invoke('control',{'action':'stop'})['ok']
execute("document.querySelector('[aria-label=\"Свернуть плеер\"]').click()")
print('actual local mpv playback, seek, 800px cover, offline LRC and local history: PASS',flush=True)
write_track(root/'Gamma.wav','Local Gamma')
native_wait(lambda p:p['total']==3)
wait("return document.querySelectorAll('.local-tracks .track-row').length===3")
(root/'Gamma.wav').unlink();native_wait(lambda p:p['total']==2)
print('filesystem watcher adds and removes music without a manual rescan: PASS',flush=True)
# Queue behavior uses deterministic playback while retaining real local metadata.
execute('''
window.localPlayCalls=0;
window.__VESPER_TEST_INVOKE__=(cmd,args,...rest)=>{
 if(cmd==='play_local'){localPlayCalls++;return emitState({position:0})}
 return localBase(cmd,args,...rest);
};document.querySelector('nav button').click();
''')
wait("return !!document.querySelector('.hero .primary')")
execute("document.querySelector('.hero .primary').click()")
wait("return document.querySelector('.now-title').textContent==='Track 0' && !document.querySelector('.main-play').disabled")
execute("[...document.querySelectorAll('.sidebar nav button')].find(b=>b.textContent==='На устройстве').click()")
wait("return document.querySelectorAll('.local-tracks .track-row').length===2")
execute("document.querySelector('[aria-label=\"Воспроизвести следующим: Local Alpha\"]').click();document.querySelector('[aria-label=\"Добавить в очередь: Local Beta\"]').click();window.localRefreshBefore=testCalls.filter(c=>c.cmd==='refresh_queue').length;document.querySelector('.playback [aria-label=\"Следующий трек\"]').click()")
wait("return document.querySelector('.now-title').textContent==='Local Alpha' && localPlayCalls===1")
assert execute("return testCalls.filter(c=>c.cmd==='refresh_queue').length===localRefreshBefore")
invoke('plugin:event|emit',{'event':'track-ended','payload':None})
wait("return document.querySelector('.now-title').textContent==='Track 1' && testCalls.filter(c=>c.cmd==='refresh_queue').length===localRefreshBefore+1")
assert execute("return testCalls.filter(c=>c.cmd==='refresh_queue').at(-1).args.signals.videoInteraction.every(i=>!i.videoId.startsWith('local:'))")
execute("if(!document.querySelector('.queue-panel'))document.querySelector('[aria-label=\"Очередь воспроизведения\"]').click();const q=document.querySelector('input[aria-label=\"Поиск в очереди\"]');q.value='Local Beta';q.dispatchEvent(new Event('input',{bubbles:true}))")
wait("return document.querySelectorAll('.queue-item').length===1 && document.querySelector('.queue-item strong').textContent==='Local Beta'")
execute("window.localBeforeMove=+document.querySelector('.queue-item').dataset.queueIndex;document.querySelector('[aria-label=\"Перетащить: Local Beta\"]').dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowUp',bubbles:true}))")
wait("return +document.querySelector('.queue-item').dataset.queueIndex===localBeforeMove-1")
execute("document.querySelector('[aria-label=\"Убрать из очереди: Local Beta\"]').click()")
wait("return document.querySelectorAll('.queue-item').length===0")
execute("document.querySelector('[aria-label=\"Очистить поиск в очереди\"]').click();document.querySelector('[aria-label=\"Закрыть очередь\"]').click()")
print('mixed queue defers radio skip, retains manual music, filters, reorders and removes: PASS',flush=True)
execute("document.querySelector('.local-actions .secondary').click()")
wait("return !!document.querySelector('.music-folder')")
execute("document.querySelector('.music-folder button').click()")
for _ in range(100):
    if not invoke('local_status')['value']['folders']:break
    time.sleep(.05)
else:raise AssertionError('Folder was not detached')
assert (root/'Alpha.wav').is_file() and (root/'sub/Beta.wav').is_file()
assert not invoke('play_local',{'id':page['items'][0]['id']})['ok']
execute("window.__VESPER_TEST_INVOKE__=localBase;testFeed=localSavedFeed;testQueue=localSavedQueue;document.querySelector('nav button').click();document.querySelector('.toast button')?.click()")
wait("return !!document.querySelector('.hero .primary')")
execute("document.querySelector('.hero .primary').click()")
wait("return document.querySelector('.now-title').textContent==='Track 0' && !document.querySelector('.main-play').disabled")
shutil.rmtree(root)
print('detaching revokes local playback access and preserves source files: PASS',flush=True)
