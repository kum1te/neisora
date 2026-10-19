"""Read the actual player position only inside a disposable production test profile."""
import json
import os
from pathlib import Path
import socket
import time


def wait_position(target, seconds=10):
    profile = Path(os.environ['NEISORA_SMOKE_CONFIG'])
    assert str(profile).startswith('/tmp/neisora-'), 'Isolated test profile required'
    connections = list(profile.rglob('mpv-*.sock'))
    assert len(connections) == 1, connections
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(3)
            connection.connect(str(connections[0]))
            connection.sendall(b'{"command":["get_property","time-pos"],"request_id":91}\n')
            with connection.makefile() as lines:
                for line in lines:
                    result = json.loads(line)
                    if result.get('request_id') == 91:
                        break
        if result.get('error') == 'success' and abs(result['data'] - target) <= .8:
            return
        time.sleep(.1)
    raise AssertionError(('Actual mpv seek did not reach the final slider target', result))
