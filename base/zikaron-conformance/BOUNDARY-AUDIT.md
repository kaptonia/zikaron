# Boundary audit: both sides of every closed boundary

A law is closed at a boundary when the criterion decides every input on one
side or the other of it. A boundary is **witnessed on both sides** when the
corpus holds an input the criterion decides one way and an input it decides
the other way, so that an implementation which draws the line anywhere else
diverges from the criterion on at least one case. This audit measures that
mechanically for the three criterion programs and their corpora, then reads
every boundary the corpora had not witnessed against the law, and fills the
ones the law decides.

## Method

`bcov.py core|kit|scan|core2|kit2 <out.json>` runs one criterion program
in-process over its corpus with a tracer that records every executed
line-to-line arc inside the criterion's own files. Every `if` and `elif`,
every loop, and every exception handler in those files is a branch point; a
branch point whose arms were both taken by some case is witnessed on both
sides; the rest are listed with their source line. The measure is a tool in
the sense of the first principle of this repository: it never enters a
digest, the criterion is not modified for it, and a reading it gives is only
as good as the reading of the law that follows it.

The generators of both corpora already count the closed vocabularies the
laws name (every token, verdict, label and finding name) and report each one
witnessed in the output of the criterion; both report none unwitnessed.
Branch arms are the finer measure: they see a boundary the vocabulary
cannot, such as the positive side of a rule whose negative side already has
a token.

## Result, seed 1

| program | corpus | cases before → after | branch arms | arms not both-sided, before → after |
|---|---|---:|---:|---:|
| `impl-py` (zikaron/1 criterion) | `corpus/` | 1797 → 7254 → 7589 → 7647 | 591 → 653 | 48 → 35 → 35 → 35 |
| `kit-py` (zikaron.kit/1 candidate, embedding `impl-py`) | `kit-corpus/` | 913 → 978 → 992 | 1059 → 1127 | 149 → 128 → 140 |
| `scan-py` (§9 scanner) | `zikaron-core/fixtures/` | 21 → 53 → 69 | 289 → 378 | 101 → 67 → 79 |

