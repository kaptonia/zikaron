# ZIKARON kit grammar v2 (`zikaron.kit/2`)

The client's document and reading law: the second, smaller law a ZIKARON
client needs beyond the wire law, so that two clients agree byte for byte on
the documents they exchange and on the verdicts they show. Where
[`zikaron/2`](zikaron-v2.md) says what an entry is and what a ledger's audit
says, `zikaron.kit/2` says what a badge payload is, what a record kit is, in
a directory or in one file, what a grant package is, and whether a grant is
good to rely on now. It is self-sufficient in the same sense as its parent:
every predicate a verifier needs is stated here in full, and it consumes
`zikaron/2`, and through it `zikaron/1`, only through the `zikaron/2` law's
frozen core.

**Status.** The law is the set of total predicates in this text. A
conforming implementation embeds the `zikaron/2` core of the release digest
named in §10.4, and agrees with the core named in §10.5 of this text on
every input; until that core is named this text is the criterion. Evolution
goes through `zikaron.kit/3`.

**Rule classes.** Three classes appear, and every rule below is marked:

- **[D]** document predicates: decided offline from the byte strings the
  predicate is given (one document, several documents, or the enumeration of
  a directory), and nothing else.
- **[R]** reading predicates: decided offline from documents together with
  one or more `zikaron/2` audit inputs (the three inputs of `zikaron/2` §8)
  and, where stated, a notice input (§6.3) and a caller-supplied `now`. No
  predicate of this law consults a chain or a clock; what a chain said is
  inside the audit input and the notice input, and what time it is comes
  from the caller.
- **[X]** chain-facing rules: how a notice input is read from a chain
  (§6.3). They are the work of the reader that builds the input, as the
  completeness rule of `zikaron/2` §9.4 is for an anchor set, and the freeze
  of §10 reaches them only through the inputs they produce.

## 1. Notation

`zikaron/2` §1 applies: byte strings, `hex20`, `hex32`, `hex65`, tokens,
skeleton and prose strings, `int`, `bool`, bytewise order, `sha256`,
`keccak256`, secp256k1 and its order `n`, UTF-8, Unix seconds. In addition:

- **`doc_id(b)`** = `sha256(b)` for any byte string `b`: identity is total
  here as in `zikaron/2` §2.1.
- **`accept(b)`**: the acceptance of a byte string as an entry, decided by
  the embedded core, yielding the entry or a rejection token: where `b`
  passes `zikaron/2` §4.3 steps 1 to 3 and its `spec` is byte-equal to
  `zikaron/1`, `accept` is `zikaron/1` §4.3, decided by the `zikaron/1` core
  the `zikaron/2` core embeds (`zikaron/2` §12.6), and otherwise it is
  `zikaron/2` §4.3; the token is that law's §10 token, the two laws spelling
  one closed set of tokens. **`canonical(b)`**: `zikaron/2` §3.5 alone.
- **`audit(I)`**: the `zikaron/2` audit (`zikaron/2` §8) of an audit input
  `I`, decided by the embedded core. It yields the report of `zikaron/2`
  §8.6 and the **ledger**: the ledger entries of `zikaron/2` §8.1 of the
  segment that law reads (the whole input where the report's `continues` is
  `null`, the new segment of `zikaron/2` §12.6 otherwise) and, where the
  report's `continues` is not `null`, the ledger entries of `zikaron/1` §8.1
  of the old segment, each with its `seq`, `prev`, `author`, `entryType`,
  `body`, `entry_id`, and the findings of its own segment's report that name
  it. An input on which the core yields the `NO_LABEL` object of `zikaron/2`
  §8.6, under §9.4 or because what it was handed is no audit input at all,
  is an **invalid input**; every reading predicate names its outcome for
  one.
- **`now`**: an `int` of Unix seconds supplied by the caller, or `null`
  when the caller has no trusted time.
- **`base64url`**: RFC 4648 §5 with no padding characters, and canonical:
  the unused trailing bits of the last character are zero.
- **sort keys**: where a rule of this law sorts by a key that is an `int`,
  the comparison is numeric ascending; where the key is a string, it is the
  bytewise order above; a sort over several keys compares them in the order
  the rule lists them.

## 2. Canonical form

**[D] 2.1** Every document of this law, which is the kit manifest (§4.3)
alone, is a canonical byte string under `zikaron/2` §3, decided by
`canonical(b)`; the rejection tokens of `zikaron/2` §3 apply unchanged. The
directory manifest of §4.5 is a value a verifier computes and never a
document it is handed: its bytes are those `zikaron/2` §3.4 writes for that
value, its `path` strings being prose, and no predicate decides them by
`canonical`. Every result object a reading or a check yields (§5.3, §7.3,
§7.5) is written in the same canonical form, though no predicate of this law
decides one. A badge payload (§3) is ASCII text and no such document: §3.2
decides it in full and never calls `canonical`; a one-file kit (§4.1a) is a
byte layout of its own and no such document either. A document's published
bytes are its canonical bytes and nothing else (`zikaron/2` §2.2).

## 3. Badge payload

A badge carries a grant, and the chain of grants above it, as one text a
customer can scan and a verifier can decode without any other input.

**[D] 3.1 Encoding.** A payload is the ASCII text `zikaron-grant:` followed
by one or more **segments** joined by `.`, in order from the original
author's grant to the grant displayed. Each segment is `base64url` of the
canonical bytes of one entry `accept` yields (§1) of type `grant`, under
either law. The payload's total length in bytes is at most `BADGE_CAP` =
2953, the capacity of a version 40 QR code in byte mode at error correction
level L, so that every lawful payload has a QR rendering. **`encode(e_0 …
e_m)`**, over one or more byte strings in order, yields the payload or one
token, decided in this order: for each `k` in order, before the next is
examined, `e_k` fails `accept`: `E_BADGE_ENTRY`, carrying `k`; its entry is
not of type `grant`: `E_BADGE_TYPE`, carrying `k`. Then the payload the rule
above builds is longer than `BADGE_CAP` bytes: `E_BADGE_CAP`. Otherwise the
payload. Whether segment 0 states no upstream and whether the byte links of
§3.3 hold are read by §3.2 and never here: an encoder assembles what it is
handed, and a payload it yields may still fail §3.2 steps 4 and 5.

**[D] 3.2 Decoding, total.** `decode(p)` yields `BADGE_OK` with the list of
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
      the `zikaron/2` §10 token;
   c. its entry is not of type `grant`: `E_BADGE_TYPE`, carrying `k`.
4. Segment 0's grant's `body` has a member with key `upstream`, whatever
   its value (`zikaron/2` §6's presence rule: a key present is a member
   present, `null` included): `E_BADGE_INCOMPLETE` (the payload does not
   start at a grant that states no upstream).
5. For each `k` ≥ 1 in order, the byte link of §3.3 between segments
   `k − 1` and `k` fails: `E_BADGE_LINK`, carrying `k`.
6. Otherwise `BADGE_OK` with the list.

**[D] 3.3 Byte link.** Between an upstream grant entry `u` and a downstream
grant entry `d`: the byte link **holds** iff `d.body` carries a member
`upstream` whose value is a string byte-equal to `hex32(entry_id(u))`, and
`d.body.work` = `u.body.work`. `upstream` is an optional field of a
`zikaron/2` grant (`zikaron/2` §6.3), whose form that law tests and whose
meaning it leaves to readers, and an extra member of a `zikaron/1` grant
(`zikaron/1` §6.10); this law reads it under either law as the downstream
party's own statement of what it holds. Whether `d`'s issuer is `u`'s
grantee is a question of ledgers, decided by §7.5 and never by bytes alone.

A payload of one segment is a grant that claims no upstream; a payload of
several is a chain whose byte links hold. Whether each grant is good now is
§7, which needs the issuers' ledgers; the payload alone proves only what
its bytes say.

## 4. Record kit

A record kit hands a reader the bytes that match a ledger's anchors:
entries, the files they digest, and their cross-references, sealed by one
canonical manifest so that the reader's verification is a pure function of
an enumeration, which a directory yields by a walk (§4.1) and one file
yields by its layout (§4.1a). A kit is a pile in the sense of `zikaron/2`
§8: it may carry bytes that fail acceptance, and verification reports them
without failing.

