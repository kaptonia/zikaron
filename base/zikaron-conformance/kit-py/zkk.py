#!/usr/bin/env python3
"""zkk: a zikaron.kit/1 implementation in pure Python 3.

The command-line contract of zikaron-conformance/HARNESS-KIT.md:

    zkk fpm-check    <path>
    zkk ack-check    <path>
    zkk sign         <privkey-hex> <path> <domain>
    zkk pair         <fpm-path> <ack-path>
    zkk attribute    <fpm-path> <ack-path> <bytes-path>
    zkk badge-encode <entry-path>...
    zkk badge-decode <path>
    zkk kit-verify   <dir>
    zkk depth        <audit-input.json> <work-hex32>
    zkk grant-check  <grant-path> [--audit <audit-input.json>] [--now <int>]
    zkk chain-check  <hops.json> [--now <int>]

All output is one canonical JSON value (zikaron/1 s3.4) on stdout with no
trailing newline.  Exit 0 whenever an answer was produced, 2 for harness
misuse.  The zikaron/1 core of ../impl-py is embedded and decides every
parent predicate.
"""

import json
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.join(os.path.dirname(_HERE), "impl-py"))

from zkcanon import Fault, Obj, canon, parse15                    # noqa: E402
from zkcrypto import (N, address_of, eip191_digest, message,      # noqa: E402
                      pubkey, sha256, sign_digest)
import zkkdoc                                                     # noqa: E402
import zkkkit                                                     # noqa: E402
import zkkread                                                    # noqa: E402
from zkkread import NoLabel                                       # noqa: E402

MAX_INT = (1 << 53) - 1


def emit(v):
    sys.stdout.buffer.write(canon(v))
    sys.stdout.buffer.flush()


def die(msg):
    sys.stderr.write(msg + "\n")
    sys.exit(2)


def read_file(path):
    try:
        with open(path, "rb") as fh:
            return fh.read()
    except OSError as exc:
        die("zkk: cannot read %s: %s" % (path, exc))


def is_hex32_str(s):
    if not isinstance(s, str) or len(s) != 66 or not s.startswith("0x"):
        return False
    for c in s[2:]:
        if c not in "0123456789abcdef":
            return False
    return True


def load_view_or_none(path, extra_pile=None):
    """The audit of the input at `path` (with `extra_pile` appended to its
    pile), or None when the input is invalid under zikaron/1 s9.4."""
    raw = read_file(path)
    try:
        return zkkread.load_view(raw, extra_pile=extra_pile)
    except NoLabel:
        return None


# --------------------------------------------------------------------------
# Documents
# --------------------------------------------------------------------------

def cmd_fpm_check(argv):
    if len(argv) != 1:
        die("usage: zkk fpm-check <path>")
    b = read_file(argv[0])
    try:
        m = zkkdoc.fpm_accept(b)
    except Fault as f:
        pairs = [("ok", False), ("token", f.token)]
        if isinstance(f, zkkdoc.RowFault):
            pairs.append(("index", f.index))
        emit(Obj(pairs))
        return
    emit(Obj([("doc_id", m.doc_id), ("ok", True)]))


def cmd_ack_check(argv):
    if len(argv) != 1:
        die("usage: zkk ack-check <path>")
    b = read_file(argv[0])
    try:
        a = zkkdoc.ack_accept(b)
    except Fault as f:
        emit(Obj([("ok", False), ("token", f.token)]))
        return
    emit(Obj([("doc_id", a.doc_id), ("ok", True)]))


def cmd_sign(argv):
    if len(argv) != 3:
        die("usage: zkk sign <privkey-hex> <path> <domain>")
    key = argv[0]
    if key.startswith("0x") or key.startswith("0X"):
        key = key[2:]
    if len(key) != 64 or not all(c in "0123456789abcdefABCDEF" for c in key):
        die("zkk: bad private key")
    priv = int(key, 16)
    if not (1 <= priv <= N - 1):
        die("zkk: private key out of range")
    domain = argv[2]
    if domain not in zkkdoc.DOMAINS:
        die("zkk: domain must be one of %s" % (", ".join(zkkdoc.DOMAINS),))
    b = read_file(argv[1])
    try:
        v = parse15(b)
    except Fault as f:
        die("zkk: preimage file is not a readable value (%s)" % f.token)
    if not isinstance(v, Obj):
        die("zkk: preimage is not an object")
    pre = sha256(canon(v))
    digest = eip191_digest(message(domain, pre))
    r, s, vv = sign_digest(priv, digest)
    sig = r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([vv])
    emit(Obj([
        ("digest", "0x" + digest.hex()),
        ("presig", "0x" + pre.hex()),
        ("sig", "0x" + sig.hex()),
        ("signer", "0x" + address_of(pubkey(priv)).hex()),
    ]))


