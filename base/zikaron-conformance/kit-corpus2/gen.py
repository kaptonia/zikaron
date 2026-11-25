#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
gen.py - corpus generator for the `zikaron.kit/2` convergence run.

Written from docs/zikaron-kit-v2.md (the kit law), docs/zikaron-v2.md and
docs/zikaron-v1.md (the parents) and zikaron-conformance/HARNESS-KIT2.md (the
command-line contract) alone.  No implementation of the kit law was consulted.
The parents' side of the work (canonicalizer, parser, secp256k1 with RFC 6979
and low-s, EIP-191 digest, envelope and body tests) is reused from the parent
corpus generators `../corpus2/gen.py` and `../corpus/gen.py`, which are tools
here and never evidence for this law; the audit of a zikaron/2 ledger, its
continued segment included, and everything about `zikaron.kit/2` below are
the corpus author's own reading of the texts and are recorded in
manifest.json as a *prediction*, a third opinion beside the two
implementations under test.

Usage:  python3 gen.py --seed 1
Populates ./badges ./kits ./packages ./checks ./chains and writes
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
PARENT_GEN = os.path.normpath(os.path.join(HERE, os.pardir, "corpus2", "gen.py"))
PARENT1_GEN = os.path.normpath(os.path.join(HERE, os.pardir, "corpus", "gen.py"))

_spec = importlib.util.spec_from_file_location("zk2gen", PARENT_GEN)
pg = importlib.util.module_from_spec(_spec)
sys.modules["zk2gen"] = pg
_spec.loader.exec_module(pg)
pg1 = pg.v1gen

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

BADGE_PREFIX = b"zikaron-grant:"
BADGE_CAP = 2953
KIT_SPEC = "zikaron.kit/2"
KIT_MAGIC = b"zikaron-kit-file/1\n"

KIT_REQ = {"contents", "entries", "files", "note_md", "root", "spec"}
KIT_OPT = {"anchoredOn", "anchors"}

ZK1_TOKENS = ["E_UTF8", "E_JSON", "E_NUMBER", "E_DEPTH", "E_DUP_KEY", "E_KEY_CHARSET",
              "E_VALUE_CHARSET", "E_NOT_CANONICAL", "E_ENVELOPE", "E_ENVELOPE_MISSING",
              "E_ENVELOPE_CLOSED", "E_SPEC", "E_ENTRYTYPE", "E_AUTHOR", "E_SEQ", "E_PREV",
              "E_PREV_SEQ", "E_BODY", "E_SIG_FORM", "E_GENESIS_PLACE", "E_BODY_FIELD",
              "E_SIG_V", "E_SIG_RANGE", "E_SIG_HIGH_S", "E_SIG_RECOVER", "E_SIG_SIGNER"]

BADGE_TOKENS = ["E_BADGE_PREFIX", "E_BADGE_CAP", "E_BADGE_B64", "E_BADGE_ENTRY",
                "E_BADGE_TYPE", "E_BADGE_INCOMPLETE", "E_BADGE_LINK"]

KIT_VERDICTS = ["KIT_OK", "E_KIT_UNREADABLE", "E_KIT_MANIFEST_ABSENT", "E_KIT_MANIFEST",
                "E_KIT_ENTRY_BYTES", "E_KIT_FILE", "E_KIT_CONTENT", "E_KIT_EXTRA"]

KIT_RULES = ["canonical", "members", "spec", "root", "entries", "files", "contents",
             "anchoredOn", "anchors", "note_md"]

GP_VERDICTS = ["GP_OK", "E_GP_NO_CODE", "E_GP_ROOT", "E_GP_UNCARRIED", "E_GP_CONTENT",
               "E_GP_TERMS"]

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
# sec 1: accept(b), dispatched by spec
# ==========================================================================



def accept_check(b):
    """sec 1: zikaron/1 sec 4.3 where b passes zikaron/2 sec 4.3 steps 1 to 3
    and its spec is zikaron/1, zikaron/2 sec 4.3 otherwise.  Returns
    (True, entry_id) or (False, token)."""
    v, tok = pg.accept_canonical(b)
    if tok is None and isinstance(v, JObj) and set(v.keys()) == set(pg.SEVEN) \
            and v.get("spec") == "zikaron/1":
        return pg1.zk_check(b)
    return pg.zk_check(b)


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
        ok, val = accept_check(raw)
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


def badge_entries(p):
    """The decoded grant byte strings of a payload that decodes, else None."""
    d = badge_decode(p)
    if not d.get("ok"):
        return None
    segs = p[len(BADGE_PREFIX):].split(b".")
    return [b64u_seg(seg)[0] for seg in segs]


def badge_encode(entry_byte_strings):
    """sec 6.1 over one or more entries, in order."""
    for k, b in enumerate(entry_byte_strings):
        ok, val = accept_check(b)
        if not ok:
            return {"ok": False, "token": "E_BADGE_ENTRY", "index": k}
        if parse(b).get("entryType") != "grant":
            return {"ok": False, "token": "E_BADGE_TYPE", "index": k}
    payload = BADGE_PREFIX + ".".join(b64u(b) for b in entry_byte_strings).encode("ascii")
    if len(payload) > BADGE_CAP:
        return {"ok": False, "token": "E_BADGE_CAP"}
    return {"payload": payload.decode("ascii")}


# ==========================================================================
# sec 4 and sec 5: record kits and grant packages
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
    """sec 4.1.  Returns (enumeration, None) or (None, failing relative path)."""
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
    """sec 4.3.  Returns (parsed, None, None) or (None, rule, inner token)."""
    v, tok = pg.accept_canonical(b)
    if tok:
        return None, "canonical", tok
    if not isinstance(v, JObj):
        return None, "members", None
    ks = set(v.keys())
    if not KIT_REQ <= ks or not ks <= (KIT_REQ | KIT_OPT):
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
        if not isinstance(row, JObj):
            return None, "files", None
        rk = set(row.keys())
        if rk != {"path", "sha256", "size"} and rk != {"path", "sha256", "size", "name_md"}:
            return None, "files", None
        if not kit_path_ok(row.get("path")):
            return None, "files", None
        if not is_hex32(row.get("sha256")):
            return None, "files", None
        if not is_int(row.get("size")):
            return None, "files", None
        if row.has("name_md") and not is_str(row.get("name_md")):
            return None, "files", None
    fpaths = [row.get("path") for row in files]
    if len(set(fpaths)) != len(fpaths):
        return None, "files", None
    for a in fpaths:
        for b2 in fpaths:
            if b2.startswith(a + "/"):
                return None, "files", None
    if [bwise(x) for x in fpaths] != sorted(bwise(x) for x in fpaths):
        return None, "files", None
    fmap = {row.get("path"): row for row in files}
    contents = v.get("contents")
    if not isinstance(contents, list):
        return None, "contents", None
    for row in contents:
        if not isinstance(row, JObj):
            return None, "contents", None
        rk = set(row.keys())
        if rk != {"content", "path"} and rk != {"content", "path", "name_md"}:
            return None, "contents", None
        if not is_hex32(row.get("content")):
            return None, "contents", None
        pth = row.get("path")
        if not kit_path_ok(pth):
            return None, "contents", None
        if row.has("name_md") and not is_str(row.get("name_md")):
            return None, "contents", None
        if pth in fmap:
            if fmap[pth].get("sha256") != row.get("content"):
                return None, "contents", None
        elif not any(q.startswith(pth + "/") for q in fpaths):
            return None, "contents", None
    dir_origs = [row.get("path") for row in contents if row.get("path") not in fmap]
    for p in dir_origs:
        if any(q.startswith(p + "/") for q in dir_origs):
            return None, "contents", None
    for p in dir_origs:
        names = [(fmap[q].get("name_md") if fmap[q].has("name_md") else q[len(p) + 1:])
                 for q in fpaths if q.startswith(p + "/")]
        if len(set(names)) != len(names):
            return None, "contents", None
        for nm in names:
            if any(sg in ("", ".", "..") for sg in nm.split("/")):
                return None, "contents", None
        if any(o.startswith(nm + "/") for nm in names for o in names):
            return None, "contents", None
    crows = [(row.get("content"), row.get("path")) for row in contents]
    if len(set(crows)) != len(crows):
        return None, "contents", None
    if [(bwise(a), bwise(b2)) for a, b2 in crows] != sorted(
            (bwise(a), bwise(b2)) for a, b2 in crows):
        return None, "contents", None
    if v.has("anchoredOn"):
        ao = v.get("anchoredOn")
        if not isinstance(ao, JObj) or set(ao.keys()) != {"chainId", "registry", "fromBlock"}:
            return None, "anchoredOn", None
        if not (is_int(ao.get("chainId")) and is_hex20(ao.get("registry")) and is_int(ao.get("fromBlock"))):
            return None, "anchoredOn", None
    if v.has("anchors"):
        an = v.get("anchors")
        if not isinstance(an, list):
            return None, "anchors", None
        rows = []
        for row in an:
            if not isinstance(row, JObj) or set(row.keys()) != {"entry_id", "chainId", "tx"}:
                return None, "anchors", None
            if not is_hex32(row.get("entry_id")) or row.get("entry_id") not in ents:
                return None, "anchors", None
            if not is_int(row.get("chainId")) or not is_hex32(row.get("tx")):
                return None, "anchors", None
            rows.append((bwise(row.get("entry_id")), row.get("chainId"), bwise(row.get("tx"))))
        if len(set(rows)) != len(rows) or rows != sorted(rows):
            return None, "anchors", None
    if not is_str(v.get("note_md")):
        return None, "note_md", None
    return v, None, None


def dir_manifest_digest(v, dirpath):
    """sec 4.5: the sha256 of the canonical directory manifest recomputed from
    the files under dirpath/ with the names name_md restores."""
    pre = dirpath + "/"
    rows = []
    for row in v.get("files"):
        pth = row.get("path")
        if pth.startswith(pre):
            nm = row.get("name_md") if row.has("name_md") else pth[len(pre):]
            rows.append((nm.encode("utf-8"), nm, row.get("sha256")))
    rows.sort(key=lambda r: r[0])
    doc = obj([("files", [{"path": nm, "sha256": h} for _b, nm, h in rows])])
    return hx(sha256(cbytes(doc)))


def _count_ok(b):
    if not b or any(c < 0x30 or c > 0x39 for c in b):
        return False
    return not (len(b) > 1 and b[0] == 0x30)


def parse_onefile(data):
    """sec 4.1a.  Returns (enumeration, None) or (None, subject)."""
    if not data.startswith(KIT_MAGIC):
        return None, "."
    i = len(KIT_MAGIC)
    n = len(data)
    enum = {}
    first = True
    prev = None
    while i < n:
        j = data.find(b"\n", i)
        if j < 0:
            return None, "."
        path = data[i:j]
        try:
            pstr = path.decode("utf-8")
        except UnicodeDecodeError:
            return None, "."
        if not kit_path_ok(pstr):
            return None, "."
        k = data.find(b"\n", j + 1)
        if k < 0:
            return None, pstr
        cnt = data[j + 1:k]
        if not _count_ok(cnt):
            return None, pstr
        size = int(cnt)
        start = k + 1
        end = start + size
        if end >= n or data[end:end + 1] != b"\n":
            return None, pstr
        if first:
            if pstr != "manifest.json":
                return None, pstr
            first = False
        else:
            if pstr in enum or (prev is not None and not path > prev):
                return None, pstr
            prev = path
        enum[pstr] = data[start:end]
        i = end + 1
    if first:
        return None, "."
    return enum, None


def enumeration_of(target):
    """A directory walked (sec 4.1) or a regular file read (sec 4.1a)."""
    if os.path.isdir(target):
        return walk(target)
    try:
        with open(target, "rb") as f:
            data = f.read()
    except OSError:
        return None, "."
    return parse_onefile(data)


def kit_verify_enum(enum):
    """sec 4.4 over an enumeration.  Returns (result, parsed manifest or None)."""
    if "manifest.json" not in enum:
        return {"verdict": "E_KIT_MANIFEST_ABSENT"}, None
    v, rule, inner = kit_manifest_rule(enum["manifest.json"])
    if rule:
        return {"verdict": "E_KIT_MANIFEST", "subject": rule}, None
    named = {"manifest.json"}
    for e in v.get("entries"):
        pth = "entries/" + e[2:] + ".zk1"
        named.add(pth)
        if pth not in enum or hx(sha256(enum[pth])) != e:
            return {"verdict": "E_KIT_ENTRY_BYTES", "subject": e}, v
    for row in v.get("files"):
        pth = "files/" + row.get("path")
        named.add(pth)
        if pth not in enum:
            return {"verdict": "E_KIT_FILE", "subject": row.get("path")}, v
        if hx(sha256(enum[pth])) != row.get("sha256") or len(enum[pth]) != row.get("size"):
            return {"verdict": "E_KIT_FILE", "subject": row.get("path")}, v
    fpaths = set(row.get("path") for row in v.get("files"))
    for row in v.get("contents"):
        pth = row.get("path")
        if pth in fpaths:
            continue
        if dir_manifest_digest(v, pth) != row.get("content"):
            return {"verdict": "E_KIT_CONTENT", "subject": pth}, v
    extras = sorted((pth for pth in enum if pth not in named), key=bwise)
    if extras:
        return {"verdict": "E_KIT_EXTRA", "subject": extras[0]}, v
    invalid = []
    for e in v.get("entries"):
        ok, val = accept_check(enum["entries/" + e[2:] + ".zk1"])
        if not ok:
            invalid.append({"entry_id": e, "token": val})
    return {"verdict": "KIT_OK", "kit_id": hx(sha256(enum["manifest.json"])),
            "counts": {"entries": len(v.get("entries")), "files": len(v.get("files"))},
            "invalid_entries": invalid}, v


def kit_verify(target):
    """sec 4.1 or 4.1a then sec 4.4."""
    enum, bad = enumeration_of(target)
    if bad is not None:
        return {"verdict": "E_KIT_UNREADABLE", "subject": bad}
    r, _v = kit_verify_enum(enum)
    return r


GP_INDEXED = ("E_BADGE_B64", "E_BADGE_ENTRY", "E_BADGE_TYPE", "E_BADGE_LINK")


def grant_package(target):
    """sec 5.2 and sec 5.3."""
    enum, bad = enumeration_of(target)
    if bad is not None:
        return {"verdict": "E_KIT_UNREADABLE", "subject": bad}
    r, v = kit_verify_enum(enum)
    if r["verdict"] != "KIT_OK":
        return {"verdict": r["verdict"], "subject": r.get("subject")}
    code = enum.get("files/zikaron-grant.txt")
    if code is None:
        return {"verdict": "E_GP_NO_CODE", "subject": None}
    d = badge_decode(code)
    if not d.get("ok"):
        return {"verdict": d["token"], "subject": d.get("index") if d["token"] in GP_INDEXED else None}
    hops = badge_entries(code)
    ids = [hx(sha256(h)) for h in hops]
    root = v.get("root")
    if root is None:
        b0 = body_of(hops[0])
        led = b0.get("ledger") if b0.has("ledger") else None
        if not is_hex20(led):
            return {"verdict": "E_GP_ROOT", "subject": None}
        root = led
    for h in ids:
        if "entries/" + h[2:] + ".zk1" not in enum:
            return {"verdict": "E_GP_UNCARRIED", "subject": h}
    work = body_of(hops[-1]).get("work")
    for row in v.get("contents"):
        if row.get("content") != work:
            return {"verdict": "E_GP_CONTENT", "subject": row.get("path")}
    terms = set(body_of(h).get("terms") for h in hops)
    tpaths = sorted((pth for pth in enum if pth.startswith("files/terms/")), key=bwise)
    for pth in tpaths:
        if hx(sha256(enum[pth])) not in terms:
            return {"verdict": "E_GP_TERMS", "subject": pth[len("files/terms/"):]}
    per_hop = [[pth[len("files/terms/"):] for pth in tpaths
                if hx(sha256(enum[pth])) == body_of(h).get("terms")] for h in hops]
    return {"verdict": "GP_OK", "kit_id": hx(sha256(enum["manifest.json"])), "root": root,
            "hops": ids,
            "contents": [{"content": row.get("content"), "path": row.get("path")} for row in v.get("contents")],
            "terms": per_hop}


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
        all(c in "0123456789abcdefABCDEF" for c in v[2:])


def _anchor_rows_ok(rows):
    if not isinstance(rows, list):
        return False
    seen = {}
    for a in rows:
        if not isinstance(a, dict):
            return False
        if not {"chainId", "blockNumber", "blockTimestamp", "tx", "sender", "hash", "verdict"} <= set(a.keys()):
            return False
        if not (is_int(a["chainId"]) and is_int(a["blockNumber"]) and is_int(a["blockTimestamp"])):
            return False
        if not (is_hex32(a["tx"]) and is_hex20(a["sender"]) and is_hex32(a["hash"])):
            return False
        if a["verdict"] not in ("counted", "UNPROVEN", "VOID"):
            return False
        k = (a["chainId"], a["blockNumber"], a["tx"], a["hash"])
        a7 = {kk: a[kk] for kk in ("chainId", "blockNumber", "blockTimestamp", "tx", "sender", "hash", "verdict")}
        if k in seen and seen[k] != a7:
            return False
        seen[k] = a7
    return True


def input_valid(inp):
    """zikaron/2 sec 9.4: the well-formedness a verifier decides from the three
    inputs, as HARNESS2.md carries them (members beyond the four are ignored)."""
    if not isinstance(inp, dict):
        return False
    if not {"root", "pile", "anchors", "basis"} <= set(inp.keys()):
        return False
    if not is_hex20(inp["root"]):
        return False
    if not isinstance(inp["pile"], list) or not all(_hexstr(x) for x in inp["pile"]):
        return False
    if not pg._basis_ok(inp["basis"]):
        return False
    if not _anchor_rows_ok(inp["anchors"]):
        return False
    for a in inp["anchors"]:
        if not any(c["chainId"] == a["chainId"] and c["fromBlock"] <= a["blockNumber"] <= c["toBlock"]
                   and a["sender"] in c["senders"] for c in inp["basis"]["chains"]):
            return False
    return True


def _dedupe(rows):
    out, seen = [], set()
    for a in rows:
        k = (a["chainId"], a["blockNumber"], a["tx"], a["hash"])
        if k in seen:
            continue
        seen.add(k)
        out.append(a)
    return out


class Audit(object):
    __slots__ = ("valid", "label", "ledger", "by_id", "findings", "counted", "unproven",
                 "lineage", "basis", "root", "retired", "continued", "cont_ids", "entries", "seed")

    def __init__(self):
        self.valid = False
        self.label = None
        self.ledger = []
        self.by_id = {}
        self.findings = []
        self.counted = []
        self.unproven = []
        self.lineage = set()
        self.basis = None
        self.root = None
        self.retired = set()
        self.continued = False
        self.cont_ids = set()
        self.entries = []
        self.seed = None


def _walk_findings(seed, ledger, with_migration):
    """The walk of sec 8.2 (zikaron/1 without, zikaron/2 with the migration
    closure) and sec 8.4's pairs.  Returns (findings, any_gap)."""
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
    any_gap = False
    for sq in sorted(by_seq):
        grp = by_seq[sq]
        for e in grp:
            if e.seq != expected and e.seq != prev_seq:
                findings.append({"name": "SEQ_GAP", "entry_id": e.id, "hard": False})
                any_gap = True
                expected = e.seq if e.seq == MAXINT else e.seq + 1
                certain = False
            elif e.seq == expected:
                expected = e.seq if e.seq == MAXINT else e.seq + 1
            if e.seq >= 1:
                pool = by_seq.get(e.seq - 1)
                if pool and not any(x.id == e.prev for x in pool):
                    findings.append({"name": "PREV_MISMATCH", "entry_id": e.id, "hard": True})
            if e.seq == 0:
                if e.author != seed:
                    findings.append({"name": "ROOT_MISMATCH", "entry_id": e.id, "hard": True})
            else:
                if closed or e.author != auth:
                    findings.append({"name": "AUTHORITY_MISMATCH", "entry_id": e.id, "hard": certain})
            prev_seq = e.seq
        if not closed:
            if with_migration and any(e.etype == "migration" and e.author == auth for e in grp):
                closed = True
            else:
                succ = [e for e in grp if e.etype == "succession" and e.author == auth]
                if succ:
                    auth = min(succ, key=lambda e: bytes.fromhex(e.id[2:])).body.get("to")
    for i in range(len(order)):
        for j in range(i + 1, len(order)):
            x, y = order[i], order[j]
            if x.id == y.id:
                continue
            if x.seq == y.seq or (x.prev is not None and x.prev == y.prev):
                findings.append({"name": "EQUIVOCATION", "entry_id": x.id, "b": y.id, "hard": True})
    return findings, any_gap


def audit_v1(root, raw, anchors):
    """The zikaron/1 audit over a segment's root, byte strings and records (the
    unavailable and evidence sets empty), read structurally."""
    a = Audit()
    a.valid = True
    a.root = root
    pile_ids = set(hx(sha256(x)) for x in raw)
    ents = {}
    for b in raw:
        ok, val = pg1.zk_check(b)
        if ok and b not in ents:
            ents[b] = Ent(b, parse(b))
    entries = list(ents.values())
    lineage = {root}
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
    kept = [x for x in anchors if x["sender"] in lineage]
    a.counted = [x for x in kept if x["verdict"] == "counted"]
    a.unproven = [x for x in kept if x["verdict"] == "UNPROVEN"]
    counted_hashes = set(x["hash"] for x in a.counted)
    findings, any_gap = _walk_findings(root, ledger, False)
    a.findings = findings
    have = set(e.id for e in ledger)
    missing = counted_hashes - have
    if any(f["hard"] for f in findings):
        a.label = "BROKEN_CHAIN"
    elif any(x["hash"] not in pile_ids and x["hash"] not in counted_hashes for x in a.unproven):
        a.label = "UNAVAILABLE"
    elif any_gap or missing:
        a.label = "GAPS"
    else:
        a.label = "COMPLETE"
    return a


def _line(e, by_id):
    out, seen, cur = [], set(), e
    while cur is not None and cur.id not in seen:
        seen.add(cur.id)
        out.append(cur)
        cur = by_id.get(cur.prev) if cur.prev is not None else None
    return out


def continuation(root, raw, anchors):
    """zikaron/2 sec 12.6: (genesis, succession, cut, old byte strings, old audit)
    or None."""
    v2 = {}
    for b in raw:
        ok, _ = pg.zk_check(b)
        if ok and b not in v2:
            v2[b] = Ent(b, parse(b))
    v1 = {}
    for b in raw:
        ok, _ = pg1.zk_check(b)
        if ok and b not in v1:
            v1[b] = Ent(b, parse(b))
    v1_by_id = {e.id: e for e in v1.values()}
    by_hash = {}
    for x in anchors:
        by_hash.setdefault(x["hash"], []).append(x)
    best = None
    for g in v2.values():
        if g.seq != 0 or g.etype != "genesis" or not g.body.has("continues"):
            continue
        c = g.body.get("continues")
        if not isinstance(c, JObj) or c.get("root") != root:
            continue
        sv = v1_by_id.get(c.get("entry_id"))
        if sv is None or sv.etype != "succession" or sv.author != g.author:
            continue
        if sv.body.get("to") != g.author or sv.body.get("kind") != "migration":
            continue
        recs = by_hash.get(sv.id)
        if not recs:
            continue
        cut_ = min(x["blockTimestamp"] for x in recs)
        if any(e.etype == "succession" and e.author == sv.author and e.body.get("to") != sv.author
               and any(x["blockTimestamp"] < cut_ for x in by_hash.get(e.id, []))
               for e in v1.values()):
            continue
        recs_le = [x for x in anchors if x["blockTimestamp"] <= cut_]
        old = set()
        for e in _line(sv, v1_by_id):
            old.add(e.b)
        hashes = set(x["hash"] for x in recs_le)
        for b in set(raw):
            if hx(sha256(b)) in hashes:
                old.add(b)
                if b in v1:
                    for e in _line(v1[b], v1_by_id):
                        old.add(e.b)
        # zikaron/2 sec 12.6: a record at or before the cut, or anchoring an
        # old ledger entry (zikaron/1 sec 8.1 over the old byte strings)
        old_ids = set(audit_v1(root, sorted(old), recs_le).by_id.keys())
        recs_old = [x for x in anchors if x["blockTimestamp"] <= cut_ or x["hash"] in old_ids]
        a1 = audit_v1(root, sorted(old), recs_old)
        if sv.id not in a1.by_id:
            continue
        key = (cut_, bytes.fromhex(g.id[2:]))
        if best is None or key < best[0]:
            best = (key, (g, sv, cut_, old, a1, recs_old))
    return None if best is None else best[1]


LABEL_ORDER = ("BROKEN_CHAIN", "UNPROVEN", "GAPS", "COMPLETE")


def audit2(inp):
    """A structural reading of zikaron/2 sec 8 over one audit input, the
    continued segment included (sec 12.6).  The corpus author's own reading,
    reusing the parents' entry predicates."""
    a = Audit()
    a.valid = input_valid(inp)
    if not a.valid:
        return a
    a.basis = inp["basis"]
    a.root = inp["root"]
    root = inp["root"]
    raw = [bytes.fromhex(x[2:]) for x in inp["pile"]]
    anchors = _dedupe(inp["anchors"])
    cont = continuation(root, raw, anchors)
    if cont is None:
        seed, seg_raw, seg_anchors, a1 = root, raw, anchors, None
    else:
        g, sv, cut_, old, a1, recs_old = cont
        old_v1 = set(b for b in old if pg1.zk_check(b)[0])
        seed = g.author
        seg_raw = [b for b in raw if b not in old_v1]
        old_keys = set((x["chainId"], x["blockNumber"], x["tx"], x["hash"]) for x in recs_old)
        seg_anchors = [x for x in anchors if (x["chainId"], x["blockNumber"], x["tx"], x["hash"]) not in old_keys]
    pile_ids = set(hx(sha256(x)) for x in raw)      # sec 8.6 item 15 rule 2: the whole pile
    ents = {}
    for b in seg_raw:
        ok, val = pg.zk_check(b)
        if ok and b not in ents:
            ents[b] = Ent(b, parse(b))
    entries = list(ents.values())
    by_id = {e.id: e for e in entries}
    ret = {}
    rseq = {}
    for x in seg_anchors:
        e = by_id.get(x["hash"])
        if e is not None and e.etype in ("succession", "migration"):
            if e.author not in ret or x["blockTimestamp"] < ret[e.author]:
                ret[e.author] = x["blockTimestamp"]
                rseq[e.author] = e.seq
            elif x["blockTimestamp"] == ret[e.author] and e.seq < rseq[e.author]:
                rseq[e.author] = e.seq

    def retired_at_key(key, x):
        if key not in ret or x["blockTimestamp"] < ret[key]:
            return False
        if x["blockTimestamp"] > ret[key]:
            return True
        e = by_id.get(x["hash"])
        return e is not None and e.author == key and e.seq > rseq[key]
    timely = set(e.id for e in entries if e.author not in ret)
    for x in seg_anchors:
        f = by_id.get(x["hash"])
        if f is None:
            continue
        for e in _line(f, by_id):
            if e.id in timely:
                continue
            r = ret[e.author]
            if x["blockTimestamp"] < r or (x["blockTimestamp"] == r and e.seq <= rseq[e.author]):
                timely.add(e.id)
    lineage = {seed}
    changed = True
    while changed:
        changed = False
        for e in entries:
            if e.etype == "succession" and e.id in timely and e.author in lineage:
                t = e.body.get("to")
                if isinstance(t, str) and t not in lineage:
                    lineage.add(t)
                    changed = True
    ledger = [e for e in entries if e.author in lineage and e.id in timely]
    a.retired = set(e.id for e in entries if e.author in lineage and e.id not in timely)
    a.entries = entries
    a.seed = seed
    a.continued = a1 is not None
    a.cont_ids = set(e.id for e in ledger if e.seq == 0 and e.etype == "genesis" and e.body.has("continues")
                     and isinstance(e.body.get("continues"), JObj)
                     and e.body.get("continues").get("entry_id") == sv.id) if a1 is not None else set()

    def retired_at(x):
        return retired_at_key(x["sender"], x)
    kept = [x for x in seg_anchors if x["sender"] in lineage and not retired_at(x)]
    counted = [x for x in kept if x["verdict"] == "counted"]
    unproven = [x for x in kept if x["verdict"] == "UNPROVEN"]
    counted_hashes = set(x["hash"] for x in counted)
    findings, any_gap = _walk_findings(seed, ledger, True)
    have = set(e.id for e in ledger)
    missing = counted_hashes - have
    if any(f["hard"] for f in findings):
        label = "BROKEN_CHAIN"
    elif any(x["hash"] not in pile_ids and x["hash"] not in counted_hashes for x in unproven):
        label = "UNPROVEN"
    elif any_gap or missing:
        label = "GAPS"
    else:
        label = "COMPLETE"
    if a1 is not None:
        other = "UNPROVEN" if a1.label == "UNAVAILABLE" else a1.label
        label = min((label, other), key=LABEL_ORDER.index)
        ledger = ledger + a1.ledger
        counted = counted + a1.counted
        unproven = unproven + a1.unproven
        lineage = lineage | a1.lineage
        findings = findings + a1.findings
    a.label = label
    a.ledger = ledger
    a.by_id = {e.id: e for e in ledger}
    a.counted = counted
    a.unproven = unproven
    a.lineage = lineage
    a.findings = findings
    return a


def findings_on(a, entry_id, name):
    return [f for f in a.findings if f["name"] == name and (
        f["entry_id"] == entry_id or f.get("b") == entry_id)]


def _reach(a, F, memo):
    """sec 6.1: the ledger entries reachable from F along `prev`, and through a
    genesis's `continues.entry_id`."""
    if F.id in memo:
        return memo[F.id]
    seen = set()
    stack = [F]
    while stack:
        cur = stack.pop()
        if cur.id in seen:
            continue
        seen.add(cur.id)
        if cur.prev is not None and cur.prev in a.by_id:
            stack.append(a.by_id[cur.prev])
        if cur.id in a.cont_ids and cur.body.has("continues"):
            c = cur.body.get("continues")
            if isinstance(c, JObj) and isinstance(c.get("entry_id"), str) and c.get("entry_id") in a.by_id:
                stack.append(a.by_id[c.get("entry_id")])
    memo[F.id] = seen
    return seen


def unproven_reach(a):
    """sec 7.2 check 4: the ids an undiscarded UNPROVEN record would anchor."""
    memo = {}
    out = set()
    for rec in a.unproven:
        F = a.by_id.get(rec["hash"])
        if F is None:
            continue
        out |= set(_reach(a, F, memo))
    return out


def anchoring(a):
    """Returns (anchored_ids, bound_by_id) per sec 6.2."""
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
# sec 7: grant check, ledger link, chain check
# ==========================================================================


def _with_grant(inp, g):
    j = json.loads(json.dumps(inp))
    if isinstance(j, dict) and isinstance(j.get("pile"), list):
        j["pile"] = list(j["pile"]) + [hx(g)]
    return j


def covering(basis, lineage):
    """sec 7.2 check 4's coverage test, over both segments' lineages."""
    ch = basis["chains"]
    if not ch:
        return False
    for o in ch:
        if not o["registries"]:
            return False
        if not set(lineage) <= set(o["senders"]):
            return False
    return True


# --- sec 6.3, 6.4, 7.1: the notice input, the key in office, the member -----

N_KEYS = {"basis", "revocations", "accessions"}
N_BASIS_KEYS = {"chainId", "fromBlock", "toBlock", "revocationContract", "book", "bookFromBlock"}
N_REC_KEYS = {"chainId", "blockNumber", "txIndex", "logIndex", "tx", "sender", "grant", "verdict"}
N_BOOK_KEYS = {"chainId", "blockNumber", "txIndex", "logIndex", "holder", "to", "root"}


def _uint_ok(v):
    return is_int(v) and 0 <= v <= MAXINT


def _n_records_ok(rows, keys, cid, lo, hi, notice):
    if not isinstance(rows, list):
        return False
    seen = set()
    for r in rows:
        if not isinstance(r, dict) or set(r.keys()) != keys:
            return False
        if not (_uint_ok(r["chainId"]) and r["chainId"] == cid):
            return False
        if not (_uint_ok(r["blockNumber"]) and _uint_ok(r["txIndex"]) and _uint_ok(r["logIndex"])):
            return False
        if not (lo <= r["blockNumber"] <= hi):
            return False
        if notice:
            if not (is_hex32(r["tx"]) and is_hex20(r["sender"]) and is_hex32(r["grant"])):
                return False
            if r["verdict"] not in ("counted", "UNPROVEN", "VOID"):
                return False
        else:
            if not (is_hex20(r["holder"]) and is_hex20(r["to"]) and is_hex20(r["root"])):
                return False
        pos = (r["blockNumber"], r["txIndex"], r["logIndex"])
        if pos in seen:
            return False
        seen.add(pos)
    return True


def notices_valid(n):
    """sec 6.3: the well-formedness of a notice input, over the JSON value."""
    if not isinstance(n, dict) or set(n.keys()) != N_KEYS:
        return False
    b = n["basis"]
    if not isinstance(b, dict) or set(b.keys()) != N_BASIS_KEYS:
        return False
    for k in ("chainId", "fromBlock", "toBlock", "bookFromBlock"):
        if not _uint_ok(b[k]):
            return False
    if not (is_hex20(b["revocationContract"]) and is_hex20(b["book"])):
        return False
    if not (b["fromBlock"] <= b["toBlock"] and b["bookFromBlock"] <= b["toBlock"]):
        return False
    if not _n_records_ok(n["revocations"], N_REC_KEYS, b["chainId"], b["fromBlock"], b["toBlock"], True):
        return False
    if not _n_records_ok(n["accessions"], N_BOOK_KEYS, b["chainId"], b["bookFromBlock"], b["toBlock"], False):
        return False
    return True


def _pos(r):
    return (r["blockNumber"], r["txIndex"], r["logIndex"])


def office_at_position(n, root, pos):
    """sec 6.4: the `to` of the last book record of `root` strictly before `pos`."""
    key = root
    for r in sorted(n["accessions"], key=_pos):
        if _pos(r) < pos and r["root"] == root:
            key = r["to"]
    return key


def office_at_block(n, root, block):
    """sec 6.4: by the block alone; a record in the block moves nothing at it."""
    key = root
    for r in sorted(n["accessions"], key=_pos):
        if r["blockNumber"] < block and r["root"] == root:
            key = r["to"]
    return key


def member_of(gbody):
    """sec 7.1: (carries, (chain, at) or None)."""
    if not gbody.has("revocation"):
        return False, None
    m = gbody.get("revocation")
    if isinstance(m, JObj) and set(m.keys()) == {"chain", "at"} and is_int(m.get("chain")) \
            and is_hex20(m.get("at")):
        return True, (m.get("chain"), m.get("at"))
    return True, None


def n_applies(n, member):
    return n is not None and notices_valid(n) and member is not None \
        and n["basis"]["chainId"] == member[0] and n["basis"]["revocationContract"] == member[1]


def book_root(g, inp, nts=None, member=None):
    """sec 7.1, in order: the `root` of the least-positioned book record of N whose
    `to` is the author, where N applies and holds one; I's root where I is valid;
    the grant's `ledger` where hex20; its author."""
    author = parse(g).get("author")
    if n_applies(nts, member):
        for r in sorted(nts["accessions"], key=_pos):
            if r["to"] == author:
                return r["root"]
    if inp is not None and input_valid(inp):
        return inp["root"]
    b = body_of(g)
    if b.has("ledger") and is_hex20(b.get("ledger")):
        return b.get("ledger")
    return author


def _member_checks(g, gid, gbody, inp, a, have_audit, nts, st):
    """sec 7.2 for a grant carrying a revocation member: checks 2, 3, 4, 6."""
    _c, member = member_of(gbody)
    app = n_applies(nts, member)
    root = book_root(g, inp, nts, member)
    if not have_audit:
        st[1] = "UNKNOWN"
    elif a.label == "BROKEN_CHAIN":
        st[1] = "FAIL"
    elif not any(e.etype == "genesis" and e.author == a.seed for e in a.entries):
        st[1] = "UNKNOWN"
    else:
        st[1] = "PASS"
    if not have_audit or st[1] == "FAIL":
        st[2] = st[3] = "UNKNOWN"
    else:
        author = parse(g).get("author")
        standing = []
        if app:
            standing = [x for x in _dedupe(inp["anchors"])
                        if x["chainId"] == member[0] and x["hash"] == gid and x["verdict"] == "counted"
                        and x["blockNumber"] <= nts["basis"]["toBlock"]
                        and x["sender"] == office_at_block(nts, root, x["blockNumber"])]
        # the table's two unconditional convictions: EXCLUDED of either report, or a zikaron/1 entry
        # MALFORMED outside the old segment (RETIRED and the never-moved zikaron/1 ledger are not that)
        table_fails = (gid not in a.by_id and gid not in a.retired
                       and not (parse(g).get("spec") == "zikaron/1" and not a.continued))
        seated = app and author in (set([root]) | set(r["to"] for r in nts["accessions"] if r["root"] == root))
        if st[1] == "PASS" and table_fails and not seated:
            st[2] = "FAIL"
        elif not app:
            st[2] = "UNKNOWN"
        elif a.root != root:
            st[2] = "FAIL"
        elif any(x["sender"] == author for x in standing):
            st[2] = "PASS"
        elif standing:
            st[2] = "FAIL"
        else:
            st[2] = "UNKNOWN"
        st[3] = ("PASS" if standing else "UNKNOWN") if app else "UNKNOWN"
    if not app:
        st[5] = "UNKNOWN"
    else:
        hit = None
        for r in sorted(nts["revocations"], key=_pos):
            if r["grant"] != gid:
                continue
            if r["sender"] != office_at_position(nts, root, _pos(r)):
                continue
            if r["verdict"] == "counted":
                hit = "FAIL"
                break
            hit = "UNKNOWN"   # UNPROVEN or VOID by the key in office
        st[5] = hit or "PASS"


