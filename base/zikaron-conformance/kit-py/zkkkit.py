"""zikaron.kit/1 section 7: the disclosure kit.

The walk of s7.1, the kit paths of s7.2, the manifest of s7.3, and the
verification of s7.4.
"""

import os
import stat

from zkcanon import Fault, Obj, accept as canonical
from zkcrypto import sha256
from zkentry import is_hex, is_int, is_prose, validate

SPEC_KIT = "zikaron.kit/1"

MANIFEST_PATH = b"manifest.json"
MANIFEST_KEYS = ("contents", "entries", "files", "note_md", "proofs", "root",
                 "spec")
MANIFEST_KEYSET = frozenset(MANIFEST_KEYS)

FILE_KEYSET = frozenset(("path", "sha256", "size"))
CONTENT_KEYSET = frozenset(("content", "path"))
PROOF_KEYSET = frozenset(("path", "sha256", "tx"))


def show(path_bytes):
    """A relative path as a JSON string.  Kit paths are lowercase ASCII
    (s7.2); a byte string a filesystem yielded that is not valid UTF-8 is
    rendered with replacement characters and can never be in the named set."""
    return path_bytes.decode("utf-8", "replace")


# --------------------------------------------------------------------------
# s7.1 the walk
# --------------------------------------------------------------------------

class Unreadable(Exception):
    def __init__(self, path_bytes):
        super().__init__(path_bytes)
        self.path = path_bytes


def _walk_dir(root, rel, out):
    """Walk one directory; raises Unreadable at the first entry that fails."""
    full = os.path.join(root, rel) if rel else root
    try:
        names = os.listdir(full)
    except OSError:
        # s7.1: the kit directory itself is named `.`
        raise Unreadable(rel if rel else b".")
    names = [n for n in names if n != b"." and n != b".."]
    names.sort()
    for name in names:
        r = (rel + b"/" + name) if rel else name
        f = os.path.join(root, r)
        try:
            st = os.lstat(f)
        except OSError:
            raise Unreadable(r)
        mode = st.st_mode
        if stat.S_ISLNK(mode):
            raise Unreadable(r)
        if stat.S_ISREG(mode):
            try:
                with open(f, "rb") as fh:
                    out[r] = fh.read()
            except OSError:
                raise Unreadable(r)
        elif stat.S_ISDIR(mode):
            _walk_dir(root, r, out)
        else:
            raise Unreadable(r)


def walk(dirpath):
    """s7.1.  Returns the enumeration as {relative path bytes: bytes}, or
    raises Unreadable carrying the failing entry's path."""
    out = {}
    _walk_dir(os.fsencode(dirpath), b"", out)
    return out


# --------------------------------------------------------------------------
# s7.2 kit paths
# --------------------------------------------------------------------------

_SEG_EXTRA = frozenset((0x2E, 0x5F, 0x2D))          # . _ -


def is_kit_path(s):
    if not isinstance(s, str):
        return False
    try:
        b = s.encode("utf-8")
    except UnicodeEncodeError:
        return False
    if len(b) == 0 or len(b) > 1024:
        return False
    if b[0:1] == b"/":
        return False
    for seg in b.split(b"/"):
        if len(seg) < 1 or len(seg) > 255:
            return False
        if seg == b"." or seg == b"..":
            return False
        if seg[0:1] == b"-":
            return False
        for c in seg:
            if not (0x61 <= c <= 0x7A or 0x30 <= c <= 0x39 or c in _SEG_EXTRA):
                return False
    return True


# --------------------------------------------------------------------------
# s7.3 the manifest
# --------------------------------------------------------------------------

class RuleFail(Exception):
    def __init__(self, rule, token=None):
        super().__init__(rule)
        self.rule = rule
        self.token = token


class Manifest:
    __slots__ = ("raw", "d", "root", "entries", "files", "contents", "proofs",
                 "doc_id")


def _bytewise_sorted(items):
    for i in range(1, len(items)):
        if items[i - 1].encode("utf-8") > items[i].encode("utf-8"):
            return False
    return True


