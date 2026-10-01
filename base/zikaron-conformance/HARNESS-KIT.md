# zkk convergence harness: command-line contract for zikaron.kit/1

The measuring interface shared by every independent implementation of
`zikaron.kit/1` (docs/zikaron-kit-v1.md). It defines nothing about the law.
Every implementation embeds a conforming `zikaron/1` core for acceptance
and audit; the criterion here (`zikaron-conformance/kit-py/`) embeds the
frozen core `zikaron-conformance/impl-py/` of release digest
`0xbecf…32fc`, and an independent implementation embeds its own
independent `zikaron/1` core.

All output is one canonical JSON value (zikaron/1 §3.4 form) on stdout, no
trailing newline. Exit 0 whenever the program produced its JSON answer (a
rejection is an answer); exit 2 for harness misuse only. Harness misuse is
what HARNESS.md makes it for `zk1`: a path the command cannot read, absent or otherwise; bad arguments; an
argument beyond a command's own list. A command's argument list is the
token sequence its line below spells: positional arguments in their places,
each option token taking its value as the next separate token and standing
in any relative order after the positional arguments, no option repeated;
any other token sequence, a repeated option, a joined `--now=<int>`, or an
option before a positional argument among them, is misuse. `<work-hex32>` outside `zikaron/1` §1's `hex32`, and `--now` outside `zikaron/1` §3.2's `int` as that section spells it (decimal digits, no sign, no leading zero,
in [0, 2^53 − 1]), are bad arguments. `kit-verify`'s `<dir>` is the directory the reader was handed and is no
entry of the walk: a `<dir>` that resolves to a directory, through symbolic
links or not, is that directory, and §7.1's rule against following links
reaches the entries the walk examines and never its own argument; a `<dir>`
that is absent, or that resolves to something other than a directory, is
misuse; a directory that exists and cannot be listed is §7.1's first walk failure and answers
`{"subject":".","verdict":"E_KIT_UNREADABLE"}` with exit 0. No command exits
with any code but 0 and 2. No corpus file exceeds sixteen mebibytes; a
candidate accepts every input of sixteen mebibytes or less, and above that
bound it may answer or refuse, a refusal being exit code 2 with no JSON.

Executable name: `zkk`. Commands:

## Documents

- `zkk fpm-check <path>`: §4.2 over the file's bytes.
  Accepted: `{"doc_id":"0x<64>","ok":true}`. Rejected:
  `{"ok":false,"token":"E_..."}`, plus `"index":k` when the token is
  `E_FPM_ROW`.
- `zkk ack-check <path>`: §5.2, same shapes (no index).
- `zkk sign <privkey-hex> <path> <domain>`: `<path>` holds the canonical
  bytes of the document without `sig` (any RFC 8259 spelling is
  re-canonicalized). The command reads no member of the file and removes
  none: a file carrying a top-level `sig` is signed with it inside the
  preimage, exactly as `zk1 sign` does, and stripping it before signing is
  the caller's own step. `<privkey-hex>`, `<path>`, and misuse are as
  HARNESS.md states them for `zk1 sign`: `<privkey-hex>` is sixty-four
  hexadecimal digits of either case, with or without a leading `0x` or
  `0X`, naming a scalar in [1, n − 1], and any other argument is harness
  misuse; a `<path>` whose bytes fail `zikaron/1` §3.5 tests 1 through 5,
  and one whose value is not an object, are each harness misuse (exit 2, no
  JSON); an object of any member count is signed. `<domain>` is exactly
  `zikaron.fpm/1` or `zikaron.ack/1`. Output
  `{"digest":..,"presig":..,"sig":..,"signer":..}` as in HARNESS.md.
- `zkk pair <fpm-path> <ack-path>`: §5.3. Output `{"verdict":"<name>"}`;
  when `PAIRED` also `"recipient"` and `"variant"`; when `FPM_INVALID` or
  `ACK_INVALID` also `"token"` with the document token.
- `zkk attribute <fpm-path> <ack-path> <bytes-path>`: §5.4. Output
  `{"attributed":true,"recipient":..,"verdict":"ATTRIBUTED"}`,
  `{"attributed":false,"verdict":"NOT_ATTRIBUTED"}`, or
  `{"attributed":false,"verdict":"<pairing verdict>"}`.

## Badge

- `zkk badge-encode <entry-path>...`: §6.1 over one or more entries in
  order. Output `{"payload":"zikaron-grant:..."}`; on failure
  `{"ok":false,"token":"E_BADGE_ENTRY"|"E_BADGE_TYPE"|"E_BADGE_CAP"}` with
  `"index":k` for the two per-segment tokens.
- `zkk badge-decode <path>`: §6.2 over the file's bytes as the payload.
  Output `{"grants":["0x<entry_id>",...],"ok":true}` or
  `{"ok":false,"token":"E_..."}` with `"index":k` for E_BADGE_B64,
  E_BADGE_ENTRY, E_BADGE_TYPE, E_BADGE_LINK, and `"inner":"E_..."` for
  E_BADGE_ENTRY.

## Disclosure kit

- `zkk kit-verify <dir>`: §7.1 walk then §7.4. Output on success
  `{"counts":{"entries":n,"files":n,"proofs":n},"invalid_entries":[{"entry_id":..,"token":..},...],"kit_id":"0x<64>","verdict":"KIT_OK"}`;
  on failure `{"verdict":"E_KIT_...","subject":"<path, id, or rule>"}`
  (no `subject` member for `E_KIT_MANIFEST_ABSENT`). A proof file's contents are opaque (§7.5): §7.4 step 5 pins its bytes by
  digest and this law reads nothing inside them.

## Depth

- `zkk depth <audit-input.json> <work-hex32>`: §9. Output the reading
  object of §9.2 exactly (`{"valid":false}` for an invalid input).

## Grant check

- `zkk grant-check <grant-path> [--audit <audit-input.json>] [--now <int>]`:
  §10.2. Output the §10.3 result object exactly:
  `{"basis":<basis or null>,"checks":[{"n":1,"reason":null|"E_..."|"NOT_A_GRANT","state":"PASS"|"FAIL"|"UNKNOWN","token":"BAD_SIG"},...six...],"failed":[...],"verdict":"GREEN"|"PARTIAL"|"FAIL"}`.
  Omitting `--audit` passes `null`; omitting `--now` passes `null`.
- `zkk chain-check <hops.json> [--now <int>]`: §10.5. `<hops.json>` is an
  array each of whose elements is an object with exactly the members
  `grant` (a path) and `audit` (a path or `null`), from the original
  author's grant onward, each path resolved against the working directory
  and never against the directory holding the file; any other shape, an
  object repeating a member name included, is harness misuse. Output the §10.5 result object exactly:
  `{"failing":null|{"index":k,"kind":"incomplete"|"link"|"hop"|"empty"},"hops":[<§10.3 objects>],"links":[true|false|null,...],"token":null|"CHAIN_INCOMPLETE"|"CHAIN_LINK"|"CHAIN_EMPTY","verdict":..}`.

Audit input files use the shape of HARNESS.md (root, pile, anchors,
unavailable, evidence, basis).
