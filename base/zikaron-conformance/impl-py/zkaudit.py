"""zikaron/1 sections 7, 8, 9.4 and 9.5: the audit."""

import json

from zkcanon import MAX_INT, Fault, Obj
from zkcrypto import sha256
from zkentry import KNOWN_TYPES, attestation_ok, is_hex, is_int, validate

MAXSEQ = MAX_INT


class NoLabel(Exception):
    pass


def need(cond):
    if not cond:
        raise NoLabel()


# --------------------------------------------------------------------------
# Lenient reading of the harness's audit input file
# --------------------------------------------------------------------------

def _reject(*_a, **_k):
    raise NoLabel()


def _parse_int(sv):
    # HARNESS: every number in the file is an integer of s3.1's universe
    if sv[:1] == "-" or int(sv) > MAX_INT:
        raise NoLabel()
    return int(sv)


def _pairs_hook(pairs):
    keys = [k for k, _ in pairs]
    if len(set(keys)) != len(keys):
        raise NoLabel()
    return Obj(pairs)


def _depth_ok(text):
    # HARNESS: a container opening at depth 129 makes the file unreadable
    depth, in_str, esc = 0, False, False
    for ch in text:
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
        elif ch == '"':
            in_str = True
        elif ch in "[{":
            depth += 1
            if depth > 128:
                return False
        elif ch in "]}":
            depth -= 1
    return True


def _no_lone_surrogate(v):
    # HARNESS: every string of the file decodes to scalar values
    if isinstance(v, str):
        need(not any(0xD800 <= ord(c) <= 0xDFFF for c in v))
    elif isinstance(v, list):
        for x in v:
            _no_lone_surrogate(x)
    elif isinstance(v, Obj):
        for k, x in v.dict().items():
            _no_lone_surrogate(k)
            _no_lone_surrogate(x)


def load_input(raw):
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        raise NoLabel()
    need(_depth_ok(text))
    try:
        v = json.loads(text, object_pairs_hook=_pairs_hook,
                          parse_float=_reject, parse_constant=_reject,
                          parse_int=_parse_int)
    except NoLabel:
        raise
    except Exception:
        raise NoLabel()
    _no_lone_surrogate(v)
    return v


def _hexbytes(v):
    """A `0x<hex>` blob of any length (harness encoding, not a law field)."""
    if not isinstance(v, str) or len(v) < 2 or v[0] != "0" or v[1] != "x":
        raise NoLabel()
    body = v[2:]
    if len(body) % 2:
        raise NoLabel()
    for c in body:
        if c not in "0123456789abcdefABCDEF":
            raise NoLabel()
    return bytes.fromhex(body)


def _uint(v):
    need(is_int(v) and 0 <= v <= MAX_INT)
    return v


def _hex20(v):
    need(is_hex(v, 20))
    return v


def _hex32(v):
    need(is_hex(v, 32))
    return v


def _calldata(v):
    """s9.4: `0x` followed by an even number of lowercase hexadecimal digits."""
    need(isinstance(v, str) and len(v) >= 2 and v[0] == "0" and v[1] == "x")
    body = v[2:]
    need(len(body) % 2 == 0)
    for c in body:
        need(c in "0123456789abcdef")
    return bytes.fromhex(body)


# --------------------------------------------------------------------------
# s9.4 basis validity
# --------------------------------------------------------------------------

_CHAIN_KEYS = frozenset(("chainId", "fromBlock", "toBlock", "registries",
                         "senders"))
_BARE_KEYS = frozenset(("chainId", "tx"))
_ADOPT_KEYS = frozenset(("chainId", "throughBlock"))
_BASIS_KEYS = frozenset(("chains", "bareTx", "adoptionChains"))


def _exact(o, keys):
    need(isinstance(o, Obj))
    d = o.dict()
    need(frozenset(d.keys()) == keys)
    return d


def check_basis(basis):
    """Raise NoLabel unless `basis` is a zikaron/1 basis.  Returns the set of
    adoption chain ids."""
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
                # s9.4: bytewise ascending, no repeated element
                need(prev_a is None or prev_a.encode("ascii") < a.encode("ascii"))
                prev_a = a
            lists[arr] = tuple(cd[arr])
        # s9.4: chains ordered by (chainId, fromBlock) ascending; two objects of
        # one chainId have ranges that do not overlap and, where they touch,
        # registries or senders that differ
        if prev is not None:
            need((cid, fb) > (prev[0], prev[1]))
            if cid == prev[0]:
                need(fb > prev[2])
                if fb == prev[2] + 1:
                    need(lists["registries"] != prev[3] or lists["senders"] != prev[4])
        prev = (cid, fb, tb, lists["registries"], lists["senders"])
    bare = d["bareTx"]
    need(isinstance(bare, list))
    prev_key = None
    for o in bare:
        od = _exact(o, _BARE_KEYS)
        key = (_uint(od["chainId"]), _hex32(od["tx"]).encode("ascii"))
        need(prev_key is None or prev_key < key)
        prev_key = key
    ac = d["adoptionChains"]
    need(isinstance(ac, list))
    ids = set()
    prev_cid = None
    for o in ac:
        od = _exact(o, _ADOPT_KEYS)
        cid = _uint(od["chainId"])
        _uint(od["throughBlock"])
        # s9.4: at most one object per chainId, ordered by chainId ascending
        need(prev_cid is None or prev_cid < cid)
        prev_cid = cid
        ids.add(cid)
    return ids


