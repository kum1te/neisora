"""Stage an online Windows installer without redistributing upstream mpv/FFmpeg builds.

The installed NSIS hook downloads the pinned archive directly from its publisher,
checks SHA-256 before extracting exactly two members, and cleans its temporary files.
Run after runtime checks, immediately before tauri build. Re-run prepare-runtime.py
before any further development checks that expect staging mpv.exe.
"""
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
runtime = root / 'src-tauri/resources/runtime'
manifest = json.loads((root / 'tools/runtime-manifest.json').read_text())
mpv = manifest['mpv-windows']
extractor = manifest['sevenzip-windows']
if not extractor['url'].startswith('https://github.com/ip7z/7zip/releases/download/') or len(extractor['sha256']) != 64 or any(c not in '0123456789abcdef' for c in extractor['sha256']):
    raise ValueError('Invalid pinned archive extractor')
if not mpv['url'].startswith('https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/'):
    raise ValueError('Unexpected mpv publisher')
if len(mpv['sha256']) != 64 or any(c not in '0123456789abcdef' for c in mpv['sha256']):
    raise ValueError('Invalid mpv checksum')
script = r'''$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
function Get-PinnedFileHash([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose(); $stream.Dispose() }
}
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('neisora-media-' + [Guid]::NewGuid().ToString('N'))
$stage = 'create temporary directory'
try {
    New-Item -ItemType Directory -Path $temporary | Out-Null
    $archive = Join-Path $temporary 'mpv.7z'
    $stage = 'download media engine'
    Invoke-WebRequest -UseBasicParsing -Uri '__URL__' -OutFile $archive -TimeoutSec 180
    $stage = 'verify media engine checksum'
    if ((Get-PinnedFileHash $archive) -ne '__SHA__') {
        throw 'Media engine checksum mismatch. Installation stopped.'
    }
    # The Windows inbox tar does not include the LZMA codec on supported systems.
    # Fetch a pinned standalone 7zr directly from its publisher; never package it.
    $stage = 'download archive extractor'
    $extractor = Join-Path $temporary '7zr.exe'
    Invoke-WebRequest -UseBasicParsing -Uri '__EXTRACTOR_URL__' -OutFile $extractor -TimeoutSec 120
    if ((Get-PinnedFileHash $extractor) -ne '__EXTRACTOR_SHA__') {
        throw 'Archive extractor checksum mismatch. Installation stopped.'
    }
    $stage = 'extract media engine'
    & $extractor x -y "-o$temporary" $archive mpv.exe d3dcompiler_43.dll
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract media engine' }
    foreach ($name in @('mpv.exe', 'd3dcompiler_43.dll')) {
        $source = Join-Path $temporary $name
        if (!(Test-Path -LiteralPath $source -PathType Leaf) -or (Get-Item -LiteralPath $source).Length -eq 0) { throw 'Missing media engine file' }
    }
    $stage = 'install verified media engine files'
    foreach ($name in @('mpv.exe', 'd3dcompiler_43.dll')) {
        Move-Item -LiteralPath (Join-Path $temporary $name) -Destination (Join-Path $PSScriptRoot $name) -Force
    }
    Remove-Item -LiteralPath (Join-Path $PSScriptRoot 'install-mpv.log') -Force -ErrorAction SilentlyContinue
} catch {
    $detail = $stage + ': ' + $_.Exception.Message
    $detail | Out-File -LiteralPath (Join-Path $PSScriptRoot 'install-mpv.log') -Encoding utf8
    [Console]::Error.WriteLine($detail)
    exit 1
} finally {
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
'''.replace('__URL__', mpv['url']).replace('__SHA__', mpv['sha256']).replace('__EXTRACTOR_URL__', extractor['url']).replace('__EXTRACTOR_SHA__', extractor['sha256'])
(runtime / 'install-mpv.ps1').write_text(script, encoding='utf-8-sig')
for name in ['mpv.exe', 'd3dcompiler_43.dll', 'MPV-SOURCE.txt']:
    (runtime / name).unlink(missing_ok=True)
(runtime / 'MPV-DOWNLOAD.txt').write_text(
    'The installer fetches mpv directly from its upstream publisher.\n'
    'This installer does not contain or redistribute the mpv/FFmpeg build.\n'
    f'URL: {mpv["url"]}\nSHA-256: {mpv["sha256"]}\n'
    f'Temporary extractor: {extractor["url"]}\nSHA-256: {extractor["sha256"]}\n'
    'Build recipes and license information: https://github.com/shinchiro/mpv-winbuild-cmake\n', encoding='utf-8')
print('Installer staging: upstream mpv binaries excluded; verified online installation hook prepared')
