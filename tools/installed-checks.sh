#!/bin/sh
set -eu
test_root=$(mktemp -d /tmp/neisora-installed-XXXXXX)
export XDG_CONFIG_HOME="$test_root/config" XDG_DATA_HOME="$test_root/data"
export NEISORA_SMOKE_CONFIG="$XDG_CONFIG_HOME"
export VESPER_MPV_AO=null WEBKIT_DISABLE_DMABUF_RENDERER=1
export NEISORA_TEST_BINARY=${NEISORA_TEST_BINARY:-/usr/bin/neisora}
smoke_port=${NEISORA_SMOKE_PORT:-4456}
case "$smoke_port" in ''|0*|*[!0-9]*) echo 'Invalid smoke port' >&2; exit 1;; esac
if test "$smoke_port" -lt 1024 || test "$smoke_port" -gt 65534; then echo 'Smoke port out of range' >&2; exit 1; fi
export VESPER_WEBDRIVER_URL="http://127.0.0.1:$smoke_port"
export VESPER_WEBDRIVER_SESSION="$test_root/session.json"
if test -n "${NEISORA_NATIVE_DRIVER:-}"; then set -- --native-driver "$NEISORA_NATIVE_DRIVER"; else set --; fi
# Each restart uses a fresh native driver too: older bundled WebKit drivers
# can exit after their first session and cannot be reused by tauri-driver.
start_driver() {
    setsid tauri-driver --port "$smoke_port" --native-port "$((smoke_port + 1))" "$@" >> "$test_root/driver.log" 2>&1 &
    driver_pid=$!
    python3 - <<'PY_READY'
import os,time,urllib.request
for _ in range(100):
    try:
        urllib.request.urlopen(os.environ['VESPER_WEBDRIVER_URL']+'/status',timeout=1).close();break
    except OSError:time.sleep(.1)
else:raise SystemExit('WebDriver did not start')
PY_READY
}
stop_driver() {
    python3 - "$driver_pid" <<'PY_STOP'
import os,signal,sys
try:os.killpg(int(sys.argv[1]),signal.SIGTERM)
except ProcessLookupError:pass
PY_STOP
    wait "$driver_pid" 2>/dev/null || true
}
cleanup() {
    result=$?
    if test "$result" -ne 0; then tail -n 100 "$test_root/driver.log" >&2 || true; fi
    stop_driver
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
start_driver "$@"
if test "${NEISORA_SMOKE_LOCAL:-0}" = 1; then
    python3 tools/local_production.py
    stop_driver
    start_driver "$@"
    python3 tools/session_production.py
elif test "${NEISORA_SMOKE_ONLINE:-0}" = 1; then
    python3 tools/production_smoke.py --create --mpv-restart --crash-cleanup
else
    # Cloud runner IPs may be rejected by YouTube. Provider-dependent playback
    # is a separate explicit release smoke test on a normal network.
    python3 tools/production_smoke.py --create --offline
fi