**[D] 4.1 Enumeration.** A reader turns a directory into an **enumeration**,
the set of pairs `(path, bytes)` for every regular file under it, by a
**walk**; the **path** of any entry the walk examines is the sequence of
listing names from the kit directory to it joined by `/`, with no leading
and no trailing `/`, and the enumeration carries that path for a regular
file with its bytes. The kit directory is the directory the reader was
handed, its name resolved as the platform resolves it, through a symbolic
link where the name is one; the walk examines the entries its listing yields
and their descendants, and the kit directory itself for its listing alone.
The walk: list the directory, discard the listing entries named `.` and
`..`, examine the rest in bytewise order of their names, and for each entry
in that order either read it in full (a regular file), or walk it before
moving to the next name (a directory), without following symbolic links. The
walk fails at the first entry it examines that is a symbolic link, a
directory that cannot be listed, a file that cannot be read in full, neither
a regular file nor a directory, or named by bytes that are not valid UTF-8;
`verify_kit(dir)` then yields `E_KIT_UNREADABLE` carrying that entry's path,
the one-character string `.` where the failing entry is the kit directory
itself, and the path of the directory that listed it where the entry's own
name is not valid UTF-8, that path being the same one-character string `.`
where the listing directory is the kit directory, since a path this law's
output cannot spell names nothing; the walk examines nothing else. Otherwise
`verify_kit(dir)` is §4.4 over the enumeration.

**[D] 4.1a One-file form.** A kit may travel as one byte string: the 18
bytes `zikaron-kit-file/1` followed by one line feed, then one or more
items, each the item's kit path (§4.2), a line feed, the item's byte count
in decimal with no sign and no leading zeros (zero written `0`), a line
feed, the item's bytes, and a line feed; `manifest.json` is the first item,
and the items after it follow in strictly ascending bytewise order of their
paths among themselves, none of them at `manifest.json`, so that no two
items share a path; the byte string ends after the last item's trailing line
feed. The **enumeration** of such a byte string is the set of its `(path,
bytes)` pairs, and `verify_kit` over it is §4.4 over that enumeration. A
byte string that departs from this layout at any byte, that holds no item,
whose first item is not `manifest.json`, any of whose items has a path that
is not a kit path, whose items are out of order or repeat a path, or whose
declared count runs past its end, yields `E_KIT_UNREADABLE` carrying as its
subject the path of the item being read where that item's path line was read
in full and is a kit path, and the one-character string `.` otherwise;
`E_KIT_MANIFEST_ABSENT` is unreachable in this form. A regular file the
reader was handed as a kit and cannot read in full yields `E_KIT_UNREADABLE`
with subject `.`. The layout carries no digest of its own: the manifest
inside is the kit's seal, as for a directory.

**[D] 4.2 Kit paths.** A kit path is a skeleton string of one or more
segments joined by `/`; each segment is one to 255 bytes from `a-z0-9._-`,
is neither `.` nor `..`, and does not begin with `-`; the whole path is at
most 1024 bytes and begins with no `/`. Kit paths are compared bytewise and,
being lowercase ASCII, collide on no case-folding or normalizing filesystem.
A path in `files` names `files/<path>`.

**[D] 4.3 Manifest.** The enumeration must hold a pair at path
`manifest.json` whose bytes are a canonical object with exactly these
members, the two marked optional present or absent:

| key | form | rule |
|---|---|---|
| `spec` | string | byte-equal to `zikaron.kit/2` |
| `root` | `null` or hex20 | the ledger the kit's author says the entries belong to, or `null`; data for readers |
| `entries` | array | hex32 elements, distinct, sorted bytewise; may be empty |
| `files` | array | elements each an object with exactly the members `path` (a kit path), `sha256` (hex32), `size` (int), and, optionally, `name_md` (prose string: the name the file had before it was made a kit path, or, for a file delivered under a directory original, its path relative to that directory with segments joined by `/`; a producer omits it where the kit path spells the name, and no rule tests this); paths distinct; no listed path followed by `/` is a prefix of another listed path, so that a file and a directory never share a name; sorted by `path` bytewise; may be empty |
| `contents` | array | elements each an object with exactly the members `content` (hex32), `path` (a kit path), and, optionally, `name_md` (prose string; data for readers, read by no predicate of this law: for a directory original, the name the directory had before it was made a kit path, one name and never a path; a producer omits it where the last segment of its `path` spells the name, and on a file original, whose `files` row carries the name, a `contents` row's `name_md` there naming nothing (§9)); `path` is either a path listed in `files`, whose listed `sha256` must equal `content` (a **file original**), or a path not listed in `files` that, followed by `/`, is a prefix of at least one listed path (a **directory original**, whose manifest §4.4 recomputes and §4.5 spells), the `files` rule above making the two readings exclusive; no directory original's `path` followed by `/` is a prefix of another directory original's `path`, so that a file lies under at most one directory original; for each directory original, the names §4.4 step 5 gives the rows of `files` under it are distinct, each is one or more non-empty segments joined by `/` none of which is `.` or `..`, and no name followed by `/` is a prefix of another, so that the names spell a directory (a name's charset is unrestricted, and what a recipient's file system makes of a name is the recipient's); no two rows share both `content` and `path`, whatever their `name_md`; sorted by `content` then `path`; may be empty |
| `anchoredOn` | optional; object | exactly the members `chainId` (int), `registry` (hex20), and `fromBlock` (int): where the kit's author says its entries are anchored; data for readers, read by no predicate of this law |
| `anchors` | optional; array | elements each an object with exactly the members `entry_id` (hex32, an element of `entries`), `chainId` (int), and `tx` (hex32): the transactions the kit's author says anchor its entries, one row per anchor the author chooses to name; rows distinct; sorted by `entry_id`, `chainId`, `tx`; data for readers, read by no predicate of this law, so that a reader of another law that takes an entry with the chain and transaction that anchor it (a court's evidence list, say) finds them in the manifest beside the root and the bytes, and verifies them against the chain by that law's own rules |
| `note_md` | prose string | may be the empty string |

Every member list this section spells is closed, the manifest's own and its
rows' elements alike: an element that is not an object, or that carries a
member beyond its row's list, fails its row's rule. The manifest's rules are
decided in this order, and the first failure names its **rule** as the
subject: `canonical` (the bytes fail `canonical`; which `zikaron/2` token
they fail on reaches no output of this law, one rule name being the whole of
the subject), `members` (the root is not an object, or its member set lacks
one of the six required keys or carries a key beyond the eight), then the
rows of the table in order, `spec`, `root`, `entries`, `files`, `contents`,
`anchoredOn`, `anchors`, `note_md`, each row's tests in the order its rule
states them, an absent optional row testing nothing. The kit's identity is
`doc_id` of the manifest's bytes.

**[D] 4.4 Verification.** `verify_kit(enumeration)` yields one verdict,
decided in this order; the first failure is the verdict and carries its
subject:

1. No pair at `manifest.json`: `E_KIT_MANIFEST_ABSENT`.
2. Its bytes fail §4.3: `E_KIT_MANIFEST` with the failing rule as its
   subject.
3. For each id in `entries` in order: no pair at `entries/<id without
   0x>.zk1`, or that pair's `doc_id` is not the id: `E_KIT_ENTRY_BYTES` with
   the id.
4. For each row of `files` in order: no pair at `files/<path>`, or the
   pair's bytes have another `sha256` or another length: `E_KIT_FILE` with
   the row's `path`.
5. For each row of `contents` in order whose `path` is a directory original:
   the directory manifest of §4.5, recomputed from the rows of `files` whose
   `path` begins with the row's `path` followed by `/`, each named by its
   `name_md` where present and by its `path` with that prefix removed
   otherwise, has a `sha256` of its canonical bytes other than `content`:
   `E_KIT_CONTENT` with the row's `path`.
6. Any pair whose path is not in the **named set**: `manifest.json`,
   `entries/<id without 0x>.zk1` for every listed id, and `files/<path>` for
   every listed file path; `E_KIT_EXTRA` with the bytewise-smallest such
   path.
