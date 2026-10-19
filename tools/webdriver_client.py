"""Read-only inspection of the test instance. Never imports a browser session."""
import base64
import json
import pathlib
import os
import urllib.request

BASE = os.environ.get('VESPER_WEBDRIVER_URL', 'http://127.0.0.1:4444')
SESSION = pathlib.Path(os.environ.get('VESPER_WEBDRIVER_SESSION', '/tmp/vesper-webdriver.json'))

def request(path, body=None, method=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, headers={'Content-Type': 'application/json'}, method=method)
    try:
        with urllib.request.urlopen(req, timeout=60) as response:
            return json.load(response)['value']
    except urllib.error.HTTPError as e:
        raise RuntimeError(e.read().decode()) from e

def create():
    value = request('/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': str(pathlib.Path(os.environ.get('NEISORA_TEST_BINARY', 'src-tauri/target/debug/neisora')).resolve())}}}})
    SESSION.write_text(json.dumps(value))
    return value['sessionId']

def sid(): return json.loads(SESSION.read_text())['sessionId']
def execute(script): return request('/session/' + sid() + '/execute/sync', {'script':script,'args':[]})
def invoke(name, args=None):
    script = 'const done=arguments[arguments.length-1]; window.__TAURI_INTERNALS__.invoke(' + json.dumps(name) + ',' + json.dumps(args or {}) + ').then(v=>done({ok:true,value:v})).catch(e=>done({ok:false,error:String(e)}));'
    return request('/session/' + sid() + '/execute/async', {'script':script,'args':[]})
def screenshot(name):
    value = request('/session/' + sid() + '/screenshot')
    pathlib.Path('artifacts').mkdir(exist_ok=True)
    pathlib.Path('artifacts/' + name + '.png').write_bytes(base64.b64decode(value))

if __name__ == '__main__':
    import sys
    if '--create' in sys.argv: print(create())
    else:
        print(execute('return {title:document.title,body:document.body.innerText.slice(0,2500)}'))
        screenshot('native-window')
