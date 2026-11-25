#!/bin/sh
# The release digest of the zikaron.kit/2 criterion (docs/zikaron-kit-v2.md
# §10.5): sha256 over the lines "<sha256 of file>  <name>\n" for the four
# source files of kit2-py in this fixed order, followed by the line
# "<zikaron/2 core digest>  zikaron/2\n" as criterion-digest2.sh prints it.
# Run from anywhere; python3 only.
set -e
DIR="$(cd "$(dirname "$0")/kit2-py" && pwd)"
CORE=$(sh "$(dirname "$0")/criterion-digest2.sh")
LAW="$(dirname "$0")/../docs/zikaron-kit-v2.md"
if [ ! -f "$LAW" ]; then
  echo "law text $LAW is not beside this script's tree" >&2
  exit 1
fi
# The kit law names the zikaron/2 core it embeds as a backticked digest in
# s10.4 at that law's naming; until then no kit digest exists, and a kit core
# built on any other zikaron/2 core is another core. The guard reads the
# naming and never the layout of the text around it.
if ! grep -qF "\`$CORE\`" "$LAW"; then
  echo "core $CORE is not the zikaron/2 core named in $LAW s10.4" >&2
  exit 1
fi
LISTING=""
for f in zkk2.py zkk2doc.py zkk2kit.py zkk2read.py; do
  H=$(python3 -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$DIR/$f")
  LISTING="$LISTING$H  $f
"
done
LISTING="$LISTING$CORE  zikaron/2
"
printf '%s' "$LISTING" | python3 -c "import hashlib,sys; print('0x'+hashlib.sha256(sys.stdin.buffer.read()).hexdigest())"
