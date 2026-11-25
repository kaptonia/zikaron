"""zikaron.kit/2 sections 6 and 7: the reading predicates.

Reachability and anchoring as read (s6.1, s6.2), the notice input and the key
in office as the book says (s6.3, s6.4), the grant check, the ledger link and
the chain check (s7).  The zikaron/2 audit is taken from the embedded core
(../impl2-py): the report, the ledger of both segments, the lineages and the
trimmed anchor records all come out of it and are never re-decided here; the
anchor set as the audit received it is read for a grant carrying a revocation
member (s7.2 checks 3 and 4).
"""

import json

import zk2audit
from zk2audit import NoLabel
from zkcanon import MAX_INT, Fault, Obj
from zkentry import is_hex, is_int
from zkk2doc import accept, byte_link

CHECK_TOKENS = ("BAD_SIG", "BROKEN_LEDGER", "NOT_IN_LEDGER", "UNANCHORED",
                "EXPIRED", "REVOKED")

NOTICE_KEYS = frozenset(("basis", "revocations", "accessions"))
NBASIS_KEYS = frozenset(("chainId", "fromBlock", "toBlock", "revocationContract",
                         "book", "bookFromBlock"))
NREC_KEYS = frozenset(("chainId", "blockNumber", "txIndex", "logIndex", "tx",
                       "sender", "grant", "verdict"))
BREC_KEYS = frozenset(("chainId", "blockNumber", "txIndex", "logIndex", "holder",
                       "to", "root"))
VERDICTS = ("counted", "UNPROVEN", "VOID")
MEMBER_KEYS = frozenset(("chain", "at"))


class NoNotice(Exception):
    """s6.3: the bytes are no notice input (an invalid notice input)."""


def _need(cond):
    if not cond:
        raise NoNotice()


def _uint(v):
    _need(is_int(v) and 0 <= v <= MAX_INT)
    return v


class Notices:
    """A notice input (s6.3): its basis, and its two record arrays sorted by
    position, each record as (position, members)."""
    __slots__ = ("basis", "chainId", "toBlock", "at", "revocations", "accessions")


def _records(arr, keys, cid, lo, hi, notice):
    _need(isinstance(arr, list))
    out = []
    seen = set()
    for r in arr:
        _need(isinstance(r, Obj))
        rd = r.dict()
        _need(frozenset(rd.keys()) == keys)
        _need(_uint(rd["chainId"]) == cid)
        bn = _uint(rd["blockNumber"])
        ti = _uint(rd["txIndex"])
        li = _uint(rd["logIndex"])
        _need(lo <= bn <= hi)
        if notice:
            _need(is_hex(rd["tx"], 32) and is_hex(rd["sender"], 20) and is_hex(rd["grant"], 32))
            _need(rd["verdict"] in VERDICTS)
        else:
            _need(is_hex(rd["holder"], 20) and is_hex(rd["to"], 20) and is_hex(rd["root"], 20))
        pos = (bn, ti, li)
        _need(pos not in seen)
        seen.add(pos)
        out.append((pos, rd))
    out.sort(key=lambda x: x[0])
    return out


def load_notices(raw):
    """s6.3 over the bytes of a notices file (HARNESS-KIT2.md): a Notices, or
    NoNotice where the bytes are no notice input."""
    try:
        v = zk2audit.load_input(raw)
    except NoLabel:
        raise NoNotice()
    _need(isinstance(v, Obj))
    d = v.dict()
    _need(frozenset(d.keys()) == NOTICE_KEYS)
    b = d["basis"]
    _need(isinstance(b, Obj))
    bd = b.dict()
    _need(frozenset(bd.keys()) == NBASIS_KEYS)
    cid = _uint(bd["chainId"])
    fb = _uint(bd["fromBlock"])
    tb = _uint(bd["toBlock"])
    bfb = _uint(bd["bookFromBlock"])
    _need(is_hex(bd["revocationContract"], 20) and is_hex(bd["book"], 20))
    _need(fb <= tb and bfb <= tb)
    n = Notices()
    n.basis = b
    n.chainId = cid
    n.toBlock = tb
    n.at = bd["revocationContract"]
    n.revocations = _records(d["revocations"], NREC_KEYS, cid, fb, tb, True)
    n.accessions = _records(d["accessions"], BREC_KEYS, cid, bfb, tb, False)
    return n


def book_lineage(n, root):
    """s6.4: the root and the `to` of every book record of that root."""
    return set([root]) | set(rd["to"] for _p, rd in n.accessions if rd["root"] == root)


def in_office_at_position(n, root, pos):
    """s6.4: the `to` of the last book record of root `root` whose position is
    less than `pos`, else the root."""
    key = root
    for p, rd in n.accessions:
        if p >= pos:
            break
        if rd["root"] == root:
            key = rd["to"]
    return key


