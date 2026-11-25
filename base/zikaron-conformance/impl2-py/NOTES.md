# Interpretive choices, `zikaron/2` in Python (impl2-py)

Written from `docs/zikaron-v2.md`, `docs/zikaron-v1.md` as §12.6 invokes it,
and `HARNESS2.md`. The core is three files over the `zikaron/1` core's own
`zkcanon.py` and `zkcrypto.py` (the canonical form and the curve are the same
text in both laws) and over the frozen `zikaron/1` core as a black box for a
continued ledger's old segment (`zkaudit.audit` fed the old segment as a
`HARNESS.md` input). Below is every place the text admits more than one
reading, with the reading taken.

## §7.4 retirement and timeliness

- **Retirement ranges over the anchor set as received**, every sender and
  every verdict, before the trim of §8.1. A record of a stranger, or an
  `UNPROVEN` or `VOID` record, anchoring a succession the key signed sets its
  retirement.
- **The byte strings a retirement reads are the segment's pile that pass §4.3
  under this law**: a `zikaron/1` succession the same key signed (in the old
  segment of a continued ledger) retires nobody here; that segment's law reads
  it.
- **Timeliness walks `prev` through byte strings of any author**, as the
  text says ("whatever their `author`"), with a visited set, stopping where
  no held byte string matches. A byte string is timely iff its author has no
  retirement, or some record earlier than that author's retirement, or of
  the retirement's own second while the byte string sits at or below that
  author's retiring seq, anchors a byte string on whose line it lies.
- **The retiring seq** is the smallest `seq` among the §4.3-passing
  successions and migrations of the key that a record of the retirement's
  own second anchors, any sender and any verdict; it is defined wherever the
  retirement is.
- **"Has retired at a record"** is `blockTimestamp` later than the
  retirement, or equal to it where the record's `hash` is the `entry_id` of
  a §4.3-passing byte string whose `author` is the key at a `seq` above its
  retiring seq. The predicate is of the key and the record alone and serves
  the trim, which asks it of the record's sender; timeliness reads the
  retirement and the retiring seq against the bounded byte string's own
  position, so a record of that second bounds, of the key's byte strings on
  the anchored line, those at or before its retiring seq and none above it,
  whoever sends the record and whatever it anchors. A hash-only record,
  junk, another key's byte string and the key's own byte strings at or
  before its retiring seq, anchored in that second, are records at which the
  key has not retired, and a successor's record of that second anchoring the
  ceding key's late byte string is kept by the trim.

## §8.1, §8.2

- The trim discards a record on either ground (sender outside the lineage,
  or sender retired at it); the `DISCARDED` row form does not say which.
- The migration closure is decided after every entry at a position has been
  visited, as the succession rule is, and only while the walk is not already
  closed; once closed, every later visited entry records `AUTHORITY_MISMATCH`
  (hard iff `certain`), and no succession reopens the walk.

## §8.6 item 15, rule 2

The pile rule 2 names is the whole pile as the audit received it, both
segments of a continued ledger included: a late `UNPROVEN` record of an
excluded old byte string is a new record over bytes in hand, and moves no
label. Every record whose hash is an
old-segment byte string passing `zikaron/1` §4.3 is an old record (§12.6),
so no record of the new segment names bytes the new segment's pile lacks
while the whole pile holds them, and the clause has no cross-segment case.

## §12.6 continuation

- **Candidates** are tested in full: a `seq`-0 byte string passing §4.3 whose
  `continues` is an object with `root` equal to the audited root and
  `entry_id` naming a byte string passing `zikaron/1` §4.3 that is a
  succession whose `author` and `to` both equal the genesis's `author` and
  whose `kind` is the token `migration`, anchored by at least one record of
  any sender and verdict, and a ledger entry of the `zikaron/1` audit over the
  old segment computed for that candidate. Ledger membership is read off the
  black-box report: an entry is a ledger entry iff its `entry_id` is in
  `anchored` or `unanchored` (`zikaron/1` §8.7 item 5 closes that partition).
- **A candidate's key has not handed off under `zikaron/1` before its cut**:
  where any record earlier than the candidate's cut, on any chain and
  whatever its sender and verdict, anchors a `zikaron/1` succession passing
  that law's §4.3 whose `author` is the closing succession's `author` and
  whose `to` is another key, the closing succession makes no candidate; a
  record in the cut's own second is not earlier, and a succession to itself
  hands nothing off. A seat whose `zikaron/1` key came back to it after an
  anchored handing off has no candidate.
- **The old segment's byte strings** are fixed first: the closing
  succession's line along `prev` through byte strings passing `zikaron/1`
  §4.3 (every byte string on the line passes it, the line stopping before
  one that does not); every pile byte string whose `entry_id` is the `hash`
  of a record whose `blockTimestamp` is not later than the cut, whatever test
  it passes; and, from each of those that passes `zikaron/1` §4.3, its line
  through byte strings passing that law. The new segment's pile is every byte
  string of the pile that is not an old-segment byte string passing
  `zikaron/1` §4.3, so junk and this law's entries anchored before the cut are
  in both.
- **The old segment's records** are those whose `blockTimestamp` is not later
  than the cut, and every record, whatever its time, whose `hash` is the
  `entry_id` of an old ledger entry: a `zikaron/1` ledger entry of the old
  byte strings, decided by that law's lineage (`zkaudit._lineage`) over the
  old byte strings alone. A late record of an excluded or malformed old
  byte string is a new record and meets this law's trim. The new segment's
  records are every other record, identified by `(chainId, blockNumber, tx,
  hash)`.
- **Every predicate of the new segment's audit**, retirement and timeliness
  included, reads the new segment's records and pile alone; a `zikaron/2`
  succession anchored at or before the cut retires nobody in the new
  segment.
- The old segment is handed to the `zikaron/1` core as a `HARNESS.md` input
  with empty `unavailable` and `evidence` and the basis spelled with empty
  `bareTx` and `adoptionChains`; the report that comes back is nested as
  `continues.report` byte for byte.
- The combined label is the first of `BROKEN_CHAIN`, `UNPROVEN`, `GAPS`,
  `COMPLETE` either report carries, `UNAVAILABLE` of the old report reading as
  `UNPROVEN`.

## Harness

- A `check` under this command applies `zikaron/2` §4.3 alone; a `zikaron/1`
  entry is `E_SPEC` here.
- The audit input object is open (a member beyond the four is ignored) and an
  anchor record is open beyond its seven members; the basis and its `chains`
  objects are arity-closed.
