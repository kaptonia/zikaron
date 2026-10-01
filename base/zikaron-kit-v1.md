# ZIKARON kit grammar v1 (`zikaron.kit/1`)

The client's document and reading law: the second, smaller law a ZIKARON
client needs beyond the wire law, so that two clients agree byte for byte
on the documents they exchange and on the verdicts they show. Where
[`zikaron/1`](zikaron-v1.md) says what an entry is and what a ledger's
audit says, `zikaron.kit/1` says what a fingerprint manifest and its
acknowledgement are, what a badge payload is, what a disclosure kit is, how
deep a history reads, and whether a grant is good to rely on
now. It is self-sufficient in the same sense as its parent: every predicate
a verifier needs is stated here in full, and it consumes `zikaron/1` only
through that law's frozen core.

**Status.** The law is the set of total predicates in this text. A
conforming implementation embeds the `zikaron/1` core of the release
digest named in §13.4, and agrees with the core named in §13.5 of this text
on every input; until that core is named this text is the criterion.
Evolution goes through `zikaron.kit/2`.

**Rule classes.** Two classes appear, and every rule below is marked:

- **[D]** document predicates: decided offline from the byte strings the
  predicate is given (one document, several documents, or the enumeration
  of a directory), and nothing else.
- **[R]** reading predicates: decided offline from documents together with
  one or more `zikaron/1` audit inputs (the five inputs of `zikaron/1` §8)
  and, where stated, a caller-supplied `now`. No predicate of this law
  consults a chain or a clock; what a chain said is inside the audit
  input, and what time it is comes from the caller.

## 1. Notation

`zikaron/1` §1 applies: byte strings, `hex20`, `hex32`, `hex65`, tokens,
skeleton and prose strings, `int`, `bool`, bytewise order, `sha256`,
`keccak256`, secp256k1 and its order `n`, UTF-8, Unix seconds. In addition:

- **`doc_id(b)`** = `sha256(b)` for any byte string `b`: identity is total
  here as in `zikaron/1` §2.1.
- **`accept(b)`**: the `zikaron/1` acceptance of a byte string as an entry
  (`zikaron/1` §4.3), decided by the embedded core, yielding the entry or
  a `zikaron/1` §10 token. **`canonical(b)`**: `zikaron/1` §3.5 alone.
- **`audit(I)`**: the `zikaron/1` audit (`zikaron/1` §8) of an audit input
  `I`, decided by the embedded core. It yields the report of `zikaron/1`
  §8.7 and the **ledger**, the set of ledger entries of `zikaron/1` §8.1,
  each with its `seq`, `prev`, `author`, `entryType`, `body`, `entry_id`,
  and the findings that name it. An input the core refuses under
  `zikaron/1` §9.4 (no label) is an **invalid input**; every reading
  predicate names its outcome for one.
- **`now`**: an `int` of Unix seconds supplied by the caller, or `null`
  when the caller has no trusted time.
- **`base64url`**: RFC 4648 §5 with no padding characters, and canonical:
  the unused trailing bits of the last character are zero.

## 2. Canonical form

**[D] 2.1** Every document of this law, a manifest (§4), an acknowledgement
(§5), and a kit manifest (§7.3), is a canonical byte string under
`zikaron/1` §3, decided by `canonical(b)`; the rejection tokens of
`zikaron/1` §3 apply unchanged. Every result object a reading or a check
yields (§9.2, §10.3, §10.5) is written in the same canonical form, though no
predicate of this law decides one. A badge payload (§6) is ASCII text and no such document: §6.2 decides it in full and never calls `canonical`. A document's published bytes are its
canonical bytes and nothing else (`zikaron/1` §2.2).

## 3. Signing

**[D] 3.1 Construction.** A signed document of this law is an object whose
top-level member `sig` is hex65. `B` is the canonical bytes of the object
with the top-level `sig` removed and nothing else; `presig = sha256(B)`;
the message is `D || 0x0A || hex32(presig)` with `D` the document's domain
literal (§3.2); digest, signature form, range, low-s, recovery, and address
derivation follow `zikaron/1` §5.3 through §5.5 verbatim; the recovered
address must be byte-equal to the document's signer member named in its
table. Failure tokens: `E_SIG_V`, `E_SIG_RANGE`,
`E_SIG_HIGH_S`, `E_SIG_RECOVER`, `E_SIG_SIGNER`, with the meanings of
`zikaron/1` §10; `E_SIG_FORM` is decided by the document's own order (§4.2,
§5.2) before this section runs.

**[D] 3.2 Domains.** This law defines exactly two domain literals, and the
list is closed: `zikaron.fpm/1` for fingerprint manifests (§4) and
`zikaron.ack/1` for acknowledgements (§5). Both messages are 80 bytes. The
two literals differ from each other at their ninth byte (`f` and `a`), and
each differs from every domain of `zikaron/1` at its eighth byte (`.`
where `zikaron/1` carries `/`); by the argument
of `zikaron/1` §5.6 no byte string is a valid message under two domains,
of this law or across laws.