def in_office_at_block(n, root, block):
    """s6.4: by the block alone; a record in the block itself moves nothing."""
    key = root
    for p, rd in n.accessions:
        if p[0] >= block:
            break
        if rd["root"] == root:
            key = rd["to"]
    return key


def revocation_member(entry):
    """s7.1: (carries, member) where `carries` says whether body has the key
    `revocation` and `member` is (chain, at) where it is well-formed, else None."""
    bd = entry.body.dict()
    if "revocation" not in bd:
        return (False, None)
    m = bd["revocation"]
    if isinstance(m, Obj):
        md = m.dict()
        if frozenset(md.keys()) == MEMBER_KEYS and is_int(md["chain"]) and is_hex(md["at"], 20):
            return (True, (md["chain"], md["at"]))
    return (True, None)


def applies(notices, member):
    """s7.1: N applies to g."""
    return (notices is not None and member is not None
            and notices.chainId == member[0] and notices.at == member[1])


def book_root(entry, view, notices, member):
    """s7.1: the root under which s6.4 reads the book for a grant carrying the
    member, in this order: the `root` of the least-positioned book record of N
    whose `to` is the author, where N applies and holds one; I's root; the grant's
    `ledger` member where hex20; its author."""
    if applies(notices, member):
        for _p, rd in notices.accessions:    # sorted by position
            if rd["to"] == entry.author:
                return rd["root"]
    if view is not None:
        return view.root
    led = entry.body.dict().get("ledger")
    if is_hex(led, 20):
        return led
    return entry.author


def _plain(v):
    if isinstance(v, Obj):
        return dict((k, _plain(x)) for k, x in v.pairs)
    if isinstance(v, list):
        return [_plain(x) for x in v]
    return v


class View:
    """audit(I): the zikaron/2 report with the ledger of both segments."""
    __slots__ = ("root", "basis", "report", "label", "ledger", "have", "by_eid",
                 "counted", "lineage", "findings", "bounds", "unproven_reach",
                 "retired", "continued", "cont_eids", "anchors", "seed", "seg_entries")


def _reach(entry, by_eid, cont_eids):
    """s6.1: the entry_ids reachable from `entry` along prev through ledger
    entries, and from every ledger entry at seq 0 of the new segment whose
    continues.entry_id is the one the report carries (the selected genesis
    and any timely twin naming the same close); a member named continues on
    any other entry, an uncontinued root's genesis included, is data and no
    edge."""
    seen = set()
    stack = [entry]
    while stack:
        cur = stack.pop()
        if cur.eid in seen:
            continue
        seen.add(cur.eid)
        if cur.prev is not None:
            nxt = by_eid.get(cur.prev)
            if nxt is not None:
                stack.append(nxt)
        if cur.eid in cont_eids:
            c = cur.body.dict().get("continues")
            if isinstance(c, Obj):
                ce = c.dict().get("entry_id")
                if isinstance(ce, str):
                    nxt = by_eid.get(ce)
                    if nxt is not None:
                        stack.append(nxt)
    return seen


