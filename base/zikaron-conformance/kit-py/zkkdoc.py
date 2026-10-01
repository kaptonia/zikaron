"""zikaron.kit/1 sections 2 through 6: the document predicates.

Signing (s3), fingerprint manifests (s4), acknowledgements (s5), pairing and
attribution (s5.3, s5.4), and badge payloads (s6).  Every zikaron/1 predicate
is taken from the embedded frozen core (../impl-py) and never re-derived here.
"""

from zkcanon import Fault, Obj, accept as canonical, canon
from zkcrypto import eip191_digest, message, sha256
from zkentry import check_sig, is_hex, is_prose, validate

# --------------------------------------------------------------------------
# s3.2 domains (closed) and s6.1 constants
# --------------------------------------------------------------------------

DOMAIN_FPM = "zikaron.fpm/1"
DOMAIN_ACK = "zikaron.ack/1"
DOMAINS = (DOMAIN_FPM, DOMAIN_ACK)

SPEC_FPM = "zikaron.fpm/1"
SPEC_ACK = "zikaron.ack/1"

BADGE_PREFIX = b"zikaron-grant:"
BADGE_CAP = 2953


class RowFault(Fault):
    """s4.2's E_FPM_ROW, carrying the index of the row that failed."""

    def __init__(self, token, index):
        super().__init__(token)
        self.index = index


# --------------------------------------------------------------------------
# s3.1 construction
# --------------------------------------------------------------------------

def presig_bytes(v):
    """B: the canonical bytes of the object with the top-level `sig` removed
    and nothing else (s3.1)."""
    return canon(Obj([(k, val) for k, val in v.pairs if k != "sig"]))


def doc_digest(v, domain):
    """The EIP-191 digest of D || 0x0A || hex32(sha256(B)) (s3.1, s5.3)."""
    return eip191_digest(message(domain, sha256(presig_bytes(v))))


def check_doc_sig(v, domain, sighex, signer_hex):
    """s3.1: form, range, low-s, recovery, and signer equality.  Raises Fault
    with E_SIG_FORM / E_SIG_V / E_SIG_RANGE / E_SIG_HIGH_S / E_SIG_RECOVER /
    E_SIG_SIGNER in the order zikaron/1 s5.4 and s5.5 fix."""
    check_sig(doc_digest(v, domain), sighex, signer_hex)


def doc_id(b):
    """s1: doc_id(b) = sha256(b), total."""
    return "0x" + sha256(b).hex()


# --------------------------------------------------------------------------
# s4 fingerprint manifest
# --------------------------------------------------------------------------

FPM_KEYS = ("author", "grant", "note_md", "rows", "sig", "spec", "work")
FPM_KEYSET = frozenset(FPM_KEYS)
ROW_KEYSET = frozenset(("recipient", "variant"))


class Fpm:
    __slots__ = ("raw", "value", "d", "rows", "doc_id")

    def __init__(self, raw, value, d, rows):
        self.raw = raw
        self.value = value
        self.d = d
        self.rows = rows                     # list of (recipient, variant)
        self.doc_id = doc_id(raw)


def fpm_accept(b):
    """s4.2, in the order the law writes it.  Raises Fault or returns Fpm."""
    v = canonical(b)                                              # canonical(b)
    if not isinstance(v, Obj):
        raise Fault("E_DOC")
    d = v.dict()
    for k in FPM_KEYS:
        if k not in d:
            raise Fault("E_DOC_MISSING")
    for k in d:
        if k not in FPM_KEYSET:
            raise Fault("E_DOC_CLOSED")
    if d["spec"] != SPEC_FPM:
        raise Fault("E_SPEC")
    if not is_hex(d["author"], 20):
        raise Fault("E_FPM_AUTHOR")
    if not is_hex(d["work"], 32):
        raise Fault("E_FPM_WORK")
    if not (d["grant"] is None or is_hex(d["grant"], 32)):
        raise Fault("E_FPM_GRANT")
    rows = d["rows"]
    if not isinstance(rows, list) or len(rows) == 0:
        raise Fault("E_FPM_ROWS")
    parsed = []
    for i, r in enumerate(rows):                     # array order, with the index
        if not isinstance(r, Obj) or frozenset(r.keys()) != ROW_KEYSET:
            raise RowFault("E_FPM_ROW", i)
        rd = r.dict()
        if not is_hex(rd["recipient"], 20) or not is_hex(rd["variant"], 32):
            raise RowFault("E_FPM_ROW", i)
        parsed.append((rd["recipient"], rd["variant"]))
    recipients = [p[0] for p in parsed]
    if len(set(recipients)) != len(recipients):
        raise Fault("E_FPM_DUP_RECIPIENT")
    variants = [p[1] for p in parsed]
    if len(set(variants)) != len(variants):
        raise Fault("E_FPM_DUP_VARIANT")
    for i in range(1, len(recipients)):
        if recipients[i - 1].encode("utf-8") > recipients[i].encode("utf-8"):
            raise Fault("E_FPM_ROW_ORDER")
    if not is_prose(d["note_md"]):
        raise Fault("E_FPM_NOTE")
    if not is_hex(d["sig"], 65):
        raise Fault("E_SIG_FORM")
    check_doc_sig(v, DOMAIN_FPM, d["sig"], d["author"])
    return Fpm(b, v, d, parsed)


