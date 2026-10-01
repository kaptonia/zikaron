#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
gen.py - corpus generator for the `zikaron.kit/1` convergence run.

Written from docs/zikaron-kit-v1.md (the kit law), docs/zikaron-v1.md (the
frozen parent) and zikaron-conformance/HARNESS-KIT.md (the command-line
contract) alone.  No implementation of the kit law was consulted.  The
`zikaron/1` side of the work (canonicalizer, parser, secp256k1 with RFC 6979
and low-s, EIP-191 digest, envelope and body tests, the audit walk) is reused
from the parent corpus generator `../corpus/gen.py`, which is a tool here and
never evidence for this law; everything about `zikaron.kit/1` below is the
corpus author's own reading of the text and is recorded in manifest.json as a
*prediction*, a third opinion beside the two implementations under test.

Usage:  python3 gen.py --seed 1
Populates ./docs ./pairs ./badges ./kits ./depth ./checks ./chains and writes
manifest.json and README.md.  Deterministic: one seed, one byte-identical tree.
"""

import argparse
import importlib.util
import json
import os
import random
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PARENT_GEN = os.path.normpath(os.path.join(HERE, os.pardir, "corpus", "gen.py"))

_spec = importlib.util.spec_from_file_location("zk1gen", PARENT_GEN)
pg = importlib.util.module_from_spec(_spec)
sys.modules["zk1gen"] = pg
_spec.loader.exec_module(pg)

sha256 = pg.sha256
hx = pg.hx
cbytes = pg.cbytes
JObj = pg.JObj
J = pg.J
N = pg.N
HALF_N = pg.HALF_N
MAXINT = pg.MAXINT
HX32 = pg.HX32
HX20 = pg.HX20
eid = pg.eid

FPM_DOMAIN = "zikaron.fpm/1"
ACK_DOMAIN = "zikaron.ack/1"
ZK1_DOMAIN = "zikaron/1"
BADGE_PREFIX = b"zikaron-grant:"
BADGE_CAP = 2953
KIT_SPEC = "zikaron.kit/1"

FPM_KEYS = ("author", "grant", "note_md", "rows", "sig", "spec", "work")
ACK_KEYS = ("fpm", "note_md", "recipient", "sig", "spec", "variant")
KIT_KEYS = ("contents", "entries", "files", "note_md", "proofs", "root", "spec")

ZK1_TOKENS = ["E_UTF8", "E_JSON", "E_NUMBER", "E_DEPTH", "E_DUP_KEY", "E_KEY_CHARSET",
              "E_VALUE_CHARSET", "E_NOT_CANONICAL", "E_ENVELOPE", "E_ENVELOPE_MISSING",
              "E_ENVELOPE_CLOSED", "E_SPEC", "E_ENTRYTYPE", "E_AUTHOR", "E_SEQ", "E_PREV",
              "E_PREV_SEQ", "E_BODY", "E_SIG_FORM", "E_GENESIS_PLACE", "E_BODY_FIELD",
              "E_SIG_V", "E_SIG_RANGE", "E_SIG_HIGH_S", "E_SIG_RECOVER", "E_SIG_SIGNER"]

DOC_TOKENS = ["E_DOC", "E_DOC_MISSING", "E_DOC_CLOSED", "E_SPEC",
              "E_FPM_AUTHOR", "E_FPM_WORK", "E_FPM_GRANT", "E_FPM_ROWS", "E_FPM_ROW",
              "E_FPM_DUP_RECIPIENT", "E_FPM_DUP_VARIANT", "E_FPM_ROW_ORDER", "E_FPM_NOTE",
              "E_ACK_RECIPIENT", "E_ACK_FPM", "E_ACK_VARIANT", "E_ACK_NOTE",
              "E_SIG_FORM", "E_SIG_V", "E_SIG_RANGE", "E_SIG_HIGH_S", "E_SIG_RECOVER",
              "E_SIG_SIGNER"]

BADGE_TOKENS = ["E_BADGE_PREFIX", "E_BADGE_CAP", "E_BADGE_B64", "E_BADGE_ENTRY",
                "E_BADGE_TYPE", "E_BADGE_INCOMPLETE", "E_BADGE_LINK"]

KIT_VERDICTS = ["KIT_OK", "E_KIT_UNREADABLE", "E_KIT_MANIFEST_ABSENT", "E_KIT_MANIFEST",
                "E_KIT_ENTRY_BYTES", "E_KIT_FILE", "E_KIT_PROOF_BYTES", "E_KIT_EXTRA"]

KIT_RULES = ["canonical", "members", "spec", "root", "entries", "files", "contents",
             "proofs", "note_md"]

PAIR_VERDICTS = ["PAIRED", "FPM_INVALID", "ACK_INVALID", "ACK_FPM_MISMATCH", "ACK_NO_ROW",
                 "ACK_VARIANT_MISMATCH"]

CHECK_TOKENS = ["BAD_SIG", "BROKEN_LEDGER", "NOT_IN_LEDGER", "UNANCHORED", "EXPIRED",
                "REVOKED"]


# ==========================================================================
# small helpers over the parent's value universe
# ==========================================================================


def obj(pairs):
    """A JObj from an ordered (key, value) list; values converted by J()."""
    return JObj([(k, J(v)) for k, v in pairs])


def parse(b):
    """The parsed value of byte string b, or None when b is not canonical."""
    v, tok = pg.accept_canonical(b)
    return v


def bwise(s):
    return s.encode("utf-8") if isinstance(s, str) else s


def is_str(v):
    return isinstance(v, str)


def is_int(v):
    return pg.is_int(v)


is_hex20 = pg.is_hex20
is_hex32 = pg.is_hex32
is_hex65 = pg.is_hex65


# ==========================================================================
# sec 3: signing a document of this law
# ==========================================================================


def presig_bytes(members):
    """sec 3.1: `B` is the canonical bytes of the object with `sig` removed."""
    return cbytes(JObj([(k, v) for k, v in members if k != "sig"]))


def sign_members(priv, members, domain):
    presig = sha256(presig_bytes(members))
    digest = pg.eip191_digest(domain, presig)
    r, s, v = pg.ecdsa_sign(priv, digest)
    return pg.sig_hex(r, s, v), (r, s, v)


def doc_bytes(members, sig, unsorted=False):
    full = JObj([(k, v) for k, v in members if k != "sig"] + [("sig", sig)])
    return cbytes(full, sort=not unsorted)


def sig_fault(v, domain, signer_key):
    """sec 3.1 after `sig` is known to be hex65.  Returns a token or None."""
    raw = bytes.fromhex(v.get("sig")[2:])
    r = int.from_bytes(raw[0:32], "big")
    s = int.from_bytes(raw[32:64], "big")
    vv = raw[64]
    if vv not in (27, 28):
        return "E_SIG_V"
    if not (1 <= r <= N - 1) or not (1 <= s <= N - 1):
        return "E_SIG_RANGE"
    if s > HALF_N:
        return "E_SIG_HIGH_S"
    presig = sha256(presig_bytes(v.members))
    digest = pg.eip191_digest(domain, presig)
    pub = pg._recover_cached(digest, r, s, vv)
    if pub is None:
        return "E_SIG_RECOVER"
    if hx(pg.pub_to_addr(pub)) != v.get(signer_key):
        return "E_SIG_SIGNER"
    return None


# ==========================================================================
# sec 4.2 and sec 5.2: the two document predicates
# ==========================================================================


class Doc(object):
    """The answer of fpm_check / ack_check."""

    __slots__ = ("ok", "token", "index", "doc_id")

    def __init__(self, ok, token=None, index=None, doc_id=None):
        self.ok = ok
        self.token = token
        self.index = index
        self.doc_id = doc_id

    def out(self):
        """The HARNESS-KIT.md output shape."""
        if self.ok:
            return {"doc_id": self.doc_id, "ok": True}
        out = {"ok": False, "token": self.token}
        if self.index is not None:
            out["index"] = self.index  # HARNESS-KIT: E_FPM_ROW carries the index
        return out


def _member_set(v, keys):
    ks = set(v.keys())
    if not set(keys) <= ks:
        return "E_DOC_MISSING"
    if ks - set(keys):
        return "E_DOC_CLOSED"
    return None


def fpm_check(b):
    v, tok = pg.accept_canonical(b)
    if tok:
        return Doc(False, tok)
    if not isinstance(v, JObj):
        return Doc(False, "E_DOC")
    t = _member_set(v, FPM_KEYS)
    if t:
        return Doc(False, t)
    if v.get("spec") != FPM_DOMAIN:
        return Doc(False, "E_SPEC")
    if not is_hex20(v.get("author")):
        return Doc(False, "E_FPM_AUTHOR")
    if not is_hex32(v.get("work")):
        return Doc(False, "E_FPM_WORK")
    g = v.get("grant")
    if not (g is None or is_hex32(g)):
        return Doc(False, "E_FPM_GRANT")
    rows = v.get("rows")
    if not isinstance(rows, list) or len(rows) == 0:
        return Doc(False, "E_FPM_ROWS")
    for i, r in enumerate(rows):
        if not isinstance(r, JObj):
            return Doc(False, "E_FPM_ROW", i)
        if set(r.keys()) != {"recipient", "variant"}:
            return Doc(False, "E_FPM_ROW", i)
        if not is_hex20(r.get("recipient")):
            return Doc(False, "E_FPM_ROW", i)
        if not is_hex32(r.get("variant")):
            return Doc(False, "E_FPM_ROW", i)
    recips = [r.get("recipient") for r in rows]
    if len(set(recips)) != len(recips):
        return Doc(False, "E_FPM_DUP_RECIPIENT")
    variants = [r.get("variant") for r in rows]
    if len(set(variants)) != len(variants):
        return Doc(False, "E_FPM_DUP_VARIANT")
    if [bwise(x) for x in recips] != sorted(bwise(x) for x in recips):
        return Doc(False, "E_FPM_ROW_ORDER")
    if not is_str(v.get("note_md")):
        return Doc(False, "E_FPM_NOTE")
    if not is_hex65(v.get("sig")):
        return Doc(False, "E_SIG_FORM")
    t = sig_fault(v, FPM_DOMAIN, "author")
    if t:
        return Doc(False, t)
    return Doc(True, doc_id=hx(sha256(b)))


def ack_check(b):
    v, tok = pg.accept_canonical(b)
    if tok:
        return Doc(False, tok)
    if not isinstance(v, JObj):
        return Doc(False, "E_DOC")
    t = _member_set(v, ACK_KEYS)
    if t:
        return Doc(False, t)
    if v.get("spec") != ACK_DOMAIN:
        return Doc(False, "E_SPEC")
    if not is_hex20(v.get("recipient")):
        return Doc(False, "E_ACK_RECIPIENT")
    if not is_hex32(v.get("fpm")):
        return Doc(False, "E_ACK_FPM")
    if not is_hex32(v.get("variant")):
        return Doc(False, "E_ACK_VARIANT")
    if not is_str(v.get("note_md")):
        return Doc(False, "E_ACK_NOTE")
    if not is_hex65(v.get("sig")):
        return Doc(False, "E_SIG_FORM")
    t = sig_fault(v, ACK_DOMAIN, "recipient")
    if t:
        return Doc(False, t)
    return Doc(True, doc_id=hx(sha256(b)))


# ==========================================================================
# sec 5.3 pairing and sec 5.4 attribution
# ==========================================================================


def pair(m, a):
    d = fpm_check(m)
    if not d.ok:
        return {"verdict": "FPM_INVALID", "token": d.token}
    d2 = ack_check(a)
    if not d2.ok:
        return {"verdict": "ACK_INVALID", "token": d2.token}
    mv, av = parse(m), parse(a)
    if av.get("fpm") != hx(sha256(m)):
        return {"verdict": "ACK_FPM_MISMATCH"}
    row = None
    for r in mv.get("rows"):
        if r.get("recipient") == av.get("recipient"):
            row = r
            break
    if row is None:
        return {"verdict": "ACK_NO_ROW"}
    if row.get("variant") != av.get("variant"):
        return {"verdict": "ACK_VARIANT_MISMATCH"}
    return {"verdict": "PAIRED", "recipient": row.get("recipient"),
            "variant": row.get("variant")}


def attribute(m, a, x):
    p = pair(m, a)
    if p["verdict"] != "PAIRED":
        # HARNESS-KIT: {"attributed":false,"verdict":"<pairing verdict>"}, no token
        return {"attributed": False, "verdict": p["verdict"]}
    hit = hx(sha256(x)) == p["variant"]
    if hit:
        return {"attributed": True, "recipient": p["recipient"], "verdict": "ATTRIBUTED"}
    return {"attributed": False, "verdict": "NOT_ATTRIBUTED"}


# ==========================================================================
# sec 6: badge payloads
# ==========================================================================

B64AL = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
B64IX = {c: i for i, c in enumerate(B64AL)}


def b64u(b):
    """RFC 4648 sec 5 with no padding, canonical trailing bits."""
    out = []
    for i in range(0, len(b) - len(b) % 3, 3):
        n = (b[i] << 16) | (b[i + 1] << 8) | b[i + 2]
        out += [B64AL[(n >> 18) & 63], B64AL[(n >> 12) & 63], B64AL[(n >> 6) & 63],
                B64AL[n & 63]]
    rem = len(b) % 3
    if rem == 1:
        n = b[-1] << 16
        out += [B64AL[(n >> 18) & 63], B64AL[(n >> 12) & 63]]
    elif rem == 2:
        n = (b[-2] << 16) | (b[-1] << 8)
        out += [B64AL[(n >> 18) & 63], B64AL[(n >> 12) & 63], B64AL[(n >> 6) & 63]]
    return "".join(out)


def b64u_seg(seg):
    """sec 6.2 step 3a over one segment's raw bytes.  Returns (bytes, None)
    or (None, 'E_BADGE_B64')."""
    if len(seg) == 0:
        return None, "E_BADGE_B64"
    try:
        s = seg.decode("ascii")
    except UnicodeDecodeError:
        return None, "E_BADGE_B64"
    for c in s:
        if c not in B64IX:
            return None, "E_BADGE_B64"
    if len(s) % 4 == 1:
        return None, "E_BADGE_B64"
    bits = 0
    acc = 0
    out = bytearray()
    for c in s:
        acc = (acc << 6) | B64IX[c]
        bits += 6
        if bits >= 8:
            bits -= 8
            out.append((acc >> bits) & 0xFF)
    if bits and (acc & ((1 << bits) - 1)) != 0:
        return None, "E_BADGE_B64"
    return bytes(out), None


def body_of(entry_bytes):
    return parse(entry_bytes).get("body")


def byte_link(u_bytes, d_bytes):
    """sec 6.3.  Both arguments are accepted grant entries."""
    ub, db = body_of(u_bytes), body_of(d_bytes)
    if not db.has("upstream"):
        return False
    up = db.get("upstream")
    if not isinstance(up, str):
        return False
    if up != hx(sha256(u_bytes)):
        return False
    return db.get("work") == ub.get("work")


def badge_decode(p):
    """sec 6.2, total."""
    if not p.startswith(BADGE_PREFIX):
        return {"ok": False, "token": "E_BADGE_PREFIX"}
    if len(p) > BADGE_CAP:
        return {"ok": False, "token": "E_BADGE_CAP"}
    segs = p[len(BADGE_PREFIX):].split(b".")
    decoded = []
    for k, seg in enumerate(segs):
        raw, tok = b64u_seg(seg)
        if tok:
            return {"ok": False, "token": tok, "index": k}
        ok, val = pg.zk_check(raw)
        if not ok:
            return {"ok": False, "token": "E_BADGE_ENTRY", "index": k, "inner": val}
        if parse(raw).get("entryType") != "grant":
            return {"ok": False, "token": "E_BADGE_TYPE", "index": k}
        decoded.append(raw)
    if body_of(decoded[0]).has("upstream"):
        return {"ok": False, "token": "E_BADGE_INCOMPLETE"}
    for k in range(1, len(decoded)):
        if not byte_link(decoded[k - 1], decoded[k]):
            return {"ok": False, "token": "E_BADGE_LINK", "index": k}
    return {"ok": True, "grants": [hx(sha256(x)) for x in decoded]}


def badge_encode(entry_byte_strings):
    """sec 6.1 over one or more entries, in order."""
    for k, b in enumerate(entry_byte_strings):
        ok, val = pg.zk_check(b)
        if not ok:
            return {"ok": False, "token": "E_BADGE_ENTRY", "index": k}
        if parse(b).get("entryType") != "grant":
            return {"ok": False, "token": "E_BADGE_TYPE", "index": k}
    payload = BADGE_PREFIX + ".".join(b64u(b) for b in entry_byte_strings).encode("ascii")
    if len(payload) > BADGE_CAP:
        return {"ok": False, "token": "E_BADGE_CAP"}
    return {"payload": payload.decode("ascii")}


# ==========================================================================
# sec 7: disclosure kits
# ==========================================================================

KITSEG_OK = set("abcdefghijklmnopqrstuvwxyz0123456789._-")


def kit_path_ok(p):
    """sec 7.2."""
    if not isinstance(p, str):
        return False
    raw = p.encode("utf-8")
    if len(raw) == 0 or len(raw) > 1024:
        return False
    if p.startswith("/"):
        return False
    segs = p.split("/")
    if len(segs) == 0:
        return False
    for s in segs:
        rb = s.encode("utf-8")
        if len(rb) < 1 or len(rb) > 255:
            return False
        if any(c not in KITSEG_OK for c in s):
            return False
        if s in (".", ".."):
            return False
        if s.startswith("-"):
            return False
    return True


def walk(dirpath):
    """sec 7.1.  Returns (enumeration, None) or (None, failing relative path)."""
    enum = {}

    def rec(rel, absp):
        try:
            names = os.listdir(absp)
        except OSError:
            return rel if rel else "."
        for nm in sorted(names, key=lambda x: x.encode("utf-8", "surrogateescape")):
            if nm in (".", ".."):
                continue
            child_rel = (rel + "/" + nm) if rel else nm
            child_abs = os.path.join(absp, nm)
            if os.path.islink(child_abs):
                return child_rel
            if os.path.isdir(child_abs):
                bad = rec(child_rel, child_abs)
                if bad is not None:
                    return bad
                continue
            if os.path.isfile(child_abs):
                try:
                    with open(child_abs, "rb") as f:
                        enum[child_rel] = f.read()
                except OSError:
                    return child_rel
                continue
            return child_rel
        return None

    bad = rec("", dirpath)
    if bad is not None:
        return None, bad
    return enum, None


def kit_manifest_rule(b):
    """sec 7.3.  Returns (parsed, None, None) or (None, rule, inner token)."""
    v, tok = pg.accept_canonical(b)
    if tok:
        return None, "canonical", tok
    if not isinstance(v, JObj) or set(v.keys()) != set(KIT_KEYS):
        return None, "members", None
    if v.get("spec") != KIT_SPEC:
        return None, "spec", None
    r = v.get("root")
    if not (r is None or is_hex20(r)):
        return None, "root", None
    ents = v.get("entries")
    if not isinstance(ents, list):
        return None, "entries", None
    for e in ents:
        if not is_hex32(e):
            return None, "entries", None
    if len(set(ents)) != len(ents):
        return None, "entries", None
    if [bwise(x) for x in ents] != sorted(bwise(x) for x in ents):
        return None, "entries", None
    files = v.get("files")
    if not isinstance(files, list):
        return None, "files", None
    for row in files:
        if not isinstance(row, JObj) or set(row.keys()) != {"path", "sha256", "size"}:
            return None, "files", None
        if not kit_path_ok(row.get("path")):
            return None, "files", None
        if not is_hex32(row.get("sha256")):
            return None, "files", None
        if not is_int(row.get("size")):
            return None, "files", None
    fpaths = [row.get("path") for row in files]
    if len(set(fpaths)) != len(fpaths):
        return None, "files", None
    if [bwise(x) for x in fpaths] != sorted(bwise(x) for x in fpaths):
        return None, "files", None
    fmap = {row.get("path"): row for row in files}
    contents = v.get("contents")
    if not isinstance(contents, list):
        return None, "contents", None
    for row in contents:
        if not isinstance(row, JObj) or set(row.keys()) != {"content", "path"}:
            return None, "contents", None
        if not is_hex32(row.get("content")):
            return None, "contents", None
        p = row.get("path")
        if not isinstance(p, str) or p not in fmap:
            return None, "contents", None
        if fmap[p].get("sha256") != row.get("content"):
            return None, "contents", None
    crows = [(row.get("content"), row.get("path")) for row in contents]
    if len(set(crows)) != len(crows):
        return None, "contents", None
    if [(bwise(a), bwise(b2)) for a, b2 in crows] != sorted(
            (bwise(a), bwise(b2)) for a, b2 in crows):
        return None, "contents", None
    proofs = v.get("proofs")
    if not isinstance(proofs, list):
        return None, "proofs", None
    for row in proofs:
        if not isinstance(row, JObj) or set(row.keys()) != {"path", "sha256", "tx"}:
            return None, "proofs", None
        if not kit_path_ok(row.get("path")):
            return None, "proofs", None
        if not is_hex32(row.get("sha256")):
            return None, "proofs", None
        if not is_hex32(row.get("tx")):
            return None, "proofs", None
    ppaths = [row.get("path") for row in proofs]
    if len(set(ppaths)) != len(ppaths):
        return None, "proofs", None
    if [bwise(x) for x in ppaths] != sorted(bwise(x) for x in ppaths):
        return None, "proofs", None
    if not is_str(v.get("note_md")):
        return None, "note_md", None
    return v, None, None


def kit_verify(dirpath):
    """sec 7.1 walk then sec 7.4."""
    enum, bad = walk(dirpath)
    if bad is not None:
        return {"verdict": "E_KIT_UNREADABLE", "subject": bad}
    if "manifest.json" not in enum:
        return {"verdict": "E_KIT_MANIFEST_ABSENT"}
    v, rule, inner = kit_manifest_rule(enum["manifest.json"])
    if rule:
        return {"verdict": "E_KIT_MANIFEST", "subject": rule}
    named = {"manifest.json"}
    invalid = []
    for e in v.get("entries"):
        p = "entries/" + e[2:] + ".zk1"
        named.add(p)
        if p not in enum or hx(sha256(enum[p])) != e:
            return {"verdict": "E_KIT_ENTRY_BYTES", "subject": e}
    for row in v.get("files"):
        p = "files/" + row.get("path")
        named.add(p)
        if p not in enum:
            return {"verdict": "E_KIT_FILE", "subject": row.get("path")}
        if hx(sha256(enum[p])) != row.get("sha256") or len(enum[p]) != row.get("size"):
            return {"verdict": "E_KIT_FILE", "subject": row.get("path")}
    for row in v.get("proofs"):
        p = "proofs/" + row.get("path")
        named.add(p)
        if p not in enum or hx(sha256(enum[p])) != row.get("sha256"):
            return {"verdict": "E_KIT_PROOF_BYTES", "subject": row.get("path")}
    extras = sorted((p for p in enum if p not in named), key=bwise)
    if extras:
        return {"verdict": "E_KIT_EXTRA", "subject": extras[0]}
    for e in v.get("entries"):
        p = "entries/" + e[2:] + ".zk1"
        ok, val = pg.zk_check(enum[p])
        if not ok:
            invalid.append({"entry_id": e, "token": val})
    return {"verdict": "KIT_OK", "kit_id": hx(sha256(enum["manifest.json"])),
            "counts": {"entries": len(v.get("entries")),
                       "files": len(v.get("files")),
                       "proofs": len(v.get("proofs"))},
            "invalid_entries": invalid}


# ==========================================================================
# the parent's audit, read structurally (sec 8, sec 9.4 well-formedness)
# ==========================================================================


class Ent(object):
    __slots__ = ("b", "id", "env", "seq", "prev", "author", "etype", "body")

    def __init__(self, b, env):
        self.b = b
        self.id = hx(sha256(b))
        self.env = env
        self.seq = env.get("seq")
        self.prev = env.get("prev")
        self.author = env.get("author")
        self.etype = env.get("entryType")
        self.body = env.get("body")


def _hexstr(v):
    return isinstance(v, str) and v.startswith("0x") and len(v) % 2 == 0 and \
        all(c in "0123456789abcdef" for c in v[2:])


def input_valid(inp):
    """sec 9.4: the well-formedness a verifier decides from the five inputs."""
    if not isinstance(inp, dict):
        return False
    if set(inp.keys()) != {"root", "pile", "anchors", "unavailable", "evidence", "basis"}:
        return False
    if not is_hex20(inp["root"]):
        return False
    if not isinstance(inp["pile"], list) or not all(_hexstr(x) for x in inp["pile"]):
        return False
    if not isinstance(inp["unavailable"], list) or not all(is_hex32(x) for x in inp["unavailable"]):
        return False
    if not pg._basis_ok(inp["basis"]):
        return False
    seen = {}
    for a in inp["anchors"]:
        if not isinstance(a, dict):
            return False
        if set(a.keys()) != {"chainId", "blockNumber", "blockTimestamp", "tx", "sender",
                             "hash", "verdict"}:
            return False
        if not (is_int(a["chainId"]) and is_int(a["blockNumber"]) and is_int(a["blockTimestamp"])):
            return False
        if not (is_hex32(a["tx"]) and is_hex20(a["sender"]) and is_hex32(a["hash"])):
            return False
        if a["verdict"] not in ("counted", "UNPROVEN", "VOID"):
            return False
        k = (a["chainId"], a["tx"], a["hash"])
        if k in seen and seen[k] != a:
            return False
        seen[k] = a
    seenE = {}
    for e in inp["evidence"]:
        if not isinstance(e, dict) or set(e.keys()) != {"chainId", "tx", "sender", "calldata"}:
            return False
        if not (is_int(e["chainId"]) and is_hex32(e["tx"]) and is_hex20(e["sender"])
                and _hexstr(e["calldata"])):
            return False
        k = (e["chainId"], e["tx"])
        if k in seenE and seenE[k] != e:
            return False
        seenE[k] = e
    return True


class Audit(object):
    __slots__ = ("valid", "label", "ledger", "by_id", "findings", "counted", "unproven",
                 "lineage", "basis", "root")


def audit(inp):
    """A structural reading of sec 8 over one audit input.  The corpus author's
    own reading, reusing the parent generator's entry predicate."""
    a = Audit()
    a.valid = input_valid(inp)
    a.label = None
    a.ledger = []
    a.by_id = {}
    a.findings = []
    a.counted = []
    a.unproven = []
    a.lineage = set()
    a.basis = None
    a.root = None
    if not a.valid:
        return a
    a.basis = inp["basis"]
    a.root = inp["root"]
    raw = [bytes.fromhex(x[2:]) for x in inp["pile"]]
    pile_ids = set(hx(sha256(x)) for x in raw)
    ents = {}
    for b in raw:
        ok, val = pg.zk_check(b)
        if ok and b not in ents:
            ents[b] = Ent(b, parse(b))
    entries = list(ents.values())

    lineage = {inp["root"]}
    changed = True
    while changed:
        changed = False
        for e in entries:
            if e.etype == "succession" and e.author in lineage:
                t = e.body.get("to")
                if isinstance(t, str) and t not in lineage:
                    lineage.add(t)
                    changed = True
    a.lineage = lineage
    ledger = [e for e in entries if e.author in lineage]
    a.ledger = ledger
    a.by_id = {e.id: e for e in ledger}

    anchors = []
    seen = set()
    for x in inp["anchors"]:
        k = (x["chainId"], x["tx"], x["hash"])
        if k in seen:
            continue
        seen.add(k)
        if x["sender"] in lineage:
            anchors.append(x)
    counted = [x for x in anchors if x["verdict"] == "counted"]
    unproven = [x for x in anchors if x["verdict"] == "UNPROVEN"]
    a.counted = counted
    a.unproven = unproven
    counted_hashes = set(x["hash"] for x in counted)
    unavail = set(h for h in inp["unavailable"] if h in counted_hashes and h not in pile_ids)

    order = sorted(ledger, key=lambda e: (e.seq, bytes.fromhex(e.id[2:])))
    by_seq = {}
    for e in order:
        by_seq.setdefault(e.seq, []).append(e)

    findings = []
    expected = 0
    auth = inp["root"]
    certain = True
    prev_seq = None
    for s in sorted(by_seq):
        grp = by_seq[s]
        for e in grp:
            if e.seq != expected and e.seq != prev_seq:
                findings.append({"name": "SEQ_GAP", "entry_id": e.id, "hard": False})
                expected = e.seq if e.seq == MAXINT else e.seq + 1
                certain = False
            elif e.seq == expected:
                expected = e.seq if e.seq == MAXINT else e.seq + 1
            if e.seq >= 1:
                pool = by_seq.get(e.seq - 1)
                if pool and not any(x.id == e.prev for x in pool):
                    findings.append({"name": "PREV_MISMATCH", "entry_id": e.id, "hard": True})
            if e.seq == 0:
                if e.author != inp["root"]:
                    findings.append({"name": "ROOT_MISMATCH", "entry_id": e.id, "hard": True})
            else:
                if e.author != auth:
                    findings.append({"name": "AUTHORITY_MISMATCH", "entry_id": e.id,
                                     "hard": certain})
            prev_seq = e.seq
        succ = [e for e in grp if e.etype == "succession"]
        if succ:
            auth = min(succ, key=lambda e: bytes.fromhex(e.id[2:])).body.get("to")

    for i in range(len(order)):
        for j in range(i + 1, len(order)):
            x, y = order[i], order[j]
            if x.id == y.id:
                continue
            if x.seq == y.seq or (x.prev is not None and x.prev == y.prev):
                findings.append({"name": "EQUIVOCATION", "entry_id": x.id, "b": y.id,
                                 "hard": True})
    a.findings = findings

    have = set(e.id for e in ledger)
    missing = counted_hashes - have - unavail
    if any(f["hard"] for f in findings):
        a.label = "BROKEN_CHAIN"
    elif unavail or any(x["hash"] not in pile_ids and x["hash"] not in counted_hashes
                        for x in unproven):
        # the parent's sec 8.7 rule 2: an UNPROVEN record moves the label only
        # when its hash is over bytes nobody holds and no counted record carries
        a.label = "UNAVAILABLE"
    elif any(f["name"] == "SEQ_GAP" for f in findings) or missing:
        a.label = "GAPS"
    else:
        a.label = "COMPLETE"
    return a


