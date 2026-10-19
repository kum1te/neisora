"""Download hash-verified, exact Ubuntu sources recorded by AppImage staging.

Requires matching deb-src indexes (apt-get update). Archives are release artifacts,
never source-tree inputs. Fail rather than publish without corresponding sources.
"""
import concurrent.futures,hashlib,json,subprocess,tarfile,urllib.request
from pathlib import Path
root=Path(__file__).resolve().parents[1]
version=json.loads((root/'package.json').read_text())['version']
manifest=root/'artifacts/appimage-runtime/licenses/system-components.txt'
cache=root/'artifacts/appimage-sources';cache.mkdir(parents=True,exist_ok=True)
packages={tuple(line.split()[2:4]) for line in manifest.read_text().splitlines() if line.strip()}
def fields(block):
 result={};key=None
 for line in block.splitlines():
  if line.startswith((' ','\t')) and key:result[key]+='\n'+line.strip()
  elif ':' in line:key,value=line.split(':',1);result[key]=value.strip()
 return result
records=[];seen=set()
for name,source_version in sorted(packages):
 raw=subprocess.check_output(['apt-cache','showsrc','--only-source',name],text=True)
 matches=[v for block in raw.split('\n\n') if (v:=fields(block)).get('Version')==source_version]
 if not matches:raise SystemExit('Missing exact deb-src metadata for '+name+'='+source_version)
 info=matches[0]
 for line in info['Checksums-Sha256'].splitlines():
  if not line.strip():continue
  digest,size,filename=line.split()
  if filename in seen:continue
  if Path(filename).name!=filename:raise SystemExit('Unsafe source filename')
  seen.add(filename)
  records.append({'package':name,'version':source_version,'file':filename,'size':int(size),'sha256':digest,'url':'https://archive.ubuntu.com/ubuntu/'+info['Directory']+'/'+filename})
def file_hash(path):
 digest=hashlib.sha256()
 with path.open('rb') as source:
  while block:=source.read(1024*1024):digest.update(block)
 return digest.hexdigest()
def fetch(record):
 path=cache/record['file']
 def valid():return path.is_file() and path.stat().st_size==record['size'] and file_hash(path)==record['sha256']
 if not valid():
  temporary=path.with_name(path.name+'.part')
  with urllib.request.urlopen(record['url'],timeout=120) as response,temporary.open('wb') as out:
   count=0
   while block:=response.read(1024*1024):
    count+=len(block)
    if count>record['size']:raise RuntimeError('Oversized source download')
    out.write(block)
  temporary.replace(path)
  if not valid():raise RuntimeError('Source hash mismatch: '+record['file'])
 return path
print('Exact Ubuntu sources:',len(packages),'packages;',round(sum(r['size'] for r in records)/1024**2,1),'MiB',flush=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:paths=list(pool.map(fetch,records))
release=root/'release';release.mkdir(exist_ok=True)
index=release/'appimage-sources.json';index.write_text(json.dumps(records,indent=2)+'\n')
# Most files are already compressed; avoid expensive recompression.
with tarfile.open(release/f'neisora-{version}-appimage-sources.tar','w') as archive:
 archive.add(index,arcname=index.name)
 for path in sorted(paths):archive.add(path,arcname=path.name)
print(f'Corresponding AppImage sources: {len(packages)} packages, {len(paths)} verified archives')
