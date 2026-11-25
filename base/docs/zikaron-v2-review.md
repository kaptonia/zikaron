# `zikaron/2`: the review record

The record of the second wire law's drafting, its rounds, and its naming.
The law is `docs/zikaron-v2.md`; its base is the frozen `zikaron/1`
(`docs/zikaron-v1.md`, core `0xbecf…32fc`, named 2026-09-05), which this law
leaves untouched and invokes for a continued ledger's old segment (§12.6).

## Revision of 2026-10-27: retirement, migration, continuation, pruning

**Why the law moved.** The judge ruled (2026-10-24, 2026-10-27; the package
`zikaron2-修法包-2026-10-27`) that a key handed off must do no harm anywhere
once an audit holds a record of its handing off; that a ledger moves from
`zikaron/1` to `zikaron/2` in one step under the same key, keeping its root
and its history; that the rules for moving a key, moving law and adding a
chain are one family form, GALEED's 2026-10-23 ledger law being the model;
and that the law keeps only what the client consumes: adoption, the bare
anchor form, the unavailable set and proof-kit rows go, `retraction`, `for`,
`upstream`, `ledger` and the continuation come in. The plan's verification
draft (the 核验稿) found that GALEED's lineage rule, copied as written, would
have broken offline verification of any ledger that ever changed hands,
since ZIKARON's pile is separate from its anchor set; the judge ruled for
timeliness as the one test (ruling 2 of the 纪要), and every clause below
follows from it.

**What changed, section by section.** The preamble names eight entry types,
one anchor form and three audit inputs. §1 says what `blockTimestamp`
decides under §7.4. §5.2, §5.3 and §5.6 keep one domain, `zikaron/2`; §5.7
adds the seat's duties on handing off, adding a chain, moving law, and
anchoring nothing but its own ledger entries. §6 renumbers: `succession` is
§6.5 with `to` naming a fresh key, `migration` is §6.6, `annotation` §6.7,
`retraction` §6.8; `genesis` takes `continues`, `history` takes `for`,
`grant` takes `upstream` and `ledger`. §7.2 says a `RETIRED` entry satisfies
no link; §7.3 closes authority at a migration and reads a continued ledger
under its old root with the continuation's author as its seed; §7.4 defines
retirement, timeliness and the lineage over timely successions; §7.5 and
§7.6 follow. §8 has three inputs; §8.1 lists untimely entries of lineage
keys as `RETIRED` and discards records of retired keys; §8.2 closes the walk
at a migration; §8.4 and §8.5 read "at a record at which it has not
retired"; §8.6 is the report, fifteen contents, `RETIRED` at item 8,
`continues` at item 14, the label `UNPROVEN` in place of `UNAVAILABLE` and
decided over both reports of a continued ledger. §9.1 is one form; §9.2 and
§9.3 drop the evidence record and read retirement; §9.4's basis is `chains`
alone; §9.5 (time) reaches through `continues`; §9.6 (proof kits) is one
form, outside the kit law's documents. §10 drops the attestation relation.
§11 adds retraction, succession books, and the one order §7.4 makes. §12.3
is the continuation clause every successor law states; §12.4 reads
retirement; §12.5 is unnamed; §12.6 says how a `zikaron/1` ledger is
continued: the candidates, the cut, the two segments' inputs, the one
`zikaron/1` core that reads the old segment.

**Where the drafting departed from the judge's plan**, each a place the
plan's text could not be run as written and the judge is asked to confirm:

1. §12.6, the records of the closing succession. The plan divided the anchor
   set by time alone (records at or before the cut to the old segment, the
   rest to the new). A seat that anchors its closing succession on a second
   chain, or on a chain it adds, at a later block time would then put that
   record into the new segment, where the succession's bytes are not in the
   pile and the record is `MISSING` for good: every migration anchored on
   two chains at two times would read `GAPS`. The text now puts every record
   whose `hash` is the closing succession's `entry_id` into the old segment
   whatever its time, and keeps them out of the new. Inverted form: a record
   belongs to the segment whose law reads the bytes it anchors, the time
   rule deciding the rest; harmed without it, every seat with a basis of two
   chains.
2. §12.6, the old segment's byte strings: "every byte string passing
   `zikaron/1` §4.3 reachable along `prev`, through byte strings passing
   `zikaron/1` §4.3, from one of these that passes it" says what the plan's
   "reachable from one of these" left open for an anchored byte string that
   passes neither law.
3. §7.5 says a lineage key names a stranger "by a timely succession (§7.4)"
   where GALEED says "at any record at which it has not retired": in this
   law a succession enters the lineage by timeliness, which any record of
   any sender decides, and the GALEED phrase would have read as a sender
   test.
4. §6's gloss on arrays ("No row of this section names an array") and on the
   one value §12.6 matches in a `zikaron/1` entry; §7.2's `RETIRED` clause;
   §8.1's sentence that a malformed byte string's `entry_id` "answers §8.6
   item 15 as any byte string of the pile does"; §8.6 item 15 restated with
   the continued-ledger rule before the ordered list: wording the plan did
   not supply, rules it implied.
5. §9.6 keeps its mark [X] and says no document of `zikaron.kit/2` carries a
   kit, as the plan asked, in one sentence.
6. HARNESS2.md: the audit input holds `root`, `pile`, `anchors`, `basis`;
   `continues.report` is nested exactly as `zk1 audit` writes it over the
   old segment's inputs, its basis spelled with empty `bareTx` and
   `adoptionChains`; a `zk2 check` applies this law alone and names a
   `zikaron/1` entry `E_SPEC`, the dispatch by `spec` being the kit law's.

### Round 0: the checkpoint

The law was drafted from `zikaron-v1.md` by a script that quotes each base
passage once and replaces it (100 edits, every quoted passage found exactly
once), rewrapped at 76 columns; the forbidden words of the plan's self-check
(`adoption`, `attestation`, `bareTx`, `adoptionChains`, `unavailable`,
`evidence set`, `five inputs`, the old domain) occur nowhere but in §12.6's
naming of `zikaron/1`'s empty inputs and §8.6's reading of its
`UNAVAILABLE`.

The candidate core `zikaron-conformance/impl2-py/` is three files (`zk2.py`,
`zk2entry.py`, `zk2audit.py`) over the `zikaron/1` core's `zkcanon.py` and
`zkcrypto.py`, and over its `zkaudit.py` as a black box for the old segment,
which it feeds as a `HARNESS.md` input and reads back as a report; its
`NOTES.md` lists the readings taken. `criterion-digest2.sh` computes its
digest over the three files and the `zikaron/1` core digest, refusing any
other embedded core. `HARNESS2.md` is the `zk2` contract.

The corpus `zikaron-conformance/corpus2/` is generated by its own `gen.py`,
the `zikaron/1` generator carried over where the texts agree and the
continued segment predicted by that generator's predictor imported as a
tool: at seed 1, 3528 canon cases, 31 sign cases and 553 audit cases (205
`COMPLETE`, 146 `NO_LABEL`, 100 `BROKEN_CHAIN`, 85 `GAPS`, 17 `UNPROVEN`),
every closed vocabulary witnessed, the generator's hand labels and code
agreeing on every case, and the criterion agreeing with every prediction
(label and every count) through `compare2.py`, which takes the manifest as
the corpus and exits 1 on any divergence or disagreement. The new scenarios:
retirement at its own second and one later, on another chain, the smallest
time across chains, set by a stranger's, an `UNPROVEN` or a `VOID` record,
absent without a record; untimely entries of a retired key, anchored by
nobody, by the old key, by the successor; a late succession extending
nothing; a succession with no record of its own, timely through a later
entry's; a fork anchored by a third party before and after the retirement;
migrations at the last position, closing the walk, beside a succession, out
of office, by a stranger, across a gap, retiring their signer; adding a
chain; continuation clean, at the cut's own second, with the old key's
`zikaron/1` entries after the cut, forks anchored before and after the cut,
junk before the cut, the label taken from either segment, two candidates at
two cuts and at one cut in both id orders, a forged candidate anchored
first, a stranger's candidate, the root unchanged and the old root key
outside the new lineage, `continues` with a wrong root, a non-migration
kind, no record, an extra member, the cut across two chains and under one,
and the label rule over bytes of the other segment. Seeds 2 to 6 were
generated and compared the same way, each with zero divergences and zero
disagreements, every closed vocabulary witnessed at each seed. `bcov.py
core2` at seed 1 measures 867 branch arms over 7640 cases, 111 never taken:
fifteen are the `zk2` command line's misuse arms (wrong argument counts, an
unreadable file, a bad key), one is the equivocation scan's guard against a
byte string meeting itself, and 95 lie in the `zikaron/1` core's own files,
the arms of adoption, the bare anchor form, the unavailable set and the
evidence set, which §12.6's empty inputs never reach.

Court `da72da95` holds the checkpoint; the base trees came in at `24004dfc`.

### Round 1: seven seats, one fold

Seven seats read Court `da72da95`: a reader of this law (A), a reader of the
kit law (B), the corpora and comparators (D), the tooling and the digest
path (E), and three independent implementations of both laws from the texts
alone, in Rust, Node and Python. The three implementations measured the
committed seed-1 corpora and seeds 2 to 6 of both with the programs as first
written, zero divergences in every cell, then fuzzed the criterion as a
black box: Rust 20,460 `zikaron/2` invocations over 16,500 inputs and 11,000
kit inputs, Node 12,000 and 10,000, Python 10,500 and 10,500, with zero
disagreements, and Node's hand probes found the one divergence class of the
round, in the kit law (its record). The readers reported 17, 19, 26 and 14
items; the implementers 10, 7 and 8; the seats tagged ten of them
load-bearing, every one a place where the criterion had taken one of two
readings the text allowed and the independent programs had happened to take
the same.

**Folded into this law**, one pass, each sentence now stating the one
reading:

- §12.6, the old segment's byte strings: the first clause reads as the third
  did, "every byte string of the pile passing `zikaron/1` §4.3 that is
  reachable from `s` along `prev` through such byte strings"; a line's
  endpoint passes that law or is not on the line (A1, node 2, D6).
- §12.6, the old segment's records: a record belongs to the segment whose
  law reads the bytes it anchors, and the time rule places the rest. The old
  byte strings are fixed first (the succession's line, the bytes anchored at
  or before the cut, their `zikaron/1` lines); the old records are those at
  or before the cut and every record, whatever its time, whose `hash` is an
  old byte string passing `zikaron/1` §4.3; the new segment's records are
  the rest. The round-0 text applied this to the closing succession alone,
  and a seat that anchored its closed history on a chain it added, the duty
  §5.7 states, would have read `GAPS` for good (rust 6). This is the one
  fold of the round that changes an answer of the criterion core, on inputs
  the committed corpus did not hold; by the standard's §五 之一 7 this round is
  the previous round of the next.
- §7.4 and §12.6: every predicate of the new segment's audit, retirement and
  timeliness included, ranges over the new segment's inputs alone; §7.4 says
  so and §12.6 says it once (A3, rust 2, py 1, node 4, D1). The consequence
  the seats asked the judge to confirm stands written: a `zikaron/2`
  succession or a stranger's record anchored at or before the cut retires
  nobody in the new segment.
- §8.6 item 15 rule 2 now reads the pile of the segment the report reads,
  and the clause that named the whole pile is gone: under the old-records
  rule above, no record of the new segment names bytes the new segment's
  pile lacks while the whole pile holds them, so the clause had no case left
  (A2 asked which segment's records it meant; neither answer has an instance
  now). `UNAVAILABLE` of the old report still reads as `UNPROVEN` in the
  label order.