def findings_on(a, entry_id, name):
    return [f for f in a.findings if f["name"] == name and (
        f["entry_id"] == entry_id or f.get("b") == entry_id)]


def _reach(a, F, memo):
    """sec 8.1: the ledger entries reachable from F by following `prev`."""
    if F.id in memo:
        return memo[F.id]
    seen = []
    cur = F
    guard = set()
    while cur is not None and cur.id not in guard:
        guard.add(cur.id)
        seen.append(cur.id)
        cur = a.by_id.get(cur.prev) if cur.prev is not None else None
    memo[F.id] = set(seen)
    return memo[F.id]


def unproven_reach(a):
    """sec 10.2 check 4: the ids an undiscarded UNPROVEN record would anchor."""
    memo = {}
    out = set()
    for rec in a.unproven:
        F = a.by_id.get(rec["hash"])
        if F is None:
            continue
        out |= set(_reach(a, F, memo))
    return out


def anchoring(a):
    """Returns (anchored_ids, bound_by_id) per sec 8.2."""
    memo = {}
    bound = {}
    for rec in a.counted:
        F = a.by_id.get(rec["hash"])
        if F is None:
            continue
        for eidv in _reach(a, F, memo):
            ts = rec["blockTimestamp"]
            if eidv not in bound or ts < bound[eidv]:
                bound[eidv] = ts
    return set(bound.keys()), bound


# ==========================================================================
# sec 9: depth reading
# ==========================================================================


def depth(inp, work):
    a = audit(inp)
    if not a.valid:
        return {"valid": False}
    H = [e for e in a.ledger if e.etype == "history" and e.body.get("content") == work]
    out = {"valid": True, "label": a.label, "found": len(H) > 0}
    if not H:
        out["earliest"] = None
        out["deepest"] = 0
        out["continuity"] = {"anchored": 0, "span": 0}
        return out
    anchored, bound = anchoring(a)
    bounds = [bound[e.id] for e in H if e.id in anchored]
    out["earliest"] = min(bounds) if bounds else None
    out["deepest"] = len(bounds)
    key = lambda e: (e.seq, bytes.fromhex(e.id[2:]))
    H0 = min(H, key=key)
    Hmax = max(H, key=key)
    span = Hmax.seq - H0.seq + 1
    counted_hashes = set(x["hash"] for x in a.counted)
    memo = {}
    seqs = set()
    for e in a.ledger:
        if not (H0.seq <= e.seq <= Hmax.seq):
            continue
        if e.id not in counted_hashes:
            continue
        if H0.id in _reach(a, e, memo):
            seqs.add(e.seq)
    out["continuity"] = {"anchored": len(seqs), "span": span}
    return out


# ==========================================================================
# sec 10: grant check, ledger link, chain check
# ==========================================================================


def _with_grant(inp, g):
    j = json.loads(json.dumps(inp))
    if isinstance(j, dict) and isinstance(j.get("pile"), list):
        j["pile"] = list(j["pile"]) + [hx(g)]
    return j


def covering(basis, lineage):
    """sec 10.2 check 4's coverage test."""
    ch = basis["chains"]
    if not ch:
        return False
    for o in ch:
        if not o["registries"]:
            return False
        if not set(lineage) <= set(o["senders"]):
            return False
    return True


def grant_check(g, inp, now):
    """sec 10.2 and sec 10.3."""
    st = ["UNKNOWN"] * 6
    reason = [None] * 6

    ok, val = pg.zk_check(g)
    if not ok:
        st[0], reason[0] = "FAIL", val
    elif parse(g).get("entryType") != "grant":
        st[0], reason[0] = "FAIL", "NOT_A_GRANT"
    else:
        st[0] = "PASS"

    a = None
    if inp is not None:
        a = audit(_with_grant(inp, g))
    have_audit = a is not None and a.valid

    if st[0] == "FAIL":
        return _result(st, reason, a if have_audit else None)

    gid = hx(sha256(g))
    gbody = parse(g).get("body")

    # check 2
    if not have_audit:
        st[1] = "UNKNOWN"
    elif a.label == "BROKEN_CHAIN":
        st[1] = "FAIL"
        reason[1] = None
    else:
        st[1] = "PASS"

    # check 3
    if not have_audit or st[1] == "FAIL":
        st[2] = "UNKNOWN"
    elif gid not in a.by_id:
        st[2] = "FAIL"
    elif findings_on(a, gid, "AUTHORITY_MISMATCH"):
        st[2] = "UNKNOWN"
    else:
        st[2] = "PASS"

    # check 4
    if not have_audit or st[1] == "FAIL" or st[2] == "FAIL":
        st[3] = "UNKNOWN"
    else:
        anchored, _bound = anchoring(a)
        if gid in anchored:
            st[3] = "PASS"
        elif not covering(a.basis, a.lineage):
            st[3] = "UNKNOWN"
        elif gid in unproven_reach(a):
            st[3] = "UNKNOWN"  # sec 10.2 check 4: an undecided anchor of bytes that would anchor g
        elif a.label == "COMPLETE":
            st[3] = "FAIL"
        else:
            st[3] = "UNKNOWN"

    # check 5
    if not gbody.has("window"):
        st[4] = "PASS"
    elif now is None:
        st[4] = "UNKNOWN"
    else:
        w = gbody.get("window")
        if now < w.get("from") or now > w.get("to"):
            st[4] = "FAIL"
        else:
            st[4] = "PASS"

    # check 6
    if not have_audit or st[1] == "FAIL" or st[2] == "FAIL":
        st[5] = "UNKNOWN"
    else:
        hit = False
        for e in a.ledger:
            if e.etype != "revocation":
                continue
            if e.body.get("grant") != gid:
                continue
            if findings_on(a, e.id, "AUTHORITY_MISMATCH"):
                continue
            hit = True
            break
        if hit:
            st[5] = "FAIL"
        elif a.label == "COMPLETE":
            st[5] = "PASS"
        else:
            st[5] = "UNKNOWN"

    return _result(st, reason, a if have_audit else None)


def _result(st, reason, a):
    checks = [{"n": i + 1, "token": CHECK_TOKENS[i], "state": st[i],
               "reason": reason[i] if (i == 0 and st[i] == "FAIL") else None}
              for i in range(6)]
    if any(x == "FAIL" for x in st):
        verdict = "FAIL"
    elif any(x == "UNKNOWN" for x in st):
        verdict = "PARTIAL"
    else:
        verdict = "GREEN"
    return {"verdict": verdict,
            "basis": (a.basis if a is not None else None),
            "checks": checks,
            "failed": [CHECK_TOKENS[i] for i in range(6) if st[i] == "FAIL"]}


def ledger_link(u, d, inp_d):
    """sec 10.4.  u and d are accepted grant byte strings."""
    if not byte_link(u, d):
        return False
    if inp_d is None or not input_valid(inp_d):
        return None
    if inp_d["root"] != body_of(u).get("grantee"):
        return False
    return True


def chain_check(hops, now):
    """sec 10.5.  `hops` is a list of (grant bytes, audit input or None)."""
    if not hops:
        return {"verdict": "FAIL", "hops": [], "links": [], "token": "CHAIN_EMPTY",
                "failing": {"kind": "empty", "index": 0}}
    results = [grant_check(g, i, now) for g, i in hops]
    c1 = [r["checks"][0]["state"] != "FAIL" for r in results]
    links = []
    for k in range(1, len(hops)):
        if not c1[k - 1] or not c1[k]:
            links.append(None)
        else:
            links.append(ledger_link(hops[k - 1][0], hops[k][0], hops[k][1]))
    failing = None
    token = None
    if c1[0] and body_of(hops[0][0]).has("upstream"):
        failing = {"kind": "incomplete", "index": 0}
        token = "CHAIN_INCOMPLETE"
    if failing is None:
        for k, lk in enumerate(links):
            if lk is False:
                failing = {"kind": "link", "index": k + 1}
                token = "CHAIN_LINK"
                break
    if failing is None:
        for k, r in enumerate(results):
            if r["verdict"] == "FAIL":
                failing = {"kind": "hop", "index": k}
                token = None
                break
    if failing is not None:
        verdict = "FAIL"
    elif all(r["verdict"] == "GREEN" for r in results) and all(lk is True for lk in links):
        verdict = "GREEN"
    else:
        verdict = "PARTIAL"
    return {"verdict": verdict, "hops": results, "links": links, "token": token,
            "failing": failing}


# ==========================================================================
# corpus bookkeeping
# ==========================================================================

MANIFEST = []
COUNTS = {}
NOTES = []
_n = {}


def seqno(cat):
    _n[cat] = _n.get(cat, 0) + 1
    return _n[cat]


def W(rel, data):
    p = os.path.join(HERE, rel)
    d = os.path.dirname(p)
    if d and not os.path.isdir(d):
        os.makedirs(d)
    with open(p, "wb") as f:
        f.write(data)
    return rel


def WJ(rel, value):
    return W(rel, (json.dumps(value, indent=1, sort_keys=True) + "\n").encode("utf-8"))


def CASE(category, path, cmd, args, note, predicted, **extra):
    rec = {"path": path, "category": category, "cmd": cmd, "args": args,
           "note": note, "predicted": predicted}
    rec.update(extra)
    MANIFEST.append(rec)
    COUNTS[category] = COUNTS.get(category, 0) + 1
    return rec


def testkey(tag, seed):
    d = int.from_bytes(sha256(("zkk-key/%s/%d" % (tag, seed)).encode()), "big")
    return (d % (N - 2)) + 1


KEYS = {}


def K(t):
    return KEYS[t]


def AD(t):
    return pg.privkey_to_addr_hex(KEYS[t])


# ==========================================================================
# document construction
# ==========================================================================


def rowobj(recipient, variant):
    return {"recipient": recipient, "variant": variant}


def sorted_rows(rows):
    return sorted(rows, key=lambda r: bwise(r["recipient"]))


def fpm_members(author, work, grant, rows, note):
    return [("author", author), ("grant", grant), ("note_md", note),
            ("rows", J(rows)), ("spec", FPM_DOMAIN), ("work", work)]


def mk_fpm(priv, work, rows, grant=None, note="", author=None, spec=None,
           sig=None, domain=FPM_DOMAIN):
    author = AD("author") if author is None else author
    ms = fpm_members(author, work, grant, rows, note)
    if spec is not None:
        ms = [(k, (spec if k == "spec" else v)) for k, v in ms]
    if sig is None:
        sig, _ = sign_members(priv, ms, domain)
    return doc_bytes(ms, sig)


def ack_members(recipient, fpm_id, variant, note):
    return [("fpm", fpm_id), ("note_md", note), ("recipient", recipient),
            ("spec", ACK_DOMAIN), ("variant", variant)]


def mk_ack(priv, fpm_id, variant, note="", recipient=None, spec=None, sig=None,
           domain=ACK_DOMAIN):
    recipient = pg.privkey_to_addr_hex(priv) if recipient is None else recipient
    ms = ack_members(recipient, fpm_id, variant, note)
    if spec is not None:
        ms = [(k, (spec if k == "spec" else v)) for k, v in ms]
    if sig is None:
        sig, _ = sign_members(priv, ms, domain)
    return doc_bytes(ms, sig)


def sig_parts(s):
    raw = bytes.fromhex(s[2:])
    return (int.from_bytes(raw[0:32], "big"), int.from_bytes(raw[32:64], "big"), raw[64])


def sig_of(b):
    return parse(b).get("sig")


def resign_with(b, r, s, v):
    return pg.set_key(b, "sig", pg.sig_hex(r, s, v))