def load_view(raw, extra_pile=None):
    plain = _plain(zk2audit.load_input(raw))
    if extra_pile is not None:
        if not isinstance(plain, dict) or not isinstance(plain.get("pile"), list):
            raise NoLabel()
        plain = dict(plain)
        plain["pile"] = list(plain["pile"]) + ["0x" + extra_pile.hex()]
    try:
        raw2 = json.dumps(plain, ensure_ascii=True).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError):
        raise NoLabel()
    res = zk2audit.audit_full(raw2)
    rd = res.report.dict()

    ledger = list(res.new.ledger)
    counted = list(res.new.counted)
    unproven = [a for a in res.new.anchors if a.verdict == "UNPROVEN"]
    lineage = set(res.new.lineage)
    findings = {}
    for f in rd["findings"]:
        fd = f.dict()
        findings.setdefault(fd["entry_id"], set()).add(fd["name"])
    if res.cont is not None:
        r1 = res.cont.report.dict()
        old_ids = set(row.dict()["entry_id"] for row in r1["anchored"]) | set(r1["unanchored"])
        ledger.extend(e for e in res.old_entries if e.eid in old_ids)
        for a in res.old_anchors:
            if a.sender in res.old_lineage:
                if a.verdict == "counted":
                    counted.append(a)
                elif a.verdict == "UNPROVEN":
                    unproven.append(a)
        lineage |= set(res.old_lineage)
        for f in r1["findings"]:
            fd = f.dict()
            findings.setdefault(fd["entry_id"], set()).add(fd["name"])

    v = View()
    v.root = res.root
    v.basis = res.basis
    v.anchors = list(res.anchors)   # the anchor set as the audit received it (s7.2 checks 3, 4)
    v.seed = res.new.seed           # auth(0) of the segment this law reads (s7.2 check 2 for a member grant)
    v.seg_entries = list(res.new.entries)   # every byte string of the new segment passing s4.3
    v.report = res.report
    v.label = rd["label"]
    v.ledger = ledger
    v.by_eid = dict((e.eid, e) for e in ledger)
    v.have = frozenset(v.by_eid.keys())
    v.counted = counted
    v.lineage = lineage
    v.findings = findings
    v.retired = frozenset(row.dict()["entry_id"] for row in rd["retired"])
    v.continued = res.cont is not None
    if res.cont is not None:
        sid = res.cont.succession.eid
        v.cont_eids = frozenset(e.eid for e in res.new.ledger if e.seq == 0 and e.etype == "genesis"
                                and isinstance(e.body.dict().get("continues"), Obj)
                                and e.body.dict()["continues"].dict().get("entry_id") == sid)
    else:
        v.cont_eids = frozenset()

    bounds = {}
    cache = {}
    for a in counted:
        f = v.by_eid.get(a.hash)
        if f is None:
            continue
        r = cache.get(a.hash)
        if r is None:
            r = _reach(f, v.by_eid, v.cont_eids)
            cache[a.hash] = r
        for eid in r:
            prior = bounds.get(eid)
            if prior is None or a.blockTimestamp < prior:
                bounds[eid] = a.blockTimestamp
    v.bounds = bounds
    ureach = set()
    for a in unproven:
        f = v.by_eid.get(a.hash)
        if f is None:
            continue
        ureach |= _reach(f, v.by_eid, v.cont_eids)
    v.unproven_reach = frozenset(ureach)
    return v


def _covering(view):
    bd = view.basis.dict()
    chains = bd["chains"]
    if not chains:
        return False
    for c in chains:
        cd = c.dict()
        if not cd["registries"]:
            return False
        if not view.lineage <= set(cd["senders"]):
            return False
    return True


def _member_checks(entry, view, notices, states):
    """s7.2 for a grant carrying a revocation member: checks 2, 3, 4 and 6."""
    has_member, member = revocation_member(entry)
    app = applies(notices, member)
    root = book_root(entry, view, notices, member)
    if view is None:
        states[1] = "UNKNOWN"
    elif view.label == "BROKEN_CHAIN":
        states[1] = "FAIL"
    elif not any(e.etype == "genesis" and e.author == view.seed for e in view.seg_entries):
        states[1] = "UNKNOWN"   # no spine for the walk to start from (the byte string, timely or not)
    else:
        states[1] = "PASS"
    if view is None or states[1] == "FAIL":
        states[2] = "UNKNOWN"
        states[3] = "UNKNOWN"
    else:
        standing = []
        if app:
            standing = [a for a in view.anchors
                        if a.chainId == member[0] and a.hash == entry.eid and a.verdict == "counted"
                        and a.blockNumber <= notices.toBlock
                        and a.sender == in_office_at_block(notices, root, a.blockNumber)]
        # the table's check 3 fails the entry: EXCLUDED of either report, or a zikaron/1 entry
        # MALFORMED outside the old segment (RETIRED and the never-moved zikaron/1 ledger are not that)
        table_fails = (entry.eid not in view.have and entry.eid not in view.retired
                       and not (entry.value.dict()["spec"] == "zikaron/1" and not view.continued))
        seated = app and entry.author in book_lineage(notices, root)
        if states[1] == "PASS" and table_fails and not seated:
            states[2] = "FAIL"   # a key neither rail ever seated signed in the issuer's name
        elif not app:
            states[2] = "UNKNOWN"
        elif view.root != root:
            states[2] = "FAIL"   # the ledger audited is not the seat the book gave the signer
        elif any(a.sender == entry.author for a in standing):
            states[2] = "PASS"
        elif standing:
            states[2] = "FAIL"
        else:
            states[2] = "UNKNOWN"
        states[3] = ("PASS" if standing else "UNKNOWN") if app else "UNKNOWN"
    if not app:
        states[5] = "UNKNOWN"
    else:
        hit = None
        for pos, rd in notices.revocations:
            if rd["grant"] != entry.eid:
                continue
            if rd["sender"] != in_office_at_position(notices, root, pos):
                continue
            if rd["verdict"] == "counted":
                hit = "FAIL"
                break
            hit = "UNKNOWN"   # UNPROVEN or VOID, by the key in office
        states[5] = hit if hit is not None else "PASS"


