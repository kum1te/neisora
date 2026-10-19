"""Fetch pinned public helpers for desktop packages. No runtime auto-downloads."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import tarfile
import urllib.request
import zipfile
import subprocess

root = Path(__file__).resolve().parents[1]
manifest = json.loads((root/'tools/runtime-manifest.json').read_text())
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--arch', choices=['x86_64', 'aarch64'], default=platform.machine())
parser.add_argument('--platform', choices=['linux', 'windows'], default='windows' if os.name == 'nt' else 'linux')
args = parser.parse_args()
arch = {'x86_64': 'x64', 'aarch64': 'arm64'}[args.arch]
output = root/'src-tauri/resources/runtime'
# This is generated staging data: rebuild it to prevent cross-platform leftovers.
if output.exists(): shutil.rmtree(output)
output.mkdir(parents=True)
from runtime_download import fetch

version = manifest['node']['version']
if args.platform == 'linux':
    stem = f'node-v{version}-linux-{arch}'
    archive = fetch(f'https://nodejs.org/dist/v{version}/{stem}.tar.xz', stem+'.tar.xz', manifest['node']['sha256'][args.arch])
    with tarfile.open(archive) as tar:
        for member, destination, limit in [('bin/node', 'node', 160*1024*1024), ('LICENSE', 'NODE-LICENSE', 2*1024*1024)]:
            entry = tar.getmember(f'{stem}/{member}')
            if not entry.isfile() or entry.size > limit:
                raise ValueError('Unexpected archive member')
            with tar.extractfile(entry) as source, (output/destination).open('wb') as target:
                shutil.copyfileobj(source, target)
else:
    if args.arch != 'x86_64':
        raise ValueError('Windows runtime is currently validated only for x86_64')
    # Never leave Linux helpers in an incrementally reused Windows staging folder.
    for name in ['node', 'mpv', 'python3', 'SOURCE-CHECK.txt']:
        (output/name).unlink(missing_ok=True)
    stem = f'node-v{version}-win-x64'
    archive = fetch(f'https://nodejs.org/dist/v{version}/{stem}.zip', stem+'.zip', manifest['node']['windows_x64_sha256'])
    with zipfile.ZipFile(archive) as z:
        for member, destination in [('node.exe', 'node.exe'), ('LICENSE', 'NODE-LICENSE')]:
            with z.open(f'{stem}/{member}') as source, (output/destination).open('wb') as target:
                shutil.copyfileobj(source, target)
    python = manifest['python-windows']
    filename = f'python-{python["version"]}-embed-amd64.zip'
    archive = fetch(f'https://www.python.org/ftp/python/{python["version"]}/{filename}', filename, python['sha256'])
    target = output/'python'
    if target.exists(): shutil.rmtree(target)
    target.mkdir()
    with zipfile.ZipFile(archive) as z:
        for member in z.infolist():
            if '/' in member.filename or '\\' in member.filename or member.filename in ['.', '..'] or member.file_size > 30*1024*1024:
                raise ValueError('Unexpected Python archive member')
            with z.open(member) as source, (target/member.filename).open('wb') as destination:
                shutil.copyfileobj(source, destination)
    mpv = manifest['mpv-windows']
    archive = fetch(mpv['url'], mpv['filename'], mpv['sha256'])
    # Extract only exact public binary members, never upstream installers/updaters.
    sevenzip = shutil.which('7z') or shutil.which('7za')
    if not sevenzip: raise ValueError('7-Zip is required to prepare the Windows runtime')
    for name in ['mpv.exe', 'd3dcompiler_43.dll']:
        with (output/name).open('wb') as destination:
            subprocess.run([sevenzip, 'x', '-so', str(archive), name], stdout=destination, check=True)
        if not (output/name).stat().st_size: raise ValueError('Empty mpv archive member')
    (output/'MPV-SOURCE.txt').write_text(
        'mpv Windows build: '+mpv['url']+'\n'
        'Build scripts: https://github.com/shinchiro/mpv-winbuild-cmake/tree/'+mpv['release']+'\n'
        'mpv source: https://github.com/mpv-player/mpv/commit/'+mpv['commit']+'\n'
        'This development artifact is not a public release. Corresponding-source packaging remains a release gate.\n', encoding='utf-8')
version = manifest['yt-dlp']['version']
base = f'https://github.com/yt-dlp/yt-dlp/releases/download/{version}'
shutil.copyfile(fetch(base+'/yt-dlp', f'yt-dlp-{version}', manifest['yt-dlp']['sha256']), output/'yt-dlp')
for name in ['LICENSE', 'THIRD_PARTY_LICENSES.txt']:
    source = fetch(f'https://raw.githubusercontent.com/yt-dlp/yt-dlp/{version}/{name}', f'yt-dlp-{version}-{name}')
    shutil.copyfile(source, output/f'YT-DLP-{name}')
if args.platform == 'linux':
    for name in ['node', 'yt-dlp']: (output/name).chmod(0o755)
(output/'versions.json').write_text(json.dumps({'platform': args.platform, 'arch': args.arch, 'node':manifest['node']['version'], 'yt-dlp':version}, indent=2)+'\n')
print(f'Prepared Node {manifest["node"]["version"]} and yt-dlp {version} for {args.arch}')