def nonresidue_r():
    """An `r` whose x-coordinate lies on no curve point: recovery must fail."""
    r = 2
    while True:
        alpha = (r * r * r + 7) % pg.P
        beta = pow(alpha, (pg.P + 1) // 4, pg.P)
        if (beta * beta) % pg.P != alpha:
            return r
        r += 1


NONRES_R = None


# ==========================================================================
# docs/ : byte strings for `zkk fpm-check` and `zkk ack-check`
# ==========================================================================


def D(slug, data, cmd, note, hand=None):
    i = seqno("docs")
    path = W("docs/d%04d_%s.bin" % (i, slug), data)
    return D_reg(path, cmd, note, hand)


def D_reg(path, cmd, note, hand=None):
    with open(os.path.join(HERE, path), "rb") as f:
        data = f.read()
    d = (fpm_check if cmd == "fpm-check" else ack_check)(data)
    pred = d.out()
    extra = {}
    if d.index is not None:
        extra["predicted_row_index"] = d.index
    if hand is not None:
        extra["hand_expect"] = hand
        got = "ok" if d.ok else d.token
        if got != hand:
            extra["disagreement"] = True
            NOTES.append("docs %s: hand=%s code=%s (%s)" % (path, hand, got, note))
    CASE("docs", path, cmd, [cmd, path], note, pred, **extra)
    return path


def build_docs(rng):
    global NONRES_R
    NONRES_R = nonresidue_r()
    A = AD("author")
    W1 = hx(sha256(b"the work, first cut"))
    W2 = hx(sha256(b"another work"))
    R = [AD("r1"), AD("r2"), AD("r3"), AD("r4")]
    RS = sorted(R, key=bwise)
    V = [hx(sha256(("variant %d" % i).encode())) for i in range(6)]
    rows1 = [rowobj(R[0], V[0])]
    rows3 = sorted_rows([rowobj(R[0], V[0]), rowobj(R[1], V[1]), rowobj(R[2], V[2])])

    good = mk_fpm(K("author"), W1, rows1)
    good3 = mk_fpm(K("author"), W1, rows3, grant=HX32(0x9EA7), note="three variants")
    fid = hx(sha256(good))
    ackgood = mk_ack(K("r1"), fid, V[0])

    # -- valid manifests, the passing side of every closed boundary ---------
    D("fpm_ok_min", good, "fpm-check",
      "sec 4.1: the smallest lawful manifest: one row, grant null, note_md empty.", "ok")
    D("fpm_ok_three_rows", good3, "fpm-check",
      "sec 4.1: three rows sorted by recipient, distinct recipients and variants, a hex32 "
      "grant.", "ok")
    D("fpm_ok_grant_hex32", mk_fpm(K("author"), W1, rows1, grant=HX32(0)), "fpm-check",
      "sec 6.11 of the parent read into sec 4.1: the all-zero hex32 is a legal `grant`.", "ok")
    D("fpm_ok_work_zero", mk_fpm(K("author"), HX32(0), rows1), "fpm-check",
      "sec 4.1: `work` is a form, not a reference; the all-zero digest passes.", "ok")
    D("fpm_ok_recipient_zero",
      mk_fpm(K("author"), W1, [rowobj(HX20(0), V[1])]), "fpm-check",
      "sec 4.1: the all-zero address is a legal recipient.", "ok")
    D("fpm_ok_note_prose",
      mk_fpm(K("author"), W1, rows1, note="line\nbreak \u0001 caf\u00e9 \U0001f600"),
      "fpm-check",
      "sec 3.3 of the parent: `note_md` opens a prose subtree, so control scalars and astral "
      "scalars are lawful there.", "ok")
    big = sorted_rows([rowobj(HX20(0x1000 + i), HX32(0x2000 + i)) for i in range(20)])
    D("fpm_ok_twenty_rows", mk_fpm(K("author"), W2, big, note="a wide delivery"), "fpm-check",
      "sec 4.1: twenty rows, sorted, all distinct.", "ok")
    D("fpm_ok_author_is_recipient",
      mk_fpm(K("author"), W1, [rowobj(A, V[3])]), "fpm-check",
      "sec 4.1 states no rule against the author holding a row of its own manifest.", "ok")

    # a manifest signed with recovery id 1 (v = 28), so both v values are witnessed
    v28 = None
    for i in range(400):
        cand = mk_fpm(K("author"), W1, rows1, note="v28 probe %d" % i)
        if sig_parts(sig_of(cand))[2] == 28:
            v28 = cand
            break
    D("fpm_ok_v28", v28, "fpm-check",
      "sec 5.4 of the parent read through sec 3.1: a manifest whose recovery id is 1 "
      "(v = 28); `fpm_ok_min` carries v = 27.", "ok")

    # -- sec 3 canonical layer ---------------------------------------------
    D("canon_utf8", good[:12] + b"\xff" + good[13:], "fpm-check",
      "sec 2.1 -> parent sec 3.5 test 1: a byte that is not valid UTF-8.", "E_UTF8")
    D("canon_json_trailing", good + b"}", "fpm-check",
      "parent sec 3.5 test 2: trailing bytes after the JSON text.", "E_JSON")
    D("canon_json_bom", b"\xef\xbb\xbf" + good, "fpm-check",
      "parent sec 3.5: a byte order mark is not whitespace under JSON-text.", "E_JSON")
    D("canon_number", pg.sub(good, b'"work":"' + W1.encode() + b'"', b'"work":01'),
      "fpm-check",
      "parent sec 3.1/3.2: a value position of numeric shape whose bytes are not an integer.",
      "E_NUMBER")
    D("canon_depth", pg.add_key(good, "z", pg.deep_list(128)), "fpm-check",
      "parent sec 3.5: a container opening at nesting depth 129 fails before the closed "
      "member set is examined.", "E_DEPTH")
    D("canon_dup_key", pg.sub(good, b'"note_md":""', b'"note_md":"","note_md":""'),
      "fpm-check", "parent sec 3.5 test 3: a repeated key.", "E_DUP_KEY")
    D("canon_key_charset", pg.sub(good, b'"note_md":', b'"\\u0001note_md":'), "fpm-check",
      "parent sec 3.3/3.5 test 4: every test is applied to decoded strings, so an escape "
      "that decodes to a control scalar fails the key charset and not the JSON grammar.",
      "E_KEY_CHARSET")
    D("canon_value_charset", pg.sub(good, b'"spec":"zikaron.fpm/1"',
                                    b'"spec":"\\u0001zikaron.fpm/1"'), "fpm-check",
      "parent sec 3.5 test 5: `spec` is outside every prose subtree, so a control scalar in "
      "it fails the value charset.", "E_VALUE_CHARSET")
    D("canon_not_canonical", pg.rot_top(good, 3), "fpm-check",
      "parent sec 3.5 test 6: the members are the right members in the wrong order.",
      "E_NOT_CANONICAL")
    D("canon_trailing_newline", good + b"\n", "fpm-check",
      "sec 2.1: a document's published bytes are its canonical bytes and nothing else.",
      "E_NOT_CANONICAL")

    # -- sec 4.2 member set -------------------------------------------------
    D("fpm_root_array", b"[]", "fpm-check", "sec 4.2: the root is not an object.", "E_DOC")
    D("fpm_root_string", b'"zikaron.fpm/1"', "fpm-check",
      "sec 4.2: a canonical scalar root is not an object.", "E_DOC")
    D("fpm_missing_note", pg.drop_key(good, "note_md"), "fpm-check",
      "sec 4.2: an absent key of the seven.", "E_DOC_MISSING")
    D("fpm_missing_grant", pg.drop_key(good, "grant"), "fpm-check",
      "sec 4.1: `grant` is a member whose value may be null, never an absent key.",
      "E_DOC_MISSING")
    D("fpm_closed_extra", pg.add_key(good, "extra", "x"), "fpm-check",
      "sec 4.2: a foreign key.", "E_DOC_CLOSED")
    D("fpm_missing_beats_closed", pg.add_key(pg.drop_key(good, "grant"), "extra", "x"),
      "fpm-check",
      "sec 4.2 order: E_DOC_MISSING for an absent key, *then* E_DOC_CLOSED for a foreign one.",
      "E_DOC_MISSING")

    # -- spec ---------------------------------------------------------------
    D("fpm_spec_ack", mk_fpm(K("author"), W1, rows1, spec=ACK_DOMAIN), "fpm-check",
      "sec 4.2: `spec` must be byte-equal to zikaron.fpm/1; the sibling domain is not it.",
      "E_SPEC")
    D("fpm_spec_v2", mk_fpm(K("author"), W1, rows1, spec="zikaron.fpm/2"), "fpm-check",
      "sec 13.3: a later law is a new spec id, and this one is not accepted here.", "E_SPEC")
    D("fpm_spec_case", mk_fpm(K("author"), W1, rows1, spec="Zikaron.fpm/1"), "fpm-check",
      "sec 4.1: byte-equality, so case matters.", "E_SPEC")

    # -- author / work / grant forms ---------------------------------------
    D("fpm_author_upper", pg.set_key(good, "author", "0x" + "A" * 40), "fpm-check",
      "parent sec 1: hex20 is lowercase.", "E_FPM_AUTHOR")
    D("fpm_author_short", pg.set_key(good, "author", "0x" + "a" * 39), "fpm-check",
      "parent sec 1: hex20 is exactly 42 characters.", "E_FPM_AUTHOR")
    D("fpm_author_null", pg.set_key(good, "author", None), "fpm-check",
      "sec 4.1: `author` is hex20 and null is a value, never an absence.", "E_FPM_AUTHOR")
    D("fpm_work_hex20", pg.set_key(good, "work", HX20(1)), "fpm-check",
      "sec 4.1: `work` is hex32.", "E_FPM_WORK")
    D("fpm_work_noprefix", pg.set_key(good, "work", "a" * 64), "fpm-check",
      "parent sec 1: the 0x prefix is part of the form.", "E_FPM_WORK")
    D("fpm_grant_hex20", pg.set_key(good, "grant", HX20(2)), "fpm-check",
      "sec 4.1: `grant` is null or hex32.", "E_FPM_GRANT")
    D("fpm_grant_empty", pg.set_key(good, "grant", ""), "fpm-check",
      "sec 4.1: the empty string is not hex32 and is not null.", "E_FPM_GRANT")
    D("fpm_grant_false", pg.set_key(good, "grant", False), "fpm-check",
      "sec 4.1: `false` is a value of the universe and not a spelling of absence.",
      "E_FPM_GRANT")

    # -- rows ---------------------------------------------------------------
    D("fpm_rows_empty", mk_fpm(K("author"), W1, []), "fpm-check",
      "sec 4.1: `rows` is non-empty.", "E_FPM_ROWS")
    D("fpm_rows_object", pg.set_key(good, "rows", {"recipient": R[0], "variant": V[0]}),
      "fpm-check", "sec 4.2: `rows` is an array.", "E_FPM_ROWS")
    D("fpm_rows_string", pg.set_key(good, "rows", "rows"), "fpm-check",
      "sec 4.2: `rows` is an array.", "E_FPM_ROWS")
    D("fpm_rows_null", pg.set_key(good, "rows", None), "fpm-check",
      "sec 4.2: `rows` is an array.", "E_FPM_ROWS")
    D("fpm_row0_not_object", mk_fpm(K("author"), W1, ["x"]), "fpm-check",
      "sec 4.2: every element of `rows` is an object; the index travels with the token.",
      "E_FPM_ROW")
    D("fpm_row1_not_object",
      mk_fpm(K("author"), W1, [rowobj(R[0], V[0]), 7]), "fpm-check",
      "sec 4.2: the rows are examined in array order, so this names index 1.", "E_FPM_ROW")
    D("fpm_row2_missing_variant",
      mk_fpm(K("author"), W1, [rowobj(R[0], V[0]), rowobj(R[1], V[1]),
                               {"recipient": R[2]}]), "fpm-check",
      "sec 4.1: a row has exactly the members `recipient` and `variant`; index 2.",
      "E_FPM_ROW")
    D("fpm_row_extra_member",
      mk_fpm(K("author"), W1, [{"recipient": R[0], "variant": V[0], "note": "x"}]),
      "fpm-check", "sec 4.1: a row is arity-closed at two members.", "E_FPM_ROW")
    D("fpm_row_recipient_form",
      mk_fpm(K("author"), W1, [rowobj(HX32(3), V[0])]), "fpm-check",
      "sec 4.1: `recipient` is hex20.", "E_FPM_ROW")
    D("fpm_row_variant_form",
      mk_fpm(K("author"), W1, [rowobj(R[0], HX20(3))]), "fpm-check",
      "sec 4.1: `variant` is hex32.", "E_FPM_ROW")
    D("fpm_row_before_dup",
      mk_fpm(K("author"), W1, [rowobj(R[0], V[0]), rowobj(R[0], V[0]), "x"]), "fpm-check",
      "sec 4.2 order: every row's own form is tested before the distinctness tests, so the "
      "malformed row at index 2 is named and not the duplicate.", "E_FPM_ROW")

    D("fpm_dup_recipient",
      mk_fpm(K("author"), W1, [rowobj(R[0], V[0]), rowobj(R[0], V[1])]), "fpm-check",
      "sec 4.1: recipients are distinct across rows.", "E_FPM_DUP_RECIPIENT")
    D("fpm_dup_recipient_unsorted",
      mk_fpm(K("author"), W1, [rowobj(RS[2], V[0]), rowobj(RS[0], V[1]),
                               rowobj(RS[2], V[2])]),
      "fpm-check",
      "sec 4.2 order: duplicate recipients are named before the sort test.",
      "E_FPM_DUP_RECIPIENT")
    D("fpm_dup_variant",
      mk_fpm(K("author"), W1, sorted_rows([rowobj(R[0], V[0]), rowobj(R[1], V[0])])),
      "fpm-check",
      "sec 4.1: variants are distinct across rows, which is what makes sec 5.4's "
      "attribution name at most one row.", "E_FPM_DUP_VARIANT")
    D("fpm_dup_variant_unsorted",
      mk_fpm(K("author"), W1, [rowobj(RS[1], V[0]), rowobj(RS[0], V[0])]), "fpm-check",
      "sec 4.2 order: duplicate variants are named before the sort test.",
      "E_FPM_DUP_VARIANT")
    D("fpm_row_order",
      mk_fpm(K("author"), W1, [rowobj(RS[2], V[2]), rowobj(RS[0], V[0])]), "fpm-check",
      "sec 4.1: rows are sorted by `recipient` bytewise.", "E_FPM_ROW_ORDER")
    D("fpm_row_order_three",
      mk_fpm(K("author"), W1, [rowobj(RS[0], V[0]), rowobj(RS[2], V[2]),
                               rowobj(RS[1], V[1])]),
      "fpm-check", "sec 4.1: one row out of place is enough.", "E_FPM_ROW_ORDER")

    # -- note_md ------------------------------------------------------------
    D("fpm_note_null", pg.set_key(good, "note_md", None), "fpm-check",
      "sec 4.2: `note_md` is a string; the empty string is the empty note.", "E_FPM_NOTE")
    D("fpm_note_int", pg.set_key(good, "note_md", 0), "fpm-check",
      "sec 4.2: `note_md` is a string.", "E_FPM_NOTE")
    D("fpm_note_array", pg.set_key(good, "note_md", ["a\u0001b"]), "fpm-check",
      "parent sec 3.3: the array is inside a prose subtree so its control scalar is lawful "
      "bytes; sec 4.2 then refuses it for not being a string.", "E_FPM_NOTE")

    # -- sec 3.1 signature boundaries --------------------------------------
    D("fpm_sig_form_short", pg.set_key(good, "sig", "0x" + "a" * 128), "fpm-check",
      "sec 4.2: `sig` is hex65.", "E_SIG_FORM")
    D("fpm_sig_form_null", pg.set_key(good, "sig", None), "fpm-check",
      "sec 4.2: `sig` is hex65.", "E_SIG_FORM")
    D("fpm_sig_form_upper", pg.set_key(good, "sig", "0x" + "A" * 130), "fpm-check",
      "parent sec 1: hex65 is lowercase.", "E_SIG_FORM")
    gr, gs, gv = sig_parts(sig_of(good))
    D("fpm_sig_v_zero", resign_with(good, gr, gs, 0), "fpm-check",
      "parent sec 5.4 through sec 3.1: v is 27 or 28.", "E_SIG_V")
    D("fpm_sig_v_29", resign_with(good, gr, gs, 29), "fpm-check",
      "parent sec 5.4: 29 is outside the closed pair.", "E_SIG_V")
    D("fpm_sig_r_zero", resign_with(good, 0, gs, gv), "fpm-check",
      "parent sec 5.4: 1 <= r <= n - 1.", "E_SIG_RANGE")
    D("fpm_sig_s_zero", resign_with(good, gr, 0, gv), "fpm-check",
      "parent sec 5.4: 1 <= s <= n - 1.", "E_SIG_RANGE")
    D("fpm_sig_r_n", resign_with(good, N, gs, gv), "fpm-check",
      "parent sec 5.4: r = n is out of range.", "E_SIG_RANGE")
    D("fpm_sig_r_n_minus_1", resign_with(good, N - 1, gs, gv), "fpm-check",
      "parent sec 5.4: r = n - 1 is inside the range, so the range test passes and a later "
      "test names the fault.")
    D("fpm_sig_high_s", resign_with(good, gr, N - gs, 55 - gv), "fpm-check",
      "parent sec 5.4: the malleability twin any reader can compute is refused by low-s.",
      "E_SIG_HIGH_S")
    D("fpm_sig_s_half_n", resign_with(good, gr, HALF_N, gv), "fpm-check",
      "parent sec 5.4: s = (n - 1) / 2 exactly is inside low-s, so E_SIG_HIGH_S does not "
      "fire and a later test names the fault.")
    D("fpm_sig_s_half_n_plus", resign_with(good, gr, HALF_N + 1, gv), "fpm-check",
      "parent sec 5.4: one above the low-s bound.", "E_SIG_HIGH_S")
    D("fpm_sig_recover", resign_with(good, NONRES_R, gs, gv), "fpm-check",
      "parent sec 5.4: r is the x-coordinate of no curve point, so recovery fails.",
      "E_SIG_RECOVER")
    D("fpm_sig_signer_other_key", mk_fpm(K("r1"), W1, rows1), "fpm-check",
      "sec 3.1: the recovered address must equal `author`; here another key signed.",
      "E_SIG_SIGNER")
    D("fpm_sig_signer_author_swapped", pg.set_key(good, "author", AD("r2")), "fpm-check",
      "sec 3.1: changing `author` after signing changes both the signer test and the "
      "pre-signature bytes.", "E_SIG_SIGNER")
    D("fpm_sig_other_kit_domain", mk_fpm(K("author"), W1, rows1, domain=ACK_DOMAIN),
      "fpm-check",
      "sec 3.2: a signature made under zikaron.ack/1 over this manifest's presig. The two "
      "literals differ at their ninth byte, so the digests differ and the recovered address "
      "is not the author.", "E_SIG_SIGNER")
    D("fpm_sig_zk1_domain", mk_fpm(K("author"), W1, rows1, domain=ZK1_DOMAIN), "fpm-check",
      "sec 3.2: a signature made under the parent's entry domain. Each kit literal differs "
      "from every zikaron/1 domain at its eighth byte or earlier.", "E_SIG_SIGNER")
    D("fpm_sig_over_other_doc",
      mk_fpm(K("author"), W1, rows1, sig=sig_of(good3)), "fpm-check",
      "sec 3.1: a valid signature of the author's over a different manifest's presig.",
      "E_SIG_SIGNER")

    # -- acknowledgements ---------------------------------------------------
    D("ack_ok_min", ackgood, "ack-check",
      "sec 5.1: the smallest lawful acknowledgement.", "ok")
    D("ack_ok_note", mk_ack(K("r2"), fid, V[1], note="received, thank you"), "ack-check",
      "sec 5.1: a non-empty note; `fpm` and `variant` are forms, not references.", "ok")
    D("ack_ok_zero_fpm", mk_ack(K("r3"), HX32(0), HX32(0)), "ack-check",
      "sec 5.1: all-zero hex32 values are legal spellings.", "ok")
    D("ack_missing_variant", pg.drop_key(ackgood, "variant"), "ack-check",
      "sec 5.2: an absent key of the six.", "E_DOC_MISSING")
    D("ack_closed_extra", pg.add_key(ackgood, "author", AD("author")), "ack-check",
      "sec 5.2: a foreign key, the manifest's own signer member included.", "E_DOC_CLOSED")
    D("ack_root_array", b"[1]", "ack-check", "sec 5.2: the root is not an object.", "E_DOC")
    D("ack_spec_fpm", mk_ack(K("r1"), fid, V[0], spec=FPM_DOMAIN), "ack-check",
      "sec 5.2: `spec` must be byte-equal to zikaron.ack/1.", "E_SPEC")
    D("ack_recipient_form", pg.set_key(ackgood, "recipient", HX32(1)), "ack-check",
      "sec 5.1: `recipient` is hex20.", "E_ACK_RECIPIENT")
    D("ack_recipient_null", pg.set_key(ackgood, "recipient", None), "ack-check",
      "sec 5.1: null is a value, never an absence.", "E_ACK_RECIPIENT")
    D("ack_fpm_form", pg.set_key(ackgood, "fpm", HX20(1)), "ack-check",
      "sec 5.1: `fpm` is hex32.", "E_ACK_FPM")
    D("ack_variant_form", pg.set_key(ackgood, "variant", "0x" + "g" * 64), "ack-check",
      "parent sec 1: `g` is not a hexadecimal digit.", "E_ACK_VARIANT")
    D("ack_note_int", pg.set_key(ackgood, "note_md", 1), "ack-check",
      "sec 5.2: `note_md` is a string.", "E_ACK_NOTE")
    D("ack_sig_form", pg.set_key(ackgood, "sig", "0x"), "ack-check",
      "sec 5.2: `sig` is hex65.", "E_SIG_FORM")
    ar, asx, av = sig_parts(sig_of(ackgood))
    D("ack_sig_v", resign_with(ackgood, ar, asx, 26), "ack-check",
      "parent sec 5.4 through sec 3.1.", "E_SIG_V")
    D("ack_sig_range", resign_with(ackgood, 0, asx, av), "ack-check",
      "parent sec 5.4.", "E_SIG_RANGE")
    D("ack_sig_high_s", resign_with(ackgood, ar, N - asx, 55 - av), "ack-check",
      "parent sec 5.4.", "E_SIG_HIGH_S")
    D("ack_sig_recover", resign_with(ackgood, NONRES_R, asx, av), "ack-check",
      "parent sec 5.4.", "E_SIG_RECOVER")
    D("ack_sig_signer", mk_ack(K("r2"), fid, V[0], recipient=AD("r1")), "ack-check",
      "sec 3.1: the recovered address must equal `recipient`.", "E_SIG_SIGNER")
    D("ack_sig_fpm_domain", mk_ack(K("r1"), fid, V[0], domain=FPM_DOMAIN), "ack-check",
      "sec 3.2: a signature made under the manifest domain over this acknowledgement's "
      "presig.", "E_SIG_SIGNER")
    D("ack_sig_zk1_domain", mk_ack(K("r1"), fid, V[0], domain=ZK1_DOMAIN), "ack-check",
      "sec 3.2: a signature made under zikaron/1.", "E_SIG_SIGNER")
    D("ack_canon_rot", pg.rot_top(ackgood, 2), "ack-check",
      "parent sec 3.5 test 6.", "E_NOT_CANONICAL")

    # -- each document read by the other command ---------------------------
    fpath = [r["path"] for r in MANIFEST if r["path"].endswith("_fpm_ok_min.bin")][0]
    apath = [r["path"] for r in MANIFEST if r["path"].endswith("_ack_ok_min.bin")][0]
    D_reg(fpath, "ack-check",
          "sec 5.2 over a lawful manifest: the member sets differ, so the closed set names "
          "the fault before `spec` is read.", "E_DOC_MISSING")
    D_reg(apath, "fpm-check",
          "sec 4.2 over a lawful acknowledgement: likewise.", "E_DOC_MISSING")

    # -- 150 randomized valid documents ------------------------------------
    valid_pool = []
    for i in range(150):
        if i % 2 == 0:
            n = rng.randrange(1, 7)
            rs = []
            seenr, seenv = set(), set()
            while len(rs) < n:
                rr = HX20(rng.getrandbits(160))
                vv = HX32(rng.getrandbits(256))
                if rr in seenr or vv in seenv:
                    continue
                seenr.add(rr)
                seenv.add(vv)
                rs.append(rowobj(rr, vv))
            rs = sorted_rows(rs)
            gt = None if rng.random() < 0.4 else HX32(rng.getrandbits(256))
            b = mk_fpm(K("author"), HX32(rng.getrandbits(256)), rs, grant=gt,
                       note=pg.rand_prose(rng))
            D("rand_fpm_%03d" % i, b, "fpm-check",
              "randomized lawful manifest (%d rows)." % len(rs), "ok")
            valid_pool.append((b, "fpm-check"))
        else:
            key = rng.choice(["r1", "r2", "r3", "r4"])
            b = mk_ack(K(key), HX32(rng.getrandbits(256)), HX32(rng.getrandbits(256)),
                       note=pg.rand_prose(rng))
            D("rand_ack_%03d" % i, b, "ack-check",
              "randomized lawful acknowledgement.", "ok")
            valid_pool.append((b, "ack-check"))

    # -- 300 mutations ------------------------------------------------------
    kinds = ["flip", "insert", "delete", "swap", "truncate", "dup_slice", "splice"]
    pool = [b for b, _ in valid_pool]
    made = 0
    guard = 0
    while made < 300 and guard < 6000:
        guard += 1
        base, cmd = valid_pool[rng.randrange(len(valid_pool))]
        kind = kinds[rng.randrange(len(kinds))]
        if rng.random() < 0.08:
            try:
                mb = pg.rot_top(base, rng.randrange(1, 6))
            except Exception:
                continue
            kind = "rotate_members"
        else:
            mb = pg.mutate(rng, base, kind, pool)
        if mb is None or len(mb) == 0:
            continue
        made += 1
        D("mut%03d_%s" % (made, kind), mb, cmd,
          "randomized mutation (%s) of a lawful %s document." %
          (kind, "manifest" if cmd == "fpm-check" else "acknowledgement"))
    return {"good": good, "good3": good3, "fid": fid, "ackgood": ackgood,
            "W1": W1, "W2": W2, "R": R, "V": V, "rows1": rows1, "rows3": rows3}


# ==========================================================================
# pairs/ : sec 5.3 pairing and sec 5.4 attribution
# ==========================================================================


def P(slug, m, a, note, x=None, xnote=None):
    i = seqno("pairs")
    base = "pairs/p%03d_%s" % (i, slug)
    mp = W(base + ".fpm.bin", m)
    ap = W(base + ".ack.bin", a)
    CASE("pairs", mp, "pair", ["pair", mp, ap], note, pair(m, a), ack_path=ap)
    if x is not None:
        xp = W(base + ".x.bin", x)
        CASE("pairs", mp, "attribute", ["attribute", mp, ap, xp],
             xnote or note, attribute(m, a, x), ack_path=ap, bytes_path=xp)


def build_pairs(rng, t):
    A = AD("author")
    R = t["R"]
    W1, W2 = t["W1"], t["W2"]
    x1 = b"the bytes handed to the first recipient"
    x2 = b"the bytes handed to the second recipient"
    x3 = b"bytes nobody was handed"
    v1, v2 = hx(sha256(x1)), hx(sha256(x2))
    rows = sorted_rows([rowobj(R[0], v1), rowobj(R[1], v2)])
    m = mk_fpm(K("author"), W1, rows, note="two variants of one delivery")
    mid = hx(sha256(m))
    m2 = mk_fpm(K("author"), W2, rows, note="a second delivery")
    mid2 = hx(sha256(m2))

    m_badrow = pg.set_key(m, "rows", [{"recipient": R[0], "variant": v1, "zz": 1}])
    P("fpm_row_token_no_index", m_badrow, mk_ack(K("r1"), mid, v1),
      note="sec 5.3 step 1: a manifest failing E_FPM_ROW makes the pairing FPM_INVALID carrying "
           "the token and no index, though fpm-check on the same bytes prints the index.")
    P("paired_row0", m, mk_ack(K("r1"), mid, v1), x=x1,
      note="sec 5.3: every test passes and the verdict carries the row's recipient and "
           "variant.",
      xnote="sec 5.4: the bytes hash to the acknowledged variant.")
    P("paired_row1", m, mk_ack(K("r2"), mid, v2), x=x2,
      note="sec 5.3: the row is found by `recipient`, so the second row pairs as readily as "
           "the first.",
      xnote="sec 5.4: ATTRIBUTED against the second row.")
    P("not_attributed", m, mk_ack(K("r1"), mid, v1), x=x3,
      note="sec 5.3: a clean pairing.",
      xnote="sec 5.4: a PAIRED pairing over bytes that hash to something else.")
    P("fpm_invalid", pg.drop_key(m, "note_md"), mk_ack(K("r1"), mid, v1), x=x1,
      note="sec 5.3 step 1: `m` is not a manifest, and the verdict carries the sec 4.2 "
           "token.",
      xnote="sec 5.4: attribution yields the pairing verdict when the pairing is not "
            "PAIRED.")
    P("ack_invalid", m, pg.set_key(mk_ack(K("r1"), mid, v1), "note_md", None), x=x1,
      note="sec 5.3 step 2: `a` is not an acknowledgement.",
      xnote="sec 5.4 over a failed pairing.")
    P("fpm_invalid_beats_ack",
      pg.drop_key(m, "note_md"), pg.set_key(mk_ack(K("r1"), mid, v1), "note_md", None),
      note="sec 5.3 order: both documents fail, and step 1 names the manifest.")
    P("ack_fpm_mismatch", m, mk_ack(K("r1"), mid2, v1), x=x1,
      note="sec 5.3 step 3: the acknowledgement names another manifest's doc_id, so it "
           "cannot be pointed at this one.",
      xnote="sec 5.4 over a failed pairing.")
    P("ack_fpm_zero", m, mk_ack(K("r1"), HX32(0), v1),
      note="sec 5.3 step 3: `fpm` is a well-formed hex32 that is no manifest's doc_id.")
    P("ack_no_row", m, mk_ack(K("r3"), mid, v1), x=x1,
      note="sec 5.3 step 4: the signer holds no row of this manifest.",
      xnote="sec 5.4 over a failed pairing.")
    P("ack_variant_mismatch", m, mk_ack(K("r1"), mid, v2), x=x2,
      note="sec 5.3 step 5: the recipient acknowledges another row's variant. A recipient "
           "can sign only its own row.",
      xnote="sec 5.4 over a failed pairing: the bytes do hash to the acknowledged variant, "
            "and the pairing verdict still governs.")
    P("ack_signer_not_recipient", m,
      mk_ack(K("r2"), mid, v1, recipient=R[0]),
      note="sec 5.2 through sec 3.1: another key signed for this recipient, so the "
           "acknowledgement is not one and step 2 names it.")
    single = mk_fpm(K("author"), W1, [rowobj(R[0], hx(sha256(b"")))])
    P("attributed_empty_file", single, mk_ack(K("r1"), hx(sha256(single)), hx(sha256(b""))),
      x=b"",
      note="sec 5.3: a one-row manifest whose variant is the digest of the empty byte "
           "string.",
      xnote="sec 5.4: the empty file attributes, identity being total.")
    P("attributed_binary", m, mk_ack(K("r2"), mid, v2), x=x2,
      note="sec 5.3: repeated pairing of the second row.",
      xnote="sec 5.4 over the exact delivered bytes.")


# ==========================================================================
# badges/ : sec 6
# ==========================================================================

UNSET = object()


def grant_entry(priv, seq=1, prev=None, grantee=None, work=None, terms=None,
                upstream=UNSET, window=None, scope=None, extra_body=None,
                etype="grant"):
    prev = HX32(0x11) if prev is None else prev
    body = {}
    if etype == "grant":
        body["grantee"] = grantee if grantee is not None else HX20(0xBEEF)
        body["work"] = work if work is not None else HX32(0xC0)
        body["terms"] = terms if terms is not None else HX32(0x7E)
        if window is not None:
            body["window"] = window
        if scope is not None:
            body["scope_md"] = scope
    elif etype == "history":
        body = {"content": work if work is not None else HX32(0xC0),
                "mode": {"mark": "hand", "toolchain": HX32(0x7001)}}
    elif etype == "annotation":
        body = {"note_md": scope or "a note"}
    if upstream is not UNSET:
        body["upstream"] = upstream
    if extra_body:
        body.update(extra_body)
    return pg.make_entry(priv, etype, seq, prev, body)


def padded_grant(priv, target, **kw):
    kw = dict(kw)
    kw["scope"] = ""
    base = grant_entry(priv, **kw)
    pad = target - len(base)
    if pad < 0:
        raise AssertionError("target %d below floor %d" % (target, len(base)))
    kw["scope"] = "x" * pad
    out = grant_entry(priv, **kw)
    assert len(out) == target, (len(out), target)
    return out


def payload(entries):
    return BADGE_PREFIX + ".".join(b64u(b) for b in entries).encode("ascii")


def B(slug, p, note):
    i = seqno("badges")
    path = W("badges/b%03d_%s.payload" % (i, slug), p)
    CASE("badges", path, "badge-decode", ["badge-decode", path], note, badge_decode(p))
    return path


def BE(slug, entries, note):
    i = seqno("badges")
    d = "badges/e%03d_%s" % (i, slug)
    paths = [W("%s/%02d.zk1" % (d, k), b) for k, b in enumerate(entries)]
    CASE("badges", d, "badge-encode", ["badge-encode"] + paths, note,
         badge_encode(entries), entry_paths=paths)


def build_badges(rng, t):
    WK = HX32(0xC0FFEE)
    g0 = grant_entry(K("author"), grantee=AD("r1"), work=WK, terms=HX32(1))
    g0id = eid(g0)
    g1 = grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(2), upstream=g0id)
    g1id = eid(g1)
    g2 = grant_entry(K("r2"), grantee=AD("r3"), work=WK, terms=HX32(3), upstream=g1id)
    hist = grant_entry(K("author"), etype="history", work=WK)
    ann = grant_entry(K("author"), etype="annotation", scope="not a grant")

    B("ok_one", payload([g0]), "sec 6.2 step 6: one segment, a grant that states no "
                               "upstream.")
    B("ok_two", payload([g0, g1]), "sec 6.2: two segments whose byte link holds.")
    B("ok_three", payload([g0, g1, g2]), "sec 6.2: three segments, links at k = 1 and 2.")

    # step 1: prefix
    B("prefix_wrong", b"zikaron-badge:" + b64u(g0).encode(),
      "sec 6.2 step 1: the 14 bytes must be exactly `zikaron-grant:`.")
    B("prefix_short", b"zikaron-grant", "sec 6.2 step 1: thirteen bytes are not fourteen.")
    B("prefix_empty", b"", "sec 6.2 step 1 over the empty byte string.")
    B("prefix_case", b"ZIKARON-GRANT:" + b64u(g0).encode(),
      "sec 6.2 step 1: byte-equality, so case matters.")

    # step 2: the cap, on both sides
    cap_ok = padded_grant(K("author"), 2204, grantee=AD("r1"), work=WK, terms=HX32(4))
    p_ok = payload([cap_ok])
    assert len(p_ok) == 2953, len(p_ok)
    B("cap_2953", p_ok, "sec 6.1/6.2 step 2: a payload of exactly BADGE_CAP bytes, the "
                        "largest lawful one.")
    cap_over = padded_grant(K("author"), 2205, grantee=AD("r1"), work=WK, terms=HX32(4))
    p_over = payload([cap_over])
    assert len(p_over) == 2954, len(p_over)
    B("cap_2954", p_over, "sec 6.2 step 2: one byte over BADGE_CAP.")
    B("cap_beats_segments", p_over[:-1] + b"*",
      "sec 6.2 order: the cap is decided before any segment is examined, so a payload over "
      "the cap carrying a foreign byte is E_BADGE_CAP and not E_BADGE_B64.")
    B("prefix_beats_cap", b"x" + p_over,
      "sec 6.2 order: the prefix is decided before the cap.")
    gen_b = pg.make_entry(K("author"), "genesis", 0, None, {"statement_md": "for the encoder"})
    BE("encode_type_before_cap", [gen_b, cap_over],
       "sec 6.1 order: each segment's entry and type are tested in order before the cap, so "
       "a genesis at index 0 beside an oversized grant is E_BADGE_TYPE at 0 and not E_BADGE_CAP.")
    BE("encode_cap_after_segments", [cap_over],
       "sec 6.1 order: one accepted grant whose payload is one byte over BADGE_CAP fails the "
       "cap after every segment passed.")

    # step 3a: base64url
    B("b64_empty_after_prefix", BADGE_PREFIX,
      "sec 6.2 step 3: no dots yield one segment, and it is empty.")
    B("b64_leading_dot", BADGE_PREFIX + b"." + b64u(g0).encode(),
      "sec 6.2 step 3: a leading dot yields an empty segment at index 0.")
    B("b64_trailing_dot", payload([g0]) + b".",
      "sec 6.2 step 3: a trailing dot yields an empty segment at the end.")
    B("b64_doubled_dot",
      BADGE_PREFIX + (b64u(g0) + ".." + b64u(g1)).encode(),
      "sec 6.2 step 3: a doubled dot yields an empty segment at index 1.")
    B("b64_only_dot", BADGE_PREFIX + b".",
      "sec 6.2 step 3: one dot, two empty segments; index 0 is named.")
    B("b64_foreign_plus", BADGE_PREFIX + (b64u(g0)[:-2] + "+A").encode(),
      "sec 6.2 step 3a: `+` is outside A-Za-z0-9-_ (this is base64url, not base64).")
    B("b64_foreign_slash", BADGE_PREFIX + (b64u(g0)[:-2] + "/A").encode(),
      "sec 6.2 step 3a: `/` is outside the alphabet.")
    B("b64_foreign_pad", BADGE_PREFIX + (b64u(g0) + "==").encode(),
      "sec 6.1: base64url with no padding characters, so `=` is a foreign byte.")
    B("b64_foreign_high_byte", BADGE_PREFIX + b64u(g0).encode() + b"\xff",
      "sec 6.2 step 3a: a byte outside ASCII is outside the alphabet.")
    B("b64_len_1_mod_4", BADGE_PREFIX + b"A",
      "sec 6.2 step 3a: a length of 1 mod 4 encodes no whole byte count.")
    B("b64_len_5", BADGE_PREFIX + b"AAAAA",
      "sec 6.2 step 3a: 5 is 1 mod 4.")
    B("b64_len_2_ok", BADGE_PREFIX + b"AA",
      "sec 6.2 step 3a passes (2 mod 4, trailing bits zero) and step 3b then names the "
      "decoded byte string, which is one 0x00 byte.")
    B("b64_trailing_bits_len2", BADGE_PREFIX + b"AB",
      "sec 6.2 step 3a: at 2 mod 4 the last character's low four bits are unused and must "
      "be zero; `B` carries 1.")
    B("b64_trailing_bits_len3", BADGE_PREFIX + b"AAB",
      "sec 6.2 step 3a: at 3 mod 4 the last character's low two bits are unused and must be "
      "zero.")
    B("b64_trailing_bits_len3_ok", BADGE_PREFIX + b"AAE",
      "sec 6.2 step 3a passes: `E` is index 4, whose low two bits are zero; step 3b then "
      "names the decoded two bytes.")

    # step 3b and 3c
    B("entry_not_json", BADGE_PREFIX + b64u(b"not json at all").encode(),
      "sec 6.2 step 3b: the decoded bytes fail `accept`, and the parent's token travels "
      "with the index.")
    B("entry_not_canonical", BADGE_PREFIX + b64u(pg.rot_top(g0, 2)).encode(),
      "sec 6.2 step 3b: a re-encoding of a valid entry is a different byte string that is "
      "not an entry.")
    B("entry_bad_sig",
      BADGE_PREFIX + b64u(pg.set_key(g0, "author", AD("r4"))).encode(),
      "sec 6.2 step 3b: the parent's signature test names the fault.")
    B("entry_at_index_1",
      BADGE_PREFIX + (b64u(g0) + "." + b64u(b"{}")).encode(),
      "sec 6.2 step 3: segments are examined in order, so the index travels with the "
      "token.")
    B("type_history", BADGE_PREFIX + b64u(hist).encode(),
      "sec 6.2 step 3c: an accepted entry that is not a grant.")
    B("type_annotation_at_1",
      BADGE_PREFIX + (b64u(g0) + "." + b64u(ann)).encode(),
      "sec 6.2 step 3c at index 1.")
    B("b64_beats_entry",
      BADGE_PREFIX + (b64u(g0)[:-1] + "*" + "." + b64u(b"{}")).encode(),
      "sec 6.2 step 3: for each segment, 3a is decided before 3b, and segment 0 is decided "
      "before segment 1 is examined.")
    B("type_beats_next_segment",
      BADGE_PREFIX + (b64u(hist) + "." + "A").encode(),
      "sec 6.2 step 3: segment 0's type fault is named before segment 1's base64 fault, "
      "because each segment is decided before the next is examined.")

    # step 4: incomplete
    B("incomplete_upstream_value",
      payload([grant_entry(K("author"), grantee=AD("r1"), work=WK, terms=HX32(5),
                           upstream=HX32(0xABC))]),
      "sec 6.2 step 4: segment 0's grant states an upstream, so the payload does not start "
      "at the original author's grant.")
    B("incomplete_upstream_null",
      payload([grant_entry(K("author"), grantee=AD("r1"), work=WK, terms=HX32(6),
                           upstream=None)]),
      "sec 6.2 step 4 with the parent sec 6's presence rule: a key present is a member "
      "present, null included.")
    B("incomplete_beats_link",
      payload([grant_entry(K("author"), grantee=AD("r1"), work=WK, terms=HX32(7),
                           upstream=None), g1]),
      "sec 6.2 order: step 4 is decided before any link of step 5, and this payload's link "
      "is broken too.")

    # step 5: links
    bad_up = grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(8),
                         upstream=HX32(0xDEAD))
    B("link_upstream_value", payload([g0, bad_up]),
      "sec 6.3: `upstream` is a string that is not hex32(entry_id(u)).")
    bad_work = grant_entry(K("r1"), grantee=AD("r2"), work=HX32(0xFEED), terms=HX32(9),
                           upstream=g0id)
    B("link_work", payload([g0, bad_work]),
      "sec 6.3: the upstream names the right entry and the works differ.")
    nonstr = grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(10), upstream=7)
    B("link_nonstring_upstream", payload([g0, nonstr]),
      "sec 6.3: `upstream` must be a string; an int is a member present whose value is no "
      "hex32 spelling.")
    B("link_null_upstream",
      payload([g0, grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(11),
                               upstream=None)]),
      "sec 6.3: `upstream` present with value null is a member present and no string.")
    B("link_absent_upstream",
      payload([g0, grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(12))]),
      "sec 6.3: a downstream grant carrying no `upstream` at all.")
    g2bad = grant_entry(K("r2"), grantee=AD("r3"), work=WK, terms=HX32(13),
                        upstream=HX32(0xBAD))
    B("link_broken_at_2", payload([g0, g1, g2bad]),
      "sec 6.2 step 5: the links are decided in order and the smallest failing k is named.")
    B("link_first_of_two_broken", payload([g0, bad_up, g2]),
      "sec 6.2 step 5: link 1 is false and link 2 is false as well; index 1 is named.")

    # -- 100 randomized mutations of lawful payloads ------------------------
    bases = [payload([g0]), payload([g0, g1]), payload([g0, g1, g2]), p_ok]
    kinds = ["flip", "insert", "delete", "swap", "truncate", "dup_slice", "splice"]
    made = 0
    guard = 0
    while made < 100 and guard < 3000:
        guard += 1
        base = bases[rng.randrange(len(bases))]
        kind = kinds[rng.randrange(len(kinds))]
        mb = pg.mutate(rng, base, kind, bases)
        if mb is None or len(mb) == 0:
            continue
        made += 1
        B("mut%03d_%s" % (made, kind), mb,
          "randomized mutation (%s) of a lawful payload." % kind)

    # -- badge-encode -------------------------------------------------------
    BE("one_grant", [g0], "sec 6.1: one accepted grant.")
    BE("two_grants", [g0, g1], "sec 6.1: two grants in order from the original author's.")
    BE("three_grants", [g0, g1, g2], "sec 6.1: three grants.")
    BE("cap_exact", [cap_ok], "sec 6.1: the encoding is exactly BADGE_CAP bytes.")
    BE("cap_over", [cap_over], "sec 6.1: one byte over the cap.")
    BE("cap_over_two", [g0, cap_ok], "sec 6.1: two grants whose joined encoding exceeds "
                                     "the cap.")
    BE("not_an_entry", [b"{}"], "sec 6.1: an input that fails `accept`.")
    BE("not_a_grant", [hist], "sec 6.1: an accepted entry that is not of type grant.")
    BE("not_a_grant_at_1", [g0, ann], "sec 6.1: the second input is not a grant.")
    BE("seg0_upstream", [grant_entry(K("author"), grantee=AD("r1"), work=WK,
                                     terms=HX32(14), upstream=HX32(0xABC))],
       "sec 6.1 states the encoding and sec 6.2 step 4 refuses this payload on decoding: "
       "whether the encoder is required to refuse it too is a place where the text and "
       "HARNESS-KIT.md leave two readings.")
    BE("unlinked_pair", [g0, bad_up],
       "sec 6.1 over two grants whose byte link does not hold: the same question as "
       "`seg0_upstream` for step 5.")
    return {"g0": g0, "g1": g1, "g2": g2, "WK": WK, "hist": hist, "ann": ann}


