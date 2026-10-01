"""zikaron/1 sections 4, 5 and 6: envelope, signing, bodies."""

from zkcanon import Fault, Obj, accept, canon
from zkcrypto import (N, HALF_N, address_of, eip191_digest, message, recover,
                      sha256)

SEVEN = ("author", "body", "entryType", "prev", "seq", "sig", "spec")
SEVEN_SET = frozenset(SEVEN)

DOMAIN_ENTRY = "zikaron/1"
DOMAIN_ADOPTION = "zikaron/1-adoption"
KNOWN_TYPES = frozenset(("genesis", "history", "grant", "revocation", "adoption",
                         "succession", "annotation"))

_HEXL = frozenset("0123456789abcdef")


# --------------------------------------------------------------------------
# s1 forms
# --------------------------------------------------------------------------

def is_hex(v, nbytes):
    if not isinstance(v, str):
        return False
    if len(v) != 2 + 2 * nbytes:
        return False
    if v[0] != "0" or v[1] != "x":
        return False
    for c in v[2:]:
        if c not in _HEXL:
            return False
    return True


def is_token(v):
    if not isinstance(v, str) or len(v) == 0:
        return False
    for ch in v:
        o = ord(ch)
        if o < 0x21 or o > 0x7E:
            return False
    return True


def is_int(v):
    return type(v) is int


def is_prose(v):
    return isinstance(v, str)


def hexb(v):
    """The bytes of a hexN string."""
    return bytes.fromhex(v[2:])


# --------------------------------------------------------------------------
# s6 body tables
# --------------------------------------------------------------------------

def _bad():
    raise Fault("E_BODY_FIELD")


def _check_body(etype, body):
    d = body.dict()

    def req(k, test):
        if k not in d or not test(d[k]):
            _bad()

    def opt(k, test):
        if k in d and not test(d[k]):
            _bad()

    if etype == "genesis":
        req("statement_md", is_prose)
    elif etype == "history":
        req("content", lambda v: is_hex(v, 32))
        if "mode" not in d or not isinstance(d["mode"], Obj):
            _bad()
        m = d["mode"].dict()
        if "mark" not in m or not is_token(m["mark"]):
            _bad()
        if "toolchain" not in m or not is_hex(m["toolchain"], 32):
            _bad()
        opt("note_md", is_prose)
    elif etype == "grant":
        req("grantee", lambda v: is_hex(v, 20))
        req("work", lambda v: is_hex(v, 32))
        req("terms", lambda v: is_hex(v, 32))
        opt("history", lambda v: is_hex(v, 32))
        if "window" in d:
            w = d["window"]
            if not isinstance(w, Obj):
                _bad()
            wd = w.dict()
            if "from" not in wd or not is_int(wd["from"]):
                _bad()
            if "to" not in wd or not is_int(wd["to"]):
                _bad()
            if wd["from"] > wd["to"]:
                _bad()
        opt("scope_md", is_prose)
    elif etype == "revocation":
        req("grant", lambda v: is_hex(v, 32))
        opt("case", lambda v: is_hex(v, 32))
    elif etype == "adoption":
        a = d.get("anchors")
        if not isinstance(a, list) or len(a) == 0:
            _bad()
        for el in a:
            if not isinstance(el, Obj):
                _bad()
            ed = el.dict()
            if "chainId" not in ed or not is_int(ed["chainId"]):
                _bad()
            if "tx" not in ed or not is_hex(ed["tx"], 32):
                _bad()
            if "payloadKind" not in ed or not is_token(ed["payloadKind"]):
                _bad()
            if "content" not in ed or not is_hex(ed["content"], 32):
                _bad()
        if ("attestor" in d) != ("attestation" in d):
            _bad()
        opt("attestor", lambda v: is_hex(v, 20))
        opt("attestation", lambda v: is_hex(v, 65))
    elif etype == "succession":
        req("to", lambda v: is_hex(v, 20))
        req("kind", is_token)
        req("effective", is_int)
        req("statement_md", is_prose)
    elif etype == "annotation":
        opt("subject", lambda v: is_hex(v, 32))
        req("note_md", is_prose)
    # s6.9: an unknown type has only the s6.10 test, already met by step 9.


# --------------------------------------------------------------------------
# s5 signature
# --------------------------------------------------------------------------

