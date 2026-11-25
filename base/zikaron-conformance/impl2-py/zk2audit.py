"""zikaron/2 sections 7, 8, 9.4 and 12.6: the audit.

The retirement, timeliness and lineage of s7.4, the trim and the ledger of
s8.1, the walk closed by a migration (s8.2), the reconciliation (s8.5), the
report (s8.6), and the continuation of a zikaron/1 ledger (s12.6), whose old
segment is handed whole to the frozen zikaron/1 core of ../impl-py and read
back from its report.
"""

import json
import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(_HERE), "impl-py"))

from zkcanon import MAX_INT, Fault, Obj                             # noqa: E402
from zkcrypto import sha256                                          # noqa: E402
import zkaudit as v1audit                                            # noqa: E402
import zkentry as v1entry                                            # noqa: E402
from zk2entry import KNOWN_TYPES, validate                           # noqa: E402

MAXSEQ = MAX_INT
NoLabel = v1audit.NoLabel
need = v1audit.need
load_input = v1audit.load_input
VERDICTS = v1audit.VERDICTS
Anchor = v1audit.Anchor

LABEL_ORDER = ("BROKEN_CHAIN", "UNPROVEN", "GAPS", "COMPLETE")


# --------------------------------------------------------------------------
# s9.4 basis validity, s8 input reading
# --------------------------------------------------------------------------

_CHAIN_KEYS = frozenset(("chainId", "fromBlock", "toBlock", "registries", "senders"))
_BASIS_KEYS = frozenset(("chains",))


def _exact(o, keys):
    need(isinstance(o, Obj))
    d = o.dict()
    need(frozenset(d.keys()) == keys)
    return d


def _uint(v):
    need(v1entry.is_int(v) and 0 <= v <= MAX_INT)
    return v


def _hex20(v):
    need(v1entry.is_hex(v, 20))
    return v


def _hex32(v):
    need(v1entry.is_hex(v, 32))
    return v


def check_basis(basis):
    """Raise NoLabel unless `basis` is a zikaron/2 basis (s9.4)."""
    d = _exact(basis, _BASIS_KEYS)
    chains = d["chains"]
    need(isinstance(chains, list))
    prev = None
    for c in chains:
        cd = _exact(c, _CHAIN_KEYS)
        cid = _uint(cd["chainId"])
        fb = _uint(cd["fromBlock"])
        tb = _uint(cd["toBlock"])
        need(fb <= tb)
        lists = {}
        for arr in ("registries", "senders"):
            need(isinstance(cd[arr], list))
            prev_a = None
            for a in cd[arr]:
                _hex20(a)
                need(prev_a is None or prev_a.encode("ascii") < a.encode("ascii"))
                prev_a = a
            lists[arr] = tuple(cd[arr])
        if prev is not None:
            need((cid, fb) > (prev[0], prev[1]))
            if cid == prev[0]:
                need(fb > prev[2])
                if fb == prev[2] + 1:
                    need(lists["registries"] != prev[3] or lists["senders"] != prev[4])
        prev = (cid, fb, tb, lists["registries"], lists["senders"])


def _reaches(basis, rec):
    for c in basis.dict()["chains"]:
        cd = c.dict()
        if (cd["chainId"] == rec.chainId and cd["fromBlock"] <= rec.blockNumber <= cd["toBlock"]
                and rec.sender in cd["senders"]):
            return True
    return False