# ==========================================================================
# kits/ : sec 7
# ==========================================================================


def frow(p, s, n):
    return {"path": p, "sha256": s, "size": n}


def crow(c, p):
    return {"content": c, "path": p}


def prow(p, s, tx):
    return {"path": p, "sha256": s, "tx": tx}


def mfst(root=None, entries=None, files=None, contents=None, proofs=None, note_md=""):
    return {"spec": KIT_SPEC, "root": root,
            "entries": [] if entries is None else entries,
            "files": [] if files is None else files,
            "contents": [] if contents is None else contents,
            "proofs": [] if proofs is None else proofs,
            "note_md": note_md}


def assemble(entries=(), files=(), proofs=(), root=None, note_md="", contents_for=None):
    """A kit whose manifest and disk agree, as an author's export would."""
    ids = sorted((eid(b) for b in entries), key=bwise)
    disk = {}
    for b in entries:
        disk["entries/" + eid(b)[2:] + ".zk1"] = b
    frows = []
    for p, b in sorted(files, key=lambda x: bwise(x[0])):
        disk["files/" + p] = b
        frows.append(frow(p, hx(sha256(b)), len(b)))
    crows = []
    for p, b in files:
        if contents_for is None or p in contents_for:
            crows.append((hx(sha256(b)), p))
    crows.sort(key=lambda x: (bwise(x[0]), bwise(x[1])))
    prows = []
    for p, b, tx in sorted(proofs, key=lambda x: bwise(x[0])):
        disk["proofs/" + p] = b
        prows.append(prow(p, hx(sha256(b)), tx))
    m = mfst(root, ids, frows, [crow(c, p) for c, p in crows], prows, note_md)
    return m, disk


