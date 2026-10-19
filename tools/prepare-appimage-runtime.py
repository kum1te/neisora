"""Stage pinned AppImage runtime notices and corresponding static libfuse sources."""
import json,shutil,sys,tarfile
from pathlib import Path
from runtime_download import fetch
root=Path(__file__).resolve().parents[1]
manifest=json.loads((root/'tools/appimage-runtime-sources.json').read_text())
output=Path(sys.argv[1]);output.mkdir(parents=True,exist_ok=True)
cache=root/'artifacts/appimage-sources';cache.mkdir(parents=True,exist_ok=True)
notices=[]
for record in manifest['sources']:
 archive=fetch(record['url'],record['file'],record['sha256'])
 shutil.copy2(archive,cache/record['file'])
 with tarfile.open(archive) as source:
  for member in source.getmembers():
   if not member.isfile() or member.size>300_000:continue
   if Path(member.name).name not in ['LICENSE','COPYING','COPYING.LIB','COPYRIGHT','LICENSE.txt']:continue
   notices.append('\n'+record['package']+' / '+member.name+'\n'+source.extractfile(member).read().decode('utf-8'))
for record in manifest['licenses']:
 path=fetch(record['url'],'appimage-runtime-'+record['name']+'-LICENSE',record['sha256'])
 notices.append('\n'+record['name']+'\n'+path.read_text())
(output/'NOTICES.txt').write_text('\n'.join(notices))
(output/'SOURCE.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('Pinned AppImage runtime notices and libfuse corresponding sources staged')
