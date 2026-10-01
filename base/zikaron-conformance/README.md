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
- `kit-corpus/gen.py`: the kit corpus generator (about a thousand cases per seed,
  every closed vocabulary witnessed on both sides).
- `compare-kit.py`: runs a candidate (`ZKK_CANDIDATE=<path to zkk>`) and
  `kit-py` over the kit corpus.
- `scan-py/`: an independent scanner of the chain-facing layer written
  from section 9 of the wire law; with `../zikaron-core/fixtures/` it is
  the reference for anchoring and scanning.

## History

Production Rust implementations of both laws, the ledger directory, and
the anchoring layer were built and converged here (last present at
commit 1ea0f11); they served to prove the laws implementable and to find
the corpus's blind spots, and were retired from the tree so that the
implementations the Desk anchors are built from the criteria alone.