def _reaches(basis, rec):
    """s9.4 [C]: the basis reaches the record by bareTx or by a chains object."""
    d = basis.dict()
    for o in d["bareTx"]:
        od = o.dict()
        if od["chainId"] == rec.chainId and od["tx"] == rec.tx:
            return True
    for c in d["chains"]:
        cd = c.dict()
        if (cd["chainId"] == rec.chainId and cd["fromBlock"] <= rec.blockNumber <= cd["toBlock"]
                and rec.sender in cd["senders"]):
            return True
    return False


# --------------------------------------------------------------------------
# s8 the audit
# --------------------------------------------------------------------------

class Anchor:
    __slots__ = ("chainId", "blockNumber", "blockTimestamp", "tx", "sender",
                 "hash", "verdict")


class Evidence:
    __slots__ = ("chainId", "tx", "sender", "calldata")


VERDICTS = ("counted", "UNPROVEN", "VOID")


def _read_inputs(v):
    need(isinstance(v, Obj))
    d = v.dict()
    for k in ("root", "pile", "anchors", "unavailable", "evidence", "basis"):
        need(k in d)
    root = _hex20(d["root"])
    need(isinstance(d["pile"], list))
    pile = [_hexbytes(x) for x in d["pile"]]

    need(isinstance(d["anchors"], list))
    anchors = {}
    for a in d["anchors"]:
        need(isinstance(a, Obj))
        ad = a.dict()
        for k in ("chainId", "blockNumber", "blockTimestamp", "tx", "sender",
                  "hash", "verdict"):
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
            # s9.4: at most one record per (chainId, blockNumber, tx, hash);
            # two that disagree on a remaining member are no audit input.
            need(prior.blockTimestamp == rec.blockTimestamp
                 and prior.sender == rec.sender
                 and prior.verdict == rec.verdict)
        else:
            anchors[key] = rec

    need(isinstance(d["unavailable"], list))
    unavailable = set(_hex32(x) for x in d["unavailable"])

    need(isinstance(d["evidence"], list))
    evidence = {}
    for e in d["evidence"]:
        need(isinstance(e, Obj))
        ed = e.dict()
        for k in ("chainId", "tx", "sender", "calldata"):
            need(k in ed)
        rec = Evidence()
        rec.chainId = _uint(ed["chainId"])
        rec.tx = _hex32(ed["tx"])
        rec.sender = _hex20(ed["sender"])
        rec.calldata = _calldata(ed["calldata"])
        key = (rec.chainId, rec.tx)
        prior = evidence.get(key)
        if prior is not None:
            # s9.4: at most one evidence record per (chainId, tx); two that
            # disagree on sender or calldata are no zikaron/1 audit input
            need(prior.sender == rec.sender and prior.calldata == rec.calldata)
        else:
            evidence[key] = rec

    adoption_chain_ids = check_basis(d["basis"])
    for rec in anchors.values():
        need(_reaches(d["basis"], rec))
    for rec in evidence.values():
        # s9.4 [C]: an evidence record on a chain no adoptionChains object names
        need(rec.chainId in adoption_chain_ids)
    return (root, pile, list(anchors.values()), unavailable, evidence,
            d["basis"], adoption_chain_ids)


def _lineage(root, entries):
    lin = {root}
    changed = True
    while changed:
        changed = False
        for e in entries:
            if e.etype == "succession" and e.author in lin:
                to = e.body.dict()["to"]
                if to not in lin:
                    lin.add(to)
                    changed = True
    return lin


def _prefix_lineage(root, ledger, k):
    """s7.4: the prefix lineage at k, over ledger successions with seq < k."""
    lin = {root}
    changed = True
    while changed:
        changed = False
        for e in ledger:
            if e.etype == "succession" and e.seq < k and e.author in lin:
                to = e.body.dict()["to"]
                if to not in lin:
                    lin.add(to)
                    changed = True
    return lin


