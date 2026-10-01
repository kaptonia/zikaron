# scan-py: where the text left a choice

Independent Python implementation of the `zikaron/1` chain-facing scan
(`scan_replay.py`), written from `docs/zikaron-v1.md` sections 3, 8, 9 and
`zikaron-conformance/HARNESS.md` alone. Every place those texts admitted more
than one reading is recorded below: section, the readings, the choice taken.

## Transport spelling (fixture format, not the law)

1. **Hex quantities.** Readings: the JSON-RPC `QUANTITY` convention (minimal
   lowercase, `0x0` for zero) or a zero-padded spelling. Choice: minimal
   lowercase, `hex(n)` in Python, for `fromBlock`, `toBlock`, the
   `eth_getCode` block, and the `eth_getBlockByNumber` block.
2. **`address` array order.** Readings: the `registries` array as the basis
   declares it, or sorted/deduplicated. Choice: as declared, in order, since
   the recording compares params as JSON values and arrays are ordered.
3. **Exchange matching.** Readings: exchanges consumed once, or reusable.
   Choice: reusable; the first exchange whose `method` and `params` match
   answers, and answers again for a repeated call. Params equality is deep
   and key-order-insensitive, with `true`/`false` never equal to an integer.
4. **Recorded errors.** Readings: a recorded `error` is a transport failure,
   or a definite negative answer. Choice: transport failure. On `eth_getCode`
   that is the section 9.3 `UNPROVEN` verdict; on every other method it is a
   loud failure, exit 2, like a missing recording.

## Section 9.4 (scan basis and completeness)

5. **Basis canonicity.** The basis arrives as a parsed JSON member of the
   fixture, so the section 3.5 roundtrip test over its bytes cannot be run.
   Readings: re-serialize and test, or test the value universe through the
   forms. Choice: the forms. Every member is an `int` in [0, 2^53 − 1] (a
   JSON boolean is not an int) or a lowercase `hex20`/`hex32`, which is what a
   canonical basis of this shape can carry, and the ordering and uniqueness
   rules of 9.4 are tested on the parsed value: `chains` ascending by
   `(chainId, fromBlock)`, two objects of one chain neither overlapping nor
   adjacent with equal `registries` and `senders`; `registries` and `senders`
   bytewise ascending without repeats; `bareTx` ascending by `(chainId, tx)`;
   `adoptionChains` ascending by `chainId`. Nothing else is testable here.
6. **Absent `basis`.** Readings: harness misuse (exit 2), or not a
   `zikaron/1` basis. Choice: not a basis, so `{"ok":false,"reason":"NO_LABEL"}`
   and exit 0. A missing or unreadable *file* is still exit 2.
7. **Empty `registries` on a declared chain.** Readings: still issue
   `eth_getLogs` for every chunk with `address: []`, or issue none. Choice:
   issue none. No log emitted by a declared registry can exist when none is
   declared, so the scan surface is empty; skipping also means an absent
   recording for a degenerate filter is not a loud failure. The anchor set is
   the same under either reading.
8. **Empty `senders`.** Choice: the chunks are still queried. `senders` filters
   the logs the scan admits (9.4), never the query surface, which 9.1 defines
   by emitter and topic 0.
9. **Chunk stepping.** `fromBlock .. min(fromBlock+1999, toBlock)`, then the
   next chunk begins at the previous chunk's end plus one, so the chunks are
   disjoint and cover the inclusive range exactly.
10. **Re-checking what the filter already asked for.** A recording answers
    here, and no node enforces the filter, so the log's `address` is re-tested
    against `registries` and its `blockNumber` against `[fromBlock, toBlock]`
    before the log is read. A log outside either is silently not an anchor.
11. **One record per `(chainId, blockNumber, tx, hash)`.** Readings: reject
    a second, or collapse. Choice: collapse, as 9.4 states; the first record
    formed stands. This covers a bare calldata repeating a word and the
    (unreachable in practice) case of the registry and bare scans meeting on
    one key, where every field agrees anyway. A chain that included one
    transaction hash in two blocks would yield two records, one per block, as
    9.2 says.

