#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
gen.py - corpus generator for the `zikaron.face/1` convergence run.

Written from docs/zikaron-face-v1.md (the law), docs/zikaron-v2.md (the
parent) and zikaron-conformance/HARNESS-FACE.md (the command-line contract)
alone.  No implementation of the face law was consulted for the cases before
`build_boundaries`.  The cases of `build_boundaries` and `build_round1`
(round 1), `build_round2` and `build_round2_readers` (round 2), and
`build_round3` and `build_round3_readers` (round 3) were chosen by review
seats that had read and mutated `face-py/` to find what the corpus left
unwitnessed; their predictions are this file's predictor's, as every other
case's.  The parent's side of the work (canonicalizer, parser, secp256k1
with RFC 6979 and low-s, the entry predicate) is reused from the parent
corpus generator `../corpus2/gen.py`, which is a tool here and never
evidence for this law; everything about `zikaron.face/1` below is the corpus
author's own reading of the text and is recorded in manifest.json as a
*prediction*, a third opinion beside the implementations under test.

Usage:  python3 gen.py --seed 1
Populates ./entries ./carried ./tables and writes manifest.json and
README.md.  Deterministic: one seed, one byte-identical tree.
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

_spec = importlib.util.spec_from_file_location("zk2gen", PARENT_GEN)
pg = importlib.util.module_from_spec(_spec)
sys.modules["zk2gen"] = pg
_spec.loader.exec_module(pg)

sha256 = pg.sha256
keccak256 = pg.keccak256
hx = pg.hx
cbytes = pg.cbytes
JObj = pg.JObj
J = pg.J
N = pg.N
HALF_N = pg.HALF_N
MAXINT = pg.MAXINT

TYPE = "zikaron.face/1"
DOMAIN = b"zikaron.face/1"
SELECTOR = bytes.fromhex("eecdf927")
SHAPES = ("home", "account", "key", "persona", "release", "reroot", "origin")
SHAPES4 = ("home", "account", "key", "release")     # the shapes the earlier builders draw from
REASONS = ("SHAPE", "ROOT", "AUTHOR", "SIGNATURE", "SUBJECT", "UNSEATED")
DIRS = ("entries", "carried", "tables", "joined")

T0 = 1700000000
CA, CB, CC, CX = 1, 10, 137, 999

MANIFEST = []
_counters = {}
KEYS = {}


# ==========================================================================
# keys, hashes, entries
# ==========================================================================

def K(tag):
    return KEYS[tag]


def A(tag):
    """The address of a key tag, or the literal where `tag` is one already."""
    if tag in KEYS:
        return pg.privkey_to_addr_hex(KEYS[tag])
    return tag


def H32(*parts):
    return hx(sha256(("face-corpus/" + "/".join(str(p) for p in parts)).encode()))


def H20(*parts):
    return "0x" + sha256(("face-corpus/" + "/".join(str(p) for p in parts)).encode()).hex()[:40]


def REG(chain):
    return H20("registry", chain)


def BOOK(chain):
    return H20("book", chain)


_seq = [0]


def mk(tag, body, etype=TYPE, seq=None, prev=None, **kw):
    """A signed zikaron/2 entry of key `tag` with `body`, at a fresh seq."""
    if seq is None:
        _seq[0] += 1
        seq = _seq[0]
    if prev is None:
        prev = H32("prev", seq)
    return pg.make_entry(K(tag), etype, seq, prev, body, **kw)


def eid(b):
    return pg.eid(b)


# ==========================================================================
# the predictor: the corpus author's reading of the law
# ==========================================================================

LABEL_ALPHABET = "abcdefghijklmnopqrstuvwxyz0123456789-"


def p_label(v):
    """sec 1: 1 to 32 characters of a-z, 0-9, hyphen."""
    if not isinstance(v, str) or not 1 <= len(v) <= 32:
        return False
    for ch in v:
        if ch not in LABEL_ALPHABET:
            return False
    return True


def p_handle(v):
    """sec 1: 1 to 128 scalar values, neither end a space."""
    return isinstance(v, str) and 1 <= len(v) <= 128 and not v.startswith(" ") and not v.endswith(" ")


# sec 1: the scalar values no name carries
BARRED = [(0x0000, 0x0020), (0x007F, 0x00A0), (0x00AD, 0x00AD), (0x034F, 0x034F), (0x0600, 0x0605),
          (0x061C, 0x061C), (0x06DD, 0x06DD), (0x070F, 0x070F), (0x0890, 0x0891), (0x08E2, 0x08E2),
          (0x115F, 0x1160), (0x1680, 0x1680), (0x17B4, 0x17B5), (0x180B, 0x180F), (0x2000, 0x200F),
          (0x2028, 0x202F), (0x205F, 0x206F), (0x2800, 0x2800), (0x3000, 0x3000), (0x3164, 0x3164),
          (0xE000, 0xF8FF), (0xFDD0, 0xFDEF), (0xFE00, 0xFE0F), (0xFEFF, 0xFEFF), (0xFFA0, 0xFFA0),
          (0xFFF0, 0xFFFF), (0x110BD, 0x110BD), (0x110CD, 0x110CD), (0x13430, 0x1343F),
          (0x13441, 0x13442), (0x16FE4, 0x16FE4), (0x1BC9D, 0x1BC9D), (0x1BCA0, 0x1BCA3), (0x1D159, 0x1D159),
          (0x1D173, 0x1D17A), (0xE0000, 0xE0FFF), (0xF0000, 0x10FFFF)]


def p_barred(o):
    if o % 0x10000 >= 0xFFFE:
        return True
    for lo, hi in BARRED:
        if lo <= o <= hi:
            return True
    return False


def p_name(v):
    if not isinstance(v, str) or not 1 <= len(v) <= 32:
        return False
    for ch in v:
        if p_barred(ord(ch)):
            return False
    return True


def p_shape(body):
    """sec 2.2: the row whose member set and forms the body has, or None."""
    if not isinstance(body, JObj):
        return None
    names = body.keys()
    if len(set(names)) != len(names):
        return None
    m = dict(body.members)
    note_ok = ("note_md" not in m) or isinstance(m["note_md"], str)
    core = set(names) - {"note_md"}
    act = m.get("act")
    if act == "home" and isinstance(act, str):
        if set(names) == {"act", "root", "chain"} and pg.is_hex20(m["root"]) and pg.is_int(m["chain"]):
            return "home"
        return None
    if act == "release" and isinstance(act, str):
        if core == {"act", "root", "subject"} and note_ok and pg.is_hex20(m["root"]) \
                and pg.is_hex32(m["subject"]):
            return "release"
        return None
    if act in ("reroot", "origin") and isinstance(act, str):
        other = "to" if act == "reroot" else "from"
        if core == {"act", "root", other} and note_ok and pg.is_hex20(m["root"]) and pg.is_hex20(m[other]) \
                and m[other] != m["root"]:
            return act
        return None
    if act == "bind" and isinstance(act, str):
        kind = m.get("kind")
        if not isinstance(kind, str) or not note_ok or not pg.is_hex20(m.get("root")):
            return None
        if kind == "account" and core == {"act", "root", "kind", "platform", "handle"} \
                and p_label(m["platform"]) and p_handle(m["handle"]):
            return "account"
        if kind == "key" and core == {"act", "root", "kind", "address", "role", "sig"} \
                and pg.is_hex20(m["address"]) and p_label(m["role"]) and pg.is_hex65(m["sig"]):
            return "key"
        if kind == "persona" and set(names) == {"act", "root", "kind", "name_md"} and p_name(m["name_md"]):
            return "persona"
    return None


def p_declaration(root, address, role):
    """sec 3.1: four lines, single line feeds between them, none after."""
    return b"\n".join([b"zikaron.face/1", b"root:" + root.encode(), b"address:" + address.encode(),
                       b"role:" + role.encode()])


def p_message(decl):
    return DOMAIN + b"\x0a" + hx(sha256(decl)).encode("ascii")


def p_digest(decl):
    m = p_message(decl)
    assert len(m) == 81
    return keccak256(b"\x19Ethereum Signed Message:\x0a" + b"81" + m)


def p_consent(m):
    """sec 3.3 for the members `m` of a body of the key shape."""
    raw = bytes.fromhex(m["sig"][2:])
    r = int.from_bytes(raw[:32], "big")
    s = int.from_bytes(raw[32:64], "big")
    v = raw[64]
    if v not in (27, 28):
        return False
    if not (1 <= r <= N - 1 and 1 <= s <= N - 1):
        return False
    if s > HALF_N:
        return False
    q = pg.ecdsa_recover(p_digest(p_declaration(m["root"], m["address"], m["role"])), r, s, v)
    if q is None:
        return False
    return hx(pg.pub_to_addr(q)) == m["address"]


def sign_consent(tag, root, address, role, decl=None, digest=None):
    """The consent key `tag` gives; `decl` or `digest` override what is signed."""
    if digest is None:
        digest = p_digest(decl if decl is not None else p_declaration(root, address, role))
    r, s, v = pg.ecdsa_sign(K(tag), digest)
    return pg.sig_hex(r, s, v)


def p_entry(b, root):
    """HARNESS-FACE.md `entry`: accept, then sec 2.2 and sec 3.3 under `root`."""
    ok, tok = pg.zk_check(b)
    if not ok:
        return {"ok": False, "token": tok}
    env = pg.parsed_entry(b)
    if env.get("entryType") != TYPE:
        return {"ok": True, "entry_id": tok, "face": False}
    body = env.get("body")
    shape = p_shape(body)
    rooted = consent = None
    if shape is not None:
        m = dict(body.members)
        rooted = (m["root"] == root)
        if shape == "key":
            consent = p_consent(m)
    return {"ok": True, "entry_id": tok, "face": True, "shape": shape, "rooted": rooted,
            "consent": consent}


def p_carried(c):
    """sec 5.1."""
    if len(c) > 36 and c[:4] == SELECTOR and sha256(c[36:]) == c[4:36]:
        ok, tok = pg.zk_check(c[36:])
        if ok:
            return {"carried": tok}
    return {"carried": None}


def _uint(v):
    return pg.is_int(v)


def p_valid(v):
    """sec 6.1 and 6.2 over a parsed JSON value (python containers)."""
    if not isinstance(v, dict) or set(v.keys()) != {"root", "chains", "anchors", "accessions", "pile"}:
        return False
    if not pg.is_hex20(v["root"]):
        return False
    for k in ("chains", "anchors", "accessions", "pile"):
        if not isinstance(v[k], list):
            return False
    chains = {}
    prev_id = None
    for c in v["chains"]:
        if not isinstance(c, dict) or set(c.keys()) != {"chainId", "fromBlock", "toBlock", "registries",
                                                         "book", "bookFromBlock"}:
            return False
        if not (_uint(c["chainId"]) and _uint(c["fromBlock"]) and _uint(c["toBlock"])):
            return False
        if c["fromBlock"] > c["toBlock"]:
            return False
        regs = c["registries"]
        if not isinstance(regs, list) or not all(pg.is_hex20(x) for x in regs):
            return False
        if any(regs[i] >= regs[i + 1] for i in range(len(regs) - 1)):
            return False
        if c["book"] is None:
            if c["bookFromBlock"] is not None:
                return False
        else:
            if not pg.is_hex20(c["book"]) or not _uint(c["bookFromBlock"]):
                return False
            if c["bookFromBlock"] > c["toBlock"]:
                return False
        if prev_id is not None and c["chainId"] <= prev_id:
            return False
        prev_id = c["chainId"]
        chains[c["chainId"]] = c
    times = {}
    seen = {}
    for a in v["anchors"]:
        if not isinstance(a, dict) or not set(a.keys()) >= {"chainId", "blockNumber", "blockTimestamp", "tx",
                                                             "sender", "hash", "verdict"}:
            return False
        if not (_uint(a["chainId"]) and _uint(a["blockNumber"]) and _uint(a["blockTimestamp"])):
            return False
        if not (pg.is_hex32(a["tx"]) and pg.is_hex32(a["hash"]) and pg.is_hex20(a["sender"])):
            return False
        if not isinstance(a["verdict"], str) or a["verdict"] not in ("counted", "UNPROVEN", "VOID"):
            return False
        c = chains.get(a["chainId"])
        if c is None or not c["fromBlock"] <= a["blockNumber"] <= c["toBlock"]:
            return False
        key = (a["chainId"], a["blockNumber"], a["tx"], a["hash"])
        rest = (a["blockTimestamp"], a["sender"], a["verdict"])
        if seen.setdefault(key, rest) != rest:
            return False
        times.setdefault((a["chainId"], a["blockNumber"]), set()).add(a["blockTimestamp"])
    seen = set()
    for x in v["accessions"]:
        if not isinstance(x, dict) or set(x.keys()) != {"chainId", "blockNumber", "blockTimestamp", "txIndex",
                                                         "logIndex", "tx", "holder", "to", "root"}:
            return False
        for k in ("chainId", "blockNumber", "blockTimestamp", "txIndex", "logIndex"):
            if not _uint(x[k]):
                return False
        if not (pg.is_hex32(x["tx"]) and pg.is_hex20(x["holder"]) and pg.is_hex20(x["to"]) and pg.is_hex20(x["root"])):
            return False
        c = chains.get(x["chainId"])
        if c is None or c["book"] is None or not c["bookFromBlock"] <= x["blockNumber"] <= c["toBlock"]:
            return False
        key = (x["chainId"], x["blockNumber"], x["txIndex"], x["logIndex"])
        if key in seen:
            return False
        seen.add(key)
        times.setdefault((x["chainId"], x["blockNumber"]), set()).add(x["blockTimestamp"])
    # one clock per chain
    per_chain = {}
    for (cid, bn), ts in times.items():
        if len(ts) != 1:
            return False
        per_chain.setdefault(cid, []).append((bn, next(iter(ts))))
    for cid, rows in per_chain.items():
        rows.sort()
        for i in range(len(rows) - 1):
            if rows[i][1] > rows[i + 1][1]:
                return False
    for s in v["pile"]:
        if not isinstance(s, str) or s[:2] != "0x" or len(s) % 2 != 0:
            return False
        if any(ch not in "0123456789abcdefABCDEF" for ch in s[2:]):
            return False
    return True


NO_TABLE = {"ok": False, "reason": "NO_TABLE"}


def p_table(v):
    """sec 7 over a parsed JSON value: the face table, or NO_TABLE."""
    if not p_valid(v):
        return NO_TABLE
    R = v["root"]
    ents = {}
    for s in v["pile"]:
        b = bytes.fromhex(s[2:])
        ok, tok = pg.zk_check(b)
        if ok:
            ents[tok] = pg.parsed_entry(b)
    face = dict((i, e) for i, e in ents.items() if e.get("entryType") == TYPE)
    shp = dict((i, p_shape(e.get("body"))) for i, e in face.items())
    mem = dict((i, dict(e.get("body").members)) for i, e in face.items())

    # one sorted list of operations; within a block the anchors precede the accessions, and a
    # home that an opening declares is committed when the walk leaves the block
    ops = []
    once = set()                                  # a record given twice is one record (sec 6.2)
    for a in v["anchors"]:
        k = (a["chainId"], a["blockNumber"], a["tx"], a["hash"])
        if k in once:
            continue
        once.add(k)
        ops.append(((a["blockTimestamp"], a["chainId"], a["blockNumber"], 0, 0, 0), a))
    for x in v["accessions"]:
        ops.append(((x["blockTimestamp"], x["chainId"], x["blockNumber"], 1, x["txIndex"], x["logIndex"]), x))
    ops.sort(key=lambda o: o[0])

    office, home = R, None
    heads = {}                                    # chain -> the head of its book, R where absent
    line = [(R, None)]
    in_line = [R]
    seats = {}
    openings = []
    unread = set()
    unproven = 0
    pending = None
    at = None
    sealed = False                                # closed once the walk has left the block of a seated reroot
    closing = False
    for key, rec in ops:
        block = key[:3]
        if block != at:
            if pending is not None:
                home = pending
                pending = None
            if closing:
                sealed = True
            at = block
        chain = key[1]
        if key[3] == 1 and pending is not None:
            home = pending                            # the opening block's accessions are read under the new home
            pending = None
        if key[3] == 0:
            if sealed:
                continue                              # a sealed seat holds no record in office
            if rec["verdict"] == "UNPROVEN" and rec["sender"] == office:
                unproven += 1
            if rec["verdict"] != "counted" or rec["sender"] != office:
                continue
            h = rec["hash"]
            if h not in ents:
                unread.add(h)
                continue
            if h not in face or shp[h] is None:
                continue
            if face[h].get("author") != office:
                continue
            if shp[h] == "home":
                if mem[h]["chain"] == chain and heads.get(chain, R) == office:
                    seats.setdefault(h, []).append(rec)
                    if mem[h]["root"] == R:
                        openings.append((rec, h))
                        pending = chain
            else:
                if home is not None and home == chain:
                    seats.setdefault(h, []).append(rec)
                    if shp[h] == "reroot" and mem[h]["root"] == R:
                        closing = True
        else:
            if rec["root"] == R:
                if rec["holder"] == office and (home is None or home == chain):
                    office = rec["to"]
                    line.append((rec["to"], rec))
                    in_line.append(rec["to"])
                heads[chain] = rec["to"]
    if pending is not None:
        home = pending

    def order(a):
        return (a["blockTimestamp"], a["chainId"], a["blockNumber"], bytes.fromhex(a["tx"][2:]))

    def moment(i):
        return sorted(seats[i], key=order)[0]

    def reads(i):
        if shp[i] is None or mem[i]["root"] != R:
            return False
        if shp[i] == "key" and not p_consent(mem[i]):
            return False
        return True

    standing = set()
    binds = set()
    for i in face:
        if reads(i) and i in seats and shp[i] != "release":
            standing.add(i)
            if shp[i] in ("account", "key", "persona"):
                binds.add(i)
    rels = set()
    for i in face:
        if shp[i] == "release" and reads(i) and i in seats and mem[i]["subject"] in binds:
            rels.add(i)
            standing.add(i)

    rows = {}
    for b in binds:
        since = moment(b)
        mine = sorted((q for q in rels if mem[q]["subject"] == b),
                      key=lambda q: (order(moment(q)), bytes.fromhex(q[2:])))
        if mine:
            q = mine[0]
            until = moment(q)
            if order(until) < order(since):
                until = since
            span = (since, b, q, until)
        else:
            span = (since, b, None, None)
        if shp[b] == "account":
            f = (0, mem[b]["platform"].encode(), mem[b]["handle"].encode())
        elif shp[b] == "persona":
            f = (2, mem[b]["name_md"].encode("utf-8"), b"")
        else:
            f = (1, mem[b]["address"].encode(), mem[b]["role"].encode())
        rows.setdefault(f, []).append(span)

    def ref(a):
        return {"chainId": a["chainId"], "blockNumber": a["blockNumber"],
                "blockTimestamp": a["blockTimestamp"], "tx": a["tx"]}

    faces_out = []
    for f in sorted(rows):
        spans = sorted(rows[f], key=lambda sp: (order(sp[0]), bytes.fromhex(sp[1][2:])))
        state = "released" if all(sp[2] is not None for sp in spans) else "bound"
        outs = [{"entry": b, "since": ref(since), "release": q,
                 "until": None if until is None else ref(until)} for since, b, q, until in spans]
        if f[0] == 0:
            faces_out.append({"kind": "account", "platform": f[1].decode(), "handle": f[2].decode(),
                              "state": state, "spans": outs})
        elif f[0] == 2:
            faces_out.append({"kind": "persona", "name_md": f[1].decode("utf-8"), "state": state, "spans": outs})
        else:
            faces_out.append({"kind": "key", "address": f[1].decode(), "role": f[2].decode(),
                              "state": state, "spans": outs})

    # sec 7.5: the persona, the open span of a persona row last in since, then in entry
    cands = []
    for f in rows:
        if f[0] == 2:
            for since, b, q, until in rows[f]:
                if q is None:
                    cands.append((order(since), bytes.fromhex(b[2:]), b, f[1].decode("utf-8")))
    cands.sort()
    persona = None if not cands else {"entry": cands[-1][2], "name_md": cands[-1][3]}

    # sec 7.4: the reroot and the origin, the standing entry of its shape first in moment, then in entry
    def chosen(name, member):
        mine = sorted((order(moment(i)), bytes.fromhex(i[2:]), i) for i in standing if shp[i] == name)
        if not mine:
            return None
        i = mine[0][2]
        return {"entry": i, member: mem[i][member], "since": ref(moment(i))}

    misread = []
    anchored_by_line = set(a["hash"] for a in v["anchors"] if a["sender"] in in_line)
    for i in sorted(face, key=lambda z: bytes.fromhex(z[2:])):
        if i in standing or i not in anchored_by_line:
            continue
        if shp[i] is None:
            why = "SHAPE"
        elif mem[i]["root"] != R:
            why = "ROOT"
        elif face[i].get("author") not in in_line:
            why = "AUTHOR"
        elif shp[i] == "key" and not p_consent(mem[i]):
            why = "SIGNATURE"
        elif shp[i] == "release" and mem[i]["subject"] not in binds:
            why = "SUBJECT"
        else:
            assert i not in seats, "an entry that reads and is seated stands"
            why = "UNSEATED"
        misread.append({"entry": i, "reason": why})

    lineage = []
    for key, x in line:
        lineage.append({"key": key, "accession": None if x is None else {
            "chainId": x["chainId"], "blockNumber": x["blockNumber"], "blockTimestamp": x["blockTimestamp"],
            "txIndex": x["txIndex"], "logIndex": x["logIndex"], "tx": x["tx"], "holder": x["holder"]}})
    homes = [{"entry": i, "chainId": a["chainId"], "blockNumber": a["blockNumber"],
              "blockTimestamp": a["blockTimestamp"], "tx": a["tx"], "sender": a["sender"]}
             for a, i in sorted(openings, key=lambda p: (order(p[0]), bytes.fromhex(p[1][2:])))]
    return {"root": R, "marker": "zikaron:" + R, "basis": v["chains"], "home": home, "office": office,
            "lineage": lineage, "homes": homes, "faces": faces_out, "misread": misread, "persona": persona,
            "reroot": chosen("reroot", "to"), "origin": chosen("origin", "from"),
            "unread": len(unread), "unproven": unproven}


def p_joined(v):
    """sec 7.9 over a parsed JSON value, an array of two face inputs: joined or not, or NO_TABLE."""
    if not isinstance(v, list) or len(v) != 2:
        return NO_TABLE
    a, b = p_table(v[0]), p_table(v[1])
    if a == NO_TABLE or b == NO_TABLE:
        return NO_TABLE
    return {"joined": a["reroot"] is not None and a["reroot"]["to"] == b["root"]
            and b["origin"] is not None and b["origin"]["from"] == a["root"]}


def out_text(value):
    """The canonical JSON text of an answer (zikaron/2 sec 3.4)."""
    return cbytes(J(value)).decode("utf-8")


# ==========================================================================
# case registration
# ==========================================================================

def _n(cat):
    _counters[cat] = _counters.get(cat, 0) + 1
    return _counters[cat]


def _write(rel, data):
    with open(os.path.join(HERE, rel), "wb") as f:
        f.write(data)


def E(slug, data, note, root=None):
    """An `entry` case: the file's bytes read under `root` (default: the root key's address)."""
    root = A("root") if root is None else root
    rel = "entries/e%03d_%s.bin" % (_n("entries"), slug)
    _write(rel, data)
    MANIFEST.append({"path": rel, "category": "entries", "cmd": "entry", "args": ["entry", rel, root],
                     "note": note, "predicted": {"exit": 0, "stdout": out_text(p_entry(data, root))}})


def D(slug, root, address, role, note, key=None):
    """A `declare` case; `key` is a private key spelling or None."""
    args = ["declare", root, address, role] + ([key] if key is not None else [])
    decl = p_declaration(root, address, role)
    ans = {"declaration": hx(decl), "message": hx(p_message(decl)), "digest": hx(p_digest(decl))}
    if key is not None:
        d = int(key[2:] if key[:2] in ("0x", "0X") else key, 16)
        r, s, v = pg.ecdsa_sign(d, p_digest(decl))
        ans["sig"] = pg.sig_hex(r, s, v)
        ans["signer"] = pg.privkey_to_addr_hex(d)
    MANIFEST.append({"path": "declare/d%03d_%s" % (_n("declare"), slug), "category": "declare",
                     "cmd": "declare", "args": args, "note": note,
                     "predicted": {"exit": 0, "stdout": out_text(ans)}})


def Cd(slug, data, note):
    """A `carried` case: the file's bytes as calldata."""
    rel = "carried/c%03d_%s.bin" % (_n("carried"), slug)
    _write(rel, data)
    MANIFEST.append({"path": rel, "category": "carried", "cmd": "carried", "args": ["carried", rel],
                     "note": note, "predicted": {"exit": 0, "stdout": out_text(p_carried(data))}})


def Tb(slug, world, note):
    """A `table` case from a World or a plain value, written as compact JSON."""
    value = world.value() if isinstance(world, World) else world
    rel = "tables/t%03d_%s.json" % (_n("tables"), slug)
    _write(rel, json.dumps(value, separators=(",", ":")).encode("utf-8"))
    MANIFEST.append({"path": rel, "category": "tables", "cmd": "table", "args": ["table", rel],
                     "note": note, "predicted": {"exit": 0, "stdout": out_text(p_table(value))}})


def Tr(slug, raw, note, value=None):
    """A `table` case whose file bytes are given as they are.  `value` is the parsed input where the
    transport's reader accepts the file; without it the file is one that reader refuses (NO_TABLE)."""
    rel = "tables/t%03d_%s.json" % (_n("tables"), slug)
    _write(rel, raw)
    ans = NO_TABLE if value is None else p_table(value)
    MANIFEST.append({"path": rel, "category": "tables", "cmd": "table", "args": ["table", rel],
                     "note": note, "predicted": {"exit": 0, "stdout": out_text(ans)}})


def Jn(slug, first, second, note):
    """A `joined` case from two Worlds or plain values, written as one compact JSON array."""
    value = [x.value() if isinstance(x, World) else x for x in (first, second)]
    rel = "joined/j%03d_%s.json" % (_n("joined"), slug)
    _write(rel, json.dumps(value, separators=(",", ":")).encode("utf-8"))
    MANIFEST.append({"path": rel, "category": "joined", "cmd": "joined", "args": ["joined", rel],
                     "note": note, "predicted": {"exit": 0, "stdout": out_text(p_joined(value))}})


def Jr(slug, raw, note, value=None):
    """A `joined` case whose file bytes are given as they are; `value` as for Tr."""
    rel = "joined/j%03d_%s.json" % (_n("joined"), slug)
    _write(rel, raw)
    ans = NO_TABLE if value is None else p_joined(value)
    MANIFEST.append({"path": rel, "category": "joined", "cmd": "joined", "args": ["joined", rel],
                     "note": note, "predicted": {"exit": 0, "stdout": out_text(ans)}})


def Mu(slug, args, note):
    """A harness-misuse case: exit 2, nothing on stdout."""
    MANIFEST.append({"path": "misuse/m%03d_%s" % (_n("misuse"), slug), "category": "misuse",
                     "cmd": args[0] if args else "", "args": list(args), "note": note,
                     "predicted": {"exit": 2}})


# ==========================================================================
# worlds: a face input under construction
# ==========================================================================

class World(object):
    def __init__(self, root="root", chains=((CA, True), (CB, True), (CC, True))):
        self.root = A(root)
        self.chains = []
        for cid, book in chains:
            self.chains.append({"chainId": cid, "fromBlock": 0, "toBlock": 1000000,
                                "registries": [REG(cid)],
                                "book": BOOK(cid) if book else None,
                                "bookFromBlock": 0 if book else None})
        self.anchors = []
        self.accessions = []
        self.pile = []
        self.ts = {}
        self._tx = 0

    def time(self, chain, block):
        return self.ts.get((chain, block), T0 + block)

    def tx(self):
        self._tx += 1
        return H32("tx", self.root, self._tx)

    def put(self, *blobs):
        for b in blobs:
            self.pile.append(hx(b))
        return self

    def anchor(self, chain, block, sender, what, verdict="counted", tx=None, carry=True):
        """Anchor entry bytes (put in the pile unless carry is False) or a hash."""
        if isinstance(what, bytes):
            h = eid(what)
            if carry:
                self.put(what)
        else:
            h = what
        rec = {"chainId": chain, "blockNumber": block, "blockTimestamp": self.time(chain, block),
               "tx": tx or self.tx(), "sender": A(sender), "hash": h, "verdict": verdict}
        self.anchors.append(rec)
        return rec

    def accede(self, chain, block, holder, to, txi=0, logi=0, root=None):
        rec = {"chainId": chain, "blockNumber": block, "blockTimestamp": self.time(chain, block),
               "txIndex": txi, "logIndex": logi, "tx": self.tx(), "holder": A(holder), "to": A(to),
               "root": self.root if root is None else A(root)}
        self.accessions.append(rec)
        return rec

    def value(self):
        return {"root": self.root, "chains": self.chains, "anchors": self.anchors,
                "accessions": self.accessions, "pile": self.pile}


def b_home(chain, root=None):
    return {"act": "home", "root": A("root") if root is None else root, "chain": chain}


def b_account(platform, handle, root=None, note=None):
    b = {"act": "bind", "root": A("root") if root is None else root, "kind": "account",
         "platform": platform, "handle": handle}
    if note is not None:
        b["note_md"] = note
    return b


