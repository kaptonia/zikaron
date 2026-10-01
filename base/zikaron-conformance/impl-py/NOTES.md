# Interpretive choices, `zikaron/1` in Python (impl-py)

Written from `docs/zikaron-v1.md` and the harness contract alone. Every place
where I judged the text to admit two readings is below: the section, the two
readings, the one I took, and why. Ordered by section. The three marked
**UNDECIDABLE** are the ones where I do not believe the text picks a side.

---

## §3 Canonical form

### 3.1 — `01` and other digit runs that begin with a valid integer

- **(a)** RFC 8259 parses `0` as a number, then `1` is a grammar failure:
  `E_JSON`, triggering byte the `1`.
- **(b)** "The bytes there" is the *maximal* run `01`; that is not an integer
  under §3.2, so `E_NUMBER`, triggering byte the `0`.

**Taken: (b).** §3.1 defines "the bytes there" as the maximal run over
`0-9 + - . e E`, and §3.5 test 2 names the fault whose triggering byte comes
earliest; `E_NUMBER`'s trigger is the first byte of the value position, which
precedes the `1`. Same for `1x` (run is `1`, a valid integer, so the fault is
the ordinary `E_JSON` at `x`) and `1e` / `1.` / `12345678901234567890`.

### 3.1 — numeric shape opened by `+` or `.`

- **(a)** `+1`, `.5` are plain grammar failures under RFC 8259: `E_JSON`.
- **(b)** Their first byte is in `- + . 0-9`, so they open a value position of
  numeric shape and fail `E_NUMBER`.

**Taken: (b),** which is what §3.1 says in as many words, and what the text's
own `-Infinity` worked example confirms.

### 3.3 — "every byte 0x20–0x7E" of a decoded string

- **(a)** Test the scalar values: every scalar in U+0020..U+007E.
- **(b)** Test the UTF-8 bytes of the decoded string.

