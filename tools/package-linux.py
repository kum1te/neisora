"""Package an Ubuntu-baseline binary for portable use and Arch Linux (system mpv/Python)."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import time

root = Path(__file__).resolve().parents[1]
version = json.loads((root/'package.json').read_text())['version']
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', required=True)
parser.add_argument('--output', default=str(root/'release'))
args = parser.parse_args()
output = Path(args.output).resolve(); output.mkdir(parents=True,exist_ok=True)
epoch = int(os.environ.get('SOURCE_DATE_EPOCH',time.time()))

def archive(directory, destination, prefix='', zstd=False):
    def owner(info):
        info.uid=info.gid=0; info.uname=info.gname='root'; info.mtime=epoch
        return info
    if zstd:
        compressed=subprocess.Popen(['zstd','-q','-T2','-10','-f','-o',str(destination)],stdin=subprocess.PIPE)
        with tarfile.open(fileobj=compressed.stdin,mode='w|') as tar:
            for item in sorted(directory.iterdir()):tar.add(item,arcname=item.name,filter=owner)
        compressed.stdin.close()
        if compressed.wait():raise RuntimeError('Compression failed')
    else:
        with destination.open('wb') as raw, gzip.GzipFile(fileobj=raw, filename='', mode='wb', mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed,mode='w|') as tar:tar.add(directory,arcname=prefix,filter=owner)

with tempfile.TemporaryDirectory(prefix='neisora-package-') as work:
    stage=Path(work)/'neisora';stage.mkdir()
    shutil.copy2(args.binary,stage/'neisora')
    shutil.copytree(root/'src-tauri/resources/runtime',stage/'runtime')
    shutil.copytree(root/'src-tauri/resources/licenses',stage/'licenses')
    for file in ['LICENSE','README.md']:shutil.copy2(root/file,stage/file)
    shutil.copy2(root/'icon.svg',stage/'neisora.svg')
    sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
    dirty=bool(subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],cwd=root,text=True).strip())
    (stage/'build.json').write_text(json.dumps({'version':version,'arch':'x86_64','baseline':'Ubuntu 22.04 / glibc 2.35','commit':sha,'dirty':dirty,'builtAt':epoch},indent=2)+'\n')
    archive(stage,output/'neisora-linux-x86_64.tar.gz','neisora')
    pkg=Path(work)/'arch';(pkg/'usr/bin').mkdir(parents=True)
    lib=pkg/'usr/lib/neisora';lib.mkdir(parents=True)
    shutil.copy2(stage/'neisora',pkg/'usr/bin/neisora')
    for name in ['runtime','licenses']:shutil.copytree(stage/name,lib/name)
    shutil.copy2(stage/'build.json',lib/'build.json')
    docs=pkg/'usr/share/licenses/neisora';docs.mkdir(parents=True);shutil.copy2(root/'LICENSE',docs/'LICENSE')
    icons=pkg/'usr/share/icons/hicolor/256x256/apps';icons.mkdir(parents=True);shutil.copy2(root/'src-tauri/icons/icon.png',icons/'neisora.png')
    apps=pkg/'usr/share/applications';apps.mkdir(parents=True)
    (apps/'neisora.desktop').write_text('[Desktop Entry]\nType=Application\nName=neisora\nExec=neisora\nIcon=neisora\nCategories=AudioVideo;Audio;Player;\nStartupWMClass=neisora\nTerminal=false\n')
    size=sum(p.stat().st_size for p in pkg.rglob('*') if p.is_file())
    (pkg/'.PKGINFO').write_text(f'pkgname = neisora\npkgbase = neisora\npkgver = {version}-1\npkgdesc = YouTube Music with a custom desktop interface\nurl = https://github.com/kum1te/neisora\nbuilddate = {epoch}\npackager = kum1te\nsize = {size}\narch = x86_64\nlicense = GPL-3.0-only\ndepend = glibc>=2.35\ndepend = gtk3\ndepend = dbus\ndepend = webkit2gtk-4.1\ndepend = mpv\ndepend = python\noptdepend = imagemagick: wallpaper colors\n')
    entries=['#mtree']
    for path in sorted(pkg.rglob('*')):
        name='./'+path.relative_to(pkg).as_posix()
        name=name.replace('\\','\\134').replace(' ','\\040')
        mode=oct(path.stat().st_mode & 0o777)[2:]
        entry=f'{name} uid=0 gid=0 mode={mode} time={epoch}.0'
        if path.is_dir():entry+=' type=dir'
        else:entry+=f' type=file size={path.stat().st_size} sha256digest={hashlib.sha256(path.read_bytes()).hexdigest()}'
        entries.append(entry)
    with (pkg/'.MTREE').open('wb') as raw,gzip.GzipFile(fileobj=raw,filename='',mode='wb',mtime=epoch) as stream:
        stream.write(('\n'.join(entries)+'\n').encode())
    archive(pkg,output/f'neisora-{version}-1-x86_64.pkg.tar.zst',zstd=True)
for path in output.iterdir():
    if path.is_file():path.chmod(0o644)
print('Created portable and Arch Linux packages')
