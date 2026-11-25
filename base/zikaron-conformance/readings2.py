#!/usr/bin/env python3
"""Other-reading variants of the zikaron/2 criterion over corpus2 (a measure,
never a criterion; see BOUNDARY-AUDIT.md).

bcov.py asks whether both arms of every branch of the criterion are taken.
It cannot see a point where the criterion does not branch at all and a
careful reader of the law might: a parser that tests depth on every bracket
byte, a pile collapsed by spelling, a seat that stops moving after a gap.
This instrument asks the opposite question. For every point of
docs/zikaron-v2.md where another reading could be taken, the table READINGS
holds one variant: the criterion with that one reading swapped in and
nothing else. A corpus that cannot tell a variant from the criterion lets
an implementation that took that reading pass compare2.py, so the gate is:

    every variant is told apart from the criterion by at least one case of
    the corpus `corpus2/gen.py --seed N` builds, for every N in the run.

Usage:  readings2.py [--seeds 1-6] [--only ID[,ID...]] [--jobs N] [--list]
Exits 0 when every variant is caught at every seed, 1 otherwise. A variant
no seed catches has shown no difference from the criterion at all: read it
again before growing the corpus, since a variant that cannot differ is no
reading. Nothing is written into the tree: the corpora are generated, and
the variants are built, in a temporary directory that is removed at the end.

A row of READINGS is (id, clause, the other reading in a sentence, edits),
an edit being (file, text, replacement) applied to a copy of the criterion's
sources; `text` must occur exactly once in the file, so a variant whose
anchor a change of the criterion has moved fails loudly and is never
silently the criterion itself. Tooling: rebuild, replace, or delete freely.
"""
import argparse, concurrent.futures, hashlib, json, os, shutil, subprocess, sys, tempfile

ROOT = os.path.dirname(os.path.abspath(__file__))
V1 = "impl-py"
V2 = "impl2-py"
SOURCES = {V1: ("zkcanon.py", "zkcrypto.py", "zkentry.py", "zkaudit.py"),
           V2: ("zk2.py", "zk2entry.py", "zk2audit.py")}

READINGS = []


def reading(rid, clause, other, *edits):
    assert all(r[0] != rid for r in READINGS), rid
    # the worker remembers the curve and the hash across programs, so no row may edit them
    assert edits and all(e[0] != V1 + "/zkcrypto.py" for e in edits), rid
    READINGS.append((rid, clause, other, edits))


# ==========================================================================
# the table
# ==========================================================================
CANON = V1 + "/zkcanon.py"
ENTRY1 = V1 + "/zkentry.py"
AUDIT1 = V1 + "/zkaudit.py"
ENTRY2 = V2 + "/zk2entry.py"
AUDIT2 = V2 + "/zk2audit.py"


def _lines(rel):
    return open(os.path.join(ROOT, rel), encoding="utf-8").read().split("\n")


def line(rel, needle):
    """The one line of `rel` carrying `needle`, whole."""
    hits = [l for l in _lines(rel) if needle in l]
    assert len(hits) == 1, (rel, needle, len(hits))
    return hits[0] + "\n"


def stmt(rel, token):
    """The statement of `rel` that names `token`: from its first line, the
    nearest line above at a function body's own indentation, through the
    line naming the token."""
    ls = _lines(rel)
    hits = [k for k, l in enumerate(ls) if 'Fault("%s")' % token in l or 'fail("%s")' % token in l]
    assert len(hits) == 1, (rel, token, len(hits))
    j = hits[0]
    while not (ls[j].startswith("    ") and ls[j][4:5] != " "):
        j -= 1
    return "\n".join(ls[j:hits[0] + 1]) + "\n"


def swap(rel, first, second, between=""):
    """The edit that decides `second` before `first`, two statements that
    stand one after the other in `rel` (with `between` between them)."""
    return (rel, first + between + second, between + second + first)


# --------------------------------------------------------------------------
# s3: the parse, the six tests
# --------------------------------------------------------------------------

# The variant parser tests depth before the grammar at one place where the
# grammar refuses a bracket (s3.5 test 2: "a bracket the grammar refuses
# opens no container"). One row per place, since a parser may take the
# other reading at one of them and not at the rest.
_DEPTH = '''if self.b[self.i] in (0x7B, 0x5B) and depth + 1 > MAX_DEPTH:
                raise Fault("E_DEPTH")
            '''

reading("depth-at-member-name", "s3.5",
        "a bracket where a member name is expected, inside 128 containers, fails E_DEPTH",
        (CANON, '''            if self.b[self.i] != 0x22:
                raise Fault("E_JSON")''',
                '''            ''' + _DEPTH + '''if self.b[self.i] != 0x22:
                raise Fault("E_JSON")'''))

reading("depth-at-colon", "s3.5",
        "a bracket where a member's colon is expected, inside 128 containers, fails E_DEPTH",
        (CANON, '''            if self.i >= self.n or self.b[self.i] != 0x3A:
                raise Fault("E_JSON")''',
                '''            if self.i >= self.n:
                raise Fault("E_JSON")
            ''' + _DEPTH + '''if self.b[self.i] != 0x3A:
                raise Fault("E_JSON")'''))

