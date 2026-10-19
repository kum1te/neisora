"""Exercise AppImage mpv against a real Pulse protocol server, without audible output.

--start-server creates a disposable null sink for CI. Without it this checks the
current user's audio server using a one-second silent stream at zero volume.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import wave

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--start-server', action='store_true')
parser.add_argument('image', type=Path)
args = parser.parse_args()
image = args.image.resolve()
with tempfile.TemporaryDirectory(prefix='neisora-image-audio-') as temporary:
    root = Path(temporary)
    subprocess.run([str(image), '--appimage-extract'], cwd=root,
                   stdout=subprocess.DEVNULL, check=True)
    mpv = root/'squashfs-root/usr/bin/mpv'
    linked = subprocess.check_output(['ldd', str(mpv)], text=True)
    assert 'libpulse.so.0 =>' in linked and 'not found' not in linked, linked
    assert not any('libpulse' in line and str(root) in line for line in linked.splitlines()), linked
    assert not any('libfontconfig' in line and str(root) in line for line in linked.splitlines()), linked
    sample = root/'silence.wav'
    with wave.open(str(sample), 'wb') as output:
        output.setparams((1, 2, 8000, 0, 'NONE', 'not compressed'))
        output.writeframes(b'\0\0'*8000)
    env = dict(os.environ)
    server = None
    with (root/'pulse.log').open('w+') as log:
        try:
            if args.start_server:
                for name in ['home', 'runtime', 'state']:
                    (root/name).mkdir(mode=0o700)
                socket = root/'pulse.sock'
                env.update(HOME=str(root/'home'), XDG_RUNTIME_DIR=str(root/'runtime'),
                           PULSE_RUNTIME_PATH=str(root/'runtime'), PULSE_STATE_PATH=str(root/'state'),
                           PULSE_SERVER='unix:'+str(socket))
                server = subprocess.Popen([
                    shutil.which('pulseaudio') or 'pulseaudio', '-n', '--daemonize=no',
                    '--exit-idle-time=-1', '--log-level=error', '--log-target=stderr',
                    '--load=module-null-sink sink_name=neisora_test',
                    '--load=module-native-protocol-unix socket='+str(socket)+' auth-anonymous=1',
                ], env=env, stdout=log, stderr=log)
                for _ in range(100):
                    if socket.exists():
                        break
                    if server.poll() is not None:
                        log.seek(0)
                        raise AssertionError('Test audio server failed: '+log.read())
                    time.sleep(.1)
                else:
                    raise AssertionError('Test audio socket did not appear')
            result = subprocess.run([
                str(mpv), '--no-config', '--load-scripts=no', '--no-video', '--no-terminal',
                '--ytdl=no', '--ao=pulse', '--volume=0', str(sample),
            ], env=env, capture_output=True, text=True, timeout=20)
            assert result.returncode == 0, (result.returncode, result.stderr[-4000:])
            assert 'Fontconfig error' not in result.stderr, result.stderr[-4000:]
        finally:
            if server is not None:
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
print('AppImage: host audio client, actual Pulse protocol playback and font configuration PASS', flush=True)
