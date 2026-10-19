"""Finalize scoped ELF search paths before the pinned output plugin packs an image.

Do not export the bundled library directory to child system tools: a newer Bash,
Mesa driver or ImageMagick may require newer symbols than our Ubuntu baseline.
"""
import os
from pathlib import Path
import stat
import subprocess
import sys
from appimage_policy import HOST_LIBRARIES

packer, *arguments = sys.argv[1:]
directory = None
for n, argument in enumerate(arguments):
    if argument.startswith('--appdir='): directory = argument.split('=', 1)[1]
    elif argument == '--appdir' and n+1 < len(arguments): directory = arguments[n+1]
if directory:
    root = Path(directory).resolve()
    if not (root/'usr/bin/neisora').is_file(): raise SystemExit('Unexpected AppDir')
    # These libraries form the interface to the host graphics/input driver stack.
    prefixes = tuple(name.split('.so',1)[0]+'.so.' for name in HOST_LIBRARIES)
    for path in (root/'usr/lib').rglob('*'):
        if (path.name.startswith(prefixes) or path.name.startswith('libpulsecommon-')) and not path.is_dir():
            path.unlink()
    for path in (root/'usr').rglob('*'):
        if path.is_symlink() or not path.is_file(): continue
        with path.open('rb') as source:
            header = source.read(20)
            if header[:4] != b'\x7fELF': continue
            # Relocatable .o files are not runtime executables/shared libraries.
            byte_order = 'little' if header[5] == 1 else 'big'
            if int.from_bytes(header[16:18], byte_order) not in (2, 3): continue
        relative = os.path.relpath(root/'usr/lib', path.parent)
        wanted = '$ORIGIN' if relative == '.' else '$ORIGIN/'+relative
        path.chmod(stat.S_IMODE(path.stat().st_mode) | stat.S_IWUSR)
        # RUNPATH is deliberately non-transitive: a host Mesa driver must resolve
        # its own dependencies from the host, not inherit the executable's RPATH.
        subprocess.run(['patchelf','--set-rpath',wanted,str(path)],check=True)
    (root/'AppRun.wrapped').unlink(missing_ok=True)
    launcher = Path(__file__).with_name('appimage-launcher.sh')
    (root/'AppRun').write_bytes(launcher.read_bytes())
    (root/'AppRun').chmod(0o755)
    print('AppImage: scoped ELF paths; host graphics libraries; no global library override',flush=True)
os.environ['APPIMAGE_EXTRACT_AND_RUN']='1'
os.execv(packer,[packer,*arguments])
