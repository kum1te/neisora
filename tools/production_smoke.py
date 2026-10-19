"""Real public playback in a fresh, unauthenticated production installation.

Uses an existing isolated WebDriver session, or --create with NEISORA_TEST_BINARY.
Never run against a working account; network access to YouTube is required.
"""
import sys
import time
import os
import signal
from pathlib import Path
import hashlib
import json
import subprocess
from webdriver_client import create, execute, invoke
from mpv_probe import wait_position

def wait(script, seconds=60):
    end = time.monotonic()+seconds
    while time.monotonic() < end:
        if execute(script): return
        time.sleep(.1)
    raise AssertionError(script)

def control(action, value=None):
    result = invoke('control', {'action': action, 'value': value})
    assert result['ok'], result

if '--create' in sys.argv:
    create()
wait('return !!document.querySelector(".search input")', 20)
assert not execute('return typeof window.__VESPER_TEST_INVOKE__ === "function"')
status = invoke('status')
assert status['ok'] and not status['value']['authenticated'], 'Requires an unauthenticated test profile'
assert status['value']['mpv'] and status['value']['extractor'], status
print('production installation resolves all playback helpers: PASS', flush=True)
if '--offline' in sys.argv:
    manifest=json.loads(Path('tools/runtime-manifest.json').read_text())
    runtime=Path('/usr/lib/neisora/runtime')
    assert (runtime.parent/'licenses/NEISORA-LICENSE.txt').read_text()==Path('LICENSE').read_text()
    assert subprocess.check_output([str(runtime/'node'),'--version'],text=True).strip()=='v'+manifest['node']['version']
    assert hashlib.sha256((runtime/'yt-dlp').read_bytes()).hexdigest()==manifest['yt-dlp']['sha256']
    assert subprocess.check_output([str(runtime/'yt-dlp'),'--version'],text=True).strip()==manifest['yt-dlp']['version']
    control('volume_commit',37)
    assert invoke('status')['value']['volume']==37
    execute('document.querySelector(".sidebar-bottom nav button").click()')
    wait('return !!document.querySelector(".settings-page")',8)
    print('installed helpers match the pinned versions; Settings and persistent IPC work offline: PASS',flush=True)
    execute('window.__TAURI_INTERNALS__.invoke("plugin:window|close",{label:"main"}).catch(()=>{})')
    sys.exit(0)
execute('const n=document.querySelector(".search input");n.value="BABYMETAL MEGITSUNE";n.dispatchEvent(new Event("input",{bubbles:true}));document.querySelector(".search").dispatchEvent(new Event("submit",{bubbles:true,cancelable:true}))')
wait('return !!document.querySelector(".track-row")')
execute('document.querySelector(".track-row").click()')
wait('return !document.querySelector(".main-play").disabled && document.querySelector(".main-play").getAttribute("aria-label")==="Пауза" && document.querySelector(".seek>span:last-child").textContent!=="0:00"')
print('public search and real mpv playback: PASS', flush=True)
if '--media' in sys.argv:
    def media(method):
        subprocess.check_output(['gdbus','call','--session','--dest','org.mpris.MediaPlayer2.neisora','--object-path','/org/mpris/MediaPlayer2','--method','org.mpris.MediaPlayer2.Player.'+method],text=True,timeout=5)
    # Native bus calls must reach actual mpv, without a fixture or keyboard focus.
    time.sleep(.5)
    media('Pause')
    wait('return document.querySelector(".main-play").getAttribute("aria-label")==="Воспроизвести"',8)
    media('Play')
    wait('return document.querySelector(".main-play").getAttribute("aria-label")==="Пауза"',8)
    for _ in range(24):
        result=invoke('listening_history')
        if result['ok'] and result['value']:break
        time.sleep(.5)
    else:raise AssertionError('Audible production playback was not recorded in local history')
    assert result['value'][0]['item']['title']==execute('return document.querySelector(".now-title").textContent')
    print('production MPRIS controls actual mpv; successful listening persists in local history: PASS',flush=True)