7. Otherwise `KIT_OK`, carrying the kit's id (§4.3), the counts of entries
   and files, and the list `invalid_entries`, in `entries` order, of listed
   ids whose bytes fail `accept`, each with its token (informational: a kit
   may carry an anchored byte string that is not an entry, which is exactly
   what a `MISSING` row asks to see).

A file original's row is decided by §4.3's manifest rules, its `sha256` by
step 4, and a directory original's row by step 5, so `KIT_OK` says of every
`contents` row that its original is in the kit and matches its `content`. A
directory that holds only `manifest.json` with empty arrays is a kit that
proves nothing, and `KIT_OK` says so through its counts.

**4.5 Meaning.** A kit proves that its bytes are the bytes its manifest
names, to any reader whose walk or reading yields the same enumeration; a
reader that cannot hold a file (`zikaron/2` §11) reports `E_KIT_UNREADABLE`
for it, and that is the reader's limit, named in §9. Whether the entries are
a ledger's, whether they are anchored, and what they are worth, are the
reader's audit (`zikaron/2` §8) over the kit's entries as a pile, and the
reader's weighing; a kit is an input to those, never their conclusion. An
author's self-verification before export is the same function over the same
enumeration. A `contents` row pins the convention that its `content` is the
`sha256` of the bytes at its `path` (a file original), or the `sha256` of
the bytes `zikaron/2` §3.4 writes for a **directory manifest**: the object
`{"files":[…]}` whose one member `files` is an array of objects each with
exactly `path` and `sha256`, one per regular file under the recorded
directory, `path` the file's names from that directory joined by `/` as a
prose string, `sha256` the hex32 of its bytes, the rows sorted by `path`
bytewise, the names being distinct (§4.3); symbolic links and everything
that is not a regular file are left out. A kit delivers a recorded directory
as one file per row, each under `files/<directory path>/<kit path>`, under
one `contents` row naming the directory path (§4.3), and §4.4 recomputes the
manifest from them with the names their `files` rows' `name_md` restore. The
directory's own name travels as that `contents` row's `name_md`, read by no
predicate of this law, so that a reader lands the directory under the name
it had and its files under theirs. A work digested another way (a mode that
names its own function) is delivered with no `contents` row, and the mode
says how to check it. A file is tied to an entry through the digest alone: a
`contents` row's `content` is what a history entry's `content` or a grant's
`work` names, and the reader joins them by that value.

## 5. Grant package

A grant package is a kit that carries a grant's badge payload, the terms it
hashes, the issuer's publish address, and the originals the grant is over,
so that a recipient verifies the grant and takes delivery of the work from
one file.

**[D] 5.1 Form.** A grant package is a kit (§4) whose enumeration holds,
under `files/`, `zikaron-grant.txt` (a badge payload, §3), and may hold
`terms/<path>` (the terms documents the chain's grants hash) and
`publish.txt` (by convention one https address, the issuer's publish
address; no step reads it); whose `entries/` hold every hop of the payload
and may hold the ledgers of the hops' issuers, a package carrying, for a hop
whose grant carries a revocation member (§7.1), that hop, the `history`
entry its `history` names where the package's author holds it, and the
genesis, successions and migrations of that issuer's ledger, of its old
segment too where it continues a `zikaron/1` ledger (`zikaron/2` §12.6), and
no other entry of that ledger, since §7 reads such a grant's standing from
the chain the member names and its withdrawal from the notices of §6.3, and
never from that ledger's other entries; whose `contents` rows, where
present, name the delivered originals; and whose manifest's `root` names the
ledger root of the first hop's issuer, as the terms name it, which the first
hop's `author` need not equal once that ledger has changed hands
(`zikaron/2` §7.3); the root of every later hop's issuer is the preceding
hop's `grantee` (§7.4). Each hop is read under its own root over the whole
of `entries/`, entries of other ledgers standing excluded there.

**[D] 5.2 Verdict.** `verify_grant_package(enumeration)` yields one verdict,
decided in this order, the first failure carrying its subject:

1. `verify_kit` (§4.4) is not `KIT_OK`: its verdict, with its subject.
2. No pair at `files/zikaron-grant.txt`: `E_GP_NO_CODE`.
3. `decode` (§3.2) of its bytes is not `BADGE_OK`: the badge token, with the
   index it carries where it carries one.
4. The manifest's `root` is `null` and the first hop's `body` carries no
   member `ledger` whose value is hex20: `E_GP_ROOT`.
5. Some hop of the payload is the `entry_id` of no pair under `entries/`:
   `E_GP_UNCARRIED`, carrying the first such hop's `entry_id` in payload
   order.
6. Some `contents` row's `content` differs from the hops' `body.work` (one
   value across the payload, §3.3): `E_GP_CONTENT`, carrying the first such
   row's `path` in `contents` order.
7. Some pair under `files/terms/` has a `sha256` of its bytes that equals no
   hop's `body.terms`: `E_GP_TERMS`, carrying the bytewise-smallest such
   pair's path written without the `files/terms/` prefix (a terms document
   digested under another function travels outside `files/terms/`, and the
   terms say how to check it).
8. Otherwise `GP_OK`, carrying the kit's id, the first hop's root (the
   manifest's `root`, or the first hop's `ledger` where that is `null`), the
   hops in order, each `contents` row's `content` and `path` in `contents`
   order, and, for each hop, the paths under `files/terms/` whose bytes'
   `sha256` is its `body.terms`.

Whether each hop is good now is §7: the first hop under the root `GP_OK`
carries and each later hop under the preceding hop's `grantee`, over the
piles the package carries in `entries/` and the reader's own inputs; a
package proves what its bytes say and nothing about revocation.

**[D] 5.3 Result form.** `GP_OK` is written as the canonical object with
exactly `verdict` (`GP_OK`), `kit_id` (hex32), `root` (hex20), `hops` (the
hops' `entry_id`s in payload order), `contents` (one object per row of the
manifest's `contents`, in that order, with exactly the row's `content` and
`path`), and `terms` (one element per hop in order, each the array, in
bytewise order, of the paths of the pairs under `files/terms/` whose bytes'
`sha256` is that hop's `body.terms`, each path written without the
`files/terms/` prefix); a failure is the canonical object with exactly
`verdict` (the token) and `subject`: the kit verdict's subject, the badge
token's index, the hop's `entry_id`, the row's `path`, or the pair's path
under `files/terms/` written without that prefix, and `null` where the step
names none. The paths in `terms` and the subject of `E_GP_TERMS` are written
without the `files/terms/` prefix; every other path a result carries, the
subject of a kit verdict included, is written as the step that names it
spells it.

## 6. Reachability and anchoring, as read

Two definitions the reading predicates share, over the ledger and the
audit input's anchor records.

**[R] 6.1 Reachable.** Over the ledger, let `F → F'` hold iff `F.prev` is
the `entry_id` of ledger entry `F'`, or `F` is a ledger entry at `seq` 0 of
the new segment whose `body.continues.entry_id` is the `entry_id` that the
report of the audit the predicate reads (`I'` in §7) carries in `continues`,
and `F'` is the ledger entry that `entry_id` names (`zikaron/2` §9.5, which
bounds through every genesis naming the closing succession: every such
ledger entry commits in its signed bytes to that succession, whatever its
`continues.root` and whether or not §12.6 selected it; where two of them are
ledger entries the parent law reads a fault there, the equivocation of
`zikaron/2` §8.4 or the root mismatch of §8.2, so wherever check 4 reads the
edge at most one ledger entry at `seq` 0 carries it); a member named
`continues` on any other entry, the genesis of an uncontinued root and an
extra member of either law's §6.10 among them, is data and no edge, as the
parent law reads it. Ledger entry `E` is **reachable** from ledger entry `F`
iff `E` = `F` or `(F, E)` lies in the transitive closure of `→`. The ledger
is finite, so the closure is finite; reachability follows `prev` through
ledger entries only and stops where no ledger entry matches (`zikaron/2`
§9.5).

