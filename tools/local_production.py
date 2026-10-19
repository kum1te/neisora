"""Native local-library smoke in a fresh production profile; no account or fixture IPC."""
import hashlib,json,os,subprocess,time
from pathlib import Path
from webdriver_client import create,execute,invoke
from local_fixtures import write_track
from mpv_probe import wait_position
assert os.environ['NEISORA_SMOKE_CONFIG'].startswith('/tmp/neisora-')
root=Path(os.environ['XDG_CONFIG_HOME']).parent/'production-music'
write_track(root/'Альфа #1 %.wav','Local Alpha');write_track(root/'sub/Beta.wav','Local Beta')
profile=Path(os.environ['XDG_CONFIG_HOME'])/'local.kumite.vesper';profile.mkdir(parents=True,mode=0o700,exist_ok=True)
index=profile/'local-music.json';index.write_text(json.dumps({'roots':[{'id':hashlib.sha1(os.fsencode(root)).hexdigest(),'path':str(root),'available':True}],'tracks':[],'revision':0}));index.chmod(0o600)
create()
def wait(script,seconds=20):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.1)
    raise AssertionError(script)
wait('return !!document.querySelector(".search input")')
assert not execute('return typeof window.__VESPER_TEST_INVOKE__==="function"')
assert invoke('status')['value']['authenticated'] is False
execute("[...document.querySelectorAll('.sidebar nav button')].find(b=>b.textContent==='На устройстве').click()")
wait("return document.querySelectorAll('.local-tracks .track-row').length===2")
execute("document.querySelector('.local-tracks .track-row').click()")
wait("return document.querySelector('.now-title').textContent==='Local Alpha' && !document.querySelector('.main-play').disabled && document.querySelector('.main-play').getAttribute('aria-label')==='Пауза'")
wait("return document.querySelector('.cover-button img')?.naturalWidth===800")
for _ in range(30):
    metadata=subprocess.check_output(['dbus-send','--session','--print-reply','--dest=org.mpris.MediaPlayer2.neisora','/org/mpris/MediaPlayer2','org.freedesktop.DBus.Properties.Get','string:org.mpris.MediaPlayer2.Player','string:Metadata'],text=True,timeout=5)
    if 'file://' in metadata and 'local-art' in metadata:break
    time.sleep(.1)
else:raise AssertionError('MPRIS did not receive the cached local artwork URL')
execute("document.querySelector('.expand-player').click()")
wait("return document.querySelectorAll('.lyric-line').length===3 && document.querySelector('.lyrics-source').textContent.includes('Локальный LRC')")
assert invoke('control',{'action':'pause'})['ok']
assert invoke('control',{'action':'seek','value':2})['ok']
wait("return Math.abs(+document.querySelector('input[aria-label=\"Позиция трека\"]').value-2)<.8")
# Rapid UI commits must converge on the newest target in the real player.
execute('const slider=document.querySelector(".seek input");for(const value of [12,3,15,7,11]){slider.value=value;slider.dispatchEvent(new Event("change",{bubbles:true}))}')
wait_position(11)
print('rapid slider commits reach the newest actual mpv position: PASS',flush=True)
assert invoke('control',{'action':'toggle'})['ok']
for _ in range(80):
    entries=invoke('listening_history')
    if entries['ok'] and any(e['item'].get('source')=='local' for e in entries['value']):break
    time.sleep(.15)
else:raise AssertionError('Local history was not persisted')
subprocess.check_output(['dbus-send','--session','--print-reply','--dest=org.mpris.MediaPlayer2.neisora','/org/mpris/MediaPlayer2','org.mpris.MediaPlayer2.Player.Next'],text=True,timeout=5)
wait("return document.querySelector('.now-title').textContent==='Local Beta' && !document.querySelector('.main-play').disabled")
assert invoke('control',{'action':'pause'})['ok']
assert not execute('return !!document.querySelector(".toast")'),execute('return document.querySelector(".toast")?.textContent')
# Real mpv EOF events must trigger repeat, not only fixture events.
def repeat(label):
    for _ in range(3):
        if execute('return document.querySelector(".expanded-controls .repeat-button").getAttribute("aria-label")') == label: return
        execute('document.querySelector(".expanded-controls .repeat-button").click()')
    raise AssertionError('Repeat mode did not switch')
repeat('Повтор одного трека')
assert invoke('control',{'action':'seek','value':29.4})['ok']
wait_position(29.4)
wait('return +document.querySelector(".seek input").value>28')
assert invoke('control',{'action':'toggle'})['ok']
wait("return document.querySelector('.now-title').textContent==='Local Beta' && +document.querySelector('.seek input').value<3 && document.querySelector('.main-play').getAttribute('aria-label')==='Пауза'")
assert invoke('control',{'action':'pause'})['ok']
repeat('Повтор очереди')
assert invoke('control',{'action':'seek','value':29.4})['ok']
wait_position(29.4)
wait('return +document.querySelector(".seek input").value>28')
assert invoke('control',{'action':'toggle'})['ok']
wait("return document.querySelector('.now-title').textContent==='Local Alpha' && !document.querySelector('.main-play').disabled && document.querySelector('.main-play').getAttribute('aria-label')==='Пауза'")
assert invoke('control',{'action':'pause'})['ok']
repeat('Повтор выключен')
assert not execute('return !!document.querySelector(".toast")'),execute('return document.querySelector(".toast")?.textContent')
print('production: real mpv EOF repeats one track and wraps the finite local playlist: PASS',flush=True)