def b_key(face, role, root=None, sig=None, note=None, address=None):
    root = A("root") if root is None else root
    address = A(face) if address is None else address
    if sig is None:
        sig = sign_consent(face, root, address, role)
    b = {"act": "bind", "root": root, "kind": "key", "address": address, "role": role, "sig": sig}
    if note is not None:
        b["note_md"] = note
    return b


def b_release(subject, root=None, note=None):
    b = {"act": "release", "root": A("root") if root is None else root,
         "subject": eid(subject) if isinstance(subject, bytes) else subject}
    if note is not None:
        b["note_md"] = note
    return b


def homed(chain=CA, block=10, **kw):
    """A world whose root has opened its home on `chain` at `block`."""
    w = World(**kw)
    w.anchor(chain, block, "root", mk("root", b_home(chain)))
    return w


# ==========================================================================
# entries: the reading of one entry (sec 2, sec 3)
# ==========================================================================

def _eth_digest(msg):
    return keccak256(b"\x19Ethereum Signed Message:\x0a" + str(len(msg)).encode("ascii") + msg)


def build_entries(rng):
    R = A("root")
    fa = A("fa")

    E("home", mk("root", b_home(CA)), "the home shape, rooted")
    E("account", mk("root", b_account("x", "alice")), "the account shape, no note")
    E("account_note", mk("root", b_account("github", "alice-dev", note="first account")),
      "the account shape with a note")
    E("key", mk("root", b_key("fa", "payout")), "the key shape, the consent holds")
    E("key_note", mk("root", b_key("fa", "galeed-judge", note="the judge's key")),
      "the key shape with a note, the consent holds")
    E("release", mk("root", b_release(H32("subject", 1))), "the release shape, no note")
    E("release_note", mk("root", b_release(H32("subject", 2), note="account lost")),
      "the release shape with a note")
    E("account_by_stranger", mk("stranger", b_account("x", "alice")),
      "the entry command reads shape, root and consent; whose key signed the entry is the walk's question")
    E("key_face_is_root", mk("root", b_key("root", "payout")),
      "a key face whose address is the root itself: the consent holds like any other")

    for name, body in (("home", b_home(CA, root=A("k1"))), ("account", b_account("x", "alice", root=A("k1"))),
                       ("key", b_key("fa", "payout", root=A("k1"))),
                       ("release", b_release(H32("subject", 3), root=A("k1")))):
        E("unrooted_" + name, mk("root", body), "the %s shape under another root: rooted false" % name)
    E("unrooted_key_consent_holds", mk("root", b_key("fa", "payout", root=A("k1"))),
      "a key entry for another root whose consent names that root: consent true, rooted false", root=R)
    E("rooted_under_k1", mk("root", b_account("x", "alice", root=A("k1"))),
      "the same kind of entry read under the root it names", root=A("k1"))

    # every member missing, an extra member, the optional member in another form
    bases = {"home": b_home(CA), "account": b_account("x", "alice", note="n"),
             "key": b_key("fa", "payout", note="n"), "release": b_release(H32("subject", 4), note="n")}
    for name in SHAPES4:
        body = bases[name]
        for k in sorted(body):
            b = dict(body)
            del b[k]
            E("%s_without_%s" % (name, k), mk("root", b),
              "the %s row without `%s`: %s" % (name, k, "the note is optional" if k == "note_md" else "no shape"))
        b = dict(body)
        b["extra"] = 1
        E("%s_extra_member" % name, mk("root", b), "the %s row with a member beyond it: no shape" % name)
        for tag, val in (("int", 5), ("null", None), ("array", ["n"]), ("object", {"a": "b"}), ("bool", True)):
            b = dict(body)
            b["note_md"] = val
            E("%s_note_%s" % (name, tag), mk("root", b),
              "the %s row with `note_md` a %s: no shape (the home row has no note at all)" % (name, tag))
    E("home_with_string_note", mk("root", dict(b_home(CA), note_md="n")),
      "the home row carries no note: a string note is a member beyond the row")
    E("account_note_empty", mk("root", b_account("x", "alice", note="")), "an empty note is a string")
    E("account_note_prose", mk("root", b_account("x", "alice", note="主账号 ✓\n second line\t")),
      "a note is a prose string: any scalar value, control characters included")

    def form(slug, body, note):
        E(slug, mk("root", body), note)

    form("home_chain_zero", b_home(0), "chain 0 is an int")
    form("home_chain_max", b_home(MAXINT), "chain 2^53 - 1 is an int")
    for tag, val in (("string", "1"), ("null", None), ("bool", True), ("array", [1]), ("object", {})):
        form("home_chain_" + tag, dict(b_home(CA), chain=val), "chain a %s: no shape" % tag)
    for name in SHAPES4:
        body = bases[name]
        for tag, val in (("short", "0x12"), ("upper", "0x" + R[2:].upper()), ("bare", R[2:]), ("int", 5),
                         ("null", None), ("hex32", H32("r"))):
            form("%s_root_%s" % (name, tag), dict(body, root=val), "`root` outside hex20 (%s): no shape" % tag)
    for tag, val in (("cased", "Home"), ("unknown", "unbind"), ("int", 1), ("null", None), ("empty", ""),
                     ("bind_cased", "BIND"), ("space", "bind ")):
        form("act_" + tag, dict(b_account("x", "alice"), act=val), "`act` %r: no shape" % (val,))
    form("act_home_with_account_members", dict(b_account("x", "alice"), act="home"),
         "`act` home over the account members: no shape")
    form("act_release_with_home_members", dict(b_home(CA), act="release"), "`act` release over the home members")
    for tag, val in (("unknown", "group"), ("cased", "Account"), ("int", 0), ("null", None), ("empty", "")):
        form("kind_" + tag, dict(b_account("x", "alice"), kind=val), "`kind` %r: no shape" % (val,))
    form("kind_key_over_account_members", dict(b_account("x", "alice"), kind="key"),
         "`kind` key over the account members: no shape")
    form("kind_account_over_key_members", dict(b_key("fa", "payout"), kind="account"),
         "`kind` account over the key members: no shape")
    form("account_with_address", dict(b_account("x", "alice"), address=fa), "an account row with `address`")
    form("key_with_handle", dict(b_key("fa", "payout"), handle="alice"), "a key row with `handle`")
    form("release_with_kind", dict(b_release(H32("subject", 5)), kind="account"), "a release row with `kind`")
    form("home_with_kind", dict(b_home(CA), kind="account"), "a home row with `kind`")
    form("empty_body", {}, "an empty body: no shape")

    labels = (("empty", "", False), ("one", "a", True), ("len32", "a" * 32, True), ("len33", "a" * 33, False),
              ("upper", "X", False), ("space", "x y", False), ("underscore", "x_y", False),
              ("dot", "x.y", False), ("slash", "x/y", False), ("hyphen", "-", True), ("hyphens", "-x-", True),
              ("digit", "0", True), ("mixed", "abc-123", True), ("int", 7, False), ("null", None, False))
    for tag, val, ok in labels:
        form("platform_" + tag, dict(b_account("x", "alice"), platform=val),
             "`platform` %r: %s" % (val, "a label" if ok else "no label, no shape"))
        role = val if isinstance(val, str) else "payout"
        sig = sign_consent("fa", R, fa, role) if p_label(val) else sign_consent("fa", R, fa, "payout")
        form("role_" + tag, dict(b_key("fa", "payout", sig=sig), role=val),
             "`role` %r: %s" % (val, "a label, the consent names it" if ok else "no label, no shape"))
    handles = (("empty", "", False), ("one", "a", True), ("len128", "h" * 128, True), ("len129", "h" * 129, False),
               ("lead_space", " alice", False), ("trail_space", "alice ", False), ("only_space", " ", False),
               ("two_spaces", "  ", False), ("inner_space", "alice bob", True), ("upper", "Alice", True),
               ("at", "@alice", True), ("host", "example.org", True),
               ("printable", "!\"#$%&'()*+,-./0123456789:;<=>?@AZ[\\]^_`az{|}~", True),
               ("int", 12, False), ("null", None, False), ("array", ["alice"], False))
    for tag, val, ok in handles:
        form("handle_" + tag, dict(b_account("x", "alice"), handle=val),
             "`handle` %r: %s" % (val if not isinstance(val, str) or len(val) < 40 else val[:8] + "...",
                                  "a handle" if ok else "no handle, no shape"))
    form("handle_non_ascii", b_account("x", "alé"),
         "a handle above U+007E sits outside every prose subtree: zikaron/2 refuses the bytes (E_VALUE_CHARSET)")
    form("handle_tab", b_account("x", "a\tb"), "a control character in a handle: zikaron/2 refuses the bytes")
    form("platform_non_ascii", b_account("é", "alice"), "a platform above U+007E: zikaron/2 refuses the bytes")
    for tag, val in (("upper", "0x" + fa[2:].upper()), ("short", "0x1234"), ("hex32", H32("a")), ("int", 1)):
        form("address_" + tag, dict(b_key("fa", "payout"), address=val), "`address` outside hex20 (%s)" % tag)
    good = sign_consent("fa", R, fa, "payout")
    for tag, val in (("upper", "0x" + good[2:].upper()), ("short", good[:-2]), ("long", good + "00"),
                     ("bare", good[2:]), ("int", 1), ("null", None)):
        form("sig_" + tag, dict(b_key("fa", "payout"), sig=val), "`sig` outside hex65 (%s): no shape" % tag)
    for tag, val in (("upper", "0x" + H32("s")[2:].upper()), ("short", "0x12"), ("hex20", fa), ("int", 1), ("null", None)):
        form("subject_" + tag, dict(b_release(H32("subject", 6)), subject=val), "`subject` outside hex32 (%s)" % tag)

    # the consent (sec 3): what holds and what does not
    def consent(slug, sig, note, role="payout", address=None, root=None):
        E("consent_" + slug, mk("root", b_key("fa", role, sig=sig, address=address, root=root)), note)

    T = p_declaration(R, fa, "payout")
    consent("other_key", sign_consent("fb", R, fa, "payout"), "signed by another key: the recovered address is not `address`")
    consent("other_root", sign_consent("fa", A("k1"), fa, "payout"), "a consent given to another root holds under none but that one")
    consent("other_role", sign_consent("fa", R, fa, "eravon-seller"), "a consent for another role")
    consent("other_address", sign_consent("fa", R, A("fb"), "payout"), "a consent whose declaration names another address")
    consent("for_fb_by_fa", sign_consent("fa", R, A("fb"), "payout"), "`address` is fb, the signer fa", address=A("fb"))
    consent("trailing_lf", sign_consent("fa", R, fa, "payout", decl=T + b"\n"), "a declaration with a line feed after the last line")
    consent("crlf", sign_consent("fa", R, fa, "payout", decl=T.replace(b"\n", b"\r\n")), "a declaration joined by CR LF")
    consent("upper_hex", sign_consent("fa", R, fa, "payout", decl=p_declaration("0x" + R[2:].upper(), fa, "payout")),
            "a declaration spelling the root in uppercase hexadecimal")
    consent("no_0x", sign_consent("fa", R, fa, "payout", decl=p_declaration(R[2:], fa[2:], "payout")),
            "a declaration without the 0x prefixes")
    consent("lines_swapped", sign_consent("fa", R, fa, "payout",
                                          decl=b"\n".join([b"zikaron.face/1", b"address:" + fa.encode(),
                                                           b"root:" + R.encode(), b"role:payout"])),
            "a declaration with its second and third lines exchanged")
    hT = hx(sha256(T)).encode()
    consent("entry_domain", sign_consent("fa", R, fa, "payout", digest=_eth_digest(b"zikaron/2\x0a" + hT)),
            "the right hash under the entry domain `zikaron/2`: no consent")
    consent("next_domain", sign_consent("fa", R, fa, "payout", digest=_eth_digest(b"zikaron.face/2\x0a" + hT)),
            "the right hash under `zikaron.face/2`")
    consent("personal_sign_of_text", sign_consent("fa", R, fa, "payout", digest=_eth_digest(T)),
            "an EIP-191 signature over the declaration text itself")
    consent("bare_hash", sign_consent("fa", R, fa, "payout", digest=sha256(T)), "a signature over sha256 of the declaration")
    consent("hash_not_hex", sign_consent("fa", R, fa, "payout", digest=_eth_digest(DOMAIN + b"\x0a" + sha256(T))),
            "a message carrying the 32 raw hash bytes in place of their hex32 spelling")
    some = pg.parsed_entry(mk("fa", b_account("x", "alice"))).get("sig")
    consent("entry_signature", some, "an entry's own signature offered as a consent")
    r, s, v = pg.ecdsa_sign(K("fa"), p_digest(T))
    raw = lambda rr, ss, vv: hx(rr.to_bytes(32, "big") + ss.to_bytes(32, "big") + bytes([vv & 0xFF]))
    consent("v_recid", raw(r, s, v - 27), "v written as the bare recovery id")
    consent("v_26", raw(r, s, 26), "v 26")
    consent("v_29", raw(r, s, 29), "v 29")
    consent("v_flipped", raw(r, s, 55 - v), "the other recovery id: another address")
    consent("high_s", raw(r, N - s, 55 - v), "the high-s twin of a consent that holds")
    consent("r_zero", raw(0, s, v), "r zero")
    consent("s_zero", raw(r, 0, v), "s zero")
    consent("r_is_n", raw(N, s, v), "r equal to n")
    consent("s_is_n", raw(r, N, v), "s equal to n")
    consent("s_half_plus_one", raw(r, HALF_N + 1, v), "s one above (n - 1) / 2")
    consent("r_off_curve", raw(pg.bad_r(), s, v), "an r that is the x-coordinate of no point: recovery fails")
    E("consent_s_at_half", mk("root", b_key("fa", "payout", sig=raw(r, HALF_N, v))),
      "s exactly (n - 1) / 2 passes the form tests; it is another signature and recovers another address")

    # entries of other types, and bytes that are no entries
    E("history", mk("root", pg.HISB(1), etype="history"), "an entry of another type")
    E("genesis", pg.make_entry(K("root"), "genesis", 0, None, pg.GENB), "a genesis")
    E("face_at_seq_0", pg.make_entry(K("root"), TYPE, 0, None, b_home(CA)),
      "a face entry at seq 0: zikaron/2 puts a genesis there and nothing else")
    for tag, lit in (("next", "zikaron.face/2"), ("bare", "zikaron.face"), ("cased", "Zikaron.face/1"),
                     ("longer", "zikaron.face/1x"), ("short", "face"), ("hyphen", "zikaron-face/1")):
        E("type_" + tag, mk("root", b_home(CA), etype=lit), "entryType %s: an entry, no face entry" % lit)
    E("spec_v1", mk("root", b_home(CA), spec="zikaron/1"), "a spec of `zikaron/1`: no entry under this law's accept")
    E("not_json", b"hello", "bytes that are no JSON")
    E("empty_file", b"", "no bytes")
    one = mk("root", b_account("x", "alice"))
    E("not_canonical", one.replace(b'":', b'": ', 1), "an entry spelled with a space: not canonical")
    E("sig_of_another_entry", mk("root", b_account("x", "bob"), sig_override=pg.parsed_entry(one).get("sig")),
      "an envelope carrying another entry's signature")
    E("truncated", one[:-1], "an entry cut one byte short")

    # random bodies: one member dropped, added or replaced
    pool = [None, True, False, 0, 1, MAXINT, "", "x", "bind", "home", "release", "account", "key", "payout",
            "alice", " ", [], {}, R, fa, H32("pool", 1), H32("pool", 2), good, "a" * 33, "h" * 129, "X"]
    for i in range(72):
        name = rng.choice(SHAPES4)
        b = dict(bases[name])
        if name == "key":
            b = dict(b_key("fa", rng.choice(["payout", "eravon-seller", "galeed-judge"]), note="n"))
        how = rng.randrange(4)
        if how == 0:
            del b[rng.choice(sorted(b))]
        elif how == 1:
            b[rng.choice(["extra", "chain", "kind", "subject", "handle", "platform", "address", "role", "sig"])] = \
                rng.choice(pool)
        elif how == 2:
            b[rng.choice(sorted(b))] = rng.choice(pool)
        else:
            k1, k2 = rng.choice(sorted(b)), rng.choice(sorted(b))
            b[k1], b[k2] = b[k2], b[k1]
        root = rng.choice([R, R, R, A("k1")])
        E("rand_%02d_%s" % (i, name), mk(rng.choice(["root", "k1", "stranger"]), b),
          "a %s body with one random change, read under %s" % (name, "the root" if root == R else "another root"),
          root=root)


# ==========================================================================
# declare: the declaration, its message and digest, and consents (sec 3)
# ==========================================================================

def build_declare(rng):
    R, fa = A("root"), A("fa")
    D("plain", R, fa, "payout", "the declaration, message and digest")
    D("root_is_address", R, R, "payout", "a declaration whose address is the root")
    for tag, role in (("len1", "a"), ("len32", "r" * 32), ("hyphen", "-"), ("digits", "0123456789"),
                      ("judge", "galeed-judge"), ("seller", "eravon-seller"), ("buyer", "eravon-buyer")):
        D("role_" + tag, R, fa, role, "role %s" % role)
    key = "%064x" % K("fa")
    D("signed", R, fa, "payout", "with the key of the address: the consent that holds", key=key)
    D("signed_0x", R, fa, "payout", "the key spelled with 0x", key="0x" + key)
    D("signed_0X_upper", R, fa, "payout", "the key spelled with 0X and uppercase digits", key="0X" + key.upper())
    D("signed_by_other", R, fa, "payout", "another key signs the same declaration: signer is not the address",
      key="%064x" % K("fb"))
    D("signed_key_one", R, fa, "payout", "the least private key", key="%064x" % 1)
    D("signed_key_max", R, fa, "payout", "the greatest private key", key="%064x" % (N - 1))
    for i in range(12):
        role = "".join(rng.choice(LABEL_ALPHABET) for _ in range(rng.randrange(1, 33)))
        root, addr = H20("d-root", i, rng.random()), H20("d-addr", i, rng.random())
        d = rng.randrange(1, N)
        D("rand_%02d" % i, root, addr, role, "a random declaration", key=("%064x" % d) if i % 2 else None)


# ==========================================================================
# carried: an entry riding its anchor (sec 5.1)
# ==========================================================================

def build_carried(rng):
    assert keccak256(b"anchor(bytes32)")[:4] == SELECTOR
    e = mk("root", b_account("x", "alice"))
    ident = bytes.fromhex(eid(e)[2:])
    good = SELECTOR + ident + e
    Cd("account", good, "selector, entry_id, entry bytes: carried")
    h = mk("root", pg.HISB(2), etype="history")
    Cd("history", SELECTOR + sha256(h) + h, "an entry of any type is carried")
    k = mk("root", b_key("fa", "payout"))
    Cd("key", SELECTOR + sha256(k) + k, "a key entry carried")
    bad = mk("root", {"act": "nothing"})
    Cd("shapeless_face_entry", SELECTOR + sha256(bad) + bad, "a face entry whose body has no shape is an entry and is carried")
    Cd("empty", b"", "no calldata")
    Cd("selector_only", SELECTOR, "four bytes")
    Cd("len35", (SELECTOR + ident)[:35], "35 bytes")
    Cd("len36", SELECTOR + ident, "exactly 36 bytes: an anchor with nothing after the hash")
    Cd("len37", SELECTOR + ident + e[:1], "37 bytes: one byte after the hash")
    Cd("id_bit_flipped", SELECTOR + bytes([ident[0] ^ 1]) + ident[1:] + e, "the hash differs from sha256 of the tail by one bit")
    Cd("tail_byte_flipped", SELECTOR + ident + e[:-2] + bytes([e[-2] ^ 1]) + e[-1:], "the tail differs by one bit")
    junk = b"not an entry at all"
    Cd("tail_not_entry", SELECTOR + sha256(junk) + junk, "the hash matches a tail that is no entry")
    v1 = mk("root", b_home(CA), spec="zikaron/1")
    Cd("tail_v1_entry", SELECTOR + sha256(v1) + v1, "the hash matches a tail whose spec is zikaron/1")
    Cd("tail_plus_lf_old_id", SELECTOR + ident + e + b"\n", "the entry and a line feed under the entry's own id")
    Cd("tail_plus_lf_new_id", SELECTOR + sha256(e + b"\n") + e + b"\n", "the hash matches entry-and-line-feed, which is not canonical")
    many = keccak256(b"anchorMany(bytes32[])")[:4]
    Cd("anchor_many", many + (32).to_bytes(32, "big") + (1).to_bytes(32, "big") + ident + e,
       "anchorMany: another selector carries nothing")
    Cd("anchor_many_shaped_like_anchor", many + ident + e, "anchorMany's selector over anchor's layout")
    Cd("selector_zero", b"\x00\x00\x00\x00" + ident + e, "a zero selector")
    Cd("selector_last_byte", SELECTOR[:3] + bytes([SELECTOR[3] ^ 1]) + ident + e, "a selector differing in its last bit")
    Cd("selector_revoke", keccak256(b"revoke(bytes32,bytes32)")[:4] + ident + e, "the revocation contract's selector")
    Cd("padded", SELECTOR + ident + b"\x00" * 32 + e, "a zero word between the hash and the entry")
    Cd("padded_hash_of_padded", SELECTOR + sha256(b"\x00" * 32 + e) + b"\x00" * 32 + e,
       "the hash matches the padded tail, which is no entry")
    e2 = mk("root", b_account("x", "bob"))
    Cd("two_entries", SELECTOR + ident + e + e2, "two entries after the hash of the first")
    Cd("id_of_other_entry", SELECTOR + sha256(e2) + e, "the hash of another entry")
    Cd("shifted_by_four", b"\x00" * 4 + good, "the calldata four bytes late")
    for i in range(24):
        c = bytearray(good)
        how = rng.randrange(4)
        if how == 0:
            c[rng.randrange(len(c))] ^= 1 << rng.randrange(8)
        elif how == 1:
            c = c[:rng.randrange(len(c))]
        elif how == 2:
            c += bytes([rng.randrange(256)])
        Cd("rand_%02d" % i, bytes(c), "the carried calldata with one random change (or none)")
    return good


# ==========================================================================
# misuse
# ==========================================================================

def build_misuse(an_entry, a_calldata, a_table):
    R, fa = A("root"), A("fa")
    Mu("no_command", [], "no command")
    Mu("unknown_command", ["faces"], "an unknown command")
    Mu("entry_no_args", ["entry"], "entry with no argument")
    Mu("entry_no_root", ["entry", an_entry], "entry without a root")
    Mu("entry_three_args", ["entry", an_entry, R, R], "entry with an argument beyond its list")
    Mu("entry_root_upper", ["entry", an_entry, "0x" + R[2:].upper()], "a root in uppercase hexadecimal")
    Mu("entry_root_short", ["entry", an_entry, R[:-2]], "a root one byte short")
    Mu("entry_root_bare", ["entry", an_entry, R[2:]], "a root without 0x")
    Mu("entry_absent", ["entry", "entries/absent.bin", R], "an absent entry file")
    Mu("entry_directory", ["entry", "entries", R], "a directory as the entry path")
    Mu("declare_two_args", ["declare", R, fa], "declare without a role")
    Mu("declare_five_args", ["declare", R, fa, "payout", "%064x" % K("fa"), "x"], "declare with an argument beyond its list")
    for tag, role in (("empty", ""), ("upper", "Payout"), ("len33", "r" * 33), ("space", "pay out"), ("dot", "pay.out")):
        Mu("declare_role_" + tag, ["declare", R, fa, role], "a role that is no label")
    Mu("declare_root_upper", ["declare", "0x" + R[2:].upper(), fa, "payout"], "a root in uppercase hexadecimal")
    Mu("declare_address_short", ["declare", R, fa[:-2], "payout"], "an address one byte short")
    Mu("declare_key_short", ["declare", R, fa, "payout", "1234"], "a private key of four digits")
    Mu("declare_key_zero", ["declare", R, fa, "payout", "0" * 64], "the private key zero")
    Mu("declare_key_n", ["declare", R, fa, "payout", "%064x" % N], "the private key n")
    Mu("declare_key_not_hex", ["declare", R, fa, "payout", "g" * 64], "a private key that is no hexadecimal")
    Mu("carried_no_args", ["carried"], "carried with no argument")
    Mu("carried_two_args", ["carried", a_calldata, a_calldata], "carried with an argument beyond its list")
    Mu("carried_absent", ["carried", "carried/absent.bin"], "an absent calldata file")
    Mu("carried_directory", ["carried", "carried"], "a directory as the calldata path")
    Mu("table_no_args", ["table"], "table with no argument")
    Mu("table_two_args", ["table", a_table, a_table], "table with an argument beyond its list")
    Mu("table_absent", ["table", "tables/absent.json"], "an absent input file")
    Mu("table_directory", ["table", "tables"], "a directory as the input path")


# ==========================================================================
# tables: the face input and the reading (sec 6, sec 7)
# ==========================================================================

