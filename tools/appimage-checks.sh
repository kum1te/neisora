#!/bin/sh
# The automation protocol must match the WebKit inside the image, not the host's version.
set -eu
image=$(realpath "$1")
driver=$(realpath "${NEISORA_MATCHING_WEBKIT_DRIVER:-test-tools/WebKitWebDriver}")
test_root=$(mktemp -d /tmp/neisora-image-driver-XXXXXX)
trap 'rm -rf "$test_root"' EXIT
trap 'exit 1' HUP INT TERM
chmod +x "$image" "$driver"
(cd "$test_root" && "$image" --appimage-extract > /dev/null)
# Keep the image's library paths scoped to the driver, rather than Python/test utilities.
python3 - "$test_root" "$driver" <<'PY'
from pathlib import Path
import shlex,sys,shutil
root=Path(sys.argv[1])
wrapper=root/'matching-driver'
image=root/'squashfs-root'
driver=image/'usr/bin/WebKitWebDriver'
shutil.copy2(sys.argv[2],driver)
wrapper.write_text('#!/bin/sh\nexport APPDIR='+shlex.quote(str(image))+'\nunset LD_LIBRARY_PATH\n. "$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"\nexport WEBKIT_EXEC_PATH="$APPDIR/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1"\nexport WEBKIT_INJECTED_BUNDLE_PATH="$WEBKIT_EXEC_PATH/injected-bundle"\nexec '+shlex.quote(str(driver))+' "$@"\n')
wrapper.chmod(0o755)
PY
APPIMAGE_EXTRACT_AND_RUN=1 NEISORA_EXPECT_APPIMAGE=1 NEISORA_SMOKE_LOCAL=1 NEISORA_TEST_BINARY="$image" NEISORA_NATIVE_DRIVER="$test_root/matching-driver" sh tools/installed-checks.sh
