"""Adversarial IPC/XSS/CSP checks. Run only against the isolated fixture instance."""
import json
import time
from webdriver_client import execute, invoke, request, sid


def check(name, value):
    assert value, name
    print(name, 'PASS', flush=True)


def wait(script, seconds=8):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if execute(script):
            return True
        time.sleep(.05)
    return False


assert execute('return typeof window.__VESPER_TEST_INVOKE__ === "function"'), 'Use the isolated fixture instance after regression.py'
assert not invoke('status')['value']['authenticated'], 'Do not use an authenticated profile'

for command, arguments in [
    ('play', {'id': '--exec=sh'}),
    ('save_playback_session', {'value':'{"version":1,"queue":{"items":[]}}'}),
    ('play', {'id':'abcdefghijk','position':-1}),
    ('play', {'id':'abcdefghijk','position':604801}),
    ('play', {'id': '../../etc/passwd'}),
    ('prefetch', {'id': 'https://127.0.0.1/'}),
    ('sync_media', {'snapshot': {'id': '../../etc/passwd'}}),
    ('record_history', {'item': {'id': '../../etc/passwd'}, 'playback': 'test'}),
    ('save_collection', {'browseId': '../../etc/passwd', 'saved': True}),
    ('track_like', {'id': '../../etc/passwd'}),
    ('playlist_change', {'change': {'action':'run','url':'file:///etc/passwd'}}),
    ('playlist_change', {'change': {'action':'create','title':'<script>','description':'','privacy':'PRIVATE'}}),
    ('playlist_change', {'change': {'action':'add','playlistId':'RDmix','videoId':'abcdefghijk'}}),
    ('playlist_change', {'change': {'action':'delete','playlistId':'PLtest'}}),
    ('play_local', {'id':'../../etc/passwd'}),
    ('local_artwork', {'id':'file:///etc/passwd'}),
    ('local_lyrics', {'id':'../../etc/passwd'}),
    ('remove_music_folder', {'id':'../../etc/passwd'}),
    ('library', {'category': '../../etc/passwd'}),
    ('import_browser', {'browser': 'firefox;echo should-not-run'}),
    ('import_browser', {'browser': '../../etc/passwd'}),
    ('control', {'action': 'run', 'value': 0}),
    ('control', {'action': 'seek', 'value': 'not-a-number'}),
    ('set_appearance', {'preferences': {'selected': 'x', 'custom': [], 'followWallpaper': False, 'visualizer': {'mode': '../../x', 'intensity': .5, 'speed': 1}}}),
    ('set_appearance', {'preferences': {'selected': 'x', 'custom': [{'id':'x','name':'x','accent':'red;position:fixed','background':'#000000','surface':'#111111','text':'#ffffff'}], 'followWallpaper': False, 'visualizer': {'mode': 'rings', 'intensity': .5, 'speed': 1}}}),
]:
    check('IPC rejects ' + command + ' ' + next(iter(arguments)), not invoke(command, arguments)['ok'])

execute('''window.__securityXss=false;window.securityPayload='<img src=x onerror="window.__securityXss=true">';
for(const section of testFeed.sections) for(const item of section.items) item.title=securityPayload;
document.querySelector('[aria-label="Обновить подборки"]').click();''')
check('titles are escaped, not interpreted as HTML', wait('return document.querySelector(".card-title")?.textContent===securityPayload && !window.__securityXss && !document.querySelector("img[onerror]")'))
execute('''window.securityOriginalFixture=window.__VESPER_TEST_INVOKE__;window.__VESPER_TEST_INVOKE__=(cmd,args,...rest)=>cmd==='get_lyrics'?Promise.resolve({synced:false,plain:securityPayload,lines:[],source:'Security fixture'}):securityOriginalFixture(cmd,args,...rest);document.querySelector('.expand-player').click()''')
check('lyrics are escaped, not interpreted as HTML', wait('return document.querySelector(".plain-lyrics")?.textContent===securityPayload && !window.__securityXss && !document.querySelector(".plain-lyrics img")'))
execute('''window.securityViolations=[];document.addEventListener('securitypolicyviolation',e=>securityViolations.push(e.effectiveDirective));window.securityConnectSettled=false;fetch('http://127.0.0.1:9/neisora-security').then(()=>{window.securityConnectAllowed=true}).catch(()=>{}).finally(()=>{window.securityConnectSettled=true});''')
check('CSP blocks unapproved network destinations', wait('return securityConnectSettled && !window.securityConnectAllowed && securityViolations.includes("connect-src")'))
check('CSP blocks dynamic JavaScript compilation', execute('try { new Function("return 1")(); return false } catch(e) { return e.name === "EvalError" }'))
execute('document.querySelector("[aria-label=\\"Свернуть плеер\\"]").click();window.__VESPER_TEST_INVOKE__=securityOriginalFixture')

# Exercise the real remote-origin boundary on a page served only on loopback.
# The login webview uses a separate WebKit context that WebDriver cannot enumerate.
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread

class RemotePage(BaseHTTPRequestHandler):
    def do_GET(self):
        data = b'<!doctype html><title>Untrusted origin security probe</title><p>Local test fixture</p>'
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_):
        pass

invoke('logout')  # Isolated unauthenticated profile only; close any previous login window.
server = ThreadingHTTPServer(('127.0.0.1', 0), RemotePage)
Thread(target=server.serve_forever, daemon=True).start()
origin = 'http://127.0.0.1:' + str(server.server_port)
try:
    request('/session/' + sid() + '/url', {'url': origin})
    result = execute('return {origin:location.origin,ipc:typeof window.__TAURI_INTERNALS__?.invoke==="function"}')
    check('probe runs on a distinct untrusted origin', result['origin'] == origin)
    if result['ipc']:
        response = invoke('status')
        check('remote origin cannot call application IPC', not response['ok'])
    else:
        check('remote origin has no callable IPC bridge', True)
finally:
    request('/session/' + sid() + '/url', {'url': 'tauri://localhost'})
    server.shutdown()
    server.server_close()
print('Adversarial checks passed', flush=True)
