"""Linux process-tree sampler. PSS avoids counting shared WebKit pages twice."""
import argparse
import json
import os
from pathlib import Path
import time


def snapshot(root):
    processes = {}
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            fields = path.joinpath('stat').read_text().rsplit(')', 1)[1].split()
            processes[int(path.name)] = (int(fields[1]), int(fields[11]) + int(fields[12]))
        except (OSError, IndexError, ValueError):
            pass
    selected = {root}
    while True:
        children = {pid for pid, (parent, _) in processes.items() if parent in selected}
        added = children - selected
        if not added:
            break
        selected.update(added)
    rows = {}
    for pid in selected:
        if pid not in processes:
            continue
        try:
            path = Path('/proc') / str(pid)
            rollup = {line.split(':')[0]: int(line.split()[1]) for line in path.joinpath('smaps_rollup').read_text().splitlines() if line.startswith(('Rss:', 'Pss:'))}
            rows[pid] = {'ticks': processes[pid][1], 'pss_kib': rollup['Pss'], 'rss_kib': rollup['Rss'], 'fds': len(list(path.joinpath('fd').iterdir())), 'threads': len(list(path.joinpath('task').iterdir()))}
        except (OSError, ValueError, KeyError):
            pass
    return rows


def measure(root, seconds=6, interval=.5):
    start = time.monotonic()
    previous = snapshot(root)
    ticks = 0
    samples = []
    while time.monotonic() - start < seconds:
        time.sleep(interval)
        current = snapshot(root)
        ticks += sum(max(0, row['ticks'] - previous.get(pid, row)['ticks']) for pid, row in current.items())
        samples.append({key: sum(row[key] for row in current.values()) for key in ('pss_kib', 'rss_kib', 'fds', 'threads')})
        previous = current
    elapsed = time.monotonic() - start
    return {'seconds': round(elapsed, 2), 'cpu_one_core_percent': round(ticks / os.sysconf('SC_CLK_TCK') / elapsed * 100, 2), 'pss_mib_min': round(min(r['pss_kib'] for r in samples) / 1024, 1), 'pss_mib_max': round(max(r['pss_kib'] for r in samples) / 1024, 1), 'fds_final': samples[-1]['fds'], 'threads_final': samples[-1]['threads'], 'processes_final': len(previous)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int)
    parser.add_argument('--seconds', type=float, default=6)
    args = parser.parse_args()
    print(json.dumps(measure(args.pid, max(.5, args.seconds)), indent=2))
