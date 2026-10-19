#!/bin/sh
set -eu
project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
build_dir=${NEISORA_BUILD_DIR:-"$project_dir/src-tauri/target/release"}
binary="$build_dir/neisora"
runtime="$build_dir/runtime"
test -d "$runtime" || runtime="$project_dir/src-tauri/resources/runtime"
test -x "$binary" || { echo 'Сначала собери приложение: pnpm release' >&2; exit 1; }
for helper in node yt-dlp; do
    test -x "$runtime/$helper" || { echo 'Подготовь компоненты: python3 tools/prepare-runtime.py' >&2; exit 1; }
done
prefix=${NEISORA_INSTALL_PREFIX:-"$HOME/.local"}
install_root="$prefix/lib/neisora"
apps_dir="$prefix/share/applications"
icons_dir="$prefix/share/icons/hicolor"
mkdir -p "$install_root" "$prefix/bin" "$apps_dir" "$icons_dir/256x256/apps" "$icons_dir/scalable/apps"
version=$(sed -n 's/.*"version": "\([^"]*\)".*/\1/p' "$project_dir/package.json" | head -n1)
build_id=$(sha256sum "$binary" "$runtime/node" "$runtime/yt-dlp" | sha256sum | cut -c1-12)
release_name="$version-$build_id"
stage="$install_root/.install-$$"
trap 'rm -rf -- "$stage"; rm -f -- "$install_root/.current-$$"' EXIT
trap 'exit 1' HUP INT TERM
if ! test -d "$install_root/$release_name"; then
    mkdir -p "$stage/runtime"
    cp "$binary" "$stage/neisora"
    cp -R "$runtime/." "$stage/runtime/"
    cp "$project_dir/LICENSE" "$stage/LICENSE"
    if test -d "$project_dir/src-tauri/resources/licenses"; then cp -R "$project_dir/src-tauri/resources/licenses" "$stage/licenses"; fi
    chmod 755 "$stage/neisora" "$stage/runtime/node" "$stage/runtime/yt-dlp"
    mv "$stage" "$install_root/$release_name"
fi
ln -s "$release_name" "$install_root/.current-$$"
mv -Tf "$install_root/.current-$$" "$install_root/current"
cat > "$prefix/bin/.neisora-$$" <<EOF_LAUNCHER
#!/bin/sh
exec "$install_root/current/neisora" "\$@"
EOF_LAUNCHER
chmod +x "$prefix/bin/.neisora-$$"
mv -f "$prefix/bin/.neisora-$$" "$prefix/bin/neisora"
ln -sf neisora "$prefix/bin/vesper"
for name in neisora vesper-music local.kumite.vesper; do
    cp "$project_dir/src-tauri/icons/icon.png" "$icons_dir/256x256/apps/$name.png"
    cp "$project_dir/icon.svg" "$icons_dir/scalable/apps/$name.svg"
done
cat > "$apps_dir/neisora.desktop" <<EOF_DESKTOP
[Desktop Entry]
Type=Application
Name=neisora
Comment=Твоя музыка из YouTube Music
Exec="$prefix/bin/neisora"
Icon=neisora
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=neisora
EOF_DESKTOP
for name in vesper-music local.kumite.vesper; do
    cp "$apps_dir/neisora.desktop" "$apps_dir/$name.desktop"
    printf '\nNoDisplay=true\n' >> "$apps_dir/$name.desktop"
done
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$apps_dir"; fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache -f -t "$icons_dir"; fi
echo "Установлено: $prefix/bin/neisora ($version)"