def grant_check(g, inp, now, nts=None):
    """sec 7.2 and sec 7.3."""
    st = ["UNKNOWN"] * 6
    reason = [None] * 6

    ok, val = accept_check(g)
    if not ok:
        st[0], reason[0] = "FAIL", val
    elif parse(g).get("entryType") != "grant":
        st[0], reason[0] = "FAIL", "NOT_A_GRANT"
    else:
        st[0] = "PASS"

    a = None
    if inp is not None:
        a = audit2(_with_grant(inp, g))
    have_audit = a is not None and a.valid

    if st[0] == "FAIL":
        return _result(st, reason, a if have_audit else None, nts)

    gid = hx(sha256(g))
    gbody = parse(g).get("body")

    if member_of(gbody)[0]:
        _member_checks(g, gid, gbody, inp, a, have_audit, nts, st)
        if not gbody.has("window"):
            st[4] = "PASS"
        elif now is None:
            st[4] = "UNKNOWN"
        else:
            w = gbody.get("window")
            st[4] = "FAIL" if (now < w.get("from") or now > w.get("to")) else "PASS"
        return _result(st, reason, a if have_audit else None, nts)

    # check 2
    if not have_audit:
        st[1] = "UNKNOWN"
    elif a.label == "BROKEN_CHAIN":
        st[1] = "FAIL"
        reason[1] = None
    else:
        st[1] = "PASS"

    # check 3
    gate46 = False
    if not have_audit or st[1] == "FAIL":
        st[2] = "UNKNOWN"
    elif parse(g).get("spec") == "zikaron/1" and not a.continued:
        st[2] = "UNKNOWN"      # sec 7.2 check 3: a zikaron/1 grant of a ledger that never moved
        gate46 = True
    elif gid not in a.by_id:
        if gid in a.retired:
            st[2] = "FAIL" if a.label == "COMPLETE" else "UNKNOWN"
        else:
            st[2] = "FAIL"
        gate46 = True
    elif findings_on(a, gid, "AUTHORITY_MISMATCH"):
        st[2] = "UNKNOWN"
    else:
        st[2] = "PASS"

    # check 4
    if not have_audit or st[1] == "FAIL" or gate46:
        st[3] = "UNKNOWN"
    else:
        anchored, _bound = anchoring(a)
        if gid in anchored:
            st[3] = "PASS"
        elif not covering(a.basis, a.lineage):
            st[3] = "UNKNOWN"
        elif gid in unproven_reach(a):
            st[3] = "UNKNOWN"  # sec 7.2 check 4: an undecided anchor of bytes that would anchor g
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
    if not have_audit or st[1] == "FAIL" or gate46:
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

    return _result(st, reason, a if have_audit else None, nts)


def _result(st, reason, a, nts=None):
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
            "notices": (nts["basis"] if (nts is not None and notices_valid(nts)) else None),
            "checks": checks,
            "failed": [CHECK_TOKENS[i] for i in range(6) if st[i] == "FAIL"]}


def ledger_link(u, d, inp_d, nts_d=None):
    """sec 7.4.  u and d are accepted grant byte strings."""
    if not byte_link(u, d):
        return False
    carries, member = member_of(body_of(d))
    if (inp_d is None or not input_valid(inp_d)) and not (carries and n_applies(nts_d, member)):
        return None
    if book_root(d, inp_d, nts_d, member) != body_of(u).get("grantee"):
        return False
    return True


def _hop3(h):
    return (h[0], h[1], h[2] if len(h) > 2 else None)


def chain_check(hops, now):
    """sec 7.5.  `hops` is a list of (grant bytes, audit input or None[, notice
    input or None])."""
    if not hops:
        return {"verdict": "FAIL", "hops": [], "links": [], "token": "CHAIN_EMPTY",
                "failing": {"kind": "empty", "index": 0}}
    hops = [_hop3(h) for h in hops]
    results = [grant_check(g, i, now, n) for g, i, n in hops]
    c1 = [r["checks"][0]["state"] != "FAIL" for r in results]
    links = []
    for k in range(1, len(hops)):
        if not c1[k - 1] or not c1[k]:
            links.append(None)
        else:
            links.append(ledger_link(hops[k - 1][0], hops[k][0], hops[k][1], hops[k][2]))
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
    d = int.from_bytes(sha256(("zkk2-key/%s/%d" % (tag, seed)).encode()), "big")
    return (d % (N - 2)) + 1


KEYS = {}


def K(t):
    return KEYS[t]


def AD(t):
    return pg.privkey_to_addr_hex(KEYS[t])


# ==========================================================================
# document construction
# ==========================================================================


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
# badges/ : sec 3
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

    B("ok_one", payload([g0]), "sec 3.2 step 6: one segment, a grant that states no "
                               "upstream.")
    B("ok_two", payload([g0, g1]), "sec 3.2: two segments whose byte link holds.")
    B("ok_three", payload([g0, g1, g2]), "sec 3.2: three segments, links at k = 1 and 2.")

    # step 1: prefix
    B("prefix_wrong", b"zikaron-badge:" + b64u(g0).encode(),
      "sec 3.2 step 1: the 14 bytes must be exactly `zikaron-grant:`.")
    B("prefix_short", b"zikaron-grant", "sec 3.2 step 1: thirteen bytes are not fourteen.")
    B("prefix_empty", b"", "sec 3.2 step 1 over the empty byte string.")
    B("prefix_case", b"ZIKARON-GRANT:" + b64u(g0).encode(),
      "sec 3.2 step 1: byte-equality, so case matters.")

    # step 2: the cap, on both sides
    cap_ok = padded_grant(K("author"), 2204, grantee=AD("r1"), work=WK, terms=HX32(4))
    p_ok = payload([cap_ok])
    assert len(p_ok) == 2953, len(p_ok)
    B("cap_2953", p_ok, "sec 3.1/3.2 step 2: a payload of exactly BADGE_CAP bytes, the "
                        "largest lawful one.")
    cap_over = padded_grant(K("author"), 2205, grantee=AD("r1"), work=WK, terms=HX32(4))
    p_over = payload([cap_over])
    assert len(p_over) == 2954, len(p_over)
    B("cap_2954", p_over, "sec 3.2 step 2: one byte over BADGE_CAP.")
    B("cap_beats_segments", p_over[:-1] + b"*",
      "sec 3.2 order: the cap is decided before any segment is examined, so a payload over "
      "the cap carrying a foreign byte is E_BADGE_CAP and not E_BADGE_B64.")
    B("prefix_beats_cap", b"x" + p_over,
      "sec 3.2 order: the prefix is decided before the cap.")
    gen_b = pg.make_entry(K("author"), "genesis", 0, None, {"statement_md": "for the encoder"})
    BE("encode_type_before_cap", [gen_b, cap_over],
       "sec 3.1 order: each segment's entry and type are tested in order before the cap, so "
       "a genesis at index 0 beside an oversized grant is E_BADGE_TYPE at 0 and not E_BADGE_CAP.")
    BE("encode_cap_after_segments", [cap_over],
       "sec 3.1 order: one accepted grant whose payload is one byte over BADGE_CAP fails the "
       "cap after every segment passed.")

    # step 3a: base64url
    B("b64_empty_after_prefix", BADGE_PREFIX,
      "sec 3.2 step 3: no dots yield one segment, and it is empty.")
    B("b64_leading_dot", BADGE_PREFIX + b"." + b64u(g0).encode(),
      "sec 3.2 step 3: a leading dot yields an empty segment at index 0.")
    B("b64_trailing_dot", payload([g0]) + b".",
      "sec 3.2 step 3: a trailing dot yields an empty segment at the end.")
    B("b64_doubled_dot",
      BADGE_PREFIX + (b64u(g0) + ".." + b64u(g1)).encode(),
      "sec 3.2 step 3: a doubled dot yields an empty segment at index 1.")
    B("b64_only_dot", BADGE_PREFIX + b".",
      "sec 3.2 step 3: one dot, two empty segments; index 0 is named.")
    B("b64_foreign_plus", BADGE_PREFIX + (b64u(g0)[:-2] + "+A").encode(),
      "sec 3.2 step 3a: `+` is outside A-Za-z0-9-_ (this is base64url, not base64).")
    B("b64_foreign_slash", BADGE_PREFIX + (b64u(g0)[:-2] + "/A").encode(),
      "sec 3.2 step 3a: `/` is outside the alphabet.")
    B("b64_foreign_pad", BADGE_PREFIX + (b64u(g0) + "==").encode(),
      "sec 3.1: base64url with no padding characters, so `=` is a foreign byte.")
    B("b64_foreign_high_byte", BADGE_PREFIX + b64u(g0).encode() + b"\xff",
      "sec 3.2 step 3a: a byte outside ASCII is outside the alphabet.")
    B("b64_len_1_mod_4", BADGE_PREFIX + b"A",
      "sec 3.2 step 3a: a length of 1 mod 4 encodes no whole byte count.")
    B("b64_len_5", BADGE_PREFIX + b"AAAAA",
      "sec 3.2 step 3a: 5 is 1 mod 4.")
    B("b64_len_2_ok", BADGE_PREFIX + b"AA",
      "sec 3.2 step 3a passes (2 mod 4, trailing bits zero) and step 3b then names the "
      "decoded byte string, which is one 0x00 byte.")
    B("b64_trailing_bits_len2", BADGE_PREFIX + b"AB",
      "sec 3.2 step 3a: at 2 mod 4 the last character's low four bits are unused and must "
      "be zero; `B` carries 1.")
    B("b64_trailing_bits_len3", BADGE_PREFIX + b"AAB",
      "sec 3.2 step 3a: at 3 mod 4 the last character's low two bits are unused and must be "
      "zero.")
    B("b64_trailing_bits_len3_ok", BADGE_PREFIX + b"AAE",
      "sec 3.2 step 3a passes: `E` is index 4, whose low two bits are zero; step 3b then "
      "names the decoded two bytes.")

    # step 3b and 3c
    B("entry_not_json", BADGE_PREFIX + b64u(b"not json at all").encode(),
      "sec 3.2 step 3b: the decoded bytes fail `accept`, and the parent's token travels "
      "with the index.")
    B("entry_not_canonical", BADGE_PREFIX + b64u(pg.rot_top(g0, 2)).encode(),
      "sec 3.2 step 3b: a re-encoding of a valid entry is a different byte string that is "
      "not an entry.")
    B("entry_bad_sig",
      BADGE_PREFIX + b64u(pg.set_key(g0, "author", AD("r4"))).encode(),
      "sec 3.2 step 3b: the parent's signature test names the fault.")
    B("entry_at_index_1",
      BADGE_PREFIX + (b64u(g0) + "." + b64u(b"{}")).encode(),
      "sec 3.2 step 3: segments are examined in order, so the index travels with the "
      "token.")
    B("type_history", BADGE_PREFIX + b64u(hist).encode(),
      "sec 3.2 step 3c: an accepted entry that is not a grant.")
    B("type_annotation_at_1",
      BADGE_PREFIX + (b64u(g0) + "." + b64u(ann)).encode(),
      "sec 3.2 step 3c at index 1.")
    B("b64_beats_entry",
      BADGE_PREFIX + (b64u(g0)[:-1] + "*" + "." + b64u(b"{}")).encode(),
      "sec 3.2 step 3: for each segment, 3a is decided before 3b, and segment 0 is decided "
      "before segment 1 is examined.")
    B("type_beats_next_segment",
      BADGE_PREFIX + (b64u(hist) + "." + "A").encode(),
      "sec 3.2 step 3: segment 0's type fault is named before segment 1's base64 fault, "
      "because each segment is decided before the next is examined.")

    # step 4: incomplete
    B("incomplete_upstream_value",
      payload([grant_entry(K("author"), grantee=AD("r1"), work=WK, terms=HX32(5),
                           upstream=HX32(0xABC))]),
      "sec 3.2 step 4: segment 0's grant states an upstream, so the payload does not start "
      "at the original author's grant.")
    B("incomplete_upstream_null",
      payload([grant_entry(K("author"), grantee=AD("r1"), work=WK, terms=HX32(6),
                           upstream=None)]),
      "sec 3.2 step 4 with the parent sec 6's presence rule: a key present is a member "
      "present, null included.")
    B("incomplete_beats_link",
      payload([grant_entry(K("author"), grantee=AD("r1"), work=WK, terms=HX32(7),
                           upstream=None), g1]),
      "sec 3.2 order: step 4 is decided before any link of step 5, and this payload's link "
      "is broken too.")

    # step 5: links
    bad_up = grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(8),
                         upstream=HX32(0xDEAD))
    B("link_upstream_value", payload([g0, bad_up]),
      "sec 3.3: `upstream` is a string that is not hex32(entry_id(u)).")
    bad_work = grant_entry(K("r1"), grantee=AD("r2"), work=HX32(0xFEED), terms=HX32(9),
                           upstream=g0id)
    B("link_work", payload([g0, bad_work]),
      "sec 3.3: the upstream names the right entry and the works differ.")
    nonstr = grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(10), upstream=7)
    B("link_nonstring_upstream", payload([g0, nonstr]),
      "sec 3.2 step 3b: a zikaron/2 grant whose `upstream` is an int fails accept at its body "
      "row, so the payload fails at the entry before any link; sec 3.3's string test is "
      "reached by a zikaron/1 grant (link_v1_upstream_int).")
    B("link_null_upstream",
      payload([g0, grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(11),
                               upstream=None)]),
      "sec 3.2 step 3b: a zikaron/2 grant with `upstream` null fails accept at its body row; "
      "the presence rule of sec 3.2 step 4 is reached by a zikaron/1 grant "
      "(incomplete_v1_upstream_null).")
    B("link_absent_upstream",
      payload([g0, grant_entry(K("r1"), grantee=AD("r2"), work=WK, terms=HX32(12))]),
      "sec 3.3: a downstream grant carrying no `upstream` at all.")
    v1up_null = E1("author", "grant", 1, HX32(0x11), {"grantee": AD("r1"), "work": WK, "terms": HX32(60), "upstream": None})
    B("incomplete_v1_upstream_null", payload([v1up_null]),
      "sec 3.2 step 4 over a zikaron/1 grant, whose law reads `upstream` as data: present "
      "with value null it is a member present, and the payload does not start at the "
      "original author's grant.")
    v1up_int = E1("r1", "grant", 1, HX32(0x11), {"grantee": AD("r2"), "work": WK, "terms": HX32(61), "upstream": 5})
    B("link_v1_upstream_int", payload([g0, v1up_int]),
      "sec 3.3 over a zikaron/1 downstream grant whose `upstream` is an int: a member "
      "present whose value is no string; the link fails at index 1.")
    g2bad = grant_entry(K("r2"), grantee=AD("r3"), work=WK, terms=HX32(13),
                        upstream=HX32(0xBAD))
    B("link_broken_at_2", payload([g0, g1, g2bad]),
      "sec 3.2 step 5: the links are decided in order and the smallest failing k is named.")
    B("link_first_of_two_broken", payload([g0, bad_up, g2]),
      "sec 3.2 step 5: link 1 is false and link 2 is false as well; index 1 is named.")

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
    BE("one_grant", [g0], "sec 3.1: one accepted grant.")
    BE("two_grants", [g0, g1], "sec 3.1: two grants in order from the original author's.")
    BE("three_grants", [g0, g1, g2], "sec 3.1: three grants.")
    BE("cap_exact", [cap_ok], "sec 3.1: the encoding is exactly BADGE_CAP bytes.")
    BE("cap_over", [cap_over], "sec 3.1: one byte over the cap.")
    BE("cap_over_two", [g0, cap_ok], "sec 3.1: two grants whose joined encoding exceeds "
                                     "the cap.")
    BE("not_an_entry", [b"{}"], "sec 3.1: an input that fails `accept`.")
    BE("not_a_grant", [hist], "sec 3.1: an accepted entry that is not of type grant.")
    BE("not_a_grant_at_1", [g0, ann], "sec 3.1: the second input is not a grant.")
    BE("seg0_upstream", [grant_entry(K("author"), grantee=AD("r1"), work=WK,
                                     terms=HX32(14), upstream=HX32(0xABC))],
       "sec 3.1: an encoder assembles what it is handed, so a payload sec 3.2 step 4 "
       "refuses on decoding is encoded all the same.")
    BE("unlinked_pair", [g0, bad_up],
       "sec 3.1 over two grants whose byte link does not hold: the encoder assembles "
       "them, and sec 3.2 step 5 refuses the payload on decoding.")
    return {"g0": g0, "g1": g1, "g2": g2, "WK": WK, "hist": hist, "ann": ann}


# ==========================================================================
# kits/ : sec 4; packages/ : sec 5
# ==========================================================================


def frow(p, s, n, name=None):
    r = {"path": p, "sha256": s, "size": n}
    if name is not None:
        r["name_md"] = name
    return r


def crow(c, p, name=None):
    r = {"content": c, "path": p}
    if name is not None:
        r["name_md"] = name
    return r


def arow(entry_id, cid, tx):
    return {"entry_id": entry_id, "chainId": cid, "tx": tx}


def mfst(root=None, entries=None, files=None, contents=None, note_md="", anchored_on=None,
         anchors=None):
    m = {"spec": KIT_SPEC, "root": root,
         "entries": [] if entries is None else entries,
         "files": [] if files is None else files,
         "contents": [] if contents is None else contents,
         "note_md": note_md}
    if anchored_on is not None:
        m["anchoredOn"] = anchored_on
    if anchors is not None:
        m["anchors"] = anchors
    return m


def dirman_digest(rows):
    """sec 4.5: the digest of the directory manifest over (name, sha256) rows."""
    rows = sorted(rows, key=lambda r: bwise(r[0]))
    return hx(sha256(cbytes(obj([("files", [{"path": nm, "sha256": h} for nm, h in rows])]))))


def assemble(entries=(), files=(), root=None, note_md="", contents_for=None, dirs=(),
             anchored_on=None, anchors=None, dir_names=None):
    """A kit whose manifest and disk agree, as an author's export would.
    `files` holds (kit path, bytes) or (kit path, bytes, name_md); `dirs`
    names directory originals, each a contents row over the files under it;
    `dir_names` maps a directory original to the name_md its row carries."""
    ids = sorted((eid(b) for b in entries), key=bwise)
    disk = {}
    for b in entries:
        disk["entries/" + eid(b)[2:] + ".zk1"] = b
    frows = []
    for f in sorted(files, key=lambda x: bwise(x[0])):
        pth, b = f[0], f[1]
        disk["files/" + pth] = b
        frows.append(frow(pth, hx(sha256(b)), len(b), f[2] if len(f) > 2 else None))
    crows = []
    for f in files:
        if contents_for is None or f[0] in contents_for:
            crows.append((hx(sha256(f[1])), f[0]))
    for d in dirs:
        rows = []
        for f in files:
            if f[0].startswith(d + "/"):
                rows.append((f[2] if len(f) > 2 else f[0][len(d) + 1:], hx(sha256(f[1]))))
        crows.append((dirman_digest(rows), d))
    crows.sort(key=lambda x: (bwise(x[0]), bwise(x[1])))
    m = mfst(root, ids, frows, [crow(c, pth, (dir_names or {}).get(pth)) for c, pth in crows],
             note_md, anchored_on, anchors)
    return m, disk


def onefile_bytes(items):
    """sec 4.1a over {path: bytes} with manifest.json present."""
    out = KIT_MAGIC + b"manifest.json\n" + str(len(items["manifest.json"])).encode() + b"\n" + items["manifest.json"] + b"\n"
    for pth in sorted((k for k in items if k != "manifest.json"), key=bwise):
        out += pth.encode("utf-8") + b"\n" + str(len(items[pth])).encode() + b"\n" + items[pth] + b"\n"
    return out


def _items(manifest, disk, extras, raw_manifest, no_manifest):
    items = {}
    if raw_manifest is not None:
        items["manifest.json"] = raw_manifest
    elif not no_manifest:
        items["manifest.json"] = cbytes(J(manifest))
    for pth, b in sorted((disk or {}).items()):
        items[pth] = b
    for pth, b in extras:
        items[pth] = b
    return items


def _write_dir(d, items, symlinks=(), empty_dirs=(), fifos=(), chmods=(), chmod_root=None):
    absd = os.path.join(HERE, d)
    os.makedirs(absd)
    for pth, b in items.items():
        W(d + "/" + pth, b)
    for pth in empty_dirs:
        os.makedirs(os.path.join(absd, pth))
    for pth, target in symlinks:
        pp = os.path.join(absd, pth)
        pd = os.path.dirname(pp)
        if pd and not os.path.isdir(pd):
            os.makedirs(pd)
        os.symlink(target, pp)
    for pth in fifos:
        os.mkfifo(os.path.join(absd, pth))
    for pth, mode in chmods:
        os.chmod(os.path.join(absd, pth), mode)
    if chmod_root is not None:
        os.chmod(absd, chmod_root)


def make_kit(slug, note, manifest=None, disk=None, extras=(), symlinks=(), empty_dirs=(),
             raw_manifest=None, no_manifest=False, fifos=(), chmods=(), chmod_root=None,
             onefile=False, raw_onefile=None, chmod_file=None, link_to=None):
    i = seqno("kits")
    items = _items(manifest, disk, extras, raw_manifest, no_manifest)
    if link_to is not None:
        d = "kits/k%03d_%s" % (i, slug) + (".zkit" if link_to.endswith(".zkit") else "")
        os.symlink(os.path.join(HERE, link_to), os.path.join(HERE, d))
    elif onefile or raw_onefile is not None:
        d = "kits/k%03d_%s.zkit" % (i, slug)
        W(d, raw_onefile if raw_onefile is not None else onefile_bytes(items))
        if chmod_file is not None:
            os.chmod(os.path.join(HERE, d), chmod_file)
    else:
        d = "kits/k%03d_%s" % (i, slug)
        _write_dir(d, items, symlinks, empty_dirs, fifos, chmods, chmod_root)
    pred = kit_verify(os.path.join(HERE, d))
    CASE("kits", d, "kit-verify", ["kit-verify", d], note, pred)
    return d


