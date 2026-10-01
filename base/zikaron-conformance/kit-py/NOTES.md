# zkk (kit-py): the readings this implementation chose

A pure Python 3 implementation of `zikaron.kit/1` (docs/zikaron-kit-v1.md),
written from that text and `docs/zikaron-v1.md` alone, exposing the command
line of `zikaron-conformance/HARNESS-KIT.md`, and embedding the frozen
`zikaron/1` core of `zikaron-conformance/impl-py` (release digest
`0x3ea2ff36…d2142d`, the digest §13.4 names) for every parent predicate:
canonical acceptance, entry acceptance, signing, and the audit with its
report and its ledger. No parent predicate is re-derived here.

    zkk.py        the command line
    zkkdoc.py     §2–§6: signing, manifests, acknowledgements, pairing,
                  attribution, badge encode and decode
    zkkkit.py     §7: the walk, kit paths, the manifest, verification
    zkkread.py    §8–§10: reachability and bounds, depth, the grant check,
                  the ledger link, the chain check
    selftest.py   a live end-to-end run of all eleven commands

`HARNESS-KIT.md` fixes the members of every output and disclaims defining the
law; the law fixes every value. Where the two are read together the harness
governs the shape, and this implementation follows it exactly. Below is every
place the law's own text admits more than one reading, with the readings, the
choice, and why.

## 1. Kit manifest row objects are closed

**Section.** §7.3. **Readings.** (i) A row of `files`, `contents`, or `proofs`
is an object with **exactly** the listed members. (ii) A row may carry members
beyond them, the table saying "elements `{path, sha256, size}`" without the
word "exactly" that §7.3's own root sentence, §4.1's rows, and `zikaron/1`
§9.4 all use.

**Choice.** (i), closed. **Why.** `zikaron.kit/1` opens no object to extra
members anywhere in its text. The parent grants that allowance once, in §6.10,
and confines it to entry bodies; §9.4 states that §6.10 reaches no object of
that section. Reading a silent table as an implicit opening would make this
law the only place in either text where arity closure is waived without being
stated. The brace notation itself reads as an exact member list.

## 2. `Hmax`'s tie-break

**Section.** §9.2. **Readings.** The element with the largest `seq`,
"(likewise)", is disambiguated by (i) the bytewise-**smallest** `entry_id`
among equals, the rule `H0` states verbatim, or (ii) the bytewise-largest, a
mirror of `H0`'s rule.

**Choice.** (i). **Why.** "Likewise" points at the parenthetical it repeats,
which reads "bytewise-smallest `entry_id` among equals". A mirrored rule would
have to be written out. The choice reaches only `span`, and only when two
history entries of one work share the largest `seq`.

## 3. The subject of `E_KIT_FILE` and `E_KIT_PROOF_BYTES`

**Section.** §7.4 steps 4 and 5. **Readings.** "the path" is (i) the row's own
`path` member, or (ii) the enumeration path `files/<path>` the row names.

**Choice.** (i), the row's `path`. **Why.** §7.2 draws the distinction in the
other direction: "A path in `files` names `files/<path>`", so the row's member
is the path and `files/<path>` is what it names. Step 6's subject is an
enumeration path, because a stray pair has no row to name it.

## 4. `badge-encode`'s order of tests

**Section.** §6.1; the harness names the three tokens and gives an index to
the two per-segment ones. **Choice.** The inputs are examined in the order
given; each is tested for acceptance (`E_BADGE_ENTRY`, with the index) and
then for type `grant` (`E_BADGE_TYPE`, with the index) before the next is
examined; the cap is tested last, once every input is an accepted grant.

**Why.** §6.1 defines the payload as the segments of accepted grant entries,
so the cap is a property of a payload that exists. §6.2's decoding order is
the only order the law states for the same three conditions, and it examines
each segment fully before the next.

## 5. Check 5 survives a failed check 2

**Section.** §10.2. **Choice.** A `FAIL` at check 2 leaves checks 3, 4 and 6
`UNKNOWN` and leaves check 5 decided on its own terms; only a `FAIL` at check
1 leaves all of 2 to 6 `UNKNOWN`. **Why.** Each check names the checks its
failure suppresses, and check 2 names three. Check 5 reads `g.body.window` and
`now` alone and needs no ledger.

## 6. The `AUTHORITY_MISMATCH` test of checks 3 and 6

**Section.** §10.2. **Choice.** Presence of a finding named
`AUTHORITY_MISMATCH` carrying the entry's `entry_id` in the report of `I'`;
the finding's hardness is not tested. **Why.** "(soft, since a hard one
labelled the ledger broken)" states why the finding reached at that point is
soft, having passed check 2. It is a justification of the state the finding is
in.

## 7. Validity of `I` is tested on `I'`

**Section.** §10.1, §10.2. **Choice.** Checks 2, 3, 4 and 6 read "I is null or
invalid" off the audit of `I'`, which is `I` with `g` added to its pile.
**Why.** The two are equivalent: `zikaron/1` §9.4 well-formedness is decided
over the root, the anchor and evidence records, the unavailable set and the
basis, and over the pile only in that each element is a byte blob; appending
one well-formed blob can neither create nor cure a fault. Taking the audit
once, over `I'`, is what §10.1 directs.

## 8. Paths a filesystem yields that are not valid UTF-8

**Section.** §7.1, §7.2, §12 ("Platforms"). **Choice.** Rendered into the
subject with U+FFFD replacement. **Why.** Kit paths are lowercase ASCII
(§7.2), so such a pair is never in the named set and its only appearance is as
an `E_KIT_EXTRA` or `E_KIT_UNREADABLE` subject. The reading is the reader's
limit §7.6 and §12 name, and it reaches no lawful kit.

## 9. Reachability terminates

**Section.** §8.1. **Choice.** The walk of `prev` keeps a visited set.
**Why.** `→` is a partial function on `entry_id`s, so a cycle needs a `sha256`
cycle; the predicate is total all the same, and totality is not left to the
difficulty of constructing an input.

## 10. Harness misuse, by name

These exit 2 and write nothing to stdout, as "harness misuse only" allows:
a file that cannot be read (a hop's `grant` or `audit` included); a
`kit-verify` argument that is not a listable directory; `badge-encode` with no
path; a `sign` domain outside the two §3.2 literals (the list is closed); a
`depth` work argument that is not hex32; a `--now` that is not a `zikaron/1`
`int` (decimal digits, no sign, at most 2^53 − 1); a `hops.json` that is not
an array of `{grant, audit}`.

The kit directory named on the command line is the walk's input. §7.1's "a
directory that cannot be listed" reaches the directories the walk descends
into, and that is what the implementation reports; a top directory that cannot
be listed is a bad argument.

## Undecidable, and left as such

Nothing in the text was undecidable. Every choice above is decidable from the
two laws and the harness read together; items 1, 2 and 3 are the three where
the law's own sentence carries the whole weight, and they are the places where
two independent implementations are likeliest to part.

## Verification

`python3 selftest.py` builds two ledgers, two audit inputs, a signed manifest
and acknowledgement (signed through `zkk sign` itself), a badge payload, and
ten kit directories, then runs all eleven commands as subprocesses and pins 28
outputs byte for byte. Beyond that, each of §11's document tokens, each of the
nine kit manifest rule names, `KIT_OK`, `BADGE_OK`, every pairing and
attribution verdict, every check state in all six checks, and each chain
failing kind has been witnessed on a live input, and 4000 mutation samples
over the manifests, entries and payloads produced no panic, hang, or
nondeterministic answer.
