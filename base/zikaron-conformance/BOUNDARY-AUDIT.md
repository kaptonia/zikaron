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

`bcov.py core|kit|scan <out.json>` runs one criterion program in-process over
its corpus with a tracer that records every executed line-to-line arc inside
the criterion's own files. Every `if` and `elif`, every loop, and every
exception handler in those files is a branch point; a branch point whose arms
were both taken by some case is witnessed on both sides; the rest are listed
with their source line. The measure is a tool in the sense of the first
principle of this repository: it never enters a digest, the criterion is not
modified for it, and a reading it gives is only as good as the reading of the
law that follows it.

The generators of both corpora already count the closed vocabularies the laws
name (every token, verdict, label and finding name) and report each one
witnessed in the output of the criterion; both report none unwitnessed. Branch
arms are the finer measure: they see a boundary the vocabulary cannot, such as
the positive side of a rule whose negative side already has a token.

## Result, seed 1

| program | corpus | cases before → after | branch arms | arms not both-sided, before → after |
|---|---|---:|---:|---:|
| `impl-py` (zikaron/1 criterion) | `corpus/` | 1797 → 7254 → 7589 → 7647 | 591 → 653 | 48 → 35 → 35 → 35 |
| `kit-py` (zikaron.kit/1 candidate, embedding `impl-py`) | `kit-corpus/` | 913 → 978 → 992 | 1059 → 1127 | 149 → 128 → 140 |
| `scan-py` (§9 scanner) | `zikaron-core/fixtures/` | 21 → 53 → 69 | 289 → 378 | 101 → 67 → 79 |

Every arm that remains is classified below; none of them decides a label, a
token, a verdict, or a record. The third figure in the `impl-py` row is the
recast of 2026-09-05, when the law and its criterion were rebuilt to the
galeed standard: the arms the recast added (two windows per chain, the reach
rule, the `DISCARDED` list, the audit reader's five restrictions, the
`sign` command's argument rules) are all witnessed, and the thirty-five that
remain are the classes below plus the `sign` misuse exits. The third figure
in the `scan-py` row is the same recast: the scanner now recovers every
sender from the transaction's signature (§9.1), tests the ordering and
uniqueness rules of §9.4, keys records on `(chainId, blockNumber, tx, hash)`,
and refuses creations and signature-less transactions in both forms; the
sixteen fixtures added (54 to 69) witness those arms, and the seventy-nine
that remain are the environment and unreachable classes below, grown by the
signature reader's own arms against a lying node.

**Seeds.** The corpora mix cases built by construction with seeded
mutations, so an arm can be witnessed by luck under one seed and missed under
another. Seeds 2, 3 and 4 were generated beside seed 1 and measured the same
way. The first pass found four arms of `impl-py` that flickered between seeds
(a junk byte after an array element, a text ending where a value should
begin, an escape letter outside the eight, and the order of the two ids an
`EQUIVOCATION` on a shared `prev` names). Patching each with a hand-written
case would have been a wall: the next seed finds the next arm. The two
classes were closed instead (next section), after which the set of
unwitnessed `impl-py` arms is identical under seeds 1 to 4 and no `kit-py`
arm is witnessed under one seed and missed under another. Re-measured after
the recast's third round (7647 cases, 653 arms): seeds 1 to 4 each leave the
same thirty-five arms, byte-identical lists, and every seed's predictions
agree with the criterion. The kit corpus was re-measured after the kit's second
round under the frozen parent (992 cases, 1127 arms): the `kit-py` arms left
unwitnessed are identical under seeds 1 to 4, and the `impl-py` arms the kit
corpus happens to reach still vary by seed (140 to 143 in all), every one of
them witnessed by the core corpus under every seed. The `impl-py`
arms the kit corpus happens to reach still vary by seed, and every one of
them is witnessed by the core corpus under every seed.

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
  `anchorMany` with one hash twice (47); **§9.5 an element named twice** yields one evidence
  record (48); **a chains object with no registries** reads no log and asks
  for none (49); **a transaction the chain does not carry** in `bareTx` and
  among the elements (50, 51); **a pending transaction** in both places
  (52, 53).
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
harness contracts fix exit status 2 and no message for these, so a candidate's
wording there is its own and the comparators cannot hold such a case. Nine
arms in `zk1.py`, thirty in `zkk.py`, three in `scan_replay.py`.

**Arms the law makes unreachable.** `attestation_ok` with `prev` null (§6.6:
an adoption is never at `seq` 0, and §6.5 rejects one, so no ledger entry
reaches this test); the fork walk's `a.eid == b.eid` (§8.1 collapses
byte-identical copies before the walk); `_emit`'s fall-through (a value
outside the universe cannot be parsed in); `int_literal` of an empty run (the
parser only calls it with a digit); the recovery test `r >= P` (`r < N < P`
after the range test); the Jacobian point-at-infinity arms and the RFC 6979
retry (a key in range with a digest never produces `k = 0` or `r = 0`); the
depth reading's cache hit (ledger entries are distinct); in the scanner's
recovery, `r` or `s` out of range, an `r` that is no curve abscissa, and a
recovered point at infinity (a transaction a chain included carries a
signature a node validated), and the Jacobian infinity arms again.

**Environment, in `scan_replay.py`.** A node answering outside the JSON-RPC
forms: a transaction, receipt, log or header that is not an object, a null
`eth_getLogs`, a header without `timestamp`, a null `eth_getCode`, an access
or authorization list entry that is not an object, a typed transaction whose
parity is neither 0 nor 1, and a transaction object whose signed bytes do not
hash to the hash it was asked for by (the node is answering about other
bytes). The
scanner raises a loud error and produces no fragment; §9.3 names the one
case the law decides (state that cannot be consulted is `UNPROVEN`, fixture
12), and the rest is the scanner's own conduct toward a broken node.

**Filter-implied, in `scan_replay.py`.** A log whose address is not declared,
whose topic 0 is another event's, or whose block lies outside the range:
`eth_getLogs` is asked with the address list, the topic 0 and the range, so a
conforming node never returns such a log, and a transaction with status 0
emits no log that any node returns. These arms guard against a lying node;
the fixtures 05, 07, 08 and 20 witness the same sentences of §9.1 on the
side a node can produce.

**Tracing artefacts.** Module-level loops that ran before the tracer was
installed, `__main__` guards, `__repr__`, and helpers no command reaches
(`presig_of`, `jneg`).

## Reading

With seed 1 of both corpora and the sixty-nine fixtures, every branch point
of the three criterion programs that an input can steer is witnessed on both
sides. An implementation that agrees with the criteria on these corpora
agrees with them at every boundary the laws close, and the arms it may still
take differently are the ones listed above, none of which is a decision of
the law. Regenerate with another seed and rerun `bcov.py` to repeat the
measure; the generators' own counts of closed vocabularies stay at zero
unwitnessed.