def split_sig(sighex):
    raw = hexb(sighex)
    r = int.from_bytes(raw[0:32], "big")
    s = int.from_bytes(raw[32:64], "big")
    v = raw[64]
    return r, s, v


def check_sig(digest, sighex, expected_addr_hex, tokens=True):
    """s5.4 and s5.5.  Raises Fault when `tokens`, else returns bool."""
    r, s, v = split_sig(sighex)
    def fail(tok):
        if tokens:
            raise Fault(tok)
        return False
    if v != 27 and v != 28:
        return fail("E_SIG_V")
    if not (1 <= r <= N - 1) or not (1 <= s <= N - 1):
        return fail("E_SIG_RANGE")
    if s > HALF_N:
        return fail("E_SIG_HIGH_S")
    pt = recover(digest, r, s, v - 27)
    if pt is None:
        return fail("E_SIG_RECOVER")
    if "0x" + address_of(pt).hex() != expected_addr_hex:
        return fail("E_SIG_SIGNER")
    return True


def presig_of(six_member_bytes):
    return sha256(six_member_bytes)


def entry_digest(b6):
    return eip191_digest(message(DOMAIN_ENTRY, sha256(b6)))


# --------------------------------------------------------------------------
# s4.3 the thirteen steps
# --------------------------------------------------------------------------

class Entry:
    __slots__ = ("raw", "value", "eid", "eidb", "etype", "author", "seq",
                 "prev", "body")

    def __init__(self, raw, value, eid, etype, author, seq, prev, body):
        self.raw = raw
        self.value = value
        self.eid = eid
        self.eidb = bytes.fromhex(eid[2:])
        self.etype = etype
        self.author = author
        self.seq = seq
        self.prev = prev
        self.body = body


def validate(b):
    """Decide s4.3 over the byte string `b`.  Raises Fault, or returns Entry."""
    v = accept(b)                                                   # 1
    if not isinstance(v, Obj):                                      # 2
        raise Fault("E_ENVELOPE")
    d = v.dict()
    for k in SEVEN:                                                 # 3
        if k not in d:
            raise Fault("E_ENVELOPE_MISSING")
    for k in d:
        if k not in SEVEN_SET:
            raise Fault("E_ENVELOPE_CLOSED")
    if not (isinstance(d["spec"], str) and d["spec"] == "zikaron/1"):  # 4
        raise Fault("E_SPEC")
    if not is_token(d["entryType"]):                                # 5
        raise Fault("E_ENTRYTYPE")
    if not is_hex(d["author"], 20):                                 # 6
        raise Fault("E_AUTHOR")
    if not is_int(d["seq"]):                                        # 7
        raise Fault("E_SEQ")
    prev = d["prev"]
    if not (prev is None or is_hex(prev, 32)):                      # 8
        raise Fault("E_PREV")
    if (prev is None) != (d["seq"] == 0):
        raise Fault("E_PREV_SEQ")
    if not isinstance(d["body"], Obj):                              # 9
        raise Fault("E_BODY")
    if not is_hex(d["sig"], 65):                                    # 10
        raise Fault("E_SIG_FORM")
    if (d["entryType"] == "genesis") != (d["seq"] == 0):            # 11
        raise Fault("E_GENESIS_PLACE")
    _check_body(d["entryType"], d["body"])                          # 12
    b6 = canon(Obj([(k, val) for k, val in v.pairs if k != "sig"]))  # 13
    check_sig(entry_digest(b6), d["sig"], d["author"])
    return Entry(b, v, "0x" + sha256(b).hex(), d["entryType"], d["author"],
                 d["seq"], prev, d["body"])


# --------------------------------------------------------------------------
# s6.6 adoption attestation
# --------------------------------------------------------------------------

def attestation_preimage(entry):
    d = entry.body.dict()
    return canon(Obj([("adopter", entry.author),
                      ("anchors", d["anchors"]),
                      ("prev", entry.prev)]))


def attestation_ok(entry):
    """s6.6 verification.  False when absent or failing any test."""
    d = entry.body.dict()
    if "attestor" not in d or "attestation" not in d:
        return False
    if entry.prev is None:
        return False
    C = attestation_preimage(entry)
    digest = eip191_digest(message(DOMAIN_ADOPTION, sha256(C)))
    return check_sig(digest, d["attestation"], d["attestor"], tokens=False)
