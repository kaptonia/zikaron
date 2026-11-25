#!/bin/sh
# The release digest of the zikaron.face/1 criterion (docs/zikaron-face-v1.md
# §10.5): sha256 over the lines "<sha256 of file>  <name>\n" for the three
# source files of face-py in this fixed order, followed by the line
# "<zikaron/2 core digest>  zikaron/2\n" as criterion-digest2.sh prints it.
# Run from anywhere; python3 only.
set -e
DIR="$(cd "$(dirname "$0")/face-py" && pwd)"
CORE=$(sh "$(dirname "$0")/criterion-digest2.sh")
LAW="$(dirname "$0")/../docs/zikaron-face-v1.md"
if [ ! -f "$LAW" ]; then
  echo "law text $LAW is not beside this script's tree" >&2
  exit 1
fi
# The face law names the zikaron/2 core it embeds as a backticked digest
# (s10.4); a face core built on any other zikaron/2 core is another core.
# The guard looks for that backticked digest anywhere in the text and reads
# nothing of the layout around it.
if ! grep -qF "\`$CORE\`" "$LAW"; then
  echo "core $CORE is not the zikaron/2 core named in $LAW s10.4" >&2
  exit 1
fi
LISTING=""
for f in zkf.py zkfentry.py zkftable.py; do
  H=$(python3 -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$DIR/$f")
  LISTING="$LISTING$H  $f
"
done
LISTING="$LISTING$CORE  zikaron/2
"
printf '%s' "$LISTING" | python3 -c "import hashlib,sys; print('0x'+hashlib.sha256(sys.stdin.buffer.read()).hexdigest())"
