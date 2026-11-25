"""zikaron.face/1 sections 6 and 7: the face input and the reading.

The file reader of an input (the five restrictions of HARNESS2.md) is the
embedded zikaron/2 core's; the forms, the walk and the table are this
law's.
"""

import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
_UP = os.path.dirname(_HERE)
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.join(_UP, "impl2-py"))
sys.path.insert(0, os.path.join(_UP, "impl-py"))

from zkcanon import MAX_INT, Fault, Obj                              # noqa: E402
from zkentry import is_hex, is_int                                   # noqa: E402
import zk2audit                                                      # noqa: E402
import zkfentry                                                      # noqa: E402

VERDICTS = ("counted", "UNPROVEN", "VOID")
REASONS = ("SHAPE", "ROOT", "AUTHOR", "SIGNATURE", "SUBJECT", "UNSEATED")

_INPUT_KEYS = frozenset(("root", "chains", "anchors", "accessions", "pile"))
_CHAIN_KEYS = frozenset(("chainId", "fromBlock", "toBlock", "registries",
                         "book", "bookFromBlock"))
_ANCHOR_KEYS = frozenset(("chainId", "blockNumber", "blockTimestamp", "tx",
                          "sender", "hash", "verdict"))
_ACCESSION_KEYS = frozenset(("chainId", "blockNumber", "blockTimestamp",
                             "txIndex", "logIndex", "tx", "holder", "to", "root"))


class NoTable(Exception):
    """The inputs are no valid face input (s6.2)."""


def need(cond):
    if not cond:
        raise NoTable()


def _exact(o, keys):
    need(isinstance(o, Obj))
    d = o.dict()
    need(frozenset(d.keys()) == keys)
    return d


def _uint(v):
    need(is_int(v) and 0 <= v <= MAX_INT)
    return v


def _hex20(v):
    need(is_hex(v, 20))
    return v


def _hex32(v):
    need(is_hex(v, 32))
    return v


def _bytes_of(v):
    """A pile element: `0x` and an even number of hex digits of either case."""
    need(isinstance(v, str) and len(v) >= 2 and v[0] == "0" and v[1] == "x")
    body = v[2:]
    need(len(body) % 2 == 0)
    need(all(c in "0123456789abcdefABCDEF" for c in body))
    return bytes.fromhex(body)


class Anchor:
    __slots__ = ("chain", "block", "time", "tx", "sender", "hash", "verdict")


class Accession:
    __slots__ = ("chain", "block", "time", "txi", "logi", "tx", "holder", "to", "root")


class Input:
    __slots__ = ("root", "chains", "anchors", "accessions", "pile")


# --------------------------------------------------------------------------
# s6.1, s6.2 the face input, valid
# --------------------------------------------------------------------------

def _value(raw):
    try:
        return zk2audit.load_input(raw)
    except zk2audit.NoLabel:
        raise NoTable()


def load(raw):
    """Read a face input from the bytes of its file; NoTable where the file
    is unreadable or the value is no valid face input."""
    return _input(_value(raw))


def load_pair(raw):
    """HARNESS-FACE.md `joined`: an array of exactly two face inputs."""
    v = _value(raw)
    need(isinstance(v, list) and len(v) == 2)
    return _input(v[0]), _input(v[1])


