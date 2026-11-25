# zkk2 convergence harness: command-line contract for zikaron.kit/2

The measuring interface shared by every independent implementation of
`zikaron.kit/2` (docs/zikaron-kit-v2.md). It defines nothing about the law.
Every implementation embeds a conforming `zikaron/2` core, which embeds a
conforming `zikaron/1` core, for acceptance and audit; the criterion here
(`zikaron-conformance/kit2-py/`) embeds `zikaron-conformance/impl2-py/`,
which embeds the frozen core `zikaron-conformance/impl-py/` of release
digest `0xbecf…32fc`, and an independent implementation embeds its own
independent cores.

All output is one canonical JSON value (zikaron/2 §3.4 form) on stdout, no
trailing newline. Exit 0 whenever the program produced its JSON answer (a
rejection is an answer); exit 2 for harness misuse only. Harness misuse is
what HARNESS2.md makes it for `zk2`, a path the command cannot read being
what the paragraph on audit and notice files below defines, absent or
otherwise, other than a `kit-verify` or `grant-package` `<path>` that
resolves to a directory or a regular file (below); bad arguments; an
argument beyond a command's own list. A command's argument list is the token
sequence its line below spells: positional arguments in their places, each
option token taking its value as the next separate token and standing in any
relative order after the positional arguments, no option repeated; any other
token sequence, a repeated option, a joined `--now=<int>`, or an option
before a positional argument among them, is misuse. `--now` outside
`zikaron/2` §3.2's `int` as that section spells it (decimal digits, no sign,
no leading zero, in [0, 2^53 − 1]) is a bad argument. `kit-verify`'s and
`grant-package`'s `<path>` is what the reader was handed: a `<path>` that
resolves to a directory is the kit directory of §4.1 and the walk applies; a
`<path>` that resolves to a regular file is read in full and its bytes are a
one-file kit under §4.1a; a `<path>` that is absent, or that resolves to
something other than a directory or a regular file, is misuse; a directory
that exists and cannot be listed, and a regular file that exists and cannot
be read in full, each answer `{"subject":".","verdict":"E_KIT_UNREADABLE"}`
with exit 0 (§4.1, §4.1a). No command exits with any code but 0 and 2. No
corpus file exceeds sixteen mebibytes; a candidate accepts every input of
sixteen mebibytes or less, and above that bound it may answer or refuse, a
refusal being exit code 2 with no JSON.

Executable name: `zkk2`. Commands:

## Badge

- `zkk2 badge-encode <entry-path>...`: §3.1 over one or more entries in
  order. Output `{"payload":"zikaron-grant:..."}`; on failure
  `{"ok":false,"token":"E_BADGE_ENTRY"|"E_BADGE_TYPE"|"E_BADGE_CAP"}` with
  `"index":k` for the two per-segment tokens.
- `zkk2 badge-decode <path>`: §3.2 over the file's bytes as the payload.
  Output `{"grants":["0x<entry_id>",...],"ok":true}` or
  `{"ok":false,"token":"E_..."}` with `"index":k` for E_BADGE_B64,
  E_BADGE_ENTRY, E_BADGE_TYPE, E_BADGE_LINK, and `"inner":"E_..."` for
  E_BADGE_ENTRY.

## Record kit

- `zkk2 kit-verify <path>`: §4.1 walk (a directory) or §4.1a reading (a
  regular file), then §4.4. Output on success
  `{"counts":{"entries":n,"files":n},"invalid_entries":[{"entry_id":..,"token":..},...],"kit_id":"0x<64>","verdict":"KIT_OK"}`;
  on failure `{"verdict":"E_KIT_...","subject":"<path, id, or rule>"}`
  (no `subject` member for `E_KIT_MANIFEST_ABSENT`).

## Grant package