def build_tables(rng):
    import copy
    R = A("root")

    # ---- the walk, scenario by scenario -------------------------------------
    w = World()
    Tb("empty", w, "a root and nothing else: no home, the root in office")
    Tb("no_chains", {"root": R, "chains": [], "anchors": [], "accessions": [], "pile": []},
       "an input that declares no chain")

    w = World()
    w.anchor(CA, 5, "root", mk("root", b_account("x", "alice")))
    Tb("bind_without_home", w, "a binding anchored by the root before any opening: UNSEATED")

    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    Tb("home_then_account", w, "the root opens its home on one chain and binds an account in the next block")

    w = homed()
    w.anchor(CA, 10, "root", mk("root", b_account("x", "alice")))
    Tb("bind_in_opening_block", w, "a binding in the opening's own block: the home takes effect from the next step")

    w = World()
    e = mk("root", b_account("x", "alice"))
    w.anchor(CA, 9, "root", e)
    w.anchor(CA, 10, "root", mk("root", b_home(CA)))
    Tb("bind_before_opening", w, "a binding anchored before the opening: UNSEATED")
    w.anchor(CA, 12, "root", e, carry=False)
    Tb("bind_before_and_after_opening", w, "the same binding anchored again after the opening: its moment is the later record")

    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_key("fa", "payout")))
    w.anchor(CA, 12, "root", mk("root", b_key("fb", "payout", sig=sign_consent("fa", R, A("fb"), "payout"))))
    Tb("key_faces", w, "a key face whose consent holds, and one whose consent does not: SIGNATURE")

    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b)
    w.anchor(CA, 20, "root", mk("root", b_release(b)))
    Tb("bind_then_release", w, "a binding and its release: one closed span, state released")
    b2 = mk("root", b_account("x", "alice", note="again"))
    w.anchor(CA, 30, "root", b2)
    Tb("release_then_bind_again", w, "the same face bound again after its release: two spans, state bound")

    w = homed()
    b1 = mk("root", b_account("x", "alice"))
    b2 = mk("root", b_account("x", "alice", note="twice"))
    w.anchor(CA, 11, "root", b1)
    w.anchor(CA, 12, "root", b2)
    w.anchor(CA, 13, "root", mk("root", b_release(b1)))
    Tb("two_bindings_one_released", w, "one face bound twice, one binding released: the row stays bound")

    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.put(b)
    hm = mk("root", b_home(CA))
    w.anchor(CA, 11, "root", mk("root", b_release(H32("nothing"))))
    w.anchor(CA, 12, "root", mk("root", b_release(b)))
    w.anchor(CA, 13, "root", hm)
    w.anchor(CA, 14, "root", mk("root", b_release(hm)))
    Tb("release_of_no_binding", w, "releases naming an id nobody holds, an unseated binding, and a home entry: SUBJECT")

    w = homed()
    b = mk("root", b_account("x", "alice"))
    q = mk("root", b_release(b))
    w.anchor(CA, 11, "root", q)
    w.anchor(CA, 15, "root", b)
    Tb("release_before_binding", w, "the release is anchored before the binding it names: the span begins and ends at the binding's moment")

    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 14, "root", b)
    w.anchor(CA, 12, "root", b, carry=False)
    w.anchor(CA, 12, "root", b, carry=False)
    q1 = mk("root", b_release(b))
    q2 = mk("root", b_release(b, note="second"))
    w.anchor(CA, 31, "root", q1)
    w.anchor(CA, 30, "root", q2)
    Tb("moments_are_earliest", w, "a binding anchored three times and released twice: the least record of each, the earlier release")

    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b)
    q1 = mk("root", b_release(b))
    q2 = mk("root", b_release(b, note="twin"))
    t = w.tx()
    w.anchor(CA, 20, "root", q1, tx=t)
    w.anchor(CA, 20, "root", q2, tx=t)
    Tb("two_releases_one_transaction", w, "two releases of one binding anchored by one transaction: the lesser entry id ends the span")

    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_account("x", "unproven")), verdict="UNPROVEN")
    w.anchor(CA, 12, "root", mk("root", b_account("x", "void")), verdict="VOID")
    w.anchor(CA, 13, "root", mk("root", b_account("x", "counted")))
    Tb("verdicts", w, "records whose verdict is UNPROVEN or VOID are not in office")

    w = World()
    w.anchor(CA, 10, "root", mk("root", b_home(CA)), verdict="VOID")
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    Tb("opening_void", w, "a home entry whose only record is VOID opens nothing")

    w = homed()
    w.anchor(CA, 11, "stranger", mk("root", b_account("x", "by-stranger-sender")))
    w.anchor(CA, 12, "root", mk("stranger", b_account("x", "by-stranger-author")))
    w.anchor(CA, 13, "stranger", mk("stranger", b_account("x", "all-stranger")))
    Tb("strangers", w, "a record sent by a stranger is not in office (UNSEATED); an entry signed by a stranger is misread AUTHOR")

    w = homed()
    old = mk("root", b_account("x", "signed-by-old"))
    new = mk("k1", b_account("x", "signed-by-new"))
    w.anchor(CA, 12, "root", new)
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", old)
    Tb("author_old_and_future", w, "an entry of the old key anchored by the new one, and an entry of the coming key anchored before it holds office: both UNSEATED")

    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_account("x", "old-in-block")))
    w.anchor(CA, 20, "k1", mk("k1", b_account("x", "new-in-block")))
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "root", mk("root", b_account("x", "old-after")))
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "new-after")))
    Tb("accession_block", w, "an accession moves nothing for the anchors of its own block: old key seated in it, new key from the next")

    w = homed()
    w.accede(CA, 20, "k1", "k2")
    w.accede(CA, 21, "stranger", "k3")
    w.anchor(CA, 22, "root", mk("root", b_account("x", "alice")))
    Tb("accession_by_non_office", w, "accessions whose holder is not the office move nothing")

    w = homed()
    w.accede(CA, 20, "root", "k1", txi=0, logi=0)
    w.accede(CA, 20, "k1", "k2", txi=1, logi=3)
    w.anchor(CA, 21, "k2", mk("k2", b_account("x", "alice")))
    Tb("two_accessions_one_block_in_order", w, "two accessions of one block taken in position order: both move the office")
    w = homed()
    w.accede(CA, 20, "root", "k1", txi=1, logi=3)
    w.accede(CA, 20, "k1", "k2", txi=0, logi=0)
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "alice")))
    Tb("two_accessions_one_block_reversed", w, "the second hand-over sits before the first in the block: it finds its holder out of office")
    w = homed()
    w.accede(CA, 20, "root", "k1", txi=2, logi=5)
    w.accede(CA, 20, "root", "k2", txi=2, logi=4)
    Tb("two_accessions_same_holder", w, "two hand-overs by one holder in one block: the lower logIndex moves the office, the other finds it gone")

    # before a home the books are read together
    w = World()
    w.accede(CA, 100, "root", "k1")
    w.accede(CB, 200, "root", "k2")
    w.anchor(CB, 150, "root", mk("root", b_home(CB)))
    w.anchor(CB, 300, "k2", mk("k2", b_home(CB)))
    w.anchor(CA, 400, "k1", mk("k1", b_home(CA)))
    w.anchor(CA, 401, "k1", mk("k1", b_account("x", "alice")))
    Tb("first_handover_wins", w, "before a home: the root hands over on one chain, then again on another; only the first moves the office; the root and the second key open nothing")

    w = World()
    w.accede(CA, 100, "root", "k1")
    w.accede(CC, 300, "root", "thief")
    w.anchor(CC, 400, "thief", mk("thief", b_home(CC)))
    w.anchor(CC, 401, "thief", mk("thief", b_account("x", "thief")))
    w.anchor(CC, 402, "root", mk("root", b_home(CC)))
    Tb("retired_key_cannot_open", w, "a key that handed the seat on hands it again on an untouched chain to a thief's key: neither opens a home")

    w = World()
    w.accede(CA, 100, "root", "k1")
    w.accede(CB, 150, "root", "k1")
    w.anchor(CA, 200, "k1", mk("k1", b_home(CA)))
    Tb("same_handover_two_chains", w, "the same hand-over recorded on two chains: the lineage shows it once")

    w = World()
    w.ts[(CB, 100)] = T0 + 100
    w.ts[(CA, 500)] = T0 + 100
    w.accede(CB, 100, "root", "k1")
    w.anchor(CA, 500, "root", mk("root", b_home(CA)))
    Tb("same_second_anchor_on_lower_chain", w, "an opening on chain 1 and a hand-over on chain 10 in one second: chain 1 goes first, the root still in office")
    w = World()
    w.ts[(CA, 100)] = T0 + 100
    w.ts[(CB, 500)] = T0 + 100
    w.accede(CA, 100, "root", "k1")
    w.anchor(CB, 500, "root", mk("root", b_home(CB)))
    Tb("same_second_anchor_on_higher_chain", w, "a hand-over on chain 1 and an opening on chain 10 in one second: the hand-over goes first, the root out of office")

    w = World()
    w.ts[(CC, 5)] = T0 + 50
    w.ts[(CA, 900)] = T0 + 60
    w.anchor(CC, 5, "root", mk("root", b_home(CC)))
    w.anchor(CA, 900, "root", mk("root", b_home(CA)))
    w.ts[(CC, 6)] = T0 + 70
    w.anchor(CC, 6, "root", mk("root", b_account("x", "on-old-home")))
    w.ts[(CA, 901)] = T0 + 80
    w.anchor(CA, 901, "root", mk("root", b_account("x", "on-new-home")))
    Tb("time_before_chain_id", w, "steps go by time before chain id: the opening on chain 137 precedes the one on chain 1")

    # once a home stands only its book moves the office
    w = homed()
    w.accede(CB, 20, "root", "k1")
    w.anchor(CA, 21, "root", mk("root", b_account("x", "still-root")))
    w.anchor(CA, 22, "k1", mk("k1", b_account("x", "k1-nowhere")))
    w.accede(CA, 30, "root", "k2")
    w.anchor(CA, 31, "k2", mk("k2", b_account("x", "k2-home")))
    Tb("home_book_alone", w, "after the opening an accession on another chain moves nothing; the home's own does")

    # moving the home
    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b)
    w.anchor(CB, 50, "root", mk("root", b_home(CB)))
    w.anchor(CA, 60, "root", mk("root", b_account("x", "late-on-old-home")))
    w.anchor(CB, 61, "root", mk("root", b_account("x", "on-new-home")))
    w.anchor(CB, 62, "root", mk("root", b_release(b)))
    Tb("home_moves", w, "the office opens a home on another chain: the old home seats nothing after, bindings made there stand and are released from the new one")

    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CB, 30, "root", mk("root", b_home(CB)))
    w.anchor(CB, 31, "root", mk("root", b_account("x", "thief")))
    w.anchor(CA, 32, "k1", mk("k1", b_account("x", "owner")))
    Tb("retired_key_cannot_move_home", w, "a key retired in the home's book declares a home elsewhere: not in office, nothing moves")

    w = homed()
    w.ts[(CA, 20)] = T0 + 20
    w.ts[(CB, 77)] = T0 + 20
    w.accede(CA, 20, "root", "k1")
    w.anchor(CB, 77, "root", mk("root", b_home(CB)))
    Tb("move_in_handover_second_higher_chain", w, "the outgoing key moves the home to chain 10 in the second of its hand-over on chain 1: the hand-over goes first")
    w = homed(chain=CB)
    w.ts[(CB, 20)] = T0 + 20
    w.ts[(CA, 77)] = T0 + 20
    w.accede(CB, 20, "root", "k1")
    w.anchor(CA, 77, "root", mk("root", b_home(CA)))
    w.anchor(CA, 78, "root", mk("root", b_account("x", "alice")))
    Tb("move_in_handover_second_lower_chain", w, "the same on the lower chain id: the opening goes first, the home moves, and the hand-over finds the home elsewhere")

    w = homed()
    w.anchor(CB, 20, "root", mk("root", b_home(CA)))
    w.anchor(CA, 21, "root", mk("root", b_home(CX)))
    w.anchor(CA, 22, "root", mk("root", b_home(CA, root=A("k1"))))
    w.anchor(CA, 23, "root", mk("root", b_account("x", "alice")))
    Tb("home_entries_that_open_nothing", w, "a home entry anchored on a chain it does not name, one naming an undeclared chain, one for another root")

    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_home(CA)))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "alice")))
    Tb("home_opened_twice", w, "a second opening on the same chain: listed, and the home stays")

    w = World()
    w.anchor(CA, 10, "root", mk("root", b_home(CA)))
    w.accede(CA, 10, "root", "k1")
    w.anchor(CA, 11, "k1", mk("k1", b_account("x", "alice")))
    Tb("opening_and_accession_first_block", w, "the first opening and a hand-over in one block: the home was nothing, so the hand-over moves the office")
    w = homed()
    w.anchor(CB, 50, "root", mk("root", b_home(CB)))
    w.accede(CB, 50, "root", "k1")
    w.anchor(CB, 51, "root", mk("root", b_account("x", "root-after-the-block")))
    w.anchor(CB, 52, "k1", mk("k1", b_account("x", "k1-after-the-block")))
    Tb("opening_and_accession_moving_block", w, "a move by the root and its hand-over on the new chain in one block: the hand-over is read under the new home and moves the office")

    w = homed()
    e = mk("root", b_account("x", "alice"))
    w.anchor(CB, 11, "root", e)
    Tb("binding_off_home", w, "a binding anchored on a chain that is not the home: UNSEATED")
    w.anchor(CA, 12, "root", e, carry=False)
    Tb("binding_on_two_chains", w, "the same binding anchored on the home too: seated there, its moment the home's record")

    # a chain without a book
    w = World(chains=((CA, False), (CB, True)))
    w.anchor(CA, 10, "root", mk("root", b_home(CA)))
    w.accede(CB, 20, "root", "k1")
    w.anchor(CA, 21, "root", mk("root", b_account("x", "alice")))
    Tb("home_without_book", w, "a home on a chain whose basis names no book: the office cannot move while the home stands there")

    # unread
    w = homed()
    w.anchor(CA, 11, "root", H32("unread", 1))
    w.anchor(CA, 12, "root", H32("unread", 1))
    w.anchor(CB, 13, "root", H32("unread", 2))
    w.anchor(CA, 14, "stranger", H32("unread", 3))
    w.anchor(CA, 15, "root", H32("unread", 4), verdict="VOID")
    w.anchor(CA, 16, "root", mk("root", pg.HISB(3), etype="history"))
    Tb("unread", w, "unread counts distinct hashes the office anchored and the pile does not hold: two here")

    # the pile
    w = homed()
    e = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", e)
    w.put(e, e, b"junk", b"", mk("root", pg.HISB(4), etype="history"), mk("root", b_account("x", "never-anchored")),
          mk("root", {"act": "nothing"}), mk("stranger", {"act": "nothing"}))
    Tb("pile", w, "the pile holds copies, bytes that are no entries, an entry of another type, and face entries nobody anchored, which the misread list leaves out")

    # rows and their order
    w = homed()
    n = 11
    for platform, handle in (("x", "bob"), ("github", "zed"), ("x", "Alice"), ("x", "alice"), ("github", "amy"),
                             ("web", "example.org")):
        w.anchor(CA, n, "root", mk("root", b_account(platform, handle)))
        n += 1
    for face, role in (("fb", "payout"), ("fa", "payout"), ("fa", "eravon-seller"), ("fa", "galeed-judge")):
        w.anchor(CA, n, "root", mk("root", b_key(face, role)))
        n += 1
    Tb("rows_in_order", w, "account rows by platform then handle, key rows by address then role; Alice and alice are two faces")

    # misread reasons, first that applies
    w = homed()
    w.anchor(CA, 11, "root", mk("stranger", b_account("x", "a", root=A("k1"))))
    w.anchor(CA, 12, "root", mk("stranger", b_key("fb", "payout", sig=sign_consent("fa", R, A("fb"), "payout"))))
    w.anchor(CA, 13, "root", mk("root", b_key("fb", "payout", sig=sign_consent("fa", R, A("fb"), "payout"))), verdict="VOID")
    w.anchor(CA, 14, "root", mk("root", b_release(H32("nothing", 2))), verdict="VOID")
    w.anchor(CA, 15, "root", mk("root", {"act": "bind", "root": A("k1")}))
    Tb("reasons_first_applies", w, "every entry anchored by the root, so every one is listed: ROOT before AUTHOR, AUTHOR before SIGNATURE, SIGNATURE before UNSEATED, SUBJECT before UNSEATED, SHAPE before ROOT")

    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b)
    w.anchor(CA, 12, "stranger", mk("root", b_release(b)))
    Tb("release_unseated", w, "a release of a standing binding whose only record a stranger sent: UNSEATED, the binding stays bound")

    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_key("fa", "payout", sig=sign_consent("fa", A("k1"), A("fa"), "payout"))))
    Tb("consent_for_another_root", w, "a key entry carrying a consent given to another root: SIGNATURE")

    # the two rails: the ledger's own succession moves nothing here
    w = homed()
    w.anchor(CA, 20, "root", mk("root", pg.SUCB(A("k1")), etype="succession"))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "old-key-after-succession")))
    w.anchor(CA, 22, "k1", mk("k1", b_account("x", "new-key-before-accession")))
    Tb("two_rails", w, "a succession entry is anchored and the book has not moved: the old key is in office, the new one is not")

    # one second, many blocks (a chain whose blocks share a second)
    w = homed()
    for bn in (20, 21, 22):
        w.ts[(CA, bn)] = T0 + 20
    w.anchor(CA, 20, "root", mk("root", b_account("x", "first")))
    w.accede(CA, 21, "root", "k1")
    w.anchor(CA, 22, "k1", mk("k1", b_account("x", "second")))
    Tb("blocks_sharing_a_second", w, "three blocks of one chain in one second keep their block order")

    # an entry of a key seated only on a branch the walk never took
    w = World()
    w.accede(CA, 100, "root", "k1")
    w.accede(CA, 110, "k1", "k2")
    w.accede(CB, 105, "k1", "k3")
    w.anchor(CA, 200, "k2", mk("k2", b_home(CA)))
    w.anchor(CA, 201, "k2", mk("k3", b_account("x", "branch")))
    Tb("line_grows_by_time", w, "before a home the hand-overs of two chains interleave by time: chain 10's comes first here and seats k3; chain 1's later one finds k1 gone")

    # ---- validity: sec 6.1 and 6.2 -------------------------------------------
    base = homed()
    base.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    base.accede(CA, 20, "root", "k1")
    base = base.value()
    assert p_table(base) != NO_TABLE

    def bad(slug, change, note):
        v = copy.deepcopy(base)
        change(v)
        assert p_table(v) == NO_TABLE, slug
        Tb("invalid_" + slug, v, note + ": NO_TABLE")

    def good(slug, change, note):
        v = copy.deepcopy(base)
        change(v)
        assert p_table(v) != NO_TABLE, slug
        Tb("valid_" + slug, v, note)

    for k in ("root", "chains", "anchors", "accessions", "pile"):
        bad("without_" + k, lambda v, k=k: v.pop(k), "an input without `%s`" % k)
    bad("extra_member", lambda v: v.update(basis={}), "a member beyond the five")
    bad("root_upper", lambda v: v.update(root="0x" + R[2:].upper()), "a root in uppercase")
    bad("root_short", lambda v: v.update(root=R[:-2]), "a root one byte short")
    for k in ("chains", "anchors", "accessions", "pile"):
        bad(k + "_object", lambda v, k=k: v.update({k: {}}), "`%s` an object" % k)
        bad(k + "_null", lambda v, k=k: v.update({k: None}), "`%s` null" % k)
    Tb("invalid_value_array", [], "a value that is an array: NO_TABLE")
    Tb("invalid_value_string", "table", "a value that is a string: NO_TABLE")
    Tb("invalid_value_null", None, "a value that is null: NO_TABLE")

    C0 = lambda v: v["chains"][0]
    for k in ("chainId", "fromBlock", "toBlock", "registries", "book", "bookFromBlock"):
        bad("chain_without_" + k, lambda v, k=k: C0(v).pop(k), "a chain object without `%s`" % k)
    bad("chain_extra_member", lambda v: C0(v).update(senders=[]), "a chain object with a member beyond its six")
    bad("chain_not_object", lambda v: v["chains"].__setitem__(0, 1), "a chain that is no object")
    bad("chain_id_string", lambda v: C0(v).update(chainId="1"), "chainId a string")
    bad("chains_descending", lambda v: v["chains"].reverse(), "chains in descending order")
    bad("chains_repeated", lambda v: v["chains"].insert(1, copy.deepcopy(v["chains"][0])), "one chain declared twice")
    bad("from_above_to", lambda v: C0(v).update(fromBlock=12, toBlock=11), "fromBlock above toBlock")
    good("from_equals_to", lambda v: (v.update(anchors=[], accessions=[]), C0(v).update(fromBlock=7, toBlock=7, bookFromBlock=7)),
         "fromBlock equal to toBlock")
    bad("registries_descending", lambda v: C0(v).update(registries=[H20("r", 2), H20("r", 2)]), "a registry listed twice")
    bad("registries_unsorted", lambda v: C0(v).update(registries=sorted([H20("r", 1), H20("r", 2)], reverse=True)),
        "registries in descending order")
    good("registries_two", lambda v: C0(v).update(registries=sorted([H20("r", 1), H20("r", 2)])), "two registries ascending")
    good("registries_empty", lambda v: C0(v).update(registries=[]), "a chain with no registry")
    bad("registries_upper", lambda v: C0(v).update(registries=["0x" + REG(CA)[2:].upper()]), "a registry in uppercase")
    bad("registries_object", lambda v: C0(v).update(registries={}), "registries an object")
    bad("book_without_start", lambda v: C0(v).update(bookFromBlock=None), "a book with no bookFromBlock")
    bad("start_without_book", lambda v: (v.update(accessions=[]), C0(v).update(book=None)), "a bookFromBlock with no book")
    bad("book_start_above_to", lambda v: C0(v).update(bookFromBlock=1000001), "bookFromBlock above toBlock")
    good("book_start_equals_to", lambda v: (v.update(accessions=[]), C0(v).update(bookFromBlock=1000000)),
         "bookFromBlock equal to toBlock")
    bad("book_upper", lambda v: C0(v).update(book="0x" + BOOK(CA)[2:].upper()), "a book in uppercase")

    A0 = lambda v: v["anchors"][1]
    for k in ("chainId", "blockNumber", "blockTimestamp", "tx", "sender", "hash", "verdict"):
        bad("anchor_without_" + k, lambda v, k=k: A0(v).pop(k), "an anchor record without `%s`" % k)
    good("anchor_extra_member", lambda v: A0(v).update(logIndex=0), "an anchor record with a member beyond its seven is read as without it")
    bad("anchor_verdict", lambda v: A0(v).update(verdict="COUNTED"), "a verdict outside the three")
    bad("anchor_verdict_null", lambda v: A0(v).update(verdict=None), "a null verdict")
    bad("anchor_tx_short", lambda v: A0(v).update(tx="0x12"), "an anchor's tx outside hex32")
    bad("anchor_sender_upper", lambda v: A0(v).update(sender="0x" + R[2:].upper()), "an anchor's sender in uppercase")
    bad("anchor_block_string", lambda v: A0(v).update(blockNumber="11"), "blockNumber a string")
    bad("anchor_chain_undeclared", lambda v: A0(v).update(chainId=CX), "an anchor on a chain the basis does not declare")
    bad("anchor_below_range", lambda v: C0(v).update(fromBlock=11), "an anchor below its chain's fromBlock")
    good("anchor_at_from", lambda v: C0(v).update(fromBlock=10), "an anchor in the block fromBlock names")
    bad("anchor_above_range", lambda v: C0(v).update(toBlock=19), "a record above its chain's toBlock")
    good("anchor_at_to", lambda v: (v.update(accessions=[]), C0(v).update(toBlock=11)), "an anchor in the block toBlock names")
    good("anchor_twice", lambda v: v["anchors"].append(copy.deepcopy(A0(v))), "one anchor record given twice is one record")
    bad("anchor_twice_other_verdict", lambda v: v["anchors"].append(dict(A0(v), verdict="VOID")),
        "two records agreeing on chain, block, tx and hash")
    good("anchor_same_tx_other_hash", lambda v: v["anchors"].append(dict(A0(v), hash=H32("other"))),
         "two records of one transaction with two hashes are two records")
    good("anchor_same_hash_other_block", lambda v: v["anchors"].append(dict(A0(v), blockNumber=12, blockTimestamp=T0 + 12)),
         "one transaction hash in two blocks is two records")

    X0 = lambda v: v["accessions"][0]
    for k in ("chainId", "blockNumber", "blockTimestamp", "txIndex", "logIndex", "tx", "holder", "to"):
        bad("accession_without_" + k, lambda v, k=k: X0(v).pop(k), "an accession record without `%s`" % k)
    bad("accession_extra_member", lambda v: X0(v).update(emitter=R), "an accession record with a member beyond its nine")
    bad("accession_holder_short", lambda v: X0(v).update(holder="0x12"), "a holder outside hex20")
    bad("accession_log_index_string", lambda v: X0(v).update(logIndex="0"), "logIndex a string")
    bad("accession_chain_undeclared", lambda v: X0(v).update(chainId=CX), "an accession on an undeclared chain")
    bad("accession_no_book", lambda v: v["chains"][0].update(book=None, bookFromBlock=None), "an accession on a chain whose basis names no book")
    bad("accession_below_book_start", lambda v: C0(v).update(bookFromBlock=21), "an accession below bookFromBlock")
    good("accession_at_book_start", lambda v: C0(v).update(bookFromBlock=20), "an accession in the block bookFromBlock names")
    good("accession_at_to", lambda v: C0(v).update(toBlock=20), "an accession in the block toBlock names")
    bad("accession_twice", lambda v: v["accessions"].append(copy.deepcopy(X0(v))), "one accession record twice")
    bad("accession_same_position", lambda v: v["accessions"].append(dict(X0(v), to=A("k2"))),
        "two accession records at one position")
    good("accession_other_log_index", lambda v: v["accessions"].append(dict(X0(v), logIndex=1, to=A("k2"))),
         "two accession records of one transaction at two log indexes")

    bad("clock_two_times_one_block", lambda v: v["anchors"].append(dict(A0(v), tx=H32("tx-clock"), blockTimestamp=T0 + 12)),
        "two records of one block with two timestamps")
    bad("clock_anchor_and_accession", lambda v: X0(v).update(blockNumber=11), "an anchor and an accession of one block with two timestamps")
    bad("clock_runs_back", lambda v: A0(v).update(blockTimestamp=T0 + 9), "a later block with an earlier timestamp")
    good("clock_stands_still", lambda v: A0(v).update(blockTimestamp=T0 + 10), "two blocks of one chain in one second")
    good("clock_other_chain_free", lambda v: v["anchors"].append(dict(A0(v), chainId=CB, blockNumber=11, blockTimestamp=5)),
         "each chain keeps its own clock")

    bad("pile_no_prefix", lambda v: v["pile"].append("abcd"), "a pile element without 0x")
    bad("pile_upper_prefix", lambda v: v["pile"].append("0Xabcd"), "a pile element spelled 0X")
    bad("pile_odd", lambda v: v["pile"].append("0xabc"), "a pile element of an odd number of digits")
    bad("pile_not_hex", lambda v: v["pile"].append("0xzz"), "a pile element that is no hexadecimal")
    bad("pile_int", lambda v: v["pile"].append(5), "a pile element that is an int")
    good("pile_upper_digits", lambda v: v["pile"].__setitem__(0, "0x" + v["pile"][0][2:].upper()),
         "pile digits in uppercase spell the same bytes")
    good("pile_empty_string", lambda v: v["pile"].append("0x"), "the empty byte string in the pile")

    # ---- the transport: files the reader refuses, and spellings it accepts -----
    text = json.dumps(base, separators=(",", ":"))
    Tr("file_empty", b"", "an empty file")
    Tr("file_not_json", b"{", "a file that is no JSON")
    Tr("file_bom", b"\xef\xbb\xbf" + text.encode(), "a byte order mark before the value")
    Tr("file_bad_utf8", text.encode().replace(b'"pile"', b'"pile\xff"', 1), "bytes that are no UTF-8")
    Tr("file_leading_zero", text.replace('"fromBlock":0', '"fromBlock":00', 1).encode(), "an integer with a leading zero")
    Tr("file_fraction", text.replace('"fromBlock":0', '"fromBlock":0.0', 1).encode(), "a number with a fraction")
    Tr("file_exponent", text.replace('"toBlock":1000000', '"toBlock":1e6', 1).encode(), "a number with an exponent")
    Tr("file_negative", text.replace('"fromBlock":0', '"fromBlock":-0', 1).encode(), "a negative zero")
    Tr("file_int_2p53", text.replace('"toBlock":1000000', '"toBlock":9007199254740992', 1).encode(), "an integer of 2^53")
    Tr("file_duplicate_member", text.replace('{"root"', '{"root":"%s","root"' % R, 1).encode(), "a member name twice")
    Tr("file_lone_surrogate", text.replace('"pile":[', '"pile":["\\ud800",', 1).encode(), "a lone surrogate escape")
    Tr("file_depth_129", text.replace('"pile":[', '"pile":[' + "[" * 128 + "]" * 128 + ",", 1).encode(),
       "a container opening at depth 129")
    v = copy.deepcopy(base)
    v["pile"].insert(0, "x")
    for _ in range(126):
        v["pile"][0] = [v["pile"][0]]
    Tr("file_depth_128", json.dumps(v, separators=(",", ":")).encode(),
       "containers to depth 128 are read; the pile then holds an array, and the input is not valid", value=v)
    Tr("file_trailing_garbage", (text + "x").encode(), "a value followed by a byte")
    Tr("file_two_values", (text + text).encode(), "two values")
    Tr("spelling_pretty", json.dumps(base, indent=2, sort_keys=True).encode(), "the same value indented and re-ordered", value=base)
    Tr("spelling_escapes", text.replace('"root"', '"\\u0072oot"', 1).replace('"counted"', '"\\u0063ounted"', 1).encode(),
       "member names and strings spelled with escapes", value=base)
    Tr("spelling_whitespace", (" \n\t" + text.replace(",", " ,\n") + "\n").encode(), "whitespace around tokens", value=base)
    Tr("int_max", text.replace('"toBlock":1000000', '"toBlock":9007199254740991', 1).encode(),
       "toBlock 2^53 - 1", value=dict(base, chains=[dict(base["chains"][0], toBlock=MAXINT)] + base["chains"][1:]))
    return text


# ==========================================================================
# random worlds
# ==========================================================================

def build_random(rng, count, seed):
    seat = ["root", "k1", "k2", "k3"]
    for n in range(count):
        other = random.Random("face-corpus/book-root/%d/%d" % (seed, n))   # its own stream: the worlds keep their draws,
        # and `office` below goes on tracking by holder alone, a guide for the draws and no prediction
        books = [(CA, True), (CB, rng.random() < 0.85), (CC, rng.random() < 0.6)]
        w = World(chains=books)
        with_book = [c for c, b in books if b]
        chains = [c for c, _ in books]
        office, home = "root", None
        binds = []
        ents = []
        block = 1
        second = T0
        for step in range(rng.randrange(6, 34)):
            block += rng.randrange(1, 4)
            second += rng.choice([0, 0, 1, 1, 2, 7])
            chain = rng.choice(chains) if home is None or rng.random() < 0.3 else home
            w.ts[(chain, block)] = second
            roll = rng.random()
            sender = office if rng.random() < 0.75 else rng.choice(seat + ["thief"])
            if roll < 0.18 and chain in with_book:
                holder = office if rng.random() < 0.7 else rng.choice(seat + ["thief"])
                to = rng.choice(seat + ["thief"])
                under = None if other.random() < 0.92 else other.choice(seat + ["thief", "stranger"])
                w.accede(chain, block, holder, to, txi=rng.randrange(3), logi=rng.randrange(1000), root=under)
                if holder == office and (home is None or home == chain):
                    office = to
                if rng.random() < 0.3:
                    w.anchor(chain, block, sender, mk(sender, b_account("x", "r%d-%d" % (n, step))))
            elif roll < 0.36:
                target = rng.choice(chains) if rng.random() < 0.8 else chain
                author = sender if rng.random() < 0.85 else rng.choice(seat + ["thief"])
                w.anchor(chain, block, sender, mk(author, b_home(target)),
                         verdict=rng.choice(["counted"] * 8 + ["VOID", "UNPROVEN"]))
                if sender == office and target == chain:
                    home = chain
            elif roll < 0.86:
                author = sender if rng.random() < 0.85 else rng.choice(seat + ["thief"])
                kind = rng.random()
                if kind < 0.45:
                    e = mk(author, b_account(rng.choice(["x", "github", "web"]), rng.choice(["alice", "bob", "Alice", "carol"])))
                    binds.append(e)
                elif kind < 0.7:
                    face = rng.choice(["fa", "fb"])
                    sig = None if rng.random() < 0.85 else sign_consent("fa", A("k1"), A(face), "payout")
                    e = mk(author, b_key(face, rng.choice(["payout", "eravon-seller"]), sig=sig))
                    binds.append(e)
                elif binds:
                    e = mk(author, b_release(rng.choice(binds)))
                else:
                    e = mk(author, b_release(H32("none", n, step)))
                ents.append(e)
                w.anchor(chain, block, sender, e, verdict=rng.choice(["counted"] * 9 + ["VOID"]),
                         carry=rng.random() < 0.93)
            elif ents:
                w.anchor(chain, block, sender, rng.choice(ents), carry=False)
            else:
                w.anchor(chain, block, sender, H32("loose", n, step))
        Tb("random_%03d" % n, w, "a random world: hand-overs, openings, bindings and releases by seat keys and others over up to three chains")


