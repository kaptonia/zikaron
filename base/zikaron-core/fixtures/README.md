# Chain-facing fixture corpus

Each `<nn>_<name>.json` is one scan's whole conversation with a node, together
with the canonical fragment that conversation produced. The sibling `<nn>_<name>.note` says which sentence
of `docs/zikaron-v1.md` the scenario is about.

The corpus is closed: nothing in it needs a chain. A reader who holds these
files can reproduce every fragment offline, byte for byte, with no node, no
network, and no anvil.

## Fixture shape

```
{
  "basis":      the section 9.4 object the scan ran under, as written
  "adoptions":  the array of {chainId, tx} elements the scan looked for evidence on
  "rpc":        { "<chainId>": [ {method, params, result|error}, ... ] }
  "expected":   the canonical fragment string: {"anchors":[...],"basis":{...},"evidence":[...]}
}
```

The scanner reads no `from` field: section 9.1 defines the sender as the
address recovered from the transaction's own signature, so every recorded
transaction object carries a real signature, the scanner re-encodes it, checks
that the signed bytes hash to the transaction's hash, and recovers the signer.
The hand-written recordings are signed with anvil's published test keys.

An exchange the recording does not hold is an error on replay and never a
guess, so a fixture with a hole fails loudly instead of quietly answering
something else. Errors the node returned are recorded as errors, which is how
fixture 12 replays a node that will not serve historical state.

## The scenarios

| Fixture | Section | What it holds |
| --- | --- | --- |
| `01_registry_anchor` | 9.1, 9.3 | One registry anchor from a declared sender through a declared registry. One record, verdict `counted`. |
| `02_anchor_many_three` | 9.1 | `anchorMany` with three hashes: one log per element, three records sharing one transaction. |
| `03_bare_two_words` | 9.1 | Bare form, recipient equal to sender, calldata exactly 2 × 32 bytes. Two records, no contract anywhere. |
| `04_foreign_sender_declared` | 9.4 | A sender outside the audited lineage whose address the basis names. Recorded; section 8.1 is what later trims it. |
| `05_foreign_sender_absent` | 9.4 | The same chain under a basis whose `senders` omits that address. No record, and the basis carries that silence. |
| `06_reverted_call` | 9.1 | A contract calls the registry and reverts. Status 0, no log, no record. |
| `07_undeclared_emitter` | 9.1 | Two identical registries, one declared. The undeclared contract's well-formed log is never read. |
| `08_derived_topic_not_in_calldata` | 9.1 | A declared contract emits `Anchored(msg.sender, keccak256(abi.encode(x)))`. Topic 2 is nowhere in the calldata, so it is no anchor. |
| `09_delegation_void` | 9.3, 5.7 | The sender installed an EIP-7702 delegation designator before anchoring. Verdict `VOID`. |
| `10_delegation_revoked_counted` | 9.3 | Installed and revoked in earlier blocks. Both boundaries codeless, verdict `counted`. |
| `11_delegation_in_anchor_block` | 9.3 | Codeless at the block before, designator installed inside the anchor's own block. Verdict `VOID` on the second boundary alone. |
| `12_state_not_served_unproven` | 9.3 | The endpoint refuses `eth_getCode`. Verdict `UNPROVEN`, and the recording carries the node's error verbatim. |
| `13_adoption_offsets` | 9.5 | Six evidence records carrying a 32-byte content at offsets 0, 4, 32, 36, at 5, and truncated. The core proves the first four. |
| `14_adoption_chain_absent` | 9.5 | An element on a chain no `adoptionChains` object names. No evidence record. |
| `15_adoption_above_through_block` | 9.4 | An included transaction above the chain's `throughBlock`. No evidence record. |
| `16_two_chains_same_hash` | 9.2, 9.4 | One hash anchored on chain 31337 and on chain 8453 at different timestamps. Two records, one exchange log per chainId; the basis lists the chains in ascending order as 9.4 requires. |
| `17_bare_not_multiple_of_32` | 9.1 | A self-directed transaction carrying 33 bytes, named in `bareTx`. No record. |
| `18_bare_to_not_sender` | 9.1 | Exactly 32 bytes of calldata sent to another address, named in `bareTx`. No record. |
| `19_contract_creation` | 9.1 | A contract creation named in `bareTx`. No recipient, so no bare anchor. |
| `20_contract_caller_topic1` | 9.1 | A contract calls the declared registry, so topic 1 is the contract. The hash does sit in the calldata; the log is still no anchor. |
| `21_anchor_many_skewed_offset` | 9.1 | A non-canonical `anchorMany` encoding places the element at offset 101. The registry emits; the law declines to read the hash there. |
| `22_rogue_two_topics` | 9.1 | A declared registry emits a log with the `Anchored` topic 0 and only two topics, the hash in the data. Not an anchor. |
| `23_rogue_three_topics_with_data` | 9.1 | Three well-formed topics and 32 data bytes from a declared registry. Any data byte disqualifies the log. |
| `24_rogue_four_topics` | 9.1 | Four topics and no data from a declared registry. Not an anchor. |
| `25_bare_reverted_under_delegation` | 9.1, 9.3 | The sender delegated (EIP-7702) to code that reverts, then sent itself 32 bytes. Status 0, so no record at all; status is tested before the codeless test. |
| `26_bare_in_block_zero` | 9.3 | Hand-written: a bare anchor whose legacy EIP-155 transaction sits in block 0, where the codeless test has one boundary. Verdict `counted`. |
| `27` to `46`, `_basis_*` | 9.4 | Twenty bases that fail their form, one clause each: member sets, arities, integer and hex forms, `fromBlock` above `toBlock`, two equal `chains` objects (overlapping and unordered), duplicate transaction and adoption-chain identities. Each yields `{"ok":false,"reason":"NO_LABEL"}` with no exchange. |
| `47_anchor_many_same_hash_twice` | 9.4 | `anchorMany` with one hash twice: two logs, one record per `(chainId, blockNumber, tx, hash)`. |
| `48_adoption_element_named_twice` | 9.5 | An adoption element named twice in the elements looked for; one evidence record. |
| `49_chain_without_registries` | 9.1, 9.4 | A chains object whose `registries` is empty. No `eth_getLogs` exchange, no record. |
| `50_bare_tx_unknown` | 9.1 | `bareTx` names a transaction the chain does not carry. The node's null is recorded; no record. |
| `51_adoption_tx_unknown` | 9.5 | An adoption element names a transaction the chain does not carry. No evidence record. |
| `52_bare_tx_pending` | 9.1 | A self-directed 32-byte transaction known to the node and not yet included. No receipt, no status, no record. |
| `53_adoption_tx_pending` | 9.5 | An adoption element names a pending transaction. No evidence record. |
| `54_legacy_unprotected_no_anchor` | 9.1 | Hand-written: a self-directed 32-byte legacy transaction signed without EIP-155 (`v` = 27 or 28). Its signature names no chain, so it has no sender and no record. |
| `55_creation_log_not_anchor` | 9.1 | Hand-written: a registry-form log emitted during a contract creation, naming the sender and a word the init code carries. Init code is not calldata; no record. |
| `56_two_windows_adjacent_senders_differ` | 9.4 | Hand-written: two `chains` objects of one chain over blocks 0..2 and 3..5 with different `senders`. Each window's senders bound that window alone: two records, and the first sender's anchor in the second window is outside the basis. |
| `57_unsigned_tx_no_sender` | 9.1, 9.5 | Hand-written: a transaction with no signature (a system type, `r` = `s` = 0, the node asserting a `from`). Its registry-form log is no anchor and an adoption element naming it gets no evidence record. |
| `58_adoption_names_creation` | 9.5 | Hand-written: an adoption element names a contract creation. One evidence record with the recovered sender and the empty calldata `0x`. |
| `59_bare_type1_access_list` | 9.1 | Hand-written: an EIP-2930 transaction with an access list, self-directed, 32 bytes. The sender is recovered over the type 1 payload; one record, `counted`. |
| `60` to `66`, `_basis_*` | 9.4 | Seven bases that fail the ordering and uniqueness rules: `registries` unsorted, `senders` repeated, `chains` unordered, two windows of one chain overlapping, two adjacent windows with equal `registries` and `senders`, `bareTx` unordered, `adoptionChains` unordered. Each yields the no-label answer with no exchange. |
| `67_bare_type3_blob` | 9.1 | Hand-written: an EIP-4844 transaction with a blob versioned hash, self-directed, 32 bytes. Recovered over the type 3 payload; one record, `counted`. |
| `68_bare_type4_authorization` | 9.1, 9.3 | Hand-written: an EIP-7702 transaction carrying one authorization, self-directed, 32 bytes. Recovered over the type 4 payload; the codeless answers, never the list, give the verdict `counted`. |
| `69_unknown_tx_type_no_sender` | 9.1 | Hand-written: a transaction of type 0x10 carrying nonzero `r` and `s`. No signing payload the scanner can rebuild, so no sender and no record. |