def make_kit(slug, note, manifest=None, disk=None, extras=(), symlinks=(), empty_dirs=(),
             raw_manifest=None, no_manifest=False, fifos=(), chmods=(), chmod_root=None):
    i = seqno("kits")
    d = "kits/k%03d_%s" % (i, slug)
    absd = os.path.join(HERE, d)
    os.makedirs(absd)
    if raw_manifest is not None:
        W(d + "/manifest.json", raw_manifest)
    elif not no_manifest:
        W(d + "/manifest.json", cbytes(J(manifest)))
    for p, b in sorted((disk or {}).items()):
        W(d + "/" + p, b)
    for p, b in extras:
        W(d + "/" + p, b)
    for p in empty_dirs:
        os.makedirs(os.path.join(absd, p))
    for p, target in symlinks:
        pp = os.path.join(absd, p)
        pd = os.path.dirname(pp)
        if pd and not os.path.isdir(pd):
            os.makedirs(pd)
        os.symlink(target, pp)
    for p in fifos:
        os.mkfifo(os.path.join(absd, p))
    for p, mode in chmods:
        os.chmod(os.path.join(absd, p), mode)
    if chmod_root is not None:
        os.chmod(absd, chmod_root)
    pred = kit_verify(absd)
    CASE("kits", d, "kit-verify", ["kit-verify", d], note, pred)
    return d


def build_kits(rng, t):
    ROOT = AD("author")
    fbytes = {"note.txt": b"the delivery note, in plain text\n",
              "art/final.bin": bytes(range(256)) * 3,
              "art/draft.bin": b"an earlier cut\n"}
    CH = hx(sha256(fbytes["art/final.bin"]))
    e_gen = pg.make_entry(K("author"), "genesis", 0, None,
                          {"statement_md": "the works of one hand"})
    e_his = pg.make_entry(K("author"), "history", 1, eid(e_gen),
                          {"content": CH, "mode": {"mark": "hand",
                                                   "toolchain": HX32(0x7001)}})
    e_gr = pg.make_entry(K("author"), "grant", 2, eid(e_his),
                         {"grantee": AD("r1"), "work": CH, "terms": HX32(0x7E)})
    ents = [e_gen, e_his, e_gr]
    files = sorted(fbytes.items())
    proofs = [("anchor-1.json", b'{"header":"opaque to this law"}', HX32(0xA1)),
              ("anchor-2.json", b"\x00\x01\x02 raw bytes", HX32(0xA2))]

    full_m, full_disk = assemble(ents, files, proofs, root=ROOT,
                                 note_md="a kit that proves what its bytes say",
                                 contents_for={"art/final.bin"})

    make_kit("ok_full", "sec 7.4 step 7: entries, files, contents and proofs all agree "
                        "with the enumeration the walk yields.",
             full_m, full_disk)
    import copy as _copy
    fkey = sorted(k for k in full_disk if k.startswith("files/"))[0]
    same_len = bytes((b + 1) % 256 for b in full_disk[fkey])
    make_kit("file_wrong_sha_same_size",
             "sec 7.4 step 4: a file whose bytes have the listed length and another sha256 "
             "fails the digest leg alone.",
             full_m, dict(full_disk, **{fkey: same_len}))
    m_files = _copy.deepcopy(full_m); m_files["files"][0]["zz"] = 1
    make_kit("files_row_extra_member",
             "sec 7.3: a files element is an object with exactly path, sha256 and size; a "
             "foreign member fails the files rule.", m_files, full_disk)
    m_proofs = _copy.deepcopy(full_m); m_proofs["proofs"][0]["zz"] = 1
    make_kit("proofs_row_extra_member",
             "sec 7.3: a proofs element is an object with exactly path, sha256 and tx; a "
             "foreign member fails the proofs rule.", m_proofs, full_disk)
    make_kit("unreadable_directory",
             "sec 7.1: a directory the walk cannot list fails the walk with that directory's "
             "path as the subject (mode 0000; regenerate rather than copy).",
             full_m, full_disk, chmods=[("files/art", 0)])
    make_kit("unreadable_file",
             "sec 7.1: a file the walk cannot read in full fails the walk with that file's "
             "path as the subject (mode 0000; regenerate rather than copy).",
             full_m, full_disk, chmods=[(fkey, 0)])
    make_kit("kit_directory_unlistable",
             "sec 7.1: the kit directory itself cannot be listed; the subject is the "
             "one-character string `.` (mode 0000; regenerate rather than copy).",
             full_m, full_disk, chmod_root=0)
    make_kit("ok_empty", "sec 7.4: a directory holding only manifest.json with empty "
                         "arrays is a kit that proves nothing, and KIT_OK says so through "
                         "its counts.",
             mfst(None, [], [], [], [], ""), {})
    make_kit("ok_root_null", "sec 7.3: `root` is null or hex20, and null is the author "
                             "saying nothing about which ledger the entries belong to.",
             *assemble(ents, [], [], root=None))
    make_kit("ok_note_prose",
             "sec 7.3: `note_md` is a prose string, so control and astral scalars are "
             "lawful in it.",
             *assemble([], files, [], root=ROOT, note_md=" café \U0001f600"))
    seg255 = "a" * 251 + ".bin"
    make_kit("ok_segment_255",
             "sec 7.2: a segment of exactly 255 bytes, the largest a kit path admits.",
             *assemble([], [(seg255, b"at the segment ceiling\n")], [], root=ROOT))
    longp = "a" * 200 + "/" + "b" * 200 + "/" + "c" * 196 + ".bin"
    make_kit("ok_long_path",
             "sec 7.2: a 602-byte path of three segments, well inside the 1024-byte "
             "ceiling, walked through two directory levels.",
             *assemble([], [(longp, b"deep\n")], [], root=ROOT))
    make_kit("ok_walk_order_names",
             "sec 7.1/7.2: the directory `a` beside the files `a-b` and `a.b`. The three "
             "order as 0x2d < 0x2e < 0x2f, so the manifest's `files` order and the walk's "
             "order both turn on the separator byte, and a walk that sorts `a/b` under `a` "
             "yields the same enumeration as one that does not.",
             *assemble([], [("a-b", b"dash\n"), ("a.b", b"dot\n"),
                            ("a/b", b"under the directory\n")], [], root=ROOT))
    make_kit("ok_empty_directory",
             "sec 7.1: a directory contributes no pair to the enumeration, so an empty one "
             "is not an extra and the kit verifies.",
             full_m, full_disk, empty_dirs=["scratch"])
    bad_entry = b'{"spec":"zikaron/1"}'
    inv_m, inv_disk = assemble(ents, [], [], root=ROOT)
    inv_m["entries"] = sorted(inv_m["entries"] + [hx(sha256(bad_entry))], key=bwise)
    inv_disk["entries/" + hx(sha256(bad_entry))[2:] + ".zk1"] = bad_entry
    make_kit("ok_invalid_entry",
             "sec 7.4 step 7: a listed id whose bytes are at the right path with the right "
             "doc_id and fail `accept`. The kit is a pile: verification reports it in "
             "`invalid_entries` and does not fail, which is exactly what a MISSING row asks "
             "to see.",
             inv_m, inv_disk)

    # -- step 1: the manifest is absent -------------------------------------
    make_kit("manifest_absent",
             "sec 7.4 step 1: no pair at manifest.json.",
             no_manifest=True, disk={"files/note.txt": b"orphan\n"})
    make_kit("manifest_absent_empty_dir",
             "sec 7.4 step 1 over an empty directory: the enumeration is empty.",
             no_manifest=True, disk={})

    # -- step 2: the manifest's own rules -----------------------------------
    canon_m = cbytes(J(full_m))
    make_kit("rule_canonical_order", "sec 7.3 rule `canonical`: the members are the right "
                                     "members in the wrong order.",
             raw_manifest=pg.rot_top(canon_m, 3), disk=full_disk)
    make_kit("rule_canonical_utf8", "sec 7.3 rule `canonical`: bytes that are not UTF-8.",
             raw_manifest=canon_m[:5] + b"\xff" + canon_m[6:], disk=full_disk)
    make_kit("rule_canonical_newline",
             "sec 7.3 rule `canonical`: a trailing newline is one JSON text and not the "
             "canonical bytes.",
             raw_manifest=canon_m + b"\n", disk=full_disk)
    make_kit("rule_members_array", "sec 7.3 rule `members`: the root is not an object.",
             raw_manifest=b"[]")
    make_kit("rule_members_missing",
             "sec 7.3 rule `members`: the member set is not exactly the seven keys.",
             raw_manifest=pg.drop_key(canon_m, "proofs"), disk=full_disk)
    make_kit("rule_members_extra",
             "sec 7.3 rule `members`: a foreign key. One rule covers both directions here, "
             "unlike sec 4.2's two.",
             raw_manifest=pg.add_key(canon_m, "version", 1), disk=full_disk)
    m = dict(full_m)
    m["spec"] = "zikaron.kit/2"
    make_kit("rule_spec", "sec 7.3 rule `spec`.", m, full_disk)
    m = dict(full_m)
    m["root"] = "0x" + "a" * 39
    make_kit("rule_root_form", "sec 7.3 rule `root`: null or hex20.", m, full_disk)
    m = dict(full_m)
    m["root"] = 0
    make_kit("rule_root_int", "sec 7.3 rule `root`: an int is neither.", m, full_disk)

    m = dict(full_m)
    m["entries"] = list(reversed(full_m["entries"]))
    make_kit("rule_entries_unsorted", "sec 7.3 rule `entries`: sorted bytewise.", m,
             full_disk)
    m = dict(full_m)
    m["entries"] = [full_m["entries"][0], full_m["entries"][0]]
    make_kit("rule_entries_duplicate", "sec 7.3 rule `entries`: distinct.", m, full_disk)
    m = dict(full_m)
    m["entries"] = [HX20(1)]
    make_kit("rule_entries_form", "sec 7.3 rule `entries`: hex32 elements.", m, full_disk)
    m = dict(full_m)
    m["entries"] = "none"
    make_kit("rule_entries_notarray", "sec 7.3 rule `entries`: an array.", m, full_disk)

    m = dict(full_m)
    m["files"] = list(reversed(full_m["files"]))
    make_kit("rule_files_unsorted", "sec 7.3 rule `files`: sorted by `path` bytewise.", m,
             full_disk)
    m = dict(full_m)
    m["files"] = [full_m["files"][0], full_m["files"][0]]
    make_kit("rule_files_duplicate", "sec 7.3 rule `files`: paths distinct.", m, full_disk)
    for slug, path, why in [
            ("uppercase", "Note.txt", "sec 7.2: segments are drawn from a-z0-9._- so "
                                      "uppercase is outside the charset, which is what "
                                      "makes kit paths collide on no case-folding "
                                      "filesystem"),
            ("space", "my note.txt", "sec 7.2: the space is outside the segment charset"),
            ("dotdot", "art/../note.txt", "sec 7.2: no segment is `..`"),
            ("dot", "art/./note.txt", "sec 7.2: no segment is `.`"),
            ("leading_dash", "-note.txt", "sec 7.2: no segment begins with `-`"),
            ("leading_slash", "/note.txt", "sec 7.2: the path begins with no `/`"),
            ("empty_segment", "art//note.txt", "sec 7.2: every segment is one byte or "
                                               "more"),
            ("empty_path", "", "sec 7.2: one or more segments"),
            ("backslash", "art\\note.txt", "sec 7.2: `\\` is outside the segment charset "
                                           "and is not a separator here"),
            ("segment_256", "a" * 252 + ".bin", "sec 7.2: a segment of 256 bytes is one "
                                                "over the ceiling"),
            ("path_1025", "/".join(["a" * 204] * 4 + ["a" * 205]),
             "sec 7.2: 1025 bytes is one over the whole-path ceiling"),
            ("not_a_string", 7, "sec 7.3: `path` is a kit path, and a kit path is a "
                                "string")]:
        m = dict(full_m)
        m["files"] = [frow(path, hx(sha256(b"x")), 1)]
        m["contents"] = []
        make_kit("rule_files_path_" + slug, why + ".", m, full_disk)
    m = dict(full_m)
    m["files"] = [{"path": "note.txt", "sha256": hx(sha256(b"x"))}]
    m["contents"] = []
    make_kit("rule_files_row_members", "sec 7.3 rule `files`: rows are `{path, sha256, "
                                       "size}` and nothing else.", m, full_disk)
    m = dict(full_m)
    m["files"] = [frow("note.txt", hx(sha256(b"x")), "1")]
    m["contents"] = []
    make_kit("rule_files_size_form", "sec 7.3 rule `files`: `size` is an int.", m,
             full_disk)

    m = dict(full_m)
    m["contents"] = [crow(hx(sha256(b"x")), "nowhere.txt")]
    make_kit("rule_contents_unlisted_path",
             "sec 7.3 rule `contents`: `path` is a path listed in `files`.", m, full_disk)
    m = dict(full_m)
    m["contents"] = [crow(HX32(0xBAD), "art/final.bin")]
    make_kit("rule_contents_sha_mismatch",
             "sec 7.3 rule `contents`: the listed `sha256` of that path must equal "
             "`content`. This is the convention sec 7.6 pins, and it is a manifest rule "
             "and not a file test.", m, full_disk)
    m = dict(full_m)
    m["contents"] = [crow(CH, "art/final.bin"), crow(CH, "art/final.bin")]
    make_kit("rule_contents_duplicate_rows", "sec 7.3 rule `contents`: rows distinct.", m,
             full_disk)
    two_c = sorted([(hx(sha256(fbytes["art/final.bin"])), "art/final.bin"),
                    (hx(sha256(fbytes["note.txt"])), "note.txt")],
                   key=lambda x: (bwise(x[0]), bwise(x[1])))
    m = dict(full_m)
    m["contents"] = [crow(c, p) for c, p in reversed(two_c)]
    make_kit("rule_contents_unsorted",
             "sec 7.3 rule `contents`: sorted by `content` then `path`.", m, full_disk)

    m = dict(full_m)
    m["proofs"] = list(reversed(full_m["proofs"]))
    make_kit("rule_proofs_unsorted", "sec 7.3 rule `proofs`: sorted by `path`.", m,
             full_disk)
    m = dict(full_m)
    m["proofs"] = [full_m["proofs"][0], full_m["proofs"][0]]
    make_kit("rule_proofs_duplicate", "sec 7.3 rule `proofs`: paths distinct.", m,
             full_disk)
    m = dict(full_m)
    m["proofs"] = [prow("anchor-1.json", hx(sha256(b"x")), HX20(1))]
    make_kit("rule_proofs_tx_form", "sec 7.3 rule `proofs`: `tx` is hex32.", m, full_disk)
    m = dict(full_m)
    m["note_md"] = None
    make_kit("rule_note_md", "sec 7.3 rule `note_md`: a prose string, and null is not a "
                             "string.", m, full_disk)
    m = dict(full_m)
    m["note_md"] = ["a"]
    make_kit("rule_note_md_array", "sec 7.3 rule `note_md`: an array of prose is not "
                                   "prose.", m, full_disk)

    def mut(**kw):
        m = json.loads(json.dumps(full_m))
        m.update(kw)
        return m
    fm = json.loads(json.dumps(full_m))
    make_kit("rule_files_notarray", "sec 7.3 rule `files`: the member is not an array.",
             mut(files="x"), full_disk)
    r = json.loads(json.dumps(fm["files"])); r[0]["sha256"] = "0x12"
    make_kit("rule_files_sha_form", "sec 7.3 rule `files`: a row's sha256 is not hex32.",
             mut(files=r), full_disk)
    make_kit("rule_contents_notarray", "sec 7.3 rule `contents`: the member is not an array.",
             mut(contents=5), full_disk)
    r = json.loads(json.dumps(fm["contents"])); r[0]["extra"] = 1
    make_kit("rule_contents_row_members",
             "sec 7.3 rule `contents`: a row whose member set is not exactly {content, path}.",
             mut(contents=r), full_disk)
    r = json.loads(json.dumps(fm["contents"])); r[0]["content"] = "0xAB"
    make_kit("rule_contents_content_form",
             "sec 7.3 rule `contents`: a row's content is not hex32.", mut(contents=r), full_disk)
    make_kit("rule_proofs_notarray", "sec 7.3 rule `proofs`: the member is not an array.",
             mut(proofs="p"), full_disk)
    r = json.loads(json.dumps(fm["proofs"])); del r[0]["tx"]
    make_kit("rule_proofs_row_members",
             "sec 7.3 rule `proofs`: a row whose member set is not exactly {path, sha256, tx}.",
             mut(proofs=r), full_disk)
    r = json.loads(json.dumps(fm["proofs"])); r[0]["path"] = "-bad"
    make_kit("rule_proofs_path_form",
             "sec 7.3 rule `proofs`: a row's path is not a kit path (it begins with `-`).",
             mut(proofs=r), full_disk)
    r = json.loads(json.dumps(fm["proofs"])); r[0]["sha256"] = "zz"
    make_kit("rule_proofs_sha_form", "sec 7.3 rule `proofs`: a row's sha256 is not hex32.",
             mut(proofs=r), full_disk)

    # -- steps 3, 4, 5 ------------------------------------------------------
    d2 = dict(full_disk)
    del d2["entries/" + eid(e_his)[2:] + ".zk1"]
    make_kit("entry_bytes_absent",
             "sec 7.4 step 3: no pair at entries/<id>.zk1 for a listed id.", full_m, d2)
    d2 = dict(full_disk)
    d2["entries/" + eid(e_his)[2:] + ".zk1"] = e_gr
    make_kit("entry_bytes_mismatch",
             "sec 7.4 step 3: the pair is there and its doc_id is not the id. Identity is "
             "total, so this is decided without asking whether the bytes are an entry.",
             full_m, d2)
    d2 = dict(full_disk)
    del d2["files/note.txt"]
    make_kit("file_absent", "sec 7.4 step 4: no pair at files/<path>.", full_m, d2)
    d2 = dict(full_disk)
    d2["files/note.txt"] = b"other bytes entirely\n"
    make_kit("file_wrong_sha256", "sec 7.4 step 4: the pair's bytes have another sha256.",
             full_m, d2)
    m = dict(full_m)
    m["files"] = [frow(r["path"], r["sha256"],
                       r["size"] + 1 if r["path"] == "note.txt" else r["size"])
                  for r in full_m["files"]]
    make_kit("file_wrong_size",
             "sec 7.4 step 4: the sha256 agrees and the length does not, and the length is "
             "its own test.", m, full_disk)
    d2 = dict(full_disk)
    del d2["proofs/anchor-2.json"]
    make_kit("proof_absent", "sec 7.4 step 5: no pair at proofs/<path>.", full_m, d2)
    d2 = dict(full_disk)
    d2["proofs/anchor-1.json"] = b"a different opaque blob"
    make_kit("proof_wrong_sha256",
             "sec 7.4 step 5: the proof's bytes are pinned by digest and read no further "
             "(sec 7.5).", full_m, d2)

    # -- step 6: extras -----------------------------------------------------
    make_kit("extra_top", "sec 7.4 step 6: a pair whose path is not in the named set.",
             full_m, full_disk, extras=[("readme.txt", b"stray\n")])
    make_kit("extra_in_files",
             "sec 7.4 step 6: an unlisted file under files/ is an extra like any other.",
             full_m, full_disk, extras=[("files/uncounted.bin", b"stray\n")])
    make_kit("extra_ds_store",
             "sec 7.4 step 6: a filesystem's own droppings are pairs of the enumeration; "
             "`.DS_Store` is not even a kit path, and the named set decides the question "
             "without asking.",
             full_m, full_disk, extras=[(".DS_Store", b"\x00\x01mac\n")])
    make_kit("extra_smallest_of_several",
             "sec 7.4 step 6: several extras, and the bytewise-smallest path is the "
             "subject: `.DS_Store` (0x2e) before `files/uncounted.bin` (0x66) before "
             "`zz.txt` (0x7a).",
             full_m, full_disk,
             extras=[("zz.txt", b"z\n"), ("files/uncounted.bin", b"u\n"),
                     (".DS_Store", b"m\n")])
    make_kit("extra_entries_dir",
             "sec 7.4 step 6: a byte string under entries/ that no listed id names.",
             full_m, full_disk,
             extras=[("entries/" + HX32(0xFFFF)[2:] + ".zk1", b"unlisted\n")])

    # -- order of the steps -------------------------------------------------
    d2 = dict(full_disk)
    del d2["files/note.txt"]
    make_kit("order_file_before_extra",
             "sec 7.4 order: step 4 is decided before step 6, so the absent file is named "
             "and not the extra.", full_m, d2, extras=[("aaa.txt", b"a\n")])
    d2 = dict(full_disk)
    del d2["files/note.txt"]
    del d2["entries/" + eid(e_gen)[2:] + ".zk1"]
    make_kit("order_entry_before_file",
             "sec 7.4 order: step 3 before step 4.", full_m, d2)
    m = dict(full_m)
    m["spec"] = "wrong"
    d2 = dict(full_disk)
    del d2["entries/" + eid(e_gen)[2:] + ".zk1"]
    make_kit("order_manifest_before_entry",
             "sec 7.4 order: step 2 before step 3.", m, d2)
    d2 = dict(full_disk)
    d2["proofs/anchor-1.json"] = b"changed"
    del d2["files/note.txt"]
    make_kit("order_file_before_proof",
             "sec 7.4 order: step 4 before step 5.", full_m, d2)

    # -- step 0: the walk ---------------------------------------------------
    make_kit("unreadable_fifo",
             "sec 7.1: an entry that is neither a regular file nor a directory (a named "
             "pipe). The walk fails there without opening it; a reader that opens before it "
             "classifies would wait on the pipe forever.",
             full_m, full_disk, fifos=["zz-pipe"])
    make_kit("unreadable_symlink",
             "sec 7.1: the walk fails at the first entry it examines that is a symbolic "
             "link, and verify_kit examines nothing else, so a kit that would otherwise "
             "verify is E_KIT_UNREADABLE.",
             full_m, full_disk, symlinks=[("zz-link", "manifest.json")])
    make_kit("unreadable_symlink_in_files",
             "sec 7.1: a symbolic link under files/ carries its relative path as the "
             "subject.",
             full_m, full_disk, symlinks=[("files/link.bin", "../manifest.json")])
    make_kit("unreadable_walk_order",
             "sec 7.1: the walk examines names in bytewise order and walks a directory "
             "before moving to the next name, so `a` is entered before `a-b` is examined "
             "and the subject is `a/z`, not `a-b`. The regular file `a.b` would be an "
             "extra under sec 7.4 step 6 and is never reached.",
             mfst(None, [], [], [], [], ""), {},
             extras=[("a.b", b"a dot b\n")],
             symlinks=[("a/z", "../manifest.json"), ("a-b", "manifest.json")])
    make_kit("unreadable_beats_manifest_absent",
             "sec 7.1: the walk's failure precedes every test of sec 7.4, the manifest's "
             "presence included.",
             no_manifest=True, disk={}, symlinks=[("link", "nowhere")])

    # -- 20 randomized valid kits -------------------------------------------
    for i in range(20):
        n_e = rng.randrange(0, 4)
        es = []
        prev = None
        for k in range(n_e):
            if k == 0:
                b = pg.make_entry(K("author"), "genesis", 0, None,
                                  {"statement_md": pg.rand_prose(rng)})
            else:
                b = pg.make_entry(K("author"), rng.choice(["history", "annotation"]), k,
                                  prev, pg.random_body(rng, "history") if k % 2 else
                                  {"note_md": pg.rand_prose(rng)})
            prev = eid(b)
            es.append(b)
        n_f = rng.randrange(0, 4)
        fs = []
        used = set()
        for k in range(n_f):
            segs = [("".join(rng.choice("abcdefghijklmnopqrstuvwxyz0123456789._-")
                             for _ in range(rng.randrange(1, 9))))
                    for _ in range(rng.randrange(1, 3))]
            segs = [s if not s.startswith("-") else "a" + s[1:] for s in segs]
            segs = [s if s not in (".", "..") else "a" for s in segs]
            p = "/".join(segs)
            if p in used or not kit_path_ok(p):
                continue
            used.add(p)
            fs.append((p, bytes(rng.randrange(256) for _ in range(rng.randrange(0, 40)))))
        n_p = rng.randrange(0, 3)
        ps = []
        for k in range(n_p):
            ps.append(("p%d.bin" % k,
                       bytes(rng.randrange(256) for _ in range(rng.randrange(1, 30))),
                       HX32(rng.getrandbits(256))))
        rt = None if rng.random() < 0.3 else HX20(rng.getrandbits(160))
        cf = set(p for p, _ in fs if rng.random() < 0.6)
        mm, dd = assemble(es, fs, ps, root=rt, note_md=pg.rand_prose(rng), contents_for=cf)
        make_kit("rand_%02d" % i,
                 "randomized lawful kit: %d entries, %d files, %d proofs."
                 % (len(es), len(fs), len(ps)), mm, dd)