**3.3 Key obligations.** A manifest is signed by the author's key under
`zikaron/1` §5.7's allowance for a client's own domains, produced by the
author's own tooling. An acknowledgement is signed by the recipient's
**acknowledging key**, a key under exactly the obligations `zikaron/1`
§5.7 places on an author's key: it signs `zikaron/1` entries and
attestations and messages of that law's §5.2 shape under domain literals
of this or another client law, all produced by its holder's own tooling,
and nothing else; it never answers an EIP-191 `personal_sign` request
from any application; it authorizes no EIP-7702 delegation. A recipient's
own anchor key satisfies this, and a wallet that signs for applications
does not: a signature phished over the 80-byte message is a complete
forged acknowledgement, and no verifier can tell it from a real one. A
verifier cannot test these obligations and does not try.

## 4. Fingerprint manifest (`zikaron.fpm/1`)

A manifest is the author's signed table of the per-recipient variants of
one delivery: which recipient was handed which bytes, by digest, before the
bytes went out.

**[D] 4.1 Members, closed.** A manifest is a canonical object with exactly
these members and no others:

| key | form | rule |
|---|---|---|
| `spec` | string | byte-equal to `zikaron.fpm/1` |
| `author` | hex20 | the signer (§3.1) |
| `work` | hex32 | the content digest of the work delivered, in the sense of `zikaron/1` §6.2; data for readers |
| `grant` | `null` or hex32 | the `entry_id` of the grant this delivery serves, or `null` for a delivery under no grant; data for readers |
| `rows` | array | non-empty; every element an object with exactly the members `recipient` (hex20) and `variant` (hex32); recipients distinct across rows; variants distinct across rows; rows sorted by `recipient` bytewise |
| `note_md` | prose string | may be the empty string |
| `sig` | hex65 | the signature (§3) |

**[D] 4.2 Order of decision.** `canonical(b)` first (its tokens); then the
root is an object (`E_DOC`); the member set is exactly the seven keys
(`E_DOC_MISSING` for an absent key, then `E_DOC_CLOSED` for a foreign key);
`spec` (`E_SPEC`, a `spec` of any other form or value included); `author` (`E_FPM_AUTHOR`); `work` (`E_FPM_WORK`); `grant`
(`E_FPM_GRANT`); `rows` a non-empty array (`E_FPM_ROWS`); each row in array
order is an object with exactly the two members in their forms
(`E_FPM_ROW`, carrying the zero-based index); recipients distinct
(`E_FPM_DUP_RECIPIENT`); variants distinct (`E_FPM_DUP_VARIANT`); rows
sorted (`E_FPM_ROW_ORDER`); `note_md` a string (`E_FPM_NOTE`); `sig` hex65
(`E_SIG_FORM`); then §3.1 with the recovered address required equal to
`author`. A byte string that passes is a manifest; nothing else is.

**4.3 Meaning.** A manifest commits the author, before delivery, to the
digest of the bytes each recipient will receive, and to their being
distinct, so that a leaked byte string names at most one row of one
manifest (§5.4); across manifests the author's rows may repeat, and what
that means is the author's affair.
Producing the variants is outside this law. Whether `work` and `grant`
name anything is reference resolution in the reader's hands (`zikaron/1`
§11); the grammar tests forms.

## 5. Acknowledgement (`zikaron.ack/1`)

An acknowledgement is one recipient's signed statement of the digest of
the bytes that recipient received, against one manifest.

**[D] 5.1 Members, closed.**

| key | form | rule |
|---|---|---|
| `spec` | string | byte-equal to `zikaron.ack/1` |
| `recipient` | hex20 | the signer (§3.1) |
| `fpm` | hex32 | the `doc_id` of the manifest acknowledged |
| `variant` | hex32 | the digest of the bytes received |
| `note_md` | prose string | may be the empty string |
| `sig` | hex65 | the signature (§3) |

**[D] 5.2 Order of decision.** `canonical(b)`; root object (`E_DOC`);
member set exactly the six keys (`E_DOC_MISSING`, `E_DOC_CLOSED`); `spec`
(`E_SPEC`, a `spec` of any other form or value included); `recipient` (`E_ACK_RECIPIENT`); `fpm` (`E_ACK_FPM`); `variant`
(`E_ACK_VARIANT`); `note_md` (`E_ACK_NOTE`); `sig` hex65 (`E_SIG_FORM`);
§3.1 with the recovered address required equal to `recipient`. A byte
string that passes is an acknowledgement; nothing else is.

**[D] 5.3 Pairing.** `pair(m, a)`, over a manifest byte string `m` and an
acknowledgement byte string `a`, yields one verdict from a closed set,
decided in this order:

1. `m` is not a manifest (§4.2): `FPM_INVALID`, carrying the token and no
   index.
2. `a` is not an acknowledgement (§5.2): `ACK_INVALID`, carrying the token.
3. `a.fpm` ≠ `hex32(doc_id(m))`: `ACK_FPM_MISMATCH`.
4. No row of `m` has `recipient` = `a.recipient`: `ACK_NO_ROW`.
5. That row's `variant` ≠ `a.variant`: `ACK_VARIANT_MISMATCH`.
6. Otherwise `PAIRED`, carrying `recipient` and `variant`.

