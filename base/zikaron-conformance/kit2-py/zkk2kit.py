"""zikaron.kit/2 sections 4 and 5: the record kit and the grant package.

The walk of s4.1, the one-file form of s4.1a, kit paths (s4.2), the manifest
(s4.3), verification (s4.4) with the directory manifest of s4.5, and the
grant package (s5).
"""

import os
import stat

from zkcanon import Fault, Obj, accept as canonical, canon
from zkcrypto import sha256
from zkentry import is_hex, is_int, is_prose
from zkk2doc import accept, badge_decode

SPEC_KIT = "zikaron.kit/2"
MAGIC = b"zikaron-kit-file/1\n"

MANIFEST_PATH = b"manifest.json"
REQUIRED_KEYS = frozenset(("contents", "entries", "files", "note_md", "root", "spec"))
OPTIONAL_KEYS = frozenset(("anchoredOn", "anchors"))

FILE_KEYS = frozenset(("path", "sha256", "size"))
FILE_KEYS_NAMED = frozenset(("path", "sha256", "size", "name_md"))
CONTENT_KEYS = frozenset(("content", "path"))
CONTENT_KEYS_NAMED = frozenset(("content", "path", "name_md"))
ANCHOREDON_KEYSET = frozenset(("chainId", "registry", "fromBlock"))
ANCHOR_KEYSET = frozenset(("entry_id", "chainId", "tx"))


def show(path_bytes):
    return path_bytes.decode("utf-8", "replace")


# --------------------------------------------------------------------------
# s4.1 the walk, s4.1a the one-file form
# --------------------------------------------------------------------------

class Unreadable(Exception):
    def __init__(self, subject):
        super().__init__(subject)
        self.subject = subject


def _walk_dir(root, rel, out):
    full = os.path.join(root, rel) if rel else root
    try:
        names = os.listdir(full)
    except OSError:
        raise Unreadable(show(rel) if rel else ".")
    names = [n for n in names if n != b"." and n != b".."]
    names.sort()
    for name in names:
        r = (rel + b"/" + name) if rel else name
        try:
            name.decode("utf-8")
        except UnicodeDecodeError:
            raise Unreadable(show(rel) if rel else ".")
        f = os.path.join(root, r)
        try:
            st = os.lstat(f)
        except OSError:
            raise Unreadable(show(r))
        mode = st.st_mode
        if stat.S_ISLNK(mode):
            raise Unreadable(show(r))
        if stat.S_ISREG(mode):
            try:
                with open(f, "rb") as fh:
                    out[r] = fh.read()
            except OSError:
                raise Unreadable(show(r))
        elif stat.S_ISDIR(mode):
            _walk_dir(root, r, out)
        else:
            raise Unreadable(show(r))


def walk(dirpath):
    out = {}
    _walk_dir(os.fsencode(dirpath), b"", out)
    return out


def _is_count(b):
    if not b or any(c < 0x30 or c > 0x39 for c in b):
        return False
    if len(b) > 1 and b[0] == 0x30:
        return False
    return True


def parse_onefile(data):
    """s4.1a.  Returns the enumeration {path bytes: bytes} or raises
    Unreadable carrying the subject the section names."""
    if not data.startswith(MAGIC):
        raise Unreadable(".")
    i = len(MAGIC)
    n = len(data)
    items = {}
    first = True
    prev = None
    while i < n:
        j = data.find(b"\n", i)
        if j < 0:
            raise Unreadable(".")
        path = data[i:j]
        try:
            pstr = path.decode("utf-8")
        except UnicodeDecodeError:
            raise Unreadable(".")
        if not is_kit_path(pstr):
            raise Unreadable(".")
        k = data.find(b"\n", j + 1)
        if k < 0:
            raise Unreadable(pstr)
        cnt = data[j + 1:k]
        if not _is_count(cnt):
            raise Unreadable(pstr)
        size = int(cnt)
        start = k + 1
        end = start + size
        if end >= n or data[end:end + 1] != b"\n":
            raise Unreadable(pstr)
        if first:
            if path != MANIFEST_PATH:
                raise Unreadable(pstr)
            first = False
        else:
            if path in items or (prev is not None and not path > prev):
                raise Unreadable(pstr)
            prev = path
        items[path] = data[start:end]
        i = end + 1
    if first:
        raise Unreadable(".")
    return items


# --------------------------------------------------------------------------
# s4.2 kit paths
# --------------------------------------------------------------------------

_SEG_EXTRA = frozenset((0x2E, 0x5F, 0x2D))


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
# s4.3 the manifest
# --------------------------------------------------------------------------

class RuleFail(Exception):
    def __init__(self, rule):
        super().__init__(rule)
        self.rule = rule


class Manifest:
    __slots__ = ("raw", "d", "root", "entries", "files", "contents", "doc_id", "value")


def _bytewise_sorted(items):
    for i in range(1, len(items)):
        if items[i - 1].encode("utf-8") > items[i].encode("utf-8"):
            return False
    return True


