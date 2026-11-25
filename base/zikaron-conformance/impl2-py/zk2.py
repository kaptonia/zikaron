#!/usr/bin/env python3
"""zk2: a zikaron/2 implementation in pure Python 3.

Command-line contract per HARNESS2.md:
    zk2 check <path>
    zk2 canon <path>
    zk2 sign  <privkey-hex> <path> [domain]
    zk2 audit <path>

The zikaron/1 core of ../impl-py supplies the canonical form, the curve, and
the audit of a continued ledger's old segment (zikaron/2 s12.6).
"""

import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.join(os.path.dirname(_HERE), "impl-py"))

from zkcanon import Fault, Obj, canon, parse15                      # noqa: E402
from zkcrypto import (N, address_of, eip191_digest, message, pubkey,  # noqa: E402
                      sha256, sign_digest)
from zk2entry import DOMAIN_ENTRY, validate                         # noqa: E402
import zk2audit                                                     # noqa: E402


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
        die("zk2: cannot read %s: %s" % (path, exc))


def cmd_check(argv):
    if len(argv) != 1:
        die("usage: zk2 check <path>")
    b = read_file(argv[0])
    try:
        validate(b)
    except Fault as f:
        emit(Obj([("ok", False), ("token", f.token)]))
        return
    emit(Obj([("entry_id", "0x" + sha256(b).hex()), ("ok", True)]))


def cmd_canon(argv):
    if len(argv) != 1:
        die("usage: zk2 canon <path>")
    b = read_file(argv[0])
    try:
        v = parse15(b)
    except Fault as f:
        emit(Obj([("ok", False), ("token", f.token)]))
        return
    emit(Obj([("canon", "0x" + canon(v).hex()), ("ok", True)]))


def cmd_sign(argv):
    if len(argv) not in (2, 3):
        die("usage: zk2 sign <privkey-hex> <path> [domain]")
    key = argv[0]
    if key.startswith("0x") or key.startswith("0X"):
        key = key[2:]
    if len(key) != 64 or any(c not in "0123456789abcdefABCDEF" for c in key):
        die("zk2: bad private key")
    priv = int(key, 16)
    if not (1 <= priv <= N - 1):
        die("zk2: private key out of range")
    domain = argv[2] if len(argv) == 3 else DOMAIN_ENTRY
    if not domain.isascii() or "\n" in domain:
        die("zk2: a domain literal is ASCII and carries no line feed")
    b = read_file(argv[1])
    try:
        v = parse15(b)
    except Fault as f:
        die("zk2: preimage file is not a readable value (%s)" % f.token)
    if not isinstance(v, Obj):
        die("zk2: preimage is not an object")
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


def cmd_audit(argv):
    if len(argv) != 1:
        die("usage: zk2 audit <path>")
    raw = read_file(argv[0])
    try:
        rep = zk2audit.audit(raw)
    except zk2audit.NoLabel:
        emit(Obj([("ok", False), ("reason", "NO_LABEL")]))
        return
    emit(rep)


def main(argv):
    if len(argv) < 2:
        die("usage: zk2 {check|canon|sign|audit} ...")
    cmd = argv[1]
    rest = argv[2:]
    if cmd == "check":
        cmd_check(rest)
    elif cmd == "canon":
        cmd_canon(rest)
    elif cmd == "sign":
        cmd_sign(rest)
    elif cmd == "audit":
        cmd_audit(rest)
    else:
        die("zk2: unknown command %r" % cmd)
    return 0


if __name__ == "__main__":
    sys.setrecursionlimit(10000)
    sys.exit(main(sys.argv))