Every arm that remains is classified below for the three `zikaron/1`
programs; none of those decides a label, a token, a verdict, or a record.
The second laws' programs have their own section and their own reading
below. The third figure in the `impl-py` row is the
recast of 2026-09-05, when the law and its criterion were rebuilt to the
galeed standard: the arms the recast added (two windows per chain, the reach
rule, the `DISCARDED` list, the audit reader's five restrictions, the
`sign` command's argument rules) are all witnessed, and the thirty-five that
remain are the classes below plus the `sign` misuse exits. The third figure
in the `scan-py` row is the same recast: the scanner now recovers every
sender from the transaction's signature (§9.1), tests the ordering and
uniqueness rules of §9.4, keys records on `(chainId, blockNumber, tx,
hash)`, and refuses creations and signature-less transactions in both forms;
the sixteen fixtures added (54 to 69) witness those arms, and the
seventy-nine that remain are the environment and unreachable classes below,
grown by the signature reader's own arms against a lying node.

**Seeds.** The corpora mix cases built by construction with seeded
mutations, so an arm can be witnessed by luck under one seed and missed
under another. Seeds 2, 3 and 4 were generated beside seed 1 and measured
the same way. The first pass found four arms of `impl-py` that flickered
between seeds (a junk byte after an array element, a text ending where a
value should begin, an escape letter outside the eight, and the order of the
two ids an `EQUIVOCATION` on a shared `prev` names). Patching each with a
hand-written case would have been a wall: the next seed finds the next arm.
The two classes were closed instead (next section), after which the set of
unwitnessed `impl-py` arms is identical under seeds 1 to 4 and no `kit-py`
arm is witnessed under one seed and missed under another. Re-measured after
the recast's third round (7647 cases, 653 arms): seeds 1 to 4 each leave the
same thirty-five arms, byte-identical lists, and every seed's predictions
agree with the criterion. The kit corpus was re-measured after the kit's
second round under the frozen parent (992 cases, 1127 arms): the `kit-py`
arms left unwitnessed are identical under seeds 1 to 4, and the `impl-py`
arms the kit corpus happens to reach still vary by seed (140 to 143 in all),
every one of them witnessed by the core corpus under every seed.

## Two boundaries closed by construction

**Grammar states (§3.5).** Every arm of the parser is a state of the grammar
meeting either the end of the text or a byte the state does not admit. The
corpus now holds, seed-free, every prefix and every single-byte substitution
(from a thirteen-byte alphabet covering each byte class the grammar
distinguishes) of nine short templates that together reach every construct:
object, array, every escape and a `\u` escape, integers, the three literals,
nesting, whitespace, multibyte UTF-8, a duplicate key. About 3,400 `canon/`
cases, deterministic under every seed; a state the templates reach has both
its end-of-text arm and its junk-byte arm witnessed, and a junk byte no one
thought of is covered with the rest.

**Bytewise order (§8.2, §8.4, §8.7, kit §9).** Wherever the law orders two
entries by `entry_id`, which of two hashes is smaller is luck, so the
generator now builds each such case in both orders by a deterministic
search over the competitor's prose: the two-way fork, two successions
competing at one seq (authority follows the smaller id, so which seq-4
entry is an `AUTHORITY_MISMATCH` flips), two entries on one `prev`, and the
kit reading's `H0` among twins. Wherever the law sorts a list of the report
or says the report does not depend on input order (§8, §8.7), every audit
case, depth reading and grant check with array inputs registers a twin with
every input array reversed, so each sorted list is witnessed from an input
in another order. About 120 audit cases and 47 kit cases, seed-free.

## Boundaries the corpora had not witnessed, now filled

### zikaron/1, in `corpus/gen.py`

- **§3.5 test 2, texts that end early or carry a byte the state does not
  admit.** Found first as five missing truncations, then as three more arms
  under other seeds; closed as the grammar-state construction above.
- **§7.4 with §9.5, the prefix lineage's positive side.** The corpus
  witnessed a key that joins the lineage after the adoption and never one
  that joined before it: no case had a succession earlier than an adoption
  whose evidence sender was the successor. Two `audit/` cases, one where
  the sender is the successor and one where it is the root that handed the
  seat over, both proven.
- **§9.4 input forms, the no-label answer.** The corpus witnessed malformed
  members and never an input that is not a JSON text, not UTF-8, carries a
  duplicate member name, a fraction, or `NaN`, or a pile element that lacks
  `0x` or carries a non-hex character. Seven `audit/` cases, two of them
  written as raw bytes (`A_raw`).
- **§8.4, the two ids of an equivocation on a shared `prev`.** The finding
  names the bytewise-smaller id first whichever entry the walk met first;
  the corpus had witnessed one order only, by luck of the hashes. Closed
  as the bytewise-order construction above, together with the other
  clauses of its kind.

### zikaron.kit/1, in `kit-corpus/gen.py`

- **§3.1 from the signing side.** `zkk sign` is in the harness contract and
  no case exercised it; the corpus witnessed the two domains only through
  verification. Two `sign/` cases, one per domain, predicted by the
  generator's own signature path.
- **§7.1, an entry neither regular file nor directory.** A named pipe
  (`unreadable_fifo`). The walk must classify before it opens: a reader that
  opens first waits on the pipe forever.
- **§7.3, rows of `files`, `contents` and `proofs`.** Nine kits: each of the
  three members not an array, a `files` row with a malformed `sha256`, a
  `contents` row with a member outside its two or a malformed `content`, a
  `proofs` row with a member outside its three, a malformed path, or a
  malformed `sha256`.
- **§9 with the parent's §8.1.** A depth input that is not an object, one
  whose pile is not an array, a pile carrying every entry twice (the reading
  equals the reading over the pile), and a counted history whose `prev`
  names no entry in hand, so that the first history is not reachable from it
  and its `seq` does not join the counted seqs.
- **§10.1, `I'` over an invalid audit input.** Two grant checks whose audit
  input is not an object or whose pile is not an array.

### Chain-facing §9, in `scan-py/scenarios.py`, fixtures 22 to 69

- **§9.1 log form from a declared registry**: two topics, three topics with
  data, four topics (22 to 24, the `Rogue` contract).
- **§9.1 status 0 of the bare form**: a sender delegated under EIP-7702 to
  code that reverts, then sending itself 32 bytes (25). Status is tested
  before the codeless test, so the record is absent rather than `VOID`.
- **§9.3 block 0 has one boundary** (26). No node includes a transaction in
  its genesis block, so this recording is written by hand and says so.
- **§9.4 basis forms**, twenty fixtures with an empty `rpc` and the
  no-label fragment (27 to 46).
- **§9.4 one record per (chainId, blockNumber, tx, hash)** through
  `anchorMany` with one hash twice (47); **§9.5 an element named twice**
  yields one evidence record (48); **a chains object with no registries**
  reads no log and asks for none (49); **a transaction the chain does not
  carry** in `bareTx` and among the elements (50, 51); **a pending
  transaction** in both places (52, 53).
- **§9.1 the sender is the recovered signer**, hand-written and signed with
  test keys: a legacy signature naming no chain (54), a signature-less
  system transaction in both forms and as evidence (57), a type this scanner
  does not encode (69), and the positive side over the type 1, 3 and 4
  signing payloads (59, 67, 68); **a registry-form log inside a creation**
  (55) and **an adoption naming a creation**, whose record carries the
  empty calldata (58).
- **§9.4 two windows of one chain** adjacent with differing `senders` (56),
  and the **seven ordering and uniqueness refusals** (60 to 66).

### A correction to the third opinion

The kit generator's prediction for `attribute` over a pairing that is not
`PAIRED` carried the pairing's token; `HARNESS-KIT.md` gives
`{"attributed":false,"verdict":"<pairing verdict>"}` with no token, which is
what the criterion answers. The prediction was wrong and was corrected; no
implementation and no text changed.

## Arms that remain, by class

**Harness misuse.** Argument counts, an unreadable file, a private key out
of form or range, an unknown command, a kit path that is not a directory, a
`--now` without a value, a hops file that is not an array of hops. The
harness contracts fix exit status 2 and no message for these, so a
candidate's wording there is its own and the comparators cannot hold such a
case. Nine arms in `zk1.py`, thirty in `zkk.py`, three in `scan_replay.py`.

**Arms the law makes unreachable.** `attestation_ok` with `prev` null (§6.6:
an adoption is never at `seq` 0, and §6.5 rejects one, so no ledger entry
reaches this test); the fork walk's `a.eid == b.eid` (§8.1 collapses
byte-identical copies before the walk); `_emit`'s fall-through (a value
outside the universe cannot be parsed in); `int_literal` of an empty run
(the parser only calls it with a digit); the recovery test `r >= P` (`r < N
< P` after the range test); the Jacobian point-at-infinity arms and the RFC
6979 retry (a key in range with a digest never produces `k = 0` or `r = 0`);
the depth reading's cache hit (ledger entries are distinct); in the
scanner's recovery, `r` or `s` out of range, an `r` that is no curve
abscissa, and a recovered point at infinity (a transaction a chain included
carries a signature a node validated), and the Jacobian infinity arms again.

