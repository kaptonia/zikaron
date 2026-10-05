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
   that path. A file is written only after the verdict says it may be.
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
   it, e.g. `mode: 缺或不成形,由 --mark 与 --toolchain(或 --file) 给`. It is not part of this
   contract: stdout and the exit code are the same with or without it.
5. **Statuses are passed through as they are.** GREEN / PARTIAL / FAIL, COMPLETE / GAPS /
   UNAVAILABLE / BROKEN_CHAIN, KIT_OK / BADGE_OK / PAIRED and every token are bytes written by the
   base layer; the command line does not merge, rewrite or translate them.
6. **`--now` injection.** Deadlines and windows are judged only by chain time and the injected now;
   without `--now` nothing is injected.

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
subject is the thing at fault and nothing else: the flag (`--seq --prev` when the fault is between
two), the value or the path. What it means for people is the second line, worded in one table in the
code (`out::Said`); the lines after the first are not part of the contract.

```
E_UNREADABLE /nowhere/at/all
读不出
```

**A sealed ledger is not read.** Every local file ZIKARON Desk keeps, ledger entries included, is sealed
(it starts with `zikaron-local/1`, `zikaron_glue::sealed`), and the command line holds no passcode. When the
ledger folder a verb reads holds a sealed file (every entry-writing verb, `kit-export`, `audit` and the verbs
that read through it when `--ledger` is a ledger folder, `show --entry` on a ledger folder), the verb stops as
misuse, exit 2 and zero bytes on stdout:

```
E_UNREADABLE <the ledger path>
已锁定:这是 ZIKARON Desk 封存的本机数据,命令行不读
```

