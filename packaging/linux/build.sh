#!/bin/bash
# Build the Linux packages: a .deb and an AppImage. Run on Linux (x86_64 or aarch64).
#
# Usage: packaging/linux/build.sh
#
# Needs: the build dependencies listed in README.md, dpkg-deb, mksquashfs, and for the AppImage the pinned
# AppImage runtime: network access to fetch it, or the file given as APPIMAGE_RUNTIME.
#
# Output goes to dist/: zikaron-desk_<version>_<arch>.deb and ZIKARON-<version>-<arch>.AppImage, with
# SHA256SUMS-linux-<arch>.txt naming each package as it is made (a build that stops at the AppImage leaves the
# .deb with its sum, and exits non-zero).
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
PACK="$TARGET_DIR/release/zikaron-pack"
NOTICE_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
# Every date the packages record: the moment of the commit being built, unless SOURCE_DATE_EPOCH says another
# (dpkg-deb reads it for the archive's dates).
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || echo 0)}"
# The AppImage is the AppImage runtime followed by a squashfs image of the AppDir, made here as in
# cross-build.sh: no other tool goes into it. The runtime is pinned by its bytes for each architecture
# (`zikaron-pack appimage-runtime --arch`: x86_64 is type2-runtime 8f39b89, the one the shipped AppImages
# carry; an architecture with no pin is refused by name, after the .deb is built). The download below is the
# moving `continuous` build; when it is no longer that build the check stops here, and the pinned file is
# given as APPIMAGE_RUNTIME.
RUNTIME_URL="https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-$ARCH"

# Keep local paths (home directory, checkout, build directory) out of the shipped binaries: the packaging tool
# is built first and gives the flags, from the one table of path mappings (`zikaron-pack rustflags`).
cargo build --release --locked -p zikaron-pack
CARGO_ENCODED_RUSTFLAGS="$("$PACK" rustflags --target-dir "$TARGET_DIR")"
export CARGO_ENCODED_RUSTFLAGS
cargo build --release --locked -p app -p zikaron-cli
# The built binaries carry no path of this machine (`zikaron-pack no-paths`); no package is made if they do.
"$PACK" no-paths "$TARGET_DIR/release/app" "$TARGET_DIR/release/zikaron"

rm -rf "$WORK"
mkdir -p "$WORK" "$DIST"
# The notices for everything the packages install, with the licences of the three fonts this build embeds.
"$PACK" notices --target "$NOTICE_TARGET" --root app --root zikaron-cli \
  --with crates/zikaron-ui/fonts/OFL-Inter.txt --with crates/zikaron-ui/fonts/OFL-JetBrainsMono.txt \
  --with crates/zikaron-ui/fonts/OFL-NotoSansSC.txt --out "$WORK/THIRD-PARTY-LICENSES.txt"

# Lay out the files the way both packages install them under /usr.
stage() {
  local root="$1"
  install -Dm755 "$TARGET_DIR/release/app" "$root/usr/bin/zikaron-desk"
  install -Dm755 "$TARGET_DIR/release/zikaron" "$root/usr/bin/zikaron"
  # The toolchain words in each binary's `.comment` (not loaded at run time) zeroed in the staged copies.
  "$PACK" elf-comment "$root/usr/bin/zikaron-desk" "$root/usr/bin/zikaron"
  install -Dm644 packaging/linux/zikaron-desk.desktop "$root/usr/share/applications/zikaron-desk.desktop"
  for s in 16 24 32 48 64 128 256 512; do
    install -Dm644 "packaging/icon/hicolor/$s.png" "$root/usr/share/icons/hicolor/${s}x${s}/apps/zikaron-desk.png"
  done
  install -Dm644 packaging/icon/zikaron.svg "$root/usr/share/icons/hicolor/scalable/apps/zikaron-desk.svg"
  install -Dm644 LICENSE "$root/usr/share/doc/zikaron-desk/copyright"
  install -Dm644 "$WORK/THIRD-PARTY-LICENSES.txt" "$root/usr/share/doc/zikaron-desk/THIRD-PARTY-LICENSES.txt"
}

# .deb
DEB_ROOT="$WORK/deb"
stage "$DEB_ROOT"
mkdir -p "$DEB_ROOT/DEBIAN"
SIZE="$(du -sk "$DEB_ROOT/usr" | cut -f1)"
sed -e "s/@VERSION@/$VERSION/" -e "s/@ARCH@/$DEB_ARCH/" -e "s/@SIZE@/$SIZE/" packaging/linux/control > "$DEB_ROOT/DEBIAN/control"
DEB="$DIST/zikaron-desk_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --root-owner-group --build "$DEB_ROOT" "$DEB"
( cd "$DIST" && sha256sum "$(basename "$DEB")" > "SHA256SUMS-linux-$ARCH.txt" )

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
ln -s zikaron-desk.png "$APPDIR/.DirIcon"
RUNTIME="${APPIMAGE_RUNTIME:-$WORK/runtime-$ARCH}"
[ -f "$RUNTIME" ] || curl -fsSL -o "$RUNTIME" "$RUNTIME_URL"
"$PACK" appimage-runtime "$RUNTIME" --arch "$ARCH"
mksquashfs "$APPDIR" "$WORK/image.squashfs" -root-owned -noappend -comp zstd -quiet -no-xattrs
APPIMAGE="$DIST/ZIKARON-$VERSION-$ARCH.AppImage"
cat "$RUNTIME" "$WORK/image.squashfs" > "$APPIMAGE"
chmod 755 "$APPIMAGE"

rm -rf "$WORK"
( cd "$DIST" && sha256sum "$(basename "$DEB")" "$(basename "$APPIMAGE")" > "SHA256SUMS-linux-$ARCH.txt" )
echo "built: $DEB"
echo "built: $APPIMAGE"
