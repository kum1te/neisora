"""Build a pinned Khronos Vulkan loader locally for mpv's Windows import table.

A machine without GPU drivers must still be able to start audio-only mpv.
This is an app-local DLL, not a system runtime installer or GPU driver.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import zipfile
from runtime_download import fetch

if os.name != 'nt': raise SystemExit('Build this helper on Windows/MSVC')
root = Path(__file__).resolve().parents[1]
manifest = json.loads((root/'tools/runtime-manifest.json').read_text(encoding='utf-8'))['vulkan-windows']
stage = root/'artifacts/vulkan-build'
if stage.exists(): shutil.rmtree(stage)
stage.mkdir(parents=True)
sources = {}
for component, repo in [('loader','Vulkan-Loader'), ('headers','Vulkan-Headers')]:
    pin = manifest[component]
    url = f'https://codeload.github.com/KhronosGroup/{repo}/zip/{pin["commit"]}'
    archive = fetch(url, repo+'-'+pin['commit']+'.zip', pin['sha256'])
    with zipfile.ZipFile(archive) as z:
        for entry in z.infolist():
            destination = (stage/entry.filename).resolve()
            if not destination.is_relative_to(stage) or entry.file_size > 50*1024*1024:
                raise ValueError('Unsafe source archive member')
        z.extractall(stage)
    sources[component] = stage/(repo+'-'+pin['commit'])
headers = stage/'headers-install'
def run(*args): subprocess.run(list(map(str,args)), check=True)
run('cmake','-S',sources['headers'],'-B',stage/'headers-build','-A','x64',f'-DCMAKE_INSTALL_PREFIX={headers}', '-DVULKAN_HEADERS_ENABLE_TESTS=OFF')
run('cmake','--build',stage/'headers-build','--config','Release','--target','install')
run('cmake','-S',sources['loader'],'-B',stage/'loader-build','-A','x64',f'-DVULKAN_HEADERS_INSTALL_DIR={headers}',f'-DCMAKE_PREFIX_PATH={headers}', '-DUPDATE_DEPS=OFF','-DBUILD_TESTS=OFF','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded')
run('cmake','--build',stage/'loader-build','--config','Release','--parallel','2')
output = root/'src-tauri/resources/runtime'
shutil.copy2(stage/'loader-build/loader/Release/vulkan-1.dll', output/'vulkan-1.dll')
notices = output/'vulkan-licenses'
notices.mkdir(exist_ok=True)
for component, source in sources.items():
    for pattern in ['LICENSE*','LICENSES/*']:
        for path in source.glob(pattern):
            if path.is_file(): shutil.copy2(path,notices/(component+'-'+path.name))
(notices/'SOURCE.txt').write_text(json.dumps(manifest,indent=2)+'\nSources: https://github.com/KhronosGroup/Vulkan-Loader and https://github.com/KhronosGroup/Vulkan-Headers\nBuilt using tools/prepare-vulkan.py with MSVC, Apache-2.0.\n',encoding='utf-8')
print('Prepared app-local Vulkan loader from pinned Khronos sources')