**Environment, in `scan_replay.py`.** A node answering outside the JSON-RPC
forms: a transaction, receipt, log or header that is not an object, a null
`eth_getLogs`, a header without `timestamp`, a null `eth_getCode`, an access
or authorization list entry that is not an object, a typed transaction whose
parity is neither 0 nor 1, and a transaction object whose signed bytes do
not hash to the hash it was asked for by (the node is answering about other
bytes). The scanner raises a loud error and produces no fragment; §9.3 names
the one case the law decides (state that cannot be consulted is `UNPROVEN`,
fixture 12), and the rest is the scanner's own conduct toward a broken node.

**Filter-implied, in `scan_replay.py`.** A log whose address is not
declared, whose topic 0 is another event's, or whose block lies outside the
range: `eth_getLogs` is asked with the address list, the topic 0 and the
range, so a conforming node never returns such a log, and a transaction with
status 0 emits no log that any node returns. These arms guard against a
lying node; the fixtures 05, 07, 08 and 20 witness the same sentences of
§9.1 on the side a node can produce.

**Tracing artefacts.** Module-level loops that ran before the tracer was
installed, `__main__` guards, `__repr__`, and helpers no command reaches
(`presig_of`, `jneg`).

## `zikaron/2` and `zikaron.kit/2`: `bcov.py core2`, `bcov.py kit2`

The second laws' candidates are measured the same way over `corpus2/` and
`kit-corpus2/` at seed 1. `core2` traces `impl2-py/` and, since the
candidate runs it, the frozen `impl-py/`; `kit2` traces `kit2-py/`,
`impl2-py/` and `impl-py/`. The revision of 2026-10-27, round 6:

| program | corpus | cases | branch arms | arms not both-sided |
|---|---|---:|---:|---:|
| `impl2-py` (zikaron/2 candidate, over `impl-py`) | `corpus2/` | 7903 | 892 | 59 |
| `kit2-py` (zikaron.kit/2 candidate, over both) | `kit-corpus2/` | 685 | 1350 | 163 |

Four classes join the ones above.

**Unreachable by §12.6's empty inputs.** The `zikaron/2` core hands the
frozen `zikaron/1` core an old segment whose unavailable set and adoption
evidence set are empty and whose basis has empty `bareTx` and
`adoptionChains`, so `zkaudit.py`'s arms for the bare form, the evidence
records, the attestation test, the adoption proof (`_prefix_lineage`,
`_contains_at_word`, the `cid in adoption_chain_ids` branch) and the
unavailable set are never entered from this candidate; they are witnessed by
`zikaron/1`'s own corpus, which the digest `0xbecf…32fc` freezes.

**Decided first by the caller.** `zkentry.py`'s envelope tests at
`zikaron/1` §4.3 steps 2 and 3, worded as `zikaron/2` §4.3 steps 1 to 3, and
`zkaudit.py`'s input well-formedness: the `zikaron/2` core has already
refused what would take them, and the old segment is a subset of an input
§9.4 passed. In `kit2-py`, `load_view`'s call with no extra pile, which no
command makes.

**Reachable through the old segment.** `zkentry.py`'s body rows and
`zikaron/1` §4.3 steps 5 to 11, and `zkaudit.py`'s walk findings: a
`zikaron/1`-spec byte string that fails a later step, a `SEQ_GAP`, a
`PREV_MISMATCH`, a `ROOT_MISMATCH`, an unknown type and an adoption inside
an old segment. The walk findings, the unknown type and the adoption were
taken under `core2` from round 1 (`continue_old_segment_*`,
`continue_line_stops_before_*`,
`continue_old_root_key_readmitted_opens_genesis`); round 2 added one
old-segment byte string per `zikaron/1` §4.3 token of steps 5 to 13 and per
body type (`continue_old_segment_v1_token_*`), and round 3 one per required
body row on both sides and round 4 one per optional row present and out of
form (`continue_old_segment_v1_row_*`), so that the nested report's
`malformed` tokens, which lie on the surface §12.2 compares, and the pass
side of each row are witnessed. Under `kit2` the old segments the kit corpus
builds take fewer of them, and the kit law decides nothing by them.