def _read_inputs(v):
    need(isinstance(v, Obj))
    d = v.dict()
    for k in ("root", "pile", "anchors", "basis"):
        need(k in d)
    root = _hex20(d["root"])
    need(isinstance(d["pile"], list))
    pile = [v1audit._hexbytes(x) for x in d["pile"]]
    need(isinstance(d["anchors"], list))
    anchors = {}
    for a in d["anchors"]:
        need(isinstance(a, Obj))
        ad = a.dict()
        for k in ("chainId", "blockNumber", "blockTimestamp", "tx", "sender", "hash", "verdict"):
            need(k in ad)
        rec = Anchor()
        rec.chainId = _uint(ad["chainId"])
        rec.blockNumber = _uint(ad["blockNumber"])
        rec.blockTimestamp = _uint(ad["blockTimestamp"])
        rec.tx = _hex32(ad["tx"])
        rec.sender = _hex20(ad["sender"])
        rec.hash = _hex32(ad["hash"])
        need(isinstance(ad["verdict"], str) and ad["verdict"] in VERDICTS)
        rec.verdict = ad["verdict"]
        key = (rec.chainId, rec.blockNumber, rec.tx, rec.hash)
        prior = anchors.get(key)
        if prior is not None:
            need(prior.blockTimestamp == rec.blockTimestamp and prior.sender == rec.sender
                 and prior.verdict == rec.verdict)
        else:
            anchors[key] = rec
    check_basis(d["basis"])
    for rec in anchors.values():
        need(_reaches(d["basis"], rec))
    return root, pile, list(anchors.values()), d["basis"]


# --------------------------------------------------------------------------
# helpers
# --------------------------------------------------------------------------

def _plain(v):
    if isinstance(v, Obj):
        return dict((k, _plain(x)) for k, x in v.pairs)
    if isinstance(v, list):
        return [_plain(x) for x in v]
    return v


def _finding(name, position, entry_id, extra, hard, second=""):
    pairs = [("name", name), ("position", position), ("entry_id", entry_id), ("hard", hard)]
    pairs.extend(extra)
    return (position, name.encode("ascii"), entry_id.encode("ascii"),
            second.encode("ascii"), Obj(pairs))


def _line(e, by_eid):
    """The byte strings on `e`'s line: `e`, then along `prev` through the
    held byte strings `by_eid` holds, stopping where none matches."""
    out = []
    seen = set()
    cur = e
    while cur is not None and cur.eid not in seen:
        seen.add(cur.eid)
        out.append(cur)
        cur = by_eid.get(cur.prev) if cur.prev is not None else None
    return out


def _parse_pile(pile, validator):
    """Byte-identical copies collapse; returns (entries, malformed{eid: token},
    by_raw{bytes: entry})."""
    seen = set()
    entries = []
    malformed = {}
    by_raw = {}
    for b in pile:
        if b in seen:
            continue
        seen.add(b)
        try:
            e = validator(b)
            entries.append(e)
            by_raw[b] = e
        except Fault as f:
            malformed["0x" + sha256(b).hex()] = f.token
    return entries, malformed, by_raw


# --------------------------------------------------------------------------
# s7.4 retirement, timeliness, lineage
# --------------------------------------------------------------------------

def retirements(entries, anchors):
    """key -> smallest blockTimestamp of any record (any sender, any verdict)
    anchoring a succession or migration the key signed, and key -> its
    retiring seq: the smallest seq among the successions and migrations it
    signed that a record of that second anchors."""
    by_eid = dict((e.eid, e) for e in entries)
    ret = {}
    rseq = {}
    for a in anchors:
        e = by_eid.get(a.hash)
        if e is None or e.etype not in ("succession", "migration"):
            continue
        prior = ret.get(e.author)
        if prior is None or a.blockTimestamp < prior:
            ret[e.author] = a.blockTimestamp
            rseq[e.author] = e.seq
        elif a.blockTimestamp == prior and e.seq < rseq[e.author]:
            rseq[e.author] = e.seq
    return ret, rseq


def retired_at(key, rec, ret, rseq, by_eid):
    """s7.4: `key` has retired at `rec` iff the record is later than the key's
    retirement, or is of the retirement's own second and anchors a byte
    string passing s4.3 whose author is the key at a seq above its retiring
    seq. The predicate serves the trim of s8.1 (asked of the record's sender);
    timeliness reads the retirement and the retiring seq directly."""
    r = ret.get(key)
    if r is None or rec.blockTimestamp < r:
        return False
    if rec.blockTimestamp > r:
        return True
    e = by_eid.get(rec.hash)
    return e is not None and e.author == key and e.seq > rseq[key]


