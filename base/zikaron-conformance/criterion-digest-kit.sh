#!/bin/sh
# The release digest of the zikaron.kit/1 criterion (docs/zikaron-kit-v1.md
# §13.5): sha256 over the lines "<sha256 of file>  <name>\n" for the four
# source files of kit-py, in this fixed order. The parent core it embeds is
# pinned by its own digest (zikaron/1 §12.5, kit law §13.4).
set -e
DIR="$(cd "$(dirname "$0")/kit-py" && pwd)"
LISTING=""
for f in zkk.py zkkdoc.py zkkkit.py zkkread.py; do
  H=$(python3 -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$DIR/$f")
  LISTING="$LISTING$H  $f
"
done
printf '%s' "$LISTING" | python3 -c "import hashlib,sys; print('0x'+hashlib.sha256(sys.stdin.buffer.read()).hexdigest())"
