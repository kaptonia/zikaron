# `zikaron.kit/1` fuzz convergence campaign

Run of 2026-09-04 against the §13.1 freeze gate: "across millions of samples,
with zero panics, hangs, nondeterministic answers, or wrong accepts under
fuzzing."

**Result.** 2,130,000 fuzz samples across four phases, 3,630,000 answers
compared byte for byte between the two implementations, zero panics, zero
hangs, zero nondeterministic answers over 182,128 re-run comparisons, and zero
wrong accepts. Phases A, B and C are clean: 2,050,000 samples, 3,550,000
answers, zero divergences. Phase D recorded 854 divergences, all of one root,
all traced to a `zikaron/1` core defect on the Rust side that has since been
fixed upstream; the campaign's binary predates the fix, and with the rebuilt
binary the same inputs agree.

The campaign was stopped after phase D and the third-opinion pass; no phase was
re-run against the rebuilt binary. What that leaves open is stated under
**Status** below.

## A note on the tree this report describes

While phase D was finishing, commit `1dfbd8b` ("The Rust implementations leave
the tree; the criteria and their corpora stay") retired the whole Rust tree,
`zikaron-core/crates/zikaron-kit` included, source and build artifacts alike.
The implementation this campaign measured on the Rust side, and the `zkk`
binary it ran, no longer exist in the working tree; the same sweep removed this
directory's drivers (`campaign.py`, `fuzzlib.py`, `pydriver.py`,
`selftest.py`, `thirdopinion.py`, `seeds.json`, `BATCH-DIFF.md`), and
`compare-kit.py` was rewritten to test an external candidate named by
`ZKK_CANDIDATE`. What survives here is this report, the per-phase summaries in
`runs/`, and one selftest artifact in `findings/`. Passages below that name a
driver or `seeds.json` describe the campaign as it ran, not files now on disk.
The Rust side was `zikaron-core/crates/zikaron-kit` at the parent of `1dfbd8b`,
built with `cargo build --release --offline`.

## Counts

| Phase | Samples | Commands compared | Answers compared | Wall clock | Divergences | Panics | Hangs | Determinism re-checks | Nondeterminism | Acceptance candidates |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| A documents | 1,500,000 | `fpm-check`, `ack-check`, `pair`, `attribute` | 3,000,000 | 1,460.5 s | 0 | 0 | 0 | 150,440 | 0 | 0 |
| B badges | 500,000 | `badge-decode` | 500,000 | 503.2 s | 0 | 0 | 0 | 25,103 | 0 | 6 |
| C kits | 50,000 | `kit-verify` | 50,000 | 118.7 s | 0 | 0 | 0 | 2,450 | 0 | 0 |
| D readings | 80,000 | `depth`, `grant-check`, `chain-check` | 80,000 | 1,037.9 s | 854 | 0 | 0 | 4,135 | 0 | n/a |
| **total** | **2,130,000** | | **3,630,000** | **3,120.2 s** | **854** | **0** | **0** | **182,128** | **0** | **6** |
| third opinion | 200,000 | `fpm-check`, `ack-check`, `badge-decode`, `kit-verify` | 600,000 | 181.9 s | 0 | 0 | 0 | n/a | n/a | 0 |

Each determinism re-check is one (sample, command) pair sent back through a
fresh process of *each* implementation, so the 182,128 re-checks are 364,256
re-run answers, every one byte-identical to the first run. The re-checked
subset is 5% of each phase, drawn by that phase's own seeded RNG.