def make_package(slug, note, manifest=None, disk=None, extras=(), onefile=True, raw_onefile=None,
                 raw_manifest=None, no_manifest=False, chmod_file=None):
    i = seqno("packages")
    items = _items(manifest, disk, extras, raw_manifest, no_manifest)
    if onefile or raw_onefile is not None:
        d = "packages/g%03d_%s.zkgrant" % (i, slug)
        W(d, raw_onefile if raw_onefile is not None else onefile_bytes(items))
        if chmod_file is not None:
            os.chmod(os.path.join(HERE, d), chmod_file)
    else:
        d = "packages/g%03d_%s" % (i, slug)
        _write_dir(d, items)
    pred = grant_package(os.path.join(HERE, d))
    CASE("packages", d, "grant-package", ["grant-package", d], note, pred)
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
    full_m, full_disk = assemble(ents, files, root=ROOT,
                                 note_md="a kit that proves what its bytes say",
                                 contents_for={"art/final.bin"})
    make_kit("ok_full", "sec 4.4 step 7: entries, files and contents all agree with the "
                        "enumeration the walk yields.", full_m, full_disk)
    make_kit("ok_full_onefile", "sec 4.1a: the same kit as one byte string; the "
                                "enumeration is the set of its (path, bytes) pairs and "
                                "the verdict is the same.", full_m, full_disk, onefile=True)
    import copy as _copy
    fkey = sorted(k for k in full_disk if k.startswith("files/"))[0]
    same_len = bytes((b + 1) % 256 for b in full_disk[fkey])
    make_kit("file_wrong_sha_same_size",
             "sec 4.4 step 4: a file whose bytes have the listed length and another sha256 "
             "fails the digest leg alone.", full_m, dict(full_disk, **{fkey: same_len}))
    m_files = _copy.deepcopy(full_m); m_files["files"][0]["zz"] = 1
    make_kit("files_row_extra_member",
             "sec 4.3: a files element is an object with exactly path, sha256, size and "
             "optionally name_md; a foreign member fails the files rule.", m_files, full_disk)
    make_kit("unreadable_directory",
             "sec 4.1: a directory the walk cannot list fails the walk with that directory's "
             "path as the subject (mode 0000; regenerate rather than copy).",
             full_m, full_disk, chmods=[("files/art", 0)])
    make_kit("unreadable_file",
             "sec 4.1: a file the walk cannot read in full fails the walk with that file's "
             "path as the subject (mode 0000; regenerate rather than copy).",
             full_m, full_disk, chmods=[(fkey, 0)])
    make_kit("kit_directory_unlistable",
             "sec 4.1: the kit directory itself cannot be listed; the subject is the "
             "one-character string `.` (mode 0000; regenerate rather than copy).",
             full_m, full_disk, chmod_root=0)
    make_kit("ok_empty", "sec 4.4: a directory holding only manifest.json with empty arrays "
                         "is a kit that proves nothing, and KIT_OK says so through its counts.",
             mfst(None, [], [], [], ""), {})
    make_kit("ok_empty_onefile", "sec 4.1a: the smallest one-file kit, the manifest alone.",
             mfst(None, [], [], [], ""), {}, onefile=True)
    make_kit("ok_root_null", "sec 4.3: `root` is null or hex20.",
             *assemble(ents, [], root=None))
    make_kit("ok_note_prose",
             "sec 4.3: `note_md` is a prose string, so control and astral scalars are lawful.",
             *assemble([], files, root=ROOT, note_md=" café \U0001f600"))
    seg255 = "a" * 251 + ".bin"
    make_kit("ok_segment_255",
             "sec 4.2: a segment of exactly 255 bytes, the largest a kit path admits.",
             *assemble([], [(seg255, b"at the segment ceiling\n")], root=ROOT))
    longp = "a" * 200 + "/" + "b" * 200 + "/" + "c" * 196 + ".bin"
    make_kit("ok_long_path",
             "sec 4.2: a 602-byte path of three segments, walked through two directory levels.",
             *assemble([], [(longp, b"deep\n")], root=ROOT))
    make_kit("ok_walk_order_names",
             "sec 4.1/4.2: the directory `a` beside the files `a-b` and `a.b`; the three order "
             "as 0x2d < 0x2e < 0x2f, so the manifest's order and the walk's order both turn "
             "on the separator byte.",
             *assemble([], [("a-b", b"dash\n"), ("a.b", b"dot\n"), ("a/b", b"under the directory\n")], root=ROOT))
    make_kit("ok_empty_directory",
             "sec 4.1: a directory contributes no pair to the enumeration, so an empty one is "
             "not an extra and the kit verifies.", full_m, full_disk, empty_dirs=["scratch"])
    bad_entry = b'{"spec":"zikaron/2"}'
    inv_m, inv_disk = assemble(ents, [], root=ROOT)
    inv_m["entries"] = sorted(inv_m["entries"] + [hx(sha256(bad_entry))], key=bwise)
    inv_disk["entries/" + hx(sha256(bad_entry))[2:] + ".zk1"] = bad_entry
    make_kit("ok_invalid_entry",
             "sec 4.4 step 7: a listed id whose bytes are at the right path with the right "
             "doc_id and fail `accept`; the kit is a pile, and `invalid_entries` names it.",
             inv_m, inv_disk)
    v1_gen = E1("author", "genesis", 0, None, {"statement_md": "under the first law"})
    v1_gr = E1("author", "grant", 1, eid(v1_gen), {"grantee": AD("r1"), "work": CH, "terms": HX32(0x7E)})
    mix_m, mix_disk = assemble([v1_gen, v1_gr, e_gen], [], root=ROOT)
    make_kit("ok_mixed_laws_entries",
             "sec 1/4.4 step 7: entries of both laws are accepted by `accept`, dispatched by "
             "spec, so a kit carrying a continued ledger's old segment lists no invalid "
             "entry.", mix_m, mix_disk)
    bad_v1 = pg1.set_key(v1_gr, "author", AD("r4"))
    bv_m, bv_disk = assemble([v1_gen, bad_v1], [], root=ROOT)
    make_kit("ok_invalid_v1_entry",
             "sec 1: a zikaron/1-shaped byte string whose signature fails is read by the "
             "zikaron/1 core and listed with that law's token.", bv_m, bv_disk)

    # -- name_md, directory originals, anchoredOn, anchors ------------------
    nfiles = [("work/final.bin", fbytes["art/final.bin"], "Final Cut (v2).bin"),
              ("work/notes.txt", fbytes["note.txt"], "notes/read me.txt"),
              ("plain.txt", b"a file whose kit path spells its name\n")]
    dm, dd = assemble(ents, nfiles, root=ROOT, dirs=["work"])
    make_kit("ok_directory_original",
             "sec 4.3/4.4 step 5/4.5: a contents row over the directory `work`; its content is "
             "the sha256 of the directory manifest recomputed from the two files under it "
             "with the names name_md restores.", dm, dd)
    ok_dir = make_kit("ok_directory_original_onefile",
                      "sec 4.1a with sec 4.4 step 5: the same kit as one file.", dm, dd, onefile=True)
    ok_dir_dir = make_kit("ok_directory_original_again",
                          "sec 4.4 step 5: the directory-original kit once more, as the target of a link.", dm, dd)
    make_kit("ok_argument_is_link_to_directory",
             "sec 4.1 with HARNESS-KIT2: the kit directory is the directory the reader was handed, "
             "its name resolved as the platform resolves it; a symbolic link to a kit directory "
             "is that directory, and only the entries the walk examines fail it.", link_to=ok_dir_dir)
    make_kit("ok_argument_is_link_to_onefile",
             "sec 4.1a with HARNESS-KIT2: a symbolic link to a one-file kit is read as that file.",
             link_to=ok_dir)
    make_kit("onefile_unreadable",
             "sec 4.1a: a one-file kit the reader cannot read in full yields E_KIT_UNREADABLE with "
             "subject `.` (mode 0000; regenerate rather than copy).", dm, dd, onefile=True, chmod_file=0)
    nfiles_x = [("work/final.bin", fbytes["art/final.bin"], "Final Cut (v2).bin"),
                ("work/notes.txt", fbytes["note.txt"], "Final Cut (v2).bin"),
                ("plain.txt", b"a file whose kit path spells its name\n")]
    make_kit("rule_contents_directory_names_collide",
             "sec 4.3 rule `contents`: two files under one directory original restore one name "
             "by name_md; the directory manifest would describe no directory, so the row fails.",
             *assemble(ents, nfiles_x, root=ROOT, dirs=["work"]))
    nfiles_y = [("work/final.bin", fbytes["art/final.bin"], "notes.txt"),
                ("work/notes.txt", fbytes["note.txt"]),
                ("plain.txt", b"a file whose kit path spells its name\n")]
    make_kit("rule_contents_directory_name_md_equals_sibling_path",
             "sec 4.3 rule `contents`: a name_md equal to a sibling's path with the prefix removed "
             "is the same collision.", *assemble(ents, nfiles_y, root=ROOT, dirs=["work"]))
    nfiles_z = [("work/final.bin", fbytes["art/final.bin"], "Final Cut (v2).bin"),
                ("work/sub/notes.txt", fbytes["note.txt"], "read me.txt"),
                ("plain.txt", b"a file whose kit path spells its name\n")]
    make_kit("rule_contents_nested_directory_originals",
             "sec 4.3 rule `contents`: a directory original under another; a file lies under at "
             "most one, so the row fails.", *assemble(ents, nfiles_z, root=ROOT, dirs=["work", "work/sub"]))
    nfiles_w = [("work/final.bin", fbytes["art/final.bin"], "Final Cut (v2).bin"),
                ("workshop/notes.txt", fbytes["note.txt"], "notes.txt")]
    make_kit("ok_directory_original_distinct_subdirectories",
             "sec 4.3 rule `contents`: two directory originals side by side, `work` and `workshop`, "
             "neither followed by `/` a prefix of the other.", *assemble(ents, nfiles_w, root=ROOT, dirs=["work", "workshop"]))
    nfiles_t1 = [("work/x", b"a file named x", "x"), ("work/x-y", b"under a directory named x", "x/y")]
    make_kit("rule_contents_directory_name_is_file_and_directory",
             "sec 4.3 rule `contents`: the names under one directory original spell a directory; "
             "`x` beside `x/y` makes one name a file and a directory, so the row fails.",
             *assemble(ents, nfiles_t1, root=ROOT, dirs=["work"]))
    for slug, nm in (("dotdot", "../z"), ("empty", ""), ("absolute", "/a"), ("empty_segment", "a//b"), ("dot", "./a"), ("dot_alone", ".")):
        make_kit("rule_contents_directory_name_" + slug,
                 "sec 4.3 rule `contents`: a name under a directory original is one or more non-empty "
                 "segments joined by `/`, none `.` or `..`; this one is not.",
                 *assemble(ents, [("work/final.bin", fbytes["art/final.bin"], nm), ("work/notes.txt", fbytes["note.txt"], "n.txt")], root=ROOT, dirs=["work"]))
    make_kit("ok_directory_original_names_share_a_prefix",
             "sec 4.3 rule `contents`: names `a` and `ab` under one directory original share a "
             "prefix without a `/`; the tree rule passes.",
             *assemble(ents, [("work/final.bin", fbytes["art/final.bin"], "a"), ("work/notes.txt", fbytes["note.txt"], "ab")], root=ROOT, dirs=["work"]))
    make_kit("ok_directory_original_nested_names",
             "sec 4.3 rule `contents`: names `sub/a.bin` and `sub/b.txt` under one directory original "
             "spell a directory with a subdirectory; the passing side of the tree rule.",
             *assemble(ents, [("work/final.bin", fbytes["art/final.bin"], "sub/a.bin"), ("work/notes.txt", fbytes["note.txt"], "sub/b.txt")], root=ROOT, dirs=["work"]))
    nfiles_u = [("work/final.bin", fbytes["art/final.bin"], "r\u00e9sum\u00e9 \U0001f600.bin"),
                ("work/notes.txt", fbytes["note.txt"], "\u8bfb\u6211.txt"),
                ("plain.txt", b"a file whose kit path spells its name\n")]
    um, ud = assemble(ents, nfiles_u, root=ROOT, dirs=["work"])
    make_kit("ok_directory_original_non_ascii_names",
             "sec 2.1/4.5: the directory manifest is a value the verifier computes, its path "
             "strings prose; names outside the skeleton charset are serialized by zikaron/2 "
             "sec 3.4 and no sec 3.5 test applies.", um, ud)
    make_kit("ok_directory_original_non_ascii_names_onefile",
             "sec 4.1a with sec 4.5: the same kit as one file.", um, ud, onefile=True)
    nfiles_r = [("work/final.bin", fbytes["art/final.bin"], "final.bin"),
                ("plain.txt", b"a file whose kit path spells its name\n", "plain.txt")]
    make_kit("ok_name_md_redundant",
             "sec 4.3 rule `files`: a name_md equal to the name the kit path spells is a "
             "producer's redundancy and no rule tests it.", *assemble(ents, nfiles_r, root=ROOT, dirs=["work"]))
    dm2, dd2 = assemble(ents, nfiles, root=ROOT, dirs=["work"], contents_for={"work/final.bin"})
    m = _copy.deepcopy(dm)
    m["contents"] = [crow(c["content"], c["path"]) for c in m["contents"]] + [crow(HX32(0xBAD), "work")]
    m["contents"].sort(key=lambda r: (bwise(r["content"]), bwise(r["path"])))
    make_kit("content_two_rows_one_directory",
             "sec 4.3/4.4 step 5: two contents rows name one directory original with different "
             "content; the rows are distinct and sorted, and the one whose content is not the "
             "recomputed digest fails step 5 with the row's path.", m, dd)
    make_kit("ok_file_and_directory_rows",
             "sec 4.3: a file original and a directory original in one manifest, the two "
             "readings exclusive by the files rule.", dm2, dd2)
    m = _copy.deepcopy(dm)
    for row in m["contents"]:
        if row["path"] == "work":
            row["content"] = HX32(0xBAD)
    m["contents"].sort(key=lambda r: (bwise(r["content"]), bwise(r["path"])))
    make_kit("content_directory_mismatch",
             "sec 4.4 step 5: the directory manifest recomputed from the files under `work` "
             "has another sha256; E_KIT_CONTENT with the row's path.", m, dd)
    m = _copy.deepcopy(dm)
    for row in m["files"]:
        if row["path"] == "work/final.bin":
            del row["name_md"]
    make_kit("content_directory_name_md_absent",
             "sec 4.4 step 5: with name_md absent the file's name in the recomputed manifest "
             "is its kit path below the directory, so the digest differs from one computed "
             "with the original name.", m, dd)
    m = _copy.deepcopy(dm)
    m["contents"] = [crow(c["content"], c["path"]) for c in m["contents"]] + [crow(HX32(0xCC), "nowhere")]
    m["contents"].sort(key=lambda r: (bwise(r["content"]), bwise(r["path"])))
    make_kit("rule_contents_directory_without_files",
             "sec 4.3 rule `contents`: a path not listed in files that is a prefix of no listed "
             "path is neither a file original nor a directory original.", m, dd)
    m = _copy.deepcopy(dm)
    m["files"].append(frow("work", hx(sha256(b"x")), 1))
    m["files"].sort(key=lambda r: bwise(r["path"]))
    make_kit("rule_files_prefix_of_another",
             "sec 4.3 rule `files`: a listed path followed by `/` is a prefix of another "
             "listed path, so a file and a directory would share a name.", m, dd)
    m = _copy.deepcopy(full_m)
    m["files"][0]["name_md"] = 7
    make_kit("rule_files_name_md_form", "sec 4.3 rule `files`: name_md is a prose string.", m, full_disk)
    m = _copy.deepcopy(full_m)
    m["files"][0]["name_md"] = "a name with \u0000 control and \U0001f600"
    make_kit("ok_name_md_prose",
             "sec 4.3: name_md carries any scalar value, the file's original name as it was.",
             m, full_disk)
    # -- the directory original's own name: a contents row's name_md -----------
    nm_m, nm_d = assemble(ents, nfiles, root=ROOT, dirs=["work"], dir_names={"work": "Final Cut (v2) masters"})
    make_kit("ok_directory_original_named",
             "sec 4.3 rule `contents` with sec 4.5 and sec 9: the directory original's row carries "
             "name_md, the name the directory had before it was made the kit path `work`; no "
             "predicate reads it, step 5 recomputes the same directory manifest from the files "
             "rows, and the kit verifies.", nm_m, nm_d)
    make_kit("ok_directory_original_named_onefile",
             "sec 4.1a with sec 4.3: the same kit as one file.", nm_m, nm_d, onefile=True)

    def named_row(val):
        mm = _copy.deepcopy(nm_m)
        for row in mm["contents"]:
            if row["path"] == "work":
                row["name_md"] = val
        return mm
    make_kit("rule_contents_name_md_form",
             "sec 4.3 rule `contents`: a row's name_md is a prose string; an int fails the row's rule, "
             "the subject the rule name.", named_row(7), nm_d)
    make_kit("rule_contents_name_md_null",
             "sec 4.3 rule `contents`: null is not a string, so a row whose name_md is null fails the "
             "row's rule.", named_row(None), nm_d)
    make_kit("ok_directory_original_name_md_with_slash",
             "sec 4.3 with sec 9: a contents row's name_md is one name, which sec 4.3 tests nowhere; a "
             "`/` inside it fails no rule, and sec 9 has the reader restore it as one name.",
             named_row("masters/final cut"), nm_d)
    make_kit("ok_directory_original_name_md_redundant",
             "sec 4.3 rule `contents`: a name_md equal to the name the kit path spells is a producer's "
             "redundancy and no rule tests it.", named_row("work"), nm_d)
    mm = _copy.deepcopy(nm_m)
    for row in mm["contents"]:
        if row["path"] == "work":
            row["size"] = 1
    make_kit("rule_contents_row_members_beyond_name_md",
             "sec 4.3 rule `contents`: a row carrying name_md and a member beyond the three fails the "
             "row's rule; the list is closed.", mm, nm_d)
    mm = _copy.deepcopy(full_m)
    for row in mm["contents"]:
        row["name_md"] = "Final Cut.bin"
    make_kit("ok_contents_file_original_name_md",
             "sec 4.3 rule `contents`: name_md on a file original's row is admitted by the row's form and "
             "tested by no rule; a producer omits it, the files row carrying the name, which sec 9 has the "
             "reader take from the files row alone.", mm, full_disk)
    # -- a row's identity is its pair (content, path), whatever its name_md ------
    def twin_rows(second):
        mm = _copy.deepcopy(nm_m)
        work = [r for r in mm["contents"] if r["path"] == "work"][0]
        mm["contents"] = sorted(mm["contents"] + [second(work)], key=lambda r: (bwise(r["content"]), bwise(r["path"])))
        return mm
    make_kit("rule_contents_rows_share_content_and_path_one_named",
             "sec 4.3 rule `contents`: two rows of one content and path, one carrying name_md and one bare, "
             "are two rows of one original; no two rows share both, whatever their name_md.",
             twin_rows(lambda w: crow(w["content"], w["path"])), nm_d)
    mm = twin_rows(lambda w: crow(w["content"], w["path"]))
    mm["contents"] = [r for r in mm["contents"] if "name_md" not in r or r["path"] != "work"] + [r for r in mm["contents"] if "name_md" in r and r["path"] == "work"]
    make_kit("rule_contents_rows_share_content_and_path_bare_first",
             "sec 4.3 rule `contents`: the same two rows of one original with the bare row first; the tie order "
             "changes nothing, the rows share both members.", mm, nm_d)
    make_kit("rule_contents_rows_share_content_and_path_names_differ",
             "sec 4.3 rule `contents`: two rows of one content and path whose name_md differ would give one "
             "directory original two names; the rows share both members, so the row fails.",
             twin_rows(lambda w: crow(w["content"], w["path"], "Final Cut (v2) proofs")), nm_d)
    for slug, val, shown in (("empty", "", "the empty string"), ("dot", ".", "`.`"), ("dotdot", "..", "`..`")):
        make_kit("ok_directory_original_name_md_" + slug,
                 "sec 4.3 with sec 9: a contents row's name_md is data no rule tests; %s as a directory "
                 "original's name fails no rule, and what a platform makes of it is the reader's limit, "
                 "reported and never rewritten." % shown, named_row(val), nm_d)
    # -- the named row against the rest of the row's rule, step 5, and the order of the rules (seat D) --
    mm = named_row("Final Cut (v2) masters")
    for row in mm["contents"]:
        if row["path"] == "work":
            row["content"] = HX32(0xBAD)
    mm["contents"].sort(key=lambda r: (bwise(r["content"]), bwise(r["path"])))
    make_kit("content_named_directory_mismatch",
             "sec 4.4 step 5: a directory original whose row carries name_md is recomputed like any other; "
             "its content is not the digest, so E_KIT_CONTENT with the row's path.", mm, nm_d)
    mm = _copy.deepcopy(nm_m)
    mm["contents"].append(crow(HX32(0xCC), "nowhere", "Nowhere"))
    mm["contents"].sort(key=lambda r: (bwise(r["content"]), bwise(r["path"])))
    make_kit("rule_contents_named_row_unlisted_path",
             "sec 4.3 rule `contents`: a named row's path is still a file original or a directory original; "
             "`nowhere` is neither, so the rule fails.", mm, nm_d)
    mm = _copy.deepcopy(full_m)
    mm["contents"] = [crow(HX32(0xBAD), "art/final.bin", "Final Cut.bin")]
    make_kit("rule_contents_named_file_original_sha_mismatch",
             "sec 4.3 rule `contents`: a named file original's listed sha256 must equal its content; it does "
             "not, so the rule fails.", mm, full_disk)
    mm = _copy.deepcopy(nm_m)
    for row in mm["contents"]:
        if row["path"] == "work/final.bin":
            row["name_md"] = "../Another Cut.bin"
    make_kit("ok_contents_file_original_under_directory_name_md",
             "sec 4.3 rule `contents` with sec 4.4 step 5 and sec 9: the file original `work/final.bin` lies "
             "under the directory original `work` and its contents row carries a name_md of `..` and a name "
             "other than its files row's; the tree rule binds the names step 5 gives files rows, step 5 reads "
             "files rows alone, sec 9 has the reader take the file's name from its files row, and the kit "
             "verifies.", mm, nm_d)
    mm = named_row(7)
    mm["note_md"] = None
    make_kit("rule_order_contents_name_md_before_note",
             "sec 4.3: the rules are decided in order and a row's name_md is tested within `contents`, so "
             "`contents` is named before note_md.", mm, nm_d)
    ao = {"chainId": 1, "registry": HX20(0x9E6), "fromBlock": 100}
    anc = sorted([arow(eid(e_gr), 1, HX32(0xA1)), arow(eid(e_gen), 1, HX32(0xA0)),
                  arow(eid(e_gr), 8453, HX32(0xA2)), arow(eid(e_gr), 1, HX32(0xA3))],
                 key=lambda r: (bwise(r["entry_id"]), r["chainId"], bwise(r["tx"])))
    am, ad = assemble(ents, files, root=ROOT, contents_for={"art/final.bin"}, anchored_on=ao, anchors=anc)
    make_kit("ok_anchored_on_and_anchors",
             "sec 4.3: the two optional members present and well formed; no predicate reads "
             "them.", am, ad)
    make_kit("ok_anchored_on_and_anchors_onefile", "sec 4.1a: the same kit as one file.",
             am, ad, onefile=True)
    for slug, val, why in [("extra_member", dict(ao, note="x"), "exactly three members"),
                           ("missing_member", {"chainId": 1, "registry": HX20(0x9E6)}, "fromBlock absent"),
                           ("registry_hex32", dict(ao, registry=HX32(1)), "registry is hex20"),
                           ("chainid_string", dict(ao, chainId="1"), "chainId is an int"),
                           ("fromblock_negative_shape", dict(ao, fromBlock="0"), "fromBlock is an int"),
                           ("not_object", [1], "an object"),
                           ("null", None, "an absent optional member is spelled by the absent key, and null is a present value of a refused form")]:
        m = _copy.deepcopy(am); m["anchoredOn"] = val
        make_kit("rule_anchoredon_" + slug, "sec 4.3 rule `anchoredOn`: %s." % why, m, ad)
    for slug, val, why in [("extra_member", [dict(anc[0], n=1)], "exactly three members per row"),
                           ("missing_tx", [{"entry_id": eid(e_gr), "chainId": 1}], "tx absent"),
                           ("entry_id_not_listed", [arow(HX32(0xFFFF), 1, HX32(0xA1))], "entry_id is an element of entries"),
                           ("entry_id_form", [arow(HX20(1), 1, HX32(0xA1))], "entry_id is hex32"),
                           ("chainid_string", [arow(eid(e_gr), "1", HX32(0xA1))], "chainId is an int"),
                           ("tx_form", [arow(eid(e_gr), 1, "0x12")], "tx is hex32"),
                           ("duplicate", [anc[0], anc[0]], "rows distinct"),
                           ("unsorted_entry_id", [anc[-1], anc[0]], "sorted by entry_id first"),
                           ("unsorted_chainid", [r for r in anc if r["entry_id"] == eid(e_gr)][::-1], "then by chainId numerically"),
                           ("not_array", {"a": 1}, "an array"),
                           ("row_not_object", [1], "rows are objects")]:
        m = _copy.deepcopy(am); m["anchors"] = val
        make_kit("rule_anchors_" + slug, "sec 4.3 rule `anchors`: %s." % why, m, ad)
    m = _copy.deepcopy(am)
    m["anchors"] = sorted([arow(eid(e_gr), 2, HX32(0xA1)), arow(eid(e_gr), 10, HX32(0xA0))],
                          key=lambda r: (bwise(r["entry_id"]), r["chainId"], bwise(r["tx"])))
    make_kit("ok_anchors_chainid_numeric",
             "sec 4.3 rule `anchors`: chainId sorts numerically, 2 before 10.", m, ad)
    m = _copy.deepcopy(am)
    m["anchors"] = [arow(eid(e_gr), 10, HX32(0xA0)), arow(eid(e_gr), 2, HX32(0xA1))]
    make_kit("rule_anchors_chainid_decimal_order",
             "sec 4.3 rule `anchors`: 10 before 2 is the decimal-string order and not the "
             "numeric one.", m, ad)
    m = _copy.deepcopy(am); m["anchors"] = []
    make_kit("ok_anchors_empty", "sec 4.3: an empty anchors array is lawful.", m, ad)

    # -- step 1: the manifest is absent -------------------------------------
    make_kit("manifest_absent", "sec 4.4 step 1: no pair at manifest.json.",
             no_manifest=True, disk={"files/note.txt": b"orphan\n"})
    make_kit("manifest_absent_empty_dir",
             "sec 4.4 step 1 over an empty directory: the enumeration is empty.",
             no_manifest=True, disk={})

    # -- step 2: the manifest's own rules -----------------------------------
    canon_m = cbytes(J(full_m))
    make_kit("rule_canonical_order", "sec 4.3 rule `canonical`: the members are the right "
                                     "members in the wrong order.",
             raw_manifest=pg.rot_top(canon_m, 3), disk=full_disk)
    make_kit("rule_canonical_utf8", "sec 4.3 rule `canonical`: bytes that are not UTF-8.",
             raw_manifest=canon_m[:5] + b"\xff" + canon_m[6:], disk=full_disk)
    make_kit("rule_canonical_newline",
             "sec 4.3 rule `canonical`: a trailing newline is one JSON text and not the "
             "canonical bytes.", raw_manifest=canon_m + b"\n", disk=full_disk)
    make_kit("rule_members_array", "sec 4.3 rule `members`: the root is not an object.",
             raw_manifest=b"[]")
    make_kit("rule_members_missing",
             "sec 4.3 rule `members`: one of the six required keys is absent.",
             raw_manifest=pg.drop_key(canon_m, "contents"), disk=full_disk)
    make_kit("rule_members_extra",
             "sec 4.3 rule `members`: a key beyond the eight.",
             raw_manifest=pg.add_key(canon_m, "version", 1), disk=full_disk)
    make_kit("rule_members_proofs_is_foreign",
             "sec 4.3 rule `members`: `proofs`, a key of zikaron.kit/1's manifest, is a key "
             "beyond the eight here.", raw_manifest=pg.add_key(canon_m, "proofs", []), disk=full_disk)
    m = dict(full_m); m["spec"] = "zikaron.kit/1"; m["proofs"] = []
    make_kit("rule_members_real_kit1_manifest", "sec 4.3 rule `members` with sec 10.3: a zikaron.kit/1 "
             "manifest carries `proofs`, so this law refuses it at `members`, before `spec`.", m, full_disk)
    m = dict(full_m); m["spec"] = "zikaron.kit/1"
    make_kit("rule_spec_previous", "sec 4.3 rule `spec`: a manifest whose spec names zikaron.kit/1 and whose members are this law's fails at spec here, and zikaron.kit/1 would refuse it too, at members, for lacking proofs; it is read by "
                                   "that law, never by this one.", m, full_disk)
    m = dict(full_m); m["spec"] = "zikaron.kit/3"
    make_kit("rule_spec_next", "sec 4.3 rule `spec`.", m, full_disk)
    m = dict(full_m); m["root"] = "0x" + "a" * 39
    make_kit("rule_root_form", "sec 4.3 rule `root`: null or hex20.", m, full_disk)
    m = dict(full_m); m["root"] = 0
    make_kit("rule_root_int", "sec 4.3 rule `root`: an int is neither.", m, full_disk)
    m = dict(full_m); m["entries"] = list(reversed(full_m["entries"]))
    make_kit("rule_entries_unsorted", "sec 4.3 rule `entries`: sorted bytewise.", m, full_disk)
    m = dict(full_m); m["entries"] = [full_m["entries"][0], full_m["entries"][0]]
    make_kit("rule_entries_duplicate", "sec 4.3 rule `entries`: distinct.", m, full_disk)
    m = dict(full_m); m["entries"] = [HX20(1)]
    make_kit("rule_entries_form", "sec 4.3 rule `entries`: hex32 elements.", m, full_disk)
    m = dict(full_m); m["entries"] = "none"
    make_kit("rule_entries_notarray", "sec 4.3 rule `entries`: an array.", m, full_disk)
    m = dict(full_m); m["files"] = list(reversed(full_m["files"]))
    make_kit("rule_files_unsorted", "sec 4.3 rule `files`: sorted by `path` bytewise.", m, full_disk)
    m = dict(full_m); m["files"] = [full_m["files"][0], full_m["files"][0]]
    make_kit("rule_files_duplicate", "sec 4.3 rule `files`: paths distinct.", m, full_disk)
    for slug, path, why in [
            ("uppercase", "Note.txt", "sec 4.2: segments are drawn from a-z0-9._- so uppercase is outside the charset"),
            ("space", "my note.txt", "sec 4.2: the space is outside the segment charset"),
            ("dotdot", "art/../note.txt", "sec 4.2: no segment is `..`"),
            ("dot", "art/./note.txt", "sec 4.2: no segment is `.`"),
            ("leading_dash", "-note.txt", "sec 4.2: no segment begins with `-`"),
            ("leading_slash", "/note.txt", "sec 4.2: the path begins with no `/`"),
            ("empty_segment", "art//note.txt", "sec 4.2: every segment is one byte or more"),
            ("empty_path", "", "sec 4.2: one or more segments"),
            ("backslash", "art\\note.txt", "sec 4.2: `\\` is outside the segment charset"),
            ("segment_256", "a" * 252 + ".bin", "sec 4.2: a segment of 256 bytes is one over the ceiling"),
            ("path_1025", "/".join(["a" * 204] * 4 + ["a" * 205]), "sec 4.2: 1025 bytes is one over the whole-path ceiling"),
            ("not_a_string", 7, "sec 4.3: `path` is a kit path, and a kit path is a string")]:
        m = dict(full_m); m["files"] = [frow(path, hx(sha256(b"x")), 1)]; m["contents"] = []
        make_kit("rule_files_path_" + slug, why + ".", m, full_disk)
    m = dict(full_m); m["files"] = [{"path": "note.txt", "sha256": hx(sha256(b"x"))}]; m["contents"] = []
    make_kit("rule_files_row_members", "sec 4.3 rule `files`: rows are `{path, sha256, size}` "
                                       "with optionally name_md and nothing else.", m, full_disk)
    m = dict(full_m); m["files"] = [frow("note.txt", hx(sha256(b"x")), "1")]; m["contents"] = []
    make_kit("rule_files_size_form", "sec 4.3 rule `files`: `size` is an int.", m, full_disk)
    m = dict(full_m); m["files"] = [1]; m["contents"] = []
    make_kit("rule_files_row_not_object", "sec 4.3 rule `files`: every row is an object.", m, full_disk)
    m = dict(full_m); m["contents"] = [crow(hx(sha256(b"x")), "nowhere.txt")]
    make_kit("rule_contents_unlisted_path",
             "sec 4.3 rule `contents`: `path` is a listed path or a prefix of one.", m, full_disk)
    m = dict(full_m); m["contents"] = [crow(HX32(0xBAD), "art/final.bin")]
    make_kit("rule_contents_sha_mismatch",
             "sec 4.3 rule `contents`: a file original's listed sha256 must equal content.", m, full_disk)
    m = dict(full_m); m["contents"] = [crow(CH, "art/final.bin"), crow(CH, "art/final.bin")]
    make_kit("rule_contents_duplicate_rows", "sec 4.3 rule `contents`: no two rows share both content and path; "
                                            "here two identical rows.", m, full_disk)
    two_c = sorted([(hx(sha256(fbytes["art/final.bin"])), "art/final.bin"),
                    (hx(sha256(fbytes["note.txt"])), "note.txt")],
                   key=lambda x: (bwise(x[0]), bwise(x[1])))
    m = dict(full_m); m["contents"] = [crow(c, pth) for c, pth in reversed(two_c)]
    make_kit("rule_contents_unsorted", "sec 4.3 rule `contents`: sorted by `content` then `path`.", m, full_disk)
    m = dict(full_m); m["contents"] = [crow(CH, "Art/final.bin")]
    make_kit("rule_contents_path_not_kit_path", "sec 4.3 rule `contents`: `path` is a kit path.", m, full_disk)
    m = dict(full_m); m["note_md"] = None
    make_kit("rule_note_md", "sec 4.3 rule `note_md`: a prose string, and null is not a string.", m, full_disk)
    m = dict(full_m); m["note_md"] = ["a"]
    make_kit("rule_note_md_array", "sec 4.3 rule `note_md`: an array of prose is not prose.", m, full_disk)

    def mut(**kw):
        m = json.loads(json.dumps(full_m))
        m.update(kw)
        return m
    fm = json.loads(json.dumps(full_m))
    make_kit("rule_files_notarray", "sec 4.3 rule `files`: the member is not an array.", mut(files="x"), full_disk)
    r = json.loads(json.dumps(fm["files"])); r[0]["sha256"] = "0x12"
    make_kit("rule_files_sha_form", "sec 4.3 rule `files`: a row's sha256 is not hex32.", mut(files=r), full_disk)
    make_kit("rule_contents_notarray", "sec 4.3 rule `contents`: the member is not an array.", mut(contents=5), full_disk)
    make_kit("rule_contents_row_not_object", "sec 4.3 rule `contents`: every row is an object; an element that "
                                            "is not one fails its row's rule.", mut(contents=[1]), full_disk)
    r = json.loads(json.dumps(fm["contents"])); r[0]["extra"] = 1
    make_kit("rule_contents_row_members",
             "sec 4.3 rule `contents`: a row carrying a member beyond content, path and the "
             "optional name_md; the list is closed.", mut(contents=r), full_disk)
    r = json.loads(json.dumps(fm["contents"])); r[0]["content"] = "0xAB"
    make_kit("rule_contents_content_form", "sec 4.3 rule `contents`: a row's content is not hex32.", mut(contents=r), full_disk)
    m = dict(full_m); m["spec"] = "x"; m["note_md"] = None
    make_kit("rule_order_spec_before_note", "sec 4.3: the rules are decided in order, so spec "
                                            "is named before note_md.", m, full_disk)
    m = _copy.deepcopy(am); m["anchoredOn"] = None; m["anchors"] = "x"
    make_kit("rule_order_anchoredon_before_anchors", "sec 4.3: anchoredOn is decided before anchors.", m, ad)

    # -- steps 3, 4, 6 ------------------------------------------------------
    d2 = dict(full_disk); del d2["entries/" + eid(e_his)[2:] + ".zk1"]
    make_kit("entry_bytes_absent", "sec 4.4 step 3: no pair at entries/<id>.zk1 for a listed id.", full_m, d2)
    d2 = dict(full_disk); d2["entries/" + eid(e_his)[2:] + ".zk1"] = e_gr
    make_kit("entry_bytes_mismatch", "sec 4.4 step 3: the pair is there and its doc_id is not the id.", full_m, d2)
    d2 = dict(full_disk); del d2["files/note.txt"]
    make_kit("file_absent", "sec 4.4 step 4: no pair at files/<path>.", full_m, d2)
    d2 = dict(full_disk); d2["files/note.txt"] = b"other bytes entirely\n"
    make_kit("file_wrong_sha256", "sec 4.4 step 4: the pair's bytes have another sha256.", full_m, d2)
    m = dict(full_m)
    m["files"] = [frow(r["path"], r["sha256"], r["size"] + 1 if r["path"] == "note.txt" else r["size"])
                  for r in full_m["files"]]
    make_kit("file_wrong_size", "sec 4.4 step 4: the sha256 agrees and the length does not.", m, full_disk)
    make_kit("extra_top", "sec 4.4 step 6: a pair whose path is not in the named set.",
             full_m, full_disk, extras=[("readme.txt", b"stray\n")])
    make_kit("extra_in_files", "sec 4.4 step 6: an unlisted file under files/ is an extra like any other.",
             full_m, full_disk, extras=[("files/uncounted.bin", b"stray\n")])
    make_kit("extra_proofs_dir", "sec 4.4 step 6: a zikaron.kit/1 proofs/ file is an extra here.",
             full_m, full_disk, extras=[("proofs/anchor-1.json", b"{}")])
    make_kit("extra_ds_store", "sec 4.4 step 6: a filesystem's own droppings are pairs of the enumeration.",
             full_m, full_disk, extras=[(".DS_Store", b"\x00\x01mac\n")])
    make_kit("extra_smallest_of_several",
             "sec 4.4 step 6: several extras, and the bytewise-smallest path is the subject.",
             full_m, full_disk, extras=[("zz.txt", b"z\n"), ("files/uncounted.bin", b"u\n"), (".DS_Store", b"m\n")])
    make_kit("extra_entries_dir", "sec 4.4 step 6: a byte string under entries/ that no listed id names.",
             full_m, full_disk, extras=[("entries/" + HX32(0xFFFF)[2:] + ".zk1", b"unlisted\n")])
    d2 = dict(full_disk); del d2["files/note.txt"]
    make_kit("order_file_before_extra", "sec 4.4 order: step 4 before step 6.", full_m, d2, extras=[("aaa.txt", b"a\n")])
    d2 = dict(full_disk); del d2["files/note.txt"]; del d2["entries/" + eid(e_gen)[2:] + ".zk1"]
    make_kit("order_entry_before_file", "sec 4.4 order: step 3 before step 4.", full_m, d2)
    m = dict(full_m); m["spec"] = "wrong"
    d2 = dict(full_disk); del d2["entries/" + eid(e_gen)[2:] + ".zk1"]
    make_kit("order_manifest_before_entry", "sec 4.4 order: step 2 before step 3.", m, d2)
    m = _copy.deepcopy(dm)
    for row in m["contents"]:
        if row["path"] == "work":
            row["content"] = HX32(0xBAD)
    m["contents"].sort(key=lambda r: (bwise(r["content"]), bwise(r["path"])))
    make_kit("order_content_before_extra", "sec 4.4 order: step 5 before step 6.", m, dd, extras=[("zz.txt", b"z\n")])
    d2 = dict(dd); del d2["files/plain.txt"]
    make_kit("order_file_before_content", "sec 4.4 order: step 4 before step 5.", m, d2)

    # -- the walk -------------------------------------------------------------
    make_kit("unreadable_fifo", "sec 4.1: an entry that is neither a regular file nor a directory (a named pipe).",
             full_m, full_disk, fifos=["zz-pipe"])
    make_kit("unreadable_symlink", "sec 4.1: the walk fails at the first symbolic link it examines.",
             full_m, full_disk, symlinks=[("zz-link", "manifest.json")])
    make_kit("unreadable_symlink_in_files", "sec 4.1: a symbolic link under files/ carries its relative path as the subject.",
             full_m, full_disk, symlinks=[("files/link.bin", "../manifest.json")])
    make_kit("unreadable_walk_order",
             "sec 4.1: names in bytewise order, a directory walked before the next name, so "
             "the subject is `a/z` and not `a-b`.",
             mfst(None, [], [], [], ""), {}, extras=[("a.b", b"a dot b\n")],
             symlinks=[("a/z", "../manifest.json"), ("a-b", "manifest.json")])
    make_kit("unreadable_beats_manifest_absent",
             "sec 4.1: the walk's failure precedes every test of sec 4.4.",
             no_manifest=True, disk={}, symlinks=[("link", "nowhere")])

    # -- the one-file form, every departure ---------------------------------
    good = onefile_bytes(_items(full_m, full_disk, (), None, False))
    make_kit("onefile_wrong_magic", "sec 4.1a: the first 19 bytes are the 18-byte magic and its line feed; subject `.`.",
             raw_onefile=b"zikaron-kit-file/2\n" + good[19:])
    make_kit("onefile_empty", "sec 4.1a: the empty byte string departs at its first byte.", raw_onefile=b"")
    make_kit("onefile_magic_only", "sec 4.1a: the magic with no item; manifest.json is the first item, and there is none.",
             raw_onefile=KIT_MAGIC)
    make_kit("onefile_first_not_manifest",
             "sec 4.1a: the first item's path is not manifest.json; its path is the subject.",
             raw_onefile=KIT_MAGIC + b"aaa.txt\n2\nhi\n" + good[19:])
    make_kit("onefile_truncated_in_bytes", "sec 4.1a: the declared count runs past the end.",
             raw_onefile=good[:-5])
    make_kit("onefile_missing_final_lf", "sec 4.1a: the last item's trailing line feed is absent.",
             raw_onefile=good[:-1])
    make_kit("onefile_count_leading_zero", "sec 4.1a: a count with a leading zero departs from the layout.",
             raw_onefile=good.replace(b"\nmanifest.json\n", b"\nmanifest.json\n0", 1))
    make_kit("onefile_count_not_decimal", "sec 4.1a: a count that is not decimal digits.",
             raw_onefile=KIT_MAGIC + b"manifest.json\nxx\n" + good[19:])
    make_kit("onefile_path_not_kit_path", "sec 4.1a: an item path that is not a kit path; subject `.`.",
             raw_onefile=good + b"Bad Path\n1\nx\n")
    make_kit("onefile_path_not_utf8", "sec 4.1a: an item path that is not UTF-8; subject `.`.",
             raw_onefile=good + b"\xff\n1\nx\n")
    make_kit("onefile_no_count_line", "sec 4.1a: the bytes end at a path with no line feed, so the "
             "path line is not read in full; the subject is `.`.",
             raw_onefile=good + b"zz.txt")
    make_kit("onefile_count_unterminated", "sec 4.1a: a path line read in full, then a count "
             "with no line feed after it; the subject is that path.",
             raw_onefile=good + b"zz.txt\n1")
    full_m_bytes = cbytes(J(full_m))
    mlen = len(full_m_bytes)
    make_kit("onefile_count_shorter_than_bytes", "sec 4.1a: a count shorter than the item's bytes puts a byte "
             "other than a line feed where the item's line feed is due; the subject is the item.",
             raw_onefile=KIT_MAGIC + b"manifest.json\n" + str(mlen - 1).encode() + b"\n" + full_m_bytes + b"\n")
    items_s = _items(full_m, full_disk, (), None, False)
    keys_s = sorted((k for k in items_s if k != "manifest.json"), key=bwise)
    def layout(order, items):
        out = KIT_MAGIC + b"manifest.json\n" + str(len(items["manifest.json"])).encode() + b"\n" + items["manifest.json"] + b"\n"
        for pth in order:
            out += pth.encode() + b"\n" + str(len(items[pth])).encode() + b"\n" + items[pth] + b"\n"
        return out
    make_kit("onefile_out_of_order", "sec 4.1a: items after the manifest in other than bytewise order of their paths.",
             raw_onefile=layout(list(reversed(keys_s)), items_s))
    make_kit("onefile_repeated_path", "sec 4.1a: two items share a path.",
             raw_onefile=layout(keys_s + [keys_s[-1]], items_s))
    make_kit("onefile_manifest_repeated", "sec 4.1a: manifest.json a second time, a repeated path.",
             raw_onefile=good + b"manifest.json\n2\n{}\n")
    make_kit("onefile_zero_count", "sec 4.1a: a count of zero is written `0`, and an empty item is lawful.",
             raw_onefile=layout(keys_s + ["zz.txt"], dict(items_s, **{"zz.txt": b""})))
    make_kit("onefile_item_with_newlines", "sec 4.1a: an item's bytes may hold line feeds; the count decides.",
             raw_onefile=layout(keys_s + ["zz.txt"], dict(items_s, **{"zz.txt": b"a\nb\n\n"})))
    make_kit("onefile_extra_after_end", "sec 4.1a: bytes after the last item's line feed must be another item; a lone line feed is not.",
             raw_onefile=good + b"\n")
    make_kit("onefile_binary_junk", "sec 4.1a over bytes that are no layout at all.",
             raw_onefile=bytes(range(256)))
    make_kit("onefile_manifest_absent_impossible_but_extra",
             "sec 4.1a: an item whose path the manifest does not name is an extra under sec 4.4.",
             raw_onefile=layout(keys_s + ["zz.txt"], dict(items_s, **{"zz.txt": b"z"})))

    # -- randomized valid kits, half of them one-file -----------------------
    for i in range(24):
        n_e = rng.randrange(0, 4)
        es = []
        prev = None
        for k in range(n_e):
            if k == 0:
                es.append(pg.make_entry(K("author"), "genesis", 0, None, {"statement_md": pg.rand_prose(rng)}))
            else:
                es.append(pg.make_entry(K("author"), rng.choice(["history", "annotation"]), k, prev,
                                        pg.random_body(rng, "history") if k % 2 else {"note_md": pg.rand_prose(rng)}))
            prev = eid(es[-1])
        n_f = rng.randrange(0, 5)
        fs = []
        used = set()
        for k in range(n_f):
            segs = [("".join(rng.choice("abcdefghijklmnopqrstuvwxyz0123456789._-") for _ in range(rng.randrange(1, 9))))
                    for _ in range(rng.randrange(1, 3))]
            segs = [sg if not sg.startswith("-") else "a" + sg[1:] for sg in segs]
            segs = [sg if sg not in (".", "..") else "a" for sg in segs]
            pth = "/".join(segs)
            if pth in used or not kit_path_ok(pth) or any(u.startswith(pth + "/") or pth.startswith(u + "/") for u in used):
                continue
            used.add(pth)
            entry = (pth, bytes(rng.randrange(256) for _ in range(rng.randrange(0, 40))))
            if rng.random() < 0.4:
                entry = entry + (pg.rand_prose(rng, rng.randrange(1, 12)),)
            fs.append(entry)
        rt = None if rng.random() < 0.3 else HX20(rng.getrandbits(160))
        cf = set(f[0] for f in fs if rng.random() < 0.6)
        dirs = sorted(set(f[0].split("/")[0] for f in fs if "/" in f[0] and rng.random() < 0.5))
        dirs = [d for d in dirs if d not in used]
        dn = dict((d, pg.rand_prose(rng, rng.randrange(1, 12))) for d in dirs if rng.random() < 0.5)
        ao = None if rng.random() < 0.5 else {"chainId": rng.randrange(1, 1 << 16), "registry": HX20(rng.getrandbits(160)), "fromBlock": rng.randrange(0, 1 << 20)}
        anc = None
        if es and rng.random() < 0.5:
            anc = sorted(set((eid(rng.choice(es)), rng.randrange(1, 3), HX32(rng.getrandbits(256))) for _ in range(rng.randrange(1, 4))),
                         key=lambda r: (bwise(r[0]), r[1], bwise(r[2])))
            anc = [arow(*r) for r in anc]
        mm, ddk = assemble(es, fs, root=rt, note_md=pg.rand_prose(rng), contents_for=cf, dirs=dirs,
                           anchored_on=ao, anchors=anc, dir_names=dn)
        make_kit("rand_%02d" % i, "randomized lawful kit: %d entries, %d files, %d directory originals, %d of them named."
                 % (len(es), len(fs), len(dirs), len(dn)), mm, ddk, onefile=(i % 2 == 1))
    return {"e_gen": e_gen, "e_his": e_his, "e_gr": e_gr, "CH": CH, "fbytes": fbytes, "ROOT": ROOT}


