#!/bin/sh
# Installs Marko for the current user on Linux or macOS. No root needed.
#
#   curl -fsSL https://raw.githubusercontent.com/OWNER/marko/main/install.sh | sh
#
# Options (pass after `sh -s --` when piping):
#   --version v0.2.0   install that release instead of the latest
#   --from-source      build from this checkout with cargo instead of downloading
#   --uninstall        remove what this script installed
#
# Linux:  ~/.local/bin/marko, a menu entry, and icons under ~/.local/share.
# macOS:  ~/Applications/Marko.app and ~/.local/bin/marko pointing into it.
set -eu

REPO="${MARKO_REPO:-OWNER/marko}"
VERSION="latest"
MODE="download"
BIN_DIR="${MARKO_BIN_DIR:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="$HOME/Applications"

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --from-source) MODE="source"; shift ;;
        --uninstall) MODE="uninstall"; shift ;;
        -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

say() { printf '%s\n' "marko: $*"; }
die() { printf '%s\n' "marko: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "this needs '$1'"; }

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64|amd64) ARCH=x86_64 ;;
    aarch64|arm64) ARCH=aarch64 ;;
    *) die "no build for $ARCH; try --from-source" ;;
esac
case "$OS" in
    Linux) PLATFORM=linux ;;
    Darwin) PLATFORM=macos ;;
    *) die "this script is for Linux and macOS; on Windows use install.ps1" ;;
esac

uninstall() {
    rm -f "$BIN_DIR/marko"
    if [ "$PLATFORM" = linux ]; then
        rm -f "$DATA_DIR/applications/marko.desktop"
        find "$DATA_DIR/icons/hicolor" -name marko.png -delete 2>/dev/null || true
        command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA_DIR/applications" 2>/dev/null || true
    else
        rm -rf "$APP_DIR/Marko.app"
    fi
    say "removed. Your settings stay in the config folder (dev.marko.app)."
}

# Puts the binary, the menu entry and the icons in place from an unpacked Linux archive in $1.
install_linux() {
    src="$1"
    mkdir -p "$BIN_DIR" "$DATA_DIR/applications"
    install -m 755 "$src/marko" "$BIN_DIR/marko"
    sed "s|^Exec=marko|Exec=$BIN_DIR/marko|" "$src/marko.desktop" > "$DATA_DIR/applications/marko.desktop"
    for png in "$src"/icons/marko-*.png; do
        [ -f "$png" ] || continue
        size="$(basename "$png" .png | sed 's/marko-//')"
        mkdir -p "$DATA_DIR/icons/hicolor/${size}x${size}/apps"
        install -m 644 "$png" "$DATA_DIR/icons/hicolor/${size}x${size}/apps/marko.png"
    done
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA_DIR/applications" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$DATA_DIR/icons/hicolor" 2>/dev/null || true
}

install_macos() {
    src="$1"
    mkdir -p "$APP_DIR" "$BIN_DIR"
    rm -rf "$APP_DIR/Marko.app"
    cp -R "$src/Marko.app" "$APP_DIR/"
    xattr -dr com.apple.quarantine "$APP_DIR/Marko.app" 2>/dev/null || true
    ln -sf "$APP_DIR/Marko.app/Contents/MacOS/marko" "$BIN_DIR/marko"
}

from_source() {
    need cargo
    root="$(cd "$(dirname "$0")" && pwd)"
    [ -f "$root/Cargo.toml" ] || die "--from-source runs from a checkout of the repository"
    say "building (this takes a few minutes the first time)…"
    (cd "$root" && cargo build --release --locked)
    target="${CARGO_TARGET_DIR:-$root/target}"
    stage="$(mktemp -d)"
    trap 'rm -rf "$stage"' EXIT
    if [ "$PLATFORM" = linux ]; then
        mkdir -p "$stage/icons"
        cp "$target/release/marko" "$root/packaging/linux/marko.desktop" "$stage/"
        cp "$root"/packaging/icons/png/*.png "$stage/icons/" 2>/dev/null || true
        install_linux "$stage"
    else
        app="$stage/Marko.app/Contents"
        mkdir -p "$app/MacOS" "$app/Resources"
        cp "$target/release/marko" "$app/MacOS/"
        cp "$root/packaging/icons/marko.icns" "$app/Resources/" 2>/dev/null || true
        version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
        sed "s/VERSION/$version/g" "$root/packaging/macos/Info.plist" > "$app/Info.plist"
        install_macos "$stage"
    fi
}

download() {
    need curl
    need tar
    asset="marko-$PLATFORM-$ARCH.tar.gz"
    if [ "$VERSION" = latest ]; then
        url="https://github.com/$REPO/releases/latest/download/$asset"
    else
        url="https://github.com/$REPO/releases/download/$VERSION/$asset"
    fi
    stage="$(mktemp -d)"
    trap 'rm -rf "$stage"' EXIT
    say "downloading $asset ($VERSION)…"
    curl -fL --progress-bar "$url" -o "$stage/$asset" || die "download failed: $url"
    tar xzf "$stage/$asset" -C "$stage"
    if [ "$PLATFORM" = linux ]; then install_linux "$stage/marko"; else install_macos "$stage"; fi
}

case "$MODE" in
    uninstall) uninstall; exit 0 ;;
    source) from_source ;;
    *) download ;;
esac

say "installed. Run 'marko' or open it from your applications."
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say "note: $BIN_DIR is not on your PATH; add it to use 'marko' in a terminal." ;;
esac