def timely_set(entries, anchors, ret, rseq):
    """The entry_ids of the entries that are timely (s7.4): the author has no
    retirement, or some record earlier than the author's retirement, or of
    the retirement's own second while the entry sits at or below the author's
    retiring seq, anchors a byte string from whose line the entry is
    reachable."""
    by_eid = dict((e.eid, e) for e in entries)
    out = set(e.eid for e in entries if e.author not in ret)
    cache = {}
    for a in anchors:
        f = by_eid.get(a.hash)
        if f is None:
            continue
        line = cache.get(f.eid)
        if line is None:
            line = _line(f, by_eid)
            cache[f.eid] = line
        for e in line:
            if e.eid in out:
                continue
            r = ret[e.author]
            if a.blockTimestamp < r or (a.blockTimestamp == r and e.seq <= rseq[e.author]):
                out.add(e.eid)
    return out


def lineage(seed, entries, timely):
    lin = {seed}
    changed = True
    while changed:
        changed = False
        for e in entries:
            if e.etype == "succession" and e.eid in timely and e.author in lin:
                to = e.body.dict()["to"]
                if to not in lin:
                    lin.add(to)
                    changed = True
    return lin


def has_retired(rec, ret, rseq, by_eid):
    return retired_at(rec.sender, rec, ret, rseq, by_eid)


# --------------------------------------------------------------------------
# s8.2 walk, s8.4 equivocation
# --------------------------------------------------------------------------

def _walk(seed, ledger):
    order = sorted(ledger, key=lambda e: (e.seq, e.eidb))
    by_seq = {}
    for e in ledger:
        by_seq.setdefault(e.seq, []).append(e)
    findings = []
    expected = 0
    auth = seed
    closed = False
    certain = True
    prev_seq = None
    any_gap = False
    i = 0
    n = len(order)
    while i < n:
        s = order[i].seq
        j = i
        while j < n and order[j].seq == s:
            j += 1
        group = order[i:j]
        for e in group:
            if e.seq == expected:
                expected = e.seq if e.seq == MAXSEQ else e.seq + 1
            elif prev_seq is not None and e.seq == prev_seq:
                pass
            else:
                findings.append(_finding("SEQ_GAP", e.seq, e.eid,
                                         [("expected", expected), ("actual", e.seq)], False))
                any_gap = True
                expected = e.seq if e.seq == MAXSEQ else e.seq + 1
                certain = False
            prev_seq = e.seq
            if e.seq >= 1:
                pred = by_seq.get(e.seq - 1)
                if pred and not any(x.eid == e.prev for x in pred):
                    findings.append(_finding("PREV_MISMATCH", e.seq, e.eid, [("seq", e.seq)], True))
            if e.seq >= 1 and (closed or e.author != auth):
                findings.append(_finding("AUTHORITY_MISMATCH", e.seq, e.eid,
                                         [("seq", e.seq), ("certain", certain)], certain))
            if e.seq == 0 and e.author != seed:
                findings.append(_finding("ROOT_MISMATCH", 0, e.eid,
                                         [("seq", 0), ("actual", e.author)], True))
        if not closed:
            if any(e.etype == "migration" and e.author == auth for e in group):
                closed = True
            else:
                succs = [e for e in group if e.etype == "succession" and e.author == auth]
                if succs:
                    auth = min(succs, key=lambda e: e.eidb).body.dict()["to"]
        i = j
    return findings, any_gap


def _equivocation(ledger):
    out = []
    m = len(ledger)
    for i in range(m):
        a = ledger[i]
        for j in range(i + 1, m):
            b = ledger[j]
            if a.eid == b.eid:
                continue
            if a.seq == b.seq or (a.prev is not None and a.prev == b.prev):
                x, y = (a, b) if a.eidb < b.eidb else (b, a)
                seq = a.seq if a.seq < b.seq else b.seq
                out.append(_finding("EQUIVOCATION", seq, x.eid,
                                    [("seq", seq), ("a", x.eid), ("b", y.eid)], True, second=y.eid))
    return out


# --------------------------------------------------------------------------
# s12.6 continuation
# --------------------------------------------------------------------------

class Continuation:
    __slots__ = ("genesis", "succession", "cut", "old_bytes", "old_recs", "report")