def _input(v):
    d = _exact(v, _INPUT_KEYS)
    inp = Input()
    inp.root = _hex20(d["root"])

    need(isinstance(d["chains"], list))
    inp.chains = d["chains"]
    span = {}                         # chainId -> (fromBlock, toBlock, bookFromBlock or None)
    last = None
    for c in d["chains"]:
        cd = _exact(c, _CHAIN_KEYS)
        cid = _uint(cd["chainId"])
        lo = _uint(cd["fromBlock"])
        hi = _uint(cd["toBlock"])
        need(lo <= hi)
        need(isinstance(cd["registries"], list))
        prev = None
        for a in cd["registries"]:
            _hex20(a)
            need(prev is None or prev < a)
            prev = a
        book, start = cd["book"], cd["bookFromBlock"]
        need((book is None) == (start is None))
        if book is not None:
            _hex20(book)
            need(_uint(start) <= hi)
        need(last is None or last < cid)
        last = cid
        span[cid] = (lo, hi, start)

    clock = {}                        # (chainId, blockNumber) -> blockTimestamp

    def tick(cid, bn, ts):
        need(clock.setdefault((cid, bn), ts) == ts)

    need(isinstance(d["anchors"], list))
    inp.anchors = []
    seen = {}                         # the record's key -> its other three members
    for r in d["anchors"]:
        need(isinstance(r, Obj))
        rd = r.dict()                 # members beyond the seven are read by nothing
        need(_ANCHOR_KEYS <= frozenset(rd.keys()))
        a = Anchor()
        a.chain = _uint(rd["chainId"])
        a.block = _uint(rd["blockNumber"])
        a.time = _uint(rd["blockTimestamp"])
        a.tx = _hex32(rd["tx"])
        a.sender = _hex20(rd["sender"])
        a.hash = _hex32(rd["hash"])
        a.verdict = rd["verdict"]
        need(isinstance(a.verdict, str) and a.verdict in VERDICTS)
        need(a.chain in span and span[a.chain][0] <= a.block <= span[a.chain][1])
        key = (a.chain, a.block, a.tx, a.hash)
        rest = (a.time, a.sender, a.verdict)
        if key in seen:               # given twice: one record where the seven agree
            need(seen[key] == rest)
            continue
        seen[key] = rest
        tick(a.chain, a.block, a.time)
        inp.anchors.append(a)

    need(isinstance(d["accessions"], list))
    inp.accessions = []
    seen = set()
    for r in d["accessions"]:
        rd = _exact(r, _ACCESSION_KEYS)
        x = Accession()
        x.chain = _uint(rd["chainId"])
        x.block = _uint(rd["blockNumber"])
        x.time = _uint(rd["blockTimestamp"])
        x.txi = _uint(rd["txIndex"])
        x.logi = _uint(rd["logIndex"])
        x.tx = _hex32(rd["tx"])
        x.holder = _hex20(rd["holder"])
        x.to = _hex20(rd["to"])
        x.root = _hex20(rd["root"])
        need(x.chain in span and span[x.chain][2] is not None
             and span[x.chain][2] <= x.block <= span[x.chain][1])
        key = (x.chain, x.block, x.txi, x.logi)
        need(key not in seen)
        seen.add(key)
        tick(x.chain, x.block, x.time)
        inp.accessions.append(x)

    # one clock per chain: the smaller block carries no greater a time
    before = {}
    for (cid, bn) in sorted(clock):
        ts = clock[(cid, bn)]
        need(cid not in before or before[cid] <= ts)
        before[cid] = ts

    need(isinstance(d["pile"], list))
    inp.pile = [_bytes_of(x) for x in d["pile"]]
    return inp


# --------------------------------------------------------------------------
# s7 the reading
# --------------------------------------------------------------------------

def _order(a):
    """s7.3: the order of seated records."""
    return (a.time, a.chain, a.block, a.tx)


def _ref(a):
    return Obj([("chainId", a.chain), ("blockNumber", a.block),
                ("blockTimestamp", a.time), ("tx", a.tx)])


