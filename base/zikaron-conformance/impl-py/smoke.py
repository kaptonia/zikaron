#!/usr/bin/env python3
"""A handful of smoke tests.  The corpus comparison is the real test."""

import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from zkcanon import Obj, canon, parse15                      # noqa: E402
from zkcrypto import (N, address_of, eip191_digest, message,  # noqa: E402
                      pubkey, sha256, sign_digest, recover)
from zkentry import DOMAIN_ENTRY, DOMAIN_ADOPTION            # noqa: E402

PRIV = 0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318
PRIV2 = 0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef

FAILS = []


def ck(name, got, want):
    if got != want:
        FAILS.append("%s:\n  got  %r\n  want %r" % (name, got, want))
    else:
        print("ok   %s" % name)


def run(*args):
    p = subprocess.run([sys.executable, os.path.join(HERE, "zk1.py")] + list(args),
                       capture_output=True)
    return p.returncode, p.stdout


def addr(priv):
    return "0x" + address_of(pubkey(priv)).hex()


def sign_entry(priv, six_pairs):
    b6 = canon(Obj(six_pairs))
    digest = eip191_digest(message(DOMAIN_ENTRY, sha256(b6)))
    r, s, v = sign_digest(priv, digest)
    sig = "0x" + (r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([v])).hex()
    return canon(Obj(six_pairs + [("sig", sig)]))


def tmpwrite(b):
    fd, path = tempfile.mkstemp()
    os.write(fd, b)
    os.close(fd)
    return path


H0 = "0x" + "00" * 32


def entry(priv, seq, prev, etype, body):
    return sign_entry(priv, [
        ("spec", "zikaron/1"),
        ("entryType", etype),
        ("author", addr(priv)),
        ("seq", seq),
        ("prev", prev),
        ("body", Obj(body)),
    ])


