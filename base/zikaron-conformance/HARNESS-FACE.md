# zkf convergence harness: command-line contract for zikaron.face/1

The measuring interface shared by every independent implementation of
`zikaron.face/1` (docs/zikaron-face-v1.md). It defines nothing about the
law. Every implementation embeds a conforming `zikaron/2` core for
acceptance and for the signature tests; the criterion here
(`zikaron-conformance/face-py/`) embeds `zikaron-conformance/impl2-py/`,
which embeds the frozen core `zikaron-conformance/impl-py/`, and an
independent implementation embeds its own independent cores.

All output is one canonical JSON value (`zikaron/2` §3.4 form) on stdout, no
trailing newline. Exit 0 whenever the program produced its JSON answer (a
rejection is an answer); exit 2 for harness misuse only, with nothing on
stdout. Harness misuse is: a path the command cannot read; a bad argument;
an argument count other than the command's own. No command takes an option,
and every argument is positional. No command exits with any code but 0 and
2; the diagnostic a misuse exit writes to stderr is the implementation's
own. No corpus file exceeds sixteen mebibytes; a candidate accepts every
input of sixteen mebibytes or less, and above that bound it may answer or
refuse, a refusal being exit code 2 with no JSON.

A path the command cannot read is one that resolves to anything but a
regular file or a character device, or to no file at all, whatever the
cause: an absent file, a directory, a named pipe, a socket, and a file
without read permission among them. A regular file or a character device is
read to its end, and a candidate may refuse as misuse what exceeds the
sixteen-mebibyte bound, which a character device without end exceeds.
Nothing is opened before its kind is known, so no path of another kind
blocks the command. A path argument is handed to the platform as the
operating system delivered it, whatever its bytes, and only the platform's
refusal to open it makes it misuse. The corpus names regular files, and for
misuse an absent path and a directory.

`<root>` and `<address>` are `hex20`s as `zikaron/2` §1 spells one, `0x` and
forty lowercase hexadecimal digits; `<role>` is a label (law §1); any other
spelling of one of them is a bad argument. `<privkey-hex>` is sixty-four
hexadecimal digits of either case, with or without a leading `0x` or `0X`,
naming a scalar in [1, n − 1], as HARNESS2.md's `sign` takes it; any other
argument there is a bad argument.

Executable name: `zkf`. Commands:

## `zkf entry <entry-path> <root>`

Reads the file's bytes as `b` and answers §1's `accept(b)`, then §2.2 and
§3.3, under the root `<root>`.

- `accept` rejects: `{"ok":false,"token":"E_..."}`, the token `zikaron/2`
  §4.3 names.
- An entry of another type: `{"entry_id":"0x<64
  hex>","face":false,"ok":true}`.
- A face entry: `{"consent":<bool or null>,"entry_id":"0x<64
  hex>","face":true,"ok":true,"rooted":<bool or
  null>,"shape":<"home"|"account"|"key"|"persona"|"release"|"reroot"|"origin"|null>}`.
  `shape` is the body's shape (§2.2) or `null`; `rooted` is whether the
  body's `root` is byte-equal to `<root>`, and `null` where the body has no
  shape; `consent` is whether the consent holds (§3.3) for a body of the
  `key` shape, read from the body's own three members whatever `<root>`
  says, and `null` for every other body.

## `zkf declare <root> <address> <role> [<privkey-hex>]`

§3.1 and §3.2 for the three arguments: `{"declaration":"0x<hex of
T>","digest":"0x<64 hex>","message":"0x<hex of M>"}`, the hexadecimal digits
lowercase. With `<privkey-hex>` the answer carries two members more: `sig`,
the consent that key gives to the declaration, signed as §3.4 obliges (RFC
6979 over the digest, low-s by negation), `0x` and 130 lowercase hexadecimal
digits; and `signer`, that key's address. The command signs with the key it
is given, whatever `<address>` says: the corpus builds consents that hold
and consents that do not with it.

## `zkf carried <calldata-path>`

Reads the file's bytes as `c` and answers §5.1: `{"carried":"0x<64 hex>"}`,
the `entry_id` of `carried(c)`, or `{"carried":null}`.

## `zkf table <face-input.json>`

The file holds a face input (§6.1) as a JSON text. It is read by the reader
of an audit input file in HARNESS2.md, with that reader's five restrictions
and no others: valid UTF-8 with no byte order mark; every number written as
`zikaron/2` §3.2 writes an integer and within [0, 2^53 − 1]; no object
repeating a member name; every string decoding to scalar values; no
container opening at nesting depth 129. A file that reader refuses, and a
value that is no valid face input under §6.1 and §6.2, give
`{"ok":false,"reason":"NO_TABLE"}`: an answer, never misuse. Beyond the
argument count, misuse here is a path the command cannot read and nothing
else. An anchor record of the input may carry members beyond its seven,
which the command reads past (law §6.1).

On a valid face input the output is the face table of §7.8, one canonical
JSON object with exactly the members that section lists: `basis`, `faces`,
`home`, `homes`, `lineage`, `marker`, `misread`, `office`, `origin`,
`persona`, `reroot`, `root`, `unproven`, `unread`, empty arrays where a list
is empty, `basis` being the input's `chains` re-canonicalized. A name (law
§1) is written as `zikaron/2` §3.4 writes any string: its scalar values as
their UTF-8, a quotation mark and a backslash escaped.

## `zkf joined <pair.json>`

The file holds a JSON array of exactly two face inputs, the first and the
second of law §7.9. It is read by the reader of `table`, with that reader's
five restrictions and no others. The pair's array is the first level of the
file and the depth bound counts from it, so a face input that alone opens a
container at depth 128 has no spelling as an element of a pair: the reader
refuses that file. A file that reader refuses, a value that is no array of
exactly two elements, and an array either of whose elements is no valid face
input under §6.1 and §6.2 give `{"ok":false,"reason":"NO_TABLE"}`: an
answer, never misuse. Beyond the argument count, misuse here is a path the
command cannot read and nothing else.

On two valid face inputs the output is `{"joined":true}` where the first is
joined to the second (§7.9), and `{"joined":false}` otherwise.

## Test keys

For corpus construction any secp256k1 private key may be used. The corpus
manifest states the argument vector of every case, a path being relative to
the corpus directory, and the answer its author predicts for it.
