#!/bin/bash
# Build ZIKARON.app and wrap it in a .dmg and a .pkg.
#
# Usage: packaging/macos/build.sh [--identity NAME]
#   --identity NAME   code signing identity (default: "-", an ad-hoc signature)
#
# Output goes to dist/: ZIKARON-<version>-macos-<arch>.dmg and .pkg.
# The .pkg installs ZIKARON.app into /Applications and the `zikaron` command line into /usr/local/bin.
set -euo pipefail

IDENTITY="-"
while [ $# -gt 0 ]; do
  case "$1" in
    --identity) IDENTITY="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' crates/app/Cargo.toml | head -1)"
ARCH="$(uname -m)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
DIST="$ROOT/dist"
WORK="$DIST/macos-work"
APP="$WORK/ZIKARON.app"

# Keep local paths (home directory, checkout, build directory) out of the shipped binaries.
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME_DIR="${RUSTUP_HOME:-$HOME/.rustup}"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$CARGO_HOME_DIR=/cargo --remap-path-prefix=$RUSTUP_HOME_DIR=/rustup --remap-path-prefix=$TARGET_DIR=/target --remap-path-prefix=$ROOT=/zikaron --remap-path-prefix=$HOME=/home"
cargo build --release --locked -p app -p zikaron-cli

rm -rf "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$DIST"
# The window binary and the command line must differ by more than case: macOS volumes are usually case-insensitive.
cp "$TARGET_DIR/release/app" "$APP/Contents/MacOS/zikaron-desk"
cp "$TARGET_DIR/release/zikaron" "$APP/Contents/MacOS/zikaron"
cp packaging/icon/zikaron.icns "$APP/Contents/Resources/zikaron.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

codesign --force --sign "$IDENTITY" --timestamp=none "$APP/Contents/MacOS/zikaron"
codesign --force --sign "$IDENTITY" --timestamp=none "$APP/Contents/MacOS/zikaron-desk"
codesign --force --sign "$IDENTITY" --timestamp=none "$APP"
codesign --verify --strict "$APP"

# .dmg: the app and a link to /Applications.
DMG_SRC="$WORK/dmg"
mkdir -p "$DMG_SRC"
cp -R "$APP" "$DMG_SRC/"
ln -s /Applications "$DMG_SRC/Applications"
DMG="$DIST/ZIKARON-$VERSION-macos-$ARCH.dmg"
rm -f "$DMG"
hdiutil create -quiet -volname "ZIKARON $VERSION" -srcfolder "$DMG_SRC" -fs HFS+ -format UDZO "$DMG"
codesign --force --sign "$IDENTITY" --timestamp=none "$DMG"

# .pkg: the app into /Applications, the command line into /usr/local/bin.
PKG_ROOT="$WORK/pkgroot"
mkdir -p "$PKG_ROOT/Applications" "$PKG_ROOT/usr/local/bin"
cp -R "$APP" "$PKG_ROOT/Applications/"
cp "$APP/Contents/MacOS/zikaron" "$PKG_ROOT/usr/local/bin/zikaron"
xattr -cr "$PKG_ROOT"
PKG="$DIST/ZIKARON-$VERSION-macos-$ARCH.pkg"
rm -f "$PKG"
pkgbuild --quiet --root "$PKG_ROOT" --identifier com.kaptonia.zikaron.pkg --version "$VERSION" \
  --install-location / "$PKG"

rm -rf "$WORK"
( cd "$DIST" && shasum -a 256 "$(basename "$DMG")" "$(basename "$PKG")" > "SHA256SUMS-macos-$ARCH.txt" )
echo "built: $DMG"
echo "built: $PKG"
