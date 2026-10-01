#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
gen.py - corpus generator for the `zikaron/1` convergence run.

Written from docs/zikaron-v1.md and zikaron-conformance/HARNESS.md alone, by
carrying over the twin law's generator where the two texts say the same thing
in the same words (canonical form, signing, chain, audit, anchoring) and
rewriting every part the evidence ledger states differently (the envelope key
`author`, the seven bodies, the four audit inputs). No implementation of the
law was consulted. Everything here (canonicalizer, parser, secp256k1 with
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
KNOWN_TYPES = ("genesis", "history", "grant", "revocation", "adoption", "succession", "annotation")


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

    if etype == "genesis":
        return None if req("statement_md", is_str) else E
    if etype == "history":
        if not req("content", is_hex32):
            return E
        if not body.has("mode"):
            return E
        mode = body.get("mode")
        if not isinstance(mode, JObj):
            return E
        if not (mode.has("mark") and is_token(mode.get("mark"))):
            return E
        if not (mode.has("toolchain") and is_hex32(mode.get("toolchain"))):
            return E
        if not opt("note_md", is_str):
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
        return None
    if etype == "revocation":
        if not req("grant", is_hex32):
            return E
        if not opt("case", is_hex32):
            return E
        return None
    if etype == "adoption":
        if not body.has("anchors"):
            return E
        a = body.get("anchors")
        if not isinstance(a, list) or len(a) == 0:
            return E
        for el in a:
            if not isinstance(el, JObj):
                return E
            if not (el.has("chainId") and is_int(el.get("chainId"))):
                return E
            if not (el.has("tx") and is_hex32(el.get("tx"))):
                return E
            if not (el.has("payloadKind") and is_token(el.get("payloadKind"))):
                return E
            if not (el.has("content") and is_hex32(el.get("content"))):
                return E
        ha, hb = body.has("attestor"), body.has("attestation")
        if ha != hb:
            return E
        if ha:
            if not is_hex20(body.get("attestor")):
                return E
            if not is_hex65(body.get("attestation")):
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
    if etype == "annotation":
        if not opt("subject", is_hex32):
            return E
        if not req("note_md", is_str):
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
    digest = eip191_digest("zikaron/1", presig)
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
    if v.get("spec") != "zikaron/1":
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
    digest = eip191_digest("zikaron/1", presig)
    r, s, v = ecdsa_sign(priv, digest)
    return sig_hex(r, s, v), presig, digest, (r, s, v)


def make_entry(priv, entryType, seq, prev, body, spec="zikaron/1", author=None,
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


def entry_digest(author_hex, entryType, seq, prev, body, spec="zikaron/1"):
    env6 = build_env(spec, entryType, author_hex, seq, prev, J(body) if isinstance(body, dict) else body)
    presig = sha256(cbytes(env6))
    return eip191_digest("zikaron/1", presig), presig


def eid(b: bytes) -> str:
    return hx(sha256(b))


# --------------------------------------------------------------------------
# adoption attestation (sec 6.6)
# --------------------------------------------------------------------------


def attestation_C(author, anchors, prev):
    return JObj([("adopter", author), ("anchors", J(anchors)), ("prev", prev)])


def sign_attestation(priv, author, anchors, prev):
    C = cbytes(attestation_C(author, anchors, prev))
    presig = sha256(C)
    digest = eip191_digest("zikaron/1-adoption", presig)
    r, s, v = ecdsa_sign(priv, digest)
    return sig_hex(r, s, v), C, presig, digest


def attestation_ok(env: JObj):
    """sec 6.6, used by the audit predictor only (never an entry predicate)."""
    body = env.get("body")
    if not body.has("attestor"):
        return False
    att = body.get("attestation")
    C = cbytes(attestation_C(env.get("author"), body.get("anchors"), env.get("prev")))
    digest = eip191_digest("zikaron/1-adoption", sha256(C))
    raw = bytes.fromhex(att[2:])
    r = int.from_bytes(raw[0:32], "big")
    s = int.from_bytes(raw[32:64], "big")
    v = raw[64]
    if v not in (27, 28):
        return False
    if not (1 <= r <= N - 1 and 1 <= s <= N - 1):
        return False
    if s > HALF_N:
        return False
    pub = _recover_cached(digest, r, s, v)
    if pub is None:
        return False
    return hx(pub_to_addr(pub)) == body.get("attestor")





# --------------------------------------------------------------------------
# the audit predictor (sec 8, sec 9.4 well-formedness)
# --------------------------------------------------------------------------


def _basis_ok(basis):
    if not isinstance(basis, dict):
        return False
    if set(basis.keys()) != {"chains", "bareTx", "adoptionChains"}:
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
            # sec 9.4: bytewise ascending, no repeated element
            if any(arr[i].encode() >= arr[i + 1].encode() for i in range(len(arr) - 1)):
                return False
        # sec 9.4: ordered by (chainId, fromBlock); two windows of one chain do not
        # overlap, and where they touch their registries or senders differ
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
    bt = basis["bareTx"]
    if not isinstance(bt, list):
        return False
    prev = None
    for o in bt:
        if not isinstance(o, dict) or set(o.keys()) != {"chainId", "tx"}:
            return False
        if not (is_int(o["chainId"]) and is_hex32(o["tx"])):
            return False
        k = (o["chainId"], o["tx"].encode())
        if prev is not None and k <= prev:
            return False
        prev = k
    ac = basis["adoptionChains"]
    if not isinstance(ac, list):
        return False
    prev_c = None
    for o in ac:
        if not isinstance(o, dict) or set(o.keys()) != {"chainId", "throughBlock"}:
            return False
        if not (is_int(o["chainId"]) and is_int(o["throughBlock"])):
            return False
        # sec 9.4: at most one object per chainId, ordered by chainId ascending
        if prev_c is not None and o["chainId"] <= prev_c:
            return False
        prev_c = o["chainId"]
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
    """sec 8 with sec 9.4: the four inputs by their forms; any failure is no label."""
    if not isinstance(inp, dict):
        return False
    if not _nums_in_universe(inp) or _depth(inp) > 128:
        return False
    for k in ("root", "pile", "anchors", "unavailable", "evidence", "basis"):
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
    if not isinstance(inp["unavailable"], list) or not all(_hexn_ok(u, 32) for u in inp["unavailable"]):
        return False
    if not isinstance(inp["evidence"], list):
        return False
    for e in inp["evidence"]:
        if not isinstance(e, dict) or not is_int(e.get("chainId")):
            return False
        if not (_hexn_ok(e.get("tx"), 32) and _hexn_ok(e.get("sender"), 20)):
            return False
        c = e.get("calldata")
        if not (isinstance(c, str) and c[:2] == "0x" and len(c) % 2 == 0 and all(ch in "0123456789abcdef" for ch in c[2:])):
            return False
    return True


def predict_audit(inp):
    """Predicts sec 8.7's label plus a few counts. Not authoritative."""
    out = {"label": None, "entries": 0, "findings": 0, "hard": 0,
           "f_SEQ_GAP": 0, "f_PREV_MISMATCH": 0, "f_AUTHORITY_MISMATCH": 0,
           "f_ROOT_MISMATCH": 0, "f_EQUIVOCATION": 0,
           "missing": 0, "anchored": 0, "unanchored": 0, "excluded": 0,
           "unknown_type": 0, "malformed": 0, "unavailable": 0, "unproven": 0, "void": 0,
           "discarded": 0, "adoption_unproven": 0}
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
    seenE = {}
    for e in inp["evidence"]:
        k = (e["chainId"], e["tx"])
        e4 = {kk: e[kk] for kk in ("chainId", "tx", "sender", "calldata")}
        if k in seenE and seenE[k] != e4:
            out["label"] = "NO_LABEL"
            return out
        seenE[k] = e4
    evidence = list(seenE.values())
    ac_chains = set(o["chainId"] for o in inp["basis"]["adoptionChains"])
    for e in evidence:
        # sec 9.4 [C]: an evidence record on a chain no adoptionChains object names
        if e["chainId"] not in ac_chains:
            out["label"] = "NO_LABEL"
            return out
    # sec 9.4 [C]: every record lies within the basis's reach
    bas = inp["basis"]
    for a in anchors:
        by_bare = any(o["chainId"] == a["chainId"] and o["tx"] == a["tx"] for o in bas["bareTx"])
        by_chain = any(c["chainId"] == a["chainId"] and c["fromBlock"] <= a["blockNumber"] <= c["toBlock"]
                       and a["sender"] in c["senders"] for c in bas["chains"])
        if not (by_bare or by_chain):
            out["label"] = "NO_LABEL"
            return out

    raw = [bytes.fromhex(x[2:]) for x in inp["pile"]]
    pile_ids = set(hx(sha256(x)) for x in raw)
    ents = {}
    malformed = set()
    for b in raw:
        ok, val = zk_check(b)
        if ok and b not in ents:
            ents[b] = Ent(b, parsed_entry(b))
        elif not ok:
            malformed.add(hx(sha256(b)))
    entries = list(ents.values())
    out["malformed"] = len(malformed)

    root = inp["root"]
    lineage = {root}
    changed = True
    while changed:
        changed = False
        for e in entries:
            if e.etype == "succession" and e.author in lineage:
                t = e.env.get("body").get("to")
                if t not in lineage:
                    lineage.add(t)
                    changed = True

    ledger = [e for e in entries if e.author in lineage]
    excluded = [e for e in entries if e.author not in lineage]
    out["entries"] = len(ledger)
    out["excluded"] = len(excluded)
    out["unknown_type"] = sum(1 for e in ledger if e.etype not in KNOWN_TYPES)

    out["discarded"] = sum(1 for a in anchors if a["sender"] not in lineage)
    anchors = [a for a in anchors if a["sender"] in lineage]
    counted = [a for a in anchors if a["verdict"] == "counted"]
    counted_hashes = set(a["hash"] for a in counted)
    unproven = [a for a in anchors if a["verdict"] == "UNPROVEN"]
    void = [a for a in anchors if a["verdict"] == "VOID"]
    out["unproven"] = len(unproven)
    out["void"] = len(void)

    unavail = set(h for h in inp["unavailable"] if h in counted_hashes and h not in pile_ids)
    out["unavailable"] = len(unavail)

    order = sorted(ledger, key=lambda e: (e.seq, bytes.fromhex(e.id[2:])))
    by_seq = {}
    for e in order:
        by_seq.setdefault(e.seq, []).append(e)

    findings = []
    expected = 0
    auth = root
    certain = True
    prev_seq = None
    for s in sorted(by_seq):
        grp = by_seq[s]
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
                if e.author != root:
                    findings.append(("ROOT_MISMATCH", True))
            else:
                if e.author != auth:
                    findings.append(("AUTHORITY_MISMATCH", certain))
            prev_seq = e.seq
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
    missing = counted_hashes - have - unavail
    out["missing"] = len(missing)
    out["unanchored"] = sum(1 for e in ledger if e.id not in counted_hashes)
    out["anchored"] = sum(1 for e in ledger if e.id in counted_hashes)

    unproven_open = [a for a in unproven if a["hash"] not in pile_ids and a["hash"] not in counted_hashes]
    ev = {(e["chainId"], e["tx"]): e for e in evidence}
    unproven_elems = 0
    for e in ledger:
        if e.etype != "adoption":
            continue
        pre = {root}
        ch = True
        while ch:
            ch = False
            for x in ledger:
                if x.etype == "succession" and x.seq < e.seq and x.author in pre:
                    t = x.env.get("body").get("to")
                    if t not in pre:
                        pre.add(t)
                        ch = True
        att = attestation_ok(e.env) if e.env.get("body").has("attestor") else False
        for el in e.env.get("body").get("anchors"):
            cid = el.get("chainId")
            tx = el.get("tx")
            content = bytes.fromhex(el.get("content")[2:])
            proven = False
            if cid in ac_chains and (cid, tx) in ev:
                cd = bytes.fromhex(ev[(cid, tx)]["calldata"][2:])
                hit = False
                off = 0
                while off + 32 <= len(cd):
                    if (off % 32 == 0 or (off - 4) % 32 == 0) and off >= 0:
                        if cd[off:off + 32] == content:
                            hit = True
                            break
                    off += 1
                if hit:
                    sender = ev[(cid, tx)]["sender"]
                    if sender in pre:
                        proven = True
                    elif att and e.env.get("body").get("attestor") == sender:
                        proven = True
            if not proven:
                unproven_elems += 1
    out["adoption_unproven"] = unproven_elems

    if out["hard"]:
        out["label"] = "BROKEN_CHAIN"
    elif unavail or unproven_open:
        out["label"] = "UNAVAILABLE"
    elif any(nm == "SEQ_GAP" for nm, _ in findings) or missing:
        out["label"] = "GAPS"
    else:
        out["label"] = "COMPLETE"
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


def S(slug, priv, data: bytes, domain, note):
    _counters["sign"] += 1
    base = "s%03d_%s" % (_counters["sign"], slug)
    _write(os.path.join(HERE, "sign", base + ".bin"), data)
    presig = sha256(data)
    digest = eip191_digest(domain, presig)
    r, s, v = ecdsa_sign(priv, digest)
    meta = {
        "input": base + ".bin",
        "privkey": "0x%064x" % priv,
        "domain": domain,
        "note": note,
        "predicted": {
            "presig": hx(presig),
            "digest": hx(digest),
            "sig": sig_hex(r, s, v),
            "signer": privkey_to_addr_hex(priv),
        },
    }
    with open(os.path.join(HERE, "sign", base + ".meta.json"), "w") as f:
        json.dump(meta, f, indent=1, sort_keys=True)
    MANIFEST.append({
        "path": "sign/" + base + ".bin",
        "meta": "sign/" + base + ".meta.json",
        "category": "sign",
        "note": note,
        "domain": domain,
        "privkey": meta["privkey"],
        "predicted": meta["predicted"],
    })


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
    if expect_label is not None:
        rec["hand_expect_label"] = expect_label
        if expect_label != p["label"]:
            rec["disagreement"] = True
            WARNINGS.append("audit/%s: hand=%s code=%s (%s)" % (base, expect_label, p["label"], note))
    MANIFEST.append(rec)
    return rec


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
    t.adopt = make_entry(K("root"), "adoption", 1, t.gid,
                         {"anchors": [{"chainId": 1, "tx": HX32(0xAA), "payloadKind": "calldata",
                                       "content": HX32(0xC0)}]})
    t.unk = make_entry(K("root"), "windmill", 1, t.gid, {})
    return t


# sec 3.5, closed by construction: every prefix and every single-byte
# substitution of a small set of templates that together reach every state of
# the grammar. Seed-free; the same cases under every seed.
_SYNTAX_TEMPLATES = [
    ("full", b'{"a":[1,-20,"b\\"\\\\\\/\\b\\f\\n\\r\\t\\u00e9",true,false,null],"c":{"d":{}},"e":[]}'),
    ("escapes", b'"x\\u0041\\ud83dy"'),
    ("nesting", b'[[[[1]]],{"k":"v"}]'),
    ("envelope", b'{"prev":null,"seq":0,"spec":"zikaron/1"}'),
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
    C("ws_after_colon", sub(t.gen, b'"spec":"zikaron/1"', b'"spec": "zikaron/1"'),
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
    C("dup_envelope_key", sub(t.ann, b'"spec":"zikaron/1"', b'"spec":"zikaron/1","spec":"zikaron/1"'),
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
    C("spec_wrong_version", set_key(t.ann, "spec", "zikaron/2"), "sec 4.3 step 4", expect="E_SPEC")
    C("spec_trailing_space", set_key(t.ann, "spec", "zikaron/1 "), "sec 4.3 step 4: byte-equal",
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
      "sec 1/6.8: 0x21 is the first token byte, and an unrecognized type is an entry all the same",
      expect="ok")
    C("entrytype_tilde", make_entry(K("root"), "~", 1, t.gid, {"note_md": "n"}),
      "sec 1/6.8: 0x7E is the last token byte", expect="ok")
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
    C("step4_before_step5", set_key(set_key(t.ann, "spec", "zikaron/2"), "entryType", ""),
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
    C("sig_galeed_adoption_domain", foreign_domain_sig_entry(t.gid, "zikaron/1-adoption"),
      "sec 5.6: a signature made under the twin law's second domain placed in sig",
      expect="E_SIG_SIGNER")
    C("sig_zikaron2_domain", foreign_domain_sig_entry(t.gid, "zikaron/2"),
      "sec 5.6: a signature under a 75-byte sibling domain; lengths decide nothing",
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

    # 6.5 / 6.6 adoption
    AN = [{"chainId": 1, "tx": HX32(0xAA), "payloadKind": "calldata", "content": HX32(0xC0)}]
    C("body_adoption_ok", t.adopt, "sec 6.5: one anchor element", expect="ok")
    bc("body_adoption_missing_anchors", "adoption", {}, "sec 6.5", expect=E)
    bc("body_adoption_empty_anchors", "adoption", {"anchors": []},
       "sec 6.5: anchors is a non-empty array", expect=E)
    bc("body_adoption_anchors_object", "adoption", {"anchors": AN[0]}, "sec 6.5", expect=E)
    bc("body_adoption_anchors_null", "adoption", {"anchors": None}, "sec 6.5", expect=E)
    bc("body_adoption_element_string", "adoption", {"anchors": ["x"]},
       "sec 6.5: every element an object", expect=E)
    bc("body_adoption_no_chainid", "adoption",
       {"anchors": [{"tx": HX32(1), "payloadKind": "k", "content": HX32(2)}]}, "sec 6.5",
       expect=E)
    bc("body_adoption_no_tx", "adoption",
       {"anchors": [{"chainId": 1, "payloadKind": "k", "content": HX32(2)}]}, "sec 6.5", expect=E)
    bc("body_adoption_no_payloadkind", "adoption",
       {"anchors": [{"chainId": 1, "tx": HX32(1), "content": HX32(2)}]}, "sec 6.5", expect=E)
    bc("body_adoption_no_content", "adoption",
       {"anchors": [{"chainId": 1, "tx": HX32(1), "payloadKind": "k"}]}, "sec 6.5", expect=E)
    bc("body_adoption_chainid_string", "adoption",
       {"anchors": [{"chainId": "1", "tx": HX32(1), "payloadKind": "k", "content": HX32(2)}]},
       "sec 6.5", expect=E)
    bc("body_adoption_payloadkind_empty", "adoption",
       {"anchors": [{"chainId": 1, "tx": HX32(1), "payloadKind": "", "content": HX32(2)}]},
       "sec 1: a token is non-empty", expect=E)
    bc("body_adoption_payloadkind_space", "adoption",
       {"anchors": [{"chainId": 1, "tx": HX32(1), "payloadKind": "a b", "content": HX32(2)}]},
       "sec 1", expect=E)
    bc("body_adoption_second_element_bad", "adoption",
       {"anchors": AN + [{"chainId": 2, "tx": HX32(3), "payloadKind": "k", "content": HX20(1)}]},
       "sec 6.5: the row is applied to every element", expect=E)
    bc("body_adoption_repeat_element", "adoption", {"anchors": AN + AN},
       "sec 6.5: elements may repeat; order is the author's", expect="ok")
    bc("body_adoption_element_extra", "adoption",
       {"anchors": [dict(AN[0], note_md="\x00", deep={"a": [1]})]},
       "sec 6.10: members beyond an anchors element's row", expect="ok")
    bc("body_adoption_attestor_alone", "adoption", {"anchors": AN, "attestor": A_("k2")},
       "sec 6.5: attestor without attestation", expect=E)
    bc("body_adoption_attestation_alone", "adoption",
       {"anchors": AN, "attestation": "0x" + "11" * 65},
       "sec 6.5: attestation without attestor", expect=E)
    bc("body_adoption_both_garbage", "adoption",
       {"anchors": AN, "attestor": A_("k2"), "attestation": "0x" + "11" * 65},
       "sec 6.6: an attestation that fails every test does not void the entry", expect="ok")
    bc("body_adoption_attestor_null", "adoption",
       {"anchors": AN, "attestor": None, "attestation": "0x" + "11" * 65}, "sec 6.5", expect=E)
    bc("body_adoption_attestation_null", "adoption",
       {"anchors": AN, "attestor": A_("k2"), "attestation": None},
       "sec 6: an optional field written null is present with a form its row refuses", expect=E)
    bc("body_adoption_attestation_short", "adoption",
       {"anchors": AN, "attestor": A_("k2"), "attestation": "0x" + "11" * 64}, "sec 1", expect=E)
    C("body_adoption_attested_valid", attested_adoption(t.gid),
      "sec 6.6: a well-formed attestation by a second key", expect="ok")

    # 6.7 succession
    SB = {"to": A_("k2"), "kind": "handover", "effective": 1700000000, "statement_md": "s"}
    C("body_succession_ok", t.succ, "sec 6.7: all four required fields", expect="ok")
    bc("body_succession_no_to", "succession", {k: v for k, v in SB.items() if k != "to"},
       "sec 6.7", expect=E)
    bc("body_succession_no_kind", "succession", {k: v for k, v in SB.items() if k != "kind"},
       "sec 6.7", expect=E)
    bc("body_succession_no_effective", "succession",
       {k: v for k, v in SB.items() if k != "effective"}, "sec 6.7", expect=E)
    bc("body_succession_no_statement", "succession",
       {k: v for k, v in SB.items() if k != "statement_md"}, "sec 6.7", expect=E)
    bc("body_succession_to_hex32", "succession", dict(SB, to=HX32(1)), "sec 1", expect=E)
    bc("body_succession_kind_keyrotation", "succession", dict(SB, kind="keyrotation"),
       "sec 6.7: the grammar names keyrotation and reads nothing", expect="ok")
    bc("body_succession_kind_space", "succession", dict(SB, kind="hand over"),
       "sec 1/6: 0x20 is the one scalar value sec 3.5 admits and a token refuses", expect=E)
    bc("body_succession_kind_unknown", "succession", dict(SB, kind="court-order/2031"),
       "sec 6.7: the grammar refuses no other value", expect="ok")
    bc("body_succession_kind_empty", "succession", dict(SB, kind=""), "sec 1", expect=E)
    bc("body_succession_kind_int", "succession", dict(SB, kind=1), "sec 6.7", expect=E)
    bc("body_succession_effective_zero", "succession", dict(SB, effective=0),
       "sec 6.7: Unix seconds zero", expect="ok")
    bc("body_succession_effective_string", "succession", dict(SB, effective="1700000000"),
       "sec 6.7", expect=E)
    bc("body_succession_effective_null", "succession", dict(SB, effective=None), "sec 6",
       expect=E)
    bc("body_succession_to_self", "succession", dict(SB, to=A_("root")),
       "sec 6.7: to may equal author", expect="ok")
    bc("body_succession_statement_int", "succession", dict(SB, statement_md=1), "sec 6.7",
       expect=E)

    # 6.8 annotation
    C("body_annotation_ok", t.ann, "sec 6.8: note_md present", expect="ok")
    bc("body_annotation_no_note", "annotation", {}, "sec 6.8", expect=E)
    bc("body_annotation_note_empty", "annotation", {"note_md": ""},
       "sec 6.11: the empty prose string is a legal spelling of a prose string row", expect="ok")
    bc("body_annotation_subject_ok", "annotation", {"note_md": "n", "subject": HX32(3)},
       "sec 6.8: the optional subject", expect="ok")
    bc("body_annotation_subject_null", "annotation", {"note_md": "n", "subject": None},
       "sec 6", expect=E)
    bc("body_annotation_subject_hex20", "annotation", {"note_md": "n", "subject": HX20(3)},
       "sec 1", expect=E)
    bc("body_annotation_note_object", "annotation", {"note_md": {"a": "b"}}, "sec 6.8", expect=E)
    bc("body_annotation_note_null", "annotation", {"note_md": None}, "sec 6", expect=E)

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
    valid_pool = [t.gen, t.hist, t.grant, t.rev, t.succ, t.ann, t.adopt, t.unk, maxent]
    types = ["genesis", "history", "grant", "revocation", "adoption", "succession",
             "annotation", "windmill", "sale/2031", "x"]
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


def craft(entryType, seq, prev, body, sigval, author=None, spec="zikaron/1"):
    if author is None:
        author = A_("root")
    bv = body if isinstance(body, (JObj, list)) else J(body)
    if isinstance(body, list):
        bv = J(body)
    env6 = build_env(spec, entryType, author, seq, prev, bv)
    return cbytes(JObj(env6.members + [("sig", sigval)]))


def adoption_domain_sig_entry(prev):
    return _other_domain_entry(prev, "zikaron/1-adoption", "signed under the adoption domain")


def find_v28(prev):
    for i in range(100000):
        env6 = build_env("zikaron/1", "annotation", A_("root"), 1, prev,
                         J({"n": i, "note_md": "seeking v=28"}))
        digest = eip191_digest("zikaron/1", sha256(cbytes(env6)))
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
    env6 = build_env("zikaron/1", "annotation", A_("root"), 1, prev,
                     J({"note_md": "sR equals zG"}))
    digest = eip191_digest("zikaron/1", sha256(cbytes(env6)))
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
    env6 = build_env("zikaron/1", "annotation", A_("root"), 1, prev, J({"note_md": note}))
    presig = sha256(cbytes(env6))
    r, s, v = ecdsa_sign(K("root"), eip191_digest(domain, presig))
    return cbytes(JObj(env6.members + [("sig", sig_hex(r, s, v))]))


def foreign_domain_sig_entry(prev, domain):
    return _other_domain_entry(prev, domain, "signed under " + domain)


def sig_over_other_b6(prev):
    other = build_env("zikaron/1", "annotation", A_("root"), 1, prev, J({"note_md": "other"}))
    r, s, v = ecdsa_sign(K("root"), eip191_digest("zikaron/1", sha256(cbytes(other))))
    env6 = build_env("zikaron/1", "annotation", A_("root"), 1, prev, J({"note_md": "this"}))
    return cbytes(JObj(env6.members + [("sig", sig_hex(r, s, v))]))


def attested_adoption(prev, anchors=None, attestor_key="k2", author_key="root",
                      seq=1, attest_prev=None, corrupt=False, attestor_addr=None):
    if anchors is None:
        anchors = [{"chainId": 1, "tx": HX32(0xAA), "payloadKind": "calldata",
                    "content": HX32(0xC0)}]
    author = A_(author_key)
    att_sig, _, _, _ = sign_attestation(K(attestor_key), author, anchors,
                                        prev if attest_prev is None else attest_prev)
    if corrupt:
        raw = bytearray(bytes.fromhex(att_sig[2:]))
        raw[5] ^= 0x01
        att_sig = hx(bytes(raw))
    body = {"anchors": anchors,
            "attestor": attestor_addr if attestor_addr else A_(attestor_key),
            "attestation": att_sig}
    return make_entry(K(author_key), "adoption", seq, prev, body)


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
        return {"statement_md": rand_prose(rng)}
    if etype == "history":
        b = {"content": HX32(rng.getrandbits(256)),
             "mode": {"mark": rand_token(rng), "toolchain": HX32(rng.getrandbits(256))}}
        if rng.random() < 0.5:
            b["note_md"] = rand_prose(rng)
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
        return b
    if etype == "revocation":
        b = {"grant": HX32(rng.getrandbits(256))}
        if rng.random() < 0.5:
            b["case"] = HX32(rng.getrandbits(256))
        return b
    if etype == "adoption":
        n = rng.randrange(1, 4)
        return {"anchors": [{"chainId": rng.randrange(0, 1 << 20),
                             "tx": HX32(rng.getrandbits(256)),
                             "payloadKind": rand_token(rng),
                             "content": HX32(rng.getrandbits(256))} for _ in range(n)]}
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
    def b6(etype, seq, prev, body, author_key="root", spec="zikaron/1"):
        return cbytes(build_env(spec, etype, A_(author_key), seq, prev, J(body)))

    P0 = HX32(0x11)
    S("genesis", K("root"), b6("genesis", 0, None, {"statement_md": "the works of one author"}),
      "zikaron/1", "sec 5.1: B6 of a genesis; prev null, seq 0")
    S("history", K("root"),
      b6("history", 1, P0, {"content": HX32(0xC0), "mode": {"mark": "hand", "toolchain": HX32(7)}}),
      "zikaron/1", "sec 5.1: B6 of a history entry")
    S("grant", K("root"),
      b6("grant", 2, P0, {"grantee": HX20(0xBEEF), "work": HX32(0xC0), "terms": HX32(0x7E),
                          "window": {"from": 0, "to": MAXINT}}),
      "zikaron/1", "sec 5.1: B6 of a grant carrying both ends of the integer universe")
    S("revocation", K("root"), b6("revocation", 3, P0, {"grant": HX32(0xAB), "case": HX32(0xCD)}),
      "zikaron/1", "sec 5.1: B6 of a revocation with a cited case")
    S("adoption", K("root"),
      b6("adoption", 4, P0, {"anchors": [{"chainId": 1, "tx": HX32(0xAA),
                                          "payloadKind": "calldata", "content": HX32(0xC0)}]}),
      "zikaron/1", "sec 5.1: B6 of an adoption")
    S("succession", K("root"),
      b6("succession", 5, P0, {"to": A_("k2"), "kind": "handover", "effective": 0,
                               "statement_md": "the seat moves"}),
      "zikaron/1", "sec 5.1: B6 of a succession")
    S("annotation", K("root"), b6("annotation", 6, P0, {"note_md": "a note"}),
      "zikaron/1", "sec 5.1: B6 of an annotation")
    S("unknown_type", K("root"), b6("windmill", 7, P0, {}),
      "zikaron/1", "sec 6.9: B6 of an unknown type with an empty body")
    S("max_seq", K("root"), b6("annotation", MAXINT, P0, {"note_md": "top"}),
      "zikaron/1", "sec 3.2: seq at 2^53 - 1 inside the preimage")
    S("prose_controls", K("root"),
      b6("annotation", 8, P0, {"note_md": "\x00\x01\x08\x09\x0a\x0c\x0d\x1f\x7f\x80"}),
      "zikaron/1", "sec 3.4 item 3: every escape form inside the preimage")
    S("prose_astral", K("root"),
      b6("annotation", 9, P0, {"note_md": "\U0001f600\U0010ffff中é‮"}),
      "zikaron/1", "sec 3.4 item 3: astral and bidi scalars raw in the preimage")
    S("deep_body", K("root"), b6("windmill", 10, P0, {"d": deep_list(120)}),
      "zikaron/1", "sec 3.4: a deeply nested body inside B6")
    S("member_order", K("root"),
      b6("windmill", 11, P0, {"a b": 1, "ab": 2, "A": 3, "a": 4, "~": 5, " ": 6}),
      "zikaron/1", "sec 3.4 item 5: bytewise key order inside the preimage")
    S("body_named_sig", K("root"),
      b6("windmill", 11, P0, {"sig": "0x" + "11" * 65, "body": {"seq": 1}}),
      "zikaron/1", "sec 5.1: a member named sig inside body sits inside B6")
    S("key_one", 1, b6("annotation", 12, P0, {"note_md": "smallest private key"}, "root"),
      "zikaron/1", "sec 5: private key 1")
    S("key_n_minus_1", N - 1, b6("annotation", 13, P0, {"note_md": "largest private key"}, "root"),
      "zikaron/1", "sec 5: private key n - 1")
    S("key_half_n", HALF_N, b6("annotation", 14, P0, {"note_md": "key at (n-1)/2"}, "root"),
      "zikaron/1", "sec 5: private key (n - 1) / 2")
    for i in range(8):
        S("rand_b6_%d" % i, testkey("sign", 100 + i),
          b6(["genesis", "history", "grant", "revocation", "adoption", "succession",
              "annotation", "windmill"][i],
             0 if i == 0 else i, None if i == 0 else HX32(rng.getrandbits(256)),
             random_body(rng, ["genesis", "history", "grant", "revocation", "adoption",
                               "succession", "annotation", "windmill"][i])),
          "zikaron/1", "randomized B6 with a randomized private key")

    # sec 6.6 attestation preimages under the second domain
    AN1 = [{"chainId": 1, "tx": HX32(0xAA), "payloadKind": "calldata", "content": HX32(0xC0)}]
    AN2 = AN1 + [{"chainId": 8453, "tx": HX32(0xBB), "payloadKind": "log", "content": HX32(0xD0)}]
    S("att_one_anchor", K("k2"), cbytes(attestation_C(A_("root"), AN1, HX32(0x11))),
      "zikaron/1-adoption", "sec 6.6: C over one anchor element; the 85-byte message")
    S("att_two_anchors", K("k2"), cbytes(attestation_C(A_("root"), AN2, HX32(0x12))),
      "zikaron/1-adoption", "sec 6.6: C over two elements on two chains")
    S("att_repeated_anchor", K("k2"), cbytes(attestation_C(A_("root"), AN1 + AN1, HX32(0x13))),
      "zikaron/1-adoption", "sec 6.6: C where the anchors array repeats one element")
    S("att_extra_member", K("k2"),
      cbytes(attestation_C(A_("root"), [dict(AN1[0], note_md="\x00", z={"a": [1]})], HX32(0x14))),
      "zikaron/1-adoption",
      "sec 6.10/6.6: an extra member inside an anchors element sits inside C")
    S("att_other_adopter", K("k2"), cbytes(attestation_C(A_("k3"), AN1, HX32(0x11))),
      "zikaron/1-adoption", "sec 6.6: the same anchors under a different adopter")
    S("att_other_prev", K("k2"), cbytes(attestation_C(A_("root"), AN1, HX32(0x99))),
      "zikaron/1-adoption", "sec 6.6: the same anchors written against a different head")
    S("att_zero_prev", K("k2"), cbytes(attestation_C(A_("root"), AN1, HX32(0))),
      "zikaron/1-adoption", "sec 6.11: the all-zero prev is a legal hex32")
    S("att_key_one", 1, cbytes(attestation_C(A_("root"), AN1, HX32(0x15))),
      "zikaron/1-adoption", "sec 5: attestation preimage under private key 1")
    for i in range(6):
        S("rand_att_%d" % i, testkey("sign-att", 200 + i),
          cbytes(attestation_C(HX20(rng.getrandbits(160)),
                               [{"chainId": rng.randrange(1 << 20),
                                 "tx": HX32(rng.getrandbits(256)),
                                 "payloadKind": rand_token(rng),
                                 "content": HX32(rng.getrandbits(256))}
                                for _ in range(rng.randrange(1, 4))],
                               HX32(rng.getrandbits(256)))),
          "zikaron/1-adoption", "randomized attestation preimage C")
    S("other_literal", K("root"), cbytes(J({"note_md": "a client's document"})),
      "client.doc/1", "sec 5.6/5.7: a document under a literal no domain of this law spells")
    S("domain_tab", K("root"), cbytes(J({"note_md": "a client's document"})),
      "client.doc/1\t", "HARNESS: a domain is any bytes below 0x80 without a line feed; a tab is signed as given")
    S("domain_del", K("root"), cbytes(J({"note_md": "a client's document"})),
      "client.doc/1\x7f", "HARNESS: a domain is any bytes below 0x80 without a line feed; 0x7F is signed as given")



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


def ADB(i):
    return {"anchors": [{"chainId": 1, "tx": HX32(0xAA00 + i), "payloadKind": "calldata",
                         "content": HX32(i)}]}


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


def basis(chains=(), bare=(), adopt=(), raw=False):
    """A sec 9.4 basis; arrays in the order the section fixes unless raw."""
    ch, bt, ac = list(chains), list(bare), list(adopt)
    if not raw:
        ch = sorted(ch, key=lambda c: c["chainId"])
        bt = sorted(bt, key=lambda o: (o["chainId"], o["tx"].encode()))
        ac = sorted(ac, key=lambda o: o["chainId"])
    return {"chains": ch, "bareTx": bt, "adoptionChains": ac}


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


def ev(cid, tx, sender, calldata):
    return {"chainId": cid, "tx": tx, "sender": sender, "calldata": calldata}


def inp(root, pile, anchors=(), unavailable=(), evidence=(), bas=None):
    return {"root": root, "pile": [hx(b) for b in pile], "anchors": list(anchors),
            "unavailable": list(unavailable), "evidence": list(evidence),
            "basis": bas if bas is not None else basis()}


def calldata_at(content_hex, offset, partial=False):
    c = bytes.fromhex(content_hex[2:])
    pre = bytes((i * 7 + 3) % 256 for i in range(offset))
    if partial:
        return hx(pre + c[:16])
    return hx(pre + c + b"\x22" * 8)


ROOTB = None   # set by build_audit once the keys exist: one chain, every hand-case sender


def build_audit(rng, t):
    global ROOTB
    R = A_("root")
    ROOTB = basis((chainobj(1, senders=(R, A_("k2"), A_("k3"), A_("k9"))),), (),
                  ({"chainId": 1, "throughBlock": 1000},))
    CL, CID = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1)),
                           ("root", "grant", GRB(2)), ("root", "annotation", ANNB("a note")),
                           ("root", "revocation", RVB(4))])

    A("clean_ledger", inp(R, CL), "A clean five-entry ledger under one key: no anchors, no "
      "gaps, no forks. sec 8.7 label 4.", "COMPLETE")
    A("empty_pile", inp(R, []), "An empty pile. sec 8.7: COMPLETE asserts only that the inputs "
      "held no fault, and the entry count travels with it.", "COMPLETE")
    FOR, _ = build_chain([("k9", "genesis", GENB), ("k9", "history", HISB(3)),
                          ("k9", "annotation", ANNB("elsewhere"))])
    A("only_foreign_entries", inp(R, FOR),
      "A pile holding only entries signed outside the audited lineage. sec 8.1: they are not "
      "input; sec 8.7 item 7 lists them as EXCLUDED and no label turns on it.", "COMPLETE")
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
      "sec 6.7/7.3: a succession whose `to` equals the key already in office is legal and "
      "authority is unchanged.", "COMPLETE")
    SP, _ = build_chain([("root", "genesis", GENB), ("root", "succession", SUCB(A_("k2"))),
                         ("k2", "annotation", ANNB("k2 writes")),
                         ("k2", "succession", SUCB(R)), ("root", "annotation", ANNB("back"))])
    A("succession_to_prior_holder", inp(R, SP),
      "sec 7.3: a succession back to a key that held office before; authority is a function of "
      "the prefix and switches as written.", "COMPLETE")
    SL, _ = build_chain([("root", "genesis", GENB), ("root", "annotation", ANNB("one")),
                         ("root", "succession", SUCB(A_("k2")))])
    A("succession_at_last_position", inp(R, SL),
      "sec 6.7: the ledger's last entry hands the seat off and nothing follows; silence is the "
      "ordinary end.", "COMPLETE")
    A("withheld_succession", inp(R, [SU[0]] + SU[2:]),
      "sec 8.7: the successor's entries are present and the succession that would admit them is "
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
      "sec 9.3/8.7: an UNPROVEN record takes no part in the reconciliation and is carried in "
      "the UNPROVEN list; its hash is in have, so label rule 2 does not fire.", "COMPLETE")
    A("anchor_verdict_void",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="VOID")], bas=ROOTB),
      "sec 9.3/8.7: a VOID record is informational, takes no part in the reconciliation, and no "
      "label turns on it, even when its hash is in no ledger entry.", "COMPLETE")
    A("anchor_verdict_void_and_unproven",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="VOID"),
                  anchor(HX32(0xF00D), R, bn=12, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.7: both lists non-empty; only the UNPROVEN list moves the label.", "UNAVAILABLE")
    A("anchor_sender_outside_lineage",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9"))], bas=ROOTB),
      "sec 8.1: the audit discards every anchor record whose sender is not in the lineage, so "
      "no key outside it can put a hash into this ledger's reconciliation.",
      "COMPLETE")
    A("anchor_sender_retired_lineage_key",
      inp(R, SU, [anchor(eid(SU[2]), R)], bas=ROOTB),
      "sec 7.4/8.1: every lineage key can anchor, whenever it sent the "
      "transaction; the retired root's anchor of the successor's entry counts.", "COMPLETE")
    A("anchor_sender_not_in_basis_senders",
      inp(R, CL, [anchor(CID[0], R)],
          bas=basis((chainobj(1, senders=(A_("k2"),)),), ({"chainId": 1, "tx": HX32(0xA0064)},))),
      "sec 9.4: `senders` bounds the registry-form scan and nothing else; a record the basis "
      "reaches through `bareTx` is admitted whatever its sender, and sec 8.1 re-filters neither.",
      "COMPLETE")
    A("anchor_outside_basis_reach",
      inp(R, CL, [anchor(CID[0], R)], bas=basis((chainobj(1, senders=(A_("k2"),)),), ())),
      "sec 9.4 [C]: a record no `bareTx` object names and no `chains` object reaches (its sender "
      "outside that object's `senders`) is a record no scan of that basis produces; no label.",
      "NO_LABEL")
    A("anchor_outside_basis_range",
      inp(R, CL, [anchor(CID[0], R, bn=2000)], bas=ROOTB),
      "sec 9.4 [C]: a record whose blockNumber lies outside every `chains` object's range of its "
      "chain, and whose transaction `bareTx` does not name, is outside the basis's reach; no label.",
      "NO_LABEL")
    A("anchor_outside_basis_chain",
      inp(R, CL, [anchor(CID[0], R, cid=10)], bas=ROOTB),
      "sec 9.4 [C]: a record on a chain the basis does not declare is outside its reach; no label.",
      "NO_LABEL")
    A("missing_on_two_chains",
      inp(R, CL, [anchor(H_UNSEEN, R, cid=1, bn=50, ts=1700000000, tx=HX32(0x501)),
                  anchor(H_UNSEEN, R, cid=8453, bn=10, ts=1699000000, tx=HX32(0x101))],
          bas=basis((chainobj(1, senders=(R,)), chainobj(8453, senders=(R,))), ())),
      "sec 8.7 item 4: one MISSING row carrying both anchoring transactions in ascending "
      "(chainId, blockNumber, tx) order; sec 9.2: the smaller blockTimestamp gives the "
      "existence bound and blockNumber never orders across chains.", "GAPS")
    A("anchored_hash_missing", inp(R, CL, [anchor(H_UNSEEN, R)], bas=ROOTB),
      "sec 8.5 forward: a counted hash that is neither in `have` nor unavailable.", "GAPS")
    A("anchored_hash_unavailable",
      inp(R, CL, [anchor(H_UNSEEN, R)], [H_UNSEEN], bas=ROOTB),
      "sec 8.6: the verifier attempted retrieval and did not obtain usable bytes.",
      "UNAVAILABLE")
    A("unavailable_not_in_counted_set",
      inp(R, CL, [anchor(CID[0], R)], [H_UNSEEN], bas=ROOTB),
      "sec 8.1: the audit discards from the unavailable set every hash that is not the hash of "
      "a remaining counted record.", "COMPLETE")
    A("unavailable_of_unproven_anchor",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="UNPROVEN")], [H_UNSEEN], bas=ROOTB),
      "sec 8.1/9.3: the hash leaves the unavailable set because its record is not counted, and "
      "the UNPROVEN list still moves the label.", "UNAVAILABLE")
    A("anchored_excluded_entry",
      inp(R, CL + [FOR[0]], [anchor(eid(FOR[0]), R)], bas=ROOTB),
      "sec 8.5: `have` holds ledger entry_ids and nothing else, so bytes held under a "
      "signature outside the lineage are MISSING while the bytes appear in EXCLUDED.", "GAPS")
    A("anchored_excluded_entry_also_unavailable",
      inp(R, CL + [FOR[0]], [anchor(eid(FOR[0]), R)], [eid(FOR[0])], bas=ROOTB),
      "sec 8.1: a hash that is the entry_id of a byte string in the pile leaves the unavailable "
      "set, whatever any other source answered, so the row stays MISSING.", "GAPS")
    BADB = bytearray(CL[1]); BADB[len(BADB) // 2] ^= 0x01; BADB = bytes(BADB)
    A("anchored_bytes_fail_4_3",
      inp(R, CL + [BADB], [anchor(eid(BADB), R)], bas=ROOTB),
      "sec 8.6: a byte string that was retrieved and fails sec 4.3 is not unavailable; it is "
      "not input, and its anchor lands in MISSING.", "GAPS")
    A("anchored_bytes_fail_4_3_also_unavailable",
      inp(R, CL + [BADB], [anchor(eid(BADB), R)], [eid(BADB)], bas=ROOTB),
      "sec 8.1/8.6: the same, with the hash also declared unavailable; bytes in hand are in "
      "hand.", "GAPS")
    A("unavailable_beats_gaps",
      inp(R, CL[:2] + CL[3:], [anchor(H_UNSEEN, R)], [H_UNSEEN], bas=ROOTB),
      "sec 8.7 label order: rule 2 fires before rule 3 even though a SEQ_GAP was recorded.",
      "UNAVAILABLE")
    A("hard_beats_unavailable",
      inp(R, CL[:3] + [tw_a, tw_b], [anchor(H_UNSEEN, R)], [H_UNSEEN], bas=ROOTB),
      "sec 8.7 label order: rule 1 fires first.", "BROKEN_CHAIN")

    # ---------------- pile shape, basis shape, duplicate records -------------
    A("duplicates_in_pile", inp(R, CL + CL),
      "sec 8.1: byte-identical copies collapse to one entry, and the report does not depend on "
      "the order in which any input was supplied.", "COMPLETE")
    A("pile_of_malformed_only", inp(R, [BADB, b"{}", b"not json at all"]),
      "sec 8.1: byte strings that fail sec 4.3 are no ledger entries: they enter no walk, no "
      "have and no EXCLUDED row, and sec 8.7 item 10 lists each with its token.", "COMPLETE")
    XG, XI = build_chain([("root", "genesis", GENB), ("root", "history", HISB(1))])
    A2 = mk("root", "annotation", 2, XI[1], ANNB("first on this head"))
    both_orders("equivocation_shared_prev",
                lambda k: (lambda b: (inp(R, XG + [A2, b]), eid(A2) < eid(b)))(
                    mk("root", "annotation", 3, XI[1], ANNB("second on this head %d" % k))),
                "sec 8.4: two ledger entries share a prev at seqs 2 and 3; the finding names the "
                "bytewise-smaller entry_id first whichever entry the walk met first.")
    WF = inp(R, CL)
    A_raw("input_not_json", b"not a json text at all",
          "HARNESS: the audit input is not a JSON text; no label.")
    A_raw("input_not_utf8", b"\xff\xfe" + json.dumps(WF).encode("utf-8"),
          "HARNESS: the audit input is not valid UTF-8; no label.")
    A_raw("input_duplicate_member",
          json.dumps(WF)[:-1].encode("utf-8") + b', "root": "' + R.encode() + b'"}',
          "HARNESS: the input reader refuses an object that repeats a member name; no label.")
    A_raw("input_float_member", json.dumps(WF)[:-1].encode("utf-8") + b', "extra": 1.5}',
          "HARNESS: the input reader admits only integers of sec 3.1's universe, in any member; "
          "no label.")
    A_raw("input_nan_constant", json.dumps(WF)[:-1].encode("utf-8") + b', "extra": NaN}',
          "HARNESS: NaN is no value the input reader admits; no label.")
    A("pile_element_without_0x", dict(WF, pile=[WF["pile"][0][2:]] + WF["pile"][1:]),
      "HARNESS: a pile element that does not begin with 0x; no label.", "NO_LABEL")
    A("pile_element_nonhex", dict(WF, pile=["0xzz" + WF["pile"][0][4:]] + WF["pile"][1:]),
      "HARNESS: a pile element carrying a non-hex character; no label.", "NO_LABEL")
    A("root_mismatch_lone_foreign_genesis",
      inp(R, [gen2, mk("root", "succession", 1, eid(gen2), SUCB(A_("k2")))]),
      "sec 8.2: ROOT_MISMATCH is tested over every seq-0 ledger entry.", "BROKEN_CHAIN")

    A("basis_chainid_in_both_arrays",
      inp(R, CL, [anchor(CID[0], R)],
          bas=basis((chainobj(1, senders=(R,)),), ({"chainId": 1, "tx": HX32(0x1)},))),
      "sec 9.4: one chainId named in both arrays is ordinary and is not a second object.",
      "COMPLETE")
    A("basis_extra_top_member",
      dict(inp(R, CL), basis=dict(basis((chainobj(1),)), note="scan of 2031-04")),
      "sec 9.4: the basis is an object with exactly these members; sec 6.10 reaches no object "
      "of this section.", "NO_LABEL")
    A("basis_chains_object_extra_member",
      inp(R, CL, bas=basis((dict(chainobj(1), rpc="https://example"),))),
      "sec 9.4: an object bearing a member beyond its list is not a zikaron/1 basis.",
      "NO_LABEL")
    A("basis_two_windows_one_block_apart",
      inp(R, CL, bas=basis((chainobj(1, tb=100, senders=(R,)), chainobj(1, fb=102, tb=300, senders=(R,))))),
      "sec 9.4: ranges one block apart are not adjacent, so equal lists are a basis; the inside "
      "edge of the adjacency clause.", "COMPLETE")
    A("basis_two_baretx_one_pair",
      inp(R, CL, bas=basis((chainobj(1),), ({"chainId": 1, "tx": HX32(5)},
                                            {"chainId": 1, "tx": HX32(5)}))),
      "sec 9.4: at most one bareTx object per (chainId, tx).", "NO_LABEL")
    A("input_anchor_verdict_outside_set",
      inp(R, CL, [dict(anchor(CID[0], R), verdict="maybe")], bas=ROOTB),
      "sec 9.4: an anchor record whose verdict is outside the three values is not an audit input.",
      "NO_LABEL")
    A("input_anchor_sender_not_hex20",
      inp(R, CL, [dict(anchor(CID[0], R), sender="0xabc")],
          bas=basis((chainobj(1, senders=(R,)),), ({"chainId": 1, "tx": HX32(0xA0064)},))),
      "sec 9.4: a record any of whose seven members fails the form sec 9.2 names is not an audit input.",
      "NO_LABEL")
    A("input_root_uppercase",
      dict(inp(R, CL), root="0x" + R[2:].upper()),
      "sec 8 with sec 1: the root is hex20, lowercase; another spelling is not an audit input.",
      "NO_LABEL")
    A("input_pile_absent",
      {k: v for k, v in inp(R, CL).items() if k != "pile"},
      "sec 8: the audit has five inputs; one absent is no audit input.",
      "NO_LABEL")
    A("input_anchors_not_array",
      dict(inp(R, CL), anchors={}),
      "sec 8: the anchor set is a set of records.",
      "NO_LABEL")
    A("input_pile_odd_hex",
      dict(inp(R, CL), pile=inp(R, CL)["pile"] + ["0xabc"]),
      "sec 8: a pile element that is not whole bytes is no byte string.",
      "NO_LABEL")
    A("input_unavailable_not_hex32",
      dict(inp(R, CL), unavailable=["0x12"]),
      "sec 8.6: the unavailable set holds hashes.",
      "NO_LABEL")
    A("input_extra_member_ignored",
      dict(inp(R, CL), extra=[{"chainId": 1, "tx": HX32(7)}]),
      "sec 8 with HARNESS: the audit has five inputs and the file six members; a seventh is not one of them, "
      "and nothing in this law refuses an input object for carrying it.",
      "COMPLETE")
    A("input_anchor_record_extra_member",
      inp(R, CL, [dict(anchor(CID[0], R), note="x")], bas=ROOTB),
      "sec 8: a record with a member beyond its seven is read by its seven; nothing in the law refuses it.",
      "COMPLETE")
    A("basis_two_baretx_different_chains",
      inp(R, CL, bas=basis((chainobj(1),), ({"chainId": 1, "tx": HX32(5)},
                                            {"chainId": 8453, "tx": HX32(5)}))),
      "sec 9.4: the same tx hash looked for on two chains is two objects and is legal.",
      "COMPLETE")
    A("basis_missing_member",
      dict(inp(R, CL), basis={"chains": [], "adoptionChains": []}),
      "sec 9.4: exactly these members; bareTx absent alone.", "NO_LABEL")
    A("basis_registry_uppercase",
      inp(R, CL, bas=basis((chainobj(1, regs=("0x" + "A" * 40,)),))),
      "sec 9.4/sec 1: registries is an array of hex20.", "NO_LABEL")
    A("basis_fromblock_string",
      inp(R, CL, bas=basis((dict(chainobj(1), fromBlock="0"),))),
      "sec 9.4: fromBlock is an int.", "NO_LABEL")
    A("basis_empty_covers_no_chain", inp(R, CL, bas=basis()),
      "sec 8.7: under a basis that covers no chain a COMPLETE label says nothing about "
      "anchoring, which is why the basis travels with the label.", "COMPLETE")

    A("dup_anchor_records_agree",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900)), anchor(CID[0], R, tx=HX32(0x900))],
          bas=ROOTB),
      "sec 9.4: the anchor set holds at most one record per (chainId, blockNumber, tx, hash); "
      "two records that agree on every remaining member are one record.", "COMPLETE")
    A("dup_anchor_records_disagree_timestamp",
      inp(R, CL, [anchor(CID[0], R, tx=HX32(0x900), ts=1),
                  anchor(CID[0], R, tx=HX32(0x900), ts=2)], bas=ROOTB),
      "sec 9.4: two records agreeing on (chainId, blockNumber, tx, hash) and disagreeing on blockTimestamp are not a zikaron/1 audit input.",
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
      "sec 9.4: fromBlock above toBlock; the object is not a zikaron/1 basis.", "NO_LABEL")
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
    A("basis_baretx_unsorted",
      inp(R, CL, bas=basis((chainobj(1),), ({"chainId": 1, "tx": HX32(9)}, {"chainId": 1, "tx": HX32(8)}),
                           raw=True)),
      "sec 9.4: bareTx is ordered by (chainId, tx) ascending.", "NO_LABEL")
    A("basis_arrays_sorted_two_chains",
      inp(R, CL, bas=basis((chainobj(8453, senders=(HX20(9), HX20(3))), chainobj(1, regs=(HX20(5), HX20(4)))))),
      "sec 9.4: the helper's ordering, two chains and two-element arrays each in order.", "COMPLETE")
    A("unavailable_hash_is_ledger_entry",
      inp(R, CL, [anchor(CID[0], R)], [CID[0]], bas=ROOTB),
      "sec 8.1: a hash that is the entry_id of a ledger entry in the pile leaves the unavailable "
      "set; bytes in hand are in hand.", "COMPLETE")
    A("missing_two_transactions_one_chain",
      inp(R, CL, [anchor(H_UNSEEN, R, cid=1, bn=50, tx=HX32(0x502)),
                  anchor(H_UNSEEN, R, cid=1, bn=50, tx=HX32(0x501))], bas=ROOTB),
      "sec 9.2/8.7 item 4: two anchors of one hash in two transactions of one chain; one "
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
      "HARNESS: a number at or above 2^53 anywhere in the file makes it unreadable, here in a record's blockTimestamp, so sec 9.2's form test is never reached.",
      "NO_LABEL")
    A("pile_empty_byte_string", inp(R, CL + [b""]),
      "sec 8.1/8.7: the empty byte string fails sec 4.3 as E_JSON and lands in MALFORMED with "
      "its token; nothing else changes.", "COMPLETE")
    A("malformed_pile_bytes_listed", inp(R, CL + [BADB, b"{}"]),
      "sec 8.7 item 10: every byte string that fails sec 4.3 is listed once with its token.",
      "COMPLETE")
    A("malformed_anchored_row_and_missing",
      inp(R, CL + [BADB], [anchor(eid(BADB), R)], bas=ROOTB),
      "sec 8.7: the anchor of retrieved bytes that fail sec 4.3 is MISSING, and the MALFORMED "
      "row makes the two situations distinguishable.", "GAPS")
    UNK, _ = build_chain([("root", "genesis", GENB), ("root", "dispatch", {"case": HX32(1), "note_md": "n"}),
                          ("root", "annotation", ANNB("after")), ("root", "pass/1", {})])
    A("unknown_types_listed", inp(R, UNK),
      "sec 6.9/8.7 item 9: every ledger entry of a type outside the seven is listed with its "
      "seq and type, sorted by (seq, entryType, entry_id).", "COMPLETE")
    A("unknown_types_sorted_by_type",
      inp(R, UNK + [mk("root", "aardvark", 1, eid(UNK[0]), {})]),
      "sec 8.7 item 9: two unknown types at one seq sort by entryType bytewise; the fork is "
      "hard all the same.", "BROKEN_CHAIN")
    A("unproven_duplicate_of_counted_hash",
      inp(R, CL, [anchor(CID[0], R, bn=10), anchor(CID[0], R, bn=20, tx=HX32(0x777), verdict="UNPROVEN")],
          bas=ROOTB),
      "sec 8.7 item 15 rule 2: an UNPROVEN record whose hash is already counted moves no label; "
      "the row is still carried.", "COMPLETE")
    A("unproven_of_hash_in_have_not_counted",
      inp(R, CL, [anchor(CID[0], R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.7 item 15 rule 2: an UNPROVEN record whose hash is in have and counted nowhere "
      "moves no label either; the entry is UNANCHORED and the reader sees why.", "COMPLETE")
    A("unproven_of_unseen_hash_moves_label",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.7 item 15 rule 2: an UNPROVEN record whose hash is neither in have nor counted "
      "is the case the label answers.", "UNAVAILABLE")
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
      "sec 8.7 item 14: every anchor record the lineage trim discarded is a DISCARDED row "
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
      "sec 8.7 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of a pile byte "
      "string held under a foreign signature moves no label; bytes in hand are in hand.",
      "COMPLETE")
    JUNK = b'{"junk":1}'
    A("unproven_of_malformed_bytes",
      inp(R, CL + [JUNK], [anchor(eid(JUNK), R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.7 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of a pile byte "
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
      "HARNESS: every number in the audit input file is an integer of sec 3.1's universe, in a "
      "member the reader ignores as anywhere else; -1 makes the file unreadable.", "NO_LABEL")
    A("input_ignored_member_number_above_universe", dict(inp(R, CL), extra=[{"n": 2 ** 53}]),
      "HARNESS: 2^53 in an ignored member is outside sec 3.1's universe and the file is refused.",
      "NO_LABEL")
    A("input_ignored_member_lone_surrogate", dict(inp(R, CL), extra=["\ud800"]),
      "HARNESS: a lone surrogate escape names no scalar value; the file is refused, in an "
      "ignored member as anywhere else.", "NO_LABEL")
    A("input_ignored_member_surrogate_pair", dict(inp(R, CL), extra=["\U0001f600"]),
      "HARNESS: a surrogate pair is RFC 8259's spelling of one scalar value above U+FFFF and "
      "is read as that value; the report is the clean ledger's.", "COMPLETE")
    deep = 7
    for _ in range(126):
        deep = [deep]
    A("input_ignored_member_depth_128", dict(inp(R, CL), extra=[deep]),
      "HARNESS: the file carries sec 3.5's depth bound; a container opening at depth 128 inside "
      "an ignored member is read, and the report is the clean ledger's.", "COMPLETE")
    A("input_ignored_member_depth_129", dict(inp(R, CL), extra=[[deep]]),
      "HARNESS: a container opening at depth 129, the root counted as depth 1, makes the file "
      "unreadable, in an ignored member as anywhere else; no reader needs an unbounded stack.",
      "NO_LABEL")
    A_raw("input_ignored_member_leading_zero",
          json.dumps(dict(inp(R, CL), extra=[{"n": 7}])).replace('"n": 7', '"n": 07').encode(),
          "HARNESS: a leading zero is not sec 3.2's spelling of an integer, in an ignored member "
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
      "sec 8.7 item 5 with sec 9.4: two chains carry one entry_id; the dedup key holds chainId, "
      "so records differing only there are two records, and the row's anchors sort by chainId "
      "first.", "COMPLETE")
    A("anchored_and_missing_together",
      inp(R, CL, [anchor(CID[0], R, bn=100), anchor(H_UNSEEN, R, bn=101)], bas=ROOTB),
      "sec 8.7 item 5: the partition on the face of the report, ANCHORED and MISSING both "
      "non-empty in one report.", "GAPS")
    A("void_of_ledger_entry_id",
      inp(R, CL, [anchor(CID[0], R, verdict="VOID")], bas=ROOTB),
      "sec 9.3/8.5: a VOID record takes no part in the reconciliation, so the entry it names "
      "is UNANCHORED and no MISSING row follows.", "COMPLETE")
    A("unproven_sender_outside_lineage",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9"), verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.1/8.7 item 12: a record whose sender is outside the lineage is discarded before "
      "the UNPROVEN list is built, so it moves no label; it is a DISCARDED row.", "COMPLETE")
    A("void_sender_outside_lineage",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9"), verdict="VOID")], bas=ROOTB),
      "sec 8.1/8.7 item 13: the VOID list holds records undiscarded by sec 8.1 alone.",
      "COMPLETE")
    A("unproven_of_counted_hash_not_in_have",
      inp(R, CL, [anchor(H_UNSEEN, R, bn=50, tx=HX32(0x501)),
                  anchor(H_UNSEEN, R, bn=60, tx=HX32(0x777), verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.7 item 15 rule 2, second escape: an UNPROVEN hash counted elsewhere and absent "
      "from the pile moves no label; the counted record's MISSING row gives GAPS.", "GAPS")
    A("unknown_types_same_type_one_seq",
      inp(R, UNK + [mk("root", "dispatch", 1, eid(UNK[0]), {"x": 1})]),
      "sec 8.7 item 9: two unknown entries of one type at one seq sort by entry_id bytewise; "
      "the fork is hard all the same.", "BROKEN_CHAIN")
    A("unknown_type_excluded_entry_not_listed",
      inp(R, UNK + [mk("k9", "dispatch", 4, eid(UNK[3]), {})]),
      "sec 8.7 item 9: the list is over ledger entries; an EXCLUDED entry of an unknown type "
      "is an EXCLUDED row and no UNKNOWN_TYPE row.", "COMPLETE")
    A("adoption_is_a_known_type",
      inp(R, UNK[:3] + [mk("root", "adoption", 3, eid(UNK[2]), ADB(3))]),
      "sec 8.7 item 9: adoption is one of the seven and earns no UNKNOWN_TYPE row, witnessed by hand and "
      "never by the seed.", "COMPLETE")
    A("malformed_duplicate_collapses", inp(R, CL + [b"{}", b"{}"]),
      "sec 8.1/8.7 item 10: byte-identical copies collapse to one entry and to one MALFORMED "
      "row.", "COMPLETE")
    A("basis_registries_unsorted",
      inp(R, CL, bas=basis((chainobj(1, regs=(HX20(5), HX20(4)), raw=True),))),
      "sec 9.4: registries is bytewise ascending.", "NO_LABEL")
    A("basis_baretx_chain_unsorted",
      inp(R, CL, bas=basis((chainobj(1),), ({"chainId": 8453, "tx": HX32(8)},
                                              {"chainId": 1, "tx": HX32(9)}), raw=True)),
      "sec 9.4: bareTx is ordered by (chainId, tx); the tx components ascend, so chainId alone "
      "decides.", "NO_LABEL")
    A("pile_element_uppercase_hex",
      dict(inp(R, CL), pile=[hx(CL[0])[:2] + hx(CL[0])[2:].upper()] + [hx(b) for b in CL[1:]]),
      "HARNESS: a pile element's digits carry no case rule; the report is byte-identical to "
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
    A("unavailable_two_hashes_sorted",
      inp(R, CL, [anchor(H_UNSEEN, R, bn=50, tx=HX32(0x501)), anchor(HX32(0xF00D), R, bn=60, tx=HX32(0x601))],
          unavailable=[H_UNSEEN, HX32(0xF00D)], bas=ROOTB),
      "sec 8.7 item 11: the unavailable set with two elements, sorted bytewise; neither hash "
      "is MISSING.", "UNAVAILABLE")

    # ---------------- round three: the corpus reader's sides, the reviewers' inputs ----
    H_F00D = HX32(0xF00D)
    A("unavailable_of_void_anchor",
      inp(R, CL, [anchor(H_UNSEEN, R, verdict="VOID")], [H_UNSEEN], bas=ROOTB),
      "sec 8.1: the hash leaves the unavailable set because its record is not counted, a VOID "
      "verdict as much as an UNPROVEN one, and no label turns on the VOID list.", "COMPLETE")
    A("unavailable_of_discarded_record",
      inp(R, CL, [anchor(H_UNSEEN, A_("k9"))], [H_UNSEEN], bas=ROOTB),
      "sec 8.1: the sender trim runs first, so the record is discarded and the hash is the hash "
      "of no undiscarded counted record and leaves the unavailable set.", "COMPLETE")
    A("unavailable_of_no_record",
      inp(R, CL, [], [H_UNSEEN], bas=ROOTB),
      "sec 8.1: a hash that is the hash of no record at all meets the first of the two discard "
      "conditions and leaves the unavailable set; the two conditions are a union.", "COMPLETE")
    A("missing_two_hashes_sorted",
      inp(R, CL, [anchor(H_UNSEEN, R, bn=50, tx=HX32(0x501)),
                  anchor(H_F00D, R, bn=60, tx=HX32(0x601))], bas=ROOTB),
      "sec 8.7 item 4: two counted hashes neither in have nor unavailable; the rows sort "
      "bytewise by hash.", "GAPS")
    A("missing_anchors_sorted_by_block",
      inp(R, CL, [anchor(H_UNSEEN, R, bn=60, tx=HX32(0x501)),
                  anchor(H_UNSEEN, R, bn=50, tx=HX32(0x777))], bas=ROOTB),
      "sec 8.7 item 4: the anchors of one MISSING row sort by blockNumber before tx.", "GAPS")
    A("anchored_anchors_sorted_by_tx",
      inp(R, CL, [anchor(CID[0], R, bn=50, tx=HX32(0x777)),
                  anchor(CID[0], R, bn=50, tx=HX32(0x501))], bas=ROOTB),
      "sec 8.7 item 5: two records of one entry_id in one block, so the tx component of the "
      "anchor ordering decides.", "COMPLETE")
    XA = mk("k9", "annotation", 1, HX32(0x11), ANNB("left"))
    XB = mk("k9", "annotation", 1, HX32(0x11), ANNB("right"))
    A("excluded_two_at_one_seq", inp(R, CL + [XA, XB]),
      "sec 8.7 item 7: two excluded entries at one seq sort by entry_id bytewise, and sec 8.4 "
      "records no equivocation, since neither is a ledger entry.", "COMPLETE")
    FIVE = lambda verdict, snd=R: [
        anchor(H_UNSEEN, snd, cid=8453, bn=50, ts=1699000000, tx=HX32(0x501), verdict=verdict),
        anchor(H_UNSEEN, snd, bn=60, tx=HX32(0x501), verdict=verdict),
        anchor(H_UNSEEN, snd, bn=50, tx=HX32(0x777), verdict=verdict),
        anchor(H_UNSEEN, snd, bn=50, tx=HX32(0x501), verdict=verdict),
        anchor(H_F00D, snd, bn=50, tx=HX32(0x501), verdict=verdict)]
    TWOCH = basis((chainobj(1, senders=(R,)), chainobj(8453, senders=(R,))))
    A("unproven_rows_sorted", inp(R, CL, FIVE("UNPROVEN"), bas=TWOCH),
      "sec 8.7 item 12: five UNPROVEN records differing first at each of hash, chainId, "
      "blockNumber and tx; the rows sort by the four in that order.", "UNAVAILABLE")
    A("void_rows_sorted", inp(R, CL, FIVE("VOID"), bas=TWOCH),
      "sec 8.7 item 13: the VOID list sorts as the UNPROVEN list does; no label turns on it.",
      "COMPLETE")
    K4, K9 = A_("k4"), A_("k9")
    A("discarded_rows_sorted",
      inp(R, CL, [anchor(H_UNSEEN, K9, bn=50, tx=HX32(0x502)),
                  anchor(H_UNSEEN, K4, cid=8453, bn=50, ts=1699000000, tx=HX32(0x501)),
                  anchor(H_UNSEEN, K4, bn=50, tx=HX32(0x777)),
                  anchor(H_UNSEEN, K4, bn=50, tx=HX32(0x501)),
                  anchor(H_F00D, K4, bn=50, tx=HX32(0x501))],
          bas=basis((chainobj(1, senders=(K4, K9)), chainobj(8453, senders=(K4,))))),
      "sec 8.7 item 14: five discarded records differing first at each of sender, chainId, tx "
      "and hash; the rows sort by (sender, chainId, blockNumber, tx, hash).", "COMPLETE")
    A("basis_chains_member_absent", inp(R, CL, bas={"bareTx": [], "adoptionChains": []}),
      "sec 9.4: exactly these members; chains absent.", "NO_LABEL")
    A("basis_chains_not_array", inp(R, CL, bas={"chains": {}, "bareTx": [], "adoptionChains": []}),
      "sec 9.4: chains is an array of objects.", "NO_LABEL")
    A("basis_chains_object_member_absent",
      inp(R, CL, bas={"chains": [{"chainId": 1, "fromBlock": 0, "toBlock": 1000,
                                  "registries": []}], "bareTx": [], "adoptionChains": []}),
      "sec 9.4: a chains object carries exactly its five members; senders absent.", "NO_LABEL")
    A("basis_baretx_object_extra_member",
      inp(R, CL, bas={"chains": [], "bareTx": [{"chainId": 1, "tx": HX32(5), "rpc": "x"}], "adoptionChains": []}),
      "sec 9.4: a bareTx object carries exactly chainId and tx.", "NO_LABEL")
    A("basis_baretx_object_member_absent",
      inp(R, CL, bas={"chains": [], "bareTx": [{"chainId": 1}], "adoptionChains": []}),
      "sec 9.4: exactly these members; tx absent.", "NO_LABEL")
    A("basis_baretx_tx_not_hex32",
      inp(R, CL, bas={"chains": [], "bareTx": [{"chainId": 1, "tx": "0x12"}], "adoptionChains": []}),
      "sec 9.4 with sec 1: tx is hex32.", "NO_LABEL")
    A("basis_toblock_string",
      inp(R, CL, bas={"chains": [{"chainId": 1, "fromBlock": 0, "toBlock": "1000",
                                  "registries": [], "senders": []}], "bareTx": [], "adoptionChains": []}),
      "sec 9.4: toBlock is an int.", "NO_LABEL")
    A("basis_sender_not_hex20",
      inp(R, CL, bas={"chains": [{"chainId": 1, "fromBlock": 0, "toBlock": 1000,
                                  "registries": [], "senders": ["0xabc"]}], "bareTx": [], "adoptionChains": []}),
      "sec 9.4 with sec 1: senders is an array of hex20.", "NO_LABEL")
    A("basis_baretx_two_on_one_chain_sorted",
      inp(R, CL, bas=basis((chainobj(1),), ({"chainId": 1, "tx": HX32(8)},
                                            {"chainId": 1, "tx": HX32(9)}))),
      "sec 9.4: two bareTx objects of one chain in ascending tx order are a basis.", "COMPLETE")
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
    A("anchor_baretx_names_other_chain",
      inp(R, CL, [anchor(CID[0], R, cid=1, bn=2000, tx=HX32(0xA07D0))],
          bas=basis((chainobj(1, fb=0, tb=1000, senders=(R,)),),
                    ({"chainId": 8453, "tx": HX32(0xA07D0)},))),
      "sec 9.4 [C]: a bareTx object naming the transaction on another chain does not reach the "
      "record, and no chains object of its chain holds its block; no label.", "NO_LABEL")
    A_raw("input_ignored_member_exponent",
          b'{"root":"' + R.encode() + b'","pile":[],"anchors":[],"unavailable":[],'
          b'"basis":{"adoptionChains":[],"bareTx":[],"chains":[]},"evidence":[],"extra":[{"n":1e3}]}',
          "HARNESS: an exponent is not sec 3.2's spelling of an integer, in an ignored member "
          "as anywhere else; the file is refused.")
    A_raw("input_basis_transport_member_order",
          b'{"root":"' + R.encode() + b'","pile":' + json.dumps([hx(b) for b in CL]).encode() +
          b',"anchors":[],"unavailable":[],"basis":{"chains":[],"bareTx":[],"adoptionChains":[]},"evidence":[]}',
          "HARNESS with sec 8.7 item 1: the basis arrives in any RFC 8259 spelling and the report "
          "carries it re-canonicalized, so a file spelling chains before bareTx yields the same "
          "report as the canonical spelling; label COMPLETE.", expect_label="COMPLETE",
          predict_from=inp(R, CL))
    A_raw("input_byte_order_mark",
          b"\xef\xbb\xbf" + json.dumps(inp(R, CL)).encode("utf-8"),
          "HARNESS: a byte order mark is not whitespace under RFC 8259's JSON-text production, "
          "so the file is unreadable, as it is for an entry.")
    A("input_basis_absent",
      {"root": R, "pile": [hx(b) for b in CL], "anchors": [], "unavailable": [], "evidence": []},
      "HARNESS: basis is a required member of the audit input file.", "NO_LABEL")
    A("input_evidence_absent",
      {"root": R, "pile": [hx(b) for b in CL], "anchors": [], "unavailable": [], "basis": basis()},
      "HARNESS: evidence is a required member of the audit input file.", "NO_LABEL")
    A("input_unavailable_not_array",
      {"root": R, "pile": [hx(b) for b in CL], "anchors": [], "unavailable": {}, "basis": basis(), "evidence": []},
      "HARNESS: unavailable is an array of hex32.", "NO_LABEL")
    A("input_pile_element_not_string",
      {"root": R, "pile": [1], "anchors": [], "unavailable": [], "basis": basis(), "evidence": []},
      "HARNESS: every pile element is a string of the transport encoding.", "NO_LABEL")
    A_raw("input_root_not_object", b"[1,2]",
          "HARNESS: a file whose root value is not an object gives no label, and is no misuse.")
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
      "sec 8.7: the chainId and blockNumber of a MISSING row's anchors compare numerically, so "
      "chain 2 precedes chain 10 and block 9 precedes block 10.", "GAPS")
    A("anchored_anchor_sort_keys_numeric",
      inp(R, CL, [dict(a, hash=CID[0]) for a in THREE], bas=WIDE),
      "sec 8.7 item 5: the same two components inside an ANCHORED row.", "COMPLETE")
    A("unproven_sort_keys_numeric",
      inp(R, CL, [dict(a, verdict="UNPROVEN") for a in THREE], bas=WIDE),
      "sec 8.7 item 12: chainId and blockNumber compare numerically.", "UNAVAILABLE")
    A("void_sort_keys_numeric",
      inp(R, CL, [dict(a, verdict="VOID") for a in THREE], bas=WIDE),
      "sec 8.7 item 13: the same, and no label turns on the list.", "COMPLETE")
    A("discarded_sort_keys_numeric",
      inp(R, CL, [dict(a, sender=A_("k9")) for a in THREE],
          bas=basis((chainobj(2, senders=(A_("k9"),)), chainobj(10, senders=(A_("k9"),))))),
      "sec 8.7 item 14: chainId compares numerically, so chain 2 precedes chain 10.", "COMPLETE")
    LONG, LID = build_chain([("root", "genesis", GENB)] +
                            [("root", "annotation", ANNB("e%d" % i)) for i in range(1, 11)])
    A("findings_position_numeric",
      inp(R, LONG + [mk("root", "annotation", 9, LID[8], ANNB("twin at 9")),
                     mk("root", "annotation", 10, LID[9], ANNB("twin at 10"))]),
      "sec 8.7 item 3: two equivocations at positions 9 and 10; position compares numerically, "
      "so 9 sorts first.", "BROKEN_CHAIN")
    A("excluded_seq_numeric",
      inp(R, LONG + [mk("k9", "annotation", 9, LID[8], ANNB("foreign at 9")),
                     mk("k9", "annotation", 10, LID[9], ANNB("foreign at 10"))]),
      "sec 8.7 item 7: seq compares numerically, so the row at 9 precedes the row at 10.",
      "COMPLETE")
    A("unknown_type_seq_numeric",
      inp(R, LONG + [mk("root", "dispatch", 9, LID[8], {}), mk("root", "dispatch", 10, LID[9], {})]),
      "sec 8.7 item 9: seq compares numerically before entryType.", "BROKEN_CHAIN")
    A("input_unavailable_repeated_element",
      inp(R, CL, [anchor(H_UNSEEN, R)], [H_UNSEEN, H_UNSEEN], bas=ROOTB),
      "HARNESS: unavailable transports a set, so a repeated element collapses and item 11 "
      "carries one row.", "UNAVAILABLE")
    A("input_root_uppercase_prefix", inp("0X" + R[2:], CL),
      "sec 9.4 with sec 1: an uppercase 0x prefix fails hex20 as an uppercase digit does.",
      "NO_LABEL")
    A("pile_element_uppercase_prefix",
      dict(inp(R, CL), pile=["0X" + hx(CL[0])[2:]] + [hx(b) for b in CL[1:]]),
      "HARNESS: the transport prefix is the two bytes 0x, and the case rule reaches the digits "
      "alone.", "NO_LABEL")
    A("unknown_types_sorted_by_case",
      inp(R, CL[:2] + [mk("root", "Zulu", 1, CID[0], {}), mk("root", "aardvark", 1, CID[0], {})]),
      "sec 8.7 item 9: entryType compares bytewise, so Zulu precedes aardvark.", "BROKEN_CHAIN")
    A("basis_chains_element_not_object", inp(R, CL, bas={"chains": [1], "bareTx": [], "adoptionChains": []}),
      "sec 9.4: chains holds objects.", "NO_LABEL")
    A("basis_baretx_element_not_object", inp(R, CL, bas={"chains": [], "bareTx": [1], "adoptionChains": []}),
      "sec 9.4: bareTx holds objects.", "NO_LABEL")
    A("basis_baretx_not_array", inp(R, CL, bas={"chains": [], "bareTx": {}, "adoptionChains": []}),
      "sec 9.4: bareTx is an array.", "NO_LABEL")
    for arr in ("registries", "senders"):
        A("basis_%s_not_array" % arr,
          inp(R, CL, bas={"chains": [dict(chainobj(1), **{arr: "0x00"})], "bareTx": [], "adoptionChains": []}),
          "sec 9.4: %s is an array of hex20." % arr, "NO_LABEL")
    for k in ("chainId", "fromBlock", "toBlock", "registries"):
        A("basis_chains_object_lacks_%s" % k.lower(),
          inp(R, CL, bas={"chains": [{kk: v for kk, v in chainobj(1).items() if kk != k}], "bareTx": [], "adoptionChains": []}),
          "sec 9.4: a chains object carries exactly its five members; %s absent." % k, "NO_LABEL")
    A("basis_chainid_string",
      inp(R, CL, bas={"chains": [dict(chainobj(1), chainId="1")], "bareTx": [], "adoptionChains": []}),
      "sec 9.4: chainId is an int.", "NO_LABEL")
    A("basis_baretx_object_lacks_chainid",
      inp(R, CL, bas={"chains": [], "bareTx": [{"tx": HX32(5)}], "adoptionChains": []}),
      "sec 9.4: exactly these members; chainId absent.", "NO_LABEL")
    A("basis_baretx_chainid_string",
      inp(R, CL, bas={"chains": [], "bareTx": [{"chainId": "1", "tx": HX32(5)}], "adoptionChains": []}),
      "sec 9.4: a bareTx object's chainId is an int.", "NO_LABEL")
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
    for k in ("root", "anchors", "unavailable"):
        A("input_%s_absent" % k, {kk: v for kk, v in inp(R, CL).items() if kk != k},
          "HARNESS: %s is a required member of the audit input file." % k, "NO_LABEL")
    A("input_root_not_string", dict(inp(R, CL), root=1),
      "HARNESS with sec 9.4: root is hex20, and an int is outside its form.", "NO_LABEL")
    A("input_pile_not_array", dict(inp(R, CL), pile="0x00"),
      "HARNESS: pile is an array of strings.", "NO_LABEL")

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
    A("basis_baretx_chainid_numeric",
      inp(R, CL, bas=basis((chainobj(1),), ({"chainId": 2, "tx": HX32(9)},
                                            {"chainId": 10, "tx": HX32(8)}), raw=True)),
      "sec 9.4: bareTx is ordered by chainId numerically, 2 then 10, before tx.", "COMPLETE")
    A("input_basis_not_object", dict(inp(R, CL), basis=[]),
      "sec 9.4: the basis is an object; an array is not a basis.", "NO_LABEL")
    A("input_anchors_element_not_object", dict(inp(R, CL), anchors=[1]),
      "HARNESS with sec 9.2: anchors holds objects.", "NO_LABEL")
    for k in ("blockNumber", "sender", "hash"):
        A("input_anchor_record_lacks_%s_reached" % k.lower(),
          inp(R, CL, [{kk: v for kk, v in anchor(CID[0], R).items() if kk != k}],
              bas=basis((chainobj(1, senders=(R,)),), ({"chainId": 1, "tx": HX32(0xA0064)},))),
          "sec 9.2/9.4: the record lacks %s under a basis whose bareTx names its transaction, "
          "so the absent member alone refuses it." % k, "NO_LABEL")

    A("input_anchor_blocknumber_string_reached",
      inp(R, CL, [dict(anchor(CID[0], R), blockNumber="100")],
          bas=basis((chainobj(1, senders=(R,)),), ({"chainId": 1, "tx": HX32(0xA0064)},))),
      "sec 9.2 with sec 1: blockNumber is an int, under a basis whose bareTx names the "
      "transaction, so the form fault alone refuses it.", "NO_LABEL")

    # ---------------- adoption (sec 9.5) --------------------------------------
    CONTENT = HX32(0xC0FFEE)
    TX1 = HX32(0xAA01)
    ANC = [{"chainId": 1, "tx": TX1, "payloadKind": "calldata", "content": CONTENT}]
    G0 = mk("root", "genesis", 0, None, GENB)
    I0 = eid(G0)
    AD = mk("root", "adoption", 1, I0, {"anchors": ANC})
    ADA = attested_adoption(I0, ANC, "k2", "root", 1)

    A("adoption_proven_lineage_sender",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 9.5: the chain is in adoptionChains, the evidence record carries the content at "
      "offset 4, and the sender is in the prefix lineage at k. Proven.", "COMPLETE")
    A("adoption_attested_valid",
      inp(R, [G0, ADA], evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 0))], bas=ROOTB),
      "sec 6.6/9.5: the sender is outside the prefix lineage, and the body carries an "
      "attestation that passes sec 6.6 whose attestor is byte-equal to that sender.", "COMPLETE")
    A("adoption_attestation_invalid",
      inp(R, [G0, attested_adoption(I0, ANC, "k2", "root", 1, corrupt=True)],
          evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 0))], bas=ROOTB),
      "sec 6.6: an attestation that fails does not void the entry; the element is unproven and "
      "no label turns on it.", "COMPLETE")
    A("adoption_attestor_not_sender",
      inp(R, [G0, ADA], evidence=[ev(1, TX1, A_("k3"), calldata_at(CONTENT, 0))], bas=ROOTB),
      "sec 9.5: the attestation passes but its attestor is not the transaction's sender.",
      "COMPLETE")
    A("adoption_attestation_replayed_prev",
      inp(R, [G0, attested_adoption(I0, ANC, "k2", "root", 1, attest_prev=HX32(0x99))],
          evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 0))], bas=ROOTB),
      "sec 6.6: copied into an entry written on a different head the attestation fails, because "
      "prev differs.", "COMPLETE")
    A("adoption_attestor_wrong_address",
      inp(R, [G0, attested_adoption(I0, ANC, "k2", "root", 1, attestor_addr=A_("k3"))],
          evidence=[ev(1, TX1, A_("k3"), calldata_at(CONTENT, 0))], bas=ROOTB),
      "sec 6.6: the recovered address is not byte-equal to `attestor`.", "COMPLETE")
    A("adoption_evidence_absent",
      inp(R, [G0, AD], bas=ROOTB),
      "sec 9.5 with sec 8.7 item 8: the transaction is in no record of the adoption evidence set; unprovenness has "
      "one report form and one only.", "COMPLETE")
    A("adoption_chain_not_in_basis",
      inp(R, [G0, AD], bas=basis((chainobj(1, senders=(R,)),), (), ())),
      "sec 9.5: the element's chainId is not in the basis's adoptionChains, so the element is "
      "unproven for want of a chain the basis does not reach.", "COMPLETE")
    A("input_evidence_chain_not_in_basis",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, calldata_at(CONTENT, 4))],
          bas=basis((chainobj(1, senders=(R,)),), (), ())),
      "sec 9.4 [C]: an evidence record on a chain no adoptionChains object names is outside the "
      "basis's reach; no label.", "NO_LABEL")
    for off in (0, 4, 32, 36):
        A("adoption_offset_%d" % off,
          inp(R, [G0, AD], evidence=[ev(1, TX1, R, calldata_at(CONTENT, off))], bas=ROOTB),
          "sec 9.5: the content begins at offset %d, %s." %
          (off, "a multiple of 32" if off % 32 == 0 else "4 plus a multiple of 32"), "COMPLETE")
    A("adoption_offset_8_misaligned",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, calldata_at(CONTENT, 8))], bas=ROOTB),
      "sec 9.5: offset 8 is neither a multiple of 32 nor 4 plus one, so the bytes are there and "
      "the element is unproven all the same.", "COMPLETE")
    A("adoption_partially_out_of_range",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, calldata_at(CONTENT, 32, partial=True))],
          bas=ROOTB),
      "sec 9.5: the 32 bytes must lie wholly within the calldata.", "COMPLETE")
    A("adoption_excluded_entry",
      inp(R, [G0, mk("k9", "adoption", 1, I0, {"anchors": ANC})], bas=ROOTB),
      "sec 9.5: an adoption entry this audit excluded names no transaction this report reads, "
      "and no ADOPTION_UNPROVEN row is ever recorded for one.", "COMPLETE")
    SUC2 = mk("root", "succession", 2, eid(AD), SUCB(A_("k2")))
    A("adoption_sender_joins_lineage_later",
      inp(R, [G0, AD, SUC2], evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 4))],
          bas=ROOTB),
      "sec 7.4/9.5: the sender is in the whole-set lineage but not in the prefix lineage at k; "
      "a key that joins later never retroactively authorizes an earlier adoption.", "COMPLETE")
    SUC2A = mk("root", "succession", 2, eid(ADA), SUCB(A_("k2")))
    A("adoption_late_key_but_attested",
      inp(R, [G0, ADA, SUC2A], evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 4))],
          bas=ROOTB),
      "sec 9.5: the same sender, licensed by the attestation route instead of the lineage "
      "route.", "COMPLETE")
    TX2 = HX32(0xAA02)
    ANC2 = ANC + [{"chainId": 1, "tx": TX2, "payloadKind": "log", "content": HX32(0xDEAD)}]
    A("adoption_two_elements_one_unproven",
      inp(R, [G0, mk("root", "adoption", 1, I0, {"anchors": ANC2})],
          evidence=[ev(1, TX1, R, calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 9.5: ADOPTION_UNPROVEN carries the zero-based index of the element in the entry's "
      "anchors array; element 0 is proven and element 1 is not.", "COMPLETE")
    A("adoption_same_tx_twice",
      inp(R, [G0, mk("root", "adoption", 1, I0, {"anchors": ANC + ANC})],
          evidence=[ev(1, TX1, R, calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 9.5: an adoption may name one transaction twice; both are proven or unproven like "
      "any other.", "COMPLETE")


    SUC1 = mk("root", "succession", 1, I0, SUCB(A_("k2")))
    AD2 = mk("k2", "adoption", 2, eid(SUC1), {"anchors": ANC})
    A("adoption_sender_is_earlier_successor",
      inp(R, [G0, SUC1, AD2], evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 4))],
          bas=ROOTB),
      "sec 7.4/9.5: a succession at seq 1 puts k2 in the prefix lineage at 2, so k2's own "
      "transaction proves the element of the adoption k2 wrote at seq 2.", "COMPLETE")
    A("adoption_sender_is_retired_root",
      inp(R, [G0, SUC1, AD2], evidence=[ev(1, TX1, R, calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 7.4/9.5: the lineage only grows; the root that handed the seat over at seq 1 is "
      "still in the prefix lineage at 2, and its transaction proves the element.", "COMPLETE")
    A("adoption_two_successions_both_extend_prefix",
      inp(R, [G0, SUC1, mk("root", "succession", 1, I0, SUCB(A_("k3"))), AD2],
          evidence=[ev(1, TX1, A_("k3"), calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 7.4/9.5: competing successions at one position all extend the prefix lineage, so the "
      "loser's key can still license the adoption; the fork is hard all the same.",
      "BROKEN_CHAIN")
    ANC2B = ANC + [{"chainId": 1, "tx": HX32(0xAA03), "payloadKind": "calldata", "content": CONTENT}]
    A("adoption_attested_two_elements",
      inp(R, [G0, attested_adoption(I0, ANC2B, "k2", "root", 1)],
          evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 0)),
                    ev(1, HX32(0xAA03), A_("k2"), calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 6.6/9.5: two elements both licensed by the attestation route; one attestation covers "
      "the whole anchors array and both are proven.", "COMPLETE")
    A("dup_evidence_records_agree",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, calldata_at(CONTENT, 4)),
                                  ev(1, TX1, R, calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 9.4: the adoption evidence set holds at most one record per (chainId, tx); two that "
      "agree are one record.", "COMPLETE")
    A("dup_evidence_records_disagree",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, calldata_at(CONTENT, 4)),
                                  ev(1, TX1, A_("k2"), calldata_at(CONTENT, 4))], bas=ROOTB),
      "sec 9.4: two evidence records agreeing on (chainId, tx) and disagreeing on sender are "
      "no audit input.", "NO_LABEL")
    A("input_evidence_calldata_odd",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, "0xabc")], bas=ROOTB),
      "sec 9.4: calldata is 0x followed by an even number of hexadecimal digits.", "NO_LABEL")
    A("input_evidence_calldata_uppercase",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, "0xAB")], bas=ROOTB),
      "sec 9.4: calldata's digits are lowercase, as sec 1's hex forms are.", "NO_LABEL")
    A("input_evidence_calldata_empty",
      inp(R, [G0, AD], evidence=[ev(1, TX1, R, "0x")], bas=ROOTB),
      "sec 9.4: the empty calldata is written 0x and is a record; the element is unproven.",
      "COMPLETE")
    A("input_evidence_member_absent",
      inp(R, [G0, AD], evidence=[{"chainId": 1, "tx": TX1, "sender": R}], bas=ROOTB),
      "sec 9.4: an evidence record lacking one of its four members is no audit input.",
      "NO_LABEL")
    A("input_evidence_extra_member_ignored",
      inp(R, [G0, AD], evidence=[dict(ev(1, TX1, R, calldata_at(CONTENT, 4)), logIndex=3)],
          bas=ROOTB),
      "HARNESS with sec 9.4: an evidence record is read by its four members and a member "
      "beyond them is ignored.", "COMPLETE")
    A("input_evidence_not_array", dict(inp(R, [G0, AD]), evidence={}),
      "HARNESS: evidence is an array of objects.", "NO_LABEL")
    A("basis_two_adoptionchains_one_id",
      inp(R, CL, bas=basis((chainobj(1),), (), ({"chainId": 1, "throughBlock": 5},
                                                {"chainId": 1, "throughBlock": 9}), raw=True)),
      "sec 9.4: at most one adoptionChains object per chainId.", "NO_LABEL")
    A("basis_adoptionchains_unsorted",
      inp(R, CL, bas=basis((chainobj(1),), (), ({"chainId": 10, "throughBlock": 5},
                                                {"chainId": 2, "throughBlock": 9}), raw=True)),
      "sec 9.4: adoptionChains is ordered by chainId ascending, numerically.", "NO_LABEL")
    A("basis_adoptionchains_numeric_order",
      inp(R, CL, bas=basis((chainobj(1),), (), ({"chainId": 2, "throughBlock": 5},
                                                {"chainId": 10, "throughBlock": 9}), raw=True)),
      "sec 9.4: adoptionChains ids compare numerically, so 2 before 10 is ascending and the basis stands (bytewise, 10 would precede 2).", "COMPLETE")
    A("basis_adoptionchains_object_extra_member",
      inp(R, CL, bas=basis((chainobj(1),), (), ({"chainId": 1, "throughBlock": 5, "rpc": "x"},))),
      "sec 9.4: an adoptionChains object carries exactly chainId and throughBlock.", "NO_LABEL")
    A("basis_adoptionchains_member_absent",
      inp(R, CL, bas={"chains": [], "bareTx": []}),
      "sec 9.4: exactly these members; adoptionChains absent.", "NO_LABEL")
    A("basis_adoptionchains_throughblock_string",
      inp(R, CL, bas=basis((chainobj(1),), (), ({"chainId": 1, "throughBlock": "5"},))),
      "sec 9.4: throughBlock is an int.", "NO_LABEL")

    # ---------------- round 1, corpus third opinion: isolated witnesses ----------
    STR = mk("k9", "succession", 1, eid(CL[0]), SUCB(A_("k4")))
    A("lineage_stranger_succession_extends_nothing",
      inp(R, CL + [STR], [anchor(HX32(0xBEEF), A_("k4"))],
          bas=basis((chainobj(1, senders=(A_("k4"),)),), (), ({"chainId": 1, "throughBlock": 1000},))),
      "sec 7.4/8.1: the lineage is extended by the `to` of a succession whose author is already "
      "in the set; a stranger's succession extends nothing, so its `to` cannot put a hash into "
      "this ledger's reconciliation and the record is a DISCARDED row.", "COMPLETE")
    UT0 = mk("root", "genesis", 0, None, GENB)
    UT1 = mk("root", "zulu", 1, eid(UT0), {})
    UT2 = mk("root", "aardvark", 2, eid(UT1), {})
    A("unknown_type_seq_before_type", inp(R, [UT0, UT1, UT2]),
      "sec 8.7 item 9: the list sorts by seq before entryType, so zulu at 1 precedes aardvark "
      "at 2 though the types sort the other way.", "COMPLETE")
    A("basis_adoptionchains_not_array",
      inp(R, CL, bas={"chains": [], "bareTx": [], "adoptionChains": {}}),
      "sec 9.4: adoptionChains is an array of objects.", "NO_LABEL")
    A("basis_adoptionchains_element_not_object",
      inp(R, CL, bas={"chains": [], "bareTx": [], "adoptionChains": [1]}),
      "sec 9.4: adoptionChains holds objects.", "NO_LABEL")
    A("basis_adoptionchains_chainid_string",
      inp(R, CL, bas={"chains": [], "bareTx": [], "adoptionChains": [{"chainId": "1", "throughBlock": 1000}]}),
      "sec 9.4: an adoptionChains object's chainId is an int.", "NO_LABEL")
    for k in ("chainId", "throughBlock"):
        A("basis_adoptionchains_object_lacks_%s" % k.lower(),
          inp(R, CL, bas={"chains": [], "bareTx": [],
                          "adoptionChains": [{kk: v for kk, v in {"chainId": 1, "throughBlock": 1000}.items() if kk != k}]}),
          "sec 9.4: an adoptionChains object carries exactly chainId and throughBlock; %s absent." % k,
          "NO_LABEL")
    A("input_anchor_blocktimestamp_string_reached",
      inp(R, CL, [dict(anchor(CID[0], R), blockTimestamp="1700000000")],
          bas=basis((chainobj(1, senders=(R,)),), ({"chainId": 1, "tx": HX32(0xA0064)},))),
      "sec 9.2 with sec 1: blockTimestamp is an int, under a basis whose bareTx names the "
      "transaction, so the form fault alone refuses it.", "NO_LABEL")
    A("input_evidence_element_not_object", dict(inp(R, CL, bas=ROOTB), evidence=[1]),
      "HARNESS with sec 9.4: evidence holds objects.", "NO_LABEL")
    for k in ("chainId", "tx", "sender"):
        A("input_evidence_lacks_%s" % k.lower(),
          inp(R, CL, evidence=[{kk: v for kk, v in ev(1, TX1, R, calldata_at(CONTENT, 0)).items() if kk != k}],
              bas=ROOTB),
          "sec 9.4: an evidence record lacking one of its four members is no audit input; %s absent." % k,
          "NO_LABEL")
    A("input_evidence_tx_not_hex32", inp(R, CL, evidence=[ev(1, "0x12", R, "0x")], bas=ROOTB),
      "sec 9.4 with sec 1: an evidence record's tx is hex32.", "NO_LABEL")
    A("input_evidence_sender_not_hex20", inp(R, CL, evidence=[ev(1, TX1, "0xabc", "0x")], bas=ROOTB),
      "sec 9.4 with sec 1: an evidence record's sender is hex20.", "NO_LABEL")
    A("dup_evidence_records_disagree_calldata",
      inp(R, CL, evidence=[ev(1, TX1, R, calldata_at(CONTENT, 0)), ev(1, TX1, R, calldata_at(CONTENT, 4))],
          bas=ROOTB),
      "sec 9.4: two evidence records agreeing on (chainId, tx) and disagreeing on calldata are "
      "no audit input.", "NO_LABEL")
    A("adoption_offset_flush_right",
      inp(R, [G0, AD],
          evidence=[ev(1, TX1, R, hx(bytes((i * 7 + 3) % 256 for i in range(32)) + bytes.fromhex(CONTENT[2:])))],
          bas=ROOTB),
      "sec 9.5: the 32 bytes begin at offset 32 and end at the calldata's last byte, lying "
      "wholly within it; proven.", "COMPLETE")
    SUK = mk("root", "succession", 1, I0, SUCB(A_("k2")))
    A("adoption_succession_at_the_same_seq",
      inp(R, [G0, AD, SUK], evidence=[ev(1, TX1, A_("k2"), calldata_at(CONTENT, 0))], bas=ROOTB),
      "sec 7.4/9.5: the prefix lineage at k is extended by successions with seq less than k, so "
      "a succession at k itself licenses nothing; the element is unproven and the fork at 1 is "
      "hard.", "BROKEN_CHAIN")
    A("input_ignored_member_max_int", dict(inp(R, CL), extra=[{"n": 2 ** 53 - 1}]),
      "HARNESS: 2^53 - 1 is the largest integer sec 3.1 admits and is read, in an ignored member "
      "as anywhere else; the report is the clean ledger's.", "COMPLETE")

    # ---------------- round 2, corpus third opinion: isolated witnesses ----------
    AD11 = mk("root", "adoption", 1, eid(CL[0]),
              {"anchors": [{"chainId": 1, "tx": HX32(0xA00 + i), "payloadKind": "raw",
                            "content": HX32(0xC00 + i)} for i in range(11)]})
    A("adoption_unproven_index_numeric", inp(R, [CL[0], AD11], bas=basis()),
      "sec 8.7 item 8: eleven unproven elements sort by index numerically, 0 through 10, "
      "where a decimal-string order would put 10 after 1; informational, label COMPLETE.",
      "COMPLETE")
    ch = [CL[0]]
    for s in range(1, 12):
        body = ADB(s) if s in (2, 10) else {"note_md": "n%d" % s}
        ch.append(mk("root", "adoption" if s in (2, 10) else "annotation", s, eid(ch[-1]), body))
    A("adoption_unproven_seq_numeric_walk_order", inp(R, ch, bas=basis()),
      "sec 8.2 and sec 8.7 item 8: a twelve-entry ledger is walked in numeric seq order (0, 1, "
      "2, ..., 11, never 0, 1, 10, 11, 2), so no gap is found, and the two ADOPTION_UNPROVEN "
      "rows sort seq 2 before seq 10; label COMPLETE.", "COMPLETE")
    ADX = attested_adoption(I0, attest_prev=HX32(0x99))
    A("adoption_broken_attestation_lineage_sender",
      inp(R, [G0, ADX], evidence=[ev(1, HX32(0xAA), R, calldata_at(HX32(0xC0), 0))], bas=ROOTB),
      "sec 9.5 with sec 6.6: the two licensing routes are a disjunction; an attestation "
      "signed against another head fails sec 6.6, and the element is proven all the same "
      "because the record's sender is the root, in the prefix lineage at 1.", "COMPLETE")
    A("adoption_broken_attestation_foreign_sender",
      inp(R, [G0, ADX], evidence=[ev(1, HX32(0xAA), A_("k3"), calldata_at(HX32(0xC0), 0))], bas=ROOTB),
      "sec 9.5 with sec 6.6: the same entry with a foreign sender; the broken attestation "
      "licenses nothing and the element is unproven.", "COMPLETE")
    K3 = A_("k3")
    B_RK3 = basis((chainobj(1, senders=(R, K3)),), (), ({"chainId": 1, "throughBlock": 1000},))
    A("anchored_row_excludes_discarded_record",
      inp(R, CL[:1], [anchor(eid(CL[0]), R, tx=HX32(0x701)), anchor(eid(CL[0]), K3, tx=HX32(0x702))],
          bas=B_RK3),
      "sec 8.7 items 5 and 14: two counted records of the genesis, one from the root and one "
      "from a stranger the basis scans; the ANCHORED row carries the root's record alone and "
      "the stranger's is a DISCARDED row.", "COMPLETE")
    A("missing_row_excludes_discarded_record",
      inp(R, CL[:1], [anchor(HX32(0xBEEF), R, tx=HX32(0x701)), anchor(HX32(0xBEEF), K3, tx=HX32(0x702))],
          bas=B_RK3),
      "sec 8.7 items 4 and 14: the same two records over a hash no pile byte string carries; "
      "the MISSING row carries the root's record alone, the stranger's is DISCARDED; GAPS.",
      "GAPS")
    A_raw("input_reader_utf8_inside_ignored_string",
          (json.dumps(inp(R, CL), sort_keys=True)[:-1] + ',"ignored":"').encode("utf-8")
          + b"\xc3\x28" + b'"}',
          "HARNESS: the reader's first restriction, valid UTF-8 over the whole file, is the only "
          "rule the two bytes C3 28 inside an ignored member's string reach; the file is "
          "unreadable and gives no label.")
    TW = [mk("root", "annotation", 1, I0, {"note_md": "twin %d" % i}) for i in range(4)]
    A("four_way_fork_findings_sort", inp(R, [G0] + TW),
      "sec 8.7 item 3: six EQUIVOCATIONs at one position sort by (entry_id, second entry_id), "
      "(A,B),(A,C),(A,D),(B,C),(B,D),(C,D), where sorting by the second id alone would put "
      "(B,C) before (A,D); BROKEN_CHAIN.", "BROKEN_CHAIN")

    A("unproven_hash_is_excluded_entry",
      inp(R, CL + [GX], [anchor(eid(GX), R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.7 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of an excluded "
      "entry in the pile is over bytes in hand (the whole pile, as in sec 8.1), so the label "
      "is not UNAVAILABLE.", "COMPLETE")
    A("unproven_hash_is_malformed_bytes",
      inp(R, CL + [b"{}"], [anchor(eid(b"{}"), R, verdict="UNPROVEN")], bas=ROOTB),
      "sec 8.7 item 15 rule 2: an UNPROVEN record whose hash is the entry_id of a byte string "
      "that fails sec 4.3 is over bytes in hand all the same; MALFORMED lists the bytes and "
      "the label is not UNAVAILABLE.", "COMPLETE")

    # ---------------- randomized ledgers with injected faults -----------------
    for i in range(24):
        L = rng.randrange(1, 8)
        specs = [("root", "genesis", GENB)]
        for j in range(1, L):
            et = rng.choice(["history", "grant", "annotation", "revocation", "adoption",
                             "succession", "windmill"])
            if et == "succession":
                specs.append(("root", et, SUCB(A_(rng.choice(["k2", "k3"])))))
            else:
                specs.append(("root", et, random_body(rng, et)))
        pile, ids = build_chain(specs)
        anchors, unav = [], []
        faults = []
        for _ in range(rng.randrange(1, 4)):
            f = rng.choice(["drop", "twin", "bad_prev", "foreign", "reauthor", "dup",
                            "anchor_missing", "anchor_have", "anchor_unproven", "anchor_void",
                            "anchor_foreign_sender", "unavailable", "none"])
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
            elif f == "unavailable":
                h = HX32(rng.getrandbits(256))
                anchors.append(anchor(h, A_("root"), bn=rng.randrange(1, 999)))
                unav.append(h)
        rng.shuffle(pile)
        A("rand_ledger_%02d" % i, inp(R, pile, anchors, unav, [], ROOTB),
          "randomized ledger of %d written entries with injected faults: %s"
          % (L, ", ".join(faults)))


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

    counts = {}
    for r in MANIFEST:
        counts[r["category"]] = counts.get(r["category"], 0) + 1
    man = {
        "corpus": "zikaron/1 convergence corpus",
        "law": "docs/zikaron-v1.md",
        "harness": "zikaron-conformance/HARNESS.md",
        "seed": seed,
        "generator": "gen.py",
        "keys": {tag: {"privkey": "0x%064x" % KEYS[tag], "address": privkey_to_addr_hex(KEYS[tag])}
                 for tag in sorted(KEYS)},
        "counts": counts,
        "coverage": coverage(),
        "prediction_disclaimer":
            "Every `predicted*` field is the corpus author's own reading of the law, produced "
            "by gen.py and by nothing else. It is a third opinion beside the two "
            "implementations under test and is not authoritative. `hand_expect` records the "
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
    cov = coverage()
    gaps = ([t for t in TOKENS if t not in cov["sec_10_tokens_and_accept"]] +
            [f for f in FINDINGS if f not in cov["sec_8_3_report_rows"]] +
            [l for l in LABELS if l not in cov["sec_8_7_labels"]] +
            [v for v in ("counted", "UNPROVEN", "VOID")
             if v not in cov["sec_9_3_anchor_verdicts"]])
    print("  closed vocabularies unwitnessed: %s" % (", ".join(gaps) if gaps else "none"))
    if WARNINGS:
        print("  hand/code disagreements (%d):" % len(WARNINGS))
        for w in WARNINGS:
            print("      " + w)
    else:
        print("  hand/code disagreements: none")


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
            for k in ("missing", "anchored", "unanchored", "excluded", "unknown_type",
                      "malformed", "unavailable", "unproven", "void", "discarded",
                      "adoption_unproven"):
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
    return {"sec_10_tokens_and_accept": tok, "sec_8_7_labels": lab,
            "sec_8_3_report_rows": fin, "sec_9_3_anchor_verdicts": verd}


TOKENS = ["E_UTF8", "E_JSON", "E_NUMBER", "E_DEPTH", "E_DUP_KEY", "E_KEY_CHARSET",
          "E_VALUE_CHARSET", "E_NOT_CANONICAL", "E_ENVELOPE", "E_ENVELOPE_MISSING",
          "E_ENVELOPE_CLOSED", "E_SPEC", "E_ENTRYTYPE", "E_AUTHOR", "E_SEQ", "E_PREV",
          "E_PREV_SEQ", "E_BODY", "E_SIG_FORM", "E_GENESIS_PLACE", "E_BODY_FIELD", "E_SIG_V",
          "E_SIG_RANGE", "E_SIG_HIGH_S", "E_SIG_RECOVER", "E_SIG_SIGNER"]
FINDINGS = ["SEQ_GAP", "PREV_MISMATCH", "AUTHORITY_MISMATCH", "ROOT_MISMATCH", "EQUIVOCATION",
            "MISSING", "UNANCHORED", "EXCLUDED"]
LABELS = ["BROKEN_CHAIN", "UNAVAILABLE", "GAPS", "COMPLETE"]


README = """# zikaron/1 convergence corpus

Generated by `gen.py` from `docs/zikaron-v1.md` (the law) and
`zikaron-conformance/HARNESS.md` (the command-line contract) alone, carrying
over the twin law's generator where the two texts agree word for word. No
implementation of the law was read while building it.

## Regenerating

    python3 gen.py --seed %d

The run is deterministic: the same seed rebuilds byte-identical directories.
`canon/`, `sign/` and `audit/` are deleted and repopulated on every run, and
`manifest.json` and this file are rewritten. Requires Python 3 and
`pycryptodome` (for `Crypto.Hash.keccak`); secp256k1 with RFC 6979 and low-s,
the sec 3.4 canonicalizer, the sec 3.5 and sec 4.3 decision order and the sec 8
walk are all implemented inside `gen.py`.

## Layout

- `canon/` (%d files, one per case) - byte strings for `zk1 check <path>` and
  `zk1 canon <path>`. Every file is raw bytes; many are not valid UTF-8 and
  most are not valid JSON, which is the point.
- `sign/` (%d cases, two files each) - `sNNN_*.bin` holds the canonical bytes of a six-member
  envelope (or, under a family law's domain, of that document). The sibling
  `sNNN_*.meta.json` names the private key and the domain to pass to
  `zk1 sign <privkey-hex> <path> [domain]`.
- `audit/` (%d cases, two files each) - `aNNN_*.json` is one `zk1 audit <path>` input in the
  HARNESS.md format; the sibling `aNNN_*.note` says what the scenario probes.
- `manifest.json` - every file with its category, its note, and the corpus
  author's own prediction.

## Reading manifest.json

Each case carries the author's prediction as a third opinion:

- canon cases: `predicted` (`"ok"` or the sec 10 token `zk1 check` should
  name), `predicted_entry_id`, `predicted_canon` (what `zk1 canon` should
  answer, since it applies sec 3.5 tests 1 through 5 only) and
  `predicted_canon_sha256` (SHA-256 of the canonical bytes it should print,
  so long outputs stay comparable).
- audit cases: `predicted_label` and `predicted_counts` (ledger entry count,
  finding counts, list sizes).
- sign cases: `predicted` with `presig`, `digest`, `sig` and `signer`.

None of these is authoritative. Where the two implementations agree with each
other and disagree with a prediction, the prediction is wrong. Where they
disagree with each other, the law decides, and the prediction is only a hint
about which reading a third reader reached. `hand_expect` (canon) and
`hand_expect_label` (audit) carry the author's prose reasoning written before
the code ran; a case where the two differ is flagged `disagreement: true` and
is worth reading closely.

## Comparing two implementations

    for f in canon/*.bin;  do zk1 check "$f"; zk1 canon "$f"; done
    for m in sign/*.meta.json; do ... zk1 sign <privkey> <bin> <domain>; done
    for f in audit/*.json; do zk1 audit "$f"; done

Every command writes one canonical JSON value and exits 0 whenever it produced
an answer. Diff the two implementations' streams byte for byte.
"""


if __name__ == "__main__":
    main()