def build_packages(rng, t):
    ROOT = AD("author")
    WORK = b"the work, as delivered"
    WK = hx(sha256(WORK))
    TERMS = b"the terms of the grant\n"
    TH = hx(sha256(TERMS))
    g0 = pg.make_entry(K("author"), "genesis", 0, None, {"statement_md": "an issuer"})
    h1 = pg.make_entry(K("author"), "history", 1, eid(g0), {"content": WK, "mode": {"mark": "bytes-sha256/1", "toolchain": HX32(0x7001)}})
    gr = pg.make_entry(K("author"), "grant", 2, eid(h1), {"grantee": AD("r1"), "work": WK, "terms": TH, "history": eid(h1)})
    ann = pg.make_entry(K("author"), "annotation", 3, eid(gr), {"note_md": "after"})
    code = payload([gr])
    ledger = [g0, h1, gr, ann]

    def pkg(entries=ledger, files_extra=(), root=ROOT, contents_for=None, dirs=(), code_bytes=code,
            terms=(("terms/terms.txt", TERMS),), publish=True, work_files=(("work/delivered.bin", WORK, "The Work.bin"),),
            dir_names=None):
        fs = [("zikaron-grant.txt", code_bytes)] + list(terms) + list(work_files) + list(files_extra)
        if publish:
            fs.append(("publish.txt", b"https://example.test/ledger\n"))
        return assemble(entries, fs, root=root, contents_for=contents_for if contents_for is not None else {"work/delivered.bin"}, dirs=dirs,
                        dir_names=dir_names)

    m, d = pkg()
    make_package("ok_onefile", "sec 5.2 step 8: a package as one file; kit, code, root, every hop carried, "
                               "the original's digest the work, the terms document the hop's terms.", m, d)
    make_package("ok_directory", "sec 5.2: the same package as a directory.", m, d, onefile=False)
    m2, d2 = pkg(root=None, entries=[g0, h1, pg.make_entry(K("author"), "grant", 2, eid(h1), {"grantee": AD("r1"), "work": WK, "terms": TH, "ledger": ROOT}), ann],
                 code_bytes=payload([pg.make_entry(K("author"), "grant", 2, eid(h1), {"grantee": AD("r1"), "work": WK, "terms": TH, "ledger": ROOT})]))
    make_package("ok_root_from_ledger_member", "sec 5.2 step 4: the manifest's root is null and the first hop's "
                                               "`ledger` is hex20; GP_OK carries it as the root.", m2, d2)
    m3, d3 = pkg(root=None)
    make_package("root_absent", "sec 5.2 step 4: root null and the first hop carries no ledger; E_GP_ROOT.", m3, d3)
    m4, d4 = pkg(terms=())
    make_package("ok_no_terms_file", "sec 5.1: terms documents are optional; the hop's terms list is empty.", m4, d4)
    m5, d5 = pkg(work_files=(), contents_for=set())
    make_package("ok_no_originals", "sec 5.1: contents rows are optional; a package may carry the grant alone.", m5, d5)
    m6, d6 = pkg(publish=False)
    make_package("ok_no_publish", "sec 5.1: publish.txt is optional.", m6, d6)
    m7, d7 = pkg(work_files=(("work/a.bin", b"not the work", "a.bin"),), contents_for={"work/a.bin"})
    make_package("content_not_the_work", "sec 5.2 step 6: a contents row whose content differs from the hops' work (one value across the payload).", m7, d7)
    m26, d26 = pkg()
    make_package("kit_extra_under_terms", "sec 5.3: the subject of a kit verdict is written as the step names it; "
                 "an unlisted pair under files/terms/ is E_KIT_EXTRA with its whole enumeration path.",
                 m26, d26, extras=[("files/terms/zzz.txt", b"unlisted")])
    m27, d27 = pkg()
    for row in m27["files"]:
        if row["path"] == "terms/terms.txt":
            row["sha256"] = HX32(0xBAD)
    make_package("kit_file_under_terms", "sec 5.3: E_KIT_FILE carries the files row's path, `terms/terms.txt`, "
                 "which has no files/terms/ prefix to strip.", m27, d27)
    m8, d8 = pkg(terms=(("terms/terms.txt", TERMS), ("terms/other.txt", b"some other document")))
    make_package("terms_unmatched", "sec 5.2 step 7: a pair under files/terms/ whose sha256 is no hop's terms; "
                                    "the unmatched pair's path, without the prefix, is the subject (one such pair here).", m8, d8)
    m9, d9 = pkg(terms=(("terms/a.txt", TERMS), ("terms/b.txt", TERMS)))
    make_package("ok_two_terms_files_one_hop", "sec 5.3: every terms path whose sha256 is the hop's terms is listed, in bytewise order.", m9, d9)
    m10, d10 = pkg(entries=[g0, h1, ann])
    make_package("hop_uncarried", "sec 5.2 step 5: the payload's hop is the entry_id of no pair under entries/.", m10, d10)
    m11, d11 = pkg(files_extra=(), code_bytes=b"zikaron-grant:" + b"*")
    make_package("badge_b64", "sec 5.2 step 3: the badge payload fails to decode; the badge token with its index.", m11, d11)
    bad_up2 = pg.make_entry(K("r1"), "grant", 1, HX32(0x11), {"grantee": AD("r2"), "work": WK, "terms": TH, "upstream": HX32(0xDEAD)})
    m21, d21 = pkg(entries=[g0, h1, gr, ann, bad_up2], code_bytes=payload([gr, bad_up2]))
    make_package("badge_link_index", "sec 5.2 step 3 with sec 5.3: a badge whose link fails at index 1; the subject is "
                                     "that index.", m21, d21)
    v1n = E1("author", "grant", 2, eid(h1), {"grantee": AD("r1"), "work": WK, "terms": TH, "upstream": None})
    m22, d22 = pkg(entries=[g0, h1, v1n, ann], code_bytes=payload([v1n]))
    make_package("badge_incomplete_null_subject", "sec 5.2 step 3 with sec 5.3: E_BADGE_INCOMPLETE carries no index, so the "
                                                  "subject is null.", m22, d22)
    cap_g = padded_grant(K("author"), 2205, grantee=AD("r1"), work=WK, terms=TH)
    m23, d23 = pkg(entries=[g0, h1, cap_g, ann], code_bytes=payload([cap_g]))
    make_package("badge_cap_null_subject", "sec 5.2 step 3 with sec 5.3: E_BADGE_CAP carries no index; the subject is "
                                           "null.", m23, d23)
    v1l = E1("author", "grant", 2, eid(h1), {"grantee": AD("r1"), "work": WK, "terms": TH, "ledger": "not-an-address"})
    m24, d24 = pkg(root=None, entries=[g0, h1, v1l, ann], code_bytes=payload([v1l]))
    make_package("root_v1_ledger_not_hex20", "sec 5.2 step 4: the manifest's root is null and the first hop, a zikaron/1 "
                                             "grant whose law reads `ledger` as data, carries one that is no hex20; "
                                             "E_GP_ROOT.", m24, d24)
    m25, d25 = pkg()
    make_package("onefile_unreadable", "sec 4.1a: a package file the reader cannot read in full is E_KIT_UNREADABLE "
                                       "with subject `.` (mode 0000; regenerate rather than copy).", m25, d25, chmod_file=0)
    m12, d12 = pkg(code_bytes=payload([ann]))
    make_package("badge_type", "sec 5.2 step 3: the payload's entry is not a grant; E_BADGE_TYPE with index 0.", m12, d12)
    m13b, d13b = pkg(code_bytes=b"zikaron-grant:" + b64u(b"{}").encode())
    make_package("badge_entry", "sec 5.2 step 3: the payload's segment decodes to bytes that fail accept; "
                                "E_BADGE_ENTRY with index 0.", m13b, d13b)
    m13, d13 = pkg(code_bytes=b"zikaron-badge:" + b64u(gr).encode())
    make_package("badge_prefix", "sec 5.2 step 3: E_BADGE_PREFIX carries no index; subject null.", m13, d13)
    m14, d14 = pkg()
    del d14["files/zikaron-grant.txt"]
    m14["files"] = [r for r in m14["files"] if r["path"] != "zikaron-grant.txt"]
    make_package("no_code", "sec 5.2 step 2: no pair at files/zikaron-grant.txt.", m14, d14)
    m15, d15 = pkg()
    m15["spec"] = "zikaron.kit/1"
    make_package("kit_fails_manifest", "sec 5.2 step 1: the kit's own verdict, with its subject, is the package's.", m15, d15)
    m16, d16 = pkg()
    del d16["files/publish.txt"]
    make_package("kit_fails_file", "sec 5.2 step 1: a kit failure at step 4 is the package's verdict.", m16, d16)
    make_package("kit_unreadable", "sec 5.2 step 1 with sec 4.1a: a departure from the one-file layout.",
                 raw_onefile=b"not a kit")
    make_package("kit_manifest_absent", "sec 5.2 step 1: E_KIT_MANIFEST_ABSENT, whose subject is null.",
                 disk={"files/zikaron-grant.txt": code}, no_manifest=True, onefile=False)
    # a two-hop chain: the author's grant, then the grantee's grant onward
    gr2 = pg.make_entry(K("r1"), "grant", 1, HX32(0x11), {"grantee": AD("r2"), "work": WK, "terms": HX32(0x7F), "upstream": eid(gr)})
    g0b = pg.make_entry(K("r1"), "genesis", 0, None, {"statement_md": "the grantee's ledger"})
    code2 = payload([gr, gr2])
    T2 = b"the second hop's terms"
    gr2b = pg.make_entry(K("r1"), "grant", 1, eid(g0b), {"grantee": AD("r2"), "work": WK, "terms": hx(sha256(T2)), "upstream": eid(gr)})
    code2b = payload([gr, gr2b])
    m17, d17 = pkg(entries=ledger + [g0b, gr2b], code_bytes=code2b, terms=(("terms/first.txt", TERMS), ("terms/second.txt", T2)))
    make_package("ok_two_hops", "sec 5.1/5.3: two hops, the second issuer's ledger carried too, and each hop's terms "
                                "paths listed per hop.", m17, d17)
    m18, d18 = pkg(entries=ledger + [g0b], code_bytes=code2b, terms=(("terms/first.txt", TERMS), ("terms/second.txt", T2)))
    make_package("two_hops_second_uncarried", "sec 5.2 step 5: the first uncarried hop, in payload order, is the subject.", m18, d18)
    # -- round 5 fold --------------------------------------------------------
    firstU = secondU = None
    for k_ in range(256):
        c1 = pg.make_entry(K("author"), "grant", 2, eid(h1), {"grantee": AD("r2"), "work": WK, "terms": TH, "history": eid(h1), "scope_md": "first %d" % k_})
        c2 = pg.make_entry(K("r2"), "grant", 1, HX32(0x11), {"grantee": AD("r3"), "work": WK, "terms": TH, "upstream": eid(c1)})
        if eid(c1) > eid(c2):
            firstU, secondU = c1, c2; break
    assert firstU is not None
    m31, d31 = pkg(entries=[g0, h1, ann], code_bytes=payload([firstU, secondU]))
    make_package("two_hops_both_uncarried_payload_order", "sec 5.2 step 5: the byte links hold, both hops are uncarried, and "
                 "the payload order is the reverse of the id order; the subject is the first hop in payload order, the "
                 "bytewise-larger id.", m31, d31)
    z2 = None
    for k_ in range(4096):
        cand_ = b"not z2 %d" % k_
        if sha256(cand_) < sha256(b"not z1"):
            z2 = cand_; break
    assert z2 is not None
    m32, d32 = pkg(work_files=(("work/z1.bin", b"not z1", "z1.bin"), ("work/z2.bin", z2, "z2.bin")), contents_for={"work/z1.bin", "work/z2.bin"})
    make_package("two_contents_rows_mismatch", "sec 5.2 step 6: two contents rows whose content differs from the work, the "
                 "row with the smaller content carrying the larger path; the subject is the first such row in contents "
                 "order, work/z2.bin, where the bytewise-smallest path would be work/z1.bin.", m32, d32)
    m33, d33 = pkg(terms=(("terms/terms.txt", TERMS), ("terms/zz.txt", b"zz"), ("terms/bb.txt", b"bb")))
    make_package("two_terms_unmatched_smallest_path", "sec 5.2 step 7: two pairs under files/terms/ match no hop's terms; the "
                 "bytewise-smallest path, without the prefix, is the subject.", m33, d33)
    # a v1 grant in a continued ledger, carried with both segments
    v1g = E1("author", "genesis", 0, None, {"statement_md": "under the first law"})
    v1h = E1("author", "history", 1, eid(v1g), {"content": WK, "mode": {"mark": "bytes-sha256/1", "toolchain": HX32(0x7001)}})
    v1gr = E1("author", "grant", 2, eid(v1h), {"grantee": AD("r1"), "work": WK, "terms": TH})
    v1m = E1("author", "succession", 3, eid(v1gr), {"to": ROOT, "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    g2c = pg.make_entry(K("author"), "genesis", 0, None, {"statement_md": "continued", "continues": {"root": ROOT, "entry_id": eid(v1m)}})
    m19, d19 = pkg(entries=[v1g, v1h, v1gr, v1m, g2c], code_bytes=payload([v1gr]))
    make_package("ok_v1_grant_continued_ledger", "sec 1/5: the badge carries a zikaron/1 grant, accepted by spec, and the "
                                                 "package carries both segments of the continued ledger.", m19, d19)
    # a package with a directory original whose work digest is the directory manifest
    DW = [("work/a.txt", b"part a", "A.txt"), ("work/b.txt", b"part b", "sub/B.txt")]
    ddig = dirman_digest([(f[2], hx(sha256(f[1]))) for f in DW])
    grd = pg.make_entry(K("author"), "grant", 2, eid(h1), {"grantee": AD("r1"), "work": ddig, "terms": TH})
    m20, d20 = pkg(entries=[g0, h1, grd], code_bytes=payload([grd]), work_files=DW, contents_for=set(), dirs=["work"])
    make_package("ok_directory_original", "sec 5.2 step 6 with sec 4.5: a directory original whose manifest digest is "
                                          "the work the grant is over.", m20, d20)
    m28, d28 = pkg(entries=[g0, h1, grd], code_bytes=payload([grd]), work_files=DW, contents_for=set(), dirs=["work"],
                   dir_names={"work": "The Work (masters)"})
    make_package("ok_directory_original_named", "sec 5.2 step 8 with sec 5.3: the directory original's row carries "
                                                "name_md; GP_OK's contents rows carry each row's content and path, the "
                                                "name staying in the manifest.", m28, d28)
    m29 = json.loads(json.dumps(m28))
    for row in m29["contents"]:
        row["name_md"] = 3
    make_package("kit_manifest_contents_name_md_form", "sec 5.2 step 1: the kit verdict E_KIT_MANIFEST with its rule "
                                                      "`contents` as the subject, the directory row's name_md being no string.", m29, d28)
    m30 = json.loads(json.dumps(m28))
    m30["contents"] = sorted(m30["contents"] + [crow(r["content"], r["path"]) for r in m30["contents"]],
                             key=lambda r: (bwise(r["content"]), bwise(r["path"])))
    make_package("kit_manifest_contents_rows_share_content_and_path", "sec 5.2 step 1 with sec 4.3: a named row and a bare "
                                                                     "row of one content and path fail `contents`, so the package "
                                                                     "never reaches step 8 with two rows of one original.", m30, d28)


# ==========================================================================
# ledger and audit-input construction for the reading predicates
# ==========================================================================

REG = HX20(0x9E6)


def basis(chains=(), *_ignored):
    return {"chains": list(chains)}


def chainobj(cid=1, fb=0, tb=1000, regs=(REG,), senders=()):
    # parent sec 9.4: registries and senders are bytewise ascending
    return {"chainId": cid, "fromBlock": fb, "toBlock": tb,
            "registries": sorted(set(regs)), "senders": sorted(set(senders))}


def anchor(h, sender, cid=1, bn=100, ts=1700000000, tx=None, verdict="counted"):
    return {"chainId": cid, "blockNumber": bn, "blockTimestamp": ts,
            "tx": tx if tx else HX32(0xA00000 + cid * 4096 + bn), "sender": sender,
            "hash": h, "verdict": verdict}


def AINP(root, pile, anchors=(), bas=None):
    return {"root": root, "pile": [hx(b) for b in pile], "anchors": list(anchors),
            "basis": bas if bas is not None else basis()}


def E1(keytag, etype, seq, prev, body):
    """A zikaron/1 entry (the continued segment's)."""
    return pg1.make_entry(K(keytag), etype, seq, prev, body)


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
# the reversed twin of an audit input
# ==========================================================================



def _reversed_arrays(inp):
    return dict((k, (list(reversed(v)) if isinstance(v, list) else v)) for k, v in inp.items())


# ==========================================================================
# checks/ : sec 7.2
# ==========================================================================


def _reversed_notices(nts):
    if not isinstance(nts, dict):
        return nts
    return dict((k, (list(reversed(v)) if isinstance(v, list) else v)) for k, v in nts.items())


def CK(slug, g, inp, now, note, expect_verdict=None, twin=True, nts=None):
    if twin and ((isinstance(inp, dict) and _reversed_arrays(inp) != inp)
                 or (isinstance(nts, dict) and _reversed_notices(nts) != nts)):
        CK(slug + "_reversed", g, _reversed_arrays(inp) if isinstance(inp, dict) else inp, now,
           "input arrays reversed: " + note, expect_verdict, twin=False, nts=_reversed_notices(nts))
    i = seqno("checks")
    base = "checks/c%03d_%s" % (i, slug)
    gp = W(base + ".grant.zk1", g)
    args = ["grant-check", gp]
    ap = None
    np_ = None
    if inp is not None:
        ap = WJ(base + ".audit.json", inp)
        args += ["--audit", ap]
    if nts is not None:
        np_ = WJ(base + ".notices.json", nts)
        args += ["--notices", np_]
    if now is not None:
        args += ["--now", str(now)]
    pred = grant_check(g, inp, now, nts)
    extra = {"audit_path": ap, "notices_path": np_, "now": now}
    if expect_verdict is not None and expect_verdict != pred["verdict"]:
        extra["disagreement"] = True
        NOTES.append("checks %s: hand=%s code=%s" % (base, expect_verdict, pred["verdict"]))
    CASE("checks", gp, "grant-check", args, note, pred, **extra)
    return gp


def CK_order(slug, g, inp, now, note):
    """A grant check whose --now precedes --audit: the harness lets option tokens
    stand in any relative order after the positional argument."""
    i = seqno("checks")
    base = "checks/c%03d_%s" % (i, slug)
    gp = W(base + ".grant.zk1", g)
    ap = WJ(base + ".audit.json", inp)
    args = ["grant-check", gp, "--now", str(now), "--audit", ap]
    pred = grant_check(g, inp, now)
    CASE("checks", gp, "grant-check", args, note, pred, audit_path=ap, notices_path=None, now=now)
    return gp


def CK_raw(slug, g, raw, note):
    """A grant check whose audit file is written as given (bytes that are no
    audit input); the prediction is the check over an invalid input."""
    i = seqno("checks")
    base = "checks/c%03d_%s" % (i, slug)
    gp = W(base + ".grant.zk1", g)
    ap = W(base + ".audit.json", raw)
    args = ["grant-check", gp, "--audit", ap]
    pred = grant_check(g, None, None)
    CASE("checks", gp, "grant-check", args, note, pred, audit_path=ap, notices_path=None, now=None)
    return gp


def CKN_raw(slug, g, inp, raw, note):
    """A grant check whose notices file is written as given (bytes that are no
    notice input); the prediction is the check over an invalid notice input."""
    i = seqno("checks")
    base = "checks/c%03d_%s" % (i, slug)
    gp = W(base + ".grant.zk1", g)
    ap = WJ(base + ".audit.json", inp)
    np_ = W(base + ".notices.json", raw)
    args = ["grant-check", gp, "--audit", ap, "--notices", np_]
    pred = grant_check(g, inp, None, None)
    CASE("checks", gp, "grant-check", args, note, pred, audit_path=ap, notices_path=np_, now=None)
    return gp


def build_checks(rng, t):
    global NONRES_R
    if NONRES_R is None:
        NONRES_R = nonresidue_r()
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
        ("E_VALUE_CHARSET", pg.sub(canon, b'"spec":"zikaron/2"',
                                   b'"spec":"\\u0001zikaron/2"')),
        ("E_NOT_CANONICAL", pg.rot_top(canon, 2)),
        ("E_ENVELOPE", b"[]"),
        ("E_ENVELOPE_MISSING", pg.drop_key(canon, "prev")),
        ("E_ENVELOPE_CLOSED", pg.add_key(canon, "extra", 1)),
        ("E_SPEC", pg.set_key(canon, "spec", "zikaron/3")),
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
           "sec 7.2 check 1: `g` fails `accept` with the parent's %s. A FAIL here leaves "
           "checks 2 to 6 UNKNOWN and the verdict is FAIL." % tok, "FAIL")
    _gen0, _g0, _t20 = issuer("iss", gbody(AD("r4"), HX32(0xC0DE)))
    CK("audit_input_not_object", _g0, [1, 2], None,
       "sec 7.1: an audit input that is not an object; I' cannot be formed, the input is "
       "invalid under the parent's sec 9.4, and the checks that need it are UNKNOWN.")
    CK("audit_input_pile_not_array", _g0, {"root": AD("iss"), "pile": "0x00", "anchors": [],
                                          "basis": basis()},
       None,
       "sec 7.1: an audit input whose pile is not an array; I' cannot be formed.")
    CK("c1_not_a_grant", E("iss", "history", 1, prev1,
                           {"content": WK, "mode": {"mark": "hand",
                                                    "toolchain": HX32(1)}}),
       None, None,
       "sec 7.2 check 1: an accepted entry that is not of type grant, carrying reason "
       "NOT_A_GRANT.", "FAIL")
    CK("c1_not_a_grant_with_audit",
       E("iss", "annotation", 2, eid(G), {"note_md": "a note, not a grant"}),
       AINP(IS, [gen, ann], A_G, bas=COV), 1700000500,
       "sec 7.2 check 1: a FAIL at check 1 leaves every later check UNKNOWN even where the "
       "audit input would have decided them, and `basis` still carries the basis of I.",
       "FAIL")

    # ---- the verdicts -----------------------------------------------------
    CK("green_anchored", G, AINP(IS, [gen, ann], A_G, bas=COV), 1700000500,
       "sec 7.2: every check passes. The grant is a ledger entry of I' (sec 7.1 adds it "
       "to the pile), it is anchored, the basis is covering, the label is COMPLETE and no "
       "revocation names it.", "GREEN")
    CK("green_no_now", G, AINP(IS, [gen, ann], A_G, bas=COV), None,
       "sec 7.2 check 5: with no `window` the check passes without reading `now`, so a "
       "caller with no trusted time still reaches GREEN.", "GREEN")
    CK("green_bounded_by_later_anchor", G,
       AINP(IS, [gen, ann], [anchor(eid(ann), IS, ts=1700000700)], bas=COV), None,
       "sec 7.2 check 4 through sec 6.2: the anchor names a later entry from which the "
       "grant is reachable.", "GREEN")
    genw, GW, annw = issuer("iss", gbody(AD("r1"), WK, window={"from": 1000, "to": 2000}))
    IW = AINP(IS, [genw, annw], [anchor(eid(GW), IS, ts=1700000000)], bas=COV)
    CK("c5_window_at_from", GW, IW, 1000,
       "sec 7.2 check 5: the window test is inclusive at both ends.", "GREEN")
    CK("c5_window_at_to", GW, IW, 2000,
       "sec 7.2 check 5: inclusive at the upper end too.", "GREEN")
    CK("c5_window_inside", GW, IW, 1500, "sec 7.2 check 5: inside the window.", "GREEN")
    CK("c5_window_before", GW, IW, 999,
       "sec 7.2 check 5: `now` below `from`.", "FAIL")
    CK("c5_window_after", GW, IW, 2001,
       "sec 7.2 check 5: `now` above `to`.", "FAIL")
    CK("c5_window_now_null", GW, IW, None,
       "sec 7.2 check 5: a window with no trusted time is UNKNOWN, never a pass and never "
       "a failure.", "PARTIAL")

    CK("no_audit_input", G, None, 1700000500,
       "sec 7.1: the caller holds no audit input for the issuer, so checks 2, 3, 4 and 6 "
       "are UNKNOWN and `basis` is null.", "PARTIAL")
    CK("no_audit_no_now", G, None, None,
       "sec 7.1 and check 5: nothing but the bytes; only checks 1 and 5 can be decided.",
       "PARTIAL")
    CK("invalid_audit_input", G,
       {"root": IS, "pile": [hx(gen)], "anchors": [], "basis": {"chains": [], "extra": 1}},
       1700000500,
       "sec 7.2: an invalid audit input is read exactly as a null one, and `basis` is "
       "null.", "PARTIAL")

    # ---- check 2 ----------------------------------------------------------
    tw1 = E("iss", "annotation", 2, eid(G), {"note_md": "left"})
    tw2 = E("iss", "annotation", 2, eid(G), {"note_md": "right"})
    CK("c2_broken_chain", G, AINP(IS, [gen, tw1, tw2], A_G, bas=COV), 1700000500,
       "sec 7.2 check 2: the report of I' labels BROKEN_CHAIN (one EQUIVOCATION, hard), "
       "which leaves checks 3, 4 and 6 UNKNOWN while check 5 is still decided.", "FAIL")

    # ---- check 3 ----------------------------------------------------------
    Gs = E("stranger", "grant", 1, eid(gen), body)
    CK("c6_record_of_excluded_bytes", G,
       AINP(IS, [gen, ann, Gs], A_G + [anchor(eid(Gs), IS, bn=11, ts=1700000001)], bas=COV),
       1700000500,
       "sec 6.2 with the parent's sec 8.5: a counted record of a stranger's bytes anchors no "
       "ledger entry and bounds nothing, and its hash is MISSING (`have` holds ledger "
       "entries alone) so the label is GAPS: check 4 passes on the grant's own record, "
       "check 6 is UNKNOWN, PARTIAL.", "PARTIAL")
    CK("c3_author_outside_lineage", Gs, AINP(IS, [gen], [], bas=COV), None,
       "sec 7.2 check 3: `g` is not a ledger entry of I' because its author is outside the "
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
       "sec 7.2 check 3: the grant carries an AUTHORITY_MISMATCH the walk recorded soft, "
       "because a gap could hide a succession. Soft is UNKNOWN here, since a hard one would "
       "have labelled the ledger broken at check 2.", "PARTIAL")

    rev7 = E("iss2", "revocation", 7, eid(Gsoft), {"grant": eid(Gsoft)})
    CK("c3_soft_authority_check6_still_decided", Gsoft,
       AINP(IS, [LA[0], LA[1], rev7], [anchor(eid(Gsoft), IS, ts=1700000000)],
            bas=cover([IS, AD("iss2")])),
       None,
       "sec 7.2 checks 3 and 6: the soft AUTHORITY_MISMATCH leaves check 3 UNKNOWN without gating "
       "checks 4 and 6; the key in office revokes the grant at seq 7, so check 6 FAILS and the verdict "
       "is FAIL, where a reader gating check 6 on any UNKNOWN from check 3 answers PARTIAL.", "FAIL")

    # ---- check 4 ----------------------------------------------------------
    CK("c4_unanchored_complete_covering", G,
       AINP(IS, [gen, ann], [anchor(eid(gen), IS, ts=1700000000)], bas=COV), None,
       "sec 7.2 check 4: the grant is not anchored (an anchor of an earlier entry bounds "
       "nothing later), the basis is covering and the label is COMPLETE, so absence "
       "convicts.", "FAIL")
    CK("c4_not_covering_chains_empty", G,
       AINP(IS, [gen, ann], [], bas=basis()), None,
       "sec 7.2 check 4: a basis whose `chains` array is empty is not covering, and the "
       "scan's silence convicts nobody.", "PARTIAL")
    CK("c4_not_covering_registries_empty", G,
       AINP(IS, [gen, ann], [], bas=basis([chainobj(1, 0, 1000, (), [IS])])), None,
       "sec 7.2 check 4: a chains object with an empty `registries` array is not covering.",
       "PARTIAL")
    LS2, LS2I = build_seq([("iss", "genesis", 0, GEN),
                           ("iss", "grant", 1, body),
                           ("iss", "succession", 2, SUC),
                           ("iss2", "annotation", 3, {"note_md": "new hand"})])
    G2 = LS2[1]
    CK("c4_not_covering_sender_absent", G2,
       AINP(IS, [LS2[0], LS2[2], LS2[3]], [], bas=cover([IS])), None,
       "sec 7.2 check 4: the ledger's whole-set lineage holds the successor's key and the "
       "chain object's `senders` does not, so the basis says nothing about that key's "
       "anchors and is not covering.", "PARTIAL")
    LGAP, LGAPI = build_seq([("iss", "genesis", 0, GEN),
                             ("iss", "grant", 1, body),
                             ("iss", "annotation", 3, {"note_md": "after a gap"})])
    Gg = LGAP[1]
    CK("c4_label_gaps", Gg, AINP(IS, [LGAP[0], LGAP[2]], [], bas=COV), None,
       "sec 7.2 check 4: covering basis, grant unanchored, label GAPS. A MISSING entry "
       "may bound it, so the check is UNKNOWN and check 6 is UNKNOWN with it.", "PARTIAL")
    CK("c4_label_gaps_missing_hash", G,
       AINP(IS, [gen, ann], A_G + [anchor(HX32(0xF00D), IS, bn=200, ts=1700000000)], bas=COV), None,
       "sec 7.2 check 4 with the parent's sec 8.5: the grant is anchored so check 4 "
       "passes, and a counted hash of bytes nobody produced moves the label to GAPS, which "
       "leaves check 6 UNKNOWN.", "PARTIAL")
    CK("c4_unproven_anchor_of_grant", G,
       AINP(IS, [gen, ann], [anchor(eid(G), IS, bn=300, verdict="UNPROVEN")],
            bas=COV), None,
       "sec 7.2 check 4: an UNPROVEN anchor takes no part in the reconciliation, so the "
       "grant is not anchored; the parent's sec 8.6 rule 2 leaves the label COMPLETE for an "
       "UNPROVEN record over bytes in hand, and check 4 reads the undecided anchor of the "
       "grant's own bytes and stays UNKNOWN: PARTIAL, never a conviction on silence.", "PARTIAL")
    GU = grant_entry(K("iss"), seq=1, prev=eid(gen), grantee=AD("r1"), work=WK,
                     terms=HX32(0x7E), upstream=HX32(0xDE))
    CHN("incomplete_only_hop_green",
        [(GU, AINP(IS, [gen], [anchor(eid(GU), IS)], bas=COV))], None,
        "sec 7.5: a single GREEN hop whose body carries `upstream` has the failing point "
        "`incomplete`, and the chain verdict is decided in order, so FAIL though every hop is "
        "GREEN and every link (none) is true.", "FAIL")
    CK("c4_void_anchor", G,
       AINP(IS, [gen, ann], [anchor(eid(G), IS, bn=400, verdict="VOID")], bas=COV),
       None,
       "sec 7.2 check 4: a VOID anchor is informational and no label turns on it, so the "
       "label is COMPLETE with the grant unanchored under a covering basis: FAIL.", "FAIL")

    # ---- check 6 ----------------------------------------------------------
    genr, Gr, rev = issuer("iss", body, tail="revocation")
    CK("c6_revoked", Gr, AINP(IS, [genr, rev], [anchor(eid(Gr), IS, ts=1700000000)],
                              bas=COV), None,
       "sec 7.2 check 6: a ledger entry of type revocation names this grant and carries no "
       "AUTHORITY_MISMATCH.", "FAIL")
    geno, Go, revo = issuer("iss", body, tail="revocation_other")
    CK("c6_revocation_of_another_grant", Go,
       AINP(IS, [geno, revo], [anchor(eid(Go), IS, ts=1700000000)], bas=COV), None,
       "sec 7.2 check 6: a revocation whose `grant` names something else withdraws nothing "
       "here.", "GREEN")
    LR, LRI = build_seq([("iss", "genesis", 0, GEN),
                         ("iss", "grant", 1, body),
                         ("iss", "succession", 2, SUC)])
    Gsr = LR[1]
    rev_soft = E("iss", "revocation", 6, LRI[2], {"grant": eid(Gsr)})
    CK("c6_revocation_with_soft_authority", Gsr,
       AINP(IS, [LR[0], LR[2], rev_soft], [anchor(eid(Gsr), IS, ts=1700000000)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 6: the revocation was written by a key the succession had already "
       "replaced, and the gap before it makes the finding soft. A revocation carrying an "
       "AUTHORITY_MISMATCH does not fail the check, and the label (GAPS) leaves it "
       "UNKNOWN.", "PARTIAL")
    CK("c6_absent_under_gaps", Gg,
       AINP(IS, [LGAP[0], LGAP[2]], [anchor(eid(Gg), IS, ts=1700000000)], bas=COV),
       None,
       "sec 7.2 check 6: no revocation is in the record and the record is not COMPLETE, so "
       "the check is UNKNOWN: a withheld revocation is exactly what an incomplete record "
       "hides.", "PARTIAL")

    # ---- combinations -----------------------------------------------------
    CK("fail_two_checks", GW,
       AINP(IS, [genw, annw], [anchor(eid(genw), IS, ts=1700000000)], bas=COV), 3000,
       "sec 7.3: two checks fail, and `failed` carries their tokens in check order.",
       "FAIL")
    genrw, Grw, revw = issuer("iss", gbody(AD("r1"), WK, window={"from": 1000, "to": 2000}),
                              tail="revocation")
    CK("fail_expired_and_revoked", Grw,
       AINP(IS, [genrw, revw], [anchor(eid(Grw), IS, ts=1700000000)], bas=COV), 5,
       "sec 7.3: checks 5 and 6 both fail.", "FAIL")
    CK("partial_then_fail", G,
       AINP(IS, [gen, ann], [], bas=basis()), None,
       "sec 7.2: a non-covering basis leaves check 4 UNKNOWN while every other check "
       "passes, which is PARTIAL and never green.", "PARTIAL")
    CK("green_with_succession", G2,
       AINP(IS, [LS2[0], LS2[2], LS2[3]], [anchor(eid(G2), IS, ts=1700000000)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 4: coverage asks for every key of the whole-set lineage, and here "
       "the basis names both.", "GREEN")

    # ---- the second law: timeliness, and the continued ledger -------------
    T0 = 1700000000
    LT, LTI = build_seq([("iss", "genesis", 0, GEN), ("iss", "succession", 1, SUC),
                         ("iss2", "annotation", 2, {"note_md": "new hand"})])
    late_g = E("iss", "grant", 2, LTI[1], body)
    CK("c3_retired_grant", late_g,
       AINP(IS, LT, [anchor(LTI[0], IS, bn=10, ts=T0), anchor(LTI[1], IS, bn=11, ts=T0 + 1),
                     anchor(LTI[2], AD("iss2"), bn=12, ts=T0 + 2)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3 with the parent's sec 7.4: the grant is signed by the ceding key "
       "and reachable from nothing any record at or before its retirement anchors; not "
       "timely, RETIRED, no ledger entry, so NOT_IN_LEDGER fails.", "FAIL")
    CK("c3_timely_grant_of_handed_off_key", G2,
       AINP(IS, [LS2[0], LS2[2], LS2[3]],
            [anchor(eid(G2), IS, bn=10, ts=T0), anchor(LS2I[2], IS, bn=11, ts=T0 + 1), anchor(LS2I[3], AD("iss2"), bn=12, ts=T0 + 2)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 7.2: the grant the ceding key signed before handing off is anchored before the "
       "retirement, timely, a ledger entry, and GREEN.", "GREEN")
    v1g = E1("iss", "genesis", 0, None, GEN)
    v1gr = E1("iss", "grant", 1, eid(v1g), body)
    v1m = E1("iss", "succession", 2, eid(v1gr), {"to": IS, "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    c_gen = E("iss", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": IS, "entry_id": eid(v1m)}})
    c_ann = E("iss", "annotation", 1, eid(c_gen), {"note_md": "under the second law"})
    CONT_PILE = [v1g, v1m, c_gen, c_ann]
    CONT_ANC = [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1gr), IS, bn=11, ts=T0 + 1),
                anchor(eid(v1m), IS, bn=12, ts=T0 + 2), anchor(eid(c_gen), IS, bn=13, ts=T0 + 3),
                anchor(eid(c_ann), IS, bn=14, ts=T0 + 4)]
    CK("green_v1_grant_in_continued_ledger", v1gr,
       AINP(IS, CONT_PILE, CONT_ANC, bas=COV), None,
       "sec 1/7.2 with the parent's sec 12.6: a zikaron/1 grant is accepted by spec, is a "
       "ledger entry of the continued segment, is anchored there, and the whole reads "
       "COMPLETE; GREEN without re-signing.", "GREEN")
    CK("green_v1_grant_old_segment_stranger_record", v1gr,
       AINP(IS, CONT_PILE, CONT_ANC + [anchor(eid(v1gr), AD("r1"), bn=11, ts=T0 + 1, tx=HX32(0xB11))],
            bas=cover([IS, AD("r1")])), None,
       "sec 6.2 with zikaron/1 sec 8.1: a record of the continued segment whose sender the "
       "basis names but the segment's lineage does not is discarded there and bounds "
       "nothing; GREEN as before.", "GREEN")
    CK("green_v1_grant_unproven_old_record", v1gr,
       AINP(IS, CONT_PILE, [r for r in CONT_ANC if r["hash"] != eid(v1gr)]
            + [anchor(eid(v1gr), IS, bn=11, ts=T0 + 1, verdict="UNPROVEN")], bas=COV), None,
       "sec 6.2 with zikaron/1 sec 8.6: the old grant's only record of its own is UNPROVEN, "
       "but the migration's counted record reaches it along prev, so it is anchored, the "
       "UNPROVEN record is not open, the continued segment reads COMPLETE, and the verdict "
       "is GREEN.", "GREEN")
    v1x = E1("iss", "annotation", 1, eid(v1g), {"note_md": "a second seq 1"})
    CK("c2_old_segment_equivocation", v1gr,
       AINP(IS, CONT_PILE + [v1x], CONT_ANC + [anchor(eid(v1x), IS, bn=11, ts=T0 + 1, tx=HX32(0xB12))],
            bas=COV), None,
       "sec 7.2 check 2 with the parent's sec 8.6: two zikaron/1 entries of the old key at "
       "seq 1 are an equivocation, the continued segment's report labels BROKEN_CHAIN, and "
       "the whole takes the first label in order.", "FAIL")
    CK("green_two_records_of_one_entry", v1gr,
       AINP(IS, CONT_PILE, CONT_ANC + [anchor(eid(c_ann), IS, cid=2, bn=14, ts=T0 + 4)],
            bas=cover([IS], cid=(1, 2))), None,
       "sec 6.2: two counted records of one entry on two chains; the bound is the smaller "
       "time and the verdict is GREEN.", "GREEN")
    c_rev = E("iss", "revocation", 2, eid(c_ann), {"grant": eid(v1gr)})
    CK("c6_v1_grant_revoked_in_new_segment", v1gr,
       AINP(IS, CONT_PILE + [c_rev], CONT_ANC + [anchor(eid(c_rev), IS, bn=15, ts=T0 + 5)], bas=COV), None,
       "sec 7.2 check 6 with the parent's sec 12.3: a revocation of this law in the new "
       "segment names the old grant as it would name an entry of its own.", "FAIL")
    CK("c4_v1_grant_bounded_by_new_segment_anchor", v1gr,
       AINP(IS, CONT_PILE, [anchor(eid(v1m), IS, bn=12, ts=T0 + 2), anchor(eid(c_ann), IS, bn=14, ts=T0 + 4)], bas=COV), None,
       "sec 6.1/6.2 with the parent's sec 9.5: the new segment's anchor of the annotation "
       "reaches the genesis by prev and the old segment by continues.entry_id, so the old "
       "grant is anchored though no record of its own segment names it.", "GREEN")
    v1h = E1("iss", "genesis", 0, None, GEN)
    v1s = E1("iss", "succession", 1, eid(v1h), SUC)
    v1gr2 = E1("iss2", "grant", 2, eid(v1s), body)
    v1m2 = E1("iss2", "succession", 3, eid(v1gr2), {"to": AD("iss2"), "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    c_gen2 = E("iss2", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": IS, "entry_id": eid(v1m2)}})
    HPILE = [v1h, v1s, v1gr2, v1m2, c_gen2]
    HANC = [anchor(eid(v1h), IS, bn=10, ts=T0), anchor(eid(v1s), IS, bn=11, ts=T0 + 1),
            anchor(eid(v1gr2), AD("iss2"), bn=12, ts=T0 + 2), anchor(eid(v1m2), AD("iss2"), bn=13, ts=T0 + 3),
            anchor(eid(c_gen2), AD("iss2"), bn=14, ts=T0 + 4)]
    CK("green_old_lineage_covered", v1gr2, AINP(IS, HPILE, HANC, bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 4: a covering basis names every key of the continued segment's "
       "zikaron/1 lineage and of the new segment's; both named, GREEN.", "GREEN")
    c_gr2 = E("iss2", "grant", 1, eid(c_gen2), body)
    CK("c4_old_lineage_not_covered", c_gr2,
       AINP(IS, HPILE, [a for a in HANC if a["sender"] == AD("iss2")], bas=cover([AD("iss2")])), None,
       "sec 7.2 check 4: a new-segment grant no record bounds; the basis names the migrating "
       "key and not the old root, a key of the continued segment's lineage, so it is not "
       "covering, the check is UNKNOWN and the verdict PARTIAL.", "PARTIAL")
    CK("c4_both_lineages_covered_unanchored", c_gr2,
       AINP(IS, HPILE, [a for a in HANC if a["sender"] == AD("iss2")], bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 4: the same grant under a basis naming both lineages' keys; covering, "
       "the whole reads COMPLETE, and absence convicts.", "FAIL")
    # -- round 1 fold: the seats' witnesses
    CK("green_v1_grant_anchored_only_through_continues", v1gr,
       AINP(IS, CONT_PILE, [anchor(eid(v1m), IS, bn=12, ts=T0 + 2, verdict="VOID"), anchor(eid(c_ann), IS, bn=14, ts=T0 + 4)], bas=COV), None,
       "sec 6.1/6.2: the closing succession's only record is VOID, which makes the candidate "
       "and counts for nothing; the old grant is anchored through the new annotation alone, "
       "by prev to the genesis and by its continues to the succession, then along prev.", "GREEN")
    CK("green_v1_grant_anchored_only_by_old_record", v1gr,
       AINP(IS, CONT_PILE, [anchor(eid(v1m), IS, bn=12, ts=T0 + 2)], bas=COV), None,
       "sec 6.2: the counted anchors of the continued segment anchor its entries; the closing "
       "succession's record, the only one, reaches the grant along prev.", "GREEN")
    CK("c4_v1_grant_reached_only_by_old_unproven_record", v1gr,
       AINP(IS, CONT_PILE, [anchor(eid(v1m), IS, bn=12, ts=T0 + 2, verdict="UNPROVEN")], bas=COV), None,
       "sec 7.2 check 4: the only record is UNPROVEN, of the continued segment; it makes the "
       "candidate, anchors nothing, and is an undecided anchor of bytes that would anchor "
       "the grant: UNKNOWN.", "PARTIAL")
    CK("green_anchored_grant_under_non_covering_basis", G2,
       AINP(IS, [LS2[0], LS2[2]], [anchor(eid(G2), IS, bn=10, ts=T0), anchor(LS2I[2], IS, bn=11, ts=T0 + 1)], bas=cover([IS])), None,
       "sec 7.2 check 4 order: the grant is anchored, so the check passes before coverage is "
       "asked; the basis names the root alone though the succession admits a second key.", "GREEN")
    sg0 = E1("iss", "genesis", 0, None, GEN)
    sg1 = E1("iss", "succession", 1, eid(sg0), SUC)
    sg3 = E1("iss", "grant", 3, HX32(0x3333), body)
    sg4 = E1("iss2", "succession", 4, eid(sg3), {"to": AD("iss2"), "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    sgen = E("iss2", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": IS, "entry_id": eid(sg4)}})
    CK("c3_soft_authority_finding_in_old_segment", sg3,
       AINP(IS, [sg0, sg1, sg4, sgen],
            [anchor(eid(sg0), IS, bn=10, ts=T0), anchor(eid(sg1), IS, bn=11, ts=T0 + 1), anchor(eid(sg3), IS, bn=12, ts=T0 + 2),
             anchor(eid(sg4), AD("iss2"), bn=13, ts=T0 + 3), anchor(eid(sgen), AD("iss2"), bn=14, ts=T0 + 4)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3 over the continued segment: the old grant, signed by the ceding key "
       "after a gap, carries a soft AUTHORITY_MISMATCH in the zikaron/1 report; check 3 is "
       "UNKNOWN, the grant is anchored, and the label GAPS leaves check 6 UNKNOWN.", "PARTIAL")
    mg0 = E("iss", "genesis", 0, None, GEN)
    mg1 = E("iss", "migration", 1, eid(mg0), {"spec": "zikaron/3", "statement_md": "moving on"})
    mg2 = E("iss", "grant", 2, eid(mg1), body)
    CK("c3_grant_after_own_migration_retired", mg2,
       AINP(IS, [mg0, mg1], [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(mg1), IS, bn=11, ts=T0 + 1)], bas=COV), None,
       "sec 7.2 check 3 with the parent's sec 7.4: the migration's record retires the issuer at "
       "T0+1, and the unanchored grant after it is reached by nothing anchored by then: "
       "RETIRED under a COMPLETE record, FAIL at check 3.", "FAIL")
    CK("c2_grant_after_own_migration_anchored_before_it", mg2,
       AINP(IS, [mg0, mg1], [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(mg2), IS, bn=10, ts=T0, tx=HX32(0xB32)), anchor(eid(mg1), IS, bn=11, ts=T0 + 1)], bas=COV), None,
       "sec 7.2 check 2 with the parent's sec 7.3 and sec 7.4: the grant after the seat's own "
       "migration is anchored a second before the migration's record, at which the seat has "
       "not retired, so it is timely and a ledger entry; no key holds authority after the "
       "migration, the finding is hard, and the ledger is broken.", "FAIL")
    CK("c3_grant_after_own_migration_anchored_in_its_second", mg2,
       AINP(IS, [mg0, mg1], [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(mg1), IS, bn=11, ts=T0 + 1), anchor(eid(mg2), IS, bn=11, ts=T0 + 1, tx=HX32(0xB31))], bas=COV), None,
       "sec 7.2 check 3 with the parent's sec 7.4: the grant after the seat's own migration, "
       "anchored by the seat in the migration's second at a position above its retiring seq, "
       "rides a record at which the seat has retired: DISCARDED, the grant RETIRED and no "
       "ledger entry under a COMPLETE record; check 3 fails.", "FAIL")
    st_over = E("iss2", "annotation", 3, eid(mg2), {"note_md": "over the late grant"})
    CK("c3_grant_after_own_migration_reached_through_stranger_anchored_by_seat_in_its_second", mg2,
       AINP(IS, [mg0, mg1, st_over], [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(mg1), IS, bn=11, ts=T0 + 1),
                                      anchor(eid(st_over), IS, bn=11, ts=T0 + 1, tx=HX32(0xB33))], bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3 with the parent's sec 7.4: in the migration's second the seat anchors a "
       "stranger's entry whose prev is the seat's grant above its retiring seq; the record anchors "
       "what another key signed, so the seat has not retired at it and it counts, while the grant "
       "is made timely by no record of that second: RETIRED, the stranger's entry EXCLUDED, the "
       "record's hash MISSING, GAPS; checks 3, 4 and 6 UNKNOWN.", "PARTIAL")
    CK("c3_grant_after_own_migration_reached_through_stranger_record_in_its_second", mg2,
       AINP(IS, [mg0, mg1, st_over], [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(mg1), IS, bn=11, ts=T0 + 1),
                                      anchor(eid(st_over), AD("iss2"), bn=11, ts=T0 + 1, tx=HX32(0xB34))], bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3 with the parent's sec 7.4: the same entry anchored in that second by the "
       "stranger itself; the record is DISCARDED, its sender outside the lineage, and the grant, "
       "made timely by no record of that second, is RETIRED under a COMPLETE record; check 3 "
       "fails.", "FAIL")
    lgr = E("iss", "grant", 1, eid(mg0), body)
    lmg = E("iss", "migration", 2, eid(lgr), {"spec": "zikaron/3", "statement_md": "moving on"})
    ly2 = E("iss", "annotation", 2, eid(lgr), {"note_md": "the seat's own twin of its migration"})
    ll3 = E("iss", "annotation", 3, eid(ly2), {"note_md": "the seat above its retiring seq"})
    LA = [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(lgr), IS, bn=11, ts=T0 + 1), anchor(eid(lmg), IS, bn=12, ts=T0 + 2)]
    CK("c2_grant_before_own_migration_low_twin_bounded_by_discarded_record", lgr,
       AINP(IS, [mg0, lmg, ly2, ll3], LA + [anchor(eid(ll3), IS, bn=12, ts=T0 + 2, tx=HX32(0xB35))], bas=COV), None,
       "sec 7.2 check 2 with the parent's sec 7.4: in the migration's second the seat anchors its "
       "seq-3 entry above its retiring seq 2; the record is one at which the seat has retired, "
       "DISCARDED, yet it bounds what the seat signed at or before its retiring seq on that "
       "entry's line, so the seat's seq-2 twin of its migration is timely and forks it, the "
       "ledger is broken and the grant before the migration fails check 2.", "FAIL")
    CK("green_grant_before_own_migration_low_twin_record_a_second_after", lgr,
       AINP(IS, [mg0, lmg, ly2, ll3], LA + [anchor(eid(ll3), IS, bn=13, ts=T0 + 3, tx=HX32(0xB36))], bas=COV), None,
       "sec 7.2 with the parent's sec 7.4: the same record one second after the migration's "
       "bounds nothing of the seat's; the twin and the entry above it are RETIRED, the record "
       "DISCARDED, the record COMPLETE and the grant GREEN.", "GREEN")
    mgr = E("iss", "grant", 1, eid(mg0), body)
    mgm = E("iss", "migration", 2, eid(mgr), {"spec": "zikaron/3", "statement_md": "moving on"})
    CK("green_grant_before_own_migration", mgr,
       AINP(IS, [mg0, mgm], [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(mgr), IS, bn=11, ts=T0 + 1), anchor(eid(mgm), IS, bn=12, ts=T0 + 2)], bas=COV), None,
       "sec 7.2: a grant before the seat's migration, anchored before it; the migration closes "
       "the ledger and the grant is GREEN.", "GREEN")
    rgr = E("iss", "grant", 1, eid(mg0), body)
    rtr = E("iss", "retraction", 2, eid(rgr), {"subject": eid(rgr), "note_md": "withdrawn"})
    CK("green_grant_with_retraction_in_ledger", rgr,
       AINP(IS, [mg0, rtr], [anchor(eid(mg0), IS, bn=10, ts=T0), anchor(eid(rgr), IS, bn=11, ts=T0 + 1), anchor(eid(rtr), IS, bn=12, ts=T0 + 2)], bas=COV), None,
       "sec 7.2: a retraction naming the grant is a ledger entry like any other and no check "
       "reads it; what it means is the parent's sec 11.", "GREEN")
    CK("c4_unproven_record_of_excluded_bytes_complete", G,
       AINP(IS, [gen, ann, Gs], [anchor(eid(gen), IS, bn=10, ts=T0), anchor(eid(Gs), IS, bn=11, ts=T0 + 1, verdict="UNPROVEN")], bas=COV), None,
       "sec 7.2 check 4: an UNPROVEN record over a stranger's bytes anchors no ledger entry "
       "and reaches the grant through nothing; its hash is in the pile so the label stays "
       "COMPLETE, the basis is covering, and absence convicts.", "FAIL")
    far = E("iss", "annotation", 3, HX32(0x4444), {"note_md": "after a gap"})
    CK("c4_reach_stops_at_missing_prev", G,
       AINP(IS, [gen, far], [anchor(eid(far), IS, bn=12, ts=T0 + 2)], bas=COV), None,
       "sec 6.1: reachability stops where no ledger entry matches the prev; the anchored "
       "entry after a gap bounds nothing below it, the grant is unanchored, and the label "
       "GAPS leaves check 4 UNKNOWN.", "PARTIAL")
    v1late = E1("iss", "grant", 3, eid(v1m), body)
    c_annx = E("iss", "annotation", 2, eid(c_ann), {"note_md": "names bytes", "continues": {"entry_id": eid(v1late)}})
    CK("c4_extra_member_continues_is_no_edge", v1late,
       AINP(IS, CONT_PILE + [v1gr, c_annx],
            CONT_ANC + [anchor(eid(v1late), IS, bn=11, ts=T0 + 1, tx=HX32(0xB21), verdict="VOID"), anchor(eid(c_annx), IS, bn=15, ts=T0 + 5)],
            bas=COV), None,
       "sec 6.1: a member named continues on an annotation is data and no edge; the old grant, "
       "anchored before the cut by a VOID record alone, is unanchored, the record is "
       "COMPLETE under a covering basis, and absence convicts.", "FAIL")
    rg0 = E("iss", "genesis", 0, None, GEN)
    rg1 = E("iss", "grant", 1, eid(rg0), body)
    rg2 = E("iss", "annotation", 2, eid(rg1), {"note_md": "withheld by the relay"})
    rg3 = E("iss", "succession", 3, eid(rg2), SUC)
    CK("c3_retired_under_gaps", rg1,
       AINP(IS, [rg0, rg3], [anchor(eid(rg3), IS, bn=13, ts=T0 + 3)], bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3: the grant is bounded by nothing a record at or before the retirement "
       "anchors because the entry between it and the anchored succession is not held; it is "
       "RETIRED, the label is GAPS, and absence convicts only on a complete record: "
       "UNKNOWN.", "PARTIAL")
    CK("green_line_whole_bounds_grant", rg1,
       AINP(IS, [rg0, rg2, rg3], [anchor(eid(rg3), IS, bn=13, ts=T0 + 3)], bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3: with the line whole the succession's record bounds the grant, which is "
       "timely and anchored; GREEN.", "GREEN")
    rgx = E("iss", "grant", 4, eid(rg3), body)
    CK("c3_retired_grant_after_handoff_complete", rgx,
       AINP(IS, [rg0, rg1, rg2, rg3], [anchor(eid(rg3), IS, bn=13, ts=T0 + 3)], bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3: a grant the ceding key signed after its handing off, reached by "
       "nothing anchored at or before its retirement; RETIRED under a COMPLETE record: "
       "FAIL.", "FAIL")
    rev_x = E("iss2", "revocation", 4, eid(rg3), {"grant": eid(rg1)})
    CK("c3_retired_under_gaps_with_revocation", rg1,
       AINP(IS, [rg0, rg3, rev_x], [anchor(eid(rg3), IS, bn=13, ts=T0 + 3), anchor(eid(rev_x), AD("iss2"), bn=14, ts=T0 + 4)], bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 3: the grant is RETIRED under GAPS, so check 3 is UNKNOWN and leaves "
       "checks 4 and 6 UNKNOWN though a timely revocation of it stands in the ledger.", "PARTIAL")
    v1_after_cut = E1("iss", "grant", 3, eid(v1m), body)
    CK("c3_v1_grant_signed_after_cut", v1_after_cut,
       AINP(IS, CONT_PILE, CONT_ANC, bas=COV), None,
       "sec 7.2 check 3: a zikaron/1 grant signed after the closing succession is reachable from "
       "it by no prev and anchored at or before the cut by no record: a byte string of the "
       "new segment, MALFORMED there, so FAIL.", "FAIL")
    v1_str = E1("r1", "grant", 1, eid(v1g), body)
    CK("c3_v1_grant_excluded_in_old_segment", v1_str,
       AINP(IS, CONT_PILE, CONT_ANC + [anchor(eid(v1_str), IS, bn=11, ts=T0 + 1, tx=HX32(0xB32))], bas=COV), None,
       "sec 7.2 check 3: a stranger's zikaron/1 grant anchored before the cut is an old byte "
       "string, EXCLUDED in the old segment's report: FAIL.", "FAIL")
    far_g = E("iss", "grant", 3, HX32(0x4545), body)
    gen_c = E("iss", "genesis", 0, None, {"statement_md": "uncontinued", "continues": {"root": IS, "entry_id": eid(far_g)}})
    CK("c4_uncontinued_genesis_continues_is_no_edge", far_g,
       AINP(IS, [gen_c], [anchor(eid(gen_c), IS, bn=10, ts=T0)], bas=COV), None,
       "sec 6.1: the continues member of a genesis the report selects as no continuation is "
       "data; the anchored genesis reaches the grant after a gap through nothing, the label "
       "is GAPS, check 4 UNKNOWN.", "PARTIAL")
    far_g2 = E("iss", "grant", 3, HX32(0x4546), body)
    v1g_c = E1("iss", "genesis", 0, None, dict(GEN, continues={"entry_id": eid(far_g2)}))
    v1gr_c = E1("iss", "grant", 1, eid(v1g_c), body)
    v1m_c = E1("iss", "succession", 2, eid(v1gr_c), {"to": IS, "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    c_gen_c = E("iss", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": IS, "entry_id": eid(v1m_c)}})
    CK("c4_v1_genesis_continues_member_is_no_edge", far_g2,
       AINP(IS, [v1g_c, v1m_c, c_gen_c],
            [anchor(eid(v1g_c), IS, bn=10, ts=T0), anchor(eid(v1m_c), IS, bn=12, ts=T0 + 2), anchor(eid(c_gen_c), IS, bn=13, ts=T0 + 3)], bas=COV), None,
       "sec 6.1: a zikaron/1 genesis carrying continues as an extra member is no edge, though "
       "anchored; the new-segment grant after a gap stays unanchored, GAPS, check 4 UNKNOWN.",
       "PARTIAL")
    c_gr_cut = E("iss", "grant", 1, eid(c_gen), body)
    CK("green_new_segment_grant_anchored_at_cut", c_gr_cut,
       AINP(IS, [v1g, v1m, c_gen], CONT_ANC[:3] + [anchor(eid(c_gen), IS, bn=13, ts=T0 + 3), anchor(eid(c_gr_cut), IS, bn=12, ts=T0 + 2, tx=HX32(0xB40))], bas=COV), None,
       "sec 6.2: the counted anchors are those of either segment; a new-segment grant anchored "
       "in the cut's second has an old record, which anchors it here though the new report "
       "lists it UNANCHORED and the old report its hash MISSING: check 4 PASS, check 6 "
       "UNKNOWN under GAPS.", "PARTIAL")
    v1rev = E1("iss", "revocation", 2, eid(v1gr), {"grant": eid(v1gr)})
    v1m_r = E1("iss", "succession", 3, eid(v1rev), {"to": IS, "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    c_gen_r = E("iss", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": IS, "entry_id": eid(v1m_r)}})
    CK("c6_v1_revocation_in_old_segment", v1gr,
       AINP(IS, [v1g, v1rev, v1m_r, c_gen_r],
            [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1gr), IS, bn=11, ts=T0 + 1), anchor(eid(v1rev), IS, bn=12, ts=T0 + 2),
             anchor(eid(v1m_r), IS, bn=13, ts=T0 + 3), anchor(eid(c_gen_r), IS, bn=14, ts=T0 + 4)], bas=COV), None,
       "sec 7.2 check 6 with sec 1: the ledger spans both segments; a zikaron/1 revocation of the "
       "grant in the old segment fails the check.", "FAIL")
    v1rev_soft = E1("iss", "revocation", 4, HX32(0x3434), {"grant": eid(v1gr2)})
    v1m_s = E1("iss2", "succession", 5, eid(v1rev_soft), {"to": AD("iss2"), "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    c_gen_s = E("iss2", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": IS, "entry_id": eid(v1m_s)}})
    CK("c6_v1_revocation_soft_in_old_segment", v1gr2,
       AINP(IS, [v1h, v1s, v1rev_soft, v1m_s, c_gen_s],
            [anchor(eid(v1h), IS, bn=10, ts=T0), anchor(eid(v1s), IS, bn=11, ts=T0 + 1), anchor(eid(v1gr2), AD("iss2"), bn=12, ts=T0 + 2),
             anchor(eid(v1rev_soft), IS, bn=13, ts=T0 + 3), anchor(eid(v1m_s), AD("iss2"), bn=14, ts=T0 + 4), anchor(eid(c_gen_s), AD("iss2"), bn=15, ts=T0 + 5)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 6: a zikaron/1 revocation in the old segment signed by the ceded key after "
       "a gap carries a soft AUTHORITY_MISMATCH, so it does not fail the check; the label GAPS "
       "leaves it UNKNOWN.", "PARTIAL")
    CK("green_now_zero", G, AINP(IS, [gen, ann], A_G, bas=COV), 0,
       "HARNESS-KIT2: --now 0 is the least int and is an answer; the grant has no window.", "GREEN")
    CK("green_now_max_int", G, AINP(IS, [gen, ann], A_G, bas=COV), 9007199254740991,
       "HARNESS-KIT2: --now 2^53 - 1 is the greatest int and is an answer.", "GREEN")
    # -- round 4 fold --------------------------------------------------------
    gR1 = E1("r1", "genesis", 0, None, GEN)
    XREC = [anchor(eid(gR1), AD("r1"), bn=11, ts=T0 + 1, tx=HX32(0xB50))]
    COV3 = cover([IS, AD("iss2"), AD("r1")])
    CK("green_handed_off_key_reanchors_excluded_old_bytes", v1gr2,
       AINP(IS, HPILE + [gR1], HANC + XREC + [anchor(eid(gR1), IS, bn=50, ts=T0 + 50, tx=HX32(0xB51))], bas=COV3), None,
       "sec 6.2 with the parent's sec 12.6: a stranger's zikaron/1 genesis anchored before the cut "
       "is an EXCLUDED old byte string and no old ledger entry; the handed-off root's late record "
       "of it is a new record, DISCARDED there, and the grant stays GREEN.", "GREEN")
    CK("green_unproven_record_of_excluded_old_bytes_whole_pile", v1gr2,
       AINP(IS, HPILE + [gR1], HANC + XREC + [anchor(eid(gR1), AD("iss2"), bn=50, ts=T0 + 50, tx=HX32(0xB52), verdict="UNPROVEN")], bas=COV3), None,
       "sec 7.2 with the parent's sec 8.6 item 15 rule 2: the key in office's late UNPROVEN record "
       "of the excluded old bytes is a new record over bytes in hand; the label stays COMPLETE "
       "and the grant GREEN.", "GREEN")
    v1late2 = E1("iss2", "grant", 4, eid(v1m2), body)
    CK("c4_old_record_by_stranger_anchors_nothing", v1late2,
       AINP(IS, HPILE, HANC + [anchor(eid(v1late2), AD("r1"), bn=13, ts=T0 + 3, tx=HX32(0xB53))], bas=COV3), None,
       "sec 6.2: the counted anchors of the old segment are those its own zikaron/1 trim keeps; a "
       "stranger's record in the cut's second of a grant after the close anchors nothing, the "
       "basis is covering and the record COMPLETE, so check 4 fails.", "FAIL")
    CK("c4_two_covering_chains", G, AINP(IS, [gen, ann], [], bas=cover([IS], cid=(1, 2))), None,
       "sec 7.2 check 4: every chains object covers, the grant is unanchored and the label "
       "COMPLETE: FAIL.", "FAIL")
    CK("c4_second_chain_without_registries", G,
       AINP(IS, [gen, ann], [], bas=basis([chainobj(1, 0, 1000, (REG,), [IS]), chainobj(2, 0, 1000, (), [IS])])), None,
       "sec 7.2 check 4: a basis is covering only where every chains object has a non-empty "
       "registries array; the second chain has none, so UNKNOWN.", "PARTIAL")
    CK("c4_second_chain_without_lineage_sender", G,
       AINP(IS, [gen, ann], [], bas=basis([chainobj(1, 0, 1000, (REG,), [IS]), chainobj(2, 0, 1000, (REG,), [AD("r1")])])), None,
       "sec 7.2 check 4: the second chains object's senders lack the root, so the basis is not "
       "covering: UNKNOWN.", "PARTIAL")
    CK("c4_unproven_record_by_stranger_is_discarded", G,
       AINP(IS, [gen, ann], [anchor(eid(G), AD("r1"), bn=10, ts=T0, verdict="UNPROVEN")], bas=cover([IS, AD("r1")])), None,
       "sec 7.2 check 4: an UNPROVEN record of the grant by a stranger the basis names is "
       "discarded by the parent's sec 8.1, so it is no undecided anchor of the grant; covering, "
       "COMPLETE, FAIL.", "FAIL")
    LSR, LSRI = build_seq([("iss", "genesis", 0, GEN), ("iss", "succession", 1, SUC), ("iss2", "grant", 2, body)])
    CK("c4_unproven_record_by_retired_key_is_discarded", LSR[2],
       AINP(IS, [LSR[0], LSR[1]], [anchor(LSRI[1], IS, bn=11, ts=T0 + 1), anchor(LSRI[2], IS, bn=15, ts=T0 + 5, tx=HX32(0xB54), verdict="UNPROVEN")],
            bas=cover([IS, AD("iss2")])), None,
       "sec 7.2 check 4: the ceding key's UNPROVEN record of the successor's grant, after the "
       "ceding key's retirement, is discarded by the parent's sec 8.1 and is no undecided "
       "anchor; the grant is unanchored under a covering basis and a COMPLETE label: FAIL.", "FAIL")
    N0 = E("iss", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": IS, "entry_id": eid(v1m)}})
    N1 = E("iss", "annotation", 1, eid(N0), {"note_md": "under the second law"})
    Sn = E("iss", "succession", 2, eid(N1), {"to": AD("issb"), "kind": "handover", "effective": 1700000000, "statement_md": "handed on"})
    def twin_pair(want_smaller):
        for k_ in range(256):
            G2x = E("iss", "genesis", 0, None, {"statement_md": "twin %d" % k_, "continues": {"root": IS, "entry_id": eid(v1m)}})
            if (eid(G2x) < eid(N0)) == want_smaller:
                return G2x
        raise AssertionError("no twin found")
    for want, slug in ((True, "twin_below"), (False, "twin_above")):
        G2x = twin_pair(want)
        CK("green_v1_grant_edge_from_timely_genesis_" + slug, v1gr,
           AINP(IS, [v1g, v1m, N0, N1, Sn, G2x],
                [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1m), IS, bn=12, ts=T0 + 2, verdict="VOID"),
                 anchor(eid(N0), IS, bn=13, ts=T0 + 3), anchor(eid(Sn), IS, bn=15, ts=T0 + 5)],
                bas=cover([IS, AD("issb")])), None,
           "sec 6.1: two geneses by the same key name the closing succession; the handing on at "
           "T0+5 retires the key, the unanchored twin is RETIRED, and whichever the parent "
           "selects (the id order decides, here the twin %s), the edge runs from the timely "
           "seq-0 ledger entry whose continues.entry_id is the report's; the old grant is "
           "anchored through it: GREEN."
           % ("below" if want else "above"), "GREEN")
    G2c = twin_pair(True)
    CK("green_v1_grant_twin_geneses_counted_record", v1gr,
       AINP(IS, [v1g, v1m, N0, N1, Sn, G2c],
            [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1m), IS, bn=12, ts=T0 + 2),
             anchor(eid(N0), IS, bn=13, ts=T0 + 3), anchor(eid(Sn), IS, bn=15, ts=T0 + 5)],
            bas=cover([IS, AD("issb")])), None,
       "sec 6.1/6.2: the same pair with the closing succession counted; the old grant is "
       "anchored by that record along prev whichever genesis is selected.", "GREEN")
    # -- round 5 fold --------------------------------------------------------
    G2r = None
    for k_ in range(256):
        cand_ = E("iss", "genesis", 0, None, {"statement_md": "twin %d under another root" % k_, "continues": {"root": AD("r1"), "entry_id": eid(v1m)}})
        if eid(cand_) > eid(N0):
            G2r = cand_; break
    N1r = E("iss", "annotation", 1, eid(G2r), {"note_md": "under the second law"})
    Snr = E("iss", "succession", 2, eid(N1r), {"to": AD("issb"), "kind": "handover", "effective": 1700000000, "statement_md": "handed on"})
    CK("green_v1_grant_edge_from_timely_entry_under_other_root", v1gr,
       AINP(IS, [v1g, v1m, N0, G2r, N1r, Snr],
            [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1m), IS, bn=12, ts=T0 + 2, verdict="VOID"),
             anchor(eid(G2r), IS, bn=13, ts=T0 + 3), anchor(eid(Snr), IS, bn=15, ts=T0 + 5)],
            bas=cover([IS, AD("issb")])), None,
       "sec 6.1: the genesis sec 12.6 selects (N0, root r) is unanchored and RETIRED at the handing "
       "on; the timely seq-0 ledger entry names the same succession under another continues.root, "
       "is no candidate, and still carries the edge, since the edge reads entry_id alone: the old "
       "grant is anchored through it, GREEN; a reader adding a root test answers FAIL at check 4.", "GREEN")
    CK("c4_unproven_reach_through_continues_edge", v1gr,
       AINP(IS, [v1g, v1m, N0, N1],
            [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1m), IS, bn=12, ts=T0 + 2, verdict="VOID"),
             anchor(eid(N0), IS, bn=13, ts=T0 + 3, verdict="UNPROVEN")], bas=COV), None,
       "sec 7.2 check 4 through sec 6.1: the close's record is VOID, the genesis's record UNPROVEN; "
       "the grant is unanchored, the basis covers, the label COMPLETE, and the undecided anchor "
       "reaches the grant through the continues edge, so check 4 is UNKNOWN: PARTIAL; a reach "
       "along prev alone answers FAIL.", "PARTIAL")
    CK("c4_old_unproven_record_by_outside_key_discarded", v1gr,
       AINP(IS, [v1g, v1m, N0],
            [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1m), IS, bn=12, ts=T0 + 2, verdict="VOID"),
             anchor(eid(v1m), AD("issb"), bn=17, ts=T0 + 7, tx=HX32(0xB60), verdict="UNPROVEN")], bas=cover([IS, AD("issb")])), None,
       "sec 6.2 with the parent's sec 12.6: the late UNPROVEN record of the closing succession is an "
       "old record (its hash an old ledger entry) by a key outside the zikaron/1 lineage, which the "
       "old trim discards, so it is no undecided anchor; the grant is unanchored under a covering "
       "basis and a COMPLETE label: FAIL (the same record from the old key is c121's UNKNOWN).", "FAIL")
    G5 = E("iss", "genesis", 0, None, {"statement_md": "names the old grant", "continues": {"root": IS, "entry_id": eid(v1gr)}})
    N15 = E("iss", "annotation", 1, eid(G5), {"note_md": "under the second law"})
    Sn5 = E("iss", "succession", 2, eid(N15), {"to": AD("issb"), "kind": "handover", "effective": 1700000000, "statement_md": "handed on"})
    CK("c4_continues_edge_only_from_entry_naming_the_close", v1gr,
       AINP(IS, [v1g, v1m, N0, G5, N15, Sn5],
            [anchor(eid(v1g), IS, bn=10, ts=T0), anchor(eid(v1m), IS, bn=12, ts=T0 + 2, verdict="VOID"),
             anchor(eid(G5), IS, bn=13, ts=T0 + 3), anchor(eid(Sn5), IS, bn=15, ts=T0 + 5)],
            bas=cover([IS, AD("issb")])), None,
       "sec 6.1: the timely seq-0 ledger entry's continues.entry_id names the old grant itself and "
       "not the report's succession, so it carries no edge; the genesis naming the close is "
       "unanchored and RETIRED; the close's record is VOID, so the old grant is unanchored under a "
       "covering basis and a COMPLETE label: FAIL; a reader drawing an edge from any continues "
       "member answers GREEN.", "FAIL")
    CK_order("green_now_before_audit", G, AINP(IS, [gen, ann], A_G, bas=COV), 1700000500,
             "HARNESS-KIT2: option tokens stand in any relative order after the positional "
             "argument; --now before --audit is the same check.")
    v1g_alone = E1("iss", "genesis", 0, None, GEN)
    v1gr_alone = E1("iss", "grant", 1, eid(v1g_alone), body)
    CK("c3_v1_grant_of_ledger_never_moved", v1gr_alone,
       AINP(IS, [v1g_alone], [anchor(eid(v1g_alone), IS, bn=10, ts=T0), anchor(eid(v1gr_alone), IS, bn=11, ts=T0 + 1)], bas=COV), None,
       "sec 7.2 check 3 with sec 9: a zikaron/1 grant whose issuer's ledger has no "
       "continuation; this law's audit reads the ledger as no ledger, and the check is "
       "UNKNOWN with checks 4 and 6, the zikaron.kit/1 core being the check for it.", "PARTIAL")
    CK("c3_v1_grant_of_ledger_never_moved_offline", v1gr_alone,
       AINP(IS, [v1g_alone], [], bas=COV), None,
       "sec 7.2 check 3: the same grant with no record at all; the label would read COMPLETE, "
       "and the gate keeps checks 4 and 6 UNKNOWN all the same.", "PARTIAL")
    hs0 = E("iss", "genesis", 0, None, GEN)
    hs1 = E("iss", "succession", 1, eid(hs0), SUC)
    hs2 = E("iss2", "grant", 2, eid(hs1), body)
    CK("green_grant_by_successor_under_original_root", hs2,
       AINP(IS, [hs0, hs1], [anchor(eid(hs0), IS, bn=10, ts=T0), anchor(eid(hs1), IS, bn=11, ts=T0 + 1), anchor(eid(hs2), AD("iss2"), bn=12, ts=T0 + 2)],
            bas=cover([IS, AD("iss2")])), None,
       "sec 5.1/7.1: a ledger that changed hands is read under the root the terms name; the "
       "successor's grant is a ledger entry under it and GREEN.", "GREEN")
    up_inp = AINP(IS, [gen, ann], A_G, bas=COV)
    up_inp["pile"] = ["0x" + x[2:].upper() for x in up_inp["pile"]]
    CK("green_pile_hex_uppercase", G, up_inp, 1700000500,
       "HARNESS2: the pile's hexadecimal digits are of either case; the bytes are the same "
       "and the verdict GREEN.", "GREEN")
    CK_raw("audit_file_not_json", G, b"{ this is not json",
           "HARNESS-KIT2 with sec 1: a readable audit file on which the core answers NO_LABEL is "
           "an invalid input, an answer and never misuse; checks 2, 3, 4 and 6 are UNKNOWN.")
    CK_raw("audit_file_not_utf8", G, b"\xff\xfe{}",
           "HARNESS-KIT2 with sec 1: an audit file that is not UTF-8 is read as no audit input "
           "at all, an invalid input.")
    genl, gl, annl = issuer("iss", dict(body, ledger=IS))
    CK("green_grant_with_ledger_member", gl,
       AINP(IS, [genl, annl], [anchor(eid(gl), IS, ts=1700000000)], bas=COV), None,
       "sec 7.1: the grant's `ledger` names the root the caller audits under; the check "
       "reads the audit input's root and the member is data.", "GREEN")

    # ---- the revision of 2026-11-09: a grant carrying a revocation member ---------
    AT = HX20(0xAA)
    BOOK = HX20(0xBB)
    IS2 = AD("iss2")

    def CKM(slug, g, inp, now, nts, note, expect=None):
        return CK(slug, g, inp, now, note, expect, nts=nts)

    def nb(cid=1, fb=0, tb=1000, at=AT, book=BOOK, bfb=0):
        return {"chainId": cid, "fromBlock": fb, "toBlock": tb, "revocationContract": at,
                "book": book, "bookFromBlock": bfb}

    def nrec(grant, sender, bn, ti=0, li=0, verdict="counted", cid=1, tx=None):
        return {"chainId": cid, "blockNumber": bn, "txIndex": ti, "logIndex": li,
                "tx": tx if tx else HX32(0xF0000 + bn * 16 + ti * 4 + li), "sender": sender,
                "grant": grant, "verdict": verdict}

    def brec(holder, to, root, bn, ti=0, li=0, cid=1):
        return {"chainId": cid, "blockNumber": bn, "txIndex": ti, "logIndex": li,
                "holder": holder, "to": to, "root": root}

    def NT(revs=(), accs=(), **kw):
        return {"basis": nb(**kw), "revocations": list(revs), "accessions": list(accs)}

    mbody = dict(body, revocation={"chain": 1, "at": AT})
    mgen, MG, mann = issuer("iss", mbody)
    MGID = eid(MG)
    MA = [anchor(MGID, IS, bn=100, ts=T0)]
    MI = AINP(IS, [mgen, mann], MA, bas=COV)
    COV2 = cover([IS, IS2])
    MI2 = AINP(IS, [mgen, mann], MA, bas=COV2)
    mrev = E("iss", "revocation", 3, eid(mann), {"grant": MGID})

    CKM("m_green", MG, MI, None, NT(),
       "sec 7.2 for a grant carrying a revocation member: the pile holds the genesis, the "
       "issuer anchored the grant at block 100 while in office (no accession in the book), the "
       "notice input applies and carries no notice: every check passes, and the result's "
       "`notices` is the notice basis.", "GREEN")
    CKM("m_green_ledger_revocation_unnoticed", MG, AINP(IS, [mgen, mann, mrev], MA + [anchor(eid(mrev), IS, bn=120, ts=T0 + 20)], bas=COV), None, NT(),
       "sec 7.2 check 6 for a grant carrying the member: a revocation entry of the ledger naming "
       "the grant is data here, and with no notice in the input the check passes (sec 9, two rails): "
       "GREEN where the ledger path answered FAIL.", "GREEN")
    CKM("m_c6_notice_counted", MG, MI, None, NT(revs=[nrec(MGID, IS, 150)]),
       "sec 7.2 check 6 for a grant carrying the member: a counted notice naming the grant by the key "
       "in office at its position fails the check.", "FAIL")
    CKM("m_c6_notice_unproven", MG, MI, None, NT(revs=[nrec(MGID, IS, 150, verdict="UNPROVEN")]),
       "sec 7.2 check 6: an UNPROVEN notice of this grant by the key in office leaves the check "
       "UNKNOWN; the codeless test was not decided and silence acquits nothing.", "PARTIAL")
    CKM("m_c6_notice_void_unknown", MG, MI, None, NT(revs=[nrec(MGID, IS, 150, verdict="VOID")]),
        "sec 7.2 check 6: a VOID notice of the grant by the key in office leaves the check UNKNOWN, "
        "as an UNPROVEN one does: the law cannot read it as the key's own act and acquits on it "
        "nowhere.", "PARTIAL")
    CKM("m_c6_void_by_stranger_ignored", MG, MI, None, NT(revs=[nrec(MGID, AD("stranger"), 150, verdict="VOID")]),
        "sec 7.2 check 6: a VOID notice by a key not in office withdraws nothing and leaves no doubt.", "GREEN")
    CKM("m_c6_void_by_retired_key_ignored", MG, MI2, None, NT(revs=[nrec(MGID, IS, 150, verdict="VOID")], accs=[brec(IS, IS2, IS, 140)]),
        "sec 7.2 check 6: a VOID notice by the retired root is a key's out of office and is read nowhere.", "GREEN")
    CKM("m_c6_counted_beside_void", MG, MI, None, NT(revs=[nrec(MGID, IS, 150, verdict="VOID"), nrec(MGID, IS, 160)]),
        "sec 7.2 check 6 order: a counted notice fails the check whatever VOID notices stand beside it.", "FAIL")
    CKM("m_c6_notice_by_stranger_ignored", MG, MI, None, NT(revs=[nrec(MGID, AD("stranger"), 150)]),
       "sec 7.2 check 6: a notice by a key not in office at its position withdraws nothing.", "GREEN")
    CKM("m_c6_notice_by_retired_key_ignored", MG, MI2, None, NT(revs=[nrec(MGID, IS, 150)], accs=[brec(IS, IS2, IS, 140)]),
       "sec 6.4 and sec 7.2 check 6: the book moved the seat at block 140; the root's notice at "
       "150 is a retired key's and withdraws nothing.", "GREEN")
    CKM("m_c6_notice_by_successor", MG, MI2, None, NT(revs=[nrec(MGID, IS2, 150)], accs=[brec(IS, IS2, IS, 140)]),
       "sec 6.4 and sec 7.2 check 6: the key the book put in office at 140 notices the grant at "
       "150: FAIL, though the ledger holds no succession at all.", "FAIL")
    CKM("m_c6_notice_same_block_before_accession", MG, MI2, None,
       NT(revs=[nrec(MGID, IS, 140, ti=0, li=0)], accs=[brec(IS, IS2, IS, 140, ti=1, li=0)]),
       "sec 6.4: positions compare (block, txIndex, logIndex); the root's notice in the "
       "accession's block at an earlier transaction is before it, so the root is in office and the "
       "notice counts.", "FAIL")
    CKM("m_c6_notice_same_block_by_root_after_accession", MG, MI2, None,
       NT(revs=[nrec(MGID, IS, 140, ti=2, li=0)], accs=[brec(IS, IS2, IS, 140, ti=1, li=0)]),
       "sec 6.4: the root's notice in the accession's block at a later transaction is after it, "
       "the root is out of office, and the notice withdraws nothing.", "GREEN")
    CKM("m_c6_notice_same_block_by_successor_after_accession", MG, MI2, None,
       NT(revs=[nrec(MGID, IS2, 140, ti=2, li=0)], accs=[brec(IS, IS2, IS, 140, ti=1, li=0)]),
       "sec 6.4: the successor's notice after the accession in the same block counts.", "FAIL")
    CKM("m_c6_notice_same_tx_log_order", MG, MI2, None,
       NT(revs=[nrec(MGID, IS2, 140, ti=3, li=1)], accs=[brec(IS, IS2, IS, 140, ti=3, li=0)]),
       "sec 6.4: inside one transaction the log index orders the accession before the notice, so "
       "the successor is in office at the notice.", "FAIL")
    CKM("m_c6_notice_same_tx_log_order_root_first", MG, MI2, None,
       NT(revs=[nrec(MGID, IS, 140, ti=3, li=0)], accs=[brec(IS, IS2, IS, 140, ti=3, li=1)]),
       "sec 6.4: the root's notice at log 0 precedes the accession at log 1 of the same "
       "transaction; the root is in office there and the notice counts.", "FAIL")
    CKM("m_c6_notice_of_another_grant", MG, MI, None, NT(revs=[nrec(HX32(0xDEAD), IS, 150)]),
       "sec 7.2 check 6: a notice whose grant is another hash withdraws nothing here.", "GREEN")
    CKM("m_c6_counted_beside_unproven", MG, MI, None,
       NT(revs=[nrec(MGID, IS, 150, verdict="UNPROVEN"), nrec(MGID, IS, 160)]),
       "sec 7.2 check 6 order: a counted notice of the grant fails the check whatever UNPROVEN "
       "notices stand beside it.", "FAIL")
    CKM("m_c6_notice_at_fromBlock", MG, MI, None, NT(revs=[nrec(MGID, IS, 0)]),
       "sec 6.3: a notice in the basis's first block is inside the range.", "FAIL")
    CKM("m_c6_notice_at_toBlock", MG, MI, None, NT(revs=[nrec(MGID, IS, 1000)]),
       "sec 6.3: a notice in the basis's last block is inside the range.", "FAIL")
    CKM("m_c6_notice_without_audit", MG, None, None, NT(revs=[nrec(MGID, IS, 150)]),
       "sec 7.2 for a grant carrying the member with no audit input: checks 2, 3 and 4 are "
       "UNKNOWN, check 6 reads the notice input alone and fails, `basis` is null and `notices` "
       "is the notice basis.", "FAIL")
    CKM("m_no_audit_no_notices", MG, None, None, None,
       "sec 7.2 for a grant carrying the member with neither input: checks 2, 3, 4 and 6 UNKNOWN, "
       "`notices` null.", "PARTIAL")
    CKM("m_badge_alone", MG, AINP(IS, [], MA, bas=COV), None, NT(),
       "sec 9 (a badge alone): an audit input of empty pile built from the chain, with the "
       "notice input; check 2 is UNKNOWN for want of a genesis among the ledger entries, checks "
       "3, 4 and 6 are decided from the chain, and the verdict is PARTIAL.", "PARTIAL")
    CKM("m_badge_alone_revoked", MG, AINP(IS, [], MA, bas=COV), None, NT(revs=[nrec(MGID, IS, 150)]),
       "sec 9 (a badge alone): the same empty pile with a notice: FAIL at check 6 with no ledger "
       "in hand.", "FAIL")
    CKM("m_c2_spine_only", MG, AINP(IS, [mgen], MA, bas=COV), None, NT(),
       "sec 7.2 check 2 for a grant carrying the member: a pile holding the genesis alone passes "
       "(the label GAPS gates nothing on this path), and the grant is GREEN.", "GREEN")
    CKM("m_c2_genesis_of_stranger_only", MG, AINP(IS, [E("stranger", "genesis", 0, None, GEN)], MA, bas=cover([IS, AD("stranger")])), None, NT(),
       "sec 7.2 check 2: a stranger's genesis is EXCLUDED and no ledger entry, so the ledger holds "
       "no genesis and check 2 is UNKNOWN.", "PARTIAL")
    tw1m = E("iss", "annotation", 2, MGID, {"note_md": "left"})
    tw2m = E("iss", "annotation", 2, MGID, {"note_md": "right"})
    CKM("m_c2_broken_chain", MG, AINP(IS, [mgen, tw1m, tw2m], MA, bas=COV), None, NT(),
       "sec 7.2 check 2 for a grant carrying the member: BROKEN_CHAIN fails the check and leaves "
       "3 and 4 UNKNOWN, while check 6 is decided from the notices and passes.", "FAIL")
    CKM("m_c2_broken_chain_and_notice", MG, AINP(IS, [mgen, tw1m, tw2m], MA, bas=COV), None, NT(revs=[nrec(MGID, IS, 150)]),
       "sec 7.2: check 6 is gated by nothing but its inputs, so a broken ledger and a notice fail "
       "two checks and `failed` carries both tokens.", "FAIL")
    CKM("m_c3_anchor_by_successor_author_root", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS2, bn=200, ts=T0 + 100)], bas=COV2), None,
       NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.2 check 3 for a grant carrying the member: the key in office at 200 anchored the "
       "grant, which a key then out of office signed; a standing record exists, so check 4 passes "
       "and check 3 fails.", "FAIL")
    sg0 = E("iss", "genesis", 0, None, GEN)
    sg1 = E("iss", "succession", 1, eid(sg0), SUC)
    sg2 = E("iss2", "grant", 2, eid(sg1), mbody)
    SA = [anchor(eid(sg0), IS, bn=10, ts=T0), anchor(eid(sg1), IS, bn=11, ts=T0 + 1), anchor(eid(sg2), IS2, bn=200, ts=T0 + 100)]
    CKM("m_c3_successor_grant_anchored_in_office", sg2, AINP(IS, [sg0, sg1], SA, bas=COV2), None, NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.2 for a grant carrying the member, signed and anchored by the key the book put in "
       "office at 140: standing record, author in office, GREEN under the root the terms name.", "GREEN")
    CKM("m_c3_successor_grant_book_silent", sg2, AINP(IS, [sg0, sg1], SA, bas=COV2), None, NT(),
       "sec 6.4 and sec 7.2 check 3: the ledger moved the seat and the book did not; by the book "
       "the root is still in office at 200, so the successor's record is no standing record and "
       "checks 3 and 4 are UNKNOWN (sec 9, two rails).", "PARTIAL")
    CKM("m_c3_root_anchor_after_accession", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=200, ts=T0 + 100)], bas=COV2), None, NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.2 check 3: the root anchored the grant at 200 after the book moved the seat at 140; "
       "no standing record, checks 3 and 4 UNKNOWN.", "PARTIAL")
    CKM("m_c3_root_anchor_in_accession_block", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=140, ts=T0 + 40)], bas=COV2), None, NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 6.4: a record in the accession's own block reads the root as in office (strictly "
       "earlier blocks alone move the seat); the root's anchor there stands, GREEN.", "GREEN")
    CKM("m_c3_root_anchor_block_after_accession", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=141, ts=T0 + 41)], bas=COV2), None, NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 6.4: one block after the accession the root is out of office; no standing record, "
       "checks 3 and 4 UNKNOWN.", "PARTIAL")
    CKM("m_c3_anchor_beyond_toBlock", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=1500, ts=T0 + 500)], bas=basis([chainobj(1, 0, 2000, (REG,), [IS])])), None, NT(),
       "sec 7.2 check 3: a record beyond the notice basis's toBlock is no standing record, since "
       "the book was read to toBlock alone; checks 3 and 4 UNKNOWN.", "PARTIAL")
    CKM("m_c3_anchor_at_toBlock", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=1000, ts=T0 + 500)], bas=COV), None, NT(),
       "sec 7.2 check 3: a record in the notice basis's last block is within what was read.", "GREEN")
    CKM("m_c3_anchor_unproven", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=100, ts=T0, verdict="UNPROVEN")], bas=COV), None, NT(),
       "sec 7.2 check 3: an UNPROVEN record is no standing record; checks 3 and 4 UNKNOWN.", "PARTIAL")
    CKM("m_c3_anchor_void", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=100, ts=T0, verdict="VOID")], bas=COV), None, NT(),
       "sec 7.2 check 3: a VOID record is no standing record.", "PARTIAL")
    CKM("m_c3_anchor_on_other_chain", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, cid=2, bn=100, ts=T0)], bas=cover([IS], cid=(1, 2))), None, NT(),
       "sec 7.2 checks 3 and 4 for a grant carrying the member read the member's chain alone; a "
       "record on another chain is no standing record.", "PARTIAL")
    CKM("m_c3_two_records_one_in_office", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=100, ts=T0), anchor(MGID, IS2, bn=200, ts=T0 + 100)], bas=COV2), None, NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.2 check 3: two standing records, the root's at 100 and the successor's at 200; some "
       "standing record's block has the author in office, so PASS.", "GREEN")
    CKM("m_c3_two_records_none_with_author_in_office", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS2, bn=200, ts=T0 + 100), anchor(MGID, IS2, bn=300, ts=T0 + 200)], bas=COV2), None, NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.2 check 3: two standing records, both the successor's, the author out of office at "
       "both: FAIL.", "FAIL")
    rg0 = E("iss", "genesis", 0, None, GEN)
    rg1 = E("iss", "grant", 1, eid(rg0), mbody)
    rg3 = E("iss", "succession", 3, HX32(0x3333), SUC)
    CKM("m_c3_record_trimmed_by_ledger_still_standing", rg1,
       AINP(IS, [rg0, rg3], [anchor(eid(rg0), IS, bn=10, ts=T0), anchor(eid(rg3), IS, bn=50, ts=T0 + 50), anchor(eid(rg1), IS, bn=60, ts=T0 + 60)], bas=COV2), None, NT(),
       "sec 7.2 check 3 reads the anchor set as the audit received it: the ledger retires the root "
       "at the succession's record (T0+50), discards the root's later record of the grant, and, the "
       "succession's line not reaching the grant across the gap, lists the grant RETIRED; the book "
       "shows no accession, so by the book the root is in office at 60 and that record is a "
       "standing record: checks 3 and 4 PASS, check 2 PASS through the genesis under GAPS, GREEN "
       "(sec 9, two rails).", "GREEN")
    CKM("m_audit_invalid_sender_outside_senders", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS2, bn=200, ts=T0 + 100)], bas=COV), None, NT(accs=[brec(IS, IS2, IS, 140)]),
       "sec 9 with the parent's sec 9.4: a record whose sender the basis does not name is outside "
       "the basis's reach, the input has no label, and checks 2, 3 and 4 are UNKNOWN; check 6 "
       "still reads the notices.", "PARTIAL")
    CKM("m_n_other_chain", MG, MI, None, NT(cid=2),
       "sec 7.1: a notice input read from another chain does not apply to the grant; checks 3, 4 "
       "and 6 UNKNOWN, `notices` still the basis handed in.", "PARTIAL")
    CKM("m_n_other_contract", MG, MI, None, NT(at=HX20(0xCC)),
       "sec 7.1: a notice input read from another contract does not apply.", "PARTIAL")
    CKM("m_n_other_contract_with_notice", MG, MI, None, NT(revs=[nrec(MGID, IS, 150)], at=HX20(0xCC)),
       "sec 7.1: a notice in an input that does not apply withdraws nothing: UNKNOWN, never FAIL.", "PARTIAL")
    CKM("m_chain_zero", E("iss", "grant", 1, eid(mgen), dict(body, revocation={"chain": 0, "at": AT})),
       AINP(IS, [mgen], [anchor(eid(E("iss", "grant", 1, eid(mgen), dict(body, revocation={"chain": 0, "at": AT}))), IS, cid=0, bn=100, ts=T0)],
            bas=basis([chainobj(0, 0, 1000, (REG,), [IS])])), None, NT(cid=0),
       "sec 7.1: chain 0 is a legal int and a legal chain id; the member, the record and the notice "
       "basis agree on it.", "GREEN")
    # the member's form
    for slug, mem, note in (
        ("string_chain", {"chain": "1", "at": AT}, "`chain` a string"),
        ("short_at", {"chain": 1, "at": "0xaa"}, "`at` not hex20"),
        ("missing_at", {"chain": 1}, "`at` absent"),
        ("extra_member", {"chain": 1, "at": AT, "since": 5}, "a member beyond the two"),
        ("null", None, "the value null"),
        ("array", [1, AT], "an array"),
        ("bool_chain", {"chain": True, "at": AT}, "`chain` a boolean"),
    ):
        gm = E("iss", "grant", 1, eid(mgen), dict(body, revocation=mem))
        CKM("m_member_malformed_" + slug, gm, AINP(IS, [mgen, E("iss", "annotation", 2, eid(gm), {"note_md": "x"})], [anchor(eid(gm), IS, bn=100, ts=T0)], bas=COV), None, NT(),
           "sec 7.1: a revocation member present with %s is a member carried and not well-formed; "
           "the grant is read by the chain and the notice input applies to it nowhere, so checks 3, 4 "
           "and 6 are UNKNOWN and never fall back to the ledger, while check 2 reads the pile." % note, "PARTIAL")
    # the notice input's form
    for slug, nts, note in (
        ("array", [1], "an array"),
        ("missing_member", {"basis": nb(), "revocations": []}, "`accessions` absent"),
        ("extra_member", dict(NT(), extra=1), "a member beyond the three"),
        ("basis_extra_member", {"basis": dict(nb(), x=1), "revocations": [], "accessions": []}, "a basis member beyond the six"),
        ("basis_missing_member", {"basis": {k: v for k, v in nb().items() if k != "book"}, "revocations": [], "accessions": []}, "the basis's `book` absent"),
        ("basis_from_above_to", NT(fb=2000), "`fromBlock` above `toBlock`"),
        ("basis_book_from_above_to", NT(bfb=2000), "`bookFromBlock` above `toBlock`"),
        ("basis_chain_string", NT(cid="1"), "`chainId` a string"),
        ("basis_chain_negative", NT(cid=-1), "`chainId` negative"),
        ("basis_chain_above_universe", NT(cid=2 ** 53), "`chainId` at 2^53"),
        ("basis_chain_bool", NT(cid=True), "`chainId` a boolean"),
        ("basis_contract_short", NT(at="0xaa"), "`revocationContract` not hex20"),
        ("basis_book_uppercase", NT(book="0x" + "AB" * 20), "`book` with uppercase hex digits"),
        ("record_chain_differs", NT(revs=[nrec(MGID, IS, 150, cid=2)]), "a notice record's `chainId` not the basis's"),
        ("record_below_from", NT(revs=[nrec(MGID, IS, 5)], fb=10), "a notice record below `fromBlock`"),
        ("record_above_to", NT(revs=[nrec(MGID, IS, 1500)]), "a notice record above `toBlock`"),
        ("book_record_below_book_from", NT(accs=[brec(IS, IS2, IS, 5)], bfb=10), "a book record below `bookFromBlock`"),
        ("book_record_above_to", NT(accs=[brec(IS, IS2, IS, 1500)]), "a book record above `toBlock`"),
        ("record_extra_member", NT(revs=[dict(nrec(MGID, IS, 150), note="x")]), "a notice record with a member beyond the eight"),
        ("record_missing_member", NT(revs=[{k: v for k, v in nrec(MGID, IS, 150).items() if k != "tx"}]), "a notice record without `tx`"),
        ("record_verdict_unknown", NT(revs=[nrec(MGID, IS, 150, verdict="maybe")]), "a verdict outside the three tokens"),
        ("record_sender_short", NT(revs=[dict(nrec(MGID, IS, 150), sender="0x01")]), "a notice record's `sender` not hex20"),
        ("record_grant_short", NT(revs=[nrec("0x01", IS, 150)]), "a notice record's `grant` not hex32"),
        ("record_tx_short", NT(revs=[nrec(MGID, IS, 150, tx="0x01")]), "a notice record's `tx` not hex32"),
        ("record_txindex_negative", NT(revs=[nrec(MGID, IS, 150, ti=-1)]), "a notice record's `txIndex` negative"),
        ("record_logindex_float", NT(revs=[dict(nrec(MGID, IS, 150), logIndex=1.0)]), "a notice record's `logIndex` written as a fraction"),
        ("record_not_object", NT(revs=[5]), "a notice record that is no object"),
        ("records_not_array", {"basis": nb(), "revocations": {}, "accessions": []}, "`revocations` not an array"),
        ("book_record_extra_member", NT(accs=[dict(brec(IS, IS2, IS, 140), tx=HX32(1))]), "a book record with a member beyond the seven"),
        ("book_record_holder_short", NT(accs=[dict(brec(IS, IS2, IS, 140), holder="0x01")]), "a book record's `holder` not hex20"),
        ("book_record_chain_differs", NT(accs=[brec(IS, IS2, IS, 140, cid=2)]), "a book record's `chainId` not the basis's"),
        ("duplicate_notice_position", NT(revs=[nrec(MGID, IS, 150), nrec(HX32(0xDEAD), IS, 150)]), "two notice records at one position"),
        ("duplicate_notice_record", NT(revs=[nrec(MGID, IS, 150), nrec(MGID, IS, 150)]), "one notice record spelled twice"),
        ("duplicate_book_position", NT(accs=[brec(IS, IS2, IS, 140), brec(IS, AD("r1"), IS, 140)]), "two book records at one position"),
    ):
        CKM("m_n_invalid_" + slug, MG, MI, None, nts,
           "sec 6.3: a notice input with %s is an invalid notice input, read as null: checks 3, 4 and "
           "6 UNKNOWN, `notices` null." % note, "PARTIAL")
    CKN_raw("m_n_invalid_null", MG, MI, b"null",
            "sec 6.3: a notices file holding the JSON value null is no notice input; checks 3, 4 and 6 "
            "UNKNOWN, `notices` null.")
    CKM("m_n_two_notice_positions_same_block", MG, MI, None, NT(revs=[nrec(HX32(0xDEAD), IS, 150, ti=0), nrec(HX32(0xBEEF), IS, 150, ti=1)]),
       "sec 6.3: two notice records in one block at two transactions are two positions; neither "
       "names the grant.", "GREEN")
    CKM("m_n_same_position_across_arrays", MG, MI2, None, NT(revs=[nrec(MGID, IS2, 140, ti=0, li=0)], accs=[brec(IS, IS2, IS, 140, ti=0, li=0)]),
       "sec 6.3: the position rule is within each array; a notice and an accession at one position "
       "are two records, and the accession is not before the notice, so the root is in office and "
       "the successor's notice withdraws nothing.", "GREEN")
    CKN_raw("m_n_not_json", MG, MI, b"{ this is not json",
            "HARNESS-KIT2 with sec 6.3: a readable notices file that is not JSON is an invalid notice "
            "input, an answer and never misuse; checks 3, 4 and 6 UNKNOWN, `notices` null.")
    CKN_raw("m_n_not_utf8", MG, MI, b"\xff\xfe{}",
            "HARNESS-KIT2 with sec 6.3: a notices file that is not UTF-8 is read as no notice input.")
    CKN_raw("m_n_repeated_member", MG, MI,
            ('{"basis": %s, "revocations": [], "accessions": [], "accessions": []}' % json.dumps(nb())).encode(),
            "HARNESS-KIT2 with sec 6.3: a notices file repeating a member name is no notice input.")
    CKN_raw("m_n_integer_as_float", MG, MI,
            json.dumps({"basis": dict(nb(), toBlock=1000.0), "revocations": [], "accessions": []}).encode(),
            "HARNESS-KIT2 with sec 6.3: an integer written as 1000.0 is no int of sec 3.2.")
    # the root the book is read under
    ML = E("iss", "grant", 1, eid(mgen), dict(mbody, ledger=IS))
    CKM("m_root_from_ledger_member_no_audit", ML, None, None, NT(revs=[nrec(eid(ML), IS2, 150)], accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.1 with no audit input: the book is read under the grant's `ledger` member; the key it "
       "put in office notices the grant, FAIL at check 6.", "FAIL")
    CKM("m_root_from_author_no_audit", MG, None, None, NT(revs=[nrec(MGID, IS2, 150)], accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.1 with no audit input and no `ledger` member: the book is read under the grant's "
       "author.", "FAIL")
    CKM("m_root_from_author_no_audit_other_root_in_book", MG, None, None, NT(revs=[nrec(MGID, IS2, 150)], accs=[brec(AD("r1"), IS2, AD("r1"), 140)]),
       "sec 6.4: book records of another root move nothing under this one; the successor's notice "
       "is a stranger's here and check 6 passes, checks 2, 3 and 4 UNKNOWN with no audit input.", "PARTIAL")
    CKM("m_root_from_audit_not_author", sg2, AINP(IS, [sg0, sg1], SA, bas=COV2), None, NT(revs=[nrec(eid(sg2), IS, 130)], accs=[brec(IS, IS2, IS, 140)]),
       "sec 7.1: with an audit input the book is read under its root, the terms' statement, and "
       "never under the grant's author; under the root the ceding key is in office at 130, before "
       "the accession, and its notice of the successor's grant counts: FAIL (under the author as "
       "root the notice would be a stranger's).", "FAIL")
    # a grant carrying no member beside a notice input
    CKM("nonmember_notices_carried", G, AINP(IS, [gen, ann], A_G, bas=COV), None, NT(),
       "sec 7.1 and sec 7.3: a grant carrying no member is read by the ledger whatever notice input "
       "the caller holds; the result's `notices` carries the notice basis all the same.", "GREEN")
    CKM("nonmember_notice_naming_it_ignored", G, AINP(IS, [gen, ann], A_G, bas=COV), None, NT(revs=[nrec(eid(G), IS, 150)]),
       "sec 7.1: a notice naming a grant that carries no member is read nowhere; GREEN by the "
       "ledger.", "GREEN")
    CKM("nonmember_invalid_notices", G, AINP(IS, [gen, ann], A_G, bas=COV), None, NT(cid="x"),
       "sec 7.3: an invalid notice input beside a grant carrying no member leaves `notices` null and "
       "moves no check.", "GREEN")
    CKM("m_c3_book_record_of_other_root_ignored", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS, bn=200, ts=T0 + 100)], bas=COV2), None,
        NT(accs=[brec(AD("r1"), IS2, AD("r1"), 140)]),
        "sec 6.4: a book record of another root moves nothing under this one, by block as by "
        "position; the root is in office at 200 and its record stands: GREEN.", "GREEN")
    MGW = E("iss", "grant", 1, eid(mgen), dict(mbody, window={"from": 1000, "to": 2000}))
    CKM("m_c5_window_now_null", MGW, AINP(IS, [mgen, E("iss", "annotation", 2, eid(MGW), {"note_md": "x"})], [anchor(eid(MGW), IS, bn=100, ts=T0)], bas=COV), None, NT(),
        "sec 7.2 check 5 for a grant carrying the member: a window with no trusted time is UNKNOWN, "
        "the other five checks passing from the chain.", "PARTIAL")
    CKM("m_window_expired_and_notice", E("iss", "grant", 1, eid(mgen), dict(mbody, window={"from": 1000, "to": 2000})), None, 5,
       NT(revs=[nrec(eid(E("iss", "grant", 1, eid(mgen), dict(mbody, window={"from": 1000, "to": 2000}))), IS, 150)]),
       "sec 7.2 for a grant carrying the member: checks 5 and 6 fail together with no audit input.", "FAIL")
    # ---- seat D additions (round 1): boundaries no case witnessed on one side ----
    IS3 = HX20(0x33)
    COV3 = cover([IS, IS2, IS3])
    CKM("d_n_from_equals_to", MG, MI, None, NT(fb=1000, revs=[nrec(MGID, IS, 1000)]),
        "sec 6.3: `fromBlock` equal to `toBlock` is a lawful basis of one block; its notice "
        "counts.", "FAIL")
    CKM("d_n_bookfrom_equals_to", MG, MI, None, NT(bfb=1000),
        "sec 6.3: `bookFromBlock` equal to `toBlock` is lawful.", "GREEN")
    CKM("d_n_book_record_at_bookfrom", MG, MI2, None,
        NT(bfb=10, accs=[brec(IS, IS2, IS, 10)], revs=[nrec(MGID, IS2, 150)]),
        "sec 6.3: a book record in the block `bookFromBlock` itself is inside the book's range; "
        "the successor it seats notices the grant: FAIL.", "FAIL")
    CKM("d_n_notice_below_bookfrom_above_from", MG, MI, None,
        NT(fb=0, bfb=500, revs=[nrec(MGID, IS, 100)]),
        "sec 6.3: a notice record's range is `fromBlock` to `toBlock`; `bookFromBlock` bounds "
        "book records alone.", "FAIL")
    CKM("d_n_two_notices_one_tx_two_logs", MG, MI, None,
        NT(revs=[nrec(HX32(0xDEAD), IS, 150, ti=2, li=0), nrec(MGID, IS, 150, ti=2, li=1)]),
        "sec 6.3: two notice records of one transaction at two log indices are two positions.", "FAIL")
    CKM("d_n_two_notices_one_log_two_txs", MG, MI, None,
        NT(revs=[nrec(HX32(0xDEAD), IS, 150, ti=1, li=3), nrec(MGID, IS, 150, ti=2, li=3)]),
        "sec 6.3: two notice records with one block and one log index at two transactions are "
        "two positions.", "FAIL")
    CKM("d_n_two_notices_two_blocks_one_tx_log", MG, MI, None,
        NT(revs=[nrec(HX32(0xDEAD), IS, 150, ti=1, li=3), nrec(MGID, IS, 151, ti=1, li=3)]),
        "sec 6.3: two notice records at one txIndex and logIndex in two blocks are two positions.", "FAIL")
    CKM("d_o_two_accessions_one_tx", MG, AINP(IS, [mgen, mann], MA, bas=COV3), None,
        NT(accs=[brec(IS, IS2, IS, 140, ti=0, li=0), brec(IS2, IS3, IS, 140, ti=0, li=1)],
           revs=[nrec(MGID, IS3, 150)]),
        "sec 6.4: two accessions of one transaction at two log indices; the last before the notice "
        "seats the third key, whose notice counts.", "FAIL")
    CKM("d_o_second_key_after_third_seated", MG, AINP(IS, [mgen, mann], MA, bas=COV3), None,
        NT(accs=[brec(IS, IS2, IS, 140), brec(IS2, IS3, IS, 160)], revs=[nrec(MGID, IS2, 170)]),
        "sec 6.4: the key in office is the `to` of the last book record before the position; the "
        "second key's notice after the third is seated withdraws nothing.", "GREEN")
    CKM("d_o_second_key_between_accessions", MG, AINP(IS, [mgen, mann], MA, bas=COV3), None,
        NT(accs=[brec(IS, IS2, IS, 140), brec(IS2, IS3, IS, 160)], revs=[nrec(MGID, IS2, 150)]),
        "sec 6.4: between the two accessions the second key is in office and its notice counts.", "FAIL")
    CKM("d_o_holder_not_in_office", MG, MI2, None,
        NT(accs=[brec(AD("stranger"), IS2, IS, 140)], revs=[nrec(MGID, IS2, 150)]),
        "sec 6.4: the book lineage tests nothing about its records; a `holder` that is not the key "
        "in office still seats `to`, whose notice counts.", "FAIL")
    CKM("d_o_seat_returns_to_root", MG, MI2, None,
        NT(accs=[brec(IS, IS2, IS, 140), brec(IS2, IS, IS, 160)], revs=[nrec(MGID, IS, 170)]),
        "sec 6.4: a `to` seen before (the root again) is in office after its record; the root's "
        "notice at 170 counts.", "FAIL")
    CKM("d_c3_anchor_before_book_deployed", MG, MI, None, NT(bfb=500),
        "sec 7.2 check 3: a standing record has no lower bound; an anchor at block 100 before the "
        "book's first block reads the root in office: GREEN.", "GREEN")
    CKM("d_c3_only_descendant_anchored", MG, AINP(IS, [mgen, mann], [anchor(eid(mann), IS, bn=100, ts=T0)], bas=COV), None, NT(),
        "sec 7.2 checks 3 and 4: standing records are records of the grant's own `entry_id`; an "
        "anchor of a later entry on its line anchors it under sec 6.2 and is no standing record: "
        "UNKNOWN.", "PARTIAL")
    CKM("d_c3_two_accessions_one_block_anchor_next_block", MG, AINP(IS, [mgen, mann], [anchor(MGID, IS3, bn=141, ts=T0 + 41)], bas=COV3), None,
        NT(accs=[brec(IS, IS2, IS, 140, ti=0), brec(IS2, IS3, IS, 140, ti=1)]),
        "sec 6.4 by block: the last of two accessions in block 140 seats the third key at 141; "
        "its record of the grant stands and the author is out of office: check 3 FAIL, check 4 PASS.", "FAIL")
    CKM("d_c6_unproven_by_stranger_ignored", MG, MI, None, NT(revs=[nrec(MGID, AD("stranger"), 150, verdict="UNPROVEN")]),
        "sec 7.2 check 6: an UNPROVEN notice by a key not in office is no notice of this check; PASS.", "GREEN")
    CKM("d_c6_unproven_by_retired_key_ignored", MG, MI2, None,
        NT(revs=[nrec(MGID, IS, 150, verdict="UNPROVEN")], accs=[brec(IS, IS2, IS, 140)]),
        "sec 7.2 check 6: an UNPROVEN notice by the retired root withdraws nothing and leaves no "
        "doubt; PASS.", "GREEN")
    CKM("d_c1_not_a_grant_notices_carried", E("iss", "history", 1, eid(mgen),
                                             {"content": WK, "mode": {"mark": "hand", "toolchain": HX32(1)}}),
        MI, None, NT(),
        "sec 7.3: `notices` is the notice basis whenever a valid input was handed in, check 1 "
        "failing included.", "FAIL")
    v1g = E1("iss", "genesis", 0, None, GEN)
    v1m = E1("iss", "grant", 1, eid(v1g), dict(body, revocation={"chain": 1, "at": AT}))
    CKM("d_v1_member_no_audit_notice", v1m, None, None, NT(revs=[nrec(eid(v1m), IS, 150)]),
        "sec 7.1 and 7.2: a zikaron/1 grant carrying the member is read by the chain like any "
        "other; the author's counted notice fails check 6 with no audit input.", "FAIL")
    v1j = E1("iss", "grant", 1, eid(v1g), dict(body, revocation={"chain": 1, "at": AT}, ledger="not-an-address"))
    CKM("d_v1_member_ledger_not_hex20_root_author", v1j, None, None,
        NT(accs=[brec(IS, IS2, IS, 140)], revs=[nrec(eid(v1j), IS2, 150)]),
        "sec 7.1: a `ledger` member whose value is not hex20 (an extra member of a zikaron/1 "
        "grant) names no root; the book is read under the author, whose successor notices the "
        "grant: FAIL.", "FAIL")
    CKM("d_v1_member_with_v1_pile", v1m, AINP(IS, [v1g], [anchor(eid(v1m), IS, bn=100, ts=T0)], bas=COV), None, NT(),
        "sec 7.2 for a zikaron/1 grant carrying the member over an uncontinued zikaron/1 pile: "
        "the table's check-3 row for a zikaron/1 entry is not read; checks 3, 4 and 6 are decided "
        "from the chain.")
    ML2 = E("iss", "grant", 1, eid(mgen), dict(mbody, ledger=AD("r4")))
    CKM("d_root_from_ledger_member_not_author_no_audit", ML2, None, None,
        NT(accs=[brec(AD("r4"), IS2, AD("r4"), 140)], revs=[nrec(eid(ML2), IS2, 150)]),
        "sec 7.1 with no audit input: the book is read under the grant's `ledger` member, which "
        "is not its author; that root's successor notices the grant: FAIL.", "FAIL")
    CKM("d_root_from_ledger_member_audit_invalid", ML2,
        AINP(IS, [mgen], [anchor(eid(ML2), IS2, bn=100, ts=T0)], bas=COV), None,
        NT(accs=[brec(AD("r4"), IS2, AD("r4"), 140)], revs=[nrec(eid(ML2), IS2, 150)]),
        "sec 7.1: an invalid audit input names no root (its sender lies outside `senders`); the "
        "book is read under the grant's `ledger`: FAIL at check 6.", "FAIL")
    # HARNESS-KIT2: options stand in any relative order after the positional argument
    i = seqno("checks")
    base = "checks/c%03d_d_option_order" % i
    gp_o = W(base + ".grant.zk1", MG)
    np_o = WJ(base + ".notices.json", NT(revs=[nrec(MGID, IS, 150)]))
    ap_o = WJ(base + ".audit.json", MI)
    CASE("checks", gp_o, "grant-check", ["grant-check", gp_o, "--now", "5", "--notices", np_o, "--audit", ap_o],
         "HARNESS-KIT2: option tokens stand in any relative order after the positional argument.",
         grant_check(MG, MI, 5, NT(revs=[nrec(MGID, IS, 150)])), audit_path=ap_o, notices_path=np_o, now=5)

    # ---- round 1: the root through the book (seat F), a successor's record over an empty pile (Node) --
    IS3b = HX20(0x33)
    CKM("f_root_via_book_successor_grant_noticed_by_next", sg2, None, None,
        NT(accs=[brec(IS, IS2, IS, 140), brec(IS2, IS3b, IS, 160)], revs=[nrec(eid(sg2), IS3b, 170)]),
        "sec 7.1 with no audit input: the successor's grant carries no `ledger`, and the book names "
        "the root its key acceded under; the key the book then seated notices the grant: FAIL at "
        "check 6 (under the author as root the notice would be a stranger's).", "FAIL")
    CKM("f_root_via_book_retired_key_notice_ignored", sg2, None, None,
        NT(accs=[brec(IS, IS2, IS, 140), brec(IS2, IS3b, IS, 160)], revs=[nrec(eid(sg2), IS2, 170)]),
        "sec 7.1 with no audit input: the book names the root; the retired successor's own notice of "
        "its grant, after the seat moved on, withdraws nothing (under the author as root it would "
        "count).", "PARTIAL")
    CKM("f_root_via_book_least_position", sg2, None, None,
        NT(accs=[brec(AD("r4"), IS2, AD("r4"), 150), brec(IS, IS2, IS, 140)], revs=[nrec(eid(sg2), IS2, 170), nrec(eid(sg2), AD("r4"), 145)]),
        "sec 7.1: two book records name the author as `to` under two roots; the least in position "
        "decides, so the book is read under the first root, whose later record keeps the author in "
        "office: its notice counts, FAIL.", "FAIL")
    CKM("f_root_via_book_before_ledger_member", E("iss2", "grant", 2, eid(sg1), dict(mbody, ledger=IS2)), None, None,
        NT(accs=[brec(IS, IS2, IS, 140), brec(IS2, IS3b, IS, 160)], revs=[nrec(eid(E("iss2", "grant", 2, eid(sg1), dict(mbody, ledger=IS2))), IS3b, 170)]),
        "sec 7.1: the book's root for the author comes before the grant's own `ledger` member, which "
        "names the author itself; a holder of a retired key cannot read the book under itself: "
        "FAIL.", "FAIL")
    CKM("m_c3_successor_record_empty_pile", sg2, AINP(IS, [], [anchor(eid(sg2), IS2, bn=200, ts=T0 + 100)], bas=COV2), None,
        NT(accs=[brec(IS, IS2, IS, 140)]),
        "sec 7.2 checks 3 and 4 over an empty pile: the key the book seated at 140 anchored its own "
        "grant at 200; the record stands though no succession of the pile names the key, so checks "
        "3 and 4 PASS, check 2 UNKNOWN for want of a genesis: PARTIAL.", "PARTIAL")
    # ---- round 1: a rotated issuer's spine (seat A, L1), a forger choosing the rail (A, L2), no lower bound (A, M04) --
    rs0 = E("iss", "genesis", 0, None, GEN)
    rs1 = E("iss", "annotation", 1, eid(rs0), {"note_md": "one"})
    rs2 = E("iss", "annotation", 2, eid(rs1), {"note_md": "two"})
    rs3 = E("iss", "succession", 3, eid(rs2), SUC)
    rs4 = E("iss2", "grant", 4, eid(rs3), mbody)
    RSA = [anchor(eid(rs2), IS, bn=50, ts=T0 + 50), anchor(eid(rs3), IS, bn=60, ts=T0 + 60), anchor(eid(rs4), IS2, bn=200, ts=T0 + 100)]
    RSN = NT(accs=[brec(IS, IS2, IS, 140)])
    CKM("a_rotated_issuer_full_pile", rs4, AINP(IS, [rs0, rs1, rs2, rs3], RSA, bas=COV2), None, RSN,
        "sec 7.2 for a successor's grant carrying the member over the whole ledger: COMPLETE, the "
        "genesis a ledger entry, the successor's record standing: GREEN.", "GREEN")
    CKM("a_rotated_issuer_spine_green", rs4, AINP(IS, [rs0, rs3], RSA, bas=COV2), None, RSN,
        "sec 7.2 check 2 reads the byte string: the spine (genesis and succession) of a root that "
        "handed off without anchoring its genesis leaves the genesis RETIRED under the parent's "
        "sec 7.4 (the succession's prev is not held), yet the genesis passes sec 4.3 with the root "
        "as author, so check 2 PASSES and the grant is GREEN; a reader of the report's lists alone "
        "answers PARTIAL.", "GREEN")
    CKM("a_rotated_issuer_spine_without_genesis", rs4, AINP(IS, [rs3], RSA, bas=COV2), None, RSN,
        "sec 7.2 check 2: a pile holding the succession alone has no genesis of the root: UNKNOWN, "
        "PARTIAL.", "PARTIAL")
    fg = E("stranger", "grant", 1, eid(mgen), dict(mbody, ledger=IS))
    CKM("a_forged_member_grant_excluded_fail", fg, AINP(IS, [mgen, mann], MA, bas=cover([IS, AD("stranger")])), None, NT(),
        "sec 7.2 check 3 for a grant carrying the member: check 2 PASS, the stranger's grant stands "
        "EXCLUDED, and the stranger is no key of the book lineage: FAIL, as the ledger path convicts "
        "it; the member takes nothing from the conviction.", "FAIL")
    CKM("a_excluded_but_seated_by_book_not_convicted", fg, AINP(IS, [mgen, mann], MA, bas=cover([IS, AD("stranger")])), None,
        NT(accs=[brec(IS, AD("stranger"), IS, 140)]),
        "sec 7.2 check 3: the same EXCLUDED grant where the book seated its signer: no conviction; "
        "no standing record either, so UNKNOWN, PARTIAL.", "PARTIAL")
    CKM("a_forged_member_grant_badge_alone_not_convicted", fg, AINP(IS, [], MA, bas=cover([IS, AD("stranger")])), None, NT(),
        "sec 7.2 check 3: over an empty pile check 2 is UNKNOWN, so the EXCLUDED clause convicts "
        "nobody; checks 3 and 4 UNKNOWN, PARTIAL.", "PARTIAL")
    CKM("a_c3_anchor_below_notice_from_block", MG, MI, None, NT(fb=500),
        "sec 7.2 check 3: a standing record has no lower block bound; the notice basis's fromBlock "
        "bounds notices alone, and the record at block 100 stands: GREEN.", "GREEN")
    # ---- round 2, seat D: the folded boundaries witnessed on their other side ----------------
    # the EXCLUDED conviction: the old segment's EXCLUDED list (either report)
    d2s = E1("stranger", "grant", 1, eid(CONT_PILE[0]), dict(body, revocation={"chain": 1, "at": AT}))
    d2_cov = cover([IS, AD("stranger")])
    CKM("d2_v1_stranger_member_grant_old_excluded_fail", d2s,
        AINP(IS, CONT_PILE, CONT_ANC + [anchor(eid(d2s), AD("stranger"), bn=11, ts=T0 + 1, tx=HX32(0xD201))], bas=d2_cov),
        None, NT(),
        "sec 7.2 check 3 for a grant carrying the member: a stranger's zikaron/1 grant anchored before "
        "the cut is an old byte string, EXCLUDED of the old segment's report; check 2 PASS and the "
        "stranger is no key of the book lineage: FAIL (a reader of the new report's EXCLUDED alone "
        "answers PARTIAL).", "FAIL")
    # the same forgery after the cut: MALFORMED of the new report, the table's other conviction
    CKM("d2_v1_stranger_member_grant_after_cut_convicted", d2s,
        AINP(IS, CONT_PILE, CONT_ANC, bas=d2_cov), None, NT(),
        "sec 7.2 check 3 for a grant carrying the member: a stranger's zikaron/1 grant no record anchors "
        "at or before the cut is a byte string of the new segment, which it fails (MALFORMED outside the "
        "old segment, the table's other unconditional conviction); check 2 PASS and the stranger is no "
        "key of the book lineage: FAIL, as the twin without the member reads by the table.", "FAIL")
    d2s_plain = E1("stranger", "grant", 1, eid(CONT_PILE[0]), body)
    CK("d2_v1_stranger_grant_after_cut_table_fail", d2s_plain, AINP(IS, CONT_PILE, CONT_ANC, bas=d2_cov), None,
       "sec 7.2 table, check 3: the same zikaron/1 forgery without the member stands in MALFORMED of "
       "the report of I' outside the old segment: FAIL.", "FAIL")
    d2o = E1("iss", "grant", 3, eid(CONT_PILE[1]), dict(body, revocation={"chain": 1, "at": AT}))
    CKM("d2_v1_own_key_member_grant_after_cut", d2o,
        AINP(IS, CONT_PILE, CONT_ANC + [anchor(eid(d2o), IS, bn=100, ts=T0 + 100)], bas=COV), None, NT(),
        "sec 7.2 for a grant carrying the member: the key in office signs a zikaron/1 grant after its "
        "ledger moved (MALFORMED of the new report, which the table convicts); the member's checks read "
        "the book and the standing record: GREEN.", "GREEN")
    # a stranger seated under its own root: sec 7.1 reads the book under that root, and the audit's differs
    CKM("d2_excluded_seated_under_other_root_fails_by_root", fg, AINP(IS, [mgen, mann], MA, bas=cover([IS, AD("stranger")])), None,
        NT(accs=[brec(AD("r4"), AD("stranger"), AD("r4"), 140)]),
        "sec 7.1 with sec 7.2 check 3: a book record of another root seats the stranger there, so the "
        "book is read under that root, where the stranger is seated and the conviction does not reach "
        "it; the audit's root is the issuer's, which differs from the root sec 7.1 derives: FAIL.",
        "FAIL")
    # RETIRED is no conviction: a key the book never seated, signing above its retiring seq
    SUC4 = dict(SUC, to=AD("r4"))
    rt0 = E("iss", "genesis", 0, None, GEN)
    rt1 = E("iss", "succession", 1, eid(rt0), SUC)
    rt2 = E("iss2", "succession", 2, eid(rt1), SUC4)
    rtg = E("iss2", "grant", 3, eid(rt2), mbody)
    RTA = [anchor(eid(rt1), IS, bn=20, ts=T0 + 20), anchor(eid(rt2), IS2, bn=30, ts=T0 + 30),
           anchor(eid(rtg), IS2, bn=200, ts=T0 + 200)]
    CKM("d2_retired_grant_unseated_key_not_convicted", rtg,
        AINP(IS, [rt0, rt1, rt2], RTA, bas=cover([IS, IS2, AD("r4")])), None,
        NT(accs=[brec(IS, AD("r4"), IS, 140)]),
        "sec 7.2 check 3 for a grant carrying the member: the intermediate key signed above its retiring "
        "seq (RETIRED, which the table fails on COMPLETE and this path convicts nowhere, the two rails of "
        "sec 9) and the book went from the root to the key now in office; the key is out of office at its "
        "record: UNKNOWN, PARTIAL.",
        "PARTIAL")
    # check 2 reads auth(0) of the segment this law reads: the continuation's author, not the root
    d2c = E("iss2", "grant", 1, eid(c_gen2), mbody)
    CKM("d2_continued_member_grant_genesis_of_continuation_author", d2c,
        AINP(IS, HPILE, HANC + [anchor(eid(d2c), AD("iss2"), bn=100, ts=T0 + 100)], bas=cover([IS, AD("iss2")])), None,
        NT(accs=[brec(IS, AD("iss2"), IS, 50)]),
        "sec 7.2 check 2 for a grant carrying the member over a continued ledger: the genesis passing "
        "zikaron/2 sec 4.3 is the continuation's, whose author is auth(0) of the new segment though not "
        "the root; check 2 PASS, the book seated the key at 50, its record at 100 stands: GREEN.", "GREEN")
    # a genesis is no spine: an honest successor beside a genesis-only pile, the book silent
    CKM("d2_genesis_only_successor_book_silent_fail", sg2, AINP(IS, [sg0], SA, bas=COV2), None, NT(),
        "sec 7.2 check 3 for a grant carrying the member: the pile holds the genesis and not the "
        "succession, so the successor's grant stands EXCLUDED, check 2 PASS, and the silent book never "
        "seated it: FAIL, as the table answers over the same pile.", "FAIL")
    CKM("d2_genesis_only_successor_book_seated", sg2, AINP(IS, [sg0], SA, bas=COV2), None, NT(accs=[brec(IS, IS2, IS, 140)]),
        "sec 7.2 for a grant carrying the member: the same pile where the book seated the successor at "
        "140: no conviction, its record at 200 stands: GREEN.", "GREEN")
    # the least-positioned book record decides the root, and the last one would answer otherwise
    CKM("d2_root_via_book_least_position_decides", sg2, None, None,
        NT(accs=[brec(AD("r4"), IS2, AD("r4"), 150), brec(IS, IS2, IS, 140)], revs=[nrec(eid(sg2), IS2, 145)]),
        "sec 7.1: two book records name the author as `to` under two roots; the least in position "
        "(block 140, root iss) decides, under which the author is in office at 145 and its notice "
        "counts: FAIL (under the later record's root the author is not yet in office at 145 and the "
        "notice withdraws nothing).", "FAIL")
    # the conviction comes before check 3's gate on N applying: a forger who writes a member that no
    # input applies to, or no member shape at all, is convicted as the table convicts the same bytes
    FCOV = cover([IS, AD("stranger")])
    fgo = E("stranger", "grant", 1, eid(mgen), dict(body, ledger=IS, revocation={"chain": 999, "at": AT}))
    CKM("d2_forged_member_other_chain_convicted", fgo, AINP(IS, [mgen, mann], MA, bas=FCOV), None, NT(),
        "sec 7.2 check 3 for a grant carrying the member: the stranger's EXCLUDED grant names a chain no "
        "input applies to; the conviction comes before the gate on N applying, no book is read and none "
        "seats the stranger: FAIL, as the same bytes without the member read by the table.", "FAIL")
    fgn = E("stranger", "grant", 1, eid(mgen), dict(body, ledger=IS, revocation=None))
    CKM("d2_forged_member_null_convicted", fgn, AINP(IS, [mgen, mann], MA, bas=FCOV), None, NT(),
        "sec 7.1 and 7.2: a `revocation` key with value null carries the member and is not well-formed, so "
        "N applies to nothing; the conviction comes first all the same: FAIL.", "FAIL")
    CKM("d2_forged_member_notices_null_convicted", fg, AINP(IS, [mgen, mann], MA, bas=FCOV), None, None,
        "sec 7.2 check 3: the well-formed forgery of a_forged_member_grant_excluded_fail with no notice "
        "input: the conviction comes before the gate on N applying, and with no book read none seats "
        "the stranger: FAIL.", "FAIL")

    # ---- round 2, seats A, F, Node: the book's root before the audit's, continued ledgers, check 4 beside a conviction --
    IS3c = HX20(0x33)
    CKM("f2_audit_under_author_conflicts_with_book", sg2,
        AINP(IS2, [], [anchor(eid(sg2), IS2, bn=200, ts=T0 + 100)], bas=cover([IS2])), None,
        NT(accs=[brec(IS, IS2, IS, 140), brec(IS2, IS3c, IS, 160)], revs=[nrec(eid(sg2), IS3c, 170)]),
        "sec 7.1 and 7.2 check 3: a reader built the audit input under the successor's own key, and the "
        "book names the seat that key acceded to; the book's root comes first and the audit's root "
        "differs from it: check 3 FAIL; the key the book seated next noticed the grant: check 6 FAIL "
        "(under the audit's root the notice would be a stranger's and the grant PARTIAL).", "FAIL")
    CKM("f2_thief_genesis_under_retired_key_fails", MG, MI, None,
        NT(accs=[brec(AD("r4"), IS, AD("r4"), 50), brec(IS, AD("r1"), AD("r4"), 80)], revs=[nrec(MGID, AD("r1"), 120)]),
        "sec 7.1 and 7.2: the issuer's key acceded under another root at 50 and that seat moved on at 80; "
        "a ledger opened under the key and audited under it is not the seat the book gave the key: "
        "check 3 FAIL, its record at 100 is no standing record (the key then in office is another), "
        "and the key in office noticed the grant at 120: check 6 FAIL.", "FAIL")
    CKM("f2_audit_root_agrees_with_book", sg2, AINP(IS, [sg0, sg1], SA, bas=COV2), None,
        NT(accs=[brec(IS, IS2, IS, 140)]),
        "sec 7.1: the terms' root and the book's root for the signer agree; no conflict, and the "
        "successor's record at 200 stands: GREEN.", "GREEN")
    cg2 = E("iss2", "grant", 1, eid(c_gen2), mbody)
    HN = NT(accs=[brec(IS, AD("iss2"), IS, 140)])
    CKM("a2_cont_member_noticed", cg2, AINP(IS, HPILE, HANC + [anchor(eid(cg2), AD("iss2"), bn=150, ts=T0 + 150)], bas=cover([IS, AD("iss2")])), None,
        NT(accs=[brec(IS, AD("iss2"), IS, 140)], revs=[nrec(eid(cg2), AD("iss2"), 160)]),
        "sec 7.2 check 6 on a continued ledger: the continuation's author, seated by the book at 140, "
        "notices its grant at 160: FAIL.", "FAIL")
    fgv1 = E1("stranger", "grant", 1, eid(v1h), mbody)
    CKM("a2_cont_forged_v1_member_excluded_old", fgv1,
        AINP(IS, HPILE, HANC + [anchor(eid(fgv1), AD("stranger"), bn=12, ts=T0 + 2, tx=HX32(0xA2F1))], bas=cover([IS, AD("iss2"), AD("stranger")])), None, HN,
        "sec 7.2 check 3: a stranger's zikaron/1 grant carrying the member, anchored at or before the cut, "
        "stands EXCLUDED in the old segment's report; the book never seated the stranger: FAIL.", "FAIL")
    CKM("a2_cont_forged_v1_member_malformed_new", fgv1, AINP(IS, HPILE, HANC, bas=cover([IS, AD("iss2"), AD("stranger")])), None, HN,
        "sec 7.2 check 3: the same forgery with no record at or before the cut is a byte string of the "
        "new segment, MALFORMED there, the table's other unconditional conviction: FAIL.", "FAIL")
    rg1c = E("iss", "grant", 1, eid(c_gen2), mbody)
    CKM("a2_cont_root_key_member_grant_in_office", rg1c,
        AINP(IS, HPILE, HANC + [anchor(eid(rg1c), IS, bn=100, ts=T0 + 100)], bas=cover([IS, AD("iss2")])), None, HN,
        "sec 7.2 on a continued ledger: the zikaron/1 root signs a zikaron/2 grant carrying the member "
        "(EXCLUDED of the new report, the ledger having moved) and anchors it at 100 while the book still "
        "holds it (the accession is at 140); the root is the first key of the book lineage, so no "
        "conviction, and its record stands: GREEN.", "GREEN")
    sgf = E("stranger", "grant", 1, eid(mgen), mbody)
    CKM("a2_holder_only_not_lineage", sgf, AINP(IS, [mgen], [anchor(eid(mgen), IS, bn=10, ts=T0)], bas=cover([IS])), None,
        NT(accs=[brec(AD("stranger"), AD("k2"), IS, 140)]),
        "sec 6.4 with sec 7.2 check 3: the book lineage is the root and the `to` of its records, never a "
        "`holder`; a record whose holder is the stranger seats k2, and the stranger's EXCLUDED grant is "
        "convicted: FAIL.", "FAIL")
    CKM("a2_forged_excluded_anchored_by_key_in_office", sgf,
        AINP(IS, [mgen], [anchor(eid(mgen), IS, bn=10, ts=T0), anchor(eid(sgf), IS, bn=100, ts=T0 + 100)], bas=cover([IS])), None, NT(),
        "sec 7.2 checks 3 and 4: a stranger's EXCLUDED grant whose hash the key in office anchored; check 3 "
        "FAIL by the conviction, check 4 PASS by the standing record, decided by its own row.", "FAIL")
    rgen2 = E("iss", "genesis", 0, None, GEN)
    rsuc2 = E("iss", "succession", 1, eid(rgen2), SUC)
    rlate = E("iss", "grant", 2, eid(rsuc2), mbody)
    RANC2 = [anchor(eid(rgen2), IS, bn=10, ts=T0), anchor(eid(rsuc2), IS, bn=11, ts=T0 + 1)]
    CKM("a2_retired_member_grant_book_silent", rlate,
        AINP(IS, [rgen2, rsuc2], RANC2 + [anchor(eid(rlate), IS, bn=100, ts=T0 + 100)], bas=cover([IS, IS2])), None, NT(),
        "sec 9 (two rails): the root signs above its retiring seq, RETIRED on a COMPLETE record, and the "
        "book never moved; the member path reads the book, under which the root is in office and its "
        "record stands: GREEN, where the table reads FAIL.", "GREEN")
    lp2 = E("iss2", "grant", 5, HX32(0x77), mbody)
    CKM("a2_least_position_discriminates", lp2, None, None,
        NT(revs=[nrec(eid(lp2), IS2, 170)],
           accs=[brec(IS, IS2, IS, 140), brec(AD("issb"), IS2, AD("issb"), 150), brec(IS2, AD("k2"), AD("issb"), 160)]),
        "sec 7.1: the least-positioned record naming the author as `to` is under the first root, where the "
        "author is still in office at 170 and its notice counts: FAIL; under the later root the seat moved "
        "on at 160 and the notice would withdraw nothing.", "FAIL")

    # ---- round 3, seat D: the round-2 boundaries on the side no case held ----------------
    # the book lineage is read only where N applies: an input of another chain whose book seats the forger
    CKM("d3_forged_member_other_chain_book_not_read", fgo, AINP(IS, [mgen, mann], MA, bas=FCOV), None,
        NT(accs=[brec(IS, AD("stranger"), IS, 50)]),
        "sec 7.2 check 3 for a grant carrying the member: the forgery names chain 999 and the notice input "
        "is chain 1's, whose book seats the stranger under the issuer's root at 50; N does not apply, so no "
        "book is read and nothing seats the stranger: FAIL by the conviction (a reader of the book of an "
        "input that does not apply answers PARTIAL).", "FAIL")
    # the MALFORMED conviction needs a continued ledger: a zikaron/1 grant over a ledger that never continued
    d3v1 = E1("stranger", "grant", 1, eid(mgen), mbody)
    CKM("d3_v1_member_grant_uncontinued_v2_ledger_not_convicted", d3v1, AINP(IS, [mgen, mann], MA, bas=FCOV), None, NT(),
        "sec 7.2 check 3 for a grant carrying the member: a stranger's zikaron/1 grant beside a zikaron/2 "
        "ledger whose report carries continues null; check 2 PASS, but the MALFORMED conviction reaches a "
        "zikaron/1 entry only where the report carries continues not null, and the entry is no EXCLUDED "
        "row; N applies, the roots agree, no standing record: UNKNOWN, PARTIAL (as the table's "
        "never-moved clause reads the same bytes).", "PARTIAL")
    CK("d3_v1_grant_uncontinued_v2_ledger_table", E1("stranger", "grant", 1, eid(mgen), body),
       AINP(IS, [mgen, mann], MA, bas=FCOV), None,
       "sec 7.2 table, check 3: the same zikaron/1 bytes without the member over a ledger whose report "
       "carries continues null: UNKNOWN, checks 4 and 6 UNKNOWN.", "PARTIAL")
    # a path that resolves to a character device is read in full (HARNESS-KIT2): /dev/null is no notice input
    i = seqno("checks")
    base = "checks/c%03d_d3_notices_path_character_device" % i
    gp_cd = W(base + ".grant.zk1", MG)
    ap_cd = WJ(base + ".audit.json", MI)
    np_cd = base + ".notices.json"
    os.symlink("/dev/null", os.path.join(HERE, np_cd))
    CASE("checks", gp_cd, "grant-check", ["grant-check", gp_cd, "--audit", ap_cd, "--notices", np_cd],
         "HARNESS-KIT2: a --notices path resolving to a character device (a symbolic link to /dev/null) is "
         "read in full, zero bytes, which are no JSON: an invalid notice input, an answer and never misuse; "
         "`notices` null and checks 3, 4 and 6 UNKNOWN.", grant_check(MG, MI, None, None),
         audit_path=ap_cd, notices_path=np_cd, now=None)
    # ---- round 4, seat D: a character device at the two other paths grant-check reads ----
    i = seqno("checks")
    base = "checks/c%03d_d4_audit_path_character_device" % i
    gp4 = W(base + ".grant.zk1", MG)
    ap4 = base + ".audit.json"
    os.symlink("/dev/null", os.path.join(HERE, ap4))
    np4 = WJ(base + ".notices.json", NT(revs=[nrec(MGID, IS, 150)]))
    CASE("checks", gp4, "grant-check", ["grant-check", gp4, "--audit", ap4, "--notices", np4],
         "HARNESS-KIT2: an --audit path resolving to a character device (a symbolic link to /dev/null) is "
         "read in full, zero bytes, which are no JSON: an invalid audit input, an answer and never misuse; "
         "`basis` null, checks 2, 3 and 4 UNKNOWN, and check 6 FAIL on the notice of the key in office, "
         "which waits on N and the root alone.",
         grant_check(MG, None, None, NT(revs=[nrec(MGID, IS, 150)])),
         audit_path=ap4, notices_path=np4, now=None)
    i = seqno("checks")
    gp5 = "checks/c%03d_d4_grant_path_character_device.grant.zk1" % i
    os.symlink("/dev/null", os.path.join(HERE, gp5))
    CASE("checks", gp5, "grant-check", ["grant-check", gp5],
         "HARNESS-KIT2: a grant path resolving to a character device is read in full: zero bytes, which "
         "fail accept; check 1 FAIL with its zikaron/2 token, checks 2 to 6 UNKNOWN; an answer and never misuse.",
         grant_check(b"", None, None, None), audit_path=None, notices_path=None, now=None)


