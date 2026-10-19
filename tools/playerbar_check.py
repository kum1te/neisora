"""Real WebKit playerbar geometry and passive MPRIS notification latency."""
import json,os,select,subprocess,time
from pathlib import Path
from webdriver_client import execute,invoke

def wait(script,seconds=8):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.04)
    raise AssertionError(script)
window=subprocess.check_output(['xdotool','search','--onlyvisible','--class','neisora'],text=True,timeout=5).splitlines()[0]
for width in [1920,1494,1024,800,640,480,420]:
    subprocess.run(['xdotool','windowsize',window,str(width),'660'],check=True,timeout=5)
    wait(f'return innerWidth==={width}')
    time.sleep(.1)
    result=execute('''const box=s=>document.querySelector(s).getBoundingClientRect();const play=box('.playback .main-play'),seek=box('.seek'),text=box('.current-track>div'),like=box('.like-button'),options=box('.player-options');return {width:innerWidth,playCenter:play.x+play.width/2,timelineCenter:seek.x+seek.width/2,likeGap:like.x-text.right,optionsRight:options.right,overflow:document.documentElement.scrollWidth>innerWidth};''')
    assert not result['overflow'] and result['optionsRight']<=width+1,result
    assert -1<=result['likeGap']<=18,result
    assert execute("const a=document.querySelector('.like-button').getBoundingClientRect(),b=document.querySelector('.share-button').getBoundingClientRect(),c=document.querySelector('.current-track').getBoundingClientRect();return b.x>=a.right-1&&b.x-a.right<=18&&b.right<=c.right+1&&Math.abs(b.y-a.y)<2")
    if width>760:assert abs(result['playCenter']-result['timelineCenter'])<=1,result
    # Long metadata must shrink without pushing out the like or right controls.
    execute('window.barTitle=document.querySelector(".now-title").textContent;document.querySelector(".now-title").firstChild.nodeValue="Очень длинное название песни ".repeat(12)')
    assert execute('''const a=document.querySelector('.like-button').getBoundingClientRect(),b=document.querySelector('.current-track').getBoundingClientRect();const c=document.querySelector('.share-button').getBoundingClientRect();return a.x>=b.x&&c.right<=b.right+1''')
    execute('document.querySelector(".now-title").firstChild.nodeValue=barTitle')
    if width in [1494,420]:
        Path('artifacts').mkdir(exist_ok=True)
        subprocess.run(['import','-window',window,f'artifacts/playerbar-{width}.png'],check=True,timeout=10)
print('playerbar: Play centered over timeline, like stays beside metadata, long titles/right actions fit at 420–1920 px: PASS',flush=True)
subprocess.run(['xdotool','windowsize',window,'1360','860'],check=True,timeout=5)
wait('return innerWidth===1360')

# The listener only subscribes to signals. Get/Introspect polling of the player
# during the measurement would wake its worker and conceal delayed delivery.
monitor=subprocess.Popen(['stdbuf','-oL','gdbus','monitor','--session','--dest','org.mpris.MediaPlayer2.neisora','--object-path','/org/mpris/MediaPlayer2'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
try:
    time.sleep(.3)
    while select.select([monitor.stdout],[],[],0)[0]:os.read(monitor.stdout.fileno(),65536)
    latencies=[]
    for n in range(5):
        title=f'Passive signal probe {n}'
        started=time.monotonic()
        result=invoke('sync_media',{'snapshot':{'id':'test0000999','title':title,'artist':'Test','position':0,'duration':200,'volume':60,'paused':False,'active':True}})
        assert result['ok'],result
        received=b''
        while time.monotonic()-started<1:
            if select.select([monitor.stdout],[],[],.02)[0]:
                received+=os.read(monitor.stdout.fileno(),65536)
                if title.encode() in received:break
        latency=time.monotonic()-started
        assert title.encode() in received and latency<.5,(latency,received)
        latencies.append(round(latency*1000,1))
        time.sleep(.12)
    print('MPRIS: passive PropertiesChanged signals without polling, latency ms: '+str(latencies)+' PASS',flush=True)
    while select.select([monitor.stdout],[],[],0)[0]:os.read(monitor.stdout.fileno(),65536)
    previous=execute('return document.querySelector(".now-title").textContent')
    started=time.monotonic()
    execute('document.querySelector('+json.dumps('.playback [aria-label="Следующий трек"]')+').click()')
    wait('return document.querySelector(".now-title").textContent!=='+json.dumps(previous))
    title=execute('return document.querySelector(".now-title").textContent')
    received=b''
    while time.monotonic()-started<1:
        if select.select([monitor.stdout],[],[],.02)[0]:
            received+=os.read(monitor.stdout.fileno(),65536)
            if title.encode() in received:break
    elapsed=time.monotonic()-started
    assert title.encode() in received and elapsed<.5,(elapsed,received)
    print(f'MPRIS: real UI Next → passive metadata notification in {elapsed*1000:.1f} ms: PASS',flush=True)
finally:
    monitor.terminate();monitor.wait(timeout=3)