# ==========================================================================
# boundaries the cases above leave one-sided (round 1, seat D): hand-built,
# seed-independent, appended after the random worlds so that every earlier
# case keeps its bytes
# ==========================================================================

def build_boundaries():
    import copy
    R, fa = A("root"), A("fa")

    # sec 1: a label's alphabet at the edges of its three ranges
    for tag, val, ok in (("edges", "az09-", True), ("z", "z", True), ("nine", "9", True),
                         ("backtick", "`", False), ("brace", "{", False), ("colon", ":", False),
                         ("at", "@", False), ("upper_z", "Z", False)):
        E("platform_" + tag, mk("root", dict(b_account("x", "alice"), platform=val)),
          "`platform` %r: %s" % (val, "a label" if ok else "no label, no shape"))
        sig = sign_consent("fa", R, fa, val) if ok else sign_consent("fa", R, fa, "payout")
        E("role_" + tag, mk("root", dict(b_key("fa", "payout", sig=sig), role=val)),
          "`role` %r: %s" % (val, "a label, the consent names it" if ok else "no label, no shape"))
    D("role_edges", R, fa, "az09-", "a role at the edges of the label alphabet")
    for tag, role in (("backtick", "`"), ("brace", "{"), ("colon", ":")):
        Mu("declare_role_" + tag, ["declare", R, fa, role], "a role that is no label")

    # sec 6.1 and 6.2 over a base input of its own
    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    w.accede(CA, 20, "root", "k1")
    base = w.value()

    def var(slug, change, note, valid):
        v = copy.deepcopy(base)
        change(v)
        assert (p_table(v) != NO_TABLE) == valid, slug
        Tb(("valid_" if valid else "invalid_") + slug, v, note if valid else note + ": NO_TABLE")

    C0 = lambda v: v["chains"][0]
    A1 = lambda v: v["anchors"][1]
    X0 = lambda v: v["accessions"][0]
    var("chain_from_string", lambda v: C0(v).update(fromBlock="0"), "fromBlock a string", False)
    var("chain_to_string", lambda v: C0(v).update(toBlock="1000000"), "toBlock a string", False)
    var("chain_book_start_string", lambda v: C0(v).update(bookFromBlock="0"), "bookFromBlock a string beside a book", False)
    var("chain_book_int", lambda v: C0(v).update(book=1), "a book that is an int", False)
    var("chain_registry_int", lambda v: C0(v).update(registries=[1]), "a registry that is an int", False)
    var("anchor_hash_short", lambda v: A1(v).update(hash="0x12"), "an anchor's hash outside hex32", False)
    var("anchor_hash_upper", lambda v: A1(v).update(hash="0x" + A1(v)["hash"][2:].upper()), "an anchor's hash in uppercase", False)
    var("anchor_time_string", lambda v: A1(v).update(blockTimestamp=str(A1(v)["blockTimestamp"])), "blockTimestamp a string", False)
    var("accession_to_upper", lambda v: X0(v).update(to="0x" + X0(v)["to"][2:].upper()), "an accession's to in uppercase", False)
    var("accession_tx_short", lambda v: X0(v).update(tx="0x12"), "an accession's tx outside hex32", False)
    var("accession_tx_index_string", lambda v: X0(v).update(txIndex="0"), "txIndex a string", False)
    var("accession_time_string", lambda v: X0(v).update(blockTimestamp=str(X0(v)["blockTimestamp"])), "an accession's blockTimestamp a string", False)
    var("anchor_same_hash_other_tx", lambda v: v["anchors"].append(dict(A1(v), tx=H32("other-tx"))),
        "two records of one block and one hash in two transactions are two records", True)
    var("accession_other_tx_index", lambda v: v["accessions"].append(dict(X0(v), txIndex=1, to=A("k2"))),
        "two accession records of one block at one logIndex and two txIndexes", True)
    var("accession_same_position_other_block", lambda v: v["accessions"].append(dict(X0(v), blockNumber=21, blockTimestamp=X0(v)["blockTimestamp"] + 1, to=A("k2"))),
        "one position in two blocks", True)
    var("accession_same_position_other_chain", lambda v: v["accessions"].append(dict(X0(v), chainId=CB, to=A("k2"))),
        "one position on two chains", True)
    var("accession_below_from_block", lambda v: (C0(v).update(fromBlock=5, bookFromBlock=0), X0(v).update(blockNumber=3, blockTimestamp=T0 + 3)),
        "an accession below the chain's fromBlock and within the book's range: bookFromBlock has no lower bound but 0", True)
    var("clock_accession_runs_back", lambda v: X0(v).update(blockTimestamp=T0 + 5),
        "an accession in a later block than an anchor of its chain with an earlier timestamp", False)
    var("clock_two_accessions_one_block", lambda v: v["accessions"].append(dict(X0(v), logIndex=1, blockTimestamp=X0(v)["blockTimestamp"] + 1)),
        "two accession records of one block with two timestamps", False)

    # sec 7.2: accessions of one block by txIndex before logIndex
    w = homed()
    w.accede(CA, 20, "root", "k1", txi=0, logi=9)
    w.accede(CA, 20, "k1", "k2", txi=1, logi=2)
    w.anchor(CA, 21, "k2", mk("k2", b_account("x", "alice")))
    Tb("accessions_tx_index_first", w, "two accessions of one block whose txIndex order and logIndex order disagree: txIndex decides, both move the office")

    # sec 7.2: a hand-over to the key in office is appended to the line
    w = homed()
    w.accede(CA, 20, "root", "root")
    w.accede(CA, 21, "root", "k1")
    Tb("handover_to_itself", w, "a hand-over whose to is its holder moves the office to the same key and lengthens the line")

    # sec 7.2 and 7.6: the author of a home entry
    w = World()
    w.anchor(CA, 10, "root", mk("stranger", b_home(CA)))
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    Tb("home_by_stranger_author", w, "a home entry signed by a stranger and anchored by the office opens nothing: AUTHOR")
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.anchor(CA, 10, "k1", mk("root", b_home(CA)))
    w.anchor(CA, 11, "k1", mk("k1", b_account("x", "alice")))
    Tb("home_by_earlier_key", w, "a home entry signed by the root and anchored by its successor in office opens nothing: no home, both entries UNSEATED")

    # sec 7.3: the seated order goes by chainId before blockNumber at one second
    w = World()
    w.ts[(CA, 5)] = T0 + 5
    w.anchor(CA, 5, "root", mk("root", b_home(CA)))
    e = mk("root", b_account("x", "alice"))
    w.ts[(CA, 9)] = T0 + 50
    w.ts[(CB, 3)] = T0 + 50
    w.ts[(CB, 4)] = T0 + 50
    w.anchor(CA, 9, "root", e)
    w.anchor(CB, 3, "root", mk("root", b_home(CB)))
    w.anchor(CB, 4, "root", e, carry=False)
    Tb("moment_chain_before_block", w, "one binding seated on chain 1 at block 9 and on chain 10 at block 4 in one second: its moment is chain 1's record")

    # sec 7.8: ties broken by entry
    w = World()
    t = w.tx()
    w.anchor(CA, 10, "root", mk("root", b_home(CA)), tx=t)
    w.anchor(CA, 10, "root", mk("root", b_home(CA)), tx=t)
    Tb("two_openings_one_transaction", w, "two home entries opened by one transaction: homes ordered by entry")
    w = homed()
    t = w.tx()
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice", note="one")), tx=t)
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice", note="two")), tx=t)
    Tb("two_bindings_one_transaction", w, "two bindings of one face seated by one transaction: spans ordered by entry")

    # sec 7.4 and 7.6: a release of a release; AUTHOR before SUBJECT
    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b)
    q = mk("root", b_release(b))
    w.anchor(CA, 12, "root", q)
    w.anchor(CA, 13, "root", mk("root", b_release(q)))
    w.anchor(CA, 14, "root", mk("stranger", b_release(H32("nothing", 3))))
    Tb("release_of_release_and_stranger_release", w, "a release naming a standing release: SUBJECT; a stranger's release of nothing: AUTHOR before SUBJECT")

    # sec 7.7: unread is read against the office of its step
    w = World()
    w.anchor(CA, 5, "root", H32("unread", 10))
    w.accede(CA, 6, "root", "k1")
    w.anchor(CA, 7, "root", H32("unread", 11))
    w.anchor(CA, 8, "k1", H32("unread", 12))
    Tb("unread_before_home_by_step_office", w, "before any home: the root's hash before its hand-over and k1's after count, the root's after does not")

    # sec 7.8: every opening record is listed, one entry opened twice included
    w = World()
    h = mk("root", b_home(CA))
    w.anchor(CA, 10, "root", h)
    w.anchor(CA, 12, "root", h, carry=False)
    Tb("one_home_entry_opened_twice", w, "one home entry anchored twice by the office: homes lists both opening records")

    # sec 7.3: the tx tie of a moment whatever order the records are given in
    w = homed()
    b = mk("root", b_account("x", "alice"))
    lo, hi = sorted([H32("tie", 1), H32("tie", 2)])
    w.anchor(CA, 11, "root", b, tx=hi)
    w.anchor(CA, 11, "root", b, tx=lo, carry=False)
    Tb("moment_tx_tie_greater_listed_first", w, "one binding seated twice in one block, the greater tx listed first: its moment is the lesser tx")

    # sec 6.1: the two upper bounds of a chain's ranges, each the only fault of its input
    var("from_above_to_alone", lambda v: (v.update(anchors=[], accessions=[]), C0(v).update(fromBlock=12, toBlock=11, bookFromBlock=0)),
        "fromBlock above toBlock in an input with no record to fall outside it", False)
    var("book_start_above_to_alone", lambda v: (v.update(accessions=[]), C0(v).update(bookFromBlock=1000001)),
        "bookFromBlock above toBlock in an input with no accession to fall outside it", False)


# ==========================================================================
# round 1, the other seats' witnesses and the fold's two new rules: hand-built,
# seed-independent, after every earlier case
# ==========================================================================

def build_round1():
    R = A("root")

    # sec 7.5: a release in its binding's block with the lesser tx ends the span at the binding's moment
    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b, tx="0x" + "f" * 64)
    w.anchor(CA, 11, "root", mk("root", b_release(b)), tx="0x" + "0" * 63 + "1")
    Tb("release_same_block_lesser_tx", w, "a release in its binding's block whose tx is the lesser: the release's moment precedes the binding's by tx, and the span ends at the binding's moment")

    # sec 7.7: bytes in the pile that accept rejects hold nothing
    w = homed()
    junk = b"not an entry"
    w.anchor(CA, 11, "root", eid(junk), carry=False)
    w.put(junk)
    Tb("unread_rejected_bytes", w, "the office anchors the sha256 of bytes the pile holds and accept rejects: unread 1")

    # sec 7.5: the state reads every span
    w = homed()
    b1 = mk("root", b_account("x", "alice"))
    b2 = mk("root", b_account("x", "alice", note="later"))
    w.anchor(CA, 11, "root", b1)
    w.anchor(CA, 12, "root", b2)
    w.anchor(CA, 13, "root", mk("root", b_release(b2)))
    Tb("later_of_two_bindings_released", w, "one face bound twice, the later binding released: the earlier span is open, the row is bound")

    # sec 7.8: spans of one block ordered by tx before entry
    w = homed()
    p = mk("root", b_account("x", "alice", note="p"))
    q = mk("root", b_account("x", "alice", note="q"))
    hi_e, lo_e = (p, q) if eid(p) > eid(q) else (q, p)
    w.anchor(CA, 11, "root", hi_e, tx="0x" + "0" * 63 + "1")
    w.anchor(CA, 11, "root", lo_e, tx="0x" + "0" * 63 + "2")
    Tb("spans_tx_before_entry", w, "two spans of one row in one block: the greater entry id with the lesser tx comes first")

    # sec 7.7: unproven records by the office, and by others
    w = World()
    w.anchor(CA, 10, "root", mk("root", b_home(CA)), verdict="UNPROVEN")
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    Tb("unproven_opening", w, "the only record of the opening is UNPROVEN: no home, the binding UNSEATED, unproven 1")
    w = homed()
    b = mk("root", b_account("x", "stolen"))
    w.anchor(CA, 11, "root", b)
    w.anchor(CA, 20, "root", mk("root", b_release(b)), verdict="UNPROVEN")
    Tb("unproven_release", w, "the release of a binding is UNPROVEN: the row reads bound and unproven is 1")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "root", mk("root", b_account("x", "a")), verdict="UNPROVEN")
    w.anchor(CA, 22, "stranger", mk("stranger", b_account("x", "b")), verdict="UNPROVEN")
    w.anchor(CA, 23, "k1", mk("k1", b_account("x", "c")), verdict="UNPROVEN")
    w.anchor(CA, 24, "k1", mk("k1", b_account("x", "c")), verdict="UNPROVEN", carry=False)
    Tb("unproven_counts_the_office", w, "UNPROVEN records by a retired key and by a stranger count nothing; two by the office count two")

    # sec 7.6: who adds to the misread list
    w = homed()
    w.accede(CA, 30, "root", "k1")
    w.anchor(CA, 11, "stranger", mk("stranger", b_account("x", "flood-1")))
    w.anchor(CA, 12, "stranger", mk("stranger", b_key("fb", "payout")))
    w.anchor(CA, 13, "stranger", mk("root", b_account("x", "copied-by-stranger")))
    w.put(mk("root", b_account("x", "never-anchored")))
    w.anchor(CA, 14, "root", mk("root", b_account("x", "void")), verdict="VOID")
    w.anchor(CA, 31, "root", mk("root", b_account("x", "after-retirement")))
    w.anchor(CA, 32, "root", mk("stranger", b_account("x", "thief-signed-retired-sent")))
    Tb("misread_scope", w, "listed: what a key of the line anchored, whatever the verdict or the time (the VOID binding, the retired root's after its hand-over); left out: what strangers anchored and what nobody anchored")
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 6, "root", "k2")
    w.anchor(CB, 7, "k2", mk("k2", b_home(CB)))
    w.anchor(CB, 8, "k2", mk("k2", b_account("x", "fork-key")))
    Tb("misread_scope_fork_key", w, "a key the retired root handed the seat to on a second book never joins the line: its entries are left out")


# ==========================================================================
# round 2, seat D2: the numeric orders of sec 6.1, 7.1 and 7.3 across a digit
# count, the bounds and forms of sec 6 each as the input's only fault, the
# opening's type and shape, a release's root, the reason order beside ROOT,
# the unproven count off the home and without bytes, and the tie orders that
# held at some seeds alone: hand-built, seed-independent, after every earlier
# case
# ==========================================================================

def build_round2():
    import copy
    R = A("root")

    # sec 6.1: chains ascend by chainId compared numerically; 9 and 10 order apart as decimal strings
    w = World(chains=((9, True), (10, True)))
    Tb("valid_chains_nine_then_ten", w, "chains 9 then 10: ascending as numbers, descending as decimal strings")
    v = World(chains=((9, True), (10, True))).value()
    v["chains"].reverse()
    assert p_table(v) == NO_TABLE
    Tb("invalid_chains_ten_then_nine", v, "chains 10 then 9: descending as numbers, ascending as decimal strings: NO_TABLE")

    # sec 6.1 and 6.2 over a base input of its own, each change the input's only fault
    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    w.accede(CA, 20, "root", "k1")
    base = w.value()

    def var(slug, change, note, valid):
        v = copy.deepcopy(base)
        change(v)
        assert (p_table(v) != NO_TABLE) == valid, slug
        Tb(("valid_" if valid else "invalid_") + slug, v, note if valid else note + ": NO_TABLE")

    C0 = lambda v: v["chains"][0]
    A1 = lambda v: v["anchors"][1]
    X0 = lambda v: v["accessions"][0]
    var("anchor_above_range_alone", lambda v: (C0(v).update(toBlock=20), A1(v).update(blockNumber=21, blockTimestamp=T0 + 21)),
        "an anchor one block above toBlock in an input whose accession lies within the range", False)
    var("chain_from_false", lambda v: v["chains"][2].update(fromBlock=False),
        "fromBlock false on a chain no record names: a JSON boolean is no int", False)
    var("accession_tx_index_true", lambda v: X0(v).update(txIndex=True), "txIndex true: a JSON boolean is no int", False)
    var("clock_anchor_runs_back_after_accession",
        lambda v: v["anchors"].append(dict(A1(v), blockNumber=25, blockTimestamp=T0 + 15, tx=H32("tx-late"))),
        "an anchor in a later block than an accession of its chain with an earlier timestamp", False)

    # sec 7.1: steps compared numerically, each where the decimal strings order the other way
    w = World(chains=((9, True), (10, True)))
    w.ts[(9, 50)] = T0 + 50
    w.ts[(10, 40)] = T0 + 50
    w.accede(9, 50, "root", "k1")
    w.anchor(10, 40, "root", mk("root", b_home(10)))
    Tb("steps_chain_nine_before_ten", w, "a hand-over on chain 9 and the root's opening on chain 10 in one second: chain 9 goes first, so the root opens nothing")
    w = World()
    w.ts[(CA, 9)] = T0 + 9
    w.ts[(CA, 10)] = T0 + 9
    w.accede(CA, 9, "root", "k1")
    w.anchor(CA, 10, "k1", mk("k1", b_home(CA)))
    Tb("steps_block_nine_before_ten", w, "a hand-over in block 9 and the new key's opening in block 10 of one chain in one second: block 9 goes first and the home opens")
    w = World()
    w.ts[(CB, 3)] = 9
    w.ts[(CA, 4)] = 10
    w.accede(CB, 3, "root", "k1")
    w.anchor(CA, 4, "root", mk("root", b_home(CA)))
    Tb("steps_second_nine_before_ten", w, "a hand-over at second 9 on chain 10 and the root's opening at second 10 on chain 1: the hand-over goes first, so the root opens nothing")

    # sec 7.3: a moment compares chainId and blockNumber numerically
    w = World(chains=((9, True), (10, True)))
    w.ts[(9, 5)] = T0 + 5
    w.anchor(9, 5, "root", mk("root", b_home(9)))
    e = mk("root", b_account("x", "alice"))
    for c, bn in ((9, 6), (10, 2), (10, 3)):
        w.ts[(c, bn)] = T0 + 60
    w.anchor(9, 6, "root", e)
    w.anchor(10, 2, "root", mk("root", b_home(10)))
    w.anchor(10, 3, "root", e, carry=False)
    Tb("moment_chain_nine_before_ten", w, "one binding seated on chain 9 and, after a move, on chain 10 in one second: its moment is chain 9's record")
    w = homed()
    e = mk("root", b_account("x", "alice"))
    w.ts[(CA, 99)] = T0 + 99
    w.ts[(CA, 100)] = T0 + 99
    w.anchor(CA, 100, "root", e, tx="0x" + "0" * 63 + "1")
    w.anchor(CA, 99, "root", e, carry=False, tx="0x" + "f" * 64)
    Tb("moment_block_ninety_nine_before_hundred", w, "one binding seated in blocks 99 and 100 of one chain in one second: its moment is block 99, whose tx is the greater")

    # sec 7.2: an opening is a face entry of the home shape
    w = World()
    w.anchor(CA, 10, "root", mk("root", b_home(CA), etype="zikaron.face/2"))
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    Tb("home_body_of_another_type", w, "an entry of another type whose body is a home body, anchored by the office on the chain it names: no opening, and the entry is no misread face entry")
    w = World()
    w.anchor(CA, 10, "root", mk("root", dict(b_home(CA), extra=1)))
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    Tb("home_body_beyond_its_row", w, "a face entry whose home body carries a member beyond its row, anchored by the office on the chain it names: no opening, SHAPE")

    # sec 7.4: a release reads only under R
    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b)
    w.anchor(CA, 12, "root", mk("root", b_release(b, root=A("k1"))))
    Tb("release_under_another_root", w, "a release under another root that names a standing binding: ROOT, and the row stays bound")

    # sec 7.5: the least moment ends a span whatever the entry ids say
    w = homed()
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 11, "root", b)
    q1 = mk("root", b_release(b, note="one"))
    q2 = mk("root", b_release(b, note="two"))
    lo, hi = (q1, q2) if eid(q1) < eid(q2) else (q2, q1)
    w.anchor(CA, 20, "root", hi)
    w.anchor(CA, 30, "root", lo)
    Tb("release_least_moment_greater_entry", w, "two releases of one binding, the earlier the greater entry id: the earlier ends the span")

    # sec 7.8: homes by the opening record before the entry, whatever the entry ids say
    w = World()
    h1 = mk("root", b_home(CA))
    h2 = mk("root", b_home(CA))
    lo, hi = (h1, h2) if eid(h1) < eid(h2) else (h2, h1)
    w.anchor(CA, 10, "root", hi)
    w.anchor(CA, 20, "root", lo)
    Tb("homes_by_record_before_entry", w, "two openings, the earlier record's entry the greater: homes lists the earlier record first")

    # sec 7.7: the unproven count off the home, and of a hash whose bytes nobody holds
    w = homed()
    w.anchor(CB, 30, "root", mk("root", b_account("x", "elsewhere")), verdict="UNPROVEN")
    Tb("unproven_off_home", w, "an UNPROVEN record by the office on a chain that is not the home: unproven 1")
    w = homed()
    w.anchor(CA, 30, "root", H32("unproven-unheld"), verdict="UNPROVEN")
    Tb("unproven_hash_unheld", w, "an UNPROVEN record by the office of a hash the pile does not hold: unproven 1, unread 0")

    # sec 7.6: ROOT before SIGNATURE, and ROOT before SUBJECT
    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_key("fa", "payout", root=A("k1"), sig=sign_consent("fa", R, A("fa"), "payout"))))
    w.anchor(CA, 12, "root", mk("root", b_release(H32("nothing", 4), root=A("k1"))))
    Tb("reasons_root_before_signature_and_subject", w, "a key entry under another root whose consent fails, and a release under another root of no binding, both anchored by the root: ROOT for both")

    # sec 7.8: key rows by address before role, whichever of the two addresses the seed makes lesser
    lo, hi = ("fa", "fb") if A("fa") < A("fb") else ("fb", "fa")
    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_key(hi, "aa")))
    w.anchor(CA, 12, "root", mk("root", b_key(lo, "zz")))
    Tb("key_rows_address_before_role", w, "two key rows, the lesser address with the greater role: the lesser address comes first")


# ==========================================================================
# the closed vocabularies and what witnesses them
# ==========================================================================

VOCAB = (["entry:rejected", "entry:other-type", "shape:none", "rooted:true", "rooted:false",
          "consent:true", "consent:false", "declare:plain", "declare:signed",
          "carried:entry", "carried:null", "table:NO_TABLE", "table:read", "home:null", "home:set",
          "lineage:root-alone", "lineage:moved", "homes:none", "homes:one", "homes:several",
          "span:open", "span:closed", "unread:zero", "unread:nonzero", "unproven:zero",
          "unproven:nonzero", "persona:null", "persona:set", "misuse"]
         + ["shape:" + s for s in SHAPES] + ["state:bound", "state:released"]
         + ["reason:" + r for r in REASONS]
         + ["reroot:null", "reroot:set", "origin:null", "origin:set",
            "joined:true", "joined:false", "joined:NO_TABLE"])


def labels_of(case, answer):
    """The vocabulary labels one case's answer witnesses; `answer` is the parsed stdout, or None for misuse."""
    cmd = case["cmd"]
    if answer is None:
        return ["misuse"]
    out = []
    if cmd == "entry":
        if not answer["ok"]:
            return ["entry:rejected"]
        if not answer["face"]:
            return ["entry:other-type"]
        out.append("shape:" + (answer["shape"] or "none"))
        if answer["rooted"] is not None:
            out.append("rooted:" + ("true" if answer["rooted"] else "false"))
        if answer["consent"] is not None:
            out.append("consent:" + ("true" if answer["consent"] else "false"))
    elif cmd == "declare":
        out.append("declare:signed" if "sig" in answer else "declare:plain")
    elif cmd == "carried":
        out.append("carried:null" if answer["carried"] is None else "carried:entry")
    elif cmd == "table":
        if answer.get("ok") is False:
            return ["table:NO_TABLE"]
        out.append("table:read")
        out.append("home:null" if answer["home"] is None else "home:set")
        out.append("lineage:root-alone" if len(answer["lineage"]) == 1 else "lineage:moved")
        out.append("homes:" + ("none" if not answer["homes"] else "one" if len(answer["homes"]) == 1 else "several"))
        out.append("unread:zero" if answer["unread"] == 0 else "unread:nonzero")
        out.append("unproven:zero" if answer["unproven"] == 0 else "unproven:nonzero")
        out.append("persona:null" if answer["persona"] is None else "persona:set")
        out.append("reroot:null" if answer["reroot"] is None else "reroot:set")
        out.append("origin:null" if answer["origin"] is None else "origin:set")
        for row in answer["faces"]:
            out.append("state:" + row["state"])
            for sp in row["spans"]:
                out.append("span:open" if sp["release"] is None else "span:closed")
        for m in answer["misread"]:
            out.append("reason:" + m["reason"])
    elif cmd == "joined":
        if answer.get("ok") is False:
            return ["joined:NO_TABLE"]
        out.append("joined:true" if answer["joined"] else "joined:false")
    return out


# ==========================================================================
# round 2, the readers' witnesses (seats A2 and E2): hand-built,
# seed-independent, after every earlier case
# ==========================================================================

def build_round2_readers():
    import copy

    # sec 7.7: the counts read the office as the step found it, inside the hand-over block
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "root", mk("root", b_account("x", "u")), verdict="UNPROVEN")
    Tb("unproven_outgoing_office_in_handover_block", w, "an UNPROVEN record of the outgoing office in its hand-over block counts: unproven 1")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "k1", mk("k1", b_account("x", "u")), verdict="UNPROVEN")
    Tb("unproven_incoming_key_in_handover_block", w, "an UNPROVEN record of the incoming key in the hand-over block does not count: unproven 0")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "root", H32("unheld", "hand-over"))
    Tb("unread_outgoing_office_in_handover_block", w, "the outgoing office's unheld hash in its hand-over block is unread: unread 1")
    w = homed()
    h = H32("unheld", "twice")
    w.anchor(CA, 20, "root", h, verdict="UNPROVEN")
    w.anchor(CA, 21, "root", h, verdict="UNPROVEN")
    Tb("unproven_one_hash_twice", w, "unproven counts records: one hash in two UNPROVEN records of the office counts two, and neither is unread")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", pg.HISB(5), etype="history"), verdict="UNPROVEN")
    Tb("unproven_entry_of_another_type", w, "an UNPROVEN record of an entry of another type the pile holds counts: unproven 1")
    w = homed()
    w.anchor(CB, 50, "root", mk("root", b_home(CB)), verdict="UNPROVEN")
    w.anchor(CB, 51, "root", mk("root", b_account("x", "on-b")))
    Tb("unproven_move_of_the_home", w, "the record of a move is UNPROVEN: the home stays on chain 1, the binding on chain 10 is UNSEATED, unproven 1")

    # sec 7.3: a release anchored off the home, and a release on a home the seat moved to
    w = homed()
    b = mk("root", b_account("x", "kept"))
    w.anchor(CA, 11, "root", b)
    w.anchor(CB, 12, "root", mk("root", b_release(b)))
    Tb("release_off_home", w, "a release anchored off the home is UNSEATED and the row stays bound")
    w = homed()
    b = mk("root", b_account("x", "moved"))
    w.anchor(CA, 11, "root", b)
    w.anchor(CB, 12, "root", mk("root", b_home(CB)))
    w.anchor(CB, 13, "root", mk("root", b_release(b)))
    w.anchor(CA, 14, "root", mk("root", b_release(b, note="old home")))
    Tb("release_on_new_home", w, "after a move a release on the new home ends a binding made on the old one, and a release on the old home is UNSEATED")

    # sec 7.5: the least release ends the span wherever its moment falls against the binding's
    w = homed()
    b = mk("root", b_account("x", "early"))
    w.anchor(CA, 11, "root", mk("root", b_release(b, note="before")))
    w.anchor(CA, 12, "root", b)
    w.anchor(CA, 13, "root", mk("root", b_release(b, note="after")))
    Tb("release_before_and_after_binding", w, "one release before the binding and one after: the earlier ends the span, at the binding's moment")

    # sec 7.3: blockNumber before tx inside one second of one chain
    w = homed()
    w.ts[(CA, 12)] = T0 + 12
    w.ts[(CA, 13)] = T0 + 12
    b = mk("root", b_account("x", "alice"))
    w.anchor(CA, 12, "root", b, tx="0x" + "f" * 64)
    w.anchor(CA, 13, "root", b, tx="0x" + "0" * 63 + "1", carry=False)
    Tb("moment_block_before_tx", w, "one binding seated in blocks 12 and 13 of one second, the later block with the lesser tx: its moment is block 12")

    # sec 6.1: three member forms on their failing side alone
    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    w.accede(CA, 20, "root", "k1")
    base = w.value()
    for slug, change, note in (
            ("anchor_chain_string", lambda v: v["anchors"][1].update(chainId=str(CA)), "an anchor's chainId a string"),
            ("accession_chain_string", lambda v: v["accessions"][0].update(chainId=str(CA)), "an accession's chainId a string"),
            ("accession_block_string", lambda v: v["accessions"][0].update(blockNumber="20"), "an accession's blockNumber a string")):
        v = copy.deepcopy(base)
        change(v)
        assert p_table(v) == NO_TABLE, slug
        Tb("invalid_" + slug, v, note + ": NO_TABLE")