def cmd_pair(argv):
    if len(argv) != 2:
        die("usage: zkk pair <fpm-path> <ack-path>")
    m = read_file(argv[0])
    a = read_file(argv[1])
    verdict, token, row = zkkdoc.pair(m, a)
    if verdict == "PAIRED":
        emit(Obj([("verdict", "PAIRED"), ("recipient", row[0]),
                  ("variant", row[1])]))
    elif verdict in ("FPM_INVALID", "ACK_INVALID"):
        emit(Obj([("verdict", verdict), ("token", token)]))
    else:
        emit(Obj([("verdict", verdict)]))


def cmd_attribute(argv):
    if len(argv) != 3:
        die("usage: zkk attribute <fpm-path> <ack-path> <bytes-path>")
    m = read_file(argv[0])
    a = read_file(argv[1])
    x = read_file(argv[2])
    verdict, _token, row = zkkdoc.attribute(m, a, x)
    if verdict == "ATTRIBUTED":
        emit(Obj([("attributed", True), ("recipient", row[0]),
                  ("verdict", "ATTRIBUTED")]))
    else:
        emit(Obj([("attributed", False), ("verdict", verdict)]))


# --------------------------------------------------------------------------
# Badge
# --------------------------------------------------------------------------

def cmd_badge_encode(argv):
    if len(argv) < 1:
        die("usage: zkk badge-encode <entry-path>...")
    blobs = [read_file(p) for p in argv]
    token, a, b = zkkdoc.badge_encode(blobs)
    if token == "BADGE":
        emit(Obj([("payload", a.decode("ascii"))]))
        return
    pairs = [("ok", False), ("token", token)]
    if token in ("E_BADGE_ENTRY", "E_BADGE_TYPE"):
        pairs.append(("index", a))
    emit(Obj(pairs))


def cmd_badge_decode(argv):
    if len(argv) != 1:
        die("usage: zkk badge-decode <path>")
    p = read_file(argv[0])
    token, a, b = zkkdoc.badge_decode(p)
    if token == "BADGE_OK":
        emit(Obj([("ok", True), ("grants", [e.eid for e in a])]))
        return
    pairs = [("ok", False), ("token", token)]
    if token in ("E_BADGE_B64", "E_BADGE_ENTRY", "E_BADGE_TYPE",
                 "E_BADGE_LINK"):
        pairs.append(("index", a))
    if token == "E_BADGE_ENTRY":
        pairs.append(("inner", b))
    emit(Obj(pairs))


# --------------------------------------------------------------------------
# Disclosure kit
# --------------------------------------------------------------------------

def cmd_kit_verify(argv):
    if len(argv) != 1:
        die("usage: zkk kit-verify <dir>")
    if not os.path.isdir(argv[0]):
        die("zkk: not a directory: %s" % argv[0])
    try:
        enumeration = zkkkit.walk(argv[0])
    except zkkkit.Unreadable as u:
        emit(Obj([("verdict", "E_KIT_UNREADABLE"),
                  ("subject", zkkkit.show(u.path))]))
        return
    verdict, subject, ok = zkkkit.verify_kit(enumeration)
    if verdict == "KIT_OK":
        kit_id, n_entries, n_files, n_proofs, invalid = ok
        emit(Obj([
            ("verdict", "KIT_OK"),
            ("kit_id", kit_id),
            ("counts", Obj([("entries", n_entries), ("files", n_files),
                            ("proofs", n_proofs)])),
            ("invalid_entries", [Obj([("entry_id", eid), ("token", tok)])
                                 for eid, tok in invalid]),
        ]))
        return
    if subject is None:
        emit(Obj([("verdict", verdict)]))
    else:
        emit(Obj([("verdict", verdict), ("subject", subject)]))


