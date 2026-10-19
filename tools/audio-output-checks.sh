#!/bin/sh
# Run with dbus-run-session + xvfb-run. Own audio server, no real device access.
set -eu
test_root=$(mktemp -d /tmp/neisora-output-XXXXXX)
export XDG_CONFIG_HOME="$test_root/config" XDG_DATA_HOME="$test_root/data"
export XDG_RUNTIME_DIR="$test_root/runtime" PULSE_RUNTIME_PATH="$test_root/runtime" PULSE_STATE_PATH="$test_root/state"
mkdir -m700 "$test_root/runtime" "$test_root/state"
export PULSE_SERVER="unix:$test_root/pulse.sock" VESPER_MPV_AO=pulse WEBKIT_DISABLE_DMABUF_RENDERER=1
export VESPER_WEBDRIVER_URL=http://127.0.0.1:4486 VESPER_WEBDRIVER_SESSION="$test_root/session.json"
pulseaudio -n --daemonize=no --exit-idle-time=-1 --log-level=error --load="module-native-protocol-unix socket=$test_root/pulse.sock auth-anonymous=1" --load="module-null-sink sink_name=neisora_two" > "$test_root/pulse.log" 2>&1 &
pulse_pid=$!
driver_pid=''
cleanup(){
    result=$?
    if test "$result" -ne 0; then tail -n 100 "$test_root/driver.log" "$test_root/pulse.log" >&2 || true; fi
    if test -n "$driver_pid"; then kill -- "-$driver_pid" 2>/dev/null || true; wait "$driver_pid" 2>/dev/null || true; fi
    kill "$pulse_pid" 2>/dev/null || true; wait "$pulse_pid" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP
python3 - <<'PY_READY'
import os,time
from pathlib import Path
for _ in range(100):
 if Path(os.environ['PULSE_SERVER'][5:]).exists():break
 time.sleep(.1)
else:raise SystemExit('Private Pulse server failed')
PY_READY
export NEISORA_OUTPUT_MODULE=$(pactl load-module module-null-sink sink_name=neisora_one)
setsid tauri-driver --port 4486 --native-port 4487 > "$test_root/driver.log" 2>&1 &
driver_pid=$!
python3 - <<'PY_READY'
import time,urllib.request
for _ in range(100):
 try:urllib.request.urlopen('http://127.0.0.1:4486/status',timeout=1).close();break
 except OSError:time.sleep(.1)
else:raise SystemExit('Output test WebDriver failed')
PY_READY
python3 tools/audio_output_production.py