# ==========================================================================
# round 3, seat D3: the comma below the hyphen, the keys of sec 6.2 against
# members outside them, three hex forms and a chainId each the only fault of
# its input, releases naming seated entries that do not read, a home entry
# signed by a key yet to hold the seat, and a lone surrogate whose removal
# would leave a valid input: hand-built, seed-independent, after every
# earlier case
# ==========================================================================

def build_round3():
    import copy
    R, fa = A("root"), A("fa")

    # sec 1: every printable character as a one-character label, in `platform` and in `role`, so that
    # every character of U+0020 to U+007E sits on its side of the label alphabet; and the comma, the
    # lower neighbour of the hyphen, as the role argument of `declare`
    for c in range(0x20, 0x7F):
        ch = chr(c)
        ok = p_label(ch)
        E("platform_char_%02x" % c, mk("root", dict(b_account("x", "alice"), platform=ch)),
          "`platform` %r: %s" % (ch, "a label" if ok else "no label, no shape"))
        sig = sign_consent("fa", R, fa, ch if ok else "payout")
        E("role_char_%02x" % c, mk("root", dict(b_key("fa", "payout", sig=sig), role=ch)),
          "`role` %r: %s" % (ch, "a label, the consent names it" if ok else "no label, no shape"))
    Mu("declare_role_comma", ["declare", R, fa, ","], "a role that is no label")
    key = "%064x" % K("fa")
    Mu("declare_key_63_digits", ["declare", R, fa, "payout", key[1:]], "a private key of sixty-three digits")
    Mu("declare_key_65_digits", ["declare", R, fa, "payout", "0" + key], "a private key of sixty-five digits, a zero before the sixty-four")

    # sec 6.1 and 6.2 over a base input of its own, each change the input's only fault
    w = homed()
    w.anchor(CA, 11, "root", mk("root", b_account("x", "alice")))
    w.accede(CA, 20, "root", "k1")
    base = w.value()

    def var(slug, change, note, valid):
        v = copy.deepcopy(base)
        change(v)
        assert (p_table(v) != NO_TABLE) == valid, slug
        Tb(("valid_" if valid else "invalid_") + slug, v, note if valid else note + ": NO_TABLE")

    C2 = lambda v: v["chains"][2]
    A1 = lambda v: v["anchors"][1]
    X0 = lambda v: v["accessions"][0]
    var("chain_id_string_alone", lambda v: C2(v).update(chainId=str(CC)),
        "chainId a string on a chain no record names, its decimal value above the chain before it", False)
    var("anchor_twice_other_sender", lambda v: v["anchors"].append(dict(A1(v), sender=A("stranger"))),
        "two records agreeing on chain, block, tx and hash and differing in sender alone", False)
    var("accession_same_position_other_tx", lambda v: v["accessions"].append(dict(X0(v), tx=H32("tx-one-position"))),
        "two accession records at one position differing in tx alone", False)
    var("accession_same_position_other_holder", lambda v: v["accessions"].append(dict(X0(v), holder=A("k2"))),
        "two accession records at one position differing in holder alone", False)
    var("anchor_tx_upper", lambda v: A1(v).update(tx="0x" + A1(v)["tx"][2:].upper()), "an anchor's tx in uppercase", False)
    var("accession_tx_upper", lambda v: X0(v).update(tx="0x" + X0(v)["tx"][2:].upper()), "an accession's tx in uppercase", False)
    var("accession_holder_upper", lambda v: X0(v).update(holder="0x" + X0(v)["holder"][2:].upper()),
        "an accession's holder in uppercase", False)

    # sec 7.4 and 7.6: a release names a binding, and a seated entry that does not read is none
    w = homed()
    k = mk("root", b_key("fb", "payout", sig=sign_consent("fa", R, A("fb"), "payout")))
    o = mk("root", b_account("x", "alice", root=A("k1")))
    w.anchor(CA, 11, "root", k)
    w.anchor(CA, 12, "root", o)
    w.anchor(CA, 13, "root", mk("root", b_release(k)))
    w.anchor(CA, 14, "root", mk("root", b_release(o)))
    Tb("release_of_seated_entries_that_do_not_read", w,
       "releases naming a seated key entry whose consent fails and a seated account entry under another root: no row, SUBJECT for both releases")

    # sec 7.2: the author of an opening is the office as the step found it
    w = World()
    w.anchor(CA, 10, "root", mk("k1", b_home(CA)))
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "alice")))
    Tb("home_by_future_key", w,
       "the root anchors a home entry its successor signed, before the hand-over: no opening, both entries UNSEATED")

    # the transport: a lone surrogate whose removal would leave a valid input
    text = json.dumps(base, separators=(",", ":"))
    assert text.count('"pile"') == 1
    Tr("file_lone_surrogate_in_name", text.replace('"pile"', '"pi\\ud800le"').encode(),
       "a lone surrogate escape inside a member name: the reader refuses the file")


# ==========================================================================
# round 3, the law reader's witnesses (seat A3): hand-built, seed-independent,
# after every earlier case
# ==========================================================================

def build_round3_readers():
    # sec 7.2 item 3 and 7.3: the author is read against the office the step found, in the hand-over block
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "root", mk("k1", b_account("x", "incoming-signed")))
    Tb("author_incoming_key_in_handover_block", w, "in the hand-over block the outgoing office anchors a binding the incoming key signed: UNSEATED")
    w = World()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "root", mk("k1", b_home(CA)))
    w.anchor(CA, 30, "k1", mk("k1", b_account("x", "after")))
    Tb("opening_by_incoming_key_in_handover_block", w, "before a home the outgoing root anchors, in its hand-over block, a home entry the incoming key signed: no opening")

    # sec 7.3 and 9: a move back to an earlier home
    w = homed()
    w.anchor(CB, 20, "root", mk("root", b_home(CB)))
    w.anchor(CA, 30, "root", mk("root", b_home(CA, root=A("root"))))
    w.anchor(CA, 40, "root", mk("root", b_account("x", "back-home")))
    Tb("home_moves_back", w, "home on chain 1, moved to chain 10, moved back by a new home entry: the binding on chain 1 stands")
    w = World()
    h = mk("root", b_home(CA))
    w.anchor(CA, 10, "root", h)
    w.anchor(CB, 20, "root", mk("root", b_home(CB)))
    w.anchor(CA, 30, "root", h, carry=False)
    w.anchor(CA, 40, "root", mk("root", b_account("x", "back-home-same-entry")))
    Tb("home_moves_back_same_entry", w, "the first home entry anchored again on its chain moves the home back: the binding stands")

    # sec 7.5: two releases of one binding in one block, two transactions
    w = homed()
    b = mk("root", b_account("x", "two-releases-one-block"))
    w.anchor(CA, 20, "root", b)
    q1, q2 = mk("root", b_release(b)), mk("root", b_release(b, note="second"))
    lo, hi = sorted([q1, q2], key=eid)
    w.anchor(CA, 30, "root", hi, tx="0x" + "0" * 63 + "1")
    w.anchor(CA, 30, "root", lo, tx="0x" + "0" * 63 + "2")
    Tb("two_releases_one_block_tx_decides", w, "two releases of one binding in one block: the lesser tx, carrying the greater entry id, ends the span")

    # sec 7.8: two openings in one block, two transactions
    w = World()
    h1, h2 = mk("root", b_home(CA)), mk("root", b_home(CA))
    lo, hi = sorted([h1, h2], key=eid)
    w.anchor(CA, 10, "root", hi, tx="0x" + "0" * 63 + "1")
    w.anchor(CA, 10, "root", lo, tx="0x" + "0" * 63 + "2")
    Tb("two_openings_one_block_tx_decides", w, "two openings in one block by two transactions: homes lists the lesser tx first, its entry the greater")

    # sec 7.3: an author in reach that never joined the line
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.accede(CB, 21, "root", "thief")
    w.anchor(CA, 30, "k1", mk("thief", b_account("x", "fork-signed")))
    Tb("author_in_reach_outside_line", w, "the office anchors on the home an entry a key in reach signed that never joined the line: AUTHOR")


# ==========================================================================
# sec 7.2 item 3 and 7.3, the author is the office: hand-built, seed-independent,
# after every earlier case
# ==========================================================================

def build_author_is_office():
    # sec 7.3: the key in office seats what it signed itself
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "by-k1-in-office")))
    Tb("author_office_after_handover", w, "the successor in office anchors a binding it signed itself: bound")

    # sec 7.3: what a retired key signed, the office seats in no shape
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("root", b_account("x", "retired-signed")))
    Tb("author_retired_office_anchors_binding", w, "the office anchors a binding the retired root signed: no row, UNSEATED")
    w = homed()
    b = mk("root", b_account("x", "real"))
    w.anchor(CA, 12, "root", b)
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("root", b_release(b)))
    Tb("author_retired_office_anchors_release", w, "the office anchors a release the retired root signed: the binding stays bound, the release UNSEATED")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("root", b_key("fa", "payout")))
    Tb("author_retired_office_anchors_key_face", w, "the office anchors a key face the retired root signed, its consent good: no row, UNSEATED")

    # sec 7.2 item 3: a home entry opens when the key in office signed it
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.anchor(CA, 10, "k1", mk("k1", b_home(CA)))
    w.anchor(CA, 11, "k1", mk("k1", b_account("x", "alice")))
    Tb("home_by_office_after_handover", w, "a home entry the successor in office signed and anchors opens the home")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CB, 30, "k1", mk("root", b_home(CB)))
    w.anchor(CA, 40, "k1", mk("k1", b_account("x", "still-home-ca")))
    Tb("home_by_retired_key_moves_nothing", w, "the office anchors on chain 10 a home entry the retired root signed: the home stays on chain 1")
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.anchor(CA, 10, "k1", mk("root", b_home(CA)))
    w.anchor(CA, 11, "k1", mk("k1", b_home(CA)))
    w.anchor(CA, 12, "k1", mk("k1", b_account("x", "after")))
    Tb("home_by_retired_key_before_home", w, "before a home the office anchors a home entry the retired root signed, then one of its own: the second alone opens")

    # sec 7.3: seated is said of a record at its step
    w = homed()
    e = mk("root", b_account("x", "twice"))
    w.anchor(CA, 12, "root", e)
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", e, carry=False)
    Tb("seated_by_author_then_anchored_by_successor", w, "a binding its author anchored in office, anchored again by the successor: one span, since the first record")
    w = homed()
    b = mk("root", b_account("x", "old-binding"))
    w.anchor(CA, 12, "root", b)
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("k1", b_release(b)))
    Tb("release_by_office_of_predecessors_binding", w, "the successor in office releases a binding its predecessor made: released")

    # sec 7.2 item 1 and 7.3: the hand-over block, the outgoing key's own entry and the incoming key's own record
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "root", mk("root", b_account("x", "outgoing-in-block")))
    Tb("author_outgoing_key_in_handover_block", w, "in its hand-over block the outgoing office anchors a binding it signed: bound")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "k1", mk("k1", b_account("x", "incoming-own")))
    Tb("author_incoming_key_own_record_in_handover_block", w, "in the hand-over block the incoming key anchors a binding it signed: not yet in office, UNSEATED")

    # sec 7.3: three keys, the office seats its own entry alone
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.accede(CA, 30, "k1", "k2")
    w.anchor(CA, 40, "k2", mk("root", b_account("x", "by-root")))
    w.anchor(CA, 41, "k2", mk("k1", b_account("x", "by-k1")))
    w.anchor(CA, 42, "k2", mk("k2", b_account("x", "by-k2")))
    Tb("author_of_three_keys", w, "the third key in office anchors bindings the first, the second and itself signed: its own alone is bound")

    # sec 7.2 item 3 and 9: the first home entry anchored again by a successor moves nothing back
    w = World()
    h = mk("root", b_home(CA))
    w.anchor(CA, 10, "root", h)
    w.anchor(CB, 20, "root", mk("root", b_home(CB)))
    w.accede(CB, 25, "root", "k1")
    w.anchor(CA, 30, "k1", h, carry=False)
    w.anchor(CB, 40, "k1", mk("k1", b_account("x", "stays-on-chain-10")))
    Tb("home_entry_anchored_again_by_successor", w, "the root's first home entry anchored again on chain 1 by its successor: the home stays on chain 10")


# ==========================================================================
# sec 6.1 and 7.2 item 2, the accession record names its root: hand-built,
# seed-independent, after every earlier case
# ==========================================================================

def build_book_root():
    import copy
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "alice")))
    base = w.value()

    def var(slug, change, note, valid):
        v = copy.deepcopy(base)
        change(v)
        assert (p_table(v) != NO_TABLE) == valid, slug
        Tb(("valid_" if valid else "invalid_") + slug, v, note if valid else note + ": NO_TABLE")

    X0 = lambda v: v["accessions"][0]
    var("accession_without_root", lambda v: X0(v).pop("root"), "an accession record without `root`", False)
    var("accession_root_upper", lambda v: X0(v).update(root="0x" + X0(v)["root"][2:].upper()),
        "an accession's root in uppercase", False)
    var("accession_root_hex32", lambda v: X0(v).update(root=H32("root-as-hex32")), "an accession's root a hex32", False)
    var("accession_root_null", lambda v: X0(v).update(root=None), "an accession's root null", False)
    var("accession_same_position_other_root", lambda v: v["accessions"].append(dict(X0(v), root=A("k2"))),
        "two accession records at one position differing in root alone", False)

    # sec 7.2 item 2: a record moves the office under the root R alone
    w = homed()
    w.accede(CA, 20, "root", "k1", root="stranger")
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "k1-under-another-root")))
    w.anchor(CA, 22, "root", mk("root", b_account("x", "root-still")))
    Tb("accession_other_root_moves_nothing", w, "a hand-over whose holder is the office and whose root is another address moves nothing")
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 20, "k1", "k2", root="k1")
    w.anchor(CA, 25, "k1", mk("k1", b_home(CA)))
    w.anchor(CA, 30, "k2", mk("k2", b_account("x", "k2-unseated")))
    w.anchor(CA, 31, "k1", mk("k1", b_account("x", "k1-still-office")))
    Tb("accession_in_book_not_entered", w, "before a home the office hands over, in a book that never held the seat, under its own address as root: nothing moves")

    # a seat entering a second book under its root, after the fact and as its first record
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CA, 6, "k1", "k2")
    w.accede(CB, 9, "root", "k2")
    w.anchor(CB, 10, "k2", mk("k2", b_home(CB)))
    w.accede(CB, 20, "k2", "k3")
    w.anchor(CB, 30, "k3", mk("k3", b_account("x", "k3-after-enrolment")))
    Tb("accession_after_enrolment", w, "the root hands the key in office its record in a second book, which moves nothing; the next hand-over there, under R, moves the office")
    w = World()
    w.accede(CA, 5, "root", "k2")
    w.anchor(CA, 10, "k2", mk("k2", b_home(CA)))
    w.anchor(CA, 11, "k2", mk("k2", b_account("x", "enrolled-first")))
    Tb("accession_enrolment_first_record", w, "the root's hand-over to a later key as the first record of any book moves the office")

    # the retired root hands on under R in a book the seat never entered
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 6, "root", "thief")
    w.accede(CB, 7, "thief", "stranger")
    w.anchor(CB, 8, "stranger", mk("stranger", b_home(CB)))
    w.anchor(CA, 10, "k1", mk("k1", b_home(CA)))
    w.anchor(CA, 11, "k1", mk("k1", b_account("x", "seat-keeps")))
    Tb("retired_root_hands_on_under_root", w, "the retired root hands on under R in a book the seat never entered: no holder there is the office, nothing moves and nothing is listed")


# ==========================================================================
# sec 6.1 and 6.2, an anchor record as zikaron/2 holds it: hand-built,
# seed-independent, after every earlier case
# ==========================================================================

def build_anchor_records():
    import copy
    w = homed()
    w.anchor(CA, 12, "root", H32("unproven-twice"), verdict="UNPROVEN")
    v = w.value()
    v["anchors"].append(copy.deepcopy(v["anchors"][1]))
    Tb("valid_anchor_twice_unproven_once", v, "an UNPROVEN record of the office given twice is one record: unproven 1")
    w = World()
    w.anchor(CA, 10, "root", mk("root", b_home(CA)))
    v = w.value()
    v["anchors"].append(copy.deepcopy(v["anchors"][0]))
    Tb("valid_anchor_twice_opening_once", v, "an opening record given twice is one record: homes lists it once")
    w = homed()
    w.anchor(CA, 12, "root", H32("unproven-extra"), verdict="UNPROVEN")
    v = w.value()
    v["anchors"].append(dict(v["anchors"][1], logIndex=1))
    v["anchors"][1]["logIndex"] = 0
    Tb("valid_anchor_twice_extra_member_differs", v, "two records that agree on the seven and differ in a member beyond them are one record: unproven 1")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "extra-members")))
    v = w.value()
    v["anchors"][1].update(logIndex=3, registry=REG(CA), note="read by nothing")
    Tb("valid_anchor_three_extra_members", v, "an anchor record carrying three members beyond its seven seats its entry all the same")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "bad-under-extra")))
    v = w.value()
    v["anchors"][1].update(logIndex=3)
    v["anchors"][1].pop("verdict")
    Tb("invalid_anchor_extra_member_without_verdict", v, "a member beyond the seven stands for none of them: NO_TABLE")


# ==========================================================================
# sec 6.2, the anchor record given twice and the members beyond its seven
# (the check seat's two witnesses): hand-built, seed-independent, after
# every earlier case
# ==========================================================================

def build_anchor_records_checked():
    import copy
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "alice")))
    base = w.value()
    v = copy.deepcopy(base)
    v["anchors"].append(dict(v["anchors"][1], blockTimestamp=v["anchors"][1]["blockTimestamp"] + 1))
    assert p_table(v) == NO_TABLE
    Tb("invalid_anchor_twice_other_time", v, "two records agreeing on chain, block, tx and hash and differing in blockTimestamp alone: NO_TABLE")
    v = copy.deepcopy(base)
    v["anchors"][1].update(a_null=None, a_true=True, a_false=False, an_int=7, a_string="s", an_array=[1, "two", None],
                           an_object={"deep": {"deeper": [True]}}, prose_md="任意 text")
    Tb("valid_anchor_extra_members_of_every_kind", v, "an anchor record whose members beyond the seven hold every kind of value: read as without them")


# ==========================================================================
# sec 7.2, the head of a chain's book and the opening's test of it; the
# opening block's accessions under the new home: hand-built,
# seed-independent, after every earlier case
# ==========================================================================

def build_home_book():
    # an opening needs the chain's book to hold the office under R
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.anchor(CB, 10, "k1", mk("k1", b_home(CB)))
    w.anchor(CA, 11, "k1", mk("k1", b_home(CA)))
    w.anchor(CA, 12, "k1", mk("k1", b_account("x", "home-where-the-book-holds-it")))
    Tb("opening_needs_the_books_head", w, "the office opens on a chain whose book still holds the root, then on the chain whose book holds it: the second alone opens")
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 8, "root", "k1")
    w.anchor(CB, 10, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 11, "k1", mk("k1", b_account("x", "entered-then-opened")))
    Tb("opening_after_entering_the_book", w, "the root hands the key in office its record in a second book, which moves no office and sets that book's head: the opening there counts")
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 10, "root", "k1")
    w.anchor(CB, 10, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 11, "k1", mk("k1", b_account("x", "one-block-early")))
    Tb("opening_in_the_block_of_entering", w, "an opening in the block of the record that enters the book: the head is read as the step found it, no opening")
    w = World(chains=((CA, True), (CB, False), (CC, True)))
    w.accede(CA, 5, "root", "k1")
    w.anchor(CB, 10, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 11, "k1", mk("k1", b_account("x", "no-book-no-successor")))
    Tb("opening_without_book_by_successor", w, "a chain whose basis names no book holds the root alone: a successor opens nothing there")

    # the book of the chain names another key: the holder of the outgoing key cannot carry the home there
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "seat-keeps")))
    w.accede(CB, 20, "root", "k1")
    w.anchor(CB, 21, "root", mk("root", b_home(CB)))
    w.anchor(CB, 22, "root", mk("root", b_account("x", "outgoing-on-the-other-chain")))
    w.accede(CA, 23, "root", "k1")
    w.anchor(CA, 24, "k1", mk("k1", b_account("x", "incoming-at-home")))
    Tb("opening_where_the_book_moved_on", w, "the incoming key accedes on another chain first; the outgoing key, still in office at home, opens there: that book's head is the incoming key, no opening, and the hand-over at home then moves the office")
    w = World()
    w.accede(CA, 100, "root", "k1")
    w.accede(CB, 200, "root", "k2")
    w.anchor(CB, 400, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 401, "k1", mk("k1", b_account("x", "alice")))
    Tb("opening_where_the_book_names_another", w, "the root hands over on one chain, then again on another: the office opens nothing on the chain whose book names the other key")

    # the head follows every record under R, whoever its holder, and no record under another root
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 6, "thief", "k1")
    w.anchor(CB, 10, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 11, "k1", mk("k1", b_account("x", "head-by-any-holder")))
    Tb("head_follows_any_holder_under_root", w, "a record under R whose holder is no office sets the head of its chain's book: the opening there counts")
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 6, "root", "k1", root="k1")
    w.anchor(CB, 10, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 11, "k1", mk("k1", b_account("x", "head-under-another-root")))
    Tb("head_ignores_another_root", w, "a record under another root sets no head: the book still holds the root, no opening")

    # the opening block's accessions are read under the new home
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.accede(CB, 16, "root", "k1")
    w.anchor(CB, 50, "k1", mk("k1", b_home(CB)))
    w.accede(CB, 50, "k1", "k2")
    w.anchor(CB, 51, "k2", mk("k2", b_account("x", "k2-after-the-opening-block")))
    w.anchor(CB, 52, "k1", mk("k1", b_account("x", "k1-after-the-opening-block")))
    Tb("opening_block_accession_under_new_home", w, "a move and a hand-over on the new chain in one block: the hand-over is read under the new home and moves the office")
    w = homed(chain=CB)
    w.anchor(CA, 50, "root", mk("root", b_home(CA)))
    w.accede(CB, 50, "root", "k1")
    w.anchor(CA, 60, "root", mk("root", b_account("x", "root-on-the-new-home")))
    Tb("old_home_accession_in_the_second_of_the_move", w, "a hand-over on the old home in the second of the move, read after the opening by chain id: the old home's book moves nothing")

    # a move back needs the old home's book in step
    w = homed()
    w.anchor(CB, 20, "root", mk("root", b_home(CB)))
    w.accede(CB, 30, "root", "k1")
    w.anchor(CA, 40, "k1", mk("k1", b_home(CA)))
    w.accede(CA, 45, "root", "k1")
    w.anchor(CA, 50, "k1", mk("k1", b_home(CA)))
    w.anchor(CA, 51, "k1", mk("k1", b_account("x", "back-once-the-book-is-in-step")))
    Tb("move_back_needs_the_book_in_step", w, "after a hand-over on the new home, the move back counts only once the old home's book holds the office: two openings before it, the third after")


# ==========================================================================
# sec 1, 2 and 7, the persona: hand-built, seed-independent, after every
# earlier case
# ==========================================================================

def b_persona(name, root=None):
    return {"act": "bind", "root": A("root") if root is None else root, "kind": "persona", "name_md": name}


# the rows the table gained after the cases of `build_persona` were numbered, and barred values inside ranges
BARRED_LATER = [(0x13441, 0x13442), (0x16FE4, 0x16FE4), (0x1BC9D, 0x1BC9D), (0x1D159, 0x1D159)]
BARRED_INSIDE = [0x2065, 0xFFFC, 0xFFFD, 0xE0100]


def name_edges(rows=None):
    """For each barred range: its two ends, and the free scalar value next to each (None where none)."""
    def free(o, step):
        o += step
        while 0 <= o <= 0x10FFFF and (0xD800 <= o <= 0xDFFF or p_barred(o)):
            o += step
        return o if 0 <= o <= 0x10FFFF else None
    return [(lo, hi, free(lo, -1), free(hi, 1)) for lo, hi in (BARRED if rows is None else rows)]


PLANE_END = ([0x1FFFE, 0x1FFFF, 0xEFFFE], [0x1FFFD, 0x20000])