def _v1_input_json(root, old_bytes, recs, basis):
    chains = _plain(basis.dict()["chains"])
    doc = {
        "root": root,
        "pile": ["0x" + b.hex() for b in sorted(old_bytes)],
        "anchors": [{"chainId": a.chainId, "blockNumber": a.blockNumber,
                     "blockTimestamp": a.blockTimestamp, "tx": a.tx, "sender": a.sender,
                     "hash": a.hash, "verdict": a.verdict} for a in recs],
        "unavailable": [],
        "evidence": [],
        "basis": {"chains": chains, "bareTx": [], "adoptionChains": []},
    }
    return json.dumps(doc, ensure_ascii=True).encode("ascii")


def old_segment(root, pile, anchors, basis, s, cut, v1_by_raw):
    """The old segment of s12.6 for a candidate's succession `s` and cut `cut`:
    (old byte strings, old anchor records, the zikaron/1 report)."""
    v1_by_eid = dict((e.eid, e) for e in v1_by_raw.values())
    # the old byte strings: s's line, the bytes anchored at or before the cut
    # (whatever they pass), and the zikaron/1 lines of those that pass
    recs_le = [a for a in anchors if a.blockTimestamp <= cut]
    old = set()
    for e in _line(s, v1_by_eid):
        old.add(e.raw)
    hashes = set(a.hash for a in recs_le)
    for b in set(pile):
        if ("0x" + sha256(b).hex()) in hashes:
            old.add(b)
            e = v1_by_raw.get(b)
            if e is not None:
                for x in _line(e, v1_by_eid):
                    old.add(x.raw)
    # the old records: at or before the cut, or anchoring an old ledger entry
    # (a zikaron/1 ledger entry of the old byte strings; that law's lineage is
    # a function of the byte strings alone), whatever their time
    old_entries = [v1_by_raw[b] for b in old if b in v1_by_raw]
    old_lin = v1audit._lineage(root, old_entries)
    old_ids = set(e.eid for e in old_entries if e.author in old_lin)
    recs_old = [a for a in anchors if a.blockTimestamp <= cut or a.hash in old_ids]
    report = v1audit.audit(_v1_input_json(root, old, recs_old, basis))
    return old, recs_old, report


def _v1_ledger_ids(report):
    rd = report.dict()
    ids = set(row.dict()["entry_id"] for row in rd["anchored"])
    ids |= set(rd["unanchored"])
    return ids


def find_continuation(root, pile, anchors, basis, v2_entries, v1_by_raw):
    v1_by_eid = dict((e.eid, e) for e in v1_by_raw.values())
    by_hash = {}
    for a in anchors:
        by_hash.setdefault(a.hash, []).append(a)
    best = None
    for g in v2_entries:
        if g.seq != 0 or g.etype != "genesis":
            continue
        bd = g.body.dict()
        c = bd.get("continues")
        if not isinstance(c, Obj):
            continue
        cd = c.dict()
        if cd.get("root") != root:
            continue
        s = v1_by_eid.get(cd.get("entry_id"))
        if s is None or s.etype != "succession" or s.author != g.author:
            continue
        sd = s.body.dict()
        if sd.get("to") != g.author or sd.get("kind") != "migration":
            continue
        recs = by_hash.get(s.eid)
        if not recs:
            continue
        cut = min(a.blockTimestamp for a in recs)
        # s12.6: a key whose handing off under zikaron/1 a record earlier than
        # the cut anchors (a zikaron/1 succession by s's author to another
        # key) closes nothing
        handed_off = False
        for e in v1_by_raw.values():
            if e.etype == "succession" and e.author == s.author and e.body.dict().get("to") != s.author:
                if any(a.blockTimestamp < cut for a in by_hash.get(e.eid, [])):
                    handed_off = True
                    break
        if handed_off:
            continue
        old, recs_old, report = old_segment(root, pile, anchors, basis, s, cut, v1_by_raw)
        if s.eid not in _v1_ledger_ids(report):
            continue
        key = (cut, g.eidb)
        if best is None or key < best[0]:
            cont = Continuation()
            cont.genesis = g
            cont.succession = s
            cont.cut = cut
            cont.old_bytes = old
            cont.old_recs = recs_old
            cont.report = report
            best = (key, cont)
    return None if best is None else best[1]


