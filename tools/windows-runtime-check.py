"""Offline native Windows playback: bundled helpers, Unicode path, real mpv pipe."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time
import wave
import os

root = Path(__file__).resolve().parents[1]
runtime = root/'src-tauri/resources/runtime'
manifest = json.loads((root/'tools/runtime-manifest.json').read_text())
python = runtime/'python/python.exe'
assert subprocess.check_output([str(runtime/'node.exe'), '--version'], text=True).strip() == 'v'+manifest['node']['version']
assert hashlib.sha256((runtime/'yt-dlp').read_bytes()).hexdigest() == manifest['yt-dlp']['sha256']
assert subprocess.check_output([str(python), str(runtime/'yt-dlp'), '--version'], text=True).strip() == manifest['yt-dlp']['version']
subprocess.run([str(python), '-c', 'import sys, sqlite3, ssl; sys.path.insert(0,sys.argv[1]); from yt_dlp.cookies import extract_cookies_from_browser', str(runtime/'yt-dlp')], check=True)
with tempfile.TemporaryDirectory(prefix='neisora-музыка-') as directory:
    song = Path(directory)/'трек 日本語.wav'
    with wave.open(str(song), 'wb') as f:
        f.setnchannels(1); f.setsampwidth(2); f.setframerate(8000); f.writeframes(b'\0\0'*8000*20)
    pipe = rf'\\.\pipe\neisora-runtime-check-{os.getpid()}'
    process = subprocess.Popen([str(runtime/'mpv.exe'), '--no-config', '--load-scripts=no', '--input-media-keys=no', '--media-controls=no', '--idle=yes', '--no-video', '--terminal=yes', '--log-file='+str(Path(directory)/'mpv-check.log'), '--ao=null', '--input-ipc-server='+pipe], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
    try:
        for _ in range(150):
            try: stream = open(pipe, 'r+b', buffering=0); break
            except OSError:
                if process.poll() is not None:
                    log = Path(directory)/'mpv-check.log'
                    raise AssertionError(f'mpv exited {process.returncode:#x}: '+process.stderr.read().decode(errors='replace')+(log.read_text(encoding='utf-8', errors='replace')[-4000:] if log.exists() else ' (no log file)'))
                time.sleep(.1)
        else: raise AssertionError('mpv IPC unavailable')
        with stream:
            serial = 0
            def command(*args):
                global serial
                serial += 1
                stream.write((json.dumps({'command':args, 'request_id':serial})+'\n').encode())
                while True:
                    message = bytearray()
                    while not message.endswith(b'\n'):
                        chunk = stream.read(1)
                        if not chunk: raise AssertionError('mpv closed IPC')
                        message.extend(chunk)
                    result = json.loads(message)
                    if result.get('request_id') == serial:
                        assert result.get('error') == 'success', result
                        return result.get('data')
            command('loadfile', str(song))
            for _ in range(100):
                try:
                    if command('get_property', 'duration') >= 19: break
                except AssertionError: pass
                time.sleep(.1)
            else: raise AssertionError('Unicode local track did not load')
            command('set_property', 'pause', True)
            command('seek', 7, 'absolute+exact')
            for _ in range(50):
                if abs(command('get_property','time-pos')-7) < .3: break
                time.sleep(.1)
            else: raise AssertionError('Exact seek failed')
            command('set_property', 'volume', 37)
            assert command('get_property', 'volume') == 37
            command('quit')
        process.wait(timeout=10)
    finally:
        if process.poll() is None: process.kill()
        process.wait(timeout=10)
print('Windows runtime: pinned Node/yt-dlp, Python cookie module, actual mpv playback/Unicode/seek/volume/IPC PASS')