A recipient can sign only its own row, because the signer must equal
`recipient` and the row is found by `recipient`; an acknowledgement cannot
be pointed at another manifest, because `fpm` is the manifest's `doc_id`.

**[D] 5.4 Attribution.** `attribute(m, a, x)`, over a manifest, an
acknowledgement, and a byte string `x`, yields one verdict: the pairing
verdict's name alone, without the token §5.3 attaches to it, when
`pair(m, a)` is not `PAIRED`; otherwise `ATTRIBUTED`, carrying the row's `recipient`, iff
`hex32(sha256(x))` = `a.variant`, else `NOT_ATTRIBUTED`. Attribution is a
statement about bytes and a signed acknowledgement, never about who leaked
them; what follows from it is the parties' affair. Because variants are
distinct within a manifest (§4.1), one byte string attributes to at most
one row of one manifest.

## 6. Badge payload

A badge carries a grant, and the chain of grants above it, as one text a
customer can scan and a verifier can decode without any other input.

**[D] 6.1 Encoding.** A payload is the ASCII text `zikaron-grant:` followed
by one or more **segments** joined by `.`, in order from the original
author's grant to the grant displayed. Each segment is `base64url` of the
canonical bytes of one accepted `zikaron/1` entry of type `grant`. The
payload's total length in bytes is at most `BADGE_CAP` = 2953, the
capacity of a version 40 QR code in byte mode at error correction level L,
so that every lawful payload has a QR rendering. **`encode(e_0 … e_m)`**,
over one or more byte strings in order, yields the payload or one token,
decided in this order: for each `k` in order, before the next is examined,
`e_k` fails `accept`: `E_BADGE_ENTRY`, carrying `k`; its entry is not of
type `grant`: `E_BADGE_TYPE`, carrying `k`. Then the payload the rule above
builds is longer than `BADGE_CAP` bytes: `E_BADGE_CAP`. Otherwise the
payload. Whether segment 0 states no upstream and whether the byte links of
§6.3 hold are read by §6.2 and never here: an encoder assembles what it is
handed, and a payload it yields may still fail §6.2 steps 4 and 5.

**[D] 6.2 Decoding, total.** `decode(p)` yields `BADGE_OK` with the list of
grant entries, or one token, decided in this order; a verifier renders
nothing of a payload that yields a token:

1. `p` does not begin with the 14 bytes `zikaron-grant:`: `E_BADGE_PREFIX`.
2. `p` is longer than `BADGE_CAP` bytes: `E_BADGE_CAP`.
3. The remainder is split into segments: the maximal runs of bytes
   between `.` bytes, from the start to the end, so that `n` dots yield
   `n + 1` segments and a leading, trailing, or doubled dot yields an empty
   one. For each segment `k` in that order, before the next segment is
   examined:
   a. the segment is empty, holds a byte outside `A-Za-z0-9-_`, has a
      length ≡ 1 (mod 4), or its unused trailing bits are not zero:
      `E_BADGE_B64`, carrying `k`;
   b. its decoded bytes fail `accept`: `E_BADGE_ENTRY`, carrying `k` and
      the `zikaron/1` §10 token;
   c. its entry is not of type `grant`: `E_BADGE_TYPE`, carrying `k`.
4. Segment 0's grant's `body` has a member with key `upstream`, whatever
   its value (`zikaron/1` §6's presence rule: a key present is a member
   present, `null` included): `E_BADGE_INCOMPLETE` (the payload does not
   start at a grant that states no upstream).
5. For each `k` ≥ 1 in order, the byte link of §6.3 between segments
   `k − 1` and `k` fails: `E_BADGE_LINK`, carrying `k`.
6. Otherwise `BADGE_OK` with the list.

**[D] 6.3 Byte link.** Between an upstream grant entry `u` and a
downstream grant entry `d`: the byte link **holds** iff `d.body` carries
a member `upstream` whose value is a string byte-equal to
`hex32(entry_id(u))`, and `d.body.work` = `u.body.work`. `upstream` is an
extra member under `zikaron/1` §6.10 and carries no meaning there; this
law reads it as the downstream party's own statement of what it holds.
Whether `d`'s issuer is `u`'s grantee is a question of ledgers, decided by
§10.5 and never by bytes alone.

A payload of one segment is a grant that claims no upstream; a payload of
several is a chain whose byte links hold. Whether each grant is good now is
§10, which needs the issuers' ledgers; the payload alone proves only what
its bytes say.

## 7. Disclosure kit

A disclosure kit is a directory that hands a reader the bytes that match a
ledger's anchors: entries, content files, proof kits, and their
cross-references, sealed by one canonical manifest so that the reader's
verification is a pure function of the enumeration the walk yields. A kit
is a pile
in the sense of `zikaron/1` §8: it may carry bytes that fail acceptance, and
verification reports them without failing.

