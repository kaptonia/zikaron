#!/bin/bash
# Build the Linux packages: a .deb and an AppImage. Run on Linux (x86_64 or aarch64).
#
# Usage: packaging/linux/build.sh
#
# Needs: the build dependencies listed in README.md, dpkg-deb, and for the AppImage either `appimagetool`
# on PATH or network access to fetch it (set APPIMAGETOOL to a local copy to skip the download).
#
# Output goes to dist/: zikaron-desk_<version>_<arch>.deb and ZIKARON-<version>-<arch>.AppImage.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' crates/app/Cargo.toml | head -1)"
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) DEB_ARCH=amd64 ;;
  aarch64) DEB_ARCH=arm64 ;;
  *) echo "unsupported architecture: $ARCH" >&2; exit 2 ;;
esac
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
DIST="$ROOT/dist"
WORK="$DIST/linux-work"

# Keep local paths (home directory, checkout, build directory) out of the shipped binaries.
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME_DIR="${RUSTUP_HOME:-$HOME/.rustup}"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$CARGO_HOME_DIR=/cargo --remap-path-prefix=$RUSTUP_HOME_DIR=/rustup --remap-path-prefix=$TARGET_DIR=/target --remap-path-prefix=$ROOT=/zikaron --remap-path-prefix=$HOME=/home"
cargo build --release --locked -p app -p zikaron-cli

rm -rf "$WORK"
mkdir -p "$DIST"

# Lay out the files the way both packages install them under /usr.
stage() {
  local root="$1"
  install -Dm755 "$TARGET_DIR/release/app" "$root/usr/bin/zikaron-desk"
  install -Dm755 "$TARGET_DIR/release/zikaron" "$root/usr/bin/zikaron"
  install -Dm644 packaging/linux/zikaron-desk.desktop "$root/usr/share/applications/zikaron-desk.desktop"
  for s in 16 24 32 48 64 128 256 512; do
    install -Dm644 "packaging/icon/hicolor/$s.png" "$root/usr/share/icons/hicolor/${s}x${s}/apps/zikaron-desk.png"
  done
  install -Dm644 packaging/icon/zikaron.svg "$root/usr/share/icons/hicolor/scalable/apps/zikaron-desk.svg"
  install -Dm644 LICENSE "$root/usr/share/doc/zikaron-desk/copyright"
}

# .deb
DEB_ROOT="$WORK/deb"
stage "$DEB_ROOT"
mkdir -p "$DEB_ROOT/DEBIAN"
SIZE="$(du -sk "$DEB_ROOT/usr" | cut -f1)"
sed -e "s/@VERSION@/$VERSION/" -e "s/@ARCH@/$DEB_ARCH/" -e "s/@SIZE@/$SIZE/" packaging/linux/control > "$DEB_ROOT/DEBIAN/control"
DEB="$DIST/zikaron-desk_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --root-owner-group --build "$DEB_ROOT" "$DEB"

# AppImage
APPDIR="$WORK/ZIKARON.AppDir"
stage "$APPDIR"
cp packaging/linux/zikaron-desk.desktop "$APPDIR/zikaron-desk.desktop"
cp packaging/icon/hicolor/256.png "$APPDIR/zikaron-desk.png"
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/zikaron-desk" "$@"
EOF
chmod 755 "$APPDIR/AppRun"
TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -z "$TOOL" ]; then
  TOOL="$WORK/appimagetool"
  curl -fsSL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
  chmod 755 "$TOOL"
fi
APPIMAGE="$DIST/ZIKARON-$VERSION-$ARCH.AppImage"
ARCH="$ARCH" "$TOOL" --appimage-extract-and-run "$APPDIR" "$APPIMAGE" 2>/dev/null || ARCH="$ARCH" "$TOOL" "$APPDIR" "$APPIMAGE"

rm -rf "$WORK"
( cd "$DIST" && sha256sum "$(basename "$DEB")" "$(basename "$APPIMAGE")" > "SHA256SUMS-linux-$ARCH.txt" )
echo "built: $DEB"
echo "built: $APPIMAGE"