**`while True` has no false exit.** `zkcanon.py`'s three scanning loops
leave by `break` or `return`; the tracer reports a false arm that no input
can take.

Under `kit2`, the arms of `zkk2.py`'s own files that remain are, no misuse
arm among them since round 4, tracing artefacts (the `__main__` guard,
`doc_id`, the alphabet table); the non-UTF-8 directory name of §9, which
APFS refuses (errno 92); operating-system and encoding errors a corpus file
cannot raise once the reader holds it (`zkk2kit.py` `_walk_dir`'s `except
OSError:` and `is_kit_path`'s `except UnicodeEncodeError:`, `zkk2read.py`
`load_view`'s `except (TypeError, ValueError, UnicodeEncodeError):`);
`load_view`'s call with no extra pile, which no command makes; a reach that
meets an entry twice, which a tree of `prev` links never does; and a
`continues` that is no object, a `continues.entry_id` that is no string or
one naming no ledger entry, on a `seq`-0 ledger entry whose
`continues.entry_id` is the report's, which `zikaron/2` §6.1 and §12.6 close
first. The `impl2-py` arms missed under `kit2` are taken under `core2` (the
kit law reads the `zikaron/2` audit through its report alone), except
`zk2audit.py`'s equivocation self-meeting guard, untaken under both; the
`impl-py` arms missed under `kit2` are taken under `core2` except those
missed there too, which the `core2` classes hold. `zkaudit.py` :180 and :182
are the `zikaron/1` basis rule for two chain objects of one `chainId` whose
ranges touch, taken from round 4 by the two continued inputs whose basis has
two such objects (`continue_basis_two_windows_*`). `zk2audit.py`'s
`_equivocation` guard `if a.eid == b.eid:`, the equivocation scan's
self-meeting guard, is the `zikaron/2` twin of `zkaudit.py` :433 and sits in
the law-unreachable class with it. Under `core2`, `zkentry.py` :156 false
(`check_sig` with no tokens, the attestation path), `zkaudit.py` :267 and
:457 (duplicate records and duplicate byte strings, which the `zikaron/2`
core collapses first) and `zkcanon.py` :30 (`keys`, an artefact) are the
arms the classes above name by their kind. `zkk2read.py`'s old-segment arms
(a stranger's record, an `UNPROVEN` and a `VOID` record of the continued
segment, a counted record of excluded bytes, a reach stopping at a missing
`prev`) are taken from round 1
(`green_v1_grant_old_segment_stranger_record`,
`green_v1_grant_unproven_old_record`,
`c4_v1_grant_reached_only_by_old_unproven_record`,
`c4_unproven_record_of_excluded_bytes_complete`,
`c4_reach_stops_at_missing_prev`).

**Reading, `core2` and `kit2`.** The classification above is of the measured
list at seed 1 of each corpus; the arms a corpus could take and does not are
the round's corpus work, and every such arm is named here until a case takes
it. At seeds 2 to 6 of the round-6 corpora, measured at the round-6 fold,
`core2`'s missed set is identical to seed 1's at every seed (59 arms over
7900 to 7903 cases), and under `kit2` the `kit2-py` and `impl2-py` missed
sets are identical at every seed while 163 to 165 arms are missed in all,
the variation lying in `impl-py` arms (`zkaudit.py` :436, `zkcanon.py` :185,
:211, :222, :228) each taken under `core2` at every seed.

### The revision of 2026-10-30

| program | corpus | cases | branch arms | arms not both-sided |
|---|---|---:|---:|---:|
| `impl2-py` (zikaron/2 criterion, over `impl-py`) | `corpus2/` | 7973 | 895 | 59 |
| `kit2-py` (zikaron.kit/2 criterion, over both) | `kit-corpus2/` | 695 | 1352 | 164 |

The core moved in one predicate and one loop. `retirements` returns the
retiring seq beside the retirement, adding the `elif` that lowers a key's
retiring seq when a second handing off of the key is anchored in the same
second; `retired_at` of a key and a record is new (its call and its two
tests by time are arms of this measure; its third test, a returned
conjunction over the anchored bytes, is no arm), and the trim asks it of the
record's sender; `timely_set` marks an entry timely through a record earlier
than its author's retirement, or of the retirement's own second while the
entry sits at or below the author's retiring seq, in two tests (`if e.eid in
out` and the test by time and position) that replace the old bound. Under
`core2` every arm of the moved code that the measure sees is taken on both
sides at seed 1 and at seeds 2 to 6: the `elif`'s true side by the reversed
twins of `retire_same_second_late_succession_by_old_key`,
`retire_same_second_late_succession_by_stranger_record`,
`retire_two_handoffs_same_second_lower_sets_retiring_seq`,
`retire_two_handoffs_lower_anchored_by_void_record`,
`retire_two_handoffs_lower_anchored_by_stranger` and
`retire_two_handoffs_lower_anchored_on_other_chain`, whose input order meets
the higher handing off first (closed by construction: each case's reversed
twin), its false side by those cases in the other order and by the base
cases holding a second same-second handing off at no lower seq
(`retire_two_successions_same_position_same_second`,
`retire_smallest_time_across_chains`,
`retired_key_late_succession_extends_nothing`, the `add_chain_*` cases); the
position test's false side by every record later than an author's retirement
whose line holds that author's byte strings, by every case anchoring the
ceding key's late byte string in that second
(`retire_same_second_late_twin_by_old_key` and its kin,
`retire_same_second_low_twin_reached_only_through_late_twin` among them),
and by the cases reaching it through another key's byte string
(`retire_same_second_wrapper_sent_by_old_key`, the five
`retired_*_in_retirement_second` scenarios,
`retire_same_second_own_low_entry_naming_late_twin`). The missed set is the
round-6 set at every seed, line numbers moved (the true side of
`_equivocation`'s `if a.eid == b.eid:` among them), 7970 to 7973 cases;
round 2's sixteen core cases and four kit cases, witnesses of clauses the
code holds without a branch of its own, leave every set as it was. Under
`kit2` the kit corpus gained ten cases over round 6
(`c2_grant_after_own_migration_anchored_before_it`, the two
`c3_grant_after_own_migration_reached_through_stranger_*` cases,
`c2_grant_before_own_migration_low_twin_bounded_by_discarded_record`,
`green_grant_before_own_migration_low_twin_record_a_second_after`, and their
reversed twins; 695 cases) and one arm joins the list, taken under `core2`:
the `elif`, since no kit case holds two handing offs of one key in one
second. The rebuilt case
`c3_grant_after_own_migration_anchored_in_its_second`, whose grant anchored
in the migration's second is now `DISCARDED`, leaves the walk past a closed
ledger, which the added case takes again, so the `kit2-py` and `impl-py`
missed sets are the round-6 sets. At seeds 2 to 6 the `kit2` missed set is
seed 1's with the same `impl-py` variation as before (165 to 166 arms at
seeds 2 to 6: 166, 165, 165, 166, 165), the `elif` alone being new at each
seed.

### The revision of 2026-10-31

| program | corpus | cases | branch arms | arms not both-sided |
|---|---|---:|---:|---:|
| `kit2-py` (zikaron.kit/2 criterion, over both) | `kit-corpus2/` | 718 | 1356 | 164 |

The core moved in its key-set constants (`CONTENT_KEYSET` became
`CONTENT_KEYS` and `CONTENT_KEYS_NAMED`, one line, so every later line of
`zkk2kit.py` moved by one) and in `check_manifest`'s `contents` loop;
`core2` is untouched and its row above stands. Two branch points replace the
loop's first test, and one is added after the `path` test: `if not
isinstance(r, Obj)` (its true side by `rule_contents_row_not_object`, its
false side by every other row), `if ks != CONTENT_KEYS and ks !=
CONTENT_KEYS_NAMED` (its true side by `rule_contents_row_members` and
`rule_contents_row_members_beyond_name_md`, its false side by every row of
two members and by the named rows of `ok_directory_original_named` and its
kin), and `if "name_md" in rd and not is_prose(rd["name_md"])` (its true
side by `rule_contents_name_md_form`, `rule_contents_name_md_null` and the
package `kit_manifest_contents_name_md_form`, its false side by the named
rows that verify and, the conjunction short-circuiting, by every row without
the member). Under `kit2` every arm of the moved loop is taken on both sides
at seed 1 and at seeds 2 to 6; the `kit2-py` missed set is the set of the
revision of 2026-10-30 at every seed, line numbers moved; 718 cases at every
seed, from 695 (706 at round 0); at seeds 2 to 6 the count is 168, 165, 165,
166, 166: the `kit2-py` arms' missed set is seed 1's at every seed, and the
parent cores' arms vary with the randomized kits' entries, whose random
draws the directory names round 1 gave half the randomized directory
originals moved (`zk2entry.py`'s optional-member tests at seed 2, where one
arm is reached by no case and the measure counts 1355 arms; `zkcanon.py`'s
`\r` escape at seed 6), joining the `impl-py` variation across seeds seen
before.

### The revision of 2026-11-09

| program | corpus | cases | branch arms | arms not both-sided |
|---|---|---:|---:|---:|
| `kit2-py` (zikaron.kit/2 criterion, over both) | `kit-corpus2/` | 1084 | 1441 | 155 |

The core moved in `zkk2read.py` (the notice input, the key in office, the
member path) and `zkk2.py` (`--notices`, the three-member hop); `core2` is
untouched and its row above stands. The new branch points of round 0, each
taken on both sides at seed 1 and at seeds 2 to 6, the `if` and loop arms by
`bcov.py` and the conjunctions, comprehension filters and conditional
expressions, which `bcov.py` does not score, by the cases named (seat E's
conjunct-level tracer confirmed each conjunct witnessed as the first false
one at every seed): `load_notices`'s and `_records`'s `_need` conditions
(one arm pair in `_need`, its raising side by the `m_n_invalid_*` cases and
the raw files, its passing side by every valid input) and `load_input`'s
`NoLabel` (`m_n_not_json`, `m_n_not_utf8`, `m_n_repeated_member`,
`m_n_integer_as_float`); `_records`'s `if notice:` (both arrays of every
input); `in_office_at_position`'s and `in_office_at_block`'s `if p >=
pos`/`if p[0] >= block` break (records after and before the position) and
`if rd["root"] == root` (`m_root_from_author_no_audit_other_root_in_book`,
`m_c3_book_record_of_other_root_ignored` against every case of the root's
own records); `revocation_member`'s `if "revocation" not in bd`, `if
isinstance(m, Obj)` and the well-formed test (`m_member_malformed_*` against
`m_green`); `applies`' conjunction (`m_n_other_chain`, `m_n_other_contract`,
`m_no_audit_no_notices`); `book_root`'s `if view is not None` and `if
is_hex(led, 20)` (`m_root_from_ledger_member_no_audit`,
`m_root_from_author_no_audit`); `_member_checks`'s check 2 chain
(`m_no_audit_no_notices`, `m_c2_broken_chain`, `m_badge_alone`, `m_green`),
its gate on checks 3 and 4, the standing-record comprehension's conjunction
(every `m_c3_*`), the `PASS`/`FAIL`/`UNKNOWN` cascade (`m_green`,
`m_c3_anchor_by_successor_author_root`, `m_c3_root_anchor_after_accession`),
the check 6 walk's `continue`s (`m_c6_notice_of_another_grant`,
`m_c6_void_by_stranger_ignored`, `m_c6_notice_by_stranger_ignored`), its
`counted` break (`m_c6_notice_counted`) and its `UNPROVEN` fall-through
(`m_c6_notice_unproven`); `grant_check`'s dispatch on the member and the
member path's window tests (`m_c5_window_now_null`,
`m_window_expired_and_notice`); `ledger_link`'s `if view_d is None` and its
applying test (the `link_*_from_notices_*` chains,
`link_unknown_notices_not_applying`, `link_unknown_member_hop_no_inputs`);
`zkk2.py`'s `--notices` option arms and `load_notices_or_none`'s `NoNotice`
(the misuse cases and `m_n_invalid_*`). Under `kit2` the `kit2-py` missed
set is the set of the revision of 2026-10-31 at every seed, line numbers
moved; 928 cases at seed 1, from 718; at seeds 2 to 6 the count not
both-sided is 163, 160, 160, 161, 161, the variation lying in the parent
cores' arms as before.

Round 1 of the revision (Court `9b832dbf` to the fold) moved `zkk2read.py`
again: `book_root` takes the notice input and the member and walks the book
records for one naming the author as `to` (its `applies` test and loop, both
sides by `f_root_via_book_*` against `m_root_from_*`), `book_lineage` is
new, check 2 tests the segment's byte strings for a genesis of the seed
(`a_rotated_issuer_spine_green` against
`a_rotated_issuer_spine_without_genesis` and
`m_c2_genesis_of_stranger_only`), check 3 opens with the `EXCLUDED`
conviction (`a_forged_member_grant_excluded_fail` against
`a_excluded_but_seated_by_book_not_convicted` and
`a_forged_member_grant_badge_alone_not_convicted`), the check-6 walk lost
its `VOID` skip (`m_c6_notice_void_unknown` against
`m_c6_void_by_stranger_ignored`), and `zkk2.py`'s `read_file` catches
`ValueError` and `UnicodeError` (the two misuse cases
`hops_notices_path_nul` and `hops_grant_path_lone_surrogate`). At seed 1:
1009 cases, 1440 arms, 158 not both-sided; at seeds 2 to 6, 162, 159, 159,
160, 160; the `kit2-py` missed set is the round-0 set at every seed, line
numbers moved.

Round 2 of the revision (Court `7f36e452` to the fold) moved `zkk2read.py`
once more: `book_root` takes the book's record for the author before the
view's root (`f2_audit_under_author_conflicts_with_book` and
`d2_excluded_seated_under_other_root_fails_by_root` against
`f2_audit_root_agrees_with_book` and every member case the book is silent
on), check 3 convicts before its gate on the notice input applying, by a
conjunction over the table's two convictions and the book lineage read only
where the input applies (`d2_forged_member_other_chain_convicted`,
`d2_forged_member_null_convicted`,
`d2_forged_member_notices_null_convicted`,
`d2_v1_stranger_member_grant_after_cut_convicted` against
`d2_retired_grant_unseated_key_not_convicted`,
`a_excluded_but_seated_by_book_not_convicted` and
`d2_v1_own_key_member_grant_after_cut`), then answers `UNKNOWN` where the
input does not apply (`m_n_other_chain`) and `FAIL` where the view's root
differs from the derived one (`f2_thief_genesis_under_retired_key_fails`,
the chain `link_true_book_root_over_audit_under_author`), and check 4 reads
its standing records under the same gate
(`a2_forged_excluded_anchored_by_key_in_office` beside the convictions);
`zkk2.py`'s `read_file` tests the path's kind before opening it
(`grant_check_path_is_pipe` and `notices_path_is_pipe` against every
readable path). At seed 1: 1064 cases, 1446 arms, 158 not both-sided; at
seeds 2 to 6, 162, 159, 159, 160, 160; the `kit2-py` missed set is the
round-0 set at every seed, line numbers moved.

Round 3 of the revision (Court `340fff55` to the fold) moved `zkk2.py`
alone: `cmd_chain_check` reads the hops file through `zk2audit.load_input`
(its `NoLabel` arm by `hops_grant_path_surrogate_escape_of_existing_file`,
`hops_not_json`, `d2_hops_bom` and the two `d2_hops_*_lone_surrogate` cases,
the NUL-path cases passing the reader and being refused at the open, its
passing arm by every chain) and takes each hop from an `Obj`
(`hops_element_not_object` against every chain). The round-2 arms the corpus
held on one side are now both-sided at every seed: the book lineage read
only where the notice input applies, at the conviction
(`d3_forged_member_other_chain_book_not_read`) and at `book_root` through
the link (`d3_link_book_of_unapplied_input_not_read`,
`a3_link_nonapplying_book_ignored`, `a3_link_nonmember_hop_book_ignored`);
the `MALFORMED` conviction's `continued` conjunct
(`d3_v1_member_grant_uncontinued_v2_ledger_not_convicted` against
`d2_v1_stranger_member_grant_after_cut_convicted`); and `read_file`'s
`S_ISCHR` disjunct, which `bcov.py` does not score as an arm, by
`d3_notices_path_character_device` against the pipe cases, now at `--audit`
and the hop's two paths too. At seed 1: 1078 cases, 1441 arms, 156 not
both-sided; at seeds 2 to 6, 160, 157, 157, 158, 158 (seed 2 counts 1440
arms, one fewer than the other seeds, a parent-core arm the tracer sees only
where a random kit entry reaches it); the `kit2-py` missed set is the
round-0 set at every seed, line numbers moved.

Round 4 of the revision (Court `5fab845e` to the fold) moved no criterion
file: the fold is prose, six corpus cases and the comparator. The path-kind
test is now witnessed on both sides at every site the commands read a path
from (`d4_audit_path_character_device`, `d4_grant_path_character_device` and
the chain `d4_hop_paths_character_device` against `hops_audit_path_is_pipe`,
`chain_hops_path_is_pipe`, `chain_hops_path_character_device` and the pipe
cases of rounds 2 and 3), the `S_ISCHR` disjunct `bcov.py` does not score
among them. At seed 1: 1084 cases, 1441 arms, 155 not both-sided; at seeds 2
to 6, 159, 156, 156, 157, 157; the `kit2-py` missed set is the round-0 set
at every seed, line numbers unchanged since round 3. The revision closed in
this round.

## Other readings: `readings2.py` over `corpus2/`

`bcov.py` scores the branch points the criterion has. A point of the law at
which the criterion does not branch, and a careful reader of the text might,
is outside its sight: no arm exists to be witnessed, so the count is clean
while a corpus tells nothing apart there. Three such points were reported
from outside (kaptonia/zikaron issue 5, by the author of an independent
Haskell implementation of `zikaron/1` written from the text alone, who took
the other reading of each contested point in that implementation's notes in
turn and ran the result over that law's corpus), and `corpus2/` had the same
three:

- **§3.5 test 2.** The criterion tests depth only where a container opens.
  A parser that tests it on every bracket byte answers `E_DEPTH` where a
  bracket the grammar refuses stands inside 128 containers, and the law
  answers `E_JSON` ("a bracket the grammar refuses opens no container"). No
  case stood there at any seed.
- **§8.1.** The criterion collapses the pile by its bytes. A reader that
  collapses it by the spelling of the transport's hexadecimal digits keeps
  two copies of one byte string spelled in two cases. No case spelled one
  byte string twice in different cases at any seed.
- **§8.2.** The criterion moves the seat at a succession by the key in
  office whether or not the walk is certain. A reader that stops moving it
  after a gap records an `AUTHORITY_MISMATCH` the law does not. Only random
  ledgers reached the point: seeds 2 and 4 caught it (4 and 2 cases), seeds
  1, 3, 5 and 6 did not.

`readings2.py` is the measure of that class. Its table holds, for each point
of `docs/zikaron-v2.md` where another reading could be taken, one variant:
the criterion's sources with that one reading swapped in and nothing else,
as an edit whose anchor must occur exactly once, so that a variant a change
of the criterion has overtaken fails loudly and is never silently the
criterion. It generates the corpus of each seed in a temporary directory,
answers every canon, sign and audit case with the criterion and with each
variant through their own command functions, and requires that every
variant differ from the criterion on at least one case at every seed; a case
a variant breaks on counts for nothing. The rows come from the points the
three sets of notes name (the outside implementer's, `impl2-py/NOTES.md`,
and the reading contests of the review record): the parse and the six tests
of §3.5, the order of §4.3's steps taken pairwise and of §5.4's bullets, the
body rows, §9.4's input forms, §7.4's retirement, timeliness and lineage,
the walk and the fork of §8.2 and §8.4, the reconciliation and the label of
§8.5 and §8.6, and §12.6's candidate, cut, segments and combined label, with
the walk's two gap rules again inside the old segment.

Before the cases below, at seeds 1 to 6 (4409 to 4412 canon, sign and
audit cases a seed), 68 of the 81 variants were caught at every seed and
thirteen were not: the three reported points, as seven rows (the depth test
at each of the four places the grammar refuses a bracket, the pile by
spelling, the frozen seat, and the frozen seat in the old segment), and six
more of the same class the table found:

| variant | clause | caught before | the other reading |
|---|---|---|---|
| `depth-at-member-name`, `depth-at-colon`, `depth-after-member`, `depth-after-element` | §3.5 | never | a refused bracket inside 128 containers fails `E_DEPTH` |
| `step-spec-before-envelope-closed` | §4.3 | never | `E_SPEC` is decided before `E_ENVELOPE_CLOSED` |
| `step-prev-before-seq` | §4.3 | never | `E_PREV` is decided before `E_SEQ` |
| `step-prev-seq-before-prev` | §4.3 | never | step 8's relation is decided before its form |
| `step-body-before-prev-seq` | §4.3 | never | `E_BODY` is decided before `E_PREV_SEQ` |
| `sig-range-before-v` | §5.4 | never | `E_SIG_RANGE` is decided before `E_SIG_V` |
| `pile-by-spelling` | §8.1 | never | pile copies collapse by spelling |
| `seat-frozen-after-gap` | §8.2 | seeds 2 and 4 | after a gap a succession no longer moves the seat |
| `old-seat-frozen-after-gap` | §12.6 | never | the same, in the old segment |
| `old-authority-ungraded-after-gap` | §12.6 | never | in the old segment authority is not compared after a gap |

`corpus2/gen.py` now builds, at every seed and from no draw of the seed's
stream (`build_readings`, registered after every other canon and audit case,
whose names and bytes are as they were): thirteen `depth_*_bracket_*` byte
strings, ten where the grammar refuses the bracket (`E_JSON`) and three
where it admits it at a value position after a comma (`E_DEPTH`);
`step3_before_step4`, `step7_before_step8`, `step8_form_before_relation`,
`step8_before_step9`, `sig_v_beats_range` and `sig_v_beats_high_s`; five
`pile_*_hex_cases` inputs (an entry, a malformed byte string, an excluded
entry and the twins of a fork, each spelled again in another case);
`seat_moves_at_the_entry_opening_a_gap`,
`seat_moves_at_a_position_after_a_gap`, `seat_moves_twice_after_a_gap` and
`ceding_key_graded_soft_after_its_gap_succession`; and three continued
ledgers, `continue_old_segment_seat_moves_after_gap`,
`continue_old_entry_in_two_hex_cases` and
`continue_old_segment_authority_graded_after_gap`: 19 canon cases and, with
the reversed twins, 24 audit cases, 4485 cases at seed 1 from 4442. With
them `readings2.py --seeds 1-6` exits 0, every one of the 81 variants caught
at every seed (4452 to 4455 cases a seed), the thirteen by the cases named
for them. The criterion is untouched, so its digest is; `compare2.py`, the
criterion as candidate, reads zero divergences and zero disagreements with
the generator's predictions at seeds 1 to 6; and `bcov.py core2` counts the
same arms and the same arms not both-sided before and after (895 arms and
the same 59 not both-sided at seed 1, over 7973 cases before and 8035
after), which is the point of the class: these cases stand where the
criterion has no branch to take.

Two limits. The table is a list of the points someone has read two ways, and
a point nobody has yet read twice is not in it: a row is added when a new
implementation's notes, a reader or an issue names one, and the corpus grows
only where the new row is not caught. And the measure is of `corpus2/`
alone: `corpus/` (the `zikaron/1` corpus the report was made against) and
the kit corpora are as they were.

## Reading

With seed 1 of both corpora and the sixty-nine fixtures, every branch point
of the three criterion programs that an input can steer is witnessed on both
sides. An implementation that agrees with the criteria on these corpora
takes every branch the criteria's code takes on both sides, and the arms it
may still take differently are the ones listed above, none of which is a
decision of the law; a clause the code holds without a branch of its own is
witnessed by the cases that name it, and the generators' predictors, mutated
clause by clause, are the measure of those, with `readings2.py`'s variants of
the criterion itself for `zikaron/2`. Regenerate with another seed and
rerun `bcov.py` to repeat the measure; the generators' own counts of closed
vocabularies stay at zero unwitnessed.