def main():
    A = addr(PRIV)
    B = addr(PRIV2)

    # --- 1. a signed genesis checks -------------------------------------
    g = entry(PRIV, 0, None, "genesis", [("statement_md", "the archive\n")])
    p = tmpwrite(g)
    rc, out = run("check", p)
    ck("genesis check", (rc, out),
       (0, canon(Obj([("entry_id", "0x" + sha256(g).hex()), ("ok", True)]))))

    # tamper with one byte of the author -> E_SIG_SIGNER
    bad = g.replace(A.encode(), (A[:-1] + ("0" if A[-1] != "0" else "1")).encode())
    p = tmpwrite(bad)
    rc, out = run("check", p)
    ck("tampered author", out,
       canon(Obj([("ok", False), ("token", "E_SIG_SIGNER")])))

    # --- 2. tokens -------------------------------------------------------
    cases = [
        (b'{"a":1,"a":2}', "E_DUP_KEY"),
        (b'{"a":1}\n', "E_NOT_CANONICAL"),
        (b'\xef\xbb\xbf{}', "E_JSON"),
        (b'{"a":1.5}', "E_NUMBER"),
        (b'[-Infinity]', "E_NUMBER"),
        (b'[Infinity]', "E_JSON"),
        (b'["\\ud800"]', "E_JSON"),
        (b'{"":1}', "E_KEY_CHARSET"),
        (b'{"a":"\\u0001"}', "E_VALUE_CHARSET"),
        (b'{"a_md":"\\u0001"}', "E_ENVELOPE_MISSING"),   # already canonical
        (b'{"a_md":"\\u0041"}', "E_NOT_CANONICAL"),      # canonical is "A"
        (b'{"a_md":"\\u000A"}', "E_NOT_CANONICAL"),      # canonical is \n
        (b'{"a_md":"\\u007f"}', "E_NOT_CANONICAL"),      # canonical is raw DEL
        (b'{"a":"\x7f"}', "E_VALUE_CHARSET"),
        (b'{"a_md":"\x7f"}', "E_ENVELOPE_MISSING"),
        (b'\xff', "E_UTF8"),
        (b"[" * 129, "E_DEPTH"),
        (b"[" * 128 + b"]" * 128, "E_ENVELOPE"),
        (b'{"a":1}', "E_ENVELOPE_MISSING"),
        (b'[1,2]', "E_ENVELOPE"),
    ]
    for src, tok in cases:
        p = tmpwrite(src)
        rc, out = run("check", p)
        ck("token %r" % src[:24], out,
           canon(Obj([("ok", False), ("token", tok)])))

    # a duplicate key inside an otherwise fine entry names E_DUP_KEY
    dup = g[:-1] + b',"sig":"0x' + b"00" * 65 + b'"}'
    p = tmpwrite(dup)
    rc, out = run("check", p)
    ck("dup sig key", out, canon(Obj([("ok", False), ("token", "E_DUP_KEY")])))

    # --- 3. canon --------------------------------------------------------
    p = tmpwrite(b'  { "b" : 2 , "a" : [ 1 , true ] } ')
    rc, out = run("canon", p)
    ck("canon", out, canon(Obj([
        ("canon", "0x" + b'{"a":[1,true],"b":2}'.hex()), ("ok", True)])))

    # --- 4. sign round-trips through recovery ---------------------------
    six = canon(Obj([
        ("spec", "zikaron/1"), ("entryType", "genesis"), ("author", A),
        ("seq", 0), ("prev", None), ("body", Obj([("statement_md", "x")]))]))
    p = tmpwrite(six)
    rc, out = run("sign", "0x%064x" % PRIV, p)
    res = json.loads(out)
    ck("sign signer", res["signer"], A)
    sigb = bytes.fromhex(res["sig"][2:])
    r = int.from_bytes(sigb[:32], "big")
    s = int.from_bytes(sigb[32:64], "big")
    ck("sign low-s", s <= (N - 1) // 2, True)
    pt = recover(bytes.fromhex(res["digest"][2:]), r, s, sigb[64] - 27)
    ck("sign recovers", "0x" + address_of(pt).hex(), A)

    # --- 5. audit of a two-entry ledger ---------------------------------
    e0 = entry(PRIV, 0, None, "genesis", [("statement_md", "hello")])
    id0 = "0x" + sha256(e0).hex()
    e1 = entry(PRIV, 1, id0, "history",
               [("content", H0),
                ("mode", Obj([("mark", "hand"), ("toolchain", H0)]))])
    id1 = "0x" + sha256(e1).hex()
    basis = {"chains": [], "bareTx": [], "adoptionChains": []}
    inp = {
        "root": A,
        "pile": ["0x" + e0.hex(), "0x" + e1.hex()],
        "anchors": [], "unavailable": [], "evidence": [], "basis": basis,
    }
    p = tmpwrite(json.dumps(inp).encode())
    rc, out = run("audit", p)
    rep = json.loads(out)
    ck("audit entries", rep["entries"], 2)
    ck("audit findings", rep["findings"], [])
    ck("audit label", rep["label"], "COMPLETE")
    ck("audit unanchored", rep["unanchored"], sorted([id0, id1]))

    # an anchor of e0 counted, e1 unanchored -> still COMPLETE
    inp["anchors"] = [{"chainId": 1, "blockNumber": 10, "blockTimestamp": 100,
                       "tx": H0, "sender": A, "hash": id0,
                       "verdict": "counted"}]
    p = tmpwrite(json.dumps(inp).encode())
    rc, out = run("audit", p)
    rep = json.loads(out)
    ck("audit anchored label", rep["label"], "COMPLETE")
    ck("audit anchored unanchored", rep["unanchored"], [id1])

    # anchor of bytes nobody produced -> MISSING, GAPS
    h9 = "0x" + "99" * 32
    inp["anchors"].append({"chainId": 1, "blockNumber": 11,
                           "blockTimestamp": 101, "tx": H0, "sender": A,
                           "hash": h9, "verdict": "counted"})
    p = tmpwrite(json.dumps(inp).encode())
    rc, out = run("audit", p)
    rep = json.loads(out)
    ck("audit missing", rep["missing"],
       [{"hash": h9, "anchors": [{"chainId": 1, "tx": H0}]}])
    ck("audit missing label", rep["label"], "GAPS")

    # a fork at seq 1 -> EQUIVOCATION, BROKEN_CHAIN
    e1b = entry(PRIV, 1, id0, "annotation", [("note_md", "twin")])
    id1b = "0x" + sha256(e1b).hex()
    inp2 = dict(inp)
    inp2["anchors"] = []
    inp2["pile"] = ["0x" + e0.hex(), "0x" + e1.hex(), "0x" + e1b.hex()]
    p = tmpwrite(json.dumps(inp2).encode())
    rc, out = run("audit", p)
    rep = json.loads(out)
    ck("audit fork label", rep["label"], "BROKEN_CHAIN")
    a, b = sorted([id1, id1b])
    ck("audit fork findings", rep["findings"], [
        {"name": "EQUIVOCATION", "position": 1, "entry_id": a, "hard": True,
         "seq": 1, "a": a, "b": b}])

    # an entry signed by a stranger is EXCLUDED, not a fault
    ex = entry(PRIV2, 1, id0, "annotation", [("note_md", "not mine")])
    inp3 = dict(inp)
    inp3["anchors"] = []
    inp3["pile"] = ["0x" + e0.hex(), "0x" + e1.hex(), "0x" + ex.hex()]
    p = tmpwrite(json.dumps(inp3).encode())
    rc, out = run("audit", p)
    rep = json.loads(out)
    ck("audit excluded label", rep["label"], "COMPLETE")
    ck("audit excluded", rep["excluded"],
       [{"seq": 1, "author": B, "entry_id": "0x" + sha256(ex).hex()}])

    # malformed basis -> NO_LABEL
    inp4 = dict(inp3)
    inp4["basis"] = {"chains": [], "bareTx": []}
    p = tmpwrite(json.dumps(inp4).encode())
    rc, out = run("audit", p)
    ck("no label", out, canon(Obj([("ok", False), ("reason", "NO_LABEL")])))

    # --- 6. adoption attestation ----------------------------------------
    anchors = [Obj([("chainId", 1), ("content", H0),
                    ("payloadKind", "raw"), ("tx", H0)])]
    C = canon(Obj([("adopter", A), ("anchors", anchors), ("prev", id0)]))
    dg = eip191_digest(message(DOMAIN_ADOPTION, sha256(C)))
    r, s, v = sign_digest(PRIV2, dg)
    att = "0x" + (r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([v])).hex()
    ad = entry(PRIV, 1, id0, "adoption",
               [("anchors", anchors), ("attestor", B), ("attestation", att)])
    p = tmpwrite(ad)
    rc, out = run("check", p)
    ck("adoption check", out,
       canon(Obj([("entry_id", "0x" + sha256(ad).hex()), ("ok", True)])))

    inp5 = {"root": A,
            "pile": ["0x" + e0.hex(), "0x" + ad.hex()],
            "anchors": [], "unavailable": [],
            "evidence": [{"chainId": 1, "tx": H0, "sender": B,
                          "calldata": "0x" + "00" * 32}],
            "basis": {"chains": [], "bareTx": [],
                      "adoptionChains": [{"chainId": 1, "throughBlock": 99}]}}
    p = tmpwrite(json.dumps(inp5).encode())
    rc, out = run("audit", p)
    rep = json.loads(out)
    ck("adoption proven", rep["adoption_unproven"], [])

    # no adoptionChains entry -> unproven
    inp5["basis"]["adoptionChains"] = []
    p = tmpwrite(json.dumps(inp5).encode())
    rc, out = run("audit", p)
    rep = json.loads(out)
    ck("adoption unproven", rep["adoption_unproven"],
       [{"seq": 1, "entry_id": "0x" + sha256(ad).hex(), "index": 0}])

    print()
    if FAILS:
        for f in FAILS:
            print("FAIL " + f)
        sys.exit(1)
    print("all smoke tests pass")


if __name__ == "__main__":
    main()