# ==========================================================================
# chains/ : sec 7.5
# ==========================================================================


def CHN(slug, hops, now, note, expect=None):
    i = seqno("chains")
    base = "chains/h%03d_%s" % (i, slug)
    rows = []
    for k, h in enumerate(hops):
        g, ip, nt = _hop3(h)
        gp = W("%s.g%d.zk1" % (base, k), g)
        ap = WJ("%s.a%d.json" % (base, k), ip) if ip is not None else None
        np_ = WJ("%s.n%d.json" % (base, k), nt) if nt is not None else None
        rows.append({"grant": gp, "audit": ap, "notices": np_})
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


def M(slug, args, note):
    """A harness-misuse case: both programs exit 2 with no output."""
    i = seqno("misuse")
    MANIFEST.append({"path": "misuse/m%03d_%s" % (i, slug), "category": "misuse", "cmd": (args[0] if args else ""),
                     "args": list(args), "note": note, "predicted": {"exit": 2}})
    COUNTS["misuse"] = COUNTS.get("misuse", 0) + 1


def build_misuse():
    chk = next(r for r in MANIFEST if r["category"] == "checks" and "--audit" in r["args"])
    gp, ap = chk["args"][1], chk["args"][3]
    kit = next(r for r in MANIFEST if r["category"] == "kits")["path"]
    chain = next(r for r in MANIFEST if r["category"] == "chains")["args"][1]
    M("now_leading_zero", ["grant-check", gp, "--audit", ap, "--now", "01"],
      "HARNESS-KIT2: --now is an int as sec 3.2 spells it; a leading zero is misuse")
    M("now_joined", ["grant-check", gp, "--audit", ap, "--now=5"],
      "HARNESS-KIT2: a joined --now=<int> is misuse")
    M("now_repeated", ["grant-check", gp, "--now", "5", "--now", "6"],
      "HARNESS-KIT2: a repeated option is misuse")
    M("now_negative", ["grant-check", gp, "--now", "-1"], "HARNESS-KIT2: a sign is outside sec 3.2's form")
    M("now_too_large", ["grant-check", gp, "--now", "9007199254740992"], "HARNESS-KIT2: 2^53 is outside the universe")
    M("option_before_positional", ["grant-check", "--now", "5", gp], "HARNESS-KIT2: an option before a positional argument")
    M("audit_path_absent", ["grant-check", gp, "--audit", "checks/no-such-file.json"],
      "HARNESS-KIT2: an --audit path that cannot be read is misuse")
    M("kit_path_absent", ["kit-verify", "kits/no-such-kit"], "HARNESS-KIT2: an absent kit path is misuse")
    M("kit_two_paths", ["kit-verify", kit, kit], "HARNESS-KIT2: kit-verify takes one path")
    M("package_no_path", ["grant-package"], "HARNESS-KIT2: grant-package takes one path")
    M("hops_object", ["chain-check", "manifest.json"],
      "HARNESS-KIT2: a hops file whose root is an object is no hops list; misuse")
    M("chain_extra_argument", ["chain-check", chain, "extra"], "HARNESS-KIT2: an argument beyond the command's list")
    M("badge_decode_no_path", ["badge-decode"], "HARNESS-KIT2: badge-decode takes one path")
    M("unknown_command", ["frobnicate", kit], "HARNESS-KIT2: an unknown command is misuse")
    M("no_command", [], "HARNESS-KIT2: no command is misuse")
    for cmd in ("badge-encode", "kit-verify", "grant-check", "chain-check"):
        M(cmd.replace("-", "_") + "_no_argument", [cmd], "HARNESS-KIT2: %s takes a positional argument" % cmd)
    hb = W("chains/m_hops_bad_element.hops.json", json.dumps([{"grant": gp}]).encode())
    M("hops_element_shape", ["chain-check", hb], "HARNESS-KIT2: a hops element is exactly {grant, audit, notices}; audit and notices absent, over a real grant")
    hc = W("chains/m_hops_extra_member.hops.json", json.dumps([{"grant": gp, "audit": None, "notices": None, "extra": 1}]).encode())
    M("hops_element_extra_member", ["chain-check", hc], "HARNESS-KIT2: a hops element with a member beyond the three, over a real grant")
    h2 = W("chains/m_hops_two_members.hops.json", json.dumps([{"grant": gp, "audit": None}]).encode())
    M("hops_element_without_notices", ["chain-check", h2], "HARNESS-KIT2: a hops element is exactly {grant, audit, notices}; notices absent")
    npth = next(r for r in MANIFEST if r["category"] == "checks" and "--notices" in r["args"])["args"]
    np_ = npth[npth.index("--notices") + 1]
    M("notices_path_absent", ["grant-check", gp, "--notices", "checks/no-such-notices.json"],
      "HARNESS-KIT2: a --notices path that cannot be read is misuse")
    M("notices_repeated", ["grant-check", gp, "--notices", np_, "--notices", np_], "HARNESS-KIT2: a repeated --notices is misuse")
    M("notices_without_value", ["grant-check", gp, "--notices"], "HARNESS-KIT2: --notices without its value")
    M("notices_joined", ["grant-check", gp, "--notices=" + np_], "HARNESS-KIT2: a joined --notices= is no option token")
    M("notices_directory", ["grant-check", gp, "--notices", "checks/"], "HARNESS-KIT2: a directory is a --notices path grant-check cannot read")
    M("notices_before_positional", ["grant-check", "--notices", np_, gp], "HARNESS-KIT2: an option before the positional argument")
    M("chain_notices_option", ["chain-check", chain, "--notices", np_], "HARNESS-KIT2: --notices belongs to grant-check's line, not chain-check's")
    hn1 = W("chains/m_hops_notices_not_path.hops.json", json.dumps([{"grant": gp, "audit": None, "notices": 5}]).encode())
    M("hops_notices_not_path", ["chain-check", hn1], "HARNESS-KIT2: a hop's notices is a path or null")
    hn2 = W("chains/m_hops_notices_absent.hops.json", json.dumps([{"grant": gp, "audit": None, "notices": "checks/no-such-notices.json"}]).encode())
    M("hops_notices_path_absent", ["chain-check", hn2], "HARNESS-KIT2: a hop's notices path that cannot be read is misuse")
    hn3 = W("chains/m_hops_notices_directory.hops.json", json.dumps([{"grant": gp, "audit": None, "notices": "checks/"}]).encode())
    M("hops_notices_directory", ["chain-check", hn3], "HARNESS-KIT2: a hop's notices is a path the command cannot read")
    hu_n = W("chains/m_notices_unreadable.json", b"{}")
    os.chmod(os.path.join(HERE, hu_n), 0)
    M("notices_unreadable", ["grant-check", gp, "--notices", hu_n], "HARNESS-KIT2: a --notices path the command cannot read (mode 0000) is misuse")
    hnul = W("chains/m_hops_notices_nul.hops.json", json.dumps([{"grant": gp, "audit": None, "notices": "chains/a\u0000b.json"}]).encode())
    M("hops_notices_path_nul", ["chain-check", hnul], "HARNESS-KIT2: a hop path holding U+0000 names no file the system can open; misuse")
    hsur = W("chains/m_hops_grant_surrogate.hops.json", b'[{"grant": "chains/\\ud800.zk1", "audit": null, "notices": null}]')
    M("hops_grant_path_lone_surrogate", ["chain-check", hsur], "HARNESS-KIT2: a hop path holding a lone surrogate names no file the system can open; misuse")
    # round 2, seat D: every path site of a hop, each failure class
    for site in ("grant", "audit"):
        hop_ = {"grant": gp, "audit": None, "notices": None}
        hop_[site] = "chains/a\u0000b.json"
        hx_ = W("chains/m_d2_hops_%s_nul.hops.json" % site, json.dumps([hop_]).encode())
        M("d2_hops_%s_path_nul" % site, ["chain-check", hx_], "HARNESS-KIT2: a hop's %s path holding U+0000 names no file the system can open; misuse" % site)
    for site in ("audit", "notices"):
        hop_ = '{"grant": "%s", "audit": null, "notices": null}' % gp
        hop_ = hop_.replace('"%s": null' % site, '"%s": "chains/\\ud800.json"' % site)
        hx_ = W("chains/m_d2_hops_%s_surrogate.hops.json" % site, ("[" + hop_ + "]").encode())
        M("d2_hops_%s_path_lone_surrogate" % site, ["chain-check", hx_], "HARNESS-KIT2: a hop's %s path holding a lone surrogate names no file the system can open; misuse" % site)
    hbom = W("chains/m_d2_hops_bom.hops.json", b"\xef\xbb\xbf" + json.dumps([{"grant": gp, "audit": None, "notices": None}]).encode())
    M("d2_hops_bom", ["chain-check", hbom], "HARNESS-KIT2: the hops file is read by the audit-input reader, which refuses a byte-order mark; misuse")
    hd = W("chains/m_hops_audit_absent.hops.json", json.dumps([{"grant": gp, "audit": "checks/no-such-audit.json", "notices": None}]).encode())
    M("hops_audit_path_absent", ["chain-check", hd], "HARNESS-KIT2: a hop's audit path that cannot be read is misuse")
    hn = W("chains/m_hops_not_json.hops.json", b"nope")
    M("hops_not_json", ["chain-check", hn], "HARNESS-KIT2: a hops file that is not JSON is misuse")
    hr = W("chains/m_hops_repeated_member.hops.json", ('[{"grant": "%s", "audit": null, "notices": null, "audit": null}]' % gp).encode())
    M("hops_repeated_member", ["chain-check", hr], "HARNESS-KIT2: an object repeating a member name is misuse")
    hg = W("chains/m_hops_grant_not_string.hops.json", b'[{"grant": 5, "audit": null, "notices": null}]')
    M("hops_grant_not_string", ["chain-check", hg], "HARNESS-KIT2: a hop's grant is a path string")
    ha = W("chains/m_hops_grant_absent.hops.json", b'[{"grant": "chains/no-such-grant.zk1", "audit": null, "notices": null}]')
    M("hops_grant_path_absent", ["chain-check", ha], "HARNESS-KIT2: a hop path that cannot be read is misuse")
    fifo_kit = next(r for r in MANIFEST if r["category"] == "kits" and "unreadable_fifo" in r["path"])["path"]
    M("kit_path_is_pipe", ["kit-verify", fifo_kit + "/zz-pipe"],
      "HARNESS-KIT2: a <path> resolving to neither a directory nor a regular file is misuse")
    M("now_without_value", ["grant-check", gp, "--now"], "HARNESS-KIT2: an option without its value")
    M("audit_without_value", ["grant-check", gp, "--audit"], "HARNESS-KIT2: an option without its value")
    M("unknown_option", ["grant-check", gp, "--foo", "1"], "HARNESS-KIT2: an unknown option")
    unread = next(r for r in MANIFEST if r["category"] == "kits" and r["path"].endswith("onefile_unreadable.zkit"))["path"]
    M("badge_decode_unreadable", ["badge-decode", unread], "HARNESS-KIT2: a path badge-decode cannot read is misuse")
    M("badge_encode_unreadable", ["badge-encode", unread], "HARNESS-KIT2: a path badge-encode cannot read is misuse")
    M("grant_check_unreadable_grant", ["grant-check", unread], "HARNESS-KIT2: a grant path that cannot be read is misuse")
    M("audit_unreadable", ["grant-check", gp, "--audit", unread], "HARNESS-KIT2: an --audit path that cannot be read is misuse")
    M("grant_path_absent", ["grant-check", "checks/no-such-grant.zk1"], "HARNESS-KIT2: an absent grant path is misuse")
    M("package_path_absent", ["grant-package", "packages/no-such-package"], "HARNESS-KIT2: an absent package path is misuse")
    M("package_path_is_pipe", ["grant-package", fifo_kit + "/zz-pipe"], "HARNESS-KIT2: a package <path> resolving to a pipe is misuse")
    M("grant_check_path_is_pipe", ["grant-check", fifo_kit + "/zz-pipe"], "HARNESS-KIT2: a grant path resolving to a named pipe is no regular file; a path the command cannot read, misuse")
    M("notices_path_is_pipe", ["grant-check", gp, "--notices", fifo_kit + "/zz-pipe"], "HARNESS-KIT2: a --notices path resolving to a named pipe is misuse")
    M("badge_decode_directory", ["badge-decode", kit + "/"], "HARNESS-KIT2: a directory is a path badge-decode cannot read")
    M("grant_check_directory", ["grant-check", kit + "/"], "HARNESS-KIT2: a directory is a path grant-check cannot read")
    M("now_empty", ["grant-check", gp, "--now", ""], "HARNESS-KIT2: an empty --now is outside sec 3.2's form")
    M("audit_repeated", ["grant-check", gp, "--audit", ap, "--audit", ap], "HARNESS-KIT2: a repeated option is misuse")
    M("chain_now_leading_zero", ["chain-check", chain, "--now", "01"], "HARNESS-KIT2: chain-check's --now is an int as sec 3.2 spells it")
    M("chain_option_before_positional", ["chain-check", "--now", "5", chain], "HARNESS-KIT2: an option before chain-check's positional argument")
    M("chain_now_repeated", ["chain-check", chain, "--now", "5", "--now", "6"], "HARNESS-KIT2: a repeated option on chain-check")
    he = W("chains/m_hops_element_not_object.hops.json", b"[5]")
    M("hops_element_not_object", ["chain-check", he], "HARNESS-KIT2: a hops element that is no object")
    hf = W("chains/m_hops_audit_not_path.hops.json", json.dumps([{"grant": gp, "audit": 5, "notices": None}]).encode())
    M("hops_audit_not_path", ["chain-check", hf], "HARNESS-KIT2: a hop's audit is a path or null")
    M("grant_check_extra_positional", ["grant-check", gp, gp], "HARNESS-KIT2: grant-check takes one positional argument")
    M("package_extra_positional", ["grant-package", kit, kit], "HARNESS-KIT2: grant-package takes one positional argument")
    pay = next(r for r in MANIFEST if r["category"] == "badges" and r["cmd"] == "badge-decode" and r["predicted"].get("ok"))["path"]
    M("badge_decode_extra_positional", ["badge-decode", pay, pay], "HARNESS-KIT2: badge-decode takes one positional argument")
    M("badge_decode_absent", ["badge-decode", "badges/no-such.payload"], "HARNESS-KIT2: a path badge-decode cannot read is misuse")
    M("badge_encode_absent", ["badge-encode", "badges/no-such.zk1"], "HARNESS-KIT2: a path badge-encode cannot read is misuse")
    M("badge_encode_directory", ["badge-encode", "badges/"], "HARNESS-KIT2: a directory is a path badge-encode cannot read")
    M("badge_encode_with_option", ["badge-encode", pay, "--now", "5"], "HARNESS-KIT2: badge-encode's line has no option")
    M("chain_hops_absent", ["chain-check", "chains/no-such.hops.json"], "HARNESS-KIT2: a hops path the command cannot read is misuse")
    M("chain_hops_directory", ["chain-check", "chains/"], "HARNESS-KIT2: a directory is a path chain-check cannot read")
    M("chain_now_without_value", ["chain-check", chain, "--now"], "HARNESS-KIT2: --now without a value on chain-check")
    M("chain_now_joined", ["chain-check", chain, "--now=5"], "HARNESS-KIT2: a joined --now=5 is no option token of chain-check's line")
    M("chain_unknown_option", ["chain-check", chain, "--foo", "1"], "HARNESS-KIT2: an option outside chain-check's line")
    M("chain_now_above_range", ["chain-check", chain, "--now", "9007199254740992"], "HARNESS-KIT2: chain-check's --now above 2^53 - 1")
    M("chain_audit_option", ["chain-check", chain, "--audit", ap], "HARNESS-KIT2: --audit belongs to grant-check's line, not chain-check's")
    M("badge_decode_with_option", ["badge-decode", pay, "--now", "5"], "HARNESS-KIT2: badge-decode's line has no option")
    hu = W("chains/m_hops_unreadable.hops.json", json.dumps([{"grant": gp, "audit": None, "notices": None}]).encode())
    os.chmod(os.path.join(HERE, hu), 0)
    M("chain_hops_unreadable", ["chain-check", hu], "HARNESS-KIT2: a hops path the command cannot read (mode 0000) is misuse")
    hd = W("chains/m_hops_grant_directory.hops.json", json.dumps([{"grant": "checks/", "audit": None, "notices": None}]).encode())
    M("hops_grant_directory", ["chain-check", hd], "HARNESS-KIT2: a hop's grant is a path the command cannot read")
    hud = W("chains/m_hops_audit_directory.hops.json", json.dumps([{"grant": gp, "audit": "checks/", "notices": None}]).encode())
    M("hops_audit_directory", ["chain-check", hud], "HARNESS-KIT2: a hop's audit is a path the command cannot read")
    M("chain_now_negative", ["chain-check", chain, "--now", "-1"], "HARNESS-KIT2: chain-check's --now is a non-negative int")
    M("chain_now_empty", ["chain-check", chain, "--now", ""], "HARNESS-KIT2: an empty --now on chain-check")
    M("chain_now_plus_sign", ["chain-check", chain, "--now", "+5"], "HARNESS-KIT2: sec 3.2's int carries no sign")
    M("package_with_option", ["grant-package", kit, "--now", "5"], "HARNESS-KIT2: grant-package's line has no option")
    M("audit_directory", ["grant-check", gp, "--audit", "checks/"], "HARNESS-KIT2: a directory is an --audit path grant-check cannot read")
    M("audit_joined", ["grant-check", gp, "--audit=" + ap], "HARNESS-KIT2: a joined --audit= is no option token")
    M("now_plus_sign", ["grant-check", gp, "--now", "+5"], "HARNESS-KIT2: sec 3.2's int carries no sign")
    M("kit_verify_with_option", ["kit-verify", kit, "--now", "5"], "HARNESS-KIT2: kit-verify's line has no option")
    # round 3, seat D: the path-kind test at the three path sites no pipe case named
    M("audit_path_is_pipe", ["grant-check", gp, "--audit", fifo_kit + "/zz-pipe"], "HARNESS-KIT2: an --audit path resolving to a named pipe is misuse")
    hpn = W("chains/m_d3_hops_notices_pipe.hops.json", json.dumps([{"grant": gp, "audit": None, "notices": fifo_kit + "/zz-pipe"}]).encode())
    M("hops_notices_path_is_pipe", ["chain-check", hpn], "HARNESS-KIT2: a hop's notices path resolving to a named pipe is misuse")
    hpg = W("chains/m_d3_hops_grant_pipe.hops.json", json.dumps([{"grant": fifo_kit + "/zz-pipe", "audit": None, "notices": None}]).encode())
    M("hops_grant_path_is_pipe", ["chain-check", hpg], "HARNESS-KIT2: a hop's grant path resolving to a named pipe is misuse")
    # round 3, seat E: the hops file is read by the audit-input reader, which refuses a lone-surrogate escape
    # even where the platform would re-encode the escapes to the bytes of a file that exists
    W("chains/m_e3_caf\u00e9.zk1", open(os.path.join(HERE, gp), "rb").read())
    hcafe = W("chains/m_e3_hops_surrogate_escape.hops.json",
              '[{"grant": "chains/m_e3_caf\\udcc3\\udca9.zk1", "audit": null, "notices": null}]'.encode())
    M("hops_grant_path_surrogate_escape_of_existing_file", ["chain-check", hcafe],
      "HARNESS-KIT2: a hop path spelled with lone-surrogate escapes is refused by the audit-input reader that reads the hops file, though the platform would re-encode the escapes to the bytes of a file that exists; misuse")
    # round 4, seat D: the path-kind test at the hop's audit and at the hops file itself
    hpa = W("chains/m_d4_hops_audit_pipe.hops.json", json.dumps([{"grant": gp, "audit": fifo_kit + "/zz-pipe", "notices": None}]).encode())
    M("hops_audit_path_is_pipe", ["chain-check", hpa], "HARNESS-KIT2: a hop's audit path resolving to a named pipe is misuse")
    M("chain_hops_path_is_pipe", ["chain-check", fifo_kit + "/zz-pipe"], "HARNESS-KIT2: a <hops.json> path resolving to a named pipe is misuse")
    hdev = "chains/m_d4_hops_character_device.hops.json"
    os.symlink("/dev/null", os.path.join(HERE, hdev))
    M("chain_hops_path_character_device", ["chain-check", hdev],
      "HARNESS-KIT2: a <hops.json> path resolving to a character device is read in full, zero bytes, which the audit-input reader refuses: misuse")


