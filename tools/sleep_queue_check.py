"""Sleep IPC/UI and queue export regressions; synthetic provider writes only."""
import json,time,subprocess
from pathlib import Path
from webdriver_client import execute,invoke

def wait(script):
    end=time.monotonic()+12
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.05)
    raise AssertionError(script+'\n'+str(execute('return document.body.innerText.slice(-1600)')))
def click(selector):execute('document.querySelector('+json.dumps(selector)+').click()')
def menu(text):
    click('.queue-tools-menu summary')
    execute('[...document.querySelectorAll(".queue-tools-menu button")].find(b=>b.textContent.includes('+json.dumps(text)+')).click()')
execute('''
window.featureBase=window.__VESPER_TEST_INVOKE__;window.featureQueue=structuredClone(testQueue);window.featureWrites=[];window.featureReject=true;
window.__VESPER_TEST_INVOKE__=async(cmd,args,...rest)=>{
 if(cmd==='status')return {authenticated:true,browser:[],mpv:true,extractor:true,volume:60};
 if(cmd==='playlist_change'){featureWrites.push(structuredClone(args.change));if(featureReject)throw 'Fixture write rejected';return 'PLsavedqueue';}
 return featureBase(cmd,args,...rest);
};
testQueue={title:'Saved queue',items:[makeTrack(0),{...makeTrack(90),id:'local:'+('a'.repeat(40)),source:'local'},makeTrack(2),makeTrack(2),makeTrack(3),makeTrack(4),makeTrack(5)],selected:0,dynamic:false,context:{playlistId:'PLfixture'},continuation:{continuation:'next'}};
''')
invoke('plugin:event|emit',{'event':'auth-updated','payload':None})
click('nav button')
wait("return !!document.querySelector('.hero .primary')")
click('.hero .primary')
wait("return document.querySelector('.now-title').textContent==='Track 0'&&!document.querySelector('.main-play').disabled")
click('[aria-label="Очередь воспроизведения"]')
wait("return !!document.querySelector('.queue-panel')")
menu('Сохранить как плейлист')
wait("return !!document.querySelector('.playlist-dialog[open]')")
assert execute("return document.querySelectorAll('.privacy-option').length===3&&!document.querySelector('.playlist-dialog select')&&document.querySelector('.privacy-option.chosen').textContent==='Личный'")
assert execute("return document.querySelector('.playlist-hint').textContent.includes('6 треков')&&document.querySelector('.playlist-hint').textContent.includes('Локальные файлы (1)')&&document.querySelector('.playlist-hint').textContent.includes('Продолжение')")
click('input[name="playlist-privacy"][value="UNLISTED"]')
assert execute("return document.querySelector('.privacy-option.chosen').textContent==='По ссылке'")
# The options are rendered by the app, including at the narrow supported width.
window=subprocess.check_output(['xdotool','search','--onlyvisible','--class','neisora'],text=True,timeout=5).splitlines()[0]
subprocess.run(['xdotool','windowsize',window,'420','740'],check=True);time.sleep(.3)
assert execute("return [...document.querySelectorAll('.privacy-option')].every(e=>{const r=e.getBoundingClientRect();return r.width>70&&r.x>=0&&r.right<=innerWidth})")
Path('artifacts').mkdir(exist_ok=True)
subprocess.run(['import','-window',window,'artifacts/privacy-09-420.png'],check=True)
click('.playlist-dialog button[type=submit]')
wait("return document.querySelector('.playlist-error')?.textContent.includes('Fixture write rejected')")
assert execute("return featureWrites.at(-1).videoIds.join(',')==='test0000000,test0000002,test0000002,test0000003,test0000004,test0000005'&&featureWrites.at(-1).privacy==='UNLISTED'")
execute('featureReject=false')
click('.playlist-dialog button[type=submit]')
wait("return !document.querySelector('.playlist-dialog')")
assert execute("return featureWrites.length===2&&document.querySelector('.now-title').textContent==='Track 0'")
subprocess.run(['xdotool','windowsize',window,'1360','860'],check=True)
print('queue export: themed privacy tiles at 420 px, private default, remote order/duplicates, local exclusion, continuation warning and retry: PASS',flush=True)

for request in [{'mode':'minutes','minutes':0},{'mode':'minutes','minutes':1441},{'mode':'minutes','minutes':-1},{'mode':'off','minutes':3},{'mode':'unknown'}]:
    assert not invoke('set_sleep_timer',{'request':request})['ok'],request
menu('Таймер сна')
wait("return !!document.querySelector('.sleep-dialog[open]')")
click('.sleep-presets button')
wait("return !document.querySelector('.sleep-dialog')")
timer=invoke('get_sleep_timer')['value']
assert timer['mode']=='minutes' and 890_000<timer['deadlineMs']-time.time()*1000<901_000,timer
menu('Таймер сна')
wait("return document.querySelector('.sleep-current')?.textContent.includes('Осталось')")
click('.sleep-cancel')
wait("return !document.querySelector('.sleep-dialog')")
assert invoke('get_sleep_timer')['value']['mode']=='off'
menu('Таймер сна')
wait("return !!document.querySelector('.sleep-dialog[open]')")
execute('const e=document.querySelector(".sleep-dialog input");e.value=7;e.dispatchEvent(new Event("input",{bubbles:true}))')
click('.sleep-dialog form .primary')
wait("return !document.querySelector('.sleep-dialog')")
assert 410_000<invoke('get_sleep_timer')['value']['deadlineMs']-time.time()*1000<421_000
menu('Таймер сна')
wait("return !!document.querySelector('.sleep-dialog[open]')")
click('.sleep-after-track')
wait("return !document.querySelector('.sleep-dialog')")
assert invoke('get_sleep_timer')['value']['mode']=='track'
assert invoke('set_sleep_timer',{'request':{'mode':'off'}})['ok']
click('[aria-label="Закрыть очередь"]')
execute("window.__VESPER_TEST_INVOKE__=featureBase;testQueue=featureQueue;document.querySelector('nav button').click();document.querySelector('.toast button')?.click()")
wait("return !!document.querySelector('.hero .primary')")
click('.hero .primary')
wait("return !document.querySelector('.main-play').disabled")
print('sleep: invalid native requests rejected, presets/custom minutes/after-track/cancel through real WebKit UI: PASS',flush=True)
