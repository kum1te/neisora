#!/bin/sh
# Run inside xvfb-run/dbus-run-session, or an isolated desktop test session.
set -eu
test_root=$(mktemp -d /tmp/neisora-gui-XXXXXX)
export XDG_CONFIG_HOME="$test_root/config" XDG_DATA_HOME="$test_root/data"
export VESPER_MPV_AO=null WEBKIT_DISABLE_DMABUF_RENDERER=1
export VESPER_WEBDRIVER_URL=http://127.0.0.1:4446
export VESPER_WEBDRIVER_SESSION="$test_root/session.json"
setsid tauri-driver --port 4446 --native-port 4447 > "$test_root/driver.log" 2>&1 &
driver_pid=$!
cleanup() {
    python3 - <<'PY' || true
import sys
sys.path.insert(0,'tools')
from webdriver_client import execute
execute('window.__TAURI_INTERNALS__.invoke("plugin:window|close",{label:"main"}).catch(()=>{})')
PY
    python3 - "$driver_pid" <<'PY_STOP'
import os,signal,sys
try:os.killpg(int(sys.argv[1]),signal.SIGTERM)
except ProcessLookupError:pass
PY_STOP
    wait "$driver_pid" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
python3 - <<'PY'
import time,urllib.request
for _ in range(100):
    try:
        urllib.request.urlopen('http://127.0.0.1:4446/status',timeout=1).close();break
    except OSError:time.sleep(.1)
else:raise SystemExit('WebDriver did not start')
PY
python3 tools/regression.py --create
python3 tools/playerbar_check.py
python3 tools/visualizer_check.py
python3 tools/audit_regression.py
python3 tools/artwork_check.py
python3 tools/features_check.py
python3 tools/local_queue_check.py
python3 tools/queue_interaction_check.py
python3 tools/large_queue.py
python3 tools/playlist_repeat_check.py
python3 tools/queue_features_check.py
python3 tools/sleep_queue_check.py
python3 tools/sharing_diagnostics_check.py
python3 tools/radio_skip_check.py
python3 tools/lyrics_output_check.py
app_pid=$(python3 - <<'PY'
import os
from pathlib import Path
for path in Path('/proc').iterdir():
    try:
        if path.name.isdigit() and path.joinpath('comm').read_text().strip()=='neisora' and ('XDG_CONFIG_HOME='+os.environ['XDG_CONFIG_HOME']).encode() in path.joinpath('environ').read_bytes().split(b'\0'):
            print(path.name);break
    except OSError:pass
else:raise SystemExit('Isolated app PID not found')
PY
)
python3 tools/lifecycle.py "$app_pid" --cycles 100
python3 tools/security.py
python3 tools/session_restore_check.py
python3 tools/session_restore_check.py --queue