- §5.7: a seat that adds a chain anchors on it first the closing succession
  of a continued `zikaron/1` ledger, then every succession and migration in
  ledger order, each at a later block time than the last (A4); a seat whose
  ledger continues a `zikaron/1` ledger anchored every `zikaron/1` entry
  before every record of the closing succession and anchors its genesis on
  each chain only after the earliest record there of that succession, and
  whoever first anchors that succession sets the cut (A5); the last sentence
  binds the key that signed the closing succession, by a record at which it
  has not retired, for a `zikaron/1` byte string that is no byte string of
  the old segment (A11, rust 6).
- §7.3, §7.4, §8.2, §8.3: `auth(0)` is the root or, for a continued ledger,
  the key §7.3's last paragraph names; the walk's `auth` starts at
  `auth(0)`, `ROOT_MISMATCH` tests against it, the lineage is seeded with it
  in the root's place, and §8.3's witnesses are ledger entries (A8, A13, py
  3).
- §7.4: the earliest record of a key's first anchored succession or
  migration makes that succession timely, and a second succession of the key
  that no record bounds by that time is not (A10).
- §7.5: an imported ledger doubles `seq` 0 wherever the imported genesis is
  timely (A13).
- §1, §7.4, §7.6, §9.2: §7.4 and §12.6 are the two places the grammar orders
  records by time and sets records of two chains side by side (A9, py 5).
- §8.1: a `RETIRED` entry enters no walk, no `have` and no finding, and a
  counted record of its hash by a lineage key at which that key has not
  retired stands in `MISSING` (A13; the corpus already said so).
- §8.4, §8.5, §8.6 item 8, §12.6: "at or before" wherever a retirement or
  the cut is the bound, as §7.4's "not later than" (A12).
- §9.5: an anchor bounds the continued ledger through a genesis that names
  the closing succession in `continues` and is reachable along `prev` (A14).
- §12.6: the first sentence says what `zikaron/1` permits and what this law
  reads into it (A6, rust 5, py 4); who can be the continuation's `author`
  is stated as a consequence of the selection and not as a bound on
  candidates (A7, py 4); a ledger continues a `zikaron/1` ledger only under
  an anchor set holding a record of its closing succession, and offline it
  is an uncontinued root (A15, stated, the alternative left to the judge);
  the empty adoption evidence set makes `zikaron/1` §9.5 list every element
  of every ledger adoption of the old segment in `ADOPTION_UNPROVEN`,
  informational (rust 9, py 6, node 3).

**Not folded, with the reason**: the alternative A15 names (an offline
continuation through the last in-office key's closing succession) widens the
selection rule and is the judge's to want; D23's whole-report prediction in
the corpus would make the generator a fourth implementation of the report,
and the gate is the three independent ones (the manifest now says what its
predictions are); D3's per-clause witness table remains the seats' mutation
work, the manifest publishing the vocabulary residue and the comparators the
counts.

**The corpus and the tooling.** corpus2 gains fifty audit scenarios the
seats asked for: the old records rule on both sides (a late counted and a
late `VOID` record of an old entry, a late record of an unseen hash), every
conjunct of the candidate definition falsified alone (`to` another key, a
history named, a byte string failing `zikaron/1`, the named succession not
held, a stranger's `UNPROVEN` record making the candidate, the cut from a
`VOID` record), the old root readmitted and opening a genesis
(`ROOT_MISMATCH` against `auth(0)`), a branch entering the old segment
through its anchored successor, a line stopping before junk and before a
`zikaron/1`-spec near-entry, the old segment's `SEQ_GAP`, `ROOT_MISMATCH`,
body failure, unknown type and adoption, `adoption` as an unknown type of
this law, a migration out of office mid-walk, an `UNPROVEN` record over a
retired twin's bytes, a retired key's late `UNPROVEN` and `VOID` records, a
string `blockTimestamp`, a retired twin reached through a stranger's entry
anchored in time and not through a malformed bridge; the `for` object's
three remaining sides in canon; sign cases with no domain argument, a bare
key and an `0X` key; the two-candidate case drawn so that the cut alone
decides at every seed; and eleven harness-misuse cases that both programs
must refuse. The notes carry this law's item numbers; the manifest's
coverage keys are §8.6's and it publishes `unwitnessed` and `undecided`.
`compare2.py` requires the generator's count keys exactly, checks the
manifest's counts against its cases and every named file, compares strictly
typed, passes no domain where the meta has none, runs misuse cases, runs the
criterion with `-B`, and takes its output path from `ZK2_OUT`.
`criterion-digest2.sh` reads the naming as a backticked digest wherever the
line wraps; `criterion-digest-kit2.sh` refuses any `zikaron/2` core the kit
law does not name, so it prints nothing until that naming. HARNESS2.md
states the ASCII condition on a sign domain as its own rule.
BOUNDARY-AUDIT.md gains the `core2` and `kit2` sections. Seed 1: canon 3531,
sign 34, audit 603, misuse eleven; seeds 2 to 6 zero divergences and zero
disagreements; `bcov.py core2` measures 878 arms over 7699 cases, 100 never
taken: fourteen are the command line's misuse and tracing arms, one the
equivocation scan's guard against a byte string meeting itself, and 85 lie
in the `zikaron/1` core's files, classified in BOUNDARY-AUDIT.md
(unreachable by §12.6's empty inputs, decided first by this core, or the
`while True` loops).

The fold is the commit after `da72da95` on the revision branch; the next
round's section names it.

### Round 2: seven seats, one fold

The seven seats read Court `c6c84901`. The three implementations first
measured the round-1 programs unchanged on the round-1 corpora: at every
seed, six corpus2 cases and twelve kit cases diverged, every one on a clause
round 1 folded (the old-records rule; the kit law's unreadable file,
`contents` clauses, `continues` edge and check 3), and nothing else. Brought
to the folded texts from the texts alone, the three programs measured zero
divergences and zero disagreements at seeds 1 to 6 for both laws, then
fuzzed the criterion as a black box, Rust 31,720 invocations, Node 26,538
and Python 25,500, with zero disagreements. The readers reported 16, 10, 13
and 9 items and the implementers 5, 5 and 6; of the sixty-four, A9, the kit
`contents` purpose clause and the kit §6.1 edge change answers and were
folded as such, five were tagged load-bearing by their seats as text that
reads two ways where the programs agreed, and the rest are text, corpus and
tooling.

**Folded into this law**, one pass:

- §12.6, the candidate: a key whose handing off under `zikaron/1` a record
  earlier than the candidate's cut anchors closes nothing (A9). A stolen key
  that handed its `zikaron/1` seat on could otherwise close the ledger first
  and seed the new lineage for good, which is the judge's ruling on
  handed-off keys not reaching the selection rule; a record in the cut's own
  second is not earlier, as §7.4 reads a key's own retirement. The sentence
  on who can be the continuation's `author` follows. This fold changes the
  criterion's answers on inputs the committed corpus did not hold; round 2
  is the previous round of round 3.
- §12.6, the summary sentences restate the definitions: a record at or
  before the cut is an old record, and so is every record of an old byte
  string passing `zikaron/1` §4.3 whatever its time, the time rule placing
  the rest (A4, rust 4, node 2, py 6); an old byte string that fails
  `zikaron/1` §4.3, one passing neither law or passing this law that some
  record at or before the cut anchors, is of both segments, and junk no
  record anchors at or before the cut is of the new segment alone (A3, rust
  2, node 1, py 5); the genesis is anchored on each chain after the earliest
  record there of the closing succession (rust 3).
- §7.3, §9.4: `auth(0)` wherever position 0 or the lineage seed was still
  called the root (A1); §1, §7.4, §11: §7.4 and §12.6 are the two places
  records are ordered by time, §12.6 picking a continuation by its earliest
  record and dividing the inputs at its cut (A5).
- §5.7: the add-chain duty names the continued ledger's successions and
  states what a record of the closing succession by a key outside the
  `zikaron/1` lineage produces (A7); the continued seat anchored every
  `zikaron/1` entry before it first anchored the closing succession, a later
  record of an old byte string being an old record (A6, rust 3); the last
  sentence binds a counted record and an old byte string passing `zikaron/1`
  §4.3 (A8).
- §8.6 item 14 and HARNESS2.md say "old segment" (A10); item 15's rule 2 is
  indented as its siblings (A2).

**Not folded, with the reason**: A's alternative to the offline sentence and
D23 remain as round 1 left them; the whole-report prediction is now partly
met by `predicted_old` (below).

**The corpus and the tooling.** `compare2.py` passed a misuse case's command
word as a directory path, so nine of the eleven round-1 misuse cases
measured an unknown command and nothing else (D1); it now rewrites only a
later token that names an existing corpus path, compares misuse predictions
strictly, requires each category's key set and its note or meta,
cross-checks the sign meta against the manifest, walks the whole corpus tree
for files no case names, compares `predicted_old`, the author's counts for
the nested `zikaron/1` report, and prints the manifest's sha256 it read (D2
to D7); `bcov.py core2` drives the manifest's vectors as the comparator
does, the misuse cases and the metas' domains included (E4). corpus2 gains
the stolen handed-off key closing first with the handover unanchored,
anchored earlier and anchored in the same second; a `zikaron/2` succession
anchored at the cut's second alone, at the cut and later, and by a stranger
at the cut, and a stranger anchoring the new succession before the cut (A11,
D9); a genesis anchored at the cut and later, and junk before and after
(A12, D10); the label pairs old `BROKEN_CHAIN` with new `GAPS`, old `GAPS`
with new `UNPROVEN`, old `UNAVAILABLE` with new `GAPS` (A13); a candidate
failing §4.3, an annotation at seq 1 carrying `continues`, a `zikaron/1`
genesis carrying it, and a stranger's closing succession at seq 1 (A14,
D10); one old-segment byte string per `zikaron/1` §4.3 token of steps 5 to
13 and per body row (E1); a sign case under the empty domain and nine misuse
cases (A16, E4); the notes A15 named. The corpus README names
`zikaron-v1.md` as a source and says what is published toward §12.1's
precondition; BOUNDARY-AUDIT.md's classes read as measured (E2, E3, E8, E9).
Seed 1: canon 3531, sign 35, audit 675, misuse nineteen, manifest sha256
`0xb777d4bc45972a5e33ea04eb0fe35c2a875ade2065293d8d3306af6bc5a0e4f2`; seeds
2 to 6 zero divergences and zero disagreements; `bcov.py core2` measures 891
arms over 7791 cases, 75 never taken: the `__main__` guard and the
equivocation scan's self-meeting guard in `impl2-py`, and 73 in the
`zikaron/1` core's files, classified in BOUNDARY-AUDIT.md.

The fold is the commit after `c6c84901` on the revision branch; the next
round's section names it.

### Round 3: seven seats, one fold

The seven seats read Court `7074486a`. The three implementations first
measured the round-2 programs unchanged: at every seed four corpus2 cases
and eight kit cases (nine at seed 2) diverged, every one on the three
clauses round 2 folded as answer-changing, and nothing else. Brought to the
folded texts, the three measured zero divergences and zero disagreements at
seeds 1 to 6 for both laws and fuzzed the criterion as a black box, Rust
25,240 invocations, Node 25,506 and Python 29,500, with zero disagreements.
The readers reported 13, 7, 11 and 9 items and the implementers 6, 5 and 6,
all tagged text; one of them, A1, changes an answer and was folded as such.

**Folded into this law**, one pass:

- §12.6, the old records: a late record is an old record only where its
  `hash` is an **old ledger entry**, a `zikaron/1` ledger entry of the old
  byte strings, that law's lineage being a function of the byte strings
  alone (A1). Round 2's rule reached every old byte string passing
  `zikaron/1` §4.3, so a key of the `zikaron/1` lineage that had handed off,
  or the continuation's `author` once retired under this law, could
  re-anchor an `EXCLUDED` old byte string at any later time and make the
  ledger read `GAPS` for good, the old law knowing no retirement; every
  other late record now meets this law's §8.1 trim. §8.6 item 15 rule 2
  reads the whole pile again, since a late `UNPROVEN` record of excluded old
  bytes is a new record over bytes the new segment's pile does not hold.
  This fold changes the criterion's answers on inputs the committed corpus
  did not hold; round 3 is the previous round of round 4.