**[D] 7.1 Enumeration.** A reader turns a directory into an **enumeration**,
the set of pairs `(path, bytes)` for every regular file under it, by a
**walk**; the **path** of any entry the walk examines is the sequence of
listing names from the kit directory to it joined by `/`, with no leading
and no trailing `/`, and the enumeration carries that path for a regular
file with its bytes. The walk: list the directory, discard the listing entries named `.`
and `..`, examine the rest in bytewise order of their names, and for each
entry in that order either read it in full (a regular file), or walk it
before moving to the next name (a directory), without following symbolic
links. The walk fails at the first entry it
examines that is a symbolic link, a directory that cannot be listed, a
file that cannot be read in full, neither a regular file nor a directory,
or named by bytes that are not valid UTF-8; `verify_kit(dir)` then yields
`E_KIT_UNREADABLE` carrying that entry's path, the one-character string `.`
where the failing entry is the kit directory itself, and the path of the
directory that listed it where the entry's own name is not valid UTF-8,
that path being the same one-character string `.` where the listing
directory is the kit directory, since a path this law's output cannot
spell names nothing; the walk
examines nothing else. Otherwise `verify_kit(dir)` is
§7.4 over the enumeration.

**[D] 7.2 Kit paths.** A kit path is a skeleton string of one or more
segments joined by `/`; each segment is one to 255 bytes from
`a-z0-9._-`, is neither `.` nor `..`, and does not begin with `-`; the
whole path is at most 1024 bytes and begins with no `/`. Kit paths are
compared bytewise and, being lowercase ASCII, collide on no case-folding
or normalizing filesystem. A path in `files` names `files/<path>`; a path
in `proofs` names `proofs/<path>`.

**[D] 7.3 Manifest.** The enumeration must hold a pair at path
`manifest.json` whose bytes are a canonical object with exactly these
members:

| key | form | rule |
|---|---|---|
| `spec` | string | byte-equal to `zikaron.kit/1` |
| `root` | `null` or hex20 | the ledger the kit's author says the entries belong to, or `null`; data for readers |
| `entries` | array | hex32 elements, distinct, sorted bytewise; may be empty |
| `files` | array | elements each an object with exactly the members `path` (a kit path), `sha256` (hex32), and `size` (int); paths distinct; sorted by `path` bytewise; may be empty |
| `contents` | array | elements each an object with exactly the members `content` (hex32) and `path` (a path listed in `files` whose listed `sha256` equals `content`); rows distinct; sorted by `content` then `path`; may be empty |
| `proofs` | array | elements each an object with exactly the members `path` (a kit path), `sha256` (hex32), and `tx` (hex32); paths distinct; sorted by `path`; may be empty |
| `note_md` | prose string | may be the empty string |

Every member list this section spells is closed, the manifest's own and
its rows' elements alike: an element that is not an object, or that
carries a member beyond its row's list, fails its row's rule. The
manifest's rules are decided in this order, and the first failure names
its **rule** as the subject: `canonical` (the bytes fail
`canonical`; which `zikaron/1` token they fail on reaches no output of this
law, one rule name being the whole of the subject), `members` (the root is not
an object, or its member set is not exactly the seven keys), then the
rows of the table in order, `spec`, `root`, `entries`, `files`,
`contents`, `proofs`, `note_md`, each row's tests in the order its rule
states them. The kit's identity is `doc_id` of the manifest's bytes.

**[D] 7.4 Verification.** `verify_kit(enumeration)` yields one verdict,
decided in this order; the first failure is the verdict and carries its
subject:

1. No pair at `manifest.json`: `E_KIT_MANIFEST_ABSENT`.
2. Its bytes fail §7.3: `E_KIT_MANIFEST` with the failing rule as its
   subject.
3. For each id in `entries` in order: no pair at `entries/<id without
   0x>.zk1`, or that pair's `doc_id` is not the id: `E_KIT_ENTRY_BYTES`
   with the id.
4. For each row of `files` in order: no pair at `files/<path>`, or the
   pair's bytes have another `sha256` or another length: `E_KIT_FILE` with
   the row's `path`.
5. For each row of `proofs` in order: no pair at `proofs/<path>`, or
   another `sha256`: `E_KIT_PROOF_BYTES` with the row's `path`.
6. Any pair whose path is not in the **named set**: `manifest.json`,
   `entries/<id without 0x>.zk1` for every listed id, `files/<path>` for
   every listed file path, and `proofs/<path>` for every listed proof
   path; `E_KIT_EXTRA` with the bytewise-smallest such path.
7. Otherwise `KIT_OK`, carrying the kit's id (§7.3), the counts of entries,
   files, and proofs, and the list `invalid_entries`, in `entries` order, of listed ids whose
   bytes fail `accept`, each with its `zikaron/1` token (informational: a
   kit may carry an anchored byte string that is not an entry, which is
   exactly what a `MISSING` row asks to see).

A directory that holds only `manifest.json` with empty arrays is a kit
that proves nothing, and `KIT_OK` says so through its counts.

**7.5 Proof files.** The bytes at a `proofs` path are opaque to this law:
§7.4 pins them by digest and reads nothing inside. A reader who holds
trusted block hashes verifies them under `zikaron/1` §9.7, in whatever
form that reader's `zikaron/1` implementation accepts, and reports what
the parent reports; that reading is chain-facing in the parent's sense,
is no predicate of this law, and is outside §13.1's corpus. `KIT_OK`
never says more than the bytes do.

