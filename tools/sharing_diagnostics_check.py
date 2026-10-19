"""Sharing and bounded diagnostic export in real WebKit, on the isolated X11 clipboard."""
import json,subprocess,time
from pathlib import Path
from webdriver_client import execute,invoke

def wait(script):
    end=time.monotonic()+12
    while time.monotonic()<end:
        if execute(script):return
        time.sleep(.05)
    raise AssertionError(script+'\n'+str(execute('return document.body.innerText.slice(-1200)')))
def click(selector):execute('document.querySelector('+json.dumps(selector)+').click()')
def clipboard():
    return subprocess.check_output(['/usr/bin/python3','-c',"import gi;gi.require_version('Gtk','3.0');from gi.repository import Gtk,Gdk;print(Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).wait_for_text() or '',end='')"],text=True,timeout=5)

# Validate the actual native boundary: there is no general URL opener or arbitrary text-copy IPC.
for target in [{'kind':'song','id':'../../private'},{'kind':'song','id':'https://evil.test'},{'kind':'song','id':'abc&x=y'},{'kind':'playlist','id':'javascript:alert(1)'},{'kind':'url','id':'https://evil.test'}]:
    for command in ['share_link','copy_share_link','open_share_link']:
        assert not invoke(command,{'target':target})['ok'],(command,target)
for kind,id,suffix in [('song','abcdefghijk','watch?v=abcdefghijk'),('playlist','PLfixture','playlist?list=PLfixture'),('album','MPREb_123','browse/MPREb_123')]:
    assert invoke('share_link',{'target':{'kind':kind,'id':id}})['value']=='https://music.youtube.com/'+suffix
execute('window.shareBase=window.__VESPER_TEST_INVOKE__;window.shareOpened=[];window.__VESPER_TEST_INVOKE__=(cmd,args,...rest)=>{if(cmd==="open_share_link"){shareOpened.push(JSON.parse(JSON.stringify(args.target)));return Promise.resolve(null);}return shareBase(cmd,args,...rest)};document.querySelector("nav button").click()')
wait("return !!document.querySelector('.hero .primary')")
assert execute("return !document.querySelector('.card [aria-label^=\"Поделиться\"],.track-menu-items [aria-label^=\"Поделиться\"],.collection-actions [aria-label^=\"Поделиться\"]')")
click('.hero .primary')
wait("return !!document.querySelector('.current-track .share-button') && !document.querySelector('.main-play').disabled")
current_id=execute("return document.querySelector('.now-title').textContent.replace('Track ','').padStart(7,'0')")
expected='https://music.youtube.com/watch?v=test'+current_id
click('.current-track .share-button')
wait("return document.querySelector('.share-dialog input')?.value==="+json.dumps(expected))
click('.share-actions .primary')
wait("return document.querySelector('.share-actions .primary').textContent.includes('Скопировано')")
assert clipboard()==expected
click('.share-actions .secondary')
wait("return !document.querySelector('.share-dialog')")
assert execute("return shareOpened.at(-1).kind==='song'&&shareOpened.at(-1).id==="+json.dumps('test'+current_id))
click('.current-track .share-button')
wait("return !!document.querySelector('.share-dialog input')")
window=subprocess.check_output(['xdotool','search','--onlyvisible','--class','neisora'],text=True,timeout=5).splitlines()[0]
subprocess.run(['xdotool','windowsize',window,'420','640'],check=True);time.sleep(.25)
assert execute("return [...document.querySelectorAll('.share-dialog button,.share-dialog input')].every(e=>{const r=e.getBoundingClientRect();return r.x>=0&&r.right<=innerWidth&&r.y>=0&&r.bottom<=innerHeight})")
Path('artifacts').mkdir(exist_ok=True)
subprocess.run(['import','-window',window,'artifacts/share-010-420.png'],check=True,timeout=10)
click('[aria-label="Закрыть окно ссылки"]')
subprocess.run(['xdotool','windowsize',window,'1360','860'],check=True)
click('.expand-player')
wait("return !!document.querySelector('[aria-label=\"Поделиться текущим треком\"]')")
assert execute("return !document.querySelector('.expanded-player header [aria-label^=\"Поделиться\"]')")
click('[aria-label="Свернуть плеер"]')
print('sharing: native URL validation, song/album/playlist links, actual clipboard, open action, current track and 420 px layout: PASS',flush=True)

assert invoke('clear_diagnostics')['ok']
# Arbitrary raw errors are reduced to allowlisted enum values, never exported.
for _ in range(40):assert not invoke('control',{'action':'PRIVATE_COOKIE https://evil.test/?sig=secret /home/private/music Track Title'})['ok']
report=invoke('diagnostic_report')['value']
assert len(report['events'])==32,report
text=json.dumps(report)
for secret in ['PRIVATE_COOKIE','evil.test','sig=secret','/home/','Track Title','cookies','file://','googlevideo']:
    assert secret not in text,secret
assert set(report['player'])=={'active','paused','buffering'}
click('nav button[aria-label="Настройки"]')
wait("return document.querySelector('.diagnostic-status')?.textContent.includes('Последний сбой')")
click('.diagnostic-actions .secondary')
wait("return document.querySelector('.diagnostic-actions .secondary').textContent.includes('Скопировано')")
copied=json.loads(clipboard())
assert copied['schema']==1 and len(copied['events'])==32
execute("document.querySelector('.diagnostic-preview').open=true")
assert json.loads(execute("return document.querySelector('.diagnostic-preview textarea').value"))==copied
subprocess.run(['xdotool','windowsize',window,'420','740'],check=True);time.sleep(.25)
assert execute("return document.documentElement.scrollWidth<=innerWidth&&document.querySelector('.diagnostic-preview textarea').getBoundingClientRect().right<=innerWidth")
click('[aria-label="Очистить журнал диагностики"]')
wait("return document.querySelector('.diagnostic-status')?.textContent.includes('Сбоев воспроизведения')")
assert not invoke('diagnostic_report')['value']['events']
subprocess.run(['xdotool','windowsize',window,'1360','860'],check=True)
execute('window.__VESPER_TEST_INVOKE__=shareBase;document.querySelector("nav button").click()')
wait("return !!document.querySelector('.hero .primary')")
print('diagnostics: real IPC failures, 32-event bound, no raw errors/metadata, exact clipboard preview, narrow layout and clearing: PASS',flush=True)
