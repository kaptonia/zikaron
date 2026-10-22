# zikaron CLI · output schema and exit codes

This document is the readable form of the output shapes, exit codes, refusal reasons and flags of the
`zikaron` command line. Programs that call it and the command line itself follow the same contract.

The source of truth in the code is `crates/zikaron-cli/src/codes.rs`. Three of its closed tables are
written out here in checkable form: exit codes, refusal reasons and output keys, matching `Exit::ALL`,
`Reason::ALL` and `Key::ALL` entry by entry. The flags come from `ALL_FLAGS` and `MORE_FLAGS` in `verbs.rs`
(section 8). The fourth table in `codes.rs`, the law's field names (`Field`), belongs to the law texts and is
not repeated here.

---

## 1 · General rules

1. **stdout is one canonical JSON value, with no trailing newline.** The byte form follows law §3.4
   (no whitespace, members in byte order, integers written plainly, the escaping of §3.4) and is
   produced by `json::canon_bytes` in `zikaron`; the command line has no printer of its own.
2. **One value per invocation.** A line end is not a separator.
3. **There is only one way to write a file.** The `--out` path writes a temporary sibling in full,
   fsyncs it and renames it into place; **if a file with that name already exists the write is
   refused and not one byte is overwritten**, and when the write cannot complete nothing is left at
   that path. A file is written only after the verdict says it may be. `kit-verify --originals-to` writes the
   originals into a folder that is there by the one way the desktop takes them (section 6): laid out in a staging
   folder inside it, checked, then placed without replacing anything; a name already there refuses them all and
   nothing is written.
4. **Misuse writes zero bytes to stdout.** "Are there any bytes?" is a one-bit signal for the
   caller: bytes mean an answer, no bytes mean misuse, and the two can be told apart without parsing.

   This rule rests on **structure**, not on case-by-case examples: the process has exactly two
   exits and they are mutually exclusive. There is only one place that prints (`out::emit`), and it
   **never returns** (it writes, flushes and exits on the spot), so once something is printed there
   is no next step and the misuse side cannot be reached; the misuse exit writes only to stderr. In
   `crates/zikaron-cli/tests/cli.rs`, `stdout_has_one_writer_and_that_writer_never_returns` checks
   the whole class by scanning the source, and `not_one_path_that_exits_two_writes_a_byte_to_stdout`
   runs every such path.
   One line for people may precede stdout on stderr when an entry-writing verb is refused by the law
   for its body (`E_BODY_FIELD`, exit 1): it names the member the core's body table refused
   (`zikaron::entry::body_fault`, the same table the thirteen steps judge by) and the flags that give
   it, the whole line in the system's language (section 3), the flags part too (one table of both languages,
   `body_flags` in `verbs.rs`): e.g. `mode: missing or malformed; given by --mark and --toolchain (or --file)`,
   or `mode: 缺或不成形,由 --mark 与 --toolchain(或 --file) 给` in Chinese. It is not part of this contract:
   stdout and the exit code are the same with or without it.
5. **Statuses are passed through as they are.** GREEN / PARTIAL / FAIL, COMPLETE / GAPS /
   UNAVAILABLE / BROKEN_CHAIN, KIT_OK / BADGE_OK / PAIRED and every token are bytes written by the
   base layer; the command line does not merge, rewrite or translate them.
6. **`--now` injection.** Deadlines and windows are judged only by chain time and the injected now;
   without `--now` nothing is injected.
7. **One exception: `mcp`.** Rules 1, 2 and 4 hold for every verb but `mcp`. `zikaron mcp` speaks the Model Context Protocol on stdin and stdout: for as long as its input stays open it writes one message per line, each line one canonical JSON value (law §3.4) followed by a line end. Its one writer is `out::frame`, which returns so the next message can follow; every other verb still prints once through `out::emit`, which never returns. `mcp` given any flag is misuse and writes zero bytes, as rule 4 says. Section 13 is the protocol.

## 2 · Exit codes

| Code | Name | Meaning | stdout |
|---|---|---|---|
| 0 | Affirmed | Answered, and the answer is affirmative | one canonical JSON value |
| 1 | Denied | Answered, and the answer is negative: the law refused these bytes, or the verdict is FAIL / BROKEN_CHAIN / NO_LABEL | one canonical JSON value |
| 2 | Misuse | Misuse: malformed arguments, an unreadable path, a flag the verb does not know | **zero bytes** |
| 3 | Partial | Answered, but neither affirmative nor negative: PARTIAL / GAPS / UNAVAILABLE | one canonical JSON value |
| 4 | Unanswered | **Could not answer**: endpoint unreachable, readings disagree, scan refused, no randomness available | one canonical JSON value |

The whole weight of this table is that 3 and 4 stand on their own:

- Folding PARTIAL into 0 would let a buyer treat an unanchored grant as a green light; folding it
  into 1 would read "not known yet" as "false".
- Folding "endpoint unreachable" into 1 would read a network failure as "this chain is fake". Law
  §9.4: **a failed scan is the absence of an answer, never an answer of absence.**

### Law labels to codes

| Label (law §8.7) | Code |
|---|---|
| COMPLETE | 0 |
| GAPS | 3 |
| UNAVAILABLE | 3 |
| BROKEN_CHAIN | 1 |
| NO_LABEL | 1 |

### Kit verdicts to codes

| Verdict (kit law §10) | Code |
|---|---|
| GREEN | 0 |
| PARTIAL | 3 |
| FAIL | 1 |

## 3 · Misuse

Exit 2, zero bytes on stdout. **The first line of stderr** has the form `<REASON> <subject>`, where
the subject runs to the end of the line (so a subject containing spaces is still one field). The
subject is the thing at fault and nothing else, and it never carries a word the person typed: only names
from the command line's own tables are said as they are, a flag of the flag table (section 8) as `--name`
(`--seq --prev` when the fault is between two). Every other word (a value, a path, a flag not in the
table, a flag written with its value as `--name=value`, a word standing where a flag belongs, a verb not in
the table, an argument that is not text) is named by its place among the arguments (the verb is `#1`)
and its length, as `#3 (12 bytes)`; a path read inside a folder the person named, by that folder's
argument; a word read from a file the person named, by its length alone, as `(12 bytes)`. A secret given
in any spelling therefore never lands in a terminal or a log. What it means for people is
the second line, worded in one table in the code (`out::Said`), in the system's language: Chinese when the
environment's locale (the first of `LC_ALL`, `LC_MESSAGES`, `LANG`, `LANGUAGE` that is set) is `zh…`,
English otherwise and when none is set. The lines after the first are not part of the contract; the first
line, stdout and the exit code are the same in either language.

`zikaron badge --decode /nowhere/at/all`:

```
E_UNREADABLE #3 (15 bytes)
读不出
```

```
E_UNREADABLE #3 (15 bytes)
cannot be read
```