**Taken: (a).** The two never disagree: a scalar above U+007F encodes to bytes
≥ 0x80, which (b) also refuses. Recorded because the wording invites (b) and an
implementer who applies (b) to the *input's raw bytes* instead of the decoded
string would be wrong twice over (§3.3's closing paragraph forbids that).

### 3.3 — what opens a prose subtree

- **(a)** Only an object member whose decoded key is `_md` or ends in `_md`.
- **(b)** Also array elements or nested values reached some other way.

**Taken: (a).** "A *member* whose decoded key…". An array element has no key
and opens nothing; a member inside an already-open subtree changes nothing,
since the subtree already covers every string at any depth within it.

### 3.5 — trigger byte of a grammar failure

The parser is strictly left to right and stops at the first byte no
continuation admits, or one past the end for a proper prefix. No observable
choice: every reading of "the first byte" yields the token `E_JSON`.

---

## §4 Envelope

### 4.3 step 4 — `spec` is not a string

- **(a)** A non-string cannot be byte-equal to `zikaron/1`: `E_SPEC`.
- **(b)** Some earlier or other token.

**Taken: (a).** Step 4's only test is byte-equality, and there is no token for
"`spec` is the wrong type".

### 4.3 step 8 — order within the step

`E_PREV` before `E_PREV_SEQ`, per "Where a step names more than one token, its
tests are decided in the order the tokens are written in that step". Likewise
`E_ENVELOPE_MISSING` before `E_ENVELOPE_CLOSED` at step 3.

---

## §5 Signing

### 5.7 — which RFC 6979

- **(a)** RFC 6979 as written: the HMAC seed carries
  `bits2octets(h1) = int2octets(bits2int(h1) mod n)`.
- **(b)** libsecp256k1's nonce function, which seeds with the raw 32-byte
  message hash and performs no reduction.

**Taken: (a),** because the law names RFC 6979 and nothing else. The two agree
unless the digest read as an integer is ≥ n, which happens with probability
about 2^-128; a corpus will not separate them. Verified against the published
secp256k1 RFC 6979 vectors ("Satoshi Nakamoto", "Alan Turing", …).

### 5.4 — a signing nonce that would need a recovery id above 1

- **(a)** Emit it anyway (a `v` outside {27, 28}, which no verifier accepts).
- **(b)** Skip to the next RFC 6979 candidate nonce.

**Taken: (b).** §5.4 states that no recovery id above 1 exists in this grammar,
so a producer that emitted one would be publishing an entry it knows fails.
Reached with probability about 2^-128.

---

## §6 Bodies

### 6.x — the form test for a "prose string" row

- **(a)** The row tests only that the value is a string; the charset was
  already decided by §3.3 and §3.5 test 5, and a `_md` key always opens a
  prose subtree, so every scalar is legal there.
- **(b)** The row re-tests some charset.

**Taken: (a).** Every prose row in §6 has a key ending in `_md`, so §3.3 has
already licensed every scalar value; nothing is left for `E_BODY_FIELD` to test
but the type.

### 6.5 / 6.6 — order of tests inside a body

Not a choice: §6 states that one token covers every body fault and that no
order of tests within a body is part of the law.

### 6.6 — an attestation on an entry whose `prev` is `null`

Unreachable (§4.2 puts every non-`genesis` entry off position 0, and `genesis`
is not `adoption`), but the code answers "the attestation does not verify"
rather than raising, since §6.6 never voids an entry.

---

## §7 / §8 Audit

### 8.2 — `certain` for an `AUTHORITY_MISMATCH` on the entry that itself opened the gap — **UNDECIDABLE**

- **(a)** `SEQ_GAP` is recorded first and sets `certain = false`, so the same
  entry's authority finding is **soft**, and a ledger with a gap and a
  post-gap authority fault labels `GAPS`.
- **(b)** `certain` is read as it stood when the entry was entered, so the
  finding on *that* entry is **hard** and only later entries go soft; the same
  ledger labels `BROKEN_CHAIN`.

**Taken: (a).** Three reasons: §8.2 writes `SEQ_GAP` before
`AUTHORITY_MISMATCH` in its own bullet order and says "Afterwards … `certain`
= false"; §8.3's ground is that "a gap could hide a succession", and the
succession a gap could hide is exactly the one that would authorize the entry
across the gap; and §8.3's "Absence never convicts" forbids convicting on a
state the walk could not observe.

This is the sharpest ambiguity in the audit and the one most likely to split
two implementations, because it moves the **label**, not only a `hard` flag.

### 8.2 — "the `seq` visited just before it" for the first visited entry

- **(a)** No previous seq exists, so only the `expected` comparison decides,
  and a ledger whose smallest `seq` is 3 records `SEQ_GAP(0, 3, …)`.
- **(b)** Some sentinel previous seq.

**Taken: (a),** which §8.2 confirms: "A ledger whose smallest `seq` is not 0
records `SEQ_GAP(0, s, …)` first."

### 8.2 — when the authority update takes effect

- **(a)** After every entry at position `k` has been visited, and it governs
  every later *visited* position, gaps included (a succession at 1 still binds
  a visited entry at 5 when 2, 3, 4 are absent).
- **(b)** It governs only position `k + 1`.

**Taken: (a).** §7.3 says `auth(j) = to` for every `j > k` until a later
succession changes it, and §8.2's update sentence says "for positions after
`k`".

### 7.4 / 8.1 — the whole-set lineage runs over which entries

- **(a)** Every byte string in the pile that passes §4.3 (§7.4: "every
  candidate entry a verifier holds").
- **(b)** Only ledger entries (circular, since the lineage is what decides
  membership).

**Taken: (a).** The two in fact coincide: an entry that extends the lineage is
signed by a key already in it, so its author is in the lineage and it is a
ledger entry. §7.4's deliberate contrast — the *prefix* lineage says "every
ledger entry (§8.1)", the whole-set one says "every candidate entry" — is what
makes (a) the reading, and the coincidence is why no corpus can separate them.

### 8.5 / 8.6 — which unavailable set `MISSING` subtracts

- **(a)** The set as trimmed by §8.1.
- **(b)** The set as supplied.

**Taken: (a).** §8.6: "After the trimming of §8.1, every hash in the counted
anchor set lands in exactly one of three places: `have`, the unavailable set,
or `MISSING`." Report item 8 also carries the trimmed set, per the harness.

### 8.4 — a pair meeting both grounds

One finding, per "whatever number of grounds the pair meets". Its `seq` is the
smaller of the two entries' `seq` values even when the ground is a shared
`prev` across different positions.

### 8.7 item 3 — what "each distinct finding tuple" means

- **(a)** The four-part sort tuple `(position, name, entry_id, second)`.
- **(b)** The finding's full member list.

**Taken: (b)** (dedup on the whole finding). Unobservable: no entry can record
two findings of one name, so no two findings ever share a sort tuple.

---

## §9.4 / §9.5

### 9.4 — what earns `NO_LABEL` — **UNDECIDABLE**

The law names two grounds: a value that is not a `zikaron/1` basis, and two
anchor or evidence records that disagree on a remaining field. The harness adds
nothing else. It leaves undefined what a verifier does with an input it cannot
read as the five inputs at all.

- **(a)** `NO_LABEL` for any structural failure to read the five inputs: a
  `root` that is not hex20, a `pile` element that is not `0x` + an even number
  of hex digits, an anchor record missing a member or carrying a bad form or an
  unrecognised `verdict`, an integer outside [0, 2^53 − 1], a duplicate key
  anywhere in the file.
- **(b)** Exit 2 as harness misuse, or coerce and continue.

**Taken: (a).** `NO_LABEL` is the only "I cannot answer" the harness defines,
and exit 2 is reserved there for a missing file or bad arguments. A corpus that
feeds malformed audit inputs will separate implementations here.

### 9.4 — extra members in the harness's own wrapper objects — **UNDECIDABLE**

- **(a)** Fatal, as §9.4 makes them inside the basis.
- **(b)** Ignored, as §6.10 makes them inside a body.

**Taken:** fatal **inside the basis** (§9.4: "§6.10 reaches no object of this
section", and its list of members is exact); **ignored** in the top-level audit
input object, in anchor records, and in evidence records, because those three
are the harness's encoding of §8's five inputs and the law states no rule for
them at all. A corpus carrying an anchor record with a stray member would split
implementations.

### 9.4 — duplicate anchor records that *agree*

- **(a)** Collapse to one record.
- **(b)** `NO_LABEL`, since the set "holds at most one record per
  (chainId, tx, hash)".

**Taken: (a).** The sentence's own qualifier is "two records that *disagree* on
any remaining field", which would be idle if agreeing duplicates were already
fatal. Same for evidence records keyed by `(chainId, tx)`.

### 9.4 — hex case

Law-level fields (`root`, every `tx`, `sender`, `hash`, `unavailable` element,
and every address inside the basis) must be lowercase, per §1: uppercase fails
the form and yields `NO_LABEL`. The harness's own opaque blobs — `pile`
elements and `calldata` — accept either case, since they are a transport
encoding of arbitrary byte strings and §1's `hex20`/`hex32`/`hex65` never
describe them.

### 9.5 — the word-offset containment test

Offsets `32m` and `4 + 32m` for m ≥ 0, with the whole 32 bytes inside the
calldata. Calldata shorter than 32 bytes proves nothing. Read identically for
§9.1's registry-form calldata containment (not implemented: chain-facing).

### 9.5 — the adoption evidence set is not trimmed

§8.1 trims the anchor set (by lineage `sender`) and the unavailable set, and
names neither the pile nor the evidence set. So an evidence record whose
`sender` is outside the lineage still counts — and is exactly the case the
attestation clause exists to serve.

### 9.5 — one attestation, several elements

Each element is tested against *its own* transaction's sender, so one entry's
single attestation proves the elements whose sender is the `attestor` and
leaves the others to the prefix lineage. Taken as written.

### 9.5 — an element whose `chainId` is in `adoptionChains` but whose transaction is in no evidence record

Unproven, and reported as `ADOPTION_UNPROVEN` like any other: "unprovenness
has one report form and one only".

---

## Harness contract

### `sign` — raw bytes or re-canonicalized bytes

- **(a)** Hash the file's bytes as they stand.
- **(b)** Read the file under §3.5 tests 1–5 and hash the §3.4 canonical bytes
  of the parsed value.

**Taken: (b),** which is what §5.1 and §6.6 define (`B6` and `C` are *canonical
bytes*, not "the bytes of a file"). Identical on any file that holds canonical
bytes, as the harness promises it does.

### `sign` — a preimage file that fails §3.5 tests 1–5

Exit 2 with a message on stderr and no JSON. The harness defines no rejection
output for `sign`, and an unreadable preimage is harness misuse.

### `sign` — a domain other than the two of §5.6

Accepted and used literally: `M = D || 0x0A || hex32(sha256(canon(file)))`,
and `decimal(len(M))` computed from that length. §5.6 closes the domain list,
but the harness passes the domain as an argument, so refusing one would make
the argument pointless.

### `sign` — private key spelling

`0x`-prefixed or bare hex, any case. Out of `[1, n − 1]` exits 2.

### `check` — `entry_id`

`sha256` of the file's bytes (§2.1, identity is total), printed only on
acceptance; a rejection carries the token and nothing else.

### `canon` — which tests

§3.5 tests 1 through 5 exactly, so a file that parses but is not canonically
spelled succeeds here and would fail `check` with `E_NOT_CANONICAL`.

---

## Verified against published vectors

- secp256k1 private keys 1, 2, 3 → the standard Ethereum addresses.
- EIP-191 personal-sign digest of `hello world` →
  `d9eba16e…cf807d68`.
- RFC 6979 / secp256k1 deterministic-`k` signatures, four published vectors,
  `(r, s)` byte-exact in low-s form.
- Canonical bytes cross-checked against an independent JCS-shaped emitter over
  20 000 random values; §3.5 acceptance cross-checked against RFC 8259 parsing
  over 40 000 random documents in four spellings; 200 000 random byte strings
  fuzzed through `parse15`, `canon`, and §4.3 with no crash and no
  non-idempotent canonicalization.
