# zikaron conformance: the criteria and their corpora

The references of the evidence ledger, and the instruments that test any
implementation against them. Everything here that is not a
criterion is tooling: rebuild, replace, or delete it freely.

## zikaron/1

- `../docs/zikaron-v1.md`: the wire law, the textbook of the core below.
- `impl-py/`: the frozen core, pure Python over the standard library in
  five source files, its release digest `0xbecf…32fc` by
  `./criterion-digest.sh` and named in the law's section 12.5 on
  2026-09-05, after four rounds of ten seats (the review record beside the
  law). A change to any of the five files is a new core and a new digest
  named by the same act. The core frozen earlier at `0x3ea2…142d` was
  retired when the law was recast to the galeed standard.
- `HARNESS.md`: the `zk1` command-line contract every candidate exposes.
- `corpus/gen.py`: the corpus generator with its own signing and
  canonicalization path (`python3 gen.py --seed N`; a manifest with the
  generator's own predictions as a third opinion).
- `compare.py`: runs a candidate (`ZK1_CANDIDATE=<path to zk1>`) and the
  criterion over the corpus and prints every divergence and every
  disagreement with the generator's prediction.
- `bcov.py`: branch-arm witnessing of a criterion over its corpus (a
  measure, never a criterion); `BOUNDARY-AUDIT.md` reads its result.

## zikaron.kit/1

- `../docs/zikaron-kit-v1.md`: the client's document and reading law.
- `kit-py/`: the frozen kit core, a pure-Python implementation over the
  frozen `impl-py/`, written from the text alone; its release digest
  `0x3f83…0535` by `./criterion-digest-kit.sh`, named in the kit law's
  section 13.5 on 2026-09-05 after three rounds of eight seats under the
  frozen parent (the kit review record beside the law). A change to any of
  its four files is a new core and a new digest named by the same act.
- `HARNESS-KIT.md`: the `zkk` contract.
- `kit-corpus/gen.py`: the kit corpus generator (about a thousand cases per
  seed, every closed vocabulary witnessed on both sides).
- `compare-kit.py`: runs a candidate (`ZKK_CANDIDATE=<path to zkk>`) and
  `kit-py` over the kit corpus.
- `scan-py/`: an independent scanner of the chain-facing layer written
  from section 9 of the wire law; with `../zikaron-core/fixtures/` it is
  the reference for anchoring and scanning.

## zikaron/2

- `../docs/zikaron-v2.md`: the wire law's second version, a retirement and a
  timeliness rule in §7.4, the `migration` and `retraction` entry types, one
  anchor form, three audit inputs, and the continuation of a `zikaron/1`
  ledger (§12.6), whose old segment the frozen `zikaron/1` core reads.
- `impl2-py/`: the reference implementation of `zikaron/2`, pure Python over
  the standard library in three source files (`zk2.py`, `zk2entry.py`,
  `zk2audit.py`) over the `zikaron/1` core of release digest `0xbecf…32fc`
  (`impl-py/`: four of its five files run, `zkcanon.py`, `zkcrypto.py` and
  `zkentry.py` as libraries and `zkaudit.py` as a black box for a continued
  ledger's old segment, and the digest covers all five); its release digest
  is computed by `./criterion-digest2.sh` over the three files and that
  core's digest, which refuses any other embedded core, and named by the
  law's §12.5.
- `HARNESS2.md`: the `zk2` command-line contract.
- `corpus2/gen.py`: the corpus generator (`python3 gen.py --seed N`), with
  the `zikaron/1` generator imported as a tool for the continued segment's
  prediction. Its predictions are the corpus author's reading of the law,
  written to the same clauses as the criterion: agreement between the two is
  one reading measured twice, and the independent evidence is the three
  implementations of the review rounds. The manifest publishes
  `unwitnessed`, the residue of the closed vocabularies no case witnesses,
  and the misuse cases of HARNESS2.md (exit 2, empty stdout).
- `compare2.py`: runs a candidate (`ZK2_CANDIDATE`) and the criterion over
  the corpus2 manifest, strictly typed, the manifest's counts and files
  checked, the coverage and `unwitnessed` recomputed from the criterion's
  answers; exits 1 on any divergence or disagreement; `ZK2_OUT` names the
  divergence file and `ZK2_EXPECT` pins the digest over every corpus byte
  and case it read, exit 1 on any other.
- `bcov.py core2`: branch-arm witnessing of the criterion over corpus2.
- `readings2.py`: the other-reading variants of the criterion over corpus2:
  for every point of the law where another reading could be taken, one
  variant of the criterion with that one reading swapped in, each of which
  the corpus of every seed must tell from the criterion; exits 1 on any
  variant a seed does not catch (a measure, never a criterion;
  `BOUNDARY-AUDIT.md` reads its result).

## zikaron.kit/2

- `../docs/zikaron-kit-v2.md`: the client law's second version: badges,
  record kits in a directory or in one file, grant packages, and the grant
  check over a continued ledger.
- `kit2-py/`: the reference implementation of `zikaron.kit/2`, four source
  files (`zkk2.py`, `zkk2doc.py`, `zkk2kit.py`, `zkk2read.py`) over
  `impl2-py/`; its release digest is computed by
  `./criterion-digest-kit2.sh`, which refuses unless §10.4 names the
  `zikaron/2` core it embeds, and named by the kit law's §10.5.
- `HARNESS-KIT2.md`: the `zkk2` contract.
- `kit-corpus2/gen.py`: the kit corpus generator; its predictions are the
  author's reading as `corpus2/gen.py`'s are.
- `compare-kit2.py`: runs a candidate (`ZKK2_CANDIDATE`) and `kit2-py` over
  the kit corpus, strictly typed, the manifest's counts and files checked;
  the coverage and `unwitnessed` recomputed from the criterion's answers;
  exits 1 on any divergence or disagreement; `ZKK2_OUT` names the divergence
  file and `ZKK2_EXPECT` pins the digest over every corpus byte and case it
  read, exit 1 on any other.
- `bcov.py kit2`: branch-arm witnessing of the kit criterion.

## zikaron.face/1

- `../docs/zikaron-face-v1.md`: the law of an identity's faces: face
  entries, a key's consent, the marker, an entry's bytes carried with its
  anchor, the face table read from the succession books and the registries
  of the chains a reader declares, and the joining of two tables where a
  seat moves to another root.
- `face-py/`: the reference implementation of `zikaron.face/1`, three source
  files (`zkf.py`, `zkfentry.py`, `zkftable.py`) over `impl2-py/`;
  `./criterion-digest-face.sh` computes its release digest and refuses
  unless the law names, as a backticked digest, the `zikaron/2` core it
  embeds; the law's §10.5 names the digest at the freeze.
- `HARNESS-FACE.md`: the `zkf` contract.
- `face-corpus/gen.py`: the face corpus generator; its predictions are the
  author's reading as `corpus2/gen.py`'s are, each the exit code and the
  exact bytes of an answer.
- `compare-face.py`: runs a candidate (`ZKF_CANDIDATE`) and `face-py` over
  the face corpus, byte for byte, the manifest's counts checked, every file
  of the corpus tree a file case's and no link or special file among them,
  every misuse case and no other predicting exit 2, the coverage and
  `unwitnessed` recomputed from the criterion's answers, and the boundary
  list equal to the comparator's own with every named case passing its
  side's test;
  exits 1 on any divergence or disagreement; `ZKF_OUT` names the divergence
  file and `ZKF_EXPECT` pins the digest over every corpus byte and case it
  read, exit 1 on any other.
- `bcov.py face`: branch-arm witnessing of the face criterion.
- `face-table-unicode.js`: reads the barred table of the law's §1 from its
  text and sets it beside the Unicode data of the `node` that runs it: the
  classes the law's sentence names, and the values barred beside them.

## History

Production Rust implementations of both laws, the ledger directory, and the
anchoring layer were built and converged here (last present before this
repository's history was rebuilt on 2026-09-16); they served to prove the
laws implementable and to find the corpus's blind spots, and were retired
from the tree so that the implementations the Desk anchors are built from
the criteria alone.