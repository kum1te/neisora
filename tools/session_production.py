"""Restore a checkpoint after a real process/driver restart, no fixture IPC."""
import json,os,time
from pathlib import Path
from webdriver_client import create,execute,invoke
from mpv_probe import wait_position
profile=Path(os.environ['XDG_CONFIG_HOME'])
assert str(profile).startswith('/tmp/neisora-installed-')
expected=json.loads((profile.parent/'resume-check.json').read_text())
create()
def wait(script):
    end=time.monotonic()+25
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.1)
    raise AssertionError(script)
wait('return document.querySelector(".now-title")?.textContent==='+json.dumps(expected['title']))
assert execute('return document.querySelector(".main-play").getAttribute("aria-label")==="Воспроизвести"')
assert execute('return Math.abs(+document.querySelector(".seek input").value-'+str(expected['position'])+')<.8')
assert not list(profile.rglob('mpv-*.sock')), 'Restoring the display must not launch mpv'
execute('document.querySelector(".main-play").click()')
wait('return document.querySelector(".main-play").getAttribute("aria-label")==="Пауза"&&!document.querySelector(".main-play").disabled')
wait_position(expected['position'],5)
assert invoke('control',{'action':'pause'})['ok']
print('production: full process restart restores queue/position on pause; Play resumes real mpv at the saved position: PASS',flush=True)

if os.environ.get('NEISORA_SMOKE_SLEEP')=='1':
    assert invoke('get_sleep_timer')['value']['mode']=='off', 'Timers must reset on restart'
    # Test the native one-minute deadline in production while real audio loops.
    for _ in range(3):
        if execute('return document.querySelector(".playback .repeat-button").getAttribute("aria-label")')=='Повтор одного трека':break
        execute('document.querySelector(".playback .repeat-button").click()')
    assert invoke('control',{'action':'toggle'})['ok']
    volume=invoke('status')['value']['volume']
    armed=invoke('set_sleep_timer',{'request':{'mode':'minutes','minutes':1}})['value']
    end=time.monotonic()+65
    while time.monotonic()<end:
        if invoke('get_sleep_timer')['value']['mode']=='off':break
        time.sleep(.5)
    else:raise AssertionError('Native minute deadline did not fire')
    assert time.time()*1000>=armed['deadlineMs']
    time.sleep(.5)
    assert execute('return document.querySelector(".main-play").getAttribute("aria-label")==="Воспроизвести"')
    assert invoke('status')['value']['volume']==volume
    time.sleep(1)
    assert execute('return document.querySelector(".main-play").getAttribute("aria-label")==="Воспроизвести"')
    print('production: real one-minute native deadline pauses playback, keeps volume, overrides repeat and stays stopped: PASS',flush=True)