reading("depth-after-member", "s3.5",
        "a bracket after a member's value, inside 128 containers, fails E_DEPTH",
        (CANON, '''            if c == 0x7D:
                self.i += 1
                return Obj(pairs)
            raise Fault("E_JSON")''',
                '''            if c == 0x7D:
                self.i += 1
                return Obj(pairs)
            ''' + _DEPTH + '''raise Fault("E_JSON")'''))

reading("depth-after-element", "s3.5",
        "a bracket after an array's element, inside 128 containers, fails E_DEPTH",
        (CANON, '''            if c == 0x5D:
                self.i += 1
                return items
            raise Fault("E_JSON")''',
                '''            if c == 0x5D:
                self.i += 1
                return items
            ''' + _DEPTH + '''raise Fault("E_JSON")'''))

reading("number-fails-where-rfc-refuses", "s3.1",
        "a number ends where RFC 8259's number grammar ends, so `1-2` and `01` fail E_JSON at the byte after it",
        (CANON, '''        while i < n and b[i] in _NUMRUN:
            i += 1
        v = int_literal(b[start:i])''',
                '''        while i < n and b[i] in _NUMRUN:
            i += 1
        j = start + 1 if b[start] == 0x2D else start
        k = j
        while k < i and 0x30 <= b[k] <= 0x39:
            k += 1
        if k > j:
            m = j + 1 if b[j] == 0x30 else k
            if m < i and b[m] == 0x2E:
                f = m + 1
                while f < i and 0x30 <= b[f] <= 0x39:
                    f += 1
                if f > m + 1:
                    m = f
            if m < i and b[m] in b"eE":
                f = m + 2 if m + 1 < i and b[m + 1] in b"+-" else m + 1
                g = f
                while g < i and 0x30 <= b[g] <= 0x39:
                    g += 1
                if g > f:
                    m = g
            i = m
        v = int_literal(b[start:i])'''))

reading("utf8-inside-strings-only", "s3.5",
        "a byte-level parser that decodes strings alone: an invalid byte outside a string, or after an earlier fault, "
        "is not E_UTF8",
        (CANON, '''    try:
        b.decode("utf-8")
    except UnicodeDecodeError:
        raise Fault("E_UTF8")
    v = _P(b).text()''',
                '''    try:
        v = _P(b).text()
    except UnicodeDecodeError:
        raise Fault("E_UTF8")'''))

reading("utf8-up-to-the-first-fault", "s3.5",
        "test 1 reaches no further than the parse does: a test 2 fault whose byte precedes the first invalid byte "
        "is the fault named",
        (CANON, '''    try:
        b.decode("utf-8")
    except UnicodeDecodeError:
        raise Fault("E_UTF8")
    v = _P(b).text()''',
                '''    bad = None
    try:
        b.decode("utf-8")
    except UnicodeDecodeError as ex:
        bad = ex.start
    try:
        v = _P(b).text()
    except UnicodeDecodeError:
        raise Fault("E_UTF8")
    except Fault as f:
        import sys
        tb = sys.exc_info()[2]
        while tb.tb_next is not None:
            tb = tb.tb_next
        loc = tb.tb_frame.f_locals
        at = loc.get("start", loc["i"]) if "i" in loc else loc["self"].i
        if bad is not None and (f.token != "E_NUMBER" and at >= bad or at > bad):
            raise Fault("E_UTF8")
        raise
    if bad is not None:
        raise Fault("E_UTF8")'''))

reading("tests-3-4-5-by-position", "s3.5",
        "tests 3, 4 and 5 are one pass in document order, the first fault met being the one named",
        (CANON, '''    _check_dup(v)
    _check_keys(v)
    _check_values(v, False)''',
                '''    def one_pass(x, prose):
        if isinstance(x, Obj):
            seen = set()
            for k, val in x.pairs:
                if k in seen:
                    raise Fault("E_DUP_KEY")
                seen.add(k)
                if len(k) == 0 or not _skeleton(k):
                    raise Fault("E_KEY_CHARSET")
                one_pass(val, prose or k.endswith("_md"))
        elif isinstance(x, list):
            for e in x:
                one_pass(e, prose)
        elif isinstance(x, str):
            if not prose and not _skeleton(x):
                raise Fault("E_VALUE_CHARSET")
    one_pass(v, False)'''))

reading("string-root-is-no-string-value", "s3.5",
        "test 5 ranges over member values and elements, so a string at the root passes it",
        (CANON, '''    _check_values(v, False)
    return v''', '''    if not isinstance(v, str):
        _check_values(v, False)
    return v'''))