def build_persona():
    import copy
    E("persona", mk("root", b_persona("Akhuzan")), "the persona shape")
    E("persona_cjk", mk("root", b_persona("圣山契约行会")), "a name outside ASCII: the member is a prose string")
    E("persona_one", mk("root", b_persona("x")), "a name of one scalar value")
    E("persona_len32", mk("root", b_persona("x" * 32)), "a name of thirty-two scalar values")
    E("persona_len32_cjk", mk("root", b_persona("名" * 32)), "thirty-two scalar values of three bytes each")
    E("persona_len32_astral", mk("root", b_persona("\U0001F600" * 32)), "thirty-two scalar values of four bytes each")
    E("persona_combining", mk("root", b_persona("é")), "a letter and a combining mark: two scalar values, normalized by nobody")
    E("persona_punctuation", mk("root", b_persona("O’Neil·K-9")), "punctuation inside a name")
    E("persona_quote_and_backslash", mk("root", b_persona('a"b\\c')), "the two characters the canonical spelling escapes each count one")
    E("persona_empty", mk("root", b_persona("")), "an empty name: no shape")
    E("persona_len33", mk("root", b_persona("x" * 33)), "thirty-three scalar values: no shape")
    E("persona_len33_cjk", mk("root", b_persona("名" * 33)), "thirty-three scalar values of three bytes each: no shape")
    E("persona_len33_astral", mk("root", b_persona("\U0001F600" * 33)), "thirty-three scalar values of four bytes each: no shape")
    E("persona_space_inside", mk("root", b_persona("a b")), "a space inside a name: no shape")
    E("persona_space_first", mk("root", b_persona(" ab")), "a space first: no shape")
    E("persona_space_last", mk("root", b_persona("ab ")), "a space last: no shape")
    E("persona_two_lines", mk("root", b_persona("a\nb")), "a line feed inside a name: no shape")
    E("persona_with_note", mk("root", dict(b_persona("Akhuzan"), note_md="n")), "the persona row carries no note: a member beyond the row")
    E("persona_extra_member", mk("root", dict(b_persona("Akhuzan"), extra=1)), "the persona row with a member beyond it: no shape")
    for k in ("act", "kind", "name_md", "root"):
        b = b_persona("Akhuzan")
        del b[k]
        E("persona_without_" + k, mk("root", b), "the persona row without `%s`: no shape" % k)
    for tag, val in (("int", 5), ("null", None), ("array", ["Akhuzan"]), ("object", {"a": "Akhuzan"}), ("bool", True)):
        E("persona_name_" + tag, mk("root", dict(b_persona("x"), name_md=val)), "`name_md` a %s: no shape" % tag)
    E("persona_kind_cased", mk("root", dict(b_persona("Akhuzan"), kind="Persona")), "`kind` in another case: no shape")
    E("persona_act_release", mk("root", dict(b_persona("Akhuzan"), act="release")), "the persona members under `act` release: no shape")
    E("persona_with_handle", mk("root", dict(b_account("persona", "Akhuzan"), kind="persona")), "the account members under `kind` persona: no shape")
    b = b_persona("Akhuzan")
    b["name"] = b.pop("name_md")
    E("persona_name_plain_member", mk("root", b), "the name under `name`: no shape")
    b = b_persona("圣山")
    b["name"] = b.pop("name_md")
    E("persona_name_plain_member_cjk", mk("root", b), "a name outside ASCII under `name`: no entry (zikaron/2 sec 3.3)")
    E("unrooted_persona", mk("root", b_persona("Akhuzan", root=A("k1"))), "the persona shape under another root: rooted false")

    made = {}

    def edge(o, barred):
        slug = "persona_%s_%04x" % ("barred" if barred else "free", o)
        if slug not in made:
            made[slug] = 1
            E(slug, mk("root", b_persona("a" + chr(o) + "b")),
              "U+%04X inside a name: %s" % (o, "barred, no shape" if barred else "free"))

    for lo, hi, below, above in name_edges([r for r in BARRED if r not in BARRED_LATER]):
        edge(lo, True)
        edge(hi, True)
        for o in (below, above):
            if o is not None:
                edge(o, False)
    for o in PLANE_END[0]:
        edge(o, True)
    for o in PLANE_END[1]:
        edge(o, False)

    # sec 7.4, 7.5: a persona entry stands like any binding, and the open span last in `since` gives the persona
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("圣山契约行会")))
    Tb("persona_alone", w, "one persona binding: its name is the persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("first")))
    w.anchor(CA, 13, "root", mk("root", b_persona("second")))
    Tb("persona_latest_of_two", w, "two persona rows bound: the later binding gives the persona")
    w = homed()
    p1, p2 = mk("root", b_persona("first")), mk("root", b_persona("second"))
    w.anchor(CA, 12, "root", p1)
    w.anchor(CA, 13, "root", p2)
    w.anchor(CA, 14, "root", mk("root", b_release(p2)))
    Tb("persona_falls_back_on_release", w, "the later of two bindings released: the persona falls back to the earlier")
    w = homed()
    p1, p2 = mk("root", b_persona("first")), mk("root", b_persona("second"))
    w.anchor(CA, 12, "root", p1)
    w.anchor(CA, 13, "root", p2)
    w.anchor(CA, 14, "root", mk("root", b_release(p1)))
    Tb("persona_release_of_the_earlier", w, "the earlier of two bindings released: the persona stays the later")
    w = homed()
    p1, p2 = mk("root", b_persona("first")), mk("root", b_persona("second"))
    w.anchor(CA, 12, "root", p1)
    w.anchor(CA, 13, "root", p2)
    w.anchor(CA, 14, "root", mk("root", b_release(p1)))
    w.anchor(CA, 15, "root", mk("root", b_release(p2)))
    Tb("persona_all_released", w, "every persona binding released: no persona, both rows released")
    w = homed()
    p1, p2 = mk("root", b_persona("alpha")), mk("root", b_persona("beta"))
    w.anchor(CA, 12, "root", p1)
    w.anchor(CA, 13, "root", p2)
    w.anchor(CA, 14, "root", mk("root", b_release(p1)))
    w.anchor(CA, 15, "root", mk("root", b_persona("alpha")))
    Tb("persona_bound_again", w, "a name released and bound again by a new entry: the new span's beginning counts, and the name is the persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("alpha")))
    w.anchor(CA, 15, "root", mk("root", b_persona("beta")))
    w.anchor(CA, 20, "root", mk("root", b_persona("alpha")))
    Tb("persona_row_of_two_open_spans", w, "a row with two open spans, the later after another row's: that row's later span gives the persona")
    w = homed()
    a2 = mk("root", b_persona("alpha"))
    w.anchor(CA, 12, "root", mk("root", b_persona("alpha")))
    w.anchor(CA, 15, "root", mk("root", b_persona("beta")))
    w.anchor(CA, 20, "root", a2)
    w.anchor(CA, 21, "root", mk("root", b_release(a2)))
    Tb("persona_row_later_span_closed", w, "a row whose later span is closed and whose open span is the earliest: the other row gives the persona")
    w = homed()
    w.ts[(CA, 30)] = T0 + 30
    w.ts[(CB, 40)] = T0 + 30
    w.ts[(CB, 41)] = T0 + 30
    w.anchor(CA, 30, "root", mk("root", b_persona("on-chain-1")))
    w.anchor(CB, 40, "root", mk("root", b_home(CB)))
    w.anchor(CB, 41, "root", mk("root", b_persona("on-chain-10")))
    Tb("persona_one_second_two_chains", w, "two persona bindings at one block time on two chains, the home moving between them: the greater chain id is the later")
    w = homed()
    e1, e2 = mk("root", b_persona("one")), mk("root", b_persona("two"))
    tx = "0x" + "0" * 63 + "7"
    w.anchor(CA, 12, "root", e1, tx=tx)
    w.anchor(CA, 12, "root", e2, tx=tx)
    Tb("persona_one_transaction_two_entries", w, "two persona bindings seated by one transaction: the moments are equal, the greater entry gives the persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("old-name")))
    w.accede(CA, 20, "root", "k1")
    k = mk("k1", b_persona("new-name"))
    w.anchor(CA, 21, "k1", k)
    w.anchor(CA, 22, "k1", mk("k1", b_release(k)))
    Tb("persona_across_a_handover", w, "the predecessor's persona stands after the hand-over; the successor binds another and releases it: the persona falls back to the predecessor's")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("root", b_persona("signed-by-the-retired")))
    Tb("persona_by_retired_key", w, "the office anchors a persona entry the retired root signed: UNSEATED, no persona")
    w = homed()
    w.anchor(CB, 12, "root", mk("root", b_persona("off-the-home")))
    Tb("persona_off_the_home", w, "a persona entry anchored off the home: UNSEATED, no persona")
    w = World()
    w.anchor(CA, 12, "root", mk("root", b_persona("before-a-home")))
    Tb("persona_before_a_home", w, "a persona entry before any home: UNSEATED, no persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("another-root", root=A("k1"))))
    Tb("persona_under_another_root", w, "a persona entry naming another root: ROOT, no persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("void")), verdict="VOID")
    Tb("persona_void_record", w, "a persona entry whose only record is VOID: UNSEATED, no persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("a b")))
    Tb("persona_name_with_a_space", w, "a persona entry whose name holds a space has no shape: SHAPE, no persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "alice")))
    w.anchor(CA, 13, "root", mk("root", b_key("fa", "payout")))
    for i, n in enumerate(["b", "\U0001F600", "中", "a", "é"]):
        w.anchor(CA, 14 + i, "root", mk("root", b_persona(n)))
    Tb("persona_rows_in_order", w, "account rows, key rows, then persona rows bytewise by the UTF-8 of their names")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("persona", "Akhuzan")))
    Tb("account_on_a_platform_called_persona", w, "an account face whose platform label is `persona` is an account row and gives no persona")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona('q"\\')))
    Tb("persona_spelled_in_the_table", w, "a name holding a quotation mark and a backslash, spelled in the table as the canonical form spells them")


# ==========================================================================
# the corpus reader's cases of the second fold: hand-built, seed-independent,
# after every earlier case
# ==========================================================================

# the last two scalar values of every plane from 2 to 14, and the free one below each pair from 2 to 12
# (planes 13 and 14 have theirs as the free neighbours of U+E0000 and U+F0000)
PLANE_END_ALL = ([p * 0x10000 + d for p in range(2, 15) for d in (0xFFFE, 0xFFFF) if p * 0x10000 + d != 0xEFFFE],
                 [p * 0x10000 + 0xFFFD for p in range(2, 13)])


def build_corpus_reader():
    for o in PLANE_END_ALL[0]:
        E("persona_barred_%04x" % o, mk("root", b_persona("a" + chr(o) + "b")), "U+%04X inside a name: the last two of a plane, barred, no shape" % o)
    for o in PLANE_END_ALL[1]:
        E("persona_free_%04x" % o, mk("root", b_persona("a" + chr(o) + "b")), "U+%04X inside a name: free" % o)
    E("persona_len32_combining", mk("root", b_persona("é" * 16)), "thirty-two scalar values, sixteen of them combining marks: a name")
    E("persona_len33_combining", mk("root", b_persona("é" * 16 + "e")), "thirty-three scalar values that normalize to seventeen and show as seventeen: no shape, nothing is normalized or grouped")
    E("persona_len32_mixed_widths", mk("root", b_persona("x" + "名" * 10 + "\U0001F600" * 10 + "é" * 5 + "z")), "thirty-two scalar values of one, two, three and four bytes: a name")
    E("persona_len33_mixed_widths", mk("root", b_persona("x" + "名" * 10 + "\U0001F600" * 10 + "é" * 5 + "zz")), "thirty-three scalar values of one, two, three and four bytes: no shape")

    # rows bytewise by UTF-8, which is no UTF-16 order: U+FF01 sorts before U+1F600
    w = homed()
    for i, n in enumerate(["\U0001F600", "！", "豈", "\U00010000"]):
        w.anchor(CA, 12 + i, "root", mk("root", b_persona(n)))
    Tb("persona_rows_utf8_not_utf16", w, "persona rows of U+F900, U+FF01, U+10000 and U+1F600: bytewise UTF-8 order, which UTF-16 code units would break")
    # names compare bytewise and nothing normalizes: two spellings of one letter, two cases, are four rows
    w = homed()
    for i, n in enumerate(["é", "é", "Alice", "alice"]):
        w.anchor(CA, 12 + i, "root", mk("root", b_persona(n)))
    Tb("persona_rows_not_normalized", w, "U+00E9, e with U+0301, Alice and alice: four rows, the persona the last bound")
    # the persona by tx inside one block: the tx decides before the entry
    w = homed()
    e1, e2 = mk("root", b_persona("by-tx-one")), mk("root", b_persona("by-tx-two"))
    lo, hi = sorted([e1, e2], key=eid)
    w.anchor(CA, 12, "root", lo, tx="0x" + "0" * 63 + "2")
    w.anchor(CA, 12, "root", hi, tx="0x" + "0" * 63 + "1")
    Tb("persona_one_block_tx_decides", w, "two persona bindings in one block, the lesser entry in the greater tx: the greater tx rules")
    # the persona by block inside one second on one chain
    w = homed()
    w.ts[(CA, 30)] = T0 + 30
    w.ts[(CA, 31)] = T0 + 30
    e1, e2 = mk("root", b_persona("by-block-one")), mk("root", b_persona("by-block-two"))
    lo, hi = sorted([e1, e2], key=eid)
    w.anchor(CA, 30, "root", hi, tx="0x" + "0" * 63 + "9")
    w.anchor(CA, 31, "root", lo, tx="0x" + "0" * 63 + "1")
    Tb("persona_one_second_block_decides", w, "two persona bindings at one second on one chain, the later block holding the lesser entry and the lesser tx: the later block rules")
    # the persona by chain before block inside one second
    w = homed()
    for c_, b_ in ((CA, 50), (CB, 39), (CB, 40)):
        w.ts[(c_, b_)] = T0 + 50
    w.anchor(CA, 50, "root", mk("root", b_persona("on-chain-1-block-50")))
    w.anchor(CB, 39, "root", mk("root", b_home(CB)))
    w.anchor(CB, 40, "root", mk("root", b_persona("on-chain-10-block-40")))
    Tb("persona_one_second_chain_before_block", w, "two persona bindings at one second, the greater chain holding the lesser block: the greater chain rules")
    # the one record of an input carries its blockNumber as a string: no other test of the input can refuse it
    w = World()
    w.anchor(CA, 11, "root", mk("root", b_home(CA)))
    v = w.value()
    v["anchors"][0]["blockNumber"] = "11"
    Tb("invalid_anchor_block_string_alone", v, "the input's one record carries blockNumber as a string: no face input")


# ==========================================================================
# the second revision round's remaining witnesses (seats A and U), and the
# four rows the barred table gained: hand-built, seed-independent, after
# every earlier case
# ==========================================================================

def build_second_round():
    have = set(c["path"].split("/", 1)[1].split("_", 1)[1].rsplit(".", 1)[0] for c in MANIFEST if c["category"] == "entries")

    def edge(o, barred, why=None):
        slug = "persona_%s_%04x" % ("barred" if barred else "free", o)
        if slug not in have:
            have.add(slug)
            E(slug, mk("root", b_persona("a" + chr(o) + "b")),
              "U+%04X inside a name: %s" % (o, why or ("barred, no shape" if barred else "free")))

    for lo, hi, below, above in name_edges(BARRED_LATER):
        edge(lo, True)
        edge(hi, True)
        for o in (below, above):
            if o is not None:
                edge(o, False)
    # barred values inside a range, away from its ends
    for o in BARRED_INSIDE:
        edge(o, True, "barred inside its range, no shape")
    E("persona_len32_regional_indicators", mk("root", b_persona("\U0001F1E6" * 32)), "thirty-two regional indicators, sixteen flags to the eye: a name")
    E("persona_len33_regional_indicators", mk("root", b_persona("\U0001F1E6" * 33)), "thirty-three regional indicators: no shape")

    # sec 7.2: a record under another root leaves the head of its chain as it stood
    w = World()
    w.accede(CA, 5, "root", "k1")
    w.accede(CB, 6, "root", "k1")
    w.accede(CB, 7, "k3", "k2", root="k3")
    w.anchor(CB, 10, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 11, "k1", mk("k1", b_account("x", "head-kept")))
    Tb("head_kept_through_another_roots_record", w, "a record under another root, after the book came to hold the office: the head stands, the opening counts")
    # sec 7.2 item 2: an opening on the home that stands is tested by the head like any other
    w = homed()
    w.accede(CA, 20, "thief", "k2")
    w.anchor(CA, 30, "root", mk("root", b_home(CA)))
    w.anchor(CA, 31, "root", mk("root", b_account("x", "still-home")))
    Tb("opening_again_on_the_home_needs_the_head", w, "the head of the home's book set to another key by a record no office made; the office anchors a second home entry there: one opening, not two")
    # sec 7.8: persona rows bytewise, no case folded
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("alpha")))
    w.anchor(CA, 13, "root", mk("root", b_persona("Zed")))
    Tb("persona_rows_bytewise_not_caseless", w, "persona rows Zed and alpha: bytewise, the capital first")


# ==========================================================================
# the third revision round's witnesses of the name (the check seat):
# hand-built, seed-independent, after every earlier case
# ==========================================================================

def build_third_round():
    E("persona_len32_quote", mk("root", b_persona("a" * 31 + '"')), "thirty-two scalar values, the last a quotation mark its spelling escapes: a name")
    E("persona_len33_quote", mk("root", b_persona("a" * 32 + '"')), "thirty-three scalar values, the last a quotation mark: no shape")
    E("persona_len32_backslashes", mk("root", b_persona("\\" * 32)), "thirty-two backslashes, sixty-four bytes of canonical spelling: a name")
    E("persona_len33_backslashes", mk("root", b_persona("\\" * 33)), "thirty-three backslashes: no shape")
    E("persona_leading_combining", mk("root", b_persona("\u0301a")), "a name that begins with a combining mark: a name, marks being free wherever they stand")


# ==========================================================================
# the reroot and origin shapes, the seal, the two members of the table that
# name them, and joined: hand-built and seed-independent but for the random
# worlds at the end, which draw from a stream of their own; after every
# earlier case
# ==========================================================================

TXLO, TXMID, TXHI = "0x" + "11" * 32, "0x" + "88" * 32, "0x" + "ee" * 32


def b_reroot(to, root=None, note=None):
    b = {"act": "reroot", "root": A("root") if root is None else A(root), "to": A(to)}
    if note is not None:
        b["note_md"] = note
    return b


def b_origin(frm, root=None, note=None):
    b = {"act": "origin", "root": A("root") if root is None else A(root), "from": A(frm)}
    if note is not None:
        b["note_md"] = note
    return b


def seat(tag, chain=CA, block=10):
    """A world under the root `tag`, its home opened on `chain` at `block`."""
    w = World(root=tag)
    w.anchor(chain, block, tag, mk(tag, b_home(chain, root=A(tag))))
    return w


def build_reroot_entries():
    R, R2 = A("root"), A("root2")
    for name, other, builder in (("reroot", "to", b_reroot), ("origin", "from", b_origin)):
        wrong = "from" if other == "to" else "to"
        base = builder("root2")
        E(name + "_plain", mk("root", base), "the %s shape, its three members" % name)
        E(name + "_with_note", mk("root", builder("root2", note="moving")), "the %s shape with a note" % name)
        E(name + "_note_empty", mk("root", builder("root2", note="")), "the %s shape, its note the empty string" % name)
        E(name + "_by_another_key", mk("thief", base), "a %s body an outsider signed: the shape reads no author" % name)
        E(name + "_other_root", mk("root", builder("root", root="root2")),
          "a %s body under another root, naming the root read under: a shape, not rooted" % name)
        E(name + "_%s_is_root" % other, mk("root", dict(base, **{other: R})),
          "`%s` byte-equal to `root`: no shape" % other)
        E(name + "_extra_member", mk("root", dict(base, extra=1)), "a member beyond the row: no shape")
        E(name + "_with_sig", mk("root", dict(base, sig=sign_consent("root2", R, R2, "payout"))),
          "a `sig` member, which the row does not have: no shape")
        E(name + "_with_kind", mk("root", dict(base, kind=name)), "a `kind` member, which the row does not have: no shape")
        b = dict(base)
        del b[other]
        E(name + "_missing_" + other, mk("root", b), "`%s` absent: no shape" % other)
        E(name + "_with_" + wrong, mk("root", dict(b, **{wrong: R2})),
          "`%s` in the place of `%s`: no shape" % (wrong, other))
        E(name + "_both_to_and_from", mk("root", dict(base, **{wrong: R2})), "`to` and `from` together: no shape")
        E(name + "_%s_short" % other, mk("root", dict(base, **{other: R2[:-2]})), "`%s` one byte short: no shape" % other)
        E(name + "_%s_long" % other, mk("root", dict(base, **{other: R2 + "00"})), "`%s` one byte long: no shape" % other)
        E(name + "_%s_upper" % other, mk("root", dict(base, **{other: "0x" + R2[2:].upper()})),
          "`%s` in uppercase hexadecimal: no shape" % other)
        E(name + "_%s_bare" % other, mk("root", dict(base, **{other: R2[2:]})), "`%s` without 0x: no shape" % other)
        E(name + "_%s_int" % other, mk("root", dict(base, **{other: 7})), "`%s` an int: no shape" % other)
        E(name + "_%s_null" % other, mk("root", dict(base, **{other: None})), "`%s` null: no shape" % other)
        E(name + "_root_short", mk("root", dict(base, root=R[:-2])), "`root` one byte short: no shape")
        E(name + "_root_absent", mk("root", {"act": name, other: R2}), "`root` absent: no shape")
        E(name + "_note_int", mk("root", dict(base, note_md=1)), "a note that is no string: no shape")
        E(name + "_note_null", mk("root", dict(base, note_md=None)), "a note that is null: no shape")
        E(name + "_act_capital", mk("root", dict(base, act=name.capitalize())), "`act` with a capital: no shape")
        E(name + "_act_with_bind_kind", mk("root", {"act": "bind", "root": R, "kind": name, other: R2}),
          "`bind` with the kind `%s`: no shape" % name)
        E(name + "_differs_in_one_digit", mk("root", dict(base, **{other: R[:-1] + ("0" if R[-1] != "0" else "1")})),
          "`%s` one digit away from `root`: a shape" % other)


def build_reroot_tables():
    R2 = "root2"

    def sealed_at(block=20, to=R2):
        w = homed()
        w.anchor(CA, block, "root", mk("root", b_reroot(to)))
        return w

    # ---- the seal closes after a seated reroot that reads
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "before-the-seal")))
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "after-the-seal")))
    Tb("reroot_seals", w, "a binding, a reroot, a binding: the first stands, the seat is sealed from the next step, the second is UNSEATED")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "no-reroot-first")))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "no-reroot-second")))
    Tb("no_reroot_no_seal", w, "the same two bindings and no reroot: both stand, reroot null")

    # ---- from the next step: the records of the reroot's own block are read as the block found the seal
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_account("x", "same-block-lesser-tx")), tx=TXLO)
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)), tx=TXMID)
    w.anchor(CA, 20, "root", mk("root", b_account("x", "same-block-greater-tx")), tx=TXHI)
    w.anchor(CA, 25, "root", mk("root", b_account("x", "after-the-block-of-bindings")))
    Tb("reroot_block_binds_on_both_sides", w,
       "bindings in the reroot's own block, one tx below it and one above: both stand, and a binding in a later block is UNSEATED")
    w = homed()
    w.ts[(CA, 20)] = T0 + 500
    w.ts[(CB, 7)] = T0 + 500
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CB, 7, "root", mk("root", b_home(CB)))
    Tb("sealed_before_an_opening_of_the_same_second", w,
       "a reroot on the home and, in the same second on a chain of greater id, a home entry: the step after the reroot finds the seal closed and nothing opens")
    w = homed(chain=CB)
    w.ts[(CB, 20)] = T0 + 500
    w.ts[(CA, 7)] = T0 + 500
    w.ts[(CA, 30)] = T0 + 600
    w.anchor(CB, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 7, "root", mk("root", b_home(CA)))
    w.anchor(CA, 30, "root", mk("root", b_account("x", "bound-on-the-new-home")))
    Tb("opening_of_the_same_second_unseats_the_reroot", w,
       "a home entry on a chain of lesser id in the second of a reroot on the old home: the home moves first, the reroot is anchored off the home and seals nothing")

    # ---- a reroot that does not read seals nothing
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot("k3", root=R2)))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "after-anothers-reroot")))
    Tb("reroot_under_another_root_seals_nothing", w, "a reroot entry whose root is another: ROOT, no seal, the later binding stands")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", {"act": "reroot", "root": A("root"), "to": A("root")}))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "after-a-reroot-to-itself")))
    Tb("reroot_to_the_root_itself_seals_nothing", w, "a reroot body naming its own root: SHAPE, no seal")

    # ---- a reroot that is not seated seals nothing
    w = homed()
    w.anchor(CB, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 30, "root", mk("root", b_account("x", "after-a-reroot-off-the-home")))
    Tb("reroot_off_the_home", w, "a reroot anchored on a chain that is not the home: UNSEATED, no seal")
    w = World()
    w.anchor(CA, 5, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 10, "root", mk("root", b_home(CA)))
    w.anchor(CA, 12, "root", mk("root", b_account("x", "after-a-reroot-before-the-home")))
    Tb("reroot_before_a_home", w, "a reroot anchored before any home stands: UNSEATED, no seal")
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.anchor(CA, 20, "k1", mk("root", b_reroot(R2)))
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "after-a-predecessors-reroot")))
    Tb("reroot_signed_by_the_key_handed_on_from", w, "the office sends a reroot its predecessor signed: no seated record, no seal")
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.anchor(CA, 20, "root", mk("k1", b_reroot(R2)))
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "after-a-retired-senders-reroot")))
    Tb("reroot_sent_by_a_retired_key", w, "a reroot the office signed, sent by the key it took the seat from: not in office, no seal")
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "after-a-retired-keys-reroot")))
    Tb("reroot_by_a_retired_key", w, "the root key, retired in the home's book, signs and sends a reroot: UNSEATED, no seal, the office binds on")
    w = homed()
    w.anchor(CA, 20, "thief", mk("thief", b_reroot("thief")))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "after-an-outsiders-reroot")))
    Tb("reroot_by_an_outsider", w, "a reroot under R signed and sent by a key never in the line: nothing, and it is in no list")
    for verdict in ("VOID", "UNPROVEN"):
        w = homed()
        w.anchor(CA, 20, "root", mk("root", b_reroot(R2)), verdict=verdict)
        w.anchor(CA, 21, "root", mk("root", b_account("x", "after-a-reroot-" + verdict.lower())))
        Tb("reroot_" + verdict.lower(), w, "a reroot whose only record is %s: no seal, the later binding stands" % verdict)
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)), carry=False)
    w.anchor(CA, 21, "root", mk("root", b_account("x", "after-a-reroot-without-bytes")))
    Tb("reroot_without_its_bytes", w, "the hash of a reroot the pile does not hold: unread counts it, no seal")
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.anchor(CA, 20, "k1", mk("k1", b_reroot(R2)))
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "after-the-offices-reroot")))
    Tb("reroot_by_a_later_office", w, "the key the root handed the seat to reroots it: sealed, the office that key")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "k1", mk("k1", b_reroot(R2)))
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "after-the-incoming-keys-reroot")))
    Tb("reroot_by_the_incoming_key_in_its_accession_block", w,
       "the incoming key reroots in the block of its accession: that block's anchors found the root in office, no seal")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 21, "k1", mk("k1", b_account("x", "after-the-outgoing-keys-reroot")))
    Tb("reroot_in_the_block_of_the_handover", w,
       "the outgoing key reroots in the block of its hand-over: seated, sealed, and the office is the incoming key")
    w = homed()
    w.anchor(CB, 20, "root", mk("root", b_home(CB)), tx=TXLO)
    w.anchor(CB, 20, "root", mk("root", b_reroot(R2)), tx=TXHI)
    w.anchor(CB, 21, "root", mk("root", b_account("x", "bound-on-the-chain-moved-to")))
    Tb("reroot_in_the_block_that_opens_its_chain", w,
       "a reroot in the block whose opening moves the home to its chain: the block found the old home, UNSEATED, no seal")

    # ---- a sealed seat holds no record in office
    w = homed()
    w.anchor(CA, 15, "root", H32("loose", "before-the-seal"))
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 25, "root", H32("loose", "after-the-seal"))
    Tb("sealed_unread_stops", w, "the office anchors a hash the pile lacks before the seal and another after: unread is 1")
    w = homed()
    w.anchor(CA, 15, "root", H32("unproven", "before-the-seal"), verdict="UNPROVEN")
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 25, "root", H32("unproven", "after-the-seal"), verdict="UNPROVEN")
    Tb("sealed_unproven_stops", w, "an UNPROVEN record of the office before the seal and another after: unproven is 1")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 20, "root", H32("unproven", "in-the-block"), verdict="UNPROVEN")
    w.anchor(CA, 20, "root", H32("loose", "in-the-block"))
    Tb("reroot_block_counts_unread_and_unproven", w, "an UNPROVEN record and a hash without bytes in the reroot's own block: each counts")
    w = homed()
    keep = mk("root", b_account("x", "kept-bound-by-the-seal"))
    w.anchor(CA, 12, "root", keep)
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 25, "root", mk("root", b_release(keep)))
    Tb("sealed_release_ends_nothing", w, "a release after the seal of a binding made before it: UNSEATED, the row stays bound")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_persona("before")))
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 25, "root", mk("root", b_persona("after")))
    Tb("sealed_persona_stays", w, "a persona bound after the seal: the persona is the one bound before it")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CB, 30, "root", mk("root", b_home(CB)))
    Tb("sealed_home_stays", w, "a home entry on another chain after the seal: no opening, the home stands")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 30, "root", mk("root", b_home(CA)))
    Tb("sealed_home_opens_no_more", w, "a home entry on the home itself after the seal: one opening in the table")

    # ---- the accessions go on moving the office, the line and the heads
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.accede(CA, 30, "root", "k1")
    w.anchor(CA, 31, "k1", mk("k1", b_account("x", "bound-by-a-key-seated-after-the-seal")))
    Tb("sealed_office_follows_the_book", w,
       "a hand-over in the home's book after the seal: the office and the line move, and what the new key anchors is UNSEATED, its author a key of the line")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.accede(CB, 30, "root", "k1")
    Tb("sealed_office_stays_for_another_chains_book", w, "a hand-over off the home after the seal: the office stands, as before the seal")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.accede(CA, 30, "root", "k1")
    w.accede(CA, 40, "k1", "k2")
    w.anchor(CA, 41, "k1", mk("k1", b_account("x", "anchored-by-a-key-of-the-later-line")))
    Tb("sealed_line_grows", w, "two hand-overs after the seal: three keys in the lineage, the last the office, and a record of the middle key lists its entry")

    # ---- the reroot of the reading: the standing reroot least in moment, then in entry
    w = homed()
    a, b = sorted((mk("root", b_reroot(R2)), mk("root", b_reroot("k3"))), key=eid)
    w.anchor(CA, 20, "root", a, tx=TXMID)
    w.anchor(CA, 20, "root", b, tx=TXLO)
    Tb("two_reroots_one_block_tx_decides", w,
       "two reroots seated in one block, the lesser tx on the greater entry_id: the one whose tx is less is the reroot, the other is in no member")
    w = homed()
    a, b = mk("root", b_reroot(R2)), mk("root", b_reroot("k3"))
    w.anchor(CA, 20, "root", a, tx=TXMID)
    w.anchor(CA, 20, "root", b, tx=TXMID)
    Tb("two_reroots_one_tx_entry_decides", w, "two reroots anchored by one transaction: the lesser entry_id is the reroot")
    w = homed()
    e = mk("root", b_reroot(R2))
    w.anchor(CA, 20, "root", e, tx=TXHI)
    w.anchor(CA, 20, "root", e, tx=TXLO)
    Tb("reroot_anchored_twice_since_least_tx", w, "one reroot anchored twice in its block: since is the record of lesser tx")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 25, "root", mk("root", b_reroot("k3")))
    Tb("second_reroot_after_the_seal", w, "a second reroot in a later block: UNSEATED, the first is the reroot")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot("k3", root=R2)), tx=TXLO)
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)), tx=TXHI)
    Tb("two_reroots_the_lesser_tx_under_another_root", w, "of two reroots in one block the one of lesser tx names another root: the other is the reroot")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)), verdict="VOID")
    w.anchor(CA, 22, "root", mk("root", b_reroot("k3")))
    Tb("second_reroot_after_a_void_first", w, "a reroot whose record is VOID, then another, counted: the second is the reroot")
    w = homed()
    e = mk("root", b_reroot(R2))
    w.anchor(CB, 15, "root", e)
    w.anchor(CA, 20, "root", e)
    Tb("reroot_since_is_its_seated_record", w, "a reroot anchored off the home, then on it: since is the record on the home")

    # ---- the origin
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_origin(R2)))
    w.anchor(CA, 21, "root", mk("root", b_account("x", "after-an-origin")))
    Tb("origin_stands_and_seals_nothing", w, "an origin entry seated on the home: the table's origin, and the seat binds on")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_origin(R2)))
    w.anchor(CA, 14, "root", mk("root", b_origin("k3")))
    Tb("two_origins_the_earlier", w, "two origins in two blocks: the earlier is the origin")
    w = homed()
    a, b = sorted((mk("root", b_origin(R2)), mk("root", b_origin("k3"))), key=eid)
    w.anchor(CA, 12, "root", a, tx=TXHI)
    w.anchor(CA, 12, "root", b, tx=TXLO)
    Tb("two_origins_one_block_tx_decides", w, "two origins in one block, the lesser tx on the greater entry_id: the lesser tx")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_origin(R2)), tx=TXMID)
    w.anchor(CA, 12, "root", mk("root", b_origin("k3")), tx=TXMID)
    Tb("two_origins_one_tx_entry_decides", w, "two origins anchored by one transaction: the lesser entry_id")
    w = homed()
    e = mk("root", b_origin(R2))
    w.anchor(CB, 11, "root", e)
    w.anchor(CA, 30, "root", e)
    w.anchor(CA, 14, "root", e)
    Tb("origin_since_is_its_least_seated_record", w, "an origin anchored off the home and twice on it: since is the earlier record on the home")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_origin("k3", root=R2)))
    Tb("origin_under_another_root", w, "an origin entry whose root is another: ROOT, origin null")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", {"act": "origin", "root": A("root"), "from": A("root")}))
    Tb("origin_from_the_root_itself", w, "an origin body naming its own root: SHAPE, origin null")
    w = homed()
    w.anchor(CB, 12, "root", mk("root", b_origin(R2)))
    Tb("origin_off_the_home", w, "an origin anchored off the home: UNSEATED")
    w = World()
    w.anchor(CA, 5, "root", mk("root", b_origin(R2)))
    w.anchor(CA, 10, "root", mk("root", b_home(CA)))
    Tb("origin_before_a_home", w, "an origin anchored before any home: UNSEATED")
    w = homed()
    w.accede(CA, 11, "root", "k1")
    w.anchor(CA, 12, "k1", mk("root", b_origin(R2)))
    Tb("origin_signed_by_the_key_handed_on_from", w, "the office sends an origin its predecessor signed: UNSEATED")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    w.anchor(CA, 25, "root", mk("root", b_origin("k3")))
    Tb("origin_after_the_seal", w, "an origin anchored after the seal: UNSEATED, origin null")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_origin("k3")))
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    Tb("origin_and_reroot_in_one_table", w, "a seat that came from one root and moved to another: both members")
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)), tx=TXLO)
    w.anchor(CA, 20, "root", mk("root", b_origin("k3")), tx=TXHI)
    w.anchor(CA, 25, "root", mk("root", b_account("x", "after-the-block-of-the-origin")))
    Tb("origin_in_the_block_of_the_reroot", w,
       "an origin in the reroot's own block, at a greater tx: it stands, the seal closes all the same and a later binding is UNSEATED")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_origin(R2)), verdict="VOID")
    w.anchor(CA, 14, "root", mk("root", b_origin("k3")))
    Tb("origin_after_a_void_first", w, "an origin whose record is VOID, then another: the second is the origin")
    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_origin(R2, note="where the seat came from")))
    w.anchor(CA, 20, "root", mk("root", b_reroot("k3", note="where the seat goes")))
    Tb("origin_and_reroot_with_notes", w, "an origin and a reroot that carry notes: read as without them")

    # ---- a release names bindings alone
    w = homed()
    e = mk("root", b_reroot(R2))
    w.anchor(CA, 15, "root", mk("root", b_release(e)))
    w.anchor(CA, 20, "root", e)
    Tb("release_names_a_reroot", w, "a release, anchored before the seal, whose subject is the reroot: SUBJECT, the reroot stands")
    w = homed()
    e = mk("root", b_reroot(R2))
    w.anchor(CA, 20, "root", e, tx=TXLO)
    w.anchor(CA, 20, "root", mk("root", b_release(e)), tx=TXHI)
    Tb("release_in_the_block_of_the_reroot_it_names", w, "a release of the reroot in the reroot's own block: SUBJECT, the seat is sealed")
    w = homed()
    e = mk("root", b_origin(R2))
    w.anchor(CA, 12, "root", e)
    w.anchor(CA, 15, "root", mk("root", b_release(e)))
    Tb("release_names_an_origin", w, "a release whose subject is the origin: SUBJECT, the origin stands")

    # ---- a seat that moved its home, then reroots; a reroot under a book that holds another key
    w = homed()
    w.accede(CA, 12, "root", "k1")
    w.accede(CB, 13, "root", "k1")
    w.anchor(CB, 15, "k1", mk("k1", b_home(CB)))
    w.anchor(CB, 20, "k1", mk("k1", b_reroot(R2)))
    w.anchor(CA, 25, "k1", mk("k1", b_home(CA)))
    Tb("reroot_on_a_home_moved_to", w, "a seat hands on, moves its home, and reroots on the new home: sealed there, and no opening moves it back")
    w = homed()
    w.accede(CA, 15, "thief", "k2")
    w.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    Tb("reroot_reads_no_head", w, "the head of the home's book set to another key by a record no office made: the office reroots all the same")


