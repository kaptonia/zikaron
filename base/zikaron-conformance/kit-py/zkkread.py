"""zikaron.kit/1 sections 8, 9 and 10: the reading predicates.

Reachability and anchoring as read (s8), depth reading (s9), the grant check,
the ledger link and the chain check (s10).  The zikaron/1 audit is taken from
the embedded frozen core: the report, the ledger, the whole-set lineage and
the trimmed anchor records all come out of ../impl-py and are never
re-decided here.
"""

import json

import zkaudit
from zkaudit import NoLabel
from zkcanon import Fault, Obj
from zkcrypto import sha256
from zkentry import validate

from zkkdoc import byte_link

CHECK_TOKENS = ("BAD_SIG", "BROKEN_LEDGER", "NOT_IN_LEDGER", "UNANCHORED",
                "EXPIRED", "REVOKED")


# --------------------------------------------------------------------------
# s1 audit(I): the embedded core's audit, with its ledger
# --------------------------------------------------------------------------

def _plain(v):
    if isinstance(v, Obj):
        return dict((k, _plain(x)) for k, x in v.pairs)
    if isinstance(v, list):
        return [_plain(x) for x in v]
    return v


class View:
    """audit(I): the report of zikaron/1 s8.7 together with the ledger."""

    __slots__ = ("root", "basis", "report", "label", "ledger", "have",
                 "by_eid", "counted", "lineage", "findings", "bounds",
                 "unproven_reach")


def _reach(entry, by_eid):
    """The entry_ids reachable from `entry` by following `prev` through ledger
    entries (s8.1), `entry` included."""
    seen = set()
    cur = entry
    while cur is not None and cur.eid not in seen:
        seen.add(cur.eid)
        cur = by_eid.get(cur.prev) if cur.prev is not None else None
    return seen


def load_view(raw, extra_pile=None):
    """audit(I) or audit(I'), where I' is I with `extra_pile` added to its
    pile (s10.1).  Raises NoLabel for an invalid input."""
    plain = _plain(zkaudit.load_input(raw))
    if extra_pile is not None:
        if not isinstance(plain, dict) or not isinstance(plain.get("pile"), list):
            raise NoLabel()
        plain = dict(plain)
        plain["pile"] = list(plain["pile"]) + ["0x" + extra_pile.hex()]
    try:
        raw2 = json.dumps(plain, ensure_ascii=True).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError):
        raise NoLabel()

    parsed = zkaudit.load_input(raw2)
    (root, pile, anchors, _unavailable, _evidence, basis,
     _acids) = zkaudit._read_inputs(parsed)
    report = zkaudit.audit(raw2)

    # zikaron/1 s8.1, decided by the core's own predicates.
    seen = set()
    entries = []
    for b in pile:
        if b in seen:
            continue
        seen.add(b)
        try:
            entries.append(validate(b))
        except Fault:
            pass
    lineage = zkaudit._lineage(root, entries)
    ledger = [e for e in entries if e.author in lineage]
    anchors = [a for a in anchors if a.sender in lineage]
    counted = [a for a in anchors if a.verdict == "counted"]
    unproven = [a for a in anchors if a.verdict == "UNPROVEN"]

    v = View()
    v.root = root
    v.basis = basis
    v.report = report
    rd = report.dict()
    v.label = rd["label"]
    v.ledger = ledger
    v.by_eid = dict((e.eid, e) for e in ledger)
    v.have = frozenset(v.by_eid.keys())
    v.counted = counted
    v.lineage = lineage
    v.findings = {}
    for f in rd["findings"]:
        fd = f.dict()
        v.findings.setdefault(fd["entry_id"], set()).add(fd["name"])

    # s8.2: anchored, and the bound.
    bounds = {}
    cache = {}
    for a in counted:
        f = v.by_eid.get(a.hash)
        if f is None:
            continue
        r = cache.get(a.hash)
        if r is None:
            r = _reach(f, v.by_eid)
            cache[a.hash] = r
        for eid in r:
            prior = bounds.get(eid)
            if prior is None or a.blockTimestamp < prior:
                bounds[eid] = a.blockTimestamp
    v.bounds = bounds

    # s10.2 check 4: the entries an undiscarded UNPROVEN record would anchor
    # had its codeless test been decided (s8.2's predicate with `counted`
    # read as `UNPROVEN`).
    ureach = set()
    for a in unproven:
        f = v.by_eid.get(a.hash)
        if f is None:
            continue
        ureach |= _reach(f, v.by_eid)
    v.unproven_reach = frozenset(ureach)
    return v