reading("surrogate-pair-read-as-one-scalar", "s3.3",
        "a high surrogate escape followed by a low one is the scalar value RFC 8259 says the pair spells",
        (CANON, '''                if 0xD800 <= cp <= 0xDFFF:
                    raise Fault("E_JSON")
                parts.append(chr(cp))
                i += 5''',
                '''                lo = b[i + 7:i + 11]
                if (0xD800 <= cp <= 0xDBFF and b[i + 5:i + 7] == b"\\\\u" and len(lo) == 4
                        and all(ch in _HEXD for ch in lo) and 0xDC00 <= int(lo.decode("ascii"), 16) <= 0xDFFF):
                    parts.append(chr(0x10000 + ((cp - 0xD800) << 10) + int(lo.decode("ascii"), 16) - 0xDC00))
                    i += 11
                elif 0xD800 <= cp <= 0xDFFF:
                    raise Fault("E_JSON")
                else:
                    parts.append(chr(cp))
                    i += 5'''))

reading("prose-stops-at-a-nested-object", "s3.3",
        "a prose subtree reaches the member's own value and its arrays, and an object inside it starts over",
        (CANON, '''stack.append((val, p or k.endswith("_md")))''', '''stack.append((val, k.endswith("_md")))'''))

reading("empty-key-admitted", "s3.3",
        "the key charset is the skeleton range alone, so the empty key passes test 4",
        (CANON, '''                if len(k) == 0 or not _skeleton(k):''', '''                if not _skeleton(k):'''))

# --------------------------------------------------------------------------
# s4.3, s5.4, s5.5: the order of the steps
# --------------------------------------------------------------------------

_T = lambda tok: stmt(ENTRY2, tok)
_PREV = line(ENTRY2, 'prev = d["prev"]')
_BODY = line(ENTRY2, '_check_body(d["entryType"], d["body"])')
_SIG = line(ENTRY2, "b6 = canon(Obj([") + line(ENTRY2, "check_sig(entry_digest(b6)")

for _a, _b, _mid in (("E_ENVELOPE_MISSING", "E_ENVELOPE_CLOSED", ""), ("E_ENVELOPE_CLOSED", "E_SPEC", ""),
                     ("E_SPEC", "E_ENTRYTYPE", ""), ("E_ENTRYTYPE", "E_AUTHOR", ""), ("E_AUTHOR", "E_SEQ", ""),
                     ("E_SEQ", "E_PREV", _PREV), ("E_PREV", "E_PREV_SEQ", ""), ("E_PREV_SEQ", "E_BODY", ""),
                     ("E_BODY", "E_SIG_FORM", ""), ("E_SIG_FORM", "E_GENESIS_PLACE", "")):
    reading("step-%s-before-%s" % (_b[2:].lower().replace("_", "-"), _a[2:].lower().replace("_", "-")), "s4.3",
            "%s is decided before %s" % (_b, _a), swap(ENTRY2, _T(_a), _T(_b), _mid))

reading("step-body-field-before-genesis-place", "s4.3", "step 12 is decided before step 11",
        swap(ENTRY2, _T("E_GENESIS_PLACE"), _BODY))
reading("step-signature-before-body-field", "s4.3", "step 13 is decided before step 12",
        swap(ENTRY2, _BODY, _SIG))

_S = lambda tok: stmt(ENTRY1, tok)
_REC = line(ENTRY1, "pt = recover(digest, r, s, v - 27)")
reading("sig-range-before-v", "s5.4", "E_SIG_RANGE is decided before E_SIG_V",
        swap(ENTRY1, _S("E_SIG_V"), _S("E_SIG_RANGE")))
reading("sig-high-s-before-range", "s5.4", "E_SIG_HIGH_S is decided before E_SIG_RANGE",
        swap(ENTRY1, _S("E_SIG_RANGE"), _S("E_SIG_HIGH_S")))
reading("sig-recover-before-high-s", "s5.4", "E_SIG_RECOVER is decided before E_SIG_HIGH_S",
        (ENTRY1, _S("E_SIG_HIGH_S") + _REC + _S("E_SIG_RECOVER"), _REC + _S("E_SIG_RECOVER") + _S("E_SIG_HIGH_S")))
reading("sig-half-order-is-high", "s5.4", "s equal to (n - 1) / 2 is already high",
        (ENTRY1, "    if s > HALF_N:", "    if s >= HALF_N:"))

# --------------------------------------------------------------------------
# s6: bodies
# --------------------------------------------------------------------------

reading("window-of-one-instant-refused", "s6.3", "a window whose from equals its to fails E_BODY_FIELD",
        (ENTRY2, '''if wd["from"] > wd["to"]:''', '''if wd["from"] >= wd["to"]:'''))
reading("optional-field-null-admitted", "s6", "an optional body field present as null is read as absent",
        (ENTRY2, '''    def opt(k, test):
        if k in d and not test(d[k]):''', '''    def opt(k, test):
        if k in d and d[k] is not None and not test(d[k]):'''))

# --------------------------------------------------------------------------
# s9.4: the audit's inputs
# --------------------------------------------------------------------------

reading("pile-by-spelling", "s8.1",
        "pile elements collapse by their hex spelling, so one byte string spelled in two cases is two",
        (AUDIT2, '''    pile = [v1audit._hexbytes(x) for x in d["pile"]]''',
                 '''    pile = [v1audit._hexbytes(x) for x in dict.fromkeys(
        x if isinstance(x, str) else repr(x) for x in d["pile"])]'''),
        (AUDIT2, '''        if b in seen:
            continue
        seen.add(b)
        try:
            e = validator(b)''',
                 '''        seen.add(b)
        try:
            e = validator(b)'''))

