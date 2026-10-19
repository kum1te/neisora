"""Bounded, checksum-verified build-time downloads. No application auto-downloads."""
import hashlib
import os
from pathlib import Path
import urllib.request

cache = Path(os.environ.get('XDG_CACHE_HOME', str(Path.home()/'.cache')))/'neisora/downloads'
cache.mkdir(parents=True, exist_ok=True)

def fetch(url, filename, expected=None):
    path = cache/filename
    if not path.exists() or (expected and hashlib.sha256(path.read_bytes()).hexdigest() != expected):
        temporary = path.with_suffix('.download')
        try:
            with urllib.request.urlopen(url, timeout=60) as source, temporary.open('wb') as target:
                total = 0
                while chunk := source.read(1024*1024):
                    total += len(chunk)
                    if total > 200*1024*1024:
                        raise ValueError('Download exceeds budget')
                    target.write(chunk)
            if expected and hashlib.sha256(temporary.read_bytes()).hexdigest() != expected:
                raise ValueError(f'Checksum mismatch: {filename}')
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    return path