def read(inp):
    """s7: the face table of a valid face input."""
    R = inp.root

    # s6.2 the entries of the pile, byte-equal copies being one
    entries = {}
    for b in inp.pile:
        try:
            e = zkfentry.accept(b)
        except Fault:
            continue
        entries[e.eid] = e
    faces = dict((i, e) for i, e in entries.items() if e.etype == zkfentry.TYPE)
    shape = dict((i, zkfentry.shape_of(e.body)) for i, e in faces.items())
    body = dict((i, e.body.dict()) for i, e in faces.items())

    def reads(i):
        """s7.4: a shape, the root, and for a key the consent."""
        if shape[i] is None or body[i]["root"] != R:
            return False
        return shape[i] != "key" or zkfentry.consent_holds(body[i])

    # s7.1 steps
    steps = {}
    for a in inp.anchors:
        steps.setdefault((a.time, a.chain, a.block), ([], []))[0].append(a)
    for x in inp.accessions:
        steps.setdefault((x.time, x.chain, x.block), ([], []))[1].append(x)

    # s7.2 the walk
    office, home = R, None
    sealed = False                     # the seal: closed from the step after a reroot is seated
    head = {}                          # chainId -> the head of its book, R where absent
    line, keys = [(R, None)], {R}
    seated = {}                        # entry_id -> seated records (s7.3)
    openings = []                      # (record, entry_id)
    in_office_unread = set()
    unproven = 0
    for when in sorted(steps):
        chain = when[1]
        anchors, accessions = steps[when]
        opens = closes = False
        for a in anchors:                                       # 1. anchors
            if sealed:                 # no record is in office
                break
            if a.verdict == "UNPROVEN" and a.sender == office:
                unproven += 1
            if a.verdict != "counted" or a.sender != office:
                continue
            if a.hash not in entries:
                in_office_unread.add(a.hash)
                continue
            i = a.hash
            if i not in faces or shape[i] is None or faces[i].author != office:
                continue
            if shape[i] == "home":
                if body[i]["chain"] != chain or head.get(chain, R) != office:
                    continue
                seated.setdefault(i, []).append(a)
                if body[i]["root"] == R:
                    openings.append((a, i))
                    opens = True
            elif home == chain:
                seated.setdefault(i, []).append(a)
                if shape[i] == "reroot" and body[i]["root"] == R:
                    closes = True
        if opens:                                               # 2. home
            home = chain
        for x in sorted(accessions, key=lambda r: (r.txi, r.logi)):   # 3. accessions
            if x.root != R:
                continue
            if x.holder == office and (home is None or home == chain):
                office = x.to
                line.append((x.to, x))
                keys.add(x.to)
            head[chain] = x.to
        if closes:                                              # 4. seal
            sealed = True

    # s7.4 standing
    def moment(i):
        return min(seated[i], key=_order)

    stands = set(i for i in faces if reads(i) and i in seated
                 and shape[i] != "release")
    bindings = set(i for i in stands if shape[i] in ("account", "key", "persona"))
    releases = set(i for i in faces if shape[i] == "release" and reads(i)
                   and i in seated and body[i]["subject"] in bindings)
    stands |= releases

    # s7.5 spans, rows, state
    ending = {}                        # binding -> the release that ends it
    for q in sorted(releases):
        b = body[q]["subject"]
        if b not in ending or (_order(moment(q)), q) < (_order(moment(ending[b])), ending[b]):
            ending[b] = q
    rows = {}
    for b in bindings:
        since = moment(b)
        if b in ending:
            q = ending[b]
            until = moment(q)
            if _order(until) < _order(since):
                until = since
            span = (since, b, q, until)
        else:
            span = (since, b, None, None)
        rows.setdefault(zkfentry.face_of(shape[b], body[b]), []).append(span)

    def row_out(face, spans):
        spans = sorted(spans, key=lambda s: (_order(s[0]), s[1]))
        state = "bound" if any(s[2] is None for s in spans) else "released"
        out = [Obj([("entry", b), ("since", _ref(since)),
                    ("release", q), ("until", None if until is None else _ref(until))])
               for since, b, q, until in spans]
        if face[0] == "account":
            return Obj([("kind", "account"), ("platform", face[1]), ("handle", face[2]),
                        ("state", state), ("spans", out)])
        if face[0] == "persona":
            return Obj([("kind", "persona"), ("name_md", face[1]),
                        ("state", state), ("spans", out)])
        return Obj([("kind", "key"), ("address", face[1]), ("role", face[2]),
                    ("state", state), ("spans", out)])

    faces_out = [row_out(f, rows[f]) for f in sorted(f for f in rows if f[0] == "account")]
    faces_out += [row_out(f, rows[f]) for f in sorted(f for f in rows if f[0] == "key")]
    faces_out += [row_out(f, rows[f]) for f in sorted((f for f in rows if f[0] == "persona"),
                                                      key=lambda f: f[1].encode("utf-8"))]

    # s7.5 the persona: the open span of a persona row greatest in since, then in entry
    ruling = None
    for f in rows:
        if f[0] != "persona":
            continue
        for since, b, q, until in rows[f]:
            if q is None and (ruling is None or (_order(since), b) > ruling[0]):
                ruling = ((_order(since), b), b, f[1])
    persona = None if ruling is None else Obj([("entry", ruling[1]), ("name_md", ruling[2])])

    # s7.4 the reroot and the origin: the standing entry least in moment, then in entry
    def first(name, member):
        mine = [i for i in stands if shape[i] == name]
        if not mine:
            return None
        i = min(mine, key=lambda i: (_order(moment(i)), i))
        return Obj([("entry", i), (member, body[i][member]), ("since", _ref(moment(i)))])

    reroot, origin = first("reroot", "to"), first("origin", "from")

    # s7.6 misread
    def reason(i):
        if shape[i] is None:
            return "SHAPE"
        if body[i]["root"] != R:
            return "ROOT"
        if faces[i].author not in keys:
            return "AUTHOR"
        if shape[i] == "key" and not zkfentry.consent_holds(body[i]):
            return "SIGNATURE"
        if shape[i] == "release" and body[i]["subject"] not in bindings:
            return "SUBJECT"
        return "UNSEATED"

    # s7.6: the entries a key of the line anchored, whatever the verdict
    listed = set(a.hash for a in inp.anchors if a.sender in keys)
    misread = [Obj([("entry", i), ("reason", reason(i))])
               for i in sorted(faces) if i not in stands and i in listed]

    # s7.8 the table
    lineage = []
    for key, x in line:
        acc = None if x is None else Obj([
            ("chainId", x.chain), ("blockNumber", x.block), ("blockTimestamp", x.time),
            ("txIndex", x.txi), ("logIndex", x.logi), ("tx", x.tx), ("holder", x.holder)])
        lineage.append(Obj([("accession", acc), ("key", key)]))
    homes = [Obj([("entry", i), ("chainId", a.chain), ("blockNumber", a.block),
                  ("blockTimestamp", a.time), ("tx", a.tx), ("sender", a.sender)])
             for a, i in sorted(openings, key=lambda p: (_order(p[0]), p[1]))]
    return Obj([
        ("root", R),
        ("marker", zkfentry.marker(R)),
        ("basis", inp.chains),
        ("home", home),
        ("office", office),
        ("persona", persona),
        ("reroot", reroot),
        ("origin", origin),
        ("lineage", lineage),
        ("homes", homes),
        ("faces", faces_out),
        ("misread", misread),
        ("unread", len(in_office_unread)),
        ("unproven", unproven),
    ])


def joined(first, second):
    """s7.9 over two face tables: the first is joined to the second."""
    a, b = first.dict(), second.dict()
    return (a["reroot"] is not None and a["reroot"].dict()["to"] == b["root"]
            and b["origin"] is not None and b["origin"].dict()["from"] == a["root"])
