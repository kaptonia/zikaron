# ZIKARON wire grammar v1 (`zikaron/1`)

The evidence ledger grammar. `zikaron/1` is the wire law of a ledger under
which any key records, at its own hand, what it wants remembered: a digest
of any content (`history`), an authorization over such a digest (`grant`)
and its withdrawal (`revocation`), a transaction it claims (`adoption`),
and the handover of its seat (`succession`). It is a base layer of evidence
for transactions of any kind; registering works is one use among them, and
the grammar names none. This document is self-sufficient: every predicate a
verifier needs is stated here in full. Nothing in any construction
document, design document, or implementation adds to or subtracts from it.

The ledger is neutral toward everything that gives an entry its value. It knows no price, no escrow, no licence term: a history entry
names a digest, a grant names a digest and a terms document, and the
grammar tests the forms it names and nothing they mean. Anchoring is
through two forms (§9) that any contract on any chain can produce or
ignore: a registry emits the registry form from its own code, and any
other contract an author calls may emit it for a word of the author's own
calldata; a log is read only where a reader has named the emitting address
in a `chains` object's `registries` of its scan basis (§9.4), and an author
who anchors through no contract at all is anchored from a lineage key's
own self-directed transactions, which a reader reaches only by naming them
in `bareTx`. Anchoring is in every case relative to a declared scan and
never a fact a report asserts without one.

**Status.** The law is the set of total predicates in this text. Every rule is
a machine decision defined over all inputs; there are no example-based
prohibitions and no blacklists. A conforming implementation agrees with the
frozen core on every input from the moment §12.5 names a release digest;
where two implementations disagree after that naming, at least one is wrong,
and the core decides which. Before the naming there is no core, this text is
the whole of the law, and a disagreement is settled by reading it and, where
the reading is contested, by amending it. After the naming this text is the
core's textbook: it says the same thing in prose, and where the two could
ever be read apart, the core binds. Evolution goes through `zikaron/2`.

**Rule classes.** Three classes of predicate appear, and every rule below is
marked with its class. A passage that states no predicate (a producer
obligation, a definition of a record, a description of what a report
carries) carries no mark. §5.1 through §5.3 and §5.6 are such passages:
they define the preimage, the message, the digest, and the domain, and §5.4
and §5.5 are the predicates they feed. Where a test of §4.3 or of a row of
§6 repeats a §3 predicate exactly, that half of the test has no reachable
failing side and contributes no boundary to §12.1: `token`'s upper edge at
U+007E, on `entryType` at §4.3 step 5 and on every `token` row of §6, is the
case, every scalar value above U+007E having failed at step 1, as
`E_VALUE_CHARSET` or, spelled by a surrogate pair, as `E_JSON`, while its
lower edge at U+0020 is reachable on both sides (§1).

- **[E]** entry predicates: decided from one byte string alone, offline:
  the steps of §4.3, and §6.6's attestation test, which no step of §4.3
  runs and whose outcome reaches a report through §9.5 alone.
- **[C]** chain predicates: decided offline from a set of entries together
  with the audit inputs §8 declares (the anchor set with its §9.3 verdicts
  and the basis it was scanned under, the unavailable set, and the adoption
  evidence set), and never by consulting a blockchain.
- **[X]** chain-facing predicates: decided against a blockchain's state or
  history. Where an [X] predicate can fail for want of the chain, it names
  the verdict a verifier returns; §9.1 and §9.2 define the anchor forms, the anchor
  record, and the adoption evidence record, and carry no verdict of their
  own.

Verification from a folder of files with offline math means class [E] in
full and class [C] over an empty anchor set under a basis declaring no chain, an
empty unavailable set, and an empty adoption evidence set; class [X] is the work of producing those
three inputs from a chain. Every list in an audit report is decided from
the five inputs of §8 and nothing else, so the freeze of §12 covers [E] and
[C] in full, the report's `ADOPTION_UNPROVEN`, `UNPROVEN`, and `VOID` lists
and its label included, and never covers how a
verifier obtains the inputs from a chain: two conforming implementations may
disagree about what a chain shows, and §9.4's completeness rule and §9.7's
proof kits, never §12.2, are what settle such a disagreement.

## 1. Notation

- **byte string**: a finite sequence of octets.
- **`hex20`**: a string of exactly 42 characters: `0x` then 40 lowercase
  hexadecimal digits `0-9a-f`. **`hex32`**: `0x` then 64 such digits (66
  characters). **`hex65`**: `0x` then 130 such digits (132 characters).
  Uppercase digits, an uppercase or absent `0x` prefix, or any other length
  fail the form.
  `hex20(a)`, `hex32(h)`, and `hex65(g)` write a 20-byte address, a 32-byte
  digest, and a 65-byte signature as `0x` followed by the bytes in lowercase
  hexadecimal, most significant nibble of each byte first. Where a rule says
  a field equals an address, a digest, or an `entry_id`, it means the field
  is byte-equal to that value's spelling under these functions.
- **bytewise order**: two octet sequences are compared position by position
  from the first; at the first position where they differ, the sequence with
  the smaller octet orders first; where one is a proper prefix of the other,
  the shorter orders first. Where a rule compares an address, a digest, or
  an `entry_id` bytewise, the octet sequence compared is that value's
  `hex20` or `hex32` spelling; the two orders agree, lowercase hexadecimal
  being order-preserving over bytes, and the spelling is what a report
  carries.
- **UTF-8**: the encoding of RFC 3629 exactly: every scalar value in its
  shortest form, no byte sequence encoding a value in U+D800–U+DFFF, and no
  byte sequence encoding a value above U+10FFFF. Any other byte sequence is
  not valid UTF-8.
- **`token`**: a non-empty string every one of whose scalar values lies in
  U+0021 through U+007E (printable ASCII without the space). Tokens compare
  bytewise.
- **`skeleton string`**: a string every one of whose scalar values lies in
  U+0020 through U+007E.
- **`prose string`**: a string, whose scalar values are unrestricted (§3.3).
  Where a row of §6 names one of these forms, it tests at §4.3 step 12
  that the value is a string; a `prose string` row tests nothing further, and
  a `token` row tests that it is non-empty and carries no U+0020, the rest
  of the token charset having been decided by §3.5 at step 1, which admits
  U+0020 in a string value where a token does not (§6). No row of §6 names
  `skeleton string`, which this law uses for the key and value charsets of
  §3.3 alone.
- **`int`**: a JSON integer in the universe of §3.1 (0 to 2^53 − 1).
- **`bool`**: the literal `true` or the literal `false` (§3.1) and no other
  value; `1`, `"true"`, and `null` are not `bool`. No row of §6 names it;
  the `certain` of a §8.7 finding and the `hard` of every finding carry it.
- **`sha256`**: FIPS 180-4 SHA-256 over bytes. **`keccak256`**: the Keccak-256
  function as used by Ethereum (the pre-FIPS Keccak padding), over bytes.
- **`secp256k1`**: the elliptic curve of that name; `n` denotes its group
  order, `0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141`.
- **Unix seconds**: an `int` counting seconds since 1970-01-01T00:00:00Z.
  Every time-valued field of an entry (`effective` in §6.7, `from` and `to`
  in §6.3) is Unix seconds and is data for readers. No predicate compares
  one to a clock, to a block, or to a field of any other entry; the only
  test any of them carries is the ordering test inside `window` (§6.3). The `blockTimestamp` of an
  anchor record (§9.2) is not a field of an entry: it is Unix seconds read
  from a block header, it enters no finding, and it appears in a report only
  in the `ANCHORED` rows of §8.7 item 5, from which §9.6 computes a reader's
  existence bound, and in the `DISCARDED` rows of item 14, which carry every
  member of a record the lineage trim removed.

## 2. Identity

**[E] 2.1 Identity is total.** For every byte string `b`, `entry_id(b) =
sha256(b)`. Identity and validity are separate predicates: a byte string has
an `entry_id` whether or not it is an entry. This is what lets an anchor
commit to bytes before anyone has read them, and lets an audit name an
anchored byte string that fails to verify.

**[E] 2.2 Publication is unique.** An entry's published bytes are its
canonical bytes (§3) and nothing else: no byte order mark, no trailing
newline, and no whitespace outside a string value. A re-encoding of a valid entry either
reproduces its bytes exactly, in which case it is the same byte string and
the same publication, or differs from them, in which case it fails §3.5 and is not an entry at
all; a re-encoding that differs only in member order, or in an escape §3.5
admits, fails at test 6, one that spells a scalar value by a surrogate pair
fails at test 2 as `E_JSON` (§3.3), and one that differs in a way test 2's
grammar refuses fails there.

## 3. Canonical form

**[E]** `zikaron/1` uses one serialization of one closed value universe,
and one acceptance test. The profile is RFC 8785 (JSON Canonicalization
Scheme) with the restrictions stated here; where this section and the RFC
differ, this section governs.

**[E] 3.1 Value universe.** A value is exactly one of: the literal `null`; the
literal `true`; the literal `false`; an integer; a string; an array of values;
an object whose members map keys to values. Nothing else is a value. In
particular no value is a number carrying a minus sign, a fraction, or an
exponent: floating point is unrepresentable. A **value position of numeric
shape** is a position at which the grammar expects a value and whose first
byte is `-`, `+`, `.`, or a decimal digit; a member-name position is never
a value position. **The bytes there** are the maximal run of bytes beginning
at that first byte and drawn from `0`–`9`, `+`, `-`, `.`, `e`, and `E`. A
value position of numeric shape fails `E_NUMBER` when the bytes there are
not an integer under §3.2, whether they fail RFC 8259's number grammar or
satisfy it outside the universe; every other failure to parse fails
`E_JSON`.

**[E] 3.2 Integers.** An integer is written in decimal with no sign, no leading
zeros (the integer zero is written `0`), and lies in [0, 2^53 − 1]. A number
outside that range fails acceptance.

**[E] 3.3 Strings.** Strings are sequences of Unicode scalar values (no
surrogate code points), encoded as UTF-8. A `\u` escape whose four digits
name a code point in U+D800–U+DFFF fails `E_JSON`, whether or not a second
such escape follows it; §3.4 defines canonical bytes only over scalar
values, and no accepted byte string reaches it carrying a surrogate. Two
charsets apply:

- **Keys** are skeleton strings (every scalar value in U+0020 through
  U+007E) and non-empty. A key carrying any other scalar value, or none,
  fails `E_KEY_CHARSET`.
- **String values** are skeleton strings, except inside a prose subtree. A member
  whose decoded key is the three bytes `_md`, or ends in them, opens a prose
  subtree: that member's own value, and every string value at any depth within it (through
  arrays and nested objects), is a prose string and may carry any scalar
  value, including U+0000 through U+001F and U+007F. Keys inside a prose
  subtree remain keys under the key rule. Outside every prose subtree, a
  string value carrying a scalar value outside U+0020 through U+007E fails
  `E_VALUE_CHARSET`.
  A prose subtree opens at whatever depth its member sits, the root object
  included; no key of the envelope (§4.1) ends in `_md`, so no entry ever
  opens one at the root, and a byte string that is not an entry and does
  open one there is decided by §3.5 like any other. U+FEFF has no special
  standing as a mark: outside a prose subtree the key and value charsets
  refuse it like any scalar value above U+007E, inside a prose subtree it is
  carried and emitted raw like any other scalar value, and only as the first
  three bytes of `b` do `EF BB BF` fail test 2 (§3.5).

Every test in this section is applied to decoded strings: the key charset,
the value charset, and the `_md` suffix are each decided after escape
decoding, never on the input's raw bytes. A raw byte in 0x00 through 0x1F
inside a string is not admitted by RFC 8259's `unescaped` production and is
a grammar failure at test 2 of §3.5 whose triggering byte is that raw byte,
named `E_JSON` where `b` is valid UTF-8 and no earlier triggering byte
names another fault, whatever charset would apply to the string it appears
in: the permission for prose strings to carry U+0000 through
U+001F is a permission over decoded scalar values and never over the input's
raw bytes, and §3.4 item 3 fixes their only canonical spelling.

**[E] 3.4 Canonical bytes.** The canonical bytes of a value are produced by:

1. `null`, `true`, `false` as those four, four, and five ASCII bytes.
2. An integer as its decimal digits per §3.2.
3. A string as `"`, its scalar values in order, `"`, where each scalar value
   is emitted as: `"` → `\"`; `\` → `\\`; U+0008 → `\b`; U+0009 → `\t`;
   U+000A → `\n`; U+000C → `\f`; U+000D → `\r`; any other value in
   U+0000–U+001F → `\u00` followed by two lowercase hexadecimal digits; every
   other scalar value (U+0020 and above, U+007F and the C1 range included) as
   its raw UTF-8 bytes, never escaped.
4. An array as `[`, its elements' canonical bytes in order joined by `,`, `]`.
5. An object as `{`, its members as `key-bytes:value-bytes` joined by `,`,
   `}`, where key-bytes are the canonical bytes of the key as a string, and
      members are ordered by the bytewise order of their decoded UTF-8 keys.
   Test 4 of §3.5 restricts every key to U+0020 through U+007E and runs
   before test 6, so no value reaches this item with a key on which that
   order differs from the RFC's UTF-16 code-unit order; a future law that
   widens the key charset must restate this item, the two orders differing
   on keys carrying a scalar value above U+FFFF.
6. No byte lies between the pieces produced by items 1 through 5: no
   whitespace is emitted as structure. A whitespace scalar value inside a
   string is part of that string and is emitted by item 3 like any other
   scalar value. The output is UTF-8.

**[E] 3.5 Acceptance is roundtrip identity.** A byte string `b` is accepted
as canonical iff it passes the following tests, decided in this order, the
first failure naming the fault:

1. `b` is valid UTF-8 (§1) over its whole length, whatever the parse of
   test 2 would have reached: `E_UTF8`.
2. `b` matches RFC 8259's `JSON-text` production: `E_JSON`; within the
   parse, a container (array or object) opening at nesting depth 129, the
   root container counted as depth 1, fails `E_DEPTH` at the moment it
   opens and before any later byte is read; a value position of numeric
   shape that is not an integer under §3.1 and §3.2 fails `E_NUMBER`; a
   surrogate escape fails `E_JSON` (§3.3). Within this test the fault named
   is the one whose triggering byte comes earliest in `b`: the triggering
   byte of `E_DEPTH` is the `[` or `{` of the container opening at depth
   129, the triggering byte of `E_NUMBER` is the first byte of the value
   position of numeric shape, the triggering byte of a surrogate escape is
   its `\`, and the triggering byte of a grammar failure is the first byte
   no continuation of the `JSON-text` production admits, or the position one
   past the end of `b` when `b` is a proper prefix of some JSON text. The
   fault whose triggering    byte comes earliest is the one named, and a fault whose triggering byte
   the parse never reaches is never named; one left-to-right parse that
   stops at its first fault realises this rule. Where two faults share a
   triggering byte, the fault named is the one earlier in the order
   `E_DEPTH`, `E_NUMBER`, surrogate escape, grammar failure; in particular a
   value position whose first byte is `+` or `.` names `E_NUMBER` and never
   `E_JSON`, though RFC 8259 admits neither byte there. That is the one
   shape a tie takes: a bracket the grammar refuses opens no container, so
   `E_DEPTH` shares no triggering byte with a grammar failure, and an escape
   the grammar admits is no grammar failure, so neither does a surrogate
   escape. So `0x10` at a value
   position fails `E_JSON` (the numeric run is `0`, an integer, and `x` is
   the fault) while `01` fails `E_NUMBER`, and a byte string carrying 128
   nested containers passes this test while one carrying 129 fails it. A
   scalar root has depth 0.
3. Every object at every depth has distinct keys after escape decoding:
   `E_DUP_KEY`.
4. Every key satisfies §3.3: `E_KEY_CHARSET`.
5. Every string value satisfies §3.3: `E_VALUE_CHARSET`.
6. The canonical bytes of the parsed value, computed by §3.4, equal `b`
   byte for byte: `E_NOT_CANONICAL`.

Test 3 ranges over every object of `b`'s parse and over the member sequence
each object's bytes write, duplicates included; tests 4 and 5 range over
every key and every string value of the whole parsed value, at every depth.
Each of the three is decided in full before the next begins, so a value that both repeats a key and
carries a key outside the charset fails `E_DUP_KEY`; unlike test 2, position
within the value never selects between two of them.

RFC 8259's `JSON-text` production admits insignificant whitespace before and
after the value, so a byte string carrying a trailing newline or a leading
space is one JSON text and fails at test 6 as `E_NOT_CANONICAL`. A byte
order mark is not whitespace under that production, so a byte string
beginning `EF BB BF` fails test 2 as `E_JSON`. Every variant spelling of a
value this universe admits (member order, escape choice) fails at test 6,
except that a spelling carrying a surrogate escape is named `E_JSON` by
test 2, a spelling whose escape decodes to a scalar value a charset refuses
is named by test 4 or test 5, and a spelling of a number outside §3.2's
form, a fraction, an exponent, a leading zero, or a sign among them, is
named `E_NUMBER` by test 2, and a spelling RFC 8259's grammar does not
admit at all, a raw byte in 0x00 through 0x1F inside a string among them,
is named `E_JSON` by test 2; a byte string that spells no value of this
universe, an unknown literal such as `NaN`, `Infinity`, or `undefined`
among them, fails at test 2 as `E_JSON`, except that `-Infinity` opens a
value position of numeric shape and fails as `E_NUMBER`. None of them needs
an individual prohibition.

## 4. Envelope

**[E] 4.1 Seven members, closed.** An entry is a canonical object with
exactly these seven members and no others:

| key | form | rule |
|---|---|---|
| `spec` | string | byte-equal to `zikaron/1`; a `spec` that is not a string fails the same test |
| `entryType` | token | open enumeration (§6.9) |
| `author` | hex20 | the signer of this entry (§5.5) |
| `seq` | int | position in the chain (§7.1) |
| `prev` | `null` or hex32 | `null` iff `seq` is 0 (§4.3 step 8); what a non-null value links to is chain law (§7.2) |
| `body` | object | typed per `entryType` (§6); an object for every type, known or unknown |
| `sig` | hex65 | the EIP-191 signature (§5) |

Keys are unique (§3.5), so the member set is a set and is decided in two
tests: if any of `spec`, `entryType`, `author`, `seq`, `prev`, `body`, `sig`
is absent, the entry fails `E_ENVELOPE_MISSING`; otherwise, if any other key
is present, it fails `E_ENVELOPE_CLOSED`. Member count alone decides
neither, since an object of seven members can both lack one of the seven and
carry an eighth key. Both tests are decided before the signature is
examined. The chain rules of §7 are not entry predicates: a byte string
whose `prev` names no entry a verifier holds is an entry all the same, and
§8.2 records the mismatch when both sides are in hand.

**[E] 4.2 Genesis placement.** `entryType` is `genesis` iff `seq` is 0. A
`genesis` entry at any other position, or a non-`genesis` entry at position 0,
fails `E_GENESIS_PLACE`. One key opens one chain, and this rule is where the
grammar says so (§7.5).

**[E] 4.3 Order of decision.** A verifier decides the tests below in this
order and reports the first failure's token. The order is part of the law, so
that every conforming verifier names the same fault for the same bytes.

1. §3.5 acceptance (its tokens).
2. The root is an object: `E_ENVELOPE`.
3. Member set is exactly the seven keys: `E_ENVELOPE_MISSING`,
   `E_ENVELOPE_CLOSED`.
4. `spec` is a string byte-equal to `zikaron/1`: `E_SPEC` (one token covers a
   `spec` of another form, since §10 gives `spec` no form token of its own).
5. `entryType` is a token: `E_ENTRYTYPE`.
6. `author` is hex20: `E_AUTHOR`.
7. `seq` is an int: `E_SEQ`.
8. `prev` is `null` or hex32: `E_PREV`; and `null` iff `seq` is 0:
   `E_PREV_SEQ`.
9. `body` is an object: `E_BODY`.
10. `sig` is hex65: `E_SIG_FORM`.
11. §4.2 genesis placement: `E_GENESIS_PLACE`.
12. §6 body table for the entry's type: `E_BODY_FIELD`.
13. §5 signature: `E_SIG_V`, `E_SIG_RANGE`, `E_SIG_HIGH_S`, `E_SIG_RECOVER`,
    `E_SIG_SIGNER`, decided in the bullet order of §5.4 followed by §5.5.

Where a step names more than one token, its tests are decided in the order
the tokens are written in that step. A byte string that passes all thirteen
is an entry. Nothing else is.

## 5. Signing

**5.1 Pre-signature bytes.** Let `E` be the seven-member envelope. `B6`
is the canonical bytes (§3.4) of the six-member object obtained from `E` by
removing the top-level member whose key is `sig`, and nothing else; a member
named `sig` at any depth inside `body` is untouched. `B6` is not an entry; it
exists only as a preimage. `presig = sha256(B6)`.

**5.2 Message.** `M = D || 0x0A || hex32(presig)`, where `D` is the
domain literal (§5.6), `0x0A` is one line feed byte, and `hex32` is as §1
defines it, so `M` ends in `0x` and 64 lowercase hexadecimal digits. For
`D = zikaron/1`, `M` is exactly 76 bytes. Under `zikaron/1` the 32-byte value
is `presig` (§5.1); under `zikaron/1-adoption` it is `sha256(C)`, which §6.6
defines and spells out there, and `M` is exactly 85 bytes.

**5.3 Digest.** `digest = keccak256(0x19 || P || decimal(len(M)) || M)`,
where `P` is the 24 bytes `Ethereum Signed Message:` followed by one line
feed byte `0x0A`, so `P` is 25 bytes and `0x19 || P` is the 26-byte EIP-191
version-0x45 prefix (the `E` of `Ethereum` is the version byte 0x45), and
`decimal(len(M))` is the byte length of `M` written in ASCII decimal with no
leading zeros (`76` for entries under `zikaron/1` and `85` for adoption
attestations under `zikaron/1-adoption`; a literal of another length gives
another decimal, which the signer computes from `M` itself). `decimal(len(M)) || M` is injective over all messages: the width of
`decimal(l)` is non-decreasing in `l`, so a shorter message yields a
strictly shorter concatenation, and two concatenations of equal length
therefore carry messages of equal length, equal decimal prefixes, and equal
bytes.

**[E] 5.4 Signature form.** `sig` decodes to 65 bytes `r || s || v` with `r`
and `s` as 32-byte big-endian integers and `v` one byte. The closed
boundaries, each tested by every verifier:

- `v` ∈ {27, 28}: `E_SIG_V`.
- 1 ≤ `r` ≤ n − 1 and 1 ≤ `s` ≤ n − 1: `E_SIG_RANGE`.
- `s` ≤ (n − 1) / 2, that is `s` ≤
  `0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0`:
  `E_SIG_HIGH_S`. Low-s is required so that the malleability twin
  `(r, n − s, 55 − v)`, which flips `v` between 27 and 28 and which any
  reader can compute from a published signature without holding the key, is
  not a second valid encoding of the same authorization: nobody can manufacture a second `entry_id` from bytes
  the author published. A signer who varies the nonce can still produce a
  second valid signature over one `B6`, which is why §5.7 obliges RFC 6979;
  a verifier holding both copies records equivocation under §8.4, so the
  obligation is enforced by the author's own exposure.
- Public key recovery from `(digest, r, s, i)`, `i = v − 27`, succeeds:
  `E_SIG_RECOVER`. Let `z` be `digest` read as a 256-bit big-endian integer
  reduced modulo `n`, and let `R` be the point of secp256k1 whose
  x-coordinate is `r` and whose y-coordinate is even when `i` is 0 and odd
  when `i` is 1, the group order `n` lying below the curve's field prime so
  that every `r` the range test admits is already a field element and no
  reduction precedes the curve equation. If no such point exists, recovery
  fails. Otherwise
  `P = r^(−1)(sR − zG)`, where `r^(−1)` is the multiplicative inverse of
  `r` modulo `n`, all scalars are taken modulo `n`, and the point arithmetic
  is on secp256k1; recovery fails if `P` is the point at infinity. Because
  this grammar admits only `v` ∈ {27, 28} and defines `R` as the point whose
  x-coordinate is exactly `r`, the candidate x-coordinate `r + n` that a
  general recovery would also try is never formed, and no recovery id above
  1 exists here.

**[E] 5.5 Signer equals author.** The recovered public key `P` yields an
address `A = last 20 bytes of keccak256(X || Y)`, where `X` and `Y` are
`P`'s affine coordinates each written as exactly 32 bytes big-endian,
left-padded with zero bytes, so `X || Y` is the 64-byte uncompressed
encoding of `P` without the 0x04 prefix. `A`, written as
hex20, must be byte-equal to `author`: `E_SIG_SIGNER`.

**5.6 Domains.** `zikaron/1` defines exactly two EIP-191 domains, and the
list is closed: `zikaron/1` for entries (§5.2) and `zikaron/1-adoption` for
adoption attestations (§6.6). Every message of this ecosystem has
one shape, and a length its literal fixes: a domain literal containing no line
feed, one line feed, and a 32-byte value written by §1's `hex32`, so `0x`
and 64 lowercase hexadecimal digits. Two domains are separated by their
literals: no literal carries a line feed, so each message's literal is
exactly the bytes before that message's first line feed, and two distinct
literals yield messages that differ at some byte, whether the literals
differ at a shared position or one is a proper prefix of the other (the
shorter message then carries `0x0A` where the longer carries the next byte
of its literal); EIP-191 binds the entire message into the digest preimage,
so two messages that differ anywhere have different digests. No byte string
is therefore a valid message under two domains of this shape, and a
signature made under one is not a signature under another. Lengths (76 and
85 here) decide nothing; the literal decides. A future domain in any
`zikaron` version keeps this shape with a literal that differs from every
earlier literal of this family and from every literal of any other law
whose messages share this shape; equal lengths are permitted (`zikaron/2` yields 76-byte messages like `zikaron/1`). A
signature made outside this family over a byte string of an entry
message's shape is a valid entry signature; §5.7 states the producer
obligation that closes that door.

**5.7 Producer obligations.** Producers sign with RFC 6979 deterministic
nonces, the HMAC of that procedure being HMAC-SHA256 and the value handed
to it as its message input the 32 bytes of `digest` (§5.3), hashed no
further, the procedure's own reduction of that input and its rejection of
an out-of-range candidate applying as it states them, and a candidate whose
`r` is not the x-coordinate itself being rejected like an out-of-range one,
so that `v` is always 27 or 28; producers emit low-s encodings by replacing
an `s` above (n − 1) / 2 with n − s and flipping `v` between 27 and 28,
never by drawing a further nonce for that reason; two conforming producers
therefore
sign one `B6` under one key to one `sig`, and a seat that signs one position
on two tools publishes one entry. A verifier cannot test determinism and
does not try; its tests are §5.4 and §5.5, which are total. An author's key signs
`zikaron/1` entries, `zikaron/1-adoption` attestations, and messages of
§5.2's shape under domain literals that no domain of this law spells (a
client's own documents, say), all produced by the author's own
tooling, and nothing else: it never answers an EIP-191 `personal_sign`
request from any application, because the domain literal lives inside the
signed payload, and a 76-byte string of the shape `zikaron/1`, line feed,
`0x`, 64 lowercase hexadecimal digits, presented by anyone, is a complete
entry signature for an entry the author has not read. An 85-byte string of
the shape `zikaron/1-adoption`, line feed, `0x`, 64 lowercase hexadecimal
digits, presented by anyone, is likewise a complete adoption attestation
over anchors the signer has not read, so a key that ever sent a transaction
an author may adopt answers no such request either; the obligation binds
every key of this ecosystem and this grammar tests none of it (§11). A verifier cannot
test this obligation either; it is the reason an author's key carries
nothing but the archive: money, escrows, and every convenience of a smart
account live on other addresses. Authors' keys are raw secp256k1 EOAs. An author's key
authorizes no EIP-7702 delegation: §9.3 gives an address carrying a
delegation designator code, so a delegation voids every anchor sent from
that key while the designator stands, and delegation convenience belongs to
a separate address that carries no archive; an authorization the author
signed once and does not hold, one with chain id 0 included, is enough for
a third party to land a designator in the block of the author's anchor, so an author's key signs no such authorization at all. A verifier cannot test this
obligation before the fact either; §9.3 is where its cost is charged.
ERC-1271 and every contract-mediated signature are rejected by
construction, since §5.4 admits only a 65-byte recoverable signature: a
permanent archive verifies offline with pure math forever, and nothing whose
answer depends on chain state may stand in for a signature.

## 6. Entry types and bodies

**[E]** `body` is an object for every type. For the seven known types, the
tables below state each field's presence (required or optional), its form,
and the test applied when it is present. The seven are `genesis` (§6.1),
`history` (§6.2), `grant` (§6.3), `revocation` (§6.4), `adoption` (§6.5),
`succession` (§6.7), and `annotation` (§6.8); §6.6 states the adoption
attestation and names no type. A member is
**present** when its key is present, whatever its value; the only spelling
of an absent optional field is the absent key, and an optional field written
with the value `null` is present with a form its row refuses. A field listed
here fails `E_BODY_FIELD` when it is required and absent, when it is present
with a form its row refuses, or when a relation a row of this section states does not hold: `from` ≤
`to` inside a `window` (§6.3), and `attestor` and `attestation` present
together or absent together (§6.5). No other row states one. Every other gloss in a form cell
says what the author means the value to be and states no predicate; whether
a reference names an existing entry, or an entry of the type its row names,
is reading law and never a fault of this step (§11). One
token covers every body fault, so no order of tests within a body is part of
the law.

A row's form names the shape a member must have at §4.3 step 12, after §3.5
has already decided charset and number shape at step 1. A row whose form is
`prose string` tests that the value is a string and nothing else; a row
whose form is `token` tests that the value is a non-empty string carrying no
U+0020, the rest of its charset being already decided; a row whose form is `int` is
failed at this step only by a value of non-numeric shape (`true`, `false`, a string, `null`, an
array, an object), every numeric
spelling outside §3.2 having failed `E_NUMBER` at step 1. Where a row's form
repeats a §3 predicate exactly, that half of the row has no reachable
failing side and contributes no boundary to §12.1; a `token` row does not
repeat one, U+0020 being the one scalar value §3.5 admits and §1's `token`
refuses, and its exclusion is a boundary of this section. The shortcut is
sound because no key of a `token` row, and no key on the path from the body
to one (`mode` and `anchors`, which are the only two), ends in `_md`, so no such value
lies inside a prose subtree. An array field is satisfied by an
array of zero elements unless its row says otherwise, and §6.5's `anchors`
is the one row of this section that says otherwise: an empty `anchors`
array fails `E_BODY_FIELD`, and the rule is §6.5's row, so a body of
another type carrying an empty array under that key is untouched by it. Every value this section matches (the seven type names, and nothing else,
since `mark`, `payloadKind`, and `kind` are open tokens) is matched by byte
equality of the decoded string against the literal as spelled here; a value
that differs in one byte, case included, is a different value, and §6.7's
`handover` and `keyrotation` are examples this grammar names and never
matches. Where a row says "object; members beyond
these permitted", the row names the complete set of tests applied to that
object and never the complete set of members it may carry (§6.10). Within an
entry, only the envelope of §4.1 is arity-closed. Outside an entry, §6.10
reaches nothing this law spells with an exact member list, and four such
lists exist: the scan basis with its `chains`, `bareTx`, and `adoptionChains` objects (§9.4),
which is an audit input; the report of §8.7 with its findings, its rows,
and the `anchors` elements inside a `MISSING` or `ANCHORED` row, which is an
audit output; the `NO_LABEL` object of §8.7, which §9.4's
well-formedness test produces and which carries its two members and nothing
else; and the attestation preimage object of §6.6, which a verifier builds
from three values the entry carries and no byte string supplies. Two records this law spells with named members are not
arity-closed: §9.4 reads the seven members §9.2 names on an anchor record,
and the four §9.2 names on an adoption evidence record, and ignores every
member beyond them.

**6.1 `genesis`.** Opens the chain (§4.2).

| field | presence | form |
|---|---|---|
| `statement_md` | required | prose string: what this ledger registers, in the author's words |

**6.2 `history`.** A content-addressed snapshot: a digest of any content,
fixed at this entry's position in the chain.

| field | presence | form |
|---|---|---|
| `content` | required | hex32: a 32-byte digest of the snapshot |
| `mode` | required | object; `mark` required token, `toolchain` required hex32 (a 32-byte digest identifying the toolchain, under the mark's own convention; its row tests the hex32 form and no predicate reads its value); members beyond these permitted (§6.10) |
| `note_md` | optional | prose string |

The grammar records `content` as 32 bytes and verifies nothing about what it
digests: how it was computed (which function, over which bytes) is disclosed
with the bytes that match it, in a sale or a suit, and the mode's mark names
the toolchain that can say. Every history entry names its producing mode,
and a hand-anchored chain names a mark for the hand: an unnamed digest is a
number nobody can recompute, and the grammar declines to record one. A
20-byte digest (a SHA-1 git object name, say) is not a legal `content`; a
mode that anchors git history publishes a 32-byte digest of the commit
object's bytes, or of whatever it chooses, and says so in its mark. Two
history entries with equal `content` are two legal entries.

**6.3 `grant`.** An authorization published.

| field | presence | form |
|---|---|---|
| `grantee` | required | hex20: any address; a contract address and the author's own address are legal |
| `work` | required | hex32: the content digest of the work granted, in the sense of §6.2 |
| `terms` | required | hex32: digest of the terms document |
| `history` | optional | hex32: the `entry_id` of a history entry the author points to for this work |
| `window` | optional | object; `from` required int, `to` required int, `from` ≤ `to`; members beyond these permitted (§6.10) |
| `scope_md` | optional | prose string: the grant's scope in the author's words |

`work` is always a content digest and never an `entry_id`; an author who wants to point at a particular history entry uses `history`. Whether the
pointed-to entry exists, and whether its `content` equals `work`, is reading
law (§11); the grammar tests forms. `window` is data for readers and judges:
its meaning, and what a grant with no window means, live in the terms. Two
grants naming one `work` are two authorizations over one digest and say
nothing about each other. Whether either excludes the other, and whether two
different `work` digests are two digests of one work, live in the terms
documents each grant hashes and in nothing this grammar publishes: the
ledger fixes the order and the parties, and a party who needs a conflict to
be publicly readable puts the fact it turns on into a document whose hash
the grant carries.

**6.4 `revocation`.** This ledger withdraws a grant it published.

| field | presence | form |
|---|---|---|
| `grant` | required | hex32: the `entry_id` of a grant entry in this same ledger |
| `case` | optional | hex32: the case file hash of a ruling the withdrawal rests on |

A revocation whose `grant` names no grant in this ledger is a legal entry
whose reference dangles (§11); a revocation naming a grant in another ledger
withdraws nothing there, because withdrawal is an act of the ledger that made
the grant. `case` is the 32-byte digest of the ruling document the
withdrawal rests on, whatever forum issued it: a judge's case file in this
ecosystem, a court's judgment, an arbitral award; the terms the grant
hashed name the forum, and a reader verifies the citation by being shown
the document. A revocation with no `case` says exactly what it
says: withdrawn, on no cited authority. Several revocations of one grant are
several legal entries.

**6.5 `adoption`.** Prior publications declared part of this history.

| field | presence | form |
|---|---|---|
| `anchors` | required | non-empty array; every element an object; `chainId` required int, `tx` required hex32, `payloadKind` required token, `content` required hex32; members beyond these permitted (§6.10) |
| `attestor` | optional | hex20 |
| `attestation` | optional | hex65 |

`attestor` and `attestation` are present together or absent together
(`E_BODY_FIELD` otherwise). `payloadKind` is the author's label for how
`content` sits in the adopted transaction's calldata; it is data for readers: its row tests the token form and nothing more, and no predicate reads its value (the one shape test the grammar applies to an adoption is §9.5's, over an element's `content` and the record's calldata, and none is over `payloadKind`). Elements may repeat; order is the author's. A chain whose id is
at or above 2^53 cannot be named by `chainId` here, in an anchor record or an
adoption evidence record (§9.2), or in a basis (§9.4), the universe of §3.1 having no larger
integer; a transaction on such a chain is adopted by no element this
grammar reads, and an anchor there is outside every basis (§9.4).

**[E] 6.6 Adoption attestation.** When present, the attestation is a
signature by `attestor` over the message
`zikaron/1-adoption || 0x0A || hex32(sha256(C))`, where `C` is the
canonical bytes (§3.4) of the three-member object
`{"adopter": author, "anchors": anchors, "prev": prev}`: `author` is this
entry's `author` as hex20, `anchors` is this body's array byte for byte, and
`prev` is this entry's `prev` as it stands in the envelope (an adoption is
never at `seq` 0, so `prev` is hex32). The message is 85 bytes. Digest,
form, range, low-s, and recovery follow §5.3 through §5.5, with the
recovered address required byte-equal to `attestor`. The attestation names
the adopter, the anchors array, and the head it was written against, and
licenses, under §9.5, every adoption entry whose `B6` carries that `author`,
that `anchors` array, and that `prev`, and licenses no other entry: copied into another
author's entry it fails because `adopter` differs, and copied into an entry
written on a different head it fails because `prev` differs, while two
entries of one author on one head that differ only in `seq`, in `sig`, or
in body members other than `anchors`, `attestor`, and `attestation` all
carry it and are one `EQUIVOCATION` (§8.4) on the shared `prev`. An attestor who intends a standing licence signs each
entry it means to license.

An attestation that fails any of these tests does not void the entry: the
entry stands as §6.5 tested it, and it supplies §9.5's second licensing route for no element, so an element
the attestation would otherwise have licensed is unproven unless §9.5's
first route stands and the record's sender is in the prefix lineage; the
two routes are a disjunction and the first stands whatever the attestation
does, while §9.5's other two tests stand whichever route is taken. Entry validity never depends on a key the author
does not hold, and no rejection token names an attestation.

**6.7 `succession`.** The ledger changes hands.

| field | presence | form |
|---|---|---|
| `to` | required | hex20: the key that signs every later entry (§7.3) |
| `kind` | required | token: why the seat moved; the grammar names `handover` and `keyrotation` as examples, reads neither, and refuses no token beyond them |
| `effective` | required | int: Unix seconds, data for readers |
| `statement_md` | required | prose string |

`kind` records why the seat moved; every token is a legal value, the chain
predicates of §7 treat every value alike, and a future reason for a seat to
move needs no `zikaron/2`. `to` may equal `author`, and may name a key of
this ledger's own lineage that held office before; §7.3 says what follows,
and §7.5 says why `to` never names a key that has opened a ledger of its
own. A ledger that stops has no closing entry to write: authors carry no
duty to keep writing, and silence is the ordinary end.

**6.8 `annotation`.** A note about an earlier entry, or about the ledger.

| field | presence | form |
|---|---|---|
| `subject` | optional | hex32: the `entry_id` of an entry in this ledger |
| `note_md` | required | prose string |

Published bytes are never edited; a correction is an annotation.

**[E] 6.9 Unknown types.** `entryType` is an open enumeration. An entry whose
type is not one of the seven above verifies with §4 in full, its body an object by §4.3 step 9 like every
other body, and §6.10 alone reaching that body's members: nothing beyond the
object test applies, and §4.3 step 12 never fails for such an entry.
Readers that do not recognize a type keep the entry, its position, and its
signature, and attach no meaning to its body. An entry of an unrecognized
type is an entry all the same, and an unrecognized type buys an author no
silence: a report that this ledger is silent on grants, revocations,
adoptions, or history covers only the types its reader recognized, and §11 leaves to readers and judges what such
a claim is worth beside the `UNKNOWN_TYPE` list of §8.7, which names every ledger entry
whose type is none of the seven of this section, whatever readings its
verifier's operator knows, and from which a reader strikes the types it
reads by some convention of its own to be left with the entries nobody
read. The same holds of §8.5, which gives an
unknown type no exemption from `UNANCHORED`, and §8.7 lists every ledger
entry of a type this grammar does not know, so that a claim of silence can
be checked against what the reader did not read. This is the evolution
valve: a later convention may add types here, and a reader that knows one
applies that type's table as a reading test of its own (§11) and never as a
`zikaron/1` entry predicate. No convention adds a fault to §4.3 step 12 and
none removes one; a byte string of an unrecognized type that passes §4.3 is
an entry for every verifier, including one that reads the convention and
finds the body wanting.

**[E] 6.10 Extra members.** Every body may carry members beyond its table, at
any depth, under the value rules of §3. Such members are data: no predicate
in this grammar reads their meaning, and no verifier accepts or rejects an
entry because of them. They are nonetheless inside the bytes the entry's
own preimage `B6` (§5.1) covers, and, where they sit inside an `anchors`
element of an adoption, inside the attestation preimage `C` (§6.6) as well,
so an extra member added to or removed from an `anchors` element after an
attestor signed invalidates that attestation and can change whether §9.5
records `ADOPTION_UNPROVEN`. Whether a document the entry hashes gives an
extra member meaning between the parties to that document is the parties'
affair (§11). What no extra member's meaning ever supplies is any output of this
grammar: validity under §4, authority under §7, any finding of §8.2 or §8.4
whether hard or soft, any row or list of §8.7 other than the
`ADOPTION_UNPROVEN` row an attestation's preimage can turn on, the label,
or an anchor verdict of §9.3. Every predicate of this grammar reads the members its rows
name and the members §4.1 names, and nothing else; a reader that derives
any output from what an extra member says is reading its own convention
into bytes that carry none. Identity is another matter and is total
(§2.1): an extra member is part of the bytes, so two byte strings that
differ only in one are two entries with two `entry_id`s, they do not
collapse under §8.1, and at one `seq` they equivocate under §8.4 like any
other pair.

**[E] 6.11 No sentinels.** Inside a body, every one of the 2^256 values of
a hex32 field, every one of the 2^160 values of a hex20 field, every one of the 2^520
values of a hex65 field, every integer of §3.2 in an `int` field, every string a `prose string` row
admits in such a field, and every token of §1 in a `token` field (no row of
this section closes a token's set) is a legal spelling of that field, the
all-zero hash, the zero integer, the one-byte token, and the empty prose
string included; a legal spelling is not by itself a legal body, since
§6.2's `mode`, §6.3's `window`, and §6.5's `anchors` elements require
members of their own,
§6.5's `anchors` is non-empty, §6.3's `window` orders its two integers, and
§6.5's `attestor` and `attestation` come together. The grammar reserves no "unset" marker inside a body: `null` is a
value of the universe (§3.1), a body field that may be absent is optional
and is spelled by the absent key, and a present body field means what its
author wrote. The one place a `null` carries a fixed meaning is the
envelope's `prev`, where §4.1 gives it that meaning explicitly and
`E_PREV_SEQ` enforces it.

## 7. Chain

**[C] 7.1 Sequence.** Entries of one ledger carry `seq` 0, 1, 2, … with no
gap and no repeat.

**[C] 7.2 Link.** For `seq` ≥ 1, `prev` equals the `entry_id` of a ledger
entry at `seq − 1` (§8.1): a commitment to that entry's exact published
bytes. Where more than one ledger entry occupies `seq − 1`, the link is
satisfied by any one of them and §8.4 records the fork separately; §8.2
states the test, and an entry the lineage left out (§8.7's `EXCLUDED`)
satisfies no link. Through this
link every entry commits to the whole prefix before it along one line of
any fork.

**[C] 7.3 Authority.** A ledger is read under a **root**, the address the
reader names as its author. The authority at position `k`, written
`auth(k)`, is the key entitled to sign the entry at `seq` `k`:

- `auth(0)` is the root. The `author` of the entry at `seq` 0 (§4.1) is the
  address that entry claims as its seat; a `seq`-0 ledger entry whose `author`
  differs from the root is a fault (§8.2) and never redefines `auth`.
- If the entry at `seq` `k` is a `succession` whose `author` is `auth(k)`,
  then `auth(j) = to` for every `j` > `k`, until a later succession changes
  it again. A succession signed by a key that is not `auth(k)` is an
  authority fault at `k` and moves nothing. Where more than one succession
  at `k` carries `auth(k)` as its `author`, §8.2 states which one moves the
  seat.
- Otherwise `auth(k + 1) = auth(k)`.

A ledger entry at `seq` `k` with `k` ≥ 1 must have `author` equal to
`auth(k)`; its
signature already recovers to `author` (§5.5), so the chain rule adds exactly
one test: the signer was the key in office. Position 0 carries no authority
test of this kind: a `seq`-0 entry is graded against the root alone (§8.2),
because §4.2 puts a `genesis` and nothing else there and no succession can
precede it. A succession to the key already in
office, to the root, or to a key that held office before is legal; authority
is a function of the prefix and switches as written. A key that has handed
off and signs again at a later position is an authority fault (§8.2).

**[C] 7.4 Lineage.** The **lineage** of a root is the least fixpoint seeded
with the root and extended by the `to` of every byte string in the pile that
passes §4.3, is a `succession`, and whose `author` is already in the set, at
whatever `seq` ≥ 1 it sits; §4.2 puts a `genesis` and nothing else at `seq`
0, so no succession opens a chain and the seed is the root alone. It decides which entries are input at all (§8.1) and which anchor
records are this seat's (§8.1): every lineage key, the keys that have held
the seat and the keys a lineage key ever named as `to`, can put a hash into
this ledger's reconciliation, whenever it sent the transaction, and no key
outside the lineage can. A second fixpoint over the same successions serves one question of §9.5: the
**prefix lineage at `k`** is the least fixpoint seeded with the root and
extended by the `to` of every ledger entry (§8.1) that is a `succession`
with `seq` less than `k` and whose `author` is already in the set, the two
fixpoints ranging over one population, since a succession whose `author` is
in the lineage is a ledger entry by §8.1; it decides which keys could have
licensed an adoption's transaction when the entry at `k` was written, competing successions at one position all
extending it, because the question is which signatures could have licensed
an act and never which twin held office. A key that joins the lineage later
never retroactively authorizes an earlier entry, signature, or adoption;
authority at a position is the walked state of §7.3, and the lineage is the
wider set of the root and every key some lineage key has ever named as
`to`, whether or not that succession moved the seat.

**[C] 7.5 One key, one ledger.** A key opens one chain: §4.2 puts every
genesis at `seq` 0, so two ledgers under one key are two entries at `seq` 0
by one signer, which §8.4 records as equivocation. A succession therefore
names a key that has opened no `zikaron/1` chain of its own and holds no
seat under any other ledger law (§12.4) (a `zikaron/2` genesis is no such chain,
since it fails §4.3, at `E_SPEC` where it keeps this envelope and earlier
where it does not, and never enters this law's inputs, which is what makes
the two-sided migration of §12.3 lawful here; a genesis under any other
law fails §4.3 the same way and merges no entries, and what such a `to`
imports is that ledger's anchors): a `to` that already holds a `zikaron/1` ledger merges that ledger
into the audited set under the ceding root alone, where the audit reads
`BROKEN_CHAIN` from the doubled `seq` 0, while the audit under the
recipient's own root is untouched, because lineage runs forward only from
the root a reader names (§7.4) and the ceding ledger's entries are
excluded there (§8.1); the grammar cannot tell that merger from a fork and
does not try. A seat is handed to a fresh key, and §6.7's permission to name a key that held office before means a key
of this ledger's own lineage. The uniqueness property is detectable exactly when both ledgers
reach one verifier's inputs and the audited root is the one whose lineage
reaches both; a clean label never asserts that no second ledger exists,
only that none was in the inputs. It follows that any lineage key can name a stranger's address as `to` and
force
`BROKEN_CHAIN` on this seat's own audit by importing that stranger's
entries; the exposure is the custody burden §8.4 states, and no reading of
this grammar lets it reach the stranger's audit. Uniqueness is a claim
readers can falsify, never one a label certifies.

**7.6 No clocks.** `effective`, `window`, and every other time-valued field
are data; the only test any of them carries is the ordering of `from`
against `to` inside a `window` (§6.3). No predicate in §7 or §8 compares a
clock or a timestamp to anything, and
§9.4 reads `blockTimestamp` only to decide whether two records are one; the only
time this grammar knows is the order of blocks that carry anchors (§9), and
that enters only through the anchor set a verifier declares.

## 8. Audit

An audit is a pure function of five inputs:

1. the audited **root** address;
2. a finite multiset of candidate byte strings, the **pile**;
3. an **anchor set** of records `(chainId, blockNumber, blockTimestamp, tx,
   sender, hash, verdict)` (§9.2), where `verdict` is `counted` when the
   codeless test of §9.3 was decided in the anchor's favour, `UNPROVEN`
   when either of its two states could not be consulted, and `VOID` when
   both were consulted and the test was decided against it, with the scan
   basis that produced it (§9.4);
4. an **unavailable set** of hashes (§8.6);
5. an **adoption evidence set** of records `(chainId, tx, sender, calldata)`
   for the transactions adoption entries name (§9.5), under the same basis.

Its output is a report (§8.7). The same five inputs produce the same report
in every conforming implementation. The pile is a multiset, the anchor set,
the unavailable set, and the evidence set are sets, and the basis has one
spelling per scan (§9.4), so no input carries an order that could reach the
report.

**[C] 8.1 Input.** Each byte string in the pile is tested by §4.3. Those that
fail are not entries and are not ledger entries: they enter no walk, no
`have`, and no `EXCLUDED` row, their `entry_id` is nonetheless computed
(§2.1) for the trimming below and may appear in the anchor set (§8.6), and
§8.7 lists them as `MALFORMED`. Byte-identical copies collapse to one, whether or not they are entries. Of the
remaining entries, the **ledger** is the lineage-signed subset (§7.4): an entry whose `author` is outside the lineage is not suspicious, it is no
ledger entry, and §8.7 lists it as excluded. Nobody is framed by junk
published near their ledger, and nobody launders their own record by
self-pollution, because signatures decide membership and nothing else does.
The audit then discards, in this order: first every anchor record whose
`sender` is not in the lineage, so that no key outside the lineage can put a
hash into this ledger's reconciliation and every lineage key can; then from the unavailable set every hash meeting either of two conditions,
that it is not the `hash` of an undiscarded record with verdict `counted`,
or that it is the `entry_id` (§2.1) of a byte string in the pile, whether
those bytes are a ledger entry, an entry §8.7 lists as excluded, or a byte
string that fails §4.3, because bytes in hand are in hand whatever any
other source answered.
The lineage is a function of the pile (§7.4), so a seat that withholds one
succession shrinks it and with it the anchor records the audit reads.
Nothing the lineage trim discards leaves the report silently: §8.7's
`DISCARDED` list carries every anchor record whose `sender` the lineage did
not hold, a list bounded by the `senders` and `bareTx` the reader itself
declared, and its `EXCLUDED` rows carry the `author` of every entry the
shrunken lineage left out. A reader who sees there an address the ledger's
successions never name asks for the succession that would admit it. No
label turns on the list, since a reader names the addresses it scans and a
stranger's anchor would otherwise charge this seat; what a seat that
withholds a succession buys is a clean label beside a row it cannot erase,
under every basis whose `senders` or `bareTx` reached the withheld key, and
silence under every basis that did not.

**[C] 8.2 Walk.** Ledger entries are ordered by `(seq, entry_id)` with
`entry_id` compared bytewise. The walk visits them in that order and records
the findings of this section from the closed vocabulary of §8.3; §8.4's
finding is recorded over pairs and by no visit. It tracks `expected`, the next
`seq` it expects (initially 0), `auth`, the authority in force (initially
the root), `certain`, true until the first gap, and `last`, the `seq` of the
entry visited immediately before the current one, absent at the first visit
and set at the end of every visit, gap or no gap. A visited entry whose
`seq` equals `expected` records no gap, leaves `certain` unchanged, and sets
`expected` to that `seq` plus 1, except that where the `seq` is 2^53 − 1 the
walk leaves `expected` at that value; `last` then equals `expected` at the end of that visit, so the next entry
at that `seq` satisfies both the "equals `expected`" rule and the fork-twin
clause of the first bullet, and the two agree: no gap, and `expected` and
`certain` unchanged. Within one visit the tests are
decided in the order the bullets below are written: `SEQ_GAP` first, so
that the entry whose `seq` opens a gap is graded for authority with
`certain` already false and its own `AUTHORITY_MISMATCH`, if any, is soft.
The first visited entry has no "`seq` visited just before it" and is judged
by `expected` alone. Every finding of this section carries the `entry_id` of the entry visited
when it was recorded.

- **`SEQ_GAP(expected, actual, entry_id)`**, soft: the visited entry's `seq`
  differs from `expected` and from the `seq` visited just before it.
  Afterwards `expected = actual + 1` and `certain = false`; where `actual`
  is 2^53 − 1 the walk leaves `expected` at `actual`, since no entry can
  occupy a position beyond the universe of §3.1. A ledger whose smallest
  `seq` is not 0 records `SEQ_GAP(0, s, …)` first. An entry whose `seq`
  equals the `seq` visited just before it is a fork twin: it records no gap
  and leaves `expected` and `certain` unchanged; §8.4 records the fork.
- **`PREV_MISMATCH(seq, entry_id)`**, hard: the visited entry has `seq`
  `s` ≥ 1, at least one ledger entry has `seq` `s − 1`, and none of those has
  `entry_id` equal to the visited entry's `prev`. The test is over the set,
  so the order of twins at a fork cannot change it. Across a gap no entry at
  `s − 1` exists, the link is unverifiable, and no prev finding is recorded:
  the gap already is the finding.
- **`AUTHORITY_MISMATCH(seq, certain, entry_id)`**, hard iff `certain`: the
    visited entry has `seq` ≥ 1 and its `author` differs from `auth` for its
  position. Authority is walked state, and walked state is certain only
  while the walk has been contiguous from 0, because a gap could hide a
  succession; after a gap the same finding is recorded soft. After every
  entry at position `k` has been visited, if any of them is a succession
  whose `author` equals `auth` for position `k`, `auth` for positions after
  `k` becomes the `to` of, among those successions, the one with the
  bytewise-smallest `entry_id`; where no succession at `k` carries that
  `author`, `auth` is unchanged, and every succession at `k` has already
  recorded `AUTHORITY_MISMATCH` for the same reason. A fork at `k` is
  already hard; this rule fixes which findings follow it, and it keeps
  authority on the line the key in office wrote, so that no retired key
  moves the seat by publishing a second succession at a position it no
  longer holds.
- **`ROOT_MISMATCH(seq, actual, entry_id)`**, hard, `seq` always 0, `actual`
  the entry's own `author`: an entry at `seq` 0 whose `author` is not the
  root. Position 0 is tested by this
  finding alone and never records `AUTHORITY_MISMATCH`, whose subject is the
  succession state position 0 cannot yet have. It is tested over every
  `seq`-0 ledger entry, so a lineage key opening a competing root under the
    audited seat is a hard fault. Absence of any `seq`-0 entry convicts
  nobody: where the ledger holds an entry at some later position, the
  missing position 0 is the first `SEQ_GAP`, and where the ledger holds no
  entry at all the walk records nothing and the report's entry count
  carries the whole of the answer.

**[C] 8.3 Hard and soft, and the closed vocabulary.** A finding is hard iff
its entire witness is in hand: two lineage-signed entries that contradict
each other, one lineage-signed entry that contradicts the audited root, or a
walked-state contradiction reached while the walk was certain. Absence never
convicts. `PREV_MISMATCH`, `ROOT_MISMATCH`, and `EQUIVOCATION` (§8.4) are
hard; `SEQ_GAP` is soft; `AUTHORITY_MISMATCH` is hard iff `certain`.
These five names are the closed vocabulary of the `findings` list of §8.7
item 3. `MISSING`, `UNANCHORED`, `EXCLUDED`, `ADOPTION_UNPROVEN`, `ANCHORED`,
`UNKNOWN_TYPE`, `MALFORMED`, and `DISCARDED` name lists of §8.7, are
outside the partition, are never hard, and are never findings of that list;
no anchor verdict ever names an adoption element, which is reported as
`ADOPTION_UNPROVEN` or not at all (§9.5); the four labels of
§8.7 are the closed label set; `counted`, `UNPROVEN`, and `VOID` are the
closed anchor verdicts (§9.3).

**[C] 8.4 Equivocation.** `EQUIVOCATION(seq, a, b)`, hard: two ledger entries
with different `entry_id` that share a `seq`, or share a non-null `prev`,
where `a` and `b` are their `entry_id`s in bytewise ascending order and
`seq` is the smaller of their two `seq` values. One finding is recorded for
each such unordered pair, whatever number of grounds the pair meets, so
three entries colliding at one position record three findings.
Byte-identical copies are one entry and never equivocate. Which lineage key
signed each side is irrelevant. A fork therefore cannot be manufactured by
any key outside the lineage and can always be manufactured by any key
inside it, a key that handed off long ago included: `BROKEN_CHAIN` says that
some key of this seat's lineage equivocated, and never which one is at
fault. Custody of retired keys is part of the author's whole burden, and so
is custody of signatures: a `sig` an author's tooling produced over bytes
the author chose not to publish is a complete second entry from the moment
it leaves the author's hands, so a producer signs a position once and
publishes exactly what it signed. A reader who wants to know which side of
a fork was written first prices the anchors (§9.6), which is the only
evidence of order this grammar carries.

**[C] 8.5 Two-way reconciliation.** Let `have` be the set of `entry_id`s of
ledger entries, and let the **counted anchor set** be the `hash` values of
the anchor records whose `verdict` is `counted`, as trimmed by §8.1.

- **Forward, `MISSING(hash)`**: every `hash` in the counted anchor set that
  is not in `have` and not in the unavailable set as trimmed by §8.1. The
  author's seat committed to bytes and the bytes are not produced; the
  commitment is on a chain and outlives the pile, and what a seat can still
  do is shrink the lineage by withholding a succession, which §8.1 answers
  with the `DISCARDED` list.
- **Backward, `UNANCHORED(entry_id)`**: every ledger entry whose `entry_id`
  is not in the counted anchor set, listed regardless of type. This list is
  informational: an entry needs no anchor to be an entry, and a reader who
  requires an anchor for a particular purpose (a grant before payment, say)
  reads this list to see it. It is type-agnostic so that no choice of
  `entryType` can hide an entry from it.

A third list carries the entries the forward direction found: §8.7's
`ANCHORED` names, for each entry in `have` whose `entry_id` the counted
anchor set holds, the records that hold it, so that the existence bounds of
§9.6 are computable from a report and never only from the anchor set that
produced it. Both directions match exact `entry_id`s and nothing else. `have` holds the
`entry_id`s of ledger entries and nothing else, so a hash whose bytes the
verifier holds under a signature outside the lineage is `MISSING` while
those bytes appear in `EXCLUDED` (§8.7); any lineage key can create such a
row, and the seat clears it only by a succession that admits that signer to
the lineage, which is for good. A `MISSING` row
that names no transaction is an accusation no reader can re-derive from a
chain, so the anchoring transactions travel with it (§8.7), and a reader who
doubts one re-scans the declared basis for that transaction. An anchor at a
later position bounds when earlier entries existed (§9.6); it does not put
their `entry_id`s in the anchor set and satisfies neither direction for them.

**8.6 Unavailable.** The unavailable set holds hashes from the anchor set for
which the verifier attempted retrieval and did not obtain usable bytes: the
storage did not answer, answered with a transport error, answered with
bytes whose `entry_id` is not the requested hash, or answered with bytes
the verifier refused to hold for size (§11). A definite negative
answer, a source reporting that it does not hold the hash, is not a
transport failure and leaves the hash in `MISSING`. A byte string that was
retrieved and fails §4.3 is not unavailable: it is no ledger entry (§8.1),
§8.7 lists it as `MALFORMED`, its `entry_id` is discarded from the
unavailable set by §8.1, and its anchor, if any, lands in `MISSING`. After the trimming of §8.1, every hash
in the counted anchor set lands in exactly one of three places: `have`, the
unavailable set, or `MISSING`. Unavailability is a fact about the verifier's
retrieval, never about a file's contents; both `MISSING` and the unavailable
set say that bytes this seat committed to are not among this ledger's
entries, and a reader who wants the accusation reads both lists together
with `EXCLUDED`, since an anchor of a hash whose bytes are in hand under a
foreign author's signature is a commitment this seat can never discharge.

**[C] 8.7 Report and label.** The report carries these fifteen contents,
sixteen members since item 1 carries two; the numbering names them and fixes each list's own order, and the order of
the members inside the report object is §3.4 item 5's. Where a sort key is
an `int`, the comparison is numeric ascending; where it is a string, the
comparison is the bytewise order of §1, and each list names which of its
keys are strings:

1. the root, and the declared basis (§9.4) as a value nested in the report,
   which §3.4 therefore spells canonically and §9.4 gives one spelling per
   scan;
2. the number of ledger entries;
3. the findings of §8.2 and §8.4, sorted by `(position, finding name,
   entry_id, second entry_id)`, where a finding's `position` is the `seq`
   argument the finding carries, `PREV_MISMATCH`, `AUTHORITY_MISMATCH`, and
   `ROOT_MISMATCH` each carrying exactly one, `SEQ_GAP`'s `position` is its
   `actual`,
   `EQUIVOCATION`'s `position` is its `seq`, which is the smaller of its two
   entries' `seq` values even where its `a` is the `entry_id` of the entry
   at the larger, `EQUIVOCATION`'s `entry_id` for this sort is its `a` and
   its `second entry_id` is its `b`, every other finding sorts with an empty
   `second entry_id`, which orders before every hex32 under §1's bytewise
   order, finding names compare bytewise, and each distinct finding, a distinct name
   carrying distinct arguments, appears exactly once, the four sort keys
   already separating every pair the walk and §8.4 can record, each name
   being recorded at most once per visited entry and `EQUIVOCATION` once
   per unordered pair;
4. `MISSING`, one row per hash carrying the `hash` and, in ascending
   `(chainId, blockNumber, tx)` order, the `(chainId, blockNumber, tx,
   sender)` of every counted anchor record undiscarded by §8.1 that
   committed to it, one element per record, `tx` comparing bytewise, the
   rows sorted bytewise by `hash`;
5. `ANCHORED`, one row per ledger entry whose `entry_id` is in the counted
   anchor set, carrying the `entry_id` and, in ascending `(chainId,
   blockNumber, tx)` order, the `(chainId, blockNumber, blockTimestamp, tx,
   sender)` of every counted anchor record undiscarded by §8.1 that
   committed to it, `tx` comparing bytewise, the rows sorted bytewise by
   `entry_id`. It is
   informational and no label turns on it. With `MISSING`, `UNANCHORED`,
   and the unavailable set of item 11 it closes the reconciliation on the
   face of the report: the `hash` of every counted anchor record
   undiscarded by §8.1 appears in exactly one of `ANCHORED`, `MISSING`, and
   item 11's list (§8.6), and every ledger entry in exactly one of
   `ANCHORED` and `UNANCHORED`. A reader asking what this
   seat had committed to by a block of some chain reads this list and
   applies §9.6;
6. `UNANCHORED`, sorted bytewise by `entry_id`;
7. `EXCLUDED(seq, author, entry_id)` for every entry that passes §4.3 and is
   not a ledger entry, byte-identical copies having collapsed to one entry
   under §8.1, sorted by `(seq, entry_id)`, `entry_id` comparing bytewise;
8. `ADOPTION_UNPROVEN(seq, entry_id, index)` (§9.5), one row per unproven
   element of a ledger adoption entry, sorted by `(seq, entry_id, index)`,
   `entry_id` comparing bytewise; it is informational and no label turns on
   it;
9. `UNKNOWN_TYPE(seq, entryType, entry_id)`, one row for every ledger entry
   whose `entryType` is none of the seven of §6, byte-identical copies
   having collapsed to one entry under §8.1, sorted by `(seq, entryType,
   entry_id)`, `entryType` comparing bytewise. It is
   informational and no label turns on it. The list is over the seven types
   this grammar knows and never over the types a particular verifier's
   operator reads by some convention, so it stays a function of the five
   inputs alone; a reader that knows a convention strikes from it the types
   that convention names and is left with the entries nobody read. This is the list
   §6.9 requires of any claim of silence;
10. `MALFORMED(entry_id, token)`, one row for every byte string in the pile
    that fails §4.3, carrying the §10 token its decision order names,
    byte-identical copies having collapsed to one, sorted bytewise by
    `entry_id`. It is informational and no label turns on it; it exists so
    that a `MISSING` row over bytes the verifier holds is distinguishable
    from one over bytes nobody produced;
11. the unavailable set as §8.1 leaves it, sorted bytewise;
12. the `UNPROVEN` list: every anchor record undiscarded by §8.1 whose
    `verdict` is `UNPROVEN` (§9.3), one row per record carrying `(hash,
    chainId, blockNumber, tx, sender)`, sorted by the first four in that
    order, `hash` and `tx` comparing bytewise;
13. the `VOID` list: every anchor record undiscarded by §8.1 whose
    `verdict` is `VOID` (§9.3), in the same row form and order; it is
    informational and no label turns on it;
14. the `DISCARDED` list: every anchor record the lineage trim of §8.1
    discarded, one row per record carrying its seven members, sorted by
    `(sender, chainId, blockNumber, tx, hash)`, `sender`, `tx`, and `hash`
    comparing bytewise; it is informational and no label turns on it, and
    it exists so that a record the reader's own basis produced is visible
    whatever the lineage made of its sender (§8.1);
15. one **label** from the closed set, decided in this order:
    1. `BROKEN_CHAIN` if any finding is hard;
    2. else `UNAVAILABLE` if the unavailable set as §8.1 leaves it is
       non-empty, or if the `UNPROVEN` list carries a record whose `hash`
       is the `entry_id` of no byte string in the pile (the whole pile, as in
       §8.1: bytes that fail §4.3 and bytes item 7 lists as excluded answer
       this clause exactly as a ledger entry's bytes do) and is not the
       `hash` of a record undiscarded by §8.1 with verdict `counted`;
    3. else `GAPS` if any `SEQ_GAP` was recorded or `MISSING` is non-empty;
    4. else `COMPLETE`.

The report is a canonical value of §3 whose root is an object with exactly
the members `root` (hex20), `basis` (the declared basis), `entries` (int),
`findings`, `missing`, `anchored`, `unanchored`, `excluded`,
`adoption_unproven`, `unknown_type`, `malformed`, `unavailable`,
`unproven`, `void`, `discarded`, and `label`, the lists in the orders items
3 through 14 fix. A finding is an
object with the members `name`, `position` (as item 3 defines it),
`entry_id`, `hard` (`true` or `false`), and exactly the arguments §8.2 and
§8.4 write for that name under the names those sections write: `SEQ_GAP`
carries `expected` and `actual`; `PREV_MISMATCH` carries `seq`;
`AUTHORITY_MISMATCH` carries `seq` and `certain`; `ROOT_MISMATCH` carries
`seq` and `actual`; `EQUIVOCATION` carries `seq`, `a`, and `b`, and its
`entry_id` member is its `a`. A `MISSING` row is `{hash, anchors}` with
`anchors` a list of `{chainId, blockNumber, tx, sender}`; an `ANCHORED` row
is `{entry_id, anchors}` with `anchors` a list of `{chainId, blockNumber,
blockTimestamp, tx, sender}`; an `EXCLUDED` row is `{seq, author, entry_id}`; an `ADOPTION_UNPROVEN` row
is `{seq, entry_id, index}`; an `UNKNOWN_TYPE` row
is `{seq, entryType, entry_id}`; a `MALFORMED` row is `{entry_id, token}`;
an `UNPROVEN` or `VOID` row is `{hash, chainId, blockNumber, tx, sender}`;
a `DISCARDED` row is `{chainId, blockNumber, blockTimestamp, tx, sender,
hash, verdict}`; `unanchored` and `unavailable` are lists of hex32. Where
the well-formedness test of §9.4 refuses the inputs, the output is the
canonical object `{"ok":false,"reason":"NO_LABEL"}` and nothing else; a
failure of a scan (§9.4) produces no report and no output of this section.

`EXCLUDED` is informational and no label turns on it: anyone may publish such
bytes, so it convicts nobody, and it exists so that a truncation is visible.
A reader who sees a run of excluded entries whose `seq` continues past the
ledger's last position knows to ask for the succession entry that would
admit them, which is the one entry whose absence a walk cannot see, and a
reader who sees a `DISCARDED` row under a sender the ledger's successions
never name asks the same question of the anchor set.
`COMPLETE` asserts only that the inputs held no fault: with zero ledger
entries it says that nothing in the inputs was this root's, and under a
basis that covers no chain it says nothing about anchoring, which is why the
entry count and the basis both travel with the label.

## 9. Anchoring

An anchor is a commitment, published on a blockchain from a lineage key, to
a 32-byte `entry_id`. It buys ordering and an upper bound on existence, and
nothing else: the chain never stores an entry, and a ledger verifies (§4,
§7, §8) without a chain in reach.

**[X] 9.1 Two forms, equal validity.** Throughout this section, **the
transaction's sender** is the address a verifier recovers from the
transaction's own secp256k1 signature, and **the transaction's calldata**
is the top-level transaction's `data` field, the bytes the sender signed,
and never any inner call's calldata. A transaction that creates a contract
carries init code in that field and no calldata: it is never a bare anchor,
by the bare form's own exclusion, and the registry form's containment test
has no field to run against, so a log emitted during a creation transaction
is not an anchor in either form whatever its topics hold, and a lineage
key's deployments carry no anchor and open no surface. A transaction that carries no such
signature (a system, deposit, or protocol-originated transaction, and any
transaction validated by account code rather than by recovery) has no
sender in this grammar: it anchors nothing in either form, and a log it
carries is not an anchor whatever its topics hold. A transaction whose
signature names no chain (a legacy transaction outside EIP-155) is not an
anchor in either form, because the same signed bytes are a valid transaction
on every chain and no reader can say which chain the sender addressed. A
receipt that carries no status field carries no status 1, so no transaction
of such a receipt is an anchor in either form; this grammar reads a status
byte and never a post-transaction state root.

- **Registry form.** A log in the receipt of a transaction with status 1,
  whose `address` field, the account whose storage context executed the
  emitting instruction, is an address that some `chains` object of the basis (§9.4) declares as a
  trusted registry, that object's `chainId` being the chain the log sits on
  and its range holding the log's block, so that a reader declaring two
  windows of one chain declares each window's registries for that window
  alone (a log emitted from within a `delegatecall`
  or `callcode` frame is emitted by the account that made that call, and a
  log emitted from an account carrying an EIP-7702 designator is emitted by
  that account itself; in every case the emitter is the account whose
  storage the executing code addressed, and never the account whose code
  ran), carrying
  exactly three topics and empty data: topic 0 byte-equal to
  `keccak256("Anchored(address,bytes32)")`, topic 1 the transaction's
  sender's 20 bytes preceded by 12 zero bytes, and topic 2 the anchored
  hash. Topic 1 is the transaction's sender and never the emitting call's
  `msg.sender`. The 32 bytes of topic 2 must also appear as 32 consecutive
  bytes of the transaction's calldata, lying wholly within it and beginning
  at an offset that is either a multiple of 32 or 4 plus a multiple of 32
  counted from the first byte of the calldata, so that in both forms the
  hash a sender is charged with is a hash the sender's own signed bytes
  carried at a word boundary; a log whose topic 2 appears nowhere in the
  calldata at such an offset is not an anchor, whatever address emitted it
  and whatever the basis declares. Containment binds the hash to the
  sender's own signed bytes and never to the sender's intention: every
  argument of an ordinary ABI call sits at such an offset, so a declared
  contract may charge a lineage key with any word that key passed it,
  including one a counterparty chose. The producer obligation below is
  therefore a rule about contracts and never only about transactions: a
  seat anchors through a contract whose emitting paths it has read and
  whose code cannot change under it, and a seat that calls a declared
  contract with arguments it did not choose knows that each of them is a
  word the contract may echo. A reader who declines that exposure lists in `registries`
  only addresses whose code it has read, and bounds `fromBlock` and
  `toBlock` to the window over which the code it read stood. A log carrying fewer or more than three
  topics, or any data byte, is not an anchor whatever its topics hold; both
  parameters are indexed, the topic 0 preimage recording no indexing, so a
  contract declaring `event Anchored(address indexed sender, bytes32 hash)`
  produces a log with two topics and 32 data bytes, which is not an anchor,
  and the declaration this form names is `event Anchored(address indexed
  sender, bytes32 indexed hash)`. The declared registry list bounds which
  logs are read, never which hashes a lineage key is charged with. Any
  contract may emit the form: a registry whose `anchor(bytes32 hash)` and
  `anchorMany(bytes32[] hashes)` emit one such log per hash and none for an
  empty array (the reference registry holds no state and has no owner, and
  nothing about its code enters this test), or any other contract an author
  calls whose code emits it for a word of the author's calldata. Such a
  contract anchors in the author's own transaction only when a lineage key
  of the seat is that transaction's sender, the transaction's calldata
  carries the hash at such an offset, and the contract emits the form
  itself; a contract that reaches a registry through an inner call (whose
  log names the contract, since `msg.sender` there is the contract), a
  contract that derives the hash from storage or from a computation, a
  contract reached through a batcher or a packed payload anchor nothing in
  that transaction; a transaction sent by anyone but a lineage key, a
  relayer and a smart account among them, puts no hash into this seat's
  reconciliation whatever contract it reached, §8.1 discarding its record
  and §8.7 item 14 carrying it; and the entry is anchored from a lineage key's own
  transactions like one no contract ever saw. This is the
  whole price of reading one log and one calldata and never the contract.
  The registry form has the bare form's exposure in the reader's hands
  rather than the sender's: a lineage key sends no transaction to any
  contract that emits the form except to anchor, a seat that cannot tell
  which contracts do so anchors through addresses it deployed or read, and
  a reader who names an address in `registries` charges this seat with every
  32-byte word of the seat's own signed calldata that address ever echoed
  into topic 2, whatever that address meant by it, the basis in the report
  being where that choice is visible. A reverted transaction emits no log
  and anchors nothing; a log with the same signature from an address no `chains` object declares
  for its own chain and block is not an anchor.
- **Bare form.** A transaction with status 1 whose recipient is its own
  sender and whose calldata is exactly `k × 32` bytes for some `k` ≥ 1; each
  32-byte word is an anchored hash. A transaction whose calldata is not a
  positive multiple of 32 bytes, whose recipient is not the sender, or which
  has no recipient (a contract creation) is not a bare anchor, whatever
  words it contains. The form carries no discriminator: a transaction a
  sender directed to its own address for some other purpose is a bare
  anchor all the same, so a lineage key sends no self-directed transaction
  whose calldata is a positive multiple of 32 bytes except to anchor, and a
  reader who names one in `bareTx` charges this seat with every word it
  carries.

The grammar has zero deployment dependency: the registry is convenience, and
the bare form needs no contract anywhere.

**[X] 9.2 Anchor record.** A verifier represents each anchor as `(chainId,
blockNumber, blockTimestamp, tx, sender, hash, verdict)`, where
`blockTimestamp` is the timestamp of the block header that includes the
anchor, `sender` is the transaction's sender, and `verdict` is `counted`,
`UNPROVEN`, or `VOID` under §9.3. Each member carries a stated form:
`chainId`, `blockNumber`, and `blockTimestamp` are `int`s of §3.1; `tx` and
`hash` are hex32; `sender` is hex20; `verdict` is one of the three tokens of
§9.3. A record that lacks one of the seven members, or whose member carries any
other form, is not a `zikaron/1` anchor record, and §9.4's no-label outcome
follows; every comparison and
sort over these members is over these spellings. Two records with equal
`hash` on different chains, or in different transactions, are two anchors
of one `entry_id`; where one chain includes one transaction hash in two
blocks, the two inclusions are two anchors of one hash. On each chain the
record with the smallest `blockTimestamp` gives the entry's existence bound
relative to that chain (§9.6), and `blockNumber` orders anchors inside one
chain and never across chains. Chains are not ranked by the grammar; how
much a given chain's block order is worth is for whoever weighs the
evidence. An adoption evidence record is `(chainId, tx, sender, calldata)`,
where `chainId` is an `int` of §3.1, `tx` is hex32, `sender` is the
transaction's sender in the sense §9.1 gives that term, hex20, and
`calldata` is the transaction's calldata in the sense §9.1 gives that term,
spelled `0x` followed by an even number of lowercase hexadecimal digits, the
empty calldata written `0x`; the record is not arity-closed, and §9.4 reads
these four and ignores every member beyond them. A transaction with no
sender under §9.1 enters no evidence record, a transaction whose signature
names no chain enters a record on no chain, a creation's record carries the
empty calldata and no byte of its init code proves an element, and a
transaction's status is not read, an included transaction having published
its calldata whatever became of its execution.

**[X] 9.3 Codeless at the anchor.** An anchor counts only if the sending
address has no code in the state at the end of the block before the
anchor's block and no code in the state at the end of the anchor's block; where the anchor's block is block 0 of its chain, the state
before it is the chain's pre-genesis state, in which every address is absent
and therefore carries no code, and this sentence consults that boundary and
decides it in the anchor's favour, so that only the second boundary requires
a query to a chain and both boundaries count as consulted for the verdicts
below. An address
absent from the state trie has no code. An account carrying an EIP-7702
delegation designator has code. Both
boundaries are objective facts about two block states, so the verdict
cannot be changed after the fact: a transaction that installs a designator
on its own sender carries no anchor, and an author who authorizes a
delegation voids every anchor sent from that key in that block and in every
block until the designator is gone, and voids none sent before. §9.1's
topic 1 already requires an anchor's log to name the transaction's own
sender, and the bare form already requires the sender to be the recipient,
so under this grammar's own forms no delegate lets a third party originate
an anchor under the author's name; the reason this test stands beside that
closure is that an address carrying code is an address whose transactions
this grammar cannot read as one key's own act under any future the EVM may
take, and the grammar declines to distinguish today's delegate semantics
from tomorrow's. The cost is stated plainly: a third party holding an
authorization the author signed once can land a designator in the block of
the author's anchor and void it, and a seat that cannot rule out such an
authorization anchors on a chain the authorization does not reach, and
where an authorization of chain id 0 leaves no such chain, anchors one hash
from more than one key, since two records of one `entry_id` are two anchors
(§9.2) and a designator reaches only the key whose holder authorized it,
while a designator that stands voids every anchor that key sends on that
chain until it is gone; a second key of the lineage costs the pair of
successions §6.7 and §7.5 make lawful, and a re-anchor after the designator
is cleared buys a bound at the later block and never the bound the voided
anchor carried. When
either state cannot be consulted, the verifier's verdict for the anchor is
`UNPROVEN`: the record enters the anchor set carrying that verdict, enters
the unavailable set never, takes no part in the reconciliation of §8.5, and
is carried in the `UNPROVEN` list of §8.7, so that an anchor found and not
proven is never silently dropped. Where both states were consulted and the
sending address carried code at either boundary, the verdict is `VOID`: the
record enters the anchor set carrying that verdict, takes no part in the
reconciliation of §8.5, and is carried in the `VOID` list of §8.7, so that
a transaction shaped like an anchor is visible in every report whatever
became of its sender's code, and an author who delegates in the block of
their own anchor buys no deniability for the bytes the chain shows. Where
both states were consulted and the address carried no code at either
boundary, the verdict is `counted`.

**9.4 Scan basis.** **[C]** An anchor set and an adoption evidence set are
always relative to one declared basis, itself a canonical value (§3) that is an object with exactly these
members:
`chains`, an array of objects each with exactly the members `chainId` (int),
`fromBlock` (int), `toBlock` (int), `registries` (array of hex20, the
addresses whose logs of the registry form are trusted on that chain:
registries proper and every other contract the reader lists), and
`senders` (array of hex20, the sender addresses scanned); `bareTx`, an
array of objects each with exactly the members `chainId` (int) and `tx`
(hex32), the transactions examined for bare anchors and the chain each was
looked for on, carrying at most one object per `(chainId, tx)` (bare
anchors have no discovery surface beyond a sender's own transaction
history, and a reader that scans only registries declares an empty array);
and `adoptionChains`, an array of objects each with exactly the members
`chainId` (int) and `throughBlock` (int), the chains the verifier can
consult for §9.5 and the height each was read through, carrying at most one
object per `chainId` and ordered by `chainId` ascending; that a transaction
not included at or below that height enters no evidence record is an
obligation on whoever produces the adoption evidence set, and no report
predicate reads `throughBlock`. Every member named here must carry the form
named for it, and `fromBlock` ≤ `toBlock`; `chains` is ordered by
`(chainId, fromBlock)` strictly ascending, two objects of one `chainId` having ranges that do not overlap, and two
objects of one `chainId` that are adjacent, the earlier object's `toBlock`
plus 1 being the later object's `fromBlock`, differing in `registries` or in
`senders`, so that a reader whose node serves two windows of
one chain declares two objects and one coverage still has one spelling;
`bareTx` is ordered by `(chainId, tx)` strictly ascending with `tx`
comparing bytewise, `adoptionChains` is ordered by `chainId` strictly
ascending, and the `int` keys of all three orderings compare numerically as
§8.7's do, a tie in any of the three being separately refused by that
array's own overlap or uniqueness rule; and `registries` and `senders` are bytewise ascending and carry
no repeated element, so that one scan has one spelling and two reports
declare the same scan exactly when their bases are byte-equal. A basis
failing any of these is not a `zikaron/1` basis. The form, arity, ordering,
and uniqueness rules of this paragraph are decided from the basis's own
bytes and are class [C]; the completeness rule below, which consults a
chain, is class [X]. No basis names a chain whose id is at or above 2^53,
`chainId` being an `int` of §3.1, and §6.10 reaches no object of this
section. An anchor on such a chain is outside every basis and enters no
anchor set: a verifier that meets one omits it and the input stands, a seat that works on such a chain anchors its entries on a chain this
grammar can name, and a future law that must reach those ids widens
§3.1 and this section together. "Itself a canonical value" means a value
of §3's universe; the basis's own spelling inside a transport is that
transport's business, and §8.7 carries it re-canonicalized. A `bareTx`
object may name a chain that `chains` does not, since the two arrays
declare two different scans. `fromBlock` and `toBlock` bound an inclusive
range. **[X]** A conforming anchor set
carries a record for every log of §9.1's registry form emitted by an
address a `chains` object declares as a registry, in a block of that
object's range, whose transaction sender is in that object's `senders`, one for every bare anchor of §9.1 in a transaction `bareTx`
names, on the chain the `bareTx` object names, one record per distinct 32-byte
word, since §9.1 makes each word an anchor and two equal words of one
transaction give two anchors this section keys to one record, and no other
record; a
conforming adoption evidence set carries a record for every transaction a
ledger adoption entry names on a chain of `adoptionChains` that is included
at or below that chain's `throughBlock` and has a sender under §9.1 and a
signature naming that chain (§9.2), each record carrying that sender and the transaction's calldata as §9.2 states,
and no other. A
verifier that did not obtain every log and every transaction the
completeness rule enumerates has not scanned this basis and produces no
`zikaron/1` anchor set under it: a range it could not read, a range that
reaches past the head of the chain it read, a `throughBlock` above that head, a transaction `bareTx` names that it
could not retrieve, a transaction a ledger adoption entry names on a chain
of `adoptionChains` that it could neither retrieve nor be told the chain
does not hold, and a chain that did not answer are each a failure of the
scan and never an empty result; the verifier narrows the
basis to what it read and reports under that narrower basis, or produces no
report at all. Producing no report is a fact about the verifier's reach and
carries no output of this law; the `NO_LABEL` object of §8.7 is the output
of this section's well-formedness test and of nothing else. A chain that
answers definitely that it holds no transaction of a hash `bareTx` names
has answered: the object yields no record and the scan stands, exactly as
§8.6 treats a definite negative from a byte store; a failure of the scan is
an absence of an answer, and never an answer of absence. `UNPROVEN` is the verdict of a record that exists and whose §9.3
states could not be consulted, and never the mark of a log that was never
fetched. The basis pins block numbers and never block hashes; which depth of
block a reader is willing to name in `toBlock`, so that the range it read
stays the range it will re-read, is the reader's affair and no predicate of
this grammar. **[C]** §6.10
reaches no object of this section: an object bearing a member beyond its
list, two `chains` objects of one `chainId` whose ranges overlap or which
are adjacent with equal `registries` and `senders`, a second `bareTx` object for one
`(chainId, tx)`, or a second `adoptionChains` object for one `chainId`, is
not a `zikaron/1` basis; one `chainId` named in several arrays is ordinary,
and so are two `chains` objects of one `chainId` over two windows. `senders`
bounds the registry-form scan and nothing else: a registry-form log whose
transaction sender is outside `senders` yields no record even when that
sender is a lineage key, while a bare anchor in a transaction `bareTx` names
yields a record whatever its sender, and §8.1 re-filters neither by
`senders`, so a bare anchor whose `sender` is a lineage key absent from
`senders` is admitted by §8.1 like any other. A basis whose `senders` omit
an address of the audited lineage therefore says nothing about that
address's registry-form anchors, and a `COMPLETE` label under it carries
that silence exactly as one under a basis that covers no chain does (§8.7);
a reader who wants the silence closed compares `senders` against the root
and every `to` in the ledger's successions. `bareTx` carries the same silence in the other
direction: a bare anchor in a transaction the array does not name, or names
on another chain, is outside the basis, and a reader who wants that silence
closed compares the array against the lineage's self-directed transactions
on every chain the reader cares about. **[C]** The anchor set holds at most
one record per `(chainId, blockNumber, tx, hash)`, and the adoption
evidence set at most one per `(chainId, tx)`; two records that agree on
the seven members §9.2 names, or two evidence records that agree on their
four, are one record, whatever members either carries beyond them. A record may carry members beyond
the members its section names, the seven of an anchor record and the four
of an evidence record alike: no predicate of this grammar reads them, they
enter no report row, and they never make two records that agree on those
members into two records; the arity closure of this section reaches the basis and its
nested objects alone. Every anchor record lies within
the basis's reach: its `(chainId, tx)` is one a `bareTx` object names, or
some one `chains` object of its `chainId` has both a range holding its
`blockNumber` and `senders` holding its `sender`; an anchor record the basis
reaches by neither route is a record no scan of that basis produces, and the
input has no label. An evidence record lies within reach exactly when some
`adoptionChains` object names its `chainId`, and one outside that reach
gives no label; an evidence record for a transaction no ledger adoption
entry names is within reach all the same, its four members carry the forms
this section names and it collapses with any record of its own `(chainId,
tx)` like every other, no predicate of a report reads its content, and
which transactions the set holds is the scan's affair and class [X]. The test
reads the basis's declarations and the record's own members, it decides
nothing about which form the record came from, and it reads `registries`
not at all: it refuses a record whose chain, block, and sender no declared
window holds together and whose transaction no `bareTx` object names, and
it never vouches that a record it admits is genuine, which is what §9.4's
completeness rule and §9.7's kits are for. An input carrying two anchor records that agree on `(chainId, blockNumber,
tx, hash)` and disagree on `blockTimestamp`, `sender`, or `verdict`, or
two evidence records that agree on `(chainId, tx)` and disagree on `sender`
or `calldata`, or a record that lacks one of the seven members §9.2 names
or any of whose seven members fails the form §9.2 names, or an evidence
record that lacks one of its four members or whose `chainId` is not an
`int`, whose `tx` is not hex32, whose `sender` is not hex20, or whose
`calldata` is not `0x` followed by an even number of lowercase hexadecimal
digits, the empty calldata written `0x`, or an audit whose root is not
hex20, whose unavailable set carries a value that is not
hex32, or whose declared basis is not a `zikaron/1` basis under this section,
is not a `zikaron/1` audit input, and a verifier handed one returns no label.
This test is decided on the inputs as supplied and before every trimming of
§8.1, so a record the lineage trim would discard still spoils the input;
the test and its no-label outcome are decided from the five inputs of §8
alone and are therefore inside the freeze of §12. **[X]** Two verifiers whose bases are byte-equal, run against chains that
agree on the blocks their bases name and on the block that includes each
transaction `bareTx` names, and that answer for both account states §9.3
consults, produce byte-equal anchor sets, because the completeness rule
above fixes them; two such verifiers that also hold one pile under one root,
and whose chains agree on whether each transaction that pile's ledger
adoption entries names is included at or below its chain's `throughBlock`
and on the bytes of each one that is, produce byte-equal adoption evidence
sets, that set's completeness rule ranging over the transactions this
pile's ledger adoption entries name, so an evidence set travels with the
pile it was gathered for and never with the basis alone, and a verifier
that adds bytes to its pile after a scan re-scans for the transactions
those bytes name. A
`bareTx` object names a transaction and never a block, so the depth at
which a reader is willing to read one is that reader's affair exactly as
`toBlock` is, and a `bareTx` object gives the basis no record of the
choice. **[C]** The pile and the unavailable set have no basis: each records
what one verifier retrieved, so two verifiers with byte-equal bases lawfully
produce different `MISSING` lists, different unavailable sets, and
different labels, and only the five inputs of §8 fix a report. The basis is
part of every report (§8.7); a label with an undeclared basis is not a
`zikaron/1` audit.

**[C] 9.5 Adoption, decided from the inputs.** An adoption entry that is a
ledger entry at `seq` `k` (§8.1) names transactions; an adoption entry this audit excluded names none that
this report reads, and no `ADOPTION_UNPROVEN` row is ever recorded for one,
and an entry of any other type names none this section reads, whatever its
body carries under the key `anchors` (§6.9). An element is **proven**
iff all three hold: the basis's `adoptionChains` carries an object whose
`chainId` is this element's `chainId`, and the adoption evidence set carries a record whose `chainId` and `tx` are
this element's `chainId` and `tx` (the first of these two conditions can fail only with the second, §9.4 giving no label to an evidence record on a chain no
`adoptionChains` object names; it is stated so that the test reads whole
against a basis that declares no chain at all); some 32 consecutive bytes of the byte string that record's `calldata`
spells, beginning at an offset that is either a multiple of 32 or 4 plus a
multiple of 32 counted from the first of those bytes, and lying wholly
within them, are byte-equal to the 32 bytes this element's `content` spells under §1's
`hex32`, so that a record whose `calldata` is the empty byte string proves
no element, there being no 32 bytes wholly within it; and either the record's `sender` is in the
prefix lineage at `k` (§7.4) or the body carries an attestation that passes
§6.6 and whose `attestor` is byte-equal to that `sender`. Otherwise the
element is **unproven**, whether it failed a test the evidence decided or
failed for want of a chain the basis does not reach (its `chainId` is not
in the basis's `adoptionChains`, or the transaction is in no record of the
adoption evidence set); unprovenness has one report form and one only. An
adoption with an unproven element is a legal entry (§6.5 tested its form)
whose claim is unproven; the report lists such elements as
`ADOPTION_UNPROVEN(seq, entry_id, index)` with `index` the zero-based
position in the entry's `anchors` array, informational, and no label turns
on them. An adoption may name a lineage key's own transactions, and may
name one transaction twice; both are proven or unproven like any other. The test reads the basis, the evidence record's four members, the ledger's
own successions before `k`, and the entry's own bytes, and never a chain: which transactions the evidence set holds is the scan's
affair (§9.4), and the report says under which basis it was read.

**[C] 9.6 Time.** An anchor of the entry at `seq` `N` on one chain is an upper
bound, relative to that chain, on when that entry, and every entry reachable
from it by following `prev` through entries the verifier holds, existed,
because each `prev` commits to its predecessor's exact bytes and a hash of
unwritten bytes cannot exist. The same argument reaches the digests those entries carry, on the terms of
the function each names: an anchor bounds, relative to that chain, the
existence of the bytes of every 32-byte digest a reachable entry names, a
`content`, a `work`, a `terms`, a `mode`'s `toolchain`, a `case`, and an
adoption element's `content` among them, wherever the document that names
the field states a function under which a digest of unwritten bytes is no
more possible than a `prev` of them. This law fixes `sha256` for `prev` and for every `entry_id` a body names,
a `history`, a `grant`, and a `subject` among them (§2.1, §6.3, §6.4, §6.8),
and fixes the function of no other field, the fields §11 resolves against a
document being the rest, so the bound over the digests this law fixes is
this grammar's and the bound over the rest is the naming document's; what
either is worth in a category is §11's, and the grammar states the bound
alone. The grammar states one bound per chain and
never a minimum across chains, because §9.2 ranks no chain and a reader who
admits a chain whose timestamps it does not trust would otherwise hand that
chain the bound for the whole ledger; a reader who wants a single bound
takes the minimum over the chains that reader is willing to weigh, and says
which those were. The bound is computed from the `ANCHORED` list of §8.7 and
the ledger entries alone, so it is class [C] and a reader's computation over
a report.
Reachability stops at the first position where no held entry matches the
`prev`, so an anchor bounds one line of a fork and never the other, and
bounds nothing across a gap. Density is a cost dial that buys resolution; it
changes no predicate in §4, §7, or §8. Transitivity is a statement about
time and never about set membership (§8.5).

**[X] 9.7 Proof kits.** A self-contained proof of one anchor has these
parts: the chain id the anchor is claimed on, the 32-byte hash claimed,
and, for the registry form, the emitting address as the receipt's log
carries it, so that a recipient can
decide §9.1's declared-registry test against a basis of the recipient's own;
the anchor's block header and the header of the block before it, that second
header and its account proof omitted where the anchor's block is block 0;
the anchor transaction with a Merkle proof against the anchor header's
`transactionsRoot`; the anchor transaction's receipt with a Merkle proof
against the anchor header's `receiptsRoot`, for both forms, since §9.1
requires status 1 of both and only the receipt carries it; and two account
proofs of the sending address, one against each header's
`stateRoot`, each showing either an account whose code hash is `keccak256`
of the empty byte string or, by exclusion, no account at that address at
all, an address absent from the state trie having no code and satisfying
§9.3 on that boundary. An account proof that shows an account carrying code
is a part of the same form and shows the boundary consulted and decided
against the anchor, so a complete kit whose proofs show code establishes a
`VOID` anchor in the sense of §9.3 exactly as one whose proofs show none
establishes a `counted` one; what a kit never shows is a boundary it
carries no proof of. A verifier binds both headers
to block hashes it trusts independently and to each other, the second
header's block hash being the anchor header's `parentHash`, recovers the
sender from the transaction, requires it to equal the log's topic 1
(registry form) or the transaction's recipient (bare form), checks both
account proofs, verifies the receipt's inclusion at the same trie index as
the transaction's, so that the receipt whose status it reads is the receipt
of the transaction it tested, and applies every test of §9.1 to the kit's
transaction and receipt: the status for both forms; for the registry form
the emitting address, the `Anchored` topic 0, the topic count of three, the
emptiness of the data, topic 1 against the recovered sender, and the
calldata containment of topic 2, and the claimed hash as that log's topic
2; for the bare form the recipient against the recovered sender, a calldata
length that is a positive multiple of 32, and the claimed hash as one of
its 32-byte words. A kit's transaction must
be signed for the chain the kit names, so that a kit cannot be relabelled
to another chain; a transaction whose signature names no chain establishes
no anchor (§9.1). The kit shows nothing about the emitting contract beyond
its address: no code, no code hash, and no account proof of it, because
§9.1 reads the log and the calldata and never the contract. Such a kit
reduces the chain-facing trust to two 32-byte block hashes and outlives any
history expiry. A kit convinces its recipient of one anchor and of the verdict §9.3 gives
it, and it supplies no record to any anchor set: a set is fixed by the
completeness rule of §9.4 and by nothing else, so a recipient persuaded by a
kit of an anchor the recipient's own basis does not enumerate widens the
basis and scans it, or reports under the basis it read. A kit missing an
account proof of a boundary that requires a query leaves that state
unconsulted and shows its anchor to be `UNPROVEN` in the sense of §9.3, the
pre-genesis boundary of a block-0 anchor requiring none and its omission
being the form this section names; a kit missing any other part, a kit whose headers are not parent and child, and a kit whose two proofs
stand at two indices show nothing at all. A self-contained proof of one
adoption evidence record has these parts: the chain id the record names,
the record's transaction with a Merkle proof against a block header's
`transactionsRoot`, and that header; a recipient binds the header to a block
hash it trusts independently, recovers the sender from the transaction,
reads the calldata as §9.1 defines it, and applies §9.5's containment test
to the `content` it holds. The kit's transaction must be signed for the
chain the kit names, as an anchor kit's must, so that an evidence kit cannot
be relabelled to another chain; a transaction whose signature names no
chain enters a record on no chain (§9.2) and such a kit shows nothing, and
a kit missing any part shows nothing at all. Such a kit convinces its
recipient of one record and supplies no record to any adoption evidence
set, which §9.4's completeness rule fixes and nothing else does.

## 10. Rejection tokens

Every rejection of a byte string offered as an entry carries exactly one
token from this closed set, decided by the order of §3.5 and §4.3. Refusals
that are not rejections of one byte string carry no token: a refusal for
size (§11), and the no-label outcome a malformed audit input produces
(§9.4). A verifier that rejects for the wrong reason is distinguishable
from one that rejects correctly.

| token | meaning |
|---|---|
| `E_UTF8` | bytes are not valid UTF-8 (§1) |
| `E_JSON` | bytes are not one JSON text, begin with a byte order mark, or carry a surrogate escape |
| `E_NUMBER` | a value position of numeric shape is not an integer in [0, 2^53 − 1] written per §3.2 |
| `E_DEPTH` | container nesting exceeds 128 |
| `E_DUP_KEY` | an object repeats a key after decoding |
| `E_KEY_CHARSET` | a key is empty or carries a scalar value outside U+0020 through U+007E |
| `E_VALUE_CHARSET` | a string value outside every prose subtree carries a scalar value outside U+0020 through U+007E |
| `E_NOT_CANONICAL` | the bytes differ from the canonical bytes of their value |
| `E_ENVELOPE` | the root is not an object |
| `E_ENVELOPE_MISSING` | one of the seven keys is absent |
| `E_ENVELOPE_CLOSED` | a key beyond the seven is present |
| `E_SPEC` | `spec` is not the string `zikaron/1` |
| `E_ENTRYTYPE` | `entryType` is not a token |
| `E_AUTHOR` | `author` is not hex20 |
| `E_SEQ` | `seq` is not an int |
| `E_PREV` | `prev` is neither `null` nor hex32 |
| `E_PREV_SEQ` | `prev` is `null` while `seq` is not 0, or `prev` is not `null` while `seq` is 0 |
| `E_BODY` | `body` is not an object |
| `E_SIG_FORM` | `sig` is not hex65 |
| `E_GENESIS_PLACE` | `genesis` off position 0, or position 0 not `genesis` |
| `E_BODY_FIELD` | a required field is absent, a present field has the wrong form, or a stated relation fails (§6.3's `from` ≤ `to`; §6.5's `attestor` and `attestation` together) |
| `E_SIG_V` | `v` is not 27 or 28 |
| `E_SIG_RANGE` | `r` or `s` is 0 or ≥ n |
| `E_SIG_HIGH_S` | `s` exceeds (n − 1) / 2 |
| `E_SIG_RECOVER` | public key recovery fails |
| `E_SIG_SIGNER` | the recovered address is not `author` |

Four further closed vocabularies exist beside this one: the five finding
names (§8.3), the four labels (§8.7), the three anchor verdicts `counted`,
`UNPROVEN`, and `VOID` (§9.3), and the four reading verdicts of §11. Each names a state of a set of entries, of a
chain record, of a field, or of a reference; a rejection token names a
state of one byte string, is carried in no finding and in no label, and
reaches a report only in the `MALFORMED` rows of §8.7 item 10, which record
why bytes the verifier holds are not entries.

## 11. Outside the grammar

The following are decided by readers, judges, terms, and the forums a
grant's terms name, never by this grammar, and no sentence above should be read as deciding them:

- **Money.** No entry moves value. Prices ride escrows, flows ride receipts;
  this grammar holds no funds and charges no fee.
- **Reference resolution.** Whether a `history`, `grant`, or `subject`
  reference names an existing entry: a reader resolves it against a
  declared **resolution basis**, the ledgers the reader holds named by their
  root addresses, and reports one of four verdicts. `RESOLVED`: this ledger
  holds an entry with that `entry_id` and, where the row that carried the
  reference names a type, the entry carries that type. `MISMATCHED`: this
  ledger holds it under another type. `FOREIGN`: this ledger does not hold
  it and some other ledger of the basis does, where **this ledger** is the
  ledger under whose root the reader is reading and is named beside the
  verdict; a byte string that is a ledger entry of two roots at once (§7.5)
  carries its references once under each root and is resolved once per
  root, and where the target itself is a ledger entry of two roots, the
  verdict under the root being read wins and `FOREIGN` is not reported.
  `DANGLING`: no ledger of the basis holds it. Two readers with different
  bases lawfully report `FOREIGN` and `DANGLING` for one reference, and the
  bases say why. The four verdicts answer resolution and nothing else:
  whether a `RESOLVED` `history` entry's `content` is byte-equal to the
  grant's `work` is a separate comparison the reader reports beside the
  verdict. A `case`, a `work`, a `terms`, a `mode`'s `toolchain`, a `content`, and an
  adoption element's `content` and `tx` carry no verdict of this set: each
  names a transaction, an object, or a document that is never an entry of
  any ledger of this grammar. A document is resolved by being shown it and
  recomputing its digest under the function named by the document that named
  the field, the terms' for `terms` and the mode's mark for a `history`
  entry's `content` (§6.2); this grammar fixes none of them and tests only
  the hex32 form, and a field whose naming document states no function is a
  digest no reader can check, which is that document's failure. A `case` is
  resolved against whatever forum the grant's terms name (§6.4).
  The grammar accepts the entry under every outcome.
- **Meaning of grants.** Exclusivity, scope, overlap between grants, a
  grant's relation to an escrow, whether a window has run, whether a later
  grant or revocation supersedes an earlier one, and what a revocation
  without a cited ruling binds: these live in the terms document the grant
  hashes, and in the standing orders of whoever adjudicates. This grammar
  relates two entries by `seq`, by `prev`, and by nothing else; §8.4's
  equivocation is a finding about position and never about subject, and an
  `annotation` with a `subject` changes no earlier entry's standing.
- **Sequence law.** Stake first, anchor second, release third is commercial
  doctrine enforced by escrows and claims; this grammar never checks a
  payment.
- **Anchoring as a requirement.** Whether an author must anchor, whether a
  buyer or an escrow may act on a grant no report lists as anchored, what
  weight a party gives an `UNPROVEN` or a `VOID` record or an
  `ADOPTION_UNPROVEN` row, and which chains, registries, senders,
  transactions, and block depth a reader's basis should cover: the terms,
  the standing orders of whoever adjudicates, and the reader who publishes
  the basis. This grammar decides what an anchor is (§9.1), what a record
  of one holds (§9.2), and what a report says about the set a reader
  declared (§8.5, §8.7, §9.5); it never decides that an entry needed one.
- **Second rails.** An author may publish elsewhere a fact this ledger
  already carries: a succession announced from a contract, a grant echoed
  in some registry, a mode mark placed in metadata. This grammar reads
  entries under a root and knows no other rail: which record a reader acts
  on, whether an author is obliged to keep the two agreeing, and what
  follows when they disagree, are the terms' and the readers'. Authority
  to sign entries is §7.3's and is decided by this ledger alone; nothing
  published on another rail moves it.
- **Office in time, and liveness.** When an entry was written, whether a
  key was in office at a moment rather than at a position, and whether a
  ledger that has stopped writing has ended: §7.3 walks positions and §7.6
  reads no clock, so the grammar answers none of the three. An anchor
  bounds existence from above on the chain that carries it (§9.6) and
  bounds nothing else; an author's liveness is a fact readers establish
  outside these bytes.
- **Weighing.** What an anchored history proves, and what minimum a category
  demands, live in each judge's standing orders. A level is not a field:
  density (how many history entries, how close together) is read from the
  ledger, and what a mark is worth is read from each judge's standing
  orders, which name the marks that seat accepts and at what weight. A
  standing order that names a level names the marks and the density it
  means by it. No statistic over this ledger is normative, and neither the
  chain nor the ledger carries one.
- **Modes.** Toolchains live above the grammar; a mode mark is an open token
  priced by its record, and the grammar verifies nothing about it.
- **Grantee form.** `grantee` is an address; whether an EOA, a contract, or
  a company stands behind it is the parties' affair.
- **Glyph similarity.** Prose subtrees carry every Unicode scalar value,
  bidirectional controls and invisible characters included, so two prose
  strings that look alike may differ in bytes and two that differ in bytes
  may look alike. Which bytes a reader is shown is a display question; the
  grammar decides identity by bytes and declines to chase appearance.
- **Size and transport.** No entry has a size ceiling in this grammar; caps
  belong to storage and transport layers and are declared there. A verifier
  may refuse an input it cannot hold, and a refusal for size is not a
  rejection under §10: it names no token, decides nothing about the bytes,
  and is reported as a retrieval failure in the sense of §8.6. The freeze
  criterion of §12.1 is over inputs a verifier accepted into memory, and
  conformance under §12.2 is agreement on the inputs the measuring contract
  obliges a verifier to accept.
- **Interpretation.** Indexers, depth statistics, and viewers compete
  freely; none is normative.

## 12. Freeze and evolution

**12.1 The frozen core is the criterion.** The reference implementation of
this grammar freezes only when an independently written implementation,
produced from this text alone, agrees with it on every input of a generated
corpus (the canonical bytes of every input that parses to a value of
§3.1, `entry_id`, every §3.5 and §4.3 token, every §8 finding, list, and
label, and the `NO_LABEL` object in a report's place), across at least one hundred thousand fuzz
samples, with zero panics, hangs, nondeterministic answers, or wrong
accepts, and with every closed boundary of classes [E] and [C] in this text
witnessed on both sides by at least one corpus input under every seed the
corpus is generated with, the boundary list being a precondition on the
corpus and never a property claimed of it afterwards, and published beside
the corpus as a working instrument. A closed boundary of class [E] or [C]
is a boundary the corpus can witness on the surface §12.2 compares: the
canonical bytes, the `entry_id`, the rejection token, and the report of
§8.7 with its lists and its label, or the `NO_LABEL` object §9.4's test
returns in a report's place. A [C] passage whose output is a reader's
computation over a report and never a member of one (§9.6) states no such
boundary and enters no precondition; the forms §9.2 names are read by the
well-formedness test of §9.4 and are boundaries of class [C] whatever the
mark on §9.2, since that test is decided from the five inputs of §8 alone;
a rule with no reachable failing side is witnessed once and states no
boundary in this sense. The closed boundaries of class [X] are
outside the corpus and outside the freeze; §9.4's completeness rule and
§9.7's kits settle a disagreement about what a chain shows. After the
freeze, the core binds and this text may be rewritten freely as a textbook.

**12.2 Conformance is a test run.** Once §12.5 names a release digest,
agreement with the frozen core on the corpus is the test anyone can run in
seconds, and it is evidence of conformance and never its definition:
conformance is agreement on every input, and an input outside the corpus on
which a verifier differs from the core is a non-conformance the corpus did
not catch and a reason to grow it.

**12.3 Forks are visible.** `zikaron/2` is a new spec id in the envelope and
a new domain literal; signatures cannot leak across (§5.6); archives under
`zikaron/1` remain valid under `zikaron/1` forever and are never re-signed.
A migration is signed on both sides: under `zikaron/1` the old chain's last
entry is a `succession` whose `to` is the key that signs the new chain's
genesis, and that succession is the whole of what this law can say about a
migration. Whether the new spec's genesis pins the old chain's head
`entry_id` is the new spec's affair; where it does, the pin fixes the head
as it stood and never as it will stand, since the succession moves
authority to the new-spec key (§7.3), which may go on writing `zikaron/1`
entries that every audit reads as clean, so a reader who relies on the pin
re-reads the old ledger for entries past the pinned head and treats a
longer old chain as the ordinary fact it is, never as a fault. A new-spec
chain that pins a head no `zikaron/1` succession handed to it inherits
nothing; the old archive's silence is the whole of the answer, exactly as
§7.3 makes it for an ordinary handover. Anchors carry no spec namespace:
the two forms of §9 commit to 32 bytes and say nothing about which law's
`entry_id` those bytes are, so the migration key, which §7.4 puts in the
`zikaron/1` lineage, puts every hash it anchors for the new chain into this
ledger's reconciliation, where no `zikaron/1` byte string can discharge it.
A reader auditing a migrated ledger under this law therefore asks whether
the ledger was whole in its own time and bounds the basis at the migration:
on each chain, `toBlock` no later than the last block the reader means to
cover of the old ledger's own anchors, or `senders` and `bareTx` without the
new key, each choice trading one silence for another and the basis saying
which was made; where the new key went on writing `zikaron/1` entries and
anchoring them past that bound, those entries stand in `UNANCHORED` under
the narrower basis, which is informational. A basis that reaches past the
migration reads the new chain's anchors as `MISSING`, which is what they
are under this law, and says nothing about the old ledger that the narrower
basis did not.

**12.4 Other ledgers on one key.** Anchors carry no spec namespace: every
hash a lineage key anchors enters the reconciliation of every ledger whose
lineage holds that key, so a hash anchored under any other law by a key of
this lineage, which no `zikaron/1` byte string can discharge, stands in
this ledger's `MISSING` list, or in its unavailable set where retrieval
failed, for good. A key is therefore a seat under one ledger law and never
under two; §7.5 states the obligation this puts on a succession.

**12.5 The core's identity.** Naming a release digest here is the freeze,
and it is the only act that performs it. The release digest of the `zikaron/1` core is

    0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc

named on 2026-09-05: the pure Python implementation in
`zikaron-conformance/impl-py/` (five source files: `zk1.py`, `zkcanon.py`,
`zkcrypto.py`, `zkentry.py`, `zkaudit.py`; the language's standard library
and nothing else), whose release digest `zikaron-conformance/criterion-digest.sh`
computes; the corpus, the harness contract, and the review record beside it
say how it converged. From the naming the core binds and this text is its textbook; a dispute
about what `zikaron/1` says on some input is settled by running the core on
it. A new core, or a change to this one, is a new digest named by the same
act.