reading("basis-empty-lists-refused", "s9.4", "registries and senders each carry at least one address",
        (AUDIT2, '''            lists[arr] = tuple(cd[arr])''', '''            lists[arr] = tuple(cd[arr])
            need(len(cd[arr]) > 0)'''))
reading("basis-adjacent-twins-admitted", "s9.4",
        "two objects of one chain whose ranges touch may carry the same registries and senders",
        (AUDIT2, '''                if fb == prev[2] + 1:
                    need(lists["registries"] != prev[3] or lists["senders"] != prev[4])
''', ''))
reading("basis-repeated-address-admitted", "s9.4", "an address list is ascending and may repeat an element",
        (AUDIT2, '''need(prev_a is None or prev_a.encode("ascii") < a.encode("ascii"))''',
                 '''need(prev_a is None or prev_a.encode("ascii") <= a.encode("ascii"))'''))
reading("anchor-conflict-first-wins", "s9.4",
        "of two records agreeing on (chainId, blockNumber, tx, hash) and differing elsewhere the first read stands",
        (AUDIT2, '''            need(prior.blockTimestamp == rec.blockTimestamp and prior.sender == rec.sender
                 and prior.verdict == rec.verdict)''', '''            pass'''))
reading("anchor-out-of-reach-dropped", "s9.4",
        "a record the declared basis does not reach is left out of the anchor set and the audit goes on",
        (AUDIT2, '''    for rec in anchors.values():
        need(_reaches(d["basis"], rec))
    return root, pile, list(anchors.values()), d["basis"]''',
                 '''    return root, pile, [rec for rec in anchors.values() if _reaches(d["basis"], rec)], d["basis"]'''))
reading("reach-by-chain-and-block-alone", "s9.4", "a basis reaches a record by chain and block range, whatever its sender",
        (AUDIT2, '''
                and rec.sender in cd["senders"]):''', '''):'''))

# --------------------------------------------------------------------------
# s7.4: retirement, timeliness, lineage
# --------------------------------------------------------------------------

reading("retirement-by-counted-records", "s7.4", "only a counted record of a succession retires its signer",
        (AUDIT2, '''        if e is None or e.etype not in ("succession", "migration"):
            continue''', '''        if e is None or e.etype not in ("succession", "migration") or a.verdict != "counted":
            continue'''))
reading("retirement-by-succession-alone", "s7.4", "a migration retires nobody",
        (AUDIT2, '''        if e is None or e.etype not in ("succession", "migration"):
            continue''', '''        if e is None or e.etype != "succession":
            continue'''))
reading("retirement-second-bounds-nothing", "s7.4",
        "only a record earlier than the retirement makes a byte string timely",
        (AUDIT2, '''            if a.blockTimestamp < r or (a.blockTimestamp == r and e.seq <= rseq[e.author]):''',
                 '''            if a.blockTimestamp < r:'''))
reading("retirement-second-bounds-every-seq", "s7.4",
        "a record of the retirement's own second makes timely whatever lies on its line, above the retiring seq too",
        (AUDIT2, '''            if a.blockTimestamp < r or (a.blockTimestamp == r and e.seq <= rseq[e.author]):''',
                 '''            if a.blockTimestamp <= r:'''))
reading("retiring-seq-is-the-largest", "s7.4",
        "of the successions anchored in the retirement's second the one of largest seq gives the retiring seq",
        (AUDIT2, '''        elif a.blockTimestamp == prior and e.seq < rseq[e.author]:''',
                 '''        elif a.blockTimestamp == prior and e.seq > rseq[e.author]:'''))
reading("timely-line-of-one-author", "s7.4",
        "the line a record bounds stops at the first byte string of another author",
        (AUDIT2, '''            line = _line(f, by_eid)
            cache[f.eid] = line''', '''            line = _line(f, by_eid)
            line = line[:next((k for k, x in enumerate(line) if x.author != f.author), len(line))]
            cache[f.eid] = line'''))
reading("retired-at-every-record-of-its-second", "s7.4",
        "a key has retired at every record of its retirement's own second",
        (AUDIT2, '''    if rec.blockTimestamp > r:
        return True''', '''    if rec.blockTimestamp >= r:
        return True'''))
reading("trim-keeps-a-retired-sender", "s8.1", "the trim discards a record for its sender's lineage alone",
        (AUDIT2, '''    discarded = [a for a in anchors if a.sender not in lin or has_retired(a, ret, rseq, by_eid)]
    kept = [a for a in anchors if a.sender in lin and not has_retired(a, ret, rseq, by_eid)]''',
                 '''    discarded = [a for a in anchors if a.sender not in lin]
    kept = [a for a in anchors if a.sender in lin]'''))
reading("lineage-through-untimely-successions", "s7.4",
        "a succession that is not timely still brings its `to` into the lineage",
        (AUDIT2, '''if e.etype == "succession" and e.eid in timely and e.author in lin:''',
                 '''if e.etype == "succession" and e.author in lin:'''))