# ==========================================================================
# ledger and audit-input construction for the reading predicates
# ==========================================================================

REG = HX20(0x9E6)


def basis(chains=(), bare=(), adopt=()):
    return {"chains": list(chains), "bareTx": list(bare), "adoptionChains": list(adopt)}


def chainobj(cid=1, fb=0, tb=1000, regs=(REG,), senders=()):
    # parent sec 9.4: registries and senders are bytewise ascending
    return {"chainId": cid, "fromBlock": fb, "toBlock": tb,
            "registries": sorted(set(regs)), "senders": sorted(set(senders))}


def anchor(h, sender, cid=1, bn=100, ts=1700000000, tx=None, verdict="counted"):
    return {"chainId": cid, "blockNumber": bn, "blockTimestamp": ts,
            "tx": tx if tx else HX32(0xA00000 + cid * 4096 + bn), "sender": sender,
            "hash": h, "verdict": verdict}


def AINP(root, pile, anchors=(), unavailable=(), evidence=(), bas=None):
    return {"root": root, "pile": [hx(b) for b in pile], "anchors": list(anchors),
            "unavailable": list(unavailable), "evidence": list(evidence),
            "basis": bas if bas is not None else basis()}


def E(keytag, etype, seq, prev, body):
    return pg.make_entry(K(keytag), etype, seq, prev, body)


def build_seq(specs):
    """specs: (keytag, etype, seq, body); each entry's `prev` is the previous
    entry of the list, so a gap in `seq` leaves the link unverifiable."""
    out, ids = [], []
    prev = None
    for keytag, etype, seq, body in specs:
        b = E(keytag, etype, seq, (None if seq == 0 else prev), body)
        out.append(b)
        ids.append(eid(b))
        prev = eid(b)
    return out, ids


GEN = {"statement_md": "one author's registrations"}


def gbody(grantee, work, terms=None, window=None, upstream=UNSET, scope=None):
    b = {"grantee": grantee, "work": work, "terms": terms or HX32(0x7E)}
    if window is not None:
        b["window"] = window
    if scope is not None:
        b["scope_md"] = scope
    if upstream is not UNSET:
        b["upstream"] = upstream
    return b


def issuer(keytag, body, tail="annotation"):
    """A three-entry ledger whose grant sits at seq 1.  The pile carries the
    genesis and the tail and never the grant, so sec 10.1's I' is the only way
    the grant reaches the ledger."""
    gen = E(keytag, "genesis", 0, None, GEN)
    g = E(keytag, "grant", 1, eid(gen), body)
    if tail == "revocation":
        t2 = E(keytag, "revocation", 2, eid(g), {"grant": eid(g)})
    elif tail == "revocation_other":
        t2 = E(keytag, "revocation", 2, eid(g), {"grant": HX32(0xDEAD)})
    else:
        t2 = E(keytag, "annotation", 2, eid(g), {"note_md": "after the grant"})
    return gen, g, t2


def cover(senders, cid=(1,)):
    return basis([chainobj(c, 0, 1000, (REG,), list(senders)) for c in cid])


# ==========================================================================
# depth/ : sec 9
# ==========================================================================


def _reversed_arrays(inp):
    return dict((k, (list(reversed(v)) if isinstance(v, list) else v)) for k, v in inp.items())


def Z(slug, inp, work, note, twin=True):
    if twin and isinstance(inp, dict) and _reversed_arrays(inp) != inp:
        Z(slug + "_reversed", _reversed_arrays(inp), work, "input arrays reversed: " + note,
          twin=False)
    i = seqno("depth")
    p = WJ("depth/z%03d_%s.audit.json" % (i, slug), inp)
    W("depth/z%03d_%s.work" % (i, slug), (work + "\n").encode())
    CASE("depth", p, "depth", ["depth", p, work], note, depth(inp, work), work=work)


def build_depth(rng, t):
    R = AD("author")
    WD = hx(sha256(b"the work under depth"))
    WD2 = hx(sha256(b"a second digest of one work"))
    HB = lambda c: {"content": c, "mode": {"mark": "hand", "toolchain": HX32(0x7001)}}

    L, LI = build_seq([("author", "genesis", 0, GEN),
                       ("author", "history", 1, HB(WD)),
                       ("author", "annotation", 2, {"note_md": "between"}),
                       ("author", "history", 3, HB(WD)),
                       ("author", "history", 4, HB(WD2))])

    Z("invalid_input",
      {"root": R, "pile": [], "anchors": [], "unavailable": [], "evidence": [],
       "basis": {"chains": [], "bareTx": []}},
      WD, "sec 9.2: an input the core refuses under the parent's sec 9.4 (the basis lacks "
          "`adoptionChains`). The reading is {\"valid\": false} and nothing else.")
    Z("input_not_object", [1, 2], WD,
      "sec 9.2: an input that is not an object at all. The reading is {\"valid\": false}.")
    Z("input_pile_not_array", dict(AINP(R, L), pile="0x00"), WD,
      "sec 9.2: an input whose pile is not an array. The reading is {\"valid\": false}.")
    Z("duplicate_pile_bytes", AINP(R, L + L), WD,
      "parent sec 8.1: byte-identical copies collapse to one entry, so the reading over the "
      "doubled pile equals the reading over the pile.")
    STRAY = E("author", "history", 3, HX32(0xBAD0), HB(WD))
    Z("counted_entry_not_reaching_first",
      AINP(R, L[:3] + [STRAY], [anchor(LI[1], R), anchor(eid(STRAY), R)], bas=cover([R])), WD,
      "sec 9: a counted history at seq 3 whose prev names no entry in hand, so the first "
      "history is not reachable from it; its seq does not join the reading's counted seqs.")
    Z("no_history_for_work", AINP(R, L), HX32(0xABCDEF),
      "sec 9.2: a work digest no history entry of this ledger names. `found` is false and "
      "the three dependent members take their stated values.")
    Z("second_digest_of_one_work", AINP(R, L), WD2,
      "sec 9.2: two digests of one work are two readings; this one finds a single "
      "unanchored history entry.")
    Z("found_none_anchored", AINP(R, L), WD,
      "sec 9.2: `found` is true with no anchors at all, so `earliest` is null, `deepest` is "
      "0 and `continuity.anchored` is 0 while `span` is not.")
    Z("one_anchored_direct",
      AINP(R, L, [anchor(LI[1], R, ts=1700000100)], bas=cover([R])), WD,
      "sec 8.2: a counted anchor whose hash is the history entry's own entry_id. The bound "
      "is that record's blockTimestamp and the entry counts in `continuity.anchored`.")
    Z("bounded_by_later_anchor",
      AINP(R, L[:3], [anchor(LI[2], R, ts=1700000200)], bas=cover([R])), WD,
      "sec 8.2: the anchor names a later entry from which the history entry is reachable, "
      "so the history entry is anchored and bounded. sec 9.2's `continuity.anchored` asks "
      "for a *direct* anchor and counts nothing here, which is the difference between a "
      "bound and a direct anchor stated twice.")
    Z("two_chains_smallest_bound",
      AINP(R, L, [anchor(LI[1], R, cid=1, bn=500, ts=1700000900),
                  anchor(LI[1], R, cid=137, bn=7, ts=1700000400)],
           bas=cover([R], cid=(1, 137))), WD,
      "sec 8.2: two records of one entry_id on two chains are two anchors (parent sec 9.2), "
      "and this law's reader takes the smallest blockTimestamp across the chains its basis "
      "declares, the single bound the parent's sec 9.6 leaves to a reader who says which "
      "chains it weighed.")
    Z("several_histories_different_bounds",
      AINP(R, L, [anchor(LI[1], R, cid=1, bn=10, ts=1700001000),
                  anchor(LI[3], R, cid=137, bn=20, ts=1700000700)],
           bas=cover([R], cid=(1, 137))), WD,
      "sec 9.2: two history entries of one digest, each directly anchored, with bounds from "
      "two chains; `earliest` is the smallest and `deepest` is two.")

    LG, LGI = build_seq([("author", "genesis", 0, GEN),
                         ("author", "history", 1, HB(WD)),
                         ("author", "annotation", 2, {"note_md": "x"}),
                         ("author", "history", 3, HB(WD)),
                         ("author", "annotation", 4, {"note_md": "y"}),
                         ("author", "history", 5, HB(WD))])
    Z("continuity_gaps",
      AINP(R, LG, [anchor(LGI[1], R, bn=1, ts=1700000100),
                   anchor(LGI[5], R, bn=2, ts=1700000500)], bas=cover([R])), WD,
      "sec 9.2: H spans seq 1 to 5, two of the six positions carry a direct anchor on H0's "
      "own line, and every element of H is anchored because the anchor at seq 5 bounds the "
      "line beneath it. `deepest` 3 against `continuity` 2 of 5 is the whole point of "
      "reading both.")
    Z("continuity_h0_equals_hmax",
      AINP(R, L[:3], [anchor(LI[1], R, ts=1700000100)], bas=cover([R])), WD,
      "sec 9.2: one history entry, so H0 is Hmax and `span` is 1.")

    tw_a = E("author", "history", 2, LI[1], HB(WD))
    got = {}
    k = 0
    while len(got) < 2:
        tw_b = E("author", "history", 2, LI[1], dict(HB(WD), note_md="the other twin %d" % k))
        got.setdefault(eid(tw_a) < eid(tw_b), tw_b)
        k += 1
    for asc in (True, False):
        Z("twins_at_a_position" + ("_ids_ascending" if asc else "_ids_descending"),
          AINP(R, L[:2] + [tw_a, got[asc]],
               [anchor(eid(tw_a), R, ts=1700000300)], bas=cover([R])), WD,
          "sec 9.2: two history entries of one digest share a seq, so H0 is the one with the "
          "bytewise-smallest entry_id among equals. The fork is hard and the label travels with "
          "the reading. Here the anchored twin has the %s id." % ("smaller" if asc else "larger"))

    fork_c = E("author", "history", 2, LI[1], HB(WD))
    fork_d = E("author", "annotation", 2, LI[1], {"note_md": "the surviving line"})
    after = E("author", "annotation", 3, eid(fork_d), {"note_md": "built on d"})
    Z("fork_h_on_losing_line",
      AINP(R, L[:2] + [fork_c, fork_d, after],
           [anchor(eid(after), R, ts=1700000600)], bas=cover([R])), WD,
      "sec 8.2 and the parent's sec 9.6: the anchor bounds one line of the fork and never "
      "the other. The history entry at seq 2 sits on the line the anchored entry does not "
      "reach, so it is not anchored and does not count.")

    Z("unproven_and_void_anchors",
      AINP(R, L, [anchor(LI[1], R, bn=1, verdict="UNPROVEN"),
                  anchor(LI[3], R, bn=2, verdict="VOID")], bas=cover([R])), WD,
      "sec 8.2: the counted anchors are the ones carrying verdict `counted`. An UNPROVEN "
      "and a VOID record take no part, so nothing is anchored; the UNPROVEN record is over "
      "bytes in hand, so under the parent's sec 8.7 rule 2 it moves no label.")
    TWA = E("author", "history", 1, LI[0], dict(HB(WD), note_md="twin a"))
    TWB = E("author", "history", 1, LI[0], dict(HB(WD), note_md="twin b"))
    TWHI = max([TWA, TWB], key=lambda b: bytes.fromhex(eid(b)[2:]))
    Z("h0_twins_same_content_anchor_on_larger",
      AINP(R, [L[0], TWA, TWB], [anchor(eid(TWHI), R)], bas=cover([R])), WD,
      "sec 9.2: two histories of one work at one seq; H0 is the bytewise-smallest entry_id, "
      "so an anchor on the other twin counts toward deepest and not toward continuity.")
    Z("nonlineage_sender_trimmed",
      AINP(R, L, [anchor(LI[1], AD("stranger"), ts=1700000100)], bas=cover([R, AD("stranger")])), WD,
      "sec 8.1: the audit discards every anchor record whose sender is not in the "
      "lineage, so no key outside the lineage can put a hash into this ledger's "
      "reconciliation.")
    LS, LSI = build_seq([("author", "genesis", 0, GEN),
                         ("author", "succession", 1, {"to": AD("k2"), "kind": "handover",
                                                      "effective": 1700000000,
                                                      "statement_md": "the seat moves"}),
                         ("k2", "history", 2, HB(WD))])
    Z("lineage_sender_after_succession",
      AINP(R, LS, [anchor(LSI[2], AD("k2"), ts=1700000800)], bas=cover([R, AD("k2")])), WD,
      "sec 8.1: every key that has held the seat can put a hash into the reconciliation, "
      "whenever it sent the transaction; the successor's anchor counts.")
    Z("history_outside_lineage",
      AINP(R, [L[0]] + [E("stranger", "genesis", 0, None, GEN),
                        E("stranger", "history", 1,
                          eid(E("stranger", "genesis", 0, None, GEN)), HB(WD))]), WD,
      "sec 8.1: entries signed outside the whole-set lineage are not input, so a stranger's "
      "history of this digest is invisible to the reading.")
    Z("empty_pile", AINP(R, []), WD,
      "sec 9.2 over an empty ledger: found false on a COMPLETE label that asserts only that "
      "the inputs held no fault.")