def _contains_at_word(calldata, target):
    """s9.5: 32 consecutive bytes at an offset that is a multiple of 32, or 4
    plus a multiple of 32, lying wholly within the calldata."""
    L = len(calldata)
    if L < 32:
        return False
    off = 0
    while off + 32 <= L:
        if calldata[off:off + 32] == target:
            return True
        off += 32
    off = 4
    while off + 32 <= L:
        if calldata[off:off + 32] == target:
            return True
        off += 32
    return False


def _finding(name, position, entry_id, extra, hard, second=""):
    """A finding as (sort key..., row).  s8.7 item 3 sorts by
    (position, finding name, entry_id, second entry_id), the empty second
    entry_id ordering before every hex32."""
    pairs = [("name", name), ("position", position), ("entry_id", entry_id),
             ("hard", hard)]
    pairs.extend(extra)
    return (position, name.encode("ascii"), entry_id.encode("ascii"),
            second.encode("ascii"), Obj(pairs))


def _walk(root, ledger):
    order = sorted(ledger, key=lambda e: (e.seq, e.eidb))
    by_seq = {}
    for e in ledger:
        by_seq.setdefault(e.seq, []).append(e)
    findings = []
    expected = 0
    auth = root
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
            # SEQ_GAP
            if e.seq == expected:
                expected = e.seq if e.seq == MAXSEQ else e.seq + 1
            elif prev_seq is not None and e.seq == prev_seq:
                pass                                     # fork twin
            else:
                findings.append(_finding(
                    "SEQ_GAP", e.seq, e.eid,
                    [("expected", expected), ("actual", e.seq)], False))
                any_gap = True
                expected = e.seq if e.seq == MAXSEQ else e.seq + 1
                certain = False
            prev_seq = e.seq
            # PREV_MISMATCH
            if e.seq >= 1:
                pred = by_seq.get(e.seq - 1)
                if pred and not any(x.eid == e.prev for x in pred):
                    findings.append(_finding(
                        "PREV_MISMATCH", e.seq, e.eid, [("seq", e.seq)], True))
            # AUTHORITY_MISMATCH
            if e.seq >= 1 and e.author != auth:
                findings.append(_finding(
                    "AUTHORITY_MISMATCH", e.seq, e.eid,
                    [("seq", e.seq), ("certain", certain)], certain))
            # ROOT_MISMATCH
            if e.seq == 0 and e.author != root:
                findings.append(_finding(
                    "ROOT_MISMATCH", 0, e.eid,
                    [("seq", 0), ("actual", e.author)], True))
        # s8.2: only a succession signed by the key in office moves authority
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
                if a.eidb < b.eidb:
                    x, y = a, b
                else:
                    x, y = b, a
                seq = a.seq if a.seq < b.seq else b.seq
                out.append(_finding("EQUIVOCATION", seq, x.eid,
                                    [("seq", seq), ("a", x.eid), ("b", y.eid)],
                                    True, second=y.eid))
    return out


