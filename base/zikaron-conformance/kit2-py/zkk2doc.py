"""zikaron.kit/2 sections 1 and 3: acceptance by spec, and badge payloads.

Every parent predicate is taken from the embedded cores (../impl2-py, which
embeds ../impl-py) and never re-derived here.
"""

import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
_UP = os.path.dirname(_HERE)
sys.path.insert(0, os.path.join(_UP, "impl2-py"))
sys.path.insert(0, os.path.join(_UP, "impl-py"))

from zkcanon import Fault, Obj, accept as canonical                 # noqa: E402
from zkcrypto import sha256                                          # noqa: E402
from zkentry import SEVEN, SEVEN_SET                                 # noqa: E402
import zkentry as v1entry                                            # noqa: E402
import zk2entry                                                      # noqa: E402

BADGE_PREFIX = b"zikaron-grant:"
BADGE_CAP = 2953


def accept(b):
    """s1 accept(b): zikaron/1 s4.3 where b passes zikaron/2 s4.3 steps 1 to 3
    and its spec is zikaron/1; zikaron/2 s4.3 otherwise.  Raises Fault."""
    v = canonical(b)
    if isinstance(v, Obj):
        d = v.dict()
        if all(k in d for k in SEVEN) and all(k in SEVEN_SET for k in d) and d["spec"] == "zikaron/1":
            return v1entry.validate(b)
    return zk2entry.validate(b)


def doc_id(b):
    return "0x" + sha256(b).hex()


# --------------------------------------------------------------------------
# s3 badge payload
# --------------------------------------------------------------------------

_ALPHABET = ("ABCDEFGHIJKLMNOPQRSTUVWXYZ"
             "abcdefghijklmnopqrstuvwxyz"
             "0123456789-_")
_B64VAL = {}
for _i, _ch in enumerate(_ALPHABET):
    _B64VAL[ord(_ch)] = _i
_B64CHR = _ALPHABET.encode("ascii")


def b64url_encode(b):
    out = bytearray()
    i = 0
    n = len(b)
    while i + 3 <= n:
        w = (b[i] << 16) | (b[i + 1] << 8) | b[i + 2]
        out.append(_B64CHR[(w >> 18) & 63])
        out.append(_B64CHR[(w >> 12) & 63])
        out.append(_B64CHR[(w >> 6) & 63])
        out.append(_B64CHR[w & 63])
        i += 3
    rest = n - i
    if rest == 1:
        w = b[i] << 16
        out.append(_B64CHR[(w >> 18) & 63])
        out.append(_B64CHR[(w >> 12) & 63])
    elif rest == 2:
        w = (b[i] << 16) | (b[i + 1] << 8)
        out.append(_B64CHR[(w >> 18) & 63])
        out.append(_B64CHR[(w >> 12) & 63])
        out.append(_B64CHR[(w >> 6) & 63])
    return bytes(out)


def b64url_decode_canonical(seg):
    if len(seg) == 0 or len(seg) % 4 == 1:
        return None
    vals = []
    for c in seg:
        v = _B64VAL.get(c)
        if v is None:
            return None
        vals.append(v)
    r = len(seg) % 4
    if r == 2 and (vals[-1] & 0x0F):
        return None
    if r == 3 and (vals[-1] & 0x03):
        return None
    acc = 0
    nbits = 0
    out = bytearray()
    for v in vals:
        acc = (acc << 6) | v
        nbits += 6
        while nbits >= 8:
            nbits -= 8
            out.append((acc >> nbits) & 0xFF)
    return bytes(out)


def byte_link(u, d):
    """s3.3: holds iff d.body.upstream is a string byte-equal to
    hex32(entry_id(u)) and d.body.work == u.body.work."""
    db = d.body.dict()
    if "upstream" not in db:
        return False
    up = db["upstream"]
    if not isinstance(up, str) or up != u.eid:
        return False
    return db.get("work") == u.body.dict().get("work")


def badge_encode(entry_byte_strings):
    segs = []
    for k, b in enumerate(entry_byte_strings):
        try:
            e = accept(b)
        except Fault as f:
            return ("E_BADGE_ENTRY", k, f.token)
        if e.etype != "grant":
            return ("E_BADGE_TYPE", k, None)
        segs.append(b64url_encode(b))
    payload = BADGE_PREFIX + b".".join(segs)
    if len(payload) > BADGE_CAP:
        return ("E_BADGE_CAP", None, None)
    return ("BADGE", payload, None)


def badge_decode(p):
    if not p.startswith(BADGE_PREFIX):
        return ("E_BADGE_PREFIX", None, None)
    if len(p) > BADGE_CAP:
        return ("E_BADGE_CAP", None, None)
    segments = p[len(BADGE_PREFIX):].split(b".")
    grants = []
    for k, seg in enumerate(segments):
        raw = b64url_decode_canonical(seg)
        if raw is None:
            return ("E_BADGE_B64", k, None)
        try:
            e = accept(raw)
        except Fault as f:
            return ("E_BADGE_ENTRY", k, f.token)
        if e.etype != "grant":
            return ("E_BADGE_TYPE", k, None)
        grants.append(e)
    if "upstream" in grants[0].body.dict():
        return ("E_BADGE_INCOMPLETE", None, None)
    for k in range(1, len(grants)):
        if not byte_link(grants[k - 1], grants[k]):
            return ("E_BADGE_LINK", k, None)
    return ("BADGE_OK", grants, None)