**A sealed ledger is read with the credential, as a home's ledger only.** Every local file ZIKARON Desk keeps,
ledger entries included, is sealed (it starts with `zikaron-local/2`; `zikaron_glue::sealed`), and the command
line holds no passcode. When `--ledger` names the ledger folder of a home the desktop keeps (its `settings` room
holds the home's label), every verb that reads it opens it with this machine's credential, the way the writing
verbs write it (section 12): `audit`, `check-grant`, `chain-check`, `depth`, `show --entry`, `kit-export` and
`scan --ledger`. A ledger folder that holds a sealed file but is no home's (a copy taken out of its home) is not
read: the verb stops as misuse, exit 2 and zero bytes on stdout:

```
E_UNREADABLE <the ledger path's place and length, as #3 (12 bytes)>
已锁定:这是 ZIKARON Desk 封存的本机数据;命令行只凭凭据读桌面所管之家的账本
```

```
E_UNREADABLE <the ledger path's place and length, as #3 (12 bytes)>
Locked: this is local data ZIKARON Desk keeps sealed; the command line reads it only as the ledger of a home the desktop keeps, with the credential
```

The command line works on plain ledger folders, on the ledgers of the homes the desktop keeps (with the
credential), and on the mirror bundles and record packages the app exports.

**A sealed home is written with the credential.** The entry-writing verbs write a home ZIKARON Desk keeps with
the command line's credential while "With a credential" is on in the desktop (Settings > Command line and AI control); section 12 says how,
and when they are refused.

The base's HARNESS leaves misuse diagnostics to the implementation and only fixes the exit code and
the empty stdout. Putting "which input could not be read" on the first line of stderr, in a fixed
form, keeps the one-bit signal on stdout intact: if the subject were written to stdout (even as a
valid JSON refusal), "bytes mean an answer" would no longer hold, and the caller would have to parse
before knowing whether it got an answer or a misuse.

## 4 · Refusal reasons (the command line's own)

Tokens from the base layer are forwarded unchanged, never renamed; this table covers only "what
happened on the shell's side".

| token | Meaning | Usual code |
|---|---|---|
| `E_ARGS` | Malformed arguments | 2 |
| `E_UNREADABLE` | A file could not be read, or the ledger is ZIKARON Desk's sealed local data (section 3); exit 1 when, opening a home the desktop keeps or reading this machine's lists (section 12), the sealed layer refuses for a cause that has no reason of its own in this table (a sealed file that does not open, a list or a register that does not read): `token` is its code in capitals, `detail` its evidence; exit 1 too when a record package sealed to one recipient does not open for a disk or randomness failure (`token` `IO`, `detail` the system's words) | 2 / 1 |
| `E_KEY` | The private key is not a scalar in [1, n−1] | 2 |
| `E_RANDOM` | The machine could not provide randomness | 4 |
| `E_LEDGER` | The storage layer (`zikaron-store`) refused (with its code and details) | 1 |
| `E_ENTRY` | The core (`zikaron`) refused this entry (with its token) | 1 |
| `E_TIP_ABSENT` | This lineage has no entries at all | 1 |
| `E_ENTRY_ABSENT` | A named entry was not found in the store (not the same as "the lineage is empty") | 1 |
| `E_TIP_FORKED` | More than one entry at the highest seq: the ledger has forked, and the command line does not pick one; on the `init` side, more than one distinct identity at seq 0 (several roots already exist); on any writing verb, a ledger that holds two roots (or would, with this entry) and no `--root` naming the one to write under, `--seq` / `--prev` given by hand included; on `hand-over`, a ledger without a single root or whose entries are not one line | 1 |
| `E_SCAN` | The anchoring layer (`zikaron-anchor`) refused to answer the scan (with its code) | 4 |
| `E_ENDPOINTS_DISAGREE` | Several endpoints returned readings that disagree | 4 |
| `E_UNREACHABLE` | The endpoint was unreachable, the send failed, or the wait timed out; for `anchor` sending directly (`--key`), also every failure before the broadcast that is not a refused estimate (section 6); for `anchor` on every road, a receipt question no node answered within the wait (with `tx`); for `anchor` sending a home's queue (`--home`, `--ledger`), also a broadcast no node answered after the batch was recorded as in flight: it is out, not refused (with `tx`, the batch stays in flight and is asked about by its hash). When the endpoint went unreached because its proxy asked who is calling, `token` says how: `PROXY_CREDENTIALS_MISSING` (no credentials were given with it) or `PROXY_CREDENTIALS_REFUSED` (the ones given were refused); the line on stderr says how `--proxy` carries them (that line also comes with every other exit-4 answer for the same cause, such as `E_SCAN`, whose `token` stays its own). When the connection followed the system, went without its automatic configuration script and was not made, the line on stderr says the script's trouble and the way out instead: remote http, `Proxy script is on remote http, not used: give a proxy with --proxy`; not fetched, `Can't fetch the proxy script: start your proxy app, or give a proxy with --proxy`; any other, `Proxy script failed (<code>): give a proxy with --proxy` with its `PAC_*` code (in Chinese per section 3); stdout is the same with or without it | 4 |
| `E_TX_STATUS` | The transaction was included in a block but its status is not 1 (a home's batch stays in its queue, recorded as reverted) | 1 |
| `E_TX_NOT_YET` | The wait ran out and the transaction is not in a block yet (a home's batch stays in flight: the desktop asks about it again, and so does an `anchor --ledger` that finds nothing else to send; there, with "Send automatically" on, a batch no node holds whose nonce is still unused is signed again at that very nonce and sent, and answered as a send is, section 12) | 4 |
| `E_TX_VOID` | What was sent will never be included: no node holds it and its nonce was used by another transaction; nothing new was sent by this answer (`tx` names it). On every road alike: the direct `anchor` on a rerun, and a home's batch through the desktop or with the credential (its entries are back in the queue) | 1 |
| `E_DOC` | The kit layer (`zikaron-kit`) refused this document, or the pairing did not match | 1 |
| `E_KIT` | The kit layer judged this kit invalid (with the kit law verdict), or a record package in one file does not read as one (`state` the layout's code) | 1 |
| `E_BADGE` | The kit layer refused to encode or decode the badge | 1 |
| `E_FRAGMENT` | The fragment lacks one of anchors / basis / evidence and cannot be assembled into an audit input | 1 |
| `E_ALREADY_ROOTED` | `init`: this ledger already has a root (the core recognises exactly one seq 0) | 1 |
| `E_NOT_EMPTY` | `init`: the ledger is not empty, and the core cannot recognise a root in it (unreadable stray files, bytes whose name fits an entry but that the core refuses, entries with no root) | 1 |
| `E_RETRACTION` | `retract`: the production rules of the deletion convention refused this entry, with the convention's `token` (`E_RETRACT_SHAPE` subject missing or not hex32, `E_RETRACT_NOT_IN_LEDGER` not on this ledger's lineage, `E_RETRACT_NOT_A_WORK` not a `history`, `E_RETRACT_REPEATED` already retracted); the ledger is left untouched | 1 |
| `E_WOULD_BREAK` | Every entry-writing verb (`init` and `retract` included): the core's offline audit of this ledger with the entry about to be written finds a chain finding of law §8.3 the ledger did not already have (a key that no longer holds the seat, a sequence gap, a broken link, an equivocation), whether hard or not; `entryId` is the entry that would have been written and `names` the findings' names; not one byte is written | 1 |
| `E_GRANT_FILE` | `check-grant`: `--grant` is a grant file (the app's single-file bundle) that does not open by the reading the app uses, with the refusing layer's `token` (the bundle shape's code, the kit law verdict, `E_GRANT_FILE_CODE` no grant code in it, a badge token, or `E_GRANT_FILE_CHAIN` with `entryId` for a hop the code names that the bundle does not carry) and `detail?`; nothing is checked | 1 |
| `E_GAS_REFUSED` | `anchor`: the endpoint refused the gas estimate for a reason about the call itself (by one table, `said::refuses_the_call`: a member about the call, or the node's JSON-RPC error with a numeric `code` the table does not name; taken as "the call would revert", the node's words in `detail`), or estimated it above the ceiling of 200,000 (it would run out of gas with the fee paid); a refusal for the endpoint's own reasons (a rate limit, a missing method, credentials, a wrong chain, an error without a numeric `code`, a page at an HTTP status) says nothing about the call and is `E_UNREACHABLE` (exit 4) instead; nothing is broadcast either way | 1 |
| `E_DESKTOP` | `--home` (section 11): the desktop gave no answer for that home; `detail` says why, one of `NOT_OPEN` (no door for that home: the desktop is locked, does not have that home open as its writer, or is not running), `CLOSED` (it locked, closed the home or quit before it answered), `PATH_TOO_LONG` (the door's place is longer than this system's local channel takes), `NOT_YOU` (the door is kept by another user), `BROKEN` (it broke off, or answered in a form this version does not read), `NO_MACHINE` (this user's machine folder cannot be found), `ALONE` (no door because "With a credential" is on, section 12: the desktop keeps its door shut while that switch is on; write without `--home`) | 4 |
| `E_ON_DESKTOP` | `--home`: the action is done on the desktop alone, never through its door (`attest` asks for the passcode; `succeed` is done there by hand, and the desktop is not asked); nothing was done | 1 |
| `E_QUEUED` | `anchor` sending a home's queue, through the desktop (`--home`) or with the credential (`--ledger`), while this machine's setting "putting on chain from the command line" leaves sending to the user: nothing was sent; `count` entries wait in the queue for the user to send them from the desktop | 3 |
| `E_DESKTOP_REFUSED` | `--home`: the desktop's action layer refused; `token` is its code (the desktop's closed table of refusals) and `detail` its evidence; exit 4 when the refusal is no answer (a network refusal, or `ADDRESS_CHAIN_UNREAD`: an address's key looked for and some network not read through), 1 otherwise. `anchor` answers `E_SEND_REFUSED` instead | 1 / 4 |
| `E_SEND_REFUSED` | `anchor` sending a home's queue, through the desktop (`--home`) or with the credential (`--ledger`): refused before anything went out, by the one sender both roads send through; `token` is its code as the desktop's closed table spells it (an empty queue `QUEUE_EMPTY`, a balance short `INSUFFICIENT_FUNDS`, the home's write gates `BROKEN`, `HANDED_OFF`, `LEDGER_NOT_FETCHED`, `NEWER_ELSEWHERE`, the nodes' own refusals and the network's), `detail` its evidence (through the desktop in its words, with the credential in the command line's: nodes, numbers in wei); exit 4 when the refusal is the network's absence of an answer, 1 otherwise | 1 / 4 |
| `E_CREDENTIAL_EXPIRED` | Opening a sealed home to write, read or send its queue (sections 3 and 12): the command line's credential has expired; it was removed, and nothing was written | 1 |
| `E_CREDENTIAL_MISSING` | Opening a sealed home: this machine has no command-line credential ("With a credential" is off on the desktop); nothing was written | 1 |
| `E_CREDENTIAL_SHAPE` | Opening a sealed home: the credential is not in its shape, does not open, or holds another master key than this machine's vault (issued before a new master key); nothing was written | 1 |
| `E_HOME_HELD` | Writing a sealed home or sending its queue (section 12): the desktop is writing it at this moment (it, or another writer on this machine, holds the home's writer lock); nothing was written; try again shortly. For `anchor --ledger` also when the lock could not be taken back within sixty seconds to settle a receipt: the batch stays in flight, and the desktop settles it, or an `anchor --ledger` that finds nothing else to send | 1 |
| `E_HOME_OTHER_MACHINE` | Opening a sealed home: the home's writer mark names another machine, or does not read; nothing was written | 1 |
| `E_ADOPTED` | Writing, handing over or taking a handoff into a plain ledger folder that ZIKARON Desk on this machine has adopted (section 12): it is written through the desktop's sealed copy from then on; nothing was written | 1 |
| `E_RECORDS` | `kit-verify`: not every record of the package holds; `records` says each one's `original` and `anchored`: an original that is not the one recorded is exit 1, otherwise (a record not anchored, the chain not read, an original missing) exit 3 | 1 / 3 |
| `E_EXTRACT` | `kit-verify --originals-to`: the originals were not taken out, and nothing was written in the folder; `token` says why: `EXISTS` (the folder holds items with these names already, `names`), `LINK` (a link stands at a name in the folder, never followed; `detail`), `CLASH` (two originals would take one name, in any case, `names`), `NOTHING` (no original in the package matches its record), `NOT_A_FOLDER` (the place given is not a folder that is there), `PATH` (a package path is not a kit path), `CHANGED` (an original laid out does not match its record: the package changed meanwhile), `LANDING` (a disk operation failed and what was placed was taken back; `detail` the system's words). A package that does not pass the kit law when read again is `E_KIT` as above | 1 |
| `E_ADDRESSED` | A record package sealed to one recipient (section 6), refused; `token` is the code of the sealed package's one table (`zikaron_seal::addressed::Why`), spelled as the desktop spells it, and `detail` its evidence (never a recipient): opening one, `ADDRESSED_NOT_RECIPIENT` (no key offered opens it), `ADDRESSED_BROKEN` (a segment after the first does not pass: changed, cut short, reordered, added to), `ADDRESSED_HEAD` (its head does not read); sealing one to an address, `ADDRESS_NOT_ANCHORED` (every node of every network read through, and no anchor the address signed) or `ADDRESS_CHAIN_UNREAD` (exit 4: no key found and some network not read through, each one's words in `detail`); read by a verb that reads packages but does not open them, `ADDRESSED_ELSEWHERE`. The line for people is the table's words (for `ADDRESSED_ELSEWHERE`, where the command line opens it). Nothing was written | 1 / 4 |
| `E_HANDED_OFF` | Writing a ledger (a plain folder or a home) this machine handed off to another place and has not taken back (section 12); `handoff` names the handoff; nothing was written | 1 |
| `E_READ_ONLY` | Writing a home the desktop keeps, or handing it over (`hand-over`), with the credential (section 12): the home is read-only, as the desktop holds it; `token` is the code as the desktop's closed table spells it: `LEDGER_NOT_FETCHED` (an imported identity's home, its ledger not fetched and checked against the chain yet), `NEWER_ELSEWHERE` (the chain holds anchors this ledger lacks; `detail` how many), `BROKEN` (the ledger's own entries break its chain; `detail` the hard findings' names); nothing was written | 1 |
| `E_HANDOFF` | `take-over`: the handoff is not taken (section 12); `token` says why: the layout's or the kit law's code (the file does not read as a handoff), `E_HANDOFF_NOTE` (it carries no handoff note), `NOT_BACK` (this place handed the ledger off and this is not the handoff after its own; `handoff` names its own), `AHEAD` (this place holds an entry the handoff lacks, `entryId`), `NOT_ONE_LINE` (the handoff's ledger has no single root, or two entries at one place, or a link to the wrong entry); nothing was written | 1 |
| `E_ON_CHAIN` | `anchor`: the chain already holds an anchor of a hash given, from this key through this registry (anchored on another side); `names` the hashes, `tx` and `blockNumber` the earliest of those anchors; nothing is sent (section 6). On every road alike: directly, through the desktop (`--home`), with the credential (`--ledger`). A home's entries the chain holds then leave its queue by the reading every node agrees on (the desktop's self-audit, and with the credential the same run), so the next run sends the rest | 1 |

## 5 · The two families of answers

**Shell answers**: the command line's own byte form, starting with `{"ok":…}`. For an affirmative
answer `ok` is true; for the negative, partial and unanswered cases `ok` is false and `reason` is
present (`ok` answers "is this an affirmative answer"; the three other states are told apart by the
exit code).

**Base answers**: the object written by `zikaron` / `zikaron-kit`, **passed through byte for byte**,
not wrapped and with no keys changed. The command line only chooses the exit code. Because the
command line makes no judgements and invents no byte forms, the byte forms of the audit report, the
six-check verdict, the chain check and the depth reading always belong to `zikaron` /
`zikaron-kit`, and the command line never touches them.

The four verbs that give base answers are `audit`, `check-grant`, `chain-check` and `depth`; in
addition, `scan` passes through the core's no-label answer when the basis is malformed.

## 6 · Per verb

> Notation: `?` marks a member that is present only under some conditions. Every shell answer also
> has `ok` (true for an affirmative answer), and negative ones also have `reason`.

| Verb | Code | Family | stdout members |
|---|---|---|---|
| `keygen` | 0 | shell | `address`, `privkey` |
| | 4 | shell | `reason=E_RANDOM` |
| `init` | 0 | shell | `entryId`, `ledger`, `seq`, `written` |
| | 1 | shell | `E_ENTRY` + `token`; or `E_LEDGER` + `detail`, `names?`, `count?`; or `E_ALREADY_ROOTED` + `author`, `count`; or `E_TIP_FORKED` + `count`; or `E_NOT_EMPTY` + `count`, `names?`; or `E_WOULD_BREAK` + `entryId`, `names` |
| `history` | 0 | shell | `entryId`, `ledger`, `seq`, `written` |
| | 1 | shell | `E_ENTRY` + `token`; or `E_LEDGER` + `detail`, `names?`, `count?`; or `E_WOULD_BREAK` + `entryId`, `names`; or `E_TIP_ABSENT`, `E_TIP_FORKED`; or `E_HANDED_OFF` + `handoff`; on a plain folder, `E_ADOPTED`; on a home, the refusals of section 12 |
| `grant` | same as `history` | | |
| `revoke` | same as `history` | | |
| `adopt` | same as `history` | | |
| `succeed` | same as `history` | | |
| `annotate` | same as `history` | | |
| `retract` | 0 | shell | `entryId`, `ledger`, `seq`, `written` |
| | 1 | shell | `E_RETRACTION` + `token`; or `E_ENTRY` + `token`; or `E_LEDGER` + `detail`, `names?`, `count?`; or `E_WOULD_BREAK` + `entryId`, `names`; or `E_TIP_ABSENT`, `E_TIP_FORKED`; or the refusals `history` gives for a handed-off ledger, an adopted folder and a home |
| `attest` | 0 | shell | `attestation`, `attestor` |
| | 1 | shell | `E_ENTRY` + `token` |
| `anchor` | 0 | shell | `blockNumber`, `tx` |
| | 1 | shell | `E_TX_STATUS` + `state` (the on-chain status), `tx`; or `E_TX_VOID` + `tx` (on a rerun: what was sent will never be included; nothing sent); or `E_ON_CHAIN` + `names` (the hashes the chain already holds from this key), `tx`, `blockNumber` (the earliest of those anchors), nothing sent; or `E_GAS_REFUSED` + `detail` (the endpoint's refusal in its debug form, e.g. `Node("…")`, or `<estimate> > 200000`), nothing broadcast |
| | 3 | shell | with `--ledger` or `--home`: `E_QUEUED` + `count` (the switch leaves sending to the user; nothing sent) |
| | 1 / 4 | shell | with `--ledger` or `--home`: `E_SEND_REFUSED` + `token`, `detail` (refused before anything went out; 4 for the network's absence of an answer); with `--ledger`, the refusals of opening a home (section 12) |
| | 4 | shell | `E_TX_NOT_YET` + `count` (seconds waited), `tx`; or `E_UNREACHABLE` + `detail`, `tx` (signed and its hash kept: the broadcast failed, the endpoint refused it or its echo was not the hash signed, or the endpoint never answered the receipt question within the wait, or, on a rerun, where an earlier one stands could not be read); or `E_UNREACHABLE` + `detail` only, nothing broadcast: the record of earlier sends could not be placed or read, the chain's earlier anchors (`E_ON_CHAIN`'s question), the head or the estimate went unanswered or came back in another shape, the nonce could not be read, or the hash could not be kept |
| `scan` | 0 | shell | `fragment`, `singleSource`, `singleSourceChains`, `sources` |
| | 1 | base | the core's no-label answer (malformed basis) |
| | 1 | shell | with `--ledger`, the refusals of reading a home (section 12) |
| | 4 | shell | `E_ENDPOINTS_DISAGREE` + `detail`, `sources`; or `E_SCAN` + `detail`, `token` (`E_NO_BASIS`: the home's settings name no chain or no registry); or, with `--ledger`, `E_UNREACHABLE` + `detail` when no endpoint of the home's chain answers its head |
| `audit` | 0 / 3 / 1 | base | the core's fifteen-item, sixteen-member report, or no label |
| | 1 | shell | what `--ledger` names does not read: `E_LEDGER` + `detail`, `names?`, `count?` (a ledger folder the storage layer refuses, or a mirror bundle whose manifest is not `desk-mirror` version 1, `detail` its `kind/version`); `E_KIT` + `state`, `detail?` (a record package the kit law judges invalid, or one in one file that does not read as one, `state` the layout's code); a record package sealed to one recipient, `E_ADDRESSED` + `token` `ADDRESSED_ELSEWHERE`, `detail` (empty); a home's ledger, the refusals of reading a home (section 12); without `--root`, `E_TIP_ABSENT` (no root in what was read) or `E_TIP_FORKED` (more than one root); `E_FRAGMENT` (the fragment cannot be assembled into an audit input); with `--out`, `E_LEDGER` + `detail` (`E_OCCUPIED` / `E_IO`), `path` when the input cannot land |
| `check-grant` | 0 / 3 / 1 | base | the kit layer's six-check verdict (`verdict`, `checks`, `failed`, `basis`) |
| | 1 | shell | `E_GRANT_FILE` + `token`, `detail?`, `entryId?` (a grant file that does not open); with `--ledger` or `--input`, the shell refusals of `audit` before `--out` |
| `chain-check` | 0 / 3 / 1 | base | the kit layer's chain check (`verdict`, `hops`, `links`, `token`, `failing`) |
| | 1 | shell | with `--ledger` or `--input`, the shell refusals of `audit` before `--out` |
| `depth` | 0 | base | the kit layer's depth reading |
| | 1 | shell | with `--ledger` or `--input`, the shell refusals of `audit` before `--out` |
| `fpm-sign` | 0 | shell | `doc`, `docId`, `path?` |
| | 1 | shell | `E_DOC` + `token`, `index` (`index` may be null); or `E_ENTRY` + `token` when signing fails; or, when the file cannot be written, `E_LEDGER` + `detail` (`E_OCCUPIED` / `E_IO`), `path` |
| `ack-sign` | 0 | shell | `doc`, `docId`, `path?`, `state?` (the pairing verdict when `--fpm-doc` is given) |
| | 1 | shell | `E_DOC` + `token`, `index` (`index` may be null); `E_ENTRY` + `token` when signing fails; when the pairing does not match, `E_DOC` + `docId`, `doc`, `state`; when the file cannot be written, `E_LEDGER` + `detail`, `path` |
| `badge` | 0 | shell | encode: `payload`, `state=BADGE_OK`; decode: `count`, `entries`, `state=BADGE_OK` |
| | 1 | shell | `E_BADGE` + `token`, `index` |
| `kit-export` | 0 | shell | `dropped` (platform junk that was dropped, each named), `entries`, `files`, `kitId`, `path`, `proofs`, `state=KIT_OK`; from a home the desktop keeps (`--ledger`, or `--home` through the desktop) also `originals` (the package paths of the originals taken; empty with `--originals none`) and `leftOut` (the places of the chosen records' originals not taken: gone, no longer the bytes recorded, or not there to read) |
| | 1 | shell | `E_KIT` + `state` (the kit law verdict), `detail` (its subject, or null when it has none); `E_LEDGER` + `detail`, `names?`, `count?` (the ledger folder does not open or read); `E_LEDGER` + `path` (the subject of a kit output failure: the `--out` path already exists, a disk operation failed, or a `--file` / `--proof` kit path is malformed or given twice); `E_ENTRY_ABSENT` + `names` (the named entries that were not found); with `--to`, `E_ADDRESSED` + `token` `ADDRESS_NOT_ANCHORED`, `detail`; or, from a home, the refusals of reading a home (section 12) |
| | 4 | shell | with `--to`, `E_ADDRESSED` + `token` `ADDRESS_CHAIN_UNREAD`, `detail` (no key found and some network not read through; nothing written) |
| `kit-verify` | 0 | shell | `entries` (how many the package carries), `kitId`, `records` (one per record: `entryId`, `content`, `original` one of `match` / `mismatch` / `missing`, `anchored` one of `anchored` / `notAnchored` / `unread`, `firstAt?` the first anchoring's block time when anchored); with `--originals-to`, `taken` (the names the originals were taken out under) and `path` (the folder) |
| | 1 / 3 | shell | `E_RECORDS` + the members of exit 0; or `E_KIT` + `state`, `detail?` (the package fails the kit law, or does not read as one file: `state` its code); a package sealed to one recipient that does not open, `E_ADDRESSED` + `token`, `detail`, or `E_UNREADABLE` + `token` `IO`, `detail`; with `--ledger`, the refusals of reading a home (section 12); with `--originals-to`, `E_EXTRACT` + `token`, `names?`, `detail?` |
| `hand-over` | 0 | shell | `entries`, `handoff` (the handoff's id), `path` |
| | 1 | shell | `E_TIP_ABSENT` (no entry), `E_TIP_FORKED` (no single root, or not one line); `E_ADOPTED`; the refusals of section 12 (a home), `E_READ_ONLY` + `token`, `detail` among them (the home's write gates); `E_LEDGER` + `detail`, `names?`, `count?` (the folder does not open or read); `E_LEDGER` + `detail`, `path` (the file does not land: `E_OCCUPIED`, `E_IO`) |
| | 4 | shell | `E_RANDOM` (no randomness for the handoff's id) |
| `take-over` | 0 | shell | `count` (entries written here), `entries` (how many the handoff carries), `handoff`, `ledger` |
| | 1 | shell | `E_HANDOFF` + `token`, `handoff?`, `entryId?`, `detail?`; `E_ADOPTED`; the refusals of section 12 (a home); `E_LEDGER` + `detail`, `names?`, `count?` (the folder does not open or take an entry) |
| `show` | 0 | shell | `author`, `entryId`, `entryType`, `prev`, `seq`, `value` |
| | 1 | shell | `E_ENTRY` + `token`; with `--entry` on a ledger folder, `E_LEDGER` + `detail`, `names?`, `count?` when the folder or the named entry file does not read; with `--entry` on a mirror bundle or record package, `E_ENTRY_ABSENT` + `entryId` when no entry there has that id, or the `--ledger` refusals of `audit` |
| `contract` | 0 | shell | `exits`, `ok`, `reasons`, `standsFor`, `verbs` (section 10) |
| `mcp` | 0 | protocol | no one value: one message per line while its input is open (section 13); exit 0 once it closes |

**`init` only writes to an empty ledger: one ledger, one root.** Genesis is the first entry. If the
ledger already holds anything (entries, unreadable stray files, bytes whose name fits an entry but
that the core refuses), `init` exits 1 and writes no entry file: if the core recognises exactly one
seq 0 entry it answers `E_ALREADY_ROOTED` (with that entry's author); if it recognises several
distinct identities it answers `E_TIP_FORKED` (two entries signed by the same key are two roots; the
same entry stored as two files with different names is still one root); otherwise it answers
`E_NOT_EMPTY`.

**Every entry is audited before it is written.** All entry-writing verbs land through one gate: the
entry is built and signed, then the core audits this ledger with it offline, and a chain finding of
law §8.3 that the ledger did not already have, hard or not, refuses the write by name
(`E_WOULD_BREAK`), not one byte written. The gate stands after the tip and the root are asked, and
`--seq` / `--prev` given by hand pass through it like the computed tip: after a `succeed`, the key that
handed the seat over can no longer write; a sequence gap, a second entry at one place and a link to
the wrong entry are refused; a ledger that already has a gap takes the next entry from the key that
holds the seat. The audit runs under the root the writer names (`--root`, the one its tip is asked under);
without it, a ledger that holds two roots, or would with this entry, cannot be audited under one and the
write is refused `E_TIP_FORKED`, not one byte written. This narrows what was written before: a write by
hand (`--seq` / `--prev`) into a ledger already holding two roots used to land unaudited.

**What the reading verbs read.** `audit`, and through it `check-grant`, `chain-check` and `depth`,
and `show --entry` read what `--ledger` names in one of four shapes: a ledger folder (as the store
reads it, strictly); the ledger of a **home the desktop keeps**, opened with this machine's credential
(section 12); a **mirror bundle** the app exports (its `mirror.json` names each entry in the
`entries/` room; the layout is named once in `zikaron_glue::mirror`, which the app writes by); or a
**record package**, a folder or one file (a disclosure kit, verified by the kit law first, its `entries/` room
read; one file is read out item by item first, with no bound on its size). Whether each entry is an entry is the
core's, in the audit. The writing verbs and `init` read ledger folders and homes only. `check-grant` over a home's
ledger reads the issuer's whole ledger there, so whether the grant is revoked is answered.

**Record packages are one file unless asked otherwise.** `kit-export` writes the package as one file at `--out`
(`--as file`, the default), the kit laid out, verified and then streamed into the file
(`zikaron_glue::container`: the header `zikaron-kit-file/1`, then each item as its path, its byte count and its
bytes, the manifest first, the rest in path order; the same layout a grant file `.zkgrant` has; the desktop names
it `.zkit`), or as a folder (`--as folder`). From a home the desktop keeps, the package takes what the desktop's
export takes: each chosen record's original from this machine's index of signed files, while the file at its place
still has the bytes recorded (named into the package as the desktop names it, the renamed ones listed in
`files/zikaron-names.json`), the terms documents kept when its chosen grants were signed, and the home's anchoring point as the note's last line.

**A record package can be sealed to one recipient.** `kit-export` given `--to` (an address) or `--to-key` (a public key,
`0x` and 66 or 130 hex digits, a point of the curve) writes the one file sealed so that only that key opens it
(`zikaron_seal::addressed`, the desktop's own: the package laid out in a folder of its own in the system's temporary place,
never beside `--out`, then sealed into `--out` the one way files land, and the plain copy removed). Its head holds the
format's mark, its version and a one-time public key, and nothing of the recipient; it is opened only to be verified
(`kit-verify`), and every other verb that reads packages refuses it by name (`E_ADDRESSED`, `ADDRESSED_ELSEWHERE`). The
answer is the plain package's, as without them. `--to` and `--to-key` together, or either with `--as folder` (only one
file is sealed), an address not in its form or a key off the curve, is misuse (exit 2, zero bytes), on every road.
An address's key is looked for on chain by the one judgement the desktop takes (`zikaron_send::recipient::key_for`):
its registry anchor logs, the transactions behind them, the key recovered from a signature that address itself made
(an anchor sent through a relay or a contract is passed over). Found anywhere, it proves itself (its address is the one
given); every node of every network read through with no such anchor is `E_ADDRESSED` `ADDRESS_NOT_ANCHORED` (exit 1);
otherwise `ADDRESS_CHAIN_UNREAD` (exit 4: the absence of an answer, never an answer of absence). Where it is looked
for: on a plain ledger folder, the networks `--endpoint` gives (repeatable; each chain one the known deployments table
names, its registry and start block from there; `--endpoint` missing, or a chain the table does not name, is misuse),
the way out by `--proxy`; on a home's ledger (with the credential, section 12), the home's network and nodes and this
machine's read-only networks, each read only past its fingerprint gate (its nodes give the pinned registry build's
code, and it is read on those nodes alone; a node answering no code there, one behind the block the registry was
deployed in, does not count; a node giving other code makes it a network not read), this machine's proxy setting
unless `--proxy` is given (section 8, `--proxy`), and no `--endpoint` (misuse). A read-only network that is the home's
own (its chain and registry) is that network: past its gate its nodes join the home's, turned away it names nothing.
A home with no chain set has no network of its own to read; with no network at all nothing is read, and that is
`ADDRESS_CHAIN_UNREAD`, never "not anchored". `--endpoint` and `--proxy` are taken only with `--to`. Through the desktop (`--home`, section 11) the desktop
looks on its own networks. No refusal leaves a byte at `--out`.

**A sealed record package is opened to be verified.** `kit-verify` given such a package opens it with the first key
offered that is its recipient, into a plain one-file package of its own in the system's temporary place (removed when
the verb ends), then verifies that package and takes its originals out exactly as a plain one (below). The keys offered:
with `--ledger` naming a home the desktop keeps, every identity on this machine (each seat of each identity in the
register, opened with the credential, section 12; `--key` and `--key-file` are then misuse, and a `--ledger` that is no
home's is misuse); otherwise the key given (`--key` or `--key-file`); none of these is misuse naming them
(`E_ARGS --key --ledger --home`). Through the desktop (`--home`) the desktop tries its own identities. No key opening it
is `E_ADDRESSED` `ADDRESSED_NOT_RECIPIENT`; a segment that fails is `ADDRESSED_BROKEN`; nothing opened is kept either way.
A plain package takes no key and no home, and a key or a home's `--ledger` given with one is not used. The chain is read
as for any package (`--fragment`), on every road but the desktop's, which reads its own networks: the same package can
read `unread` here and `anchored` through the desktop.

**`kit-verify` judges each record as the desktop's verification does** (`zikaron_glue::recordcheck`, the one
judge both call): given a package (`--kit`, a folder or one file) that passes the kit law, each record (`history`
entry) in it is answered: whether the package's file for it is the original it records (its sha256 is the record's
`content`), and whether an anchor counted on the chain read reaches it, with the earliest such block time. The
chain is read as the reading verbs read it: `--fragment` is a `scan`'s reading (and `--unavailable` its fourth
member); without it every record is `unread`, never "not anchored". Given `--originals-to` (a folder that is there), the originals
that match their records are then taken out into it by the one way the desktop takes them (`zikaron_glue::extract`):
the package read again and judged; a file original by its bytes, a signed directory by its manifest recomputed from the
files kept under its name with the names it was signed with (`zikaron_glue::recording`, the way signing computes it,
which is also how `original` judges a directory: paths joined by `/` on every system, or by `\` as a directory signed on
Windows by earlier versions has them); each under the name it had (the package's name table) when that is
one name every system takes, its package name otherwise; laid out in a staging folder inside the chosen one, checked,
then placed without replacing anything; any name already there, a link, or any step failing leaves the folder as it
was (`E_EXTRACT`). Taken, the answer is the records' answer with `taken` (the names, in order) and `path`. No home
and no key is asked.

**`scan --ledger` declares the home's basis.** In place of `--basis`, `--ledger` names a home's ledger; the basis
is the one the desktop's self-audit declares: the chain, the registry and the start block from the home's settings,
the senders of its ledger, and the chain's head, the lowest its nodes say, as the last block. `--basis` and
`--ledger` together, or neither, is misuse; `--ledger` naming a folder that is no home's is misuse (`--ledger`).
With neither `--endpoint` nor `--fixture`, a home's `--ledger` also gives the nodes: those its settings list for
that chain, read with the credential as `anchor --ledger` reads them (section 12), and, without `--proxy`, this
machine's proxy setting as a send takes it (section 8, `--proxy`); the credential's refusals are as for any read of the home (section 12); no
node set for that chain is `E_UNREACHABLE` (`detail` `NO_ENDPOINT <chain id>`), no chain set `E_SCAN` with `token`
`E_NO_BASIS`; the nodes are never said. A plain folder with neither is misuse as before (`--fixture --endpoint`).

**`check-grant` takes a grant file as it takes a grant entry.** When `--grant` starts as the app's
single-file bundle, it is opened by the same reading the app uses (`zikaron_glue::grantfile`: the
bundle shape, the kit law over the kit inside, the grant code, and every hop the code names carried in
the bundle), and the grant checked is the chain's last hop, the grant the file is for. Entry bytes go to
the six checks unchanged, as before.

**`audit --out`** lands the audit input this run assembled (contract 11, its six members, in canonical
bytes: the shape `zka audit-input` prints) at that path before the core reads it, whatever the label,
so a cross-ledger `chain-check` can take it as a hop's input file (`--hop <grant>=<input>`).

**How `scan` asks.** Each window's log question names the window's senders (as the second topic, all in one
"any of" list, one question per range); a window with no sender asks for no logs. Every log that comes back is
still judged as before, so a node that answers more changes nothing. A refused log question is asked again as
it was, three times in all, before the range is split at the limit the node names or in half. A recording made
before log questions named senders still answers them: a question naming senders that the recording lacks gets
the recorded answer to the same question without them, unfiltered. The command line keeps no record of facts
checked before; every scan asks about every log.

**`scan --adoptions`** names a JSON file whose top level is an object with an `adoptions` array; each
element is an object with `chainId` (an integer) and `tx` (hex32), the adoption transactions to read
evidence for. An element without both is misuse.

**`scan --fixture` reads the recording's own basis and adoptions.** A recording carries the basis and the
adoptions it was made with, and the fixture path uses those. `--basis`, `--adoptions`, `--proxy` or `--ledger` given
together with `--fixture` (once or more) is misuse (`E_ARGS`, the flag named); they are read only on the
`--endpoint` path.

**Endpoints.** `--endpoint` urls are `http://` or `https://`, the scheme and host read without regard
to case (`HTTPS://Node.Example` is `https://node.example`); https verifies the certificate chain and
host name with the roots compiled into the binary, the same transport the app uses. `anchor` waits for
the receipt round after round until `--wait-secs`, pausing between rounds; a round in which the
endpoint refuses is not an answer, and once the transaction is signed and its hash kept, `E_UNREACHABLE` carries `tx`: a broadcast that fails, that the
endpoint refuses or whose echo is not the hash signed (the bytes may be in a pool, and a rerun asks about them
first), and an endpoint that never answered the receipt question within the wait. Before that, `E_UNREACHABLE`
(without `tx`, nothing broadcast) covers every failure that is not a refused estimate: the head or the estimate
unanswered or in another shape, a nonce that does not read, and a hash that could not be kept. `--form registry` without `--registry`, and `--registry` with `--form bare`, are misuse (`E_ARGS
--registry`), judged before any node is asked.

**What `anchor` sends.** With `--calldata` the transaction's data is that calldata as given, and `--hash`
values, if any, are not used; without it the data is built from the `--hash` values (`anchor` / `anchorMany`
for `--form registry`, the words back to back for `--form bare`).

**`retract` only writes deletions of the convention's shape.** A deletion is a reading convention
under the open entry types of law §6.9: the type literal, the body keys (`subject` required,
`note_md` optional), the reading and the production rules live in `zikaron_glue::retraction`, and
the app and the command line both call that one place. Before writing, the production rules are
checked: the subject is hex32, it is a `history` on this ledger's lineage, and it has not been
retracted already; otherwise the command exits 1 with `E_RETRACTION`, whose `token` is one of the
convention's four, and no entry file is written. Once the production rules pass, the entry takes the
same path as `annotate`: it is put in its envelope and signed, passes the core's thirteen-step
check, and is written to the ledger. The audit lists it under `UNKNOWN_TYPE`, and the label does not
change because of it.

**`history --file` follows the record convention.** The convention lives in one place,
`zikaron_glue::recording`, which the app's anchoring desk reads too: `content` is the SHA-256 of the
file's bytes, `mode` is `{"mark":"bytes-sha256/1","toolchain":<the SHA-256 of the UTF-8 bytes of
"bytes-sha256/1">}` (the toolchain names the convention, not a program). For one file the command line
and the app write these two members alike. `--file` stands for `--content`, `--mark` and
`--toolchain`; given with any of them it is misuse (`E_ARGS`, `--file --content --mark --toolchain`).
Without `--file` nothing is filled in for the caller: the three flags are as given, and what is left out
is refused by the law.

**`anchor` reads its fees from its one endpoint.** It asks the head, that block's `baseFeePerGas` and
the priority fees paid over the twenty blocks up to it (`eth_feeHistory`, each block's median), and
takes the rule the app takes (`zikaron_anchor::send::Fees::of`): the priority fee is the median of those
block medians, at most 1 gwei, and 1 gwei when it cannot be read (`null`, refused, another shape); the
fee cap is base fee × 2 + priority fee; without a base fee the pair is the fallback (3 gwei, 1 gwei).
`anchor` then asks its one endpoint for a gas estimate of the very transaction it is about to sign, at the
head that endpoint gives, by the rule the app takes (`zikaron_anchor::send::estimate_gas`): the estimate is read
as a quantity of at most 128 bits, and the transaction carries one and a half times it, rounded up, at most the
ceiling of 200,000 (`zikaron_anchor::send::limit_for`, as in the app). An estimate the endpoint refuses for a
reason about the call itself (by the one table the app takes too, `said::refuses_the_call`: a member about the
call, or the node's JSON-RPC error with a numeric `code` the table does not name; it is taken as "the call would
revert", and the node's words are in `detail`), or one above 200,000, is refused with `E_GAS_REFUSED` (exit 1)
and nothing is broadcast. A refusal for the endpoint's own reasons (a rate limit, a missing method, credentials,
a wrong chain, an error without a numeric `code`, a page at an HTTP status) says nothing about the call: it is
`E_UNREACHABLE` (exit 4, the table's name for the reason in `detail`), as is an estimate the endpoint does not
answer, or answers in another shape, or a head it will not give; nothing is broadcast either. The affirmative answer's members do not change.

**`anchor` without `--home` asks before it signs again.** One anchoring is known by what it sends: the chain, the
sending account, the recipient and the call data. For each transaction it signs for an anchoring, the command line
keeps the nonce and the hash in this user's machine folder (`cli-sent/`, one small file each, landed after signing
and before broadcasting; no key and no endpoint are kept). Run again, it first asks its endpoint where those
transactions stand, by their hashes, by the rule the app's queue takes (`zikaron_anchor::send::earlier`):

- one has a receipt: answered as included (exit 0 with `blockNumber`, `tx`; or `E_TX_STATUS`, exit 1, when its status
  is not 1, and the record then marks it void so the next run sends the anchoring afresh); nothing is broadcast;
- the endpoint holds one and none has a receipt: its receipt is awaited as for a fresh send (`E_TX_NOT_YET` when the
  wait runs out); nothing new is signed;
- none is held and their nonce is unused: signed again at that same nonce, so at most one of them can be included;
- none is held and their nonce is used by another transaction: `E_TX_VOID` (exit 1) with `tx`, nothing is sent, and
  the record marks them void so the next run sends the anchoring afresh;
- the record cannot be read, or a question goes unanswered or comes back in another shape: `E_UNREACHABLE` (exit 4)
  with `detail` (and `tx`, the transaction asked about, when it was a question about one), and nothing is signed.

The same command run after it succeeded therefore answers the earlier inclusion and broadcasts nothing.

**`anchor` does not anchor again what the chain holds.** With `--form registry` and `--hash`, before it signs,
`anchor` asks its endpoint whether the registry already holds an anchor of any of those hashes from this key: the
registry's `Anchored` logs from the registry's deployment block (the known deployments table,
`zikaron_anchor::deploy`; block 0 for a registry no row names) to the head, read as the scan reads logs (the range
shrunk where the node limits it; each log judged again: the registry, the event, this sender, one of the hashes).
Any hash already anchored, by whichever side anchored it (the desktop, another machine): `E_ON_CHAIN` (exit 1) with
those hashes in `names` and the earliest such anchor's `tx` and `blockNumber`; nothing is sent. A head or a range
the endpoint does not answer is `E_UNREACHABLE` (exit 4) with `detail`, and nothing is sent: whether a hash is on
chain is never guessed. The bare form and `--calldata` name no registry log to ask by and are sent as before.

**`anchor --ledger`: a home's queue, with the credential.** With `--ledger` naming the ledger folder of a home the
desktop keeps, `anchor` sends that home's queue, as the desktop's send key does, through the one sender the desktop
sends by (section 12, what is sent). It takes no `--key`, `--key-file`, `--endpoint`, `--form`, `--registry`,
`--hash`, `--calldata` or `--proxy` (each is misuse, `E_ARGS` naming it): the chain, the registry and the nodes come
from the home's settings, the proxy from this machine's, the key from the vault through the credential;
`--wait-secs` is the receipt wait (ninety seconds without it). A `--ledger` that is not a home's is misuse
(`E_ARGS --ledger`). The answers are those of the same `anchor` through the desktop (section 11), built by the same
code: `blockNumber`, `tx`; `E_TX_STATUS`, `E_TX_NOT_YET`, `E_TX_VOID`, `E_UNREACHABLE` with `tx`; `E_ON_CHAIN`;
`E_QUEUED`; `E_SEND_REFUSED`.

## 7 · Closed table of output keys

`address` `anchored` `attestation` `attestor` `author` `blockNumber` `content` `count` `detail` `doc` `docId`
`dropped` `entries` `entryId` `entryType` `files` `firstAt` `fragment` `handoff` `index` `kitId` `ledger` `leftOut` `names`
`ok` `original` `originals` `path`
`payload` `prev` `privkey` `proofs` `reason` `records` `seq` `singleSource` `singleSourceChains` `sources`
`state` `taken` `token` `tx` `value` `written`

Forty-four members, matching `codes::Key` entry by entry; every member in the table can actually be
printed (the closed table has no dead entries).

**`contract` is a shape of its own.** Its answer has `ok` from this table and every other member name from
the closed table `codes::Contract` (section 10): `exits`, `code`, `name`, `reasons`, `standsFor`, `flag`,
`for`, `verbs`, `flags`, `homeFlags`. Those ten are not members of `codes::Key` and are not counted above.
The messages of `mcp` are JSON-RPC and the Model Context Protocol's own (section 13), outside both tables.

**`detail` of a storage or landing refusal** (`E_LEDGER`) is the refusing layer's code (`E_IO`,
`E_OCCUPIED`, …). For `E_IO`, a disk operation that failed, it goes on after `: ` with what the system
said: the operation, the path it was done on and the system's own words (for example
`E_IO: create directory b: Permission denied (os error 13)`); the code is the text before the first `: `.
A `kit-export` output that fails on disk carries this `detail` beside its `path`.

Keys inside base answers are not listed here: they belong to `tokens::Key` in `zikaron` and
`tokens::Key` in `zikaron-kit`, each the source of truth for its own side.

## 8 · Flags

All flags are long flags, and **every flag takes a value** (there are no boolean flags, so the
ambiguity "is the next token a value or the next flag" cannot arise; a token starting with `--` in a
value position is misuse). Where a flag may be repeated, this is stated per flag. **Each verb's own
list of flags is closed**: a flag outside that list is misuse (HARNESS: an argument beyond a
command's own list).

Body members that the law requires are always **optional** on the command line: if one is missing,
the law refuses it. The body really lacks that member, and the core's thirteen-step check returns
`E_BODY_FIELD` on the spot; if the shell checked it again, there would be two copies of the law. The
shell insists only on what it needs to do its own work. The "When absent" column below keeps the two
cases apart: **"left to the law" is a refusal by the law, "misuse" is a refusal by the shell.**

**Sixty-two flags, matching the closed table of flag names in the code (`ALL_FLAGS` plus
`MORE_FLAGS` in `verbs.rs`) entry by entry**; if the table and the code differ by one row,
`the_cli_schema_lists_every_flag_the_code_knows` in `crates/zikaron-cli/tests/cli.rs` fails.

| Flag | Value | When absent |
|---|---|---|
| `--ledger` | path to the ledger folder (a plain one, or a home's the desktop keeps); for `audit` / `check-grant` / `chain-check` / `depth` / `show --entry` also a mirror bundle or a record package (a folder or one file); for `scan`, a home's ledger whose settings declare the basis; for `hand-over` and `take-over`, the place handing or taking; for `anchor`, a home's ledger whose queue is sent with the credential (section 6), a folder that is not a home's being misuse; for `kit-verify`, a home's ledger, whose identities open a sealed package with the credential (a folder that is not a home's being misuse) | misuse for the create and write-entry verbs, `kit-export`, `show --entry`, `hand-over` and `take-over`; `anchor` without it sends directly (`--key`, `--endpoint`); for `scan`, `--basis` is taken instead (both or neither is misuse); `audit` needs this or `--input` (neither is misuse, named `--ledger`); `check-grant` / `chain-check` / `depth` with neither this nor `--input` judge without an audit outcome (a `chain-check` hop may still bring its own input); `kit-verify` without it opens a sealed package with `--key` or `--key-file` |
| `--key` | private key, sixty-four hex digits (a `0x` prefix is allowed), scalar in [1, n−1]; for `kit-verify`, the key that opens a sealed package | misuse, unless `--key-file` gives the key; malformed or out of range is misuse too. For `kit-verify`: absent is no fault, except for a sealed package opened with neither `--ledger` nor `--home` (section 6); given with `--ledger`, misuse |
| `--key-file` | path to a file holding the private key as `--key` takes it (white space and line ends at either end dropped; at most 4096 bytes); taken by every verb that takes `--key` | `--key` gives the key; both given is misuse naming both. A path that is not a readable file is misuse naming the path by its place and length; a file other accounts can read is misuse naming the path by its place and length (`E_KEY`); contents that are not a key are misuse as for `--key` |
| `--root` | hex20, the root of the lineage | taken from the genesis entry in the store; zero or several each refused by name |
| `--seq` | decimal integer | goes together with `--prev`; with both absent the tip is computed from the core's lineage |
| `--prev` | hex32 | for the write-entry verbs, as above, and giving only one of the two is misuse; for `attest` (which takes no `--seq`), the `prev` of the adoption being cosigned, required alone: absent is misuse |
| `--statement` | prose (`statement_md` of `genesis` / `succession`) | the body lacks this member; left to the law |
| `--content` | hex32 (`content` of `history`) | the body lacks this member; left to the law; with `--file`, misuse |
| `--mark` | token (`mode.mark` of `history`) | if either this or `--toolchain` is present, `mode` is set; with both absent there is no `mode`; left to the law |
| `--toolchain` | hex32 (`mode.toolchain`) | as above |
| `--note` | prose | for entry verbs, no `note_md`; for documents and kits, an empty string |
| `--grantee` | hex20 | the body lacks this member; left to the law |
| `--work` | hex32 | for `grant`, the member is absent and the law refuses; for `fpm-sign`, an empty string is written and the kit check refuses; misuse for `depth` |
| `--terms` | hex32 | the body lacks this member; left to the law |
| `--history` | hex32 (optional member of the `grant` body) | member absent |
| `--window-from` | integer, at most 9007199254740991 (the law's whole-number ceiling; past it, within 64 bits or not, misuse `E_ARGS --window-from`) | if either this or `--window-to` is present, `window` is set; with both absent there is no window |
| `--window-to` | integer, at most 9007199254740991 (as above) | as above |
| `--scope` | prose (`scope_md` of `grant`) | member absent |
| `--upstream` | hex32 | member absent |
| `--grant` | three meanings by verb: for `revoke`, the body's `grant` (hex32); for `check-grant`, the path to a grant entry file or a grant file (the app's `.zkgrant`); for `fpm-sign`, the `grant` member (hex32) | `revoke`: left to the law; `check-grant`: misuse; `fpm-sign`: null is written |
| `--case` | hex32 (optional member of `revoke`) | member absent (constraint: `case` is optional) |
| `--anchors` | path to a JSON file (an array, read by the core's parser) | for `adopt`, the body lacks this member; left to the law; misuse for `attest` |
| `--attestor` | hex20 | member absent; pairs with `--attestation`, and giving only one is refused by the law |
| `--attestation` | hex65 | as above |
| `--author` | hex20 (the party co-signed in `attest`) | misuse |
| `--to` | hex20: `to` of `succeed`; for `kit-export`, an address, `0x` and 40 hex digits in either case (a checksummed address as pasted): the address a record package is sealed to, its key found on chain (section 6; through the desktop, by the desktop) | `succeed`: the body lacks this member; left to the law. `kit-export`: the package is not sealed; with `--to-key`, or with `--as folder`, misuse |
| `--kind` | free token (`kind` of `succeed`; the shell keeps no allow-list) | the body lacks this member; left to the law |
| `--effective` | integer, at most 9007199254740991 (past it, within 64 bits or not, misuse `E_ARGS --effective`) | the body lacks this member; left to the law |
| `--subject` | hex32 (optional member of `annotate`; for `retract`, the record being deleted) | `annotate`: member absent; `retract`: refused by the convention (`E_RETRACTION` + `E_RETRACT_SHAPE`) |
| `--endpoint` | `<chain id>=<url>`, repeatable; the url `http://` or `https://`, scheme and host in any case; white space at either end of the value is not part of it, and no white space is allowed inside the value (around the `=` or in the url) | `anchor` takes **exactly one** (more is misuse: this layer has no failover); `scan` takes either this or `--fixture`, or neither with `--ledger` naming a home (its own nodes, section 6); none at all otherwise is misuse; `kit-export` takes it (one or more, each chain one the known deployments table names) only with `--to` on a plain ledger folder, needing it there and refusing it with a home's ledger; every value is read before any node is asked, and one that does not read is misuse whose subject never echoes it (the url may carry a key): its place and length, `#3 (40 bytes)`, or, when the part before the first `=` is a chain id, that id and the length after the `=`, `#3 (1= + 38 bytes)` |
| `--form` | `registry` or `bare` | misuse; a word outside the table is misuse |
| `--registry` | hex20 registry contract address | missing with `--form registry`, or given with `--form bare`: misuse (`E_ARGS --registry`), before any node is asked |
| `--hash` | hex32, repeatable | at least one of this and `--calldata`; both absent is misuse; with `--calldata` given, not used |
| `--calldata` | hex bytes | as above; when given, it is the transaction's data and takes precedence over `--hash` |
| `--wait-secs` | integer, seconds to wait for inclusion | ninety seconds |
| `--proxy` | `system`, `none`, or one proxy address `scheme://[user[:password]@]host:port` with the scheme `http` (CONNECT), `https` (TLS to the proxy, its certificate verified, then CONNECT), `socks5` (or `socks5h`) or `socks4` (or `socks4a`: a user id, no password); credentials percent-encoded, each at most 255 bytes, given to the proxy that asks for them (HTTP Basic, SOCKS5 user name and password, the SOCKS4 user id) and never echoed; host names are resolved by the proxy. Loopback nodes are always reached straight; with a proxy address, so are private (`10/8`, `172.16/12`, `192.168/16`), link-local (`169.254/16`, `fe80::/10`) and `.local` nodes. A password written here is visible to other processes of this machine while the command runs | `system`: the terminal's proxy variables when any of `https_proxy`, `http_proxy`, `all_proxy` (lower or upper case) is set, with `no_proxy`, as programs in a terminal take them, on every system; else the system's proxy settings: the configuration script at the address the system names (PAC: fetched directly, over `https` from anywhere or over `http` from this machine only (a literal loopback address or `localhost`; an `http` address elsewhere, or a redirect to one, is not fetched), at most 1 MiB within 5 s; run in a sandbox with an 8 MiB memory limit, a 512 KiB stack limit and a 2 s limit per run, its name lookups 1 s each within it), then the explicit proxies (macOS the network configuration; Windows the user's settings as the system answers them, the machine's under the per-machine policy; Linux KDE's or GSettings' stored settings, else `https_proxy`, `http_proxy`, `all_proxy`, `no_proxy`); automatic discovery (WPAD) is not read, a system with only that switched on counting as nothing set; a script that cannot be used leaves the explicit proxies, and its trouble has a name (`PAC_UNFETCHED`, `PAC_SCHEME`, `PAC_REMOTE_HTTP`, `PAC_TOO_LARGE`, `PAC_FAILED`, `PAC_TIMEOUT`, `PAC_MEMORY`, `PAC_NO_FUNCTION`, `PAC_ANSWER`); the ways a script names are tried in order until one opens (`SOCKS`: SOCKS5, then SOCKS4). Any other word or address is misuse, never echoed; with `scan --fixture`, misuse (a recording reaches no node); `kit-export` takes it only with `--to`. A home's road (`scan --ledger` reading the home's own nodes, with neither `--endpoint` nor `--fixture`; `kit-export --to --ledger`; `anchor --ledger`) without it takes this machine's proxy setting instead, as the desktop takes it: following the system there means the system's settings alone, never the terminal's proxy variables |
| `--fixture` | path to a recording file, repeatable (one recording counts as one source) | either this or `--endpoint`; both or neither is misuse |
| `--basis` | path to a law §9.4 basis file | needed when taking the `--endpoint` path unless `--ledger` names a home (neither is misuse, named `--basis --ledger`); with `--fixture`, misuse (the recording's basis is used) |
| `--adoptions` | path to an adoptions file (`{"adoptions":[{"chainId":<int>,"tx":<hex32>},…]}`) | adoption evidence is not consulted; with `--fixture`, misuse (the recording's adoptions are used) |
| `--input` | path to a ready-made audit input file (**the raw bytes are handed to the law unchanged**) | assembled from `--ledger` and `--fragment` |
| `--fragment` | path to a scan fragment or reading file (both a fragment wrapped in `fragment` and a bare fragment are accepted) | an empty fragment is used (an offline reading of the lineage); for `kit-verify`, the chain is not read and every record is `unread` |
| `--unavailable` | hex32, repeatable (the fourth member of the audit input) | empty |
| `--now` | integer (the injected now) | not injected; only chain time is used (general rule 6) |
| `--hop` | `<grant file>` or `<grant file>=<audit input file>`, repeatable | none at all is an empty chain, judged FAIL, exit 1 |
| `--entry` | entry id (bare sixty-four digits or with `0x`; the form follows the storage layer's naming rule), repeatable for `kit-export` | `kit-export`: all entries are selected; `show`: either this or `--path` |
| `--path` | path to a file (`show` reads its bytes directly) | either this or `--entry`; both or neither is misuse |
| `--out` | output path | misuse for `kit-export` and `hand-over`; for `fpm-sign` / `ack-sign`, the result is only printed, not written; for `audit`, the audit input is not written |
| `--file` | two meanings by verb: for `kit-export`, `<path in kit>=<path on disk>`, repeatable, the path in kit relative to the kit's `files/` room (`a/b.pdf` lands at `files/a/b.pdf`); for `history`, the path of one file to record by the record convention (below) | `kit-export`: no files attached; `history`: `--content`, `--mark` and `--toolchain` as given |
| `--proof` | `<path in kit>=<tx>=<path on disk>`, repeatable; the path in kit is relative to the kit's `proofs/` room | no proof bundles attached |
| `--variant` | hex32 (`variant` of `ack-sign`) | empty string, refused by the kit check |
| `--fpm` | hex32 document id | **exactly one** of this and `--fpm-doc`; both or neither is misuse |
| `--fpm-doc` | path to a fingerprint manifest file (when given, pairing is done on the spot) | as above |
| `--rows` | path to a JSON file (an array) | empty array, refused by the kit check |
| `--encode` | path to an entry file, repeatable | **exactly one** of this and `--decode`; both or neither is misuse |
| `--decode` | path to a badge payload file | as above |
| `--as` | `file` or `folder`: the form `kit-export` writes the package in | one file; any other word is misuse |
| `--kit` | path to a record package, a folder or one file (`kit-verify`) | misuse |
| `--originals` | `none` (`kit-export` from a home the desktop keeps, by `--ledger` or `--home`): the chosen records' originals are left out of the package | they go in, taken as the desktop's export page takes them (`originals`, `leftOut`); any other word is misuse; with a plain ledger folder, misuse whatever its word (its originals are each given with `--file`) |
| `--originals-to` | path to a folder that is there (`kit-verify`): the package's originals that match their records are taken out into it, all or nothing (`taken`, `path`; refused, `E_EXTRACT`, nothing written) | nothing is taken out |
| `--to-key` | a public key, `0x` and 66 (compressed) or 130 (uncompressed) hex digits, case ignored, a point of the curve (`kit-export`, on every road): the record package is sealed to it | the package is not sealed; off the curve, or with `--to` or `--as folder`, misuse |
| `--handoff` | path to a handoff file (`take-over`) | misuse |
| `--home` | path to the desktop's data folder (the home it has open): the verb is done by the running desktop through its door (section 11) | the verb runs here, on what its other flags name, as it always did |

### Flags by verb

Each verb's closed list, as its row in `ACCEPTS` in `verbs.rs` names it. The five flags `ledger`, `key`,
`root`, `seq`, `prev` are the shared list of the entry-writing verbs (`WRITE`).

| Verb | Flags it accepts |
|---|---|
| `keygen` | none |
| `init` | `ledger` `key` `statement` `home` |
| `history` | `ledger` `key` `root` `seq` `prev` `content` `mark` `toolchain` `note` `file` `home` |
| `grant` | `ledger` `key` `root` `seq` `prev` `grantee` `work` `terms` `history` `window-from` `window-to` `scope` `upstream` `home` |
| `revoke` | `ledger` `key` `root` `seq` `prev` `grant` `case` `home` |
| `adopt` | `ledger` `key` `root` `seq` `prev` `anchors` `attestor` `attestation` `home` |
| `attest` | `key` `author` `anchors` `prev` `home` |
| `succeed` | `ledger` `key` `root` `seq` `prev` `to` `kind` `effective` `statement` `home` |
| `annotate` | `ledger` `key` `root` `seq` `prev` `subject` `note` `home` |
| `retract` | `ledger` `key` `root` `seq` `prev` `subject` `note` `home` |
| `anchor` | `key` `endpoint` `form` `registry` `hash` `calldata` `wait-secs` `proxy` `home` `ledger` |
| `scan` | `endpoint` `fixture` `basis` `adoptions` `proxy` `ledger` |
| `audit` | `ledger` `root` `input` `fragment` `unavailable` `out` |
| `check-grant` | `ledger` `root` `input` `fragment` `unavailable` `now` `grant` |
| `chain-check` | `ledger` `root` `hop` `now` `input` `fragment` `unavailable` |
| `depth` | `ledger` `root` `work` `input` `fragment` `unavailable` |
| `fpm-sign` | `key` `work` `grant` `note` `rows` `out` |
| `ack-sign` | `key` `note` `fpm` `fpm-doc` `variant` `out` |
| `badge` | `encode` `decode` |
| `kit-export` | `ledger` `root` `note` `out` `entry` `file` `proof` `as` `home` `to` `to-key` `endpoint` `proxy` `originals` |
| `kit-verify` | `kit` `fragment` `unavailable` `originals-to` `home` `ledger` `key` |
| `hand-over` | `ledger` `out` |
| `take-over` | `ledger` `handoff` |
| `show` | `ledger` `entry` `path` |
| `contract` | none |
| `mcp` | none |

## 9 · Environment variables

The command line reads these four from its environment. It also finds this user's machine folder the way the desktop
does (section 11), for `--home`, the credential, the machine settings `anchor --ledger` reads, the lists of adopted
and handed-off ledgers and `anchor`'s record of what it sent: on macOS and Linux under `HOME`, on Windows under the
user's profile. Under `--proxy system` (the default) it follows the terminal's proxy variables (`https_proxy`,
`http_proxy`, `all_proxy`, `no_proxy`, lower or upper case; section 8, `--proxy`); a home's road without `--proxy`
(`scan --ledger` reading the home's own nodes, `kit-export --to --ledger`, `anchor --ledger`) goes by this machine's
proxy setting instead, as the desktop does, and reads none of them; `scan --ledger` given `--endpoint` follows
`--proxy system` as any `scan` does. The locale variables choose the
language of the lines for people (section 3).

| Variable | Effect | Default |
|---|---|---|
| `ZKA_TIMEOUT_SECS` | Total wall time of one HTTP/HTTPS exchange with a node, in seconds; `0` means no deadline | 30 |
| `ZKA_TIMEOUT_MS` | The same deadline in milliseconds; when set and readable it overrides `ZKA_TIMEOUT_SECS` | unset |
| `ZKA_MAX_ANSWER_BYTES` | The most bytes one node answer may have; `0` means no cap | 67108864 (64 MiB) |
| `ZIKARON_TRACE` | A file path: when set, diagnostic trace marks are appended there (capped at 8,388,608 bytes); stdout and the exit code are unchanged | unset (no trace) |

For the three `ZKA_` variables, a value that does not parse as a whole number is ignored and the default holds (`crates/zikaron-net/src/lib.rs`,
`Limits::from_env`; `crates/zikaron/src/trace.rs`).

## 10 · `contract`: the contract as one value

`zikaron contract` takes no flags (any flag is misuse, as for every verb) and answers, exit 0, one canonical
JSON value describing this command line's own contract. The value is **generated from the closed tables in
the code**, never from a second list: the verbs and each verb's flags from `ACCEPTS` in `verbs.rs` (the table
every verb's arguments are closed by, and of which `VERBS` is the first column), the flags that stand for
another from `args::STANDS_FOR` (the table `Args::close` reads), the exit codes from `Exit::ALL` and the
refusal reasons from `Reason::ALL` in `codes.rs`. Its member names are the closed table `codes::Contract`,
beside `ok`; it reads nothing and writes nothing.

| Member | Value |
|---|---|
| `ok` | `true` |
| `exits` | array, one object per exit code in `Exit::ALL` order: `code` (the number) and `name` (`affirmed`, `denied`, `misuse`, `partial`, `unanswered`) |
| `code` | inside `exits`: the exit code, an integer |
| `name` | inside `exits`: the code's short name; inside `verbs`: the verb's name |
| `reasons` | array of the refusal reasons in `Reason::ALL` order (section 4) |
| `standsFor` | array, one object per flag that gives another flag's value another way: `flag` and `for`; a verb whose `flags` hold `for` takes `flag` too (today one row: `key-file` for `key`) |
| `flag` | inside `standsFor`: the flag that stands for another, without `--` |
| `for` | inside `standsFor`: the flag it stands for, without `--` |
| `verbs` | array, one object per verb in `VERBS` order: `name` and `flags` |
| `flags` | inside `verbs`: that verb's closed flag list, without `--`, in the order of its row (section 8, flags by verb); empty for a verb that takes none |
| `homeFlags` | inside `verbs`, only on a verb the desktop does through its door: the flags that verb takes beside `--home`, without `--`, in the order of its row in section 11 (empty for one that takes none) |

```
{"exits":[{"code":0,"name":"affirmed"},…],"ok":true,"reasons":["E_ARGS",…],"standsFor":[{"flag":"key-file","for":"key"}],"verbs":[{"flags":[],"name":"keygen"},{"flags":["ledger","key","statement","home"],"homeFlags":["statement"],"name":"init"},…]}
```

## 11 · `--home`: through the desktop

With `--home <the desktop's data folder>`, eleven verbs are done by the running ZIKARON Desk in that data folder,
through its own action layer, instead of here: the entry is written into the desktop's ledger with the desktop's
own identity, recorded in its trail, and seen in its window at once, with nothing reopened or refreshed. The
ledger is one ledger and the state one state. The command line holds no key and no passcode on this path, and
writes nothing itself.

**The door.** While the desktop is unlocked and holds the data folder as its writer, it keeps a door open for
that folder: on macOS and Linux a local socket file in this user's machine folder (readable and writable by its
owner only), on Windows a named pipe only this user may reach and no other machine. Its place is named by the
machine folder and the data folder as the system resolves them, so a data folder the desktop does not have open
has no door. While "With a credential" is on (section 12) the desktop keeps the door shut, and a request finds
none: it is answered `E_DESKTOP` with `detail` `ALONE` (its credential is in the machine folder), never as a
locked or absent desktop. Once connected, each side asks the system who the other runs as and refuses another user. There is
no token. Locking, closing the data folder, switching it or quitting closes the door; a request still waiting is
answered `E_DESKTOP` with `detail` `CLOSED`. The machine folder is found as the desktop finds it: the pointer
`.zikaron-desk` in the folder the system keeps this app's data in (the home directory on macOS and Linux,
`%LOCALAPPDATA%\ZIKARON` on Windows), then the default `.zikaron-desk.d` in that same folder; no other place is looked at.

**Order.** The arguments are read and misuse is judged exactly as without `--home`, before the desktop is asked
(exit 2, nothing on stdout): each verb takes `--home` and the flags of its row below, and nothing else (a flag
the verb takes on its own but not with `--home` is misuse saying so); the data folder must be a folder; for
`history` the file, and for `adopt` an `--anchors` file given, must be there and read (the anchors as the law's
JSON); whole numbers are judged against the law's ceiling and entry ids by their one form. Paths are made whole
before they go (the desktop does not share this process's working folder).
Requests are done one at a time, in the order they reach the door; one that reaches it while another is being
done waits.

**Answers.** Each verb answers with its own row of section 6, built by the same code as without `--home`:
`entryId`, `ledger` (the desktop's ledger folder), `seq`, `written` (always `true`) for an entry written; `anchor`
as in section 6 (the desktop sends the batch its queue holds, all that can be sent, the way its queue page's send
key does: the gas estimate first, then the batch, then the receipt wait, by its own nodes and rules); `kit-export`
as in section 6 with `proofs` 0. The desktop answers each request from the outcome of the work that request
started, never from another of its kind: a batch broadcast whose receipt wait does not start (the desktop locks
meanwhile) is `E_TX_NOT_YET` with its `tx` and `count` 0, never a loss; an estimate made while the chain,
registry or nodes changed is refused by name (`E_SEND_REFUSED`, `token` `GAS_NOT_ESTIMATED`, the change in
`detail`); work that lands after the data folder was closed is `E_DESKTOP` with `detail` `CLOSED`. `anchor` answers
as `anchor --ledger` does (section 6): the chain already holding the anchors is `E_ON_CHAIN` with its `names`,
`tx`, `blockNumber`, and any other refusal before anything went out `E_SEND_REFUSED`. Besides those:

| Reason | Exit | When |
|---|---|---|
| `E_DESKTOP` | 4 | no answer from the desktop for that data folder (section 4 lists the seven `detail` words) |
| `E_ON_DESKTOP` | 1 | `attest`: attesting for someone asks for the passcode, which is given on the desktop alone; `succeed`: handing the ledger to another key is final and rare, and is done on the desktop by hand alone (the desktop is not asked) |
| `E_QUEUED` | 3 | `anchor` while the desktop's setting "putting on chain from the command line" is *Queue only*: nothing is sent; `count` entries wait, and the desktop shows the request at once |
| `E_SEND_REFUSED` | 1 or 4 | `anchor`: refused before anything went out: `token` its code, `detail` its evidence; 4 for a network refusal |
| `E_DESKTOP_REFUSED` | 1 or 4 | the desktop refused by name (any verb but `anchor`): `token` its code, `detail` its evidence; 4 when it is no answer (a network refusal, or `ADDRESS_CHAIN_UNREAD`: an address's key looked for and some network not read through, which the desktop also says in a notice) |

The line for people on stderr says each of these in the system's language (section 3), the desktop's own
refusals in the sentence the desktop gives them in either language.

**The verbs and the flags each takes beside `--home`.** The other fourteen verbs do not take `--home` (it is
misuse with them): their answers are keys, files or readings that are not the desktop's to give (`keygen` would
hand out a private key), or they do not touch a data folder. `mcp`'s write tools take this way while this machine
has no credential (section 13).

| Verb | Flags beside `--home` |
|---|---|
| `init` | `statement` |
| `history` | `file` `note` |
| `grant` | `grantee` `work` `terms` `history` `window-from` `window-to` `scope` `upstream` |
| `revoke` | `grant` `case` |
| `adopt` | `anchors` `attestor` `attestation` |
| `attest` | none |
| `annotate` | `subject` `note` |
| `retract` | `subject` `note` |
| `anchor` | none |
| `kit-export` | `entry` `note` `out` `as` `to` `to-key` `originals` |
| `kit-verify` | `kit` `originals-to` |

`attest` is always answered `E_ON_DESKTOP` by the desktop. `succeed` takes `--home` and always answers `E_ON_DESKTOP` (exit 1) without asking the desktop: nothing is written; its flags are closed by its own row (section 8, flags by verb), and their values are not read. `history` records the file by the record convention, as the desktop's own
record does; `kit-export` takes the
entries named (all of them without `--entry`), in the form `--as` names (one file without it), with the chosen records'
originals taken as the desktop's export page takes them unless `--originals none` (the answer's `originals` and
`leftOut`, section 6), other attachments and proof bundles being chosen on the desktop; with `--to` (an address, its key found on the desktop's network and its
read-only networks) or `--to-key` (a key), the one file is sealed to that key, and the answer is as without them; both
given, or either with `--as folder`, an address not in its form or a key off the curve, is misuse judged here (exit 2,
the desktop not asked), and the desktop's refusals of a sealed package are `E_DESKTOP_REFUSED` with the codes of
`E_ADDRESSED` (section 4) as `token`.
`kit-verify` is the desktop's verification of the package at `--kit` on its own networks (a package sealed to one
of its identities is opened with it first), answered as section 6 answers it, the originals taken out into
`--originals-to` when given. Its refusals of taking the originals out or of opening a sealed package are
`E_DESKTOP_REFUSED`, `token` the desktop's code (`EXTRACT_PATH`, `EXTRACT_NOTHING`, `EXTRACT_CLASH`,
`EXTRACT_NOT_A_FOLDER`, `EXTRACT_EXISTS`, `EXTRACT_LINK`, `EXTRACT_CHANGED`, `EXTRACT_LANDING`, `EXTRACT_KIT`; or the
codes of `E_ADDRESSED`, section 4) and `detail` its evidence; a package the kit law refuses is `E_KIT` as in section 6.

## 12 · With a credential: the credential, the switches, the adopted ledgers

**Two switches** in the desktop (Settings > Command line and AI control, the group "Automation level") decide how the command line
may write the data ZIKARON Desk keeps sealed. The higher outranks the lower, so there is one way at any time:

| With a credential | Through the desktop app | How the command line writes | The door (section 11) |
|---|---|---|---|
| off | off | it does not write this data | closed |
| off | on | through the door, while the desktop is unlocked | open while unlocked |
| on | on or off (greyed, cannot be switched) | alone, with the credential, the desktop open or not | closed |

`mcp`'s write tools follow the same switches (section 13): with the credential while it is in this machine's folder,
through the door otherwise; it adds no way and no switch of its own.

**The credential.** Turning "With a credential" on asks for the passcode and a validity (minutes, hours or days, or
never expiring). The desktop then writes a credential file in this user's machine folder, readable and writable
by its owner only: a fresh random key, the master key sealed under it, when it was issued and when it expires,
and nothing else. The command line takes nothing but this file to open the sealed data: no passcode, no key flag.
Its expiry is checked at every use. Turning the switch off removes the file at once; an expired one is removed
when it is met; a new master key on the desktop makes it worthless, and the desktop removes it and turns the
switch off. A changed passcode keeps the credential working. A credential that fails counts no passcode try.

**What is written.** `history`, `grant`, `revoke`, `adopt`, `succeed`, `annotate` and `retract` with `--ledger`
naming the ledger folder of a home the desktop keeps (its `settings` room holds the home's label) write it with
the credential: the key is the one the desktop's vault keeps for the home's identity and seat (the current one
for a home of no seat), so `--key` and `--key-file` are misuse there (`E_ARGS --key --key-file`). `history --file`
there also records the file as this machine's original of that record, in the index of signed files the desktop
keeps as it signs (its place, its name, the content it gave), before the entry lands; a record package exported
later takes the original from there, as for a file the desktop signed. The entry passes
the same write gate (`E_WOULD_BREAK`), lands in the sealed ledger, and is queued for anchoring as the desktop
queues its own. `init` is not taken there: such a home has its root already, and `init` answers as for any
ledger that is not empty (`E_NOT_EMPTY`). Before an entry is written there, the home's write gates are asked as for a
send (the sender's one judgement, below): a home under its read-only mark or whose ledger's own entries break its
chain answers `E_READ_ONLY`, a ledger handed off `E_HANDED_OFF`; nothing is written.

**What is sent.** `anchor --ledger` with the ledger folder of such a home sends its queue with the credential
(section 6), by the desktop's setting "putting on chain from the command line", judged as the desktop's door judges
it: *Send automatically* sends every entry the queue can send as one batch (estimate, the balance, the exit gate
and the home's write gates, signature, the batch recorded as in flight before its bytes go, the receipt, and the
queue settled); *Queue only* sends nothing and answers `E_QUEUED`. With nothing to send, a batch left in flight (a
sender stopped half way) is asked about by its hash and settled, and with none either the answer is
`E_SEND_REFUSED` with `token` `QUEUE_EMPTY`. A batch in flight that no node holds is judged by its nonce, by the
rule the direct `anchor` reruns by (`zikaron_anchor::send::unheld`): its nonce used by another transaction, it is
void (`E_TX_VOID`, its entries back in the queue); its nonce still unused, *Send automatically* signs it again as
recorded, at that very nonce, and sends it through the same gates as any batch, answered as a send is (`blockNumber`,
`tx`; `E_TX_NOT_YET`, …), so at most one transaction at that nonce is ever included; *Queue only* sends nothing and
answers `E_TX_NOT_YET`. A batch that some entries the chain already holds keep from going is answered `E_ON_CHAIN`,
nothing sent; those entries then leave the queue, recorded as anchored, by the reading every node of the chain agrees
on (the desktop's self-audit takes them out by the same reading; one node's word moves none), so the next run sends
the rest. The home's write gates are the desktop's: a home under its read-only
mark (`LEDGER_NOT_FETCHED`, `NEWER_ELSEWHERE`), a ledger whose own entries break its chain (`BROKEN`), a ledger
handed off (`HANDED_OFF`) send nothing. A resend at a higher fee is the desktop's alone.

**One writer at a time.** The command line takes the home's writer lock for each write and lets it go when done.
While "With a credential" is on, the desktop does the same: it takes the lock for each write of its own and lets it go
after, and before writing reads the ledger, the queue and the settings again if the command line wrote meanwhile.
A send holds the lock while it readies the batch and signs and sends it, lets it go while it waits for the
receipt, and takes it back to settle (the desktop's send and the command line's alike): settling reads the queue
anew and changes only entries still awaiting that batch. So while one side waits for a receipt the other may send:
it finds that batch in flight, leaves it alone and sends only the rest. A sender stopped while it waited leaves
its batch in flight; the desktop's next resume, or the next `anchor --ledger` with nothing else to send, asks
about it and settles it.
A home on a synced disk names the machine that writes it; another machine's mark is no license to write here.

| Reason | Exit | When |
|---|---|---|
| `E_CREDENTIAL_MISSING` | 1 | no credential on this machine ("With a credential" is off) |
| `E_CREDENTIAL_EXPIRED` | 1 | the credential's time is up; it was removed |
| `E_CREDENTIAL_SHAPE` | 1 | the credential does not read or open, or holds another master key than this machine's vault |
| `E_HOME_HELD` | 1 | the desktop is writing the home at this moment; try again shortly (for `anchor --ledger`, also the lock not taken back within sixty seconds to settle a receipt: the batch stays in flight) |
| `E_HOME_OTHER_MACHINE` | 1 | the home's writer mark names another machine, or does not read |
| `E_READ_ONLY` | 1 | writing or handing over: the home is read-only (its read-only mark, or a ledger that broke); `token` the desktop's code |

Each of these answers carries `token` (the sealed layer's code in capitals, or for `E_READ_ONLY` the desktop's) and
`detail` (its evidence). Nothing is
written in any of these. The desktop, meeting the command line's turn, does not do its write either
and says so, with a key to the switches.

**Adopted ledgers.** A plain ledger folder the command line made, which the desktop has adopted ("import an
existing key", then "adopt"), is written through the desktop's sealed copy from then on. The desktop keeps, in
the machine folder, a random salt and a list with one line per adopted ledger: the SHA-256 of the salt followed by
the ledger's root (its genesis entry id). The list alone names no ledger; only the ledger itself finds its line.
Before writing a plain ledger folder, the command line looks its root up there: on the list, it answers
`E_ADOPTED` (exit 1) and writes nothing, whether "With a credential" is on or off, and wherever the folder has been
moved or renamed. The folder itself is never touched. A list or salt that does not read is refused by name
(`E_UNREADABLE`, exit 1, with its `token` and `detail`), and nothing is written past it.

**What is read.** The verbs that read a ledger (`audit`, `check-grant`, `chain-check`, `depth`, `show --entry`,
`kit-export`, `scan --ledger`) read the ledger of a home the desktop keeps with the same credential and the same
opening as a write, and so does `kit-verify --ledger` (which reads this machine's identities' keys, not the ledger),
and are refused by the same reasons: `E_CREDENTIAL_MISSING`, `E_CREDENTIAL_EXPIRED`,
`E_CREDENTIAL_SHAPE`, and `E_HOME_OTHER_MACHINE` when the home's writer mark names another machine. A read takes
no writer lock (it changes nothing; each file the desktop writes meanwhile lands whole), so the desktop writing at
that moment is no refusal. What else a read takes from the home it takes the same way: `kit-export` the originals
of the chosen records from this machine's index of signed files (none with `--originals none`), the terms documents kept for the chosen grants and
the home's anchoring point from its settings,
`scan --ledger` the chain, the registry and the start block from its settings, and with no `--endpoint` the
home's nodes for that chain, the proxy from this machine's settings (its following the system read as the desktop reads
it, never from the terminal's proxy variables) unless `--proxy` is given; `kit-export --to` the
same chain settings and nodes, and the read-only networks this machine keeps (the desktop's table, plain in the
machine folder), the proxy likewise; `kit-verify` each seat's key of each identity in the register, from the vault.

### Handing a ledger between two places

A ledger can go back and forth between two places: a plain ledger folder the command line keeps (on another
machine, say), and a home the desktop keeps here, or two plain folders. The same two verbs at both ends:

- **`hand-over --ledger <place> --out <file>`** writes the whole ledger (every entry, one line under one root) into
  one handoff file (`.zkhand`: a record package in one file, carrying the handoff's note in
  `files/zikaron-handoff.txt`), and this place stops writing it: every entry-writing verb on it answers
  `E_HANDED_OFF`, and so does the desktop for a home. A place already handed makes the same handoff again (same id,
  same entries), so a file lost on the way is made anew, never a second pen. A home is handed only past its write
  gates, judged as for a send or a write: under its read-only mark (`LEDGER_NOT_FETCHED`, `NEWER_ELSEWHERE`) or with
  a ledger whose own entries break its chain (`BROKEN`), it answers `E_READ_ONLY` with that `token`, and nothing is
  written and no pen moves.
- **`take-over --ledger <place> --handoff <file>`** takes a handoff in. It is taken only as a follow-on: every entry
  this place has must be in the handoff, and the handoff's entries must make one line; then the entries this place
  lacks are written (in sequence), and this place holds the pen. A place that handed the ledger off takes only the
  handoff that comes back after its own (its own handoff fed back to it, or an older one, is `NOT_BACK`). A plain
  folder not there yet is made. Anything else is `E_HANDOFF` with its `token`, and nothing is written.

At any time one place writes a ledger. Which ledger a place has handed off or taken is kept in that machine's
folder (a list keyed blind, `sha256(salt ‖ root id)` under the adopted list's salt; it names no ledger). A home's
side is written with the credential, the desktop locked or not; the desktop open meanwhile shows the new entries at
once (it follows its ledger on disk). Whether an entry is on chain is not carried: the desktop reads it from the
chain at its next self-audit and queues for anchoring only what that reading finds not anchored, so it never sends
again an entry the command line anchored. A plain folder keeps no queue: for it the command line sends only what
`anchor` is given (`audit` with a `scan` reading says which entries are anchored); a home's queue is sent by
`anchor --ledger` (what is sent, above).

## 13 · `mcp`: the Model Context Protocol

`zikaron mcp` lets an AI app ask the command line through the Model Context Protocol, without a terminal. It is not
a door and holds no right of its own: each tool call is a verb of this command line, run as this same program in a
process of its own with its arguments laid out as a person would type them, and its answer is that process's answer.
It takes no flag (one given is misuse, exit 2, zero bytes).

**Transport.** Standard input and standard output only, one JSON-RPC 2.0 message per line, each line ending in one
line end (section 1, rule 7). Nothing listens on a port or a local channel and nothing is written to a file; lines
for people never go to stdout. JSON-RPC's error codes are negative: an error's `code` is written with its sign, the
one number of a message outside the law's integers. When the input closes, the call being done finishes and `mcp`
exits 0.

**Versions and methods.** `2025-11-25` and `2025-06-18` are spoken: `initialize` asking for either is answered with
it, asking for another with `2025-11-25`, and the client decides whether to go on. The one capability is `tools`
(`listChanged` false); `serverInfo` is `zikaron` and this program's version. The methods are `initialize`,
`notifications/initialized`, `ping`, `tools/list`, `tools/call` and `notifications/cancelled`; any other is answered
`-32601`. A message is read by the law's reader: a number that is not one of the law's integers (a sign, a
fraction, an exponent, past 2^53 − 1), a container past depth 128 or a member given twice is answered `-32600` with
`data` naming the token and where it stands (`at`, member names and places joined by `.`); a line that is not JSON
is answered `-32700`. Members the protocol adds that this version does not know are passed over.

**Calls.** `tools/call` is done one at a time in the order calls come; `ping` and `initialize` are answered as they
come. A call is the verb its tool names with each parameter given as the flag of that name (`window-from` as
`--window-from`), a repeated flag once per value in the order given, stdin empty, the environment `mcp`'s own
(language and proxy variables with it), and no deadline. The settings, the credential's term, the language and the
proxy are read again by that process; `mcp` never opens the master key nor reads a sealed file.

**The way a write goes**, judged at each call by the same judgement as section 11's `ALONE`: this machine's folder
holds the credential (section 12), the write is `--ledger <home>/ledger`; otherwise it is `--home <home>` (section
11), which takes the flags of the verb's row there and no other: a parameter given beside `home` that the desktop's
row does not take (`kit-verify`'s `fragment` and `unavailable`) is refused by name before anything runs. A credential
that does not open or has expired, and both ways closed, are answered by the command as typed, with its code and
sentence; `mcp` adds none. `kit-verify` goes this way when given `home` (a sealed package opened by this machine's
identities, with the credential or by the desktop), and with no `home` runs as typed (a plain package needs none; a
sealed one is then the verb's misuse).

**The tools** are read off the verb table (`mcp.rs` reads `verbs::HOME` and `ACCEPTS`; the tests pin the list):

| Tool | Parameters | Writes |
|---|---|---|
| `history` | `home` `file` `note` | yes |
| `grant` | `home` `grantee` `work` `terms` `history` `window-from` `window-to` `scope` `upstream` | yes |
| `revoke` | `home` `grant` `case` | yes |
| `adopt` | `home` `anchors` `attestor` `attestation` | yes |
| `annotate` | `home` `subject` `note` | yes |
| `retract` | `home` `subject` `note` | yes |
| `kit-export` | `home` `entry` `note` `out` `as` `to` `to-key` `originals` | yes (a record package at `out`, with the chosen records' originals unless `originals` is `none`, sealed to one recipient when given `to` or `to-key`) |
| `anchor` | `home` | yes; it sends this home's queue as section 12's "putting on chain from the command line" says, spending this identity's gas when sending is automatic (`destructiveHint`, `openWorldHint`) |
| `show` | `ledger` `entry` `path` | no |
| `audit` | `ledger` `root` `input` `fragment` `unavailable` | no |
| `scan` | `home` (given as `--ledger <home>/ledger`: the home's chain settings and its nodes, section 6) | no |
| `check-grant` | `ledger` `root` `input` `fragment` `unavailable` `now` `grant` | no |
| `chain-check` | `ledger` `root` `hop` `now` `input` `fragment` `unavailable` | no |
| `depth` | `ledger` `root` `work` `input` `fragment` `unavailable` | no |
| `badge` | `decode` | no |
| `kit-verify` | `kit` `fragment` `unavailable` `originals-to` `home` | yes, always (`readOnlyHint` false; cancelled, it is let finish and answered); it writes only when given `originals-to`, the originals taken out there; `home` may be left out |
| `contract` | none | no |

No other verb is a tool: not `keygen` (it would hand a private key over), not a way that takes `--key` or
`--key-file` (`init` without a home, the direct `anchor`, `attest`, `fpm-sign`, `ack-sign`), not `succeed` (done on
the desktop by hand), not `hand-over` or `take-over`, not the desktop's fee bump; and not a write's `ledger`, `key`,
`root`, `seq`, `prev`, `endpoint` or `proxy`, nor `audit`'s `out`, `badge`'s `encode`, `scan`'s `endpoint`, `fixture`,
`basis`, `adoptions` and `proxy`, or `kit-verify`'s `ledger` and `key` (its `home` stands for the first; no private key is
ever a parameter). A parameter is a string, a repeated one a list of strings. A path (`home`, `ledger`,
`input`, `fragment`, `history`'s `file`, `adopt`'s `anchors`, `kit-export`'s `out`, `kit-verify`'s `kit` and `originals-to`, `show`'s
`path`, `check-grant`'s `grant`, each side of a `chain-check` `hop`, `badge`'s `decode`) is taken whole only. Each
tool's description is one sentence of a closed table, in the system's language (section 3), with nothing from a
ledger.

**Results.** The text is the process's stdout, the canonical JSON value as it came; `structuredContent` is
`{"answer":<that value>,"exit":<its exit code>}`. Two kinds are marked `isError`: the process exiting 2 (misuse:
stdout is empty, so the text is its two stderr lines), and a parameter `mcp` refuses itself before anything runs (a
path not whole, a parameter not the tool's, a value of the wrong kind, a write without `home` (but `kit-verify`), a
parameter the desktop's way does not take), said in the same two lines (`E_ARGS` and the parameter's flag, `--home` for a write without one, then the sentence; a
parameter not the tool's by its place and length), with `exit` 2. A third is no answer at all: the process ended without
an exit code of section 2 (a crash, a signal) or could not be started; its text is stderr (or `E_ARGS` and the sentence
that the program could not be started), and it carries no `structuredContent`. Exits
0, 1, 3 and 4 are answers, never errors (law §9.4). A tool name not in the table is answered `-32602`.

**Cancelling.** A read being done is stopped and not answered; a write being done is not stopped (a send cut halfway
cannot be undone) and is answered when done, so the client knows what was written; a call not started is not done.
The desktop's `CLOSED` is said as it is (what was started may have been done), never tried again.

**Ledger text is data.** Notes, statements, other ledgers' content and nodes' replies in an answer are passed as
they came; nothing is rewritten or picked out of them.

**For AI apps.** Settings > Command line and AI control, "Connect an AI app", copies the configuration that starts
this: `{"mcpServers":{"zikaron":{"args":["mcp"],"command":"<this machine's zikaron, its full path>"}}}`. The key is
grey where the command line cannot be reached from outside the app (a disk image opened in place, an AppImage).