The command line works on plain ledger folders and on the mirror bundles and record packages the app exports.

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
| `E_UNREADABLE` | A file could not be read, or the ledger is ZIKARON Desk's sealed local data (section 3) | 2 |
| `E_KEY` | The private key is not a scalar in [1, n−1] | 2 |
| `E_RANDOM` | The machine could not provide randomness | 4 |
| `E_LEDGER` | The storage layer (`zikaron-store`) refused (with its code and details) | 1 |
| `E_ENTRY` | The core (`zikaron`) refused this entry (with its token) | 1 |
| `E_TIP_ABSENT` | This lineage has no entries at all | 1 |
| `E_ENTRY_ABSENT` | A named entry was not found in the store (not the same as "the lineage is empty") | 1 |
| `E_TIP_FORKED` | More than one entry at the highest seq: the ledger has forked, and the command line does not pick one; on the `init` side, more than one distinct identity at seq 0 (several roots already exist); on any writing verb, a ledger that holds two roots (or would, with this entry) and no `--root` naming the one to write under, `--seq` / `--prev` given by hand included | 1 |
| `E_SCAN` | The anchoring layer (`zikaron-anchor`) refused to answer the scan (with its code) | 4 |
| `E_ENDPOINTS_DISAGREE` | Several endpoints returned readings that disagree | 4 |
| `E_UNREACHABLE` | The endpoint was unreachable, the send failed, or the wait timed out; for `anchor`, also every failure before the broadcast that is not a refused estimate (section 6), including `--form registry` without `--registry` | 4 |
| `E_TX_STATUS` | The transaction was included in a block but its status is not 1 | 1 |
| `E_TX_NOT_YET` | The wait ran out and the transaction is not in a block yet | 4 |
| `E_DOC` | The kit layer (`zikaron-kit`) refused this document, or the pairing did not match | 1 |
| `E_KIT` | The kit layer judged this kit invalid (with the kit law verdict) | 1 |
| `E_BADGE` | The kit layer refused to encode or decode the badge | 1 |
| `E_FRAGMENT` | The fragment lacks one of anchors / basis / evidence and cannot be assembled into an audit input | 1 |
| `E_ALREADY_ROOTED` | `init`: this ledger already has a root (the core recognises exactly one seq 0) | 1 |
| `E_NOT_EMPTY` | `init`: the ledger is not empty, and the core cannot recognise a root in it (unreadable stray files, bytes whose name fits an entry but that the core refuses, entries with no root) | 1 |
| `E_RETRACTION` | `retract`: the production rules of the deletion convention refused this entry, with the convention's `token` (`E_RETRACT_SHAPE` subject missing or not hex32, `E_RETRACT_NOT_IN_LEDGER` not on this ledger's lineage, `E_RETRACT_NOT_A_WORK` not a `history`, `E_RETRACT_REPEATED` already retracted); the ledger is left untouched | 1 |
| `E_WOULD_BREAK` | Every entry-writing verb (`init` and `retract` included): the core's offline audit of this ledger with the entry about to be written finds a chain finding of law §8.3 the ledger did not already have (a key that no longer holds the seat, a sequence gap, a broken link, an equivocation), whether hard or not; `entryId` is the entry that would have been written and `names` the findings' names; not one byte is written | 1 |
| `E_GRANT_FILE` | `check-grant`: `--grant` is a grant file (the app's single-file bundle) that does not open by the reading the app uses, with the refusing layer's `token` (the bundle shape's code, the kit law verdict, `E_GRANT_FILE_CODE` no grant code in it, a badge token, or `E_GRANT_FILE_CHAIN` with `entryId` for a hop the code names that the bundle does not carry) and `detail?`; nothing is checked | 1 |
| `E_GAS_REFUSED` | `anchor`: the endpoint answered the gas estimate with a refusal, any refusal (taken as "the call would revert"; a rate limit or another node-side reason is not told apart, and the node's words are in `detail`), or estimated it above the ceiling of 200,000 (it would run out of gas with the fee paid); nothing is broadcast | 1 |

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
| | 1 | shell | `E_ENTRY` + `token`; or `E_LEDGER` + `detail`, `names?`, `count?`; or `E_WOULD_BREAK` + `entryId`, `names`; or `E_TIP_ABSENT`, `E_TIP_FORKED` |
| `grant` | same as `history` | | |
| `revoke` | same as `history` | | |
| `adopt` | same as `history` | | |
| `succeed` | same as `history` | | |
| `annotate` | same as `history` | | |
| `retract` | 0 | shell | `entryId`, `ledger`, `seq`, `written` |
| | 1 | shell | `E_RETRACTION` + `token`; or `E_ENTRY` + `token`; or `E_LEDGER` + `detail`, `names?`, `count?`; or `E_WOULD_BREAK` + `entryId`, `names`; or `E_TIP_ABSENT`, `E_TIP_FORKED` |
| `attest` | 0 | shell | `attestation`, `attestor` |
| | 1 | shell | `E_ENTRY` + `token` |
| `anchor` | 0 | shell | `blockNumber`, `tx` |
| | 1 | shell | `E_TX_STATUS` + `state` (the on-chain status), `tx`; or `E_GAS_REFUSED` + `detail` (the endpoint's refusal in its debug form, e.g. `Node("…")`, or `<estimate> > 200000`), nothing broadcast |
| | 4 | shell | `E_TX_NOT_YET` + `count` (seconds waited), `tx`; or `E_UNREACHABLE` + `detail`, `tx` (broadcast, and the endpoint never answered the receipt question within the wait); or `E_UNREACHABLE` + `detail` only, nothing broadcast: the head or the estimate went unanswered or came back in another shape, `--form registry` was given without `--registry`, the nonce could not be read, the endpoint refused the broadcast, or its echo was not the hash signed |
| `scan` | 0 | shell | `fragment`, `singleSource`, `singleSourceChains`, `sources` |
| | 1 | base | the core's no-label answer (malformed basis) |
| | 4 | shell | `E_ENDPOINTS_DISAGREE` + `detail`, `sources`; or `E_SCAN` + `detail`, `token` |
| `audit` | 0 / 3 / 1 | base | the core's fifteen-item, sixteen-member report, or no label |
| | 1 | shell | what `--ledger` names does not read: `E_LEDGER` + `detail`, `names?`, `count?` (a ledger folder the storage layer refuses, or a mirror bundle whose manifest is not `desk-mirror` version 1, `detail` its `kind/version`); `E_KIT` + `state`, `detail?` (a record package the kit law judges invalid); without `--root`, `E_TIP_ABSENT` (no root in what was read) or `E_TIP_FORKED` (more than one root); `E_FRAGMENT` (the fragment cannot be assembled into an audit input); with `--out`, `E_LEDGER` + `detail` (`E_OCCUPIED` / `E_IO`), `path` when the input cannot land |
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
| `kit-export` | 0 | shell | `dropped` (platform junk that was dropped, each named), `entries`, `files`, `kitId`, `path`, `proofs`, `state=KIT_OK` |
| | 1 | shell | `E_KIT` + `state` (the kit law verdict), `detail` (its subject, or null when it has none); `E_LEDGER` + `detail`, `names?`, `count?` (the ledger folder does not open or read); `E_LEDGER` + `path` (the subject of a kit output failure: the `--out` path already exists, a disk operation failed, or a `--file` / `--proof` kit path is malformed or given twice); or `E_ENTRY_ABSENT` + `names` (the named entries that were not found) |
| `show` | 0 | shell | `author`, `entryId`, `entryType`, `prev`, `seq`, `value` |
| | 1 | shell | `E_ENTRY` + `token`; with `--entry` on a ledger folder, `E_LEDGER` + `detail`, `names?`, `count?` when the folder or the named entry file does not read; with `--entry` on a mirror bundle or record package, `E_ENTRY_ABSENT` + `entryId` when no entry there has that id, or the `--ledger` refusals of `audit` |

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
and `show --entry` read what `--ledger` names in one of three shapes: a ledger folder (as the store
reads it, strictly); a **mirror bundle** the app exports (its `mirror.json` names each entry in the
`entries/` room; the layout is named once in `zikaron_glue::mirror`, which the app writes by); or a
**record package** (a disclosure kit, verified by the kit law first, its `entries/` room read).
Whether each entry is an entry is the core's, in the audit. The writing verbs and `init` read ledger
folders only.

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
adoptions it was made with, and the fixture path uses those. `--basis` and `--adoptions` given together with
`--fixture` are accepted and ignored; they are read only on the `--endpoint` path.

**Endpoints.** `--endpoint` urls are `http://` or `https://`, the scheme and host read without regard
to case (`HTTPS://Node.Example` is `https://node.example`); https verifies the certificate chain and
host name with the roots compiled into the binary, the same transport the app uses. `anchor` waits for
the receipt round after round until `--wait-secs`, pausing between rounds; a round in which the
endpoint refuses is not an answer, and after the broadcast `E_UNREACHABLE` (with `tx`) comes only when the
endpoint never answered within the wait. Before the broadcast, `E_UNREACHABLE` (without `tx`, nothing
broadcast) covers every failure that is not a refused estimate: the head or the estimate unanswered or in
another shape, `--form registry` without `--registry` (reported this way, exit 4, not as misuse), a nonce that
does not read, a broadcast the endpoint refuses, and an echo that is not the hash signed.

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
ceiling of 200,000 (`zikaron_anchor::send::limit_for`, as in the app). An estimate the endpoint answers with a
refusal, whatever the reason (it is taken as "the call would revert"; a rate limit is not told apart, and the
node's words are in `detail`), or one above 200,000 is refused with `E_GAS_REFUSED` (exit 1) and nothing is
broadcast; an estimate the endpoint does not answer, or answers in another shape, or a head it will not give,
is `E_UNREACHABLE` (exit 4), nothing broadcast either. The affirmative answer's members do not change.

## 7 · Closed table of output keys

`address` `attestation` `attestor` `author` `blockNumber` `count` `detail` `doc` `docId`
`dropped` `entries` `entryId` `entryType` `files` `fragment` `index` `kitId` `ledger` `names`
`ok` `path`
`payload` `prev` `privkey` `proofs` `reason` `seq` `singleSource` `singleSourceChains` `sources`
`state` `token` `tx` `value` `written`

Thirty-five members, matching `codes::Key` entry by entry; every member in the table can actually be
printed (the closed table has no dead entries).

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

**Fifty-three flags, matching the closed table of flag names in the code (`ALL_FLAGS` plus
`MORE_FLAGS` in `verbs.rs`) entry by entry**; if the table and the code differ by one row,
`the_cli_schema_lists_every_flag_the_code_knows` in `crates/zikaron-cli/tests/cli.rs` fails.

| Flag | Value | When absent |
|---|---|---|
| `--ledger` | path to the ledger folder; for `audit` / `check-grant` / `chain-check` / `depth` / `show --entry` also a mirror bundle or a record package | misuse for the create and write-entry verbs, `kit-export` and `show --entry`; `audit` needs this or `--input` (neither is misuse, named `--ledger`); `check-grant` / `chain-check` / `depth` with neither this nor `--input` judge without an audit outcome (a `chain-check` hop may still bring its own input) |
| `--key` | private key, sixty-four hex digits (a `0x` prefix is allowed), scalar in [1, n−1] | misuse; malformed or out of range is misuse too |
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
| `--window-from` | integer | if either this or `--window-to` is present, `window` is set; with both absent there is no window |
| `--window-to` | integer | as above |
| `--scope` | prose (`scope_md` of `grant`) | member absent |
| `--upstream` | hex32 | member absent |
| `--grant` | three meanings by verb: for `revoke`, the body's `grant` (hex32); for `check-grant`, the path to a grant entry file or a grant file (the app's `.zkgrant`); for `fpm-sign`, the `grant` member (hex32) | `revoke`: left to the law; `check-grant`: misuse; `fpm-sign`: null is written |
| `--case` | hex32 (optional member of `revoke`) | member absent (constraint: `case` is optional) |
| `--anchors` | path to a JSON file (an array, read by the core's parser) | for `adopt`, the body lacks this member; left to the law; misuse for `attest` |
| `--attestor` | hex20 | member absent; pairs with `--attestation`, and giving only one is refused by the law |
| `--attestation` | hex65 | as above |
| `--author` | hex20 (the party co-signed in `attest`) | misuse |
| `--to` | hex20 (`to` of `succeed`) | the body lacks this member; left to the law |
| `--kind` | free token (`kind` of `succeed`; the shell keeps no allow-list) | the body lacks this member; left to the law |
| `--effective` | integer | the body lacks this member; left to the law |
| `--subject` | hex32 (optional member of `annotate`; for `retract`, the record being deleted) | `annotate`: member absent; `retract`: refused by the convention (`E_RETRACTION` + `E_RETRACT_SHAPE`) |
| `--endpoint` | `<chain id>=<url>`, repeatable; the url `http://` or `https://`, scheme and host in any case | `anchor` takes **exactly one** (more is misuse: this layer has no failover); `scan` takes either this or `--fixture`; none at all is misuse |
| `--form` | `registry` or `bare` | misuse; a word outside the table is misuse |
| `--registry` | hex20 registry contract address | the bare form needs none; with `--form registry`, `E_UNREACHABLE` (exit 4, `detail` only, nothing broadcast), as the code reports it today |
| `--hash` | hex32, repeatable | at least one of this and `--calldata`; both absent is misuse; with `--calldata` given, not used |
| `--calldata` | hex bytes | as above; when given, it is the transaction's data and takes precedence over `--hash` |
| `--wait-secs` | integer, seconds to wait for inclusion | ninety seconds |
| `--fixture` | path to a recording file, repeatable (one recording counts as one source) | either this or `--endpoint`; both or neither is misuse |
| `--basis` | path to a law §9.4 basis file | misuse when taking the `--endpoint` path; with `--fixture`, accepted and ignored (the recording's basis is used) |
| `--adoptions` | path to an adoptions file (`{"adoptions":[{"chainId":<int>,"tx":<hex32>},…]}`) | adoption evidence is not consulted; with `--fixture`, accepted and ignored (the recording's adoptions are used) |
| `--input` | path to a ready-made audit input file (**the raw bytes are handed to the law unchanged**) | assembled from `--ledger` and `--fragment` |
| `--fragment` | path to a scan fragment or reading file (both a fragment wrapped in `fragment` and a bare fragment are accepted) | an empty fragment is used (an offline reading of the lineage) |
| `--unavailable` | hex32, repeatable (the fourth member of the audit input) | empty |
| `--now` | integer (the injected now) | not injected; only chain time is used (general rule 6) |
| `--hop` | `<grant file>` or `<grant file>=<audit input file>`, repeatable | none at all is an empty chain, judged FAIL, exit 1 |
| `--entry` | entry id (bare sixty-four digits or with `0x`; the form follows the storage layer's naming rule), repeatable for `kit-export` | `kit-export`: all entries are selected; `show`: either this or `--path` |
| `--path` | path to a file (`show` reads its bytes directly) | either this or `--entry`; both or neither is misuse |
| `--out` | output path | misuse for `kit-export`; for `fpm-sign` / `ack-sign`, the result is only printed, not written; for `audit`, the audit input is not written |
| `--file` | two meanings by verb: for `kit-export`, `<path in kit>=<path on disk>`, repeatable, the path in kit relative to the kit's `files/` room (`a/b.pdf` lands at `files/a/b.pdf`); for `history`, the path of one file to record by the record convention (below) | `kit-export`: no files attached; `history`: `--content`, `--mark` and `--toolchain` as given |
| `--proof` | `<path in kit>=<tx>=<path on disk>`, repeatable; the path in kit is relative to the kit's `proofs/` room | no proof bundles attached |
| `--variant` | hex32 (`variant` of `ack-sign`) | empty string, refused by the kit check |
| `--fpm` | hex32 document id | **exactly one** of this and `--fpm-doc`; both or neither is misuse |
| `--fpm-doc` | path to a fingerprint manifest file (when given, pairing is done on the spot) | as above |
| `--rows` | path to a JSON file (an array) | empty array, refused by the kit check |
| `--encode` | path to an entry file, repeatable | **exactly one** of this and `--decode`; both or neither is misuse |
| `--decode` | path to a badge payload file | as above |

### Flags by verb

Each verb's closed list, as its `a.close(…)` in `verbs.rs` names it. The five flags `ledger`, `key`,
`root`, `seq`, `prev` are the shared list of the entry-writing verbs (`WRITE`).

| Verb | Flags it accepts |
|---|---|
| `keygen` | none |
| `init` | `ledger` `key` `statement` |
| `history` | `ledger` `key` `root` `seq` `prev` `content` `mark` `toolchain` `note` `file` |
| `grant` | `ledger` `key` `root` `seq` `prev` `grantee` `work` `terms` `history` `window-from` `window-to` `scope` `upstream` |
| `revoke` | `ledger` `key` `root` `seq` `prev` `grant` `case` |
| `adopt` | `ledger` `key` `root` `seq` `prev` `anchors` `attestor` `attestation` |
| `attest` | `key` `author` `anchors` `prev` |
| `succeed` | `ledger` `key` `root` `seq` `prev` `to` `kind` `effective` `statement` |
| `annotate` | `ledger` `key` `root` `seq` `prev` `subject` `note` |
| `retract` | `ledger` `key` `root` `seq` `prev` `subject` `note` |
| `anchor` | `key` `endpoint` `form` `registry` `hash` `calldata` `wait-secs` |
| `scan` | `endpoint` `fixture` `basis` `adoptions` |
| `audit` | `ledger` `root` `input` `fragment` `unavailable` `out` |
| `check-grant` | `ledger` `root` `input` `fragment` `unavailable` `now` `grant` |
| `chain-check` | `ledger` `root` `hop` `now` `input` `fragment` `unavailable` |
| `depth` | `ledger` `root` `work` `input` `fragment` `unavailable` |
| `fpm-sign` | `key` `work` `grant` `note` `rows` `out` |
| `ack-sign` | `key` `note` `fpm` `fpm-doc` `variant` `out` |
| `badge` | `encode` `decode` |
| `kit-export` | `ledger` `root` `note` `out` `entry` `file` `proof` |
| `show` | `ledger` `entry` `path` |

## 9 · Environment variables

The command line reads these four from its environment.

| Variable | Effect | Default |
|---|---|---|
| `ZKA_TIMEOUT_SECS` | Total wall time of one HTTP/HTTPS exchange with a node, in seconds; `0` means no deadline | 30 |
| `ZKA_TIMEOUT_MS` | The same deadline in milliseconds; when set and readable it overrides `ZKA_TIMEOUT_SECS` | unset |
| `ZKA_MAX_ANSWER_BYTES` | The most bytes one node answer may have; `0` means no cap | 67108864 (64 MiB) |
| `ZIKARON_TRACE` | A file path: when set, diagnostic trace marks are appended there (capped at 8,388,608 bytes); stdout and the exit code are unchanged | unset (no trace) |

For the three `ZKA_` variables, a value that does not parse as a whole number is ignored and the default holds (`crates/zikaron-net/src/lib.rs`,
`Limits::from_env`; `crates/zikaron/src/trace.rs`).