def audit(raw):
    (root, pile, anchors, unavailable, evidence, basis,
     adoption_chain_ids) = _read_inputs(load_input(raw))

    # s8.1 input
    pile_ids = set("0x" + sha256(b).hex() for b in pile)
    seen = set()
    entries = []
    malformed = {}
    for b in pile:
        if b in seen:
            continue
        seen.add(b)
        try:
            entries.append(validate(b))
        except Fault as f:
            malformed["0x" + sha256(b).hex()] = f.token
    lineage = _lineage(root, entries)
    ledger = [e for e in entries if e.author in lineage]
    excluded = [e for e in entries if e.author not in lineage]
    discarded = [a for a in anchors if a.sender not in lineage]
    anchors = [a for a in anchors if a.sender in lineage]
    counted = [a for a in anchors if a.verdict == "counted"]
    counted_hashes = set(a.hash for a in counted)
    unavailable = set(h for h in unavailable
                      if h in counted_hashes and h not in pile_ids)

    # s8.2 and s8.4
    findings, any_gap = _walk(root, ledger)
    findings.extend(_equivocation(ledger))
    dedup = {}
    for f in findings:
        dedup[(f[0], f[1], f[2], f[3])] = f
    findings = sorted(dedup.values(), key=lambda f: (f[0], f[1], f[2], f[3]))
    any_hard = any(f[4].dict()["hard"] for f in findings)

    # s8.5 reconciliation
    have = set(e.eid for e in ledger)
    missing_hashes = sorted(h for h in counted_hashes
                            if h not in have and h not in unavailable)
    missing = []
    for h in missing_hashes:
        recs = sorted((a for a in counted if a.hash == h),
                      key=lambda a: (a.chainId, a.blockNumber, a.tx))
        missing.append(Obj([
            ("hash", h),
            ("anchors", [Obj([("chainId", a.chainId), ("blockNumber", a.blockNumber),
                              ("tx", a.tx), ("sender", a.sender)])
                         for a in recs]),
        ]))
    unanchored = sorted(e.eid for e in ledger if e.eid not in counted_hashes)
    # s8.7 item 5
    anchored = []
    for eid_ in sorted(have & counted_hashes):
        recs = sorted((a for a in counted if a.hash == eid_),
                      key=lambda a: (a.chainId, a.blockNumber, a.tx))
        anchored.append(Obj([
            ("entry_id", eid_),
            ("anchors", [Obj([("chainId", a.chainId), ("blockNumber", a.blockNumber),
                              ("blockTimestamp", a.blockTimestamp), ("tx", a.tx),
                              ("sender", a.sender)])
                         for a in recs]),
        ]))
    # s8.7 items 8 and 9
    unknown_rows = [Obj([("seq", e.seq), ("entryType", e.etype), ("entry_id", e.eid)])
                    for e in sorted((e for e in ledger if e.etype not in KNOWN_TYPES),
                                    key=lambda e: (e.seq, e.etype.encode("ascii"), e.eidb))]
    malformed_rows = [Obj([("entry_id", h), ("token", malformed[h])])
                      for h in sorted(malformed)]

    # s8.7 item 6
    excluded_rows = [Obj([("seq", e.seq), ("author", e.author),
                          ("entry_id", e.eid)])
                     for e in sorted(excluded, key=lambda e: (e.seq, e.eidb))]

    # s9.5 adoption, s8.7 item 8
    au = []
    for e in sorted(ledger, key=lambda e: (e.seq, e.eidb)):
        if e.etype != "adoption":
            continue
        d = e.body.dict()
        pref = None
        att = None
        for idx, el in enumerate(d["anchors"]):
            ed = el.dict()
            cid = ed["chainId"]
            tx = ed["tx"]
            proven = False
            if cid in adoption_chain_ids and (cid, tx) in evidence:
                rec = evidence[(cid, tx)]
                if _contains_at_word(rec.calldata,
                                     bytes.fromhex(ed["content"][2:])):
                    if pref is None:
                        pref = _prefix_lineage(root, ledger, e.seq)
                    if rec.sender in pref:
                        proven = True
                    else:
                        if att is None:
                            att = attestation_ok(e)
                        if att and d.get("attestor") == rec.sender:
                            proven = True
            if not proven:
                au.append(Obj([("seq", e.seq), ("entry_id", e.eid),
                               ("index", idx)]))

    # s8.7 items 12 and 13
    def rows(verdict):
        rs = [a for a in anchors if a.verdict == verdict]
        rs.sort(key=lambda a: (a.hash, a.chainId, a.blockNumber, a.tx))
        return [Obj([("hash", a.hash), ("chainId", a.chainId),
                     ("blockNumber", a.blockNumber), ("tx", a.tx),
                     ("sender", a.sender)])
                for a in rs]

    unproven_rows = rows("UNPROVEN")
    void_rows = rows("VOID")

    # s8.7 item 14: every record the lineage trim discarded, its seven members
    discarded.sort(key=lambda a: (a.sender, a.chainId, a.blockNumber, a.tx, a.hash))
    discarded_rows = [Obj([("chainId", a.chainId), ("blockNumber", a.blockNumber),
                           ("blockTimestamp", a.blockTimestamp), ("tx", a.tx),
                           ("sender", a.sender), ("hash", a.hash), ("verdict", a.verdict)])
                      for a in discarded]

    # s8.7 item 15: an UNPROVEN record of bytes in hand, under any signature or
    # none, moves no label
    unproven_open = any(a.hash not in pile_ids and a.hash not in counted_hashes
                        for a in anchors if a.verdict == "UNPROVEN")
    if any_hard:
        label = "BROKEN_CHAIN"
    elif unavailable or unproven_open:
        label = "UNAVAILABLE"
    elif any_gap or missing:
        label = "GAPS"
    else:
        label = "COMPLETE"

    return Obj([
        ("root", root),
        ("basis", basis),
        ("entries", len(ledger)),
        ("findings", [f[4] for f in findings]),
        ("missing", missing),
        ("anchored", anchored),
        ("unanchored", unanchored),
        ("excluded", excluded_rows),
        ("adoption_unproven", au),
        ("unknown_type", unknown_rows),
        ("malformed", malformed_rows),
        ("unavailable", sorted(unavailable)),
        ("unproven", unproven_rows),
        ("void", void_rows),
        ("discarded", discarded_rows),
        ("label", label),
    ])
