"""Verify the shipped image's helper closure without FUSE or system Python modules."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

image = Path(sys.argv[1]).resolve()
manifest = json.loads(Path('tools/runtime-manifest.json').read_text())
with tempfile.TemporaryDirectory(prefix='neisora-image-check-') as temporary:
    subprocess.run([str(image), '--appimage-extract'], cwd=temporary, check=True, stdout=subprocess.DEVNULL)
    root = Path(temporary)/'squashfs-root'
    assert 'ELF RPATH scopes private libraries' in (root/'AppRun').read_text(), 'Unsafe launcher or output finalizer was not applied'
    runtime = root/'usr/lib/neisora/runtime'
    python = runtime/'python/bin/python3.10'
    env = dict(os.environ, PYTHONHOME=str(runtime/'python'))
    env.pop('LD_LIBRARY_PATH', None)
    env.pop('PYTHONPATH', None)
    env['PYTHONNOUSERSITE'] = '1'
    if 'SSL_CERT_FILE' not in env:
        for path in ['/etc/ssl/certs/ca-certificates.crt','/etc/pki/tls/certs/ca-bundle.crt','/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem','/etc/ssl/cert.pem','/etc/ca-certificates/extracted/tls-ca-bundle.pem']:
            if Path(path).is_file(): env['SSL_CERT_FILE']=path; break
    assert subprocess.check_output([str(runtime/'node'), '--version'], env=env, text=True).strip() == 'v'+manifest['node']['version']
    assert hashlib.sha256((runtime/'yt-dlp').read_bytes()).hexdigest() == manifest['yt-dlp']['sha256']
    assert subprocess.check_output([str(python), str(runtime/'yt-dlp'), '--version'], env=env, text=True).strip() == manifest['yt-dlp']['version']
    subprocess.run([str(python), '-I', '-c', 'import sys, sqlite3, ssl, bz2, lzma; sys.path.insert(0, sys.argv[1]); from yt_dlp.cookies import extract_cookies_from_browser', str(runtime/'yt-dlp')], env=env, check=True)
    subprocess.run([str(python), '-c', 'import ssl; assert ssl.create_default_context().get_ca_certs(), "Host CA store unavailable"'], env=env, check=True)
    subprocess.run([str(root/'usr/bin/mpv'), '--no-config', '--version'], env=env, check=True, stdout=subprocess.DEVNULL)
    assert (root/'usr/lib/neisora/licenses/NEISORA-LICENSE.txt').read_text() == Path('LICENSE').read_text()
    assert (root/'usr/lib/neisora/licenses/appimage/mpv-copyright').is_file()
print('AppImage: bundled Node, Python stdlib/extensions, yt-dlp, cookie import module, mpv and notices PASS')
