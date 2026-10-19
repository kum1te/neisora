"""Stress expanded-player teardown in an isolated fixture instance (after regression.py)."""
import argparse
import json
from statistics import median
from pathlib import Path
import time
from webdriver_client import execute
from resources import snapshot

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('pid', type=int, help='PID of the isolated app process')
parser.add_argument('--cycles', type=int, default=100)
args = parser.parse_args()
assert execute('return typeof window.__VESPER_TEST_INVOKE__ === "function"'), 'Requires fixture instance'
# Logout in audit_regression shows a six-node toast that expires after 6.5 s.
# Dismiss it through the UI before comparing retained DOM across player cycles.
execute('document.querySelector(".toast button")?.click()')
assert execute('return !document.querySelector(".toast")'), 'Fixture notification did not close'
execute('''
window.lifecycleCounts={};window.lifecycleRAF=new Set();
window.lifecycleAdd=EventTarget.prototype.addEventListener;window.lifecycleRemove=EventTarget.prototype.removeEventListener;
EventTarget.prototype.addEventListener=function(type,...args){if(this===window||this===document||this instanceof MediaQueryList)lifecycleCounts[type]=(lifecycleCounts[type]||0)+1;return lifecycleAdd.call(this,type,...args)};
EventTarget.prototype.removeEventListener=function(type,...args){if(this===window||this===document||this instanceof MediaQueryList)lifecycleCounts[type]=(lifecycleCounts[type]||0)-1;return lifecycleRemove.call(this,type,...args)};
window.lifecycleOriginalRAF=requestAnimationFrame;window.lifecycleOriginalCancel=cancelAnimationFrame;
window.requestAnimationFrame=callback=>{let id=lifecycleOriginalRAF(t=>{lifecycleRAF.delete(id);callback(t)});lifecycleRAF.add(id);return id};
window.cancelAnimationFrame=id=>{lifecycleRAF.delete(id);lifecycleOriginalCancel(id)};
window.lifecycleRegistry=Object.getOwnPropertyNames(window).map(k=>Object.getOwnPropertyDescriptor(window,k)?.value).find(v=>v&&typeof v==='object'&&Object.prototype.hasOwnProperty.call(v,'player-state'));
window.lifecycleListeners=()=>lifecycleRegistry?Object.getOwnPropertyNames(lifecycleRegistry).reduce((n,k)=>n+Object.getOwnPropertyNames(lifecycleRegistry[k]).length,0):null;
window.lifecycleBaseline={dom:document.querySelectorAll('*').length,listeners:lifecycleListeners()};
''')
rows = []
try:
    for n in range(args.cycles):
        execute('document.querySelector(".expand-player").click()')
        time.sleep(.08)
        execute('document.querySelector("[aria-label=\\"Свернуть плеер\\"]").click()')
        time.sleep(.08)
        if (n+1) % 10 == 0:
            tree = snapshot(args.pid)
            row = {'cycle': n+1, 'pss_mib': round(sum(r['pss_kib'] for r in tree.values())/1024, 1), 'fds': sum(r['fds'] for r in tree.values()),'processes':[{ 'name':(Path('/proc')/str(pid)/'comm').read_text().strip(),'pss_mib':round(r['pss_kib']/1024,1)} for pid,r in tree.items()]}
            rows.append(row)
            if (n+1) % 25 == 0:
                print(row, flush=True)
    time.sleep(.4)
    result = execute('return {dom:document.querySelectorAll("*").length,baseline:lifecycleBaseline,listeners:lifecycleListeners(),pendingFrames:lifecycleRAF.size,events:lifecycleCounts}')
    if len(rows)>=6:
        initial=median(r['pss_mib'] for r in rows[:3]); final=median(r['pss_mib'] for r in rows[-3:])
        assert final-initial < max(128,initial*.2), {'memory_growth_mib':round(final-initial,1),'samples':rows}
    assert result['baseline']['listeners'] is not None, 'Tauri listener registry unavailable'
    assert result['dom'] == result['baseline']['dom'], result
    assert result['listeners'] == result['baseline']['listeners'], result
    assert result['pendingFrames'] == 0, result
    assert all(n == 0 for n in result['events'].values()), result
    print(json.dumps({'samples': rows, 'teardown': result}, ensure_ascii=False), flush=True)
finally:
    execute('EventTarget.prototype.addEventListener=lifecycleAdd;EventTarget.prototype.removeEventListener=lifecycleRemove;window.requestAnimationFrame=lifecycleOriginalRAF;window.cancelAnimationFrame=lifecycleOriginalCancel')
