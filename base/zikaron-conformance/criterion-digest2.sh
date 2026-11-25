#!/bin/sh
# The release digest of the zikaron/2 criterion (docs/zikaron-v2.md §12.5):
# sha256 over the lines "<sha256 of file>  <name>\n" for the three source
# files of impl2-py in this fixed order, followed by the line
# "<zikaron/1 core digest>  zikaron/1\n" as criterion-digest.sh prints it,
# that core being the one §12.6 names. Run from anywhere; python3 only.
set -e
DIR="$(cd "$(dirname "$0")/impl2-py" && pwd)"
CORE=$(sh "$(dirname "$0")/criterion-digest.sh")
LAW="$(dirname "$0")/../docs/zikaron-v2.md"
if [ ! -f "$LAW" ]; then
  echo "law text $LAW is not beside this script's tree" >&2
  exit 1
fi
# The law names the embedded core as a backticked digest (s12.6); the guard
# reads the naming and never the layout of the text around it.
if ! grep -qF "\`$CORE\`" "$LAW"; then
  echo "core $CORE is not the zikaron/1 core named in $LAW s12.6" >&2
  exit 1
fi
LISTING=""
for f in zk2.py zk2entry.py zk2audit.py; do
  H=$(python3 -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$DIR/$f")
  LISTING="$LISTING$H  $f
"
done
LISTING="$LISTING$CORE  zikaron/1
"
printf '%s' "$LISTING" | python3 -c "import hashlib,sys; print('0x'+hashlib.sha256(sys.stdin.buffer.read()).hexdigest())"