Scenarios that need no recording, the twenty-seven basis forms, are fixtures
all the same: their `rpc` member is empty and their fragment is the no-label
answer. Ten recordings (26, 54 to 59, 67 to 69) are ones no node produced:
they are written by hand, signed with test keys, and their notes say so. The
proof kits of section 9.7 are outside this corpus: they are read against
trusted block hashes, which a recording cannot supply.

## Regenerating

The corpus is produced by `zikaron-conformance/scan-py/scenarios.py`, which
starts one anvil on a free port with `--hardfork prague`, deploys the registry
of `zikaron-core/contracts` and the rogue contracts of
`zikaron-conformance/scan-py/rogue`, drives the transactions with `cast`, and
records each scan through `scan-py/record.py` over `scan_replay.py`'s own
scanner. Every recorded scenario states its expected fragment by hand before
recording; the recording must reproduce it, and the written fixture is
replayed once more with no node before it lands. `anvil`, `forge` and `cast`
must be on `PATH`. `scenarios.py hand` writes the hand-written and basis
fixtures alone and needs no node.

Fixtures 1 to 21 were recorded by an earlier scanner whose sources left the
tree (last present at commit 1ea0f11); `scan_replay.py` reproduces every one
of them byte for byte, which is what makes them a reference rather than that
scanner's memory.

Block numbers, block hashes, timestamps, transaction hashes, and deployed
addresses all move when a scenario is re-recorded. A routine run writes only
the fixtures whose files are absent; set `ZK_REGEN_FIXTURES=1` to rewrite the
recorded ones deliberately. To check the corpus without regenerating it:

```
for f in zikaron-core/fixtures/*.json; do
  python3 zikaron-conformance/scan-py/scan_replay.py "$f" > /tmp/got
  python3 -c "import json,sys; f=json.load(open('$f')); sys.exit(open('/tmp/got').read()!=f['expected'])" || echo "DIFFERS $f"
done
```
