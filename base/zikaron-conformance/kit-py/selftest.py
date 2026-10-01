#!/usr/bin/env python3
"""A live end-to-end exercise of every zkk command.

Builds, from scratch and on disk: two zikaron/1 ledgers (a genesis, a history
and a grant under root A; a genesis and a downstream grant under root B), two
audit inputs, a signed fingerprint manifest and a matching acknowledgement
(both signed through `zkk sign` itself), a badge payload, and a disclosure kit
directory.  Then it runs each of the eleven commands as a subprocess and
prints the exact bytes each one wrote.

Run:  python3 selftest.py
"""

import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
IMPL = os.path.join(os.path.dirname(HERE), "impl-py")
sys.path.insert(0, HERE)
sys.path.insert(0, IMPL)

from zkcanon import Obj, canon                                    # noqa: E402
from zkcrypto import address_of, pubkey, sha256                   # noqa: E402

WORK = os.path.join(HERE, "selftest-work")
ZKK = os.path.join(HERE, "zkk.py")
ZK1 = os.path.join(IMPL, "zk1.py")

PRIV_A = 0x1111111111111111111111111111111111111111111111111111111111111111
PRIV_B = 0x2222222222222222222222222222222222222222222222222222222222222222
PRIV_R1 = 0x3333333333333333333333333333333333333333333333333333333333333333
PRIV_R2 = 0x4444444444444444444444444444444444444444444444444444444444444444

FAILURES = []


def addr(priv):
    return "0x" + address_of(pubkey(priv)).hex()


def hexpriv(priv):
    return "%064x" % priv


def w(name, data):
    p = os.path.join(WORK, name)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "wb") as fh:
        fh.write(data)
    return p


def run(args, expect_code=0):
    proc = subprocess.run([sys.executable] + args, capture_output=True)
    if proc.returncode != expect_code:
        raise SystemExit("command %r exited %d: %s"
                         % (args[1:], proc.returncode,
                            proc.stderr.decode("utf-8", "replace")))
    return proc.stdout


def zkk(*args, **kw):
    return run([ZKK] + list(args), **kw)


def sign_entry(priv, six):
    """Sign a six-member envelope with the parent core's producer side."""
    p = w("tmp/preimage.json", canon(six))
    out = json.loads(run([ZK1, "sign", hexpriv(priv), p, "zikaron/1"]))
    pairs = list(six.pairs) + [("sig", out["sig"])]
    b = canon(Obj(pairs))
    return b, "0x" + sha256(b).hex()


def envelope(etype, author, seq, prev, body):
    return Obj([("spec", "zikaron/1"), ("entryType", etype),
                ("author", author), ("seq", seq), ("prev", prev),
                ("body", body)])


def check(label, got, want):
    got_s = got.decode("utf-8", "replace") if isinstance(got, bytes) else got
    ok = (got_s == want)
    print(("  ok   " if ok else "  FAIL ") + label)
    print("       " + got_s)
    if not ok:
        print("       expected: " + want)
        FAILURES.append(label)


def show(label, got):
    print("  ---- " + label)
    print("       " + got.decode("utf-8", "replace"))