# A native after-track timer must suppress repeat-one at real EOF, then permit manual Play.
repeat('Повтор одного трека')
assert invoke('set_sleep_timer',{'request':{'mode':'track'}})['ok']
execute('document.querySelector('+json.dumps('.expanded-controls [aria-label="Следующий трек"]')+').click()')
wait("return document.querySelector('.now-title').textContent==='Local Beta'&&!document.querySelector('.main-play').disabled")
assert invoke('get_sleep_timer')['value']['mode']=='track', 'Manual skip must not fire the timer'
execute('document.querySelector('+json.dumps('.expanded-controls [aria-label="Предыдущий трек"]')+').click()')
wait("return document.querySelector('.now-title').textContent==='Local Alpha'&&!document.querySelector('.main-play').disabled")
assert invoke('control',{'action':'pause'})['ok']
assert invoke('control',{'action':'seek','value':29.4})['ok']
wait_position(29.4)
assert invoke('control',{'action':'toggle'})['ok']
for _ in range(100):
    if invoke('get_sleep_timer')['value']['mode']=='off':break
    time.sleep(.05)
else:raise AssertionError('After-track timer did not fire at real EOF')
time.sleep(.5)
assert execute("return document.querySelector('.now-title').textContent==='Local Alpha' && document.querySelector('.main-play').getAttribute('aria-label')==='Воспроизвести'"), 'Sleep timer must override repeat-one'
execute("document.querySelector('.main-play').click()")
wait("return document.querySelector('.main-play').getAttribute('aria-label')==='Пауза' && +document.querySelector('.seek input').value<3")
assert invoke('control',{'action':'pause'})['ok']
repeat('Повтор выключен')
print('production: native after-track sleep overrides repeat and manual Play resumes: PASS',flush=True)

if os.environ.get('NEISORA_EXPECT_APPIMAGE') == '1':
    # A successful playback must come from the image, even on a host with mpv installed.
    profile_bytes = os.fsencode(str(profile))
    matches = []
    for process in Path('/proc').glob('[0-9]*'):
        try:
            executable = (process/'exe').resolve()
            if executable.name == 'mpv' and profile_bytes in (process/'cmdline').read_bytes():
                matches.append(executable)
        except OSError: pass
    assert matches and all('appimage_extracted_' in str(p) or 'squashfs-root' in str(p) for p in matches), matches
    print('AppImage playback uses its bundled mpv: PASS', flush=True)
print('production: cached local library, mpv, cover, seek, LRC, history and MPRIS Next: PASS',flush=True)

if '--online' in __import__('sys').argv:
    execute('const n=document.querySelector(".search input");n.value="BABYMETAL MEGITSUNE";n.dispatchEvent(new Event("input",{bubbles:true}));document.querySelector(".search").dispatchEvent(new Event("submit",{bubbles:true,cancelable:true}));document.querySelector("[aria-label=\\"Свернуть плеер\\"]").click()')
    wait('return !!document.querySelector(".track-actions-row .track-extra button")',60)
    execute('document.querySelector(".track-menu summary").click();document.querySelector(".track-actions-row .track-extra button").click()')
    subprocess.check_output(['dbus-send','--session','--print-reply','--dest=org.mpris.MediaPlayer2.neisora','/org/mpris/MediaPlayer2','org.mpris.MediaPlayer2.Player.Next'],text=True,timeout=5)
    wait('return !document.querySelector(".now-title").textContent.startsWith("Local ") && !document.querySelector(".main-play").disabled && document.querySelector(".main-play").getAttribute("aria-label")==="Пауза"',70)
    execute("[...document.querySelectorAll('.sidebar nav button')].find(b=>b.textContent==='На устройстве').click()")
    wait("return !!document.querySelector('[aria-label=\"Воспроизвести следующим: Local Alpha\"]')")
    execute("document.querySelector('[aria-label=\"Воспроизвести следующим: Local Alpha\"]').click();document.querySelector('.playback [aria-label=\"Следующий трек\"]').click()")
    wait("return document.querySelector('.now-title').textContent==='Local Alpha' && !document.querySelector('.main-play').disabled && document.querySelector('.main-play').getAttribute('aria-label')==='Пауза'")
    assert invoke('control',{'action':'pause'})['ok']
    print('production: actual local → YouTube → local playback in the same mixed queue: PASS',flush=True)

# Write a checkpoint before the shell restarts the complete driver/app group.
if '--online' not in __import__('sys').argv:
    execute('document.querySelector(".seek input").value=11;document.querySelector(".seek input").dispatchEvent(new Event("change",{bubbles:true}));window.dispatchEvent(new Event("pagehide"))')
    wait_position(11)
    for _ in range(100):
        result=invoke('playback_session')
        if result['ok'] and result['value'] and abs(json.loads(result['value'])['position']-11)<.8:break
        time.sleep(.05)
    else:raise AssertionError('Native checkpoint was not committed')
    checkpoint={'title':execute('return document.querySelector(".now-title").textContent'),'position':11}
    (Path(os.environ['XDG_CONFIG_HOME']).parent/'resume-check.json').write_text(json.dumps(checkpoint))