- §12.6 defines a key's **handing off under `zikaron/1`** once, a record on
  any chain whatever its `sender` and `verdict` anchoring a `zikaron/1`
  succession by that key to another key, a succession to itself handing
  nothing off, and uses it in the conjunct, the gloss and the sentence on
  the continuation's `author` (A3, rust 1, py 3, node 2, D2, D3); the "two
  keys" clause states the fault by the old segment's membership (A7); a
  ledger whose seat returned to a key after its handing off was anchored has
  no key that can close it, and hands the seat first to a key that never
  held it (A2, rust 2, py 5, node 3, the inverted form left to the judge).
- §5.7: the add-chain duty names this law's successions and migrations, and
  says what a record of the closing succession by a key outside the
  `zikaron/1` lineage is, a record of the cut all the same (A10, rust 3, py
  4, node 4); the continued seat's duty states the reading it buys and its
  closing key (A11, A2); the seat anchors a ledger entry of either segment's
  audit (A12).
- §7.4: the earliest record makes "that entry" timely, a second succession
  being one from which no record at or before that time reaches it along
  `prev` (A13); §1, §7.4, §7.6, §9.2 and §11 describe §12.6's use of time as
  comparing records with each candidate's cut (A8); §12.6's scope sentence
  names `continues` as the one member the old segment reaches (A9).
- §12.1 names the instrument the boundary list is published as: the
  manifest's residue of the closed vocabularies and the branch-arm classes
  of the boundary audit, under every seed (E4).

**Not folded, with the reason**: A2's inverted form (a handing off counts
only where no later succession on `s`'s line names the key back) changes
answers and is the judge's; D11's predictor reading check 3 by lists is an
equivalence the kit NOTES now state; D5's expected digest is printed and
named here, not yet enforced by the comparator.

**The corpus and the tooling.** corpus2 gains the handed-off root
re-anchoring excluded old bytes, the retired continuation author doing the
same, the key in office's late `UNPROVEN` and counted records of such bytes
(A1); the handback that closes nothing and the same with the handover
unanchored, a succession to itself (A2, A3); the handing off recorded by a
stranger, `UNPROVEN`, `VOID` and on another chain (A5, D3); the
two-candidate cases rebuilt on one key closing twice at one cut in both id
orders and at two cuts, and on two forked successors, the round-2 cases
renamed for the clause they witness (A4, A6, D1, rust 6); the `zikaron/1`
body rows on both sides in the old segment (E1); the token cases' notes
derived from the token each case yields and the recovery case drawn until it
fails recovery (D9); two sign cases, a re-canonicalized spelling and a
top-level `sig`, and three misuse cases (E6). `compare2.py` requires
`predicted_old`'s keys exactly, relates the sign meta's input to the case,
refuses paths that leave the corpus or cross a symbolic link, reports
symbolic links, and prints a digest over every byte it read (D4 to D7);
`compare-kit2.py` does the same with per-category key sets and the argument
vector as the one source (D6, D7). BOUNDARY-AUDIT.md's classes read as
measured at seeds 1 to 6 (E1 to E3). Seed 1: canon 3531, sign 37, audit 739,
misuse 22, manifest sha256
`0x20337f38d5152e07e3a5b57ba639ac3168fb35dd1fe04805e43fd4556891311a`, bytes
read `0xb7fa44082e9a70c5d93679c7b910fd7faa5f39f2d35339a47e516935b4a2e9fb`;
seeds 2 to 6 zero divergences and zero disagreements; `bcov.py core2`
measures 891 arms over 7860 cases, 62 never taken: the `__main__` guard and
the equivocation scan's self-meeting guard in `impl2-py`, and 60 in the
`zikaron/1` core's files, classified in BOUNDARY-AUDIT.md.

The fold is the commit after `7074486a` on the revision branch; the next
round's section names it.

### Round 4: seven seats, one fold

The seven seats read Court `1e5878cc`. The three implementations first
measured the round-3 programs unchanged: at every seed eight corpus2 cases
(`a650` to `a657`) diverged, every one on the clause round 3 folded as
answer-changing, the old ledger entry rule with §8.6 item 15 rule 2 over the
whole pile, and no kit case diverged. Brought to the folded texts, the three
measured zero divergences and zero disagreements at seeds 1 to 6 for both
laws and fuzzed the criterion as a black box, Rust 25,725 invocations, Node
25,511 and Python 30,500, with zero disagreements. The readers reported 9,
7, 12 and 7 items and the implementers 5, 4 and 6; every item of this law is
tagged text, and none changes an answer of this law, so round 5 is this
law's closing measurement with the round-4 programs unchanged. The kit law's
reader reported one load-bearing item, folded in that law's record.

**Folded into this law**, one pass:

- §12.6 defines a candidate's **old ledger** once, the ledger `zikaron/1`
  §8.1 draws under `r` from the candidate's old byte strings, a function of
  those bytes alone; the conjunct and the old-records clause read from it,
  and every other record, being later than the cut, is a record of the new
  segment under this law's §8.1 trim (rust 1, node 1, py 6, py 3). The
  continuation's genesis, failing `zikaron/1` §4.3, is of the old segment as
  well where a record at or before the cut anchors it, and an entry of this
  law so anchored is a byte string of both segments (A6). A late
  re-anchoring counts only as that segment's `zikaron/1` trim keeps it, so a
  seat re-anchors old ledger entries from a key of the `zikaron/1` lineage
  (rust 3). The offline sentence has the verifier scan a basis that reaches
  the record (§9.4) (A7).
- §5.7: the last sentence names the set §12.6 defines, "no old ledger entry"
  (A1, py 1); the continued seat's "so that" clause is stated relative to
  the reader's inputs, the reader's anchor set or a pile reaching the entry
  from the closing succession or from a byte string anchored at or before
  the cut, an entry off that line falling to the new segment as `MALFORMED`
  with its record where §8.1 and §8.5 place it (A2, rust 2, node 2, py 2).
- §7.4's gloss on a second succession quotes the definition (A3); §7.6 and
  §11 point to §12.6 for the cut (A8); §8.5's forward list names the old
  byte strings the nested report lists (A5); §8.6 item 15 rule 2's
  parenthesis adds bytes of the old segment alone (node 4).

