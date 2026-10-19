"""Prepare checksum-pinned, project-local AppImage build tools and finalizer."""
import json
import os
from pathlib import Path
import shlex
import shutil
from runtime_download import fetch

root = Path(__file__).resolve().parents[1]
target = Path(os.environ.get('CARGO_TARGET_DIR', str(root/'src-tauri/target')))
output = target/'.tauri'
output.mkdir(parents=True,exist_ok=True)
manifest = json.loads((root/'tools/appimage-tools.json').read_text())
for name, pin in manifest.items():
    source = fetch(pin['url'], 'appimage-'+name+'-'+pin['sha256'][:16], pin['sha256'])
    shutil.copyfile(source,output/name)
    (output/name).chmod(0o755)
shim=output/'linuxdeploy-plugin-appimage.AppImage'
shim.write_text('#!/bin/sh\nexec python3 '+shlex.quote(str(root/'tools/appimage-output.py'))+' '+shlex.quote(str(output/'neisora-appimage-packer'))+' "$@"\n')
shim.chmod(0o755)
print('Prepared pinned AppImage tools in the project target directory')