## Section 9.1 (the two forms)

12. **Sender.** 9.1 defines the sender as the address recovered from the
    transaction's own signature, so the node's `from` field (on the
    transaction or the receipt) is never read. The transaction object is
    re-encoded (legacy with EIP-155, and types 1 to 4 with their access,
    blob and authorization lists), the signed bytes must hash to the hash
    the transaction was asked for by, and the signer is recovered over the
    signing payload. A node whose object does not hash to the requested hash
    is answering about other bytes: loud failure, exit 2. The recovered
    sender serves the `senders` test, the topic 1 test, the bare form's
    recipient test, the 9.3 boundaries, and the record; the receipt is
    consulted for `status` alone.
12a. **No sender.** A transaction with `r` and `s` both zero, a legacy
    transaction whose `v` is below 35 (27 and 28 name no chain; anything else
    is no signature), and a typed transaction of a kind this scanner does not
    encode (a deposit or other system type) has no sender in 9.1's grammar.
    Choice: silently not an anchor in either form, exactly as a failed form
    test; nothing is recovered and no code state is consulted.
12b. **Creation transactions.** A transaction whose `to` is null is refused
    in both forms before any recovery: the registry form has no calldata to
    run its containment test against, and the bare form excludes it by name.
13. **`blockNumber` of the record.** Readings: the log's, the receipt's, or
    the transaction's. Choice: the log's for a registry anchor (the log is
    what 9.1 tests), the transaction's for a bare anchor.
14. **`tx` of a bare record.** Readings: the transaction object's `hash`, or
    the hash the `bareTx` object names. Choice: the `bareTx` object's, which
    the basis has already form-checked as lowercase `hex32`.
15. **Case.** Every hex string the chain answers with is lowercased before
    comparison and before emission; section 1 spells `hex20` and `hex32` in
    lowercase, and a node may answer with a checksummed address.
16. **`removed` logs.** The law knows no reorg flag. Choice: never consulted.
17. **Empty data.** Tested as zero bytes after decoding, so `0x` and an
    absent `data` member both pass and one data byte fails.
18. **Calldata offsets.** The admissible offsets of a 32-byte window are the
    union of `{0, 32, 64, ...}` and `{4, 36, 68, ...}`, each restricted to
    `o + 32 <= len(calldata)`. Same helper serves 9.1 topic 2 and 9.5
    `content`.
19. **A transaction the `bareTx` array names that the chain does not carry**
    (`eth_getTransactionByHash` answers `null`). Readings: loud failure, or no
    bare anchor. Choice: no bare anchor and no record; a definite negative is
    not a transport failure (the distinction 8.6 draws), and the basis's
    `bareTx` declares which transactions were examined while asserting nothing
    about whether each one exists.
20. **A pending transaction** (`blockNumber` null). Choice: not an anchor.
    A 9.2 record needs an including block, and 9.3 needs its two boundaries.
21. **Silent versus loud.** Every failed form test of 9.1 makes the thing not
    an anchor, silently. A gap in the recording is loud: a log the recording
    produced whose transaction or receipt it does not carry is exit 2.

## Section 9.3 (codeless verdict)

22. **No short-circuit.** Readings: stop at the first boundary that shows
    code, or consult both. Choice: consult both. `VOID` requires that "both
    states were consulted"; where the other boundary cannot be consulted the
    verdict is `UNPROVEN`, so `UNPROVEN` pre-empts `VOID` and the second call
    must be made even after code is seen at the first.
23. **"Cannot be consulted."** Readings: only a recorded error, or also an
    absent recording and a `null` result. Choice: all three yield `UNPROVEN`.