def check_manifest(b):
    try:
        v = canonical(b)
    except Fault:
        raise RuleFail("canonical")
    if not isinstance(v, Obj):
        raise RuleFail("members")
    keys = frozenset(v.keys())
    if not REQUIRED_KEYS <= keys or not keys <= (REQUIRED_KEYS | OPTIONAL_KEYS):
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
    if len(set(ents)) != len(ents) or not _bytewise_sorted(ents):
        raise RuleFail("entries")

    fl = d["files"]
    if not isinstance(fl, list):
        raise RuleFail("files")
    files = []
    for r in fl:
        if not isinstance(r, Obj):
            raise RuleFail("files")
        ks = frozenset(r.keys())
        if ks != FILE_KEYS and ks != FILE_KEYS_NAMED:
            raise RuleFail("files")
        rd = r.dict()
        if not is_kit_path(rd["path"]) or not is_hex(rd["sha256"], 32) or not is_int(rd["size"]):
            raise RuleFail("files")
        if "name_md" in rd and not is_prose(rd["name_md"]):
            raise RuleFail("files")
        files.append((rd["path"], rd["sha256"], rd["size"], rd.get("name_md")))
    fpaths = [x[0] for x in files]
    if len(set(fpaths)) != len(fpaths):
        raise RuleFail("files")
    for p in fpaths:
        pre = p + "/"
        for q in fpaths:
            if q.startswith(pre):
                raise RuleFail("files")
    if not _bytewise_sorted(fpaths):
        raise RuleFail("files")
    listed = dict((p, h) for p, h, _s, _n in files)

    ct = d["contents"]
    if not isinstance(ct, list):
        raise RuleFail("contents")
    contents = []
    for r in ct:
        if not isinstance(r, Obj):
            raise RuleFail("contents")
        ks = frozenset(r.keys())
        if ks != CONTENT_KEYS and ks != CONTENT_KEYS_NAMED:
            raise RuleFail("contents")
        rd = r.dict()
        if not is_hex(rd["content"], 32):
            raise RuleFail("contents")
        p = rd["path"]
        if not is_kit_path(p):
            raise RuleFail("contents")
        if "name_md" in rd and not is_prose(rd["name_md"]):
            raise RuleFail("contents")
        if p in listed:
            if listed[p] != rd["content"]:
                raise RuleFail("contents")
        else:
            pre = p + "/"
            if not any(q.startswith(pre) for q in fpaths):
                raise RuleFail("contents")
        contents.append((rd["content"], p))
    dir_origs = [p for (_c, p) in contents if p not in listed]
    for p in dir_origs:
        pre = p + "/"
        for q in dir_origs:
            if q.startswith(pre):
                raise RuleFail("contents")  # a file lies under at most one directory original
    for p in dir_origs:
        pre = p + "/"
        names = [(nm if nm is not None else fp[len(pre):]) for fp, _h, _s, nm in files
                 if fp.startswith(pre)]
        if len(set(names)) != len(names):
            raise RuleFail("contents")  # two rows restore one name
        for nm in names:
            segs = nm.split("/")
            if any(sg == "" or sg == "." or sg == ".." for sg in segs):
                raise RuleFail("contents")  # a name is non-empty segments, none . or ..
        for nm in names:
            if any(other.startswith(nm + "/") for other in names):
                raise RuleFail("contents")  # a name is a file or a directory, never both
    if len(set(contents)) != len(contents):
        raise RuleFail("contents")
    for i in range(1, len(contents)):
        a = (contents[i - 1][0].encode("utf-8"), contents[i - 1][1].encode("utf-8"))
        c = (contents[i][0].encode("utf-8"), contents[i][1].encode("utf-8"))
        if a > c:
            raise RuleFail("contents")

    if "anchoredOn" in d:
        ao = d["anchoredOn"]
        if not isinstance(ao, Obj) or frozenset(ao.keys()) != ANCHOREDON_KEYSET:
            raise RuleFail("anchoredOn")
        od = ao.dict()
        if not is_int(od["chainId"]) or not is_hex(od["registry"], 20) or not is_int(od["fromBlock"]):
            raise RuleFail("anchoredOn")

    if "anchors" in d:
        an = d["anchors"]
        if not isinstance(an, list):
            raise RuleFail("anchors")
        rows = []
        entset = set(ents)
        for r in an:
            if not isinstance(r, Obj) or frozenset(r.keys()) != ANCHOR_KEYSET:
                raise RuleFail("anchors")
            rd = r.dict()
            if not is_hex(rd["entry_id"], 32) or rd["entry_id"] not in entset:
                raise RuleFail("anchors")
            if not is_int(rd["chainId"]) or not is_hex(rd["tx"], 32):
                raise RuleFail("anchors")
            rows.append((rd["entry_id"].encode("utf-8"), rd["chainId"], rd["tx"].encode("utf-8")))
        if len(set(rows)) != len(rows):
            raise RuleFail("anchors")
        for i in range(1, len(rows)):
            if rows[i - 1] > rows[i]:
                raise RuleFail("anchors")

    if not is_prose(d["note_md"]):
        raise RuleFail("note_md")

    m = Manifest()
    m.raw = b
    m.d = d
    m.value = v
    m.root = d["root"]
    m.entries = ents
    m.files = files
    m.contents = contents
    m.doc_id = "0x" + sha256(b).hex()
    return m


