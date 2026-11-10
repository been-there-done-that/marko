#!/usr/bin/env bash
# Builds an AppDir for marko and packs it with appimagetool when that is on PATH.
# Nothing is downloaded: install appimagetool yourself if this script says it is missing.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
app_dir="$target_dir/appimage/AppDir"
out_dir="$target_dir/appimage"

cd "$root"
cargo build --release

rm -rf "$app_dir"
mkdir -p "$app_dir/usr/bin" "$app_dir/usr/share/applications" \
         "$app_dir/usr/share/icons/hicolor/256x256/apps"
install -m 755 "$target_dir/release/marko" "$app_dir/usr/bin/marko"
install -m 644 "$root/packaging/linux/marko.desktop" \
    "$app_dir/usr/share/applications/marko.desktop"
install -m 644 "$root/packaging/linux/marko.desktop" "$app_dir/marko.desktop"

# The icon is optional until one is drawn; the desktop entry names it as marko.
if [ -f "$root/packaging/linux/marko.png" ]; then
    install -m 644 "$root/packaging/linux/marko.png" \
        "$app_dir/usr/share/icons/hicolor/256x256/apps/marko.png"
    install -m 644 "$root/packaging/linux/marko.png" "$app_dir/marko.png"
else
    echo "note: packaging/linux/marko.png is missing; the AppImage will have no icon" >&2
fi

cat > "$app_dir/AppRun" <<'APPRUN'
#!/bin/sh
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/marko" "$@"
APPRUN
chmod 755 "$app_dir/AppRun"

if ! command -v appimagetool >/dev/null 2>&1; then
    cat >&2 <<MSG
AppDir is ready at $app_dir, but appimagetool is not on PATH.
Get appimagetool from https://github.com/AppImage/AppImageKit/releases (or your package
manager), make it executable, put it on PATH, then run this script again or run:
  appimagetool "$app_dir" "$out_dir/marko-x86_64.AppImage"
MSG
    exit 1
fi

mkdir -p "$out_dir"
ARCH="${ARCH:-x86_64}" appimagetool "$app_dir" "$out_dir/marko-$ARCH.AppImage"
echo "built $out_dir/marko-$ARCH.AppImage"