def build_chains(rng, t):
    WK = HX32(0xC0DE)
    A, B, C, Dd = AD("iss"), AD("issb"), AD("issc"), AD("r4")
    g0, I0 = hop("iss", gbody(B, WK))
    g1, I1 = hop("issb", gbody(C, WK, upstream=eid(g0)))
    g2, I2 = hop("issc", gbody(Dd, WK, upstream=eid(g1)))

    CHN("empty", [], None,
        "sec 7.5: an empty list yields FAIL with CHAIN_EMPTY, empty `hops` and `links`, "
        "and a failing point of kind `empty` at index 0.", "FAIL")
    CHN("single_green", [(g0, I0)], None,
        "sec 7.5: one hop, GREEN, no links to decide, no failing point.", "GREEN")
    CHN("single_partial", [(g0, None)], None,
        "sec 7.5: one hop whose audit input is null, so four of its checks are UNKNOWN.",
        "PARTIAL")
    g0f, I0f = hop("iss", gbody(B, WK), tail="revocation")
    CHN("single_fail", [(g0f, I0f)], None,
        "sec 7.5: one hop whose grant-check fails; the failing point is kind `hop` at "
        "index 0 and the chain token is null.", "FAIL")
    CHN("two_green", [(g0, I0), (g1, I1)], None,
        "sec 7.5: two hops, both GREEN, and the ledger link holds: the downstream issuer's "
        "ledger is the one the upstream grantee opened.", "GREEN")
    CHN("three_green", [(g0, I0), (g1, I1), (g2, I2)], None,
        "sec 7.5: three hops and two true links.", "GREEN")
    CHN("three_partial_audit_null", [(g0, I0), (g1, None), (g2, I2)], None,
        "sec 7.4: with I_d null the link is unknown and the array carries null; hop 1 is "
        "PARTIAL; no failing point exists, so the chain is PARTIAL.", "PARTIAL")

    up_bad, I_up = hop("issb", gbody(C, WK, upstream=HX32(0xDEAD)))
    CHN("link_false_upstream_value", [(g0, I0), (up_bad, I_up)], None,
        "sec 3.3 through sec 7.4: `upstream` is a string that is not "
        "hex32(entry_id(u)), so the byte link fails and the link is false.", "FAIL")
    wk_bad, I_wk = hop("issb", gbody(C, HX32(0xFEED), upstream=eid(g0)))
    CHN("link_false_work", [(g0, I0), (wk_bad, I_wk)], None,
        "sec 3.3: the works differ, so the byte link fails.", "FAIL")
    g1r, I1r = hop("issc", gbody(Dd, WK, upstream=eid(g0)))
    CHN("link_false_root_not_grantee", [(g0, I0), (g1r, I1r)], None,
        "sec 7.4: the byte link holds and I_d's root is not the upstream grant's "
        "`grantee`, so the ledger link is false. Whether the downstream issuer is the "
        "upstream grantee is a question of ledgers and never of bytes alone.", "FAIL")
    noup, I_noup = hop("issb", gbody(C, WK))
    CHN("link_false_upstream_absent", [(g0, I0), (noup, I_noup)], None,
        "sec 3.3: a downstream grant carrying no `upstream` member at all.", "FAIL")
    def hop1(keytag, body):
        gen1 = E1(keytag, "genesis", 0, None, GEN)
        g1_ = E1(keytag, "grant", 1, eid(gen1), body)
        t1 = E1(keytag, "annotation", 2, eid(g1_), {"note_md": "after"})
        ad = AD(keytag)
        return g1_, AINP(ad, [gen1, t1], [anchor(eid(g1_), ad, ts=1700000000)], bas=cover([ad]))
    nonstr, I_nonstr = hop1("issb", gbody(C, WK, upstream=7))
    CHN("link_false_upstream_nonstring", [(g0, I0), (nonstr, I_nonstr)], None,
        "sec 3.3 through sec 7.4: `upstream` must be a string; a zikaron/1 grant carries an int "
        "there as data, passes check 1, and the link is false.", "FAIL")
    CHN("link_unknown_audit_null", [(g0, I0), (g1, None)], None,
        "sec 7.4: I_d is null, so the ledger link is unknown and the links array carries "
        "null. An unknown link is no failing point.", "PARTIAL")
    CHN("link_null_hop_failed_check1",
        [(g0, I0), (pg.set_key(g1, "sig", "0x"), I1)], None,
        "sec 7.5: a link is undecided when either of its two grants failed check 1, so the "
        "link is null and the failing point is the hop.", "FAIL")
    CHN("link_null_first_hop_check1",
        [(b"{}", None), (g1, I1)], None,
        "sec 7.5: hop 0 failed check 1, so link 1 is undecided; g_0's `upstream` is not "
        "read either, since step 1 of the failing-point order asks whether g_0 *passed* "
        "check 1.", "FAIL")

    inc_val, I_inc = hop("iss", gbody(B, WK, upstream=HX32(0xABC)))
    CHN("incomplete_upstream_value", [(inc_val, I_inc), (g1, I1)], None,
        "sec 7.5: g_0 passed check 1 and its body carries `upstream`, so the failing point "
        "is kind `incomplete` at index 0 with token CHAIN_INCOMPLETE, whatever the links "
        "and hops say.", "FAIL")
    inc_null, I_incn = hop1("iss", gbody(B, WK, upstream=None))
    CHN("incomplete_upstream_null", [(inc_null, I_incn)], None,
        "sec 7.5 with the parent's sec 6 presence rule: `upstream` present with value null "
        "is a member present; a zikaron/1 grant carries it as data and passes check 1.", "FAIL")
    CHN("incomplete_beats_link", [(inc_val, I_inc), (up_bad, I_up)], None,
        "sec 7.5 order: the incomplete test comes before the link test, and this chain "
        "fails both.", "FAIL")

    g2f, I2f = hop("issc", gbody(Dd, WK, upstream=eid(g1)), tail="revocation")
    CHN("fail_hop_after_good_links", [(g0, I0), (g1, I1), (g2f, I2f)], None,
        "sec 7.5: every link is true and the last hop's grant-check fails, so the failing "
        "point is kind `hop` at the smallest failing index.", "FAIL")
    CHN("false_link_and_failing_hop", [(g0, I0), (up_bad, I_up), (g2f, I2f)], None,
        "sec 7.5 order: a false link at k = 1 and a failing hop at k = 2. The link is the "
        "failing point and the token is CHAIN_LINK, which is the whole reason the order is "
        "written down.", "FAIL")
    g0w, I0w = hop("iss", gbody(B, WK, window={"from": 1000, "to": 2000}))
    g1w, I1w = hop("issb", gbody(C, WK, upstream=eid(g0w)))
    CHN("window_now_applies_to_every_hop", [(g0w, I0w), (g1w, I1w)], 1500,
        "sec 7.5: `now` is one input for the whole chain, and every hop's check 5 reads "
        "it.", "GREEN")
    CHN("window_expired_hop", [(g0w, I0w), (g1w, I1w)], 9000,
        "sec 7.5: the first hop's window has run, so hop 0 fails and the chain fails at "
        "kind `hop` index 0.", "FAIL")
    v1g = E1("issb", "genesis", 0, None, GEN)
    v1gr = E1("issb", "grant", 1, eid(v1g), gbody(C, WK, upstream=eid(g0)))
    v1m = E1("issb", "succession", 2, eid(v1gr), {"to": B, "kind": "migration", "effective": 1, "statement_md": "to the second law"})
    c_gen = E("issb", "genesis", 0, None, {"statement_md": "continued", "continues": {"root": B, "entry_id": eid(v1m)}})
    Icont = AINP(B, [v1g, v1m, c_gen],
                 [anchor(eid(v1g), B, bn=10, ts=1700000000), anchor(eid(v1gr), B, bn=11, ts=1700000001),
                  anchor(eid(v1m), B, bn=12, ts=1700000002), anchor(eid(c_gen), B, bn=13, ts=1700000003)],
                 bas=cover([B]))
    CHN("two_green_continued_hop", [(g0, I0), (v1gr, Icont)], None,
        "sec 7.4/7.5 with the parent's sec 12.6: the second hop is a zikaron/1 grant in a "
        "ledger its issuer continued under the second law; the ledger link reads the root "
        "the continued ledger keeps, and the hop is GREEN.", "GREEN")
    c_rev = E("issb", "revocation", 1, eid(c_gen), {"grant": eid(v1gr)})
    Icont_rev = AINP(B, [v1g, v1m, c_gen, c_rev],
                     [anchor(eid(v1g), B, bn=10, ts=1700000000), anchor(eid(v1gr), B, bn=11, ts=1700000001),
                      anchor(eid(v1m), B, bn=12, ts=1700000002), anchor(eid(c_gen), B, bn=13, ts=1700000003),
                      anchor(eid(c_rev), B, bn=14, ts=1700000004)],
                     bas=cover([B]))
    CHN("fail_continued_hop_revoked_in_new_segment", [(g0, I0), (v1gr, Icont_rev)], None,
        "sec 7.5 with the parent's sec 12.3: the second hop's zikaron/1 grant is revoked by an "
        "entry of the new segment; the hop fails at check 6 and the chain's failing point "
        "is kind `hop` at index 1.", "FAIL")
    g1u, I1u = hop("issb", gbody(C, WK, upstream=eid(g0)), anchored=False,
                   bas=basis())
    CHN("three_partial_unanchored", [(g0, I0), (g1u, I1u), (g2, I2)], None,
        "sec 7.5: hop 1 is PARTIAL under a basis that covers nothing, every link is true "
        "and no hop fails, so the chain is PARTIAL and carries no token.", "PARTIAL")

    # ---- the revision of 2026-11-09: hops carrying a revocation member --------------
    AT = HX20(0xAA)
    BOOK = HX20(0xBB)

    def nb(cid=1, fb=0, tb=1000, at=AT, book=BOOK, bfb=0):
        return {"chainId": cid, "fromBlock": fb, "toBlock": tb, "revocationContract": at,
                "book": book, "bookFromBlock": bfb}

    def nrec(grant, sender, bn, ti=0, li=0, verdict="counted", cid=1):
        return {"chainId": cid, "blockNumber": bn, "txIndex": ti, "logIndex": li,
                "tx": HX32(0xE0000 + bn * 16 + ti * 4 + li), "sender": sender, "grant": grant, "verdict": verdict}

    def NT(revs=(), **kw):
        return {"basis": nb(**kw), "revocations": list(revs), "accessions": []}

    mem = {"chain": 1, "at": AT}
    m0, J0 = hop("iss", dict(gbody(B, WK), revocation=mem))
    m1, J1 = hop("issb", dict(gbody(C, WK, upstream=eid(m0)), revocation=mem))
    CHN("two_member_hops_green", [(m0, J0, NT()), (m1, J1, NT())], None,
        "sec 7.5 with sec 7.2 for grants carrying the member: each hop reads its own notice input; "
        "both GREEN and the link true.", "GREEN")
    CHN("member_hop_revoked_by_notice", [(m0, J0, NT()), (m1, J1, NT(revs=[nrec(eid(m1), B, 150)]))], None,
        "sec 7.5: the downstream issuer notices its grant on the chain; hop 1 fails at check 6 and "
        "the failing point is kind `hop` at index 1.", "FAIL")
    genb = E("issb", "genesis", 0, None, GEN)
    m1l = grant_entry(K("issb"), seq=1, prev=eid(genb), grantee=C, work=WK,
                      terms=HX32(0x7E), upstream=eid(m0), extra_body={"revocation": mem, "ledger": B})
    CHN("link_true_from_notices_ledger_member", [(m0, J0, NT()), (m1l, None, NT())], None,
        "sec 7.4: I_d is null and N_d applies to a downstream grant carrying the member, so the "
        "root compared is the one its `ledger` member states, the upstream grantee: the link is "
        "true, hop 1 PARTIAL (checks 2, 3, 4 UNKNOWN), the chain PARTIAL.", "PARTIAL")
    CHN("link_true_from_notices_author", [(m0, J0, NT()), (m1, None, NT())], None,
        "sec 7.4: with no `ledger` member the root compared is the downstream author, which is the "
        "upstream grantee here: true.", "PARTIAL")
    m1x = grant_entry(K("issb"), seq=1, prev=eid(genb), grantee=C, work=WK,
                      terms=HX32(0x7E), upstream=eid(m0), extra_body={"revocation": mem, "ledger": Dd})
    J1x = AINP(B, [genb], [anchor(eid(m1x), B, ts=1700000000)], bas=cover([B]))
    CHN("link_false_from_notices_ledger_member", [(m0, J0, NT()), (m1x, None, NT())], None,
        "sec 7.4: the downstream grant's `ledger` names another root than the upstream grantee: "
        "false, CHAIN_LINK.", "FAIL")
    CHN("link_unknown_notices_not_applying", [(m0, J0, NT()), (m1, None, NT(cid=2))], None,
        "sec 7.4: I_d null and a notice input that does not apply to d: unknown, the link null.", "PARTIAL")
    CHN("link_unknown_member_hop_no_inputs", [(m0, J0, NT()), (m1, None, None)], None,
        "sec 7.4: I_d and N_d both null: unknown.", "PARTIAL")
    CHN("link_unknown_nonmember_hop_with_notices", [(g0, I0), (g1, None, NT())], None,
        "sec 7.4: a downstream grant carrying no member with I_d null is unknown whatever notice "
        "input stands beside it.", "PARTIAL")
    CHN("link_audit_root_over_notices", [(m0, J0, NT()), (m1x, J1x, NT())], None,
        "sec 7.4: where I_d is given its root is the one compared, and the grant's `ledger` member "
        "naming another root is data: true, GREEN.", "GREEN")
    m1g, J1g = hop("issb", dict(gbody(C, WK, upstream=eid(g0)), revocation=mem))
    CHN("member_and_nonmember_hops", [(g0, I0, NT()), (m1g, J1g, NT())], None,
        "sec 7.5: a chain mixing a hop read by the ledger and a hop read by the chain; both GREEN "
        "under their own inputs.", "GREEN")
    CHN("member_hop_badge_alone", [(m0, AINP(A, [], [anchor(eid(m0), A, ts=1700000000)], bas=cover([A])), NT())], None,
        "sec 9 (a badge alone) in a chain: one hop over an empty pile, check 2 UNKNOWN, PARTIAL.", "PARTIAL")

    # ---- seat D additions (round 1) ----
    J1bad = AINP(B, [genb], [anchor(eid(m1x), Dd, ts=1700000000)], bas=cover([B]))
    CHN("d_link_audit_invalid_root_from_bytes_false", [(m0, J0, NT()), (m1x, J1bad, NT())], None,
        "sec 7.4: I_d is invalid (its record's sender lies outside `senders`) and N_d applies; the "
        "root compared is the one d's `ledger` states, not the invalid input's: false.", "FAIL")
    J1bad2 = AINP(Dd, [genb], [anchor(eid(m1), B, ts=1700000000)], bas=cover([Dd]))
    CHN("d_link_audit_invalid_root_from_bytes_true", [(m0, J0, NT()), (m1, J1bad2, NT())], None,
        "sec 7.4: I_d invalid with a root other than the upstream grantee; the root compared is "
        "d's author, the grantee: true.", "PARTIAL")
    CHN("d_hop_notices_invalid", [(m0, J0, NT()), (m1, None, NT(cid="x"))], None,
        "sec 7.5 with sec 6.3: a hop's invalid notice input is read as null; the link is unknown and "
        "the hop's `notices` null.", "PARTIAL")

    # ---- round 1: the two cells of sec 7.4's unknown clause (seat E), the root through the book (seat F) --
    def brec(holder, to, root, bn, ti=0, li=0, cid=1):
        return {"chainId": cid, "blockNumber": bn, "txIndex": ti, "logIndex": li,
                "holder": holder, "to": to, "root": root}

    def NT(revs=(), accs=(), **kw):
        return {"basis": nb(**kw), "revocations": list(revs), "accessions": list(accs)}
    m1bad = grant_entry(K("issb"), seq=1, prev=eid(genb), grantee=C, work=WK,
                        terms=HX32(0x7E), upstream=eid(m0), extra_body={"revocation": {"chain": "1", "at": AT}})
    CHN("link_unknown_member_malformed_with_notices", [(m0, J0, NT()), (m1bad, None, NT())], None,
        "sec 7.4: I_d null and a downstream grant whose member is not well-formed, so N_d applies "
        "to it nowhere: unknown, the link null.", "PARTIAL")
    J1d = AINP(Dd, [genb], [], bas=cover([Dd]))
    CHN("link_false_from_audit_root_member_notices_null", [(m0, J0, NT()), (m1, J1d, None)], None,
        "sec 7.4: I_d given and valid, N_d null; the root compared is I_d's, another than the "
        "upstream grantee: false, CHAIN_LINK.", "FAIL")
    CHN("link_true_from_audit_root_notices_not_applying", [(m0, J0, NT()), (m1, J1, NT(cid=2))], None,
        "sec 7.4: I_d given, N_d from another chain; the root compared is I_d's, the grantee: true; "
        "hop 1 PARTIAL since its notice input does not apply.", "PARTIAL")
    k2 = AD("k2")
    genk = E("k2", "genesis", 0, None, GEN)
    dk = grant_entry(K("k2"), seq=1, prev=eid(genk), grantee=C, work=WK, terms=HX32(0x7E), upstream=eid(m0), extra_body={"revocation": mem})
    CHN("link_true_from_book_root_of_successor", [(m0, J0, NT()), (dk, None, NT(accs=[brec(B, k2, B, 140)]))], None,
        "sec 7.4 with sec 7.1: I_d null, N_d applies, and the book names the upstream grantee as the "
        "root the downstream signer acceded under: true (under the author alone the link would be "
        "false); hop 1 PARTIAL.", "PARTIAL")
    CHN("link_false_book_silent_successor_author", [(m0, J0, NT()), (dk, None, NT())], None,
        "sec 7.4 with sec 7.1: the same hop with a book that never seated the signer; the root is "
        "the author, not the grantee: false, CHAIN_LINK.", "FAIL")
    Jk2 = AINP(k2, [genk], [anchor(eid(dk), k2, bn=200, ts=1700000100)], bas=cover([k2]))
    CHN("link_true_book_root_over_audit_under_author", [(m0, J0, NT()), (dk, Jk2, NT(accs=[brec(B, k2, B, 140)]))], None,
        "sec 7.4 with sec 7.1: hop 1's reader audited under the signer's own key, and the book names the "
        "seat it acceded to, the upstream grantee: the book's root comes first, so the link is true; the "
        "hop itself fails check 3, its audit root differing from the book's: chain FAIL at the hop.", "FAIL")

    # ---- round 3, seat D: the book of an input that does not apply names no root for the link ----
    JkB = AINP(B, [genb], [], bas=cover([B]))
    CHN("d3_link_book_of_unapplied_input_not_read", [(m0, J0, NT()), (dk, JkB, NT(cid=2, accs=[brec(Dd, k2, Dd, 140, cid=2)]))], None,
        "sec 7.4 with sec 7.1: hop 1's notice input is chain 2's and does not apply to d, though its book "
        "seats d's signer under another root; the book's step of sec 7.1 needs N_d to apply, so the root "
        "compared is I_d's, the upstream grantee: true; the hop is convicted at check 3 (a stranger to the "
        "ledger audited, no book read): chain FAIL at the hop, not at the link.", "FAIL")
    # ---- round 3, seat A: the book of an input that does not apply, and a book beside a grant carrying no member --
    CHN("a3_link_nonapplying_book_ignored", [(m0, J0, NT()), (dk, Jk2, NT(at=HX20(0xAB), accs=[brec(B, k2, B, 140)]))], None,
        "sec 7.4 with sec 7.1: hop 1's notice input names another contract and does not apply to d, though "
        "its book seats d's signer under the upstream grantee; the book's step needs N_d to apply, so the root "
        "compared is I_d's, the signer's own key: false, CHAIN_LINK (the hop itself reads PARTIAL, its "
        "input not applying).", "FAIL")
    dk_plain = grant_entry(K("k2"), seq=1, prev=eid(genk), grantee=C, work=WK, terms=HX32(0x7E), upstream=eid(m0))
    Jk2p = AINP(k2, [genk], [anchor(eid(dk_plain), k2, bn=200, ts=1700000100)], bas=cover([k2]))
    CHN("a3_link_nonmember_hop_book_ignored", [(m0, J0, NT()), (dk_plain, Jk2p, NT(accs=[brec(B, k2, B, 140)]))], None,
        "sec 7.4: a downstream grant carrying no member is read by its ledger alone, and the notice input "
        "beside it, whose book seats its signer under the grantee, is data: the root compared is I_d's, the "
        "signer's own key: false, CHAIN_LINK.", "FAIL")
    # ---- round 4, seat D: a character device at each of a hop's three paths ----
    i = seqno("chains")
    base = "chains/h%03d_d4_hop_paths_character_device" % i
    g0p = W(base + ".g0.zk1", m0)
    rows = [{"grant": g0p, "audit": base + ".a0.json", "notices": base + ".n0.json"},
            {"grant": base + ".g1.zk1", "audit": None, "notices": None}]
    for rel in (base + ".a0.json", base + ".n0.json", base + ".g1.zk1"):
        os.symlink("/dev/null", os.path.join(HERE, rel))
    hp = WJ(base + ".hops.json", rows)
    CASE("chains", hp, "chain-check", ["chain-check", hp],
         "HARNESS-KIT2 with sec 7.5: hop 0's audit and notices paths and hop 1's grant path each resolve to "
         "a character device (symbolic links to /dev/null), read in full as zero bytes: hop 0 reads an invalid "
         "audit input and an invalid notice input (PARTIAL, `basis` and `notices` null), hop 1's empty bytes "
         "fail accept, the link is undecided; an answer, never misuse: FAIL at hop 1.",
         chain_check([(m0, None, None), (b"", None, None)], None), now=None)


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
    badge, benc, kits_, krule, gp = {}, {}, {}, {}, {}
    cstate, cverd, creason, chv, cht, chl, chk = {}, {}, {}, {}, {}, {}, {}
    for r in MANIFEST:
        p = r["predicted"]
        c = r["category"]
        if c == "badges" and r["cmd"] == "badge-decode":
            tally(badge, "BADGE_OK" if p.get("ok") else p.get("token"))
        elif c == "badges":
            tally(benc, "payload" if "payload" in p else p.get("token"))
        elif c == "kits":
            tally(kits_, p["verdict"])
            if p["verdict"] == "E_KIT_MANIFEST":
                tally(krule, p["subject"])
        elif c == "packages":
            tally(gp, p["verdict"])
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
    return {"badge_decode": badge, "badge_encode": benc, "kit_verdicts": kits_,
            "kit_manifest_rules": krule, "package_verdicts": gp, "check_states": cstate,
            "check_reasons": creason, "check_verdicts": cverd, "chain_verdicts": chv,
            "chain_tokens": cht, "chain_links": chl, "chain_failing_kinds": chk}