**[R] 6.2 Anchored.** The **counted anchors** are the anchor records of the
audit input, read from its anchor set and never from the rows of either
report, that survive the trimming of `zikaron/2` §8.1 in the segment that
law reads and, for a continued ledger, those that survive the trimming of
`zikaron/1` §8.1 in the old segment (the records of each segment under that
segment's own trim, a record of the old segment by a key outside the
`zikaron/1` lineage anchoring nothing), and carry verdict `counted`. Ledger
entry `E` is **anchored** iff some counted anchor has `hash` = the
`entry_id` of a ledger entry `F` from which `E` is reachable. The **bound**
of `E` is the smallest `blockTimestamp` among such records, or `null` when
`E` is not anchored; across chains the smallest timestamp is the bound.
`zikaron/2` §9.2 gives one bound per chain and §9.5 states no minimum across
them, leaving a single figure to the reader who says which chains it
weighed; this law's reader weighs every chain the basis of `I` declares, and
the bound is relative to that basis. This is `zikaron/2` §9.5 read as a
predicate: an anchor of a later entry bounds every earlier entry on its line
and none off it.

**[R] 6.3 Notice input.** A **notice input** `N` is `null`, the caller
holding no notices, or a value of `zikaron/2` §3's universe that is an
object with exactly the members `basis`, `revocations`, and `accessions`:

- `basis` is an object with exactly the members `chainId` (int), `fromBlock`
  (int), `toBlock` (int), `revocationContract` (hex20), `book` (hex20), and
  `bookFromBlock` (int), with `fromBlock` ≤ `toBlock` and `bookFromBlock` ≤
  `toBlock`: the one chain the input was read from, the contract whose logs
  of the notice form were read over the blocks `fromBlock` to `toBlock`
  inclusive, and the succession book whose logs of the accession form were
  read over the blocks `bookFromBlock` to `toBlock` inclusive.