**7.6 Meaning.** A kit proves that its bytes are the bytes its manifest
names, to any reader whose walk yields the same enumeration; a reader
that cannot hold a file (`zikaron/1` §11) reports `E_KIT_UNREADABLE` for
it, and that is the reader's limit, named in §12. Whether the entries are a ledger's, whether they are anchored, and
what they are worth, are the reader's audit (`zikaron/1` §8) over the kit's
entries as a pile, and the reader's weighing; a kit is an input to those,
never their conclusion. An author's self-verification before export is
the same function over the same enumeration. `contents` rows pin the
convention that a listed content digest is the `sha256` of the file's
bytes; a work digested another way (a mode that names its own function)
is delivered as a file with no `contents` row, and the mode says how to
check it. A file is tied to an entry through the digest alone: a
`contents` row's `content` is what a history entry's `content` or a
grant's `work` names, and the reader joins them by that value.

## 8. Reachability and anchoring, as read

Two definitions the reading predicates share, over the ledger and the
audit input's anchor records.

**[R] 8.1 Reachable.** Over the ledger, let `F → F'` hold iff `F.prev` is
the `entry_id` of ledger entry `F'`. Ledger entry `E` is **reachable** from
ledger entry `F` iff `E` = `F` or `(F, E)` lies in the transitive closure of
`→`. The ledger is finite, so the closure is finite; reachability follows
`prev` through ledger entries only and stops where no ledger entry
matches (`zikaron/1` §9.6).

**[R] 8.2 Anchored.** The **counted anchors** are the anchor records of the
audit input that survive the trimming of `zikaron/1` §8.1 and carry
verdict `counted`. Ledger entry `E` is **anchored** iff some counted anchor
has `hash` = the `entry_id` of a ledger entry `F` from which `E` is
reachable. The **bound** of `E` is the smallest `blockTimestamp` among such
records, or `null` when `E` is not anchored; across chains the smallest
timestamp is the bound. `zikaron/1` §9.2 gives one bound per chain and
§9.6 states no minimum across them, leaving a single figure to the reader
who says which chains it weighed; this law's reader weighs every chain the
basis of `I` declares, and the bound is relative to that basis. This is `zikaron/1`
§9.6 read as a predicate: an anchor of a later entry bounds every earlier
entry on its line and none off it.

## 9. Depth reading

**[R] 9.1 Inputs.** An audit input `I` and a work digest `w` (hex32).

**[R] 9.2 Reading.** If `I` is invalid, the reading is the object
`{"valid": false}` and nothing else. Otherwise let `(report, ledger) =
audit(I)`, let `H` be the set of ledger entries of type `history` whose
`body.content` = `w`, and the reading is a canonical object with exactly
these members:

- `valid`: `true`.
- `label`: the report's label.
- `found`: `true` iff `H` is non-empty. When `false`, `earliest` is `null`,
  `deepest` is 0, and `continuity` is `{"anchored": 0, "span": 0}`.
- `earliest`: the smallest bound (§8.2) among the elements of `H` that are
  anchored, or `null` when none is.
- `deepest`: the number of elements of `H` that are anchored.
- `continuity`: with `H0` the element of `H` with the smallest `seq`
  (bytewise-smallest `entry_id` among equals) and `Hmax` the element with
  the largest `seq` (bytewise-smallest `entry_id` among equals, though only
  `Hmax.seq` is read, so the tie-break decides nothing), `span` = `Hmax.seq` − `H0.seq` + 1, and
  `anchored` = the number of distinct `seq` values `s` with `H0.seq` ≤ `s`
  ≤ `Hmax.seq` such that some ledger entry `E` at `s` from which `H0` is
  reachable (§8.1) has an `entry_id` that is the `hash` of a counted anchor
  (a direct anchor on `H0`'s own line, never a bound and never a fork
  twin off it). Both are ints; the ratio is the reader's to display. Every
  timestamp in a reading is drawn from the counted anchors of `I`, so a
  reading is relative to `I`'s basis exactly as a check result is (§10.2),
  and a reader who shows an `earliest` shows the basis beside it.

Same inputs, same object, on both sides of a sale. The reading is a
description of one ledger's record of one digest; two digests of one work
are two readings (`zikaron/1` §6.3), and what a reading is worth, on a
ledger of any label, is the reader's; the label travels with it.

## 10. Grant check

Whether a grant is good to rely on, read from its issuer's ledger and the
caller's time. The doctrine is the parent's (`zikaron/1` §8.3): a failure
needs a witness in hand, and absence acquits only when the record is
complete.

**[R] 10.1 Inputs.** A byte string `g`; an audit input `I` whose root is
the issuer's ledger root as the terms name it, or `null` when the caller
holds no audit input for the issuer; `now`. When `I` is given, the audit
is taken over `I'`, which is `I` with `g` added to its pile: the parent's
audit then decides whether `g` is a ledger entry under the root and what
findings name it. The parent's §9.4 test reads the root, the anchor records,
the unavailable set, the evidence records and the basis and never the pile,
so `I'` is an invalid input exactly when `I` is, and every check below that
gates on `I` being invalid gates on the same state it later reads from the
report of `I'`.