reading("lineage-by-seat-movement", "s7.4", "only a succession that moves the seat extends the lineage",
        (AUDIT2, '''    lin = {seed}
    changed = True
    while changed:
        changed = False
        for e in entries:
            if e.etype == "succession" and e.eid in timely and e.author in lin:
                to = e.body.dict()["to"]
                if to not in lin:
                    lin.add(to)
                    changed = True
    return lin''', '''    lin = {seed}
    auth = seed
    by_seq = {}
    for e in entries:
        if e.eid in timely:
            by_seq.setdefault(e.seq, []).append(e)
    for s in sorted(by_seq):
        succs = [e for e in by_seq[s] if e.etype == "succession" and e.author == auth]
        if succs:
            auth = min(succs, key=lambda e: e.eidb).body.dict()["to"]
            lin.add(auth)
    return lin'''))

# --------------------------------------------------------------------------
# s8.2, s8.4: the walk
# --------------------------------------------------------------------------

reading("seat-frozen-after-gap", "s8.2", "once the walk is not certain a succession no longer moves the seat",
        (AUDIT2, '''                if succs:
                    auth = min(succs''', '''                if succs and certain:
                    auth = min(succs'''))
reading("authority-ungraded-after-gap", "s8.2", "once the walk is not certain authority is no longer compared",
        (AUDIT2, '''            if e.seq >= 1 and (closed or e.author != auth):''',
                 '''            if certain and e.seq >= 1 and (closed or e.author != auth):'''))
reading("gap-entry-graded-while-certain", "s8.2",
        "the entry that opens a gap is graded for authority before the gap is recorded, so its mismatch is hard",
        (AUDIT2, '''        for e in group:
            if e.seq == expected:''', '''        for e in group:
            was = certain
            if e.seq == expected:'''),
        (AUDIT2, '''[("seq", e.seq), ("certain", certain)], certain))''', '''[("seq", e.seq), ("certain", was)], was))'''))
reading("fork-twin-gaps-again", "s8.2", "a fork twin is compared with `expected` alone and records a gap of its own",
        (AUDIT2, '''            elif prev_seq is not None and e.seq == prev_seq:
                pass''', '''            elif False:
                pass'''))
reading("prev-across-a-gap", "s8.2", "an entry with no ledger entry at the seq before it records PREV_MISMATCH",
        (AUDIT2, '''                if pred and not any(x.eid == e.prev for x in pred):''',
                 '''                if not pred or not any(x.eid == e.prev for x in pred):'''))
reading("prev-against-the-first-twin", "s8.2",
        "at a fork the link is tested against the twin of smallest entry_id alone",
        (AUDIT2, '''                if pred and not any(x.eid == e.prev for x in pred):''',
                 '''                if pred and min(pred, key=lambda x: x.eidb).eid != e.prev:'''))
reading("authority-graded-at-seq-0", "s8.2", "an entry at seq 0 by another key records AUTHORITY_MISMATCH too",
        (AUDIT2, '''            if e.seq >= 1 and (closed or e.author != auth):''',
                 '''            if closed or e.author != auth:'''))
reading("succession-out-of-office-moves-seat", "s8.2",
        "a succession by any key of the ledger moves the seat, whoever holds it",
        (AUDIT2, '''succs = [e for e in group if e.etype == "succession" and e.author == auth]''',
                 '''succs = [e for e in group if e.etype == "succession"]'''))
reading("succession-twin-of-largest-id", "s8.2",
        "of two successions by the key in office at one position the larger entry_id moves the seat",
        (AUDIT2, '''                    auth = min(succs, key=lambda e: e.eidb).body.dict()["to"]''',
                 '''                    auth = max(succs, key=lambda e: e.eidb).body.dict()["to"]'''))
reading("migration-out-of-office-closes", "s8.2", "a migration by any key of the ledger closes the walk",
        (AUDIT2, '''if any(e.etype == "migration" and e.author == auth for e in group):''',
                 '''if any(e.etype == "migration" for e in group):'''))
reading("migration-yields-to-a-succession", "s8.2",
        "where a migration and a succession by the key in office share a position the succession moves the seat",
        (AUDIT2, '''            if any(e.etype == "migration" and e.author == auth for e in group):
                closed = True
            else:
                succs = [e for e in group if e.etype == "succession" and e.author == auth]
                if succs:
                    auth = min(succs, key=lambda e: e.eidb).body.dict()["to"]''',
                 '''            succs = [e for e in group if e.etype == "succession" and e.author == auth]
            if succs:
                auth = min(succs, key=lambda e: e.eidb).body.dict()["to"]
            elif any(e.etype == "migration" and e.author == auth for e in group):
                closed = True'''))
reading("equivocation-by-seq-alone", "s8.4", "two entries sharing a prev at different seqs do not equivocate",
        (AUDIT2, '''            if a.seq == b.seq or (a.prev is not None and a.prev == b.prev):''',
                 '''            if a.seq == b.seq:'''))