# --------------------------------------------------------------------------
# s8 the audit
# --------------------------------------------------------------------------

class Segment:
    """What the audit of the segment this law reads decided, kept for a
    client law that reads beside the report."""
    __slots__ = ("seed", "entries", "malformed", "ledger", "excluded", "retired",
                 "lineage", "ret", "timely", "anchors", "counted", "findings")


def _audit_segment(root, seed, pile, anchors, basis, continues_value, whole_pile):
    pile_ids = set("0x" + sha256(b).hex() for b in whole_pile)  # s8.6 item 15 rule 2: the whole pile
    entries, malformed, _by_raw = _parse_pile(pile, validate)
    ret, rseq = retirements(entries, anchors)
    by_eid = dict((e.eid, e) for e in entries)
    timely = timely_set(entries, anchors, ret, rseq)
    lin = lineage(seed, entries, timely)
    ledger = [e for e in entries if e.author in lin and e.eid in timely]
    excluded = [e for e in entries if e.author not in lin]
    retired = [e for e in entries if e.author in lin and e.eid not in timely]
    discarded = [a for a in anchors if a.sender not in lin or has_retired(a, ret, rseq, by_eid)]
    kept = [a for a in anchors if a.sender in lin and not has_retired(a, ret, rseq, by_eid)]
    counted = [a for a in kept if a.verdict == "counted"]
    counted_hashes = set(a.hash for a in counted)

    findings, any_gap = _walk(seed, ledger)
    findings.extend(_equivocation(ledger))
    dedup = {}
    for f in findings:
        dedup[(f[0], f[1], f[2], f[3])] = f
    findings = sorted(dedup.values(), key=lambda f: (f[0], f[1], f[2], f[3]))
    any_hard = any(f[4].dict()["hard"] for f in findings)

    have = set(e.eid for e in ledger)
    missing = []
    for h in sorted(h for h in counted_hashes if h not in have):
        recs = sorted((a for a in counted if a.hash == h), key=lambda a: (a.chainId, a.blockNumber, a.tx))
        missing.append(Obj([("hash", h), ("anchors", [
            Obj([("chainId", a.chainId), ("blockNumber", a.blockNumber), ("tx", a.tx), ("sender", a.sender)])
            for a in recs])]))
    unanchored = sorted(e.eid for e in ledger if e.eid not in counted_hashes)
    anchored = []
    for eid_ in sorted(have & counted_hashes):
        recs = sorted((a for a in counted if a.hash == eid_), key=lambda a: (a.chainId, a.blockNumber, a.tx))
        anchored.append(Obj([("entry_id", eid_), ("anchors", [
            Obj([("chainId", a.chainId), ("blockNumber", a.blockNumber),
                 ("blockTimestamp", a.blockTimestamp), ("tx", a.tx), ("sender", a.sender)])
            for a in recs])]))

    def seq_rows(es):
        return [Obj([("seq", e.seq), ("author", e.author), ("entry_id", e.eid)])
                for e in sorted(es, key=lambda e: (e.seq, e.eidb))]

    unknown_rows = [Obj([("seq", e.seq), ("entryType", e.etype), ("entry_id", e.eid)])
                    for e in sorted((e for e in ledger if e.etype not in KNOWN_TYPES),
                                    key=lambda e: (e.seq, e.etype.encode("ascii"), e.eidb))]
    malformed_rows = [Obj([("entry_id", h), ("token", malformed[h])]) for h in sorted(malformed)]

    def rows(verdict):
        rs = sorted((a for a in kept if a.verdict == verdict),
                    key=lambda a: (a.hash, a.chainId, a.blockNumber, a.tx))
        return [Obj([("hash", a.hash), ("chainId", a.chainId), ("blockNumber", a.blockNumber),
                     ("tx", a.tx), ("sender", a.sender)]) for a in rs]

    discarded.sort(key=lambda a: (a.sender, a.chainId, a.blockNumber, a.tx, a.hash))
    discarded_rows = [Obj([("chainId", a.chainId), ("blockNumber", a.blockNumber),
                           ("blockTimestamp", a.blockTimestamp), ("tx", a.tx), ("sender", a.sender),
                           ("hash", a.hash), ("verdict", a.verdict)]) for a in discarded]

    unproven_open = any(a.hash not in pile_ids and a.hash not in counted_hashes
                        for a in kept if a.verdict == "UNPROVEN")
    if any_hard:
        label = "BROKEN_CHAIN"
    elif unproven_open:
        label = "UNPROVEN"
    elif any_gap or missing:
        label = "GAPS"
    else:
        label = "COMPLETE"

    seg = Segment()
    seg.seed = seed
    seg.entries = entries
    seg.malformed = malformed
    seg.ledger = ledger
    seg.excluded = excluded
    seg.retired = retired
    seg.lineage = lin
    seg.ret = ret
    seg.timely = timely
    seg.anchors = kept
    seg.counted = counted
    seg.findings = {}
    for f in findings:
        fd = f[4].dict()
        seg.findings.setdefault(fd["entry_id"], set()).add(fd["name"])

    report = Obj([
        ("root", root),
        ("basis", basis),
        ("entries", len(ledger)),
        ("findings", [f[4] for f in findings]),
        ("missing", missing),
        ("anchored", anchored),
        ("unanchored", unanchored),
        ("excluded", seq_rows(excluded)),
        ("retired", seq_rows(retired)),
        ("unknown_type", unknown_rows),
        ("malformed", malformed_rows),
        ("unproven", rows("UNPROVEN")),
        ("void", rows("VOID")),
        ("discarded", discarded_rows),
        ("continues", continues_value),
        ("label", label),
    ])
    return report, seg, label


