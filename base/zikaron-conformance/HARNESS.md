# zk1 convergence harness: command-line contract

This file fixes the *measuring interface* shared by every independent
implementation of `zikaron/1` in this convergence run. It defines nothing
about the law; every rule comes from docs/zikaron-v1.md alone. Two
implementations that follow this contract can be run on the same inputs
and their outputs compared byte for byte.

All output is written to stdout as **canonical JSON in the law's own
canonical form (§3.4)**: no whitespace, members sorted bytewise, integers
plain, strings escaped per §3.4. One JSON value per invocation, no trailing
newline. Exit code 0 whenever the program produced its JSON answer (a
rejection is an answer); exit code 2 only for harness misuse (a path the
command cannot read, absent or otherwise; bad arguments; an argument beyond
a command's own list). No command exits
with any code but 0 and 2. The diagnostic a misuse exit writes to stderr is the
implementation's own; only the exit code and the empty stdout are part of
this contract. A candidate that cannot hold an input above the sixteen-mebibyte bound
stated under `audit` refuses it under §11 the same way: exit code 2 and no
JSON.

Executable name: `zk1` (Rust: `target/release/zk1` or `cargo run --release --`;
Python: `python3 zk1.py`). Commands:

## `zk1 check <path>`
Reads the file's bytes and applies §3.5 and §4.3 in full.
- Accepted: `{"entry_id":"0x<64 hex>","ok":true}`
- Rejected: `{"ok":false,"token":"E_..."}` with the single token the law's
  decision order names.

## `zk1 canon <path>`
Reads the file's bytes, applies §3.5 tests 1 through 5 (everything except
roundtrip identity), and on success writes the canonical bytes of the parsed
value wrapped as `{"canon":"0x<hex of canonical bytes>","ok":true}`, the
digits lowercase as §1's hex functions write theirs. On
failure `{"ok":false,"token":"E_..."}`. (This lets a corpus be built from
loosely written JSON and lets two implementations be compared on §3.4 alone.)

## `zk1 sign <privkey-hex> <path> [domain]`
`<path>` holds the canonical bytes of a six-member envelope (an entry
without `sig`; any RFC 8259 spelling is re-canonicalized, and the value is
signed as given: no member is removed, so a file carrying a top-level `sig`
is signed with it inside the preimage). `domain` defaults to `zikaron/1`
when the argument is absent; when it is present it is signed as given, the
empty string included, where every byte of it is below 0x80 and none is a line feed, which is the
shape §5.6 gives every literal of this family, with `decimal(len(M))`
recomputed from the message; any literal outside that shape, one carrying a
line feed or a byte above 0x7F among them, is harness misuse. The command
reads no member of the file and derives no object from it: an adoption
attestation (§6.6) is produced by putting the three-member object whose
canonical bytes are `C` in the file and passing `zikaron/1-adoption`, and
its `presig` is then `sha256(C)`
by the same rule that makes an entry's `presig` the `sha256` of the
six-member object's canonical bytes. `<privkey-hex>` is sixty-four hexadecimal digits of either
case, with or without a leading `0x` or `0X`, this argument being a
transport spelling and none of §1's forms, naming a scalar in [1, n − 1];
any other argument, a scalar outside that range included, is harness
misuse. Computes §5.1 through §5.5 with RFC 6979 (HMAC-SHA256 over the §5.3
digest) and low-s by negation, as §5.7 fixes them, and writes
`{"digest":"0x<64 hex>","presig":"0x<64 hex>","sig":"0x<130 hex>","signer":"0x<40 hex>"}`.
A `<path>` whose bytes fail §3.5 tests 1 through 5, and one whose value is
not an object, are each harness misuse: exit code 2 and no JSON answer; an
object of any member count is signed as given. (Producer side; used to build corpora and to
cross-check that two implementations produce byte-identical signatures.)

## `zk1 audit <path>`
`<path>` holds a JSON object with members (the six are required; the
object is open, and a member beyond them is ignored):
- `root`: hex20
- `pile`: array of strings, each the two lowercase bytes `0x` followed by an even
  number of hexadecimal digits of either case, encoding one candidate byte
  string, the empty byte string written `0x` (a multiset; duplicates
  allowed); this transport encoding is not
  one of §1's hex forms, and the case rule it carries reaches its digits
  alone, an element spelled `0X` being outside the form
- `anchors`: array of objects carrying at least `{chainId:int,
  blockNumber:int, blockTimestamp:int, tx:hex32, sender:hex20, hash:hex32,
  verdict:"counted"|"UNPROVEN"|"VOID"}`; the object is open and a member
  beyond the seven is ignored (§9.4), while the basis and its nested objects
  are arity-closed; the array transports a set, two records that agree on
  the seven collapsing to one under §9.4
- `unavailable`: array of hex32, transporting a set, so a repeated element
  collapses
