"""Stage system mpv/Python from the Ubuntu 22.04 build root for AppImage.

The files are copied only into the AppImage, never native deb/rpm packages.
linuxdeploy collects ELF dependencies and rewrites their relative search paths.
"""
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
stage = root/'artifacts/appimage-runtime'
if stage.exists(): shutil.rmtree(stage)
stage.mkdir(parents=True)
if platform.machine() != 'x86_64':
    raise SystemExit('This AppImage staging is validated for Linux x86_64 only')
if sys.version_info[:2] != (3, 10):
    raise SystemExit('AppImage must be staged with Ubuntu 22.04 Python 3.10')
shutil.copy2('/usr/bin/mpv', stage/'mpv')
python = stage/'python'
(python/'bin').mkdir(parents=True)
shutil.copy2('/usr/bin/python3.10', python/'bin/python3.10')
shutil.copytree('/usr/lib/python3.10', python/'lib/python3.10',
                ignore=shutil.ignore_patterns('__pycache__', 'test', 'tests', 'idlelib', 'tkinter', 'ensurepip', 'lib2to3', 'dist-packages', 'config-*'))
# Keep distribution copyright/source URLs with the copied system components.
notices = stage/'licenses'
notices.mkdir()
packages = {'mpv', 'python3.10', 'libpython3.10-stdlib', 'libpython3.10-minimal'}
# Include the copyrights of transitive ELF dependencies and dynamically loaded GTK modules.
elfs = [Path('/usr/bin/mpv'), Path('/usr/bin/python3.10'),
        Path(os.environ.get('CARGO_TARGET_DIR', str(root/'src-tauri/target')))/'release/neisora',
        root/'src-tauri/resources/runtime/node']
for directory in ['/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1', '/usr/lib/x86_64-linux-gnu/gdk-pixbuf-2.0', '/usr/lib/x86_64-linux-gnu/gio/modules', '/usr/lib/python3.10/lib-dynload']:
    elfs.extend(p for p in Path(directory).rglob('*') if p.is_file())
files = set()
# Keep glibc, C++ runtimes and graphics-driver-facing libraries on the host.
# Everything else in the ELF closure is explicitly staged, including libraries
# that linuxdeploy's generic exclusion list assumes a full desktop has installed.
from appimage_policy import HOST_LIBRARIES
baseline = HOST_LIBRARIES
closure = {}
for path in elfs:
    try:
        with path.open('rb') as source:
            if source.read(4) != b'\x7fELF': continue
        files.add(str(path))
        result = subprocess.run(['ldd', str(path)], capture_output=True, text=True, check=False)
        for line in result.stdout.splitlines():
            value = line.split('=>')[-1].strip().split(' ')[0]
            if value.startswith('/'):
                files.add(value)
                name = Path(value).name
                if name not in baseline and not name.startswith(('libnss_', 'libpulsecommon-')):
                    closure['/usr/lib/'+name] = value
        if 'not found' in result.stdout:
            raise SystemExit('Missing build dependency for '+str(path))
    except OSError: continue
for path in sorted(files):
    # ldd often reports /lib while dpkg owns /usr/lib on merged-/usr systems.
    # Query both spellings so dependency licenses and sources are not omitted.
    for candidate in {path, str(Path(path).resolve())}:
        result = subprocess.run(['dpkg-query', '-S', candidate], capture_output=True, text=True, check=False)
        for line in result.stdout.splitlines():
            if ': ' in line: packages.add(line.rsplit(': ', 1)[0])
shutil.copytree('/usr/share/common-licenses', notices/'common-licenses')
for package in packages:
    path = Path('/usr/share/doc')/package.split(':')[0]/'copyright'
    if path.exists(): shutil.copy2(path, notices/(package.replace(':', '_')+'-copyright'))
(notices/'system-components.txt').write_text(subprocess.check_output(
    ['dpkg-query', '-W', '-f=${Package} ${Version} ${source:Package} ${source:Version} ${Homepage}\\n', *sorted(packages)], text=True))
subprocess.run([sys.executable, str(root/'tools/prepare-appimage-runtime.py'), str(notices/'runtime')], check=True)
(notices/'SOURCE.txt').write_text(
    'System components are from Ubuntu 22.04 (jammy and jammy-updates/security).\n'
    'Exact binary and corresponding source package versions: system-components.txt.\n'
    'Corresponding source archives: https://archive.ubuntu.com/ubuntu/pool/ and https://security.ubuntu.com/ubuntu/pool/\n'
    'Use apt-get source SOURCE_PACKAGE=SOURCE_VERSION with matching Ubuntu deb-src repositories.\n')
subprocess.run([sys.executable, str(root/'tools/prepare-appimage-tools.py')], check=True)
config = {'bundle': {'useLocalToolsDir': True, 'linux': {'appimage': {'files': {
    '/usr/bin/mpv': str(stage/'mpv'),
    **closure,
    '/usr/lib/neisora/runtime/python': str(python),
    '/usr/lib/neisora/licenses/appimage': str(notices),
}}}}}
(root/'artifacts/appimage.conf.json').write_text(json.dumps(config, indent=2)+'\n')
print('Staged mpv, isolated Python 3.10, and their notices for AppImage')
