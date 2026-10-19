"""Refuse inconsistent versions or a tag that does not describe the current application."""
import json
import os
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
package = json.loads((root / 'package.json').read_text())
version = package['version']
license_id = package['license']
assert license_id == 'GPL-3.0-only'
assert json.loads((root / 'src-tauri/tauri.conf.json').read_text())['bundle']['license'] == license_id
assert re.search(r'^license = \"([^\"]+)\"', (root / 'src-tauri/Cargo.toml').read_text(), re.M).group(1) == license_id
assert json.loads((root / 'src-tauri/tauri.conf.json').read_text())['version'] == version
assert re.search(r'^version = "([^"]+)"', (root / 'src-tauri/Cargo.toml').read_text(), re.M).group(1) == version
assert re.search(r'name = "neisora"\nversion = "([^"]+)"', (root / 'src-tauri/Cargo.lock').read_text()).group(1) == version
if os.environ.get('GITHUB_REF_TYPE') == 'tag':
    assert os.environ.get('GITHUB_REF_NAME') == 'v' + version, 'Release tag does not match application version'
print('Release version is consistent:', version)