reading("equivocation-at-the-larger-seq", "s8.4", "an equivocation is recorded at the larger of the two seqs",
        (AUDIT2, '''                seq = a.seq if a.seq < b.seq else b.seq''', '''                seq = a.seq if a.seq > b.seq else b.seq'''))

# --------------------------------------------------------------------------
# s8.5, s8.6: reconciliation and the report
# --------------------------------------------------------------------------

reading("findings-in-walk-order", "s8.6", "at one position the findings list in the order the walk tests them",
        (AUDIT2, '''    return (position, name.encode("ascii"), entry_id.encode("ascii"),''',
                 '''    return (position, bytes([("SEQ_GAP", "PREV_MISMATCH", "AUTHORITY_MISMATCH", "ROOT_MISMATCH",
                               "EQUIVOCATION").index(name)]), entry_id.encode("ascii"),'''))
reading("anchored-by-any-verdict", "s8.5", "a kept record of any verdict anchors and can leave a hash missing",
        (AUDIT2, '''    counted = [a for a in kept if a.verdict == "counted"]''', '''    counted = list(kept)'''))
reading("missing-skips-bytes-in-hand", "s8.5",
        "a counted hash whose bytes the pile holds is not missing, whatever those bytes are",
        (AUDIT2, '''    for h in sorted(h for h in counted_hashes if h not in have):''',
                 '''    for h in sorted(h for h in counted_hashes if h not in have and h not in pile_ids):'''))
reading("unproven-label-despite-a-counted-record", "s8.6",
        "an UNPROVEN record of bytes not in hand moves the label though a counted record names the same hash",
        (AUDIT2, '''    unproven_open = any(a.hash not in pile_ids and a.hash not in counted_hashes''',
                 '''    unproven_open = any(a.hash not in pile_ids'''))
reading("unproven-label-despite-bytes-in-hand", "s8.6",
        "an UNPROVEN record no counted record answers moves the label though the pile holds its bytes",
        (AUDIT2, '''    unproven_open = any(a.hash not in pile_ids and a.hash not in counted_hashes''',
                 '''    unproven_open = any(a.hash not in counted_hashes'''))
reading("unproven-label-reads-its-own-segment", "s8.6",
        "item 15's pile is the new segment's pile, not the whole pile of a continued ledger",
        (AUDIT2, '''for b in whole_pile)''', '''for b in pile)'''))
reading("gaps-before-unproven", "s8.6", "GAPS is named before UNPROVEN",
        (AUDIT2, '''    elif unproven_open:
        label = "UNPROVEN"
    elif any_gap or missing:
        label = "GAPS"''', '''    elif any_gap or missing:
        label = "GAPS"
    elif unproven_open:
        label = "UNPROVEN"'''))
reading("missing-alone-is-complete", "s8.6", "only a gap of the walk names GAPS",
        (AUDIT2, '''    elif any_gap or missing:''', '''    elif any_gap:'''))

# --------------------------------------------------------------------------
# s12.6: the continuation of a zikaron/1 ledger
# --------------------------------------------------------------------------

reading("cut-at-the-latest-record", "s12.6", "the cut is the latest record of the closing succession",
        (AUDIT2, '''        cut = min(a.blockTimestamp for a in recs)''', '''        cut = max(a.blockTimestamp for a in recs)'''))
reading("handing-off-in-the-cut-second", "s12.6", "a handing off anchored in the cut's own second closes nothing",
        (AUDIT2, '''if any(a.blockTimestamp < cut for a in by_hash.get(e.eid, [])):''',
                 '''if any(a.blockTimestamp <= cut for a in by_hash.get(e.eid, [])):'''))
reading("handing-off-to-itself", "s12.6", "a succession of the key to itself before the cut is a handing off",
        (AUDIT2, '''if e.etype == "succession" and e.author == s.author and e.body.dict().get("to") != s.author:''',
                 '''if e.etype == "succession" and e.author == s.author and e.eid != s.eid:'''))
reading("continuation-of-the-latest-cut", "s12.6", "of two candidates the later cut is the continuation",
        (AUDIT2, '''if best is None or key < best[0]:''', '''if best is None or key > best[0]:'''))
reading("closing-succession-of-any-kind", "s12.6", "the closing succession's kind is not read",
        (AUDIT2, '''        if sd.get("to") != g.author or sd.get("kind") != "migration":''',
                 '''        if sd.get("to") != g.author:'''))
reading("closing-succession-outside-the-old-ledger", "s12.6",
        "a closing succession the old segment's audit excludes still continues",
        (AUDIT2, '''        if s.eid not in _v1_ledger_ids(report):
            continue
''', ''))
reading("old-records-by-time-alone", "s12.6", "a record later than the cut is a new record whatever it anchors",
        (AUDIT2, '''    recs_old = [a for a in anchors if a.blockTimestamp <= cut or a.hash in old_ids]''',
                 '''    recs_old = [a for a in anchors if a.blockTimestamp <= cut]'''))
