#!/bin/bash
# Build the Linux x86_64 packages (.deb and AppImage) from another host, such as a Mac.
#
# Usage: packaging/linux/cross-build.sh
#
# Needs: zig, cargo-zigbuild (`cargo install --locked cargo-zigbuild`), mksquashfs, python3, curl.
# The binaries target glibc 2.31 (Ubuntu 20.04 and later). On a Linux machine, packaging/linux/build.sh
# does the same natively.
#
# Output goes to dist/: zikaron-desk_<version>_amd64.deb and ZIKARON-<version>-x86_64.AppImage.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' crates/app/Cargo.toml | head -1)"
TRIPLE=x86_64-unknown-linux-gnu
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
DIST="$ROOT/dist"
WORK="$DIST/linux-work"
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64"

# Keep local paths (home directory, checkout, build directory) out of the shipped binaries.
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME_DIR="${RUSTUP_HOME:-$HOME/.rustup}"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$CARGO_HOME_DIR=/cargo --remap-path-prefix=$RUSTUP_HOME_DIR=/rustup --remap-path-prefix=$TARGET_DIR=/target --remap-path-prefix=$ROOT=/zikaron --remap-path-prefix=$HOME=/home"
rustup target add "$TRIPLE" >/dev/null
cargo zigbuild --release --locked --target "$TRIPLE.2.31" -p app -p zikaron-cli
BIN="$TARGET_DIR/$TRIPLE/release"

rm -rf "$WORK"
mkdir -p "$WORK" "$DIST"

stage() {
  local root="$1"
  install -d "$root/usr/bin" "$root/usr/share/applications" "$root/usr/share/doc/zikaron-desk" \
    "$root/usr/share/icons/hicolor/scalable/apps"
  install -m755 "$BIN/app" "$root/usr/bin/zikaron-desk"
  install -m755 "$BIN/zikaron" "$root/usr/bin/zikaron"
  install -m644 packaging/linux/zikaron-desk.desktop "$root/usr/share/applications/zikaron-desk.desktop"
  for s in 16 24 32 48 64 128 256 512; do
    install -d "$root/usr/share/icons/hicolor/${s}x${s}/apps"
    install -m644 "packaging/icon/hicolor/$s.png" "$root/usr/share/icons/hicolor/${s}x${s}/apps/zikaron-desk.png"
  done
  install -m644 packaging/icon/zikaron.svg "$root/usr/share/icons/hicolor/scalable/apps/zikaron-desk.svg"
  install -m644 LICENSE "$root/usr/share/doc/zikaron-desk/copyright"
}

# .deb: an ar archive of debian-binary, control.tar.gz and data.tar.gz, every file owned by root.
DEB_ROOT="$WORK/deb"
stage "$DEB_ROOT"
DEB="$DIST/zikaron-desk_${VERSION}_amd64.deb"
python3 packaging/linux/make_deb.py "$DEB_ROOT" packaging/linux/control "$VERSION" amd64 "$DEB"

# AppImage: the AppImage runtime followed by a squashfs image of the AppDir.
APPDIR="$WORK/ZIKARON.AppDir"
stage "$APPDIR"
cp packaging/linux/zikaron-desk.desktop "$APPDIR/zikaron-desk.desktop"
cp packaging/icon/hicolor/256.png "$APPDIR/zikaron-desk.png"
ln -s zikaron-desk.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/zikaron-desk" "$@"
EOF
chmod 755 "$APPDIR/AppRun"
RUNTIME="${APPIMAGE_RUNTIME:-$WORK/runtime-x86_64}"
[ -f "$RUNTIME" ] || curl -fsSL -o "$RUNTIME" "$RUNTIME_URL"
mksquashfs "$APPDIR" "$WORK/image.squashfs" -root-owned -noappend -comp zstd -quiet -no-xattrs
APPIMAGE="$DIST/ZIKARON-$VERSION-x86_64.AppImage"
cat "$RUNTIME" "$WORK/image.squashfs" > "$APPIMAGE"
chmod 755 "$APPIMAGE"

rm -rf "$WORK"
( cd "$DIST" && shasum -a 256 "$(basename "$DEB")" "$(basename "$APPIMAGE")" > "SHA256SUMS-linux-x86_64.txt" )
echo "built: $DEB"
echo "built: $APPIMAGE"