def build_joined():
    def moved(frm="root", to="root2", block=20):
        w = seat(frm)
        w.anchor(CA, block, frm, mk(frm, b_reroot(to, root=frm)))
        return w

    def came(to="root2", frm="root", block=12):
        w = seat(to)
        w.anchor(CA, block, to, mk(to, b_origin(frm, root=to)))
        return w

    Jn("joined_both_name_each_other", moved(), came(), "the first reroots to the second, the second names the first as its origin: joined")
    Jn("joined_not_in_the_other_order", came(), moved(), "the same two inputs, the second first: not joined")
    Jn("joined_needs_the_reroot", seat("root"), came(), "the first has no reroot: not joined")
    Jn("joined_needs_the_origin", moved(), seat("root2"), "the second has no origin: not joined")
    Jn("joined_reroot_names_another", moved(to="k3"), came(), "the first reroots to a third root: not joined")
    Jn("joined_origin_names_another", moved(), came(frm="k3"), "the second names a third root as its origin: not joined")
    Jn("joined_neither", seat("root"), seat("root2"), "neither a reroot nor an origin: not joined")
    Jn("joined_one_input_twice", moved(), moved(), "one input given twice: a reroot names another root than its own, not joined")
    # a middle seat: it came from one root and moved to a third
    mid = came()
    mid.anchor(CA, 30, "root2", mk("root2", b_reroot("root3", root="root2")))
    last = came(to="root3", frm="root2")
    Jn("joined_first_link_of_three", moved(), mid, "three roots in a row: the first is joined to the second")
    Jn("joined_second_link_of_three", mid, last, "three roots in a row: the second is joined to the third")
    Jn("joined_not_across_the_middle", moved(), last, "three roots in a row: the first is not joined to the third")
    # the earliest alone counts on either side
    w = seat("root2")
    w.anchor(CA, 12, "root2", mk("root2", b_origin("k3", root="root2")))
    w.anchor(CA, 14, "root2", mk("root2", b_origin("root", root="root2")))
    Jn("joined_reads_the_earlier_origin", moved(), w, "the second names a third root first and the first root after: its origin is the earlier, not joined")
    w = seat("root")
    w.anchor(CA, 20, "root", mk("root", b_reroot("k3")), tx=TXLO)
    w.anchor(CA, 20, "root", mk("root", b_reroot("root2")), tx=TXHI)
    Jn("joined_reads_the_first_reroot", w, came(), "the first reroots twice in one block, the lesser tx to a third root: not joined")
    # a claim that is not seated joins nothing
    w = seat("root")
    w.anchor(CB, 20, "root", mk("root", b_reroot("root2")))
    Jn("joined_needs_a_seated_reroot", w, came(), "the first's reroot is anchored off its home: not joined")
    w = seat("root2")
    w.accede(CA, 11, "root2", "k1", root="root2")
    w.anchor(CA, 12, "root2", mk("root2", b_origin("root", root="root2")))
    Jn("joined_needs_an_origin_by_the_office", moved(), w,
       "the second's origin is signed and sent by its root key after that key handed the seat on: not seated, not joined")
    w = seat("root2")
    w.accede(CA, 11, "root2", "k1", root="root2")
    w.anchor(CA, 12, "k1", mk("k1", b_origin("root", root="root2")))
    Jn("joined_by_a_later_office_of_the_second", moved(), w, "the second's origin is its later office's: joined")
    w = seat("root")
    w.anchor(CA, 20, "root", mk("root", b_reroot("root2")))
    w.accede(CA, 30, "root", "k1")
    Jn("joined_whatever_the_first_office_does_after", w, came(), "the first's office is handed on after the seal: joined all the same")
    # the two inputs share nothing: other chains, other books
    wb = World(root="root2", chains=((CC, False),))
    wb.anchor(CC, 10, "root2", mk("root2", b_home(CC, root=A("root2"))))
    wb.anchor(CC, 12, "root2", mk("root2", b_origin("root", root="root2")))
    wa = World(chains=((CA, True),))
    wa.anchor(CA, 10, "root", mk("root", b_home(CA)))
    wa.anchor(CA, 20, "root", mk("root", b_reroot("root2")))
    Jn("joined_over_two_bases", wa, wb, "the second's home is a chain the first's basis lacks, with no book: joined")
    # NO_TABLE
    bad = moved().value()
    del bad["pile"]
    Jn("joined_first_no_input", bad, came(), "the first element is no valid face input: NO_TABLE")
    Jn("joined_second_no_input", moved(), bad, "the second element is no valid face input: NO_TABLE")
    for slug, value, note in (
            ("joined_one_element", [moved().value()], "an array of one input"),
            ("joined_three_elements", [moved().value(), came().value(), came().value()], "an array of three inputs"),
            ("joined_empty_array", [], "an empty array"),
            ("joined_a_single_input", moved().value(), "one face input and no array"),
            ("joined_null", None, "the value null"),
            ("joined_second_null", [moved().value(), None], "a second element that is null"),
            ("joined_first_an_array", [[moved().value()], came().value()], "a first element that is an array"),
            ("joined_an_object_of_two", {"first": moved().value(), "second": came().value()}, "an object holding the two inputs")):
        rel = "joined/j%03d_%s.json" % (_n("joined"), slug)
        _write(rel, json.dumps(value, separators=(",", ":")).encode("utf-8"))
        MANIFEST.append({"path": rel, "category": "joined", "cmd": "joined", "args": ["joined", rel],
                         "note": note + ": NO_TABLE", "predicted": {"exit": 0, "stdout": out_text(p_joined(value))}})
    good = json.dumps([moved().value(), came().value()], separators=(",", ":")).encode("utf-8")
    Jr("joined_byte_order_mark", b"\xef\xbb\xbf" + good, "a byte order mark before the array: the reader refuses the file, NO_TABLE")
    Jr("joined_trailing_bytes", good + b"x", "a byte after the array: NO_TABLE")
    Jr("joined_empty_file", b"", "an empty file: NO_TABLE")
    Jr("joined_whitespace_around", b" \n" + good + b"\n", "white space around the array: read",
       value=[moved().value(), came().value()])
    Mu("joined_no_args", ["joined"], "joined with no argument")
    first = [c["path"] for c in MANIFEST if c["category"] == "joined"][0]
    Mu("joined_two_args", ["joined", first, first], "joined with an argument beyond its list")
    Mu("joined_absent", ["joined", "joined/absent.json"], "an absent pair file")
    Mu("joined_directory", ["joined", "joined"], "a directory as the pair path")


def build_reroot_round1():
    """Witnesses of the first round's seats: the origin under each test of sec 7.3, its since inside
    one block, and the inputs behind two sentences of sec 9."""
    R2 = "root2"
    w = homed()
    e = mk("root", b_origin(R2))
    w.anchor(CA, 12, "root", e, tx=TXHI)
    w.anchor(CA, 12, "root", e, tx=TXLO)
    Tb("origin_anchored_twice_since_least_tx", w, "one origin anchored twice in its block, the greater tx first in the input: since is the record of lesser tx")
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.anchor(CA, 20, "root", mk("k1", b_origin(R2)))
    Tb("origin_sent_by_a_retired_key", w, "an origin the office signed, sent by the key it took the seat from: not in office, origin null")
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.anchor(CA, 20, "root", mk("root", b_origin(R2)))
    Tb("origin_by_a_retired_key", w, "the root key, retired in the home's book, signs and sends an origin: UNSEATED, origin null")
    w = homed()
    w.anchor(CA, 20, "thief", mk("thief", b_origin("thief")))
    Tb("origin_by_an_outsider", w, "an origin under R signed and sent by a key never in the line: nothing, and it is in no list")
    for verdict in ("VOID", "UNPROVEN"):
        w = homed()
        w.anchor(CA, 20, "root", mk("root", b_origin(R2)), verdict=verdict)
        Tb("origin_" + verdict.lower(), w, "an origin whose only record is %s: origin null" % verdict)
    w = homed()
    w.anchor(CA, 20, "root", mk("root", b_origin(R2)), carry=False)
    Tb("origin_without_its_bytes", w, "the hash of an origin the pile does not hold: unread counts it, origin null")
    w = homed()
    w.accede(CA, 20, "root", "k1")
    w.anchor(CA, 20, "k1", mk("k1", b_origin(R2)))
    Tb("origin_by_the_incoming_key_in_its_accession_block", w,
       "the incoming key names an origin in the block of its accession: that block's anchors found the root in office, origin null")
    w = homed()
    w.accede(CA, 15, "root", "k1")
    w.anchor(CA, 20, "k1", mk("k1", b_origin(R2)))
    Tb("origin_by_a_later_office", w, "the key the root handed the seat to names the origin: it stands")

    # sec 9, «A key in office that is stolen» and «The reader's chains»: a chain that runs behind
    w = homed()
    w.accede(CA, 50, "root", "k1")
    w.ts[(CB, 5)] = T0 + 40
    w.ts[(CB, 6)] = T0 + 41
    w.anchor(CB, 5, "root", mk("root", b_home(CB)))
    w.anchor(CB, 6, "root", mk("root", b_reroot("thief")))
    w.anchor(CA, 60, "k1", mk("k1", b_reroot(R2)))
    Tb("reroot_by_a_retired_key_on_a_chain_that_runs_behind", w,
       "after a hand-over on the home, the holder of the outgoing key opens a home and reroots on a chain whose clock reads before the hand-over: that reroot stands, the hand-over moves no office, and the seat's own reroot is a record of a key outside the line")
    # sec 9, «A reroot is half a proof»: joined is read under two bases
    home_a, move_b = mk("root", b_home(CA)), mk("root", b_home(CB))
    stolen = mk("root", b_reroot("thief"))
    own = mk("k1", b_reroot(R2))
    full = World()
    full.anchor(CA, 10, "root", home_a)
    full.anchor(CB, 20, "root", move_b)
    full.accede(CB, 25, "root", "k1")
    full.anchor(CA, 30, "root", stolen)
    full.anchor(CB, 40, "k1", own)
    narrow = World(chains=((CA, True),))
    narrow.anchor(CA, 10, "root", home_a)
    narrow.anchor(CA, 30, "root", stolen)
    theirs = seat("thief")
    theirs.anchor(CA, 12, "thief", mk("thief", b_origin("root", root="thief")))
    ours = seat(R2)
    ours.anchor(CA, 12, R2, mk(R2, b_origin("root", root=R2)))
    Jn("joined_under_a_basis_that_leaves_out_the_home", narrow, theirs,
       "a reader whose basis leaves out the chain the home moved to reads the reroot a retired key signed on the home it left: joined to that key's holder")
    Jn("joined_not_under_the_basis_that_holds_the_home", full, theirs,
       "the same seat under a basis that holds its home: the retired key's reroot is anchored off the home, not joined to its holder")
    Jn("joined_under_the_basis_that_holds_the_home", full, ours,
       "the same seat under a basis that holds its home: joined to the root its key in office named")

    # the transport's depth bound: an anchor record's extra member reaches it, alone and inside a pair
    import copy

    def nest(n):
        v = []
        for _ in range(n - 1):
            v = [v]
        return v

    w = homed()
    w.anchor(CA, 12, "root", mk("root", b_account("x", "deep")))
    v = copy.deepcopy(w.value())
    v["anchors"][0] = dict(v["anchors"][0], x=nest(125))
    Tb("anchor_member_depth_128", v, "an anchor record's extra member nested to depth 128 of the file: read past, the table stands")
    v = copy.deepcopy(w.value())
    v["anchors"][0] = dict(v["anchors"][0], x=nest(126))
    Tr("anchor_member_depth_129", json.dumps(v, separators=(",", ":")).encode("utf-8"),
       "the same member one level deeper, a container opening at depth 129: the reader refuses the file")
    wa = seat("root")
    wa.anchor(CA, 20, "root", mk("root", b_reroot(R2)))
    wb = seat(R2)
    wb.anchor(CA, 12, R2, mk(R2, b_origin("root", root=R2)))
    v = copy.deepcopy(wa.value())
    v["anchors"][0] = dict(v["anchors"][0], x=nest(124))
    Jn("joined_pair_depth_128", v, wb, "a pair whose first input carries a member nested to depth 128 of the pair file: read, joined")
    v = copy.deepcopy(wa.value())
    v["anchors"][0] = dict(v["anchors"][0], x=nest(125))
    Jr("joined_pair_depth_129", json.dumps([v, wb.value()], separators=(",", ":")).encode("utf-8"),
       "a first input that alone is valid at depth 128 opens a container at depth 129 of the pair file: the reader refuses the file")


def build_reroot_random(seed, count):
    seats_ = ["root", "k1", "k2", "k3"]
    for n in range(count):
        rng = random.Random("face-corpus/reroot/%d/%d" % (seed, n))
        books = [(CA, True), (CB, rng.random() < 0.85)]
        w = World(chains=books)
        with_book = [c for c, b in books if b]
        chains = [c for c, _ in books]
        office, home = "root", None                     # a guide for the draws and no prediction
        binds, ents = [], []
        block, second = 1, T0
        w.ts[(CA, 1)] = T0
        w.anchor(CA, 1, "root", mk("root", b_home(CA)))
        home = CA
        for step in range(rng.randrange(8, 30)):
            block += rng.randrange(1, 4)
            second += rng.choice([0, 0, 1, 1, 2, 7])
            chain = home if rng.random() < 0.8 else rng.choice(chains)
            w.ts[(chain, block)] = second
            sender = office if rng.random() < 0.8 else rng.choice(seats_ + ["thief"])
            author = sender if rng.random() < 0.88 else rng.choice(seats_ + ["thief"])
            roll = rng.random()
            if roll < 0.14 and chain in with_book:
                holder = office if rng.random() < 0.75 else rng.choice(seats_ + ["thief"])
                to = rng.choice(seats_)
                w.accede(chain, block, holder, to, txi=rng.randrange(3), logi=rng.randrange(1000))
                if holder == office and home == chain:
                    office = to
                if rng.random() < 0.4:
                    w.anchor(chain, block, sender, mk(author, b_account("x", "q%d-%d" % (n, step))))
            elif roll < 0.24:
                root = None if rng.random() < 0.9 else "root2"
                e = mk(author, b_reroot(rng.choice(["root2", "root3", "k3"]) if root is None else "k3", root=root))
                ents.append(e)
                w.anchor(chain, block, sender, e, verdict=rng.choice(["counted"] * 7 + ["VOID", "UNPROVEN"]),
                         carry=rng.random() < 0.9)
            elif roll < 0.34:
                e = mk(author, b_origin(rng.choice(["root2", "root3"])))
                ents.append(e)
                w.anchor(chain, block, sender, e, verdict=rng.choice(["counted"] * 8 + ["VOID"]))
            elif roll < 0.42:
                target = rng.choice(chains)
                w.anchor(chain, block, sender, mk(author, b_home(target)))
                if sender == office and target == chain:
                    home = chain
            elif roll < 0.9:
                kind = rng.random()
                if kind < 0.4:
                    e = mk(author, b_account(rng.choice(["x", "github"]), rng.choice(["alice", "bob", "carol"])))
                    binds.append(e)
                elif kind < 0.55:
                    e = mk(author, b_persona(rng.choice(["Ada", "Bo", "Cy"])))
                    binds.append(e)
                elif kind < 0.7:
                    e = mk(author, b_key(rng.choice(["fa", "fb"]), "payout"))
                    binds.append(e)
                else:
                    e = mk(author, b_release(rng.choice(binds + ents) if binds + ents else H32("none", n, step)))
                ents.append(e)
                w.anchor(chain, block, sender, e, verdict=rng.choice(["counted"] * 9 + ["VOID"]),
                         carry=rng.random() < 0.95)
            elif ents:
                w.anchor(chain, block, sender, rng.choice(ents), carry=False)
            else:
                w.anchor(chain, block, sender, H32("loose-reroot", n, step), verdict=rng.choice(["counted", "UNPROVEN"]))
        Tb("reroot_random_%03d" % n, w, "a random world with reroots and origins among its hand-overs, openings, bindings and releases")



# the boundary list of law sec 10.1: each boundary with the cases that witness each side, by slug;
# the comparator carries the test of each side and checks this list against its own
BOUNDARIES = [["1 label length 1..32", ["platform_one", "platform_len32"],
    ["platform_empty", "platform_len33"]],
    ["1 label alphabet a-z", ["platform_mixed", "platform_z"],
    ["platform_upper", "platform_backtick", "platform_brace"]],
    ["1 label alphabet 0-9", ["platform_digit", "platform_nine"],
    ["platform_slash", "platform_colon"]],
    ["1 label hyphen only", ["platform_hyphen"],
    ["platform_dot", "platform_underscore"]],
    ["1 role label (key row)", ["role_len32", "role_edges"],
    ["role_len33", "role_colon"]],
    ["1 label as declare argument", ["role_len32", "role_edges"],
    ["declare_role_len33", "declare_role_colon", "declare_role_comma"]],
    ["1 handle length 1..128", ["handle_one", "handle_len128"],
    ["handle_empty", "handle_len129"]],
    ["1 handle ends not U+0020", ["handle_inner_space"],
    ["handle_lead_space", "handle_trail_space", "handle_only_space"]],
    ["1 handle within U+0020..U+007E (parent)", ["handle_printable"],
    ["handle_non_ascii", "handle_tab"]],
    ["1 face entry type literal", ["home"],
    ["type_next", "type_longer", "type_cased"]],
    ["2.2 home exact members", ["home"],
    ["home_without_chain", "home_extra_member", "home_with_string_note"]],
    ["2.2 home chain int", ["home_chain_zero", "home_chain_max"],
    ["home_chain_string", "home_chain_bool"]],
    ["2.2 account optional note", ["account", "account_note", "account_note_empty"],
    ["account_note_int", "account_note_null"]],
    ["2.2 account exact members", ["account"],
    ["account_without_handle", "account_extra_member", "account_with_address"]],
    ["2.2 key exact members", ["key", "key_note"],
    ["key_without_sig", "key_extra_member", "key_with_handle"]],
    ["2.2 release exact members", ["release", "release_note"],
    ["release_without_subject", "release_extra_member", "release_with_kind"]],
    ["2.2 act closed", ["account"],
    ["act_unknown", "act_cased", "act_space"]],
    ["2.2 kind closed", ["key"],
    ["kind_unknown", "kind_cased", "kind_key_over_account_members"]],
    ["2.2 root hex20", ["home"],
    ["home_root_upper", "account_root_short", "key_root_bare", "release_root_hex32"]],
    ["2.2 address hex20", ["key"],
    ["address_upper", "address_short"]],
    ["2.2 sig hex65", ["key"],
    ["sig_short", "sig_long", "sig_upper"]],
    ["2.2 subject hex32", ["release"],
    ["subject_short", "subject_hex20", "subject_upper"]],
    ["entry rooted", ["home"],
    ["unrooted_home"]],
    ["3.1 declaration exact", ["key"],
    ["consent_trailing_lf", "consent_crlf", "consent_upper_hex", "consent_no_0x", "consent_lines_swapped"]],
    ["3.2 domain", ["key"],
    ["consent_entry_domain", "consent_next_domain", "consent_personal_sign_of_text", "consent_hash_not_hex"]],
    ["3.3 v in {27,28}", ["key"],
    ["consent_v_26", "consent_v_29", "consent_v_recid"]],
    ["3.3 r,s in [1,n-1]", ["key"],
    ["consent_r_zero", "consent_s_zero", "consent_r_is_n", "consent_s_is_n"]],
    ["3.3 s <= (n-1)/2", ["consent_s_at_half"],
    ["consent_s_half_plus_one", "consent_high_s"]],
    ["3.3 recovery and address", ["key", "key_face_is_root"],
    ["consent_r_off_curve", "consent_other_key", "consent_v_flipped"]],
    ["3.3 own three members", ["unrooted_key_consent_holds"],
    ["consent_other_root", "consent_other_role", "consent_other_address"]],
    ["5.1 length > 36", ["len37", "account"],
    ["len36", "len35"]],
    ["5.1 selector", ["account"],
    ["selector_last_byte", "selector_zero", "anchor_many"]],
    ["5.1 sha256(tail) = c[4:36]", ["account"],
    ["id_bit_flipped", "tail_byte_flipped", "id_of_other_entry"]],
    ["5.1 accept(tail)", ["account", "history", "shapeless_face_entry"],
    ["tail_not_entry", "tail_v1_entry", "tail_plus_lf_new_id"]],
    ["6.1 five members", ["empty"],
    ["invalid_without_root", "invalid_without_pile", "invalid_extra_member"]],
    ["6.1 root hex20", ["empty"],
    ["invalid_root_upper", "invalid_root_short"]],
    ["6.1 chain six members", ["empty"],
    ["invalid_chain_without_book", "invalid_chain_extra_member"]],
    ["6.1 chain ints", ["empty"],
    ["invalid_chain_id_string", "invalid_chain_from_string", "invalid_chain_to_string", "invalid_chain_book_start_string", "invalid_chain_from_false", "invalid_chain_id_string_alone"]],
    ["6.1 fromBlock <= toBlock", ["valid_from_equals_to"],
    ["invalid_from_above_to", "invalid_from_above_to_alone"]],
    ["6.1 registries hex20 strictly ascending", ["valid_registries_two", "valid_registries_empty"],
    ["invalid_registries_descending", "invalid_registries_unsorted", "invalid_registries_upper", "invalid_chain_registry_int"]],
    ["6.1 book null iff bookFromBlock null", ["home_without_book", "empty"],
    ["invalid_book_without_start", "invalid_start_without_book"]],
    ["6.1 book hex20", ["empty"],
    ["invalid_book_upper", "invalid_chain_book_int"]],
    ["6.1 bookFromBlock <= toBlock", ["valid_book_start_equals_to"],
    ["invalid_book_start_above_to", "invalid_book_start_above_to_alone"]],
    ["6.1 chains strictly ascending", ["empty"],
    ["invalid_chains_descending", "invalid_chains_repeated"]],
    ["6.1 anchor seven members and forms", ["home_then_account", "valid_anchor_extra_member", "valid_anchor_three_extra_members",
     "valid_anchor_extra_members_of_every_kind"],
    ["invalid_anchor_without_hash", "invalid_anchor_extra_member_without_verdict", "invalid_anchor_tx_short", "invalid_anchor_sender_upper", "invalid_anchor_hash_short", "invalid_anchor_hash_upper", "invalid_anchor_time_string", "invalid_anchor_tx_upper"]],
    ["6.1 verdict closed", ["verdicts"],
    ["invalid_anchor_verdict", "invalid_anchor_verdict_null"]],
    ["6.1 anchor chain declared, block in range", ["valid_anchor_at_from", "valid_anchor_at_to"],
    ["invalid_anchor_chain_undeclared", "invalid_anchor_below_range", "invalid_anchor_above_range_alone"]],
    ["6.1 accession nine members and forms", ["home_book_alone"],
    ["invalid_accession_without_root", "invalid_accession_root_upper", "invalid_accession_root_hex32", "invalid_accession_root_null", "invalid_accession_without_to", "invalid_accession_extra_member", "invalid_accession_holder_short", "invalid_accession_log_index_string", "invalid_accession_to_upper", "invalid_accession_tx_short", "invalid_accession_tx_index_string", "invalid_accession_time_string", "invalid_accession_tx_index_true", "invalid_accession_tx_upper", "invalid_accession_holder_upper"]],
    ["6.1 accession on a booked chain, in book range", ["valid_accession_at_book_start", "valid_accession_at_to", "valid_accession_below_from_block"],
    ["invalid_accession_no_book", "invalid_accession_below_book_start", "invalid_accession_chain_undeclared", "invalid_anchor_above_range"]],
    ["6.1 pile spelling", ["valid_pile_upper_digits", "valid_pile_empty_string"],
    ["invalid_pile_no_prefix", "invalid_pile_upper_prefix", "invalid_pile_odd", "invalid_pile_not_hex", "invalid_pile_int"]],
    ["6.2 anchor key (chain, block, tx, hash)", ["valid_anchor_same_tx_other_hash", "valid_anchor_same_hash_other_block", "valid_clock_other_chain_free", "valid_anchor_same_hash_other_tx"],
    ["invalid_anchor_twice_other_verdict", "invalid_anchor_twice_other_sender", "invalid_anchor_twice_other_time"]],
    ["6.2 two anchor records that agree on the seven are one record", ["valid_anchor_twice", "valid_anchor_twice_unproven_once", "valid_anchor_twice_opening_once",
     "valid_anchor_twice_extra_member_differs"], []],
    ["6.2 accession key (chain, block, txIndex, logIndex)", ["valid_accession_other_log_index", "valid_accession_other_tx_index", "valid_accession_same_position_other_block", "valid_accession_same_position_other_chain"],
    ["invalid_accession_twice", "invalid_accession_same_position", "invalid_accession_same_position_other_tx", "invalid_accession_same_position_other_holder",
     "invalid_accession_same_position_other_root"]],
    ["6.2 one time per block", ["valid_clock_stands_still"],
    ["invalid_clock_two_times_one_block", "invalid_clock_anchor_and_accession", "invalid_clock_two_accessions_one_block"]],
    ["6.2 clock monotone per chain", ["valid_clock_stands_still", "valid_clock_other_chain_free"],
    ["invalid_clock_runs_back", "invalid_clock_accession_runs_back", "invalid_clock_anchor_runs_back_after_accession"]],
    ["transport reader: no byte order mark", ["spelling_pretty"],
    ["file_bom"]],
    ["transport reader: integers within [0, 2^53 - 1]", ["int_max"],
    ["file_int_2p53"]],
    ["transport reader: no member named twice", ["spelling_pretty"],
    ["file_duplicate_member"]],
    ["transport reader: scalar values only", ["spelling_escapes"],
    ["file_lone_surrogate_in_name"]],
    ["transport reader: depth 128", ["anchor_member_depth_128"],
    ["file_depth_129", "anchor_member_depth_129"]],
    ["7.1 time before chainId", ["time_before_chain_id"],
    []],
    ["7.1 chainId at one second (both orders)", ["same_second_anchor_on_lower_chain", "same_second_anchor_on_higher_chain"],
    []],
    ["7.1 block order at one second", ["blocks_sharing_a_second"],
    []],
    ["7.2 anchors before accessions in a step", ["accession_block"],
    []],
    ["7.2 accessions by txIndex then logIndex", ["two_accessions_one_block_in_order", "two_accessions_one_block_reversed", "two_accessions_same_holder", "accessions_tx_index_first"],
    []],
    ["7.2 accession moves only from the office", ["two_accessions_one_block_in_order"],
    ["accession_by_non_office", "retired_root_hands_on_under_root"]],
    ["7.2 accession moves only under the root R", ["accession_enrolment_first_record", "accession_after_enrolment"],
    ["accession_other_root_moves_nothing", "accession_in_book_not_entered"]],
    ["7.2 accession to itself lengthens the line", ["handover_to_itself"],
    []],
    ["7.2 home nothing: every book; home set: home book only", ["first_handover_wins", "line_grows_by_time"],
    ["home_book_alone", "home_without_book"]],
    ["7.2 opening needs office, counted, home shape, root R, chain = step, author the office, the book's head the office",
     ["home_then_account", "home_by_office_after_handover", "opening_after_entering_the_book"],
    ["opening_void", "retired_key_cannot_open", "home_entries_that_open_nothing", "home_by_stranger_author", "home_by_earlier_key",
     "opening_needs_the_books_head", "opening_in_the_block_of_entering", "opening_without_book_by_successor",
     "opening_where_the_book_moved_on", "opening_where_the_book_names_another"]],
    ["7.2 home from the next step", ["home_then_account"],
    ["bind_in_opening_block"]],
    ["7.3 seated only on the home", ["binding_on_two_chains"],
    ["binding_off_home", "bind_without_home"]],
    ["7.3 the author is the office as the step found it", ["author_office_after_handover"],
    ["author_old_and_future", "author_retired_office_anchors_binding", "author_retired_office_anchors_key_face"]],
    ["7.3 moment order: time, chain, block, tx", ["moments_are_earliest", "moment_chain_before_block", "moment_tx_tie_greater_listed_first"],
    []],
    ["7.4 release stands only on a binding", ["bind_then_release"],
    ["release_of_no_binding", "release_of_release_and_stranger_release"]],
    ["7.5 least release, entry tie", ["moments_are_earliest", "two_releases_one_transaction"],
    []],
    ["7.5 release before binding clamps", ["release_before_binding"],
    []],
    ["7.5 state bound iff an open span", ["two_bindings_one_released"],
    ["bind_then_release"]],
    ["7.8 spans and homes entry ties", ["two_bindings_one_transaction", "two_openings_one_transaction"],
    []],
    ["7.8 row order accounts then keys", ["rows_in_order"],
    []],
    ["7.6 each reason", ["reasons_first_applies", "reasons_first_applies", "strangers", "key_faces", "release_of_no_binding", "bind_without_home"],
    []],
    ["7.6 AUTHOR before SUBJECT", ["release_of_release_and_stranger_release"],
    []],
    ["7.7 unread distinct, in office, any chain, any type", ["unread", "unread_before_home_by_step_office"],
    ["empty"]],
    ["7.6 listed when a key of the line anchored it, whatever the verdict", ["misread_scope", "reasons_first_applies"],
    ["misread_scope_fork_key", "pile", "misread_scope"]],
    ["7.7 unproven counts the office's UNPROVEN records", ["unproven_counts_the_office", "unproven_opening", "unproven_release"],
    ["empty", "home_then_account"]],
    ["7.7 bytes accept rejects hold nothing", ["unread_rejected_bytes"],
    ["pile"]],
    ["7.5 the clamp compares tx", ["release_same_block_lesser_tx"],
    ["bind_then_release"]],
    ["7.5 state reads every span", ["later_of_two_bindings_released"],
    ["bind_then_release"]],
    ["7.8 spans by since, tx before entry", ["spans_tx_before_entry"],
    []],
    ["6.1 chains compared numerically", ["valid_chains_nine_then_ten"],
    ["invalid_chains_ten_then_nine"]],
    ["7.1 steps compared numerically", ["steps_chain_nine_before_ten", "steps_block_nine_before_ten", "steps_second_nine_before_ten"],
    []],
    ["7.3 moments compared numerically", ["moment_chain_nine_before_ten", "moment_block_ninety_nine_before_hundred"],
    []],
    ["7.2 an opening is a face entry of the home shape", ["home_then_account"],
    ["home_body_of_another_type", "home_body_beyond_its_row"]],
    ["7.4 a release reads only under R", ["bind_then_release"],
    ["release_under_another_root"]],
    ["7.5 least moment before entry", ["release_least_moment_greater_entry"],
    []],
    ["7.8 homes by record before entry", ["homes_by_record_before_entry"],
    []],
    ["7.8 homes one element per opening record", ["one_home_entry_opened_twice"],
    []],
    ["7.7 unproven off the home and without bytes", ["unproven_off_home", "unproven_hash_unheld"],
    []],
    ["7.6 ROOT before SIGNATURE and SUBJECT", ["reasons_root_before_signature_and_subject"],
    []],
    ["2.2 key optional note", ["key", "key_note", "key_without_note_md"],
    ["key_note_int", "key_note_null"]],
    ["2.2 release optional note", ["release", "release_note", "release_without_note_md"],
    ["release_note_int", "release_note_null"]],
    ["2.2 home carries no note", ["home"],
    ["home_with_string_note", "home_note_null"]],
    ["2.3 one face, one row", ["two_bindings_one_released"],
    ["rows_in_order"]],
    ["7.8 key rows by address before role", ["key_rows_address_before_role"],
    []],
    ["7.7 the counts read the office as the step found it", ["unproven_outgoing_office_in_handover_block", "unread_outgoing_office_in_handover_block"],
    ["unproven_incoming_key_in_handover_block"]],
    ["7.7 unproven counts records, on every chain, of every type", ["unproven_one_hash_twice", "unproven_entry_of_another_type", "unproven_move_of_the_home"],
    []],
    ["7.3 a release is seated on the home alone", ["release_on_new_home"],
    ["release_off_home"]],
    ["7.5 the least release ends the span wherever it falls", ["release_before_and_after_binding"],
    []],
    ["7.3 moment by block before tx in one second", ["moment_block_before_tx"],
    []],
    ["6.1 record chainId and blockNumber are ints", ["home_book_alone"],
    ["invalid_anchor_chain_string", "invalid_accession_chain_string", "invalid_accession_block_string"]],
    ["7.4 a seated entry that does not read is no binding", ["bind_then_release"],
    ["release_of_seated_entries_that_do_not_read"]],
    ["7.2 an opening's author is the office as the step found it", ["home_by_office_after_handover"],
    ["home_by_future_key", "home_by_earlier_key", "home_by_retired_key_moves_nothing", "home_by_retired_key_before_home"]],
    ["declare key of sixty-four digits", ["signed", "signed_0x"],
    ["declare_key_short", "declare_key_63_digits", "declare_key_65_digits"]]]