- `revocations` is an array of **notice records**, each an object with
  exactly the members `chainId` (int, equal to the basis's), `blockNumber`
  (int, from `fromBlock` to `toBlock` inclusive), `txIndex` (int, the
  transaction's index in its block), `logIndex` (int, the log's index among
  the logs of its block, as a node reports it), `tx` (hex32), `sender`
  (hex20), `grant` (hex32), and `verdict` (one of the three tokens of
  `zikaron/2` §9.3).
- `accessions` is an array of **book records**, each an object with exactly
  the members `chainId` (int, equal to the basis's), `blockNumber` (int,
  from `bookFromBlock` to `toBlock` inclusive), `txIndex` (int) and
  `logIndex` (int) as a notice record's, `holder` (hex20), `to` (hex20), and
  `root` (hex20).

A record's **position** is `(blockNumber, txIndex, logIndex)`, compared
member by member numerically; two records of one array at one position are
one log spelled twice, and an input holding them is no notice input. Every
member named here must carry the form named for it, and `zikaron/2` §6.10
reaches no object of this section: an object bearing a member beyond its
list is no notice input. An `N` that is not `null` and fails this paragraph
is an **invalid notice input**, and every reading predicate names its
outcome for one; neither array carries an order that reaches any output, and
the input has no root, so one input serves every grant read on its chain.

**[X]** A notice record records a log of the **notice form**: a log in the
receipt of a transaction with status 1, whose `address` field, the account
whose storage context executed the emitting instruction as `zikaron/2` §9.1
reads it, is the basis's `revocationContract`, carrying exactly three topics
and empty data, topic 0 byte-equal to
`keccak256("Revoked(address,bytes32)")`, topic 1 the transaction's sender's
20 bytes preceded by 12 zero bytes, and topic 2 a 32-byte value that also
appears as 32 consecutive bytes of the transaction's calldata, lying wholly
within it and beginning at an offset that is a multiple of 32 or 4 plus a
multiple of 32, `zikaron/2` §9.1 fixing what the transaction's sender and
calldata are, so that a creation transaction, a transaction without a
recoverable signature and one whose signature names no chain carry no such
log; the record's `tx` is the transaction's hash, its `sender` the
transaction's sender, its `grant` topic 2, and its `verdict` the codeless
test of `zikaron/2` §9.3 asked of the sender at the log's block, `counted`,
`UNPROVEN` or `VOID` as that section gives it, a log whose topic 1 is not
the sender or whose topic 2 lies at no such offset being no notice whatever
contract emitted it. A book record records a log of the **accession form**:
a log in the receipt of a transaction with status 1, whose `address` field,
read the same way, is the basis's `book`, carrying exactly four topics and
empty data, topic 0 byte-equal to
`keccak256("Acceded(address,address,address)")` and topics 1, 2 and 3 each
20 bytes preceded by 12 zero bytes, the record's `holder`, `to` and `root`
in that order. A conforming notice input carries a record for every log of
the notice form in the basis's blocks and a record for every log of the
accession form in the book's blocks, and no other record, its `fromBlock` at
or before the first block at whose end the address `revocationContract`
carried code and its `bookFromBlock` at or before the first block at whose
end `book` carried code, any range serving for an address that carried none
by `toBlock` and a range from block 0 serving in every case, so that no
notice and no accession lies below what was read; a reader that did not
obtain every such log, or whose ranges begin above those blocks, has not
read the basis and produces no notice input under it, handing §7 `null`
instead, as `zikaron/2` §9.4 says of a scan it could not complete. The law
reads the two forms and never a contract's code: which contract a reader
names in `revocationContract`, which in `book`, and the block each was
deployed at are the reader's declarations, as `registries` are in
`zikaron/2` §9.4, and a grant's own `revocation` member (§7.1) says which
declaration applies to it.

**[R] 6.4 In office, as the book says.** Under a root `r` and a notice input
`N` that is neither `null` nor invalid, the **book lineage** of `r` is the
sequence whose first member is `r` and whose later members are the `to` of
each book record of `N` whose `root` is `r`, in ascending order of position.
The key **in office at a position** `p` is the `to` of the last, in
ascending order of position, of the book records of `N` whose `root` is `r`
and whose position is less than `p`, and `r` where there is none; the key
**in office at a block** `b` is the `to` of the last, in that order, of the
book records of `N` whose `root` is `r` and whose `blockNumber` is less than
`b`, and `r` where there is none, so that a record in block `b` itself moves
nothing at `b`, and a reader with an anchor record, which carries a block
and no position (`zikaron/2` §9.2), decides by the block alone. The book
lineage is read from the records as the input carries them and tests nothing
about them: a `holder` that is not the key in office before it, a `to` seen
before, or a `root` no record enters are the declared book's affair, and
this law orders what the book the reader named said. What a key signed at a
`seq`, and `zikaron/2` §7.3's authority at a position of the ledger, are
read by that law; this section reads a chain's order and never a ledger's.

## 7. Grant check

Whether a grant is good to rely on, read from its issuer's ledger and the
caller's time. The doctrine is the parent's (`zikaron/2` §8.3): a failure
needs a witness in hand, and absence acquits only when the record is
complete.

**[R] 7.1 Inputs.** A byte string `g`; an audit input `I` whose root is the
issuer's ledger root, as the terms name it or, where the caller holds no
such statement, the root this section derives below for `g` with `I` taken
as `null`, a ledger that continues an earlier one being read under the root
it continues (`zikaron/2` §7.3); or `null` when the caller holds no audit
input for the issuer; a notice input `N` (§6.3), or `null` when the caller
holds none; `now`. When `I` is given, the audit is taken over `I'`, which is
`I` with `g` added to its pile: the parent's audit then decides whether `g`
is a ledger entry under the root and what findings name it. The parent's
§9.4 test reads the root, the anchor records, and the basis and never the
pile, so `I'` is an invalid input exactly when `I` is, and every check below
that gates on `I` being invalid gates on the same state it later reads from
the report of `I'`.

`g` **carries a revocation member** iff its entry's `body` has a member with
key `revocation`, whatever its value (`zikaron/2` §6's presence rule), an
extra member under either law's §6.10 (`zikaron/2` §6.10, or `zikaron/1`
§6.10 for a `zikaron/1` grant) that this law reads as the issuer's statement
of where its withdrawals are noticed; the member is **well-formed** iff its
value is an object with exactly the members `chain` (int) and `at` (hex20).
`N` **applies to** `g` iff `N` is neither `null` nor invalid, the member is
well-formed, and `N`'s basis has `chainId` equal to `chain` and
`revocationContract` equal to `at`; a notice input read from another chain
or another contract says nothing about `g`. The **root** this section
derives for `g`, under which §6.4 reads the book and against which check 3
reads the root of `I` for a grant that carries the member, is, in this
order: the `root` of the book record of `N`, least in position, whose `to`
is `g.author`, where `N` applies to `g` and holds one (the book itself names
the seat a key acceded to; in a book whose accession is the acceding key's
own call, a record naming a key as `to` is that key's own act, so a key the
book has seated is read under the seat it acceded to, whatever root its
holder hands a caller, and which book a reader declares is the reader's,
§6.3); otherwise `I`'s root where `I` is given and not invalid; otherwise
`g.body.ledger` where `g` carries one whose value is hex20; and `g.author`
otherwise. A grant that carries no member is read by its issuer's ledger
alone, whatever `N` the caller holds: the issuer's own signed bytes decide
which way a grant is read, and no package, badge, or input does.

**[R] 7.2 Checks and verdict.** Six checks, each yielding `PASS`, `FAIL`, or
`UNKNOWN`, decided in this order. The result is the object of §7.3; its
**verdict** is `GREEN` iff every check is `PASS`, `PARTIAL` iff no check is
`FAIL` and some check is `UNKNOWN`, and `FAIL` otherwise.

| # | token | test |
|---|---|---|
| 1 | `BAD_SIG` | `g` fails `accept` (`FAIL`, carrying the `zikaron/2` token), or its entry is not of type `grant` (`FAIL`, carrying `NOT_A_GRANT`). A `FAIL` here leaves checks 2 to 6 `UNKNOWN`. |
| 2 | `BROKEN_LEDGER` | `I` is `null` or invalid: `UNKNOWN`. Else the report of `I'` labels `BROKEN_CHAIN`: `FAIL`, which leaves checks 3, 4, and 6 `UNKNOWN`. Else `PASS`. |
| 3 | `NOT_IN_LEDGER` | `I` is `null` or invalid: `UNKNOWN`. Else `g` is a `zikaron/1` entry and the report of `I'` carries `continues` `null`: `UNKNOWN`, which leaves checks 4 and 6 `UNKNOWN` (the ledger that holds it is read by `zikaron.kit/1` §10.2 and not by this law; §9 names the limit). Else `g`'s entry stands in `EXCLUDED` of either report, or is a `zikaron/1` entry standing in `MALFORMED` of the report of `I'` (outside the old segment): `FAIL`, which leaves checks 4 and 6 `UNKNOWN`. Else `g`'s entry stands in `RETIRED` of the report of `I'` (not timely, `zikaron/2` §7.4): `FAIL` where the label of the report of `I'` is `COMPLETE`, and `UNKNOWN` otherwise, either of which leaves checks 4 and 6 `UNKNOWN` (a pile missing one entry of the line that would bound `g` makes it untimely as surely as its key's signing above its retiring seq with no record earlier than its retirement reaching it, and absence convicts only on a complete record). Else `g`'s entry is a ledger entry of either segment, the lists above and the two ledgers being every place a byte string passing `accept` stands: `UNKNOWN` where it carries an `AUTHORITY_MISMATCH` finding (soft, since a hard one labelled the ledger broken), and `PASS` otherwise. |
| 4 | `UNANCHORED` | `I` is `null` or invalid, or check 3 left this check `UNKNOWN`: `UNKNOWN`. Else `g`'s entry is anchored (§6.2): `PASS`. Else the basis is **covering** iff its `chains` array is non-empty and every `chains` object has a non-empty `registries` array and a `senders` array that includes every key of the ledger's lineage (`zikaron/2` §7.4) and, for a continued ledger, every key of the old segment's lineage (`zikaron/1` §7.4); a basis that is not covering: `UNKNOWN` (the scan's silence convicts nobody). Else some anchor record of `I'` undiscarded by the trimming §6.2 names carries verdict `UNPROVEN` and a `hash` that is the `entry_id` of a ledger entry from which `g`'s entry is reachable (§6.2 with `counted` read as `UNPROVEN`): `UNKNOWN` (an anchor of bytes that would anchor `g` exists and its codeless test was not decided). Else the label of the report of `I'` is `COMPLETE`: `FAIL`. Else `UNKNOWN` (a `MISSING` entry may bound it). |
| 5 | `EXPIRED` | `g.body.window` absent: `PASS`. Else `now` is `null`: `UNKNOWN`. Else `now` < `from` or `now` > `to`: `FAIL`. Else `PASS`. |
| 6 | `REVOKED` | `I` is `null` or invalid, or check 3 left this check `UNKNOWN`: `UNKNOWN`. Else some ledger entry of `I'` of type `revocation` has `body.grant` = `hex32(entry_id(g))` and carries no `AUTHORITY_MISMATCH` finding: `FAIL`. Else the label of the report of `I'` is `COMPLETE`: `PASS`. Else `UNKNOWN` (a withheld revocation is exactly what an incomplete record hides). |

**For a grant that carries a revocation member** (§7.1), checks 1 and 5 are
the rows above and checks 2, 3, 4, and 6 are the four below, decided in the
same order: the issuer's standing is read from the chain the member names
and its withdrawal from the notices read there, so no check of the four
waits on a `COMPLETE` label, and check 6, once check 1 has passed, waits on
nothing but `N` and the root §7.1 derives.

- **2 `BROKEN_LEDGER`.** `I` is `null` or invalid: `UNKNOWN`. Else the
  report of `I'` labels `BROKEN_CHAIN`: `FAIL`, which leaves checks 3 and 4
  `UNKNOWN`. Else no byte string of `I'` that passes `zikaron/2` §4.3 is a
  `genesis` whose `author` is `auth(0)` of the segment this law reads
  (`zikaron/2` §7.3: the root, or the continuation's author), whether the
  report lists it as a ledger entry or as `RETIRED`: `UNKNOWN` (the pile
  holds no spine for the walk to start from, a badge alone being one such
  pile; what it held showed no fault, and the reader who wants this check
  decided takes the issuer's genesis, successions and migrations into the
  pile, §5.1; the check reads the byte string and not the report's lists
  because a root that handed off without anchoring its genesis on its own
  has, under `zikaron/2` §7.4, a genesis that a spine alone cannot make
  timely, and the spine is what §5.1 carries). Else `PASS`.
- **3 `NOT_IN_LEDGER`.** The **standing records** of `g` are the anchor
  records of `I` as the audit received them, before the trim of `zikaron/2`
  §8.1 (a trim by the ledger's lineage, which this check does not read, so
  that a record's `sender` is tested by the book alone), with `chainId`
  equal to the member's `chain`, `hash` equal to `hex32(entry_id(g))`,
  `verdict` `counted`, `blockNumber` not greater than `N`'s `toBlock`, and
  `sender` the key in office at their block (§6.4) under the root §7.1
  derives. `I` is `null` or invalid, or check 2 is `FAIL`: `UNKNOWN`. Else
  check 2 is `PASS`, `g`'s entry stands in `EXCLUDED` of either report or is
  a `zikaron/1` entry standing in `MALFORMED` of the report of `I'` where
  that report carries `continues` not `null` (the two cases the table's
  check 3 fails under any label; `RETIRED` is the ledger's rail and convicts
  nothing here, §9), and either `N` does not apply to `g`, so that no book
  is read, or `g.author` is no key of the book lineage of the root (§6.4,
  the root itself among its keys): `FAIL` (a key that neither the ledger's
  successions nor the book the reader read ever seated signed in the
  issuer's name, and the member takes nothing from what the ledger convicts
  of such a key, whatever chain the member names and whatever notice input
  the reader holds; a spine carries every succession, so a lawful successor
  beside its spine is never so convicted, and a badge alone, with check 2
  `UNKNOWN`, convicts nobody here; check 4 is decided by its own row). Else
  `N` does not apply to `g`: `UNKNOWN`. Else the root of `I` differs from
  the root §7.1 derives: `FAIL` (the ledger audited is not the seat the book
  gave the key that signed). Else some standing record has `sender` equal to
  `g.author`: `PASS`. Else some standing record exists: `FAIL` (the key in
  office anchored a grant that a key not then in office signed). Else
  `UNKNOWN`: the key in office has anchored no such bytes within what the
  reader read, a record beyond `toBlock`, a record by a key not in office,
  an `UNPROVEN` record, and a basis whose `senders` omit the key in office
  (§9) each being this case.
- **4 `UNANCHORED`.** `I` is `null` or invalid, check 2 is `FAIL`, or `N`
  does not apply to `g`: `UNKNOWN`. Else `g` has a standing record: `PASS`.
  Else `UNKNOWN`: a pile that holds a spine alone is short of `COMPLETE`
  wherever the seat anchored anything else, so absence convicts nothing
  here; what anchors on other chains, and anchors of later entries on `g`'s
  line, bound `g` under §6.2 for a reader who asks, and this check reads the
  member's chain alone.
- **6 `REVOKED`.** `N` does not apply to `g`: `UNKNOWN`. Else some notice
  record of `N` has `grant` equal to `hex32(entry_id(g))`, `verdict`
  `counted`, and `sender` the key in office at its position (§6.4): `FAIL`.
  Else some notice record of `N` has that `grant`, `verdict` `UNPROVEN` or
  `VOID`, and `sender` the key in office at its position: `UNKNOWN` (a
  notice of this grant by the key in office exists, and its codeless test
  was not decided or was decided against it, so the law cannot read it as
  that key's own act and reads it as no acquittal either: a key that carries
  code, by its own designator or by one a third party landed in the notice's
  block, notices again from a block where it carries none). Else `PASS`: the
  reader read every notice of the member's contract over the basis's blocks
  (§6.3) and no key in office withdrew the grant. A `revocation` entry of
  the ledger naming `g` is data here, the notice being the act this law
  reads; a notice by a key not in office at its position, a retired key's
  among them, withdraws nothing and leaves no doubt, whatever its verdict;
  and this check reads of `I` its root alone, and that only where the book
  names no seat for the signer (§7.1), so a badge with no ledger at all is
  answered.

A verifier shows every check's state; a green mark appears only for `GREEN`,
and `PARTIAL` is shown as its own state, never as green and never as red.
The window test is inclusive at both ends and reads `now` alone. Every
verdict is relative to the basis of `I` and, for a grant that carries the
member, to the basis of `N`: `GREEN` says that under the chains, registries,
and senders the reader declared, and under the contract, book and blocks it
read notices from, every check passed; a revocation anchored on a chain the
basis does not name is outside the record the reader read, and the result
carries both bases so that a reader who wants a wider record sees what was
read.

**[R] 7.3 Result form.** The result is a canonical object with exactly:
`verdict` (`GREEN`, `PARTIAL`, or `FAIL`); `basis`, the basis of `I` as a
canonical value, or `null` when `I` is `null` or invalid; `notices`, the
basis of `N` as a canonical value, or `null` when `N` is `null` or invalid,
whether or not `N` applies to `g`; `checks`, an array of six objects in
check order, each with exactly `{n, token, state, reason}`: `n` the check
number, `token` the check's token, `state` one of `PASS`, `FAIL`, `UNKNOWN`,
and `reason` the `zikaron/2` token or `NOT_A_GRANT` for check 1 in state
`FAIL` and `null` otherwise; and `failed`, the array of the tokens of the
checks in state `FAIL`, in check order (empty unless the verdict is `FAIL`).

**[R] 7.4 Ledger link.** Between an upstream grant `u` and a downstream
grant `d` read under audit input `I_d` and notice input `N_d`, the ledger
link is decided in this order: the byte link of §3.3 fails: **false**; `I_d`
is `null` or invalid, and `N_d` does not apply to `d` (§7.1, which a `d`
carrying no revocation member never satisfies): **unknown**; the root of
`d`, which is the root §7.1 derives for `d` (the book's root for `d.author`
where `N_d` applies to `d` and the book names one, otherwise `I_d`'s root
where `I_d` is given and not invalid, otherwise `d`'s hex20 `ledger` or its
`author`), ≠ `u.body.grantee`: **false**; otherwise **true**: the downstream
issuer's ledger is the one the upstream grantee opened, whatever key signs
it now. Where `N_d` applies to `d` and the book names a seat for `d.author`,
the root compared is that seat, whatever root the reader audited `d` under,
so a successor's grant links to the seat its key acceded to, and a hop
audited under another root fails its own check 3 wherever that check reads
the audit; where the book names none and `I_d` is `null` or invalid, the
root compared is the one `d`'s own `ledger` member or `author` states, the
link is then the downstream grant's own statement of its seat, and the hop's
checks 3 and 4 say whether the key in office stands behind the grant.

**[R] 7.5 Chain check.** Inputs: a list of hops `(g_k, I_k, N_k)` for `k` =
0 … `m`, ordered from the original author's grant to the grant relied on,
and `now`. An empty list yields verdict `FAIL` with token `CHAIN_EMPTY`,
`hops` and `links` empty, and `failing` `{"kind": "empty", "index": 0}`.
Otherwise every hop is checked by §7.2 in order over its own `I_k` and
`N_k`, and every link `k` = 1 … `m` is decided by §7.4 in order under `I_k`
and `N_k`, a link being **undecided** (`null`) when either of its two grants
failed check 1, and `null` also where §7.4 answers **unknown**, one `null`
covering both. Then, in this order, the first that applies is the chain's
**failing** point: `g_0` passed check 1 and `g_0.body` has a member with key
`upstream`, whatever its value: kind `incomplete`, index 0, token
`CHAIN_INCOMPLETE`; some link `k` is false: kind `link`, the smallest such
`k`, token `CHAIN_LINK`; some hop `k` has verdict `FAIL`: kind `hop`, the
smallest such `k`, token `null`. The **chain verdict** is decided in this
order: `FAIL` when a failing point exists; else `GREEN` when every hop is
`GREEN` and every link is true; else `PARTIAL`. A chain whose only failing
point is `incomplete` is `FAIL` though every hop be `GREEN`: a payload that
does not start at a grant stating no upstream proves nothing about what
stands above it. The result is a canonical object with exactly: `verdict`;
`hops`, the array of §7.3 results in hop order; `links`, the array for `k` =
1 … `m` of `true`, `false`, or `null`; `token`, the token named above or
`null`; and `failing`, `null` or `{kind, index}`.

## 8. Tokens and vocabularies

**Document tokens** (closed; one per rejection, by the orders of §3.1, §3.2,
§4.1, §4.1a, §4.4, §5.2): `E_BADGE_PREFIX`, `E_BADGE_CAP`, `E_BADGE_B64`,
`E_BADGE_ENTRY` (carrying `k`, and under §3.2 a `zikaron/2` §10 token beside
it), `E_BADGE_TYPE`, `E_BADGE_INCOMPLETE`, `E_BADGE_LINK`;
`E_KIT_UNREADABLE`, `E_KIT_MANIFEST_ABSENT`, `E_KIT_MANIFEST`,
`E_KIT_ENTRY_BYTES`, `E_KIT_FILE`, `E_KIT_CONTENT`, `E_KIT_EXTRA`;
`E_GP_NO_CODE`, `E_GP_ROOT`, `E_GP_UNCARRIED`, `E_GP_CONTENT`, `E_GP_TERMS`.

**Result vocabularies** (closed; the values the [D] and reading predicates
yield): badge `BADGE_OK`; kit `KIT_OK`; grant package `GP_OK`; check states
`PASS`, `FAIL`, `UNKNOWN`; check tokens `BAD_SIG` (with reasons
`NOT_A_GRANT` or a `zikaron/2` token), `BROKEN_LEDGER`, `NOT_IN_LEDGER`,
`UNANCHORED`, `EXPIRED`, `REVOKED`; verdicts `GREEN`, `PARTIAL`, `FAIL`;
chain tokens `CHAIN_INCOMPLETE`, `CHAIN_LINK`, `CHAIN_EMPTY`; chain failing
kinds `empty`, `incomplete`, `link`, `hop`; link values `true`, `false`,
`null`; kit manifest rule names `canonical`, `members`, `spec`, `root`,
`entries`, `files`, `contents`, `anchoredOn`, `anchors`, `note_md`.

## 9. Outside the grammar, and the openings by name

- **Chains and clocks.** This law consults no chain and reads no clock; the
  audit input and the notice input carry what the chain said, and `now`
  carries what the caller trusts of the time. Which root is the issuer's is
  the terms' statement: a reader who audits under another root reads another
  ledger. For a grant that carries no revocation member, the analysis of §7
  shows that a ledger a stranger assembles around an author's key reaches
  `PARTIAL` at best, never `GREEN`, unless every anchor of every key in its
  lineage is in hand; for a grant that carries the member, a ledger a
  stranger assembles under its own key reaches `GREEN` under that key and
  under no other root, so the root the reader declares is what the verdict
  is about.
- **A check may test only the logs it reads.** The parent's completeness
  rule binds what a verifier obtains: every log it enumerates under the
  basis, or no anchor set (`zikaron/2` §9.4). A caller that obtained every
  such log may hand §7 an audit input whose anchor set carries a record only
  for each log it tested against `zikaron/2` §9.1 and §9.3, the other logs
  left untested and carrying no record, where no check of §7.2 would take
  another state whichever of the untested logs are anchors and whatever
  verdicts `zikaron/2` §9.3 gives them: the result does not then depend on
  the logs left untested, and stands under the basis it names. Whether that
  holds is decided before §7 runs, from the pile, the hashes the untested
  logs name, and the records in hand; where it cannot be so decided, the
  input does not serve §7, and the grant is checked on a conforming anchor
  set or is not checked. For the remark of the bullet above, an anchor whose
  log was obtained and left untested under this one is in hand. This bullet
  reaches the audit input alone: a notice input is read as §6.3 says. An
  input so drawn serves §7 alone: it is not the conforming anchor set of
  `zikaron/2` §9.4, a log it leaves untested is not a log its scan failed to
  read, and the label, the entry count, and the lists of a report taken
  over it are not the parent's report of that basis and are presented as
  none. No check's condition moves.
- **An unanchored entry binds nobody who did not see it.** For a grant that
  carries no revocation member, check 6 acquits on a `COMPLETE` record, and
  a revocation the issuer never anchored is outside every anchor set;
  likewise a succession never anchored leaves the successor's keys outside
  every basis a stranger can declare. The doctrine's answer is to anchor
  revocations and successions, and this law states the limit by name. For a
  grant that carries the member, the act this law reads as withdrawal is the
  notice (§6.3): a `revocation` entry the issuer wrote into its ledger and
  never noticed through the member's contract withdraws nothing under check
  6, and the issuer notices through that contract, which anchors the
  revocation entry in the same transaction where the contract is built to.
  The notice is the key's own transaction from a key carrying no code: one
  sent through a wallet contract or a batcher names that contract in topic
  1, which is no transaction's sender, and one sent from a delegated account
  carries code and is `VOID`, and an issuer reads its own notice back in a
  notice input, as `counted`, before it holds the grant withdrawn.
- **Two rails, read apart.** For a grant that carries a revocation member,
  who is in office is read from the book the reader declared (§6.4), by the
  order of its accessions on the member's chain; `zikaron/2` §7.3 and §7.4
  read authority and lineage from the ledger's own successions and the block
  times of the records that anchor them, and that law's §11 leaves a
  disagreement between the two rails to a reader's law, which this one is.
  The two agree wherever a seat accedes in the book before it signs or
  notices under the new key and anchors nothing under the old key
  afterwards, and they disagree in the window between a succession's anchor
  and its successor's accession, where the ledger has moved and the book has
  not: a seat that writes grants carrying the member closes that window on
  its own side, and a grant checked inside it reads the book. The two rails
  meet in one place, the issuer's root: the book is read under the root §7.1
  derives, the root the book itself names for the signing key where it names
  one, and otherwise the root `I` is read under, the terms' statement or
  §7.1's own derivation, or, where the reader holds no valid `I`, the
  grant's own hex20 `ledger` or its `author`, so a seat enters the book
  under its root key or not at all, a key the book has seated under a root
  is read under that root wherever its grant travels, a key accedes to one
  seat, the first the book records for it, and a terms' root the book
  contradicts fails check 3.
- **Building the inputs for a grant that carries the member.** The reader
  builds `N` first, for the chain, contract and book it declares for the
  issuer, the terms' statement where the terms name them, from at or before
  each contract's first block to one `toBlock`: the member says where the
  issuer notices, the reader says which venues it reads, and a member naming
  another chain or contract than the reader declares gets an input that does
  not apply, so that checks 3, 4 and 6 stay `UNKNOWN` and never `GREEN`. The
  reader then builds `I`: the `senders` of the `chains` object for the
  member's chain in `I`'s basis include every key of the book lineage (§6.4)
  of the root §7.1 derives, since `zikaron/2` §9.4 yields no record for a
  sender outside `senders`, so a reader whose `senders` come from the
  ledger's pile alone reads no anchor by a key the book admitted and the
  pile does not name, and checks 3 and 4 stay `UNKNOWN`; and the
  `registries` of that object include every contract through which the
  issuer anchors, the member's contract among them wherever it emits
  `zikaron/2` §9.1's form, as a contract that anchors the revocation entry
  beside its notice does, since a revocation entry anchored through that
  contract alone is otherwise outside the basis and the ledger's label reads
  past it. A reader with no node for the member's chain builds no `N`, and
  check 6 stays `UNKNOWN` for every grant carrying that chain. Both forms
  admit any sender, so the size of a notice input is anyone's to grow; a
  notice by a key out of office decides nothing, and the cost of reading it
  is the reader's, who reads in parts where its node bounds a scan and hands
  §7 `null` where it could not read them all.
- **A badge alone.** For a grant that carries the member, a reader holding
  the badge and a node builds `N`, then `I` from the chain alone, with the
  root §7.1 derives, an empty pile, and the basis and anchor records of its
  scan; checks 1, 3, 4, 5, and 6 are then decided, check 2 is `UNKNOWN` for
  want of a spine, and the verdict is `PARTIAL` at best, `FAIL` where a
  notice stands; a reader that takes the issuer's spine into the pile (§5.1)
  reaches `GREEN`. A `zikaron/1` ledger that never moved has no `zikaron/2`
  genesis, so a grant of its key carrying the member has check 2 `UNKNOWN`
  under any pile and reads `PARTIAL` at best, as §7.2 check 3 reads such a
  grant without the member. A grant carrying no member still needs its
  issuer's ledger for checks 2 to 4 and 6, and a chain mixing the two kinds
  names, hop by hop, which issuer's ledger it still needs.
- **The book's reach.** A book records what was declared and acceded in it;
  a seat whose key moved before the book held it enters the book under its
  root key, by one declaration and one accession, and the book then holds
  the root and the key now in office and none of the keys between them.
  Under §6.4 the root is in office until that accession: a grant an
  intermediate key anchored before it has no standing record and reads
  `UNKNOWN` at checks 3 and 4, a grant the root key anchored reads as the
  root's, and a grant the key now in office anchors after it reads as that
  key's. A seat whose root key is gone, lost or handed on without the book,
  enters the book under no key, and its grants carry no member; the ledger
  reads them as before. A `to` that is a contract address holds the seat as
  any key does and notices nothing, no transaction being its own, so a seat
  that names one has given its withdrawals to nobody until the book moves
  again. The book of one chain is read for that chain: a key that lost the
  seat in one chain's book holds it in another chain's until that book moves
  too, and the reader's declared venue is what keeps a grant's member from
  choosing a chain where the book never moved. A key that accedes takes
  every grant it signed into the seat it acceded to; and an accession is
  read as the acceding account's call whatever code that account carries
  (§6.3 asks the sender and its code of the notice form alone), so a root
  whose key lets another account call for it can be acceded by that other
  into a seat not its own, after which its grants fail check 3 wherever that
  check reads an audit under its own root: the law names the limit; entering
  the book under its own root first, in a book that admits a key once, keeps
  a root out of another's seat, and an account a root lets call for it can
  still declare the root's seat on to a key of its choosing, as the root
  itself can, so a seat that lets any account call for it has handed that
  account its seat in the book.
- **A retired key's stray anchor enters no reconciliation at a record at
  which the key has retired.** Under a basis whose anchor set holds a record
  anchoring the succession or migration that retired the key, every anchor
  of that key at which it has retired, the later ones and those of the
  retirement's own second whose `hash` is a byte string the pile holds,
  passing `zikaron/2` §4.3, that the key signed above its retiring seq,
  stands in `DISCARDED` (`zikaron/2` §7.4, §8.1), and no check of §7 reads
  `DISCARDED`; such a record of the retirement's own second still reaches
  along `prev` the line it anchors, whoever sent it, so what the key signed
  there at or before its retiring seq is timely and a fork it forms fails
  check 2 (`zikaron/2` §7.4, §8.4); every other anchor of that key in that
  second, of a hash whose bytes the pile does not hold, of bytes that fail
  §4.3 or of what another key signed, is one at which the key has not
  retired and stands where `zikaron/2` §8.5 and §8.6 place a kept record by
  its verdict, a counted one in `MISSING`, so that the record is not
  `COMPLETE`, a row the bytes, once in the pile, can turn into a `DISCARDED`
  one; under a basis that reaches no such record the stray anchor is a
  counted anchor of a hash whose bytes are never produced, a `MISSING` row
  (`zikaron/2` §8.5), so the record is not `COMPLETE`, check 6 stays
  `UNKNOWN`, and check 4 stays `UNKNOWN` unless the grant is anchored. A
  `GREEN` here says that under the pile the reader was handed every check
  passed, and never that the pile is the whole ledger; a reader who wants
  that silence closed reads the parent's `DISCARDED` and `RETIRED` lists
  beside the result.
- **A verdict reaches as far as its basis.** The parent scans what a basis
  declares and nothing else; a grant `GREEN` under a basis of one chain may
  be revoked on another. Which chains a reader should declare is the terms'
  statement, and the result carries the basis it was read under.
- **A `zikaron/1` grant carrying no revocation member, under an audit that
  shows no continuation.** Check 3 leaves it `UNKNOWN` with checks 4 and 6
  where the report of `I'` carries `continues` `null`: the issuer's ledger
  is a `zikaron/1` ledger that never moved, or one that moved and whose
  continuation this input does not show (no byte string of the input meets
  `zikaron/2` §12.6's candidate condition; the closing succession's record
  or bytes, or the genesis that continues it, being absent, and a closing
  key that handed off, are among the ways), and this law's audit reads it as
  no ledger at all. A reader who supplies what §12.6 asks of a continuation
  reads the ledger under this law; for a ledger that never moved,
  `zikaron.kit/1` §10.2 over the `zikaron/1` core is the check, a reader
  that holds that core runs it, and this law names the limit.
- **Terms.** What a grant permits, whether its window's ends mean what a
  party hoped, and exclusivity: the terms and the forum.
- **Encryption.** A kit addressed to one key travels inside a family
  envelope this law does not read; what reaches this law is the enumeration
  inside it.
- **Retracted records.** A `contents` row over a record the issuer's ledger
  retracted is a row like any other; what the retraction means is the
  parent's §11.
- **Weighing.** What a kit proves beyond its bytes, and how a badge's issuer
  is trusted: the reader's.
- **Platforms.** A kit path this law admits may still be one a filesystem
  cannot hold (a reserved device name on one platform); the reader there
  reports `E_KIT_FILE` or `E_KIT_UNREADABLE`, and the law names no platform;
  a filesystem that refuses a name outside UTF-8 makes §4.1's last failure
  unreachable there, and a reader on such a platform witnesses it nowhere,
  so that failure is outside §10.1's corpus wherever the corpus is built on
  such a filesystem.
- **Restoring names.** Every `name_md` a `KIT_OK` kit carries is a name and
  never a path: a `files` row's `name_md` is `/`-separated where the row
  lies under a directory original (§4.3), whose rules it passed, and is one
  name otherwise; a `contents` row's `name_md` is one name wherever its row
  lies, which §4.3 tests nowhere, and names a directory original alone, a
  file original's name being its `files` row's; a reader that restores a
  `/`-separated name creates each segment as one name inside the directory
  the preceding segments made, restores a one-name value as one name, lands
  a directory original under its `contents` row's `name_md` where present
  and under the last segment of its `path` otherwise, restores a file
  original that lies under a directory original, on its own, under the last
  segment of its `files` row's name, and never hands a name to a path
  parser; a segment its platform cannot hold as one name, or two names it
  cannot hold apart, is that reader's limit, reported and never resolved by
  rewriting.
- **Rendering.** How checks and states are shown, beyond the rules that a
  verifier shows every check's state, `GREEN` alone is green, `PARTIAL` is
  its own state, and a payload yielding a token shows nothing.
- **Payment and escrow.** Not here.

## 10. Freeze and evolution

**10.1** The reference implementation freezes when an independently written
implementation, produced from this text and a conforming `zikaron/2` core
alone, agrees with it on every input of a generated corpus (every badge
encoding and every badge decoding, every kit verdict with its subject, its
kit id, its counts and its `invalid_entries` rows with their tokens, over
directories and one-file kits alike, every grant package verdict with what
it carries, every check result, every chain result), across at least one
hundred thousand fuzz samples, as `zikaron/2` §12.1 asks of its own core,
with zero panics, hangs, nondeterministic answers, or wrong accepts under
fuzzing, and with every closed boundary in this text witnessed on both sides
by at least one corpus input, the boundary list being a precondition on the
corpus and never a property claimed of it afterwards, and published beside
the corpus as the manifest's residue of the closed vocabularies no case
witnesses and the branch-arm classes of the conformance tree's boundary
audit. The boundary is witnessed under every seed the corpus is generated
with. A closed boundary of this text is a boundary the corpus can witness on
the surface §10.2 compares; a rule with no reachable failing side is
witnessed once and states no boundary in this sense, and §9 names the one
failure that lies outside the corpus.

**10.2** Once §10.5 names a release digest, agreement with the frozen core
on the corpus is the test anyone can run in seconds, and it is evidence of
conformance and never its definition: conformance is agreement on every
input, and an input outside the corpus on which a verifier differs from the
core is a non-conformance the corpus did not catch and a reason to grow it.

**10.3** `zikaron.kit/3` is a new spec id in every document; a document
accepted under `zikaron.kit/2` is accepted under `zikaron.kit/2` forever,
and a later law reads it by this one; this law's predicates refuse a
`zikaron.kit/1` manifest by §4.3's order, at `members` since every such
manifest carries `proofs`, and a reader holding one runs the `zikaron.kit/1`
core on it.

**10.4** This law and `zikaron/2` are two laws: a change to one is never a
change to the other, and each names its own core. This law's core embeds the
`zikaron/2` core of the release digest named in that law's §12.5, which
itself embeds the `zikaron/1` core of release digest
`0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc`
(`zikaron/2` §12.6); the `zikaron/2` core this law's core embeds is the one
of release digest
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a`,
named in that law's §12.5 on 2026-10-31. A kit core built on any other
`zikaron/2` core is another core.

**10.5 The core's identity.** Naming a release digest here is the freeze,
and it is the only act that performs it. The release digest is
`0x5905333068c04ec2c4b096269f8baa0f2864925dc31910ae4e528e0c0899125d`,
computed by `zikaron-conformance/criterion-digest-kit2.sh` over the four
source files of the pure Python implementation in
`zikaron-conformance/kit2-py/` (`zkk2.py`, `zkk2doc.py`, `zkk2kit.py`,
`zkk2read.py`; the language's standard library and the `zikaron/2` core of
§10.4, nothing else) and the `zikaron/2` core digest of §10.4, named on
2026-11-09; the kit corpus, `HARNESS-KIT2.md`, and the review record beside
it say how it converged. From the naming the core binds and this text is its
textbook; a dispute about what `zikaron.kit/2` says on some input is settled
by running the core on it. A change to any of the four files is a new core
and a new digest named by the same act.