control('pause')
control('seek', 60)
wait('const [m,s]=document.querySelector(".seek>span").textContent.split(":").map(Number);return Math.abs(m*60+s-60)<=1', 8)
# Exercise the real seek handler: concurrent raw IPC calls have no JavaScript
# submission-order guarantee and do not verify what the user does with the slider.
execute('const slider=document.querySelector(".seek input");for(const value of [12,30,50,70,90]){slider.value=value;slider.dispatchEvent(new Event("change",{bubbles:true}))}')
wait('const [m,s]=document.querySelector(".seek>span").textContent.split(":").map(Number);return Math.abs(m*60+s-90)<=1', 8)
wait_position(90)
print('paused seek and rapid seeks settle on the last position: PASS', flush=True)
control('volume', 42)
control('volume_commit', 42)
assert invoke('status')['value']['volume'] == 42
print('volume changes and persists without a restart: PASS', flush=True)
execute('document.querySelector(".expand-player").click()')
wait('return !!document.querySelector(".expanded-player")', 8)
assert execute("const c=document.querySelector('.audio-visualizer');return c instanceof HTMLCanvasElement && c.clientWidth>0 && Math.abs(c.width-c.clientWidth*devicePixelRatio)<=1 && Math.abs(c.height-c.clientHeight*devicePixelRatio)<=1")
execute('document.querySelector("[aria-label=\\"Свернуть плеер\\"]").click()')
execute('window.smokeOriginalTitle=document.querySelector(".now-title").textContent')
control('toggle')
if '--media' in sys.argv: media('Next')
else: execute('document.querySelector(".playback [aria-label=\\"Следующий трек\\"]").click()')
wait('return !document.querySelector(".main-play").disabled && document.querySelector(".now-title").textContent!==smokeOriginalTitle')
print('expanded player and real queue skip: PASS', flush=True)
control('pause')
assert not execute('return !!document.querySelector(".toast")'), execute('return document.querySelector(".toast")?.textContent')
print('production smoke checks passed', flush=True)
def isolated_playback_processes():
    profile = os.environ['NEISORA_SMOKE_CONFIG']
    assert profile.startswith('/tmp/neisora-'), 'Dedicated temporary profile required'
    candidates = []
    for proc in Path('/proc').iterdir():
        try:
            if not proc.name.isdigit() or proc.joinpath('comm').read_text().strip() != 'neisora': continue
            env = proc.joinpath('environ').read_bytes().split(b'\0')
            if f'XDG_CONFIG_HOME={profile}'.encode() in env and b'VESPER_MPV_AO=null' in env:
                candidates.append(int(proc.name))
        except OSError: pass
    assert len(candidates) == 1, 'Could not identify the isolated app unambiguously'
    app_pid = candidates[0]; children=[]
    for proc in Path('/proc').iterdir():
        try:
            if proc.name.isdigit() and proc.joinpath('comm').read_text().strip() == 'mpv' and int(proc.joinpath('stat').read_text().rsplit(')',1)[1].split()[1]) == app_pid:
                children.append(int(proc.name))
        except OSError: pass
    assert children, 'No test mpv child'
    return app_pid, children

if '--mpv-restart' in sys.argv:
    app_pid, old_children = isolated_playback_processes()
    for pid in old_children:
        os.kill(pid, signal.SIGKILL)
    wait('return document.querySelector(".main-play").getAttribute("aria-label")==="Воспроизвести"',8)
    execute('document.querySelector(".toast button")?.click();document.querySelector(".main-play").click()')
    wait('return !document.querySelector(".main-play").disabled && document.querySelector(".main-play").getAttribute("aria-label")==="Пауза"')
    new_app, new_children = isolated_playback_processes()
    assert new_app == app_pid and not set(new_children).intersection(old_children)
    assert invoke('status')['value']['volume'] == 42
    control('pause')
    print('mpv crash can restart playback with preserved volume: PASS',flush=True)
if '--crash-cleanup' in sys.argv:
    app_pid, children = isolated_playback_processes()
    os.kill(app_pid, signal.SIGKILL)
    for _ in range(100):
        alive=[]
        for pid in children:
            try:
                if not Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].lstrip().startswith('Z'): alive.append(pid)
            except FileNotFoundError: pass
        if not alive: break
        time.sleep(.1)
    assert not alive, f'mpv survived application death: {alive}'
    print('SIGKILL of the isolated app stops its mpv child: PASS', flush=True)
