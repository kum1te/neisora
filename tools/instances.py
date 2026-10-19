"""Linux single-instance checks in disposable profiles. No browser/account import."""
import argparse
import fcntl
import json
import os
import socket
import stat
from pathlib import Path
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', default='src-tauri/target/debug/neisora')
args = parser.parse_args()
binary = str(Path(args.binary).resolve())
identifier = json.loads(Path('src-tauri/tauri.conf.json').read_text())['identifier']

with tempfile.TemporaryDirectory(prefix='neisora-instances-') as work:
    root = Path(work)
    children = []
    logs = []

    def environment(name):
        return {**os.environ, 'XDG_CONFIG_HOME': str(root/name/'config'),
                'XDG_DATA_HOME': str(root/name/'data'), 'VESPER_MPV_AO': 'null',
                'WEBKIT_DISABLE_DMABUF_RENDERER': '1'}

    def start(env):
        log = tempfile.TemporaryFile()
        logs.append(log)
        child = subprocess.Popen([binary], env=env, stdout=log, stderr=log)
        children.append(child)
        return child

    def wait_locked(child, env):
        lock = Path(env['XDG_CONFIG_HOME'])/identifier/'instance.lock'
        end = time.monotonic()+12
        while time.monotonic() < end:
            assert child.poll() is None, 'App exited before acquiring the profile'
            if lock.exists():
                with lock.open('rb') as stream:
                    try:
                        fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        fcntl.flock(stream, fcntl.LOCK_UN)
                    except BlockingIOError:
                        time.sleep(.4)
                        assert child.poll() is None
                        return
            time.sleep(.05)
        raise AssertionError('Profile lock was not acquired')

    try:
        env = environment('first')
        first = start(env)
        wait_locked(first, env)
        endpoint = Path(env['XDG_CONFIG_HOME'])/identifier/'instance.sock'
        assert stat.S_IMODE(endpoint.stat().st_mode) == 0o600
        with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as sender:
            sender.settimeout(.3)
            for payload in [b'', b'bad', b'\x01\x00', b'x'*8192]:
                sender.sendto(payload, str(endpoint))
        time.sleep(.2)
        assert first.poll() is None
        print('private endpoint survives malformed activation messages: PASS', flush=True)
        assert start(env).wait(timeout=10) == 0
        assert first.poll() is None
        print('second launch exits while primary stays alive: PASS', flush=True)

        unavailable = {**env, 'DBUS_SESSION_BUS_ADDRESS': f'unix:path={root}/missing-bus'}
        assert start(unavailable).wait(timeout=10) == 0
        assert first.poll() is None
        print('activation works without D-Bus: PASS', flush=True)

        assert subprocess.run([binary, '--probe'], env=env, stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL, timeout=5).returncode != 0
        print('diagnostic mode cannot bypass profile ownership: PASS', flush=True)

        alias = root/'config-alias'
        alias.symlink_to(env['XDG_CONFIG_HOME'], target_is_directory=True)
        assert start({**env, 'XDG_CONFIG_HOME': str(alias)}).wait(timeout=10) == 0
        print('path alias routes to the same primary: PASS', flush=True)

        other_env = environment('other')
        other = start(other_env)
        wait_locked(other, other_env)
        assert first.poll() is None
        print('different profiles remain independent: PASS', flush=True)

        first.kill()
        first.wait(timeout=5)
        replacement = start(env)
        wait_locked(replacement, env)
        assert start(env).wait(timeout=10) == 0
        print('crash releases ownership and restart remains exclusive: PASS', flush=True)
    finally:
        for child in children:
            if child.poll() is None:
                child.terminate()
        for child in children:
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
        for log in logs:
            log.close()
