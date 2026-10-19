#!/bin/sh
# neisora launcher: ELF RPATH scopes private libraries, without a global override.
set -e
APPDIR=${APPDIR:-$(CDPATH= cd -- "$(/usr/bin/dirname -- "$0")" && pwd -P)}
export APPDIR
. "$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"
export PATH="$APPDIR/usr/bin:${PATH:-/usr/bin:/bin}"
export WEBKIT_INJECTED_BUNDLE_PATH="$APPDIR/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/injected-bundle"
cd "$APPDIR/usr"
exec "$APPDIR/usr/bin/neisora" "$@"
