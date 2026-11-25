#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
gen.py - corpus generator for the `zikaron/2` convergence run.

Written from docs/zikaron-v2.md and zikaron-conformance/HARNESS2.md alone, by
carrying over the zikaron/1 generator where the two texts say the same thing
in the same words (canonical form, signing, chain, the walk) and rewriting
every part the second law states differently (the eight bodies, retirement
and timeliness, the migration, the three audit inputs, the continuation of a
zikaron/1 ledger, whose old segment this generator predicts with the zikaron/1
generator's own predictor, imported as a tool and never as evidence). No
implementation of the law was consulted for any part but `build_readings`,
whose docstring states where its expectations come from. Everything here (canonicalizer, parser, secp256k1 with
RFC 6979 and low-s, EIP-191 digest, envelope and body tests, audit walk) is the
corpus author's own reading, and is recorded in manifest.json as a *prediction*,
a third opinion beside the two implementations under test. Where the author's
prose reasoning about a case disagreed with the author's own code, the manifest
carries both and the case is flagged `disagreement: true`.

Usage:  python3 gen.py --seed 1
Populates ./canon, ./sign, ./audit and writes manifest.json.
"""

import argparse
import hashlib
import hmac
import json
import os
import random
import shutil
import sys
from functools import lru_cache

from Crypto.Hash import keccak as _keccak

sys.setrecursionlimit(20000)

HERE = os.path.dirname(os.path.abspath(__file__))
MAXINT = (1 << 53) - 1

# the zikaron/1 generator, for the continued segment's prediction (a tool here)
import importlib.util as _ilu
_v1spec = _ilu.spec_from_file_location("zk1gen", os.path.join(os.path.dirname(HERE), "corpus", "gen.py"))
v1gen = _ilu.module_from_spec(_v1spec)
_v1spec.loader.exec_module(v1gen)

# --------------------------------------------------------------------------
# hashes
# --------------------------------------------------------------------------


def keccak256(b: bytes) -> bytes:
    h = _keccak.new(digest_bits=256)
    h.update(b)
    return h.digest()


def sha256(b: bytes) -> bytes:
    return hashlib.sha256(b).digest()


def hx(b: bytes) -> str:
    return "0x" + b.hex()


# --------------------------------------------------------------------------
# secp256k1 (pure python), RFC 6979, low-s, EIP-191, address recovery
# --------------------------------------------------------------------------

P = 2 ** 256 - 2 ** 32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
HALF_N = (N - 1) // 2
GX = 0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798
GY = 0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8
G = (GX, GY)


def _jac_double(p):
    x, y, z = p
    if y == 0:
        return (0, 0, 0)
    ysq = (y * y) % P
    s = (4 * x * ysq) % P
    m = (3 * x * x) % P
    nx = (m * m - 2 * s) % P
    ny = (m * (s - nx) - 8 * ysq * ysq) % P
    nz = (2 * y * z) % P
    return (nx, ny, nz)


def _jac_add(p, q):
    if p[2] == 0:
        return q
    if q[2] == 0:
        return p
    x1, y1, z1 = p
    x2, y2, z2 = q
    z1s = (z1 * z1) % P
    z2s = (z2 * z2) % P
    u1 = (x1 * z2s) % P
    u2 = (x2 * z1s) % P
    s1 = (y1 * z2s * z2) % P
    s2 = (y2 * z1s * z1) % P
    if u1 == u2:
        if s1 != s2:
            return (0, 0, 0)
        return _jac_double(p)
    h = (u2 - u1) % P
    r = (s2 - s1) % P
    h2 = (h * h) % P
    h3 = (h2 * h) % P
    u1h2 = (u1 * h2) % P
    nx = (r * r - h3 - 2 * u1h2) % P
    ny = (r * (u1h2 - nx) - s1 * h3) % P
    nz = (h * z1 * z2) % P
    return (nx, ny, nz)


def _jac_mul(p, k):
    if k % N == 0 or p[2] == 0:
        return (0, 0, 0)
    r = (0, 0, 0)
    q = p
    while k:
        if k & 1:
            r = _jac_add(r, q)
        q = _jac_double(q)
        k >>= 1
    return r


def _to_affine(p):
    if p[2] == 0:
        return None
    zi = pow(p[2], P - 2, P)
    zi2 = (zi * zi) % P
    return ((p[0] * zi2) % P, (p[1] * zi2 * zi) % P)


def mul(pt, k):
    """Affine scalar multiplication; None is the point at infinity."""
    if pt is None:
        return None
    return _to_affine(_jac_mul((pt[0], pt[1], 1), k % N))


def add(a, b):
    if a is None:
        return b
    if b is None:
        return a
    return _to_affine(_jac_add((a[0], a[1], 1), (b[0], b[1], 1)))


def on_curve(x, y):
    return (y * y - x * x * x - 7) % P == 0


def privkey_to_pub(d):
    return mul(G, d)


def pub_to_addr(pub):
    x, y = pub
    return keccak256(x.to_bytes(32, "big") + y.to_bytes(32, "big"))[12:]


def privkey_to_addr_hex(d):
    return hx(pub_to_addr(privkey_to_pub(d)))


def rfc6979_k(d: int, h1: bytes):
    """RFC 6979 section 3.2 with HMAC-SHA256, qlen = hlen = 256."""
    x = d.to_bytes(32, "big")
    z1 = int.from_bytes(h1, "big")
    z2 = z1 - N if z1 >= N else z1
    bo = z2.to_bytes(32, "big")
    v = b"\x01" * 32
    k = b"\x00" * 32
    k = hmac.new(k, v + b"\x00" + x + bo, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    k = hmac.new(k, v + b"\x01" + x + bo, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    while True:
        v = hmac.new(k, v, hashlib.sha256).digest()
        cand = int.from_bytes(v, "big")
        if 1 <= cand < N:
            yield cand
        k = hmac.new(k, v + b"\x00", hashlib.sha256).digest()
        v = hmac.new(k, v, hashlib.sha256).digest()


def ecdsa_sign(d: int, digest: bytes):
    """Returns (r, s, v) with RFC 6979 nonce and low-s; v in {27, 28}."""
    z = int.from_bytes(digest, "big") % N
    for k in rfc6979_k(d, digest):
        Rp = mul(G, k)
        if Rp is None:
            continue
        r = Rp[0] % N
        if r == 0:
            continue
        s = (pow(k, N - 2, N) * (z + r * d)) % N
        if s == 0:
            continue
        rec = (Rp[1] & 1) | (2 if Rp[0] >= N else 0)
        if s > HALF_N:
            s = N - s
            rec ^= 1
        if rec > 1:
            continue
        return r, s, 27 + rec
    raise AssertionError("unreachable")


def ecdsa_recover(digest: bytes, r: int, s: int, v: int):
    """Recovery exactly as sec 5.4 defines it. Returns a pubkey or None."""
    i = v - 27
    if i not in (0, 1):
        return None
    if not (1 <= r <= N - 1 and 1 <= s <= N - 1):
        return None
    x = r
    alpha = (x * x * x + 7) % P
    beta = pow(alpha, (P + 1) // 4, P)
    if (beta * beta) % P != alpha:
        return None
    y = beta if (beta % 2 == i) else (P - beta)
    R = (x, y)
    z = int.from_bytes(digest, "big") % N
    rinv = pow(r, N - 2, N)
    sR = mul(R, s)
    zG = mul(G, (-z) % N)
    Q = add(sR, zG)
    if Q is None:
        return None
    Q = mul(Q, rinv)
    if Q is None:
        return None
    return Q


def eip191_digest(domain: str, presig: bytes) -> bytes:
    m = domain.encode("ascii") + b"\x0a" + hx(presig).encode("ascii")
    return keccak256(b"\x19" + b"Ethereum Signed Message:\x0a" + str(len(m)).encode("ascii") + m)


def sig_hex(r, s, v):
    return hx(r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([v & 0xFF]))


# --------------------------------------------------------------------------
# the value universe (sec 3.1) and canonical bytes (sec 3.4)
# --------------------------------------------------------------------------


class JObj(object):
    """A JSON object as an ordered member list, so duplicate keys and
    non-canonical member order survive parsing."""

    __slots__ = ("members",)

    def __init__(self, members=None):
        self.members = list(members or [])

    def get(self, k, default=None):
        for kk, vv in self.members:
            if kk == k:
                return vv
        return default

    def has(self, k):
        return any(kk == k for kk, _ in self.members)

    def keys(self):
        return [k for k, _ in self.members]

    def __repr__(self):
        return "JObj(%r)" % (self.members,)


_ESC = {
    '"': '\\"',
    "\\": "\\\\",
    "\b": "\\b",
    "\t": "\\t",
    "\n": "\\n",
    "\f": "\\f",
    "\r": "\\r",
}


def cstring(s: str) -> bytes:
    out = ['"']
    for ch in s:
        e = _ESC.get(ch)
        if e is not None:
            out.append(e)
        elif ord(ch) < 0x20:
            out.append("\\u00%02x" % ord(ch))
        else:
            out.append(ch)
    out.append('"')
    return "".join(out).encode("utf-8")


def cbytes(v, sort=True) -> bytes:
    """Canonical bytes per sec 3.4. sort=False keeps the written member order,
    which is only ever used to build deliberately non-canonical inputs."""
    if v is None:
        return b"null"
    if v is True:
        return b"true"
    if v is False:
        return b"false"
    if isinstance(v, int):
        return str(v).encode("ascii")
    if isinstance(v, str):
        return cstring(v)
    if isinstance(v, list):
        return b"[" + b",".join(cbytes(e, sort) for e in v) + b"]"
    if isinstance(v, JObj):
        ms = v.members
        if sort:
            ms = sorted(ms, key=lambda kv: kv[0].encode("utf-8"))
        return b"{" + b",".join(cstring(k) + b":" + cbytes(val, sort) for k, val in ms) + b"}"
    raise TypeError("not a value of the universe: %r" % (v,))


def J(x):
    """Convert plain python containers into the universe's representation."""
    if isinstance(x, dict):
        return JObj([(k, J(v)) for k, v in x.items()])
    if isinstance(x, list):
        return [J(e) for e in x]
    return x


# --------------------------------------------------------------------------
# the parser: sec 3.5 test 2 (RFC 8259 JSON-text, E_DEPTH, E_NUMBER, surrogates)
# --------------------------------------------------------------------------

WS = " \t\n\r"
DIGITS = "0123456789"
NUMBYTES = "0123456789+-.eE"
HEXDIG = "0123456789abcdefABCDEF"


class Fault(Exception):
    def __init__(self, token):
        Exception.__init__(self, token)
        self.token = token


def is_int_literal(t: str) -> bool:
    if not t:
        return False
    for c in t:
        if c not in DIGITS:
            return False
    if len(t) > 1 and t[0] == "0":
        return False
    return int(t) <= MAXINT


class Parser(object):
    """Left-to-right single pass. Because the scan never backs up, the first
    fault raised is the fault whose triggering byte comes earliest in b, which
    is the rule sec 3.5 test 2 states."""

    def __init__(self, s: str):
        self.s = s
        self.i = 0
        self.n = len(s)
        self.depth = 0

    def err(self):
        raise Fault("E_JSON")

    def ws(self):
        while self.i < self.n and self.s[self.i] in WS:
            self.i += 1

    def text(self):
        self.ws()
        v = self.value()
        self.ws()
        if self.i != self.n:
            self.err()
        return v

    def value(self):
        if self.i >= self.n:
            self.err()
        c = self.s[self.i]
        if c in NUMBYTES and c not in "eE":
            return self.number()
        if c == '"':
            return self.string()
        if c == "{":
            return self.obj()
        if c == "[":
            return self.arr()
        if self.s.startswith("true", self.i):
            self.i += 4
            return True
        if self.s.startswith("false", self.i):
            self.i += 5
            return False
        if self.s.startswith("null", self.i):
            self.i += 4
            return None
        self.err()

    def number(self):
        j = self.i
        while j < self.n and self.s[j] in NUMBYTES:
            j += 1
        run = self.s[self.i:j]
        if not is_int_literal(run):
            raise Fault("E_NUMBER")
        self.i = j
        return int(run)

    def string(self):
        self.i += 1  # opening quote
        out = []
        while True:
            if self.i >= self.n:
                self.err()
            c = self.s[self.i]
            if c == '"':
                self.i += 1
                return "".join(out)
            if c == "\\":
                self.i += 1
                if self.i >= self.n:
                    self.err()
                e = self.s[self.i]
                if e in '"\\/':
                    out.append(e)
                    self.i += 1
                elif e == "b":
                    out.append("\b")
                    self.i += 1
                elif e == "f":
                    out.append("\f")
                    self.i += 1
                elif e == "n":
                    out.append("\n")
                    self.i += 1
                elif e == "r":
                    out.append("\r")
                    self.i += 1
                elif e == "t":
                    out.append("\t")
                    self.i += 1
                elif e == "u":
                    h = self.s[self.i + 1:self.i + 5]
                    if len(h) != 4 or any(x not in HEXDIG for x in h):
                        self.err()
                    cp = int(h, 16)
                    if 0xD800 <= cp <= 0xDFFF:
                        raise Fault("E_JSON")
                    out.append(chr(cp))
                    self.i += 5
                else:
                    self.err()
            elif ord(c) < 0x20:
                self.err()
            else:
                out.append(c)
                self.i += 1

    def _open(self):
        self.depth += 1
        if self.depth == 129:
            raise Fault("E_DEPTH")

    def obj(self):
        self._open()
        self.i += 1
        members = []
        self.ws()
        if self.i < self.n and self.s[self.i] == "}":
            self.i += 1
            self.depth -= 1
            return JObj(members)
        while True:
            self.ws()
            if self.i >= self.n or self.s[self.i] != '"':
                self.err()
            k = self.string()
            self.ws()
            if self.i >= self.n or self.s[self.i] != ":":
                self.err()
            self.i += 1
            self.ws()
            v = self.value()
            members.append((k, v))
            self.ws()
            if self.i >= self.n:
                self.err()
            if self.s[self.i] == ",":
                self.i += 1
                continue
            if self.s[self.i] == "}":
                self.i += 1
                break
            self.err()
        self.depth -= 1
        return JObj(members)

    def arr(self):
        self._open()
        self.i += 1
        out = []
        self.ws()
        if self.i < self.n and self.s[self.i] == "]":
            self.i += 1
            self.depth -= 1
            return out
        while True:
            self.ws()
            out.append(self.value())
            self.ws()
            if self.i >= self.n:
                self.err()
            if self.s[self.i] == ",":
                self.i += 1
                continue
            if self.s[self.i] == "]":
                self.i += 1
                break
            self.err()
        self.depth -= 1
        return out


# --------------------------------------------------------------------------
# sec 3.5 tests 3, 4, 5 and 6
# --------------------------------------------------------------------------


def has_dup_key(v) -> bool:
    if isinstance(v, list):
        return any(has_dup_key(e) for e in v)
    if isinstance(v, JObj):
        seen = set()
        for k, val in v.members:
            if k in seen:
                return True
            seen.add(k)
        return any(has_dup_key(val) for _, val in v.members)
    return False


def _skeleton(s: str) -> bool:
    return all(0x20 <= ord(c) <= 0x7E for c in s)


def bad_key(v) -> bool:
    if isinstance(v, list):
        return any(bad_key(e) for e in v)
    if isinstance(v, JObj):
        for k, val in v.members:
            if len(k) == 0 or not _skeleton(k):
                return True
            if bad_key(val):
                return True
    return False


def bad_value_charset(v, prose=False) -> bool:
    if isinstance(v, str):
        return (not prose) and (not _skeleton(v))
    if isinstance(v, list):
        return any(bad_value_charset(e, prose) for e in v)
    if isinstance(v, JObj):
        for k, val in v.members:
            if bad_value_charset(val, prose or k.endswith("_md")):
                return True
    return False


def accept_canonical(b: bytes):
    """sec 3.5. Returns (value, None) or (None, token)."""
    try:
        s = b.decode("utf-8")
    except UnicodeDecodeError:
        return None, "E_UTF8"
    try:
        v = Parser(s).text()
    except Fault as f:
        return None, f.token
    if has_dup_key(v):
        return None, "E_DUP_KEY"
    if bad_key(v):
        return None, "E_KEY_CHARSET"
    if bad_value_charset(v):
        return None, "E_VALUE_CHARSET"
    if cbytes(v) != b:
        return None, "E_NOT_CANONICAL"
    return v, None


def canon_command(b: bytes):
    """`zk1 canon`: sec 3.5 tests 1 through 5 only."""
    try:
        s = b.decode("utf-8")
    except UnicodeDecodeError:
        return None, "E_UTF8"
    try:
        v = Parser(s).text()
    except Fault as f:
        return None, f.token
    if has_dup_key(v):
        return None, "E_DUP_KEY"
    if bad_key(v):
        return None, "E_KEY_CHARSET"
    if bad_value_charset(v):
        return None, "E_VALUE_CHARSET"
    return cbytes(v), None


# --------------------------------------------------------------------------
# sec 4 / 5 / 6: the envelope, the body tables, the signature
# --------------------------------------------------------------------------

SEVEN = ("body", "entryType", "author", "prev", "seq", "sig", "spec")
KNOWN_TYPES = ("genesis", "history", "grant", "revocation", "succession", "migration", "annotation",
               "retraction")


def is_str(v):
    return isinstance(v, str)


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool) and 0 <= v <= MAXINT


def is_token(v):
    return isinstance(v, str) and len(v) > 0 and all(0x21 <= ord(c) <= 0x7E for c in v)


def _is_hex(v, ndig):
    if not isinstance(v, str) or len(v) != ndig + 2:
        return False
    if v[0] != "0" or v[1] != "x":
        return False
    return all(c in "0123456789abcdef" for c in v[2:])


def is_hex20(v):
    return _is_hex(v, 40)


def is_hex32(v):
    return _is_hex(v, 64)


def is_hex65(v):
    return _is_hex(v, 130)


def body_fault(etype, body: JObj):
    """sec 6. Returns 'E_BODY_FIELD' or None."""
    E = "E_BODY_FIELD"

    def req(k, pred):
        return body.has(k) and pred(body.get(k))

    def opt(k, pred):
        return (not body.has(k)) or pred(body.get(k))

    def obj_rows(k, required, optional):
        if not body.has(k):
            return True
        o = body.get(k)
        if not isinstance(o, JObj):
            return False
        for kk, pred in required:
            if not (o.has(kk) and pred(o.get(kk))):
                return False
        for kk, pred in optional:
            if o.has(kk) and not pred(o.get(kk)):
                return False
        return True

    if etype == "genesis":
        if not req("statement_md", is_str):
            return E
        if not obj_rows("continues", [("root", is_hex20), ("entry_id", is_hex32)], []):
            return E
        return None
    if etype == "history":
        if not req("content", is_hex32):
            return E
        if not body.has("mode"):
            return E
        if not obj_rows("mode", [("mark", is_token), ("toolchain", is_hex32)], []):
            return E
        if not opt("note_md", is_str):
            return E
        if not obj_rows("for", [("app", is_token), ("identity", is_hex20), ("ref", is_token)],
                        [("seat", is_token)]):
            return E
        return None
    if etype == "grant":
        if not req("grantee", is_hex20):
            return E
        if not req("work", is_hex32):
            return E
        if not req("terms", is_hex32):
            return E
        if not opt("history", is_hex32):
            return E
        if body.has("window"):
            w = body.get("window")
            if not isinstance(w, JObj):
                return E
            if not (w.has("from") and is_int(w.get("from"))):
                return E
            if not (w.has("to") and is_int(w.get("to"))):
                return E
            if w.get("from") > w.get("to"):
                return E
        if not opt("scope_md", is_str):
            return E
        if not opt("upstream", is_hex32):
            return E
        if not opt("ledger", is_hex20):
            return E
        return None
    if etype == "revocation":
        if not req("grant", is_hex32):
            return E
        if not opt("case", is_hex32):
            return E
        return None
    if etype == "succession":
        if not req("to", is_hex20):
            return E
        if not req("kind", is_token):
            return E
        if not req("effective", is_int):
            return E
        if not req("statement_md", is_str):
            return E
        return None
    if etype == "migration":
        if not req("spec", is_token):
            return E
        if not req("statement_md", is_str):
            return E
        return None
    if etype == "annotation":
        if not opt("subject", is_hex32):
            return E
        if not req("note_md", is_str):
            return E
        return None
    if etype == "retraction":
        if not req("subject", is_hex32):
            return E
        if not opt("note_md", is_str):
            return E
        return None
    return None  # sec 6.9: unknown type, only sec 6.10 applies


def b6_bytes(env: JObj) -> bytes:
    return cbytes(JObj([(k, v) for k, v in env.members if k != "sig"]))


def sig_fault(env: JObj):
    """sec 5.4 bullet order then sec 5.5. Returns token or None."""
    sig = env.get("sig")
    raw = bytes.fromhex(sig[2:])
    r = int.from_bytes(raw[0:32], "big")
    s = int.from_bytes(raw[32:64], "big")
    v = raw[64]
    if v not in (27, 28):
        return "E_SIG_V"
    if not (1 <= r <= N - 1) or not (1 <= s <= N - 1):
        return "E_SIG_RANGE"
    if s > HALF_N:
        return "E_SIG_HIGH_S"
    presig = sha256(b6_bytes(env))
    digest = eip191_digest("zikaron/2", presig)
    pub = _recover_cached(digest, r, s, v)
    if pub is None:
        return "E_SIG_RECOVER"
    if hx(pub_to_addr(pub)) != env.get("author"):
        return "E_SIG_SIGNER"
    return None


@lru_cache(maxsize=100000)
def _recover_cached(digest, r, s, v):
    return ecdsa_recover(digest, r, s, v)


@lru_cache(maxsize=200000)
def zk_check(b: bytes):
    """`zk1 check`: sec 3.5 then sec 4.3. Returns (True, entry_id) or (False, token)."""
    v, tok = accept_canonical(b)
    if tok:
        return (False, tok)
    if not isinstance(v, JObj):
        return (False, "E_ENVELOPE")
    ks = set(v.keys())
    if not set(SEVEN) <= ks:
        return (False, "E_ENVELOPE_MISSING")
    if ks - set(SEVEN):
        return (False, "E_ENVELOPE_CLOSED")
    if v.get("spec") != "zikaron/2":
        return (False, "E_SPEC")
    if not is_token(v.get("entryType")):
        return (False, "E_ENTRYTYPE")
    if not is_hex20(v.get("author")):
        return (False, "E_AUTHOR")
    if not is_int(v.get("seq")):
        return (False, "E_SEQ")
    prev = v.get("prev")
    if not (prev is None or is_hex32(prev)):
        return (False, "E_PREV")
    if (prev is None) != (v.get("seq") == 0):
        return (False, "E_PREV_SEQ")
    if not isinstance(v.get("body"), JObj):
        return (False, "E_BODY")
    if not is_hex65(v.get("sig")):
        return (False, "E_SIG_FORM")
    if (v.get("entryType") == "genesis") != (v.get("seq") == 0):
        return (False, "E_GENESIS_PLACE")
    bf = body_fault(v.get("entryType"), v.get("body"))
    if bf:
        return (False, bf)
    sf = sig_fault(v)
    if sf:
        return (False, sf)
    return (True, hx(sha256(b)))


def parsed_entry(b: bytes):
    """For the audit predictor: the parsed envelope of a byte string that is an entry."""
    v, tok = accept_canonical(b)
    return v


# --------------------------------------------------------------------------
# entry construction
# --------------------------------------------------------------------------


def build_env(spec, entryType, author, seq, prev, body):
    return JObj([
        ("body", body),
        ("entryType", entryType),
        ("author", author),
        ("prev", prev),
        ("seq", seq),
        ("spec", spec),
    ])


def sign_env(priv, env6: JObj):
    b6 = b6_bytes(env6) if env6.has("sig") else cbytes(env6)
    presig = sha256(b6)
    digest = eip191_digest("zikaron/2", presig)
    r, s, v = ecdsa_sign(priv, digest)
    return sig_hex(r, s, v), presig, digest, (r, s, v)


def make_entry(priv, entryType, seq, prev, body, spec="zikaron/2", author=None,
               sig_override=None, extra_env=None, unsorted=False):
    """Build the canonical bytes of a signed entry (or of a deliberately
    faulty near-entry).  `body` is a plain dict/JObj."""
    if author is None:
        author = privkey_to_addr_hex(priv)
    if not isinstance(body, JObj) and isinstance(body, dict):
        body = J(body)
    env6 = build_env(spec, entryType, author, seq, prev, body)
    if extra_env:
        for k, val in extra_env:
            env6.members.append((k, J(val)))
    if sig_override is not None:
        sig = sig_override
    else:
        sig, _, _, _ = sign_env(priv, env6)
    full = JObj(env6.members + [("sig", sig)])
    return cbytes(full, sort=not unsorted)


def entry_digest(author_hex, entryType, seq, prev, body, spec="zikaron/2"):
    env6 = build_env(spec, entryType, author_hex, seq, prev, J(body) if isinstance(body, dict) else body)
    presig = sha256(cbytes(env6))
    return eip191_digest("zikaron/2", presig), presig


def eid(b: bytes) -> str:
    return hx(sha256(b))


# --------------------------------------------------------------------------
# zikaron/1 entries, for the continued segment (sec 12.6)
# --------------------------------------------------------------------------


def make_entry_v1(priv, entryType, seq, prev, body, author=None):
    """A zikaron/1 entry, signed under that law's domain (the zikaron/1
    generator's own producer)."""
    return v1gen.make_entry(priv, entryType, seq, prev, body, spec="zikaron/1", author=author)


def v1_check(b: bytes):
    return v1gen.zk_check(b)


# --------------------------------------------------------------------------
# the audit predictor (sec 8, sec 9.4 well-formedness)
# --------------------------------------------------------------------------


def _basis_ok(basis):
    if not isinstance(basis, dict):
        return False
    if set(basis.keys()) != {"chains"}:
        return False
    ch = basis["chains"]
    if not isinstance(ch, list):
        return False
    prev = None
    for o in ch:
        if not isinstance(o, dict):
            return False
        if set(o.keys()) != {"chainId", "fromBlock", "toBlock", "registries", "senders"}:
            return False
        if not (is_int(o["chainId"]) and is_int(o["fromBlock"]) and is_int(o["toBlock"])):
            return False
        if o["fromBlock"] > o["toBlock"]:
            return False
        for arr, pred in ((o["registries"], is_hex20), (o["senders"], is_hex20)):
            if not isinstance(arr, list) or not all(pred(x) for x in arr):
                return False
            if any(arr[i].encode() >= arr[i + 1].encode() for i in range(len(arr) - 1)):
                return False
        lists = (tuple(o["registries"]), tuple(o["senders"]))
        if prev is not None:
            if (o["chainId"], o["fromBlock"]) <= (prev[0], prev[1]):
                return False
            if o["chainId"] == prev[0]:
                if o["fromBlock"] <= prev[2]:
                    return False
                if o["fromBlock"] == prev[2] + 1 and lists == prev[3]:
                    return False
        prev = (o["chainId"], o["fromBlock"], o["toBlock"], lists)
    return True


class Ent(object):
    __slots__ = ("b", "id", "env", "seq", "prev", "author", "etype")

    def __init__(self, b, env):
        self.b = b
        self.id = hx(sha256(b))
        self.env = env
        self.seq = env.get("seq")
        self.prev = env.get("prev")
        self.author = env.get("author")
        self.etype = env.get("entryType")


def _hexn_ok(v, n):
    return isinstance(v, str) and len(v) == 2 + 2 * n and v[:2] == "0x" and all(c in "0123456789abcdef" for c in v[2:])


def _nums_in_universe(v):
    """HARNESS: every number in the file, wherever it sits, is an int of sec 3.1, and
    every string decodes to scalar values."""
    if isinstance(v, bool):
        return True
    if isinstance(v, str):
        return not any(0xD800 <= ord(c) <= 0xDFFF for c in v)
    if isinstance(v, int):
        return 0 <= v <= MAXINT
    if isinstance(v, float):
        return False
    if isinstance(v, list):
        return all(_nums_in_universe(x) for x in v)
    if isinstance(v, dict):
        return all(_nums_in_universe(x) for x in v.values())
    return True


def _depth(v):
    if isinstance(v, list):
        return 1 + max([_depth(x) for x in v] + [0])
    if isinstance(v, dict):
        return 1 + max([_depth(x) for x in v.values()] + [0])
    return 0


def _inputs_well_formed(inp):
    """sec 8 with sec 9.4: the three inputs by their forms; any failure is no label."""
    if not isinstance(inp, dict):
        return False
    if not _nums_in_universe(inp) or _depth(inp) > 128:
        return False
    for k in ("root", "pile", "anchors", "basis"):
        if k not in inp:
            return False
    if not _hexn_ok(inp["root"], 20):
        return False
    if not isinstance(inp["pile"], list) or not all(isinstance(x, str) and x[:2] == "0x" and len(x) % 2 == 0 and all(c in "0123456789abcdefABCDEF" for c in x[2:]) for x in inp["pile"]):
        return False
    if not isinstance(inp["anchors"], list):
        return False
    for a in inp["anchors"]:
        if not isinstance(a, dict):
            return False
        for k in ("chainId", "blockNumber", "blockTimestamp"):
            if not is_int(a.get(k)):
                return False
        if not (_hexn_ok(a.get("tx"), 32) and _hexn_ok(a.get("sender"), 20) and _hexn_ok(a.get("hash"), 32)):
            return False
        if a.get("verdict") not in ("counted", "UNPROVEN", "VOID"):
            return False
    return True


COUNT_KEYS = ("entries", "findings", "hard", "f_SEQ_GAP", "f_PREV_MISMATCH", "f_AUTHORITY_MISMATCH",
              "f_ROOT_MISMATCH", "f_EQUIVOCATION", "missing", "anchored", "unanchored", "excluded",
              "retired", "unknown_type", "malformed", "unproven", "void", "discarded", "continues")


def _line(e, by_id):
    """e, then along prev through held entries, stopping where none matches."""
    out, seen, cur = [], set(), e
    while cur is not None and cur.id not in seen:
        seen.add(cur.id)
        out.append(cur)
        cur = by_id.get(cur.prev) if cur.prev is not None else None
    return out


def _v1_ledger_ids(root, raw):
    """The zikaron/1 ledger entry ids over a pile, by the zikaron/1 generator's
    own reading (lineage over successions; no timeliness in that law)."""
    ents = {}
    for b in raw:
        ok, _ = v1_check(b)
        if ok and b not in ents:
            ents[b] = Ent(b, v1gen.parsed_entry(b))
    lin = {root}
    changed = True
    while changed:
        changed = False
        for e in ents.values():
            if e.etype == "succession" and e.author in lin:
                t = e.env.get("body").get("to")
                if t not in lin:
                    lin.add(t)
                    changed = True
    return set(e.id for e in ents.values() if e.author in lin), ents


def _continuation(root, raw, anchors, basis):
    """sec 12.6: (genesis Ent, succession Ent, cut, old byte strings, v1 prediction)
    or None."""
    v2 = {}
    for b in raw:
        ok, _ = zk_check(b)
        if ok and b not in v2:
            v2[b] = Ent(b, parsed_entry(b))
    v1 = {}
    for b in raw:
        ok, _ = v1_check(b)
        if ok and b not in v1:
            v1[b] = Ent(b, v1gen.parsed_entry(b))
    v1_by_id = dict((e.id, e) for e in v1.values())
    by_hash = {}
    for a in anchors:
        by_hash.setdefault(a["hash"], []).append(a)
    best = None
    for g in v2.values():
        if g.seq != 0 or g.etype != "genesis":
            continue
        c = g.env.get("body").get("continues")
        if not isinstance(c, JObj):
            continue
        if c.get("root") != root:
            continue
        s = v1_by_id.get(c.get("entry_id"))
        if s is None or s.etype != "succession" or s.author != g.author:
            continue
        sb = s.env.get("body")
        if sb.get("to") != g.author or sb.get("kind") != "migration":
            continue
        recs = by_hash.get(s.id)
        if not recs:
            continue
        cut_ = min(a["blockTimestamp"] for a in recs)
        # sec 12.6: a key whose zikaron/1 handing off a record earlier than the cut anchors closes nothing
        if any(e.etype == "succession" and e.author == s.author and e.env.get("body").get("to") != s.author
               and any(a["blockTimestamp"] < cut_ for a in by_hash.get(e.id, []))
               for e in v1.values()):
            continue
        recs_le = [a for a in anchors if a["blockTimestamp"] <= cut_]
        old = set()
        for e in _line(s, v1_by_id):
            old.add(e.b)
        hashes = set(a["hash"] for a in recs_le)
        for b in set(raw):
            if hx(sha256(b)) in hashes:
                old.add(b)
                if b in v1:
                    for e in _line(v1[b], v1_by_id):
                        old.add(e.b)
        # sec 12.6: a record at or before the cut, or anchoring an old ledger
        # entry (zikaron/1 sec 8.1 over the old byte strings) whatever its time
        old_ids, _ = _v1_ledger_ids(root, sorted(old))
        recs_old = [a for a in anchors if a["blockTimestamp"] <= cut_ or a["hash"] in old_ids]
        oldinp = {"root": root, "pile": [hx(b) for b in sorted(old)], "anchors": recs_old,
                  "unavailable": [], "evidence": [],
                  "basis": {"chains": basis["chains"], "bareTx": [], "adoptionChains": []}}
        ledger_ids, _ = _v1_ledger_ids(root, sorted(old))
        if s.id not in ledger_ids:
            continue
        key = (cut_, bytes.fromhex(g.id[2:]))
        if best is None or key < best[0]:
            best = (key, (g, s, cut_, old, v1gen.predict_audit(oldinp), recs_old))
    return None if best is None else best[1]


LABEL_ORDER = ("BROKEN_CHAIN", "UNPROVEN", "GAPS", "COMPLETE")


def predict_audit(inp):
    """Predicts sec 8.6's label plus the list sizes. Not authoritative."""
    out = dict((k, 0) for k in COUNT_KEYS)
    out["label"] = None
    if not _inputs_well_formed(inp):
        out["label"] = "NO_LABEL"
        return out
    if not _basis_ok(inp["basis"]):
        out["label"] = "NO_LABEL"
        return out
    seen = {}
    SEVEN_A = ("chainId", "blockNumber", "blockTimestamp", "tx", "sender", "hash", "verdict")
    for a in inp["anchors"]:
        k = (a["chainId"], a["blockNumber"], a["tx"], a["hash"])
        a7 = {kk: a[kk] for kk in SEVEN_A}
        if k in seen and seen[k] != a7:
            out["label"] = "NO_LABEL"
            return out
        seen[k] = a7
    anchors = list(seen.values())
    bas = inp["basis"]
    for a in anchors:
        if not any(c["chainId"] == a["chainId"] and c["fromBlock"] <= a["blockNumber"] <= c["toBlock"]
                   and a["sender"] in c["senders"] for c in bas["chains"]):
            out["label"] = "NO_LABEL"
            return out

    root = inp["root"]
    raw = [bytes.fromhex(x[2:]) for x in inp["pile"]]
    cont = _continuation(root, raw, anchors, bas)
    if cont is None:
        seed, seg_raw, seg_anchors, v1label = root, raw, anchors, None
    else:
        g, s, cut_, old, v1pred, recs_old = cont
        old_v1 = set(b for b in old if v1_check(b)[0])
        seed = g.author
        seg_raw = [b for b in raw if b not in old_v1]
        old_keys = set((a["chainId"], a["blockNumber"], a["tx"], a["hash"]) for a in recs_old)
        seg_anchors = [a for a in anchors if (a["chainId"], a["blockNumber"], a["tx"], a["hash"]) not in old_keys]
        v1label = v1pred["label"]
        OLD_PRED[0] = v1pred
        CONT_SEL[0] = {"entry_id": s.id, "cut": cut_}
        out["continues"] = 1

    pile_ids = set(hx(sha256(x)) for x in raw)      # sec 8.6 item 15 rule 2: the whole pile
    ents = {}
    malformed = set()
    for b in seg_raw:
        ok, val = zk_check(b)
        if ok and b not in ents:
            ents[b] = Ent(b, parsed_entry(b))
        elif not ok:
            malformed.add(hx(sha256(b)))
    entries = list(ents.values())
    out["malformed"] = len(malformed)
    by_id = dict((e.id, e) for e in entries)

    # sec 7.4: retirement over every record of the anchor set as received
    ret = {}
    rseq = {}
    for a in seg_anchors:
        e = by_id.get(a["hash"])
        if e is not None and e.etype in ("succession", "migration"):
            if e.author not in ret or a["blockTimestamp"] < ret[e.author]:
                ret[e.author] = a["blockTimestamp"]
                rseq[e.author] = e.seq
            elif a["blockTimestamp"] == ret[e.author] and e.seq < rseq[e.author]:
                rseq[e.author] = e.seq

    def retired_at_key(key, a):
        # sec 7.4: later than the retirement, or of its second and anchoring a
        # byte string of the key at a seq above its retiring seq
        if key not in ret or a["blockTimestamp"] < ret[key]:
            return False
        if a["blockTimestamp"] > ret[key]:
            return True
        e = by_id.get(a["hash"])
        return e is not None and e.author == key and e.seq > rseq[key]
    # sec 7.4: timeliness through the lines of the anchored byte strings, by
    # records earlier than the entry's author's retirement, or of its second
    # while the entry sits at or below the author's retiring seq
    timely = set(e.id for e in entries if e.author not in ret)
    for a in seg_anchors:
        f = by_id.get(a["hash"])
        if f is None:
            continue
        for e in _line(f, by_id):
            if e.id in timely:
                continue
            r = ret[e.author]
            if a["blockTimestamp"] < r or (a["blockTimestamp"] == r and e.seq <= rseq[e.author]):
                timely.add(e.id)

    lineage = {seed}
    changed = True
    while changed:
        changed = False
        for e in entries:
            if e.etype == "succession" and e.id in timely and e.author in lineage:
                t = e.env.get("body").get("to")
                if t not in lineage:
                    lineage.add(t)
                    changed = True

    ledger = [e for e in entries if e.author in lineage and e.id in timely]
    out["entries"] = len(ledger)
    out["excluded"] = sum(1 for e in entries if e.author not in lineage)
    out["retired"] = sum(1 for e in entries if e.author in lineage and e.id not in timely)
    out["unknown_type"] = sum(1 for e in ledger if e.etype not in KNOWN_TYPES)

    def retired_at(a):
        return retired_at_key(a["sender"], a)
    out["discarded"] = sum(1 for a in seg_anchors if a["sender"] not in lineage or retired_at(a))
    kept = [a for a in seg_anchors if a["sender"] in lineage and not retired_at(a)]
    counted = [a for a in kept if a["verdict"] == "counted"]
    counted_hashes = set(a["hash"] for a in counted)
    unproven = [a for a in kept if a["verdict"] == "UNPROVEN"]
    out["unproven"] = len(unproven)
    out["void"] = sum(1 for a in kept if a["verdict"] == "VOID")

    order = sorted(ledger, key=lambda e: (e.seq, bytes.fromhex(e.id[2:])))
    by_seq = {}
    for e in order:
        by_seq.setdefault(e.seq, []).append(e)
    findings = []
    expected = 0
    auth = seed
    closed = False
    certain = True
    prev_seq = None
    for sq in sorted(by_seq):
        grp = by_seq[sq]
        for e in grp:
            if e.seq != expected and e.seq != prev_seq:
                findings.append(("SEQ_GAP", False))
                expected = e.seq if e.seq == MAXINT else e.seq + 1
                certain = False
            elif e.seq == expected:
                expected = e.seq if e.seq == MAXINT else e.seq + 1
            if e.seq >= 1:
                pool = by_seq.get(e.seq - 1)
                if pool and not any(x.id == e.prev for x in pool):
                    findings.append(("PREV_MISMATCH", True))
            if e.seq == 0:
                if e.author != seed:
                    findings.append(("ROOT_MISMATCH", True))
            else:
                if closed or e.author != auth:
                    findings.append(("AUTHORITY_MISMATCH", certain))
            prev_seq = e.seq
        if not closed:
            if any(e.etype == "migration" and e.author == auth for e in grp):
                closed = True
            else:
                succ = [e for e in grp if e.etype == "succession" and e.author == auth]
                if succ:
                    auth = min(succ, key=lambda e: bytes.fromhex(e.id[2:])).env.get("body").get("to")
    for i in range(len(order)):
        for j in range(i + 1, len(order)):
            a, b = order[i], order[j]
            if a.id == b.id:
                continue
            if a.seq == b.seq or (a.prev is not None and a.prev == b.prev):
                findings.append(("EQUIVOCATION", True))
    out["findings"] = len(findings)
    out["hard"] = sum(1 for _, h in findings if h)
    for nm, _h in findings:
        out["f_" + nm] = out.get("f_" + nm, 0) + 1
    have = set(e.id for e in ledger)
    missing = counted_hashes - have
    out["missing"] = len(missing)
    out["unanchored"] = sum(1 for e in ledger if e.id not in counted_hashes)
    out["anchored"] = sum(1 for e in ledger if e.id in counted_hashes)
    unproven_open = [a for a in unproven if a["hash"] not in pile_ids and a["hash"] not in counted_hashes]
    if out["hard"]:
        label = "BROKEN_CHAIN"
    elif unproven_open:
        label = "UNPROVEN"
    elif any(nm == "SEQ_GAP" for nm, _ in findings) or missing:
        label = "GAPS"
    else:
        label = "COMPLETE"
    if v1label is not None:
        other = "UNPROVEN" if v1label == "UNAVAILABLE" else v1label
        label = min((label, other), key=LABEL_ORDER.index)
    out["label"] = label
    return out


# --------------------------------------------------------------------------
# corpus bookkeeping
# --------------------------------------------------------------------------

MANIFEST = []
WARNINGS = []
_counters = {"canon": 0, "sign": 0, "audit": 0}


def _write(path, data: bytes):
    with open(path, "wb") as f:
        f.write(data)


def C(slug, data: bytes, note, expect=None):
    """Register a canon/ case. `expect` is the author's hand reasoning; the
    recorded prediction is the author's code. A disagreement is flagged."""
    _counters["canon"] += 1
    name = "c%04d_%s.bin" % (_counters["canon"], slug)
    _write(os.path.join(HERE, "canon", name), data)
    ok, tok = zk_check(data)
    pred = "ok" if ok else tok
    cb, ctok = canon_command(data)
    rec = {
        "path": "canon/" + name,
        "category": "canon",
        "note": note,
        "predicted": pred,
        "predicted_entry_id": tok if ok else None,
        "predicted_canon": "ok" if ctok is None else ctok,
        "predicted_canon_sha256": hx(sha256(cb)) if ctok is None else None,
        "bytes": len(data),
    }
    if expect is not None:
        rec["hand_expect"] = expect
        if expect != pred:
            rec["disagreement"] = True
            WARNINGS.append("canon/%s: hand=%s code=%s (%s)" % (name, expect, pred, note))
    MANIFEST.append(rec)
    return rec


def S(slug, priv, data: bytes, domain, note, key_spelling="0x", omit_domain=False, raw_bytes=None):
    """`key_spelling` is the transport spelling of the private key the case
    passes (`0x`, `0X`, or `bare`); `omit_domain` passes no domain argument,
    the harness defaulting it to `zikaron/2`."""
    _counters["sign"] += 1
    base = "s%03d_%s" % (_counters["sign"], slug)
    _write(os.path.join(HERE, "sign", base + ".bin"), data if raw_bytes is None else raw_bytes)
    presig = sha256(data)   # over the canonical bytes, whatever spelling the file carries
    if omit_domain:
        assert domain == "zikaron/2"
    digest = eip191_digest(domain, presig)
    r, s, v = ecdsa_sign(priv, digest)
    key_hex = "%064x" % priv
    spelled = {"0x": "0x" + key_hex, "0X": "0X" + key_hex.upper(), "bare": key_hex}[key_spelling]
    meta = {
        "input": base + ".bin",
        "privkey": spelled,
        "note": note,
        "predicted": {
            "presig": hx(presig),
            "digest": hx(digest),
            "sig": sig_hex(r, s, v),
            "signer": privkey_to_addr_hex(priv),
        },
    }
    if not omit_domain:
        meta["domain"] = domain
    with open(os.path.join(HERE, "sign", base + ".meta.json"), "w") as f:
        json.dump(meta, f, indent=1, sort_keys=True)
    rec = {
        "path": "sign/" + base + ".bin",
        "meta": "sign/" + base + ".meta.json",
        "category": "sign",
        "note": note,
        "privkey": meta["privkey"],
        "predicted": meta["predicted"],
    }
    if not omit_domain:
        rec["domain"] = domain
    MANIFEST.append(rec)


OLD_PRED = [None]
CONT_SEL = [None]
OLD_KEYS = ("label", "entries", "findings", "missing", "anchored", "unanchored", "excluded",
            "unknown_type", "malformed", "unproven", "void", "discarded", "unavailable", "adoption_unproven")


def A(slug, inp, note, expect_label=None, twin=True):
    """Register an audit/ case. Every case with array members also registers
    its twin with every input array reversed (sec 8: the report does not
    depend on the order in which any input was supplied), so that each sorted
    list of the report is witnessed from an input in another order."""
    if twin and isinstance(inp, dict):
        inp2 = dict((k, (list(reversed(v)) if isinstance(v, list) else v)) for k, v in inp.items())
        if inp2 != inp:
            A(slug + "_reversed", inp2, "input arrays reversed: " + note, expect_label, twin=False)
    _counters["audit"] += 1
    base = "a%03d_%s" % (_counters["audit"], slug)
    with open(os.path.join(HERE, "audit", base + ".json"), "w") as f:
        json.dump(inp, f, indent=1, sort_keys=True)
    with open(os.path.join(HERE, "audit", base + ".note"), "w") as f:
        f.write(note.rstrip() + "\n")
    try:
        OLD_PRED[0] = None
        CONT_SEL[0] = None
        p = predict_audit(inp)
    except Exception:
        # the predictor reads a well-formed input; a malformed one is the
        # law's no-label outcome, which the case's stated expectation names
        p = {"label": expect_label}
    rec = {
        "path": "audit/" + base + ".json",
        "note_file": "audit/" + base + ".note",
        "category": "audit",
        "note": note,
        "predicted_label": p["label"],
        "predicted_counts": {k: v for k, v in p.items() if k != "label"},
    }
    if OLD_PRED[0] is not None and p["label"] != "NO_LABEL":
        rec["predicted_old"] = {k: OLD_PRED[0][k] for k in OLD_KEYS if k in OLD_PRED[0]}
        rec["predicted_continues"] = dict(CONT_SEL[0])
    if expect_label is not None:
        rec["hand_expect_label"] = expect_label
        if expect_label != p["label"]:
            rec["disagreement"] = True
            WARNINGS.append("audit/%s: hand=%s code=%s (%s)" % (base, expect_label, p["label"], note))
    MANIFEST.append(rec)
    return rec


def M(slug, args, note):
    """Register a harness-misuse case: `args` are the tokens after the executable,
    a token naming a corpus file written as its corpus-relative path; both
    programs exit 2 with no JSON."""
    _counters["misuse"] = _counters.get("misuse", 0) + 1
    MANIFEST.append({"path": "misuse/m%03d_%s" % (_counters["misuse"], slug), "category": "misuse",
                     "args": list(args), "note": note, "predicted": {"exit": 2}})


def A_raw(slug, raw: bytes, note, expect_label="NO_LABEL", predict_from=None):
    """Register an audit/ case whose bytes are written as given: inputs that
    are not JSON values at all (sec 9.4 harness forms: no label), and inputs
    whose transport spelling is the point (a label is then stated by hand)."""
    _counters["audit"] += 1
    base = "a%03d_%s" % (_counters["audit"], slug)
    _write(os.path.join(HERE, "audit", base + ".json"), raw)
    with open(os.path.join(HERE, "audit", base + ".note"), "w") as f:
        f.write(note.rstrip() + "\n")
    MANIFEST.append({
        "path": "audit/" + base + ".json",
        "note_file": "audit/" + base + ".note",
        "category": "audit",
        "note": note,
        "predicted_label": expect_label,
        "predicted_counts": ({k: v for k, v in predict_audit(predict_from).items() if k != "label"}
                             if predict_from is not None else {}),
        "hand_expect_label": expect_label,
    })


# ==========================================================================
# test keys
# ==========================================================================

def testkey(tag, seed):
    d = int.from_bytes(sha256(("gl1-key/%s/%d" % (tag, seed)).encode()), "big")
    return (d % (N - 2)) + 1


KEYS = {}


def K(tag):
    return KEYS[tag]


def A_(tag):
    return privkey_to_addr_hex(KEYS[tag])


# ==========================================================================
# small helpers for building deliberately faulty bytes
# ==========================================================================


def sub(data: bytes, old: bytes, new: bytes, n=1) -> bytes:
    assert data.count(old) >= 1, "marker %r absent" % (old,)
    return data.replace(old, new, n)


def deep_list(n):
    v = []
    for _ in range(n - 1):
        v = [v]
    return v


def nested_arr_text(n):
    return ("[" * n + "]" * n).encode()


def nested_obj_text(n):
    s = "{}"
    for _ in range(n - 1):
        s = '{"a":' + s + "}"
    return s.encode()


def rot_top(data: bytes, k=1) -> bytes:
    v = Parser(data.decode("utf-8")).text()
    v.members = v.members[k:] + v.members[:k]
    return cbytes(v, sort=False)


def rot_body(data: bytes, k=1) -> bytes:
    v = Parser(data.decode("utf-8")).text()
    b = v.get("body")
    b.members = b.members[k:] + b.members[:k]
    return cbytes(v, sort=False)


def drop_key(data: bytes, key: str) -> bytes:
    v = Parser(data.decode("utf-8")).text()
    v.members = [(k, x) for k, x in v.members if k != key]
    return cbytes(v, sort=True)


def add_key(data: bytes, key: str, val) -> bytes:
    v = Parser(data.decode("utf-8")).text()
    v.members.append((key, J(val)))
    return cbytes(v, sort=True)


def set_key(data: bytes, key: str, val) -> bytes:
    v = Parser(data.decode("utf-8")).text()
    v.members = [(k, (J(val) if k == key else x)) for k, x in v.members]
    return cbytes(v, sort=True)



# ==========================================================================
# canon/ : byte strings for `zk1 check` and `zk1 canon`
# ==========================================================================


def HX32(i):
    return "0x%064x" % i


def HX20(i):
    return "0x%040x" % i


class T(object):
    """Valid templates the fault cases are cut from."""
    pass


def build_templates():
    t = T()
    t.gen = make_entry(K("root"), "genesis", 0, None, {"statement_md": "the works of one author"})
    t.gid = eid(t.gen)
    t.hist = make_entry(K("root"), "history", 1, t.gid,
                        {"content": HX32(0xC0), "mode": {"mark": "hand", "toolchain": HX32(0x7001)}})
    t.hid = eid(t.hist)
    t.grant = make_entry(K("root"), "grant", 2, t.hid,
                         {"grantee": HX20(0xBEEF), "work": HX32(0xC0), "terms": HX32(0x7E),
                          "window": {"from": 100, "to": 200}})
    t.gr_id = eid(t.grant)
    t.rev = make_entry(K("root"), "revocation", 3, t.gr_id, {"grant": t.gr_id})
    t.rid = eid(t.rev)
    t.succ = make_entry(K("root"), "succession", 4, t.rid,
                        {"to": A_("k2"), "kind": "handover", "effective": 1700000000,
                         "statement_md": "the seat moves"})
    t.sid = eid(t.succ)
    t.ann = make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "ZZMARKZZ"})
    t.nps = make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "q": "ZZMARKZZ"})
    t.keyt = make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "ZZKEYZZ": "v"})
    t.mig = make_entry(K("root"), "migration", 5, t.sid,
                       {"spec": "zikaron/3", "statement_md": "the seat moves on"})
    t.retr = make_entry(K("root"), "retraction", 2, t.hid, {"subject": t.hid})
    t.cont = make_entry(K("root"), "genesis", 0, None,
                        {"statement_md": "continued", "continues": {"root": A_("root"),
                                                                     "entry_id": HX32(0x51)}})
    t.unk = make_entry(K("root"), "windmill", 1, t.gid, {})
    return t


# sec 3.5, closed by construction: every prefix and every single-byte
# substitution of a small set of templates that together reach every state of
# the grammar. Seed-free; the same cases under every seed.
_SYNTAX_TEMPLATES = [
    ("full", b'{"a":[1,-20,"b\\"\\\\\\/\\b\\f\\n\\r\\t\\u00e9",true,false,null],"c":{"d":{}},"e":[]}'),
    ("escapes", b'"x\\u0041\\ud83dy"'),
    ("nesting", b'[[[[1]]],{"k":"v"}]'),
    ("envelope", b'{"prev":null,"seq":0,"spec":"zikaron/2"}'),
    ("spaced", b' [ 1 , 2 ] '),
    ("utf8", b'"caf\xc3\xa9 \xe4\xb8\xad \xf0\x9f\x98\x80"'),
    ("int", b"-123"),
    ("literal", b"true"),
    ("dupkey", b'{"a":1,"a":2}'),
]
_SYNTAX_BYTES = [b"{", b"]", b'"', b"\\", b":", b",", b" ", b"\x00", b"x", b"0", b"-", b"u",
                 b"\xff"]


def build_syntax_closure():
    seen = set()
    for name, t in _SYNTAX_TEMPLATES:
        for i in range(1, len(t)):
            b = t[:i]
            if b in seen:
                continue
            seen.add(b)
            C("syn_%s_cut%02d" % (name, i), b,
              "sec 3.5 closure: template %s cut after %d bytes" % (name, i))
        for i in range(len(t)):
            for x in _SYNTAX_BYTES:
                b = t[:i] + x + t[i + 1:]
                if b in seen:
                    continue
                seen.add(b)
                C("syn_%s_p%02d_%02x" % (name, i, x[0]), b,
                  "sec 3.5 closure: template %s, byte %d replaced by 0x%02x" % (name, i, x[0]))


def build_canon(rng, t):
    build_syntax_closure()
    # ---------------- sec 1 / sec 3.5 test 1: UTF-8 -----------------------
    C("utf8_valid_bmp", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "café 中文"}),
      "sec 3.3/3.4: multibyte scalars raw in a prose subtree", expect="ok")
    C("utf8_valid_astral", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "\U0001f600"}),
      "sec 3.4 item 3: an astral scalar emitted raw", expect="ok")
    C("utf8_max_scalar", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "\U0010ffff"}),
      "sec 1: U+10FFFF, the top of the scalar range, in prose", expect="ok")
    C("utf8_overlong_2byte", sub(t.ann, b"ZZMARKZZ", b"\xc0\xaf"),
      "sec 1: overlong two-byte encoding of '/'", expect="E_UTF8")
    C("utf8_overlong_3byte", sub(t.ann, b"ZZMARKZZ", b"\xe0\x80\xaf"),
      "sec 1: overlong three-byte encoding of '/'", expect="E_UTF8")
    C("utf8_overlong_nul", sub(t.ann, b"ZZMARKZZ", b"\xc0\x80"),
      "sec 1: overlong encoding of U+0000", expect="E_UTF8")
    C("utf8_surrogate_encoded_d800", sub(t.ann, b"ZZMARKZZ", b"\xed\xa0\x80"),
      "sec 1: UTF-8 bytes encoding U+D800", expect="E_UTF8")
    C("utf8_surrogate_encoded_dfff", sub(t.ann, b"ZZMARKZZ", b"\xed\xbf\xbf"),
      "sec 1: UTF-8 bytes encoding U+DFFF", expect="E_UTF8")
    C("utf8_above_10ffff", sub(t.ann, b"ZZMARKZZ", b"\xf4\x90\x80\x80"),
      "sec 1: four-byte sequence for U+110000", expect="E_UTF8")
    C("utf8_five_byte", sub(t.ann, b"ZZMARKZZ", b"\xf8\x88\x80\x80\x80"),
      "sec 1: five-byte sequence, no scalar", expect="E_UTF8")
    C("utf8_truncated", sub(t.ann, b"ZZMARKZZ", b"\xe2\x82"),
      "sec 1: truncated three-byte sequence", expect="E_UTF8")
    C("utf8_lone_continuation", sub(t.ann, b"ZZMARKZZ", b"\x80"),
      "sec 1: a bare continuation byte", expect="E_UTF8")
    C("utf8_ff_byte", sub(t.ann, b"ZZMARKZZ", b"\xff"),
      "sec 1: 0xFF, never a UTF-8 byte", expect="E_UTF8")
    C("utf8_fe_byte", sub(t.ann, b"ZZMARKZZ", b"\xfe"),
      "sec 1: 0xFE, never a UTF-8 byte", expect="E_UTF8")
    C("utf8_bad_outside_string", t.gen[:-1] + b"\xc3",
      "sec 3.5 test 1 precedes every other test: a stray lead byte after the root", expect="E_UTF8")
    C("utf8_beats_depth", b"\xff" + nested_arr_text(200),
      "sec 3.5: test 1 is whole-string and precedes test 2's E_DEPTH", expect="E_UTF8")

    # ---------------- BOM --------------------------------------------------
    C("bom_leading", b"\xef\xbb\xbf" + t.gen,
      "sec 3.5: a byte order mark is not whitespace under JSON-text", expect="E_JSON")
    C("bom_trailing", t.gen + b"\xef\xbb\xbf",
      "sec 3.5 test 2: U+FEFF after the root value is not whitespace", expect="E_JSON")
    C("bom_scalar_in_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a﻿b"}),
      "sec 3.3: U+FEFF is an ordinary scalar inside a prose subtree", expect="ok")
    C("bom_scalar_outside_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "q": "a﻿b"}),
      "sec 3.3: U+FEFF encodes to bytes outside 0x20-0x7E in a non-prose value", expect="E_VALUE_CHARSET")

    # ---------------- whitespace ------------------------------------------
    C("ws_leading_space", b" " + t.gen, "sec 3.5: leading insignificant whitespace", expect="E_NOT_CANONICAL")
    C("ws_trailing_newline", t.gen + b"\n", "sec 2.2/3.5: trailing newline", expect="E_NOT_CANONICAL")
    C("ws_trailing_space", t.gen + b" ", "sec 3.5: trailing space", expect="E_NOT_CANONICAL")
    C("ws_leading_tab", b"\t" + t.gen, "sec 3.5: leading tab is JSON whitespace", expect="E_NOT_CANONICAL")
    C("ws_crlf_both_ends", b"\r\n" + t.gen + b"\r\n", "sec 3.5: CRLF around the root", expect="E_NOT_CANONICAL")
    C("ws_after_colon", sub(t.gen, b'"spec":"zikaron/2"', b'"spec": "zikaron/2"'),
      "sec 3.4 item 6: no byte lies between the pieces", expect="E_NOT_CANONICAL")
    C("ws_after_comma", sub(t.gen, b'","', b'", "'),
      "sec 3.4 item 6: a space between members", expect="E_NOT_CANONICAL")
    C("ws_inside_string_kept", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": " a  b "}),
      "sec 3.4 item 6: whitespace inside a string is part of the string", expect="ok")
    C("ws_only", b"   ", "sec 3.5 test 2: whitespace is not a JSON text", expect="E_JSON")
    C("empty_input", b"", "sec 3.5 test 2: the empty byte string is a proper prefix", expect="E_JSON")

    # ---------------- escapes ---------------------------------------------
    C("esc_quote", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": 'a"b'}),
      "sec 3.4 item 3: quote emitted as backslash-quote", expect="ok")
    C("esc_backslash", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\\b"}),
      "sec 3.4 item 3: backslash emitted doubled", expect="ok")
    C("esc_b_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\bb"}),
      "sec 3.4 item 3: U+0008 as backslash-b in a prose subtree", expect="ok")
    C("esc_t_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\tb"}),
      "sec 3.4 item 3: U+0009 as backslash-t", expect="ok")
    C("esc_n_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\nb"}),
      "sec 3.4 item 3: U+000A as backslash-n", expect="ok")
    C("esc_f_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\fb"}),
      "sec 3.4 item 3: U+000C as backslash-f", expect="ok")
    C("esc_r_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\rb"}),
      "sec 3.4 item 3: U+000D as backslash-r", expect="ok")
    C("esc_u0000_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\x00b"}),
      "sec 3.4 item 3: U+0000 as backslash-u-0-0-0-0", expect="ok")
    C("esc_u0001_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "\x01"}),
      "sec 3.4 item 3: U+0001 as a lowercase four-digit escape", expect="ok")
    C("esc_u000b_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "\x0b"}),
      "sec 3.4 item 3: U+000B has no short form, escapes as backslash-u-0-0-0-b", expect="ok")
    C("esc_u001f_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "\x1f"}),
      "sec 3.4 item 3: U+001F, the top of the escaped range", expect="ok")
    C("esc_u001f_upper", sub(make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "\x1f"}),
                             b"\\u001f", b"\\u001F"),
      "sec 3.4 item 3: uppercase hex digits in a control escape are not canonical",
      expect="E_NOT_CANONICAL")
    C("esc_u0008_long_form", sub(t.ann, b"ZZMARKZZ", b"\\u0008"),
      "sec 3.4 item 3: U+0008 has a short form, so the long escape is not canonical",
      expect="E_NOT_CANONICAL")
    C("esc_u0009_long_form", sub(t.ann, b"ZZMARKZZ", b"\\u0009"),
      "sec 3.4 item 3: U+0009 must be backslash-t", expect="E_NOT_CANONICAL")
    C("esc_u0020_above_1f", sub(t.ann, b"ZZMARKZZ", b"\\u0020"),
      "sec 3.4 item 3: U+0020 and above are emitted raw", expect="E_NOT_CANONICAL")
    C("esc_u0041_letter", sub(t.ann, b"ZZMARKZZ", b"\\u0041"),
      "sec 3.4 item 3: an escaped ASCII letter is not canonical", expect="E_NOT_CANONICAL")
    C("esc_u00e9_lower_prose", sub(t.ann, b"ZZMARKZZ", b"\\u00e9"),
      "sec 3.4 item 3: U+00E9 must be raw UTF-8", expect="E_NOT_CANONICAL")
    C("esc_u00E9_upper_prose", sub(t.ann, b"ZZMARKZZ", b"\\u00E9"),
      "sec 3.4 item 3: uppercase digits above U+001F, still not canonical", expect="E_NOT_CANONICAL")
    C("esc_u00e9_nonprose", sub(t.nps, b"ZZMARKZZ", b"\\u00e9"),
      "sec 3.5: test 5 names the fault before test 6 can", expect="E_VALUE_CHARSET")
    C("esc_u0008_nonprose", sub(t.nps, b"ZZMARKZZ", b"\\u0008"),
      "sec 3.3: U+0008 outside a prose subtree", expect="E_VALUE_CHARSET")
    C("esc_b_nonprose", sub(t.nps, b"ZZMARKZZ", b"\\b"),
      "sec 3.3: a short escape decodes to a byte the value charset refuses", expect="E_VALUE_CHARSET")
    C("esc_t_nonprose", sub(t.nps, b"ZZMARKZZ", b"\\t"),
      "sec 3.3: tab outside a prose subtree", expect="E_VALUE_CHARSET")
    C("esc_u0080_prose", sub(t.ann, b"ZZMARKZZ", b"\\u0080"),
      "sec 3.4: the C1 range is emitted raw, so the escape is not canonical", expect="E_NOT_CANONICAL")
    C("esc_solidus", sub(t.ann, b"ZZMARKZZ", b"a\\/b"),
      "sec 3.4: an escaped solidus decodes to '/', which is emitted raw", expect="E_NOT_CANONICAL")
    C("esc_unknown_x", sub(t.ann, b"ZZMARKZZ", b"a\\xb"),
      "RFC 8259: backslash-x is not an escape", expect="E_JSON")
    C("esc_u_short", sub(t.ann, b"ZZMARKZZ", b"a\\u12b"),
      "RFC 8259: a backslash-u escape needs four hex digits", expect="E_JSON")
    C("esc_u_nonhex", sub(t.ann, b"ZZMARKZZ", b"a\\u12g4b"),
      "RFC 8259: non-hex digit in an escape", expect="E_JSON")
    C("esc_trailing_backslash", sub(t.ann, b'"ZZMARKZZ"', b'"a\\'),
      "RFC 8259: a backslash at the end of the input", expect="E_JSON")

    # surrogate escapes
    C("esc_surrogate_pair", sub(t.ann, b"ZZMARKZZ", b"\\ud83d\\ude00"),
      "sec 3.3: a surrogate escape fails whether or not a second follows it", expect="E_JSON")
    C("esc_lone_high_surrogate", sub(t.ann, b"ZZMARKZZ", b"\\ud800"),
      "sec 3.3: a lone high surrogate escape", expect="E_JSON")
    C("esc_lone_low_surrogate", sub(t.ann, b"ZZMARKZZ", b"\\udc00"),
      "sec 3.3: a lone low surrogate escape", expect="E_JSON")
    C("esc_surrogate_dfff", sub(t.ann, b"ZZMARKZZ", b"\\uDFFF"),
      "sec 3.3: the top of the surrogate block, uppercase digits", expect="E_JSON")
    C("esc_just_below_surrogates", sub(t.ann, b"ZZMARKZZ", b"\\ud7ff"),
      "sec 3.3: U+D7FF is not a surrogate, so test 6 names the fault", expect="E_NOT_CANONICAL")
    C("esc_just_above_surrogates", sub(t.ann, b"ZZMARKZZ", b"\\ue000"),
      "sec 3.3: U+E000 is not a surrogate", expect="E_NOT_CANONICAL")
    C("esc_surrogate_in_key", sub(t.keyt, b"ZZKEYZZ", b"\\ud800x"),
      "sec 3.3: a surrogate escape in a key is still E_JSON at test 2", expect="E_JSON")
    C("esc_surrogate_beats_dup_key",
      sub(sub(t.keyt, b"ZZKEYZZ", b"\\ud800"), b'"note_md":"n"', b'"note_md":"n","note_md":"n"'),
      "sec 3.5: test 2 precedes test 3", expect="E_JSON")

    # ---------------- raw controls and the two charsets --------------------
    C("raw_lf_in_string", sub(t.ann, b"ZZMARKZZ", b"a\nb"),
      "RFC 8259: an unescaped U+000A inside a string", expect="E_JSON")
    C("raw_1f_in_string", sub(t.ann, b"ZZMARKZZ", b"a\x1fb"),
      "RFC 8259: an unescaped U+001F inside a string", expect="E_JSON")
    C("raw_20_in_string", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a b"}),
      "RFC 8259: U+0020 is the first byte a string may carry raw", expect="ok")
    C("raw_7f_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\x7fb"}),
      "sec 3.3/3.4: U+007F raw in a prose subtree", expect="ok")
    C("raw_7f_nonprose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "q": "a\x7fb"}),
      "sec 3.3: U+007F is outside 0x20-0x7E", expect="E_VALUE_CHARSET")
    C("raw_c1_80_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "a\x80b"}),
      "sec 3.4: the C1 range raw in prose", expect="ok")
    C("raw_c1_9f_prose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "\x9f"}),
      "sec 3.4: U+009F, the top of C1, raw in prose", expect="ok")
    C("raw_c1_nonprose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "q": "\x80"}),
      "sec 3.3: a C1 scalar encodes to bytes outside the skeleton range", expect="E_VALUE_CHARSET")
    C("raw_7e_nonprose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "q": "~"}),
      "sec 3.3: 0x7E is the last byte the skeleton range admits", expect="ok")
    C("raw_20_nonprose", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "q": " "}),
      "sec 3.3: 0x20 is the first byte the skeleton range admits", expect="ok")
    C("empty_string_value", make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "q": ""}),
      "sec 3.3: an empty string value is a skeleton string", expect="ok")

    # ---------------- sec 3.1 / 3.2 : the integer universe ------------------
    SEQ1 = b'"seq":1,'
    C("int_zero", t.gen, "sec 3.2: the integer zero at seq 0", expect="ok")
    C("int_one", t.ann, "sec 3.2: the integer one", expect="ok")
    maxent = make_entry(K("root"), "annotation", MAXINT, t.gid, {"note_md": "top of the universe"})
    C("int_2p53_minus_1", maxent, "sec 3.2: 2^53 - 1, the top of the universe", expect="ok")
    C("int_2p53", sub(t.ann, SEQ1, b'"seq":9007199254740992,'),
      "sec 3.2: 2^53 is outside the universe", expect="E_NUMBER")
    C("int_2p53_plus_1", sub(t.ann, SEQ1, b'"seq":9007199254740993,'),
      "sec 3.2: one past 2^53", expect="E_NUMBER")
    C("int_leading_zero", sub(t.ann, SEQ1, b'"seq":01,'),
      "sec 3.2: no leading zeros", expect="E_NUMBER")
    C("int_double_zero", sub(t.ann, SEQ1, b'"seq":00,'),
      "sec 3.2: zero is written 0", expect="E_NUMBER")
    C("int_negative", sub(t.ann, SEQ1, b'"seq":-1,'),
      "sec 3.1: no value carries a minus sign", expect="E_NUMBER")
    C("int_negative_zero", sub(t.ann, SEQ1, b'"seq":-0,'),
      "sec 3.1: minus zero is not an integer here", expect="E_NUMBER")
    C("int_fraction_zero", sub(t.ann, SEQ1, b'"seq":1.0,'),
      "sec 3.1: no value carries a fraction", expect="E_NUMBER")
    C("int_fraction_half", sub(t.ann, SEQ1, b'"seq":1.5,'),
      "sec 3.1: floating point is unrepresentable", expect="E_NUMBER")
    C("int_exponent_lower", sub(t.ann, SEQ1, b'"seq":1e2,'),
      "sec 3.1: no value carries an exponent", expect="E_NUMBER")
    C("int_exponent_upper", sub(t.ann, SEQ1, b'"seq":1E2,'),
      "sec 3.1: uppercase exponent", expect="E_NUMBER")
    C("int_exponent_signed", sub(t.ann, SEQ1, b'"seq":1e+2,'),
      "sec 3.1: signed exponent", expect="E_NUMBER")
    C("int_plus_one", sub(t.ann, SEQ1, b'"seq":+1,'),
      "sec 3.1: '+' opens a value position of numeric shape", expect="E_NUMBER")
    C("int_dot_five", sub(t.ann, SEQ1, b'"seq":.5,'),
      "sec 3.1: '.' opens a value position of numeric shape", expect="E_NUMBER")
    C("int_one_dot", sub(t.ann, SEQ1, b'"seq":1.,'),
      "sec 3.1: a trailing decimal point", expect="E_NUMBER")
    C("int_minus_infinity", sub(t.ann, SEQ1, b'"seq":-Infinity,'),
      "sec 3.5: -Infinity opens a numeric-shape position whose bytes are just '-'",
      expect="E_NUMBER")
    C("int_nan", sub(t.ann, SEQ1, b'"seq":NaN,'),
      "sec 3.5: NaN is an unknown literal, not a numeric shape", expect="E_JSON")
    C("int_infinity", sub(t.ann, SEQ1, b'"seq":Infinity,'),
      "sec 3.5: Infinity is an unknown literal", expect="E_JSON")
    C("int_undefined", sub(t.ann, SEQ1, b'"seq":undefined,'),
      "sec 3.5: undefined is an unknown literal", expect="E_JSON")
    C("int_huge_digits", sub(t.ann, SEQ1, b'"seq":99999999999999999999,'),
      "sec 3.2: twenty digits, far outside the universe", expect="E_NUMBER")
    C("int_hex_literal", sub(t.ann, SEQ1, b'"seq":0x10,'),
      "sec 3.1: the bytes there are just '0', an integer, so the grammar fails at 'x'",
      expect="E_JSON")
    C("int_two_numbers", sub(t.ann, SEQ1, b'"seq":1 2,'),
      "sec 3.1: the bytes there are '1'; the grammar fails at '2'", expect="E_JSON")
    C("int_trailing_e", sub(t.ann, SEQ1, b'"seq":12e,'),
      "sec 3.1: the bytes there are '12e', not an integer", expect="E_NUMBER")
    C("int_lone_minus", sub(t.ann, SEQ1, b'"seq":-,'),
      "sec 3.1: a bare minus sign", expect="E_NUMBER")
    C("int_window_max",
      make_entry(K("root"), "grant", 2, t.hid,
                 {"grantee": HX20(1), "work": HX32(1), "terms": HX32(2),
                  "window": {"from": 0, "to": MAXINT}}),
      "sec 3.2 inside a body: 0 and 2^53 - 1", expect="ok")
    C("int_window_over",
      sub(make_entry(K("root"), "grant", 2, t.hid,
                     {"grantee": HX20(1), "work": HX32(1), "terms": HX32(2),
                      "window": {"from": 0, "to": MAXINT}}),
          b'"to":9007199254740991', b'"to":9007199254740992'),
      "sec 3.2 inside a body: 2^53 fails at test 2, never at the body table", expect="E_NUMBER")
    C("int_in_array_body",
      make_entry(K("root"), "windmill", 1, t.gid, {"xs": [0, 1, MAXINT]}),
      "sec 3.1: integers inside an array in a body", expect="ok")

    # ---------------- literals in the envelope and in bodies ---------------
    C("lit_seq_true", set_key(t.ann, "seq", True), "sec 4.3 step 7", expect="E_SEQ")
    C("lit_seq_false", set_key(t.ann, "seq", False), "sec 4.3 step 7", expect="E_SEQ")
    C("lit_seq_null", set_key(t.ann, "seq", None), "sec 4.3 step 7", expect="E_SEQ")
    C("lit_seq_string", set_key(t.ann, "seq", "1"), "sec 4.3 step 7", expect="E_SEQ")
    C("lit_seq_array", set_key(t.ann, "seq", [1]), "sec 4.3 step 7", expect="E_SEQ")
    C("lit_prev_true", set_key(t.ann, "prev", True), "sec 4.3 step 8", expect="E_PREV")
    C("lit_prev_false", set_key(t.ann, "prev", False), "sec 4.3 step 8", expect="E_PREV")
    C("lit_prev_int", set_key(t.ann, "prev", 0), "sec 4.3 step 8", expect="E_PREV")
    C("lit_body_null", set_key(t.ann, "body", None), "sec 4.3 step 9", expect="E_BODY")
    C("lit_body_true", set_key(t.ann, "body", True), "sec 4.3 step 9", expect="E_BODY")
    C("lit_body_array", set_key(t.ann, "body", []), "sec 4.3 step 9", expect="E_BODY")
    C("lit_body_string", set_key(t.ann, "body", "x"), "sec 4.3 step 9", expect="E_BODY")
    C("lit_spec_null", set_key(t.ann, "spec", None), "sec 4.3 step 4", expect="E_SPEC")
    C("lit_spec_true", set_key(t.ann, "spec", True), "sec 4.3 step 4", expect="E_SPEC")
    C("lit_spec_int", set_key(t.ann, "spec", 1), "sec 4.3 step 4", expect="E_SPEC")
    C("lit_entrytype_null", set_key(t.ann, "entryType", None), "sec 4.3 step 5", expect="E_ENTRYTYPE")
    C("lit_entrytype_true", set_key(t.ann, "entryType", True), "sec 4.3 step 5", expect="E_ENTRYTYPE")
    C("lit_entrytype_int", set_key(t.ann, "entryType", 7), "sec 4.3 step 5", expect="E_ENTRYTYPE")
    C("lit_author_null", set_key(t.ann, "author", None), "sec 4.3 step 6", expect="E_AUTHOR")
    C("lit_sig_null", set_key(t.ann, "sig", None), "sec 4.3 step 10", expect="E_SIG_FORM")
    C("lit_sig_int", set_key(t.ann, "sig", 0), "sec 4.3 step 10", expect="E_SIG_FORM")
    C("lit_body_values",
      make_entry(K("root"), "windmill", 1, t.gid,
                 {"a": None, "b": True, "c": False, "d": [None, True, False], "e": {"f": None}}),
      "sec 3.1/6.10: null, true and false are values of the universe, in a body", expect="ok")
    C("lit_null_optional_absent",
      make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n"}),
      "sec 6: the only spelling of an absent optional field is the absent key", expect="ok")

    # ---------------- keys (sec 3.3) ---------------------------------------
    C("key_empty", make_entry(K("root"), "annotation", 1, t.gid, {"": "v", "note_md": "n"}),
      "sec 3.3: a key with no byte", expect="E_KEY_CHARSET")
    C("key_space", make_entry(K("root"), "annotation", 1, t.gid, {"a b": "v", "note_md": "n"}),
      "sec 3.3: 0x20 is inside the key charset", expect="ok")
    C("key_single_space", make_entry(K("root"), "annotation", 1, t.gid, {" ": "v", "note_md": "n"}),
      "sec 3.3: a key that is one space", expect="ok")
    C("key_bang", make_entry(K("root"), "annotation", 1, t.gid, {"!": "v", "note_md": "n"}),
      "sec 3.3: 0x21", expect="ok")
    C("key_tilde", make_entry(K("root"), "annotation", 1, t.gid, {"~": "v", "note_md": "n"}),
      "sec 3.3: 0x7E, the last byte of the key charset", expect="ok")
    C("key_del", make_entry(K("root"), "annotation", 1, t.gid, {"a\x7f": "v", "note_md": "n"}),
      "sec 3.3: 0x7F is one past the key charset", expect="E_KEY_CHARSET")
    C("key_c1", make_entry(K("root"), "annotation", 1, t.gid, {"a\x80": "v", "note_md": "n"}),
      "sec 3.3: a C1 scalar in a key", expect="E_KEY_CHARSET")
    C("key_multibyte", make_entry(K("root"), "annotation", 1, t.gid, {"café": "v", "note_md": "n"}),
      "sec 3.3: a non-ASCII scalar in a key", expect="E_KEY_CHARSET")
    C("key_control_escape", sub(t.keyt, b"ZZKEYZZ", b"a\\u001fb"),
      "sec 3.3: the key charset is decided after escape decoding", expect="E_KEY_CHARSET")
    C("key_nul_escape", sub(t.keyt, b"ZZKEYZZ", b"\\u0000"),
      "sec 3.3: U+0000 in a key", expect="E_KEY_CHARSET")
    C("key_empty_in_prose_subtree",
      make_entry(K("root"), "annotation", 1, t.gid, {"note_md": {"": "\x00"}}),
      "sec 3.3: keys inside a prose subtree remain keys under the key rule",
      expect="E_KEY_CHARSET")
    C("key_control_in_prose_subtree",
      make_entry(K("root"), "annotation", 1, t.gid, {"note_md": {"a\x01": "\x00"}}),
      "sec 3.3: a control scalar in a key inside a prose subtree", expect="E_KEY_CHARSET")
    C("key_dup_beats_charset", sub(t.keyt, b'"ZZKEYZZ":"v"', b'"":"v","":"w"'),
      "sec 3.5: test 3 precedes test 4", expect="E_DUP_KEY")
    C("key_case_distinct", make_entry(K("root"), "annotation", 1, t.gid,
                                      {"a": 1, "A": 2, "note_md": "n"}),
      "sec 3.5: keys differing in case are distinct; 0x41 sorts before 0x61", expect="ok")

    # ---------------- prose subtrees (sec 3.3) ------------------------------
    C("prose_key_exactly_md", make_entry(K("root"), "windmill", 1, t.gid, {"_md": "\x00\x01\x7f"}),
      "sec 3.3: a member whose decoded key is the three bytes _md", expect="ok")
    C("prose_key_ends_md", make_entry(K("root"), "windmill", 1, t.gid, {"x_md": "\x00"}),
      "sec 3.3: a key ending in _md", expect="ok")
    C("prose_key_double_underscore", make_entry(K("root"), "windmill", 1, t.gid, {"__md": "\x00"}),
      "sec 3.3: __md ends in _md", expect="ok")
    C("prose_key_uppercase_MD", make_entry(K("root"), "windmill", 1, t.gid, {"x_MD": "\x00"}),
      "sec 3.3: the suffix test is over bytes, so _MD opens nothing", expect="E_VALUE_CHARSET")
    C("prose_key_md_no_underscore", make_entry(K("root"), "windmill", 1, t.gid, {"md": "\x00"}),
      "sec 3.3: md is not _md", expect="E_VALUE_CHARSET")
    C("prose_key_md_prefix", make_entry(K("root"), "windmill", 1, t.gid, {"_mdx": "\x00"}),
      "sec 3.3: the key must end in _md, not merely carry it", expect="E_VALUE_CHARSET")
    C("prose_nested_object", make_entry(K("root"), "windmill", 1, t.gid,
                                        {"a_md": {"b": {"c": "\x00\x1f\x7f‮"}}}),
      "sec 3.3: every string value at any depth within the member", expect="ok")
    C("prose_nested_array", make_entry(K("root"), "windmill", 1, t.gid,
                                       {"a_md": [["\x00"], {"k": ["\x1b"]}]}),
      "sec 3.3: through arrays and nested objects", expect="ok")
    C("prose_sibling_not_prose", make_entry(K("root"), "windmill", 1, t.gid,
                                            {"a_md": "\x00", "b": "\x00"}),
      "sec 3.3: the sibling outside the subtree keeps the value charset",
      expect="E_VALUE_CHARSET")
    C("prose_bidi_controls", make_entry(K("root"), "annotation", 1, t.gid,
                                        {"note_md": "a‮b‏c⁦d"}),
      "sec 11: bidirectional controls are ordinary scalars in prose", expect="ok")
    C("prose_root_md_extra_member",
      make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n"},
                 extra_env=[("x_md", "\x00")]),
      "sec 3.3/4.1: an eighth key ending in _md opens prose at the root, and test 5 "
      "passes before sec 4.3 step 3 closes the envelope", expect="E_ENVELOPE_CLOSED")

    # ---------------- depth (sec 3.5 test 2) --------------------------------
    C("depth_arr_128", nested_arr_text(128),
      "sec 3.5: 128 nested arrays, the root counted as depth 1", expect="E_ENVELOPE")
    C("depth_arr_129", nested_arr_text(129),
      "sec 3.5: a container opening at depth 129", expect="E_DEPTH")
    C("depth_obj_128", nested_obj_text(128),
      "sec 3.5: 128 nested objects; the root is an object, so sec 4.3 step 3 names the fault",
      expect="E_ENVELOPE_MISSING")
    C("depth_obj_129", nested_obj_text(129),
      "sec 3.5: an object opening at depth 129", expect="E_DEPTH")
    C("depth_entry_128",
      make_entry(K("root"), "windmill", 1, t.gid, {"d": deep_list(126)}),
      "sec 3.5: root 1, body 2, 126 arrays inside, deepest container at depth 128", expect="ok")
    C("depth_entry_129",
      make_entry(K("root"), "windmill", 1, t.gid, {"d": deep_list(127)}),
      "sec 3.5: the same body one container deeper", expect="E_DEPTH")
    C("depth_mixed_129",
      ("".join('{"a":[' for _ in range(64)) + "[]" + "]}" * 64).encode(),
      "sec 3.5: alternating objects and arrays, 129 containers", expect="E_DEPTH")
    C("depth_before_number", b"[" + nested_arr_text(128) + b",1.5]",
      "sec 3.5: the depth-129 '[' comes before the numeric-shape byte", expect="E_DEPTH")
    C("number_before_depth", b"[1.5," + nested_arr_text(128) + b"]",
      "sec 3.5: the numeric-shape byte comes first", expect="E_NUMBER")
    C("json_before_depth", b"[@," + nested_arr_text(129) + b"]",
      "sec 3.5: a grammar failure at byte 1 beats a later depth fault", expect="E_JSON")
    C("number_before_eof", b'{"a":1.5',
      "sec 3.5: E_NUMBER's byte precedes the one-past-the-end byte of a truncated text",
      expect="E_NUMBER")

    # ---------------- scalar roots ------------------------------------------
    C("root_string", b'"hello"', "sec 3.5: a scalar root has depth 0; sec 4.3 step 2",
      expect="E_ENVELOPE")
    C("root_int", b"0", "sec 3.5/4.3: an integer root", expect="E_ENVELOPE")
    C("root_null", b"null", "sec 4.3 step 2", expect="E_ENVELOPE")
    C("root_true", b"true", "sec 4.3 step 2", expect="E_ENVELOPE")
    C("root_false", b"false", "sec 4.3 step 2", expect="E_ENVELOPE")
    C("root_array_empty", b"[]", "sec 4.3 step 2", expect="E_ENVELOPE")
    C("root_object_empty", b"{}", "sec 4.3 step 3", expect="E_ENVELOPE_MISSING")
    C("root_negative", b"-1", "sec 3.1: the root is a value position of numeric shape",
      expect="E_NUMBER")
    C("root_fraction", b"1.5", "sec 3.1: a fractional root", expect="E_NUMBER")
    C("root_leading_zero", b"01", "sec 3.2: leading zero at the root", expect="E_NUMBER")
    C("root_string_control", b'"\\u0001"',
      "sec 3.3: a scalar root string is outside every prose subtree", expect="E_VALUE_CHARSET")
    C("root_string_noncanonical", b'"\\u0041"',
      "sec 3.4: an escaped letter at the root", expect="E_NOT_CANONICAL")
    C("root_trailing_garbage", b"null null", "sec 3.5 test 2: one JSON text only", expect="E_JSON")

    # ---------------- duplicate keys ----------------------------------------
    C("dup_envelope_key", sub(t.ann, b'"spec":"zikaron/2"', b'"spec":"zikaron/2","spec":"zikaron/2"'),
      "sec 3.5 test 3: a repeated envelope key", expect="E_DUP_KEY")
    C("dup_body_key", sub(t.ann, b'"note_md":"ZZMARKZZ"', b'"note_md":"a","note_md":"b"'),
      "sec 3.5 test 3: a repeated body key", expect="E_DUP_KEY")
    C("dup_after_escape_decoding", sub(t.ann, b'"note_md":"ZZMARKZZ"',
                                       b'"note_md":"a","note\\u005fmd":"b"'),
      "sec 3.5 test 3: distinct after escape decoding is the test", expect="E_DUP_KEY")
    C("dup_in_prose_subtree", sub(t.ann, b'"note_md":"ZZMARKZZ"',
                                  b'"note_md":{"a":"x","a":"y"}'),
      "sec 3.5 test 3: every object at every depth", expect="E_DUP_KEY")
    C("dup_in_array_element", sub(t.ann, b'"note_md":"ZZMARKZZ"',
                                  b'"note_md":[{"a":1,"a":2}]'),
      "sec 3.5 test 3: an object inside an array", expect="E_DUP_KEY")
    C("dup_beats_not_canonical", sub(t.ann, b'"note_md":"ZZMARKZZ"',
                                     b'"note_md":"b","note_md":"a"'),
      "sec 3.5: test 3 precedes test 6", expect="E_DUP_KEY")

    # ---------------- member order (sec 3.4 item 5) --------------------------
    C("order_envelope_rotated", rot_top(t.gen, 1),
      "sec 3.4 item 5: members ordered by the bytewise order of their keys",
      expect="E_NOT_CANONICAL")
    C("order_envelope_reversed", cbytes(
        JObj(list(reversed(Parser(t.gen.decode()).text().members))), sort=False),
      "sec 3.4 item 5: the envelope written in reverse key order", expect="E_NOT_CANONICAL")
    C("order_body_rotated", rot_body(t.grant, 1),
      "sec 3.4 item 5: body members out of order", expect="E_NOT_CANONICAL")
    C("order_space_key_correct",
      make_entry(K("root"), "windmill", 1, t.gid, {"a b": 1, "ab": 2}),
      "sec 3.4 item 5: 0x20 sorts before 0x62, so 'a b' precedes 'ab'", expect="ok")
    C("order_space_key_swapped",
      cbytes(JObj([(k, (JObj(list(reversed(v.members))) if k == "body" else v))
                   for k, v in Parser(make_entry(K("root"), "windmill", 1, t.gid,
                                                 {"a b": 1, "ab": 2}).decode()).text().members]),
             sort=False),
      "sec 3.4 item 5: the same two keys written the other way round", expect="E_NOT_CANONICAL")
    C("order_prefix_shorter_first",
      make_entry(K("root"), "windmill", 1, t.gid, {"a": 1, "ab": 2, "abc": 3}),
      "sec 1: where one key is a proper prefix of another the shorter orders first", expect="ok")
    C("order_case_correct",
      make_entry(K("root"), "windmill", 1, t.gid, {"A": 1, "Z": 2, "a": 3, "z": 4}),
      "sec 3.4 item 5: bytewise order over ASCII keys", expect="ok")

    # ---------------- sec 4.1 envelope arity --------------------------------
    for k in SEVEN:
        C("env_missing_" + k, drop_key(t.ann, k),
          "sec 4.3 step 3: the key %r is absent" % k, expect="E_ENVELOPE_MISSING")
    C("env_extra_key", add_key(t.ann, "extra", 1),
      "sec 4.3 step 3: an eighth key", expect="E_ENVELOPE_CLOSED")
    C("env_extra_two_keys", add_key(add_key(t.ann, "x", 1), "y", 2),
      "sec 4.3 step 3: two keys beyond the seven", expect="E_ENVELOPE_CLOSED")
    C("env_extra_key_sorts_first", add_key(t.ann, "aaa", 1),
      "sec 4.3 step 3: an extra key that sorts before every one of the seven",
      expect="E_ENVELOPE_CLOSED")
    C("env_seven_members_missing_and_extra", add_key(drop_key(t.ann, "sig"), "sigx", "0x00"),
      "sec 4.1: seven members that both lack one of the seven and carry an eighth key; "
      "the missing test is decided first", expect="E_ENVELOPE_MISSING")
    C("env_six_of_seven_plus_two_extra",
      add_key(add_key(drop_key(t.ann, "prev"), "p", 1), "q", 2),
      "sec 4.1: member count alone decides neither test", expect="E_ENVELOPE_MISSING")
    C("env_empty_object", b"{}", "sec 4.3 step 3 over an empty object",
      expect="E_ENVELOPE_MISSING")

    # ---------------- sec 4.1 field forms -----------------------------------
    C("spec_next_version", set_key(t.ann, "spec", "zikaron/3"), "sec 4.3 step 4: the next version's spec id", expect="E_SPEC")
    C("spec_previous_version", set_key(t.ann, "spec", "zikaron/1"),
      "sec 4.3 step 4 with HARNESS2: a zikaron/1 entry is rejected here at step 4; the reading "
      "that dispatches by spec is zikaron.kit/2's accept and never this command's", expect="E_SPEC")
    C("spec_trailing_space", set_key(t.ann, "spec", "zikaron/2 "), "sec 4.3 step 4: byte-equal",
      expect="E_SPEC")
    C("spec_uppercase", set_key(t.ann, "spec", "GALEED/1"), "sec 4.3 step 4", expect="E_SPEC")
    C("spec_empty", set_key(t.ann, "spec", ""), "sec 4.3 step 4", expect="E_SPEC")
    C("spec_galeed", set_key(t.ann, "spec", "galeed/1"), "sec 12.4: the twin law's spec id",
      expect="E_SPEC")
    C("entrytype_empty", set_key(t.ann, "entryType", ""), "sec 1: a token is non-empty",
      expect="E_ENTRYTYPE")
    C("entrytype_space", set_key(t.ann, "entryType", "a b"), "sec 1: 0x20 is outside a token",
      expect="E_ENTRYTYPE")
    C("entrytype_leading_space", set_key(t.ann, "entryType", " a"), "sec 1: token charset",
      expect="E_ENTRYTYPE")
    C("entrytype_bang", make_entry(K("root"), "!", 1, t.gid, {"note_md": "n"}),
      "sec 1/6.9: 0x21 is the first token byte, and an unrecognized type is an entry all the same",
      expect="ok")
    C("entrytype_tilde", make_entry(K("root"), "~", 1, t.gid, {"note_md": "n"}),
      "sec 1/6.9: 0x7E is the last token byte", expect="ok")
    C("entrytype_resigned_unknown", set_key(t.ann, "entryType", "!"),
      "sec 4.3: step 5 admits the token, and the stale signature then fails at step 13",
      expect="E_SIG_SIGNER")
    C("entrytype_del", set_key(t.ann, "entryType", "a\x7f"),
      "sec 3.3: 0x7F in a non-prose value is refused by test 5 before the token test is reached",
      expect="E_VALUE_CHARSET")
    C("author_uppercase", set_key(t.ann, "author", A_("root").upper().replace("0X", "0x")),
      "sec 1: hex20 digits are lowercase", expect="E_AUTHOR")
    C("author_no_prefix", set_key(t.ann, "author", A_("root")[2:]), "sec 1: the 0x prefix",
      expect="E_AUTHOR")
    C("author_short", set_key(t.ann, "author", A_("root")[:-1]), "sec 1: 39 digits",
      expect="E_AUTHOR")
    C("author_long", set_key(t.ann, "author", A_("root") + "0"), "sec 1: 41 digits",
      expect="E_AUTHOR")
    C("author_capital_X", set_key(t.ann, "author", "0X" + A_("root")[2:]), "sec 1: the prefix bytes",
      expect="E_AUTHOR")
    C("author_nonhex", set_key(t.ann, "author", "0x" + "g" * 40), "sec 1: hex digits",
      expect="E_AUTHOR")
    C("author_hex32_length", set_key(t.ann, "author", HX32(1)), "sec 1: a hex32 in a hex20 field",
      expect="E_AUTHOR")
    C("author_zero_address", set_key(t.ann, "author", HX20(0)),
      "sec 6.10: the all-zero address is a legal spelling, and the signature decides",
      expect="E_SIG_SIGNER")
    C("prev_short", set_key(t.ann, "prev", HX32(1)[:-1]), "sec 1: 63 digits", expect="E_PREV")
    C("prev_long", set_key(t.ann, "prev", HX32(1) + "0"), "sec 1: 65 digits", expect="E_PREV")
    C("prev_uppercase", set_key(t.ann, "prev", "0x" + "A" * 64), "sec 1: lowercase digits",
      expect="E_PREV")
    C("prev_empty_string", set_key(t.ann, "prev", ""), "sec 4.3 step 8", expect="E_PREV")
    C("prev_hex20", set_key(t.ann, "prev", HX20(1)), "sec 1: a hex20 in a hex32 field",
      expect="E_PREV")
    C("sig_short", set_key(t.ann, "sig", t.ann.decode().split('"sig":"')[1][:131]),
      "sec 1: 129 digits", expect="E_SIG_FORM")
    C("sig_long", set_key(t.ann, "sig", "0x" + "0" * 131), "sec 1: 131 digits", expect="E_SIG_FORM")
    C("sig_uppercase", set_key(t.ann, "sig", "0x" + "A" * 130), "sec 1: lowercase digits",
      expect="E_SIG_FORM")
    C("sig_no_prefix", set_key(t.ann, "sig", "0" * 130), "sec 1: the 0x prefix",
      expect="E_SIG_FORM")
    C("sig_empty", set_key(t.ann, "sig", ""), "sec 4.3 step 10", expect="E_SIG_FORM")

    # ---------------- sec 4.2 genesis placement and the prev hinge ----------
    C("genesis_at_zero", t.gen, "sec 4.2: entryType is genesis iff seq is 0", expect="ok")
    C("genesis_at_one_with_prev",
      make_entry(K("root"), "genesis", 1, t.gid, {"statement_md": "misplaced"}),
      "sec 4.2: a genesis off position 0", expect="E_GENESIS_PLACE")
    C("genesis_at_one_prev_null",
      craft("genesis", 1, None, {"statement_md": "misplaced"}, "0x" + "11" * 65),
      "sec 4.3: step 8 precedes step 11", expect="E_PREV_SEQ")
    C("nongenesis_at_zero",
      make_entry(K("root"), "annotation", 0, None, {"note_md": "misplaced"}),
      "sec 4.2: a non-genesis entry at position 0", expect="E_GENESIS_PLACE")
    C("unknown_type_at_zero",
      make_entry(K("root"), "windmill", 0, None, {}),
      "sec 4.2/6.8: an unknown type at position 0 is still off placement",
      expect="E_GENESIS_PLACE")
    C("nongenesis_at_zero_with_prev",
      craft("annotation", 0, HX32(1), {"note_md": "x"}, "0x" + "11" * 65),
      "sec 4.3: step 8 names the prev hinge before step 11", expect="E_PREV_SEQ")
    C("prev_null_at_five",
      craft("annotation", 5, None, {"note_md": "x"}, "0x" + "11" * 65),
      "sec 4.3 step 8: prev is null iff seq is 0", expect="E_PREV_SEQ")
    C("prev_set_at_zero",
      craft("genesis", 0, HX32(9), {"statement_md": "x"}, "0x" + "11" * 65),
      "sec 4.3 step 8: a non-null prev at seq 0", expect="E_PREV_SEQ")
    C("prev_all_zero_hex32",
      make_entry(K("root"), "annotation", 1, HX32(0), {"note_md": "x"}),
      "sec 6.10: the all-zero digest is a legal hex32 and chain law is not an entry predicate",
      expect="ok")

    # ---------------- sec 3.5 test 4 before test 5, and sec 4.3 step pairs ----
    C("key_charset_beats_value_charset",
      sub(sub(t.keyt, b'"ZZKEYZZ":"v"', b'"k\\u0001":"v","q":"\\u0001"'), b'"note_md":"n"', b'"note_md":"n"'),
      "sec 3.5: a key outside the charset and a non-prose value outside it; test 4 is decided "
      "before test 5", expect="E_KEY_CHARSET")
    # sec 3.4 item 5: members order by the decoded key, and the quote's canonical spelling
    # begins with a backslash, so the emitted bytes are not in key-byte order
    C("key_order_decoded_quote_first", b'{"\\"":1,"#":2}',
      "sec 3.4 item 5: keys order by decoded scalar values, U+0022 before U+0023, though the "
      "emitted key bytes begin 0x5C and 0x23; canonical, and no entry, so check names the "
      "envelope", expect="E_ENVELOPE_MISSING")
    C("key_order_emitted_bytes_not_decoded", b'{"#":2,"\\"":1}',
      "sec 3.4 item 5: the order of the emitted key bytes is not the canonical order",
      expect="E_NOT_CANONICAL")
    C("step4_before_step5", set_key(set_key(t.ann, "spec", "zikaron/3"), "entryType", ""),
      "sec 4.3: step 4 precedes step 5", expect="E_SPEC")
    C("step5_before_step6", set_key(set_key(t.ann, "entryType", ""), "author", "0x" + "z" * 40),
      "sec 4.3: step 5 precedes step 6", expect="E_ENTRYTYPE")
    C("step6_before_step7", set_key(set_key(t.ann, "author", "0x" + "z" * 40), "seq", "1"),
      "sec 4.3: step 6 precedes step 7", expect="E_AUTHOR")
    C("step9_before_step10", set_key(set_key(t.ann, "body", []), "sig", None),
      "sec 4.3: step 9 precedes step 10", expect="E_BODY")
    C("step10_before_step11", set_key(set_key(t.ann, "sig", None), "entryType", "genesis"),
      "sec 4.3: step 10 precedes step 11", expect="E_SIG_FORM")
    C("step11_before_step12",
      craft("genesis", 1, t.gid, {}, "0x" + "11" * 65),
      "sec 4.3: a genesis at seq 1 whose body lacks statement_md; step 11 precedes step 12",
      expect="E_GENESIS_PLACE")

    # ---------------- sec 5 signature boundaries -----------------------------
    good = Parser(t.ann.decode()).text().get("sig")
    gr = int(good[2:66], 16)
    gs = int(good[66:130], 16)
    for vv, tokname in ((0, "E_SIG_V"), (1, "E_SIG_V"), (26, "E_SIG_V"), (29, "E_SIG_V"),
                        (30, "E_SIG_V"), (255, "E_SIG_V")):
        C("sig_v_%d" % vv,
          craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, gs, vv)),
          "sec 5.4: v = %d is outside {27, 28}" % vv, expect=tokname)
    C("sig_v_27_valid", t.ann, "sec 5.4: v = 27 accepted", expect="ok")
    C("sig_v_28_valid", find_v28(t.gid), "sec 5.4: v = 28 accepted", expect="ok")
    C("sig_r_zero", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(0, gs, 27)),
      "sec 5.4: r = 0", expect="E_SIG_RANGE")
    C("sig_s_zero", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, 0, 27)),
      "sec 5.4: s = 0", expect="E_SIG_RANGE")
    C("sig_r_one", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(1, gs, 27)),
      "sec 5.4: r = 1 is the low inside edge of the range, so a later bullet names the fault",
      expect=("E_SIG_SIGNER" if pow((1 + 7) % P, (P - 1) // 2, P) == 1 else "E_SIG_RECOVER"))
    C("sig_s_one", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, 1, 27)),
      "sec 5.4: s = 1 is the low inside edge of the range and is low-s, so recovery decides",
      expect="E_SIG_SIGNER")
    C("sig_r_n", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(N, gs, 27)),
      "sec 5.4: r = n", expect="E_SIG_RANGE")
    C("sig_s_n", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, N, 27)),
      "sec 5.4: s = n", expect="E_SIG_RANGE")
    C("sig_r_n_minus_1",
      craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(N - 1, gs, 27)),
      "sec 5.4: r = n - 1 is inside the range, so a later bullet names the fault",
      expect=("E_SIG_SIGNER" if pow(((N - 1) ** 3 + 7) % P, (P - 1) // 2, P) == 1
              else "E_SIG_RECOVER"))
    C("sig_s_half_n",
      craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, HALF_N, 27)),
      "sec 5.4: s exactly (n-1)/2 passes the low-s test, so a later bullet names the fault",
      expect="E_SIG_SIGNER")
    C("sig_s_half_n_plus_1",
      craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, HALF_N + 1, 27)),
      "sec 5.4: one past (n-1)/2", expect="E_SIG_HIGH_S")
    C("sig_s_n_minus_1",
      craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, N - 1, 27)),
      "sec 5.4: s = n - 1 is in range and high", expect="E_SIG_HIGH_S")
    C("sig_malleability_twin",
      craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, N - gs, 55 - 27)),
      "sec 5.4: the malleability twin (r, n-s, 55-v) of a published signature",
      expect="E_SIG_HIGH_S")
    C("sig_high_s_beats_recover",
      craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(bad_r(), N - 1, 27)),
      "sec 5.4: the low-s bullet is decided before recovery", expect="E_SIG_HIGH_S")
    C("sig_no_curve_point",
      craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(bad_r(), gs, 27)),
      "sec 5.4: no point of secp256k1 has this x-coordinate", expect="E_SIG_RECOVER")
    C("sig_point_at_infinity", infinity_sig_entry(t.gid),
      "sec 5.4: sR = zG, so P is the point at infinity and recovery fails",
      expect="E_SIG_RECOVER")
    C("sig_wrong_signer", make_entry(K("k2"), "annotation", 1, t.gid, {"note_md": "x"},
                                     author=A_("root")),
      "sec 5.5: signed by a key that is not the author", expect="E_SIG_SIGNER")
    C("sig_galeed_domain", foreign_domain_sig_entry(t.gid, "galeed/1"),
      "sec 5.6: a signature made under the twin law's domain placed in sig", expect="E_SIG_SIGNER")
    C("sig_zikaron1_domain", foreign_domain_sig_entry(t.gid, "zikaron/1"),
      "sec 5.6: a signature made under the previous version's domain, a 76-byte message like "
      "this law's, placed in sig; the literal decides", expect="E_SIG_SIGNER")
    C("sig_zikaron3_domain", foreign_domain_sig_entry(t.gid, "zikaron/3"),
      "sec 5.6: a signature under the next version's domain; lengths decide nothing",
      expect="E_SIG_SIGNER")
    C("sig_family_domain", foreign_domain_sig_entry(t.gid, "galeed.amanah/1"),
      "sec 5.6: a signature under a family law's domain is not an entry signature",
      expect="E_SIG_SIGNER")
    C("sig_over_wrong_body", sig_over_other_b6(t.gid),
      "sec 5.1: a signature over a different B6 of the same author", expect="E_SIG_SIGNER")
    C("sig_body_field_beats_signature",
      craft("annotation", 1, t.gid, {}, "0x" + "11" * 65),
      "sec 4.3: step 12 precedes step 13", expect="E_BODY_FIELD")

    # ---------------- sec 6 body tables --------------------------------------
    def bc(slug, etype, body, note, expect, seq=1, prev=None):
        C(slug, make_entry(K("root"), etype, seq, (t.gid if prev is None else prev), body),
          note, expect=expect)

    E = "E_BODY_FIELD"
    # 6.1 genesis
    C("body_genesis_ok", t.gen, "sec 6.1: statement_md present as a prose string", expect="ok")
    C("body_genesis_missing",
      make_entry(K("root"), "genesis", 0, None, {}), "sec 6.1: statement_md absent", expect=E)
    C("body_genesis_int",
      make_entry(K("root"), "genesis", 0, None, {"statement_md": 1}),
      "sec 6.1: statement_md with a form its row refuses", expect=E)
    C("body_genesis_null",
      make_entry(K("root"), "genesis", 0, None, {"statement_md": None}),
      "sec 6: an optional-looking null is a present value with a refused form", expect=E)
    C("body_genesis_object",
      make_entry(K("root"), "genesis", 0, None, {"statement_md": {"a": "\x00"}}),
      "sec 3.3/6.1: the prose subtree still opens for the charset, and the row still refuses "
      "an object", expect=E)
    C("body_genesis_array",
      make_entry(K("root"), "genesis", 0, None, {"statement_md": ["x"]}), "sec 6.1", expect=E)
    C("body_genesis_extra",
      make_entry(K("root"), "genesis", 0, None, {"statement_md": "s", "x": 1, "y_md": "\x00"}),
      "sec 6.10: members beyond the table, at the body's top level", expect="ok")

    # 6.2 history
    HM = {"content": HX32(0xC0), "mode": {"mark": "hand", "toolchain": HX32(7)}}
    C("body_history_ok", t.hist, "sec 6.2: content and mode present", expect="ok")
    bc("body_history_note", "history", dict(HM, note_md="a\x00b"),
       "sec 6.2: the optional note_md as prose", expect="ok")
    bc("body_history_content_missing", "history", {"mode": HM["mode"]},
       "sec 6.2: content absent", expect=E)
    bc("body_history_content_hex20", "history", dict(HM, content=HX20(1)),
       "sec 6.2: a 20-byte digest is not a legal content", expect=E)
    bc("body_history_content_upper", "history", dict(HM, content="0x" + "A" * 64),
       "sec 1: hex32 is lowercase", expect=E)
    bc("body_history_content_int", "history", dict(HM, content=1),
       "sec 6.2: content with a refused form", expect=E)
    bc("body_history_content_null", "history", dict(HM, content=None),
       "sec 6: null is present with a refused form", expect=E)
    bc("body_history_content_zero", "history", dict(HM, content=HX32(0)),
       "sec 6.11: the all-zero digest is a legal spelling", expect="ok")
    bc("body_history_mode_missing", "history", {"content": HX32(1)},
       "sec 6.2: mode absent", expect=E)
    bc("body_history_mode_string", "history", dict(HM, mode="hand"),
       "sec 6.2: mode is an object", expect=E)
    bc("body_history_mode_null", "history", dict(HM, mode=None), "sec 6.2", expect=E)
    bc("body_history_mode_array", "history", dict(HM, mode=[]), "sec 6.2", expect=E)
    bc("body_history_mode_no_mark", "history", dict(HM, mode={"toolchain": HX32(7)}),
       "sec 6.2: mark absent inside mode", expect=E)
    bc("body_history_mode_no_toolchain", "history", dict(HM, mode={"mark": "hand"}),
       "sec 6.2: toolchain absent inside mode", expect=E)
    bc("body_history_mode_mark_empty", "history",
       dict(HM, mode={"mark": "", "toolchain": HX32(7)}),
       "sec 1: a token is non-empty", expect=E)
    bc("body_history_mode_mark_space", "history",
       dict(HM, mode={"mark": "a b", "toolchain": HX32(7)}),
       "sec 1: 0x20 is outside a token", expect=E)
    bc("body_history_mode_mark_int", "history",
       dict(HM, mode={"mark": 1, "toolchain": HX32(7)}), "sec 6.2", expect=E)
    bc("body_history_mode_toolchain_short", "history",
       dict(HM, mode={"mark": "hand", "toolchain": HX32(7)[:-1]}), "sec 1", expect=E)
    bc("body_history_mode_extra", "history",
       dict(HM, mode={"mark": "hand", "toolchain": HX32(7), "z": {"deep": [1, 2]}}),
       "sec 6.10: members beyond mode's row, at depth", expect="ok")
    bc("body_history_note_null", "history", dict(HM, note_md=None),
       "sec 6: an optional field written null", expect=E)
    bc("body_history_note_int", "history", dict(HM, note_md=1), "sec 6.2", expect=E)

    # 6.3 grant
    GB = {"grantee": HX20(0xBEEF), "work": HX32(0xC0), "terms": HX32(0x7E)}
    C("body_grant_ok", t.grant, "sec 6.3: the three required fields plus a window", expect="ok")
    bc("body_grant_minimal", "grant", GB, "sec 6.3: every optional field absent", expect="ok")
    bc("body_grant_no_grantee", "grant", {"work": HX32(1), "terms": HX32(2)}, "sec 6.3", expect=E)
    bc("body_grant_no_work", "grant", {"grantee": HX20(1), "terms": HX32(2)}, "sec 6.3", expect=E)
    bc("body_grant_no_terms", "grant", {"grantee": HX20(1), "work": HX32(2)}, "sec 6.3", expect=E)
    bc("body_grant_grantee_hex32", "grant", dict(GB, grantee=HX32(1)), "sec 1", expect=E)
    bc("body_grant_grantee_null", "grant", dict(GB, grantee=None), "sec 6", expect=E)
    bc("body_grant_grantee_self", "grant", dict(GB, grantee=A_("root")),
       "sec 6.3: the author's own address is a legal grantee", expect="ok")
    bc("body_grant_grantee_zero", "grant", dict(GB, grantee=HX20(0)),
       "sec 6.11: the all-zero address", expect="ok")
    bc("body_grant_history_ok", "grant", dict(GB, history=HX32(0xDEAD)),
       "sec 6.3: the optional history reference", expect="ok")
    bc("body_grant_history_null", "grant", dict(GB, history=None), "sec 6", expect=E)
    bc("body_grant_history_hex20", "grant", dict(GB, history=HX20(1)), "sec 1", expect=E)
    bc("body_grant_window_equal", "grant", dict(GB, window={"from": 5, "to": 5}),
       "sec 6.3: from == to holds the relation", expect="ok")
    bc("body_grant_window_greater", "grant", dict(GB, window={"from": 6, "to": 5}),
       "sec 6.3: from > to breaks the stated relation", expect=E)
    bc("body_grant_window_zero_zero", "grant", dict(GB, window={"from": 0, "to": 0}),
       "sec 6.3: both ends at zero", expect="ok")
    bc("body_grant_window_no_from", "grant", dict(GB, window={"to": 5}), "sec 6.3", expect=E)
    bc("body_grant_window_no_to", "grant", dict(GB, window={"from": 5}), "sec 6.3", expect=E)
    bc("body_grant_window_null", "grant", dict(GB, window=None), "sec 6", expect=E)
    bc("body_grant_window_array", "grant", dict(GB, window=[5, 6]), "sec 6.3", expect=E)
    bc("body_grant_window_from_string", "grant", dict(GB, window={"from": "5", "to": 6}),
       "sec 6.3", expect=E)
    bc("body_grant_window_from_bool", "grant", dict(GB, window={"from": True, "to": 6}),
       "sec 3.1/6.3: true is not an integer", expect=E)
    bc("body_grant_window_extra", "grant",
       dict(GB, window={"from": 1, "to": 2, "tz": "UTC", "nested": {"a": [1]}}),
       "sec 6.10: members beyond window's row", expect="ok")
    bc("body_grant_scope_null", "grant", dict(GB, scope_md=None), "sec 6", expect=E)
    bc("body_grant_scope_prose", "grant", dict(GB, scope_md="one work\x00two"),
       "sec 6.3: scope_md is prose", expect="ok")
    bc("body_grant_scope_int", "grant", dict(GB, scope_md=7), "sec 6.3", expect=E)

    # 6.4 revocation
    C("body_revocation_ok", t.rev, "sec 6.4: grant present", expect="ok")
    bc("body_revocation_missing", "revocation", {}, "sec 6.4: grant absent", expect=E)
    bc("body_revocation_grant_hex20", "revocation", {"grant": HX20(1)}, "sec 1", expect=E)
    bc("body_revocation_grant_null", "revocation", {"grant": None}, "sec 6", expect=E)
    bc("body_revocation_case_ok", "revocation", {"grant": HX32(1), "case": HX32(2)},
       "sec 6.4: the optional case file hash", expect="ok")
    bc("body_revocation_case_null", "revocation", {"grant": HX32(1), "case": None}, "sec 6",
       expect=E)
    bc("body_revocation_case_short", "revocation", {"grant": HX32(1), "case": HX32(2)[:-2]},
       "sec 1", expect=E)
    bc("body_revocation_dangling", "revocation", {"grant": HX32(0xFFFF)},
       "sec 6.4/11: a dangling reference is a legal entry", expect="ok")

    # 6.5 succession
    SB = {"to": A_("k2"), "kind": "handover", "effective": 1700000000, "statement_md": "s"}
    C("body_succession_ok", t.succ, "sec 6.5: all four required fields", expect="ok")
    bc("body_succession_no_to", "succession", {k: v for k, v in SB.items() if k != "to"},
       "sec 6.5", expect=E)
    bc("body_succession_no_kind", "succession", {k: v for k, v in SB.items() if k != "kind"},
       "sec 6.5", expect=E)
    bc("body_succession_no_effective", "succession",
       {k: v for k, v in SB.items() if k != "effective"}, "sec 6.5", expect=E)
    bc("body_succession_no_statement", "succession",
       {k: v for k, v in SB.items() if k != "statement_md"}, "sec 6.5", expect=E)
    bc("body_succession_to_hex32", "succession", dict(SB, to=HX32(1)), "sec 1", expect=E)
    bc("body_succession_kind_keyrotation", "succession", dict(SB, kind="keyrotation"),
       "sec 6.5: the grammar names keyrotation and reads nothing", expect="ok")
    bc("body_succession_kind_space", "succession", dict(SB, kind="hand over"),
       "sec 1/6: 0x20 is the one scalar value sec 3.5 admits and a token refuses", expect=E)
    bc("body_succession_kind_unknown", "succession", dict(SB, kind="court-order/2031"),
       "sec 6.5: the grammar refuses no other value", expect="ok")
    bc("body_succession_kind_empty", "succession", dict(SB, kind=""), "sec 1", expect=E)
    bc("body_succession_kind_int", "succession", dict(SB, kind=1), "sec 6.5", expect=E)
    bc("body_succession_effective_zero", "succession", dict(SB, effective=0),
       "sec 6.5: Unix seconds zero", expect="ok")
    bc("body_succession_effective_string", "succession", dict(SB, effective="1700000000"),
       "sec 6.5", expect=E)
    bc("body_succession_effective_null", "succession", dict(SB, effective=None), "sec 6",
       expect=E)
    bc("body_succession_to_self", "succession", dict(SB, to=A_("root")),
       "sec 6.5: `to` equal to `author` is a legal spelling; a seat writes no such succession, "
       "and sec 7.3 walks it as written", expect="ok")
    bc("body_succession_statement_int", "succession", dict(SB, statement_md=1), "sec 6.5",
       expect=E)

    # 6.7 annotation
    C("body_annotation_ok", t.ann, "sec 6.7: note_md present", expect="ok")
    bc("body_annotation_no_note", "annotation", {}, "sec 6.7", expect=E)
    bc("body_annotation_note_empty", "annotation", {"note_md": ""},
       "sec 6.11: the empty prose string is a legal spelling of a prose string row", expect="ok")
    bc("body_annotation_subject_ok", "annotation", {"note_md": "n", "subject": HX32(3)},
       "sec 6.7: the optional subject", expect="ok")
    bc("body_annotation_subject_null", "annotation", {"note_md": "n", "subject": None},
       "sec 6", expect=E)
    bc("body_annotation_subject_hex20", "annotation", {"note_md": "n", "subject": HX20(3)},
       "sec 1", expect=E)
    bc("body_annotation_note_object", "annotation", {"note_md": {"a": "b"}}, "sec 6.7", expect=E)
    bc("body_annotation_note_null", "annotation", {"note_md": None}, "sec 6", expect=E)

    # 6.6 migration
    MB = {"spec": "zikaron/3", "statement_md": "the seat moves on"}
    C("body_migration_ok", t.mig, "sec 6.6: spec and statement_md present", expect="ok")
    bc("body_migration_no_spec", "migration", {"statement_md": "s"}, "sec 6.6: spec absent", expect=E)
    bc("body_migration_no_statement", "migration", {"spec": "zikaron/3"},
       "sec 6.6: statement_md absent", expect=E)
    bc("body_migration_spec_empty", "migration", dict(MB, spec=""), "sec 1: a token is non-empty",
       expect=E)
    bc("body_migration_spec_space", "migration", dict(MB, spec="zikaron /3"),
       "sec 1: 0x20 is outside a token", expect=E)
    bc("body_migration_spec_int", "migration", dict(MB, spec=3), "sec 6.6", expect=E)
    bc("body_migration_spec_any_token", "migration", dict(MB, spec="galeed/1"),
       "sec 6.6: the spec is data for readers; the grammar refuses no token", expect="ok")
    bc("body_migration_statement_null", "migration", dict(MB, statement_md=None), "sec 6",
       expect=E)
    bc("body_migration_extra", "migration", dict(MB, reason_md="\x00", z=[1]),
       "sec 6.10: members beyond the table", expect="ok")
    bc("body_migration_at_seq_one", "migration", MB,
       "sec 6.6: the position a migration sits at is chain law, never a form test", expect="ok")

    # 6.8 retraction
    C("body_retraction_ok", t.retr, "sec 6.8: subject present", expect="ok")
    bc("body_retraction_no_subject", "retraction", {}, "sec 6.8: subject absent", expect=E)
    bc("body_retraction_subject_hex20", "retraction", {"subject": HX20(1)}, "sec 1", expect=E)
    bc("body_retraction_subject_null", "retraction", {"subject": None}, "sec 6", expect=E)
    bc("body_retraction_note_ok", "retraction", {"subject": HX32(1), "note_md": "why\x00"},
       "sec 6.8: the optional note_md as prose", expect="ok")
    bc("body_retraction_note_null", "retraction", {"subject": HX32(1), "note_md": None}, "sec 6",
       expect=E)
    bc("body_retraction_note_int", "retraction", {"subject": HX32(1), "note_md": 1}, "sec 6.8",
       expect=E)
    bc("body_retraction_dangling", "retraction", {"subject": HX32(0xFFFF)},
       "sec 6.8/11: a subject naming no entry is a legal entry whose reference dangles",
       expect="ok")
    bc("body_retraction_subject_zero", "retraction", {"subject": HX32(0)},
       "sec 6.11: the all-zero digest is a legal spelling", expect="ok")

    # 6.1 continues
    CB = {"statement_md": "continued", "continues": {"root": A_("root"), "entry_id": HX32(0x51)}}
    C("body_genesis_continues_ok", t.cont, "sec 6.1: continues with its two members", expect="ok")
    C("body_genesis_continues_extra",
      make_entry(K("root"), "genesis", 0, None,
                 dict(CB, continues={"root": A_("root"), "entry_id": HX32(0x51), "law": "zikaron/1"})),
      "sec 6.1/6.10: members beyond root and entry_id are permitted inside continues", expect="ok")
    C("body_genesis_continues_no_root",
      make_entry(K("root"), "genesis", 0, None, dict(CB, continues={"entry_id": HX32(0x51)})),
      "sec 6.1: root absent inside continues", expect=E)
    C("body_genesis_continues_no_entry_id",
      make_entry(K("root"), "genesis", 0, None, dict(CB, continues={"root": A_("root")})),
      "sec 6.1: entry_id absent inside continues", expect=E)
    C("body_genesis_continues_root_hex32",
      make_entry(K("root"), "genesis", 0, None, dict(CB, continues={"root": HX32(1), "entry_id": HX32(0x51)})),
      "sec 1: root is hex20", expect=E)
    C("body_genesis_continues_entry_id_hex20",
      make_entry(K("root"), "genesis", 0, None, dict(CB, continues={"root": A_("root"), "entry_id": HX20(1)})),
      "sec 1: entry_id is hex32", expect=E)
    C("body_genesis_continues_string",
      make_entry(K("root"), "genesis", 0, None, dict(CB, continues=HX32(0x51))),
      "sec 6.1: continues is an object", expect=E)
    C("body_genesis_continues_null",
      make_entry(K("root"), "genesis", 0, None, dict(CB, continues=None)),
      "sec 6: an optional field written null is present with a refused form", expect=E)
    C("body_genesis_continues_empty_object",
      make_entry(K("root"), "genesis", 0, None, dict(CB, continues={})),
      "sec 6.1: both required members absent", expect=E)
    C("body_genesis_continues_off_root",
      make_entry(K("root"), "annotation", 1, t.gid, {"note_md": "n", "continues": {"root": A_("root"), "entry_id": HX32(0x51)}}),
      "sec 6.10: continues on a body of another type is an extra member and is not tested",
      expect="ok")

    # 6.2 for
    FB = {"app": "eravon", "identity": HX20(0xE), "ref": "case/7"}
    bc("body_history_for_ok", "history", dict(HM, **{"for": FB}),
       "sec 6.2: the optional for object with its three required members", expect="ok")
    bc("body_history_for_seat", "history", dict(HM, **{"for": dict(FB, seat="seller")}),
       "sec 6.2: for with the optional seat token", expect="ok")
    bc("body_history_for_extra", "history", dict(HM, **{"for": dict(FB, milestone=2, note_md="\x00")}),
       "sec 6.10: members beyond for's row", expect="ok")
    bc("body_history_for_no_app", "history", dict(HM, **{"for": {"identity": HX20(0xE), "ref": "r"}}),
       "sec 6.2: app absent inside for", expect=E)
    bc("body_history_for_no_identity", "history", dict(HM, **{"for": {"app": "a", "ref": "r"}}),
       "sec 6.2: identity absent", expect=E)
    bc("body_history_for_no_ref", "history", dict(HM, **{"for": {"app": "a", "identity": HX20(0xE)}}),
       "sec 6.2: ref absent", expect=E)
    bc("body_history_for_app_space", "history", dict(HM, **{"for": dict(FB, app="a b")}),
       "sec 1: 0x20 is outside a token", expect=E)
    bc("body_history_for_ref_empty", "history", dict(HM, **{"for": dict(FB, ref="")}),
       "sec 1: a token is non-empty", expect=E)
    bc("body_history_for_identity_hex32", "history", dict(HM, **{"for": dict(FB, identity=HX32(1))}),
       "sec 1: identity is hex20", expect=E)
    bc("body_history_for_seat_null", "history", dict(HM, **{"for": dict(FB, seat=None)}),
       "sec 6: an optional member written null inside for", expect=E)
    bc("body_history_for_seat_space", "history", dict(HM, **{"for": dict(FB, seat="a b")}),
       "sec 1: seat is a token", expect=E)
    bc("body_history_for_app_empty", "history", dict(HM, **{"for": dict(FB, app="")}),
       "sec 1: a token is non-empty; app", expect=E)
    bc("body_history_for_ref_space", "history", dict(HM, **{"for": dict(FB, ref="case 7")}),
       "sec 1: 0x20 is outside a token; ref", expect=E)
    bc("body_history_for_seat_empty", "history", dict(HM, **{"for": dict(FB, seat="")}),
       "sec 1: a token is non-empty; seat", expect=E)
    bc("body_history_for_array", "history", dict(HM, **{"for": [FB]}),
       "sec 6.2: for is an object", expect=E)
    bc("body_history_for_null", "history", dict(HM, **{"for": None}), "sec 6", expect=E)

    # 6.3 upstream and ledger
    bc("body_grant_upstream_ok", "grant", dict(GB, upstream=HX32(0xDEAD)),
       "sec 6.3: the optional upstream reference", expect="ok")
    bc("body_grant_upstream_hex20", "grant", dict(GB, upstream=HX20(1)), "sec 1", expect=E)
    bc("body_grant_upstream_null", "grant", dict(GB, upstream=None), "sec 6", expect=E)
    bc("body_grant_upstream_int", "grant", dict(GB, upstream=1), "sec 6.3", expect=E)
    bc("body_grant_ledger_ok", "grant", dict(GB, ledger=A_("root")),
       "sec 6.3: the optional ledger root", expect="ok")
    bc("body_grant_ledger_other_key", "grant", dict(GB, ledger=HX20(0x1234)),
       "sec 6.3: ledger need not equal author once the ledger has changed hands", expect="ok")
    bc("body_grant_ledger_hex32", "grant", dict(GB, ledger=HX32(1)), "sec 1", expect=E)
    bc("body_grant_ledger_null", "grant", dict(GB, ledger=None), "sec 6", expect=E)
    bc("body_grant_ledger_upper", "grant", dict(GB, ledger="0x" + "A" * 40), "sec 1", expect=E)
    bc("body_grant_all_optional", "grant",
       dict(GB, history=HX32(5), window={"from": 1, "to": 2}, scope_md="s", upstream=HX32(6),
            ledger=A_("root")),
       "sec 6.3: every optional field present", expect="ok")

    # 6.9 unknown types
    C("body_unknown_empty", t.unk, "sec 6.9: an unknown type with an empty body", expect="ok")
    bc("body_unknown_grant_shaped", "grant-ish", GB,
       "sec 6.9: an unknown type carrying a grant-shaped body is tested by sec 6.10 alone",
       expect="ok")
    bc("body_unknown_broken_grant_shape", "grant-ish", {"grantee": 1, "work": None},
       "sec 6.9: no known type's row reaches an unknown type's body", expect="ok")
    C("body_unknown_body_not_object",
      craft("windmill", 1, t.gid, [], "0x" + "11" * 65),
      "sec 4.3: step 9 precedes step 12 for unknown types too", expect="E_BODY")
    C("body_member_named_sig",
      make_entry(K("root"), "windmill", 1, t.gid, {"sig": "0x" + "11" * 65, "n": 1}),
      "sec 5.1: B6 removes the top-level member whose key is sig and nothing else; a member "
      "named sig at any depth inside body is untouched and stays inside the preimage",
      expect="ok")
    C("body_members_named_like_envelope",
      make_entry(K("root"), "windmill", 1, t.gid,
                 {"spec": "zikaron/9", "seq": 99, "prev": None, "author": "x",
                  "entryType": "y", "body": {"sig": [1]}}),
      "sec 4.1/6.10: only the envelope is arity-closed; a body may carry members named like "
      "envelope keys", expect="ok")
    bc("body_unknown_deep_extras", "windmill",
       {"a": {"b": {"c": [{"d_md": "\x00"}, 1, None]}}, "e_md": {"f": ["\x07"]}},
       "sec 6.10: extra members at every depth, with prose subtrees inside them", expect="ok")

    # ---------------- randomized valid entries -------------------------------
    valid_pool = [t.gen, t.hist, t.grant, t.rev, t.succ, t.ann, t.mig, t.retr, t.cont, t.unk, maxent]
    types = ["genesis", "history", "grant", "revocation", "succession", "migration",
             "annotation", "retraction", "windmill", "sale/2031", "x"]
    for i in range(120):
        et = types[i % len(types)]
        seq = 0 if et == "genesis" else rng.randrange(1, 1 << 20)
        prev = None if seq == 0 else HX32(rng.getrandbits(256))
        body = random_body(rng, et)
        for _ in range(rng.randrange(0, 4)):
            k, v = random_member(rng, 0)
            body[k] = v
        b = make_entry(K(rng.choice(["root", "k2", "k3"])), et, seq, prev, body)
        valid_pool.append(b)
        C("rand_valid_%03d_%s" % (i, et.replace("/", "-")), b,
          "randomized valid entry of type %r with random extra members and random prose" % et,
          expect="ok")

    # ---------------- randomized mutations of valid entries ------------------
    kinds = ["flip", "insert", "delete", "swap", "truncate", "dup_slice",
             "shuffle_top", "shuffle_body", "splice"]
    made = 0
    i = 0
    while made < 320:
        i += 1
        base = rng.choice(valid_pool)
        kind = rng.choice(kinds)
        try:
            m = mutate(rng, base, kind, valid_pool)
        except Exception:
            continue
        if m == base or m is None:
            continue
        C("rand_mut_%03d_%s" % (made, kind), m,
          "randomized %s mutation of a valid entry" % kind)
        made += 1


# ==========================================================================
# helpers the canon cases lean on
# ==========================================================================


def craft(entryType, seq, prev, body, sigval, author=None, spec="zikaron/2"):
    if author is None:
        author = A_("root")
    bv = body if isinstance(body, (JObj, list)) else J(body)
    if isinstance(body, list):
        bv = J(body)
    env6 = build_env(spec, entryType, author, seq, prev, bv)
    return cbytes(JObj(env6.members + [("sig", sigval)]))


def find_v28(prev):
    for i in range(100000):
        env6 = build_env("zikaron/2", "annotation", A_("root"), 1, prev,
                         J({"n": i, "note_md": "seeking v=28"}))
        digest = eip191_digest("zikaron/2", sha256(cbytes(env6)))
        r, s, v = ecdsa_sign(K("root"), digest)
        if v == 28:
            return cbytes(JObj(env6.members + [("sig", sig_hex(r, s, v))]))
    raise AssertionError("no v=28")


_BAD_R = [None]


def bad_r():
    if _BAD_R[0] is None:
        r = 1
        while True:
            a = (r * r * r + 7) % P
            if a != 0 and pow(a, (P - 1) // 2, P) == P - 1:
                _BAD_R[0] = r
                break
            r += 1
    return _BAD_R[0]


def infinity_sig_entry(prev):
    env6 = build_env("zikaron/2", "annotation", A_("root"), 1, prev,
                     J({"note_md": "sR equals zG"}))
    digest = eip191_digest("zikaron/2", sha256(cbytes(env6)))
    z = int.from_bytes(digest, "big") % N
    for k in range(1, 500):
        R = mul(G, k)
        if R is None or R[0] >= N or R[0] % N == 0:
            continue
        r = R[0] % N
        s = (z * pow(k, N - 2, N)) % N
        if s == 0 or s > HALF_N:
            continue
        return cbytes(JObj(env6.members + [("sig", sig_hex(r, s, 27 + (R[1] & 1)))]))
    raise AssertionError("no infinity signature")


def _other_domain_entry(prev, domain, note):
    env6 = build_env("zikaron/2", "annotation", A_("root"), 1, prev, J({"note_md": note}))
    presig = sha256(cbytes(env6))
    r, s, v = ecdsa_sign(K("root"), eip191_digest(domain, presig))
    return cbytes(JObj(env6.members + [("sig", sig_hex(r, s, v))]))


def foreign_domain_sig_entry(prev, domain):
    return _other_domain_entry(prev, domain, "signed under " + domain)


def sig_over_other_b6(prev):
    other = build_env("zikaron/2", "annotation", A_("root"), 1, prev, J({"note_md": "other"}))
    r, s, v = ecdsa_sign(K("root"), eip191_digest("zikaron/2", sha256(cbytes(other))))
    env6 = build_env("zikaron/2", "annotation", A_("root"), 1, prev, J({"note_md": "this"}))
    return cbytes(JObj(env6.members + [("sig", sig_hex(r, s, v))]))


# ---------------- randomized value generators ------------------------------

PROSE_POOL = ([chr(c) for c in range(0x20, 0x7F)] +
              [chr(c) for c in range(0x00, 0x20)] +
              ["\x7f"] + [chr(c) for c in range(0x80, 0xA0)] +
              ["‎", "‏", "‪", "‫", "‬", "‭", "‮",
               "⁦", "⁩", "﻿", "é", "中", "\U0001f600",
               "\U0010ffff", "́"])
SKEL_POOL = [chr(c) for c in range(0x20, 0x7F)]


def rand_prose(rng, n=None):
    n = rng.randrange(0, 24) if n is None else n
    return "".join(rng.choice(PROSE_POOL) for _ in range(n))


def rand_skel(rng, n=None):
    n = rng.randrange(0, 16) if n is None else n
    return "".join(rng.choice(SKEL_POOL) for _ in range(n))


def rand_key(rng):
    while True:
        k = "".join(rng.choice(SKEL_POOL) for _ in range(rng.randrange(1, 8)))
        if not k.endswith("_md"):
            return k


def random_member(rng, depth):
    prose = rng.random() < 0.35
    k = rand_key(rng) + ("_md" if prose else "")
    return k, random_value(rng, depth, prose)


def random_value(rng, depth, prose=False):
    r = rng.random()
    if depth >= 3 or r < 0.35:
        pick = rng.randrange(6)
        if pick == 0:
            return None
        if pick == 1:
            return True
        if pick == 2:
            return False
        if pick == 3:
            return rng.randrange(0, MAXINT + 1)
        return rand_prose(rng) if prose else rand_skel(rng)
    if r < 0.6:
        return [random_value(rng, depth + 1, prose) for _ in range(rng.randrange(0, 4))]
    d = {}
    for _ in range(rng.randrange(0, 4)):
        if prose:
            d[rand_key(rng)] = random_value(rng, depth + 1, True)
        else:
            k, v = random_member(rng, depth + 1)
            d[k] = v
    return d


def random_subject(rng):
    r = rng.random()
    if r < 0.45:
        return {"kind": "evm", "chainId": rng.randrange(0, 1 << 20), "contract": HX20(rng.getrandbits(160)),
                "codeHash": HX32(rng.getrandbits(256)), "id": HX32(rng.getrandbits(256))}
    if r < 0.8:
        d = {"kind": "hash", "sha256": HX32(rng.getrandbits(256))}
        if rng.random() < 0.5:
            d["note_md"] = rand_prose(rng)
        return d
    d = {"kind": rand_token(rng)}
    for _ in range(rng.randrange(0, 3)):
        k, v = random_member(rng, 1)
        d[k] = v
    return d


def random_body(rng, etype):
    if etype == "genesis":
        b = {"statement_md": rand_prose(rng)}
        if rng.random() < 0.4:
            b["continues"] = {"root": HX20(rng.getrandbits(160)), "entry_id": HX32(rng.getrandbits(256))}
        return b
    if etype == "history":
        b = {"content": HX32(rng.getrandbits(256)),
             "mode": {"mark": rand_token(rng), "toolchain": HX32(rng.getrandbits(256))}}
        if rng.random() < 0.5:
            b["note_md"] = rand_prose(rng)
        if rng.random() < 0.4:
            b["for"] = {"app": rand_token(rng), "identity": HX20(rng.getrandbits(160)),
                        "ref": rand_token(rng)}
            if rng.random() < 0.5:
                b["for"]["seat"] = rand_token(rng)
        return b
    if etype == "grant":
        b = {"grantee": HX20(rng.getrandbits(160)), "work": HX32(rng.getrandbits(256)),
             "terms": HX32(rng.getrandbits(256))}
        if rng.random() < 0.5:
            b["history"] = HX32(rng.getrandbits(256))
        if rng.random() < 0.5:
            a = rng.randrange(0, MAXINT)
            b["window"] = {"from": a, "to": rng.randrange(a, MAXINT + 1)}
        if rng.random() < 0.5:
            b["scope_md"] = rand_prose(rng)
        if rng.random() < 0.4:
            b["upstream"] = HX32(rng.getrandbits(256))
        if rng.random() < 0.4:
            b["ledger"] = HX20(rng.getrandbits(160))
        return b
    if etype == "revocation":
        b = {"grant": HX32(rng.getrandbits(256))}
        if rng.random() < 0.5:
            b["case"] = HX32(rng.getrandbits(256))
        return b
    if etype == "migration":
        return {"spec": rand_token(rng), "statement_md": rand_prose(rng)}
    if etype == "retraction":
        b = {"subject": HX32(rng.getrandbits(256))}
        if rng.random() < 0.5:
            b["note_md"] = rand_prose(rng)
        return b
    if etype == "succession":
        return {"to": HX20(rng.getrandbits(160)), "kind": rand_token(rng),
                "effective": rng.randrange(0, MAXINT + 1), "statement_md": rand_prose(rng)}
    if etype == "annotation":
        b = {"note_md": rand_prose(rng)}
        if rng.random() < 0.5:
            b["subject"] = HX32(rng.getrandbits(256))
        return b
    return {}


def rand_token(rng):
    pool = [chr(c) for c in range(0x21, 0x7F)]
    return "".join(rng.choice(pool) for _ in range(rng.randrange(1, 10)))


def mutate(rng, base, kind, pool):
    ba = bytearray(base)
    if kind == "flip":
        i = rng.randrange(len(ba))
        ba[i] ^= 1 << rng.randrange(8)
        return bytes(ba)
    if kind == "insert":
        i = rng.randrange(len(ba) + 1)
        ba[i:i] = bytes([rng.randrange(256)])
        return bytes(ba)
    if kind == "delete":
        i = rng.randrange(len(ba))
        del ba[i]
        return bytes(ba)
    if kind == "swap":
        i = rng.randrange(len(ba) - 1)
        ba[i], ba[i + 1] = ba[i + 1], ba[i]
        return bytes(ba)
    if kind == "truncate":
        return bytes(ba[:rng.randrange(1, len(ba))])
    if kind == "dup_slice":
        i = rng.randrange(len(ba))
        j = min(len(ba), i + rng.randrange(1, 12))
        ba[i:i] = ba[i:j]
        return bytes(ba)
    if kind == "splice":
        other = rng.choice(pool)
        i = rng.randrange(len(ba))
        j = rng.randrange(len(other))
        return bytes(ba[:i]) + other[j:]
    if kind == "shuffle_top":
        v = Parser(base.decode("utf-8")).text()
        ms = list(v.members)
        rng.shuffle(ms)
        v.members = ms
        return cbytes(v, sort=False)
    if kind == "shuffle_body":
        v = Parser(base.decode("utf-8")).text()
        b = v.get("body")
        if not isinstance(b, JObj) or len(b.members) < 2:
            return None
        ms = list(b.members)
        rng.shuffle(ms)
        b.members = ms
        return cbytes(v, sort=False)
    return None


# ==========================================================================
# sign/ : (private key, six-member canonical bytes or a family document) pairs
# ==========================================================================


def build_sign(rng, t):
    def b6(etype, seq, prev, body, author_key="root", spec="zikaron/2"):
        return cbytes(build_env(spec, etype, A_(author_key), seq, prev, J(body)))

    P0 = HX32(0x11)
    S("genesis", K("root"), b6("genesis", 0, None, {"statement_md": "the works of one author"}),
      "zikaron/2", "sec 5.1: B6 of a genesis; prev null, seq 0")
    S("history", K("root"),
      b6("history", 1, P0, {"content": HX32(0xC0), "mode": {"mark": "hand", "toolchain": HX32(7)}}),
      "zikaron/2", "sec 5.1: B6 of a history entry")
    S("grant", K("root"),
      b6("grant", 2, P0, {"grantee": HX20(0xBEEF), "work": HX32(0xC0), "terms": HX32(0x7E),
                          "window": {"from": 0, "to": MAXINT}}),
      "zikaron/2", "sec 5.1: B6 of a grant carrying both ends of the integer universe")
    S("revocation", K("root"), b6("revocation", 3, P0, {"grant": HX32(0xAB), "case": HX32(0xCD)}),
      "zikaron/2", "sec 5.1: B6 of a revocation with a cited case")
    S("migration", K("root"),
      b6("migration", 4, P0, {"spec": "zikaron/3", "statement_md": "the seat moves on"}),
      "zikaron/2", "sec 5.1: B6 of a migration")
    S("retraction", K("root"), b6("retraction", 4, P0, {"subject": HX32(0xAB), "note_md": "why"}),
      "zikaron/2", "sec 5.1: B6 of a retraction")
    S("genesis_continues", K("root"),
      b6("genesis", 0, None, {"statement_md": "continued", "continues": {"root": A_("root"), "entry_id": HX32(0x51)}}),
      "zikaron/2", "sec 5.1/6.1: B6 of a genesis carrying continues")
    S("succession", K("root"),
      b6("succession", 5, P0, {"to": A_("k2"), "kind": "handover", "effective": 0,
                               "statement_md": "the seat moves"}),
      "zikaron/2", "sec 5.1: B6 of a succession")
    S("annotation", K("root"), b6("annotation", 6, P0, {"note_md": "a note"}),
      "zikaron/2", "sec 5.1: B6 of an annotation")
    S("unknown_type", K("root"), b6("windmill", 7, P0, {}),
      "zikaron/2", "sec 6.9: B6 of an unknown type with an empty body")
    S("max_seq", K("root"), b6("annotation", MAXINT, P0, {"note_md": "top"}),
      "zikaron/2", "sec 3.2: seq at 2^53 - 1 inside the preimage")
    S("prose_controls", K("root"),
      b6("annotation", 8, P0, {"note_md": "\x00\x01\x08\x09\x0a\x0c\x0d\x1f\x7f\x80"}),
      "zikaron/2", "sec 3.4 item 3: every escape form inside the preimage")
    S("prose_astral", K("root"),
      b6("annotation", 9, P0, {"note_md": "\U0001f600\U0010ffff中é‮"}),
      "zikaron/2", "sec 3.4 item 3: astral and bidi scalars raw in the preimage")
    S("deep_body", K("root"), b6("windmill", 10, P0, {"d": deep_list(120)}),
      "zikaron/2", "sec 3.4: a deeply nested body inside B6")
    S("member_order", K("root"),
      b6("windmill", 11, P0, {"a b": 1, "ab": 2, "A": 3, "a": 4, "~": 5, " ": 6}),
      "zikaron/2", "sec 3.4 item 5: bytewise key order inside the preimage")
    S("body_named_sig", K("root"),
      b6("windmill", 11, P0, {"sig": "0x" + "11" * 65, "body": {"seq": 1}}),
      "zikaron/2", "sec 5.1: a member named sig inside body sits inside B6")
    S("key_one", 1, b6("annotation", 12, P0, {"note_md": "smallest private key"}, "root"),
      "zikaron/2", "sec 5: private key 1")
    S("key_n_minus_1", N - 1, b6("annotation", 13, P0, {"note_md": "largest private key"}, "root"),
      "zikaron/2", "sec 5: private key n - 1")
    S("key_half_n", HALF_N, b6("annotation", 14, P0, {"note_md": "key at (n-1)/2"}, "root"),
      "zikaron/2", "sec 5: private key (n - 1) / 2")
    for i in range(8):
        S("rand_b6_%d" % i, testkey("sign", 100 + i),
          b6(["genesis", "history", "grant", "revocation", "migration", "succession",
              "annotation", "retraction"][i],
             0 if i == 0 else i, None if i == 0 else HX32(rng.getrandbits(256)),
             random_body(rng, ["genesis", "history", "grant", "revocation", "migration",
                               "succession", "annotation", "retraction"][i])),
          "zikaron/2", "randomized B6 with a randomized private key")

    S("previous_version_domain", K("root"), b6("annotation", 15, P0, {"note_md": "under zikaron/1"}),
      "zikaron/1", "sec 5.6 with HARNESS2: the previous version's literal is signed as given")
    S("other_literal", K("root"), cbytes(J({"note_md": "a client's document"})),
      "client.doc/1", "sec 5.6/5.7: a document under a literal no domain of this law spells")
    S("domain_tab", K("root"), cbytes(J({"note_md": "a client's document"})),
      "client.doc/1\t", "HARNESS: a domain is any bytes below 0x80 without a line feed; a tab is signed as given")
    S("domain_del", K("root"), cbytes(J({"note_md": "a client's document"})),
      "client.doc/1\x7f", "HARNESS: a domain is any bytes below 0x80 without a line feed; 0x7F is signed as given")
    S("domain_empty", K("root"), cbytes(J({"note_md": "under the empty domain"})), "",
      "HARNESS: the empty string is a domain and is signed as given")
    S("input_recanonicalized", K("root"), cbytes(J({"b": 1, "a": "x"})), "zikaron/2",
      "HARNESS: any RFC 8259 spelling is re-canonicalized before signing", raw_bytes=b'{ "b" : 1 ,\n "a":"x" }')
    S("object_empty", K("root"), cbytes(J({})), "zikaron/2",
      "HARNESS: an object of any member count is signed as given; the empty object")
    S("input_with_top_level_sig", K("root"), cbytes(J({"note_md": "carries a sig", "sig": "0x00"})), "zikaron/2",
      "HARNESS: a file carrying a top-level sig is signed with it inside the preimage; no member is removed")
    S("domain_defaulted", K("root"), b6("annotation", 16, P0, {"note_md": "no domain argument"}),
      "zikaron/2", "HARNESS: with no domain argument the domain is zikaron/2", omit_domain=True)
    S("key_bare", K("k2"), b6("annotation", 17, P0, {"note_md": "a bare key"}, author_key="k2"),
      "zikaron/2", "HARNESS: the private key spelled as sixty-four digits with no prefix", key_spelling="bare")
    S("key_upper_prefix", K("k3"), b6("annotation", 18, P0, {"note_md": "an 0X key"}, author_key="k3"),
      "zikaron/2", "HARNESS: the private key spelled with 0X and uppercase digits", key_spelling="0X")



# ==========================================================================
# audit/ : sec 8 scenarios in the HARNESS.md input format
# ==========================================================================

GENB = {"statement_md": "an author's ledger"}


def HISB(i):
    return {"content": HX32(i), "mode": {"mark": "hand", "toolchain": HX32(1)}}


def ANNB(s):
    return {"note_md": s}


def GRB(i):
    return {"grantee": HX20(i), "work": HX32(i), "terms": HX32(i + 1)}


def RVB(i):
    return {"grant": HX32(i)}


def MIGB(spec="zikaron/3"):
    return {"spec": spec, "statement_md": "the seat moves on"}


def RTB(subject):
    return {"subject": subject}


def SUCB(to, kind="handover"):
    return {"to": to, "kind": kind, "effective": 1700000000, "statement_md": "the seat moves"}


def mk(key, etype, seq, prev, body):
    return make_entry(K(key), etype, seq, prev, body)


def build_chain(specs, start_seq=0, start_prev=None):
    out, ids = [], []
    prev, seq = start_prev, start_seq
    for key, etype, body in specs:
        b = mk(key, etype, seq, (None if seq == 0 else prev), body)
        out.append(b)
        ids.append(eid(b))
        prev = eid(b)
        seq += 1
    return out, ids


def basis(chains=(), *_ignored, raw=False):
    """A sec 9.4 basis; chains in the order the section fixes unless raw."""
    ch = list(chains)
    if not raw:
        ch = sorted(ch, key=lambda c: (c["chainId"], c["fromBlock"]))
    return {"chains": ch}


def chainobj(cid=1, fb=0, tb=1000, regs=(), senders=(), raw=False):
    r, sd = list(regs), list(senders)
    if not raw:
        r = sorted(set(r), key=lambda a: a.encode())
        sd = sorted(set(sd), key=lambda a: a.encode())
    return {"chainId": cid, "fromBlock": fb, "toBlock": tb,
            "registries": r, "senders": sd}


def anchor(h, sender, cid=1, bn=100, ts=1700000000, tx=None, verdict="counted"):
    return {"chainId": cid, "blockNumber": bn, "blockTimestamp": ts,
            "tx": tx if tx else HX32(0xA0000 + bn), "sender": sender, "hash": h,
            "verdict": verdict}


def inp(root, pile, anchors=(), bas=None):
    return {"root": root, "pile": [hx(b) for b in pile], "anchors": list(anchors),
            "basis": bas if bas is not None else basis()}


ROOTB = None   # set by build_audit once the keys exist: one chain, every hand-case sender


def build_audit(rng, t):
    global ROOTB
    R = A_("root")
    ROOTB = basis((chainobj(1, senders=(R, A_("k2"), A_("k3"), A_("k9"))),))
    CL, CID = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1)),
                           ("root", "grant", GRB(2)), ("root", "annotation", ANNB("a note")),
                           ("root", "revocation", RVB(4))])

    A("clean_ledger", inp(R, CL), "A clean five-entry ledger under one key: no anchors, no "
      "gaps, no forks. sec 8.6 label 4.", "COMPLETE")
    A("empty_pile", inp(R, []), "An empty pile. sec 8.6: COMPLETE asserts only that the inputs "
      "held no fault, and the entry count travels with it.", "COMPLETE")
    FOR, _ = build_chain([("k9", "genesis", GENB), ("k9", "history", HISB(3)),
                          ("k9", "annotation", ANNB("elsewhere"))])
    A("only_foreign_entries", inp(R, FOR),
      "A pile holding only entries signed outside the audited lineage. sec 8.1: they are not "
      "input; sec 8.6 item 7 lists them as EXCLUDED and no label turns on it.", "COMPLETE")
    A("gap_at_start", inp(R, CL[1:]),
      "The seq-0 entry is absent. sec 8.2: a ledger whose smallest seq is not 0 records "
      "SEQ_GAP(0, s) first; the entry at seq 1 records no prev finding because no entry at "
      "seq 0 is held.", "GAPS")
    A("gap_in_middle", inp(R, CL[:2] + CL[3:]),
      "seq 2 is absent. sec 8.2: SEQ_GAP(2, 3), and across the gap the link is unverifiable so "
      "no PREV_MISMATCH is recorded.", "GAPS")

    SU, SUI = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(A_("k2"))),
                           ("k2", "history", HISB(4)), ("k2", "annotation", ANNB("new hand"))])
    A("succession_and_successor", inp(R, SU),
      "sec 7.3: a succession at seq 1 moves authority, and the successor's entries at 2 and 3 "
      "carry the new author.", "COMPLETE")
    SR, _ = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(R)),
                         ("root", "annotation", ANNB("still me"))])
    A("succession_to_root", inp(R, SR),
      "sec 6.5/7.3: a succession whose `to` equals the key already in office moves the seat as "
      "written, to itself; with no record anchoring it the key has no retirement, and the walk "
      "decides all the same.", "COMPLETE")
    SP, _ = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(A_("k2"))),
                         ("k2", "annotation", ANNB("k2 writes")),
                         ("k2", "succession", SUCB(R)), ("root", "annotation", ANNB("back"))])
    A("succession_to_prior_holder", inp(R, SP),
      "sec 6.5/7.3: a succession back to a key that held office before; the walk is total and "
      "moves the seat as written, and with no anchors nobody has retired.", "COMPLETE")
    SL, _ = build_chain([("root", "genesis", GENB), ("root", "annotation", ANNB("one")),
                         ("root", "succession", SUCB(A_("k2")))])
    A("succession_at_last_position", inp(R, SL),
      "sec 6.5: the ledger's last entry hands the seat off and nothing follows; silence is the "
      "ordinary end.", "COMPLETE")
    A("withheld_succession", inp(R, [SU[0]] + SU[2:]),
      "sec 8.6: the successor's entries are present and the succession that would admit them is "
      "absent. The successor is outside the lineage, so its entries are EXCLUDED, the "
      "ledger stops at seq 0, and no walk can see the missing entry.", "COMPLETE")

    # forks. Wherever the law orders two entries by entry_id (8.2 walk ties, 8.2
    # competing successions, 8.4 a/b), the case is built in both id orders by a
    # deterministic search over the competitor's prose.
    def both_orders(slug, build, note, expect_label=None):
        got = {}
        k = 0
        while len(got) < 2:
            inp_, asc = build(k)
            got.setdefault(asc, inp_)
            k += 1
            assert k < 256, slug
        for asc in (True, False):
            A(slug + ("_ids_ascending" if asc else "_ids_descending"), got[asc],
              note + (" Here the entry met first has the smaller id." if asc
                      else " Here the entry met first has the larger id."), expect_label)

    tw_a = mk("root", "annotation", 3, CID[2], ANNB("left"))
    tw_b = mk("root", "annotation", 3, CID[2], ANNB("right"))
    tw_c = mk("root", "annotation", 3, CID[2], ANNB("third"))
    both_orders("fork_two_way",
                lambda k: (lambda b: (inp(R, CL[:3] + [tw_a, b]), eid(tw_a) < eid(b)))(
                    mk("root", "annotation", 3, CID[2], ANNB("right %d" % k))),
                "sec 8.4: two ledger entries with different entry_id at one seq. One EQUIVOCATION, "
                "hard, naming the bytewise-smaller id first.", "BROKEN_CHAIN")
    s2 = mk("root", "succession", 3, CID[2], SUCB(A_("k2")))
    e2 = mk("k2", "annotation", 4, eid(s2), ANNB("k2 writes after"))

    def competing(k):
        s3 = mk("root", "succession", 3, CID[2], SUCB(A_("k3"), kind="handover-%d" % k))
        e3 = mk("k3", "annotation", 4, eid(s3), ANNB("k3 writes after"))
        return inp(R, CL[:3] + [s2, e2, s3, e3]), eid(s2) < eid(s3)
    both_orders("fork_competing_successions", competing,
                "sec 8.2: two successions compete at seq 3; authority after 3 follows the one with "
                "the bytewise-smallest entry_id, so exactly one of the two seq-4 entries is an "
                "AUTHORITY_MISMATCH, and which one turns on the id order.", "BROKEN_CHAIN")
    A("fork_three_way", inp(R, CL[:3] + [tw_a, tw_b, tw_c]),
      "sec 8.4: three entries colliding at one position record three findings, one per "
      "unordered pair.", "BROKEN_CHAIN")
    tw_s = mk("root", "succession", 3, CID[2], SUCB(A_("k2")))
    A("fork_with_succession_twin", inp(R, CL[:3] + [tw_a, tw_s] +
                                       [mk("k2", "annotation", 4, eid(tw_s), ANNB("after"))]),
      "sec 8.2: a fork at k where one twin is a succession signed by the key in office. "
      "Authority after k follows that succession, the annotation twin moving nothing, so the "
      "successor's entry at k+1 records no finding beside the fork.",
      "BROKEN_CHAIN")
    gen2 = mk("k2", "genesis", 0, None, {"statement_md": "a second opening"})
    A("twins_at_seq0", inp(R, [CL[0], gen2, mk("root", "succession", 1, CID[0], SUCB(A_("k2")))]),
      "sec 7.5/8.2: a lineage key opens a competing genesis under the audited seat. "
      "ROOT_MISMATCH on the second seq-0 entry and EQUIVOCATION between the two.",
      "BROKEN_CHAIN")
    ret = mk("root", "annotation", 2, SUI[1], ANNB("the retired key writes again"))
    A("fork_by_retired_key", inp(R, SU[:3] + [ret]),
      "sec 7.3/8.4: a key that handed off signs again at a later position. AUTHORITY_MISMATCH "
      "while certain, plus EQUIVOCATION with the successor's entry at that seq. BROKEN_CHAIN "
      "says some key of this seat's lineage equivocated and never which one is at fault.",
      "BROKEN_CHAIN")
    sp2 = mk("root", "annotation", 2, CID[0], ANNB("wrong head"))
    A("shared_prev_across_seqs", inp(R, CL[:2] + [sp2]),
      "sec 8.4: two entries at different seq sharing a non-null prev. The finding's seq is the "
      "smaller of the two, and PREV_MISMATCH also fires at seq 2.", "BROKEN_CHAIN")
    pm = mk("root", "annotation", 2, HX32(0xDEAD), ANNB("dangling head"))
    A("prev_mismatch_adjacent", inp(R, CL[:2] + [pm]),
      "sec 8.2: an entry at seq 2 whose prev matches no held entry at seq 1.", "BROKEN_CHAIN")
    A("prev_mismatch_across_gap", inp(R, CL[:2] + [mk("root", "annotation", 4, HX32(0xDEAD),
                                                      ANNB("across the gap"))]),
      "sec 8.2: the same broken link one position further out, where no entry at s-1 is held. "
      "The gap already is the finding and no prev finding is recorded.", "GAPS")

    AM, AMI = build_chain([("root", "genesis", GENB), ("root", "annotation", ANNB("one"))])
    am2 = mk("k2", "annotation", 2, AMI[1], ANNB("not yet in office"))
    am3 = mk("root", "succession", 3, eid(am2), SUCB(A_("k2")))
    A("authority_mismatch_certain", inp(R, AM + [am2, am3]),
      "sec 8.2: the walk is contiguous from 0, so the authority contradiction at seq 2 is hard.",
      "BROKEN_CHAIN")
    am4 = mk("k2", "annotation", 4, HX32(0x44), ANNB("after a gap"))
    am6 = mk("root", "succession", 6, HX32(0x66), SUCB(A_("k2")))
    A("authority_mismatch_after_gap", inp(R, AM + [am4, am6]),
      "sec 8.2: a gap could hide a succession, so after a gap the same finding is recorded soft "
      "and no label turns hard.", "GAPS")

    mx = mk("root", "annotation", MAXINT, HX32(0x77), ANNB("the last position"))
    A("seq_at_2p53_minus_1", inp(R, [CL[0], mx]),
      "sec 8.2: at seq 2^53 - 1 the walk leaves `expected` at that value, since no entry can "
      "occupy a position beyond the universe of sec 3.1.", "GAPS")
    A("seq_max_twins", inp(R, [CL[0], mx, mk("root", "annotation", MAXINT, HX32(0x77),
                                             ANNB("a twin at the last position"))]),
      "sec 8.2/8.4: two entries at 2^53 - 1; the second is a fork twin and records no second gap.",
      "BROKEN_CHAIN")

    # ---------------- anchors and two-way reconciliation (sec 8.5, 8.6) ------
    H_UNSEEN = HX32(0xBEEFBEEF)
    A("anchors_all_counted",
      inp(R, CL, [anchor(h, R, bn=10 + i) for i, h in enumerate(CID)], bas=ROOTB),
      "sec 8.5: every ledger entry_id is in the counted anchor set, so both directions are "
      "empty.", "COMPLETE")
    A("unanchored_informational",
      inp(R, CL, [anchor(CID[0], R)], bas=ROOTB),
      "sec 8.5: UNANCHORED is informational and type-agnostic; four rows here and no label "
      "turns on them.", "COMPLETE")
    A("anchor_verdict_unproven",
      inp(R, CL, [anchor(CID[0], R)] + [anchor(CID[1], R, bn=11, verdict="UNPROVEN")],
          bas=ROOTB),
      "sec 9.3/8.6: an UNPROVEN record takes no part in the reconciliation and is carried in "
      "the UNPROVEN list; its hash is in have, so label rule 2 does not fire.", "COMPLETE")
    A("anchor_verdict_void",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="VOID")], bas=ROOTB),
      "sec 9.3/8.6: a VOID record is informational, takes no part in the reconciliation, and no "
      "label turns on it, even when its hash is in no ledger entry.", "COMPLETE")
    A("anchor_verdict_void_and_unproven",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="VOID"),
                  anchor(HX32(0xF00D), R, bn=12, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6: both lists non-empty; only the UNPROVEN list moves the label.", "UNPROVEN")
    A("anchor_sender_outside_lineage",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9"))], bas=ROOTB),
      "sec 8.1: the audit discards every anchor record whose sender is not in the lineage, so "
      "no key outside it can put a hash into this ledger's reconciliation.",
      "COMPLETE")
    A("anchor_sender_handed_off_no_retirement",
      inp(R, SU, [anchor(eid(SU[2]), R)], bas=ROOTB),
      "sec 7.4/8.1: the root handed the seat off at seq 1, but no record anchors that succession, "
      "so the root has no retirement in this audit and its anchor of the successor's entry "
      "counts, as zikaron/1 reads every key.", "COMPLETE")
    A("anchor_outside_basis_reach",
      inp(R, CL, [anchor(CID[0], R)], bas=basis((chainobj(1, senders=(A_("k2"),)),))),
      "sec 9.4 [C]: a record no `chains` object reaches (its sender outside that object's "
      "`senders`) is a record no scan of that basis produces; no label.",
      "NO_LABEL")
    A("anchor_outside_basis_range",
      inp(R, CL, [anchor(CID[0], R, bn=2000)], bas=ROOTB),
      "sec 9.4 [C]: a record whose blockNumber lies outside every `chains` object's range of its "
      "chain is outside the basis's reach; no label.",
      "NO_LABEL")
    A("anchor_outside_basis_chain",
      inp(R, CL, [anchor(CID[0], R, cid=10)], bas=ROOTB),
      "sec 9.4 [C]: a record on a chain the basis does not declare is outside its reach; no label.",
      "NO_LABEL")
    A("missing_on_two_chains",
      inp(R, CL, [anchor(H_UNSEEN, R, cid=1, bn=50, ts=1700000000, tx=HX32(0x501)),
                  anchor(H_UNSEEN, R, cid=8453, bn=10, ts=1699000000, tx=HX32(0x101))],
          bas=basis((chainobj(1, senders=(R,)), chainobj(8453, senders=(R,))), ())),
      "sec 8.6 item 4: one MISSING row carrying both anchoring transactions in ascending "
      "(chainId, blockNumber, tx) order; sec 9.2: the smaller blockTimestamp gives the "
      "existence bound and blockNumber never orders across chains.", "GAPS")
    A("anchored_hash_missing", inp(R, CL, [anchor(H_UNSEEN, R)], bas=ROOTB),
      "sec 8.5 forward: a counted hash that is not in `have`.", "GAPS")
    A("anchored_excluded_entry",
      inp(R, CL + [FOR[0]], [anchor(eid(FOR[0]), R)], bas=ROOTB),
      "sec 8.5: `have` holds ledger entry_ids and nothing else, so bytes held under a "
      "signature outside the lineage are MISSING while the bytes appear in EXCLUDED.", "GAPS")
    BADB = bytearray(CL[1]); BADB[len(BADB) // 2] ^= 0x01; BADB = bytes(BADB)
    A("anchored_bytes_fail_4_3",
      inp(R, CL + [BADB], [anchor(eid(BADB), R)], bas=ROOTB),
      "sec 8.1/8.5: a byte string that was retrieved and fails sec 4.3 is not input, and its "
      "anchor lands in MISSING.", "GAPS")
    A("hard_beats_unproven",
      inp(R, CL[:3] + [tw_a, tw_b], [anchor(H_UNSEEN, R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 label order: rule 1 fires first.", "BROKEN_CHAIN")
    A("unproven_beats_gaps",
      inp(R, CL[:2] + CL[3:], [anchor(H_UNSEEN, R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 label order: rule 2 fires before rule 3 even though a SEQ_GAP was recorded.",
      "UNPROVEN")

    # ---------------- pile shape, basis shape, duplicate records -------------
    A("duplicates_in_pile", inp(R, CL + CL),
      "sec 8.1: byte-identical copies collapse to one entry, and the report does not depend on "
      "the order in which any input was supplied.", "COMPLETE")
    A("pile_of_malformed_only", inp(R, [BADB, b"{}", b"not json at all"]),
      "sec 8.1: byte strings that fail sec 4.3 are no ledger entries: they enter no walk, no "
      "have and no EXCLUDED row, and sec 8.6 item 10 lists each with its token.", "COMPLETE")
    XG, XI = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1))])
    A2 = mk("root", "annotation", 2, XI[1], ANNB("first on this head"))
    both_orders("equivocation_shared_prev",
                lambda k: (lambda b: (inp(R, XG + [A2, b]), eid(A2) < eid(b)))(
                    mk("root", "annotation", 3, XI[1], ANNB("second on this head %d" % k))),
                "sec 8.4: two ledger entries share a prev at seqs 2 and 3; the finding names the "
                "bytewise-smaller entry_id first whichever entry the walk met first.")
    WF = inp(R, CL)
    A_raw("input_not_json", b"not a json text at all",
          "HARNESS2: the audit input is not a JSON text; no label.")
    A_raw("input_not_utf8", b"\xff\xfe" + json.dumps(WF).encode("utf-8"),
          "HARNESS2: the audit input is not valid UTF-8; no label.")
    A_raw("input_duplicate_member",
          json.dumps(WF)[:-1].encode("utf-8") + b', "root": "' + R.encode() + b'"}',
          "HARNESS2: the input reader refuses an object that repeats a member name; no label.")
    A_raw("input_float_member", json.dumps(WF)[:-1].encode("utf-8") + b', "extra": 1.5}',
          "HARNESS2: the input reader admits only integers of sec 3.1's universe, in any member; "
          "no label.")
    A_raw("input_nan_constant", json.dumps(WF)[:-1].encode("utf-8") + b', "extra": NaN}',
          "HARNESS2: NaN is no value the input reader admits; no label.")
    A("pile_element_without_0x", dict(WF, pile=[WF["pile"][0][2:]] + WF["pile"][1:]),
      "HARNESS2: a pile element that does not begin with 0x; no label.", "NO_LABEL")
    A("pile_element_nonhex", dict(WF, pile=["0xzz" + WF["pile"][0][4:]] + WF["pile"][1:]),
      "HARNESS2: a pile element carrying a non-hex character; no label.", "NO_LABEL")
    A("root_mismatch_lone_foreign_genesis",
      inp(R, [gen2, mk("root", "succession", 1, eid(gen2), SUCB(A_("k2")))]),
      "sec 8.2: ROOT_MISMATCH is tested over every seq-0 ledger entry.", "BROKEN_CHAIN")

    A("basis_extra_top_member",
      dict(inp(R, CL), basis=dict(basis((chainobj(1),)), note="scan of 2031-04")),
      "sec 9.4: the basis is an object with exactly these members; sec 6.10 reaches no object "
      "of this section.", "NO_LABEL")
    A("basis_chains_object_extra_member",
      inp(R, CL, bas=basis((dict(chainobj(1), rpc="https://example"),))),
      "sec 9.4: an object bearing a member beyond its list is not a zikaron/2 basis.",
      "NO_LABEL")
    A("basis_two_windows_one_block_apart",
      inp(R, CL, bas=basis((chainobj(1, tb=100, senders=(R,)), chainobj(1, fb=102, tb=300, senders=(R,))))),
      "sec 9.4: ranges one block apart are not adjacent, so equal lists are a basis; the inside "
      "edge of the adjacency clause.", "COMPLETE")
    A("input_anchor_verdict_outside_set",
      inp(R, CL, [dict(anchor(CID[0], R), verdict="maybe")], bas=ROOTB),
      "sec 9.4: an anchor record whose verdict is outside the three values is not an audit input.",
      "NO_LABEL")
    A("input_anchor_sender_not_hex20",
      inp(R, CL, [dict(anchor(CID[0], R), sender="0xabc")], bas=ROOTB),
      "sec 9.4: a record any of whose seven members fails the form sec 9.2 names is not an audit input.",
      "NO_LABEL")
    A("input_root_uppercase",
      dict(inp(R, CL), root="0x" + R[2:].upper()),
      "sec 8 with sec 1: the root is hex20, lowercase; another spelling is not an audit input.",
      "NO_LABEL")
    A("input_pile_absent",
      {k: v for k, v in inp(R, CL).items() if k != "pile"},
      "sec 8: the audit has three inputs; one absent is no audit input.",
      "NO_LABEL")
    A("input_anchors_not_array",
      dict(inp(R, CL), anchors={}),
      "sec 8: the anchor set is a set of records.",
      "NO_LABEL")
    A("input_pile_odd_hex",
      dict(inp(R, CL), pile=inp(R, CL)["pile"] + ["0xabc"]),
      "sec 8: a pile element that is not whole bytes is no byte string.",
      "NO_LABEL")
    A("input_extra_member_ignored",
      dict(inp(R, CL), extra=[{"chainId": 1, "tx": HX32(7)}]),
      "sec 8 with HARNESS2: the audit has three inputs and the file four members; a fifth is not one of them, "
      "and nothing in this law refuses an input object for carrying it.",
      "COMPLETE")
    A("input_anchor_record_extra_member",
      inp(R, CL, [dict(anchor(CID[0], R), note="x")], bas=ROOTB),
      "sec 8: a record with a member beyond its seven is read by its seven; nothing in the law refuses it.",
      "COMPLETE")
    A("basis_missing_member",
      dict(inp(R, CL), basis={}),
      "sec 9.4: exactly the member chains; absent, the object is no basis.", "NO_LABEL")
    A("basis_registry_uppercase",
      inp(R, CL, bas=basis((chainobj(1, regs=("0x" + "A" * 40,)),))),
      "sec 9.4/sec 1: registries is an array of hex20.", "NO_LABEL")
    A("basis_fromblock_string",
      inp(R, CL, bas=basis((dict(chainobj(1), fromBlock="0"),))),
      "sec 9.4: fromBlock is an int.", "NO_LABEL")
    A("basis_empty_covers_no_chain", inp(R, CL, bas=basis()),
      "sec 8.6: under a basis that covers no chain a COMPLETE label says nothing about "
      "anchoring, which is why the basis travels with the label.", "COMPLETE")

    A("dup_anchor_records_agree",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900)), anchor(CID[0], R, tx=HX32(0x900))],
          bas=ROOTB),
      "sec 9.4: the anchor set holds at most one record per (chainId, blockNumber, tx, hash); "
      "two records that agree on every remaining member are one record.", "COMPLETE")
    A("dup_anchor_records_disagree_timestamp",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900), ts=1),
                  anchor(CID[0], R, tx=HX32(0x900), ts=2)], bas=ROOTB),
      "sec 9.4: two records agreeing on (chainId, blockNumber, tx, hash) and disagreeing on blockTimestamp are not a zikaron/2 audit input.",
      "NO_LABEL")
    A("dup_anchor_records_disagree_verdict",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900)),
                  anchor(CID[0], R, tx=HX32(0x900), verdict="VOID")], bas=ROOTB),
      "sec 9.4: disagreement on the verdict.", "NO_LABEL")
    A("dup_anchor_records_disagree_blocknumber",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900), bn=5),
                  anchor(CID[0], R, tx=HX32(0x900), bn=6)], bas=ROOTB),
      "sec 9.2/9.4: two records differing in blockNumber are two inclusions of one transaction "
      "hash, two records, and no disagreement.", "COMPLETE")
    A("dup_anchor_same_tx_different_hash",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900)), anchor(CID[1], R, tx=HX32(0x900))],
          bas=ROOTB),
      "sec 9.4: one transaction carrying two hashes is two records and is legal.", "COMPLETE")
    # ---------------- round 1: boundaries the first audit found unwitnessed ---
    A("basis_fromblock_equals_toblock",
      inp(R, CL, bas=basis((chainobj(1, fb=500, tb=500),))),
      "sec 9.4: fromBlock equal to toBlock is the inside edge of the range relation.", "COMPLETE")
    A("basis_fromblock_above_toblock",
      inp(R, CL, bas=basis((chainobj(1, fb=501, tb=500),))),
      "sec 9.4: fromBlock above toBlock; the object is not a zikaron/2 basis.", "NO_LABEL")
    A("basis_chains_unsorted",
      inp(R, CL, bas=basis((chainobj(8453), chainobj(1)), raw=True)),
      "sec 9.4: chains is ordered by (chainId, fromBlock) ascending; chain 8453 before chain 1 is no basis.", "NO_LABEL")
    A("basis_senders_unsorted",
      inp(R, CL, bas=basis((chainobj(1, senders=(HX20(2), HX20(1)), raw=True),))),
      "sec 9.4: senders is bytewise ascending.", "NO_LABEL")
    A("basis_senders_repeated",
      inp(R, CL, bas=basis((chainobj(1, senders=(HX20(1), HX20(1)), raw=True),))),
      "sec 9.4: senders carries no repeated element.", "NO_LABEL")
    A("basis_registries_repeated",
      inp(R, CL, bas=basis((chainobj(1, regs=(HX20(7), HX20(7)), raw=True),))),
      "sec 9.4: registries carries no repeated element.", "NO_LABEL")
    A("basis_arrays_sorted_two_chains",
      inp(R, CL, bas=basis((chainobj(8453, senders=(HX20(9), HX20(3))), chainobj(1, regs=(HX20(5), HX20(4)))))),
      "sec 9.4: the helper's ordering, two chains and two-element arrays each in order.", "COMPLETE")
    A("missing_two_transactions_one_chain",
      inp(R, CL, [anchor(H_UNSEEN, R, cid=1, bn=50, tx=HX32(0x502)),
                  anchor(H_UNSEEN, R, cid=1, bn=50, tx=HX32(0x501))], bas=ROOTB),
      "sec 9.2/8.6 item 4: two anchors of one hash in two transactions of one chain; one "
      "MISSING row whose transactions sort by (chainId, blockNumber, tx).", "GAPS")
    A("dup_anchor_records_disagree_sender",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900)), anchor(CID[0], A_("k2"), tx=HX32(0x900))],
          bas=ROOTB),
      "sec 9.4: two records agreeing on (chainId, blockNumber, tx, hash) and disagreeing on "
      "sender are no audit input.", "NO_LABEL")
    A("dup_anchor_records_differ_blocknumber_two_records",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900), bn=5), anchor(CID[0], R, tx=HX32(0x900), bn=6)],
          bas=ROOTB),
      "sec 9.2/9.4: one chain including one transaction hash in two blocks is two anchors of "
      "one hash, two records, and one ANCHORED row carrying both.", "COMPLETE")
    A("dup_anchor_records_agree_with_extra_member",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900)), dict(anchor(CID[0], R, tx=HX32(0x900)), note="x")],
          bas=ROOTB),
      "sec 9.4: two records that agree on the seven members are one record, whatever either "
      "carries beyond them.", "COMPLETE")
    A("input_anchor_timestamp_above_universe",
      inp(R, CL, [dict(anchor(CID[0], R), blockTimestamp=MAXINT + 1)], bas=ROOTB),
      "HARNESS2: a number at or above 2^53 anywhere in the file makes it unreadable, here in a record's blockTimestamp, so sec 9.2's form test is never reached.",
      "NO_LABEL")
    A("pile_empty_byte_string", inp(R, CL + [b""]),
      "sec 8.1/8.6: the empty byte string fails sec 4.3 as E_JSON and lands in MALFORMED with "
      "its token; nothing else changes.", "COMPLETE")
    A("malformed_pile_bytes_listed", inp(R, CL + [BADB, b"{}"]),
      "sec 8.6 item 10: every byte string that fails sec 4.3 is listed once with its token.",
      "COMPLETE")
    A("malformed_anchored_row_and_missing",
      inp(R, CL + [BADB], [anchor(eid(BADB), R)], bas=ROOTB),
      "sec 8.6: the anchor of retrieved bytes that fail sec 4.3 is MISSING, and the MALFORMED "
      "row makes the two situations distinguishable.", "GAPS")
    UNK, _ = build_chain([("root", "genesis", GENB), ("root", "dispatch", {"case": HX32(1), "note_md": "n"}),
                          ("root", "annotation", ANNB("after")), ("root", "pass/1", {})])
    A("unknown_types_listed", inp(R, UNK),
      "sec 6.9/8.6 item 9: every ledger entry of a type outside the eight is listed with its "
      "seq and type, sorted by (seq, entryType, entry_id).", "COMPLETE")
    A("unknown_types_sorted_by_type",
      inp(R, UNK + [mk("root", "aardvark", 1, eid(UNK[0]), {})]),
      "sec 8.6 item 9: two unknown types at one seq sort by entryType bytewise; the fork is "
      "hard all the same.", "BROKEN_CHAIN")
    A("unproven_duplicate_of_counted_hash",
      inp(R, CL, [anchor(CID[0], R, bn=10), anchor(CID[0], R, bn=20, tx=HX32(0x777), verdict="UNPROVEN")],
          bas=ROOTB),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is already counted moves no label; "
      "the row is still carried.", "COMPLETE")
    A("unproven_of_hash_in_have_not_counted",
      inp(R, CL, [anchor(CID[0], R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is in have and counted nowhere "
      "moves no label either; the entry is UNANCHORED and the reader sees why.", "COMPLETE")
    A("unproven_of_unseen_hash_moves_label",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is neither in have nor counted "
      "is the case the label answers.", "UNPROVEN")
    # sec 7.3/8.2: a retired key's succession at a fork moves nothing, whichever id is smaller
    HC, HCI = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(A_("k2"))),
                           ("k2", "grant", GRB(2))])
    s_hon = mk("k2", "succession", 3, HCI[2], SUCB(A_("k3")))
    e_hon = mk("k3", "grant", 4, eid(s_hon), GRB(4))

    def retired_fork(k):
        s_ret = mk("root", "succession", 3, HCI[2], SUCB(A_("k4"), kind="handover-%d" % k))
        return inp(R, HC + [s_hon, e_hon, s_ret]), eid(s_hon) < eid(s_ret)
    both_orders("fork_retired_key_succession", retired_fork,
                "sec 7.3/8.2: the retired root signs a competing succession at seq 3; it records "
                "AUTHORITY_MISMATCH and moves nothing, so the honest successor's entry at seq 4 "
                "records no finding, whichever succession has the smaller id.", "BROKEN_CHAIN")

    # ---------------- round two: DISCARDED, two windows, bytes in hand, numbers ------
    A("discarded_records_listed",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9")), anchor(eid(CL[1]), A_("k9"), bn=7)], bas=ROOTB),
      "sec 8.6 item 13: every anchor record the trim discarded is a DISCARDED row "
      "carrying its seven members, whatever its hash; no label turns on the list.", "COMPLETE")
    ER, ERI = build_chain([("root", "genesis", GENB), ("root", "annotation", ANNB("one")),
                           ("root", "succession", SUCB(A_("k2"))), ("k2", "grant", GRB(3)),
                           ("k2", "grant", GRB(4))])
    A("withheld_succession_erases_tail",
      inp(R, ER[:2], [anchor(ERI[3], A_("k2"), bn=30), anchor(ERI[4], A_("k2"), bn=31)],
          bas=basis((chainobj(1, senders=[R, A_("k2")]),))),
      "sec 8.1: the pile stops before the succession, so the successor is outside the lineage "
      "and its two anchors are discarded; the walk sees no gap and the label is COMPLETE, and "
      "the two DISCARDED rows under a sender no succession names are what a reader asks about.",
      "COMPLETE")
    A("unproven_of_excluded_bytes",
      inp(R, CL + FOR, [anchor(eid(FOR[0]), R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of a pile byte "
      "string held under a foreign signature moves no label; bytes in hand are in hand.",
      "COMPLETE")
    JUNK = b'{"junk":1}'
    A("unproven_of_malformed_bytes",
      inp(R, CL + [JUNK], [anchor(eid(JUNK), R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of a pile byte "
      "string that fails sec 4.3 moves no label either; the MALFORMED row names the bytes.",
      "COMPLETE")
    W1 = chainobj(1, 0, 100, senders=[R])
    W2 = chainobj(1, 200, 300, senders=[R])
    A("basis_two_windows_one_chain", inp(R, CL, bas=basis((W1, W2))),
      "sec 9.4: two chains objects of one chainId over two windows that do not overlap are "
      "ordinary; a reader whose node serves two windows declares two objects.", "COMPLETE")
    A("basis_two_windows_overlap",
      inp(R, CL, bas=basis((W1, chainobj(1, 100, 300, senders=[R])), raw=True)),
      "sec 9.4: two objects of one chainId whose ranges overlap are not a basis.", "NO_LABEL")
    A("basis_two_windows_touch_equal_lists",
      inp(R, CL, bas=basis((W1, chainobj(1, 101, 300, senders=[R])), raw=True)),
      "sec 9.4: two objects of one chainId whose ranges touch with equal registries and "
      "senders would be one object spelled twice, and are not a basis.", "NO_LABEL")
    A("basis_two_windows_touch_different_lists",
      inp(R, CL, bas=basis((W1, chainobj(1, 101, 300, senders=[R, A_("k2")])), raw=True)),
      "sec 9.4: two touching windows whose senders differ are two scans with one spelling, "
      "and a basis.", "COMPLETE")
    A("basis_two_windows_unsorted", inp(R, CL, bas=basis((W2, W1), raw=True)),
      "sec 9.4: chains is ordered by (chainId, fromBlock) ascending; two windows of one chain "
      "spelled in the other order are not a basis.", "NO_LABEL")
    A("input_ignored_member_negative_number", dict(inp(R, CL), extra=[{"n": -1}]),
      "HARNESS2: every number in the audit input file is an integer of sec 3.1's universe, in a "
      "member the reader ignores as anywhere else; -1 makes the file unreadable.", "NO_LABEL")
    A("input_ignored_member_number_above_universe", dict(inp(R, CL), extra=[{"n": 2 ** 53}]),
      "HARNESS2: 2^53 in an ignored member is outside sec 3.1's universe and the file is refused.",
      "NO_LABEL")
    A("input_ignored_member_lone_surrogate", dict(inp(R, CL), extra=["\ud800"]),
      "HARNESS2: a lone surrogate escape names no scalar value; the file is refused, in an "
      "ignored member as anywhere else.", "NO_LABEL")
    A("input_ignored_member_surrogate_pair", dict(inp(R, CL), extra=["\U0001f600"]),
      "HARNESS2: a surrogate pair is RFC 8259's spelling of one scalar value above U+FFFF and "
      "is read as that value; the report is the clean ledger's.", "COMPLETE")
    deep = 7
    for _ in range(126):
        deep = [deep]
    A("input_ignored_member_depth_128", dict(inp(R, CL), extra=[deep]),
      "HARNESS2: the file carries sec 3.5's depth bound; a container opening at depth 128 inside "
      "an ignored member is read, and the report is the clean ledger's.", "COMPLETE")
    A("input_ignored_member_depth_129", dict(inp(R, CL), extra=[[deep]]),
      "HARNESS2: a container opening at depth 129, the root counted as depth 1, makes the file "
      "unreadable, in an ignored member as anywhere else; no reader needs an unbounded stack.",
      "NO_LABEL")
    A_raw("input_ignored_member_leading_zero",
          json.dumps(dict(inp(R, CL), extra=[{"n": 7}])).replace('"n": 7', '"n": 07').encode(),
          "HARNESS2: a leading zero is not sec 3.2's spelling of an integer, in an ignored member "
          "as anywhere else; the file is refused.")
    A("input_anchor_record_missing_member",
      inp(R, CL, [dict((k, v) for k, v in anchor(eid(CL[1]), R).items() if k != "blockTimestamp")],
          bas=ROOTB),
      "sec 9.2/9.4: a record that lacks one of the seven members is not an anchor record, and "
      "the input has no label.", "NO_LABEL")

    # ---------------- round two: the corpus reader's unwitnessed sides --------------
    A("anchored_on_two_chains",
      inp(R, CL, [anchor(CID[0], R, cid=8453, bn=10, ts=1699000000, tx=HX32(0x101)),
                  anchor(CID[0], R, cid=1, bn=10, tx=HX32(0x101))],
          bas=basis((chainobj(1, senders=(R,)), chainobj(8453, senders=(R,))))),
      "sec 8.6 item 5 with sec 9.4: two chains carry one entry_id; the dedup key holds chainId, "
      "so records differing only there are two records, and the row's anchors sort by chainId "
      "first.", "COMPLETE")
    A("anchored_and_missing_together",
      inp(R, CL, [anchor(CID[0], R, bn=100), anchor(H_UNSEEN, R, bn=101)], bas=ROOTB),
      "sec 8.6 item 5: the partition on the face of the report, ANCHORED and MISSING both "
      "non-empty in one report.", "GAPS")
    A("void_of_ledger_entry_id",
      inp(R, CL, [anchor(CID[0], R, verdict="VOID")], bas=ROOTB),
      "sec 9.3/8.5: a VOID record takes no part in the reconciliation, so the entry it names "
      "is UNANCHORED and no MISSING row follows.", "COMPLETE")
    A("unproven_sender_outside_lineage",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9"), verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.1/8.6 item 13: a record whose sender is outside the lineage is discarded before "
      "the UNPROVEN list is built, so it moves no label; it is a DISCARDED row.", "COMPLETE")
    A("void_sender_outside_lineage",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9"), verdict="VOID")], bas=ROOTB),
      "sec 8.1/8.6 item 12: the VOID list holds records undiscarded by sec 8.1 alone.",
      "COMPLETE")
    A("unproven_of_counted_hash_not_in_have",
      inp(R, CL, [anchor(H_UNSEEN, R, bn=50, tx=HX32(0x501)),
                  anchor(H_UNSEEN, R, bn=60, tx=HX32(0x777), verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 item 15 rule 2, second escape: an UNPROVEN hash counted elsewhere and absent "
      "from the pile moves no label; the counted record's MISSING row gives GAPS.", "GAPS")
    A("unknown_types_same_type_one_seq",
      inp(R, UNK + [mk("root", "dispatch", 1, eid(UNK[0]), {"x": 1})]),
      "sec 8.6 item 9: two unknown entries of one type at one seq sort by entry_id bytewise; "
      "the fork is hard all the same.", "BROKEN_CHAIN")
    A("unknown_type_excluded_entry_not_listed",
      inp(R, UNK + [mk("k9", "dispatch", 4, eid(UNK[3]), {})]),
      "sec 8.6 item 9: the list is over ledger entries; an EXCLUDED entry of an unknown type "
      "is an EXCLUDED row and no UNKNOWN_TYPE row.", "COMPLETE")
    A("malformed_duplicate_collapses", inp(R, CL + [b"{}", b"{}"]),
      "sec 8.1/8.6 item 10: byte-identical copies collapse to one entry and to one MALFORMED "
      "row.", "COMPLETE")
    A("basis_registries_unsorted",
      inp(R, CL, bas=basis((chainobj(1, regs=(HX20(5), HX20(4)), raw=True),))),
      "sec 9.4: registries is bytewise ascending.", "NO_LABEL")
    A("pile_element_uppercase_hex",
      dict(inp(R, CL), pile=[hx(CL[0])[:2] + hx(CL[0])[2:].upper()] + [hx(b) for b in CL[1:]]),
      "HARNESS2: a pile element's digits carry no case rule; the report is byte-identical to "
      "the clean ledger's.", "COMPLETE")
    NS, NSI = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(A_("k2"))),
                           ("k2", "grant", GRB(2))])
    ns_ret = mk("root", "succession", 3, NSI[2], SUCB(A_("k3")))
    A("succession_at_k_none_in_office",
      inp(R, NS + [ns_ret, mk("k2", "grant", 4, eid(ns_ret), GRB(4))]),
      "sec 8.2: the only succession at seq 3 is signed by the retired root; it records one hard "
      "AUTHORITY_MISMATCH, auth is unchanged, and k2's entry at seq 4 records nothing.",
      "BROKEN_CHAIN")
    A("succession_at_k_none_in_office_successor_signs",
      inp(R, NS + [ns_ret, mk("k3", "grant", 4, eid(ns_ret), GRB(4))]),
      "sec 8.2: the same pile with seq 4 signed by the named successor; auth did not move, so "
      "that entry records a second hard AUTHORITY_MISMATCH.", "BROKEN_CHAIN")

    # ---------------- round three: the corpus reader's sides, the reviewers' inputs ----
    H_F00D = HX32(0xF00D)
    A("missing_two_hashes_sorted",
      inp(R, CL, [anchor(H_UNSEEN, R, bn=50, tx=HX32(0x501)),
                  anchor(H_F00D, R, bn=60, tx=HX32(0x601))], bas=ROOTB),
      "sec 8.6 item 4: two counted hashes not in have; the rows sort "
      "bytewise by hash.", "GAPS")
    A("missing_anchors_sorted_by_block",
      inp(R, CL, [anchor(H_UNSEEN, R, bn=60, tx=HX32(0x501)),
                  anchor(H_UNSEEN, R, bn=50, tx=HX32(0x777))], bas=ROOTB),
      "sec 8.6 item 4: the anchors of one MISSING row sort by blockNumber before tx.", "GAPS")
    A("anchored_anchors_sorted_by_tx",
      inp(R, CL, [anchor(CID[0], R, bn=50, tx=HX32(0x777)),
                  anchor(CID[0], R, bn=50, tx=HX32(0x501))], bas=ROOTB),
      "sec 8.6 item 5: two records of one entry_id in one block, so the tx component of the "
      "anchor ordering decides.", "COMPLETE")
    XA = mk("k9", "annotation", 1, HX32(0x11), ANNB("left"))
    XB = mk("k9", "annotation", 1, HX32(0x11), ANNB("right"))
    A("excluded_two_at_one_seq", inp(R, CL + [XA, XB]),
      "sec 8.6 item 7: two excluded entries at one seq sort by entry_id bytewise, and sec 8.4 "
      "records no equivocation, since neither is a ledger entry.", "COMPLETE")
    FIVE = lambda verdict, snd=R: [
        anchor(H_UNSEEN, snd, cid=8453, bn=50, ts=1699000000, tx=HX32(0x501), verdict=verdict),
        anchor(H_UNSEEN, snd, bn=60, tx=HX32(0x501), verdict=verdict),
        anchor(H_UNSEEN, snd, bn=50, tx=HX32(0x777), verdict=verdict),
        anchor(H_UNSEEN, snd, bn=50, tx=HX32(0x501), verdict=verdict),
        anchor(H_F00D, snd, bn=50, tx=HX32(0x501), verdict=verdict)]
    TWOCH = basis((chainobj(1, senders=(R,)), chainobj(8453, senders=(R,))))
    A("unproven_rows_sorted", inp(R, CL, FIVE("UNPROVEN"), bas=TWOCH),
      "sec 8.6 item 11: five UNPROVEN records differing first at each of hash, chainId, "
      "blockNumber and tx; the rows sort by the four in that order.", "UNPROVEN")
    A("void_rows_sorted", inp(R, CL, FIVE("VOID"), bas=TWOCH),
      "sec 8.6 item 12: the VOID list sorts as the UNPROVEN list does; no label turns on it.",
      "COMPLETE")
    K4, K9 = A_("k4"), A_("k9")
    A("discarded_rows_sorted",
      inp(R, CL, [anchor(H_UNSEEN, K9, bn=50, tx=HX32(0x502)),
                  anchor(H_UNSEEN, K4, cid=8453, bn=50, ts=1699000000, tx=HX32(0x501)),
                  anchor(H_UNSEEN, K4, bn=50, tx=HX32(0x777)),
                  anchor(H_UNSEEN, K4, bn=50, tx=HX32(0x501)),
                  anchor(H_F00D, K4, bn=50, tx=HX32(0x501))],
          bas=basis((chainobj(1, senders=(K4, K9)), chainobj(8453, senders=(K4,))))),
      "sec 8.6 item 13: five discarded records differing first at each of sender, chainId, tx "
      "and hash; the rows sort by (sender, chainId, blockNumber, tx, hash).", "COMPLETE")
    A("basis_chains_member_absent", inp(R, CL, bas={"notChains": []}),
      "sec 9.4: exactly the member chains; another member alone is no basis.", "NO_LABEL")
    A("basis_chains_not_array", inp(R, CL, bas={"chains": {}}),
      "sec 9.4: chains is an array of objects.", "NO_LABEL")
    A("basis_chains_object_member_absent",
      inp(R, CL, bas={"chains": [{"chainId": 1, "fromBlock": 0, "toBlock": 1000,
                                  "registries": []}]}),
      "sec 9.4: a chains object carries exactly its five members; senders absent.", "NO_LABEL")
    A("basis_toblock_string",
      inp(R, CL, bas={"chains": [{"chainId": 1, "fromBlock": 0, "toBlock": "1000",
                                  "registries": [], "senders": []}]}),
      "sec 9.4: toBlock is an int.", "NO_LABEL")
    A("basis_sender_not_hex20",
      inp(R, CL, bas={"chains": [{"chainId": 1, "fromBlock": 0, "toBlock": 1000,
                                  "registries": [], "senders": ["0xabc"]}]}),
      "sec 9.4 with sec 1: senders is an array of hex20.", "NO_LABEL")
    A("basis_two_windows_touch_different_registries",
      inp(R, CL, bas=basis((chainobj(1, fb=0, tb=100, regs=(HX20(4),)),
                            chainobj(1, fb=101, tb=300, regs=(HX20(4), HX20(5)))))),
      "sec 9.4: two adjacent windows whose registries differ are two scans with one spelling, "
      "and a basis.", "COMPLETE")
    A("anchor_reach_split_across_windows",
      inp(R, CL, [anchor(CID[0], R, bn=150)],
          bas=basis((chainobj(1, fb=0, tb=100, senders=(R,)),
                     chainobj(1, fb=101, tb=300, senders=(A_("k2"),))))),
      "sec 9.4 [C]: the reach test is over one chains object: the record's block lies in the "
      "second window and its sender in the first window's senders only, so no object reaches "
      "it; no label.", "NO_LABEL")
    A("input_anchor_record_missing_verdict",
      inp(R, CL, [{k: v for k, v in anchor(CID[0], R).items() if k != "verdict"}], bas=ROOTB),
      "sec 9.2/9.4: a record that lacks its verdict is no anchor record, a different fault from "
      "a verdict outside the three.", "NO_LABEL")
    A("input_anchor_hash_not_hex32",
      inp(R, CL, [dict(anchor(CID[0], R), hash="0x12")], bas=ROOTB),
      "sec 9.2 with sec 1: hash is hex32.", "NO_LABEL")
    A("input_anchor_chainid_string",
      inp(R, CL, [dict(anchor(CID[0], R), chainId="1")], bas=ROOTB),
      "sec 9.2 with sec 1: chainId is an int of sec 3.1.", "NO_LABEL")
    A_raw("input_byte_order_mark",
          b"\xef\xbb\xbf" + json.dumps(inp(R, CL)).encode("utf-8"),
          "HARNESS2: a byte order mark is not whitespace under RFC 8259's JSON-text production, "
          "so the file is unreadable, as it is for an entry.")
    A("input_basis_absent",
      {"root": R, "pile": [hx(b) for b in CL], "anchors": []},
      "HARNESS2: basis is a required member of the audit input file.", "NO_LABEL")
    A("input_pile_element_not_string",
      {"root": R, "pile": [1], "anchors": [], "basis": basis()},
      "HARNESS2: every pile element is a string of the transport encoding.", "NO_LABEL")
    A_raw("input_root_not_object", b"[1,2]",
          "HARNESS2: a file whose root value is not an object gives no label, and is no misuse.")
    # sec 7.2/8.2: the link is over ledger entries; a stranger's genesis satisfies none
    GX = mk("k9", "genesis", 0, None, {"statement_md": "a stranger's opening"})
    A("prev_names_excluded_entry",
      inp(R, [CL[0], GX, mk("root", "annotation", 1, eid(GX), ANNB("linked to the stranger"))]),
      "sec 7.2/8.2: prev names an entry the lineage left out; the link is over ledger entries, "
      "the stranger's genesis is EXCLUDED and satisfies no link, so PREV_MISMATCH is hard.",
      "BROKEN_CHAIN")
    # sec 7.4: the fixpoint reads a succession's author against the lineage, never against auth
    LF, LFI = build_chain([("root", "genesis", GENB)])
    lf_s1 = mk("k3", "succession", 1, LFI[0], SUCB(A_("k4")))
    lf_s2 = mk("root", "succession", 2, eid(lf_s1), SUCB(A_("k3")))
    lf_e3 = mk("k4", "annotation", 3, eid(lf_s2), ANNB("named by a key that never held office here"))
    A("lineage_by_succession_that_moved_nothing",
      inp(R, LF + [lf_s1, lf_s2, lf_e3]),
      "sec 7.4: k3 joins the lineage through the root's succession at 2, so k3's own succession "
      "at 1 extends the lineage to k4 though it moved nothing; all four are ledger entries, and "
      "the two authority faults are hard.", "BROKEN_CHAIN")
    # sec 6.9/8.4: an extra member is part of the bytes, so twins differing only in one equivocate
    xm_a = mk("root", "annotation", 1, CID[0], ANNB("n"))
    xm_b = mk("root", "annotation", 1, CID[0], dict(ANNB("n"), z=1))
    A("extra_member_twins_equivocate", inp(R, [CL[0], xm_a, xm_b]),
      "sec 6.9 with sec 2.1 and sec 8.4: two entries at one seq differing only in an extra body "
      "member are two entries with two entry_ids and equivocate; identity is total.",
      "BROKEN_CHAIN")

    # ---------------- round four: int sort keys against a decimal order, and per-member arms
    THREE = [anchor(H_UNSEEN, R, cid=2, bn=9, tx=HX32(0x901)),
             anchor(H_UNSEEN, R, cid=2, bn=10, tx=HX32(0x901)),
             anchor(H_UNSEEN, R, cid=10, bn=9, tx=HX32(0x901))]
    WIDE = basis((chainobj(2, senders=(R,)), chainobj(10, senders=(R,))))
    A("missing_anchor_sort_keys_numeric", inp(R, CL, THREE, bas=WIDE),
      "sec 8.6: the chainId and blockNumber of a MISSING row's anchors compare numerically, so "
      "chain 2 precedes chain 10 and block 9 precedes block 10.", "GAPS")
    A("anchored_anchor_sort_keys_numeric",
      inp(R, CL, [dict(a, hash=CID[0]) for a in THREE], bas=WIDE),
      "sec 8.6 item 5: the same two components inside an ANCHORED row.", "COMPLETE")
    A("unproven_sort_keys_numeric",
      inp(R, CL, [dict(a, verdict="UNPROVEN") for a in THREE], bas=WIDE),
      "sec 8.6 item 11: chainId and blockNumber compare numerically.", "UNPROVEN")
    A("void_sort_keys_numeric",
      inp(R, CL, [dict(a, verdict="VOID") for a in THREE], bas=WIDE),
      "sec 8.6 item 12: the same, and no label turns on the list.", "COMPLETE")
    A("discarded_sort_keys_numeric",
      inp(R, CL, [dict(a, sender=A_("k9")) for a in THREE],
          bas=basis((chainobj(2, senders=(A_("k9"),)), chainobj(10, senders=(A_("k9"),))))),
      "sec 8.6 item 13: chainId compares numerically, so chain 2 precedes chain 10.", "COMPLETE")
    LONG, LID = build_chain([("root", "genesis", GENB)] +
                            [("root", "annotation", ANNB("e%d" % i)) for i in range(1, 11)])
    A("findings_position_numeric",
      inp(R, LONG + [mk("root", "annotation", 9, LID[8], ANNB("twin at 9")),
                     mk("root", "annotation", 10, LID[9], ANNB("twin at 10"))]),
      "sec 8.6 item 3: two equivocations at positions 9 and 10; position compares numerically, "
      "so 9 sorts first.", "BROKEN_CHAIN")
    A("excluded_seq_numeric",
      inp(R, LONG + [mk("k9", "annotation", 9, LID[8], ANNB("foreign at 9")),
                     mk("k9", "annotation", 10, LID[9], ANNB("foreign at 10"))]),
      "sec 8.6 item 7: seq compares numerically, so the row at 9 precedes the row at 10.",
      "COMPLETE")
    A("unknown_type_seq_numeric",
      inp(R, LONG + [mk("root", "dispatch", 9, LID[8], {}), mk("root", "dispatch", 10, LID[9], {})]),
      "sec 8.6 item 9: seq compares numerically before entryType.", "BROKEN_CHAIN")
    A("input_root_uppercase_prefix", inp("0X" + R[2:], CL),
      "sec 9.4 with sec 1: an uppercase 0x prefix fails hex20 as an uppercase digit does.",
      "NO_LABEL")
    A("pile_element_uppercase_prefix",
      dict(inp(R, CL), pile=["0X" + hx(CL[0])[2:]] + [hx(b) for b in CL[1:]]),
      "HARNESS2: the transport prefix is the two bytes 0x, and the case rule reaches the digits "
      "alone.", "NO_LABEL")
    A("unknown_types_sorted_by_case",
      inp(R, CL[:2] + [mk("root", "Zulu", 1, CID[0], {}), mk("root", "aardvark", 1, CID[0], {})]),
      "sec 8.6 item 9: entryType compares bytewise, so Zulu precedes aardvark.", "BROKEN_CHAIN")
    A("basis_chains_element_not_object", inp(R, CL, bas={"chains": [1]}),
      "sec 9.4: chains holds objects.", "NO_LABEL")
    for arr in ("registries", "senders"):
        A("basis_%s_not_array" % arr,
          inp(R, CL, bas={"chains": [dict(chainobj(1), **{arr: "0x00"})]}),
          "sec 9.4: %s is an array of hex20." % arr, "NO_LABEL")
    for k in ("chainId", "fromBlock", "toBlock", "registries"):
        A("basis_chains_object_lacks_%s" % k.lower(),
          inp(R, CL, bas={"chains": [{kk: v for kk, v in chainobj(1).items() if kk != k}]}),
          "sec 9.4: a chains object carries exactly its five members; %s absent." % k, "NO_LABEL")
    A("basis_chainid_string",
      inp(R, CL, bas={"chains": [dict(chainobj(1), chainId="1")]}),
      "sec 9.4: chainId is an int.", "NO_LABEL")
    for k in ("chainId", "blockNumber", "tx", "sender", "hash"):
        A("input_anchor_record_lacks_%s" % k.lower(),
          inp(R, CL, [{kk: v for kk, v in anchor(CID[0], R).items() if kk != k}], bas=ROOTB),
          "sec 9.2/9.4: a record lacking one of the seven members is no anchor record; %s absent." % k,
          "NO_LABEL")
    A("input_anchor_blocknumber_string",
      inp(R, CL, [dict(anchor(CID[0], R), blockNumber="100")], bas=ROOTB),
      "sec 9.2 with sec 1: blockNumber is an int of sec 3.1.", "NO_LABEL")
    A("input_anchor_tx_not_hex32",
      inp(R, CL, [dict(anchor(CID[0], R), tx="0x12")], bas=ROOTB),
      "sec 9.2 with sec 1: tx is hex32.", "NO_LABEL")
    for k in ("root", "anchors"):
        A("input_%s_absent" % k, {kk: v for kk, v in inp(R, CL).items() if kk != k},
          "HARNESS2: %s is a required member of the audit input file." % k, "NO_LABEL")
    A("input_root_not_string", dict(inp(R, CL), root=1),
      "HARNESS2 with sec 9.4: root is hex20, and an int is outside its form.", "NO_LABEL")
    A("input_pile_not_array", dict(inp(R, CL), pile="0x00"),
      "HARNESS2: pile is an array of strings.", "NO_LABEL")

    # ---------------- round five: the basis's own int keys, the last arity arms
    A("basis_chains_int_order_numeric",
      inp(R, CL, bas=basis((chainobj(2, senders=(R,)), chainobj(10, senders=(R,))), raw=True)),
      "sec 9.4: chains is ordered by chainId numerically, so 2 then 10 is a basis.", "COMPLETE")
    A("basis_chains_int_order_decimal_string",
      inp(R, CL, bas=basis((chainobj(10, senders=(R,)), chainobj(2, senders=(R,))), raw=True)),
      "sec 9.4: 10 then 2 is the decimal-string order and not the numeric one; no label.",
      "NO_LABEL")
    A("basis_windows_fromblock_numeric",
      inp(R, CL, bas=basis((chainobj(1, fb=9, tb=9, senders=(R,)),
                            chainobj(1, fb=10, tb=20, senders=(R,))), raw=True)),
      "sec 9.4: two windows ordered by fromBlock numerically, 9 then 10, and adjacent only "
      "where 9 plus 1 is 10, which it is, their lists equal; no label.", "NO_LABEL")
    A("basis_windows_fromblock_numeric_apart",
      inp(R, CL, bas=basis((chainobj(1, fb=9, tb=9, senders=(R,)),
                            chainobj(1, fb=11, tb=20, senders=(R,))), raw=True)),
      "sec 9.4: 9 then 11 is numeric order and the windows are a block apart; a basis.",
      "COMPLETE")
    A("input_basis_not_object", dict(inp(R, CL), basis=[]),
      "sec 9.4: the basis is an object; an array is not a basis.", "NO_LABEL")
    A("input_anchors_element_not_object", dict(inp(R, CL), anchors=[1]),
      "HARNESS2 with sec 9.2: anchors holds objects.", "NO_LABEL")
    # ---------------- round 1, corpus third opinion: isolated witnesses ----------
    STR = mk("k9", "succession", 1, eid(CL[0]), SUCB(A_("k4")))
    A("lineage_stranger_succession_extends_nothing",
      inp(R, CL + [STR], [anchor(HX32(0xBEEF), A_("k4"))],
          bas=basis((chainobj(1, senders=(A_("k4"),)),))),
      "sec 7.4/8.1: the lineage is extended by the `to` of a succession whose author is already "
      "in the set; a stranger's succession extends nothing, so its `to` cannot put a hash into "
      "this ledger's reconciliation and the record is a DISCARDED row.", "COMPLETE")
    UT0 = mk("root", "genesis", 0, None, GENB)
    UT1 = mk("root", "zulu", 1, eid(UT0), {})
    UT2 = mk("root", "aardvark", 2, eid(UT1), {})
    A("unknown_type_seq_before_type", inp(R, [UT0, UT1, UT2]),
      "sec 8.6 item 9: the list sorts by seq before entryType, so zulu at 1 precedes aardvark "
      "at 2 though the types sort the other way.", "COMPLETE")
    A("input_ignored_member_max_int", dict(inp(R, CL), extra=[{"n": 2 ** 53 - 1}]),
      "HARNESS2: 2^53 - 1 is the largest integer sec 3.1 admits and is read, in an ignored member "
      "as anywhere else; the report is the clean ledger's.", "COMPLETE")

    # ---------------- round 2, corpus third opinion: isolated witnesses ----------
    K3 = A_("k3")
    B_RK3 = basis((chainobj(1, senders=(R, K3)),))
    A("anchored_row_excludes_discarded_record",
      inp(R, CL[:1], [anchor(eid(CL[0]), R, tx=HX32(0x701)), anchor(eid(CL[0]), K3, tx=HX32(0x702))],
          bas=B_RK3),
      "sec 8.6 items 5 and 14: two counted records of the genesis, one from the root and one "
      "from a stranger the basis scans; the ANCHORED row carries the root's record alone and "
      "the stranger's is a DISCARDED row.", "COMPLETE")
    A("missing_row_excludes_discarded_record",
      inp(R, CL[:1], [anchor(HX32(0xBEEF), R, tx=HX32(0x701)), anchor(HX32(0xBEEF), K3, tx=HX32(0x702))],
          bas=B_RK3),
      "sec 8.6 items 4 and 14: the same two records over a hash no pile byte string carries; "
      "the MISSING row carries the root's record alone, the stranger's is DISCARDED; GAPS.",
      "GAPS")
    A_raw("input_reader_utf8_inside_ignored_string",
          (json.dumps(inp(R, CL), sort_keys=True)[:-1] + ',"ignored":"').encode("utf-8")
          + b"\xc3\x28" + b'"}',
          "HARNESS2: the reader's first restriction, valid UTF-8 over the whole file, is the only "
          "rule the two bytes C3 28 inside an ignored member's string reach; the file is "
          "unreadable and gives no label.")
    G0 = mk("root", "genesis", 0, None, GENB)
    I0 = eid(G0)
    TW = [mk("root", "annotation", 1, I0, {"note_md": "twin %d" % i}) for i in range(4)]
    A("four_way_fork_findings_sort", inp(R, [G0] + TW),
      "sec 8.6 item 3: six EQUIVOCATIONs at one position sort by (entry_id, second entry_id), "
      "(A,B),(A,C),(A,D),(B,C),(B,D),(C,D), where sorting by the second id alone would put "
      "(B,C) before (A,D); BROKEN_CHAIN.", "BROKEN_CHAIN")

    A("unproven_hash_is_excluded_entry",
      inp(R, CL + [GX], [anchor(eid(GX), R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of an excluded "
      "entry in the pile is over bytes in hand (the pile, as in sec 8.1), so the label "
      "is not UNAVAILABLE.", "COMPLETE")
    A("unproven_hash_is_malformed_bytes",
      inp(R, CL + [b"{}"], [anchor(eid(b"{}"), R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of a byte string "
      "that fails sec 4.3 is over bytes in hand all the same; MALFORMED lists the bytes and "
      "the label is not UNPROVEN.", "COMPLETE")

    # ================ zikaron/2: retirement, timeliness, migration (sec 7.4, 6.6, 8.1)
    T = 1700000000
    K2, K3, K4, K9 = A_("k2"), A_("k3"), A_("k4"), A_("k9")
    TWO = basis((chainobj(1, senders=(R, K2, K3, K4, K9)), chainobj(8453, senders=(R, K2, K3, K4, K9))))
    RT, RTI = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1)),
                           ("root", "succession", SUCB(K2)), ("k2", "annotation", ANNB("k2 writes"))])
    BA = [anchor(RTI[0], R, bn=10, ts=T), anchor(RTI[1], R, bn=11, ts=T + 1),
          anchor(RTI[2], R, bn=12, ts=T + 2), anchor(RTI[3], K2, bn=13, ts=T + 3)]
    A("retire_clean_handoff", inp(R, RT, BA, bas=TWO),
      "sec 7.4/5.7: every entry of the ceding key anchored before the succession, the succession "
      "anchored at T+2, the successor's entry after; the root retires at T+2 and nothing it "
      "sent is later; COMPLETE.", "COMPLETE")
    A("retire_same_second_counts", inp(R, RT, BA + [anchor(H_UNSEEN, R, bn=14, ts=T + 2, tx=HX32(0x901))], bas=TWO),
      "sec 7.4: a key has retired at a record later than its retirement and, in the "
      "retirement's own second, at a record anchoring what it signed above its retiring seq; "
      "the root's record in that second anchors a hash whose bytes are not held, so the root "
      "has not retired at it, it counts, and its hash is MISSING.", "GAPS")
    A("retire_later_second_discarded", inp(R, RT, BA + [anchor(H_UNSEEN, R, bn=14, ts=T + 3, tx=HX32(0x901))], bas=TWO),
      "sec 7.4/8.1: one second after the retirement the root's record is discarded; a stolen "
      "old key puts no hash into the reconciliation and the row is DISCARDED.", "COMPLETE")
    late = mk("root", "annotation", 3, RTI[2], ANNB("the old key, after the seat moved on"))
    A("retired_fork_unanchored", inp(R, RT + [late], BA, bas=TWO),
      "sec 7.4/8.1: an entry the root signed at seq 3, a twin of the successor's, reachable from "
      "nothing any record at or before T+2 anchors; not timely, RETIRED, no equivocation.",
      "COMPLETE")
    A("retired_fork_anchored_late_by_old_key",
      inp(R, RT + [late], BA + [anchor(eid(late), R, bn=15, ts=T + 5)], bas=TWO),
      "sec 7.4/8.1: the old key anchors its own late twin after retiring; the entry is RETIRED "
      "and the record DISCARDED, and neither MISSING nor a finding follows.", "COMPLETE")
    A("retired_orphan_reanchored_by_successor",
      inp(R, RT + [late], BA + [anchor(eid(late), K2, bn=15, ts=T + 5)], bas=TWO),
      "sec 5.7/8.5: the successor anchors a retired key's entry that no record bounded before "
      "the retirement; the entry stays RETIRED and the successor's counted anchor is MISSING "
      "for good.", "GAPS")
    lsucc = mk("root", "succession", 3, RTI[2], SUCB(K9))
    stranger = mk("k9", "annotation", 4, eid(lsucc), ANNB("named by a retired key"))
    A("retired_key_late_succession_extends_nothing",
      inp(R, RT + [lsucc, stranger], BA + [anchor(eid(lsucc), R, bn=15, ts=T + 5),
                                            anchor(eid(stranger), K9, bn=16, ts=T + 6)], bas=TWO),
      "sec 7.4: a succession the retired root signed after handing off is not timely and "
      "extends nothing; the stranger it names is outside the lineage, its entry EXCLUDED, and "
      "both records DISCARDED.", "COMPLETE")
    A("retire_cross_chain_later_second",
      inp(R, RT, BA + [anchor(H_UNSEEN, R, cid=8453, bn=5, ts=T + 3, tx=HX32(0x903))], bas=TWO),
      "sec 7.4: retirement is on whichever chain the record sits; the root's record on another "
      "chain one second after its retirement on chain 1 is discarded.", "COMPLETE")
    A("retire_cross_chain_same_second",
      inp(R, RT, BA + [anchor(H_UNSEEN, R, cid=8453, bn=5, ts=T + 2, tx=HX32(0x903))], bas=TWO),
      "sec 7.4: the same hash-only record in the retirement's own second on another chain: the "
      "root has not retired at it, it counts; MISSING.", "GAPS")
    A("retire_smallest_time_across_chains",
      inp(R, RT, BA[:2] + [anchor(RTI[2], R, bn=12, ts=T + 10), anchor(RTI[2], R, cid=8453, bn=3, ts=T + 2, tx=HX32(0x904)),
                           anchor(RTI[3], K2, bn=13, ts=T + 11),
                           anchor(H_UNSEEN, R, bn=14, ts=T + 5, tx=HX32(0x905))], bas=TWO),
      "sec 7.4: the retirement is the smallest blockTimestamp over every chain; the chain-8453 "
      "record at T+2 retires the root, so its chain-1 record at T+5 is discarded though the "
      "succession's chain-1 record is later still.", "COMPLETE")
    A("retire_set_by_stranger_record",
      inp(R, RT, BA[:2] + [anchor(RTI[2], K9, bn=12, ts=T + 2), anchor(RTI[3], K2, bn=13, ts=T + 3),
                           anchor(H_UNSEEN, R, bn=14, ts=T + 3, tx=HX32(0x906))], bas=TWO),
      "sec 7.4: the retirement counts records of any sender; a stranger's record anchoring the "
      "published succession sets it, that record is itself discarded (sender outside the "
      "lineage), and the root's later record is discarded too; the succession is timely by "
      "the stranger's record.", "COMPLETE")
    A("retire_set_by_unproven_record",
      inp(R, RT, BA[:2] + [anchor(RTI[2], R, bn=12, ts=T + 2, verdict="UNPROVEN"), anchor(RTI[3], K2, bn=13, ts=T + 3),
                           anchor(H_UNSEEN, R, bn=14, ts=T + 3, tx=HX32(0x906))], bas=TWO),
      "sec 7.4: the retirement counts records of any verdict; an UNPROVEN record anchoring the "
      "succession retires the root, the hash is in the pile so no label turns on the record, "
      "and the root's later record is discarded.", "COMPLETE")
    A("retire_set_by_void_record",
      inp(R, RT, BA[:2] + [anchor(RTI[2], R, bn=12, ts=T + 2, verdict="VOID"), anchor(RTI[3], K2, bn=13, ts=T + 3),
                           anchor(H_UNSEEN, R, bn=14, ts=T + 3, tx=HX32(0x906))], bas=TWO),
      "sec 7.4: a VOID record retires the signer like any other.", "COMPLETE")
    A("no_retirement_without_record",
      inp(R, RT, BA[:2] + [anchor(RTI[3], K2, bn=13, ts=T + 3), anchor(H_UNSEEN, R, bn=14, ts=T + 9, tx=HX32(0x906))], bas=TWO),
      "sec 7.4: no record anchors the succession, so the root has no retirement and is read as "
      "zikaron/1 reads every key; its late record counts and its hash is MISSING.", "GAPS")
    A("timely_offline_successor_entries", inp(R, RT, [], bas=TWO),
      "sec 7.4/8.1: an empty anchor set retires nobody; every entry is timely, the successor's "
      "entries are ledger entries, and the record reads as zikaron/1 would.", "COMPLETE")
    # the retirement's own second (revision of 2026-10-30): the key has retired at a record
    # of that second iff it anchors what the key signed above its retiring seq, and what the
    # key signed above its retiring seq is made timely by no record of that second
    A("retire_same_second_late_twin_by_old_key",
      inp(R, RT + [late], BA + [anchor(eid(late), R, bn=12, ts=T + 2, tx=HX32(0x911))], bas=TWO),
      "sec 7.4/8.1: in the second of its handing off the root anchors its own twin at seq 3, "
      "above its retiring seq 2; the record is one at which the root has retired, DISCARDED, "
      "the twin RETIRED, no fork.", "COMPLETE")
    A("retire_same_second_late_twin_by_successor",
      inp(R, RT + [late], BA + [anchor(eid(late), K2, bn=12, ts=T + 2, tx=HX32(0x912))], bas=TWO),
      "sec 7.4/8.5: the successor anchors the root's seq-3 twin in the retirement's own second; "
      "the twin sits above the root's retiring seq, which no record of that second makes timely, "
      "RETIRED; the record is the successor's, which has not retired at it, so it counts and its "
      "hash is MISSING.", "GAPS")
    A("retire_same_second_late_twin_by_stranger",
      inp(R, RT + [late], BA + [anchor(eid(late), K9, bn=12, ts=T + 2, tx=HX32(0x913))], bas=TWO),
      "sec 7.4/8.1: a stranger's record of that second anchoring the root's seq-3 twin: the twin "
      "sits above the retiring seq, which no record of that second makes timely, RETIRED; the "
      "record is DISCARDED, its sender outside the lineage.", "COMPLETE")
    A("retire_same_second_reanchor_before_handoff",
      inp(R, RT, BA + [anchor(RTI[1], R, bn=12, ts=T + 2, tx=HX32(0x914))], bas=TWO),
      "sec 7.4: the root re-anchors its history at seq 1, at or before its retiring seq, in the "
      "second of its handing off; the record counts and the entry is anchored twice.", "COMPLETE")
    A("retire_same_second_late_succession_by_old_key",
      inp(R, RT + [lsucc, stranger], BA + [anchor(eid(lsucc), R, bn=12, ts=T + 2, tx=HX32(0x915)),
                                            anchor(eid(stranger), K9, bn=16, ts=T + 6)], bas=TWO),
      "sec 7.4: the root's succession at seq 3, anchored by the root in the second of its handing "
      "off, sits above the retiring seq, which no record of that second makes timely: it extends "
      "nothing and is RETIRED; the stranger is outside the lineage, its entry EXCLUDED, both "
      "records DISCARDED.", "COMPLETE")
    A("retire_same_second_late_succession_by_stranger_record",
      inp(R, RT + [lsucc, stranger], BA + [anchor(eid(lsucc), K9, bn=12, ts=T + 2, tx=HX32(0x916))], bas=TWO),
      "sec 7.4: the same late succession anchored in that second by the stranger it names: the "
      "succession, above the retiring seq, is made timely by no record of that second, so it "
      "admits nobody; the record is DISCARDED, its sender outside the lineage, and the stranger's "
      "entry EXCLUDED.", "COMPLETE")
    A("retire_same_second_hash_only_beside_late_twin",
      inp(R, RT + [late], BA + [anchor(H_UNSEEN, R, bn=12, ts=T + 2, tx=HX32(0x917)),
                                anchor(eid(late), R, bn=12, ts=T + 2, tx=HX32(0x918))], bas=TWO),
      "sec 7.4/8.5: two records of the root in the retirement's own second: the one anchoring a "
      "hash whose bytes are not held anchors nothing the root signed above its retiring seq and "
      "counts, MISSING; the one anchoring the seq-3 twin is DISCARDED.", "GAPS")
    A("retire_cross_chain_same_second_late_twin",
      inp(R, RT + [late], BA + [anchor(eid(late), R, cid=8453, bn=5, ts=T + 2, tx=HX32(0x919))], bas=TWO),
      "sec 7.4: the root's record on another chain, block time equal to its retirement, anchoring "
      "its seq-3 twin: retired at it on whichever chain, DISCARDED, the twin RETIRED.", "COMPLETE")
    hi_s = mk("root", "succession", 3, RTI[2], SUCB(K3))
    k3e = mk("k3", "annotation", 4, eid(hi_s), ANNB("k3 after the higher succession"))
    A("retire_two_handoffs_same_second_lower_sets_retiring_seq",
      inp(R, RT + [hi_s, k3e], BA + [anchor(eid(hi_s), R, bn=12, ts=T + 2, tx=HX32(0x91A)),
                                     anchor(eid(k3e), K3, bn=14, ts=T + 4)], bas=TWO),
      "sec 7.4: successions of the root at seq 2 and 3, both anchored in one second; the "
      "retiring seq is 2, and the seq-3 one sits above it, which no record of that second makes "
      "timely, so "
      "that succession is not timely and admits nobody: k3 is outside the lineage, its entry "
      "EXCLUDED, its record and the root's DISCARDED.", "COMPLETE")
    tw2 = mk("root", "succession", 2, RTI[1], SUCB(K3, kind="handover-twin"))
    A("retire_two_successions_same_position_same_second",
      inp(R, RT + [tw2], BA + [anchor(eid(tw2), R, bn=12, ts=T + 2, tx=HX32(0x91B))], bas=TWO),
      "sec 7.4/8.4: two successions of the root at seq 2, both anchored in the retirement's own "
      "second at the retiring seq itself; both timely, both ledger entries, a hard fork.",
      "BROKEN_CHAIN")
    # the conditions of the same-second clause and of the retiring seq that no case above reaches
    badl = sub(late, b"the old key", b"the OLD key")
    A("retire_same_second_malformed_late_bytes_by_old_key",
      inp(R, RT + [badl], BA + [anchor(eid(badl), R, bn=12, ts=T + 2, tx=HX32(0x9A1))], bas=TWO),
      "sec 7.4: in the second of its handing off the root anchors bytes that fail sec 4.3 though "
      "they spell its author and seq 3; a record over bytes that fail sec 4.3 is one at which the "
      "root has not retired: it counts, its hash is MISSING and the bytes MALFORMED.", "GAPS")
    A("retire_same_second_late_twin_void_record_by_old_key",
      inp(R, RT + [late], BA + [anchor(eid(late), R, bn=12, ts=T + 2, tx=HX32(0x9A2), verdict="VOID")], bas=TWO),
      "sec 7.4/8.1: the root's VOID record of that second anchoring its seq-3 twin is one at which "
      "it has retired, whatever the verdict: DISCARDED, no VOID row, the twin RETIRED.", "COMPLETE")
    A("retire_same_second_late_twin_unproven_record_by_old_key",
      inp(R, RT + [late], BA + [anchor(eid(late), R, bn=12, ts=T + 2, tx=HX32(0x9A3), verdict="UNPROVEN")], bas=TWO),
      "sec 7.4/8.1: the same record UNPROVEN: DISCARDED, no UNPROVEN row, the twin RETIRED.", "COMPLETE")
    BA_ns = [BA[0], BA[1], BA[3]]
    A("retire_two_handoffs_lower_anchored_by_void_record",
      inp(R, RT + [hi_s, k3e], BA_ns + [anchor(RTI[2], R, bn=12, ts=T + 2, verdict="VOID"),
                                        anchor(eid(hi_s), R, bn=12, ts=T + 2, tx=HX32(0x9A4)),
                                        anchor(eid(k3e), K3, bn=14, ts=T + 4)], bas=TWO),
      "sec 7.4: the retiring seq is taken over records of any verdict; the seq-2 succession's only "
      "record of that second is VOID, the seq-3 one's counted; the retiring seq is 2, the seq-3 "
      "succession admits nobody; one VOID row.", "COMPLETE")
    A("retire_two_handoffs_lower_anchored_by_stranger",
      inp(R, RT + [hi_s, k3e], BA_ns + [anchor(RTI[2], K9, bn=12, ts=T + 2),
                                        anchor(eid(hi_s), R, bn=12, ts=T + 2, tx=HX32(0x9A5)),
                                        anchor(eid(k3e), K3, bn=14, ts=T + 4)], bas=TWO),
      "sec 7.4: the retiring seq is taken over records of any sender; the seq-2 succession is "
      "anchored in that second by a stranger alone; the retiring seq is 2.", "COMPLETE")
    A("retire_two_handoffs_lower_anchored_on_other_chain",
      inp(R, RT + [hi_s, k3e], BA_ns + [anchor(RTI[2], R, cid=8453, bn=5, ts=T + 2),
                                        anchor(eid(hi_s), R, bn=12, ts=T + 2, tx=HX32(0x9A6)),
                                        anchor(eid(k3e), K3, bn=14, ts=T + 4)], bas=TWO),
      "sec 7.4: the retiring seq is taken over every chain; the seq-2 succession is anchored in "
      "that second on chain 8453 alone, the seq-3 one on chain 1; the retiring seq is 2.", "COMPLETE")
    y2 = mk("root", "annotation", 2, RTI[1], ANNB("the old key's own seq-2 twin"))
    l3 = mk("root", "annotation", 3, eid(y2), ANNB("the old key above its retiring seq"))
    A("retire_same_second_low_twin_reached_only_through_late_twin",
      inp(R, RT + [y2, l3], BA + [anchor(eid(l3), R, bn=12, ts=T + 2, tx=HX32(0x9A7))], bas=TWO),
      "sec 7.4: the root's seq-2 twin of its succession is reachable only from its seq-3 entry, "
      "which the root anchors in the second of its handing off; the record is one at which the "
      "root has retired, DISCARDED, yet timeliness reads that second by position: the seq-2 twin "
      "sits at the retiring seq and is timely, a fork with the succession, while the seq-3 entry "
      "above it stays RETIRED.", "BROKEN_CHAIN")
    wrap = mk("k4", "annotation", 4, eid(late), ANNB("a fresh key wraps the late twin"))
    A("retire_same_second_wrapper_sent_by_old_key",
      inp(R, RT + [late, wrap], BA + [anchor(eid(wrap), R, bn=12, ts=T + 2, tx=HX32(0x9A9))], bas=TWO),
      "sec 7.4/8.5: in the second of its handing off the root's record anchors a fresh key's entry "
      "whose prev is the root's seq-3 twin; the record anchors what another key signed, so the "
      "root has not retired at it and it counts, while the twin, above the retiring seq, is made "
      "timely by no record of that second: RETIRED, no fork; the fresh key's entry EXCLUDED and "
      "the record's hash MISSING.", "GAPS")
    # a succession with no direct anchor, covered by a later entry's record, while its signer has a retirement
    CV, CVI = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(K2)),
                           ("k2", "annotation", ANNB("k2")), ("k2", "succession", SUCB(K3)),
                           ("k3", "annotation", ANNB("k3"))])
    twin_s = mk("k2", "succession", 3, CVI[2], SUCB(K4, kind="handover-twin"))
    A("timely_succession_covered_by_later_anchor",
      inp(R, CV + [twin_s], [anchor(CVI[0], R, bn=10, ts=T), anchor(CVI[1], R, bn=11, ts=T + 1),
                             anchor(CVI[4], K3, bn=14, ts=T + 4), anchor(eid(twin_s), K2, bn=20, ts=T + 10)], bas=TWO),
      "sec 7.4: k2's retirement is T+10, set by its twin succession's record; its succession to "
      "k3 has no record of its own and is timely through k3's entry anchored at T+4, so k3 is "
      "in the lineage and its entry is a ledger entry; the twin at seq 3 is timely too and the "
      "fork is hard.", "BROKEN_CHAIN")
    fork_b = mk("root", "history", 1, RTI[0], HISB(7))
    A("fork_anchored_by_third_party_before_retirement",
      inp(R, RT + [fork_b], BA + [anchor(eid(fork_b), K9, bn=11, ts=T + 1, tx=HX32(0x907))], bas=TWO),
      "sec 7.4/8.4: a fork the root made before handing off, whose other side only a stranger "
      "anchored, at T+1, before the retirement; timeliness counts any sender, so the fork "
      "stays in the ledger and a handing off clears nothing anchored before it.",
      "BROKEN_CHAIN")
    A("fork_anchored_by_third_party_after_retirement",
      inp(R, RT + [fork_b], BA + [anchor(eid(fork_b), K9, bn=15, ts=T + 5, tx=HX32(0x907))], bas=TWO),
      "sec 7.4: the same twin anchored by the stranger only after the retirement is reachable "
      "from nothing any record at or before T+2 anchors; RETIRED, no fork, and the stranger's "
      "record DISCARDED.", "COMPLETE")
    A("retired_entry_reachable_through_foreign_author",
      inp(R, RT + [fork_b, mk("k9", "annotation", 2, eid(fork_b), ANNB("a stranger builds on the twin"))],
          BA + [anchor(HX32(0xABC), K9, bn=11, ts=T + 1, tx=HX32(0x908))], bas=TWO),
      "sec 7.4: reachability runs through byte strings of any author, but the stranger's entry "
      "is anchored by no record, so the twin is bounded by nothing and stays RETIRED; the "
      "stranger's entry is EXCLUDED.", "COMPLETE")
    # migration (sec 6.6, 7.3, 8.2)
    MG, MGI = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1)),
                           ("root", "migration", MIGB())])
    A("migration_at_last_position", inp(R, MG, [anchor(h, R, bn=10 + i, ts=T + i) for i, h in enumerate(MGI)], bas=TWO),
      "sec 6.6: a migration at the ledger's last position closes it; nothing follows and the "
      "label is COMPLETE.", "COMPLETE")
    A("migration_closes_the_walk",
      inp(R, MG + [mk("root", "annotation", 3, MGI[2], ANNB("after the close"))]),
      "sec 7.3/8.2: after a migration the key in office signed, no key holds authority at any "
      "later position; the root's own entry at seq 3 records a hard AUTHORITY_MISMATCH.",
      "BROKEN_CHAIN")
    A("migration_closes_against_later_succession",
      inp(R, MG + [mk("root", "succession", 3, MGI[2], SUCB(K2)), mk("k2", "annotation", 4, HX32(0x44), ANNB("k2"))]),
      "sec 8.2: a succession after the close records AUTHORITY_MISMATCH and moves nothing; the "
      "named key's entry records one too, both hard.", "BROKEN_CHAIN")
    A("migration_out_of_office",
      inp(R, RT + [mk("root", "migration", 4, RTI[3], MIGB())]),
      "sec 8.2: a migration signed by a key that is not auth at its position is an authority "
      "fault and closes nothing.", "BROKEN_CHAIN")
    A("migration_by_stranger_excluded",
      inp(R, MG[:2] + [mk("k9", "migration", 2, MGI[1], MIGB())]),
      "sec 8.1: a stranger's migration is EXCLUDED like any entry of its author.", "COMPLETE")
    ms = mk("root", "succession", 2, MGI[1], SUCB(K2))
    A("migration_beside_succession",
      inp(R, MG + [ms, mk("k2", "annotation", 3, eid(ms), ANNB("k2 after"))]),
      "sec 8.2: a migration and a succession by the key in office share position 2; the "
      "migration closes the ledger whatever successions share the position, so k2's entry "
      "at 3 is an authority fault beside the fork.", "BROKEN_CHAIN")
    A("migration_retires_signer",
      inp(R, MG, [anchor(MGI[0], R, bn=10, ts=T), anchor(MGI[1], R, bn=11, ts=T + 1), anchor(MGI[2], R, bn=12, ts=T + 2),
                  anchor(H_UNSEEN, R, bn=13, ts=T + 3, tx=HX32(0x910))], bas=TWO),
      "sec 6.6/7.4: the key that signed a migration retires at the earliest record anchoring it; "
      "a hash it anchors a second later, a new-law anchor say, stands in DISCARDED and in no "
      "MISSING row.", "COMPLETE")
    A("migration_new_law_anchor_same_second",
      inp(R, MG, [anchor(MGI[0], R, bn=10, ts=T), anchor(MGI[1], R, bn=11, ts=T + 1), anchor(MGI[2], R, bn=12, ts=T + 2),
                  anchor(H_UNSEEN, R, bn=13, ts=T + 2, tx=HX32(0x910))], bas=TWO),
      "sec 12.3/5.7: a new-law hash anchored in the migration's own second counts and is "
      "MISSING; the seat anchors under the new law only at a later block time.", "GAPS")
    aft = mk("root", "annotation", 3, MGI[2], ANNB("after the close, in its second"))
    A("migration_same_second_entry_after_close_by_old_key",
      inp(R, MG + [aft], [anchor(MGI[0], R, bn=10, ts=T), anchor(MGI[1], R, bn=11, ts=T + 1), anchor(MGI[2], R, bn=12, ts=T + 2),
                          anchor(eid(aft), R, bn=12, ts=T + 2, tx=HX32(0x91C))], bas=TWO),
      "sec 7.4/6.6: the migration retires its signer at T+2; the root's entry at seq 3, above its "
      "retiring seq, anchored by the root in that second, is RETIRED and its record DISCARDED; "
      "the closed ledger reads as it closed.", "COMPLETE")
    mg5 = mk("root", "migration", 5, HX32(0x55), MIGB())
    A("migration_across_gap",
      inp(R, [MG[0], mg5, mk("root", "annotation", 6, eid(mg5), ANNB("after"))]),
      "sec 8.2: a migration after a gap closes the walk with certain false; the entry after it "
      "records a soft AUTHORITY_MISMATCH and the label is GAPS.", "GAPS")
    A("add_chain_anchors_succession_first",
      inp(R, RT, BA + [anchor(RTI[2], K2, cid=8453, bn=50, ts=T + 50, tx=HX32(0x920)),
                       anchor(RTI[3], K2, cid=8453, bn=60, ts=T + 60, tx=HX32(0x921))], bas=TWO),
      "sec 5.7: a seat that adds a chain first anchors every succession on it from the key in "
      "office; the root's retirement stays T+2 (the smallest) and the new chain's records "
      "count.", "COMPLETE")
    A("add_chain_old_key_stray",
      inp(R, RT, BA + [anchor(RTI[2], K2, cid=8453, bn=50, ts=T + 50, tx=HX32(0x920)),
                       anchor(H_UNSEEN, R, cid=8453, bn=55, ts=T + 55, tx=HX32(0x922))], bas=TWO),
      "sec 7.4: the root's record on the added chain is after its retirement on chain 1; "
      "discarded.", "COMPLETE")
    A("retraction_is_a_known_type",
      inp(R, RT[:2] + [mk("root", "retraction", 2, RTI[1], RTB(RTI[1]))]),
      "sec 8.6 item 9: retraction is one of the eight and earns no UNKNOWN_TYPE row; its "
      "subject is reading law.", "COMPLETE")
    A("migration_is_a_known_type", inp(R, MG),
      "sec 8.6 item 9: migration is one of the eight and earns no UNKNOWN_TYPE row.", "COMPLETE")
    A("retired_rows_sorted",
      inp(R, RT + [late, mk("root", "annotation", 3, RTI[2], ANNB("another late twin"))], BA, bas=TWO),
      "sec 8.6 item 8: two RETIRED rows at one seq sort by entry_id bytewise.", "COMPLETE")

    # ================ continuing a zikaron/1 ledger (sec 12.6)
    def mkv1(key, etype, seq, prev, body):
        return make_entry_v1(K(key), etype, seq, prev, body)

    V1 = [mkv1("root", "genesis", 0, None, GENB)]
    V1.append(mkv1("root", "history", 1, eid(V1[0]), HISB(1)))
    V1.append(mkv1("root", "succession", 2, eid(V1[1]), SUCB(R, kind="migration")))
    V1I = [eid(b) for b in V1]
    g2 = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": V1I[2]}})
    a2 = mk("root", "annotation", 1, eid(g2), ANNB("under the second law"))
    V2 = [g2, a2]
    CA = [anchor(V1I[0], R, bn=10, ts=T), anchor(V1I[1], R, bn=11, ts=T + 1), anchor(V1I[2], R, bn=12, ts=T + 2),
          anchor(eid(g2), R, bn=13, ts=T + 3), anchor(eid(a2), R, bn=14, ts=T + 4)]
    A("continue_clean", inp(R, V1 + V2, CA, bas=TWO),
      "sec 12.6: a zikaron/1 ledger closed by a self-succession of kind migration, anchored at "
      "T+2, and a genesis of this law naming it, anchored at T+3; the old segment reads "
      "COMPLETE under zikaron/1, the new segment COMPLETE under this law.", "COMPLETE")
    A("continue_cut_same_second",
      inp(R, V1 + V2, CA[:3] + [anchor(eid(g2), R, bn=13, ts=T + 2), anchor(eid(a2), R, bn=14, ts=T + 4)], bas=TWO),
      "sec 12.6/5.7: the genesis anchored in the cut's own second is a byte string of the old "
      "segment, where zikaron/1 lists it MALFORMED and its anchor MISSING; GAPS from the old "
      "report.", "GAPS")
    A("continue_old_key_v1_entry_after_cut",
      inp(R, V1 + V2 + [mkv1("root", "annotation", 3, V1I[2], ANNB("late v1"))],
          CA + [anchor(eid(mkv1("root", "annotation", 3, V1I[2], ANNB("late v1"))), R, bn=15, ts=T + 5)], bas=TWO),
      "sec 12.6/12.4: a zikaron/1 entry the key signs after the cut is a byte string of the new "
      "segment, MALFORMED there, and its anchor MISSING.", "GAPS")
    A("continue_old_key_v1_entry_after_cut_unanchored",
      inp(R, V1 + V2 + [mkv1("root", "annotation", 3, V1I[2], ANNB("late v1"))], CA, bas=TWO),
      "sec 12.6: the same entry unanchored is MALFORMED in the new segment and nothing more.",
      "COMPLETE")
    v1twin = mkv1("root", "history", 1, V1I[0], HISB(9))
    A("continue_v1_fork_anchored_after_cut",
      inp(R, V1 + V2 + [v1twin], CA + [anchor(eid(v1twin), R, bn=15, ts=T + 5)], bas=TWO),
      "sec 12.6: a zikaron/1 twin anchored after the cut is no byte string of the old segment, "
      "which reads whole; in the new segment it is MALFORMED and its anchor MISSING.", "GAPS")
    A("continue_v1_fork_anchored_before_cut",
      inp(R, V1 + V2 + [v1twin], CA + [anchor(eid(v1twin), R, bn=11, ts=T + 1, tx=HX32(0x930))], bas=TWO),
      "sec 12.6: the twin anchored before the cut is a byte string of the old segment, where "
      "zikaron/1 records the fork; BROKEN_CHAIN from the old report.", "BROKEN_CHAIN")
    A("continue_v1_fork_unanchored",
      inp(R, V1 + V2 + [v1twin], CA, bas=TWO),
      "sec 12.6: a twin reachable from the closing succession by no prev and anchored by no "
      "record is a byte string of the new segment alone, MALFORMED there.", "COMPLETE")
    JUNK2 = b'{"junk":2}'
    A("continue_junk_anchored_before_cut",
      inp(R, V1 + V2 + [JUNK2], CA + [anchor(eid(JUNK2), R, bn=11, ts=T + 1, tx=HX32(0x931))], bas=TWO),
      "sec 12.6: bytes passing neither law, anchored before the cut, are a byte string of both "
      "segments; MALFORMED in each, MISSING in the old; GAPS.", "GAPS")
    A("continue_old_unanchored_entry_on_line",
      inp(R, V1 + V2, [CA[0], CA[2], CA[3], CA[4]], bas=TWO),
      "sec 12.6: the old segment holds the whole line from the closing succession, anchored or "
      "not; the history entry is UNANCHORED there, informational.", "COMPLETE")
    A("continue_old_segment_gaps_wins",
      inp(R, V1 + V2, CA + [anchor(H_UNSEEN, R, bn=11, ts=T + 1, tx=HX32(0x932))], bas=TWO),
      "sec 8.6 item 15: the label is the first of the order either report carries; a MISSING "
      "row in the old segment makes the whole GAPS though the new segment is clean.", "GAPS")
    A("continue_new_segment_broken_wins",
      inp(R, V1 + V2 + [mk("root", "annotation", 1, eid(g2), ANNB("a twin under the new law"))], CA, bas=TWO),
      "sec 8.6 item 15: a fork in the new segment makes the whole BROKEN_CHAIN.", "BROKEN_CHAIN")
    A("continue_v2_entry_anchored_before_cut",
      inp(R, V1 + V2, CA[:3] + [anchor(eid(g2), R, bn=13, ts=T + 3), anchor(eid(a2), R, bn=11, ts=T + 1, tx=HX32(0x933))], bas=TWO),
      "sec 12.6: an entry of this law anchored before the cut is a byte string of both segments: "
      "a ledger entry in the new, MALFORMED and MISSING in the old.", "GAPS")
    A("continue_no_record_of_migration",
      inp(R, V1 + V2, [CA[0], CA[1], CA[3], CA[4]], bas=TWO),
      "sec 12.6: a closing succession no record anchors makes no candidate; the root is "
      "continued by nothing, the genesis is read as any genesis under the root, and the "
      "zikaron/1 bytes are MALFORMED here; the anchors of the zikaron/1 bytes, counted under this law, are MISSING.", "GAPS")
    A("continue_offline_no_candidate",
      inp(R, V1 + V2, [], bas=TWO),
      "sec 12.6: with no anchor set there is no cut and no continuation; the genesis carrying "
      "continues is a genesis with a member this law does not read.", "COMPLETE")
    hand = [mkv1("root", "genesis", 0, None, GENB)]
    hand.append(mkv1("root", "succession", 1, eid(hand[0]), SUCB(K2)))
    hand.append(mkv1("k2", "succession", 2, eid(hand[1]), SUCB(K2, kind="migration")))
    HI = [eid(b) for b in hand]
    g2k = mk("k2", "genesis", 0, None, {"statement_md": "continued by k2", "continues": {"root": R, "entry_id": HI[2]}})
    a2k = mk("k2", "annotation", 1, eid(g2k), ANNB("k2 under the new law"))
    HA = [anchor(HI[0], R, bn=10, ts=T), anchor(HI[1], R, bn=11, ts=T + 1), anchor(HI[2], K2, bn=12, ts=T + 2),
          anchor(eid(g2k), K2, bn=13, ts=T + 3), anchor(eid(a2k), K2, bn=14, ts=T + 4)]
    A("continue_root_unchanged_seed_is_migrating_key",
      inp(R, hand + [g2k, a2k], HA, bas=TWO),
      "sec 7.3/12.6: the continued ledger is read under the old root, and auth(0), the seed, and "
      "the key ROOT_MISMATCH tests against are the continuation's author, k2.", "COMPLETE")
    oldroot = mk("root", "annotation", 1, eid(g2k), ANNB("the old root key under the new law"))
    A("continue_old_root_key_outside_new_lineage",
      inp(R, hand + [g2k, a2k, oldroot], HA + [anchor(eid(oldroot), R, bn=15, ts=T + 5)], bas=TWO),
      "sec 7.4/12.6: the old root key is no part of the new segment's lineage, which is seeded "
      "with k2; its entry is EXCLUDED and its record DISCARDED.", "COMPLETE")
    A("continue_migrating_key_unanchored_migration_root_mismatch",
      inp(R, hand + [g2k, a2k], [HA[0], HA[1], HA[3], HA[4]], bas=TWO),
      "sec 12.6/8.1: with no record of the closing succession there is no continuation; under "
      "the root R the zikaron/1 bytes are MALFORMED and k2, named by no succession of this "
      "law, is outside the lineage, so its genesis and entry are EXCLUDED, its records "
      "DISCARDED, and the root's records of the zikaron/1 entries MISSING.", "GAPS")
    forged_m = mkv1("root", "succession", 2, HI[1], SUCB(R, kind="migration"))
    forged_g = None
    for k_ in range(256):   # the later-cut candidate takes the smaller entry_id, so that the cut alone decides
        forged_g = mk("root", "genesis", 0, None, {"statement_md": "forged %d" % k_, "continues": {"root": R, "entry_id": eid(forged_m)}})
        if eid(forged_g) < eid(g2k):
            break
    assert eid(forged_g) < eid(g2k)
    A("continue_handed_off_root_closes_later_is_no_candidate",
      inp(R, hand + [g2k, a2k, forged_m, forged_g],
          HA + [anchor(eid(forged_m), R, bn=20, ts=T + 20), anchor(eid(forged_g), R, bn=21, ts=T + 21)], bas=TWO),
      "sec 12.6: the stolen old root key signs a second closing succession and a second genesis, "
      "anchored later; its handing off is anchored at T+1, earlier than its cut, so it is no "
      "candidate, and the cut alone would decide the same (T+20 against T+2); k2's succession "
      "continues, the forged succession falls into the new segment as MALFORMED, its records "
      "are DISCARDED, and the forged genesis is EXCLUDED.", "COMPLETE")
    forged_g2 = mk("root", "genesis", 0, None, {"statement_md": "forged, earlier", "continues": {"root": R, "entry_id": eid(forged_m)}})
    A("continue_forged_candidate_anchored_before_the_true_one",
      inp(R, hand + [g2k, a2k, forged_m, forged_g2],
          [HA[0], HA[1], anchor(HI[2], K2, bn=12, ts=T + 5), anchor(eid(g2k), K2, bn=13, ts=T + 6), anchor(eid(a2k), K2, bn=14, ts=T + 7),
           anchor(eid(forged_m), R, bn=11, ts=T + 1, tx=HX32(0x940)), anchor(eid(forged_g2), R, bn=12, ts=T + 2, tx=HX32(0x941))], bas=TWO),
      "sec 12.6: a key of the old lineage that is not the honest key is a candidate's author by "
      "a succession anchored before the honest one, its handing off being anchored in the "
      "same second as that succession and so not earlier; zikaron/1 reads that old segment "
      "as its own fault (a key that handed off under zikaron/1 signing at a later position, "
      "a hard AUTHORITY_MISMATCH) and the forged genesis wins the cut; the honest succession "
      "lands in the new segment MALFORMED.", "BROKEN_CHAIN")
    def equal_cut(k):
        fg = mk("root", "genesis", 0, None, {"statement_md": "forged %d" % k, "continues": {"root": R, "entry_id": eid(forged_m)}})
        i = inp(R, hand + [g2k, a2k, forged_m, fg],
                HA + [anchor(eid(forged_m), R, bn=12, ts=T + 2, tx=HX32(0x942)), anchor(eid(fg), R, bn=13, ts=T + 3, tx=HX32(0x943))], bas=TWO)
        return i, eid(g2k) < eid(fg)
    both_orders("continue_handed_off_root_closes_at_same_cut_is_no_candidate", equal_cut,
                "sec 12.6: the stolen old root key closes at k2's own cut; its handing off is anchored "
                "earlier, so it is no candidate whatever its genesis's id, k2's succession continues, "
                "and the old segment, holding both successions at one position, is BROKEN_CHAIN.",
                "BROKEN_CHAIN")
    s_twice = mkv1("root", "succession", 2, V1I[1], dict(SUCB(R, kind="migration"), statement_md="closed again"))
    def equal_cut_same_key(k):
        gb = mk("root", "genesis", 0, None, {"statement_md": "second %d" % k, "continues": {"root": R, "entry_id": eid(s_twice)}})
        i = inp(R, V1 + [g2, s_twice, gb],
                CA[:4] + [anchor(eid(s_twice), R, bn=12, ts=T + 2, tx=HX32(0x9B0)), anchor(eid(gb), R, bn=13, ts=T + 3, tx=HX32(0x9B1))], bas=TWO)
        return i, eid(g2) < eid(gb)
    both_orders("continue_two_candidates_equal_cut", equal_cut_same_key,
                "sec 12.6: one key closes twice at one position, both successions anchored in one "
                "second and each named by a genesis; a succession to itself hands nothing off, so "
                "both are candidates, the continuation is the genesis with the bytewise-smallest "
                "entry_id, and BROKEN_CHAIN comes from both segments: the old reads the fork at seq "
                "2, the new the two root geneses at seq 0.", "BROKEN_CHAIN")
    gb_late = None
    for k_ in range(256):
        gb_late = mk("root", "genesis", 0, None, {"statement_md": "later cut %d" % k_, "continues": {"root": R, "entry_id": eid(s_twice)}})
        if eid(gb_late) < eid(g2):
            break
    assert eid(gb_late) < eid(g2)
    A("continue_two_candidates_earliest_cut_wins",
      inp(R, V1 + [g2, s_twice, gb_late],
          CA[:4] + [anchor(eid(s_twice), R, bn=13, ts=T + 3, tx=HX32(0x9B2)), anchor(eid(gb_late), R, bn=14, ts=T + 4, tx=HX32(0x9B3))], bas=TWO),
      "sec 12.6: one key closes twice; the later-cut candidate's genesis has the smaller id, "
      "and the cut alone decides: the earlier closing succession continues, the later one falls "
      "into the new segment MALFORMED with its record MISSING, and BROKEN_CHAIN comes from the "
      "two root geneses at seq 0 in the new segment, the old report being COMPLETE.", "BROKEN_CHAIN")
    fk2 = mkv1("root", "succession", 1, HI[0], SUCB(K2))
    fk3 = mkv1("root", "succession", 1, HI[0], SUCB(K3))
    ck2 = mkv1("k2", "succession", 2, eid(fk2), SUCB(K2, kind="migration"))
    ck3 = mkv1("k3", "succession", 2, eid(fk3), SUCB(K3, kind="migration"))
    gk2 = mk("k2", "genesis", 0, None, {"statement_md": "k2 continues", "continues": {"root": R, "entry_id": eid(ck2)}})
    gk3 = mk("k3", "genesis", 0, None, {"statement_md": "k3 continues", "continues": {"root": R, "entry_id": eid(ck3)}})
    A("continue_two_forked_successors_equal_cut",
      inp(R, [hand[0], fk2, fk3, ck2, ck3, gk2, gk3],
          [HA[0], anchor(eid(fk2), R, bn=11, ts=T + 1, tx=HX32(0x9B4)), anchor(eid(fk3), R, bn=11, ts=T + 1, tx=HX32(0x9B5)),
           anchor(eid(ck2), K2, bn=12, ts=T + 2, tx=HX32(0x9B6)), anchor(eid(ck3), K3, bn=12, ts=T + 2, tx=HX32(0x9B7)),
           anchor(eid(gk2), K2, bn=13, ts=T + 3, tx=HX32(0x9B8)), anchor(eid(gk3), K3, bn=13, ts=T + 3, tx=HX32(0x9B9))], bas=TWO),
      "sec 12.6: the root hands the seat to two keys at one position, each closes at the same "
      "second and each continues; neither handed off, both are candidates of one cut, the "
      "smaller genesis id wins, and the old segment is BROKEN_CHAIN from the fork.", "BROKEN_CHAIN")
    str_m = mkv1("k9", "succession", 2, HI[1], SUCB(K9, kind="migration"))
    str_g = mk("k9", "genesis", 0, None, {"statement_md": "a stranger continues", "continues": {"root": R, "entry_id": eid(str_m)}})
    A("continue_stranger_candidate_ignored",
      inp(R, hand + [g2k, a2k, str_m, str_g],
          HA + [anchor(eid(str_m), K9, bn=11, ts=T + 1, tx=HX32(0x944)), anchor(eid(str_g), K9, bn=12, ts=T + 2, tx=HX32(0x945))], bas=TWO),
      "sec 12.6: a stranger's self-succession is no ledger entry of the old segment's zikaron/1 "
      "audit, so its genesis is no candidate, early anchor or not; the honest continuation "
      "stands and the stranger's bytes are EXCLUDED or MALFORMED.", "COMPLETE")
    A("continue_stranger_candidate_alone",
      inp(R, hand + [str_m, str_g],
          HA[:3] + [anchor(eid(str_m), K9, bn=11, ts=T + 1, tx=HX32(0x944)), anchor(eid(str_g), K9, bn=12, ts=T + 2, tx=HX32(0x945))], bas=TWO),
      "sec 12.6: with no honest candidate the root is continued by nothing; the stranger's "
      "genesis is EXCLUDED and every zikaron/1 byte string MALFORMED; the anchors of the zikaron/1 bytes, counted under this law, are MISSING.", "GAPS")
    wrong_root = mk("root", "genesis", 0, None, {"statement_md": "c", "continues": {"root": K2, "entry_id": V1I[2]}})
    A("continue_wrong_root",
      inp(R, V1 + [wrong_root], CA[:3] + [anchor(eid(wrong_root), R, bn=13, ts=T + 3)], bas=TWO),
      "sec 12.6: continues.root other than the audited root makes no candidate; the root is "
      "continued by nothing, the zikaron/1 bytes are MALFORMED here and their anchors "
      "MISSING.", "GAPS")
    A("continue_entry_id_not_migration_kind",
      inp(R, V1[:2] + [mkv1("root", "succession", 2, V1I[1], SUCB(R, kind="handover")),
                       mk("root", "genesis", 0, None, {"statement_md": "c", "continues": {"root": R, "entry_id": eid(mkv1("root", "succession", 2, V1I[1], SUCB(R, kind="handover")))}})],
          CA[:2] + [anchor(eid(mkv1("root", "succession", 2, V1I[1], SUCB(R, kind="handover"))), R, bn=12, ts=T + 2)], bas=TWO),
      "sec 12.6: a self-succession whose kind is not the token migration closes nothing this law "
      "reads; no candidate; the anchors of the zikaron/1 bytes, counted under this law, are MISSING.", "GAPS")
    A("continue_entry_id_other_key",
      inp(R, hand + [mk("root", "genesis", 0, None, {"statement_md": "c", "continues": {"root": R, "entry_id": HI[2]}})], HA[:3], bas=TWO),
      "sec 12.6: the genesis's author must be the succession's author and its to; a genesis by "
      "the old root naming k2's closing succession is no candidate, and under the root it is "
      "an ordinary genesis; the anchors of the zikaron/1 bytes, counted under this law, are MISSING.", "GAPS")
    g2x = mk("root", "genesis", 0, None, {"statement_md": "c", "continues": {"root": R, "entry_id": V1I[2], "law": "zikaron/1"}})
    A("continue_extra_member_in_continues",
      inp(R, V1 + [g2x], CA[:3] + [anchor(eid(g2x), R, bn=13, ts=T + 3)], bas=TWO),
      "sec 6.1/12.6: members beyond root and entry_id are permitted and change nothing; the "
      "genesis is the continuation.", "COMPLETE")
    A("continue_cut_smallest_across_chains",
      inp(R, V1 + V2, CA[:2] + [anchor(V1I[2], R, bn=12, ts=T + 10), anchor(V1I[2], R, cid=8453, bn=2, ts=T + 2, tx=HX32(0x950)),
                                anchor(eid(g2), R, bn=13, ts=T + 5), anchor(eid(a2), R, bn=14, ts=T + 6)], bas=TWO),
      "sec 12.6: the cut is the smallest blockTimestamp over every chain, T+2 here, so the "
      "genesis at T+5 on chain 1 is in the new segment though chain 1's record of the "
      "succession is later.", "COMPLETE")
    A("continue_cut_one_chain_scanned",
      inp(R, V1 + V2, CA[:2] + [anchor(V1I[2], R, bn=12, ts=T + 10), anchor(eid(g2), R, bn=13, ts=T + 5), anchor(eid(a2), R, bn=14, ts=T + 6)],
          bas=basis((chainobj(1, senders=(R, K2)),))),
      "sec 12.6: under a basis reaching chain 1 alone the cut is T+10, and the genesis at T+5 "
      "falls into the old segment; GAPS, as the seat's own ordering on each chain forbids.",
      "GAPS")
    hs = mk("root", "succession", 2, eid(a2), SUCB(K2))
    A("continue_then_handoff_retires",
      inp(R, V1 + V2 + [hs, mk("k2", "annotation", 3, eid(hs), ANNB("k2"))],
          CA + [anchor(eid(hs), R, bn=15, ts=T + 5), anchor(H_UNSEEN, R, bn=16, ts=T + 6, tx=HX32(0x951))], bas=TWO),
      "sec 7.4 in the new segment: the migrating key hands off under this law at T+5 and its "
      "record at T+6 is discarded.", "COMPLETE")
    A("continue_unproven_record_of_old_entry_after_cut",
      inp(R, V1 + V2, CA + [anchor(V1I[1], R, bn=15, ts=T + 5, tx=HX32(0x953), verdict="UNPROVEN")], bas=TWO),
      "sec 12.6: a record after the cut whose hash is an old ledger entry is an old record "
      "whatever its verdict; zikaron/1 reads it beside the counted record of the same entry, "
      "the UNPROVEN record is over bytes in hand, and no label moves.",
      "COMPLETE")
    A("continue_unproven_of_unseen_hash_in_new_segment",
      inp(R, V1 + V2, CA + [anchor(H_UNSEEN, R, bn=15, ts=T + 5, tx=HX32(0x954), verdict="UNPROVEN")], bas=TWO),
      "sec 8.6 item 15 rule 2 in the new segment: an UNPROVEN record over bytes nobody holds "
      "moves the label.", "UNPROVEN")
    A("continue_old_seg_unproven_label",
      inp(R, V1 + V2, CA + [anchor(H_UNSEEN, R, bn=11, ts=T + 1, tx=HX32(0x952), verdict="UNPROVEN")], bas=TWO),
      "sec 8.6 item 15: UNAVAILABLE of the old segment's zikaron/1 report reads as UNPROVEN in "
      "the label order.", "UNPROVEN")

    # ---------------- round 1 fold: witnesses the seats asked for -----------------
    A("continue_reanchor_old_entry_after_cut_on_added_chain",
      inp(R, V1 + V2, CA + [anchor(V1I[1], R, cid=8453, bn=20, ts=T + 20, tx=HX32(0x960))], bas=TWO),
      "sec 12.6/5.7: a seat that adds a chain anchors its closed history there after the cut; "
      "the record's hash is an old ledger entry, so it is an old record whatever its time, "
      "counted there beside the first, and nothing is MISSING.", "COMPLETE")
    A("continue_reanchor_old_entry_after_cut_void",
      inp(R, V1 + V2, CA + [anchor(V1I[1], R, cid=8453, bn=20, ts=T + 20, tx=HX32(0x961), verdict="VOID")], bas=TWO),
      "sec 12.6: the same record with verdict VOID is an old record too, its hash an old ledger "
      "entry; zikaron/1 carries it in its VOID list and no label turns on it.", "COMPLETE")
    A("continue_late_record_of_unseen_hash_is_new",
      inp(R, V1 + V2, CA + [anchor(H_UNSEEN, R, bn=20, ts=T + 20, tx=HX32(0x962))], bas=TWO),
      "sec 12.6: a record after the cut whose hash is no old ledger entry is a record of the new "
      "segment, where the key in office has not retired, so MISSING.", "GAPS")
    s_to_other = mkv1("root", "succession", 2, V1I[1], SUCB(K2, kind="migration"))
    g_to_other = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": eid(s_to_other)}})
    A("continue_candidate_to_differs_from_author",
      inp(R, V1[:2] + [s_to_other, g_to_other],
          CA[:2] + [anchor(eid(s_to_other), R, bn=12, ts=T + 2), anchor(eid(g_to_other), R, bn=13, ts=T + 3)], bas=TWO),
      "sec 12.6: a candidate's succession has author and to both equal to the genesis's author; "
      "one whose to is another key names no candidate, so the root is continued by nothing, "
      "the zikaron/1 bytes are MALFORMED under this law and their records MISSING.", "GAPS")
    g_hist = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": V1I[1]}})
    A("continue_names_v1_history",
      inp(R, V1 + [g_hist], CA[:3] + [anchor(eid(g_hist), R, bn=13, ts=T + 3)], bas=TWO),
      "sec 12.6: continues names a zikaron/1 history and not a succession; no candidate; GAPS "
      "from the records of the zikaron/1 bytes.", "GAPS")
    bad_s = make_entry_v1(K("root"), "succession", 1, V1I[0], {"to": R})
    g_bad = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": eid(bad_s)}})
    A("continue_names_byte_string_failing_v1",
      inp(R, V1[:1] + [bad_s, g_bad], CA[:1] + [anchor(eid(bad_s), R, bn=11, ts=T + 1), anchor(eid(g_bad), R, bn=13, ts=T + 3)], bas=TWO),
      "sec 12.6: continues names a zikaron/1-spec byte string that fails zikaron/1 sec 4.3 at "
      "its body row; no candidate; under this law the byte string is MALFORMED (E_SPEC) and "
      "its record MISSING.", "GAPS")
    A("continue_named_succession_absent_from_pile",
      inp(R, V1[:2] + V2, CA, bas=TWO),
      "sec 12.6: the closing succession is anchored and not held; no candidate; the genesis is "
      "read under the root as any genesis and the zikaron/1 records are MISSING.", "GAPS")
    A("continue_candidate_record_by_stranger_unproven",
      inp(R, V1 + V2, [CA[0], CA[1], anchor(V1I[2], K9, bn=12, ts=T + 2, tx=HX32(0x963), verdict="UNPROVEN"), CA[3], CA[4]], bas=TWO),
      "sec 12.6: the record that makes a candidate is any record, whatever its sender and "
      "verdict; a stranger's UNPROVEN record sets the cut at T+2, zikaron/1 discards it in the "
      "old segment, where the succession is UNANCHORED, informational.", "COMPLETE")
    A("continue_cut_from_void_record",
      inp(R, V1 + V2, [CA[0], CA[1], anchor(V1I[2], R, bn=12, ts=T + 2, tx=HX32(0x964), verdict="VOID"),
                       anchor(V1I[2], R, bn=14, ts=T + 4, tx=HX32(0x965)), CA[3], CA[4]], bas=TWO),
      "sec 12.6: the cut is the smallest time of the records anchoring the closing succession "
      "whatever their verdict, T+2 from a VOID record, so the genesis at T+3 is a byte string "
      "of the new segment; the counted record at T+4 is an old record by its hash.", "COMPLETE")
    s2r = mk("k2", "succession", 2, eid(a2k), SUCB(R))
    gR = mk("root", "genesis", 0, None, GENB)
    A("continue_old_root_key_readmitted_opens_genesis",
      inp(R, hand + [g2k, a2k, s2r, gR], HA + [anchor(eid(s2r), K2, bn=15, ts=T + 5), anchor(eid(gR), R, bn=16, ts=T + 6)], bas=TWO),
      "sec 7.3/8.2/12.6: k2 names the old root key into the new segment's lineage by a timely "
      "succession; that key's genesis at seq 0 is tested against auth(0), the continuation's "
      "author k2, and not against the root R it equals: ROOT_MISMATCH beside the "
      "EQUIVOCATION with k2's genesis.", "BROKEN_CHAIN")
    b1 = mkv1("root", "history", 1, V1I[0], HISB(5))
    b2 = mkv1("root", "annotation", 2, eid(b1), ANNB("off the line"))
    A("continue_old_branch_through_anchored_successor",
      inp(R, V1 + V2 + [b1, b2], CA + [anchor(eid(b2), R, bn=11, ts=T + 1, tx=HX32(0x966))], bas=TWO),
      "sec 12.6: a zikaron/1 branch off the closing succession's line enters the old segment "
      "through its anchored member, b2 anchored before the cut, and b1 through b2's prev; "
      "zikaron/1 records two equivocations.", "BROKEN_CHAIN")
    # -- round 5 fold --------------------------------------------------------
    br2 = mk("root", "annotation", 2, eid(b1), ANNB("a bridge of this law on an off-line zikaron/1 prev"))
    A("continue_old_branch_bridge_fails_v1_grammar",
      inp(R, V1 + V2 + [b1, br2], CA + [anchor(eid(br2), R, bn=11, ts=T + 1, tx=HX32(0x967))], bas=TWO),
      "sec 12.6 third clause: reach along prev into the old segment starts only at an anchored "
      "byte string passing zikaron/1 sec 4.3 and passes only through such byte strings; the "
      "bridge anchored before the cut fails that grammar, so b1 behind it is no old byte "
      "string and falls to the new segment as MALFORMED, while the bridge is a byte string of "
      "both segments (compare continue_old_branch_through_anchored_successor, whose zikaron/1 b2 carries b1 "
      "in); BROKEN_CHAIN from the new segment's hard PREV_MISMATCH at seq 2, the old report GAPS.", "BROKEN_CHAIN")
    sx5 = mkv1("root", "succession", 1, V1I[0], SUCB(K2))
    sk5 = mkv1("k2", "succession", 1, V1I[0], SUCB(K2, kind="migration"))
    gk5 = mk("k2", "genesis", 0, None, {"statement_md": "continued by k2", "continues": {"root": R, "entry_id": eid(sk5)}})
    A("continue_candidate_author_lineage_over_candidate_old_bytes_only",
      inp(R, [V1[0], sx5, sk5, gk5], [CA[0], anchor(eid(sk5), K2, bn=12, ts=T + 2, tx=HX32(0xA70)), anchor(eid(gk5), K2, bn=13, ts=T + 3, tx=HX32(0xA71))], bas=TWO),
      "sec 12.6 candidate: k2's closing succession hangs off the root's genesis and the root's "
      "zikaron/1 succession naming k2 is held but anchored by nobody and off that line, so it "
      "is no old byte string of k2's candidacy and k2 is outside the candidate's old ledger's "
      "lineage; no candidate, the root uncontinued, the zikaron/1 bytes MALFORMED under this "
      "law.", "GAPS")
    A("continue_candidate_author_named_by_anchored_off_line_succession",
      inp(R, [V1[0], sx5, sk5, gk5], [CA[0], anchor(eid(sx5), R, bn=11, ts=T + 1, tx=HX32(0xA72)), anchor(eid(sk5), K2, bn=12, ts=T + 2, tx=HX32(0xA70)), anchor(eid(gk5), K2, bn=13, ts=T + 3, tx=HX32(0xA71))], bas=TWO),
      "sec 12.6 candidate: the same succession anchored before the cut is an old byte string, "
      "k2 joins the candidate's old lineage, its closing succession is an old ledger entry and "
      "the ledger continues; zikaron/1 reads the two entries at seq 1 as EQUIVOCATION.", "BROKEN_CHAIN")
    g2b = mk("root", "genesis", 0, None, {"statement_md": "a twin naming the same close", "continues": {"root": R, "entry_id": V1I[2]}})
    A("continue_two_geneses_one_succession",
      inp(R, V1 + V2 + [g2b], CA + [anchor(eid(g2b), R, bn=13, ts=T + 3, tx=HX32(0xA77))], bas=TWO),
      "sec 12.6: two geneses by one key name one closing succession; both are candidates with one "
      "author and one cut, so the choice between them reaches no output: continues names s and the "
      "cut, and the new segment reads the two ledger entries at seq 0 as EQUIVOCATION.", "BROKEN_CHAIN")
    H27 = HX32(0xB27)
    A("continue_unproven_step_reads_new_segment_records_only",
      inp(R, V1 + V2, CA + [anchor(H27, R, bn=11, ts=T + 1, tx=HX32(0xA73)), anchor(H27, R, bn=60, ts=T + 60, tx=HX32(0xA74), verdict="UNPROVEN")], bas=TWO),
      "sec 8.6 item 15 rule 2 with sec 12.6: the counted record of bytes nobody holds at T+1 is "
      "an old record, MISSING in the old report; the UNPROVEN record at T+60 is a new record "
      "whose hash no byte string carries and no counted record of the new segment matches, "
      "so the label is UNPROVEN, and a reading that counts the old record would say GAPS.", "UNPROVEN")
    Sn5 = mk("k2", "succession", 2, eid(a2k), SUCB(K3))
    an5 = mk("k2", "annotation", 3, eid(Sn5), ANNB("by k2 after handing on, anchored before the cut only"))
    A("continue_timeliness_reads_new_segment_records_only",
      inp(R, hand + [g2k, a2k, Sn5, an5], HA + [anchor(eid(Sn5), K3, bn=15, ts=T + 5, tx=HX32(0xA75)), anchor(eid(an5), K2, bn=11, ts=T + 1, tx=HX32(0xA76))], bas=TWO),
      "sec 7.4 with sec 12.6: k2 hands on at T+5 and its annotation after that succession is "
      "anchored only at T+1, before the cut, an old record; timeliness ranges over the new "
      "segment's records alone, so the annotation is RETIRED there, while a reading that "
      "counts every record would make it a ledger entry; GAPS from the old report's MISSING over the "
      "annotation's bytes, MALFORMED under zikaron/1.", "GAPS")
    JUNKP = b'{"junk":"prev"}'
    s_jp = mkv1("root", "succession", 2, eid(JUNKP), SUCB(R, kind="migration"))
    g_jp = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": eid(s_jp)}})
    A("continue_line_stops_before_junk_prev",
      inp(R, V1[:2] + [s_jp, g_jp, JUNKP],
          CA[:2] + [anchor(eid(s_jp), R, bn=12, ts=T + 2, tx=HX32(0x967)), anchor(eid(g_jp), R, bn=13, ts=T + 3, tx=HX32(0x968))], bas=TWO),
      "sec 12.6: the closing succession's prev names bytes that pass neither law; the old line "
      "stops at the succession, zikaron/1 records PREV_MISMATCH against the history at seq 1, "
      "and the junk, anchored by nobody, is a byte string of the new segment, MALFORMED "
      "there and nowhere else.", "BROKEN_CHAIN")
    s_bs = mkv1("root", "succession", 2, eid(bad_s), SUCB(R, kind="migration"))
    g_bs = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": eid(s_bs)}})
    A("continue_line_stops_before_v1_spec_near_entry",
      inp(R, V1[:2] + [bad_s, s_bs, g_bs],
          CA[:2] + [anchor(eid(s_bs), R, bn=12, ts=T + 2, tx=HX32(0x969)), anchor(eid(g_bs), R, bn=13, ts=T + 3, tx=HX32(0x96a))], bas=TWO),
      "sec 12.6: the closing succession's prev names a zikaron/1-spec byte string that fails "
      "zikaron/1 sec 4.3 at a later step; the old line stops before it, and unanchored it is "
      "a byte string of the new segment alone, MALFORMED there at E_SPEC.", "BROKEN_CHAIN")
    A("continue_old_segment_seq_gap",
      inp(R, [V1[0], V1[2]] + V2, CA, bas=TWO),
      "sec 12.6 with zikaron/1 sec 8.2: the history at seq 1 is not held; the old segment is "
      "the genesis, anchored before the cut, and the closing succession, whose line stops at "
      "once; zikaron/1 records SEQ_GAP and the history's record is MISSING.", "GAPS")
    # -- round 2 fold --------------------------------------------------------
    s1 = mkv1("root", "succession", 1, HI[0], SUCB(R, kind="migration"))
    g1 = mk("root", "genesis", 0, None, {"statement_md": "the stolen key continues", "continues": {"root": R, "entry_id": eid(s1)}})
    A("continue_handed_off_key_closes_first_handover_unanchored",
      inp(R, hand + [g2k, a2k, s1, g1],
          [HA[0], anchor(eid(s1), R, bn=12, ts=T + 2, tx=HX32(0x980)), anchor(eid(g1), R, bn=13, ts=T + 3, tx=HX32(0x981)),
           anchor(HI[2], K2, bn=15, ts=T + 5), anchor(eid(g2k), K2, bn=16, ts=T + 6), anchor(eid(a2k), K2, bn=17, ts=T + 7)], bas=TWO),
      "sec 12.6: the root handed its zikaron/1 seat to k2 by a succession nobody anchored; the "
      "stolen root key then closes the ledger at seq 1 and anchors first. No record earlier "
      "than its cut shows its handing off, so it is a candidate and wins the cut: the honest "
      "handover, closing succession and genesis are MALFORMED or EXCLUDED here, and the law "
      "reaches the theft only where the anchor set records the handing off.", "COMPLETE")
    A("continue_handed_off_key_closes_first_handover_anchored_earlier",
      inp(R, hand + [g2k, a2k, s1, g1],
          [HA[0], HA[1], anchor(eid(s1), R, bn=12, ts=T + 2, tx=HX32(0x980)), anchor(eid(g1), R, bn=13, ts=T + 3, tx=HX32(0x981)),
           anchor(HI[2], K2, bn=15, ts=T + 5), anchor(eid(g2k), K2, bn=16, ts=T + 6), anchor(eid(a2k), K2, bn=17, ts=T + 7)], bas=TWO),
      "sec 12.6: the handover is anchored at T+1, earlier than the stolen key's closing "
      "succession at T+2, so that succession makes no candidate; k2's closing succession "
      "continues the ledger, the stolen key's succession is an old byte string at a held "
      "position (EQUIVOCATION under zikaron/1), and its genesis is EXCLUDED in the new "
      "segment.", "BROKEN_CHAIN")
    A("continue_handed_off_key_closes_first_handover_anchored_same_second",
      inp(R, hand + [g2k, a2k, s1, g1],
          [HA[0], anchor(HI[1], R, bn=12, ts=T + 2, tx=HX32(0x982)), anchor(eid(s1), R, bn=12, ts=T + 2, tx=HX32(0x980)),
           anchor(eid(g1), R, bn=13, ts=T + 3, tx=HX32(0x981)),
           anchor(HI[2], K2, bn=15, ts=T + 5), anchor(eid(g2k), K2, bn=16, ts=T + 6), anchor(eid(a2k), K2, bn=17, ts=T + 7)], bas=TWO),
      "sec 12.6: the handover's record is in the closing succession's own second, not earlier, "
      "so the stolen key's succession stays a candidate and wins the cut; the old segment "
      "holds both seq-1 successions and reads BROKEN_CHAIN.", "BROKEN_CHAIN")
    s3 = mk("root", "succession", 1, eid(g2), SUCB(K3))
    h3 = mk("root", "history", 2, eid(s3), HISB(3))
    A("continue_v2_succession_anchored_at_cut_retires_nobody",
      inp(R, V1 + [g2, s3, h3],
          CA[:3] + [anchor(eid(g2), R, bn=13, ts=T + 3), anchor(eid(s3), R, bn=12, ts=T + 2, tx=HX32(0x983)), anchor(eid(h3), R, bn=15, ts=T + 5)], bas=TWO),
      "sec 7.4/12.6: the new ledger's succession is anchored in the cut's second alone; that "
      "record is an old record, so in the new segment the root has no retirement, its "
      "history after handing off is a ledger entry with a hard AUTHORITY_MISMATCH, and the "
      "old segment lists the succession MALFORMED and MISSING. Read over the whole anchor set "
      "the history would be RETIRED instead.", "BROKEN_CHAIN")
    A("continue_v2_succession_anchored_at_cut_and_later",
      inp(R, V1 + [g2, s3, h3],
          CA[:3] + [anchor(eid(g2), R, bn=13, ts=T + 3), anchor(eid(s3), R, bn=12, ts=T + 2, tx=HX32(0x983)),
                    anchor(eid(s3), R, bn=14, ts=T + 4, tx=HX32(0x984)), anchor(eid(h3), R, bn=15, ts=T + 5)], bas=TWO),
      "sec 7.4/12.6: the same succession also anchored after the cut retires the root at T+4 in "
      "the new segment; the history at T+5 is RETIRED and its record DISCARDED; GAPS from the "
      "old segment's reading of the at-cut record.", "GAPS")
    A("continue_stranger_record_of_v2_succession_at_cut",
      inp(R, V1 + [g2, s3, h3],
          CA[:3] + [anchor(eid(g2), R, bn=13, ts=T + 3), anchor(eid(s3), K9, bn=12, ts=T + 2, tx=HX32(0x985)), anchor(eid(h3), R, bn=15, ts=T + 5)], bas=TWO),
      "sec 7.4/12.6: a stranger's record of the new succession in the cut's second is an old "
      "record (zikaron/1 discards it there) and retires nobody in the new segment.", "BROKEN_CHAIN")
    A("continue_v2_genesis_anchored_at_cut_and_later",
      inp(R, V1 + V2, CA[:3] + [anchor(eid(g2), R, bn=13, ts=T + 2, tx=HX32(0x986)), anchor(eid(g2), R, cid=8453, bn=20, ts=T + 20, tx=HX32(0x987)), CA[4]], bas=TWO),
      "sec 12.6: the genesis anchored in the cut's second is an old byte string of both "
      "segments, MALFORMED and MISSING in the old; its later record on another chain is a "
      "new record, since the genesis passes zikaron/1 nowhere, and the new segment lists the "
      "genesis ANCHORED.", "GAPS")
    JUNK3 = b'{"junk":3}'
    A("continue_junk_anchored_before_and_after_cut",
      inp(R, V1 + V2 + [JUNK3], CA + [anchor(eid(JUNK3), R, bn=11, ts=T + 1, tx=HX32(0x988)), anchor(eid(JUNK3), R, bn=20, ts=T + 20, tx=HX32(0x989))], bas=TWO),
      "sec 12.6: junk anchored before the cut is an old byte string of both segments; its "
      "later record is placed by time into the new segment, since the bytes pass zikaron/1 "
      "nowhere; MISSING in both.", "GAPS")
    A("continue_label_old_broken_new_gaps",
      inp(R, V1 + V2 + [v1twin], CA + [anchor(eid(v1twin), R, bn=11, ts=T + 1, tx=HX32(0x930)), anchor(H_UNSEEN, R, bn=20, ts=T + 20, tx=HX32(0x98a))], bas=TWO),
      "sec 8.6 item 15: the old report BROKEN_CHAIN, the new GAPS; the first in order wins.", "BROKEN_CHAIN")
    A("continue_label_old_gaps_new_unproven",
      inp(R, V1 + V2, CA + [anchor(H_UNSEEN, R, bn=11, ts=T + 1, tx=HX32(0x932)), anchor(HX32(0xBEEF3), R, bn=20, ts=T + 20, tx=HX32(0x98b), verdict="UNPROVEN")], bas=TWO),
      "sec 8.6 item 15: the old report GAPS, the new UNPROVEN; UNPROVEN precedes GAPS in the "
      "order.", "UNPROVEN")
    A("continue_label_old_unavailable_new_gaps",
      inp(R, V1 + V2, CA + [anchor(H_UNSEEN, R, bn=11, ts=T + 1, tx=HX32(0x952), verdict="UNPROVEN"), anchor(HX32(0xBEEF3), R, bn=20, ts=T + 20, tx=HX32(0x98c))], bas=TWO),
      "sec 8.6 item 15: UNAVAILABLE of the old report reads as UNPROVEN, which precedes the new "
      "report's GAPS.", "UNPROVEN")
    g_badsig = set_key(g2, "author", K9)
    A("continue_candidate_fails_sec_4_3",
      inp(R, V1 + [g_badsig], CA[:3] + [anchor(eid(g_badsig), R, bn=13, ts=T + 3, tx=HX32(0x98d))], bas=TWO),
      "sec 12.6: a candidate passes sec 4.3; a genesis whose signature does not recover to its "
      "author names no continuation, the root is continued by nothing, and the zikaron/1 "
      "records are MISSING.", "GAPS")
    a_cont = mk("root", "annotation", 1, HX32(0x77), {"note_md": "not a genesis", "continues": {"root": R, "entry_id": V1I[2]}})
    A("continue_candidate_has_seq_zero",
      inp(R, V1 + [a_cont], CA[:3] + [anchor(eid(a_cont), R, bn=13, ts=T + 3, tx=HX32(0x98e))], bas=TWO),
      "sec 12.6: a candidate has seq 0; an annotation at seq 1 carrying continues as an extra "
      "member is no candidate.", "GAPS")
    gv1c = mkv1("root", "genesis", 0, None, dict(GENB, continues={"root": R, "entry_id": V1I[2]}))
    A("continue_v1_genesis_with_continues_member",
      inp(R, V1 + V2 + [gv1c], CA + [anchor(eid(gv1c), R, bn=11, ts=T + 1, tx=HX32(0x98f))], bas=TWO),
      "sec 12.6: a zikaron/1 genesis carrying continues as an extra member is no candidate "
      "(a candidate passes this law's sec 4.3); anchored before the cut it is an old byte "
      "string, a second seq-0 entry of the old root, and zikaron/1 records the equivocation.",
      "BROKEN_CHAIN")
    str1 = mkv1("k9", "succession", 1, V1I[0], SUCB(K9, kind="migration"))
    g_str1 = mk("k9", "genesis", 0, None, {"statement_md": "a stranger continues at seq 1", "continues": {"root": R, "entry_id": eid(str1)}})
    A("continue_stranger_closing_at_seq_1",
      inp(R, V1[:2] + [str1, g_str1], CA[:2] + [anchor(eid(str1), K9, bn=12, ts=T + 2, tx=HX32(0x990)), anchor(eid(g_str1), K9, bn=13, ts=T + 3, tx=HX32(0x991))], bas=TWO),
      "sec 12.6: a closing succession by a key outside the zikaron/1 lineage is no ledger entry "
      "of the old segment at any position, seq 1 included; no candidate, GAPS from the root's "
      "records.", "GAPS")
    a_k3 = mk("k3", "annotation", 2, eid(s3), ANNB("k3 in office"))
    A("continue_stranger_anchors_new_succession_before_cut",
      inp(R, V1 + [g2, s3, a_k3],
          CA[:3] + [anchor(eid(s3), K9, bn=11, ts=T + 1, tx=HX32(0x992)), anchor(eid(g2), R, bn=13, ts=T + 3), anchor(eid(a_k3), K3, bn=15, ts=T + 5)], bas=TWO),
      "sec 7.4/12.6: a stranger anchors the new ledger's succession before the cut; that record "
      "is an old record, the root has no retirement in the new segment, its record of the "
      "genesis at T+3 counts and nothing is DISCARDED there; the old segment lists the "
      "succession MALFORMED and discards the stranger's record, so nothing is MISSING. The "
      "zikaron/2 succession, failing zikaron/1 sec 4.3, is no handing off under zikaron/1.",
      "COMPLETE")
    # the old segment's malformed vocabulary: one zikaron/1-spec byte string per token (E1)
    base_v1 = mkv1("root", "annotation", 1, V1I[0], ANNB("a zikaron/1 near-entry"))
    sig0 = Parser(base_v1.decode()).text().get("sig")
    r_hex, s_hex, v_hex = sig0[2:66], sig0[66:130], sig0[130:132]
    s_int = int(s_hex, 16)
    hi_s = "0x" + r_hex + ("%064x" % (N - s_int)) + ("1c" if v_hex == "1b" else "1b")
    flip = ("1" if r_hex[0] != "1" else "2") + r_hex[1:]
    v1_tokens = [
        ("E_ENTRYTYPE", v1gen.set_key(base_v1, "entryType", "a b")),
        ("E_AUTHOR", v1gen.set_key(base_v1, "author", "0x12")),
        ("E_SEQ", v1gen.set_key(base_v1, "seq", "1")),
        ("E_PREV", v1gen.set_key(base_v1, "prev", "0xzz")),
        ("E_PREV_SEQ", v1gen.set_key(base_v1, "prev", None)),
        ("E_BODY", v1gen.set_key(base_v1, "body", [])),
        ("E_SIG_FORM", v1gen.set_key(base_v1, "sig", "0x12")),
        ("E_GENESIS_PLACE", v1gen.set_key(base_v1, "entryType", "genesis")),
        ("E_BODY_FIELD_history", mkv1("root", "history", 1, V1I[0], {})),
        ("E_BODY_FIELD_grant", mkv1("root", "grant", 1, V1I[0], {})),
        ("E_BODY_FIELD_grant_window", mkv1("root", "grant", 1, V1I[0], {"grantee": HX20(0xBEEF), "work": HX32(0xC0), "terms": HX32(0x7E), "window": {"from": 5, "to": 1}})),
        ("E_BODY_FIELD_revocation", mkv1("root", "revocation", 1, V1I[0], {})),
        ("E_BODY_FIELD_adoption", mkv1("root", "adoption", 1, V1I[0], {"anchors": []})),
        ("E_BODY_FIELD_adoption_element", mkv1("root", "adoption", 1, V1I[0], {"anchors": [{"chainId": 1}]})),
        ("E_BODY_FIELD_annotation", mkv1("root", "annotation", 1, V1I[0], {"note_md": 1})),
        ("E_SIG_V", v1gen.set_key(base_v1, "sig", sig0[:130] + "00")),
        ("E_SIG_RANGE", v1gen.set_key(base_v1, "sig", "0x" + "0" * 64 + s_hex + v_hex)),
        ("E_SIG_HIGH_S", v1gen.set_key(base_v1, "sig", hi_s)),
        ("E_SIG_RECOVER", next(v1gen.set_key(base_v1, "sig", "0x" + r_hex[:i_] + d_ + r_hex[i_ + 1:] + s_hex + v_hex)
                               for i_ in range(64) for d_ in "0123456789abcdef" if d_ != r_hex[i_]
                               and v1_check(v1gen.set_key(base_v1, "sig", "0x" + r_hex[:i_] + d_ + r_hex[i_ + 1:] + s_hex + v_hex))[1] == "E_SIG_RECOVER")),
        ("E_SIG_SIGNER", make_entry_v1(K("root"), "annotation", 1, V1I[0], ANNB("signed by another"), author=K2)),
    ]
    for i_, (tok, b) in enumerate(v1_tokens):
        _ok, actual = v1_check(b)
        assert not _ok, tok
        A("continue_old_segment_v1_token_" + tok.lower(),
          inp(R, V1 + V2 + [b], CA + [anchor(eid(b), R, bn=11, ts=T + 1, tx=HX32(0x9A0 + i_))], bas=TWO),
          "sec 12.6 with zikaron/1 sec 4.3: a zikaron/1-spec byte string anchored before the cut "
          "is an old byte string of both segments; the old report lists it MALFORMED at %s and "
          "its record MISSING, the new report at E_SPEC." % actual, "GAPS")
    # the zikaron/1 body rows on both sides, each on the closing succession's line (E1)
    HISB1 = {"content": HX32(0xC1), "mode": {"mark": "hand", "toolchain": HX32(0x7001)}}
    GRB1 = {"grantee": HX20(0xBEEF), "work": HX32(0xC0), "terms": HX32(0x7E)}
    ADB1 = {"anchors": [{"chainId": 1, "tx": HX32(0xAA), "payloadKind": "calldata", "content": HX32(0xC0)}]}
    v1_rows = [
        ("history_mode_absent", "history", {"content": HX32(0xC1)}, False),
        ("history_mark_absent", "history", {"content": HX32(0xC1), "mode": {"toolchain": HX32(0x7001)}}, False),
        ("history_toolchain_absent", "history", {"content": HX32(0xC1), "mode": {"mark": "hand"}}, False),
        ("history_ok", "history", HISB1, True),
        ("grant_no_window_ok", "grant", GRB1, True),
        ("grant_window_ok", "grant", dict(GRB1, window={"from": 1, "to": 5}), True),
        ("grant_window_not_object", "grant", dict(GRB1, window=5), False),
        ("grant_window_from_absent", "grant", dict(GRB1, window={"to": 5}), False),
        ("grant_window_to_absent", "grant", dict(GRB1, window={"from": 1}), False),
        ("adoption_element_not_object", "adoption", {"anchors": [5]}, False),
        ("adoption_element_chainid_absent", "adoption", {"anchors": [{"tx": HX32(0xAA), "payloadKind": "calldata", "content": HX32(0xC0)}]}, False),
        ("adoption_element_payloadkind_absent", "adoption", {"anchors": [{"chainId": 1, "tx": HX32(0xAA), "content": HX32(0xC0)}]}, False),
        ("adoption_element_content_absent", "adoption", {"anchors": [{"chainId": 1, "tx": HX32(0xAA), "payloadKind": "calldata"}]}, False),
        ("adoption_attestor_without_attestation", "adoption", dict(ADB1, attestor=HX20(0xA77)), False),
        ("succession_statement_not_string", "succession", dict(SUCB(K2), statement_md=5), False),
        ("revocation_ok", "revocation", {"grant": HX32(0xAB)}, True),
        ("history_note_not_string", "history", dict(HISB1, note_md=5), False),
        ("grant_history_not_hex32", "grant", dict(GRB1, history=5), False),
        ("grant_scope_not_string", "grant", dict(GRB1, scope_md=5), False),
        ("revocation_case_not_hex32", "revocation", {"grant": HX32(0xAB), "case": 5}, False),
        ("annotation_subject_not_hex32", "annotation", {"note_md": "fine", "subject": 5}, False),
        ("annotation_ok", "annotation", {"note_md": "fine"}, True),
    ]
    for i_, (slug, et, body, ok) in enumerate(v1_rows):
        e1 = mkv1("root", et, 1, V1I[0], body)
        _ok, actual = v1_check(e1)
        assert _ok == ok, slug
        s_r = mkv1("root", "succession", 2, eid(e1), SUCB(R, kind="migration"))
        g_r = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": eid(s_r)}})
        recs = [CA[0], anchor(eid(e1), R, bn=11, ts=T + 1, tx=HX32(0x9C0 + i_)), anchor(eid(s_r), R, bn=12, ts=T + 2, tx=HX32(0x9E0 + i_)),
                anchor(eid(g_r), R, bn=13, ts=T + 3, tx=HX32(0xA00 + i_))]
        if ok:
            A("continue_old_segment_v1_row_" + slug, inp(R, [V1[0], e1, s_r, g_r], recs, bas=TWO),
              "sec 12.6 with zikaron/1 sec 6: a zikaron/1 %s whose body passes its row (%s), on the "
              "closing succession's line; a ledger entry of the old segment." % (et, slug.replace("_", " ")), "COMPLETE")
        else:
            A("continue_old_segment_v1_row_" + slug, inp(R, [V1[0], e1, s_r, g_r], recs, bas=TWO),
              ("sec 12.6 with zikaron/1 sec 6: a zikaron/1 %s whose body fails its row (%s; %s), anchored "
               "before the cut; MALFORMED in the old report with its record MISSING, and the "
               "closing succession's line stops before it (PREV_MISMATCH against the genesis is "
               "no finding, the position holding no ledger entry; SEQ_GAP).") % (et, actual, slug.replace("_", " ")), "GAPS")
    # -- round 3 fold --------------------------------------------------------
    gK9v1 = mkv1("k9", "genesis", 0, None, GENB)
    XR = [anchor(eid(gK9v1), K9, bn=11, ts=T + 1, tx=HX32(0xA20))]
    A("continue_handed_off_key_reanchors_excluded_old_bytes",
      inp(R, hand + [g2k, a2k, gK9v1], HA + XR + [anchor(eid(gK9v1), R, bn=50, ts=T + 50, tx=HX32(0xA21))], bas=TWO),
      "sec 12.6: a stranger's zikaron/1 genesis anchored before the cut is an old byte string, "
      "EXCLUDED there; the old root key, handed off under zikaron/1, re-anchors its hash after "
      "the cut. The bytes are no old ledger entry, so the record is placed by time into the "
      "new segment, where the root is outside the lineage: DISCARDED, nothing MISSING.", "COMPLETE")
    s2k3 = mk("k2", "succession", 2, eid(a2k), SUCB(K3))
    A("continue_retired_author_reanchors_excluded_old_bytes",
      inp(R, hand + [g2k, a2k, s2k3, gK9v1],
          HA + XR + [anchor(eid(s2k3), K2, bn=16, ts=T + 6), anchor(eid(gK9v1), K2, bn=60, ts=T + 60, tx=HX32(0xA22))], bas=TWO),
      "sec 12.6/7.4: the continuation's author hands the new seat on at T+6 and re-anchors the "
      "excluded old bytes at T+60; a new record of a retired key, DISCARDED.", "COMPLETE")
    A("continue_unproven_record_of_excluded_old_bytes_in_hand",
      inp(R, hand + [g2k, a2k, gK9v1], HA + XR + [anchor(eid(gK9v1), K2, bn=50, ts=T + 50, tx=HX32(0xA23), verdict="UNPROVEN")], bas=TWO),
      "sec 8.6 item 15 rule 2 reads the whole pile: the key in office's late UNPROVEN record of "
      "the excluded old bytes is a new record over bytes in hand, and moves no label.", "COMPLETE")
    A("continue_in_office_key_reanchors_excluded_old_bytes",
      inp(R, hand + [g2k, a2k, gK9v1], HA + XR + [anchor(eid(gK9v1), K2, bn=50, ts=T + 50, tx=HX32(0xA24))], bas=TWO),
      "sec 12.6/8.5: the key in office commits, after the cut, to bytes that are no ledger "
      "entry of either segment; the record is new, counted, and MISSING for good.", "GAPS")
    hb = [mkv1("root", "genesis", 0, None, GENB)]
    hb.append(mkv1("root", "succession", 1, eid(hb[0]), SUCB(K2)))
    hb.append(mkv1("k2", "succession", 2, eid(hb[1]), SUCB(R)))
    hb.append(mkv1("root", "succession", 3, eid(hb[2]), SUCB(R, kind="migration")))
    g_hb = mk("root", "genesis", 0, None, {"statement_md": "handed back and closed", "continues": {"root": R, "entry_id": eid(hb[3])}})
    HBA = [anchor(eid(hb[i]), R if i != 2 else K2, bn=10 + i, ts=T + i, tx=HX32(0xA30 + i)) for i in range(4)] + [anchor(eid(g_hb), R, bn=14, ts=T + 4, tx=HX32(0xA34))]
    A("continue_handback_key_closes_nothing",
      inp(R, hb + [g_hb], HBA, bas=TWO),
      "sec 12.6: the seat returned under zikaron/1 to the root after its handing off was anchored "
      "at T+1; the root's closing succession at T+3 makes no candidate, the ledger is read as an "
      "uncontinued root, and the zikaron/1 bytes are MALFORMED with their records MISSING.", "GAPS")
    A("continue_handback_handover_unanchored_continues",
      inp(R, hb + [g_hb], [HBA[0], HBA[2], HBA[3], HBA[4]], bas=TWO),
      "sec 12.6: the same ledger with the root's handing off anchored by nobody; no record earlier "
      "than the cut shows it, so the root's closing succession continues the ledger.", "COMPLETE")
    sr = [mkv1("root", "genesis", 0, None, GENB)]
    sr.append(mkv1("root", "succession", 1, eid(sr[0]), SUCB(R)))
    sr.append(mkv1("root", "succession", 2, eid(sr[1]), SUCB(R, kind="migration")))
    g_sr = mk("root", "genesis", 0, None, {"statement_md": "after a rotation to itself", "continues": {"root": R, "entry_id": eid(sr[2])}})
    A("continue_self_succession_is_no_handing_off",
      inp(R, sr + [g_sr], [anchor(eid(sr[i]), R, bn=10 + i, ts=T + i, tx=HX32(0xA40 + i)) for i in range(3)] + [anchor(eid(g_sr), R, bn=13, ts=T + 3, tx=HX32(0xA43))], bas=TWO),
      "sec 12.6: a zikaron/1 succession to itself, anchored before the cut, hands nothing off; "
      "the root closes and continues.", "COMPLETE")
    HOFF = [HA[0], anchor(eid(s1), R, bn=12, ts=T + 2, tx=HX32(0x980)), anchor(eid(g1), R, bn=13, ts=T + 3, tx=HX32(0x981)),
            anchor(HI[2], K2, bn=15, ts=T + 5), anchor(eid(g2k), K2, bn=16, ts=T + 6), anchor(eid(a2k), K2, bn=17, ts=T + 7)]
    A("continue_handing_off_recorded_by_stranger_blocks",
      inp(R, hand + [g2k, a2k, s1, g1], HOFF + [anchor(HI[1], K9, bn=11, ts=T + 1, tx=HX32(0xA50))], bas=TWO),
      "sec 12.6: a handing off is a record whatever its sender; the stranger's record of the "
      "root's handover at T+1 bars the stolen key's closing succession.", "BROKEN_CHAIN")
    A("continue_handing_off_recorded_unproven_blocks",
      inp(R, hand + [g2k, a2k, s1, g1], HOFF + [anchor(HI[1], R, bn=11, ts=T + 1, tx=HX32(0xA51), verdict="UNPROVEN")], bas=TWO),
      "sec 12.6: a handing off is a record whatever its verdict; UNPROVEN.", "BROKEN_CHAIN")
    A("continue_handing_off_recorded_void_blocks",
      inp(R, hand + [g2k, a2k, s1, g1], HOFF + [anchor(HI[1], R, bn=11, ts=T + 1, tx=HX32(0xA52), verdict="VOID")], bas=TWO),
      "sec 12.6: a handing off is a record whatever its verdict; VOID.", "BROKEN_CHAIN")
    A("continue_handing_off_recorded_on_other_chain_blocks",
      inp(R, hand + [g2k, a2k, s1, g1], HOFF + [anchor(HI[1], R, cid=8453, bn=5, ts=T + 1, tx=HX32(0xA53))], bas=TWO),
      "sec 12.6: a handing off is a record on any chain; the handover anchored on another chain "
      "than the closing succession's bars it all the same.", "BROKEN_CHAIN")
    # -- round 4 fold --------------------------------------------------------
    sx = mkv1("root", "succession", 1, V1I[0], SUCB(K3))
    gX = mkv1("k3", "genesis", 0, None, GENB)
    A("continue_old_lineage_reads_old_byte_strings_alone",
      inp(R, V1 + V2 + [sx, gX], CA + [anchor(eid(gX), K3, bn=11, ts=T + 1, tx=HX32(0xA60)), anchor(eid(gX), R, bn=50, ts=T + 50, tx=HX32(0xA61))], bas=TWO),
      "sec 12.6: the root's zikaron/1 succession to k3 is held but anchored by nobody and off "
      "the closing succession's line, so it is no old byte string and k3 is outside the old "
      "ledger's lineage; k3's genesis, anchored before the cut, is an EXCLUDED old byte "
      "string and no old ledger entry, so the root's late counted record of it is a new "
      "record, MISSING.", "GAPS")
    ann3 = mkv1("root", "annotation", 3, V1I[2], ANNB("after the close under zikaron/1"))
    A("continue_old_ledger_entry_off_line_reanchored_late",
      inp(R, V1 + V2 + [ann3], CA + [anchor(eid(ann3), R, bn=12, ts=T + 2, tx=HX32(0xA62)), anchor(eid(ann3), R, cid=8453, bn=20, ts=T + 20, tx=HX32(0xA63))], bas=TWO),
      "sec 12.6: a zikaron/1 annotation after the closing succession, anchored in the cut's "
      "second, is an old byte string off the line and an old ledger entry; its late record on "
      "another chain is an old record.", "COMPLETE")
    A("continue_old_lineage_seeded_by_root",
      inp(R, hand + [g2k, a2k], HA + [anchor(HI[0], K2, cid=8453, bn=20, ts=T + 20, tx=HX32(0xA64))], bas=TWO),
      "sec 12.6: the continuation's author re-anchors the old root's genesis late on another "
      "chain; the old ledger is seeded by the root, so the genesis is an old ledger entry and "
      "the record an old record, counted there since k2 is of the old lineage.", "COMPLETE")
    A("continue_unproven_record_of_entry_of_both_segments",
      inp(R, V1 + V2, CA[:3] + [anchor(eid(g2), R, bn=13, ts=T + 3), anchor(eid(a2), R, bn=11, ts=T + 1, tx=HX32(0xA65)),
                               anchor(eid(a2), R, bn=50, ts=T + 50, tx=HX32(0xA66), verdict="UNPROVEN")], bas=TWO),
      "sec 8.6 item 15 rule 2 with sec 12.6: the annotation anchored before the cut is a byte "
      "string of both segments; its late UNPROVEN record is a new record over bytes in hand "
      "and moves no label; GAPS comes from the old report's MISSING over the same bytes.", "GAPS")
    TWOW = basis((chainobj(1, 0, 500, senders=(R, K2, K3, K4)), chainobj(1, 501, 1000, senders=(R, K2, K3, K4, K9)),
                  chainobj(8453, senders=(R, K2, K3, K4, K9))))
    A("continue_basis_two_windows_adjacent",
      inp(R, V1 + V2, CA, bas=TWOW),
      "sec 9.4 through sec 12.6: a basis with two chains objects of one chainId whose ranges are "
      "adjacent and whose senders differ is a zikaron/2 basis, and the old segment receives it "
      "whole, spelled as zikaron/1 sec 9.4 spells it.", "COMPLETE")
    TWOG = basis((chainobj(1, 0, 500, senders=(R, K2, K3, K4, K9)), chainobj(1, 502, 1000, senders=(R, K2, K3, K4, K9)),
                  chainobj(8453, senders=(R, K2, K3, K4, K9))))
    A("continue_basis_two_windows_with_gap",
      inp(R, V1 + V2, CA, bas=TWOG),
      "sec 9.4 through sec 12.6: two chains objects of one chainId with a block between them; "
      "ordinary under both laws.", "COMPLETE")
    gK = mkv1("k2", "genesis", 0, None, GENB)
    A("continue_old_segment_root_mismatch",
      inp(R, hand + [g2k, a2k, gK], HA + [anchor(eid(gK), K2, bn=11, ts=T + 1, tx=HX32(0x96b))], bas=TWO),
      "sec 12.6 with zikaron/1 sec 8.2: a zikaron/1 genesis by k2, a key of that lineage, "
      "anchored before the cut, is an old byte string; zikaron/1 records ROOT_MISMATCH and "
      "the equivocation at seq 0.", "BROKEN_CHAIN")
    A("continue_old_segment_v1_body_failure_anchored",
      inp(R, V1 + V2 + [bad_s], CA + [anchor(eid(bad_s), R, bn=11, ts=T + 1, tx=HX32(0x96c))], bas=TWO),
      "sec 12.6: a byte string passing neither law, anchored before the cut, is a byte string "
      "of both segments; zikaron/1 lists it MALFORMED at its body row and its record MISSING, "
      "this law lists it MALFORMED at E_SPEC.", "GAPS")
    uw = mkv1("root", "windmill", 1, V1I[0], {})
    s_uw = mkv1("root", "succession", 2, eid(uw), SUCB(R, kind="migration"))
    g_uw = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": eid(s_uw)}})
    A("continue_old_segment_unknown_type",
      inp(R, [V1[0], uw, s_uw, g_uw],
          [CA[0], anchor(eid(uw), R, bn=11, ts=T + 1, tx=HX32(0x96d)), anchor(eid(s_uw), R, bn=12, ts=T + 2, tx=HX32(0x96e)),
           anchor(eid(g_uw), R, bn=13, ts=T + 3, tx=HX32(0x96f))], bas=TWO),
      "sec 12.6 with zikaron/1 sec 8.7: an entry of a type zikaron/1 does not know, on the "
      "closing succession's line, is an old byte string and an UNKNOWN_TYPE row of that "
      "report; no label turns on it.", "COMPLETE")
    ad = mkv1("root", "adoption", 1, V1I[0], {"anchors": [{"chainId": 1, "tx": HX32(0xAA), "payloadKind": "calldata", "content": HX32(0xC0)}]})
    s_ad = mkv1("root", "succession", 2, eid(ad), SUCB(R, kind="migration"))
    g_ad = mk("root", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": R, "entry_id": eid(s_ad)}})
    A("continue_old_segment_adoption_unproven",
      inp(R, [V1[0], ad, s_ad, g_ad],
          [CA[0], anchor(eid(ad), R, bn=11, ts=T + 1, tx=HX32(0x970)), anchor(eid(s_ad), R, bn=12, ts=T + 2, tx=HX32(0x971)),
           anchor(eid(g_ad), R, bn=13, ts=T + 3, tx=HX32(0x972))], bas=TWO),
      "sec 12.6: the old segment carries the empty adoption evidence set, so zikaron/1 sec 9.5 "
      "lists every element of the ledger adoption in ADOPTION_UNPROVEN, informational; "
      "COMPLETE.", "COMPLETE")
    A("adoption_is_an_unknown_type",
      inp(R, [RT[0], mk("root", "adoption", 1, RTI[0], {"anchors": []})]),
      "sec 8.6 item 9: adoption is none of the eight types of this law; an UNKNOWN_TYPE row "
      "and no label turns on it.", "COMPLETE")
    c0 = mk("root", "genesis", 0, None, GENB)
    c1 = mk("root", "succession", 1, eid(c0), SUCB(K2))
    c2 = mk("k2", "succession", 2, eid(c1), SUCB(K3))
    c3 = mk("k2", "migration", 3, eid(c2), MIGB())
    c4 = mk("k3", "annotation", 4, eid(c3), ANNB("k3 in office"))
    A("migration_out_of_office_closes_nothing",
      inp(R, [c0, c1, c2, c3, c4]),
      "sec 8.2: a migration signed by a key that is not auth at its position is an authority "
      "fault and closes nothing; the entry after it, by the key in office, records no "
      "finding.", "BROKEN_CHAIN")
    A("unproven_record_over_retired_twin_bytes",
      inp(R, RT + [late], BA + [anchor(eid(late), K2, bn=15, ts=T + 5, tx=HX32(0x973), verdict="UNPROVEN")], bas=TWO),
      "sec 8.6 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of a RETIRED "
      "entry is over bytes in hand (item 8 answers the clause); COMPLETE with one RETIRED "
      "and one UNPROVEN row.", "COMPLETE")
    A("retired_key_late_unproven_and_void_discarded",
      inp(R, RT, BA + [anchor(H_UNSEEN, R, bn=15, ts=T + 3, tx=HX32(0x974), verdict="UNPROVEN"),
                       anchor(HX32(0xBEEF2), R, bn=16, ts=T + 4, tx=HX32(0x975), verdict="VOID")], bas=TWO),
      "sec 8.1: the trim on the retired ground reaches records of every verdict; the root's "
      "UNPROVEN and VOID records after its retirement are DISCARDED rows and enter no list.",
      "COMPLETE")
    A("input_anchor_timestamp_string",
      inp(R, CL, [dict(anchor(CID[0], R), blockTimestamp="1700000000")], bas=ROOTB),
      "sec 9.2: blockTimestamp is an int; a string is no anchor record and the input has no "
      "label.", "NO_LABEL")
    over = mk("k9", "annotation", 4, eid(late), ANNB("a stranger builds on the twin"))
    A("retired_twin_timely_through_stranger_anchored_in_time",
      inp(R, RT + [late, over], BA + [anchor(eid(over), K9, bn=11, ts=T + 1, tx=HX32(0x976))], bas=TWO),
      "sec 7.4: timeliness runs through byte strings of any author; the stranger's entry, "
      "anchored a second before the root's retirement, bounds the twin, which is a ledger entry "
      "and a fork; the stranger's entry is EXCLUDED and its record DISCARDED.", "BROKEN_CHAIN")
    A("retired_twin_not_timely_through_stranger_in_retirement_second",
      inp(R, RT + [late, over], BA + [anchor(eid(over), K9, bn=12, ts=T + 2, tx=HX32(0x978))], bas=TWO),
      "sec 7.4: the same entry anchored in the retirement's own second; the twin sits above the "
      "root's retiring seq, so no record of that second makes it timely, whatever that record "
      "anchors: RETIRED, no fork; the stranger's entry EXCLUDED, its record DISCARDED.", "COMPLETE")
    k2over = mk("k2", "annotation", 4, eid(late), ANNB("the successor builds on the twin"))
    A("retired_twin_through_successor_entry_in_retirement_second",
      inp(R, RT + [late, k2over], BA + [anchor(eid(k2over), K2, bn=12, ts=T + 2, tx=HX32(0x979))], bas=TWO),
      "sec 7.4: the successor's seq-4 entry names the root's seq-3 twin as prev and is anchored "
      "in the retirement's second; the successor has no retirement, so its entry is timely and a "
      "ledger entry, while the twin stays RETIRED: the entry's prev names no ledger entry, a "
      "hard finding at 4.", "BROKEN_CHAIN")
    A("retired_late_succession_through_named_key_entry_by_old_key_in_retirement_second",
      inp(R, RT + [lsucc, stranger], BA + [anchor(eid(stranger), R, bn=12, ts=T + 2, tx=HX32(0x97A))], bas=TWO),
      "sec 7.4/8.5: in the second of its handing off the root anchors the entry of the key its "
      "late succession names, whose prev is that succession; the record anchors what another key "
      "signed and counts, the succession above the retiring seq is made timely by no record of "
      "that second and admits nobody, the named key's entry is EXCLUDED and the record's hash "
      "MISSING.", "GAPS")
    low1 = mk("root", "annotation", 1, eid(late), ANNB("the old key, low, naming its late twin"))
    A("retire_same_second_own_low_entry_naming_late_twin",
      inp(R, RT + [late, low1], BA + [anchor(eid(low1), R, bn=12, ts=T + 2, tx=HX32(0x97B))], bas=TWO),
      "sec 7.4: in the second of its handing off the root anchors its own seq-1 entry whose prev "
      "is its seq-3 twin; the entry sits at or before the retiring seq and is timely, a twin of "
      "the history at 1 and a fork there, while the seq-3 twin on its line stays RETIRED.",
      "BROKEN_CHAIN")
    A("retired_twin_through_stranger_cross_chain_in_retirement_second",
      inp(R, RT + [late, over], BA + [anchor(eid(over), K9, cid=8453, bn=5, ts=T + 2, tx=HX32(0x97C))], bas=TWO),
      "sec 7.4: the stranger's entry over the twin anchored on another chain at the retirement's "
      "block time; the twin above the retiring seq is RETIRED on whichever chain the record "
      "sits.", "COMPLETE")
    A("retired_twin_through_stranger_one_second_after",
      inp(R, RT + [late, over], BA + [anchor(eid(over), K9, bn=13, ts=T + 3, tx=HX32(0x97D))], bas=TWO),
      "sec 7.4: the stranger's entry over the twin anchored a second after the retirement bounds "
      "nothing of the root's; the twin RETIRED, the record DISCARDED.", "COMPLETE")
    A("retire_record_before_retirement_anchors_successor_entry",
      inp(R, RT, BA + [anchor(RTI[3], R, bn=11, ts=T + 1, tx=HX32(0x97E))], bas=TWO),
      "sec 7.4/8.1: a second before its retirement the root anchors the successor's seq-3 entry; "
      "the root has not retired at the record, it counts, and the entry is anchored twice.",
      "COMPLETE")
    s3 = mk("k9", "annotation", 3, RTI[2], ANNB("a stranger at seq 3"))
    A("retire_record_before_retirement_anchors_stranger_entry",
      inp(R, RT + [s3], BA + [anchor(eid(s3), R, bn=11, ts=T + 1, tx=HX32(0x97F))], bas=TWO),
      "sec 7.4/8.5: a second before its retirement the root anchors a stranger's seq-3 entry; the "
      "record counts and its hash is MISSING, the stranger's entry EXCLUDED.", "GAPS")
    # each byte string on a line is read against its own author's retirement and retiring seq
    N0, N0I = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(K2))])
    n2 = mk("root", "annotation", 2, N0I[1], ANNB("the old key after handing off at 1"))
    e3 = mk("k2", "annotation", 3, eid(n2), ANNB("the successor builds on the old key's late entry"))
    s4 = mk("k2", "succession", 4, eid(e3), SUCB(K3))
    A("retired_late_entry_of_earlier_key_on_successor_handoff_line",
      inp(R, N0 + [n2, e3, s4], [anchor(N0I[0], R, bn=10, ts=T), anchor(N0I[1], R, bn=11, ts=T + 1),
                                  anchor(eid(s4), K2, bn=15, ts=T + 5)], bas=TWO),
      "sec 7.4: timeliness reads each byte string on a line against its own author's retirement "
      "and retiring seq; the successor's handing off at seq 4, anchored at T+5, makes its own "
      "seq-3 entry timely, while the root's seq-2 entry on that line, above the root's retiring "
      "seq 1 and reached by no record earlier than the root's retirement at T+1, stays RETIRED: "
      "a gap at 2.", "GAPS")
    rogue1 = mk("root", "succession", 1, RTI[0], SUCB(K9, kind="handover-rogue"))
    A("retire_same_second_lower_rogue_handoff_genuine_entry_reached_through_genuine_handoff",
      inp(R, RT + [rogue1], [BA[0], BA[2], BA[3], anchor(eid(rogue1), K9, bn=12, ts=T + 2, tx=HX32(0x9D1))], bas=TWO),
      "sec 7.4: a rogue succession of the root at seq 1, anchored by the key it names in the "
      "second of the genuine handing off at seq 2, lowers the retiring seq to 1; the genuine "
      "history at seq 1, reached only through the genuine succession's record of that second, "
      "sits at the retiring seq and is timely, a fork at 1 with the rogue; the genuine succession "
      "above the retiring seq admits nobody and the successor is EXCLUDED.", "BROKEN_CHAIN")
    bridge = mk("k9", "history", 4, eid(late), {})
    A("retired_twin_not_reached_through_malformed_bridge",
      inp(R, RT + [late, bridge], BA + [anchor(eid(bridge), K9, bn=11, ts=T + 1, tx=HX32(0x977))], bas=TWO),
      "sec 7.4: timeliness reads records whose hash is the entry_id of a byte string that passes "
      "sec 4.3; a near-entry over the twin failing its body row, anchored by a stranger a second "
      "before the root's retirement, is no such byte string and bounds nothing, so the twin "
      "stays RETIRED, the bridge is MALFORMED and the record DISCARDED, its sender outside the "
      "lineage.", "COMPLETE")
    over2 = mk("k9", "annotation", 5, eid(bridge), ANNB("a stranger builds on the malformed bridge"))
    A("retired_twin_not_reached_through_malformed_middle",
      inp(R, RT + [late, bridge, over2], BA + [anchor(eid(over2), K9, bn=11, ts=T + 1, tx=HX32(0x9B1))], bas=TWO),
      "sec 7.4: the path runs along prev through byte strings that pass sec 4.3; the stranger's "
      "entry anchored a second before the root's retirement names the malformed bridge as prev, "
      "and the line stops there, so the twin behind it is reached by nothing and stays RETIRED; "
      "the stranger's entry EXCLUDED, the bridge MALFORMED, the record DISCARDED.", "COMPLETE")
    A("retire_record_before_retirement_anchors_own_late_twin",
      inp(R, RT + [late], BA + [anchor(eid(late), R, bn=11, ts=T + 1, tx=HX32(0x9B4))], bas=TWO),
      "sec 7.4/8.4: a second before its retirement the root anchors its own seq-3 twin of the "
      "successor's entry, above the retiring seq it is yet to have; a key has retired at no record "
      "earlier than its retirement, so the record counts, and it bounds the twin, which is a "
      "ledger entry: a fork at 3 and an authority fault, both hard.", "BROKEN_CHAIN")
    tw1 = mk("root", "succession", 1, RTI[0], SUCB(K3, kind="handover-twin"))
    A("retire_lower_handoff_anchored_after_the_second_sets_no_retiring_seq",
      inp(R, RT + [tw1], BA + [anchor(eid(tw1), R, bn=15, ts=T + 5, tx=HX32(0x9B5))], bas=TWO),
      "sec 7.4: the retiring seq is taken over the records of the retirement's own second alone; "
      "the root's succession at seq 1, a twin of its history, is anchored three seconds after its "
      "retirement and leaves the retiring seq at 2, so the succession at 2, bounded only by its "
      "own record of that second, is timely and k2 is in the lineage; the seq-1 twin, bounded by "
      "nothing in time, RETIRED, and the root's later record DISCARDED.", "COMPLETE")
    bads1 = sub(mk("root", "succession", 1, RTI[0], SUCB(K3, kind="handover-twin")), b"handover-twin", b"handover-TWIN")
    A("retire_same_second_malformed_lower_handoff_sets_no_retiring_seq",
      inp(R, RT + [bads1], BA + [anchor(eid(bads1), R, bn=12, ts=T + 2, tx=HX32(0x9B6))], bas=TWO),
      "sec 7.4: the retiring seq is taken over byte strings that pass sec 4.3; bytes that spell a "
      "succession of the root at seq 1 and fail sec 4.3, anchored by the root in the second of "
      "its handing off, leave the retiring seq at 2: the succession at 2 is timely, the record "
      "over the failing bytes is one at which the root has not retired, it counts, its hash is "
      "MISSING and the bytes MALFORMED.", "GAPS")
    RT2, RT2I = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1)),
                             ("root", "succession", SUCB(K2)), ("k2", "annotation", ANNB("k2 writes")),
                             ("k2", "succession", SUCB(K3)), ("k3", "annotation", ANNB("k3 writes"))])
    BA2 = [anchor(h, (R, R, R, K2, K2, K3)[i], bn=10 + i, ts=T + i) for i, h in enumerate(RT2I)]
    l5 = mk("k2", "annotation", 5, RT2I[4], ANNB("the successor's key after its own handing off"))
    A("retire_successor_same_second_late_twin_by_its_key",
      inp(R, RT2 + [l5], BA2 + [anchor(eid(l5), K2, bn=14, ts=T + 4, tx=HX32(0x9B2))], bas=TWO),
      "sec 7.4/8.1: the successor hands off at seq 4, anchored at T+4, and in that second anchors "
      "its own twin of k3's entry at seq 5, above its retiring seq 4: the record is one at which "
      "the successor has retired, DISCARDED, and the twin, made timely by no record of that "
      "second, RETIRED; no fork.", "COMPLETE")
    y4 = mk("k2", "annotation", 4, RT2I[3], ANNB("the successor's own seq-4 twin"))
    l5b = mk("k2", "annotation", 5, eid(y4), ANNB("the successor above its retiring seq"))
    A("retire_successor_same_second_low_twin_reached_only_through_late_twin",
      inp(R, RT2 + [y4, l5b], BA2 + [anchor(eid(l5b), K2, bn=14, ts=T + 4, tx=HX32(0x9B3))], bas=TWO),
      "sec 7.4: the successor's seq-4 twin of its succession is reachable only from its seq-5 "
      "entry, which it anchors in the second of its own handing off; the record is one at which "
      "it has retired, DISCARDED, and timeliness reads that second by position against the "
      "successor's retiring seq: the seq-4 twin is timely, a fork with the succession, the "
      "seq-5 entry RETIRED.", "BROKEN_CHAIN")

    # ---------------- randomized ledgers with injected faults -----------------
    for i in range(24):
        L = rng.randrange(1, 8)
        specs = [("root", "genesis", GENB)]
        for j in range(1, L):
            et = rng.choice(["history", "grant", "annotation", "revocation", "retraction",
                             "succession", "windmill"])
            if et == "succession":
                specs.append(("root", et, SUCB(A_(rng.choice(["k2", "k3"])))))
            else:
                specs.append(("root", et, random_body(rng, et)))
        pile, ids = build_chain(specs)
        anchors = []
        faults = []
        for _ in range(rng.randrange(1, 4)):
            f = rng.choice(["drop", "twin", "bad_prev", "foreign", "reauthor", "dup",
                            "anchor_missing", "anchor_have", "anchor_unproven", "anchor_void",
                            "anchor_foreign_sender", "anchor_successions", "late_stray",
                            "migration_entry", "none"])
            faults.append(f)
            if f == "drop" and len(pile) > 1:
                pile.pop(rng.randrange(len(pile)))
            elif f == "twin" and pile:
                j = rng.randrange(len(pile))
                v = Parser(pile[j].decode()).text()
                pile.append(mk("root", v.get("entryType"), v.get("seq"), v.get("prev"),
                               random_body(rng, v.get("entryType"))
                               if v.get("entryType") != "succession"
                               else SUCB(A_("k3"))))
            elif f == "bad_prev" and len(pile) > 1:
                j = rng.randrange(1, len(pile))
                v = Parser(pile[j].decode()).text()
                pile[j] = mk("root", v.get("entryType"), v.get("seq"),
                             HX32(rng.getrandbits(256)),
                             {k: v2 for k, v2 in
                              [(kk, vv) for kk, vv in v.get("body").members]})
            elif f == "foreign":
                pile.append(mk("k9", "annotation", rng.randrange(0, 6),
                               HX32(rng.getrandbits(256)), ANNB("outside")) if rng.random() < 0.5
                            else mk("k9", "genesis", 0, None, GENB))
            elif f == "reauthor" and len(pile) > 1:
                j = rng.randrange(1, len(pile))
                v = Parser(pile[j].decode()).text()
                pile[j] = mk("k2", v.get("entryType"), v.get("seq"), v.get("prev"),
                             JObj(v.get("body").members))
            elif f == "dup" and pile:
                pile.append(pile[rng.randrange(len(pile))])
            elif f == "anchor_missing":
                anchors.append(anchor(HX32(rng.getrandbits(256)), A_("root"),
                                      bn=rng.randrange(1, 999)))
            elif f == "anchor_have" and ids:
                anchors.append(anchor(rng.choice(ids), A_("root"), bn=rng.randrange(1, 999)))
            elif f == "anchor_unproven":
                anchors.append(anchor(HX32(rng.getrandbits(256)), A_("root"),
                                      bn=rng.randrange(1, 999), verdict="UNPROVEN"))
            elif f == "anchor_void":
                anchors.append(anchor(HX32(rng.getrandbits(256)), A_("root"),
                                      bn=rng.randrange(1, 999), verdict="VOID"))
            elif f == "anchor_foreign_sender":
                anchors.append(anchor(HX32(rng.getrandbits(256)), A_("k9"),
                                      bn=rng.randrange(1, 999)))
            elif f == "anchor_successions":
                for b in pile:
                    v = Parser(b.decode()).text()
                    if v.get("entryType") in ("succession", "migration"):
                        anchors.append(anchor(eid(b), A_("root"), bn=rng.randrange(1, 999),
                                              ts=1700000000 + rng.randrange(0, 12)))
            elif f == "late_stray":
                anchors.append(anchor(HX32(rng.getrandbits(256)), A_("root"),
                                      bn=rng.randrange(1, 999), ts=1700000000 + rng.randrange(0, 24)))
            elif f == "migration_entry" and pile:
                v = Parser(pile[-1].decode()).text()
                pile.append(mk("root", "migration", v.get("seq") + 1, eid(pile[-1]), MIGB()))
        rng.shuffle(pile)
        A("rand_ledger_%02d" % i, inp(R, pile, anchors, ROOTB),
          "randomized ledger of %d written entries with injected faults: %s; the faults reach "
          "sec 8.1 (lineage, retirement), sec 8.2 (walk), sec 8.4 (equivocation), sec 8.5 "
          "(reconciliation) and sec 8.6 (lists and label) as they fall" % (L, ", ".join(faults)))


# ==========================================================================
# other readings: points the law decides where a reader need not branch
# ==========================================================================

def build_readings(t):
    """Cases that tell the law's reading from another a careful reader could
    take, at points a left-to-right reader passes without a branch, so that
    the cases above reach them by chance or not at all. None draws from the
    seed's stream: every seed carries each of them, and they are registered
    after every other canon and audit case, whose names they leave as they
    were.

    Provenance. This section answers external issue kaptonia/zikaron#5
    (2026-11-11) and is the one part of this file drafted by an author who
    had read impl2-py. Each expectation in it was then read from the law
    text alone, clause by clause, by a reader who took nothing from any
    implementation, and each stands on the clause named here:

      depth_refused_bracket_*, depth_admitted_bracket_*
          zikaron/2 sec 3.5 test 2: "a bracket the grammar refuses opens no
          container, so E_DEPTH shares no triggering byte with a grammar
          failure"; a bracket at a value position is admitted, opens the
          129th container and names E_DEPTH at that byte.
      step3_before_step4, step7_before_step8, step8_before_step9
          sec 4.3: the tests are decided in the written order and the first
          failure's token is reported.
      step8_form_before_relation
          sec 4.3, closing paragraph: where a step names more than one
          token, its tests are decided in the order the tokens are written;
          step 8 writes E_PREV before E_PREV_SEQ.
      sig_v_beats_range, sig_v_beats_high_s
          sec 4.3 step 13: the signature tokens are decided in the bullet
          order of sec 5.4, whose first bullet is v.
      pile_*
          sec 8 (the pile is a multiset of candidate byte strings) and sec
          8.1 (byte-identical copies collapse to one, whether or not they
          are entries), with HARNESS2.md's pile encoding (hexadecimal digits
          of either case spell one byte string); sec 8.4 (byte-identical
          copies never equivocate) and sec 8.6 items 7 to 10 for the rows.
      seat_moves_*, ceding_key_graded_soft_after_its_gap_succession
          sec 8.2: authority is walked state; after every entry at
          position k has been visited, a succession whose author equals
          auth for position k moves auth for the positions after k, and the
          sentence carries no condition on `certain`; an AUTHORITY_MISMATCH
          recorded after a gap is soft.
      continue_old_*
          sec 12.6: the audit of a continued ledger is the zikaron/1 audit
          over the old segment; zikaron/1 sec 8.1 collapses byte-identical
          copies and zikaron/1 sec 8.2 moves the seat after every entry at
          a position has been visited, each as this law does."""
    R, K2, K3, K9 = A_("root"), A_("k2"), A_("k3"), A_("k9")

    # ---------------- sec 3.5 test 2: a refused bracket opens no container ---
    A127, A128, O127 = b"[" * 127, b"[" * 128, b'{"a":' * 127
    for slug, data, where in (
            ("member_name_object", A127 + b"{{", "where a member name is expected"),
            ("member_name_array", A127 + b"{[", "where a member name is expected"),
            ("member_name_inside_objects", O127 + b"{{", "where a member name is expected, the 128 "
                                                         "containers being objects"),
            ("member_name_after_comma", A127 + b'{"a":1,{', "where a member name is expected after a "
                                                              "member's comma"),
            ("colon_object", A127 + b'{"a"{', "where a member's colon is expected"),
            ("colon_array", A127 + b'{"a"[', "where a member's colon is expected"),
            ("after_member_object", A127 + b'{"a":1{', "after a member's value"),
            ("after_member_array", A127 + b'{"a":1[', "after a member's value"),
            ("after_element_object", A128 + b"1{", "after an array's element"),
            ("after_element_array", A128 + b"1[", "after an array's element")):
        C("depth_refused_bracket_" + slug, data,
          "sec 3.5 test 2: 128 containers are open and a bracket stands %s; the grammar refuses "
          "the byte, a bracket the grammar refuses opens no container, and the fault named is "
          "the grammar's, never E_DEPTH" % where, expect="E_JSON")
    for slug, data, where in (
            ("after_member_comma", A127 + b'{"a":1,"b":[', "after the colon of an object's second member"),
            ("after_element_comma_array", A128 + b"1,[", "after an array's comma"),
            ("after_element_comma_object", A128 + b"1,{", "after an array's comma")):
        C("depth_admitted_bracket_" + slug, data,
          "sec 3.5 test 2: 128 containers are open and a bracket stands at a value position, %s; "
          "the grammar admits it, the container it opens is the 129th, and the fault is named "
          "at that byte" % where, expect="E_DEPTH")

    # ---------------- sec 4.3: steps the cases above leave unordered ---------
    C("step3_before_step4", add_key(set_key(t.ann, "spec", "zikaron/3"), "extra", 1),
      "sec 4.3: an eighth key and a spec this law does not name; step 3 precedes step 4",
      expect="E_ENVELOPE_CLOSED")
    C("step7_before_step8", set_key(set_key(t.ann, "seq", "1"), "prev", 7),
      "sec 4.3: a seq that is no integer and a prev that is neither null nor a hex32; step 7 "
      "precedes step 8", expect="E_SEQ")
    C("step8_form_before_relation", set_key(set_key(t.ann, "seq", 0), "prev", "0x12"),
      "sec 4.3 step 8: at seq 0 a prev that is neither null nor a hex32; the form is decided "
      "before the null-iff-0 relation, which this prev breaks too", expect="E_PREV")
    C("step8_before_step9", set_key(set_key(t.ann, "prev", None), "body", []),
      "sec 4.3: a null prev at seq 1 and a body that is no object; step 8 precedes step 9",
      expect="E_PREV_SEQ")
    good = Parser(t.ann.decode()).text().get("sig")
    gr, gs = int(good[2:66], 16), int(good[66:130], 16)
    C("sig_v_beats_range", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(0, gs, 0)),
      "sec 5.4: v outside {27, 28} and r = 0; the v bullet is decided before the range",
      expect="E_SIG_V")
    C("sig_v_beats_high_s", craft("annotation", 1, t.gid, {"note_md": "ZZMARKZZ"}, sig_hex(gr, N - 1, 29)),
      "sec 5.4: v outside {27, 28} and a high s; the v bullet is decided before the low-s test",
      expect="E_SIG_V")

    # ---------------- sec 8.1: the pile is a multiset of byte strings --------
    CL, _ = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1)),
                         ("root", "grant", GRB(2)), ("root", "annotation", ANNB("a note")),
                         ("root", "revocation", RVB(4))])

    def upper(b):
        return "0x" + b.hex().upper()

    def mixed(b):
        return "0x" + "".join(c.upper() if i % 2 else c for i, c in enumerate(b.hex()))

    def piled(pile, extra, anchors=(), bas=None):
        return dict(inp(R, pile, anchors, bas), pile=[hx(b) for b in pile] + list(extra))

    A("pile_copies_in_two_hex_cases", piled(CL, [upper(CL[1])]),
      "sec 8.1 with HARNESS2's pile encoding: one entry spelled twice, in lowercase digits and "
      "in uppercase. The pile is a multiset of byte strings and the digits' case is transport: "
      "byte-identical copies collapse to one whatever spelled them, the entry count is five, "
      "and the report is the clean ledger's.", "COMPLETE")
    A("pile_copies_in_three_hex_cases", piled(CL, [upper(CL[3]), mixed(CL[3])]),
      "sec 8.1: one entry spelled three ways, lowercase, uppercase and mixed; one entry.",
      "COMPLETE")
    JUNK = b'{"junk":1}'
    A("pile_malformed_copies_in_two_hex_cases", piled(CL + [JUNK], [upper(JUNK)]),
      "sec 8.1/8.6 item 10: a byte string failing sec 4.3 spelled in two cases is one byte "
      "string and one MALFORMED row.", "COMPLETE")
    FG = mk("k9", "genesis", 0, None, GENB)
    A("pile_excluded_copies_in_two_hex_cases", piled(CL + [FG], [upper(FG)]),
      "sec 8.1/8.6 item 7: an entry outside the lineage spelled in two cases is one entry and "
      "one EXCLUDED row.", "COMPLETE")
    tw1 = mk("root", "annotation", 1, eid(CL[0]), ANNB("one twin"))
    tw2 = mk("root", "annotation", 1, eid(CL[0]), ANNB("the other twin"))
    A("pile_fork_twins_each_in_two_hex_cases", piled([CL[0], tw1, tw2], [upper(tw1), mixed(tw2)]),
      "sec 8.1/8.4: the two twins of a fork each spelled in two cases are two entries and one "
      "EQUIVOCATION; byte-identical copies are one entry and never equivocate.", "BROKEN_CHAIN")

    # ---------------- sec 8.2: the seat moves after a gap --------------------
    g0 = mk("root", "genesis", 0, None, GENB)
    h1 = mk("root", "history", 1, eid(g0), HISB(1))                 # written and never held
    s2 = mk("root", "succession", 2, eid(h1), SUCB(K2))
    a3 = mk("k2", "annotation", 3, eid(s2), ANNB("the successor, after a gap"))
    A("seat_moves_at_the_entry_opening_a_gap", inp(R, [g0, s2, a3]),
      "sec 8.2: seq 1 is not held and the root's succession at seq 2 opens the gap. Authority "
      "is walked state and the rule that moves it names no exception: after every entry at a "
      "position has been visited, a succession by the key in office moves the seat, certain "
      "or not. k2's entry at seq 3 is by the key in office and records nothing: one SEQ_GAP, "
      "no AUTHORITY_MISMATCH.", "GAPS")
    b2 = mk("root", "annotation", 2, eid(h1), ANNB("after the gap"))
    s3 = mk("root", "succession", 3, eid(b2), SUCB(K2))
    a4 = mk("k2", "annotation", 4, eid(s3), ANNB("the successor, two positions after a gap"))
    A("seat_moves_at_a_position_after_a_gap", inp(R, [g0, b2, s3, a4]),
      "sec 8.2: the gap opens at seq 2 and the root hands the seat on at seq 3, a position "
      "visited while the walk is already not certain; the seat moves all the same and k2's "
      "entry at seq 4 records nothing: one SEQ_GAP, no AUTHORITY_MISMATCH.", "GAPS")
    t3 = mk("k2", "succession", 3, eid(s2), SUCB(K3))
    a4b = mk("k3", "annotation", 4, eid(t3), ANNB("the second successor"))
    A("seat_moves_twice_after_a_gap", inp(R, [g0, s2, t3, a4b]),
      "sec 8.2: two successions after the gap, the root's at seq 2 and k2's at seq 3, each by "
      "the key in office when its position is visited; k3's entry at seq 4 records nothing.",
      "GAPS")
    c3 = mk("root", "annotation", 3, eid(s2), ANNB("the ceding key writes on"))
    A("ceding_key_graded_soft_after_its_gap_succession", inp(R, [g0, s2, c3]),
      "sec 8.2: the root's succession at seq 2, after a gap, moved the seat to k2, so the "
      "root's own entry at seq 3 differs from the authority in force and records "
      "AUTHORITY_MISMATCH, soft since the walk is not certain: the seat that moved is the "
      "seat the next entry is graded against.", "GAPS")

    # ---------------- sec 12.6: the same two points in the old segment -------
    T = 1700000000
    TWO = basis((chainobj(1, senders=(R, K2, K3, K9)), chainobj(8453, senders=(R, K2, K3, K9))))

    def mkv1(key, etype, seq, prev, body):
        return make_entry_v1(K(key), etype, seq, prev, body)

    def continued(closing, note):
        g = mk("k2", "genesis", 0, None, {"statement_md": "continued by k2",
                                          "continues": {"root": R, "entry_id": eid(closing)}})
        return [g, mk("k2", "annotation", 1, eid(g), ANNB(note))]

    o0 = mkv1("root", "genesis", 0, None, GENB)
    o1 = mkv1("root", "history", 1, eid(o0), HISB(1))               # written and never held
    o2 = mkv1("root", "succession", 2, eid(o1), SUCB(K2))
    o3 = mkv1("k2", "annotation", 3, eid(o2), ANNB("the successor under the first law"))
    o4 = mkv1("k2", "succession", 4, eid(o3), SUCB(K2, kind="migration"))
    n = continued(o4, "k2 under the second law")
    OA = [anchor(eid(o0), R, bn=10, ts=T), anchor(eid(o2), R, bn=12, ts=T + 2),
          anchor(eid(o3), K2, bn=13, ts=T + 3), anchor(eid(o4), K2, bn=14, ts=T + 4),
          anchor(eid(n[0]), K2, bn=15, ts=T + 5), anchor(eid(n[1]), K2, bn=16, ts=T + 6)]
    A("continue_old_segment_seat_moves_after_gap", inp(R, [o0, o2, o3, o4] + n, OA, bas=TWO),
      "sec 12.6 with zikaron/1 sec 8.2: the old segment lacks seq 1 and the root hands the "
      "seat to k2 at seq 2, the entry that opens the gap. Under zikaron/1 too a succession by "
      "the key in office moves the seat after a gap, so k2's entries at seq 3 and 4 record "
      "nothing there: the old report carries one SEQ_GAP and no AUTHORITY_MISMATCH, and the "
      "whole reads GAPS.", "GAPS")
    A("continue_old_entry_in_two_hex_cases", piled([o0, o2, o3, o4] + n, [upper(o3), mixed(n[1])], OA, TWO),
      "sec 8.1/12.6: the same continued ledger with one entry of each segment spelled a second "
      "time in another case; each is one byte string of its segment and the report is that of "
      "the case before.", "GAPS")
    p2 = mkv1("k2", "annotation", 2, eid(o1), ANNB("k2 before its seat"))
    p3 = mkv1("root", "succession", 3, eid(p2), SUCB(K2))
    p4 = mkv1("k2", "succession", 4, eid(p3), SUCB(K2, kind="migration"))
    m = continued(p4, "k2 under the second law, after an early entry")
    PA = [anchor(eid(o0), R, bn=10, ts=T), anchor(eid(p2), K2, bn=12, ts=T + 2),
          anchor(eid(p3), R, bn=13, ts=T + 3), anchor(eid(p4), K2, bn=14, ts=T + 4),
          anchor(eid(m[0]), K2, bn=15, ts=T + 5), anchor(eid(m[1]), K2, bn=16, ts=T + 6)]
    A("continue_old_segment_authority_graded_after_gap", inp(R, [o0, p2, p3, p4] + m, PA, bas=TWO),
      "sec 12.6 with zikaron/1 sec 8.2: the old segment lacks seq 1 and k2 writes at seq 2 "
      "before the root hands it the seat at seq 3. zikaron/1 still compares authority after a "
      "gap and records the contradiction soft: the old report carries SEQ_GAP and a soft "
      "AUTHORITY_MISMATCH at seq 2, nothing at seq 3 and 4, and the whole reads GAPS.", "GAPS")


def build_misuse():
    """HARNESS2.md's misuse rules, one case per rule, each answered by exit 2."""
    sign_in = next(r for r in MANIFEST if r["category"] == "sign")
    canon_in = next(r for r in MANIFEST if r["category"] == "canon")
    audit_in = next(r for r in MANIFEST if r["category"] == "audit")
    key = "0x%064x" % KEYS["root"]
    M("sign_domain_line_feed", ["sign", key, sign_in["path"], "client.doc/1\n"],
      "HARNESS2: a domain carrying a line feed is misuse")
    M("sign_domain_above_7f", ["sign", key, sign_in["path"], "client.doc/1\u00e9"],
      "HARNESS2: a domain with a byte above 0x7F is misuse")
    M("sign_key_zero", ["sign", "0x" + "0" * 64, sign_in["path"]],
      "HARNESS2: the private key names a scalar in [1, n - 1]; zero is misuse")
    M("sign_key_n", ["sign", "0x%064x" % N, sign_in["path"]],
      "HARNESS2: the group order is outside [1, n - 1]")
    M("sign_key_63_digits", ["sign", key[:-1], sign_in["path"]],
      "HARNESS2: sixty-three digits are not a key")
    M("sign_extra_argument", ["sign", key, sign_in["path"], "zikaron/2", "extra"],
      "HARNESS2: an argument beyond the command's list")
    M("check_two_paths", ["check", canon_in["path"], canon_in["path"]],
      "HARNESS2: check takes one path")
    M("canon_absent_path", ["canon", "canon/no-such-file.bin"],
      "HARNESS2: a path the command cannot read is misuse")
    M("audit_absent_path", ["audit", "audit/no-such-file.json"],
      "HARNESS2: a path the command cannot read is misuse")
    M("audit_no_path", ["audit"], "HARNESS2: audit takes one path")
    M("unknown_command", ["frobnicate", audit_in["path"]], "HARNESS2: an unknown command is misuse")
    bad_canon = next(r for r in MANIFEST if r["category"] == "canon" and r["predicted"] != "ok"
                     and r.get("predicted_canon") != "ok")
    M("sign_path_fails_canonical", ["sign", key, bad_canon["path"]],
      "HARNESS2: sign over bytes failing sec 3.5 tests 1 to 5 is misuse")
    arr = next((r for r in MANIFEST if r["category"] == "canon" and r.get("predicted_canon") == "ok"
                and open(os.path.join(HERE, r["path"]), "rb").read()[:1] == b"["), None)
    if arr is not None:
        M("sign_value_not_object", ["sign", key, arr["path"]],
          "HARNESS2: sign over a canonical value that is no object is misuse")
    M("sign_key_non_hex", ["sign", "0x" + "g" * 64, sign_in["path"]],
      "HARNESS2: a key carrying a digit outside hexadecimal is misuse")
    M("audit_extra_argument", ["audit", audit_in["path"], "extra"], "HARNESS2: audit takes one path")
    M("check_no_path", ["check"], "HARNESS2: check takes one path")
    M("canon_two_paths", ["canon", canon_in["path"], canon_in["path"]], "HARNESS2: canon takes one path")
    M("no_command", [], "HARNESS2: no command is misuse")
    M("canon_directory", ["canon", "canon/"], "HARNESS2: a directory is a path the command cannot read")
    M("sign_only_key", ["sign", key], "HARNESS2: sign takes a key and a path")
    M("check_absent_path", ["check", "canon/no-such-file.bin"], "HARNESS2: a path the command cannot read is misuse")
    M("sign_absent_path", ["sign", key, "sign/no-such-file.bin"], "HARNESS2: a path the command cannot read is misuse")
    M("sign_key_n_plus_one", ["sign", "0x%064x" % (N + 1), sign_in["path"]], "HARNESS2: a scalar above n - 1 is misuse, reduced by nobody")
    M("sign_key_max_256", ["sign", "0x" + "f" * 64, sign_in["path"]], "HARNESS2: 2^256 - 1 is outside [1, n - 1]")
    M("sign_key_65_digits", ["sign", key + "0", sign_in["path"]], "HARNESS2: sixty-five digits are not a key")
    M("sign_no_argument", ["sign"], "HARNESS2: sign takes a key and a path")
    M("canon_no_argument", ["canon"], "HARNESS2: canon takes a path")
    M("check_directory", ["check", "canon/"], "HARNESS2: a directory is a path check cannot read")
    M("sign_directory", ["sign", key, "sign/"], "HARNESS2: a directory is a path sign cannot read")
    M("audit_directory", ["audit", "audit/"], "HARNESS2: a directory is a path audit cannot read")


# ==========================================================================
# main
# ==========================================================================


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    seed = args.seed
    rng = random.Random(seed)

    for tag in ("root", "k2", "k3", "k4", "k9"):
        KEYS[tag] = testkey(tag, seed)

    for d in ("canon", "sign", "audit"):
        p = os.path.join(HERE, d)
        if os.path.isdir(p):
            shutil.rmtree(p)
        os.makedirs(p)
    del MANIFEST[:]
    del WARNINGS[:]
    for k in _counters:
        _counters[k] = 0

    t = build_templates()
    build_canon(rng, t)
    build_sign(rng, t)
    build_audit(rng, t)
    build_readings(t)
    build_misuse()

    counts = {}
    for r in MANIFEST:
        counts[r["category"]] = counts.get(r["category"], 0) + 1
    man = {
        "corpus": "zikaron/2 convergence corpus",
        "law": "docs/zikaron-v2.md",
        "harness": "zikaron-conformance/HARNESS2.md",
        "seed": seed,
        "generator": "gen.py",
        "keys": {tag: {"privkey": "0x%064x" % KEYS[tag], "address": privkey_to_addr_hex(KEYS[tag])}
                 for tag in sorted(KEYS)},
        "counts": counts,
        "coverage": coverage(),
        "vocabularies": vocabularies(),
        "unwitnessed": unwitnessed(),
        "undecided": [],
        "prediction_disclaimer":
            "Every `predicted*` field is the corpus author's own reading of the law, produced "
            "by gen.py and by nothing else. Its audit predictor is written to the same clauses "
            "as the criterion core by the same author, so agreement between the two is one "
            "reading measured twice and never independent evidence; the independent evidence "
            "is the three implementations of the review rounds. `hand_expect` records the "
            "author's prose reasoning about a case; where it differs from the code the case "
            "carries `disagreement: true`.",
        "cases": MANIFEST,
    }
    with open(os.path.join(HERE, "manifest.json"), "w") as f:
        json.dump(man, f, indent=1, sort_keys=True)

    with open(os.path.join(HERE, "README.md"), "w") as f:
        f.write(README % (seed, counts.get("canon", 0), counts.get("sign", 0),
                          counts.get("audit", 0)))

    print("seed %d" % seed)
    for k in sorted(counts):
        print("  %-6s %5d files" % (k, counts[k]))
    print("  canon/ predicted tokens:")
    tally = {}
    for r in MANIFEST:
        if r["category"] == "canon":
            tally[r["predicted"]] = tally.get(r["predicted"], 0) + 1
    for k in sorted(tally, key=lambda x: (-tally[x], x)):
        print("      %-22s %4d" % (k, tally[k]))
    print("  audit/ predicted labels:")
    tally = {}
    for r in MANIFEST:
        if r["category"] == "audit":
            tally[r["predicted_label"]] = tally.get(r["predicted_label"], 0) + 1
    for k in sorted(tally, key=lambda x: (-tally[x], x)):
        print("      %-22s %4d" % (k, tally[k]))
    gaps = unwitnessed()
    print("  closed vocabularies unwitnessed: %s" % (", ".join(gaps) if gaps else "none"))
    if WARNINGS:
        print("  hand/code disagreements (%d):" % len(WARNINGS))
        for w in WARNINGS:
            print("      " + w)
    else:
        print("  hand/code disagreements: none")


def vocabularies():
    """The closed vocabularies as one table of [coverage key, residue label,
    token]: a label is unwitnessed iff the tally under its key lacks its
    token. The comparator derives `unwitnessed` from this table and refuses a
    manifest whose list differs."""
    rows = ["ANCHORED", "DISCARDED", "MALFORMED", "UNKNOWN_TYPE", "UNPROVEN", "VOID", "CONTINUES"]
    return ([["sec_10_tokens_and_accept", t, t] for t in TOKENS + ["ok"]] +
            [["sec_8_6_rows_and_findings", f, f] for f in FINDINGS + [r for r in rows if r not in FINDINGS]] +
            [["sec_8_6_labels", l, l] for l in LABELS + ["NO_LABEL"]] +
            [["sec_9_3_anchor_verdicts", v, v] for v in ("counted", "UNPROVEN", "VOID")])


def unwitnessed():
    """The residue of the closed vocabularies no case witnesses (sec 12.1's
    precondition on the corpus, published in the manifest)."""
    cov = coverage()
    return sorted(set(lab for key, lab, tok in vocabularies() if tok not in cov[key]))


def coverage():
    tok, lab, fin, verd = {}, {}, {}, {}
    for r in MANIFEST:
        if r["category"] == "canon":
            tok[r["predicted"]] = tok.get(r["predicted"], 0) + 1
        elif r["category"] == "audit":
            lab[r["predicted_label"]] = lab.get(r["predicted_label"], 0) + 1
            for k, v in r["predicted_counts"].items():
                if k.startswith("f_") and v:
                    fin[k[2:]] = fin.get(k[2:], 0) + v
            for k in ("missing", "anchored", "unanchored", "excluded", "retired", "unknown_type",
                      "malformed", "unproven", "void", "discarded", "continues"):
                if r["predicted_counts"].get(k):
                    fin[k.upper()] = fin.get(k.upper(), 0) + r["predicted_counts"][k]
    for r in MANIFEST:
        if r["category"] == "audit":
            try:
                _doc = json.load(open(os.path.join(HERE, r["path"])))
            except Exception:
                continue                      # a raw no-label input is not a JSON text
            if not isinstance(_doc, dict) or not isinstance(_doc.get("anchors"), list):
                continue
            for a in _doc["anchors"]:
                v = a.get("verdict") if isinstance(a, dict) else None
                if v in ("counted", "UNPROVEN", "VOID"):
                    verd[v] = verd.get(v, 0) + 1
    return {"sec_10_tokens_and_accept": tok, "sec_8_6_labels": lab,
            "sec_8_6_rows_and_findings": fin, "sec_9_3_anchor_verdicts": verd}


TOKENS = ["E_UTF8", "E_JSON", "E_NUMBER", "E_DEPTH", "E_DUP_KEY", "E_KEY_CHARSET",
          "E_VALUE_CHARSET", "E_NOT_CANONICAL", "E_ENVELOPE", "E_ENVELOPE_MISSING",
          "E_ENVELOPE_CLOSED", "E_SPEC", "E_ENTRYTYPE", "E_AUTHOR", "E_SEQ", "E_PREV",
          "E_PREV_SEQ", "E_BODY", "E_SIG_FORM", "E_GENESIS_PLACE", "E_BODY_FIELD", "E_SIG_V",
          "E_SIG_RANGE", "E_SIG_HIGH_S", "E_SIG_RECOVER", "E_SIG_SIGNER"]
FINDINGS = ["SEQ_GAP", "PREV_MISMATCH", "AUTHORITY_MISMATCH", "ROOT_MISMATCH", "EQUIVOCATION",
            "MISSING", "UNANCHORED", "EXCLUDED", "RETIRED", "CONTINUES"]
LABELS = ["BROKEN_CHAIN", "UNPROVEN", "GAPS", "COMPLETE"]


README = """# zikaron/2 convergence corpus

Generated by `gen.py` from `docs/zikaron-v2.md` (the law),
`docs/zikaron-v1.md` (as sec 12.6 invokes it) and
`zikaron-conformance/HARNESS2.md` (the command-line contract), carrying over
the zikaron/1 generator where the two texts agree word for word; the old
segment of a continued ledger (sec 12.6) is predicted with the zikaron/1
generator's own predictor (`../corpus/gen.py`), imported as a tool and never
as evidence. No implementation of the law was read while building it.

## Regenerating

    python3 gen.py --seed %d

The run is deterministic: the same seed rebuilds byte-identical directories.
`canon/`, `sign/` and `audit/` are deleted and repopulated on every run, and
`manifest.json` and this file are rewritten. Requires Python 3 and
`pycryptodome` (for `Crypto.Hash.keccak`); secp256k1 with RFC 6979 and
low-s, the sec 3.4 canonicalizer, the sec 3.5
and sec 4.3 decision order and this law's sec 8 walk are implemented inside
`gen.py`; the old segment's zikaron/1 walk is `../corpus/gen.py`'s.

## Layout

- `canon/` (%d files, one per case) - byte strings for `zk2 check <path>` and
  `zk2 canon <path>`. Every file is raw bytes; many are not valid UTF-8 and
  most are not valid JSON, which is the point.
- `sign/` (%d cases, two files each) - `sNNN_*.bin` holds an object for
  `zk2 sign`: mostly the canonical bytes of a six-member envelope (or, under a
  family law's domain, of that document); one case is a non-canonical
  spelling, one carries a top-level `sig` and one is the empty object, all
  signed as HARNESS2.md says. The sibling `sNNN_*.meta.json` names the
  private key and, where the case passes one, the domain, for `zk2 sign
  <privkey-hex> <path> [domain]`.
- `audit/` (%d cases, two files each) - `aNNN_*.json` is one `zk2 audit
  <path>` input in the HARNESS2.md format; the sibling `aNNN_*.note` says
  what the scenario probes.
- misuse cases (category `misuse`, no files of their own) - argument vectors
  HARNESS2.md calls misuse; the prediction is `{"exit": 2}` and both programs
  must exit 2 with empty stdout (a diagnostic may go to stderr).
- `manifest.json` - every file with its category, its note, and the corpus
  author's own prediction; beside the cases, `coverage` (the closed
  vocabularies tallied), `vocabularies` (the closed vocabularies as a table
  the comparator derives the residue from), `unwitnessed` (the residue of
  those vocabularies no case witnesses: what is
  published beside this corpus toward sec 12.1's precondition is this residue
  and the branch-arm classes of `../BOUNDARY-AUDIT.md`; no per-clause list
  exists), `undecided` (empty) and `keys`. A continued audit case also carries
  `predicted_continues` and `predicted_old` (below).

## Reading manifest.json

Each case carries the author's prediction as a third opinion:

- canon cases: `predicted` (`"ok"` or the sec 10 token `zk2 check` should
  name), `predicted_entry_id`, `predicted_canon` (what `zk2 canon` should
  answer, since it applies sec 3.5 tests 1 through 5 only) and
  `predicted_canon_sha256` (SHA-256 of the canonical bytes it should print,
  so long outputs stay comparable).
- audit cases: `predicted_label` and `predicted_counts` (ledger entry count,
  finding counts, list sizes) and, for a continued input, `predicted_continues`
  (the selected continuation's `entry_id` and `cut`) and `predicted_old` (the
  nested zikaron/1 report's label and counts).
- sign cases: `predicted` with `presig`, `digest`, `sig` and `signer`; the
  meta names the private key in one of the harness's spellings and carries a
  `domain` member only where the case passes one.

None of these is authoritative: the predictor is the corpus author's reading
of the law, written to the same clauses as the criterion, so agreement
between the two is one reading measured twice; the independent evidence is
the three implementations of the review rounds. Where the two
implementations agree with each
other and disagree with a prediction, the prediction is wrong. Where they
disagree with each other, the law decides, and the prediction is only a hint
about which reading a third reader reached. `hand_expect` (canon) and
`hand_expect_label` (audit) carry the author's prose reasoning written before
the code ran; a case where the two differ is flagged `disagreement: true` and
is worth reading closely.

## Comparing two implementations

`../compare2.py` is the instrument: it runs every case of the manifest,
the misuse cases on exit code and empty stdout, and prints the manifest's
sha256 and a digest over every corpus byte and case it read, which
`ZK2_EXPECT` may pin. By hand, printing the exit code beside each answer:

    for f in canon/*.bin;  do zk2 check "$f"; echo " $?"; zk2 canon "$f"; echo " $?"; done
    for m in sign/*.meta.json; do ... zk2 sign <privkey> <bin> [<domain>]; done   # domain only where the meta has one
    for f in audit/*.json; do zk2 audit "$f"; echo " $?"; done

Every command writes one canonical JSON value and exits 0 whenever it produced
an answer. Diff the two implementations' streams byte for byte.
"""


if __name__ == "__main__":
    main()