24. **"Has code."** Any non-empty byte string in the `eth_getCode` answer is
    code, an EIP-7702 delegation designator (23 bytes) included; `0x` is no
    code, and an address absent from the state trie answers `0x`.
25. **Block 0.** Only `eth_getCode(sender, "0x0")` is issued; the pre-genesis
    state is not consulted and is not a failure to consult.

## Section 9.2 (the record)

26. **`blockTimestamp` for `UNPROVEN` and `VOID` records.** Readings: fetch
    the header for every record, or only for `counted` ones. Choice: every
    record; 9.2 states one record shape and 8.7 carries all three verdicts.
    A block header the recording cannot answer is therefore a loud failure,
    since 9.3 names `UNPROVEN` for the two account states and for nothing else.
27. **Header call.** `eth_getBlockByNumber(hexq(number), false)`, cached per
    `(chainId, blockNumber)`; `timestamp` read as a hex quantity.

## Section 9.5 (adoption evidence)

27a. **Sender and calldata of an evidence record.** The sender is recovered
    exactly as in item 12, so a transaction with no sender (item 12a) enters
    no evidence record and every element naming it is unproven for want of a
    record; a creation transaction (item 12b) is included like any other and
    its record carries the empty calldata `0x`, since 9.1 reads init code as
    no calldata.
28. **What establishes inclusion.** Readings: the transaction object's
    non-null `blockNumber`, or the receipt's. Choice: the transaction object
    alone, which also carries the `sender` and `calldata` the record needs;
    the receipt is never fetched for evidence.
29. **No status test.** 9.5 says "included at or below that chain's
    `throughBlock`" and never "status 1". Choice: a reverted transaction an
    adoption names still produces an evidence record. Whether its calldata
    proves the element is the [C] test of 9.5, outside this scan.
30. **An element on a chain outside `adoptionChains`.** Choice: no record and
    no RPC call; the element is unproven for want of a chain the basis does
    not reach, and section 9.5 leaves that finding to the audit.
31. **One record per `(chainId, tx)`.** An adoption naming one transaction
    twice, or two adoptions naming one transaction, yield one record; the
    transaction is fetched once.
32. **A malformed element** (`chainId` not an int, `tx` not a string). Choice:
    loud failure. Section 6.5 has already form-checked every element of a
    ledger adoption entry, so such an element means the fixture is not what
    the scan is defined over. `tx` is lowercased before use.

## Output

33. **Fragment shape.** `{"anchors":[...],"basis":<basis>,"evidence":[...]}` in
    the law's canonical form (3.4): members bytewise sorted, integers plain,
    no whitespace, no trailing newline (HARNESS.md). Anchors sorted by
    `(chainId, blockNumber, tx, hash)` with the ints numeric and the two hex
    strings bytewise; evidence by `(chainId, tx)`.
34. **`calldata` spelling.** Re-emitted as `0x` plus lowercase hex of the
    decoded bytes, so an empty calldata is `0x` whatever the recording spelled.
35. **The basis** is re-emitted canonically from the parsed value, unchanged
    in content.

## Undecided by the text, and why it does not bite here

- The law never spells a JSON-RPC method, a filter shape, or a chunk size:
  section 9 states what an anchor set must *contain*, and every access path is
  outside it. The fixture format fixes the access path for this comparison, so
  the choices in items 1 to 4 belong to the harness. The law states none of
  them, and two
  implementations that agree on them agree on bytes.
- Section 9.4 says the anchor set carries "a record for every log of 9.1's
  registry form ... in a block of that range whose transaction sender is in
  `senders`". It does not say whether a log whose transaction the chain will
  not produce is an absent anchor or an unanswerable scan. Read here as
  unanswerable: exit 2. Nothing in section 9.3 covers it, since that section's
  `UNPROVEN` is about the two account states alone.
- Section 9.1's bare form requires status 1 and therefore a receipt, while
  section 9.5 requires only inclusion and therefore none. The asymmetry is the
  text's, and it is followed as written.