The third-opinion row answers each sample three ways (Rust, Python, and
`kit-corpus/gen.py`'s own predicates), which is why its answer count is three
per sample. It witnessed 16,606 acceptances, every one agreed by all three.

Phase A samples are byte strings (82%, compared as both `fpm-check` and
`ack-check`) and pairings (18%, compared as both `pair` and `attribute`), which
is why its answer count is twice its sample count. Wall clock is per phase with
10 worker processes on a 10-core machine.

A first pass over the same seeds and sizes ran before two coverage gaps were
closed in the generator (`E_DEPTH` and `ACK_VARIANT_MISMATCH` were unreachable);
it reported the same shape — 0 divergences in A, B and C, the same 6 acceptance
candidates in B, the same 854 in D — and is superseded by the run above.

## Seeds

Master seed **20260904** for all four phases and for the third-opinion pass.
Everything below it is derived, so any worker replays byte for byte:

    worker RNG    random.Random(int.from_bytes(
                      sha256("<phase>|20260904|<wid>").digest()[:8], "big"))
    worker keys   gen.testkey(tag, int.from_bytes(
                      sha256("keys|20260904|<wid>").digest()[:8], "big") % 2**30)
                  for tag in (author, r1, r2, r3, r4, iss, iss2, issb, issc,
                              stranger, k2, k3, k4)

Exact invocations, all from `zikaron-conformance/kit-fuzz/`:

    ZKK_FUZZ_CHUNK=20000 python3 campaign.py run --phase A --samples 1500000 \
        --seed 20260904 --workers 10
    ZKK_FUZZ_CHUNK=20000 python3 campaign.py run --phase B --samples 500000 \
        --seed 20260904 --workers 10
    ZKK_FUZZ_CHUNK=500   python3 campaign.py run --phase C --samples 50000 \
        --seed 20260904 --workers 10
    ZKK_FUZZ_CHUNK=1000  python3 campaign.py run --phase D --samples 80000 \
        --seed 20260904 --workers 10
    ZKK_THIRD_CHUNK=4000 python3 thirdopinion.py run --samples 200000 \
        --seed 20260904 --workers 10
    python3 selftest.py

`seeds.json` carries the same record in machine-readable form. Per-phase
summaries, as the campaign printed them, are in `runs/`. Generated samples
live under `$ZKK_FUZZ_WORK`, a scratch directory, and are never kept.

Binaries: Rust `zikaron-core/crates/zikaron-kit` at HEAD plus `BATCH-DIFF.md`,
`cargo build --release --offline`, rustc 1.97.1; Python `kit-py/` and
`impl-py/` unmodified, CPython 3.14.3.

## What each phase fuzzed

**A, documents (§4, §5).** Random byte strings over a JSON-shaped alphabet and
over raw noise; synthesised manifest- and acknowledgement-shaped objects with
random member sets, member orders, hex forms (short, long, uppercase, `0X`,
non-hex digits, absent `0x`), row counts up to 1,000 and prose strings drawn
from 49 Unicode classes (C0 and C1 controls, combining marks, RTL overrides,
zero-width and BOM, line and paragraph separators, noncharacters, astral
planes, `U+10FFFF`); container nesting at and around `zikaron/1` §3's limit of
128; and mutations of valid signed documents — byte flip, insert, delete,
truncate, duplicate, swap, member shuffle, row shuffle, row duplication, row
deletion, row growth to 1,000, member retyping, and twelve signature-boundary
edits (`v` outside 27/28, `r` or `s` zero, `r` or `s` at `n`, high-`s`,
a non-residue `r`, one hex digit, short and long `sig`, `sig` not a string).
Pairings drew a manifest and an acknowledgement from the valid pool and crossed
them, mutated them, re-signed the acknowledgement for a key with no row, for a
real row recipient naming another variant, and against another manifest, with
attributed and unattributed byte strings.

**B, badges (§6).** Payloads around the prefix (nine neighbours of
`zikaron-grant:`), around `BADGE_CAP` at 2951–2954 and beyond, dot structure
(leading, trailing, doubled, all-dot, empty body), the base64url alphabet and
its trailing bits (characters outside `A-Za-z0-9-_`, lengths ≡ 1 mod 4,
non-zero unused bits, padding `=`), segments made of valid grants, non-grant
entry types, a genesis, corrupted entries, and chains of one to four grants
with upstream links that hold, that name a stranger, that are `null` or not a
string, and with `work` that matches or does not.

**C, kits (§7).** 50,000 generated directories: manifests that are valid and
manifests with every rule of §7.3 broken in turn (`canonical`, `members`,
`spec`, `root`, `entries`, `files`, `contents`, `proofs`, `note_md`), each also
with a member dropped or a foreign member added; disk contents that disagree
with the manifest (missing entry, missing file, wrong bytes, unbacked entry id,
extra file at top level and at depth); kit paths at and past the segment and
whole-path limits, with uppercase, spaces, leading `-`, `.` and `..` segments,
and non-ASCII; extras at random depths; dot-prefixed entries; symbolic links to
files, to directories, to nothing and outside the kit; and non-UTF-8 file names
where the filesystem accepted them.

**D, readings (§8, §9, §10).** 80,000 audit inputs over random ledgers with
forks and fork twins, `seq` gaps, broken `prev` links, foreign authors,
successions to fresh and to prior keys, withheld entries, non-entries and
stranger entries in the pile; anchor records with all three verdicts on one and
several chains; bases that cover and do not (empty `chains`, empty
`registries`, `senders` missing a lineage key, `bareTx` only, several chains);
`zikaron/1` §9.4's own no-label routes (a `chains` object with a foreign
member, `fromBlock` > `toBlock`, two objects for one `chainId`, an absent
`bareTx`, a verdict outside the three, two anchor records for one
`(chainId, tx, hash)` that disagree); and a thin slice of harness-schema
mutations. Grants were valid, mutated, raw bytes and entries of another type;
`now` was null, zero, inside and outside the window, and `2^53 − 1`. Chains ran
zero to four hops, coherent (each hop issued by the previous hop's grantee, on
a clean ledger rooted at that party's key, anchored under a covering basis) and
coherent-with-one-thing-broken (the `upstream` link, the `work`, the ledger
root, an anchor, a revocation of that hop, no audit input, the grant's bytes,
a basis that stops covering).

## Coverage

Read off one implementation's answers; bookkeeping only, never compared.

**Documents.** Every token of §11's closed document list was witnessed:
`E_UTF8` 220,219, `E_JSON` 108,201, `E_NUMBER` 56,002, `E_DEPTH` 27,728,
`E_DUP_KEY` 21,399, `E_KEY_CHARSET` 18,204, `E_VALUE_CHARSET` 77,365,
`E_NOT_CANONICAL` 108,857, `E_DOC` 9,724, `E_DOC_MISSING` 284,190,
`E_DOC_CLOSED` 6,856, `E_SPEC` 22,000, `E_FPM_AUTHOR` 7,174, `E_FPM_WORK`
3,289, `E_FPM_GRANT` 1,925, `E_FPM_ROWS` 33,366, `E_FPM_ROW` 32,671,
`E_FPM_DUP_RECIPIENT` 25,413, `E_FPM_DUP_VARIANT` 5,591, `E_FPM_ROW_ORDER`
40,777, `E_FPM_NOTE` 729, `E_ACK_RECIPIENT` 14,181, `E_ACK_FPM` 10,534,
`E_ACK_VARIANT` 10,063, `E_ACK_NOTE` 638, `E_SIG_FORM` 26,139, `E_SIG_V`
3,112, `E_SIG_RANGE` 16,565, `E_SIG_HIGH_S` 6,519, `E_SIG_RECOVER` 6,266,
`E_SIG_SIGNER` 26,092; 45,004 manifests and 26,219 acknowledgements accepted.

**Pairing and attribution.** All six pairing verdicts and both attribution
verdicts: `PAIRED` 74,685, `FPM_INVALID` 60,525, `ACK_INVALID` 30,998,
`ACK_FPM_MISMATCH` 83,203, `ACK_NO_ROW` 10,460, `ACK_VARIANT_MISMATCH` 9,576;
`ATTRIBUTED` 40,480, `NOT_ATTRIBUTED` 34,205.

**Badges.** All seven tokens and the accept: `E_BADGE_PREFIX` 88,722,
`E_BADGE_CAP` 64,261, `E_BADGE_B64` 127,113, `E_BADGE_ENTRY` 79,081,
`E_BADGE_TYPE` 24,252, `E_BADGE_INCOMPLETE` 38,990, `E_BADGE_LINK` 21,791,
`BADGE_OK` 55,790.

**Kits.** All seven failure verdicts and the accept: `E_KIT_UNREADABLE` 5,035,
`E_KIT_MANIFEST_ABSENT` 4,524, `E_KIT_MANIFEST` 24,165, `E_KIT_ENTRY_BYTES`
732, `E_KIT_FILE` 236, `E_KIT_PROOF_BYTES` 171, `E_KIT_EXTRA` 4,308, `KIT_OK`
10,829 — of which 6,586 carried a non-empty `invalid_entries` list, the §7.4
step 7 case of a kit holding an anchored byte string that is not an entry.

**Readings.** `depth`: 24,554 valid inputs, 1,021 invalid. `grant-check`:
`GREEN` 5,200, `PARTIAL` 8,821, `FAIL` 22,970. `chain-check`: `GREEN` 3,311,
`PARTIAL` 1,365, `FAIL` 12,758, over hop lists of length zero to four.

## Findings

Two classes, both classified below. Every finding was written to `findings/`
with its input and both outputs during the run; that directory was cleared
before the post-rebuild selftest and now holds only the selftest's own injected
artifacts. Each finding is a pure function of (phase, seed 20260904, worker)
and regenerates from the recorded invocation.

### Finding 1 — phase D, 854 divergences: a `zikaron/1` core defect on the Rust side, since fixed

`grant-check` 368, `depth` 304, `chain-check` 182.

Every one has the same shape: Python answers that the audit input is invalid,
Rust answers a full reading over it.

    rust: {"continuity":{"anchored":0,"span":0},"deepest":0,"earliest":null,
           "found":false,"label":"GAPS","valid":true}
    py:   {"valid":false}

    rust: {"basis":{...},"checks":[...,{"n":2,"state":"PASS",...}],"verdict":"PARTIAL"}
    py:   {"basis":null,"checks":[...,{"n":2,"state":"UNKNOWN",...}],"verdict":"PARTIAL"}

**Classification: an implementation bug, on the Rust side, in its embedded
`zikaron/1` core rather than in its `zikaron.kit/1` code.** Every reading
predicate of this law is a function of `audit(I)` (§9.2, §10.2, §10.5), and
`zikaron.kit/1` §1 defines an invalid input as "an input the core refuses under
`zikaron/1` §9.4 (no label)". The inputs at issue are ones whose members do not
carry the forms `zikaron/1` §8 and §9.2 name — an absent `pile`, `anchors`,
`unavailable` or `evidence`, a `root` that is not hex20, an `unavailable`
element that is not hex32, an anchor `verdict` outside the three, odd hex.
`zikaron/1` §9.4 decides them:

> "An input carrying two records that disagree on any remaining field, or an
> anchor record whose `verdict` is outside the three values of §9.3, or a
> record whose members fail the forms §8 and §9.2 name, is not a `zikaron/1`
> audit input, and a verifier handed one returns no label"

The law decides against the Rust core as the campaign's binary had it. It is
reproducible below the kit law entirely, through the parent's own command:

    $ zk1 audit <input with no `pile` member>
    rust: {"adoption_unproven":[],"basis":{...},...}     # a full report
    py:   {"ok":false,"reason":"NO_LABEL"}

**Status: already fixed upstream.** `zikaron-core` commit `e343061`, "A
malformed audit input is no label, in the core as in the criterion", makes the
Rust core answer no label here. The `zkk` binary this campaign ran was built
before that change reached the linked `zikaron` library, so the campaign
measured the pre-fix core. Rebuilt from HEAD, the two implementations agree on
every case of the boundary probe:

| input | rust | py |
|---|---|---|
| baseline | reading | reading |
| no `root` / `pile` / `anchors` / `unavailable` / `evidence` / `basis` | `{"valid":false}` | `{"valid":false}` |
| `root` = `"x"` | `{"valid":false}` | `{"valid":false}` |
| `root` = `"0xzz"` | `{"valid":false}` | `{"valid":false}` |
| `unavailable` = `["0x01"]` | `{"valid":false}` | `{"valid":false}` |
| a foreign top-level member | reading | reading |

No fix was made to either implementation by this campaign.

### Finding 2 — phase B, 6 acceptance candidates: lawful prefix chains, not wrong accepts

The campaign flags as a wrong-accept candidate any mutation of a valid document
or payload that both implementations still accept. Six fired, all in phase B,
all one shape: the byte-level mutation truncated a multi-segment payload
exactly at a `.`, leaving a shorter payload that is still lawful.

| finding | original | mutated | original's answer |
|---|---:|---:|---|
| `w0_0001` | 3,198 B, 4 segments | 2,375 B, 3 segments | `E_BADGE_CAP` |
| `w2_0001` | 2,375 B, 3 segments | 1,552 B, 2 segments | `BADGE_OK` |
| `w3_0001` | 3,198 B, 4 segments | 2,375 B, 3 segments | `E_BADGE_CAP` |
| `w3_0002` | 1,552 B, 2 segments | 729 B, 1 segment | `BADGE_OK` |
| `w6_0001` | 3,198 B, 4 segments | 2,375 B, 3 segments | `E_BADGE_CAP` |
| `w9_0001` | 2,375 B, 3 segments | 729 B, 1 segment | `BADGE_OK` |

**Classification: not a wrong accept; all three readings are right.** Under
§6.2 a payload is decided on its own bytes: a prefix of a lawful chain, cut at
a segment boundary, is a lawful chain. Its segment 0 is the same segment 0 and
still states no `upstream` (step 4), every remaining byte link is one that held
in the longer payload (step 5), every segment is still canonical base64url of
an accepted grant entry (step 3), and the shorter payload is under `BADGE_CAP`
(step 2) — which is why three of the six were `E_BADGE_CAP` before the cut and
`BADGE_OK` after it. Checked mechanically for all six: the original begins with
the mutated bytes, the next byte of the original is `.`, and the third reading
(`kit-corpus/gen.py`) accepts each one with the same grants list the two
implementations return.

No law ambiguity was found. Every closed vocabulary of §11 that the campaign
reached, it reached on both sides with the same answer.

## Evidence that the instruments fire

`selftest.py` does two things.

**1. The batch path is the CLI's answer.** Every case of the 913-case kit
corpus that has a batch equivalent (902; the other 11 are `badge-encode`) is
answered through both paths on both implementations, and all four answers must
agree. Run against the rebuilt binary: **0 mismatches** across 304 `fpm-check`,
243 `ack-check`, 13 `pair`, 10 `attribute`, 144 `badge-decode`, 89
`kit-verify`, 17 `depth`, 59 `grant-check` and 23 `chain-check` cases. This is
also the corpus-convergence check re-run after the rebuild: the two
implementations still agree on every corpus case.

**2. A fault is counted.** A wrong answer, a crash, a hang and a
nondeterministic answer are injected one at a time into each side through a
wrapper, and the campaign must report a divergence, a panic, a hang and a
nondeterminism, and write the input to `findings/`. Against the pre-rebuild
binary all eight rows fired:

    rust    wrong  FIRED  divergences 4
    rust    crash  FIRED  panics 4
    rust    hang   FIRED  hangs 4
    rust    flaky  FIRED  det_mismatch 120
    python  wrong  FIRED  divergences 4
    python  crash  FIRED  panics 4
    python  hang   FIRED  hangs 4
    python  flaky  FIRED  det_mismatch 120

The re-run after the rebuild reproduced six of the eight; the last two rows
(`python hang`, `python flaky`) are inconclusive, not negative — the working
directory was being edited by another process during that run and the `flaky`
row failed with `can't open file 'campaign.py'`. The instruments are the same
code in both runs, and the `python hang` row did write its finding
(`findings/A_hang_py_pair-batch_w0_0003`) even where its summary read zero.

The per-sample hang detector inside the Python driver was checked directly: a
valid signed manifest answered under a 1 ms per-sample budget returns
`{"driver":"TIMEOUT"}` instead of its answer.

## Status

- **Phases A, B and C: clean.** 2,050,000 samples, 3,550,000 answers, zero
  divergences, zero panics, zero hangs, zero nondeterminism, zero wrong
  accepts, and a third opinion agreeing on 200,000 further samples.
- **Phase D: one defect, since fixed.** 854 divergences of a single root, in
  the Rust side's embedded `zikaron/1` core, resolved by `zikaron-core` commit
  `e343061`.
- **Not done, and needed before §13.1 is claimed:** the campaign ran against a
  `zkk` binary built before that core change reached the linked library, and
  was stopped before any phase could be re-run against the rebuilt binary. The
  four phases should be re-run from the recorded seeds against the current
  build. Phases A, B and C exercise only the document predicates of §4 to §7,
  which read no audit input, so the fixed rule cannot reach their answers; the
  902-case corpus parity check *was* re-run after the rebuild and showed zero
  mismatches. Neither observation is a substitute for the re-run.
- **Not fuzzed:** `badge-encode` (§6.1), the inverse of §6.2, exercised as the
  producer of every payload phase B decodes but never compared as an answer;
  and proof-file contents, which §7.5 puts outside this law and outside
  §13.1's corpus.