# --------------------------------------------------------------------------
# s4.4 verification, s4.5 the directory manifest
# --------------------------------------------------------------------------

def _entry_path(eid):
    return b"entries/" + eid[2:].encode("ascii") + b".zk1"


def directory_manifest_digest(man, dirpath):
    pre = dirpath + "/"
    rows = []
    for (p, h, _s, name) in man.files:
        if p.startswith(pre):
            nm = name if name is not None else p[len(pre):]
            rows.append((nm.encode("utf-8"), nm, h))
    rows.sort(key=lambda r: r[0])
    doc = Obj([("files", [Obj([("path", nm), ("sha256", h)]) for _b, nm, h in rows])])
    return "0x" + sha256(canon(doc)).hex()


def verify_kit(enumeration):
    """s4.4.  Returns (verdict, subject, payload, manifest)."""
    mb = enumeration.get(MANIFEST_PATH)
    if mb is None:
        return ("E_KIT_MANIFEST_ABSENT", None, None, None)
    try:
        man = check_manifest(mb)
    except RuleFail as rf:
        return ("E_KIT_MANIFEST", rf.rule, None, None)
    for eid in man.entries:
        blob = enumeration.get(_entry_path(eid))
        if blob is None or ("0x" + sha256(blob).hex()) != eid:
            return ("E_KIT_ENTRY_BYTES", eid, None, man)
    for (path, digest, size, _n) in man.files:
        blob = enumeration.get(b"files/" + path.encode("utf-8"))
        if blob is None or ("0x" + sha256(blob).hex()) != digest or len(blob) != size:
            return ("E_KIT_FILE", path, None, man)
    listed = set(p for p, _h, _s, _n in man.files)
    for (content, path) in man.contents:
        if path in listed:
            continue
        if directory_manifest_digest(man, path) != content:
            return ("E_KIT_CONTENT", path, None, man)
    named = set([MANIFEST_PATH])
    for eid in man.entries:
        named.add(_entry_path(eid))
    for (path, _h, _s, _n) in man.files:
        named.add(b"files/" + path.encode("utf-8"))
    extra = [p for p in enumeration if p not in named]
    if extra:
        return ("E_KIT_EXTRA", show(min(extra)), None, man)
    invalid = []
    for eid in man.entries:
        try:
            accept(enumeration[_entry_path(eid)])
        except Fault as f:
            invalid.append((eid, f.token))
    return ("KIT_OK", None, (man.doc_id, len(man.entries), len(man.files), invalid), man)


# --------------------------------------------------------------------------
# s5 grant package
# --------------------------------------------------------------------------

GRANT_PATH = b"files/zikaron-grant.txt"
TERMS_PREFIX = b"files/terms/"


def verify_grant_package(enumeration):
    """s5.2.  Returns ("GP_OK", result Obj) or (token, subject)."""
    verdict, subject, payload, man = verify_kit(enumeration)
    if verdict != "KIT_OK":
        return (verdict, subject)
    code = enumeration.get(GRANT_PATH)
    if code is None:
        return ("E_GP_NO_CODE", None)
    token, a, _b = badge_decode(code)
    if token != "BADGE_OK":
        return (token, a if token in ("E_BADGE_B64", "E_BADGE_ENTRY", "E_BADGE_TYPE", "E_BADGE_LINK") else None)
    hops = a
    root = man.root
    if root is None:
        led = hops[0].body.dict().get("ledger")
        if not is_hex(led, 20):
            return ("E_GP_ROOT", None)
        root = led
    for h in hops:
        if _entry_path(h.eid) not in enumeration:
            return ("E_GP_UNCARRIED", h.eid)
    work = hops[-1].body.dict().get("work")
    for (content, path) in man.contents:
        if content != work:
            return ("E_GP_CONTENT", path)
    terms_of = set(h.body.dict().get("terms") for h in hops)
    tpaths = sorted(p for p in enumeration if p.startswith(TERMS_PREFIX))
    for p in tpaths:
        if ("0x" + sha256(enumeration[p]).hex()) not in terms_of:
            return ("E_GP_TERMS", show(p[len(TERMS_PREFIX):]))
    per_hop = []
    for h in hops:
        t = h.body.dict().get("terms")
        per_hop.append([show(p[len(TERMS_PREFIX):]) for p in tpaths
                        if ("0x" + sha256(enumeration[p]).hex()) == t])
    result = Obj([
        ("verdict", "GP_OK"),
        ("kit_id", man.doc_id),
        ("root", root),
        ("hops", [h.eid for h in hops]),
        ("contents", [Obj([("content", c), ("path", p)]) for c, p in man.contents]),
        ("terms", per_hop),
    ])
    return ("GP_OK", result)
