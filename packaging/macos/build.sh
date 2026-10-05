#!/bin/bash
# Build ZIKARON.app and wrap it in a .dmg and a .pkg.
#
# Usage: packaging/macos/build.sh [--identity NAME]
#   --identity NAME   code signing identity (default: "-", an ad-hoc signature)
#
# Output goes to dist/: ZIKARON-<version>-macos-<arch>.dmg and .pkg.
# The .pkg installs ZIKARON.app into /Applications and the `zikaron` command line into /usr/local/bin.
# Both carry the third-party notices (ZIKARON.app/Contents/Resources/THIRD-PARTY-LICENSES.txt) and nothing of
# the machine that built them: no extended attributes, owners and dates neutral in the .dmg's catalog, partition
# names in no one's language, and in the .pkg no owner, inode, device or system build.
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
PACK="$TARGET_DIR/release/zikaron-pack"
# Every date the packages record: the moment of the commit being built, unless SOURCE_DATE_EPOCH says another.
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || echo 0)}"

# Keep local paths (home directory, checkout, build directory) out of the shipped binaries.
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME_DIR="${RUSTUP_HOME:-$HOME/.rustup}"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$CARGO_HOME_DIR=/cargo --remap-path-prefix=$RUSTUP_HOME_DIR=/rustup --remap-path-prefix=$TARGET_DIR=/target --remap-path-prefix=$ROOT=/zikaron --remap-path-prefix=$HOME=/home"
cargo build --release --locked -p app -p zikaron-cli -p zikaron-pack

rm -rf "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$DIST"
# The window binary and the command line must differ by more than case: macOS volumes are usually case-insensitive.
# Copies carry no extended attributes (`-X`): no provenance or quarantine marks of this machine.
cp -X "$TARGET_DIR/release/app" "$APP/Contents/MacOS/zikaron-desk"
cp -X "$TARGET_DIR/release/zikaron" "$APP/Contents/MacOS/zikaron"
cp -X packaging/icon/zikaron.icns "$APP/Contents/Resources/zikaron.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
# The notices for everything the bundle holds, with the licences of the two fonts the macOS build embeds.
"$PACK" notices --target "$(rustc -vV | sed -n 's/^host: //p')" --root app --root zikaron-cli \
  --with crates/zikaron-ui/fonts/OFL-Inter.txt --with crates/zikaron-ui/fonts/OFL-JetBrainsMono.txt \
  --out "$APP/Contents/Resources/THIRD-PARTY-LICENSES.txt"
xattr -cr "$APP"

codesign --force --sign "$IDENTITY" --timestamp=none "$APP/Contents/MacOS/zikaron"
codesign --force --sign "$IDENTITY" --timestamp=none "$APP/Contents/MacOS/zikaron-desk"
codesign --force --sign "$IDENTITY" --timestamp=none "$APP"
codesign --verify --strict "$APP"

# .dmg: the app and a link to /Applications. Made first as a read/write image with no partition map, so the
# volume starts at the image's first byte and its catalog can be rewritten: owners and groups of this user
# become the "unknown" owner (whoever opens it), every date the commit's moment. Then compressed, and the
# partition names (written in this machine's language) rewritten in one neutral form.
DMG_SRC="$WORK/dmg"
mkdir -p "$DMG_SRC"
ditto --noextattr --noqtn "$APP" "$DMG_SRC/ZIKARON.app"
ln -s /Applications "$DMG_SRC/Applications"
DMG="$DIST/ZIKARON-$VERSION-macos-$ARCH.dmg"
RAW="$WORK/dmg-raw.dmg"
rm -f "$DMG" "$RAW"
hdiutil create -quiet -volname "ZIKARON $VERSION" -srcfolder "$DMG_SRC" -fs HFS+ -layout NONE -format UDRW "$RAW"
"$PACK" hfs-owners "$RAW" --epoch "$SOURCE_DATE_EPOCH"
hdiutil convert -quiet "$RAW" -format UDZO -o "$DMG"
"$PACK" dmg-names "$DMG"
hdiutil verify -quiet "$DMG"
codesign --force --sign "$IDENTITY" --timestamp=none "$DMG"

# .pkg: the app into /Applications, the command line into /usr/local/bin.
PKG_ROOT="$WORK/pkgroot"
mkdir -p "$PKG_ROOT/Applications" "$PKG_ROOT/usr/local/bin"
ditto --noextattr --noqtn "$APP" "$PKG_ROOT/Applications/ZIKARON.app"
cp -X "$APP/Contents/MacOS/zikaron" "$PKG_ROOT/usr/local/bin/zikaron"
xattr -cr "$PKG_ROOT"
PKG="$DIST/ZIKARON-$VERSION-macos-$ARCH.pkg"
FLAT="$WORK/flat.pkg"
rm -f "$PKG"
pkgbuild --quiet --root "$PKG_ROOT" --identifier com.kaptonia.zikaron.pkg --version "$VERSION" \
  --install-location / "$FLAT"
# The flat package's table of contents records the builder's owner, group, inodes, device and extended
# attributes for its own members, and PackageInfo names the system build of the packaging tools. The members
# are taken out, PackageInfo loses the system build, and the archive is made again without those properties
# (the payload byte for byte as pkgbuild made it).
EXP="$WORK/pkg-members"
pkgutil --expand "$FLAT" "$EXP"
sed -E -i '' 's/(generator-version="[^" ]*) \([^)]*\)"/\1"/' "$EXP/PackageInfo"
xattr -cr "$EXP"
( cd "$EXP" && xar -c -f "$PKG" --compression gzip --no-compress '^Payload$' \
    --prop-exclude user --prop-exclude uid --prop-exclude group --prop-exclude gid \
    --prop-exclude inode --prop-exclude deviceno --prop-exclude ea --prop-exclude FinderCreateTime \
    --prop-exclude atime --prop-exclude ctime --prop-exclude mtime \
    Bom Payload PackageInfo )

rm -rf "$WORK"
( cd "$DIST" && shasum -a 256 "$(basename "$DMG")" "$(basename "$PKG")" > "SHA256SUMS-macos-$ARCH.txt" )
echo "built: $DMG"
echo "built: $PKG"