**Not folded, with the reason**: A4, rule 2 split across the segments (the
`UNPROVEN` step decided once over both reports' rows, the whole pile and
each segment's counted records), changes answers and is the judge's, as the
seat itself recommends; A2's inverted form of the handback stays the
judge's; D12, rust 4 and node 3 asked that the kit's §6.1 name the selected
genesis, and the kit record says why the fold took the other direction.

**The corpus and the tooling.** corpus2 gains one case per operand of the
old ledger's definition where it alone decides: the old lineage over the old
byte strings alone, an old ledger entry off the closing line re-anchored
late, the old lineage seeded by the root, and an `UNPROVEN` record of an
entry of both segments (D1); two continued inputs whose basis has two
`chains` objects of one `chainId`, adjacent and with a gap (E2); the five
optional `zikaron/1` body rows present and out of form (E1); a sign over
`{}` and seven misuse cases, the key at n + 1, 2^256 − 1 and sixty-five
digits, `sign` alone and a directory for `check`, `sign` and `audit` (E6);
the notes of `a476`/`a477`, `a484` to `a489`, `a511`/`a517`/`a519`/`a521`
and `a575` read as the cases do (A9, D10). `compare2.py` predicts the
selected continuation's `entry_id` and cut (D3), refuses a bytes-read digest
other than `ZK2_EXPECT` where set (D5), recomputes the manifest's coverage
tallies from the criterion's outputs (D6), relates the hand label and the
`disagreement` flag to the prediction (D7). BOUNDARY-AUDIT.md's classes read
as measured (E1 to E3), the corpus2 README's sign line and the kit README's
snippet as the trees are (E4, D9, E7). Seed 1: canon 3531, sign 38, audit
761, misuse 29, manifest sha256
`0x8ef24c5f7dbc96894892336e917affcb9d3bee11bfba5eb0a77fd33d814dce2e`, bytes
read `0x8f753fe309e99aa5922825e5c9a5e4889ae81828d9f6c883f376b9c6ff6d24e0`;
seeds 2 to 6 zero divergences and zero disagreements; `bcov.py core2`
measures 892 arms over 7890 cases, 59 never taken: the `__main__` guard and
the equivocation scan's self-meeting guard in `impl2-py`, and 57 in the
`zikaron/1` core's files, `zkaudit.py` :180 and :182 and `zkentry.py` :74
being taken from this round, the rest classified in BOUNDARY-AUDIT.md.

The bytes-read digests of the corpus2 self-comparisons at seeds 2 to 6, for
`ZK2_EXPECT`:
`0xf7a964cdb649cb0f049853ab4a048583fcf22154fcdd8dabf14594f8047238d4`,
`0x6765c61bee0963e2b3200e97d9c1b74a9e5bb95acbc63b1af58be3d1f1b51a70`,
`0x5578e5228acd61f6ba8c0693d8f6fcb4e9529175fa961ec071db707290bbb3f7`,
`0x5613451a8e5f743389f58bb5e738abc8ab17bb8175ee09a385ee2e3fea6bd23f`,
`0xc7a2ff3455bbc3099229ee6d97b2e6022f8318ad699bdd52392778dac6766e55`.

The fold is the commit after `1e5878cc` on the revision branch; the next
round's section names it.

### Round 5: seven seats, one fold, the closing measurement

The seven seats read Court `8908ab2e`. The three implementations first
measured the round-4 programs unchanged: for this law, zero divergences and
zero disagreements at seeds 1 to 6 from all three, which is this law's
closing measurement under the standard; for the kit law, Rust and Python
zero, and Node two cases at every seed (`c181`, `c182`) on §6.1 as round 4
folded it, zero once brought to the text. The three fuzzed the criterion as
a black box, Rust 27,091 invocations, Node 29,513 and Python 27,807, with
zero disagreements. The readers reported 7, 10, 8 and 7 items and the
implementers 4, 5 and 4, every one tagged text; none changes an answer of
either law. The fold below is text and corpus cases only, so by standard 之一
7 the three round-5 programs, unchanged, were re-measured on the folded
corpora at seeds 1 to 6: zero divergences and zero disagreements for all
three, under both comparators, at every seed (the orchestrator's run over
each seat's `run.sh` command lines, 36 comparisons).

**Folded into this law**, one pass:

- §12.6: the old segment is defined for a candidate, the continuation among
  them, and the gloss on who may be a candidate's `author` names the
  `zikaron/1` lineage of that candidate's old ledger (A3); candidates naming
  one succession share its `author` and its cut, so the choice among them
  reaches no output and the selection decides which closing succession, cut
  and seed the audit reads (node 4); the old-records gloss is a sentence
  with finite verbs: a late record of the closing succession or of an old
  ledger entry anchors it into the old segment, where it counts only as
  `zikaron/1` §8.1 keeps it, so a seat that wants it counted there anchors
  from a key of the `zikaron/1` lineage, while a record of the closing
  succession from any key counts toward the cut (rust 2, A2).
- §5.7: the add-chain duty names this law's successions and migrations, and
  the key an old ledger entry re-anchored on the added chain is sent from
  (A2); the continued seat's clause reads "a record of it not later than the
  cut", reaches along `prev` through byte strings passing `zikaron/1` §4.3
  from the closing succession or from such a byte string anchored at or
  before the cut, and "one that neither reaches" falls to the new segment
  (A1, py 3).
- §8.5's forward list names the nested report's `EXCLUDED` list (rust 3);
  §8.6 item 15 rule 2 reads "bytes only the old segment holds" and "a record
  of the segment this law reads" (A4, D7).

**Not folded, with the reason**: A4's inverted form of rule 2 stays the
judge's, as in round 4; py 2 (kit §6.2: an old record anchoring a ledger
entry of the new segment that a record at or before the cut anchors, where
anchoring by segment would change answers) is the judge's, the text reading
one way.

**The corpus and the tooling.** corpus2 gains the bridge pair, a `zikaron/2`
annotation anchored before the cut on an off-line `zikaron/1` `prev`, beside
its `zikaron/1` twin (A5); the candidate-lineage pair, the root's
`zikaron/1` succession naming the closing key held off the line, unanchored
and anchored before the cut (A6); the `UNPROVEN` step and timeliness over
the new segment's records alone, an old counted record of bytes nobody holds
and an old record of an annotation after a handing on (D1); `canon` with no
argument (E6); the `zikaron/1` row cases' notes name the row (A7).
`compare2.py` folds the manifest's case list into the digest `ZK2_EXPECT`
pins (D3), recomputes every coverage key and derives `unwitnessed` from a
`vocabularies` table the manifest now publishes (D4), and requires
`predicted_continues` to be exactly `entry_id` and `cut` (D5). Round 4's
digest lists in both records had seeds 2, 4 and 5 of the two laws exchanged
and now read as the comparators print them (B9, rust 1, node 1, py 1);
BOUNDARY-AUDIT.md's caption, seed sentence and kit2 paragraph read as
measured (E1, E2); the READMEs name `ZK2_EXPECT`, `predicted_continues` and
the empty-object sign case (E5, D8), and the kit README's snippet is
indented four (E3, D6). Seed 1: canon 3531, sign 38, audit 771, misuse 30,
manifest sha256
`0xe796555343176464f6725b43e8b13bdacc61294b116dfb6efd901a0606e3bdb5`, bytes
read `0x660b2ae04a5b56b5c5481f6feeb6717843e51a99b99209839746bfd3407d6217`;
seeds 2 to 6 zero divergences and zero disagreements, bytes read for
`ZK2_EXPECT` as `compare2.py` prints them, seeds 2 to 6 in order,
`0xf4cd2fa6c8f01324abf4fdace3a9b48c01935f5da3e7017477d97fdcc880d484`,
`0x6d74aeeb4800c211eca74fa354f1c6a770da8b8150f8b64e56ac047136357f09`,
`0x82a9e333da36616b4d5dff5f17ca2f858307c4663234cd904595ce7b30885b25`,
`0xe79f3b514457996bbcf3e1f6930a24de6f85002d44c840eded03a6dd26ae0e36`,
`0x92d15097531694e5613354600c41793f59e687e2ae71ea9774bb350cde14ea07`;
`bcov.py core2` measures 892 arms over 7901 cases, 59 never taken, the
round-4 set unchanged: the `__main__` guard and the equivocation scan's
self-meeting guard in `impl2-py`, and 57 in the `zikaron/1` core's files,
classified in BOUNDARY-AUDIT.md.

The fold is the commit after `8908ab2e` on the revision branch; round 6 names
this law once its reader has re-read the folded sentences (standard 之三 2).

### Round 6: seven seats, the residue, and the re-reading before naming

The seven seats read Court `d1cfb609`. The three implementations first
measured the round-5 programs unchanged: zero divergences and zero
disagreements at seeds 1 to 6 under both comparators from all three, which
confirms this law's closing measurement and is the kit law's. Rust changed
one thing afterwards, outside both laws: its file reader had refused every
path that is not a regular file, where HARNESS2 reads any path that opens
and reads in full (a character device among them); Node and Python changed
nothing. The three fuzzed the criterion as a black box, Rust 35,624
invocations, Node 27,500 and Python 33,000, with zero disagreements beyond
Rust's harness class, zero after its change. The readers reported 8, 6, 3
and 5 items and the implementers 4, 3 and 3, every one tagged text, none
changing an answer of either law. This law's reader re-read every sentence
round 5 folded, found one reading in each, and said the law may be named at
`d1cfb609` as it stands. The fold below is text, corpus and tooling only; by
standard 之一 7 the three round-6 programs, unchanged after it, were
re-measured on the folded corpora at seeds 1 to 6: zero divergences and zero
disagreements for all three under both comparators at every seed (36
comparisons over each seat's `run.sh` command lines).

**Folded into this law**, one pass:

- §5.7, the add-chain duty of a continued seat, states the order a reader of
  that chain alone needs: this law's successions and migrations in ledger
  order, before them the closing succession, and before that, from a key of
  the `zikaron/1` lineage, every old ledger entry off the closing
  succession's line the seat wants that reader to read, since a later record
  of such an entry is, for that reader, a record of the new segment, while
  entries on the closing line are old through the succession's record (A1,
  A5, rust 2, node 3, py 1); the continued seat's clause reads "a
  `zikaron/1` entry for which neither holds" (A4, rust 3).
- §12.6: the old segment is defined "for each candidate" (A2, node 1, py 2);
  the sentence on every other record moves out of the old segment's list
  into the new segment's definition (A3); named without a candidate, the old
  segment, its byte strings, records and ledger entries, and the new segment
  are the continuation's (A2, py 2).

**Not folded, with the reason**: rust 1's `/dev/null` cases, since both
corpora hold files only and the comparators pass corpus paths alone; the
harness sentence now decides the file type (folded below). A4 and py 2 of
round 5 stay the judge's.

**The corpus and the tooling.** corpus2 gains two geneses by one key naming
one closing succession, the choice between them reaching no output (A6); the
notes of the bridge case and the timeliness case name the case they compare
with and the source of their label (A7); the manifest's `vocabularies` table
covers every list of §8.6, `NO_LABEL` and the accept outcome `ok` (A8).
`compare2.py` carries the closed vocabularies itself and refuses a manifest
whose table differs, folds the manifest file whole into the digest
`ZK2_EXPECT` pins, and refuses a non-empty `undecided` (D3). HARNESS2.md
says a path is read whatever its file type where it opens and reads in full
(rust 1). README.md names no commit this repository cannot reach (E1);
BOUNDARY-AUDIT.md's seed sentence carries the measured figures (E2); the
corpus2 README's "Comparing" and "Layout" sentences name `ZK2_EXPECT` and
`predicted_continues` and its long lines are wrapped (E4, E5). Seed 1: canon
3531, sign 38, audit 773, misuse 30, manifest sha256
`0x71fbabf7b41a2e36a8e7e774a9120ec6bdac3de66a6f24af7cfdb1063c27fd82`, bytes
read `0xa16158937d2510c10930ddd2e4460640960bfc4fa28b632d26b6c559e596c1d4`;
seeds 2 to 6 zero divergences and zero disagreements, bytes read for
`ZK2_EXPECT` as `compare2.py` prints them, seeds 2 to 6 in order,
`0xdb5a7bd643bbf61a543dcb71a8afe6e27a39d44a6f3a37c302aa36ba027ddbae`,
`0xef7cce7bf98bd09871696bb3bdd0ed65cbe5797faaf958b1880f43278b666b4c`,
`0x814fdc27aeeea2a548da2c4ca75fd52cb5572202d3400888908a5514e1cbe278`,
`0xf1f5e3673c0446d86856fafc48675afb8ab9ff1780457492970e24beb0aaf3b3`,
`0x5357aa6890dbcef8355cfe512d66c172e5716dd2c32968cc8a9e030aa272fc4f`;
`bcov.py core2` measures 892 arms over 7903 cases, 59 never taken, the
round-4 set unchanged, classified in BOUNDARY-AUDIT.md.

The fold is the commit after `d1cfb609` on the revision branch; the naming
section names it.

### Naming, 2026-10-29

After round 6's fold the two readers re-read every folded sentence once more
(standard 之三 2) and found one reading in each. The release digest of
`zikaron/2` is

    0x42ade567a7697457c807ec30507f3c4b022841dc5cc1feaba63e226602a9350e

computed by `zikaron-conformance/criterion-digest2.sh` over the three files
of `impl2-py/` and the `zikaron/1` core digest §12.6 names; the core's bytes
are those of `1e5878cc` (the round-3 fold), unchanged through rounds 4 to 6,
and the digest is the one the tooling seat measured at every round since.
Named in §12.5 on 2026-10-29 in the commit after `d896854b` on the revision
branch, which is the freeze; the reader's one text item on §5.7's add-chain
parenthetical was folded in the wording the reader gave, so the sentence
named is the one re-read. The text of `docs/zikaron-v2.md` at the naming has
sha256 `8837f3ec90fbba3be0d0b11eb01f1ee0ea2a43dedde943f27d414b82f85f13dd`.
Rounds 1 to 6: three independent implementations (Rust, Node, Python), seeds
1 to 6, the closing measurement zero at round 5 with the round-4 programs
unchanged and confirmed at round 6 with the round-5 programs unchanged, and
more than 320,000 fuzzed invocations over the six rounds with zero
disagreements on any output of the law. No digest is superseded: this is the
first naming of `zikaron/2`. The items left to the judge stand in the Round
1 to Round 6 sections: A4 (rule 2 split across the segments), A2's inverted
form of the handback, A15 (offline continuation), and the kit law's py 2
(anchoring by segment). On 2026-10-29 the judge ruled that A4 stands as
written: the label a continued ledger shows while the reader's own node is
silent is the text's, since a node that answers resolves the row to the same
GAPS under either reading and no outside party can produce the silence.


## Revision of 2026-10-30: the second of the handing off

**Why the text moved after its naming.** On 2026-10-30 the judge ruled, for
the GALEED ledger law and for this one alike, that in the second of a key's
handing off what the key itself signed at a position after its handing off
counts for nothing, and that everything else of that second counts as it
did. Under the text named on 2026-10-29 a key had retired only at records
later than its retirement, so a byte string the ceding key signed after its
succession and anchored in the succession's own second was timely, a ledger
entry, and a certain `AUTHORITY_MISMATCH` that broke the chain, while the
same record one second later stood in `DISCARDED`; the boundary sat on the
block's second, which a key's two tools, a re-send or a second machine can
share with the handing off. The rule is the one `galeed/1` §7.4 took on the
same day (its record's "Revision of 2026-10-30" carries the parallel); this
law's §12.6 cut is untouched, since the closing succession retires no key
under this law.

**What changed.** §7.4 defines a key's **retiring seq**: the smallest `seq`
among the byte strings that pass §4.3, are a `succession` or a `migration`
and have the key as `author`, whose `entry_id` is the `hash` of a record of
the anchor set whose `blockTimestamp` equals the key's retirement, on any
chain and whatever that record's `sender` and `verdict`. A key **has
retired** at a record later than its retirement, and at a record of the
retirement's own second whose `hash` is the `entry_id` of a §4.3-passing
byte string by that key at a `seq` greater than its retiring seq, and at no
other record; timeliness reads "some record … whose `blockTimestamp` is
earlier than that `author`'s retirement, or equals it while `b` sits at a
`seq` not greater than that `author`'s retiring seq" where it read "whose
`blockTimestamp` is not later than that retirement", so that in the
retirement's second a byte string is made timely by its own position against
the retiring seq, whatever the record anchors and however the line reaches
it, while the record predicate serves §8.1's trim and §8.4; the gloss, the
handing-off sentence and the second-succession sentence follow; §8.1's trim
and §8.4's custody sentence say "at a record at which it has retired" where
they said "after its retirement", §8.4's fork sentence gives the holder's
deadline by position, and §8.5's `RETIRED` reason and §8.6 item 8 name what
no record makes timely. §12.5 returned to its unnamed form. The core
`impl2-py/zk2audit.py` moved in one predicate: `retirements` returns the
retiring seq beside the retirement, `retired_at(key, record)` is the
predicate above, `timely_set` marks an entry timely through any record
earlier than its author's retirement, or of the retirement's own second
while the entry sits at or below the author's retiring seq, and the trim
asks the predicate of the record's sender; `zk2.py` and `zk2entry.py` are
unchanged. `HARNESS2.md` is unchanged.

### Round 0: the checkpoint

The text items were applied to `docs/zikaron-v2.md` and
`docs/zikaron-kit-v2.md` with every existing passage drawn from the text and
occurring once in it, and the touched paragraphs rewrapped at 76 columns.
The corpus generator's predictor took the same predicate; the hand-built
cases gained eleven scenarios with their reversed twins (22 cases, audit 795
at seed 1, from 773): the ceding key's late twin at seq 3 anchored in the
retirement's second by the key itself (`COMPLETE`, the record `DISCARDED`,
the twin `RETIRED`), by the successor (`GAPS`: the record counts, bounds
nothing of the ceding key's, and its hash is `MISSING`), by a stranger
(`COMPLETE`), on another chain at the retirement's block time (`COMPLETE`)
and beside the key's hash-only record of that second (`GAPS`: the hash-only
record counts); the key's re-anchor of its seq-1 entry in that second
(`COMPLETE`, anchored twice); its late succession at seq 3 anchored in that
second by the key and by the stranger it names (`COMPLETE`, the succession
extends nothing, the stranger `EXCLUDED`); two handing offs of one key at
seq 2 and 3 in one second (`COMPLETE`, the retiring seq 2, the higher
succession admits nobody); two successions at seq 2 in one second
(`BROKEN_CHAIN`, both at the retiring seq, both timely, a fork); a migration
and the key's entry after the close in one second (`COMPLETE`). The notes of
`retire_same_second_counts` and `retire_cross_chain_same_second` no longer
state "and at no other record" as the rule; their hash-only records count as
before and their labels stand. The kit generator's predictor took the same
change; one kit case moved with it, was rebuilt, and a twin of the old
reading was added (687 cases; the kit law's record says which). The core
named on 2026-10-29 and the revised core agree on every one of the 773 audit
cases of the committed corpus, so no answer of the base corpus moves; over
the regenerated corpus the two differ on 18 of the 22 new cases, each read
`BROKEN_CHAIN` by the old core, as the judge's description of the fault
predicts, while `retire_same_second_reanchor_before_handoff` (`COMPLETE`)
and `retire_two_successions_same_position_same_second` (`BROKEN_CHAIN`), the
clause's other side, read alike under both, with their reversed twins; the
two kit cores differ on the rebuilt pair alone. `compare2.py` and
`compare-kit2.py` at seed 1, the criterion as candidate: zero divergences,
zero disagreements with the predictions, hand and code agreeing on every
case (corpus2 manifest sha256
`0xd77e3d1cd0bbd47561819053af2a14e06caabb80946f033639ee97a3c514e4d8`, bytes
read `0x79e239f8cece3b1baa19ba83e73124cf5399c04fcd3c78133d63e8200b83d81b`;
kit-corpus2 manifest
`0x29611f21f58e8f70389b1f81f7a98373a179464b4e11683b143fb4da7bd819b8`, bytes
read `0x02538e884f25ba1304dd44d651ce7858e07778e6861131a3aacf9bd752c15ff4`).
At seeds 2 to 6, the generators rerun in a copy of the tree, `compare2.py`
reads zero divergences and zero disagreements at every seed, hand and code
agreeing on every case (audit 792 to 795 cases; bytes read
`0xa29c390aa2f1ff867998c95a2ff55189a88b7a1f8ee7e8cf5e54f9c0fa72cfcd`,
`0x86d2538cf3c4ca3d7a2498e3955e43296b73efd799112c854ba6976cba434ae5`,
`0xe1f2d79f4d847ed5d96261e867587d89ced71d32d8acc03f626df559d5d0a234`,
`0xde8a45ce107e548ff2673787067eea0e9f8dbc6343bef6259d4432e36738f8c3`,
`0x7d83179862a872546280e7263ab33bd820b0bd3ca5a8900634f64d682def5f94`).
`bcov.py core2` at seed 1: 7925 cases, 893 branch arms, 59 not both-sided,
the round-6 set with line numbers moved, every arm of the moved predicate
that the measure sees taken on both sides; at seeds 2 to 6 the same 59 (7922
to 7925 cases). `BOUNDARY-AUDIT.md` carries the revision's section.

### Round 1: six seats, one load-bearing item, one fold

Six seats read the checkpoint `2245f68a`: Rust, Node and Python implementers
of `zk2` and `zkk2`, their round-6 programs first and unchanged, and three
readers, A over the law's text, D over the corpora and comparators, E over
the tooling and the records. The first measurement with the round-6 programs
unchanged gave, for all three implementers and at every seed 1 to 6, 18
divergences in `compare2.py`, the 18 moved cases in six classes (the ceding
key's late twin anchored in the retirement's second by the key, on another
chain, after its migration, beside its hash-only record, by the successor
and by a stranger; its late succession anchored by the key and by the key it
names; two handing offs at seq 2 and 3 in one second), each resting on the
same-second clause of §7.4, with the two coverage disagreements that follow
from them, and 2 in `compare-kit2.py`, the rebuilt pair; brought to the
round-0 text from the diff alone, the programs read zero divergences and
zero disagreements at every seed. Probes with the criterion as a black box:
Rust 12,000 audits and 10,500 kit invocations; Node 12,000 audits, 12,000
kit invocations and 5,640 determinism runs; Python 19,524 audits in 42,048
invocations, 16,000 kit inputs and 1,500 audits under reshuffled inputs and
two hash seeds; no disagreement, panic, hang or nondeterministic answer.
Reader D scored nineteen predictor mutants against the criterion at seeds 1
to 6 and built twenty inputs per seed; reader A ran ten core mutants and
four kit mutants over the corpora; reader E reproduced every count and
digest of the checkpoint and falsified five of its sentences.

Five seats, A, D, Rust, Python and Node, found the same `[load-bearing]`
item, which the comparators could not see because the predictor and the core
shared it: the same-second clause was asked of the byte string the record
anchors, and timeliness then ran along `prev` through byte strings of any
author, so a record of the retirement's second anchoring another key's byte
string, or the ceding key's own at or below its retiring seq, whose `prev`
line reached the key's byte string above its retiring seq was a record at
which the key had not retired, and that byte string was timely, a ledger
entry, a certain `AUTHORITY_MISMATCH` and a fork. The corpus held it as
`retired_twin_timely_through_stranger_anchored_in_time`, a stranger's seq-4
entry over the root's seq-3 twin anchored by the stranger at the
retirement's time, `BROKEN_CHAIN`, predicted and measured alike, and three
sentences of the text said the opposite: §7.4's gloss "the second of the
handing off included", its lineage paragraph "for a succession at or before
the retiring seq alone", and §8.4's "the second of the handing off
included". The harm falls on the seat and needs no custody failure beyond
the one the revision addresses: whoever holds the ceding key's same-second
twin, and any address a reader's basis scans, signs one entry naming it as
`prev` and lands its record in the block second of the handing off, on any
chain of the basis; the seat's ledger reads `BROKEN_CHAIN` where a second
later it reads `COMPLETE`, every grant of the seat fails check 2 of
`zikaron.kit/2`, and a late succession bridged the same way admits its `to`.
The round-0 port of the `galeed/1` rule was literal, record by record, where
that law's walk reads one key's entries and this law's timeliness reads
lines of any author.

Four forms were proposed. A: a line test, the line from the anchored byte
string to `b` holding nothing `b`'s author signed above its retiring seq. D:
the record predicate over the whole anchored line, a record of that second
being one at which the key has retired if any byte string on the anchored
line is the key's above its retiring seq, so that such records are discarded
too. Rust and Node: timeliness at the retirement's second decided by the
position of the byte string being judged, the record predicate unchanged for
the trim. Python: the same position test conjoined with "at which the author
has not retired". The fold takes the Rust and Node form, the ruling's
letter: what the key signed at a position after its handing off counts for
nothing in that second, whoever anchors it and however the line reaches it,
and everything else of that second counts as it did; it is decided by the
one number §7.4 already defines and closes every `prev` route at once. The
forms differ at two built cases, each form run as a predictor mutant over
seeds 1 to 6. At
`retire_same_second_low_twin_reached_only_through_late_twin`, the root's
seq-2 twin of its succession reachable only through its seq-3 entry, which
the root anchors in that second, the position test reads the twin, at the
retiring seq, as timely and forking the succession (`BROKEN_CHAIN`), while
A's, D's and Python's forms read it as not timely (`COMPLETE`). At
`retire_same_second_own_low_entry_naming_late_twin`, the root's seq-1 entry
whose `prev` is its late twin, anchored by the root in that second, D's form
alone differs: the line holds the root's byte string above its retiring seq,
so under it the record bounds nothing of the root's, and a fork at seq 1 the
root signed while it held the seat reads `COMPLETE`. The position test was
kept because a byte string at or before the retiring seq is exactly what the
ruling leaves counting, and whoever holds the key reaches the same fork by
anchoring that twin directly, so the conjunction protects nobody and adds a
second test to the sentence. Measured against the round-0 core over the
folded corpus, the chosen form moves 14 cases at every seed 1 to 6, 10 with
their label, the seven scenarios of the channel with their reversed twins:
the low twin reached only through the late twin (`COMPLETE` to
`BROKEN_CHAIN`), the fresh key's entry over the late twin anchored by the
root (`BROKEN_CHAIN` to `GAPS`), the stranger's entry over the twin anchored
in that second on the same chain and on another (`BROKEN_CHAIN` to
`COMPLETE`), the named key's entry over the late succession anchored by the
root (`BROKEN_CHAIN` to `GAPS`), and the successor's entry over the twin and
the root's own low entry naming it, `BROKEN_CHAIN` under both with the twin
`RETIRED` where it was a ledger entry; the 773 base cases and the 18 cases
round 0 moved read alike under both.

The text folded in one pass, every item drawn from the seats' proposals.
§7.4's timeliness sentence reads "whose `blockTimestamp` is earlier than
that `author`'s retirement, or equals it while `b` sits at a `seq` not
greater than that `author`'s retiring seq"; its gloss says what a record
bounds of a key's own byte strings before the retirement, in its second and
after it; the has-retired gloss names "a byte string that passes §4.3 and
that the key signed" (Python's item 2); the two-places sentence names both
positions the section reads, the anchored byte string's for the records of
§8.1 and each bounded byte string's for timeliness (Node's item 1); the
lineage paragraph speaks of every record of that second anchoring a handing
off at the retiring seq (Python's item 5) and gives a second succession's
timeliness by time and position; §8.4's fork sentence gives the holder's
deadline by position (Python's item 3, Node's item 2) and its custody
sentence names the second; §7.6, §1's `blockTimestamp` bullet and §12.6's
analogy follow; §8.5's `RETIRED` reason and §8.6 item 8 cite what no record
makes timely (Python's item 6); the kit law's §9 bullet is retitled "counts
for nothing at a record at which the key has retired", names migrations, and
says that a stray anchor of that second counts (Python's item 4, Node's item
3, D's item 7); the conformance README's two entries name the digest their
law's section names, naming their digest script and the section that names
the digest (E's item 6). Five notes of the corpus, ten files with their
reversed twins, give the ground of their answer by position where they gave
it by the record (D's item 5), and the rebuilt case's note follows its
rebuild.

The corpus gained sixteen scenarios with their reversed twins (32 cases;
audit 827 at seed 1, from 795). D's eight witness the clauses its surviving
mutants showed unwitnessed: the key's §4.3-failing bytes above its retiring
seq anchored in that second (`GAPS`, the record counts and its hash is
`MISSING`); its late twin anchored by its own `VOID` and `UNPROVEN` records
of that second (`COMPLETE`, `DISCARDED`, no `VOID` or `UNPROVEN` row); the
lower of two handing offs anchored in that second by a `VOID` record alone,
by a stranger alone and on another chain alone (`COMPLETE`, the retiring seq
2 each time); the low twin reached only through the late twin
(`BROKEN_CHAIN`, above); a fresh key's entry over the late twin anchored by
the root in that second (`GAPS`: the record counts, the twin stays
`RETIRED`, the entry `EXCLUDED`, the hash `MISSING`). Eight more close the
channel from every side:
`retired_twin_timely_through_stranger_anchored_in_time` rebuilt with the
stranger's record a second before the retirement (`BROKEN_CHAIN`, the twin
timely through a record earlier than the retirement) and the same record in
the retirement's second (`COMPLETE`); the successor's seq-4 entry over the
twin anchored by the successor in that second (`BROKEN_CHAIN`: the entry is
timely, the twin is not, and the entry's `prev` names no ledger entry); the
named key's entry over the root's late succession anchored by the root in
that second (`GAPS`); the root's own seq-1 entry whose `prev` is its late
twin, anchored by the root in that second (`BROKEN_CHAIN`, a fork at 1, the
twin `RETIRED`); the stranger's entry over the twin on another chain at the
retirement's block time, and one second after it (`COMPLETE`); and E's pair,
the root's record a second before its retirement anchoring the successor's
seq-3 entry (`COMPLETE`) and a stranger's seq-3 entry (`GAPS`). The kit
corpus gained two cases with twins (691 cases, checks 214; the kit law's
record says which).

`compare2.py` and `compare-kit2.py` at seed 1, the criterion as candidate:
zero divergences, zero disagreements, hand and code agreeing on every case
(corpus2 manifest sha256
`0x80b001f8feddec900584faf0fb6019fb83faff14b9017ad60f4ebfbc5082e273`, bytes
read `0x0a76de38ad3c131af2d436b11b22a63f4cf212f0e5964f149d4b537d2843e37d`;
kit-corpus2 manifest
`0x9103f91bd1a0fcb65c1049b9e6aee4aab3b9174c6ae7414231012f7faeb2e34b`, bytes
read `0x1202790e9765e4480d55ff33a83a90b601e4a16ba0efec83179ba4ba2b41cd4b`).
At seeds 2 to 6, the generators rerun in a copy of the tree: zero and zero
at every seed for both comparators (audit 824, 827, 827, 827 and 825 cases;
corpus2 bytes read
`0x03b787310e601a25506fea21378f0c728d1158885a5130d5d5c7c124dd67792a`,
`0x537ce0a516ec9572c7ba475b0eea0e56a96748e43b9ad3e094e0cbd83287f7df`,
`0x65d97bd87e4cc63720f45c24b07398d8f331d5e8abd42213011c11b2797cdf3a`,
`0xa252cabdcefd57c078604ef0c6ccef1c51d593d8f03d1035d100da5fe827a425`,
`0xd8d7d800bdfe10ddda2e80ffc028f69af3dd1b35a1f1439a0508f5afc4ba6688`;
kit-corpus2 bytes read
`0x1c7b38f5955cabaf7ceee37ab3eb483540b08c1c76172e69b434312394e8f35d`,
`0x8836ef86194d9bad31e2e8725d5191eefc61ba9e370d83ddf1859c3a93c34b06`,
`0xdc47387a5284fbbec453d5769e636b56b5e6ea167b99edee166546b527123eb3`,
`0x06d8bbc07165b033f0c6f1db359e392181d905174e4ca58ae24c0bcba76d7834`,
`0x94f523902574b9c60794f127ea255ed352bef790c080cfbfd20aac6815803908`).
`bcov.py core2` at seed 1: 7957 cases, 895 arms, 59 not both-sided, the
round-6 set with line numbers moved, the two arms the new timeliness loop
adds both-sided; at seeds 2 to 6 the same 59 (7954 to 7957 cases). `bcov.py
kit2`: 691 cases, 1352 arms, 164 not both-sided at seed 1 and 166, 165, 165,
166 and 165 at seeds 2 to 6, each seed's round-0 set, the `elif` alone
beyond round 6. The core named on 2026-10-29 against the folded core over
the regenerated corpus: 42 cases differ at every seed, 36 with their label,
all of the retirement's second: the 18 of round 0; fourteen of D's sixteen
(the §4.3-failing bytes read alike under both); and ten of the sixteen
through-another-byte-string cases, those whose record sits in that second
and reaches the late byte string, while the rebuilt case a second before,
the one-second-after case and E's pair read alike. The kit named on
2026-10-29 over the core named that day, against the folded tree at seed 1:
6 divergences and zero disagreements, the rebuilt pair and the two added kit
cases with their reversed twins.

`BOUNDARY-AUDIT.md`'s section was rewritten with the new counts, its arms
named by function and source text, the `elif`'s true side credited to the
reversed twins that meet the higher handing off first, the third test of
`retired_at` noted as a returned conjunction outside the measure, and blank
lines around its table (E's items 2 to 5); this record's round-0 section
counts 18 of 22 and carries the manifests' and the bytes-read digests (E's
items 1 and 8, D's item 3, Node's item 4); `impl2-py/NOTES.md`'s retirement
entry follows the fold. Not folded: D's item 6 (the comparators compare
`retired`, `discarded` and `missing` by size and refuse no unknown word),
E's inverted form for `bcov.py` (measure conditions, not arms) and D's
mutation table are tooling, outside the law and open to change at any time;
no round-1 case passes the comparators under a wrong reading, each being
caught through its kin, and the tooling stays as it is in this revision. E's
item 7, the package README's date, binds the package of this revision, which
gives the dates the laws state; E's item 9, the naming form, binds the
naming. A `[load-bearing]` item was folded, so the round does not close:
round 2 measures the three implementers' round-1 programs unchanged over the
folded corpora, expects the divergences on the moved and added cases alone,
brings the programs to the folded text, reads zero at seeds 1 to 6, and
re-reads the folded sentences.

### Round 2: six seats over the fold, text alone, the closing round

The same six seats read the fold `6aa5df9e`. The first measurement with the
round-1 programs unchanged gave, for Rust, Node and Python alike and at
every seed 1 to 6, 14 divergences in `compare2.py` with the two coverage
disagreements that follow from them, and 4 in `compare-kit2.py`: exactly the
seven channel scenarios round 1 added, with their reversed twins (the low
twin reached only through the late twin, `COMPLETE` to `BROKEN_CHAIN`; the
wrapper sent by the old key and the named key's entry over the late
succession, `BROKEN_CHAIN` to `GAPS`; the stranger's entry over the twin in
that second on the same chain and on another, `BROKEN_CHAIN` to `COMPLETE`;
the successor's entry over the twin and the root's own low entry naming it,
`BROKEN_CHAIN` under both with the twin `RETIRED`), and the two kit cases
with theirs; the other 18 added core cases, the rebuilt case, the base cases
and the other kit cases read alike, so the gate's expectation held. Brought
to the folded text from the diff alone, the three programs read zero
divergences and zero disagreements at every seed, each in one change to its
timeliness: Rust by each byte string's earliest bounding time against its
author's retirement and, at equality, its `seq` against the retiring seq;
Node by a `bounds(b, r)` predicate of the same three cases; Python by two
reaches along `prev`, from the records earlier than the retirement and from
those of its second. Probes against the folded core with the criterion as a
black box: Rust 11,304 distinct audits with 2,250 shuffled copies and 600
reruns (14,154 criterion runs, each matched), 5,500 kit cases and 1,500 kit
determinism runs; Node 9,000 core inputs and 6,000 kit inputs, each by both
programs, and 10,032 determinism invocations under fresh hash seeds; Python
12,000 audits in 39,000 invocations, 3,000 kit inputs in 12,000 invocations
and 1,000 determinism inputs six times each; no disagreement, panic, hang or
nondeterministic answer, the fold deciding the answer on 2,484 of Rust's,
1,613 core and 1,407 kit of Node's and 1,801 of Python's inputs. Reader A
read 36 sentences, built 28 ledgers and ran six core mutants over seeds 1 to
6; reader D read the 100 cases of the revision's scenarios, ran 61 predictor
mutants at six seeds and built seven scenarios and two kit scenarios for the
seven that survived; reader E reproduced every count and digest of the fold
and ran the two old cores at every seed.

Twenty-three items came back, all `[text]`: on every corpus case at six
seeds and every probe the folded §7.4 read as written gives the criterion's
answer, and no implementation differs from it. One class carried most of
them. Three sentences still described the record predicate as inert where
timeliness reads a record's line whatever the trim does with the record:
§8.4's fork sentence ("nor by a lineage key at a record at which it has
retired"), the kit's §9 title ("counts for nothing at a record at which the
key has retired") and §8.6's closing sentence ("asks nothing"), with §6.6's
"its findings"; `retire_same_second_low_twin_reached_only_through_late_twin`
is `BROKEN_CHAIN` on a `DISCARDED` record alone, and D's kit case of the
same shape fails check 2 where the title promised nothing (A's item 1 and 2,
D's item 1, Rust's item 1, Python's item 1, Node's item 1). D marked the
class `[load-bearing]` because the law's sentences gave two answers to that
case; the core, the predictors and the three implementations give one,
§7.4's, and the sentences are glosses, so it folds as text. The second class
spoke of the moment a key signed where the rule reads a position: §7.4's
"signed after its seat had moved on … whenever it anchors" and "a key that
handed off names nobody into the seat", §8.4's "after handing off", the
kit's check 3 parenthetical, §8.6 item 8 (A's item 4, Python's item 2,
Rust's item 2, Node's item 1). The third was the lineage paragraph stating
the per-author rule for one key, "every entry on the line before it that the
key did not sign above its retiring seq", whose literal reading (A's mutant
L) passed every corpus at every seed while the criterion leaves an earlier
key's late byte string on the successor's line `RETIRED`, and "the earliest
record that sets the retirement among them" where two handing offs at
different positions share the second (A's item 3, Python's item 3, Rust's
item 4, Node's item 1); the two-places sentence's "the key" read the same
way (A's mutant F, caught 24 times). Node's item 2 found that "has retired …
whoever sends it" names a case no clause reads, the trim being the
predicate's only consumer and asking it of the sender. The kit's §9 bullet
drew three more: it dropped "passes §4.3" (Rust's item 3), it was stated
against the world where the parent decides by the pile (Node's item 3), and
it placed every stray anchor in `MISSING` where only a counted one stands
there (Python's item 4, A's item 2). D's item 2 found two notes giving a
ground the folded law does not use and the bridge case, now in the
retirement's second, witnessing nothing; D's item 3, seven clauses witnessed
on one side (L1, L5, H3, M4c, S5, Q9a, H7) and `BOUNDARY-AUDIT.md`'s
"Reading" claiming more than the branch measure shows; D's item 4, the forms
differing at two cases, not one. E's eight items corrected the kit2 range at
seeds 2 to 6 (165 to 166), the hand-named witnesses and a line reference in
`BOUNDARY-AUDIT.md`, the README's naming state ("candidate", "refuses
until"), the revision traces in `impl2-py/NOTES.md`, "seven notes" (five,
ten files), check 4 in one kit note, and the unnamed §10.4's silence on the
form its digest script reads.

The fold, in one pass. §7.4's has-retired sentence is stated over the
records a key sent, the predicate asked of a record's `sender` alone (§8.1)
and a record another key sent read under that key's retirement; its
two-places sentence names the key's byte strings; its gloss says what a
record makes timely of a key's own byte strings on the anchored line, all of
them where the record is earlier than the retirement, those at or before the
retiring seq where it is of the retirement's second, none where it is later,
so that a byte string signed above the retiring seq is made timely only by a
record earlier than the retirement and what is signed at or before it by
every record of that second whose line reaches it; "names nobody into the
seat by a succession above its retiring seq that no record earlier than its
retirement bounds"; the handing-off clause speaks of every record of that
second anchoring a handing off at the retiring seq and of the key's own byte
strings at or before it, another key's byte string on the line being timely
or not by that key's own retirement and retiring seq. §6.6 says that none of
the migrating key's own byte strings is made timely by its later record
while another key's on the anchored line are read under that key's
retirement. §8.4's fork sentence is stated by timeliness alone: a fork
cannot be manufactured out of what a lineage key signed above its retiring
seq except through a record earlier than its retirement, and can always be
manufactured out of what it signed at or before its retiring seq while some
record earlier than its retirement or of its second reaches it along `prev`,
whoever sends that record and whether or not §8.1 keeps it. §8.6 item 8 and
its closing sentence follow. The kit's §9 bullet is retitled "A retired
key's stray anchor enters no reconciliation at a record at which the key has
retired", decides by the pile and §4.3, says that such a record of the
retirement's second still reaches along `prev` the line it anchors so that a
fork there fails check 2, and places every other anchor of that second where
the parent's §8.5 and §8.6 place a kept record by its verdict; its check 3
parenthetical reads "signing above its retiring seq with no record earlier
than its retirement reaching it"; its unnamed §10.4 says the digest is
written on a line of its own, in backticks. The conformance README and the
two generators' README templates name the cores by role (reference
implementation, criterion) and carry no naming state; `BOUNDARY-AUDIT.md`
names its arms by function and source text, lists the witnesses the measure
showed, gives the kit2 range measured and claims of the branch measure only
what it shows; `impl2-py/NOTES.md` states the readings taken without the
readings they replaced; this record's round-1 section counts five notes and
two cases where the forms differ, and the kit record's note of its `PARTIAL`
case names check 4. Six corpus notes were reworded with it (the successor's
record of the late twin, the stranger's record of the late succession, and
the rebuilt bridge case).

The corpus gained D's seven scenarios, one rebuilt and six new, with their
reversed twins: `retired_twin_not_reached_through_malformed_bridge` rebuilt
with the stranger's record a second before the retirement (`COMPLETE`: a
near-entry failing §4.3 bridges nothing);
`retired_twin_not_reached_through_malformed_middle`, a passing entry over
the failing bridge (`COMPLETE`);
`retire_record_before_retirement_anchors_own_late_twin` (`BROKEN_CHAIN`: the
record counts, the twin is anchored and a fork);
`retire_lower_handoff_anchored_after_the_second_sets_no_retiring_seq` and
`retire_same_second_malformed_lower_handoff_sets_no_retiring_seq`
(`COMPLETE` and `GAPS`: the retiring seq reads the §4.3-passing byte strings
of the retirement's second alone);
`retire_successor_same_second_late_twin_by_its_key` (`COMPLETE`) and
`retire_successor_same_second_low_twin_reached_only_through_late_twin`
(`BROKEN_CHAIN`), the position test and the arm for the successor as for the
root. Two more close the per-author reading:
`retired_late_entry_of_earlier_key_on_successor_handoff_line`, the root's
seq-2 entry above its retiring seq 1 on the line of the successor's handing
off at seq 4, anchored at T+5 (`GAPS`: the successor's seq-3 entry is
timely, the root's stays `RETIRED`), the shape of A's N2 and Rust's H2; and
`retire_same_second_lower_rogue_handoff_genuine_entry_reached_through_genuine_handoff`,
a rogue succession of the root at seq 1 anchored by the key it names in the
second of the genuine handing off at seq 2, lowering the retiring seq to 1
(`BROKEN_CHAIN`: the genuine history at seq 1, reached only through the
genuine succession's record of that second, is timely by its position and
forks the rogue, while the genuine succession above the retiring seq admits
nobody), the shape Python's probes raised, which the conjoined forms read
`COMPLETE` with the rogue's `to` in office. The kit corpus gained D's two
cases with twins:
`c2_grant_before_own_migration_low_twin_bounded_by_discarded_record` (the
seat's grant at seq 1, its migration at seq 2 anchored at T0+2, its own
seq-2 twin of the migration and a seq-3 entry over it anchored by the seat
at T0+2: the record `DISCARDED`, the twin timely, a fork, check 2 `FAIL`)
and `green_grant_before_own_migration_low_twin_record_a_second_after`
(`GREEN`). Audit 843 at seed 1 (from 827; 840, 843, 843, 843, 841 at seeds 2
to 6); kit 695 cases, checks 218.

`compare2.py` and `compare-kit2.py` at seed 1, the criterion as candidate:
zero divergences, zero disagreements, hand and code agreeing on every case
(corpus2 manifest sha256
`0xd815190b7388b7d71f049b519b26b678434cd11d116efa01ef54cf21ba8ad879`, bytes
read `0xc6156347b65d3eb8f1d10c6e773afed15f4388725f6ab1104120952884de17d1`;
kit-corpus2 manifest
`0x5263fd1885fd2ccc347eb6a6fdd0b5de6649d0037b93cc8212df066c7fd1b79e`, bytes
read `0x643465e1108eb9b627fa9a346b8906ef13fc0ecdfa6189f7a7b044402d3730c9`);
at seeds 2 to 6 zero and zero for both (corpus2 bytes read
`0xbd2733194700dfd0037d8253d808eff3fd4d8bfcd2ce43bb64254bfc9cb84a0e`,
`0x2f908181af8460d46d51e83fa03fa5b7c31974495e74f8614c801d3c55835efb`,
`0x9564ef5d5f7b4111304c3220a51487e59b863f99a9df04ed9dbab444e340efc4`,
`0x38d338725f15df850a2326c575bb2964dacc46d0e7d0e14ac787c02402bce2b0`,
`0xc527eb0150dfa02c2a97710bdc7ffda1e783ddc599efcf04fae6198d635cce91`;
kit-corpus2 bytes read
`0x2eb9e89727f4d98b2d971ded7a8caf5175f5b4cac9e428ff2af5282242516c46`,
`0x8ab3a7218ffe896de23462a152946e73718cdf233320bac36acad23f2a570e39`,
`0x70ed79feb509d292381a63c858ed6afb8fc95a235710d9e2ba16648e9cdfcdf8`,
`0x5337b471c522ef030c7e2f3a404dcf7ed5de005709ae6214ba825490680e2fd3`,
`0x2b1a40ca92e15be02cd79fc0e6af4f395aa75b97cd3e29c92a36adc8ada74c05`).
`bcov.py core2`: 7973 cases, 895 arms, 59 not both-sided at seed 1, the
round-6 set, and the same 59 at seeds 2 to 6 (7970, 7973, 7973, 7973, 7971
cases); `bcov.py kit2`: 695 cases, 1352 arms, 164 not both-sided at seed 1
and 166, 165, 165, 166, 165 at seeds 2 to 6. The core named on 2026-10-29
against the folded core over the regenerated corpus: 48 cases differ at seed
1 (38 with their label; 48/38, 48/38, 48/38, 48/38, 48/38 at seeds 2 to 6),
the 42 of round 1 and 6 of the new scenarios
(`retire_same_second_lower_rogue_handoff_genuine_entry_reached_through_genuine_handoff`
(BROKEN_CHAIN to BROKEN_CHAIN);
`retire_successor_same_second_late_twin_by_its_key` (BROKEN_CHAIN to
COMPLETE);
`retire_successor_same_second_low_twin_reached_only_through_late_twin`
(BROKEN_CHAIN to BROKEN_CHAIN)); the round-0 core: 18 cases at seed 1 (14
with their label), the fourteen of round 1 and
`retire_same_second_lower_rogue_handoff_genuine_entry_reached_through_genuine_handoff`
(COMPLETE to BROKEN_CHAIN);
`retire_successor_same_second_low_twin_reached_only_through_late_twin`
(COMPLETE to BROKEN_CHAIN). The kit named on 2026-10-29 over its core
against the folded tree at seed 1: 6 divergences, the rebuilt pair, round
1's two cases and round 2's two, with their twins. The closing measurement:
the three implementers' round-2 programs, unchanged, over the folded corpora
at seeds 1 to 6, each in its own regenerated copy of the tree: zero
divergences and zero disagreements in both comparators at every seed for
Rust, Node and Python, the bytes read equal to the criterion's; a text-only
fold with corpus additions and the previous round's programs reading zero on
first measurement, so the round closes.

Not folded, as tooling outside the law: D's clause-level mutation table
beside the branch measure (its harnesses stay in the seat's workspace;
`BOUNDARY-AUDIT.md` now claims only what the branch measure shows), Node's
proposal to define "bounds" as a named predicate and restate timeliness
through it (the defining sentence keeps its form, its one reading
established by A, and the word "bound" is used in one sense alone, a
record's line reaching a byte string), and the comparators' list comparison
by size (round 1). The §12.1 fuzzing against the core to be named: the
probes of round 2 ran against the core at `6aa5df9e`, which the round-2 fold
leaves byte for byte, Rust 14,154 audit and 7,000 kit runs, Node 15,000
inputs by both programs and 10,032 determinism invocations, Python 51,000
invocations, more than 100,000 agreeing invocations against the core named,
beside round 1's 132,000 against the checkpoint core. The naming follows.

### Naming (2026-10-31)

On 2026-10-31 §12.5 named the release digest
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a`,
computed by `zikaron-conformance/criterion-digest2.sh` over the three files
of `zikaron-conformance/impl2-py/` (`zk2.py`
`6349bbf21257103ddf600c4008a0fd630f5aa4cbbd49f375a6dba185285e280e`,
`zk2entry.py`
`dcbaec8d3971e98fed1b3d393a3ddeeb696cbe93c970a485242c840ea6bc11c8`,
`zk2audit.py`
`c04dc869b9f5749f97bc1c5de22cd29fbb711d214775a8fc41f76cdab30c8bde`) and the
`zikaron/1` core digest of §12.6
(`0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc`), and
the kit law's §10.4 took it as the digest of the core its own core embeds.
The digest supersedes
`0x42ade567a7697457c807ec30507f3c4b022841dc5cc1feaba63e226602a9350e`, named
on 2026-10-29 and unnamed by the revision of 2026-10-30 at its round 0;
`zk2.py` and `zk2entry.py` are the files of that naming byte for byte, and
`zk2audit.py` moved in the predicate `retirements`, the predicate
`retired_at`, the loop `timely_set` and the trim, as the rounds above say.
The law's text at the naming has sha256
`467d309d18d53fc91ab8b6ba8db03fa1eff29f35a49885d2426de01142da70f8`. How it
converged: a round 0 (the judge's rule applied to §7.4, the core, the
predictors and the corpora, Court `2245f68a`), a round 1 of six seats whose
one `[load-bearing]` item moved the timeliness sentence to the position of
the byte string judged (Court `6aa5df9e`), and a round 2 of the same six
seats whose items were all text, folded once, after which the three
implementers' round-2 programs, unchanged, read zero divergences and zero
disagreements over the folded corpora at seeds 1 to 6; the comparators with
the criterion as candidate read zero at every seed of both rounds, hand and
code agreeing on every case. §12.1's fuzzing: against the core named, the
probes of round 2, Rust 14,154 audit runs with 7,000 kit runs, Node 15,000
inputs by both programs with 10,032 determinism invocations, Python 51,000
invocations, zero disagreements, panics, hangs or nondeterministic answers;
against the checkpoint core of round 0, the probes of round 1, Rust 22,500,
Node 29,640 and Python 80,048 invocations, zero likewise; and the branch-arm
witnessing of `BOUNDARY-AUDIT.md`, 59 arms not both-sided under `core2` at
every seed, the round-6 set, none a decision of the law.

## Textbook revision of 2026-11-09: §9.3's consequence sentences

The kit law's revision of 2026-11-09 left one item for this text (its
record, round 1, SEC I-3): §9.3's consequence sentence said that an author
who authorizes a delegation "voids every anchor sent from that key in that
block and in every block until the designator is gone", which overstates the
case of a designator installed and cleared within one block, where the rule
of the same section finds the sender codeless at both block ends and the
anchor counts. The judge allowed wording that leaves the criterion as it is
to move with that revision's package. Two sentences of §9.3 moved, neither
stating a predicate, to the words `galeed/1` §9.3 carries for the same rule,
so that the two ledger laws state the consequence in the rule's own terms,
the two block ends:

- "a transaction that installs a designator on its own sender carries no
  anchor, and an author who authorizes a delegation voids every anchor sent
  from that key in that block and in every block until the designator is
  gone, and voids none sent before" reads "a transaction that installs a
  designator on its own sender carries no anchor that counts while the
  designator stands at the end of its block, its record then being `VOID`,
  and an author who authorizes a delegation voids every anchor sent from
  that key in every block at whose end the designator stands and in the
  block after each such block, and voids none sent before";

- "while a designator that stands voids every anchor that key sends on that
  chain until it is gone" reads "while a designator that stands voids every
  anchor that key sends on that chain until the block after the one in which
  it is gone".

The rule itself, §9.3's first sentence, is as it was, and so is every other
sentence of the law; §5's obligation keeps its words, which are
`galeed/1`'s. The core is the one §12.5 names: `criterion-digest2.sh`
recomputes
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a` over
`impl2-py/` unchanged in every byte and the `zikaron/1` core digest, and
`criterion-digest-kit2.sh` recomputes the kit law's
`0x5905333068c04ec2c4b096269f8baa0f2864925dc31910ae4e528e0c0899125d`.
`corpus2/gen.py --seed 1` rebuilds the committed corpus byte for byte
(manifest sha256
`0xd815190b7388b7d71f049b519b26b678434cd11d116efa01ef54cf21ba8ad879`) and
`kit-corpus2/gen.py --seed 1` the kit corpus of the kit law's naming
(manifest sha256
`0xb21486526aac54361ab8c105ea312435661d46d3b9ecd029c7dea6882535831d`);
`compare2.py` and `compare-kit2.py`, the criterion as candidate, read zero
divergences and zero disagreements at seeds 1 to 6, the bytes read pinned at
every seed to the digests the revision of 2026-10-30 (round 2) and the kit
law's revision of 2026-11-09 (round 4) recorded; the 69 recordings of
`zikaron-core/fixtures` replay through `scan-py/scan_replay.py` to their
expected outputs; the forge suite passes, 71 tests, the two codeHashes as
`CODEHASH.md` carries them. No corpus input, prediction or answer moved. The
text's own sha256 after this revision is
`349f8b82ea5be546fb8ebe20215e72ab3a5034ffcef4a136584fdc1b4e525bf6` (before
it, `467d309d18d53fc91ab8b6ba8db03fa1eff29f35a49885d2426de01142da70f8`).
`zikaron/1` §9.3 carries the earlier words of the first sentence and is not
touched by this revision.

## Corpus growth of 2026-11-11: other readings

An issue from outside (kaptonia/zikaron issue 5, opened 2026-11-10) named
three places where a wrong reading of `zikaron/1` passes that law's corpus.
The reporter holds an implementation of `zikaron/1` in Haskell, written from
the text and the harness contract alone and at zero divergences from that
law's core over the corpora of seeds 1 to 5, and put the opposite question to
the corpus: a program that takes the other reading of one contested point and
reads every other point as the core does, is it told from the core by any
case? Ten such programs were run. The corpus told seven from the core at every
seed, two at no seed, and one at seed 4 alone, through two random ledgers. The
text decides each of the three points and the core reads each as the text
does, so the law and the criterion stand as they were. The gap is the
corpus's, and it lies where the criterion passes without a branch, which is
why `bcov.py`, a count of the criterion's own branch arms, showed it nowhere.

`corpus2/`, the corpus of this law, carried the same three:

- §3.5 test 2. A bracket the grammar refuses, standing inside 128 containers,
  answers `E_JSON`; a parser that tests depth at every bracket byte answers
  `E_DEPTH`. No case stood there at any seed.
- §8.1. The pile collapses by bytes; a reader that collapses it by the
  transport's spelling keeps two copies of a byte string written once in each
  case of the hexadecimal digits. No case spelled a byte string twice at any
  seed.
- §8.2. A succession by the key in office moves the seat after a gap as it
  does before one; a reader that stops moving it records an
  `AUTHORITY_MISMATCH` the law does not. Random ledgers reached the point at
  seeds 2 and 4 (4 and 2 cases) and at no other seed.

The growth is in the instruments alone (standard, general rule 2): a measure,
43 corpus cases, and the section of `BOUNDARY-AUDIT.md` that reads the
measure.

**The measure.** `zikaron-conformance/readings2.py` holds a table of the
points of this law that have been read two ways: by the reporter's notes, by
`impl2-py/NOTES.md` and by the reading contests this record carries. A row is
one variant of the criterion, its sources with one reading replaced and every
other byte kept, the replacement anchored on a text that must occur exactly
once, so that a row the criterion has moved away from stops the run. For each
seed the measure generates the corpus in a temporary directory, answers every
canon, sign and audit case with the criterion and with every variant, and
exits 1 unless each variant differs from the criterion on at least one case at
every seed. The table holds 81 rows: one under §3.1, three under §3.3, eight
under §3.5, twelve under §4.3, four under §5.4, one under §6, one under §6.3,
nine under §7.4, two under §8.1, eleven under §8.2, two under §8.4, two under
§8.5, six under §8.6, six under §9.4 and thirteen under §12.6.

Over the corpus Court `9e3670b4` generates, seeds 1 to 6 (4409 to 4412 canon,
sign and audit cases a seed), the measure exits 1: 68 rows caught at every
seed, thirteen missed at one seed or more. Seven of the thirteen are the three
reported points: the depth test at each of the four places the grammar refuses
a bracket, the pile by spelling, the seat frozen after a gap, and the same
seat in the old segment of a continued ledger. Six are of the same class and
were found by the table: four orders of §4.3's steps (3 against 4, 7 against
8, the form of step 8 against its relation, 8 against 9), §5.4's first bullet
against its second, and the old segment's comparison of authority after a gap.
Twelve of the thirteen were caught at no seed, the frozen seat at seeds 2 and
4 alone. `BOUNDARY-AUDIT.md` carries the thirteen rows and the method.

**The cases.** `corpus2/gen.py` gained `build_readings`: 19 canon cases
(thirteen brackets at depth 129, ten of them refused by the grammar and
answering `E_JSON`, three admitted at a value position and answering
`E_DEPTH`; four orders of §4.3's steps; `E_SIG_V` ahead of `E_SIG_RANGE` and
of `E_SIG_HIGH_S`) and 24 audit cases, twelve ledgers each with its reversed
twin (five piles that spell one byte string in two cases, four successions at
or after a gap, three continued ledgers that put the same points to the old
segment). They draw nothing from the seed's stream and are registered after
every other canon and audit case, so every seed carries all 43 and every case
the corpus held keeps its name, its bytes and its prediction. Seed 1 holds
4485 cases (audit 867, canon 3550, misuse 30, sign 38) where it held 4442,
manifest sha256
`0xba45ec02852fbae42ae52688eb35969b4ee067a641ffe93076a0d71097cb1582` where it
was `0xd815190b7388b7d71f049b519b26b678434cd11d116efa01ef54cf21ba8ad879`.

**Where the expectations come from.** The generator's predictions are a third
opinion beside the criterion and the candidate, and every other part of
`gen.py` was written from the text with no implementation consulted.
`build_readings` was drafted in a clone of Court `9e3670b4` by an author who
had read `impl2-py/`. Before it entered this tree each of the 43 expectations
was read again from the text alone, clause by clause: §3.5 test 2 for the
brackets; §4.3's written order and its closing paragraph for the step orders;
§4.3 step 13 with the bullet order of §5.4 for the signature tokens; §8, §8.1,
§8.4 and §8.6 items 7 to 10 with `HARNESS2.md`'s pile encoding for the piles;
§8.2 for the seat; §12.6 with `zikaron/1` §8.1 and §8.2 for the continued
ledgers. All 43 stand as drafted, each on the clause named, and the
generator's docstring now names the clause under each family of cases and says
that this one part of the file has this history.

**Measured in this tree.** `gen.py --seed 1` rebuilds the committed corpus
byte for byte, under Python 3.12 and under 3.9. `compare2.py`, the criterion
as candidate, reads zero divergences and zero disagreements with the
generator's predictions at seeds 1 to 6 (bytes read
`0x1af87e8891f21fb04116c1233e9e06e67024730490932858e9b898af9ddc5ca6`,
`0xe204a8a73a578cae7920220e3146cd4e40ea4bd1b4349a98d31c9ae6a7bd2e84`,
`0x22664b9b55220769452a9e6c412b74bf1e3107364dfdfe88815f334e69e69088`,
`0xcb6e499bd36d4b2ce08c3f6d74e9dc4807b5060da540da749f9ef5b03c8196b4`,
`0xe450dc89b08805534e24247dab40e4c36ae8bb84de103b8a3ad412c7542562dd` and
`0x38acbac3e0f5603e7457b2fbb26c5dcd6539b60443e53056dd2bf8b039e29771`).
`readings2.py --seeds 1-6` exits 0, each of the 81 rows caught at every seed
(4452 to 4455 cases a seed), the thirteen by the cases built for them.
`criterion-digest2.sh` recomputes
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a` and
`criterion-digest.sh`
`0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc`;
`impl2-py/`, `impl-py/`, `compare2.py` and both law texts are the bytes of
`9e3670b4`. `bcov.py core2` at seed 1 counts 8035 cases, 895 arms and 59 not
both-sided, the 59 the same list, line for line, as over the earlier corpus
(7973 cases): the new cases stand where the criterion has no branch to take.

**The three implementations.** The fold adds corpus cases and moves no answer
of the criterion, so the standard (§5.1 item 7) asks for the three
implementers' programs, unchanged, over the grown corpus at seeds 1 to 6. They
are the round-2 programs that closed the revision of 2026-10-30, and they live
outside the tree. They were recovered from the seats' own session records and
identified before use: the Python program by the sha256 the seat recorded for
each of its eight files; the Rust program by building it and finding the
binary's sha256 the one the seat recorded,
`b4acbca75761142b652ee89c3ce7aa538860ce0f200813808cd5ee709bded135`; the Node
program, for which the seat recorded no digest, by the byte sizes of its eight
source files in the seat's round-1 listing and by the round-2 difference the
seat printed, both reproduced exactly. Each was then run over the corpus
`9e3670b4` generates: zero divergences and zero disagreements at seeds 1 to 6,
the bytes read equal to the digests this record carries for that corpus (the
revision of 2026-10-30, round 2), which is the reading those programs closed
that revision with. Over the grown corpus, unchanged, Rust, Node and Python
each read zero divergences and zero disagreements at every seed 1 to 6, the
bytes read equal to the six digests above: the three read all 43 new cases as
the law does, and the fold closes by item 7.

**What did not move.** The law's text, the criterion, the comparator and the
kit corpora are untouched. `corpus/`, the corpus of `zikaron/1` against which
the report was made, is retired and grows no more; `readings2.py` measures
`corpus2/` alone.

**The package.** The replacement package of 2026-11-11 carries this growth;
its lock takes the counts and the manifest of `corpus2/` as they stand here.

**Before the judge.** Whether the standard should count this measure among the
closing conditions of §5.1, as a proposed item 10, is undecided, and the
sentence is in no tree until the judge rules. Until then `readings2.py` runs
as an instrument: when a new implementation's notes, a reader or an issue
names a point read two ways, a row is added, and the corpus grows where the
new row goes uncaught.