CANON_TOKENS = ZK1_TOKENS[:8]


def vocabularies():
    """The closed vocabularies as one table of [coverage key, residue label,
    token]: a label is unwitnessed iff the tally under its key lacks its
    token. The comparator derives `unwitnessed` from this table."""
    rows = []
    rows += [["badge_decode", "badge:" + t, t] for t in BADGE_TOKENS + ["BADGE_OK"]]
    rows += [["kit_verdicts", "kit:" + v, v] for v in KIT_VERDICTS]
    rows += [["kit_manifest_rules", "kit-rule:" + r, r] for r in KIT_RULES]
    rows += [["package_verdicts", "package:" + v, v] for v in GP_VERDICTS + ["E_KIT_MANIFEST", "E_KIT_UNREADABLE", "E_BADGE_ENTRY"]]
    for n in range(1, 7):
        for s_ in (("PASS", "FAIL") if n == 1 else ("PASS", "FAIL", "UNKNOWN")):
            rows.append(["check_states", "check%d:%s" % (n, s_), "%d:%s" % (n, s_)])
    rows += [["check_verdicts", "grant-check:" + v, v] for v in ("GREEN", "PARTIAL", "FAIL")]
    rows += [["chain_verdicts", "chain-check:" + v, v] for v in ("GREEN", "PARTIAL", "FAIL")]
    rows += [["chain_tokens", "chain-token:" + t, t] for t in ("CHAIN_EMPTY", "CHAIN_INCOMPLETE", "CHAIN_LINK", "None")]
    rows += [["chain_links", "chain-link:" + l, l] for l in ("True", "False", "None")]
    rows += [["chain_failing_kinds", "chain-failing:" + k, k] for k in ("empty", "incomplete", "link", "hop", "none")]
    rows += [["check_reasons", "check1-reason:" + t, t] for t in ["NOT_A_GRANT"] + list(ZK1_TOKENS)]
    return rows