# ==========================================================================
# checks/ : sec 10.2
# ==========================================================================


def CK(slug, g, inp, now, note, expect_verdict=None, twin=True):
    if twin and isinstance(inp, dict) and _reversed_arrays(inp) != inp:
        CK(slug + "_reversed", g, _reversed_arrays(inp), now, "input arrays reversed: " + note,
           expect_verdict, twin=False)
    i = seqno("checks")
    base = "checks/c%03d_%s" % (i, slug)
    gp = W(base + ".grant.zk1", g)
    args = ["grant-check", gp]
    ap = None
    if inp is not None:
        ap = WJ(base + ".audit.json", inp)
        args += ["--audit", ap]
    if now is not None:
        args += ["--now", str(now)]
    pred = grant_check(g, inp, now)
    extra = {"audit_path": ap, "now": now}
    if expect_verdict is not None and expect_verdict != pred["verdict"]:
        extra["disagreement"] = True
        NOTES.append("checks %s: hand=%s code=%s" % (base, expect_verdict, pred["verdict"]))
    CASE("checks", gp, "grant-check", args, note, pred, **extra)
    return gp


def build_checks(rng, t):
    IS = AD("iss")
    WK = HX32(0xC0DE)
    body = gbody(AD("r1"), WK)
    gen, G, ann = issuer("iss", body)
    COV = cover([IS])
    A_G = [anchor(eid(G), IS, ts=1700000000)]

    # ---- check 1: every parent token, and NOT_A_GRANT --------------------
    seq1 = 1
    prev1 = eid(gen)
    GV = G
    canon = GV
    variants = [
        ("E_UTF8", canon[:10] + b"\xff" + canon[11:]),
        ("E_JSON", canon + b"]"),
        ("E_NUMBER", pg.sub(canon, b'"seq":1', b'"seq":01')),
        ("E_DEPTH", pg.add_key(canon, "z", pg.deep_list(128))),
        ("E_DUP_KEY", pg.sub(canon, b'"seq":1', b'"seq":1,"seq":1')),
        ("E_KEY_CHARSET", pg.sub(canon, b'"seq":', b'"\\u0001seq":')),
        ("E_VALUE_CHARSET", pg.sub(canon, b'"spec":"zikaron/1"',
                                   b'"spec":"\\u0001zikaron/1"')),
        ("E_NOT_CANONICAL", pg.rot_top(canon, 2)),
        ("E_ENVELOPE", b"[]"),
        ("E_ENVELOPE_MISSING", pg.drop_key(canon, "prev")),
        ("E_ENVELOPE_CLOSED", pg.add_key(canon, "extra", 1)),
        ("E_SPEC", pg.set_key(canon, "spec", "zikaron/2")),
        ("E_ENTRYTYPE", pg.set_key(canon, "entryType", "")),
        ("E_AUTHOR", pg.set_key(canon, "author", "0x" + "A" * 40)),
        ("E_SEQ", pg.set_key(canon, "seq", "1")),
        ("E_PREV", pg.set_key(canon, "prev", HX20(1))),
        ("E_PREV_SEQ", E("iss", "grant", 1, None, body)),
        ("E_BODY", pg.set_key(canon, "body", [])),
        ("E_SIG_FORM", pg.set_key(canon, "sig", "0x")),
        ("E_GENESIS_PLACE", E("iss", "grant", 0, None, body)),
        ("E_BODY_FIELD", E("iss", "grant", seq1, prev1,
                           {"grantee": AD("r1"), "work": WK})),
        ("E_SIG_V", resign_with(canon, *(sig_parts(sig_of(canon))[:2] + (0,)))),
        ("E_SIG_RANGE", resign_with(canon, 0, sig_parts(sig_of(canon))[1],
                                    sig_parts(sig_of(canon))[2])),
        ("E_SIG_HIGH_S", resign_with(canon, sig_parts(sig_of(canon))[0],
                                     N - sig_parts(sig_of(canon))[1],
                                     55 - sig_parts(sig_of(canon))[2])),
        ("E_SIG_RECOVER", resign_with(canon, NONRES_R, sig_parts(sig_of(canon))[1],
                                      sig_parts(sig_of(canon))[2])),
        ("E_SIG_SIGNER", pg.make_entry(K("stranger"), "grant", seq1, prev1, body,
                                       author=IS)),
    ]
    for tok, b in variants:
        ok, got = pg.zk_check(b)
        if ok or got != tok:
            NOTES.append("checks: check-1 case meant to be %s is %s" %
                         (tok, "accepted" if ok else got))
        CK("c1_" + tok.lower(), b, None, None,
           "sec 10.2 check 1: `g` fails `accept` with the parent's %s. A FAIL here leaves "
           "checks 2 to 6 UNKNOWN and the verdict is FAIL." % tok, "FAIL")
    _gen0, _g0, _t20 = issuer("iss", gbody(AD("r4"), HX32(0xC0DE)))
    CK("audit_input_not_object", _g0, [1, 2], None,
       "sec 10.1: an audit input that is not an object; I' cannot be formed, the input is "
       "invalid under the parent's sec 9.4, and the checks that need it are UNKNOWN.")
    CK("audit_input_pile_not_array", _g0, {"root": AD("iss"), "pile": "0x00", "anchors": [],
                                          "unavailable": [], "evidence": [], "basis": basis()},
       None,
       "sec 10.1: an audit input whose pile is not an array; I' cannot be formed.")
    CK("c1_not_a_grant", E("iss", "history", 1, prev1,
                           {"content": WK, "mode": {"mark": "hand",
                                                    "toolchain": HX32(1)}}),
       None, None,
       "sec 10.2 check 1: an accepted entry that is not of type grant, carrying reason "
       "NOT_A_GRANT.", "FAIL")
    CK("c1_not_a_grant_with_audit",
       E("iss", "annotation", 2, eid(G), {"note_md": "a note, not a grant"}),
       AINP(IS, [gen, ann], A_G, bas=COV), 1700000500,
       "sec 10.2 check 1: a FAIL at check 1 leaves every later check UNKNOWN even where the "
       "audit input would have decided them, and `basis` still carries the basis of I.",
       "FAIL")

    # ---- the verdicts -----------------------------------------------------
    CK("green_anchored", G, AINP(IS, [gen, ann], A_G, bas=COV), 1700000500,
       "sec 10.2: every check passes. The grant is a ledger entry of I' (sec 10.1 adds it "
       "to the pile), it is anchored, the basis is covering, the label is COMPLETE and no "
       "revocation names it.", "GREEN")
    CK("green_no_now", G, AINP(IS, [gen, ann], A_G, bas=COV), None,
       "sec 10.2 check 5: with no `window` the check passes without reading `now`, so a "
       "caller with no trusted time still reaches GREEN.", "GREEN")
    CK("green_bounded_by_later_anchor", G,
       AINP(IS, [gen, ann], [anchor(eid(ann), IS, ts=1700000700)], bas=COV), None,
       "sec 10.2 check 4 through sec 8.2: the anchor names a later entry from which the "
       "grant is reachable.", "GREEN")
    genw, GW, annw = issuer("iss", gbody(AD("r1"), WK, window={"from": 1000, "to": 2000}))
    IW = AINP(IS, [genw, annw], [anchor(eid(GW), IS, ts=1700000000)], bas=COV)
    CK("c5_window_at_from", GW, IW, 1000,
       "sec 10.2 check 5: the window test is inclusive at both ends.", "GREEN")
    CK("c5_window_at_to", GW, IW, 2000,
       "sec 10.2 check 5: inclusive at the upper end too.", "GREEN")
    CK("c5_window_inside", GW, IW, 1500, "sec 10.2 check 5: inside the window.", "GREEN")
    CK("c5_window_before", GW, IW, 999,
       "sec 10.2 check 5: `now` below `from`.", "FAIL")
    CK("c5_window_after", GW, IW, 2001,
       "sec 10.2 check 5: `now` above `to`.", "FAIL")
    CK("c5_window_now_null", GW, IW, None,
       "sec 10.2 check 5: a window with no trusted time is UNKNOWN, never a pass and never "
       "a failure.", "PARTIAL")

    CK("no_audit_input", G, None, 1700000500,
       "sec 10.1: the caller holds no audit input for the issuer, so checks 2, 3, 4 and 6 "
       "are UNKNOWN and `basis` is null.", "PARTIAL")
    CK("no_audit_no_now", G, None, None,
       "sec 10.1 and check 5: nothing but the bytes; only checks 1 and 5 can be decided.",
       "PARTIAL")
    CK("invalid_audit_input", G,
       {"root": IS, "pile": [hx(gen)], "anchors": [], "unavailable": [], "evidence": [],
        "basis": {"chains": [], "bareTx": [], "adoptionChains": [], "extra": 1}},
       1700000500,
       "sec 10.2: an invalid audit input is read exactly as a null one, and `basis` is "
       "null.", "PARTIAL")

    # ---- check 2 ----------------------------------------------------------
    tw1 = E("iss", "annotation", 2, eid(G), {"note_md": "left"})
    tw2 = E("iss", "annotation", 2, eid(G), {"note_md": "right"})
    CK("c2_broken_chain", G, AINP(IS, [gen, tw1, tw2], A_G, bas=COV), 1700000500,
       "sec 10.2 check 2: the report of I' labels BROKEN_CHAIN (one EQUIVOCATION, hard), "
       "which leaves checks 3, 4 and 6 UNKNOWN while check 5 is still decided.", "FAIL")

    # ---- check 3 ----------------------------------------------------------
    Gs = E("stranger", "grant", 1, eid(gen), body)
    CK("c3_author_outside_lineage", Gs, AINP(IS, [gen], [], bas=COV), None,
       "sec 10.2 check 3: `g` is not a ledger entry of I' because its author is outside the "
       "root's lineage. sec 8.1 lists it as excluded and convicts nobody; the check fails "
       "and leaves 4 and 6 UNKNOWN.", "FAIL")
    SUC = {"to": AD("iss2"), "kind": "handover", "effective": 1700000000,
           "statement_md": "the seat moves"}
    LA, LAI = build_seq([("iss", "genesis", 0, GEN),
                         ("iss", "succession", 5, SUC),
                         ("iss", "grant", 6, body)])
    Gsoft = LA[2]
    CK("c3_soft_authority_after_gap", Gsoft,
       AINP(IS, [LA[0], LA[1]], [anchor(eid(Gsoft), IS, ts=1700000000)],
            bas=cover([IS, AD("iss2")])),
       None,
       "sec 10.2 check 3: the grant carries an AUTHORITY_MISMATCH the walk recorded soft, "
       "because a gap could hide a succession. Soft is UNKNOWN here, since a hard one would "
       "have labelled the ledger broken at check 2.", "PARTIAL")

    # ---- check 4 ----------------------------------------------------------
    CK("c4_unanchored_complete_covering", G,
       AINP(IS, [gen, ann], [anchor(eid(gen), IS, ts=1700000000)], bas=COV), None,
       "sec 10.2 check 4: the grant is not anchored (an anchor of an earlier entry bounds "
       "nothing later), the basis is covering and the label is COMPLETE, so absence "
       "convicts.", "FAIL")
    CK("c4_not_covering_chains_empty", G,
       AINP(IS, [gen, ann], [], bas=basis()), None,
       "sec 10.2 check 4: a basis whose `chains` array is empty is not covering, and the "
       "scan's silence convicts nobody.", "PARTIAL")
    CK("c4_not_covering_registries_empty", G,
       AINP(IS, [gen, ann], [], bas=basis([chainobj(1, 0, 1000, (), [IS])])), None,
       "sec 10.2 check 4: a chains object with an empty `registries` array is not covering.",
       "PARTIAL")
    LS2, LS2I = build_seq([("iss", "genesis", 0, GEN),
                           ("iss", "grant", 1, body),
                           ("iss", "succession", 2, SUC),
                           ("iss2", "annotation", 3, {"note_md": "new hand"})])
    G2 = LS2[1]
    CK("c4_not_covering_sender_absent", G2,
       AINP(IS, [LS2[0], LS2[2], LS2[3]], [], bas=cover([IS])), None,
       "sec 10.2 check 4: the ledger's whole-set lineage holds the successor's key and the "
       "chain object's `senders` does not, so the basis says nothing about that key's "
       "anchors and is not covering.", "PARTIAL")
    LGAP, LGAPI = build_seq([("iss", "genesis", 0, GEN),
                             ("iss", "grant", 1, body),
                             ("iss", "annotation", 3, {"note_md": "after a gap"})])
    Gg = LGAP[1]
    CK("c4_label_gaps", Gg, AINP(IS, [LGAP[0], LGAP[2]], [], bas=COV), None,
       "sec 10.2 check 4: covering basis, grant unanchored, label GAPS. A MISSING or "
       "unavailable entry may bound it, so the check is UNKNOWN and check 6 is UNKNOWN "
       "with it.", "PARTIAL")
    CK("c4_label_unavailable_hash", G,
       AINP(IS, [gen, ann], A_G + [anchor(HX32(0xF00D), IS, bn=200, ts=1700000000)],
            unavailable=[HX32(0xF00D)], bas=COV), None,
       "sec 10.2 check 4 with the parent's sec 8.6: the grant is anchored so check 4 "
       "passes, and the unavailable hash moves the label to UNAVAILABLE, which leaves "
       "check 6 UNKNOWN.", "PARTIAL")
    CK("c4_label_unavailable_unproven", G,
       AINP(IS, [gen, ann], [anchor(eid(G), IS, bn=300, verdict="UNPROVEN")],
            bas=COV), None,
       "sec 10.2 check 4: an UNPROVEN anchor takes no part in the reconciliation, so the "
       "grant is not anchored; the parent's sec 8.7 rule 2 leaves the label COMPLETE for an "
       "UNPROVEN record over bytes in hand, and check 4 reads the undecided anchor of the "
       "grant's own bytes and stays UNKNOWN: PARTIAL, never a conviction on silence.", "PARTIAL")
    CK("c4_baretx_is_not_coverage", G,
       AINP(IS, [gen, ann], [], bas={"chains": [], "bareTx": [{"chainId": 1, "tx": HX32(0xB0B0)}],
                                    "adoptionChains": []}), None,
       "sec 10.2 check 4: bareTx adds anchors and never coverage; a basis whose chains array "
       "is empty is not covering though bareTx names a transaction, so an unanchored grant "
       "is UNKNOWN and the verdict PARTIAL.", "PARTIAL")
    GU = grant_entry(K("iss"), seq=1, prev=eid(gen), grantee=AD("r1"), work=WK,
                     terms=HX32(0x7E), upstream=HX32(0xDE))
    CHN("incomplete_only_hop_green",
        [(GU, AINP(IS, [gen], [anchor(eid(GU), IS)], bas=COV))], None,
        "sec 10.5: a single GREEN hop whose body carries `upstream` has the failing point "
        "`incomplete`, and the chain verdict is decided in order, so FAIL though every hop is "
        "GREEN and every link (none) is true.", "FAIL")
    CK("c4_void_anchor", G,
       AINP(IS, [gen, ann], [anchor(eid(G), IS, bn=400, verdict="VOID")], bas=COV),
       None,
       "sec 10.2 check 4: a VOID anchor is informational and no label turns on it, so the "
       "label is COMPLETE with the grant unanchored under a covering basis: FAIL.", "FAIL")

    # ---- check 6 ----------------------------------------------------------
    genr, Gr, rev = issuer("iss", body, tail="revocation")
    CK("c6_revoked", Gr, AINP(IS, [genr, rev], [anchor(eid(Gr), IS, ts=1700000000)],
                              bas=COV), None,
       "sec 10.2 check 6: a ledger entry of type revocation names this grant and carries no "
       "AUTHORITY_MISMATCH.", "FAIL")
    geno, Go, revo = issuer("iss", body, tail="revocation_other")
    CK("c6_revocation_of_another_grant", Go,
       AINP(IS, [geno, revo], [anchor(eid(Go), IS, ts=1700000000)], bas=COV), None,
       "sec 10.2 check 6: a revocation whose `grant` names something else withdraws nothing "
       "here.", "GREEN")
    LR, LRI = build_seq([("iss", "genesis", 0, GEN),
                         ("iss", "grant", 1, body),
                         ("iss", "succession", 2, SUC)])
    Gsr = LR[1]
    rev_soft = E("iss", "revocation", 6, LRI[2], {"grant": eid(Gsr)})
    CK("c6_revocation_with_soft_authority", Gsr,
       AINP(IS, [LR[0], LR[2], rev_soft], [anchor(eid(Gsr), IS, ts=1700000000)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 10.2 check 6: the revocation was written by a key the succession had already "
       "replaced, and the gap before it makes the finding soft. A revocation carrying an "
       "AUTHORITY_MISMATCH does not fail the check, and the label (GAPS) leaves it "
       "UNKNOWN.", "PARTIAL")
    CK("c6_absent_under_gaps", Gg,
       AINP(IS, [LGAP[0], LGAP[2]], [anchor(eid(Gg), IS, ts=1700000000)], bas=COV),
       None,
       "sec 10.2 check 6: no revocation is in the record and the record is not COMPLETE, so "
       "the check is UNKNOWN: a withheld revocation is exactly what an incomplete record "
       "hides.", "PARTIAL")

    # ---- combinations -----------------------------------------------------
    CK("fail_two_checks", GW,
       AINP(IS, [genw, annw], [anchor(eid(genw), IS, ts=1700000000)], bas=COV), 3000,
       "sec 10.3: two checks fail, and `failed` carries their tokens in check order.",
       "FAIL")
    genrw, Grw, revw = issuer("iss", gbody(AD("r1"), WK, window={"from": 1000, "to": 2000}),
                              tail="revocation")
    CK("fail_expired_and_revoked", Grw,
       AINP(IS, [genrw, revw], [anchor(eid(Grw), IS, ts=1700000000)], bas=COV), 5,
       "sec 10.3: checks 5 and 6 both fail.", "FAIL")
    CK("partial_then_fail", G,
       AINP(IS, [gen, ann], [], bas=basis()), None,
       "sec 10.2: a non-covering basis leaves check 4 UNKNOWN while every other check "
       "passes, which is PARTIAL and never green.", "PARTIAL")
    CK("green_with_succession", G2,
       AINP(IS, [LS2[0], LS2[2], LS2[3]], [anchor(eid(G2), IS, ts=1700000000)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 10.2 check 4: coverage asks for every key of the whole-set lineage, and here "
       "the basis names both.", "GREEN")


# ==========================================================================
# chains/ : sec 10.5
# ==========================================================================


def CHN(slug, hops, now, note, expect=None):
    i = seqno("chains")
    base = "chains/h%03d_%s" % (i, slug)
    rows = []
    for k, (g, ip) in enumerate(hops):
        gp = W("%s.g%d.zk1" % (base, k), g)
        ap = WJ("%s.a%d.json" % (base, k), ip) if ip is not None else None
        rows.append({"grant": gp, "audit": ap})
    hp = WJ(base + ".hops.json", rows)
    args = ["chain-check", hp] + (["--now", str(now)] if now is not None else [])
    pred = chain_check(hops, now)
    extra = {}
    if expect is not None and expect != pred["verdict"]:
        extra["disagreement"] = True
        NOTES.append("chains %s: hand=%s code=%s" % (base, expect, pred["verdict"]))
    CASE("chains", hp, "chain-check", args, note, pred, now=now, **extra)


def hop(keytag, body, anchored=True, tail="annotation", bas=None):
    gen, g, t2 = issuer(keytag, body, tail=tail)
    ad = AD(keytag)
    anchors = [anchor(eid(g), ad, ts=1700000000)] if anchored else []
    return g, AINP(ad, [gen, t2], anchors,
                   bas=bas if bas is not None else cover([ad]))


def build_sign(rng, t):
    """sign/ : sec 3.1 from the signing side, one case per domain of this law.
    The file holds the document's bytes with `sig` removed; the harness signs
    them under the named domain and the prediction is the generator's own
    signature path."""
    rows = sorted_rows([rowobj(AD("r1"), "v1"), rowobj(AD("r2"), "v2")])
    fms = fpm_members(AD("author"), HX32(0x5157), HX32(0x9EA7), rows, "signed on the fpm side")
    fid = hx(sha256(presig_bytes(fms)))
    ams = ack_members(AD("r1"), fid, "v1", "signed on the ack side")
    for slug, ms, dom, keytag in (("fpm", fms, FPM_DOMAIN, "author"), ("ack", ams, ACK_DOMAIN, "r1")):
        i = seqno("sign")
        path = W("sign/s%03d_%s.presig.bin" % (i, slug), presig_bytes(ms))
        priv = K(keytag)
        sig, (r, sv, v) = sign_members(priv, ms, dom)
        presig = sha256(presig_bytes(ms))
        pred = {"digest": hx(pg.eip191_digest(dom, presig)), "presig": hx(presig),
                "sig": sig, "signer": pg.privkey_to_addr_hex(priv)}
        CASE("sign", path, "sign", ["sign", "0x%064x" % priv, path, dom],
             "sec 3.1: the %s message is the domain, a line feed, and hex32 of the presig; "
             "RFC 6979 makes the signature a function of key and digest, low-s required." % dom,
             pred)


def build_chains(rng, t):
    WK = HX32(0xC0DE)
    A, B, C, Dd = AD("iss"), AD("issb"), AD("issc"), AD("r4")
    g0, I0 = hop("iss", gbody(B, WK))
    g1, I1 = hop("issb", gbody(C, WK, upstream=eid(g0)))
    g2, I2 = hop("issc", gbody(Dd, WK, upstream=eid(g1)))

    CHN("empty", [], None,
        "sec 10.5: an empty list yields FAIL with CHAIN_EMPTY, empty `hops` and `links`, "
        "and a failing point of kind `empty` at index 0.", "FAIL")
    CHN("single_green", [(g0, I0)], None,
        "sec 10.5: one hop, GREEN, no links to decide, no failing point.", "GREEN")
    CHN("single_partial", [(g0, None)], None,
        "sec 10.5: one hop whose audit input is null, so four of its checks are UNKNOWN.",
        "PARTIAL")
    g0f, I0f = hop("iss", gbody(B, WK), tail="revocation")
    CHN("single_fail", [(g0f, I0f)], None,
        "sec 10.5: one hop whose grant-check fails; the failing point is kind `hop` at "
        "index 0 and the chain token is null.", "FAIL")
    CHN("two_green", [(g0, I0), (g1, I1)], None,
        "sec 10.5: two hops, both GREEN, and the ledger link holds: the downstream issuer's "
        "ledger is the one the upstream grantee opened.", "GREEN")
    CHN("three_green", [(g0, I0), (g1, I1), (g2, I2)], None,
        "sec 10.5: three hops and two true links.", "GREEN")
    CHN("three_partial_audit_null", [(g0, I0), (g1, None), (g2, I2)], None,
        "sec 10.4: with I_d null the link is unknown and the array carries null; hop 1 is "
        "PARTIAL; no failing point exists, so the chain is PARTIAL.", "PARTIAL")

    up_bad, I_up = hop("issb", gbody(C, WK, upstream=HX32(0xDEAD)))
    CHN("link_false_upstream_value", [(g0, I0), (up_bad, I_up)], None,
        "sec 6.3 through sec 10.4: `upstream` is a string that is not "
        "hex32(entry_id(u)), so the byte link fails and the link is false.", "FAIL")
    wk_bad, I_wk = hop("issb", gbody(C, HX32(0xFEED), upstream=eid(g0)))
    CHN("link_false_work", [(g0, I0), (wk_bad, I_wk)], None,
        "sec 6.3: the works differ, so the byte link fails.", "FAIL")
    g1r, I1r = hop("issc", gbody(Dd, WK, upstream=eid(g0)))
    CHN("link_false_root_not_grantee", [(g0, I0), (g1r, I1r)], None,
        "sec 10.4: the byte link holds and I_d's root is not the upstream grant's "
        "`grantee`, so the ledger link is false. Whether the downstream issuer is the "
        "upstream grantee is a question of ledgers and never of bytes alone.", "FAIL")
    noup, I_noup = hop("issb", gbody(C, WK))
    CHN("link_false_upstream_absent", [(g0, I0), (noup, I_noup)], None,
        "sec 6.3: a downstream grant carrying no `upstream` member at all.", "FAIL")
    nonstr, I_nonstr = hop("issb", gbody(C, WK, upstream=7))
    CHN("link_false_upstream_nonstring", [(g0, I0), (nonstr, I_nonstr)], None,
        "sec 6.3: `upstream` must be a string.", "FAIL")
    CHN("link_unknown_audit_null", [(g0, I0), (g1, None)], None,
        "sec 10.4: I_d is null, so the ledger link is unknown and the links array carries "
        "null. An unknown link is no failing point.", "PARTIAL")
    CHN("link_null_hop_failed_check1",
        [(g0, I0), (pg.set_key(g1, "sig", "0x"), I1)], None,
        "sec 10.5: a link is undecided when either of its two grants failed check 1, so the "
        "link is null and the failing point is the hop.", "FAIL")
    CHN("link_null_first_hop_check1",
        [(b"{}", None), (g1, I1)], None,
        "sec 10.5: hop 0 failed check 1, so link 1 is undecided; g_0's `upstream` is not "
        "read either, since step 1 of the failing-point order asks whether g_0 *passed* "
        "check 1.", "FAIL")

    inc_val, I_inc = hop("iss", gbody(B, WK, upstream=HX32(0xABC)))
    CHN("incomplete_upstream_value", [(inc_val, I_inc), (g1, I1)], None,
        "sec 10.5: g_0 passed check 1 and its body carries `upstream`, so the failing point "
        "is kind `incomplete` at index 0 with token CHAIN_INCOMPLETE, whatever the links "
        "and hops say.", "FAIL")
    inc_null, I_incn = hop("iss", gbody(B, WK, upstream=None))
    CHN("incomplete_upstream_null", [(inc_null, I_incn)], None,
        "sec 10.5 with the parent's sec 6 presence rule: `upstream` present with value null "
        "is a member present.", "FAIL")
    CHN("incomplete_beats_link", [(inc_val, I_inc), (up_bad, I_up)], None,
        "sec 10.5 order: the incomplete test comes before the link test, and this chain "
        "fails both.", "FAIL")

    g2f, I2f = hop("issc", gbody(Dd, WK, upstream=eid(g1)), tail="revocation")
    CHN("fail_hop_after_good_links", [(g0, I0), (g1, I1), (g2f, I2f)], None,
        "sec 10.5: every link is true and the last hop's grant-check fails, so the failing "
        "point is kind `hop` at the smallest failing index.", "FAIL")
    CHN("false_link_and_failing_hop", [(g0, I0), (up_bad, I_up), (g2f, I2f)], None,
        "sec 10.5 order: a false link at k = 1 and a failing hop at k = 2. The link is the "
        "failing point and the token is CHAIN_LINK, which is the whole reason the order is "
        "written down.", "FAIL")
    g0w, I0w = hop("iss", gbody(B, WK, window={"from": 1000, "to": 2000}))
    g1w, I1w = hop("issb", gbody(C, WK, upstream=eid(g0w)))
    CHN("window_now_applies_to_every_hop", [(g0w, I0w), (g1w, I1w)], 1500,
        "sec 10.5: `now` is one input for the whole chain, and every hop's check 5 reads "
        "it.", "GREEN")
    CHN("window_expired_hop", [(g0w, I0w), (g1w, I1w)], 9000,
        "sec 10.5: the first hop's window has run, so hop 0 fails and the chain fails at "
        "kind `hop` index 0.", "FAIL")
    g1u, I1u = hop("issb", gbody(C, WK, upstream=eid(g0)), anchored=False,
                   bas=basis())
    CHN("three_partial_unanchored", [(g0, I0), (g1u, I1u), (g2, I2)], None,
        "sec 10.5: hop 1 is PARTIAL under a basis that covers nothing, every link is true "
        "and no hop fails, so the chain is PARTIAL and carries no token.", "PARTIAL")


# ==========================================================================
# where the law and its harness left two readings
# ==========================================================================

UNDECIDED = []
# Every reading this list once carried (the E_FPM_ROW index, KIT_OK's shape,
# the attribute shape, the depth object, the chain result, the encode
# tokens, the inner token) is settled by HARNESS-KIT.md and the law as of the
# kit review's first round (2026-09-05); the review record carries the list.


# ==========================================================================
# coverage and main
# ==========================================================================


def tally(d, k):
    d[k] = d.get(k, 0) + 1


def coverage():
    docs, pairs_, attr, badge, benc, kits_, krule = {}, {}, {}, {}, {}, {}, {}
    dep, cstate, cverd, creason, chv, cht, chl, chk = {}, {}, {}, {}, {}, {}, {}, {}
    for r in MANIFEST:
        p = r["predicted"]
        c = r["category"]
        if c == "docs":
            tally(docs, "ok" if p.get("ok") else p.get("token"))
        elif c == "pairs" and r["cmd"] == "pair":
            tally(pairs_, p["verdict"])
        elif c == "pairs":
            tally(attr, p["verdict"])
        elif c == "badges" and r["cmd"] == "badge-decode":
            tally(badge, "BADGE_OK" if p.get("ok") else p.get("token"))
        elif c == "badges":
            tally(benc, "payload" if "payload" in p else p.get("token"))
        elif c == "kits":
            tally(kits_, p["verdict"])
            if p["verdict"] == "E_KIT_MANIFEST":
                tally(krule, p["subject"])
        elif c == "depth":
            tally(dep, "invalid" if not p["valid"] else
                  ("found" if p["found"] else "not_found"))
            if p["valid"] and p["found"]:
                tally(dep, "earliest_null" if p["earliest"] is None else "earliest_set")
        elif c == "checks":
            tally(cverd, p["verdict"])
            for ch in p["checks"]:
                tally(cstate, "%d:%s" % (ch["n"], ch["state"]))
                if ch["reason"]:
                    tally(creason, ch["reason"])
        elif c == "chains":
            tally(chv, p["verdict"])
            tally(cht, str(p["token"]))
            tally(chk, p["failing"]["kind"] if p["failing"] else "none")
            for lk in p["links"]:
                tally(chl, str(lk))
    return {"docs_tokens": docs, "pairing_verdicts": pairs_, "attribution": attr,
            "badge_decode": badge, "badge_encode": benc, "kit_verdicts": kits_,
            "kit_manifest_rules": krule, "depth": dep, "check_states": cstate,
            "check_reasons": creason, "check_verdicts": cverd, "chain_verdicts": chv,
            "chain_tokens": cht, "chain_links": chl, "chain_failing_kinds": chk}


CANON_TOKENS = ZK1_TOKENS[:8]


def gaps(cov):
    out = []
    for tok in DOC_TOKENS + CANON_TOKENS:
        if tok not in cov["docs_tokens"]:
            out.append("docs:" + tok)
    if "ok" not in cov["docs_tokens"]:
        out.append("docs:ok")
    for v in PAIR_VERDICTS:
        if v not in cov["pairing_verdicts"]:
            out.append("pair:" + v)
    for v in ("ATTRIBUTED", "NOT_ATTRIBUTED"):
        if v not in cov["attribution"]:
            out.append("attribute:" + v)
    for tok in BADGE_TOKENS + ["BADGE_OK"]:
        if tok not in cov["badge_decode"]:
            out.append("badge:" + tok)
    for v in KIT_VERDICTS:
        if v not in cov["kit_verdicts"]:
            out.append("kit:" + v)
    for r in KIT_RULES:
        if r not in cov["kit_manifest_rules"]:
            out.append("kit-rule:" + r)
    for n in range(1, 7):
        # check 1 has no UNKNOWN state: sec 10.2's table gives it FAIL or the residual PASS
        for s in (("PASS", "FAIL") if n == 1 else ("PASS", "FAIL", "UNKNOWN")):
            if "%d:%s" % (n, s) not in cov["check_states"]:
                out.append("check%d:%s" % (n, s))
    for v in ("GREEN", "PARTIAL", "FAIL"):
        if v not in cov["check_verdicts"]:
            out.append("grant-check:" + v)
        if v not in cov["chain_verdicts"]:
            out.append("chain-check:" + v)
    for tk in ("CHAIN_EMPTY", "CHAIN_INCOMPLETE", "CHAIN_LINK", "None"):
        if tk not in cov["chain_tokens"]:
            out.append("chain-token:" + tk)
    for lk in ("True", "False", "None"):
        if lk not in cov["chain_links"]:
            out.append("chain-link:" + lk)
    for kd in ("empty", "incomplete", "link", "hop", "none"):
        if kd not in cov["chain_failing_kinds"]:
            out.append("chain-failing:" + kd)
    if "NOT_A_GRANT" not in cov["check_reasons"]:
        out.append("check1-reason:NOT_A_GRANT")
    for tok in ZK1_TOKENS:
        if tok not in cov["check_reasons"]:
            out.append("check1-reason:" + tok)
    return out


DIRS = ("docs", "pairs", "badges", "kits", "depth", "checks", "chains")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    seed = args.seed
    rng = random.Random(seed)

    for tag in ("author", "r1", "r2", "r3", "r4", "iss", "iss2", "issb", "issc",
                "stranger", "k2"):
        KEYS[tag] = testkey(tag, seed)

    for d in DIRS:
        p = os.path.join(HERE, d)
        if os.path.isdir(p):
            for root, dirs, files in os.walk(p):
                for nm in dirs + files:
                    try:
                        os.chmod(os.path.join(root, nm), 0o755)
                    except OSError:
                        pass
                try:
                    os.chmod(root, 0o755)
                except OSError:
                    pass
            shutil.rmtree(p)
        os.makedirs(p)
    del MANIFEST[:]
    del NOTES[:]
    COUNTS.clear()
    _n.clear()

    t = build_docs(rng)
    build_pairs(rng, t)
    tb = build_badges(rng, t)
    build_kits(rng, tb)
    build_depth(rng, tb)
    build_checks(rng, tb)
    build_chains(rng, tb)
    build_sign(rng, tb)

    cov = coverage()
    man = {
        "corpus": "zikaron.kit/1 convergence corpus",
        "law": "docs/zikaron-kit-v1.md",
        "parent_law": "docs/zikaron-v1.md",
        "parent_core": "0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc",
        "harness": "zikaron-conformance/HARNESS-KIT.md",
        "seed": seed,
        "generator": "gen.py",
        "run_from": "zikaron-conformance/kit-corpus (every path in `args` is relative to "
                    "this directory)",
        "keys": {tag: {"privkey": "0x%064x" % KEYS[tag],
                       "address": pg.privkey_to_addr_hex(KEYS[tag])}
                 for tag in sorted(KEYS)},
        "counts": COUNTS,
        "coverage": cov,
        "unwitnessed": gaps(cov),
        "undecided": UNDECIDED,
        "prediction_disclaimer":
            "Every `predicted` field is the corpus author's own reading of "
            "docs/zikaron-kit-v1.md, produced by gen.py and by nothing else. It is a third "
            "opinion beside the two implementations under test and is never authoritative. "
            "Where the two implementations agree with each other and disagree with a "
            "prediction, the prediction is wrong. Where they disagree with each other, the "
            "law decides. `hand_expect` records the author's prose reasoning about a case; "
            "a case where the code reached something else carries `disagreement: true`.",
        "cases": MANIFEST,
    }
    with open(os.path.join(HERE, "manifest.json"), "w") as f:
        json.dump(man, f, indent=1, sort_keys=True)
    with open(os.path.join(HERE, "README.md"), "w") as f:
        f.write(README % (seed, COUNTS.get("docs", 0), COUNTS.get("pairs", 0),
                          COUNTS.get("badges", 0), COUNTS.get("kits", 0),
                          COUNTS.get("depth", 0), COUNTS.get("checks", 0),
                          COUNTS.get("sign", 0), COUNTS.get("chains", 0)))

    print("seed %d" % seed)
    for k in DIRS:
        print("  %-8s %4d cases" % (k, COUNTS.get(k, 0)))
    print("  total    %4d cases" % sum(COUNTS.values()))
    print("  docs tokens: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["docs_tokens"].items())))
    print("  badge decode: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["badge_decode"].items())))
    print("  kit verdicts: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["kit_verdicts"].items())))
    print("  grant-check verdicts: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["check_verdicts"].items())))
    print("  chain verdicts: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["chain_verdicts"].items())))
    g = gaps(cov)
    print("  closed boundaries unwitnessed: %s" % (", ".join(g) if g else "none"))
    if NOTES:
        print("  build notes (%d):" % len(NOTES))
        for w in NOTES:
            print("      " + w)
    else:
        print("  build notes: none")


README = """# zikaron.kit/1 convergence corpus

Generated by `gen.py` from `docs/zikaron-kit-v1.md` (the law),
`docs/zikaron-v1.md` (the frozen parent) and
`zikaron-conformance/HARNESS-KIT.md` (the command-line contract) alone. No
implementation of the kit law was read while building it. The `zikaron/1` side
of the work (canonical bytes, the entry predicate, secp256k1 with RFC 6979 and
low-s, the audit walk) is imported from `../corpus/gen.py`, the parent's own
corpus generator, which is a tool here and never evidence for this law.

## Regenerating

    cd zikaron-conformance/kit-corpus
    python3 gen.py --seed %d

The run is deterministic: one seed rebuilds a byte-identical tree. Every
directory below is deleted and repopulated on each run, and `manifest.json`
and this file are rewritten. Requires Python 3 and `pycryptodome` (for
`Crypto.Hash.keccak`), the parent generator's only dependency.

Two things in `kits/` do not survive a copy that is not a regeneration: two
empty directories (`kits/*_ok_empty_directory/scratch` and
`kits/*_manifest_absent_empty_dir/`), which git does not store, and five
symbolic links. Regenerate rather than copy.

## Running it

Every case in `manifest.json` carries an `args` array: the argument vector to
hand `zkk`, with every path relative to this directory. So a whole run is

    cd zikaron-conformance/kit-corpus
    python3 - <<'EOF' | tee ../out-a.txt
    import json, subprocess
    for c in json.load(open("manifest.json"))["cases"]:
        print(c["path"], c["cmd"],
              subprocess.run(["zkk"] + c["args"], capture_output=True).stdout.decode())
    EOF

and the two implementations are compared by diffing their two streams byte for
byte. Every command writes one canonical JSON value and exits 0 whenever it
produced an answer; a rejection is an answer.

## Layout

- `docs/` (%d cases) - byte strings for `zkk fpm-check` and `zkk ack-check`.
  Raw bytes: many are not valid UTF-8 and most are not valid JSON, which is
  the point. A few files are registered twice, once under each command.
- `pairs/` (%d cases) - `pNNN_*.fpm.bin` with `pNNN_*.ack.bin` for `zkk pair`,
  and a third `pNNN_*.x.bin` where the case also runs `zkk attribute`.
- `badges/` (%d cases) - `bNNN_*.payload` holds one payload for
  `zkk badge-decode`; `eNNN_*/NN.zk1` are the ordered entry files of one
  `zkk badge-encode` invocation.
- `kits/` (%d cases) - each `kNNN_*/` is one directory for `zkk kit-verify`.
- `depth/` (%d cases) - `zNNN_*.audit.json` is one audit input in HARNESS.md's
  format and `zNNN_*.work` repeats the work digest the `args` array passes.
- `checks/` (%d cases) - `cNNN_*.grant.zk1` with an optional
  `cNNN_*.audit.json`; the `args` array carries `--audit` and `--now` where
  the case uses them.
- `sign/` (%d cases) - `sNNN_*.presig.bin` holds a document's bytes without
  `sig` for `zkk sign` under the domain the manifest names.
- `chains/` (%d cases) - `hNNN_*.hops.json` names its hops' grant and audit
  files, which sit beside it.
- `manifest.json` - every case with its category, its command, its argument
  vector, a note naming the section it witnesses, and the corpus author's own
  predicted answer.

## Reading manifest.json

`predicted` is a third opinion and never authoritative. Beside the cases the
manifest carries:

- `coverage`: what the corpus witnessed, tallied by closed vocabulary.
- `unwitnessed`: the boundary list sec 13.1 asks for, as a precondition on the
  corpus. An empty array is the precondition met; it is never a property
  claimed of the corpus afterwards.
- `undecided`: the places where the law and its harness left two readings,
  each with both readings and with what this corpus did about it. These are
  the cases to read first when two implementations disagree.
- `keys`: the private keys every document and entry was signed with, so either
  implementation can re-sign and compare.
"""


if __name__ == "__main__":
    main()