BOUNDARIES += [[
    "1 label alphabet, every printable character (%s)" % m,
    ["%s_char_%02x" % (m, c) for c in range(0x20, 0x7F) if p_label(chr(c))],
    ["%s_char_%02x" % (m, c) for c in range(0x20, 0x7F) if not p_label(chr(c))]] for m in ("platform", "role")]
BOUNDARIES += [
    ["7.3 the author is the office as the step found it, in the hand-over block", ["author_outgoing_key_in_handover_block"],
     ["author_incoming_key_in_handover_block", "opening_by_incoming_key_in_handover_block",
      "author_incoming_key_own_record_in_handover_block"]],
    ["9 a move back to an earlier home", ["home_moves_back", "home_moves_back_same_entry"], []],
    ["7.5 the ending release by tx inside one block", ["two_releases_one_block_tx_decides"], []],
    ["7.8 homes by tx inside one block", ["two_openings_one_block_tx_decides"], []],
    ["7.3 an author in reach outside the line", ["author_in_reach_outside_line"], []],
    ["7.3 the office seats its own entry alone among the keys of the line", ["author_of_three_keys"], []],
    ["7.3 a record seated at its step stays seated after its author hands over",
     ["seated_by_author_then_anchored_by_successor", "release_by_office_of_predecessors_binding"], []],
    ["7.4 a release a retired key signed ends no span", ["author_retired_office_anchors_release"], []],
    ["9 the first home entry anchored again by a successor moves nothing back", ["home_entry_anchored_again_by_successor"], []]]

BOUNDARIES += [
    ["7.2 the head of a chain's book follows the records under R alone, whoever their holder",
     ["head_follows_any_holder_under_root"], ["head_ignores_another_root"]],
    ["7.2 the opening block's accessions are read under the new home",
     ["opening_block_accession_under_new_home", "opening_and_accession_moving_block"], ["old_home_accession_in_the_second_of_the_move"]],
    ["9 a move back needs the old home's book in step", ["move_back_needs_the_book_in_step"], []],
    ["2.2 persona exact members", ["persona", "persona_cjk"],
     ["persona_with_note", "persona_extra_member", "persona_without_act", "persona_without_kind", "persona_without_name_md",
      "persona_without_root", "persona_with_handle", "persona_name_plain_member", "persona_act_release", "persona_kind_cased"]],
    ["1 name is a string", ["persona"],
     ["persona_name_int", "persona_name_null", "persona_name_array", "persona_name_object", "persona_name_bool"]],
    ["1 name length 1..32", ["persona_one", "persona_len32", "persona_len32_cjk", "persona_len32_astral"],
     ["persona_empty", "persona_len33", "persona_len33_cjk", "persona_len33_astral"]],
    ["1 name without a space or a line break", ["persona_punctuation", "persona_combining"],
     ["persona_space_inside", "persona_space_first", "persona_space_last", "persona_two_lines"]],
    ["1 name outside ASCII rides a prose member", ["persona_cjk"], ["persona_name_plain_member_cjk"]]]
BOUNDARIES += [["1 name barred U+%04X to U+%04X" % (lo, hi),
                ["persona_free_%04x" % o for o in (below, above) if o is not None],
                ["persona_barred_%04x" % o for o in ([lo] if lo == hi else [lo, hi])]]
               for lo, hi, below, above in name_edges()]
BOUNDARIES += [
    ["1 name barred: the last two scalar values of a plane",
     ["persona_free_%04x" % o for o in PLANE_END[1]], ["persona_barred_%04x" % o for o in PLANE_END[0]]],
    ["7.4 a persona entry stands like a binding", ["persona_alone"],
     ["persona_by_retired_key", "persona_off_the_home", "persona_before_a_home", "persona_under_another_root",
      "persona_void_record", "persona_name_with_a_space"]],
    ["7.5 the persona: the open span last in since",
     ["persona_latest_of_two", "persona_falls_back_on_release", "persona_release_of_the_earlier", "persona_bound_again",
      "persona_row_of_two_open_spans", "persona_row_later_span_closed", "persona_across_a_handover"], []],
    ["7.5 the persona: none where no span of a persona row is open", ["persona_all_released", "account_on_a_platform_called_persona"], []],
    ["7.5 the persona: chains at one second by chain id", ["persona_one_second_two_chains"], []],
    ["7.5 the persona: equal moments by the greater entry", ["persona_one_transaction_two_entries"], []],
    ["7.8 rows: accounts, keys, personas, the personas bytewise by name", ["persona_rows_in_order"], []],
    ["7.8 a name spelled as the canonical form spells it", ["persona_spelled_in_the_table"], []]]

BOUNDARIES += [
    ["1 name barred: the last two scalar values of every plane from 2 to 14",
     ["persona_free_%04x" % o for o in PLANE_END_ALL[1]], ["persona_barred_%04x" % o for o in PLANE_END_ALL[0]]],
    ["1 name length in scalar values, none grouped or normalized", ["persona_len32_combining", "persona_len32_mixed_widths"],
     ["persona_len33_combining", "persona_len33_mixed_widths"]],
    ["7.8 persona rows bytewise by UTF-8, never by UTF-16 code units", ["persona_rows_utf8_not_utf16"], []],
    ["2.3 names compare bytewise, none normalized or folded", ["persona_rows_not_normalized"], []],
    ["7.5 the persona: tx before entry inside one block", ["persona_one_block_tx_decides"], []],
    ["7.5 the persona: block before tx inside one second", ["persona_one_second_block_decides"], []],
    ["6.1 anchor blockNumber an int", ["home_then_account"], ["invalid_anchor_block_string", "invalid_anchor_block_string_alone"]],
    ["7.5 the persona: chain before block inside one second", ["persona_one_second_chain_before_block"], []]]
BOUNDARIES += [
    ["1 name barred inside a range, away from its ends", ["persona_barred_%04x" % o for o in BARRED_INSIDE], []],
    ["1 name length counts each regional indicator", ["persona_len32_regional_indicators"], ["persona_len33_regional_indicators"]],
    ["7.2 a record under another root leaves the head as it stood", ["head_kept_through_another_roots_record"], []],
    ["7.2 an opening on the standing home is tested by the head", ["opening_again_on_the_home_needs_the_head"], []],
    ["7.8 persona rows bytewise, no case folded", ["persona_rows_bytewise_not_caseless"], []]]
BOUNDARIES += [
    ["1 name length counts decoded scalar values, an escape counting one", ["persona_len32_quote", "persona_len32_backslashes"],
     ["persona_len33_quote", "persona_len33_backslashes"]],
    ["1 a name may begin with a combining mark", ["persona_leading_combining"], []]]

BOUNDARIES += [[n_ % dict(s=s_, o=o_), [x % dict(s=s_, o=o_) for x in a_], [x % dict(s=s_, o=o_) for x in b_]]
    for s_, o_ in (("reroot", "to"), ("origin", "from")) for n_, a_, b_ in (
    ("2.2 %(s)s row: exactly act, root and %(o)s, with or without note_md",
     ["%(s)s_plain", "%(s)s_with_note", "%(s)s_note_empty"],
     ["%(s)s_extra_member", "%(s)s_with_sig", "%(s)s_with_kind", "%(s)s_missing_%(o)s", "%(s)s_both_to_and_from", "%(s)s_root_absent"]),
    ("2.2 %(s)s row: %(o)s a hex20", ["%(s)s_plain"],
     ["%(s)s_%(o)s_short", "%(s)s_%(o)s_long", "%(s)s_%(o)s_upper", "%(s)s_%(o)s_bare", "%(s)s_%(o)s_int", "%(s)s_%(o)s_null"]),
    ("2.2 %(s)s row: root a hex20", ["%(s)s_plain"], ["%(s)s_root_short"]),
    ("2.2 %(s)s row: %(o)s byte-unequal to root", ["%(s)s_differs_in_one_digit", "%(s)s_plain"], ["%(s)s_%(o)s_is_root"]),
    ("2.2 %(s)s row: note_md a string", ["%(s)s_with_note"], ["%(s)s_note_int", "%(s)s_note_null"]),
    ("2.2 %(s)s row: act the string %(s)s", ["%(s)s_plain"], ["%(s)s_act_capital", "%(s)s_act_with_bind_kind"]),
    ("2.2 %(s)s row: the shape reads no author, and rooted says whose it is", ["%(s)s_by_another_key", "%(s)s_other_root"], []))]
BOUNDARIES += [
    ["2.2 reroot row: a from in the place of to", ["reroot_plain"], ["reroot_with_from"]],
    ["2.2 origin row: a to in the place of from", ["origin_plain"], ["origin_with_to"]],
    ["7.2 the seal closes after a seated reroot that reads", ["reroot_seals"], ["no_reroot_no_seal"]],
    ["7.2 the seal is read as the step found it: the reroot's own block",
     ["reroot_block_binds_on_both_sides", "reroot_block_counts_unread_and_unproven", "origin_in_the_block_of_the_reroot"], []],
    ["7.2 the step after the reroot: time, then chainId", ["sealed_before_an_opening_of_the_same_second"],
     ["opening_of_the_same_second_unseats_the_reroot"]],
    ["7.2 a reroot that does not read seals nothing", ["reroot_seals"],
     ["reroot_under_another_root_seals_nothing", "reroot_to_the_root_itself_seals_nothing"]],
    ["7.2 a reroot seals by a seated record: on the home", ["reroot_seals", "reroot_on_a_home_moved_to"],
     ["reroot_off_the_home", "reroot_before_a_home", "reroot_in_the_block_that_opens_its_chain"]],
    ["7.2 a reroot seals by a seated record: its author and its sender the office",
     ["reroot_by_a_later_office", "reroot_in_the_block_of_the_handover"],
     ["reroot_signed_by_the_key_handed_on_from", "reroot_sent_by_a_retired_key", "reroot_by_a_retired_key",
      "reroot_by_an_outsider", "reroot_by_the_incoming_key_in_its_accession_block"]],
    ["7.2 a reroot seals by a seated record: counted", ["reroot_seals"], ["reroot_void", "reroot_unproven"]],
    ["7.2 a reroot seals by an entry of the pile", ["reroot_seals"], ["reroot_without_its_bytes"]],
    ["7.2 a sealed seat holds no record in office: unread", ["sealed_unread_stops"], []],
    ["7.2 a sealed seat holds no record in office: unproven", ["sealed_unproven_stops"], []],
    ["7.2 a sealed seat holds no record in office: no release, no persona", ["sealed_release_ends_nothing", "sealed_persona_stays"], []],
    ["7.2 a sealed seat holds no record in office: no opening", ["sealed_home_stays", "sealed_home_opens_no_more"], []],
    ["7.2 the accessions of the home move the office and the line of a sealed seat", ["sealed_office_follows_the_book", "sealed_line_grows"],
     ["sealed_office_stays_for_another_chains_book"]],
    ["7.2 a reroot reads no head", ["reroot_reads_no_head"], []],
    ["7.4 the reroot: tx inside one block", ["two_reroots_one_block_tx_decides"], []],
    ["7.4 the reroot: entry_id inside one tx", ["two_reroots_one_tx_entry_decides"], []],
    ["7.4 the reroot's since: its least seated record", ["reroot_anchored_twice_since_least_tx", "reroot_since_is_its_seated_record"], []],
    ["7.4 a later reroot: seated where the seal is open", ["second_reroot_after_a_void_first"], ["second_reroot_after_the_seal"]],
    ["7.4 the reroot is chosen among the reroots that read", ["two_reroots_the_lesser_tx_under_another_root"], []],
    ["7.4 an origin stands by a seated record, and seals nothing", ["origin_stands_and_seals_nothing"],
     ["origin_off_the_home", "origin_before_a_home", "origin_signed_by_the_key_handed_on_from", "origin_after_the_seal"]],
    ["7.4 an origin reads under R", ["origin_stands_and_seals_nothing"], ["origin_under_another_root", "origin_from_the_root_itself"]],
    ["7.4 the origin: the least moment, then the lesser entry_id",
     ["two_origins_the_earlier", "two_origins_one_block_tx_decides", "two_origins_one_tx_entry_decides", "origin_after_a_void_first"], []],
    ["7.4 the origin's since: its least seated record", ["origin_since_is_its_least_seated_record"], []],
    ["7.8 a reroot and an origin in one table", ["origin_and_reroot_in_one_table", "origin_and_reroot_with_notes"], []],
    ["7.4 a release names bindings alone: a reroot or an origin is none",
     ["release_names_a_reroot", "release_in_the_block_of_the_reroot_it_names", "release_names_an_origin"], []],
    ["7.9 joined: the first's reroot names the second's root", ["joined_both_name_each_other"],
     ["joined_needs_the_reroot", "joined_reroot_names_another", "joined_reads_the_first_reroot", "joined_needs_a_seated_reroot"]],
    ["7.9 joined: the second's origin names the first's root", ["joined_both_name_each_other", "joined_by_a_later_office_of_the_second"],
     ["joined_needs_the_origin", "joined_origin_names_another", "joined_reads_the_earlier_origin", "joined_needs_an_origin_by_the_office"]],
    ["7.9 joined is ordered, the first to the second", ["joined_both_name_each_other"],
     ["joined_not_in_the_other_order", "joined_one_input_twice", "joined_neither"]],
    ["7.9 joined link by link", ["joined_first_link_of_three", "joined_second_link_of_three"], ["joined_not_across_the_middle"]],
    ["7.9 joined reads each table under its own input", ["joined_over_two_bases", "joined_whatever_the_first_office_does_after"], []],
    ["7.4 an origin stands by a seated record: its author and its sender the office, counted, its bytes in the pile",
     ["origin_by_a_later_office"],
     ["origin_sent_by_a_retired_key", "origin_by_a_retired_key", "origin_by_an_outsider", "origin_void", "origin_unproven",
      "origin_without_its_bytes", "origin_by_the_incoming_key_in_its_accession_block"]],
    ["7.4 the origin's since: tx inside one block", ["origin_anchored_twice_since_least_tx"], []],
    ["7.2 the heads of a sealed seat", ["sealed_office_stays_for_another_chains_book"], []],
    ["7.2 a key a reader's clocks hold in office reroots", ["reroot_by_a_retired_key_on_a_chain_that_runs_behind"], []],
    ["7.9 joined is read under two bases", ["joined_under_a_basis_that_leaves_out_the_home", "joined_under_the_basis_that_holds_the_home"],
     ["joined_not_under_the_basis_that_holds_the_home"]],
    ["harness joined: the depth bound counts from the pair's array", ["joined_pair_depth_128"], ["joined_pair_depth_129"]],
    ["harness joined: an array of exactly two valid face inputs", ["joined_both_name_each_other", "joined_whitespace_around"],
     ["joined_first_no_input", "joined_second_no_input", "joined_one_element", "joined_three_elements", "joined_empty_array",
      "joined_a_single_input", "joined_null", "joined_second_null", "joined_first_an_array", "joined_an_object_of_two",
      "joined_byte_order_mark", "joined_trailing_bytes", "joined_empty_file"]]]

# the boundaries whose two sides give one answer on every input (law sec 10.1), with the reason
UNOBSERVABLE = [
 [
  "3.3 s <= (n-1)/2",
  "a consent with s at (n - 1) / 2 that holds needs an address fixed by the hash of a declaration that names that address, so both sides answer false"
 ],
 [
  "5.1 length > 36",
  "a tail of 36 bytes or fewer is no entry under accept, so a calldata of 36 bytes and one of 37 bytes whose tail is no entry both answer null"
 ],
 [
  "7.2 the heads of a sealed seat",
  "no record is in office after the seal, so no opening and no seating of a home entry reads a head, and a walk that set the heads of a sealed seat and one that left them give one table"
 ]
]


def coverage():
    tally = {}
    for c in MANIFEST:
        p = c["predicted"]
        ans = json.loads(p["stdout"]) if p["exit"] == 0 else None
        for lab in labels_of(c, ans):
            tally[lab] = tally.get(lab, 0) + 1
    return tally


README = """# zikaron.face/1 convergence corpus

Generated by `gen.py` from `docs/zikaron-face-v1.md` (the law),
`docs/zikaron-v2.md` (the parent) and `zikaron-conformance/HARNESS-FACE.md`
(the command-line contract) alone. No implementation of the face law was read
while building it, except by the review seats that chose the cases of
`build_boundaries`, `build_round1`, `build_round2`, `build_round2_readers`,
`build_round3`, `build_round3_readers`, `build_corpus_reader`,
`build_second_round` and `build_third_round` after reading and mutating
`face-py/`; their
predictions are the generator's own. The parent's side of
the work (canonical bytes, the entry predicate, secp256k1 with RFC 6979 and
low-s) is imported from
`../corpus2/gen.py`, the parent's own corpus generator, which is a tool here
and never evidence for this law.

## Regenerating

    cd zikaron-conformance/face-corpus
    python3 gen.py --seed %(seed)d

The run is deterministic: one seed rebuilds a byte-identical tree. The four
directories below are deleted and repopulated on each run, and
`manifest.json` and this file are rewritten. Requires Python 3 and
`pycryptodome` (for `Crypto.Hash.keccak`), the parent generator's only
dependency.

## Running it

Every case in `manifest.json` carries an `args` array: the argument vector to
hand `zkf`, with every path relative to this directory, and `predicted`: the
exit code and, for exit 0, the exact bytes of stdout. So a whole run is

    cd zikaron-conformance/face-corpus
    python3 - <<'EOF'
    import json, subprocess
    for c in json.load(open("manifest.json"))["cases"]:
        r = subprocess.run(["zkf"] + c["args"], capture_output=True)
        print(c["path"], r.returncode, r.stdout.decode())
    EOF

`../compare-face.py` is the instrument: it runs a candidate and the criterion
case by case, compares the two byte for byte and the criterion with the
prediction, and prints the manifest's sha256 and a digest over every corpus
byte it read, which `ZKF_EXPECT` may pin.

## Layout

- `entries/` (%(entries)d cases) - `eNNN_*.bin` holds one byte string for
  `zkf entry <path> <root>`; the root is in the case's `args`.
- `declare` (%(declare)d cases) - no files: the arguments of `zkf declare`
  are the case.
- `carried/` (%(carried)d cases) - `cNNN_*.bin` holds one transaction's
  calldata for `zkf carried`.
- `tables/` (%(tables)d cases) - `tNNN_*.json` holds one face input for
  `zkf table`.
- `joined/` (%(joined)d cases) - `jNNN_*.json` holds one array of two face
  inputs for `zkf joined`.
- `misuse` (%(misuse)d cases) - argument vectors that are harness misuse:
  exit 2 and nothing on stdout.

## Reading manifest.json

`cases` lists every case with its `path`, `category`, `cmd`, `args`, `note`
and `predicted`. `boundaries` lists every closed boundary of the law with
the cases that witness each side, by slug, and every rule of order, tie and
precedence with the cases that witness it, its second side empty;
`unobservable` names the boundaries whose two sides give one answer on every
input that can be built, with the reason. The comparator carries the test of
each side and requires both lists to equal its own. `coverage` counts the
cases that witness each label of the closed vocabularies (`vocabularies`),
and `unwitnessed` lists the labels no case of this seed witnesses: it is
empty, and the comparator checks it against the criterion's own answers.
Every `predicted` field is the corpus author's reading of the law, produced
by gen.py and by nothing else; the predictor is written to the same clauses
as the criterion core by the same author, so agreement between the two is
one reading measured twice, and the independent evidence is the
implementations of the review rounds.
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    seed = args.seed
    rng = random.Random(seed)

    KEYS.clear()
    for tag in ("root", "k1", "k2", "k3", "thief", "stranger", "fa", "fb", "root2", "root3"):
        KEYS[tag] = pg.testkey("face/" + tag, seed)

    for d in DIRS:
        p = os.path.join(HERE, d)
        if os.path.isdir(p):
            shutil.rmtree(p)
        os.makedirs(p)
    del MANIFEST[:]
    _counters.clear()
    _seq[0] = 0

    build_entries(rng)
    build_declare(rng)
    build_carried(rng)
    build_tables(rng)
    build_random(rng, 90, seed)
    first = dict((c["category"], c["path"]) for c in reversed(MANIFEST))
    build_misuse(first["entries"], first["carried"], first["tables"])
    build_boundaries()
    build_round1()
    build_round2()
    build_round2_readers()
    build_round3()
    build_round3_readers()
    build_author_is_office()
    build_book_root()
    build_anchor_records()
    build_anchor_records_checked()
    build_home_book()
    build_persona()
    build_corpus_reader()
    build_second_round()
    build_third_round()
    build_reroot_entries()
    build_reroot_tables()
    build_joined()
    build_reroot_random(seed, 60)
    build_reroot_round1()

    counts = {}
    for c in MANIFEST:
        counts[c["category"]] = counts.get(c["category"], 0) + 1
    cov = coverage()
    man = {
        "corpus": "zikaron.face/1 convergence corpus",
        "law": "docs/zikaron-face-v1.md",
        "harness": "zikaron-conformance/HARNESS-FACE.md",
        "seed": seed,
        "generator": "gen.py",
        "keys": dict((tag, {"privkey": "0x%064x" % KEYS[tag], "address": A(tag)}) for tag in sorted(KEYS)),
        "counts": counts,
        "coverage": cov,
        "vocabularies": VOCAB,
        "unwitnessed": sorted(lab for lab in VOCAB if lab not in cov),
        "boundaries": BOUNDARIES,
        "unobservable": UNOBSERVABLE,
        "cases": MANIFEST,
    }
    with open(os.path.join(HERE, "manifest.json"), "w") as f:
        json.dump(man, f, indent=1, sort_keys=True)
        f.write("\n")
    with open(os.path.join(HERE, "README.md"), "w") as f:
        f.write(README % dict(counts, seed=seed))

    print("seed %d" % seed)
    for k in sorted(counts):
        print("  %-8s %5d cases" % (k, counts[k]))
    print("  unwitnessed: %s" % (", ".join(man["unwitnessed"]) or "none"))
    for lab in sorted(cov):
        print("  %-22s %5d" % (lab, cov[lab]))


if __name__ == "__main__":
    main()