# --------------------------------------------------------------------------
# s9 depth reading
# --------------------------------------------------------------------------

def depth(view, w):
    """s9.2.  `view` is None for an invalid audit input."""
    if view is None:
        return Obj([("valid", False)])
    H = [e for e in view.ledger
         if e.etype == "history" and e.body.dict().get("content") == w]
    if not H:
        return Obj([
            ("valid", True),
            ("label", view.label),
            ("found", False),
            ("earliest", None),
            ("deepest", 0),
            ("continuity", Obj([("anchored", 0), ("span", 0)])),
        ])
    anchored = [e for e in H if e.eid in view.bounds]
    earliest = None
    for e in anchored:
        b = view.bounds[e.eid]
        if earliest is None or b < earliest:
            earliest = b
    smin = min(e.seq for e in H)
    smax = max(e.seq for e in H)
    h0 = min((e for e in H if e.seq == smin), key=lambda e: e.eidb)
    hmax = min((e for e in H if e.seq == smax), key=lambda e: e.eidb)
    span = hmax.seq - h0.seq + 1
    counted_hashes = set(a.hash for a in view.counted)
    seqs = set()
    cache = {}
    for e in view.ledger:
        if e.seq < h0.seq or e.seq > hmax.seq:
            continue
        if e.eid not in counted_hashes:
            continue
        r = cache.get(e.eid)
        if r is None:
            r = _reach(e, view.by_eid)
            cache[e.eid] = r
        if h0.eid in r:
            seqs.add(e.seq)
    return Obj([
        ("valid", True),
        ("label", view.label),
        ("found", True),
        ("earliest", earliest),
        ("deepest", len(anchored)),
        ("continuity", Obj([("anchored", len(seqs)), ("span", span)])),
    ])


# --------------------------------------------------------------------------
# s10.2 the six checks
# --------------------------------------------------------------------------

def _covering(view):
    """s10.2 check 4: the basis is covering."""
    bd = view.basis.dict()
    chains = bd["chains"]
    if not chains:
        return False
    lineage = set(view.lineage)
    for c in chains:
        cd = c.dict()
        if not cd["registries"]:
            return False
        if not lineage <= set(cd["senders"]):
            return False
    return True