def grant_check(g, view, notices, now):
    states = ["UNKNOWN"] * 6
    reasons = [None] * 6
    entry = None
    try:
        e = accept(g)
        if e.etype != "grant":
            states[0] = "FAIL"
            reasons[0] = "NOT_A_GRANT"
        else:
            states[0] = "PASS"
            entry = e
    except Fault as f:
        states[0] = "FAIL"
        reasons[0] = f.token

    if entry is not None and revocation_member(entry)[0]:
        bd = entry.body.dict()
        _member_checks(entry, view, notices, states)
        if "window" not in bd:
            states[4] = "PASS"
        elif now is None:
            states[4] = "UNKNOWN"
        else:
            wd = bd["window"].dict()
            states[4] = "FAIL" if (now < wd["from"] or now > wd["to"]) else "PASS"
    elif entry is not None:
        if view is None:
            states[1] = "UNKNOWN"
        elif view.label == "BROKEN_CHAIN":
            states[1] = "FAIL"
        else:
            states[1] = "PASS"
        c2_fail = states[1] == "FAIL"

        gate46 = False  # s7.2 check 3 leaving checks 4 and 6 UNKNOWN
        if view is None or c2_fail:
            states[2] = "UNKNOWN"
        elif entry.value.dict()["spec"] == "zikaron/1" and not view.continued:
            states[2] = "UNKNOWN"  # a zikaron/1 grant of a ledger that never moved (s9)
            gate46 = True
        elif entry.eid not in view.have:
            if entry.eid in view.retired:
                states[2] = "FAIL" if view.label == "COMPLETE" else "UNKNOWN"
            else:
                states[2] = "FAIL"  # EXCLUDED, or a zikaron/1 entry outside the old segment
            gate46 = True
        elif "AUTHORITY_MISMATCH" in view.findings.get(entry.eid, ()):
            states[2] = "UNKNOWN"
        else:
            states[2] = "PASS"

        if view is None or c2_fail or gate46:
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

        bd = entry.body.dict()
        if "window" not in bd:
            states[4] = "PASS"
        elif now is None:
            states[4] = "UNKNOWN"
        else:
            wd = bd["window"].dict()
            states[4] = "FAIL" if (now < wd["from"] or now > wd["to"]) else "PASS"

        if view is None or c2_fail or gate46:
            states[5] = "UNKNOWN"
        else:
            revoked = False
            for x in view.ledger:
                if x.etype != "revocation" or x.body.dict().get("grant") != entry.eid:
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
    checks = [Obj([("n", i + 1), ("token", CHECK_TOKENS[i]), ("state", states[i]), ("reason", reasons[i])])
              for i in range(6)]
    failed = [CHECK_TOKENS[i] for i in range(6) if states[i] == "FAIL"]
    result = Obj([("verdict", verdict), ("basis", view.basis if view is not None else None),
                  ("notices", notices.basis if notices is not None else None),
                  ("checks", checks), ("failed", failed)])
    return (result, entry)


def ledger_link(u, d, view_d, notices_d):
    if not byte_link(u, d):
        return False
    has_member, member = revocation_member(d)
    if view_d is None and not (has_member and applies(notices_d, member)):
        return None
    if book_root(d, view_d, notices_d, member) != u.body.dict()["grantee"]:
        return False
    return True


def chain_check(hops, now):
    if not hops:
        return Obj([("verdict", "FAIL"), ("hops", []), ("links", []), ("token", "CHAIN_EMPTY"),
                    ("failing", Obj([("kind", "empty"), ("index", 0)]))])
    results, entries, views, nots = [], [], [], []
    for (g, view, notices) in hops:
        r, e = grant_check(g, view, notices, now)
        results.append(r)
        entries.append(e)
        views.append(view)
        nots.append(notices)
    links = []
    for k in range(1, len(hops)):
        u, d = entries[k - 1], entries[k]
        links.append(None if (u is None or d is None) else ledger_link(u, d, views[k], nots[k]))
    failing = None
    token = None
    if entries[0] is not None and "upstream" in entries[0].body.dict():
        failing, token = ("incomplete", 0), "CHAIN_INCOMPLETE"
    else:
        for k in range(1, len(hops)):
            if links[k - 1] is False:
                failing, token = ("link", k), "CHAIN_LINK"
                break
        if failing is None:
            for k in range(len(hops)):
                if results[k].dict()["verdict"] == "FAIL":
                    failing, token = ("hop", k), None
                    break
    if failing is not None:
        verdict = "FAIL"
    elif all(r.dict()["verdict"] == "GREEN" for r in results) and all(x is True for x in links):
        verdict = "GREEN"
    else:
        verdict = "PARTIAL"
    return Obj([("verdict", verdict), ("hops", results), ("links", links), ("token", token),
                ("failing", None if failing is None else Obj([("kind", failing[0]), ("index", failing[1])]))])
