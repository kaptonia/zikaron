# zikaron CLI · for programs

What a program needs to call `zikaron`: verbs, flags, exit codes, refusal reasons and answer keys, one item a line.
`zikaron contract` answers the verbs, each verb's flags and must-give cells, the exit codes and every refusal
reason as one JSON value, read from the same tables the command line runs by; where this page and that answer
differ, the answer holds.

## 1 · General rules

1. stdout is one canonical JSON value (no whitespace, members in byte order), no trailing newline; `mcp` aside.
2. Misuse (exit 2) writes zero bytes to stdout; its first stderr line is `<REASON> <subject>`.
3. A shell answer starts `{"ok":…}`: `true` when affirmed; otherwise `false` with `reason`.
4. `audit`, `check-grant`, `chain-check` (and `grant-package --package`'s `check`) pass the core's object through unchanged; only the exit code is chosen.
5. A file is written only by `--out` (or `--originals-to`): written whole beside its place, then moved in; an existing name is refused, nothing overwritten.
6. Every flag is a long flag taking a value; each verb's flag list is closed; a flag outside it is misuse.
7. Time: deadlines and windows are judged by `--now` or the chain's time; a grant check given `--endpoint` and no `--now` takes the latest block's time on the chain it reads; neither, and a window's check is `UNKNOWN`.
8. A run that could not keep part of what its chain reads settled adds `unkept`.
9. A grant check with a check `UNKNOWN` for a missing piece adds `gaps`.
10. Lines for people go to stderr, in Chinese when the first set of `LC_ALL`, `LC_MESSAGES`, `LANG`, `LANGUAGE` is `zh…`, English otherwise; they are not part of the contract.
11. A secret is never echoed: a misuse subject names a typed value by its place and length (`#3 (12 bytes)`), a flag by `--name`.

## 2 · Exit codes

- `0` affirmed: answered yes (`COMPLETE`, `GREEN`, `KIT_OK`, `BADGE_OK`).
- `1` denied: answered no (`FAIL`, `BROKEN_CHAIN`, `NO_LABEL`, a refusal of the law or the shell).
- `2` misuse: malformed arguments, an unreadable path, a flag the verb does not take; stdout empty.
- `3` partial: answered, neither yes nor no (`PARTIAL`, `GAPS`, `UNPROVEN`, `E_QUEUED`, a record not anchored).
- `4` unanswered: could not answer (no node reached, readings disagree, scan refused, no randomness, `E_UNCHECKED`); never "absent".

## 3 · Answer shapes

- Affirmed: `{"ok":true, <keys of section 7>}`.
- Refused: `{"ok":false,"reason":"E_…", <keys>}`; `token` the refusing layer's code, `detail` its evidence in machine words.
- Base answer: the `zikaron/2` audit report (a continued ledger's old segment in `continues`; an unmoved `zikaron/1` ledger by its own core), or the `zikaron.kit/2` six checks or chain check (`zikaron.kit/1` for a `zikaron/1` grant of a ledger not continued).

## 4 · Refusal reasons

Each line: reason, exit, when.

- `E_ARGS` 2: malformed arguments.
- `E_UNREADABLE` 2: a path does not read, or a ledger is sealed local data outside a home; 1 when the sealed layer refuses a home's file or this machine's lists.
- `E_KEY` 2: a private key not a scalar in [1, n−1]; 1 when a key gives no address.
- `E_RANDOM` 4: no randomness.
- `E_LEDGER` 1: the store refused (`detail`, `names?`, `count?`).
- `E_ENTRY` 1: the core refused the entry (`token`; `E_BODY_FIELD` adds a stderr line naming the flags).
- `E_TIP_ABSENT` 1: the lineage has no entries.
- `E_ENTRY_ABSENT` 1: a named entry is not in the ledger.
- `E_TIP_FORKED` 1: more than one entry at the last seq, or two roots and no `--root`.
- `E_NOT_MOVED` 1: a `zikaron/1` ledger takes no new entry before `migrate`.
- `E_ALREADY_MOVED` 1: `migrate` on a `zikaron/2` ledger.
- `E_SCAN` 4: the anchoring layer refused the scan (`token`; `E_NO_BASIS` with no chain or registry set).
- `E_ENDPOINTS_DISAGREE` 4: no two places give one reading, or two readings each given twice.
- `E_UNREACHABLE` 4: no node answered, the send failed, or the wait ran out.
- `E_TX_STATUS` 1: included with status other than 1.
- `E_TX_NOT_YET` 4: the wait ran out before inclusion (`tx`, `count`).
- `E_TX_VOID` 1: what was sent will never be included; its nonce went to another transaction.
- `E_KIT` 1: the kit law judged the package invalid (`state`).
- `E_KIT_PATH` 1: `kit-export --file` path breaks the package naming rule (`path`, `token`).
- `E_BADGE` 1: badge encode or decode refused.
- `E_FRAGMENT` 1: a fragment lacks anchors or basis.
- `E_ALREADY_ROOTED` 1: `init` on a ledger with a root (`author`, `count`).
- `E_NOT_EMPTY` 1: `init` on a ledger that is not empty.
- `E_RETRACTION` 1: `retract` refused by the retraction rule (`token`).
- `E_WOULD_BREAK` 1: the entry would add a chain finding (`entryId`, `names`; `token` `NOT_IN_LINEAGE` for a key outside the lineage).
- `E_GRANT_FILE` 1: a grant file or package does not open (`token`).
- `E_GAS_REFUSED` 1: the node refused the gas estimate for the call itself, or past the ceiling.
- `E_DESKTOP` 4: `--home` got no answer; `detail` one of `NOT_OPEN`, `CLOSED`, `PATH_TOO_LONG`, `NOT_YOU`, `BROKEN`, `NO_MACHINE`, `ALONE`.
- `E_ON_DESKTOP` 1: done on the desktop only (`migrate --home`, `migrate --ledger` naming a home, `init --ledger` naming a home with no entry yet).
- `E_QUEUED` 3: `anchor` on a home whose setting leaves sending to the user (`count`).
- `E_CREDENTIAL_EXPIRED` 1: the credential expired; removed.
- `E_CREDENTIAL_MISSING` 1: no credential ("With a credential" off).
- `E_CREDENTIAL_SHAPE` 1: the credential is not in its shape, or the key brought opens another master key; removed.
- `E_HOME_HELD` 1: the desktop is writing the home now; try again.
- `E_HOME_OTHER_MACHINE` 1: the home's writer mark names another machine.
- `E_ADOPTED` 1: a plain folder this machine's desktop adopted; written through the desktop.
- `E_RECORDS` 1 or 3: `kit-verify`, not every record holds (1 for an original that is not the one recorded).
- `E_HANDED_OFF` 1: the ledger was handed off and not taken back (`handoff`).
- `E_HANDOFF` 1: `take-over` refused the handoff (`token`).
- `E_ON_CHAIN` 1: a hash given is already anchored from this key (`names`, `tx`, `blockNumber`); nothing sent.
- `E_TOO_EARLY` 4: the head is not yet later in time than the last anchor (`detail` `chainId= headTime= after=`).
- `E_SEND_REFUSED` 1 or 4: refused before anything went out (`token`, e.g. `INSUFFICIENT_FUNDS`, `QUEUE_EMPTY`, `GAS_NOT_ESTIMATED`); 4 for a network refusal.
- `E_READ_ONLY` 1: the home is read-only (`token` `READ_ONLY`, `LEDGER_NOT_FETCHED`, `NEWER_ELSEWHERE`, `BROKEN`).
- `E_GRANT_WITHHELD` 1: `grant` refused by the gate before signing (`token`, `names`).
- `E_REVOCATION` 1: `revoke` names no grant of this ledger (`token`, `entryId`).
- `E_ISSUE_REFUSED` 1 or 4: the chain-side check before a grant or revocation with the revocation member (`token`, `detail`).
- `E_NOT_NOTICED` 4: the revocation was written but its notice was not read back counted (`notice`).
- `E_ANNOTATION` 1: `annotate` names an entry that may not be named (`token`, `entryId`).
- `E_EXTRACT` 1: originals not taken out (`token` `PATH`, `NOTHING`, `CLASH`, `NAME`, `NOT_A_FOLDER`, `EXISTS`, `LINK`, `CHANGED`, `LANDING`).
- `E_ADDRESSED` 1 or 4: a package sealed to one recipient refused (`token` `ADDRESSED_…` or `ADDRESS_…`).
- `E_FACE_REFUSED` 1 or 4: the face gate, the chain's book, or no face table read (`token`, `detail`).
- `E_CREDENTIAL_PASS_MISSING` 1: no program key in `ZIKARON_CLI_PASS`.
- `E_CREDENTIAL_PASS_UNKNOWN` 1: the program key opens no row (`detail` `FORM` or `NONE`).
- `E_UNCHECKED` 4: a grant whose check cannot be made on the logs it reads.
- `E_<code>` 1 or 4: through the desktop, a cause of the desktop's own (e.g. `E_DOOR_UNREAD`, `E_IN_FLIGHT`, `E_BOOK_NOT_WHOLE`); `contract` lists all.

## 5 · Verbs

Each line: verb, what it does, what a call must give, the success keys. Flags per verb: `zikaron contract`.

- `keygen`: a new key; `address`, `privkey`.
- `init`: genesis of a plain ledger; needs `--key`, `--ledger`; `entryId`, `ledger`, `seq`, `written`.
- `history`: record content or a file (`--file`); needs `--ledger`; `entryId`, `ledger`, `seq`, `written`.
- `grant`: issue a grant; needs `--ledger`; as `history`.
- `revoke`: revoke a grant; needs `--ledger`; as `history`, plus `notice`, `tx` for a grant with the revocation member.
- `annotate`: a note on an entry or the ledger; needs `--ledger`; as `history`.
- `retract`: delete a record; needs `--ledger`; as `history`.
- `migrate`: continue a `zikaron/1` plain ledger under `zikaron/2`; needs `--ledger`; `closing`, `continuing`, `entries`, `ledger`.
- `anchor`: send an anchoring transaction (`--key` direct, `--ledger` a home's queue or a plain folder's batch, `--home` through the desktop); `blockNumber`, `tx`.
- `scan`: read anchors on chain; needs any of `--ledger`, `--fixture`, `--endpoint`; `fragment`, `sources`, `singleSource`, `singleSourceChains`, `singleSourceHistory`, `unanswered`, `unproven`, `differed`, `book`, `bookUnread`.
- `audit`: audit a ledger; needs any of `--ledger`, `--input`; base answer; `--out` writes the audit input.
- `check-grant`: six checks of a grant; needs `--grant`; base answer, `gaps?`.
- `chain-check`: check a chain of grants (`--hop`, repeatable); base answer, `gaps?`.
- `badge`: encode or decode a badge; needs one of `--encode`, `--decode`; `payload` or `count`, `entries`; `state`.
- `kit-export`: export a record package; needs `--ledger`, `--out`; `dropped`, `entries`, `files`, `kitId`, `path`, `state`, `originals?`, `leftOut?`.
- `kit-verify`: verify a record package; needs `--kit`; `entries`, `kitId`, `records` (each `entryId`, `content`, `original`, `anchored`, `firstAt?`, `deleted?`), `taken?`, `path?`.
- `grant-package`: export (`--out`) or verify (`--package`) a grant package; needs one of them; with `--out` `files`, `kitId`, `leftOut`, `originals`, `path`, `sealed`; with `--package` `check` (base answer), `package`, `gaps?`, `path?`, `taken?`.
- `clean`: list (`planned`) or, with `--yes yes`, remove (`removed`) this machine's plain exports; needs one of `--home`, `--ledger`; `kept`, `unlisted`; rows `kind`, `path`, `link?`, `why?`.
- `hand-over`: hand a plain ledger to another place; needs `--ledger`, `--out`; `entries`, `handoff`, `path`.
- `take-over`: take a handoff; needs `--ledger`, `--handoff`; `count`, `entries`, `handoff`, `ledger`.
- `show`: read one entry; needs one of `--path`, `--entry`; `author`, `entryId`, `entryType`, `prev`, `seq`, `value`.
- `contract`: the contract; `exits`, `reasons`, `standsFor`, `verbs` (each `name`, `flags`, `homeFlags?`, `required`).
- `mcp`: the Model Context Protocol on stdin and stdout (section 13).
- `face-table`: read a root's face table; needs one of `--root`, `--ledger`, `--home`; `table`, `moment`, `omitted`.
- `face-home`, `face-account`, `face-key`, `face-name`, `face-release`, `face-reroot`, `face-origin`: write a face entry; need `--ledger`; as `history`.
- `face-joined`: whether two roots' face tables are joined; needs `--root`, `--to`; `joined`, `readTo`.

## 6 · Label to exit

- Ledger label: `COMPLETE` 0, `GAPS` 3, `UNPROVEN` 3, `BROKEN_CHAIN` 1, `NO_LABEL` 1 (a `zikaron/1` core's `UNAVAILABLE` reads `UNPROVEN`).
- Kit verdict: `GREEN` 0, `PARTIAL` 3, `FAIL` 1.

## 7 · Answer keys

- `ok`: whether affirmed.
- `reason`: the refusal reason.
- `detail`: evidence, machine words.
- `token`: the refusing layer's code.
- `dropped`: platform junk left out of a package.
- `index`: a position in a list given.
- `state`: a status token (`KIT_OK`, `BADGE_OK`, a layout code).
- `entryId`, `entryType`, `seq`, `prev`, `author`, `value`: an entry's id, type, position, link, author, body.
- `address`, `privkey`: a new key.
- `ledger`: the ledger folder written.
- `entries`: a count of entries, or the entries decoded.
- `files`: the files a package carries.
- `kitId`: a package's id.
- `path`: a path written or named.
- `fragment`: the scan fragment.
- `sources`: the places a reading came from.
- `singleSource`, `singleSourceChains`, `singleSourceHistory`: readings fewer than two places gave (now, by chain, from kept history).
- `unanswered`: nodes that did not answer, with why.
- `unproven`, `differed`: readings not proved, readings that differed.
- `book`, `bookUnread`: succession book rows read (`root`, `key`), places not read.
- `unkept`: parts of the reading this machine could not keep.
- `gaps`: why a check is `UNKNOWN` (`n`, `token`, `hop?`, `gap`, `zh`, `en`).
- `root`, `key`: a root and its key in office.
- `tx`, `blockNumber`: a transaction and its block.
- `payload`: a badge payload.
- `count`: a count (entries waiting, entries written, seconds waited).
- `written`: whether the entry was newly written.
- `names`: names (hashes, findings, items).
- `records`: per record of a package.
- `content`, `original`, `anchored`, `firstAt`, `deleted`: a record's content, original match, anchoring, first anchor time, deleting entry.
- `originals`, `leftOut`: originals taken into a package, places not taken.
- `handoff`: a handoff's id.
- `taken`: originals taken out.
- `closing`, `continuing`: `migrate`'s closing succession and continuing genesis.
- `removed`, `planned`, `kept`, `unlisted`: `clean`'s rows.
- `kind`, `link`, `why`: a `clean` row's kind (`kit`, `grant`, `mirror`, `proofs`), publish address, reason kept.
- `sealed`: whether a grant package is sealed.
- `notice`: what reading a revocation's notice back found.
- `table`, `moment`, `omitted`: a face table, the moment read to, chains left out.
- `joined`, `readTo`: `face-joined`'s answer and the tables read.

## 8 · Flags

Each line: flag, value. `--key-file` stands for `--key`.

- `--ledger`: a ledger folder (plain, or a home's; a home's data folder names its `ledger` folder; relative from where the command runs).
- `--key`: private key, 64 hex digits (`0x` allowed); `kit-verify`: the key that opens a sealed package.
- `--key-file`: a file holding the key (at most 4096 bytes).
- `--root`: hex20 root.
- `--seq`, `--prev`: the position and link, given together or not at all.
- `--statement`: genesis statement (`migrate`: of the closing succession and the genesis).
- `--content`: hex32 content digest.
- `--mark`, `--toolchain`: a `history` mode (token, hex32).
- `--note`: prose.
- `--grantee`, `--work`, `--terms`, `--history`, `--upstream`, `--supersedes`: grant members (hex20 grantee, hex32 the rest).
- `--window-from`, `--window-to`: a grant window, whole numbers at most 9007199254740991.
- `--scope`: grant scope prose.
- `--grant`: `revoke` the grant id; `check-grant` a grant file or package; `grant-package` the grant to export.
- `--case`: hex32 revocation case.
- `--to`: a root (`kit-export`, `grant-package` seal to its key in office; `face-reroot`, `face-joined`).
- `--effective`: `migrate`'s effective time.
- `--subject`: the entry named (`annotate`, `retract`, `face-release`).
- `--endpoint`: `<chain id>=<url>`, repeatable.
- `--form`: `registry`.
- `--registry`: hex20 registry address.
- `--hash`: hex32, repeatable.
- `--calldata`: hex bytes, instead of `--hash`.
- `--wait-secs`: seconds to wait for inclusion (default 90).
- `--proxy`: `system` (default), `none`, or `scheme://[user[:password]@]host:port` (`http`, `https`, `socks5`, `socks5h`, `socks4`).
- `--rest-keys`: a file of `<service> <key>` lines.
- `--fixture`: a recording file, repeatable.
- `--basis`: a basis file.
- `--input`: an audit input file.
- `--fragment`: a scan fragment or reading file.
- `--book`: a `scan` reading with book rows, repeatable.
- `--notices`: a notice input file, repeatable.
- `--now`: the injected now, Unix seconds.
- `--hop`: `<grant file>` or `<grant file>=<audit input>`, repeatable.
- `--entry`: an entry id, repeatable for `kit-export`.
- `--path`: a file `show` reads.
- `--out`: the file or folder written.
- `--file`: `history` the file recorded; `kit-export` `<path in kit>=<path on disk>`, repeatable.
- `--encode`, `--decode`: badge entry file (repeatable) or payload file.
- `--as`: `file` (default) or `folder`.
- `--kit`: a record package.
- `--originals`: `none` (`kit-export`), `all` (`grant-package`).
- `--originals-to`: an existing folder for originals taken out.
- `--package`: a grant package to verify.
- `--yes`: the word `yes`.
- `--to-key`: a public key to seal a package to.
- `--handoff`: a handoff file.
- `--home`: the desktop's data folder (section 11).
- `--chain`: `face-home` chain id.
- `--platform`, `--handle`: `face-account`.
- `--address`, `--role`, `--sig`: `face-key`.
- `--name`: `face-name`.
- `--from`: `face-origin` the root the faces came from.

## 9 · Environment variables

- `ZKA_TIMEOUT_SECS`: one node exchange's deadline, seconds (default 30; 0 none).
- `ZKA_TIMEOUT_MS`: the same in milliseconds; overrides the seconds.
- `ZKA_MAX_ANSWER_BYTES`: largest node answer (default 67108864; 0 none).
- `ZIKARON_TRACE`: a file to append trace marks to.
- `ZIKARON_CLI_PASS`: a program key (`zkp_` + 64 hex digits), read only when a sealed home is opened.
- `https_proxy`, `http_proxy`, `all_proxy`, `no_proxy`: followed under `--proxy system`; a home's road uses the machine's proxy setting.
- `LC_ALL`, `LC_MESSAGES`, `LANG`, `LANGUAGE`: the language of stderr lines.

## 10 · `contract`

- Takes no flags; answers exit 0: `ok`, `exits` (`code`, `name`), `reasons`, `standsFor` (`flag`, `for`), `verbs` (`name`, `flags`, `homeFlags?`, `required`: one array per cell).

## 11 · `--home`: through the desktop

- The running desktop does the verb in its data folder; the command line holds no key.
- Verbs: `init`, `history`, `grant`, `revoke`, `annotate`, `retract`, `anchor`, `kit-export`, `kit-verify`, `grant-package`, `clean`, the face verbs; `migrate --home` answers `E_ON_DESKTOP`; the rest take no `--home`.
- Each takes `--home` and these flags only:
  - `init`: `statement`.
  - `history`: `file`, `note`.
  - `grant`: `grantee`, `work`, `terms`, `history`, `window-from`, `window-to`, `scope`, `upstream`, `supersedes`.
  - `revoke`: `grant`, `case`.
  - `annotate`, `retract`: `subject`, `note`.
  - `anchor`, `face-table`: none.
  - `kit-export`: `entry`, `note`, `out`, `as`, `to`, `to-key`, `originals`.
  - `kit-verify`: `kit`, `originals-to`.
  - `grant-package`: `grant`, `originals`, `to`, `out`.
  - `clean`: `yes`.
  - `face-home`: `chain`; `face-account`: `platform`, `handle`, `note`; `face-key`: `address`, `role`, `sig`, `note`; `face-name`: `name`; `face-release`: `subject`; `face-reroot`: `to`; `face-origin`: `from`; `face-joined`: `root`, `to`.
- No door for the folder (locked, not running, another role or identity): `E_DESKTOP` `NOT_OPEN`; "With a credential" on: `ALONE`; closed meanwhile: `CLOSED`.
- Refusals carry the reason the command line gives the same cause; a cause of the desktop's own is `E_` + its code.

## 12 · With a credential

- Settings > Command line and AI control: "With a credential" on lets the command line open the homes the desktop keeps sealed, desktop open or not; "Through the desktop app" alone uses the door.
- The program brings its program key in `ZIKARON_CLI_PASS`; keys are added on the desktop, shown once, deleted one by one.
- Writes: `history`, `grant`, `revoke`, `annotate`, `retract` and the face writes with `--ledger <home>`; signed with the home's key in the desktop's vault; `--key` is misuse there.
- `init --ledger <home>`: `E_NOT_EMPTY` with entries, `E_ON_DESKTOP` without.
- Order of refusals: `E_CREDENTIAL_MISSING`, `E_CREDENTIAL_EXPIRED`, `E_CREDENTIAL_SHAPE`, `E_CREDENTIAL_PASS_MISSING`, `E_CREDENTIAL_PASS_UNKNOWN`; then `E_HOME_HELD`, `E_HOME_OTHER_MACHINE`; then the write gates: read-only mark, the ledger's last position (`E_TIP_FORKED`), a broken chain (`E_READ_ONLY`), handed off (`E_HANDED_OFF`).
- `anchor --ledger <home>` sends the queue by the home's setting (*Send automatically* or `E_QUEUED`); nothing to send: a batch in flight is settled, else `E_SEND_REFUSED` `QUEUE_EMPTY`.
- `kit-export`, `grant-package --out` from a home pass the exit gate first (`E_UNREACHABLE`, `E_ENDPOINTS_DISAGREE`, `E_SCAN`, `E_READ_ONLY` `NEWER_ELSEWHERE`).
- `hand-over`, `take-over` move a plain ledger between places; a plain folder the desktop adopted answers `E_ADOPTED`.

## 13 · `mcp`

- `zikaron mcp`: JSON-RPC 2.0 on stdin and stdout, one message a line; versions `2025-11-25`, `2025-06-18`; capability `tools`.
- Methods: `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`, `notifications/cancelled`; others `-32601`; malformed `-32600`, not JSON `-32700`; unknown tool `-32602`.
- Flags: `--rest-keys`; the key road `--key-file` with `--endpoint` (repeatable) and `--proxy`.
- Each call runs the verb as this program in its own process; the result's text is that stdout, `structuredContent` `{"answer":…,"exit":…}`; exit 2 and a refused parameter are `isError`.
- Write tools take `home`: with the credential `--ledger <home>/ledger`, else `--home <home>`; on the key road a plain folder with `--key-file`.
- Tools: `history`, `grant`, `revoke`, `annotate`, `retract`, `kit-export`, `anchor`, `show`, `audit`, `scan`, `check-grant`, `chain-check`, `badge`, `kit-verify`, `grant-package`, `clean`, `contract`, `face-table`, `face-home`, `face-account`, `face-key`, `face-name`, `face-release`, `face-reroot`, `face-origin`, `face-joined`.
- No tool takes a private key; a path parameter is an absolute path on this machine.
- Cancelling stops a read; a write runs to its end and is answered.
- Configuration from "Connect an AI app": `{"mcpServers":{"zikaron":{"args":["mcp"],"command":"<full path of zikaron>"}}}`, with `env` `ZIKARON_CLI_PASS` while the credential is on.
