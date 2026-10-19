"""Real mpv/Pulse routing and hotplug in a disposable server, never host audio."""
import hashlib,json,os,socket,subprocess,tempfile,time
from pathlib import Path
from webdriver_client import create,execute,invoke
from local_fixtures import write_track

assert os.environ['PULSE_SERVER'].startswith('unix:/tmp/neisora-output-')
assert os.environ['XDG_CONFIG_HOME'].startswith('/tmp/neisora-output-')
profile=Path(os.environ['XDG_CONFIG_HOME'])/'local.kumite.vesper'
profile.mkdir(parents=True,exist_ok=True,mode=0o700)
music=profile.parent/'music';write_track(music/'output.wav','Output fixture')
(profile/'local-music.json').write_text(json.dumps({'roots':[{'id':hashlib.sha1(os.fsencode(music)).hexdigest(),'path':str(music),'available':True}],'tracks':[],'revision':0}))
def wait(fn):
 end=time.monotonic()+15
 while time.monotonic()<end:
  v=fn()
  if v:return v
  time.sleep(.1)
 raise AssertionError('Timed out waiting for native audio output state')
def native(cmd,args=None):
 r=invoke(cmd,args);assert r['ok'],r;return r['value']
def pactl(*args):return subprocess.check_output(['pactl',*args],text=True,timeout=5)
def mpv(prop):
 paths=list(profile.glob('mpv-*.sock'));assert len(paths)==1,paths
 with socket.socket(socket.AF_UNIX) as c:
  c.settimeout(3);c.connect(str(paths[0]));c.sendall((json.dumps({'command':['get_property',prop],'request_id':801})+'\n').encode())
  for line in c.makefile():
   result=json.loads(line)
   if result.get('request_id')==801:return result.get('data')
create();wait(lambda:execute('return !!document.querySelector(".search input")'))
execute("[...document.querySelectorAll('nav button')].find(b=>b.textContent==='На устройстве').click()")
wait(lambda:execute("return !!document.querySelector('.local-tracks .track-row')"))
outputs=native('audio_outputs');assert any(d['name']=='pulse/neisora_one' for d in outputs['devices']),outputs
native('set_audio_output',{'name':'pulse/neisora_two'})
execute("document.querySelector('.local-tracks .track-row').click()")
wait(lambda:mpv('current-ao')=='pulse')
def routed(name):
 sinks={s['index']:s['name'] for s in json.loads(pactl('--format=json','list','sinks'))}
 return any(sinks.get(i['sink'])==name for i in json.loads(pactl('--format=json','list','sink-inputs')))
wait(lambda:routed('neisora_two'))
native('control',{'action':'pause'});native('control',{'action':'seek','value':11});native('control',{'action':'volume','value':37})
native('set_audio_output',{'name':'pulse/neisora_one'});wait(lambda:routed('neisora_one'))
assert mpv('pause') is True and abs(mpv('time-pos')-11)<.8 and abs(mpv('volume')-37)<1
# Read persisted preference separately from the mpv property.
assert json.loads((profile/'audio-output.json').read_text())['name']=='pulse/neisora_one'
module=os.environ['NEISORA_OUTPUT_MODULE']
pactl('unload-module',module)
# Observe without audio_outputs IPC, which itself reconciles: prove native hotplug runs.
wait(lambda:mpv('audio-device')=='auto')
assert json.loads((profile/'audio-output.json').read_text())['name']=='pulse/neisora_one'
pactl('load-module','module-null-sink','sink_name=neisora_one')
wait(lambda:mpv('audio-device')=='pulse/neisora_one');wait(lambda:routed('neisora_one'))
assert mpv('pause') is True and abs(mpv('time-pos')-11)<.8
native('set_audio_output',{'name':'auto'})
print('Native outputs: real Pulse routing, paused seek/volume preservation, private preference, unplug fallback and replug restoration PASS',flush=True)