reading("old-bytes-bring-no-line", "s12.6",
        "a byte string anchored at or before the cut is old alone, its line along prev is not",
        (AUDIT2, '''            e = v1_by_raw.get(b)
            if e is not None:
                for x in _line(e, v1_by_eid):
                    old.add(x.raw)
''', ''))
reading("new-pile-is-the-whole-pile", "s12.6", "the new segment reads the whole pile, the old entries included",
        (AUDIT2, '''    new_pile = [b for b in pile if b not in old_v1]''', '''    new_pile = list(pile)'''))
reading("old-unavailable-reads-as-gaps", "s12.6", "the old report's UNAVAILABLE combines as GAPS",
        (AUDIT2, '''other = "UNPROVEN" if v1label == "UNAVAILABLE" else v1label''',
                 '''other = "GAPS" if v1label == "UNAVAILABLE" else v1label'''))
reading("old-seat-frozen-after-gap", "s12.6",
        "in the old segment, once the walk is not certain a succession no longer moves the seat",
        (AUDIT1, '''        if succs:
            auth = min(succs''', '''        if succs and certain:
            auth = min(succs'''))
reading("old-authority-ungraded-after-gap", "s12.6",
        "in the old segment, once the walk is not certain authority is no longer compared",
        (AUDIT1, '''            if e.seq >= 1 and e.author != auth:''', '''            if certain and e.seq >= 1 and e.author != auth:'''))
reading("old-lineage-by-seat-movement", "s12.6",
        "in the old segment, only a succession that moves the seat extends the lineage",
        (AUDIT1, '''def _lineage(root, entries):
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
    return lin''', '''def _lineage(root, entries):
    lin = {root}
    auth = root
    by_seq = {}
    for e in entries:
        by_seq.setdefault(e.seq, []).append(e)
    for s in sorted(by_seq):
        succs = [e for e in by_seq[s] if e.etype == "succession" and e.author == auth]
        if succs:
            auth = min(succs, key=lambda e: e.eidb).body.dict()["to"]
            lin.add(auth)
    return lin'''))


# ==========================================================================
# the worker: one program (the criterion or a variant) over one corpus
# ==========================================================================

class _Misuse(Exception):
    pass


def worker(impl_root, corpus, memo_in, out, memo_out):
    """Answer every canon, sign and audit case of `corpus`'s manifest with
    the program under `impl_root`, through its own command functions, and
    write {case path: [sha256 of its answers, whether the program broke]}.
    The misuse cases turn on HARNESS2.md's argument rules, which no row of
    the table edits, and are left to compare2.py. Signature recovery and
    Keccak-256, pure functions no row of the table edits, are remembered
    across programs so that a variant's pass costs the parse and the walk
    alone."""
    sys.setrecursionlimit(10000)
    sys.path.insert(0, os.path.join(impl_root, V2))
    import zk2, zkcrypto, zkentry
    memo = json.load(open(memo_in)) if memo_in != "-" else {}
    plain_recover, plain_keccak = zkcrypto.recover, zkcrypto.keccak256

    def recover(digest32, r, s, i):
        key = "r%s:%x:%x:%d" % (digest32.hex(), r, s, i)
        if key not in memo:
            memo[key] = plain_recover(digest32, r, s, i)
        pt = memo[key]
        return None if pt is None else tuple(pt)

    def keccak256(data):
        key = "k" + bytes(data).hex()
        if key not in memo:
            memo[key] = plain_keccak(data).hex()
        return bytes.fromhex(memo[key])

    zkcrypto.recover = zkentry.recover = recover
    zkcrypto.keccak256 = keccak256
    got = []
    zk2.emit = lambda v: got.append(zk2.canon(v))

    def die(msg):
        raise _Misuse(msg)

    zk2.die = die
    answers = {}
    for c in json.load(open(os.path.join(corpus, "manifest.json")))["cases"]:
        cat = c["category"]
        if cat == "canon":
            calls = [(zk2.cmd_check, [c["path"]]), (zk2.cmd_canon, [c["path"]])]
        elif cat == "audit":
            calls = [(zk2.cmd_audit, [c["path"]])]
        elif cat == "sign":
            calls = [(zk2.cmd_sign, [c["privkey"], c["path"]] + ([c["domain"]] if "domain" in c else []))]
        else:
            continue
        h = hashlib.sha256()
        broke = False
        for fn, args in calls:
            args = [os.path.join(corpus, a) if a == c["path"] else a for a in args]
            del got[:]
            try:
                fn(args)
                ans = b"".join(got)
            except _Misuse:
                ans = b"<exit 2>"
            except Exception as ex:      # an edit may break a program where the criterion answers
                ans = b"<%s>" % type(ex).__name__.encode()
                broke = True
            h.update(b"%d:" % len(ans) + ans)
        answers[c["path"]] = [h.hexdigest(), broke]
    json.dump(answers, open(out, "w"))
    if memo_out != "-":
        json.dump(memo, open(memo_out, "w"))


# ==========================================================================
# the gate
# ==========================================================================