**[R] 10.2 Checks and verdict.** Six checks, each yielding `PASS`, `FAIL`,
or `UNKNOWN`, decided in this order. The result is the object of §10.3;
its **verdict** is `GREEN` iff every check is `PASS`, `PARTIAL` iff no
check is `FAIL` and some check is `UNKNOWN`, and `FAIL` otherwise.

| # | token | test |
|---|---|---|
| 1 | `BAD_SIG` | `g` fails `accept` (`FAIL`, carrying the `zikaron/1` token), or its entry is not of type `grant` (`FAIL`, carrying `NOT_A_GRANT`). A `FAIL` here leaves checks 2 to 6 `UNKNOWN`. |
| 2 | `BROKEN_LEDGER` | `I` is `null` or invalid: `UNKNOWN`. Else the report of `I'` labels `BROKEN_CHAIN`: `FAIL`, which leaves checks 3, 4, and 6 `UNKNOWN`. Else `PASS`. |
| 3 | `NOT_IN_LEDGER` | `I` is `null` or invalid: `UNKNOWN`. Else `g` is not a ledger entry of `I'` (its author is outside the root's lineage): `FAIL`, which leaves checks 4 and 6 `UNKNOWN`. Else `g`'s entry carries an `AUTHORITY_MISMATCH` finding (soft, since a hard one labelled the ledger broken): `UNKNOWN`. Else `PASS`. |
| 4 | `UNANCHORED` | `I` is `null` or invalid, or check 3 is `FAIL`: `UNKNOWN`. Else `g`'s entry is anchored (§8.2): `PASS`. Else the basis is **covering** iff its `chains` array is non-empty and every `chains` object has a non-empty `registries` array and a `senders` array that includes every key of the ledger's lineage (`zikaron/1` §7.4) (`bareTx` adds anchors and never coverage, since a bare anchor the basis does not name has no discovery surface, `zikaron/1` §9.4); a basis that is not covering: `UNKNOWN` (the scan's silence convicts nobody). Else some anchor record of `I'` undiscarded by `zikaron/1` §8.1 carries verdict `UNPROVEN` and a `hash` that is the `entry_id` of a ledger entry from which `g`'s entry is reachable (§8.2 with `counted` read as `UNPROVEN`): `UNKNOWN` (an anchor of bytes that would anchor `g` exists and its codeless test was not decided). Else the label of the report of `I'` is `COMPLETE`: `FAIL`. Else `UNKNOWN` (a `MISSING` or unavailable entry may bound it). |
| 5 | `EXPIRED` | `g.body.window` absent: `PASS`. Else `now` is `null`: `UNKNOWN`. Else `now` < `from` or `now` > `to`: `FAIL`. Else `PASS`. |
| 6 | `REVOKED` | `I` is `null` or invalid, or check 3 is `FAIL`: `UNKNOWN`. Else some ledger entry of `I'` of type `revocation` has `body.grant` = `hex32(entry_id(g))` and carries no `AUTHORITY_MISMATCH` finding: `FAIL`. Else the label of the report of `I'` is `COMPLETE`: `PASS`. Else `UNKNOWN` (a withheld revocation is exactly what an incomplete record hides). |

A verifier shows every check's state; a green mark appears only for
`GREEN`, and `PARTIAL` is shown as its own state, never as green and never
as red. The window test is inclusive at both ends and reads `now` alone.
Every verdict is relative to the basis of `I`: `GREEN` says that under
the chains, registries, and senders the reader declared, and the bare
transactions it named, every check passed; a revocation anchored on a
chain the basis does not name is outside the record the reader read, and
the result carries the basis so that a reader who wants a wider record
sees what was read.

**[R] 10.3 Result form.** The result is a canonical object with exactly:
`verdict` (`GREEN`, `PARTIAL`, or `FAIL`); `basis`, the basis of `I` as a
canonical value, or `null` when `I` is `null` or invalid; `checks`, an
array of six
objects in check order, each with exactly `{n, token, state, reason}`:
`n` the check number, `token` the check's token, `state` one of `PASS`,
`FAIL`, `UNKNOWN`, and `reason` the `zikaron/1` token or `NOT_A_GRANT` for
check 1 in state `FAIL` and `null` otherwise; and `failed`, the array of
the tokens of the checks in state `FAIL`, in check order (empty unless the
verdict is `FAIL`).

**[R] 10.4 Ledger link.** Between an upstream grant `u` and a downstream
grant `d` read under audit input `I_d`, the ledger link is decided in this
order: the byte link of §6.3 fails: **false**; `I_d` is `null` or invalid:
**unknown**; `I_d.root` ≠ `u.body.grantee`: **false**; otherwise
**true**: the downstream issuer's ledger is the one the upstream grantee
opened, whatever key signs it now.

**[R] 10.5 Chain check.** Inputs: a list of hops `(g_k, I_k)` for `k` = 0
… `m`, ordered from the original author's grant to the grant relied on,
and `now`. An empty list yields verdict `FAIL` with token `CHAIN_EMPTY`,
`hops` and `links` empty, and `failing` `{"kind": "empty", "index": 0}`.
Otherwise every hop is checked by §10.2 in order, and every link `k` = 1
… `m` is decided by §10.4 in order, a link being **undecided** (`null`)
when either of its two grants failed check 1, and `null` also where §10.4
answers **unknown**, one `null` covering both. Then, in this order, the
first that applies is the chain's **failing** point: `g_0` passed check 1
and `g_0.body` has a member with key `upstream`, whatever its value: kind
`incomplete`, index 0, token `CHAIN_INCOMPLETE`; some link `k` is false: kind `link`, the smallest such
`k`, token `CHAIN_LINK`; some hop `k` has verdict `FAIL`: kind `hop`, the
smallest such `k`, token `null`. The **chain verdict** is decided in this order: `FAIL` when a
failing point exists; else `GREEN` when every hop is `GREEN` and every
link is true; else `PARTIAL`. A chain whose only failing point is
`incomplete` is `FAIL` though every hop be `GREEN`: a payload that does not
start at a grant stating no upstream proves nothing about what stands above
it. The result is a canonical object with
exactly: `verdict`; `hops`, the array of §10.3 results in hop order;
`links`, the array for `k` = 1 … `m` of `true`, `false`, or `null`;
`token`, the token named above or `null`; and `failing`, `null` or
`{kind, index}`.

## 11. Tokens and vocabularies

**Document tokens** (closed; one per rejection, by the orders of §4.2,
§5.2, §6.1, §6.2, §7.1, §7.4): the `zikaron/1` §3 tokens; `E_DOC`, `E_DOC_MISSING`,
`E_DOC_CLOSED`, `E_SPEC`; `E_FPM_AUTHOR`, `E_FPM_WORK`, `E_FPM_GRANT`,
`E_FPM_ROWS`, `E_FPM_ROW`, `E_FPM_DUP_RECIPIENT`, `E_FPM_DUP_VARIANT`,
`E_FPM_ROW_ORDER`, `E_FPM_NOTE`; `E_ACK_RECIPIENT`, `E_ACK_FPM`,
`E_ACK_VARIANT`, `E_ACK_NOTE`; `E_SIG_FORM`, `E_SIG_V`, `E_SIG_RANGE`,
`E_SIG_HIGH_S`, `E_SIG_RECOVER`, `E_SIG_SIGNER`; `E_BADGE_PREFIX`,
`E_BADGE_CAP`, `E_BADGE_B64`, `E_BADGE_ENTRY` (carrying `k`, and under §6.2 a `zikaron/1` §10 token beside it), `E_BADGE_TYPE`, `E_BADGE_INCOMPLETE`, `E_BADGE_LINK`;
`E_KIT_UNREADABLE`, `E_KIT_MANIFEST_ABSENT`, `E_KIT_MANIFEST`,
`E_KIT_ENTRY_BYTES`, `E_KIT_FILE`, `E_KIT_PROOF_BYTES`, `E_KIT_EXTRA`.

**Result vocabularies** (closed; the values the [D] and reading predicates
yield): pairing `PAIRED`, `FPM_INVALID`,
`ACK_INVALID`, `ACK_FPM_MISMATCH`, `ACK_NO_ROW`, `ACK_VARIANT_MISMATCH`;
attribution `ATTRIBUTED`, `NOT_ATTRIBUTED`; badge `BADGE_OK`; kit `KIT_OK`;
check states `PASS`,
`FAIL`, `UNKNOWN`; check tokens `BAD_SIG` (with reasons `NOT_A_GRANT` or a
`zikaron/1` token), `BROKEN_LEDGER`, `NOT_IN_LEDGER`, `UNANCHORED`,
`EXPIRED`, `REVOKED`; verdicts `GREEN`, `PARTIAL`, `FAIL`; chain tokens
`CHAIN_INCOMPLETE`, `CHAIN_LINK`, `CHAIN_EMPTY`; chain failing kinds
`empty`, `incomplete`, `link`, `hop`; link values `true`,
`false`, `null`; depth `valid` false; kit manifest rule names `canonical`,
`members`, `spec`, `root`, `entries`, `files`, `contents`, `proofs`,
`note_md`.

## 12. Outside the grammar, and the openings by name

- **Chains and clocks.** This law consults no chain and reads no clock;
  the audit input carries what the chain said, and `now` carries what the
  caller trusts of the time. Proof files inside a kit are read by the
  parent's chain-facing rule with the reader's own trust (§7.5). Which root is the issuer's is the terms' statement:
  a reader who audits under another root reads another ledger, and the
  analysis of §10 shows that a ledger a stranger assembles around an
  author's key reaches `PARTIAL` at best, never `GREEN`, unless every
  anchor of every key in its lineage is in hand.
- **An unanchored entry binds nobody who did not see it.** Check 6 acquits
  on a `COMPLETE` record, and a revocation the issuer never anchored is
  outside every anchor set; likewise a succession never anchored leaves
  the successor's keys outside every basis a stranger can declare. The
  doctrine's answer is to anchor revocations and successions, and this
  law states the limit by name.
- **A retired key's stray anchor caps the seat at `PARTIAL` while the seat
  publishes the succession that admitted it.** A counted anchor of a hash
  whose bytes are never produced is a `MISSING` row (`zikaron/1` §8.5), so
  the record is not `COMPLETE`, check 6 stays `UNKNOWN`, and check 4 stays
  `UNKNOWN` unless the grant is anchored. The seat's one escape is the
  parent's own: withholding the succession that named the retired key
  shrinks the lineage, the record leaves `MISSING` for `DISCARDED`
  (`zikaron/1` §8.1), and the label can be `COMPLETE` again. No check of §10
  reads `DISCARDED`, so a `GREEN` here says that under the pile the reader
  was handed every check passed, and never that the pile is the whole
  ledger. A reader who wants that silence closed reads the parent's
  `DISCARDED` list beside the result and asks for the succession that would
  admit each sender it names.
- **A verdict reaches as far as its basis.** The parent scans what a
  basis declares and nothing else; a grant `GREEN` under a basis of one
  chain may be revoked on another. Which chains a reader should declare
  is the terms' statement, and the result carries the basis it was read
  under.
- **Terms.** What a grant permits, whether its window's ends mean what a
  party hoped, exclusivity, and what a manifest's rows oblige: the terms
  and the forum.
- **Variants.** How the per-recipient bytes are made, and whether they
  are distinguishable: the author's craft.
- **Weighing.** What a depth reading is worth, what a kit proves beyond
  its bytes, and how a badge's issuer is trusted: the reader's.
- **Platforms.** A kit path this law admits may still be one a
  filesystem cannot hold (a reserved device name on one platform); the
  reader there reports `E_KIT_FILE` or `E_KIT_UNREADABLE`, and the law
  names no platform; a filesystem that refuses a name outside UTF-8 makes
  §7.1's last failure unreachable there, and a reader on such a platform
  witnesses it nowhere, so that failure is outside §13.1's corpus wherever
  the corpus is built on such a filesystem, exactly as proof verification
  is outside it (§7.5).
- **Rendering.** How checks and states are shown, beyond the rules that a verifier shows every check's
  state, `GREEN` alone is green, `PARTIAL` is its own state, a reader who
  shows an `earliest` shows the basis beside it, and a payload yielding a
  token shows nothing.
- **Payment and escrow.** Not here.

## 13. Freeze and evolution

**13.1** The reference implementation freezes when an independently
written implementation, produced from this text and a conforming
`zikaron/1` core alone, agrees with it on every input of a generated
corpus (every document token and every accepted document's `doc_id`, every
pairing and attribution verdict, every badge encoding and every badge
decoding, every kit verdict with its subject, its kit id, its counts and its
`invalid_entries` rows with their tokens, every depth reading, every check
result, every chain result), across at
least one hundred thousand fuzz samples, as `zikaron/1` §12.1 asks of its
own core, with zero panics, hangs, nondeterministic answers, or wrong
accepts under fuzzing, and with every closed boundary in this
text witnessed on both sides by at least one corpus input, the boundary
list being a precondition on the corpus and never a property claimed of it
afterwards. The boundary is witnessed under every seed the corpus is
generated with. A closed boundary of this text is a boundary the corpus can
witness on the surface §13.2 compares; a rule with no reachable failing
side is witnessed once and states no boundary in this sense, and §7.5 and
§12 name the two failures that lie outside the corpus.

**13.2** Once §13.5 names a release digest, agreement with the frozen core
on the corpus is the test anyone can run in seconds, and it is evidence of
conformance and never its definition: conformance is agreement on every
input, and an input outside the corpus on which a verifier differs from the
core is a non-conformance the corpus did not catch and a reason to grow it.

**13.3** `zikaron.kit/2` is a new spec id in every document and new domain
literals; a document accepted under `zikaron.kit/1` is accepted under
`zikaron.kit/1` forever, and a later law reads it by this one.

**13.4** This law and `zikaron/1` are two laws: a change to one is never a
change to the other, and each names its own core. This law's core embeds
the `zikaron/1` core of release digest
`0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc`
(named in that law's §12.5 on 2026-09-05); a kit core built on any other
`zikaron/1` core is another core.

**13.5 The core's identity.** The release digest of the `zikaron.kit/1`
core is

    0x3f8368ebc8b5b7c97e4b5c4240f57f6fc06421b4effbd5a7446d128434a20535

named on 2026-09-05: the pure Python implementation in
`zikaron-conformance/kit-py/` (four source files: `zkk.py`, `zkkdoc.py`,
`zkkkit.py`, `zkkread.py`) over the `zikaron/1` core §13.4 names, whose
release digest `zikaron-conformance/criterion-digest-kit.sh` computes; the
kit corpus, `HARNESS-KIT.md`, and the review record beside it say how it
converged. Naming a digest here is the freeze, and it is the only act that
performs it; a change to any of the four files is a new core and a new
digest named by the same act.
