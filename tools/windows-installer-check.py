"""Exercise the actual NSIS hook before compiling Rust, including checksum refusal."""
import json
import os
from pathlib import Path
import subprocess
import zipfile
from runtime_download import fetch

if os.name != 'nt':
    raise SystemExit('Requires a disposable Windows CI VM')
root = Path(__file__).resolve().parents[1]
work = root / 'artifacts/installer-preflight'
work.mkdir(parents=True, exist_ok=True)
archive = fetch('https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip',
                'nsis-3.11.zip', 'c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1')
with zipfile.ZipFile(archive) as z:
    if sum(i.file_size for i in z.infolist()) > 100 * 1024 * 1024:
        raise ValueError('Unexpected NSIS tool size')
    for info in z.infolist():
        path = Path(info.filename.replace('\\', '/'))
        if path.is_absolute() or '..' in path.parts or ':' in info.filename:
            raise ValueError('Unexpected NSIS tool path')
    z.extractall(work / 'tools')
compiler = work / 'tools/nsis-3.11/makensis.exe'
source = root / 'src-tauri/resources/runtime/install-mpv.ps1'
assert source.is_file()
expected = json.loads((root / 'tools/runtime-manifest.json').read_text())['mpv-windows']['sha256']

def quoted(path):
    return '"' + str(path).replace('$', '$$').replace('"', '$\\"') + '"'

for negative in [False, True]:
    label = 'bad-hash' if negative else 'valid'
    install = work / ('install-' + label)
    if install.exists():
        raise RuntimeError('Refusing to reuse a preflight installation')
    script = work / (label + '.ps1')
    content = source.read_text(encoding='utf-8-sig')
    if negative:
        assert expected in content
        content = content.replace(expected, '0' * 64)
    script.write_text(content, encoding='utf-8-sig')
    setup = work / (label + '.exe')
    nsi = work / (label + '.nsi')
    nsi.write_text('\n'.join([
        'Unicode true', 'Name "neisora installer preflight"',
        'OutFile ' + quoted(setup), 'InstallDir ' + quoted(install),
        'RequestExecutionLevel user', 'SilentInstall silent',
        '!include "LogicLib.nsh"',
        '!include ' + quoted(root / 'src-tauri/windows-installer.nsh'),
        'Section', 'SetOutPath "$INSTDIR\\runtime"',
        'File /oname=install-mpv.ps1 ' + quoted(script),
        '!insertmacro NSIS_HOOK_POSTINSTALL', 'SectionEnd',
    ]), encoding='utf-8-sig')
    subprocess.run([str(compiler), '/V2', str(nsi)], check=True, timeout=60)
    result = subprocess.run([str(setup), '/S', '/D=' + str(install)], timeout=330)
    log = install / 'runtime/install-mpv.log'
    detail = log.read_text(encoding='utf-8-sig') if log.exists() else ''
    if negative:
        assert result.returncode != 0, 'Corrupt pin must stop the installer'
        assert not (install / 'runtime/mpv.exe').exists(), 'Corrupt payload was installed'
        assert 'checksum mismatch' in detail, detail
        print('NSIS refuses a mismatched media SHA-256 before extracting or installing: PASS', flush=True)
    else:
        if result.returncode:
            raise RuntimeError('NSIS media installation failed: ' + detail)
        for name in ['mpv.exe', 'd3dcompiler_43.dll']:
            assert (install / 'runtime' / name).stat().st_size > 0
        assert not (install / 'runtime/7zr.exe').exists()
        print('Actual NSIS hook downloads, verifies and installs media engine: PASS', flush=True)
