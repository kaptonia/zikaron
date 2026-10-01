#!/bin/sh
# The release digest of the zikaron/1 criterion (docs/zikaron-v1.md §12.5):
# sha256 over the lines "<sha256 of file>  <name>\n" for the five source
# files of impl-py, in this fixed order. Run from anywhere.
set -e
DIR="$(cd "$(dirname "$0")/impl-py" && pwd)"
LISTING=""
for f in zk1.py zkcanon.py zkcrypto.py zkentry.py zkaudit.py; do
  H=$(python3 -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$DIR/$f")
  LISTING="$LISTING$H  $f
"
done
printf '%s' "$LISTING" | python3 -c "import hashlib,sys; print('0x'+hashlib.sha256(sys.stdin.buffer.read()).hexdigest())"