- `zkk2 grant-package <path>`: §4.1 or §4.1a as for `kit-verify`, then
  §5.2. Output the §5.3 result object exactly: on success
  `{"contents":[{"content":..,"path":..},...],"hops":["0x<entry_id>",...],"kit_id":"0x<64>","root":"0x<40>","terms":[["<path>",...],...],"verdict":"GP_OK"}`;
  on failure `{"subject":<as §5.3 names it, or null>,"verdict":"<token>"}`,
  the `subject` of a kit failure being that failure's subject (absent, and
  so `null` here, for `E_KIT_MANIFEST_ABSENT`), of a badge failure the
  index the badge token carries or `null`, of `E_GP_NO_CODE` and
  `E_GP_ROOT` `null`, of `E_GP_UNCARRIED` the hop's `entry_id`, of
  `E_GP_CONTENT` the row's `path`, and of `E_GP_TERMS` the pair's path as
  §5.3 writes it.

## Grant check

- `zkk2 grant-check <grant-path> [--audit <audit-input.json>] [--notices
  <notice-input.json>] [--now <int>]`: §7.2. Output the §7.3 result object
  exactly: `{"basis":<basis or
  null>,"checks":[{"n":1,"reason":null|"E_..."|"NOT_A_GRANT","state":"PASS"|"FAIL"|"UNKNOWN","token":"BAD_SIG"},...six...],"failed":[...],"notices":<notice basis or null>,"verdict":"GREEN"|"PARTIAL"|"FAIL"}`.
  Omitting `--audit` passes `null`; omitting `--notices` passes `null`;
  omitting `--now` passes `null`.
- `zkk2 chain-check <hops.json> [--now <int>]`: §7.5. `<hops.json>` is an
  array each of whose elements is an object with exactly the members
  `grant` (a path), `audit` (a path or `null`), and `notices` (a path or
  `null`), from the original author's grant onward, each path resolved
  against the working directory and never against the directory holding
  the file; any other shape, an object repeating a member name included, is
  harness misuse. Output the §7.5 result object exactly:
  `{"failing":null|{"index":k,"kind":"incomplete"|"link"|"hop"|"empty"},"hops":[<§7.3 objects>],"links":[true|false|null,...],"token":null|"CHAIN_INCOMPLETE"|"CHAIN_LINK"|"CHAIN_EMPTY","verdict":..}`.

Audit input files use the shape of HARNESS2.md (root, pile, anchors, basis);
the audit a check takes is the `zikaron/2` audit, old segment included where
the input holds one. A readable `--audit` file on which `zk2 audit` answers
`NO_LABEL`, a file that is not JSON included, is an invalid input of §1: an
answer, never misuse; an `--audit` path that cannot be read is misuse. A
notice input file is a JSON text whose value is the §6.3 object (`basis`,
`revocations`, `accessions`), read by the reader of an audit input file in
full (HARNESS2.md's restrictions: UTF-8 with no byte-order mark, no repeated
member name, integers as §3.2 spells them, hex strings as §1 spells them,
the depth bound); a readable `--notices` file whose value is no notice input
under §6.3, a file that is not JSON included, is an invalid notice input: an
answer, never misuse, read as `null` by every check and written as `null` in
the result's `notices`; a `--notices` path that cannot be read is misuse. A
hop's `notices` path is read the same way, and `<hops.json>` itself is read
by that same reader, a file it refuses being misuse. A path the command
cannot read is one that resolves to anything but a regular file or a
character device, or to no file at all, whatever the cause: an absent file,
a directory, a named pipe, a socket, a file without read permission, and a
path holding U+0000 or a lone surrogate among them; a regular file or a
character device is read to its end, and a candidate may refuse as misuse
what exceeds the sixteen-mebibyte bound, which a character device without
end exceeds, such a device lying outside the corpus and outside §10.1's
count; nothing is opened before its kind is known, so no path of another
kind blocks the command (the `<path>` of `kit-verify` and `grant-package`
excepted: a directory or a regular file, and otherwise misuse); the corpus
names no character device but `/dev/null`. A path in `argv` or in a hop is
handed to the platform as it came, bytes outside UTF-8 included, and only
the platform's refusal to open it makes it misuse; the corpus, whose
manifest is JSON, carries no such path.
