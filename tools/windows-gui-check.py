"""Native WebView2 production smoke in an isolated Windows profile (no account)."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import time

# Rust's dirs crate uses Known Folders, rather than trusting APPDATA overrides.
# This harness therefore runs only on a fresh disposable GitHub Windows VM.
if os.name != 'nt' or os.environ.get('GITHUB_ACTIONS') != 'true':
    raise SystemExit('Run this smoke test only on a disposable GitHub Windows runner')
def known_folder(csidl):
    buffer = ctypes.create_unicode_buffer(32768)
    if ctypes.windll.shell32.SHGetFolderPathW(None, csidl, None, 0, buffer):
        raise RuntimeError('Cannot resolve the Windows Known Folder')
    return Path(buffer.value)
profile = known_folder(0x1a)/'local.kumite.vesper'
webview = known_folder(0x1c)/'local.kumite.vesper'
if profile.exists() or webview.exists():
    raise SystemExit('Refusing to overwrite an existing application profile')
with tempfile.TemporaryDirectory(prefix='neisora-gui-') as temporary:
    root = Path(temporary)
    os.environ.update(VESPER_MPV_AO='null',
                      VESPER_WEBDRIVER_URL='http://127.0.0.1:4456', VESPER_WEBDRIVER_SESSION=str(root/'session.json'))
    from local_fixtures import write_track
    from webdriver_client import create, execute, invoke
    music = root/'Музыка 日本語'
    write_track(music/'Alpha.wav', 'Windows Alpha')
    write_track(music/'Beta.wav', 'Windows Beta')
    profile.mkdir(parents=True)
    (profile/'NEISORA-CI-PROFILE').write_text('disposable runner only')
    canonical = os.path.realpath(music)
    if not canonical.startswith('\\\\?\\'):
        canonical = '\\\\?\\' + canonical
    (profile/'local-music.json').write_text(json.dumps({'roots':[{'id':hashlib.sha1(canonical.encode()).hexdigest(), 'path':canonical, 'available':True}], 'tracks':[], 'revision':0}), encoding='utf-8')
    with (root/'driver.log').open('w', encoding='utf-8') as log:
        driver = subprocess.Popen(['tauri-driver', '--port', '4456', '--native-port', '4457'], stdout=log, stderr=subprocess.STDOUT)
        def wait(script, seconds=35):
            end = time.monotonic()+seconds
            while time.monotonic() < end:
                if execute(script): return
                time.sleep(.15)
            raise AssertionError(script)
        try:
            import urllib.request
            for _ in range(100):
                try: urllib.request.urlopen('http://127.0.0.1:4456/status', timeout=1).close(); break
                except OSError: time.sleep(.1)
            create()
            wait('return !!document.querySelector(".search input")')
            assert not execute('return typeof window.__VESPER_TEST_INVOKE__ === "function"')
            # The disposable runner owns this clipboard; never run against a personal desktop.
            def read_clipboard():
                user=ctypes.windll.user32;kernel=ctypes.windll.kernel32
                user.GetClipboardData.argtypes=[ctypes.c_uint];user.GetClipboardData.restype=ctypes.c_void_p
                kernel.GlobalLock.argtypes=[ctypes.c_void_p];kernel.GlobalLock.restype=ctypes.c_void_p
                kernel.GlobalUnlock.argtypes=[ctypes.c_void_p]
                assert user.OpenClipboard(None)
                try:
                    handle=user.GetClipboardData(13);assert handle
                    pointer=kernel.GlobalLock(handle);assert pointer
                    try:return ctypes.wstring_at(pointer)
                    finally:kernel.GlobalUnlock(handle)
                finally:user.CloseClipboard()
            target={'kind':'song','id':'abcdefghijk'}
            assert invoke('copy_share_link',{'target':target})['ok']
            assert read_clipboard()=='https://music.youtube.com/watch?v=abcdefghijk'
            assert not invoke('open_share_link',{'target':{'kind':'song','id':'https://evil.test'}})['ok']
            report=invoke('copy_diagnostic_report');assert report['ok']
            assert json.loads(read_clipboard())==report['value'] and report['value']['platform']=='windows'
            print('Windows native clipboard: share link, exact diagnostic report and invalid external URL rejection: PASS',flush=True)

            status = invoke('status')
            assert status['ok'] and not status['value']['authenticated'], status
            assert status['value']['mpv'] and status['value']['extractor'], status
            execute("[...document.querySelectorAll('.sidebar nav button')].find(b=>b.textContent==='На устройстве').click()")
            wait("return document.querySelectorAll('.local-tracks .track-row').length === 2")
            execute("document.querySelector('.local-tracks .track-row').click()")
            wait("return document.querySelector('.now-title').textContent==='Windows Alpha' && !document.querySelector('.main-play').disabled && document.querySelector('.main-play').getAttribute('aria-label')==='Пауза'")
            execute("document.querySelector('.expand-player').click()")
            wait("return document.querySelectorAll('.lyric-line').length===3")
            assert invoke('control', {'action':'pause'})['ok']
            assert invoke('control', {'action':'seek', 'value':2})['ok']
            wait("return Math.abs(+document.querySelector('input[aria-label=\"Позиция трека\"]').value-2)<.8")
            assert invoke('control', {'action':'volume_commit', 'value':37})['ok']
            assert invoke('status')['value']['volume'] == 37
            execute("document.querySelector('[aria-label=\"Свернуть плеер\"]').click()")
            execute("document.querySelector('.playback [aria-label=\"Следующий трек\"]').click()")
            wait("return document.querySelector('.now-title').textContent==='Windows Beta' && !document.querySelector('.main-play').disabled")
            assert not execute('return !!document.querySelector(".toast")'), execute('return document.querySelector(".toast")?.textContent')
            outputs = invoke('audio_outputs')
            assert outputs['ok'] and outputs['value']['preferred'] == 'auto', outputs
            assert invoke('set_audio_output', {'name':'auto'})['ok']
            assert not invoke('set_audio_output', {'name':'null'})['ok']
            assert not invoke('get_lyric_choice', {'id':'../../profile'})['ok']
            assert not invoke('preview_lyric_version', {'recordId':0})['ok']
            print('Windows WebView2: production IPC, bundled runtime, Unicode local indexing/playback, seek, volume, LRC, next PASS')
        except Exception:
            log.flush()
            print((root/'driver.log').read_text(encoding='utf-8', errors='replace')[-10000:])
            raise
        finally:
            # Keep the driver alive until taskkill captures its entire tree.
            # Closing the app first can let the parent exit before msedgedriver,
            # leaving its inherited driver.log handle open on Windows.
            subprocess.run(['taskkill', '/PID', str(driver.pid), '/T', '/F'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            driver.wait(timeout=15)
            if (profile/'NEISORA-CI-PROFILE').is_file():
                # The marker can only have been created after both absence checks above.
                for path in [profile, webview]:
                    for attempt in range(20):
                        try: shutil.rmtree(path); break
                        except FileNotFoundError: break
                        except PermissionError: time.sleep(.1)

    # Windows can release a descendant's file handles shortly after taskkill.
    # The file context is closed here; remove only this temporary fixture tree.
    for attempt in range(50):
        try:
            shutil.rmtree(root)
            break
        except FileNotFoundError:
            break
        except PermissionError:
            if attempt == 49:
                raise
            time.sleep(.1)