def gaps(cov):
    return sorted(set(lab for key, lab, tok in vocabularies() if tok not in cov[key]))


DIRS = ("badges", "kits", "packages", "checks", "chains")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    seed = args.seed
    rng = random.Random(seed)

    for tag in ("author", "r1", "r2", "r3", "r4", "iss", "iss2", "issb", "issc",
                "stranger", "k2", "k3"):
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

    tb = build_badges(rng, None)
    build_kits(rng, tb)
    build_packages(rng, tb)
    build_checks(rng, tb)
    build_chains(rng, tb)
    build_misuse()

    cov = coverage()
    man = {
        "corpus": "zikaron.kit/2 convergence corpus",
        "law": "docs/zikaron-kit-v2.md",
        "parent_laws": ["docs/zikaron-v2.md", "docs/zikaron-v1.md"],
        "harness": "zikaron-conformance/HARNESS-KIT2.md",
        "seed": seed,
        "generator": "gen.py",
        "run_from": "zikaron-conformance/kit-corpus2 (every path in `args` is relative to "
                    "this directory)",
        "keys": {tag: {"privkey": "0x%064x" % KEYS[tag],
                       "address": pg.privkey_to_addr_hex(KEYS[tag])}
                 for tag in sorted(KEYS)},
        "counts": COUNTS,
        "coverage": cov,
        "vocabularies": vocabularies(),
        "unwitnessed": gaps(cov),
        "undecided": UNDECIDED,
        "prediction_disclaimer":
            "Every `predicted` field is the corpus author's own reading of "
            "docs/zikaron-kit-v2.md, produced by gen.py and by nothing else. It is a third "
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
        f.write(README % (seed, COUNTS.get("badges", 0), COUNTS.get("kits", 0),
                          COUNTS.get("packages", 0), COUNTS.get("checks", 0),
                          COUNTS.get("chains", 0)))

    print("seed %d" % seed)
    for k in DIRS:
        print("  %-8s %4d cases" % (k, COUNTS.get(k, 0)))
    print("  total    %4d cases" % sum(COUNTS.values()))
    print("  badge decode: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["badge_decode"].items())))
    print("  kit verdicts: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["kit_verdicts"].items())))
    print("  package verdicts: %s" % ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(cov["package_verdicts"].items())))
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


README = """# zikaron.kit/2 convergence corpus

Generated by `gen.py` from `docs/zikaron-kit-v2.md` (the law),
`docs/zikaron-v2.md` and `docs/zikaron-v1.md` (the parents) and
`zikaron-conformance/HARNESS-KIT2.md` (the command-line contract) alone. No
implementation of the kit law was read while building it. The parents' side of
the work (canonical bytes, the entry predicates, secp256k1 with RFC 6979 and
low-s) is imported from `../corpus2/gen.py` and `../corpus/gen.py`, the
parents' own corpus generators, which are tools here and never evidence for
this law; the audit of a ledger, its continued segment included, is this
generator's own reading.

## Regenerating

    cd zikaron-conformance/kit-corpus2
    python3 gen.py --seed %d

The run is deterministic: one seed rebuilds a byte-identical tree. Every
directory below is deleted and repopulated on each run, and `manifest.json`
and this file are rewritten. Requires Python 3 and `pycryptodome` (for
`Crypto.Hash.keccak`), the parent generators' only dependency.

Some things in `kits/` and `packages/` do not survive a copy that is not a
regeneration: empty directories, which git does not store, symbolic links, a
named pipe, and directories and files whose mode is 0000. Regenerate rather
than copy.

## Running it

Every case in `manifest.json` carries an `args` array: the argument vector to
hand `zkk2`, with every path relative to this directory. So a whole run is

    cd zikaron-conformance/kit-corpus2
    python3 - <<'EOF' | tee ../out-a.txt
    import json, subprocess
    for c in json.load(open("manifest.json"))["cases"]:
        r = subprocess.run(["zkk2"] + c["args"], capture_output=True)
        print(c["path"], c["cmd"], r.returncode, r.stdout.decode())
    EOF

printing the exit code beside each answer (`subprocess.run(...).returncode`)
so that the misuse cases, which answer nothing and exit 2, are compared too;
`../compare-kit2.py` is the instrument, comparing case by case against the
prediction and printing the manifest's sha256 and a digest over every corpus
byte it read, which `ZKK2_EXPECT` may pin. Every command writes
one canonical JSON value and exits 0 whenever it produced an answer; a
rejection is an answer.

## Layout

- `badges/` (%d cases) - `bNNN_*.payload` holds one payload for
  `zkk2 badge-decode`; `eNNN_*/NN.zk1` are the ordered entry files of one
  `zkk2 badge-encode` invocation.
- `kits/` (%d cases) - each `kNNN_*/` is one directory, and each `kNNN_*.zkit`
  one file in the one-file form, for `zkk2 kit-verify`.
- `packages/` (%d cases) - each `gNNN_*/` or `gNNN_*.zkgrant` is one grant
  package for `zkk2 grant-package`.
- `checks/` (%d cases) - `cNNN_*.grant.zk1` with an optional
  `cNNN_*.audit.json` and an optional `cNNN_*.notices.json` (a notice input,
  law sec 6.3); the `args` array carries `--audit`, `--notices` and `--now`
  where the case uses them.
- `chains/` (%d cases, plus the misuse cases' hops files) - `hNNN_*.hops.json`
  names its hops' grant, audit and notices files, which sit beside it.
- misuse cases (category `misuse`; the hops-file cases own a file each in
  `chains/m_hops_*` and `chains/m_d2_hops_*`, and the unreadable notices
  file is `chains/m_notices_unreadable.json`) - argument vectors
  HARNESS-KIT2.md calls misuse; the
  prediction is `{"exit": 2}` and both programs must exit 2 with empty stdout
  (a diagnostic may go to stderr).
- `manifest.json` - every case with its category, its command, its argument
  vector, a note naming the section it witnesses, and the corpus author's own
  predicted answer.

## Reading manifest.json

`predicted` is a third opinion and never authoritative: the predictor is the
corpus author's reading of the law, written to the same clauses as the
criterion, so agreement between the two is one reading measured twice;
the independent evidence is the three implementations of the review rounds.
Beside the cases the manifest carries:

- `coverage`: what the corpus witnessed, tallied by closed vocabulary.
- `vocabularies`: the closed vocabularies as a table of coverage key, residue
  label and token; the comparator derives the residue from it and refuses a
  manifest whose table or residue differs from its own.
- `unwitnessed`: the residue of the closed vocabularies (badge tokens, kit
  verdicts and manifest rules, package verdicts, check states and verdicts,
  chain outcomes) no case witnesses; an empty array says every vocabulary is
  witnessed and nothing finer. What is published beside this corpus toward
  sec 10.1's precondition is this residue and the branch-arm classes of
  `../BOUNDARY-AUDIT.md`; no per-clause list exists.
- `undecided`: the places where the law and its harness left two readings;
  empty, every such place having been decided in the text.
- `keys`: the private keys every entry was signed with, so either
  implementation can re-sign and compare.
"""

if __name__ == "__main__":
    main()
