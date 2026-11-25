#!/usr/bin/env python3
"""zkk2: a zikaron.kit/2 implementation in pure Python 3.

The command-line contract of zikaron-conformance/HARNESS-KIT2.md:

    zkk2 badge-encode  <entry-path>...
    zkk2 badge-decode  <path>
    zkk2 kit-verify    <path>
    zkk2 grant-package <path>
    zkk2 grant-check   <grant-path> [--audit <audit-input.json>]
                       [--notices <notice-input.json>] [--now <int>]
    zkk2 chain-check   <hops.json> [--now <int>]

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

from zkcanon import Obj, canon                                       # noqa: E402
import zk2audit                                                      # noqa: E402
import zkk2doc                                                       # noqa: E402
import zkk2kit                                                       # noqa: E402
import zkk2read                                                      # noqa: E402
from zkk2read import NoLabel, NoNotice                               # noqa: E402

MAX_INT = (1 << 53) - 1


def emit(v):
    sys.stdout.buffer.write(canon(v))
    sys.stdout.buffer.flush()


def die(msg):
    sys.stderr.write(msg + "\n")
    sys.exit(2)


def read_file(path):
    """HARNESS-KIT2.md: a regular file or a character device is read in full; anything
    else the path resolves to, or no file at all, is a path the command cannot read."""
    try:
        st = os.stat(path)
        if not (stat.S_ISREG(st.st_mode) or stat.S_ISCHR(st.st_mode)):
            die("zkk2: cannot read %s: not a regular file" % path)
        with open(path, "rb") as fh:
            return fh.read()
    except (OSError, ValueError, UnicodeError) as exc:
        die("zkk2: cannot read %s: %s" % (path, exc))


def load_view_or_none(path, extra_pile=None):
    raw = read_file(path)
    try:
        return zkk2read.load_view(raw, extra_pile=extra_pile)
    except NoLabel:
        return None


def load_notices_or_none(path):
    raw = read_file(path)
    try:
        return zkk2read.load_notices(raw)
    except NoNotice:
        return None


def cmd_badge_encode(argv):
    if len(argv) < 1:
        die("usage: zkk2 badge-encode <entry-path>...")
    blobs = [read_file(p) for p in argv]
    token, a, b = zkk2doc.badge_encode(blobs)
    if token == "BADGE":
        emit(Obj([("payload", a.decode("ascii"))]))
        return
    pairs = [("ok", False), ("token", token)]
    if token in ("E_BADGE_ENTRY", "E_BADGE_TYPE"):
        pairs.append(("index", a))
    emit(Obj(pairs))


def cmd_badge_decode(argv):
    if len(argv) != 1:
        die("usage: zkk2 badge-decode <path>")
    token, a, b = zkk2doc.badge_decode(read_file(argv[0]))
    if token == "BADGE_OK":
        emit(Obj([("ok", True), ("grants", [e.eid for e in a])]))
        return
    pairs = [("ok", False), ("token", token)]
    if token in ("E_BADGE_B64", "E_BADGE_ENTRY", "E_BADGE_TYPE", "E_BADGE_LINK"):
        pairs.append(("index", a))
    if token == "E_BADGE_ENTRY":
        pairs.append(("inner", b))
    emit(Obj(pairs))


def enumeration_of(path):
    """A directory (s4.1 walk) or a regular file (s4.1a); else misuse.
    Returns the enumeration or raises zkk2kit.Unreadable."""
    if os.path.isdir(path):
        return zkk2kit.walk(path)
    if os.path.isfile(path):
        try:
            with open(path, "rb") as fh:
                data = fh.read()
        except OSError:
            raise zkk2kit.Unreadable(".")
        return zkk2kit.parse_onefile(data)
    die("zkk2: not a directory or a regular file: %s" % path)


def cmd_kit_verify(argv):
    if len(argv) != 1:
        die("usage: zkk2 kit-verify <path>")
    try:
        enumeration = enumeration_of(argv[0])
    except zkk2kit.Unreadable as u:
        emit(Obj([("verdict", "E_KIT_UNREADABLE"), ("subject", u.subject)]))
        return
    verdict, subject, ok, _man = zkk2kit.verify_kit(enumeration)
    if verdict == "KIT_OK":
        kit_id, n_entries, n_files, invalid = ok
        emit(Obj([
            ("verdict", "KIT_OK"),
            ("kit_id", kit_id),
            ("counts", Obj([("entries", n_entries), ("files", n_files)])),
            ("invalid_entries", [Obj([("entry_id", eid), ("token", tok)]) for eid, tok in invalid]),
        ]))
        return
    if subject is None:
        emit(Obj([("verdict", verdict)]))
    else:
        emit(Obj([("verdict", verdict), ("subject", subject)]))


def cmd_grant_package(argv):
    if len(argv) != 1:
        die("usage: zkk2 grant-package <path>")
    try:
        enumeration = enumeration_of(argv[0])
    except zkk2kit.Unreadable as u:
        emit(Obj([("verdict", "E_KIT_UNREADABLE"), ("subject", u.subject)]))
        return
    verdict, payload = zkk2kit.verify_grant_package(enumeration)
    if verdict == "GP_OK":
        emit(payload)
    else:
        emit(Obj([("verdict", verdict), ("subject", payload)]))


def _parse_now(s):
    if not s or not all(c in "0123456789" for c in s) or (len(s) > 1 and s[0] == "0"):
        die("zkk2: --now takes a zikaron/2 int")
    n = int(s, 10)
    if n > MAX_INT:
        die("zkk2: --now is outside the integer universe")
    return n


def cmd_grant_check(argv):
    usage = "usage: zkk2 grant-check <grant-path> [--audit <f>] [--notices <f>] [--now <int>]"
    if len(argv) < 1:
        die(usage)
    path = argv[0]
    audit_path = None
    notices_path = None
    now = None
    i = 1
    while i < len(argv):
        if argv[i] == "--audit" and i + 1 < len(argv) and audit_path is None:
            audit_path = argv[i + 1]
            i += 2
        elif argv[i] == "--notices" and i + 1 < len(argv) and notices_path is None:
            notices_path = argv[i + 1]
            i += 2
        elif argv[i] == "--now" and i + 1 < len(argv) and now is None:
            now = _parse_now(argv[i + 1])
            i += 2
        else:
            die(usage)
    g = read_file(path)
    view = None
    if audit_path is not None:
        view = load_view_or_none(audit_path, extra_pile=g)
    notices = None
    if notices_path is not None:
        notices = load_notices_or_none(notices_path)
    result, _entry = zkk2read.grant_check(g, view, notices, now)
    emit(result)


def cmd_chain_check(argv):
    if len(argv) < 1:
        die("usage: zkk2 chain-check <hops.json> [--now <int>]")
    now = None
    i = 1
    while i < len(argv):
        if argv[i] == "--now" and i + 1 < len(argv) and now is None:
            now = _parse_now(argv[i + 1])
            i += 2
        else:
            die("usage: zkk2 chain-check <hops.json> [--now <int>]")
    raw = read_file(argv[0])
    try:
        spec = zk2audit.load_input(raw)   # the audit-input reader (HARNESS-KIT2.md), lone surrogates refused
    except NoLabel:
        die("zkk2: %s is not readable JSON" % argv[0])
    if not isinstance(spec, list):
        die("zkk2: <hops.json> must be an array")
    hops = []
    for h in spec:
        hd = h.dict() if isinstance(h, Obj) else None
        if (hd is None or set(hd.keys()) != {"grant", "audit", "notices"}
                or not isinstance(hd["grant"], str)
                or not (hd["audit"] is None or isinstance(hd["audit"], str))
                or not (hd["notices"] is None or isinstance(hd["notices"], str))):
            die("zkk2: each hop is {\"grant\":<path>,\"audit\":<path>|null,\"notices\":<path>|null}")
        g = read_file(hd["grant"])
        view = None if hd["audit"] is None else load_view_or_none(hd["audit"], extra_pile=g)
        notices = None if hd["notices"] is None else load_notices_or_none(hd["notices"])
        hops.append((g, view, notices))
    emit(zkk2read.chain_check(hops, now))


COMMANDS = {
    "badge-encode": cmd_badge_encode,
    "badge-decode": cmd_badge_decode,
    "kit-verify": cmd_kit_verify,
    "grant-package": cmd_grant_package,
    "grant-check": cmd_grant_check,
    "chain-check": cmd_chain_check,
}


def main(argv):
    if len(argv) < 2:
        die("usage: zkk2 {%s} ..." % "|".join(sorted(COMMANDS)))
    fn = COMMANDS.get(argv[1])
    if fn is None:
        die("zkk2: unknown command %r" % argv[1])
    fn(argv[2:])
    return 0


if __name__ == "__main__":
    sys.setrecursionlimit(10000)
    sys.exit(main(sys.argv))
