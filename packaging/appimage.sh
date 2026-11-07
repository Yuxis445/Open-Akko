#!/usr/bin/env bash
# Build an AppImage from an already-built release binary.
# Usage: packaging/appimage.sh <version> [outdir]   (run from the repo root after `cargo build --release`)
set -euo pipefail
VER=${1:?version, e.g. v0.2.1}
OUT=${2:-dist}
ARCH=x86_64
TOOL=${APPIMAGETOOL:-appimagetool}
APPDIR=$(mktemp -d)/OpenAkko.AppDir

install -Dm755 target/release/openakko "$APPDIR/usr/bin/openakko"
install -Dm644 packaging/openakko.desktop "$APPDIR/usr/share/applications/openakko.desktop"
install -Dm644 packaging/openakko.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/openakko.svg"
install -Dm644 packaging/openakko-256.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/openakko.png"
install -Dm644 packaging/io.github.yuxis445.OpenAkko.metainfo.xml "$APPDIR/usr/share/metainfo/io.github.yuxis445.OpenAkko.appdata.xml"
install -Dm644 70-akko.rules "$APPDIR/usr/lib/udev/rules.d/70-akko.rules"
cp packaging/openakko.desktop "$APPDIR/openakko.desktop"
cp packaging/openakko-256.png "$APPDIR/openakko.png"
ln -s openakko.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/openakko" "$@"
RUN
chmod 755 "$APPDIR/AppRun"

mkdir -p "$OUT"
ARCH=$ARCH "$TOOL" --no-appstream "$APPDIR" "$OUT/OpenAkko-$VER-$ARCH.AppImage"