# --------------------------------------------------------------------------
# Depth
# --------------------------------------------------------------------------

def cmd_depth(argv):
    if len(argv) != 2:
        die("usage: zkk depth <audit-input.json> <work-hex32>")
    if not is_hex32_str(argv[1]):
        die("zkk: <work-hex32> is not hex32")
    view = load_view_or_none(argv[0])
    emit(zkkread.depth(view, argv[1]))


# --------------------------------------------------------------------------
# Grant check
# --------------------------------------------------------------------------

def _parse_now(s):
    if not s or not all(c in "0123456789" for c in s) or (len(s) > 1 and s[0] == "0"):
        die("zkk: --now takes a zikaron/1 int (decimal digits, no leading zero)")
    n = int(s, 10)
    if n > MAX_INT:
        die("zkk: --now is outside the zikaron/1 integer universe")
    return n


def cmd_grant_check(argv):
    if len(argv) < 1:
        die("usage: zkk grant-check <grant-path> [--audit <f>] [--now <int>]")
    path = argv[0]
    audit_path = None
    now = None
    i = 1
    while i < len(argv):
        if argv[i] == "--audit" and i + 1 < len(argv) and audit_path is None:
            audit_path = argv[i + 1]
            i += 2
        elif argv[i] == "--now" and i + 1 < len(argv) and now is None:
            now = _parse_now(argv[i + 1])
            i += 2
        else:
            # a repeated option is an argument beyond the command's list
            die("usage: zkk grant-check <grant-path> [--audit <f>] [--now <int>]")
    g = read_file(path)
    view = None
    if audit_path is not None:
        view = load_view_or_none(audit_path, extra_pile=g)
    result, _entry = zkkread.grant_check(g, view, now)
    emit(result)


def cmd_chain_check(argv):
    if len(argv) < 1:
        die("usage: zkk chain-check <hops.json> [--now <int>]")
    now = None
    i = 1
    while i < len(argv):
        if argv[i] == "--now" and i + 1 < len(argv) and now is None:
            now = _parse_now(argv[i + 1])
            i += 2
        else:
            die("usage: zkk chain-check <hops.json> [--now <int>]")
    raw = read_file(argv[0])
    try:
        def _no_dup(pairs):
            d = {}
            for k, v in pairs:
                if k in d:
                    raise ValueError("repeated member name")
                d[k] = v
            return d
        spec = json.loads(raw.decode("utf-8"), object_pairs_hook=_no_dup)
    except Exception:
        die("zkk: %s is not readable JSON" % argv[0])
    if not isinstance(spec, list):
        die("zkk: <hops.json> must be an array")
    hops = []
    for h in spec:
        if (not isinstance(h, dict) or set(h.keys()) != {"grant", "audit"}
                or not isinstance(h["grant"], str)
                or not (h["audit"] is None or isinstance(h["audit"], str))):
            die("zkk: each hop is {\"grant\":<path>,\"audit\":<path>|null}")
        g = read_file(h["grant"])
        ap = h["audit"]
        view = None
        if ap is not None:
            view = load_view_or_none(ap, extra_pile=g)
        hops.append((g, view))
    emit(zkkread.chain_check(hops, now))


# --------------------------------------------------------------------------

COMMANDS = {
    "fpm-check": cmd_fpm_check,
    "ack-check": cmd_ack_check,
    "sign": cmd_sign,
    "pair": cmd_pair,
    "attribute": cmd_attribute,
    "badge-encode": cmd_badge_encode,
    "badge-decode": cmd_badge_decode,
    "kit-verify": cmd_kit_verify,
    "depth": cmd_depth,
    "grant-check": cmd_grant_check,
    "chain-check": cmd_chain_check,
}


def main(argv):
    if len(argv) < 2:
        die("usage: zkk {%s} ..." % "|".join(sorted(COMMANDS)))
    fn = COMMANDS.get(argv[1])
    if fn is None:
        die("zkk: unknown command %r" % argv[1])
    fn(argv[2:])
    return 0


if __name__ == "__main__":
    sys.setrecursionlimit(10000)
    sys.exit(main(sys.argv))