def main():
    if os.path.isdir(WORK):
        shutil.rmtree(WORK)
    os.makedirs(WORK)

    A, B = addr(PRIV_A), addr(PRIV_B)
    R1, R2 = addr(PRIV_R1), addr(PRIV_R2)
    if R1 > R2:
        R1, R2 = R2, R1
        priv_r1 = PRIV_R2
    else:
        priv_r1 = PRIV_R1
    C = "0x" + "cc" * 20

    work_bytes = b"the work, as delivered"
    W = "0x" + sha256(work_bytes).hex()
    T1 = "0x" + sha256(b"terms one").hex()
    T2 = "0x" + sha256(b"terms two").hex()

    # ---------------------------------------------------------------- ledgers
    gen_a, gen_a_id = sign_entry(PRIV_A, envelope(
        "genesis", A, 0, None, Obj([("statement_md", "Ledger A.")])))
    hist_a, hist_a_id = sign_entry(PRIV_A, envelope(
        "history", A, 1, gen_a_id,
        Obj([("content", W),
             ("mode", Obj([("mark", "selftest"),
                           ("toolchain", "0x" + "11" * 32)]))])))
    grant_a, grant_a_id = sign_entry(PRIV_A, envelope(
        "grant", A, 2, hist_a_id,
        Obj([("grantee", B), ("work", W), ("terms", T1),
             ("window", Obj([("from", 1000), ("to", 2000000000)]))])))

    gen_b, gen_b_id = sign_entry(PRIV_B, envelope(
        "genesis", B, 0, None, Obj([("statement_md", "Ledger B.")])))
    grant_b, grant_b_id = sign_entry(PRIV_B, envelope(
        "grant", B, 1, gen_b_id,
        Obj([("grantee", C), ("work", W), ("terms", T2),
             ("upstream", grant_a_id)])))

    p_grant_a = w("entries/grant_a.zk1", grant_a)
    p_grant_b = w("entries/grant_b.zk1", grant_b)
    w("entries/gen_a.zk1", gen_a)
    w("entries/hist_a.zk1", hist_a)
    w("entries/gen_b.zk1", gen_b)

    def basis(sender):
        return {"chains": [{"chainId": 1, "fromBlock": 0, "toBlock": 100,
                            "registries": ["0x" + "ab" * 20],
                            "senders": [sender]}],
                "bareTx": [], "adoptionChains": []}

    audit_a = {
        "root": A,
        "pile": ["0x" + gen_a.hex(), "0x" + hist_a.hex(), "0x" + grant_a.hex()],
        "anchors": [{"chainId": 1, "blockNumber": 10,
                     "blockTimestamp": 1700000000, "tx": "0x" + "01" * 32,
                     "sender": A, "hash": grant_a_id, "verdict": "counted"}],
        "unavailable": [], "evidence": [], "basis": basis(A),
    }
    audit_b = {
        "root": B,
        "pile": ["0x" + gen_b.hex(), "0x" + grant_b.hex()],
        "anchors": [{"chainId": 1, "blockNumber": 12,
                     "blockTimestamp": 1700000100, "tx": "0x" + "02" * 32,
                     "sender": B, "hash": grant_b_id, "verdict": "counted"}],
        "unavailable": [], "evidence": [], "basis": basis(B),
    }
    p_audit_a = w("audit_a.json", json.dumps(audit_a).encode("ascii"))
    p_audit_b = w("audit_b.json", json.dumps(audit_b).encode("ascii"))

    # ------------------------------------------------------------------ s3-s5
    print("s3 signing, s4 manifest, s5 acknowledgement")
    v1 = "0x" + sha256(b"variant for R1").hex()
    v2 = "0x" + sha256(b"variant for R2").hex()
    fpm_pre = Obj([
        ("spec", "zikaron.fpm/1"), ("author", A), ("work", W),
        ("grant", grant_a_id),
        ("rows", [Obj([("recipient", R1), ("variant", v1)]),
                  Obj([("recipient", R2), ("variant", v2)])]),
        ("note_md", "two variants"),
    ])
    p = w("tmp/fpm_pre.json", canon(fpm_pre))
    sig = json.loads(zkk("sign", hexpriv(PRIV_A), p, "zikaron.fpm/1"))
    show("sign (zikaron.fpm/1)", json.dumps(sig, sort_keys=True).encode())
    fpm_bytes = canon(Obj(list(fpm_pre.pairs) + [("sig", sig["sig"])]))
    p_fpm = w("fpm.json", fpm_bytes)
    fpm_id = "0x" + sha256(fpm_bytes).hex()
    check("fpm-check accepts", zkk("fpm-check", p_fpm),
          '{"doc_id":"%s","ok":true}' % fpm_id)

    ack_pre = Obj([("spec", "zikaron.ack/1"), ("recipient", R1),
                   ("fpm", fpm_id), ("variant", v1), ("note_md", "")])
    p = w("tmp/ack_pre.json", canon(ack_pre))
    sig = json.loads(zkk("sign", hexpriv(priv_r1), p, "zikaron.ack/1"))
    ack_bytes = canon(Obj(list(ack_pre.pairs) + [("sig", sig["sig"])]))
    p_ack = w("ack.json", ack_bytes)
    check("ack-check accepts", zkk("ack-check", p_ack),
          '{"doc_id":"0x%s","ok":true}' % sha256(ack_bytes).hex())

    bad = bytearray(fpm_bytes)
    bad[-4] = bad[-4] ^ 0x01 if bad[-4] not in (0x30, 0x31) else 0x39
    p_bad = w("fpm_bad.json", bytes(bad))
    show("fpm-check on a tampered signature", zkk("fpm-check", p_bad))

    ack2_pre = Obj([("spec", "zikaron.ack/1"), ("recipient", R1),
                    ("fpm", fpm_id), ("variant", v2), ("note_md", "")])
    p = w("tmp/ack2_pre.json", canon(ack2_pre))
    sig = json.loads(zkk("sign", hexpriv(priv_r1), p, "zikaron.ack/1"))
    p_ack2 = w("ack2.json",
               canon(Obj(list(ack2_pre.pairs) + [("sig", sig["sig"])])))
    p_row = w("fpm_row.json", canon(Obj(
        [(k, [Obj([("recipient", R1), ("variant", v1)]),
              Obj([("recipient", R2)])] if k == "rows" else val)
         for k, val in fpm_pre.pairs] + [("sig", sig["sig"])])))

    p_x1 = w("bytes/x1.bin", b"variant for R1")
    p_x2 = w("bytes/x2.bin", b"something else")
    check("pair", zkk("pair", p_fpm, p_ack),
          '{"recipient":"%s","variant":"%s","verdict":"PAIRED"}' % (R1, v1))
    check("attribute (the right bytes)", zkk("attribute", p_fpm, p_ack, p_x1),
          '{"attributed":true,"recipient":"%s","verdict":"ATTRIBUTED"}' % R1)
    check("attribute (other bytes)", zkk("attribute", p_fpm, p_ack, p_x2),
          '{"attributed":false,"verdict":"NOT_ATTRIBUTED"}')
    check("attribute after a pairing that is not PAIRED",
          zkk("attribute", p_fpm, p_ack2, p_x1),
          '{"attributed":false,"verdict":"ACK_VARIANT_MISMATCH"}')
    check("pair of a manifest that is no manifest",
          zkk("pair", p_bad, p_ack),
          '{"token":"E_SIG_SIGNER","verdict":"FPM_INVALID"}')
    check("fpm-check names the failing row", zkk("fpm-check", p_row),
          '{"index":1,"ok":false,"token":"E_FPM_ROW"}')

    check("pair (wrong variant)", zkk("pair", p_fpm, p_ack2),
          '{"verdict":"ACK_VARIANT_MISMATCH"}')

    # ------------------------------------------------------------------- s6
    print("s6 badge")
    payload = json.loads(zkk("badge-encode", p_grant_a, p_grant_b))["payload"]
    p_payload = w("badge.txt", payload.encode("ascii"))
    check("badge-decode", zkk("badge-decode", p_payload),
          '{"grants":["%s","%s"],"ok":true}' % (grant_a_id, grant_b_id))
    check("badge-encode of a non-grant",
          zkk("badge-encode", os.path.join(WORK, "entries/gen_a.zk1")),
          '{"index":0,"ok":false,"token":"E_BADGE_TYPE"}')
    p_rev = w("badge_rev.txt",
              payload.encode("ascii").replace(b"zikaron-grant:", b"", 1))
    check("badge-decode without the prefix", zkk("badge-decode", p_rev),
          '{"ok":false,"token":"E_BADGE_PREFIX"}')
    p_only_b = w("badge_b.txt",
                 json.loads(zkk("badge-encode",
                                p_grant_b))["payload"].encode("ascii"))
    check("badge-decode of a payload that states an upstream",
          zkk("badge-decode", p_only_b),
          '{"ok":false,"token":"E_BADGE_INCOMPLETE"}')
    p_b64 = w("badge_b64.txt", b"zikaron-grant:")
    check("badge-decode with an empty segment", zkk("badge-decode", p_b64),
          '{"index":0,"ok":false,"token":"E_BADGE_B64"}')
    p_b64b = w("badge_b64b.txt", b"zikaron-grant:AAA.AAAA")
    check("badge-decode whose first segment is not an entry",
          zkk("badge-decode", p_b64b),
          '{"index":0,"inner":"E_JSON","ok":false,"token":"E_BADGE_ENTRY"}')
    p_twice = w("badge_twice.txt", json.loads(
        zkk("badge-encode", p_grant_a, p_grant_a))["payload"].encode("ascii"))
    check("badge-decode of two segments with no byte link",
          zkk("badge-decode", p_twice),
          '{"index":1,"ok":false,"token":"E_BADGE_LINK"}')

    # ------------------------------------------------------------------- s7
    print("s7 disclosure kit")
    kit = os.path.join(WORK, "kit")
    os.makedirs(os.path.join(kit, "entries"))
    os.makedirs(os.path.join(kit, "files"))
    ids = sorted([gen_a_id, hist_a_id, grant_a_id])
    for eid, blob in ((gen_a_id, gen_a), (hist_a_id, hist_a),
                      (grant_a_id, grant_a)):
        with open(os.path.join(kit, "entries", eid[2:] + ".zk1"), "wb") as fh:
            fh.write(blob)
    with open(os.path.join(kit, "files", "work.bin"), "wb") as fh:
        fh.write(work_bytes)
    manifest = Obj([
        ("spec", "zikaron.kit/1"), ("root", A), ("entries", ids),
        ("files", [Obj([("path", "work.bin"), ("sha256", W),
                        ("size", len(work_bytes))])]),
        ("contents", [Obj([("content", W), ("path", "work.bin")])]),
        ("proofs", []), ("note_md", "the kit"),
    ])
    with open(os.path.join(kit, "manifest.json"), "wb") as fh:
        fh.write(canon(manifest))
    check("kit-verify", zkk("kit-verify", kit),
          '{"counts":{"entries":3,"files":1,"proofs":0},'
          '"invalid_entries":[],"kit_id":"0x%s","verdict":"KIT_OK"}'
          % sha256(canon(manifest)).hex())

    kit2 = os.path.join(WORK, "kit_extra")
    shutil.copytree(kit, kit2)
    with open(os.path.join(kit2, "aa-stray.bin"), "wb") as fh:
        fh.write(b"not named")
    check("kit-verify with a stray file", zkk("kit-verify", kit2),
          '{"subject":"aa-stray.bin","verdict":"E_KIT_EXTRA"}')

    kit3 = os.path.join(WORK, "kit_link")
    shutil.copytree(kit, kit3)
    os.symlink(os.path.join(kit3, "manifest.json"),
               os.path.join(kit3, "a-link.json"))
    check("kit-verify with a symbolic link", zkk("kit-verify", kit3),
          '{"subject":"a-link.json","verdict":"E_KIT_UNREADABLE"}')

    kit4 = os.path.join(WORK, "kit_bare")
    os.makedirs(kit4)
    empty = Obj([("spec", "zikaron.kit/1"), ("root", None), ("entries", []),
                 ("files", []), ("contents", []), ("proofs", []),
                 ("note_md", "")])
    with open(os.path.join(kit4, "manifest.json"), "wb") as fh:
        fh.write(canon(empty))
    check("kit-verify of a kit that proves nothing", zkk("kit-verify", kit4),
          '{"counts":{"entries":0,"files":0,"proofs":0},'
          '"invalid_entries":[],"kit_id":"0x%s","verdict":"KIT_OK"}'
          % sha256(canon(empty)).hex())

    kit5 = os.path.join(WORK, "kit_norm")
    os.makedirs(kit5)
    with open(os.path.join(kit5, "manifest.json"), "wb") as fh:
        fh.write(b'{"spec":"zikaron.kit/1"} ')
    check("kit-verify with a non-canonical manifest", zkk("kit-verify", kit5),
          '{"subject":"canonical","verdict":"E_KIT_MANIFEST"}')

    kit6 = os.path.join(WORK, "kit_absent")
    os.makedirs(kit6)
    check("kit-verify with no manifest", zkk("kit-verify", kit6),
          '{"verdict":"E_KIT_MANIFEST_ABSENT"}')

    kit7 = os.path.join(WORK, "kit_size")
    shutil.copytree(kit, kit7)
    with open(os.path.join(kit7, "manifest.json"), "wb") as fh:
        fh.write(canon(Obj([
            ("spec", "zikaron.kit/1"), ("root", A), ("entries", ids),
            ("files", [Obj([("path", "work.bin"), ("sha256", W),
                            ("size", len(work_bytes) + 1)])]),
            ("contents", [Obj([("content", W), ("path", "work.bin")])]),
            ("proofs", []), ("note_md", "")])))
    check("kit-verify with a file of another length", zkk("kit-verify", kit7),
          '{"subject":"work.bin","verdict":"E_KIT_FILE"}')

    kit8 = os.path.join(WORK, "kit_anchored_junk")
    os.makedirs(os.path.join(kit8, "entries"))
    junk = b"anchored bytes that are no entry"
    junk_id = "0x" + sha256(junk).hex()
    with open(os.path.join(kit8, "entries", junk_id[2:] + ".zk1"), "wb") as fh:
        fh.write(junk)
    man8 = canon(Obj([
        ("spec", "zikaron.kit/1"), ("root", A), ("entries", [junk_id]),
        ("files", []), ("contents", []), ("proofs", []),
        ("note_md", "a MISSING row asks to see these bytes")]))
    with open(os.path.join(kit8, "manifest.json"), "wb") as fh:
        fh.write(man8)
    check("kit-verify listing an anchored byte string that is no entry",
          zkk("kit-verify", kit8),
          '{"counts":{"entries":1,"files":0,"proofs":0},'
          '"invalid_entries":[{"entry_id":"%s","token":"E_JSON"}],'
          '"kit_id":"0x%s","verdict":"KIT_OK"}'
          % (junk_id, sha256(man8).hex()))

    kit9 = os.path.join(WORK, "kit_entry_bytes")
    os.makedirs(kit9)
    with open(os.path.join(kit9, "manifest.json"), "wb") as fh:
        fh.write(canon(Obj([
            ("spec", "zikaron.kit/1"), ("root", A), ("entries", [grant_a_id]),
            ("files", []), ("contents", []), ("proofs", []),
            ("note_md", "")])))
    check("kit-verify whose named entry file is absent", zkk("kit-verify", kit9),
          '{"subject":"%s","verdict":"E_KIT_ENTRY_BYTES"}' % grant_a_id)

    kit10 = os.path.join(WORK, "kit_rule")
    os.makedirs(kit10)
    with open(os.path.join(kit10, "manifest.json"), "wb") as fh:
        fh.write(canon(Obj([
            ("spec", "zikaron.kit/1"), ("root", A),
            ("entries", list(reversed(sorted([hist_a_id, gen_a_id])))),
            ("files", []), ("contents", []), ("proofs", []),
            ("note_md", "")])))
    check("kit-verify with unsorted entries", zkk("kit-verify", kit10),
          '{"subject":"entries","verdict":"E_KIT_MANIFEST"}')

    # ------------------------------------------------------------------- s9
    print("s9 depth")
    show("depth on the work digest", zkk("depth", p_audit_a, W))
    show("depth on a digest the ledger never carried",
         zkk("depth", p_audit_a, "0x" + "00" * 32))
    p_broken = w("audit_broken.json", b"{not json")
    check("depth on an invalid audit input", zkk("depth", p_broken, W),
          '{"valid":false}')

    # ------------------------------------------------------------------ s10
    print("s10 grant check")
    show("grant-check with no audit input and no clock",
         zkk("grant-check", p_grant_a))
    show("grant-check under the issuer's ledger",
         zkk("grant-check", p_grant_a, "--audit", p_audit_a,
             "--now", "1700000000"))
    show("grant-check outside the window",
         zkk("grant-check", p_grant_a, "--audit", p_audit_a, "--now", "999"))
    show("grant-check of a byte string that is not an entry",
         zkk("grant-check", w("junk.bin", b"not an entry"),
             "--audit", p_audit_a))
    show("grant-check under a ledger that is not the issuer's",
         zkk("grant-check", p_grant_a, "--audit", p_audit_b,
             "--now", "1700000000"))

    rev_a, rev_a_id = sign_entry(PRIV_A, envelope(
        "revocation", A, 3, grant_a_id, Obj([("grant", grant_a_id)])))
    audit_rev = dict(audit_a)
    audit_rev["pile"] = audit_a["pile"] + ["0x" + rev_a.hex()]
    audit_rev["anchors"] = audit_a["anchors"] + [
        {"chainId": 1, "blockNumber": 14, "blockTimestamp": 1700000200,
         "tx": "0x" + "03" * 32, "sender": A, "hash": rev_a_id,
         "verdict": "counted"}]
    p_audit_rev = w("audit_rev.json", json.dumps(audit_rev).encode("ascii"))
    show("grant-check after a revocation",
         zkk("grant-check", p_grant_a, "--audit", p_audit_rev,
             "--now", "1700000000"))

    print("s10.5 chain check")
    hops = [{"grant": p_grant_a, "audit": p_audit_a},
            {"grant": p_grant_b, "audit": p_audit_b}]
    p_hops = w("hops.json", json.dumps(hops).encode("ascii"))
    show("chain-check over both hops",
         zkk("chain-check", p_hops, "--now", "1700000000"))
    p_hops_rev = w("hops_rev.json", json.dumps(
        [{"grant": p_grant_b, "audit": p_audit_b},
         {"grant": p_grant_a, "audit": p_audit_a}]).encode("ascii"))
    show("chain-check whose first grant states an upstream",
         zkk("chain-check", p_hops_rev, "--now", "1700000000"))
    p_hops_empty = w("hops_empty.json", b"[]")
    check("chain-check of an empty list", zkk("chain-check", p_hops_empty),
          '{"failing":{"index":0,"kind":"empty"},"hops":[],"links":[],'
          '"token":"CHAIN_EMPTY","verdict":"FAIL"}')

    print()
    if FAILURES:
        print("%d self-test expectation(s) failed: %s"
              % (len(FAILURES), ", ".join(FAILURES)))
        return 1
    print("every command ran; every pinned expectation held")
    return 0


if __name__ == "__main__":
    sys.exit(main())