class Result:
    """The report with what a client law reads beside it."""
    __slots__ = ("report", "root", "basis", "pile", "anchors", "new", "cont",
                 "old_pile", "old_anchors", "old_entries", "old_lineage")


def _combine(own, v1label):
    other = "UNPROVEN" if v1label == "UNAVAILABLE" else v1label
    return min((own, other), key=LABEL_ORDER.index)


def audit_full(raw):
    root, pile, anchors, basis = _read_inputs(load_input(raw))
    v2_entries, _m, _r = _parse_pile(pile, validate)
    v1_entries, _m1, v1_by_raw = _parse_pile(pile, v1entry.validate)
    cont = find_continuation(root, pile, anchors, basis, v2_entries, v1_by_raw)
    res = Result()
    res.root = root
    res.basis = basis
    res.pile = pile
    res.anchors = anchors
    res.cont = cont
    if cont is None:
        report, seg, _label = _audit_segment(root, root, pile, anchors, basis, None, pile)
        res.report = report
        res.new = seg
        res.old_pile = []
        res.old_anchors = []
        res.old_entries = []
        res.old_lineage = set()
        return res
    old_v1 = set(b for b in cont.old_bytes if b in v1_by_raw)
    new_pile = [b for b in pile if b not in old_v1]
    old_keys = set((a.chainId, a.blockNumber, a.tx, a.hash) for a in cont.old_recs)
    new_anchors = [a for a in anchors if (a.chainId, a.blockNumber, a.tx, a.hash) not in old_keys]
    continues_value = Obj([("entry_id", cont.succession.eid), ("cut", cont.cut),
                           ("report", cont.report)])
    report, seg, own = _audit_segment(root, cont.genesis.author, new_pile, new_anchors, basis,
                                      continues_value, pile)
    label = _combine(own, cont.report.dict()["label"])
    pairs = [(k, v) if k != "label" else (k, label) for k, v in report.pairs]
    res.report = Obj(pairs)
    res.new = seg
    res.old_pile = sorted(cont.old_bytes)
    res.old_anchors = list(cont.old_recs)
    res.old_entries = [v1_by_raw[b] for b in sorted(old_v1)]
    res.old_lineage = v1audit._lineage(root, res.old_entries)
    return res


def audit(raw):
    return audit_full(raw).report
