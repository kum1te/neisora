"""Collect public dependency licenses and a package inventory for Linux distribution."""
import argparse
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--target', default='x86_64-unknown-linux-gnu')
args = parser.parse_args()
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', args.target, '--manifest-path', str(root/'src-tauri/Cargo.toml')]))
output = root/'src-tauri/resources/licenses'
output.mkdir(parents=True, exist_ok=True)
(output/'NEISORA-LICENSE.txt').write_text((root/'LICENSE').read_text(encoding="utf-8"), encoding='utf-8')
(output/'NEISORA-COPYRIGHT.txt').write_text((root/'COPYRIGHT').read_text(encoding='utf-8'), encoding='utf-8')
inventory = []
notices = ['neisora dependency notices\n\nOwn application code: GPL-3.0-only. Third-party components retain their licenses.\n']

def add(name, version, license_name, directory, source):
    inventory.append({'name':name, 'version':version, 'license':license_name, 'source':source})
    notices.append(f'\n{"="*72}\n{name} {version}\nLicense: {license_name}\nSource: {source}\n')
    files = set()
    for pattern in ['LICENSE*', 'COPYING*', 'COPYRIGHT', 'NOTICE*', 'license*', 'licenses/*', 'NEISORA-PATCH.md']:
        files.update(directory.glob(pattern))
    for path in sorted(files):
        if path.is_file() and path.stat().st_size <= 2*1024*1024:
            try:
                text = path.read_text(encoding="utf-8")
            except (UnicodeError, OSError):
                continue
            notices.append(f'\n--- {path.relative_to(directory)} ---\n{text}\n')

for package in sorted(metadata['packages'], key=lambda p:(p['name'],p['version'])):
    if package['name'] == 'neisora': continue
    source=package.get('repository') or package.get('source') or 'Local source'
    if package['source'] is None:source+=' (neisora security backport; see NEISORA-PATCH.md)'
    add(package['name'], package['version'], package.get('license') or 'See included source license', Path(package['manifest_path']).parent,source)

seen = set()
for path in (root/'node_modules/.pnpm').glob('*/node_modules/*/package.json'):
    package = json.loads(path.read_text(encoding="utf-8"))
    key = (package.get('name'),package.get('version'))
    if key in seen: continue
    seen.add(key)
    source = package.get('repository') or package.get('homepage') or 'https://www.npmjs.com/package/'+str(key[0])
    add(key[0],key[1],package.get('license','See included source license'),path.parent,str(source))
for path in (root/'node_modules/.pnpm').glob('*/node_modules/@*/*/package.json'):
    package = json.loads(path.read_text(encoding="utf-8")); key=(package.get('name'),package.get('version'))
    if key in seen: continue
    seen.add(key)
    add(key[0],key[1],package.get('license','See included source license'),path.parent,str(package.get('repository') or package.get('homepage') or 'https://www.npmjs.com/package/'+str(key[0])))
(output/'THIRD_PARTY_NOTICES.txt').write_text('\n'.join(notices), encoding='utf-8')
(output/'dependencies.json').write_text(json.dumps(inventory,ensure_ascii=False,indent=2)+'\n', encoding='utf-8')
print(f'Collected license notices for {len(inventory)} dependencies')
