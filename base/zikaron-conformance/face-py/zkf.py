#!/usr/bin/env python3
"""zkf: a zikaron.face/1 implementation in pure Python 3.

The command-line contract of zikaron-conformance/HARNESS-FACE.md:

    zkf entry   <entry-path> <root>
    zkf declare <root> <address> <role> [<privkey-hex>]
    zkf carried <calldata-path>
    zkf table   <face-input.json>
    zkf joined  <pair.json>

All output is one canonical JSON value (zikaron/2 s3.4) on stdout with no
trailing newline.  Exit 0 whenever an answer was produced, 2 for harness
misuse.  The zikaron/2 core of ../impl2-py (over the zikaron/1 core of
../impl-py) is embedded and decides every parent predicate.
"""

import os
import stat
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
_UP = os.path.dirname(_HERE)
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.join(_UP, "impl2-py"))
sys.path.insert(0, os.path.join(_UP, "impl-py"))

from zkcanon import Fault, Obj, canon                                # noqa: E402
from zkcrypto import N, address_of, pubkey, sign_digest              # noqa: E402
from zkentry import is_hex                                           # noqa: E402
import zkfentry                                                      # noqa: E402
import zkftable                                                      # noqa: E402


def emit(v):
    sys.stdout.buffer.write(canon(v))
    sys.stdout.buffer.flush()


def die(msg):
    sys.stderr.write(msg + "\n")
    sys.exit(2)


def read_file(path):
    """HARNESS-FACE.md: a regular file or a character device is read in full;
    anything else the path resolves to, or no file at all, is a path the
    command cannot read."""
    try:
        st = os.stat(path)
        if not (stat.S_ISREG(st.st_mode) or stat.S_ISCHR(st.st_mode)):
            die("zkf: cannot read %s: not a regular file" % path)
        with open(path, "rb") as fh:
            return fh.read()
    except (OSError, ValueError, UnicodeError) as exc:
        die("zkf: cannot read %s: %s" % (path, exc))


def cmd_entry(argv):
    if len(argv) != 2:
        die("usage: zkf entry <entry-path> <root>")
    root = argv[1]
    if not is_hex(root, 20):
        die("zkf: <root> is a hex20")
    b = read_file(argv[0])
    try:
        e = zkfentry.accept(b)
    except Fault as f:
        emit(Obj([("ok", False), ("token", f.token)]))
        return
    if e.etype != zkfentry.TYPE:
        emit(Obj([("ok", True), ("entry_id", e.eid), ("face", False)]))
        return
    shape = zkfentry.shape_of(e.body)
    rooted = consent = None
    if shape is not None:
        d = e.body.dict()
        rooted = d["root"] == root
        if shape == "key":
            consent = zkfentry.consent_holds(d)
    emit(Obj([("ok", True), ("entry_id", e.eid), ("face", True),
              ("shape", shape), ("rooted", rooted), ("consent", consent)]))


def cmd_declare(argv):
    if len(argv) not in (3, 4):
        die("usage: zkf declare <root> <address> <role> [<privkey-hex>]")
    root, address, role = argv[0], argv[1], argv[2]
    if not is_hex(root, 20) or not is_hex(address, 20):
        die("zkf: <root> and <address> are hex20s")
    if not zkfentry.is_label(role):
        die("zkf: <role> is a label")
    decl = zkfentry.declaration(root, address, role)
    digest = zkfentry.digest_of(decl)
    pairs = [("declaration", "0x" + decl.hex()),
             ("message", "0x" + zkfentry.message_of(decl).hex()),
             ("digest", "0x" + digest.hex())]
    if len(argv) == 4:
        key = argv[3]
        if key.startswith("0x") or key.startswith("0X"):
            key = key[2:]
        if len(key) != 64 or any(c not in "0123456789abcdefABCDEF" for c in key):
            die("zkf: bad private key")
        priv = int(key, 16)
        if not (1 <= priv <= N - 1):
            die("zkf: private key out of range")
        r, s, v = sign_digest(priv, digest)
        sig = r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([v])
        pairs.append(("sig", "0x" + sig.hex()))
        pairs.append(("signer", "0x" + address_of(pubkey(priv)).hex()))
    emit(Obj(pairs))


def cmd_carried(argv):
    if len(argv) != 1:
        die("usage: zkf carried <calldata-path>")
    e = zkfentry.carried(read_file(argv[0]))
    emit(Obj([("carried", None if e is None else e.eid)]))


def cmd_table(argv):
    if len(argv) != 1:
        die("usage: zkf table <face-input.json>")
    raw = read_file(argv[0])
    try:
        inp = zkftable.load(raw)
    except zkftable.NoTable:
        emit(Obj([("ok", False), ("reason", "NO_TABLE")]))
        return
    emit(zkftable.read(inp))


def cmd_joined(argv):
    if len(argv) != 1:
        die("usage: zkf joined <pair.json>")
    raw = read_file(argv[0])
    try:
        first, second = zkftable.load_pair(raw)
    except zkftable.NoTable:
        emit(Obj([("ok", False), ("reason", "NO_TABLE")]))
        return
    emit(Obj([("joined", zkftable.joined(zkftable.read(first), zkftable.read(second)))]))


COMMANDS = {
    "entry": cmd_entry,
    "declare": cmd_declare,
    "carried": cmd_carried,
    "table": cmd_table,
    "joined": cmd_joined,
}


def main(argv):
    if len(argv) < 2:
        die("usage: zkf {%s} ..." % "|".join(sorted(COMMANDS)))
    fn = COMMANDS.get(argv[1])
    if fn is None:
        die("zkf: unknown command %r" % argv[1])
    fn(argv[2:])
    return 0


if __name__ == "__main__":
    sys.setrecursionlimit(10000)
    sys.exit(main(sys.argv))