def build_variant(tmp, row):
    rid, _clause, _other, edits = row
    dst = os.path.join(tmp, "v", rid)
    for d, names in SOURCES.items():
        os.makedirs(os.path.join(dst, d))
        for n in names:
            shutil.copy(os.path.join(ROOT, d, n), os.path.join(dst, d, n))
    for rel, text, repl in edits:
        p = os.path.join(dst, rel)
        src = open(p, encoding="utf-8").read()
        if src.count(text) != 1:
            sys.exit("readings2: %s: the anchor of its edit to %s occurs %d times, not once:\n%s"
                     % (rid, rel, src.count(text), text))
        open(p, "w", encoding="utf-8").write(src.replace(text, repl))
    return dst


def generate(tmp, seed):
    d = os.path.join(tmp, "s%d" % seed)
    for c in ("corpus", "corpus2"):
        os.makedirs(os.path.join(d, c))
        shutil.copy(os.path.join(ROOT, c, "gen.py"), os.path.join(d, c, "gen.py"))
    p = subprocess.run([sys.executable, "-B", "gen.py", "--seed", str(seed)],
                       cwd=os.path.join(d, "corpus2"), capture_output=True)
    if p.returncode != 0:
        sys.exit("readings2: gen.py --seed %d failed:\n%s" % (seed, p.stderr.decode("utf-8", "replace")[-2000:]))
    return os.path.join(d, "corpus2")


def run_worker(impl_root, corpus, memo_in, out, memo_out="-"):
    p = subprocess.run([sys.executable, "-B", os.path.abspath(__file__), "--worker",
                        impl_root, corpus, memo_in, out, memo_out], capture_output=True)
    if p.returncode != 0:
        sys.exit("readings2: the pass of %s over %s failed:\n%s"
                 % (impl_root, corpus, p.stderr.decode("utf-8", "replace")[-2000:]))
    return json.load(open(out))


def parse_seeds(s):
    out = []
    for part in s.split(","):
        a, _, b = part.partition("-")
        out.extend(range(int(a), int(b or a) + 1))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--seeds", default="1-6")
    ap.add_argument("--only", default="")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--list", action="store_true")
    a = ap.parse_args()
    rows = [r for r in READINGS if not a.only or r[0] in a.only.split(",")]
    if not rows:
        sys.exit("readings2: --only names no row of the table (see --list)")
    if a.list:
        for rid, clause, other, _e in rows:
            print("%-34s %-8s %s" % (rid, clause, other))
        return 0
    seeds = parse_seeds(a.seeds)
    tmp = tempfile.mkdtemp(prefix="readings2-")
    try:
        with concurrent.futures.ThreadPoolExecutor(a.jobs) as ex:
            variants = dict((r[0], build_variant(tmp, r)) for r in rows)
            corpora = dict(zip(seeds, ex.map(lambda s: generate(tmp, s), seeds)))
            crit = dict(zip(seeds, ex.map(
                lambda s: run_worker(ROOT, corpora[s], "-", os.path.join(tmp, "c%d.json" % s),
                                     os.path.join(tmp, "memo%d.json" % s)), seeds)))
            jobs = [(r[0], s) for r in rows for s in seeds]
            got = dict(zip(jobs, ex.map(
                lambda j: run_worker(variants[j[0]], corpora[j[1]], os.path.join(tmp, "memo%d.json" % j[1]),
                                     os.path.join(tmp, "%s.%d.json" % j)), jobs)))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    width = max(len(r[0]) for r in rows)
    print("%-*s %-7s %s" % (width, "variant", "clause", " ".join("s%-4d" % s for s in seeds)))
    missed = []
    for s in seeds:
        if any(broke for _d, broke in crit[s].values()):
            sys.exit("readings2: the criterion itself broke on a case of seed %d" % s)
    for rid, clause, other, _e in rows:
        cells = []
        first = None
        broken = 0
        for s in seeds:
            # a case the variant breaks on tells nothing of its reading and catches nothing
            diff = sorted(p for p in crit[s] if got[(rid, s)][p][0] != crit[s][p][0] and not got[(rid, s)][p][1])
            broken += sum(1 for p in crit[s] if got[(rid, s)][p][1])
            cells.append(len(diff))
            if diff and first is None:
                first = diff[0]
            if not diff:
                missed.append((rid, s))
        print("%-*s %-7s %s  %s%s" % (width, rid, clause, " ".join("%-5d" % n for n in cells),
                                      ("first: " + first) if first else "NEVER CAUGHT: " + other,
                                      ("  (broke on %d cases, not counted)" % broken) if broken else ""))
    cases = dict((s, len(crit[s])) for s in seeds)
    print("%d variants, seeds %s, cases per seed %s" % (len(rows), a.seeds, ",".join(str(cases[s]) for s in seeds)))
    if missed:
        by = {}
        for rid, s in missed:
            by.setdefault(rid, []).append(s)
        for rid, ss in by.items():
            print("MISSED %s at seed%s %s" % (rid, "" if len(ss) == 1 else "s", ",".join(map(str, ss))))
        return 1
    print("every variant is caught at every seed")
    return 0


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--worker":
        worker(*sys.argv[2:7])
        sys.exit(0)
    sys.exit(main())
