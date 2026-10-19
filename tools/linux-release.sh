#!/bin/sh
# Build on Ubuntu 22.04; do not distribute binaries built on a newer host as universal Linux binaries.
set -eu
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}
target_dir=${CARGO_TARGET_DIR:-"$PWD/src-tauri/target"}
python3 tools/prepare-runtime.py
python3 tools/prepare-notices.py
python3 tools/check-release.py
pnpm check
node --experimental-strip-types --test tools/queue.test.ts tools/playback.test.ts tools/session.test.ts
# Tauri stamps the output binary in place when bundling AppImage. Cargo can
# otherwise reuse that stamped binary from a previous CI cache for native packages.
pnpm build
rm -f "$target_dir/release/neisora"
cargo build --release --locked --manifest-path src-tauri/Cargo.toml
# Remove only this tool's older generated packages; never upload mixed versions.
python3 - <<'CLEAN'
from pathlib import Path
output=Path('release')
for pattern in ['neisora_*.AppImage','neisora_*.deb','neisora-*.rpm','neisora-*.pkg.tar.zst','neisora-linux-x86_64.tar.gz','SHA256SUMS']:
    for path in output.glob(pattern):
        if path.is_file():path.unlink()
CLEAN
python3 tools/package-linux.py --binary "$target_dir/release/neisora"
# Bundle after arch/portable: the bundler stamps the binary's installer type.
node node_modules/@tauri-apps/cli/tauri.js bundle --bundles deb,rpm
python3 tools/prepare-appimage.py
node node_modules/@tauri-apps/cli/tauri.js bundle --bundles appimage --config artifacts/appimage.conf.json
version=$(python3 -c 'import json; print(json.load(open("package.json"))["version"])')
cp "$target_dir/release/bundle/appimage/neisora_${version}_amd64.AppImage" release/
cp "$target_dir/release/bundle/deb/neisora_${version}_amd64.deb" release/
cp "$target_dir/release/bundle/rpm/neisora-${version}-1.x86_64.rpm" release/
python3 - <<'PY'
import hashlib
from pathlib import Path
directory=Path('release')
with (directory/'SHA256SUMS').open('w') as output:
    for path in sorted(directory.iterdir()):
        if not path.is_file() or not (path.name.startswith('neisora') and path.name.endswith(('.deb','.rpm','.pkg.tar.zst','.tar.gz','.AppImage','.tar'))):continue
        output.write(hashlib.sha256(path.read_bytes()).hexdigest()+'  '+path.name+'\n')
PY