def check_manifest(b):
    """s7.3.  Raises RuleFail carrying the failing rule, or returns Manifest."""
    try:
        v = canonical(b)
    except Fault as f:
        raise RuleFail("canonical", f.token)
    if not isinstance(v, Obj) or frozenset(v.keys()) != MANIFEST_KEYSET:
        raise RuleFail("members")
    d = v.dict()

    if d["spec"] != SPEC_KIT:
        raise RuleFail("spec")

    if not (d["root"] is None or is_hex(d["root"], 20)):
        raise RuleFail("root")

    ents = d["entries"]
    if not isinstance(ents, list):
        raise RuleFail("entries")
    for x in ents:
        if not is_hex(x, 32):
            raise RuleFail("entries")
    if len(set(ents)) != len(ents):
        raise RuleFail("entries")
    if not _bytewise_sorted(ents):
        raise RuleFail("entries")

    fl = d["files"]
    if not isinstance(fl, list):
        raise RuleFail("files")
    files = []
    for r in fl:
        if not isinstance(r, Obj) or frozenset(r.keys()) != FILE_KEYSET:
            raise RuleFail("files")
        rd = r.dict()
        if not is_kit_path(rd["path"]):
            raise RuleFail("files")
        if not is_hex(rd["sha256"], 32):
            raise RuleFail("files")
        if not is_int(rd["size"]):
            raise RuleFail("files")
        files.append((rd["path"], rd["sha256"], rd["size"]))
    fpaths = [x[0] for x in files]
    if len(set(fpaths)) != len(fpaths):
        raise RuleFail("files")
    if not _bytewise_sorted(fpaths):
        raise RuleFail("files")
    listed = {}
    for p, h, _s in files:
        listed[p] = h

    ct = d["contents"]
    if not isinstance(ct, list):
        raise RuleFail("contents")
    contents = []
    for r in ct:
        if not isinstance(r, Obj) or frozenset(r.keys()) != CONTENT_KEYSET:
            raise RuleFail("contents")
        rd = r.dict()
        if not is_hex(rd["content"], 32):
            raise RuleFail("contents")
        p = rd["path"]
        if not isinstance(p, str) or p not in listed or listed[p] != rd["content"]:
            raise RuleFail("contents")
        contents.append((rd["content"], p))
    if len(set(contents)) != len(contents):
        raise RuleFail("contents")
    for i in range(1, len(contents)):
        a = (contents[i - 1][0].encode("utf-8"), contents[i - 1][1].encode("utf-8"))
        c = (contents[i][0].encode("utf-8"), contents[i][1].encode("utf-8"))
        if a > c:
            raise RuleFail("contents")

    pr = d["proofs"]
    if not isinstance(pr, list):
        raise RuleFail("proofs")
    proofs = []
    for r in pr:
        if not isinstance(r, Obj) or frozenset(r.keys()) != PROOF_KEYSET:
            raise RuleFail("proofs")
        rd = r.dict()
        if not is_kit_path(rd["path"]):
            raise RuleFail("proofs")
        if not is_hex(rd["sha256"], 32):
            raise RuleFail("proofs")
        if not is_hex(rd["tx"], 32):
            raise RuleFail("proofs")
        proofs.append((rd["path"], rd["sha256"], rd["tx"]))
    ppaths = [x[0] for x in proofs]
    if len(set(ppaths)) != len(ppaths):
        raise RuleFail("proofs")
    if not _bytewise_sorted(ppaths):
        raise RuleFail("proofs")

    if not is_prose(d["note_md"]):
        raise RuleFail("note_md")

    m = Manifest()
    m.raw = b
    m.d = d
    m.root = d["root"]
    m.entries = ents
    m.files = files
    m.contents = contents
    m.proofs = proofs
    m.doc_id = "0x" + sha256(b).hex()
    return m


# --------------------------------------------------------------------------
# s7.4 verification
# --------------------------------------------------------------------------

def _entry_path(eid):
    return b"entries/" + eid[2:].encode("ascii") + b".zk1"


def verify_kit(enumeration):
    """s7.4 over an enumeration.  Returns
    (verdict, subject_or_None, ok_payload_or_None) where ok_payload is
    (kit_id, n_entries, n_files, n_proofs, invalid_entries)."""
    mb = enumeration.get(MANIFEST_PATH)
    if mb is None:
        return ("E_KIT_MANIFEST_ABSENT", None, None)
    try:
        man = check_manifest(mb)
    except RuleFail as rf:
        return ("E_KIT_MANIFEST", rf.rule, None)

    for eid in man.entries:
        blob = enumeration.get(_entry_path(eid))
        if blob is None or ("0x" + sha256(blob).hex()) != eid:
            return ("E_KIT_ENTRY_BYTES", eid, None)

    for (path, digest, size) in man.files:
        blob = enumeration.get(b"files/" + path.encode("utf-8"))
        if (blob is None or ("0x" + sha256(blob).hex()) != digest
                or len(blob) != size):
            return ("E_KIT_FILE", path, None)

    for (path, digest, _tx) in man.proofs:
        blob = enumeration.get(b"proofs/" + path.encode("utf-8"))
        if blob is None or ("0x" + sha256(blob).hex()) != digest:
            return ("E_KIT_PROOF_BYTES", path, None)

    named = set([MANIFEST_PATH])
    for eid in man.entries:
        named.add(_entry_path(eid))
    for (path, _h, _s) in man.files:
        named.add(b"files/" + path.encode("utf-8"))
    for (path, _h, _t) in man.proofs:
        named.add(b"proofs/" + path.encode("utf-8"))
    extra = [p for p in enumeration if p not in named]
    if extra:
        return ("E_KIT_EXTRA", show(min(extra)), None)

    invalid = []
    for eid in man.entries:
        blob = enumeration[_entry_path(eid)]
        try:
            validate(blob)
        except Fault as f:
            invalid.append((eid, f.token))
    return ("KIT_OK", None,
            (man.doc_id, len(man.entries), len(man.files), len(man.proofs),
             invalid))