# --------------------------------------------------------------------------
# s5 acknowledgement
# --------------------------------------------------------------------------

ACK_KEYS = ("fpm", "note_md", "recipient", "sig", "spec", "variant")
ACK_KEYSET = frozenset(ACK_KEYS)


class Ack:
    __slots__ = ("raw", "value", "d", "doc_id")

    def __init__(self, raw, value, d):
        self.raw = raw
        self.value = value
        self.d = d
        self.doc_id = doc_id(raw)


def ack_accept(b):
    """s5.2, in the order the law writes it.  Raises Fault or returns Ack."""
    v = canonical(b)
    if not isinstance(v, Obj):
        raise Fault("E_DOC")
    d = v.dict()
    for k in ACK_KEYS:
        if k not in d:
            raise Fault("E_DOC_MISSING")
    for k in d:
        if k not in ACK_KEYSET:
            raise Fault("E_DOC_CLOSED")
    if d["spec"] != SPEC_ACK:
        raise Fault("E_SPEC")
    if not is_hex(d["recipient"], 20):
        raise Fault("E_ACK_RECIPIENT")
    if not is_hex(d["fpm"], 32):
        raise Fault("E_ACK_FPM")
    if not is_hex(d["variant"], 32):
        raise Fault("E_ACK_VARIANT")
    if not is_prose(d["note_md"]):
        raise Fault("E_ACK_NOTE")
    if not is_hex(d["sig"], 65):
        raise Fault("E_SIG_FORM")
    check_doc_sig(v, DOMAIN_ACK, d["sig"], d["recipient"])
    return Ack(b, v, d)


# --------------------------------------------------------------------------
# s5.3 pairing, s5.4 attribution
# --------------------------------------------------------------------------

def pair(m_bytes, a_bytes):
    """s5.3.  Returns (verdict, token_or_None, row_or_None)."""
    try:
        m = fpm_accept(m_bytes)
    except Fault as f:
        return ("FPM_INVALID", f.token, None)
    try:
        a = ack_accept(a_bytes)
    except Fault as f:
        return ("ACK_INVALID", f.token, None)
    if a.d["fpm"] != m.doc_id:
        return ("ACK_FPM_MISMATCH", None, None)
    row = None
    for r in m.rows:
        if r[0] == a.d["recipient"]:
            row = r
            break
    if row is None:
        return ("ACK_NO_ROW", None, None)
    if row[1] != a.d["variant"]:
        return ("ACK_VARIANT_MISMATCH", None, None)
    return ("PAIRED", None, (a.d["recipient"], a.d["variant"]))


def attribute(m_bytes, a_bytes, x):
    """s5.4.  Returns (verdict, token_or_None, row_or_None) where verdict is
    the pairing verdict when it is not PAIRED, else ATTRIBUTED or
    NOT_ATTRIBUTED."""
    verdict, token, row = pair(m_bytes, a_bytes)
    if verdict != "PAIRED":
        return (verdict, token, None)
    ok = ("0x" + sha256(x).hex()) == row[1]
    return ("ATTRIBUTED" if ok else "NOT_ATTRIBUTED", None, row)


# --------------------------------------------------------------------------
# s6 badge payload
# --------------------------------------------------------------------------

_ALPHABET = ("ABCDEFGHIJKLMNOPQRSTUVWXYZ"
             "abcdefghijklmnopqrstuvwxyz"
             "0123456789-_")
_B64VAL = {}
for _i, _ch in enumerate(_ALPHABET):
    _B64VAL[ord(_ch)] = _i
_B64CHR = _ALPHABET.encode("ascii")


def b64url_encode(b):
    """RFC 4648 s5 with no padding, canonical (unused trailing bits zero)."""
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
    """s6.2 step 3a.  Returns the decoded bytes, or None when the segment is
    empty, carries a byte outside A-Za-z0-9-_, has length == 1 (mod 4), or
    its unused trailing bits are not zero."""
    if len(seg) == 0:
        return None
    if len(seg) % 4 == 1:
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
    """s6.3: holds iff d.body.upstream is a string byte-equal to
    hex32(entry_id(u)) and d.body.work == u.body.work."""
    db = d.body.dict()
    if "upstream" not in db:
        return False
    up = db["upstream"]
    if not isinstance(up, str) or up != u.eid:
        return False
    return db.get("work") == u.body.dict().get("work")


def badge_encode(entry_byte_strings):
    """s6.1.  Returns ("BADGE", payload_bytes, None), or a rejection triple
    ("E_BADGE_ENTRY", k, token), ("E_BADGE_TYPE", k, None), or
    ("E_BADGE_CAP", None, None)."""
    segs = []
    for k, b in enumerate(entry_byte_strings):
        try:
            e = validate(b)
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
    """s6.2, total.  Returns ("BADGE_OK", entries, None) or
    (token, index_or_None, inner_token_or_None)."""
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
            e = validate(raw)
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