def grant_check(g, view, now):
    """s10.2 and s10.3.  `g` is a byte string, `view` the audit of I' or None.
    Returns (result Obj, entry_or_None)."""
    states = ["UNKNOWN"] * 6
    reasons = [None] * 6

    entry = None
    try:
        e = validate(g)
        if e.etype != "grant":
            states[0] = "FAIL"
            reasons[0] = "NOT_A_GRANT"
        else:
            states[0] = "PASS"
            entry = e
    except Fault as f:
        states[0] = "FAIL"
        reasons[0] = f.token

    if entry is not None:
        # 2 BROKEN_LEDGER
        if view is None:
            states[1] = "UNKNOWN"
        elif view.label == "BROKEN_CHAIN":
            states[1] = "FAIL"
        else:
            states[1] = "PASS"
        c2_fail = states[1] == "FAIL"

        # 3 NOT_IN_LEDGER
        if view is None or c2_fail:
            states[2] = "UNKNOWN"
        elif entry.eid not in view.have:
            states[2] = "FAIL"
        elif "AUTHORITY_MISMATCH" in view.findings.get(entry.eid, ()):
            states[2] = "UNKNOWN"
        else:
            states[2] = "PASS"
        c3_fail = states[2] == "FAIL"

        # 4 UNANCHORED
        if view is None or c2_fail or c3_fail:
            states[3] = "UNKNOWN"
        elif entry.eid in view.bounds:
            states[3] = "PASS"
        elif not _covering(view):
            states[3] = "UNKNOWN"
        elif entry.eid in view.unproven_reach:
            states[3] = "UNKNOWN"
        elif view.label == "COMPLETE":
            states[3] = "FAIL"
        else:
            states[3] = "UNKNOWN"

        # 5 EXPIRED
        bd = entry.body.dict()
        if "window" not in bd:
            states[4] = "PASS"
        elif now is None:
            states[4] = "UNKNOWN"
        else:
            wd = bd["window"].dict()
            if now < wd["from"] or now > wd["to"]:
                states[4] = "FAIL"
            else:
                states[4] = "PASS"

        # 6 REVOKED
        if view is None or c2_fail or c3_fail:
            states[5] = "UNKNOWN"
        else:
            revoked = False
            for x in view.ledger:
                if x.etype != "revocation":
                    continue
                if x.body.dict().get("grant") != entry.eid:
                    continue
                if "AUTHORITY_MISMATCH" in view.findings.get(x.eid, ()):
                    continue
                revoked = True
                break
            if revoked:
                states[5] = "FAIL"
            elif view.label == "COMPLETE":
                states[5] = "PASS"
            else:
                states[5] = "UNKNOWN"

    if all(s == "PASS" for s in states):
        verdict = "GREEN"
    elif any(s == "FAIL" for s in states):
        verdict = "FAIL"
    else:
        verdict = "PARTIAL"

    checks = [Obj([("n", i + 1), ("token", CHECK_TOKENS[i]),
                   ("state", states[i]), ("reason", reasons[i])])
              for i in range(6)]
    failed = [CHECK_TOKENS[i] for i in range(6) if states[i] == "FAIL"]
    result = Obj([
        ("verdict", verdict),
        ("basis", view.basis if view is not None else None),
        ("checks", checks),
        ("failed", failed),
    ])
    return (result, entry)


# --------------------------------------------------------------------------
# s10.4 ledger link
# --------------------------------------------------------------------------

def ledger_link(u, d, view_d):
    """s10.4.  Returns True, False, or None for unknown."""
    if not byte_link(u, d):
        return False
    if view_d is None:
        return None
    if view_d.root != u.body.dict()["grantee"]:
        return False
    return True


# --------------------------------------------------------------------------
# s10.5 chain check
# --------------------------------------------------------------------------

def chain_check(hops, now):
    """s10.5.  `hops` is a list of (grant bytes, view or None) from the
    original author's grant onward."""
    if not hops:
        return Obj([
            ("verdict", "FAIL"),
            ("hops", []),
            ("links", []),
            ("token", "CHAIN_EMPTY"),
            ("failing", Obj([("kind", "empty"), ("index", 0)])),
        ])

    results = []
    entries = []
    views = []
    for (g, view) in hops:
        r, e = grant_check(g, view, now)
        results.append(r)
        entries.append(e)
        views.append(view)

    links = []
    for k in range(1, len(hops)):
        u = entries[k - 1]
        d = entries[k]
        if u is None or d is None:
            links.append(None)
        else:
            links.append(ledger_link(u, d, views[k]))

    failing = None
    token = None
    if entries[0] is not None and "upstream" in entries[0].body.dict():
        failing = ("incomplete", 0)
        token = "CHAIN_INCOMPLETE"
    else:
        for k in range(1, len(hops)):
            if links[k - 1] is False:
                failing = ("link", k)
                token = "CHAIN_LINK"
                break
        if failing is None:
            for k in range(len(hops)):
                if results[k].dict()["verdict"] == "FAIL":
                    failing = ("hop", k)
                    token = None
                    break

    if failing is not None:
        verdict = "FAIL"
    elif (all(r.dict()["verdict"] == "GREEN" for r in results)
          and all(x is True for x in links)):
        verdict = "GREEN"
    else:
        verdict = "PARTIAL"

    return Obj([
        ("verdict", verdict),
        ("hops", results),
        ("links", links),
        ("token", token),
        ("failing", None if failing is None
         else Obj([("kind", failing[0]), ("index", failing[1])])),
    ])


__all__ = ["View", "NoLabel", "load_view", "depth", "grant_check",
           "ledger_link", "chain_check", "sha256"]
