"""Exercise real mouse queue reordering and the expanded player's lyrics toggle in Xvfb."""
import json
import subprocess
import time
from pathlib import Path
from webdriver_client import execute


def wait(script):
    for _ in range(100):
        if execute(script):
            return
        time.sleep(.05)
    raise AssertionError(script)


def mouse(x, y):
    subprocess.run(['xdotool', 'mousemove', str(round(x)), str(round(y))], check=True)


def coords(selector):
    return execute('const r=document.querySelector(' + json.dumps(selector) +
                   ').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]')


# Keep the native window inside Xvfb's default 1280px screen.
window = subprocess.check_output(['xdotool', 'search', '--onlyvisible', '--name', '^neisora$'], text=True).splitlines()[-1]
geometry = dict(line.split('=', 1) for line in subprocess.check_output(['xdotool', 'getwindowgeometry', '--shell', window], text=True).splitlines())
subprocess.run(['xdotool', 'windowmove', window, '0', '0', 'windowsize', window, '1100', '800'], check=True)
time.sleep(.2)

execute('if(!document.querySelector(".queue-panel"))document.querySelector(\'[aria-label="Очередь воспроизведения"]\').click()')
wait('return document.querySelectorAll(".queue-drag").length>2')
mouse(400, 100)
time.sleep(.2)
assert execute('return [...document.querySelectorAll(".queue-edit-actions")].every(e=>getComputedStyle(e).opacity==="0")')
playing = execute('return document.querySelector(".now-title").textContent')
items = execute('return [...document.querySelectorAll(".queue-drag")].map(e=>({n:+e.dataset.queueDrag,title:e.closest(".queue-edit-row").querySelector("strong").textContent}))')
a, b = items[0], items[2]
execute(f'document.querySelector(".queue-list").scrollTop={a["n"]}*60')
time.sleep(.1)
start = coords(f'[data-queue-drag="{a["n"]}"]')
end = coords(f'[data-queue-index="{b["n"]}"]')
mouse(*start)
time.sleep(.2)
assert execute(f'return getComputedStyle(document.querySelector(\'[data-queue-drag="{a["n"]}"]\').parentElement).opacity==="1"')
subprocess.run(['xdotool', 'mousedown', '1'], check=True)
for i in range(1, 13):
    mouse(start[0] + (end[0] - start[0]) * i / 12, start[1] + (end[1] - start[1]) * i / 12)
    time.sleep(.025)
assert execute('return !!document.querySelector(".drag-target")')
subprocess.run(['xdotool', 'mouseup', '1'], check=True)
wait(f'return document.querySelector(\'[data-queue-index="{b["n"]}"] strong\').textContent==={json.dumps(a["title"])}')
assert execute('return document.querySelector(".now-title").textContent') == playing
mouse(400, 100)
time.sleep(.25)
assert execute('return !document.querySelector(".drag-source") && [...document.querySelectorAll(".queue-edit-actions")].every(e=>getComputedStyle(e).opacity==="0")')
print('real mouse drag changes queue order; controls hidden off-hover: PASS', flush=True)

execute('document.querySelector(\'[aria-label="Закрыть очередь"]\').click();document.querySelector(".expand-player").click()')
wait('return !!document.querySelector(".expanded-right")')
execute('document.querySelector(".lyrics-toggle").click()')
wait('return !document.querySelector(".expanded-right")')
wait('const r=document.querySelector(".expanded-left").getBoundingClientRect();return Math.abs(r.x+r.width/2-innerWidth/2)<2 && document.querySelector(".expanded-left").getAnimations().length===0')
Path('artifacts').mkdir(exist_ok=True)
subprocess.run(['import', '-window', 'root', 'artifacts/player-lyrics-hidden.png'], check=True)
execute('document.querySelector(\'[aria-label="Свернуть плеер"]\').click();document.querySelector(".expand-player").click()')
wait('return !!document.querySelector(".expanded-player.lyrics-hidden")')
execute('document.querySelector(".lyrics-toggle").click()')
wait('return !!document.querySelector(".expanded-right")')
# Interrupt/reverse an in-progress layout animation, then verify the final center.
execute('for(let i=0;i<5;i++)document.querySelector(".lyrics-toggle").click()')
wait('return !document.querySelector(".expanded-right")')
wait('const e=document.querySelector(".expanded-left"), r=e.getBoundingClientRect();return Math.abs(r.x+r.width/2-innerWidth/2)<2 && e.getAnimations().length===0')
execute('document.querySelector(".lyrics-toggle").click()')
wait('return !!document.querySelector(".expanded-right")')
execute('document.querySelector(\'[aria-label="Свернуть плеер"]\').click()')
print('lyrics toggle hides panel, centers player and remembers selection: PASS', flush=True)

subprocess.run(['xdotool', 'windowsize', window, geometry['WIDTH'], geometry['HEIGHT']], check=True)