- `evidence`: array of objects carrying at least `{chainId:int, tx:hex32,
  sender:hex20, calldata:"0x<hex>"}`, the adoption evidence records of §8
  (`calldata` is `0x` followed by an even number of lowercase hexadecimal
  digits, the empty calldata written `0x`); the object is open and a member
  beyond the four is ignored, and the array transports a set, two records
  that agree on the four collapsing to one under §9.4
- `basis`: the §9.4 object, in any RFC 8259 spelling of its value

The file is read by an RFC 8259 reader with five restrictions and no
others: the bytes are valid UTF-8; every number, at any depth and in any member,
is written as §3.2 writes an integer and lies in [0, 2^53 − 1], so a sign,
a fraction, an exponent, a leading zero, a value at or above 2^53, or a
literal such as `NaN` each make the file unreadable; no object repeats a member name; every string, a member name included, decodes to
scalar values, a `\u` escape
naming a lone surrogate being refused while a surrogate pair is read as
the one scalar value RFC 8259 says it spells; and a container opening at
nesting depth 129, the root counted as depth 1, makes the file unreadable,
so no reader needs an unbounded stack. A byte order mark is not whitespace
under RFC 8259's `JSON-text` production, so a file beginning `EF BB BF` is
unreadable, as it is for an entry (§3.5). No corpus file exceeds sixteen
mebibytes; a candidate accepts every input of sixteen mebibytes or less,
and above that bound it may answer or refuse, a refusal being exit code 2
with no JSON, which `compare.py` never exercises. A file the reader refuses, a file whose root value is not an object, a
required member absent or outside its form, or inputs that are not a
`zikaron/1` audit input under §9.4 (a malformed basis, two records agreeing on `(chainId, blockNumber, tx,
hash)` and disagreeing on another of the seven members, two evidence
records agreeing on `(chainId, tx)` and disagreeing on `sender` or
`calldata`, a record lacking one of its named members, a record the
declared basis reaches by neither of §9.4's two routes, an evidence record
on a chain no `adoptionChains` object names, or a root or an unavailable
element outside its form) all give
`{"ok":false,"reason":"NO_LABEL"}`; misuse is a path the command cannot read or bad arguments alone, and a refusal above the
sixteen-mebibyte bound takes misuse's exit code and is no fault. A `<path>`
argument is handed to the file system as the operating system delivered
it, whatever its bytes.
§3.3's charsets are entry predicates and do not reach this file; §3.5's
depth bound does, as the fifth restriction says.

Output: the §8.7 report as one canonical JSON object with exactly these
members (empty arrays where a list is empty):
- `root`: hex20
- `basis`: the basis object as given, re-canonicalized
- `entries`: int, the number of ledger entries
- `findings`: array in §8.7 item 3 order; each finding is an object with
  `name` (one of the five), `position` (int), `entry_id` (hex32), and the
  finding's own members: `SEQ_GAP` has `expected` and `actual` (ints);
  `PREV_MISMATCH` has `seq`; `AUTHORITY_MISMATCH` has `seq` and `certain`
  (bool); `ROOT_MISMATCH` has `seq` (0) and `actual` (hex20, the entry's
  own `author`); `EQUIVOCATION` has `seq`, `a`, `b` (hex32), and its
  `entry_id` member is its `a`. Every finding also carries `hard` (bool).
- `missing`: array of `{hash:hex32, anchors:[{chainId:int, blockNumber:int,
  tx:hex32, sender:hex20}, ...]}` in §8.7 item 4 order
- `anchored`: array of `{entry_id:hex32, anchors:[{chainId:int,
  blockNumber:int, blockTimestamp:int, tx:hex32, sender:hex20}, ...]}` in
  item 5 order
- `unanchored`: array of hex32 in item 6 order
- `excluded`: array of `{seq:int, author:hex20, entry_id:hex32}` in item 7 order
- `adoption_unproven`: array of `{seq:int, entry_id:hex32, index:int}` in
  item 8 order
- `unknown_type`: array of `{seq:int, entryType:string, entry_id:hex32}` in
  item 9 order
- `malformed`: array of `{entry_id:hex32, token:string}` in item 10 order
- `unavailable`: array of hex32, sorted bytewise (item 11, after §8.1 trimming)
- `unproven`: array of `{hash:hex32, chainId:int, blockNumber:int, tx:hex32,
  sender:hex20}` in item 12 order
- `void`: array in the same row form, item 13 order
- `discarded`: array of `{chainId:int, blockNumber:int, blockTimestamp:int,
  tx:hex32, sender:hex20, hash:hex32, verdict:string}`, the seven members of
  every anchor record the lineage trim of §8.1 discarded, in item 14 order
- `label`: one of `BROKEN_CHAIN`, `UNAVAILABLE`, `GAPS`, `COMPLETE`

Where the inputs give no label (above), the output is
`{"ok":false,"reason":"NO_LABEL"}` and nothing else.

## Test key
For corpus construction any secp256k1 private key may be used; a corpus
file may carry the private key it was built with so that both
implementations can re-sign and compare.
