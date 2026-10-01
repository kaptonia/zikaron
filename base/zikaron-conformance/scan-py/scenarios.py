#!/usr/bin/env python3
"""Produce the chain-facing fixtures that zikaron-core/fixtures holds beyond
the first twenty-one: the log-form and status boundaries of zikaron/1 s9.1
recorded from an anvil node, hand-written recordings for boundaries no node
produces (an anchor in block 0, a legacy signature naming no chain, a
signature-less transaction, a registry-form log inside a creation, two
windows of one chain), and the basis forms of s9.4 that need no recording.

    scenarios.py            writes each fixture whose file is absent
    scenarios.py hand       the hand-written and basis fixtures only (no anvil)
    ZK_REGEN_FIXTURES=1 ... rewrites them all

Needs anvil, forge and cast on PATH. Every recorded scenario states its
expected fragment by hand before recording; the recording must reproduce it,
and the written fixture is replayed once more through scan_replay.py."""

import json
import os
import socket
import subprocess
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import scan_replay as S  # noqa: E402
import record as R  # noqa: E402

FIX = os.path.normpath(os.path.join(HERE, "..", "..", "zikaron-core", "fixtures"))
ROGUE = os.path.join(HERE, "rogue")
REGISTRY_ROOT = os.path.normpath(os.path.join(HERE, "..", "..", "zikaron-core", "contracts"))
REGEN = os.environ.get("ZK_REGEN_FIXTURES") == "1"
CHAIN = 31337
NO_LABEL = '{"ok":false,"reason":"NO_LABEL"}'

# anvil's deterministic accounts: 0 relays, 2 is the audited sender
RELAYER_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
SENDER_KEY = "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"
SENDER = "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc"
RELAYER = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
H1 = "0x" + "11" * 32
H2 = "0x" + "22" * 32


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def rpc(url, method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=body, headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        r = json.loads(resp.read().decode())
    if r.get("error"):
        raise RuntimeError("%s: %s" % (method, r["error"]))
    return r["result"]


def sh(args, check=True):
    p = subprocess.run(args, capture_output=True, text=True)
    if check and p.returncode != 0:
        raise RuntimeError("%s\n%s\n%s" % (" ".join(args), p.stdout, p.stderr))
    return p.stdout


def wait_receipt(url, tx):
    for _ in range(100):
        r = rpc(url, "eth_getTransactionReceipt", [tx])
        if r is not None:
            return r
        time.sleep(0.1)
    raise RuntimeError("no receipt for %s" % tx)


def deploy(url, key, name, root=ROGUE, src="src/Rogue.sol"):
    out = sh(["forge", "create", "--root", root, "%s:%s" % (src, name), "--rpc-url", url,
              "--private-key", key, "--broadcast", "--json"])
    return json.loads(out.strip())["deployedTo"].lower()


def send(url, key, to, sig=None, args=(), data=None, auth=None, gas=None, wait=True):
    cmd = ["cast", "send", "--rpc-url", url, "--private-key", key, "--async", to]
    if sig is not None:
        cmd += [sig] + list(args)
    elif data is not None:
        cmd += [data]
    if auth is not None:
        cmd += ["--auth", auth]
    if gas is not None:
        cmd += ["--gas-limit", str(gas)]
    tx = sh(cmd).strip().splitlines()[-1].strip()
    if not wait:
        return tx.lower(), None
    rcpt = wait_receipt(url, tx)
    return tx.lower(), rcpt


def write_fixture(name, fixture, note):
    path = os.path.join(FIX, name + ".json")
    if os.path.exists(path) and not REGEN:
        print("  kept   ", name)
        return
    replay = S.canon(S.run(fixture)).decode("utf-8")
    if replay != fixture["expected"]:
        raise RuntimeError("%s: the fixture does not replay to its own fragment" % name)
    with open(path, "w") as fh:
        json.dump(fixture, fh, indent=2, sort_keys=True)
        fh.write("\n")
    with open(os.path.join(FIX, name + ".note"), "w") as fh:
        fh.write(note.rstrip() + "\n")
    print("  wrote  ", name)


def anchors_of(fixture):
    return json.loads(fixture["expected"])["anchors"]


def chain_basis(lo, hi, registries, senders):
    return {"chains": [{"chainId": CHAIN, "fromBlock": lo, "toBlock": hi,
                        "registries": registries, "senders": senders}],
            "bareTx": [], "adoptionChains": []}


# ------------------------------------------------------------ recorded scenarios

def recorded(url):
    rogue = deploy(url, RELAYER_KEY, "Rogue")
    reverter = deploy(url, RELAYER_KEY, "Reverter")

    # 22, 23, 24: a declared registry whose logs break the form of 9.1
    for name, fn, sentence in (
        ("22_rogue_two_topics", "twoTopics(bytes32)",
         "9.1: a log from a declared registry carrying two topics (the hash travels in the data) is not an anchor, whatever its topics hold. The hash sits in the calldata at offset 4, so the log's shape is the only thing the scan objects to."),
        ("23_rogue_three_topics_with_data", "threeTopicsWithData(bytes32)",
         "9.1: a log from a declared registry with three well-formed topics and 32 data bytes is not an anchor: any data byte disqualifies it."),
        ("24_rogue_four_topics", "fourTopics(bytes32)",
         "9.1: a log from a declared registry carrying four topics and no data is not an anchor."),
    ):
        tx, rcpt = send(url, SENDER_KEY, rogue, fn, [H1])
        assert S.qty(rcpt["status"]) == 1 and len(rcpt["logs"]) == 1, name
        blk = S.qty(rcpt["blockNumber"])
        fx = R.record({"basis": chain_basis(blk, blk, [rogue], [SENDER]), "adoptions": [],
                       "rpc": {str(CHAIN): url}})
        assert anchors_of(fx) == [], name
        # the recording holds the offending log: a reader can see what was declined
        logs = [ex for ex in fx["rpc"][str(CHAIN)] if ex["method"] == "eth_getLogs"]
        assert logs and any(len(ex.get("result") or []) == 1 for ex in logs), name
        write_fixture(name, fx, sentence)

    # 47: one transaction emitting one hash twice; one record per (chainId, tx, hash)
    registry = deploy(url, RELAYER_KEY, "ZikaronRegistry", root=REGISTRY_ROOT,
                      src="src/ZikaronRegistry.sol")
    tx47, r47 = send(url, SENDER_KEY, registry, "anchorMany(bytes32[])", ["[%s,%s]" % (H1, H1)])
    assert S.qty(r47["status"]) == 1 and len(r47["logs"]) == 2
    blk47 = S.qty(r47["blockNumber"])
    fx = R.record({"basis": chain_basis(blk47, blk47, [registry], [SENDER]), "adoptions": [],
                   "rpc": {str(CHAIN): url}})
    a = anchors_of(fx)
    assert len(a) == 1 and a[0]["hash"] == H1 and a[0]["verdict"] == "counted", a
    write_fixture("47_anchor_many_same_hash_twice", fx,
                  "9.4: anchorMany with one hash twice emits two logs and yields one record, since a record is at most one per (chainId, tx, hash).")

    # 48: an adoption element named twice yields one evidence record
    ac = {"chains": [], "bareTx": [], "adoptionChains": [{"chainId": CHAIN, "throughBlock": blk47}]}
    fx = R.record({"basis": ac, "adoptions": [{"chainId": CHAIN, "tx": tx47},
                                              {"chainId": CHAIN, "tx": tx47}],
                   "rpc": {str(CHAIN): url}})
    ev = json.loads(fx["expected"])["evidence"]
    assert len(ev) == 1 and ev[0]["tx"] == tx47 and ev[0]["sender"] == SENDER, ev
    write_fixture("48_adoption_element_named_twice", fx,
                  "9.5: an adoption may name one transaction twice; the evidence set holds one record for it, and the core proves both elements from that record.")

    # 49: a chain declared with no registry has no registry-form surface
    fx = R.record({"basis": chain_basis(blk47, blk47, [], [SENDER]), "adoptions": [],
                   "rpc": {str(CHAIN): url}})
    assert anchors_of(fx) == []
    assert not any(ex["method"] == "eth_getLogs" for ex in fx["rpc"][str(CHAIN)])
    write_fixture("49_chain_without_registries", fx,
                  "9.1/9.4: the declared registry list bounds which logs are read; a chains object with an empty registries array reads no log at all, and the recording holds no eth_getLogs exchange.")

    # 50, 51: a transaction the chain does not carry
    unknown = "0x" + "ee" * 32
    fx = R.record({"basis": {"chains": [], "bareTx": [{"chainId": CHAIN, "tx": unknown}],
                             "adoptionChains": []}, "adoptions": [], "rpc": {str(CHAIN): url}})
    assert anchors_of(fx) == []
    write_fixture("50_bare_tx_unknown", fx,
                  "9.1: a bareTx element naming a transaction the chain does not carry is a definite negative; no record, and the recording holds the node's null.")
    fx = R.record({"basis": ac, "adoptions": [{"chainId": CHAIN, "tx": unknown}],
                   "rpc": {str(CHAIN): url}})
    assert json.loads(fx["expected"])["evidence"] == []
    write_fixture("51_adoption_tx_unknown", fx,
                  "9.5: an adoption element naming a transaction the chain does not carry gets no evidence record; the element is unproven for want of a record.")

    # 52, 53: a transaction that is pending and not yet included
    rpc(url, "anvil_setAutomine", [False])
    txp, _ = send(url, SENDER_KEY, SENDER, data=H2, wait=False)
    txo = rpc(url, "eth_getTransactionByHash", [txp])
    assert txo is not None and txo.get("blockNumber") is None, txo
    fx = R.record({"basis": {"chains": [], "bareTx": [{"chainId": CHAIN, "tx": txp}],
                             "adoptionChains": []}, "adoptions": [], "rpc": {str(CHAIN): url}})
    assert anchors_of(fx) == []
    write_fixture("52_bare_tx_pending", fx,
                  "9.1: a self-directed 32-byte transaction that is known to the node but not included in any block has no receipt and no status; it is not a bare anchor, and the recording holds it with a null blockNumber.")
    acp = {"chains": [], "bareTx": [], "adoptionChains": [{"chainId": CHAIN, "throughBlock": blk47 + 10}]}
    fx = R.record({"basis": acp, "adoptions": [{"chainId": CHAIN, "tx": txp}],
                   "rpc": {str(CHAIN): url}})
    assert json.loads(fx["expected"])["evidence"] == []
    write_fixture("53_adoption_tx_pending", fx,
                  "9.5: an adoption element naming a pending transaction gets no evidence record: only an included transaction is a record of the evidence set.")
    rpc(url, "anvil_setAutomine", [True])
    rpc(url, "anvil_mine", ["0x1"])
    wait_receipt(url, txp)

    # 25: a delegated sender's self-send that reverts; status 0, no bare anchor
    auth = sh(["cast", "wallet", "sign-auth", reverter, "--private-key", SENDER_KEY,
               "--rpc-url", url]).strip().splitlines()[-1].strip()
    # the relayer carries the authorization in a transaction to itself, so the
    # installing transaction touches no delegated code and succeeds
    _tx0, r0 = send(url, RELAYER_KEY, RELAYER, auth=auth, gas=100000)
    assert S.qty(r0["status"]) == 1
    code = rpc(url, "eth_getCode", [SENDER, "latest"])
    assert code.lower().startswith("0xef0100"), "delegation designator expected, got %s" % code
    tx, rcpt = send(url, SENDER_KEY, SENDER, data=H2, gas=100000)
    assert S.qty(rcpt["status"]) == 0, "the delegated self-send must revert"
    fx = R.record({"basis": {"chains": [], "bareTx": [{"chainId": CHAIN, "tx": tx}],
                             "adoptionChains": []},
                   "adoptions": [], "rpc": {str(CHAIN): url}})
    assert anchors_of(fx) == []
    write_fixture("25_bare_reverted_under_delegation", fx,
                  "9.1: a self-directed transaction of exactly 32 calldata bytes whose receipt has status 0 (the sender had delegated to code that reverts) is not a bare anchor. Status is tested before the codeless test of 9.3, so the record is absent rather than VOID.")


# ------------------------------------------------------ hand-written recordings
#
# Section 9.1 reads the sender from the transaction's own signature, so a
# hand-written transaction object must carry a real signature whose signed
# bytes hash to the transaction's hash.  These helpers sign with the same
# secp256k1 code impl-py uses and encode with the scanner's own RLP.

sys.path.insert(0, os.path.normpath(os.path.join(HERE, "..", "impl-py")))
import zkcrypto as ZC  # noqa: E402

KEY_A = int("0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d", 16)  # anvil 1
KEY_B = int(SENDER_KEY, 16)  # anvil 2
REG = "0x5fbdb2315678afecb367f032d93f642f64180aa3"
GAS = "0x30d40"


def addr_of(priv):
    return "0x" + ZC.address_of(ZC.pubkey(priv)).hex()


def hq(n):
    return hex(n)


def block_hash(n):
    return "0x" + S.keccak256(b"hand block " + str(n).encode("ascii")).hex()


def hand_tx(priv, nonce, to, data, block, kind="1559", protected=True, index=0):
    """A signed transaction object as a node answers it, with the hash the
    signed bytes carry.  `kind` is "1559" or "legacy"; a legacy transaction
    with protected=False signs without EIP-155 and names no chain."""
    data_b = S.hexbytes(data)
    to_b = b"" if to is None else S.hexbytes(to)
    if kind == "legacy":
        body = [S._rq(hq(nonce)), S._rq("0x1"), S._rq(GAS), to_b, b"", data_b]
        if protected:
            r, s, v = ZC.sign_digest(priv, S.keccak256(S.rlp(body + [S._rq(hq(CHAIN)), b"", b""])))
            v = CHAIN * 2 + 35 + (v - 27)
        else:
            r, s, v = ZC.sign_digest(priv, S.keccak256(S.rlp(body)))
        raw = S.rlp(body + [S._rq(hq(v)), S._rq(hq(r)), S._rq(hq(s))])
        txo = {"type": "0x0", "nonce": hq(nonce), "gasPrice": "0x1", "gas": GAS, "to": to,
               "value": "0x0", "input": data, "v": hq(v), "r": hq(r), "s": hq(s)}
        if protected:
            txo["chainId"] = hq(CHAIN)
    else:
        # Typed transactions: 2930 (type 1), 1559 (type 2), 4844 (type 3),
        # 7702 (type 4), each with one access-list entry so the list encoding
        # is witnessed, and the type's own tail.
        ttype = {"2930": 1, "1559": 2, "4844": 3, "7702": 4}[kind]
        al_addr = "0x" + "77" * 20
        al_key = "0x" + "01" * 32
        access = [[S.hexbytes(al_addr), [S.hexbytes(al_key)]]]
        if ttype == 1:
            fields = [S._rq(hq(CHAIN)), S._rq(hq(nonce)), S._rq("0x1"), S._rq(GAS), to_b, b"", data_b, access]
            txo = {"type": "0x1", "chainId": hq(CHAIN), "nonce": hq(nonce), "gasPrice": "0x1", "gas": GAS,
                   "to": to, "value": "0x0", "input": data,
                   "accessList": [{"address": al_addr, "storageKeys": [al_key]}]}
        else:
            fields = [S._rq(hq(CHAIN)), S._rq(hq(nonce)), S._rq("0x1"), S._rq("0x2"), S._rq(GAS),
                      to_b, b"", data_b, access]
            txo = {"type": hq(ttype), "chainId": hq(CHAIN), "nonce": hq(nonce), "maxPriorityFeePerGas": "0x1",
                   "maxFeePerGas": "0x2", "gas": GAS, "to": to, "value": "0x0", "input": data,
                   "accessList": [{"address": al_addr, "storageKeys": [al_key]}]}
            if ttype == 3:
                blob = "0x01" + "ab" * 31
                fields += [S._rq("0x1"), [S.hexbytes(blob)]]
                txo.update({"maxFeePerBlobGas": "0x1", "blobVersionedHashes": [blob]})
            elif ttype == 4:
                # One authorization: the sender designating a delegate, signed
                # by the sender itself (its signature is encoded, never read).
                auth_pre = b"\x05" + S.rlp([S._rq(hq(CHAIN)), S.hexbytes(al_addr), S._rq(hq(nonce + 1))])
                ar, as_, av = ZC.sign_digest(priv, S.keccak256(auth_pre))
                fields += [[[S._rq(hq(CHAIN)), S.hexbytes(al_addr), S._rq(hq(nonce + 1)),
                             S._rq(hq(av - 27)), S._rq(hq(ar)), S._rq(hq(as_))]]]
                txo["authorizationList"] = [{"chainId": hq(CHAIN), "address": al_addr, "nonce": hq(nonce + 1),
                                             "yParity": hq(av - 27), "r": hq(ar), "s": hq(as_)}]
        r, s, v = ZC.sign_digest(priv, S.keccak256(bytes([ttype]) + S.rlp(fields)))
        parity = v - 27
        raw = bytes([ttype]) + S.rlp(fields + [S._rq(hq(parity)), S._rq(hq(r)), S._rq(hq(s))])
        txo.update({"yParity": hq(parity), "v": hq(parity), "r": hq(r), "s": hq(s)})
    h = "0x" + S.keccak256(raw).hex()
    txo.update({"hash": h, "from": addr_of(priv), "blockHash": block_hash(block),
                "blockNumber": hq(block), "transactionIndex": hq(index)})
    return txo


def unsigned_tx(claimed_from, to, data, block, index=0):
    """A transaction with no signature (a system or deposit transaction as a
    node might answer it): type 0x7e, r = s = v = 0, and a `from` the node
    asserts.  Its hash is arbitrary, since no signed bytes exist."""
    h = "0x" + S.keccak256(b"unsigned " + data.encode("ascii")).hex()
    return {"type": "0x7e", "nonce": "0x0", "gas": GAS, "to": to, "value": "0x0",
            "input": data, "v": "0x0", "r": "0x0", "s": "0x0", "hash": h, "from": claimed_from,
            "blockHash": block_hash(block), "blockNumber": hq(block), "transactionIndex": hq(index)}


def anchored_log(emitter, sender, h, txo, log_index=0):
    return {"address": emitter, "topics": [S.ANCHORED_TOPIC0, "0x" + "00" * 12 + sender[2:], h],
            "data": "0x", "blockNumber": txo["blockNumber"], "blockHash": txo["blockHash"],
            "transactionHash": txo["hash"], "transactionIndex": txo["transactionIndex"],
            "logIndex": hq(log_index), "removed": False}


def receipt_of(txo, status=1, logs=(), contract=None):
    return {"blockHash": txo["blockHash"], "blockNumber": txo["blockNumber"],
            "contractAddress": contract, "cumulativeGasUsed": "0x5208", "effectiveGasPrice": "0x1",
            "from": txo["from"], "gasUsed": "0x5208", "logs": list(logs), "logsBloom": "0x" + "00" * 256,
            "status": hq(status), "to": txo["to"], "transactionHash": txo["hash"],
            "transactionIndex": txo["transactionIndex"], "type": txo["type"]}


def hand_rpc(windows=(), txs=(), receipts=(), blocks=(), codeless=()):
    """The recorded exchanges a hand fixture needs.  `windows` are
    (fromBlock, toBlock, registries, logs) in the chunks the scanner asks
    for; `codeless` are (address, block) pairs that answer with no code."""
    ex = [{"method": "eth_chainId", "params": [], "result": hq(CHAIN)}]
    for lo, hi, regs, logs in windows:
        ex.append({"method": "eth_getLogs",
                   "params": [{"fromBlock": hq(lo), "toBlock": hq(hi), "address": list(regs),
                               "topics": [S.ANCHORED_TOPIC0]}],
                   "result": list(logs)})
    for txo in txs:
        ex.append({"method": "eth_getTransactionByHash", "params": [txo["hash"]], "result": txo})
    for rc in receipts:
        ex.append({"method": "eth_getTransactionReceipt", "params": [rc["transactionHash"]], "result": rc})
    for n in blocks:
        ex.append({"method": "eth_getBlockByNumber", "params": [hq(n), False],
                   "result": {"number": hq(n), "timestamp": hq(0x6a000000 + n)}})
    for a, n in codeless:
        ex.append({"method": "eth_getCode", "params": [a, hq(n)], "result": "0x"})
    return {str(CHAIN): ex}


def hand_fixture(basis, rpc, adoptions=()):
    fixture = {"adoptions": list(adoptions), "basis": basis, "rpc": rpc}
    fixture["expected"] = S.canon(S.run(fixture)).decode("utf-8")
    return fixture


def evidence_of(fixture):
    return json.loads(fixture["expected"])["evidence"]


def hand_written():
    A = addr_of(KEY_A)
    B = addr_of(KEY_B)
    H = "0x" + "c0" * 32
    H2 = "0x" + "c1" * 32
    H3 = "0x" + "c2" * 32

    # 26: 9.3, block 0 has one boundary.  No node includes a transaction in
    # its genesis block, so the recording is written by hand: a bare anchor in
    # block 0 from a codeless sender, signed as a legacy EIP-155 transaction.
    txo = hand_tx(KEY_A, 0, A, H, 0, kind="legacy")
    fx = hand_fixture({"chains": [], "bareTx": [{"chainId": CHAIN, "tx": txo["hash"]}], "adoptionChains": []},
                      hand_rpc(txs=[txo], receipts=[receipt_of(txo)], blocks=[0], codeless=[(A, 0)]))
    assert anchors_of(fx) == [{"blockNumber": 0, "blockTimestamp": 0x6a000000, "chainId": CHAIN,
                               "hash": H, "sender": A, "tx": txo["hash"], "verdict": "counted"}]
    write_fixture("26_bare_in_block_zero", fx,
                  "9.3: block 0 has one boundary, so a codeless sender there is counted on that single state. Hand-written recording (a legacy EIP-155 transaction signed with a test key): no node includes a transaction in its genesis block.")

    # 54: 9.1, a legacy transaction outside EIP-155 names no chain.
    txo = hand_tx(KEY_A, 0, A, H, 2, kind="legacy", protected=False)
    fx = hand_fixture({"chains": [], "bareTx": [{"chainId": CHAIN, "tx": txo["hash"]}], "adoptionChains": []},
                      hand_rpc(txs=[txo], receipts=[receipt_of(txo)], blocks=[2], codeless=[(A, 1), (A, 2)]))
    assert anchors_of(fx) == []
    write_fixture("54_legacy_unprotected_no_anchor", fx,
                  "9.1: a self-directed transaction of exactly 32 calldata bytes whose legacy signature carries v = 27 or 28 names no chain, so it has no sender in this grammar and is not a bare anchor; the node's from field is never read.")

    # 55: 9.1, a log of the registry form emitted during a creation.
    init = H + "0x60016000f3"[2:]
    txo = hand_tx(KEY_A, 0, None, init, 2)
    log = anchored_log(REG, A, H, txo)
    rc = receipt_of(txo, logs=[log], contract="0x" + "ee" * 20)
    basis = {"chains": [{"chainId": CHAIN, "fromBlock": 0, "toBlock": 3, "registries": [REG], "senders": [A]}],
             "bareTx": [], "adoptionChains": []}
    fx = hand_fixture(basis, hand_rpc(windows=[(0, 3, [REG], [log])], txs=[txo], receipts=[rc],
                                      blocks=[2], codeless=[(A, 1), (A, 2)]))
    assert anchors_of(fx) == []
    write_fixture("55_creation_log_not_anchor", fx,
                  "9.1: a log of the registry form emitted during a contract creation (the transaction has no recipient) is not an anchor whatever its topics hold: the transaction carries init code and no calldata, so the containment test has no field to run against. The log names the sender and a word the init code carries at offset 0, and is still refused.")

    # 56: 9.4, two adjacent windows of one chain with differing senders.
    t1 = hand_tx(KEY_A, 0, REG, "0x" + "00" * 4 + H[2:], 2)
    t2 = hand_tx(KEY_B, 0, REG, "0x" + "00" * 4 + H2[2:], 4)
    t3 = hand_tx(KEY_A, 1, REG, "0x" + "00" * 4 + H3[2:], 4, index=1)
    l1 = anchored_log(REG, A, H, t1)
    l2 = anchored_log(REG, B, H2, t2)
    l3 = anchored_log(REG, A, H3, t3, log_index=1)
    basis = {"chains": [
        {"chainId": CHAIN, "fromBlock": 0, "toBlock": 2, "registries": [REG], "senders": [A]},
        {"chainId": CHAIN, "fromBlock": 3, "toBlock": 5, "registries": [REG], "senders": [B]},
    ], "bareTx": [], "adoptionChains": []}
    fx = hand_fixture(basis, hand_rpc(windows=[(0, 2, [REG], [l1]), (3, 5, [REG], [l2, l3])],
                                      txs=[t1, t2, t3], receipts=[receipt_of(t1, logs=[l1]), receipt_of(t2, logs=[l2]), receipt_of(t3, logs=[l3])],
                                      blocks=[2, 4], codeless=[(A, 1), (A, 2), (B, 3), (B, 4)]))
    assert [(a["blockNumber"], a["sender"], a["hash"]) for a in anchors_of(fx)] == [(2, A, H), (4, B, H2)]
    write_fixture("56_two_windows_adjacent_senders_differ", fx,
                  "9.4: two chains objects of one chain over adjacent windows (0..2 and 3..5) whose senders differ form a basis; each window's senders bound that window alone, so the first sender's anchor in block 2 and the second sender's in block 4 are records, and the first sender's anchor in block 4 is outside the basis.")

    # 57: 9.1, a transaction without a signature has no sender.
    u = unsigned_tx(A, REG, "0x" + "00" * 4 + H[2:], 2)
    lu = anchored_log(REG, A, H, u)
    basis = {"chains": [{"chainId": CHAIN, "fromBlock": 0, "toBlock": 3, "registries": [REG], "senders": [A]}],
             "bareTx": [], "adoptionChains": [{"chainId": CHAIN, "throughBlock": 10}]}
    fx = hand_fixture(basis, hand_rpc(windows=[(0, 3, [REG], [lu])], txs=[u], receipts=[receipt_of(u, logs=[lu])],
                                      blocks=[2], codeless=[(A, 1), (A, 2)]),
                      adoptions=[{"chainId": CHAIN, "tx": u["hash"]}])
    assert anchors_of(fx) == [] and evidence_of(fx) == []
    write_fixture("57_unsigned_tx_no_sender", fx,
                  "9.1: a transaction that carries no secp256k1 signature (a system or deposit transaction; r and s are zero and the node asserts a from field) has no sender in this grammar. A registry-form log it carries is not an anchor whatever its topics hold, and an adoption element naming it gets no evidence record, so the element is unproven.")

    # 58: 9.5, an adoption naming a creation transaction.
    txo = hand_tx(KEY_A, 0, None, init, 2)
    basis = {"chains": [], "bareTx": [], "adoptionChains": [{"chainId": CHAIN, "throughBlock": 10}]}
    fx = hand_fixture(basis, hand_rpc(txs=[txo]), adoptions=[{"chainId": CHAIN, "tx": txo["hash"]}])
    assert evidence_of(fx) == [{"calldata": "0x", "chainId": CHAIN, "sender": A, "tx": txo["hash"]}]
    write_fixture("58_adoption_names_creation", fx,
                  "9.5 with 9.1: an adoption element naming a contract creation gets an evidence record whose sender is recovered from the signature and whose calldata is empty, since init code is not calldata; the element is then unproven under 9.5 for want of the content word.")

    # 69: 9.1, a typed transaction of a kind this scanner does not encode
    # carries a signature the grammar does not read.
    u = unsigned_tx(A, A, H, 2)
    u.update({"type": "0x10", "r": "0x" + "11" * 32, "s": "0x" + "22" * 32, "v": "0x0"})
    fx = hand_fixture({"chains": [], "bareTx": [{"chainId": CHAIN, "tx": u["hash"]}], "adoptionChains": []},
                      hand_rpc(txs=[u], receipts=[receipt_of(u)], blocks=[2], codeless=[(A, 1), (A, 2)]))
    assert anchors_of(fx) == []
    write_fixture("69_unknown_tx_type_no_sender", fx,
                  "9.1: a transaction of a type this scanner does not encode (0x10) has no signing payload the scanner can rebuild, so whatever r and s it carries it has no sender in this grammar and is not a bare anchor.")

    # 59, 67, 68: 9.1, the sender of a typed transaction of types 1, 3 and 4
    # (type 2 is what anvil records) is recovered over that type's own
    # signing payload.  Each is a bare anchor of one word from a codeless key.
    for name, kind, note in [
        ("59_bare_type1_access_list", "2930", "an EIP-2930 transaction (type 1) with an access list"),
        ("67_bare_type3_blob", "4844", "an EIP-4844 transaction (type 3) with a blob versioned hash"),
        ("68_bare_type4_authorization", "7702", "an EIP-7702 transaction (type 4) carrying an authorization; the chain's answer to the codeless test, not the list, decides the verdict"),
    ]:
        txo = hand_tx(KEY_B, 3, B, H2, 6, kind=kind)
        fx = hand_fixture({"chains": [], "bareTx": [{"chainId": CHAIN, "tx": txo["hash"]}], "adoptionChains": []},
                          hand_rpc(txs=[txo], receipts=[receipt_of(txo)], blocks=[6], codeless=[(B, 5), (B, 6)]))
        assert anchors_of(fx) == [{"blockNumber": 6, "blockTimestamp": 0x6a000006, "chainId": CHAIN,
                                   "hash": H2, "sender": B, "tx": txo["hash"], "verdict": "counted"}]
        write_fixture(name, fx, "9.1: the sender of %s is recovered over that type's signing payload, and the self-directed 32-byte transaction is a bare anchor of that sender." % note)


# ---------------------------------------------------------- basis forms (9.4)

def basis_forms():
    good_chain = {"chainId": 1, "fromBlock": 0, "toBlock": 10, "registries": [], "senders": []}
    good_bare = {"chainId": 1, "tx": "0x" + "ab" * 32}
    good_ac = {"chainId": 1, "throughBlock": 10}
    base = lambda **kw: dict({"chains": [], "bareTx": [], "adoptionChains": []}, **kw)  # noqa: E731
    cases = [
        ("27_basis_not_object", [1], "the basis is not an object"),
        ("28_basis_member_set", {"chains": [], "bareTx": []}, "the basis lacks a member (adoptionChains)"),
        ("29_basis_chains_not_array", base(chains={}), "chains is not an array"),
        ("30_basis_chain_not_object", base(chains=[1]), "a chains element is not an object"),
        ("31_basis_chain_member_set", base(chains=[dict(good_chain, extra=1)]), "a chains element carries a member outside its five"),
        ("32_basis_chain_block_form", base(chains=[dict(good_chain, fromBlock="0x0")]), "fromBlock is not an int"),
        ("33_basis_chain_range", base(chains=[dict(good_chain, fromBlock=11)]), "fromBlock exceeds toBlock"),
        ("34_basis_registries_not_array", base(chains=[dict(good_chain, registries="0x00")]), "registries is not an array"),
        ("35_basis_registry_form", base(chains=[dict(good_chain, registries=["0x00"])]), "a registry is not hex20"),
        ("36_basis_chains_duplicate", base(chains=[good_chain, dict(good_chain)]), "two chains objects of one chainId cover one range (equal objects overlap, and are not ordered by (chainId, fromBlock))"),
        ("37_basis_bare_not_array", base(bareTx={}), "bareTx is not an array"),
        ("38_basis_bare_not_object", base(bareTx=["0x00"]), "a bareTx element is not an object"),
        ("39_basis_bare_member_set", base(bareTx=[{"chainId": 1}]), "a bareTx element lacks tx"),
        ("40_basis_bare_tx_form", base(bareTx=[{"chainId": 1, "tx": "0xab"}]), "a bareTx tx is not hex32"),
        ("41_basis_bare_duplicate", base(bareTx=[good_bare, dict(good_bare)]), "two bareTx elements name one (chainId, tx)"),
        ("42_basis_adoption_not_array", base(adoptionChains={}), "adoptionChains is not an array"),
        ("43_basis_adoption_not_object", base(adoptionChains=[1]), "an adoptionChains element is not an object"),
        ("44_basis_adoption_member_set", base(adoptionChains=[{"chainId": 1}]), "an adoptionChains element lacks throughBlock"),
        ("45_basis_adoption_block_form", base(adoptionChains=[{"chainId": 1, "throughBlock": "10"}]), "throughBlock is not an int"),
        ("46_basis_adoption_duplicate", base(adoptionChains=[good_ac, dict(good_ac)]), "two adoptionChains objects name one chainId"),
        ("60_basis_registries_unsorted", base(chains=[dict(good_chain, registries=["0x" + "bb" * 20, "0x" + "aa" * 20])]), "registries is not bytewise ascending"),
        ("61_basis_senders_repeated", base(chains=[dict(good_chain, senders=["0x" + "aa" * 20, "0x" + "aa" * 20])]), "senders repeats an element"),
        ("62_basis_chains_unordered", base(chains=[dict(good_chain, chainId=2), good_chain]), "chains is not ascending by (chainId, fromBlock)"),
        ("63_basis_chains_overlap", base(chains=[good_chain, dict(good_chain, fromBlock=5, toBlock=20)]), "two chains objects of one chainId have overlapping ranges"),
        ("64_basis_chains_adjacent_equal", base(chains=[good_chain, dict(good_chain, fromBlock=11, toBlock=20)]), "two chains objects of one chainId are adjacent with equal registries and senders, which one object would spell"),
        ("65_basis_bare_unordered", base(bareTx=[{"chainId": 1, "tx": "0x" + "cd" * 32}, good_bare]), "bareTx is not ascending by (chainId, tx)"),
        ("66_basis_adoption_unordered", base(adoptionChains=[{"chainId": 2, "throughBlock": 1}, good_ac]), "adoptionChains is not ascending by chainId"),
    ]
    for name, basis, why in cases:
        fixture = {"adoptions": [], "basis": basis, "expected": NO_LABEL, "rpc": {}}
        assert S.canon(S.run(fixture)).decode("utf-8") == NO_LABEL, name
        write_fixture(name, fixture, "9.4: %s; the scan has no label and consults no node." % why)


def replay_all():
    bad = 0
    for n in sorted(os.listdir(FIX)):
        if not n.endswith(".json"):
            continue
        with open(os.path.join(FIX, n)) as fh:
            fx = json.load(fh)
        got = S.canon(S.run(fx)).decode("utf-8")
        if got != fx["expected"]:
            bad += 1
            print("  REPLAY DIFFERS", n)
    print("replayed %d fixtures, %d differ" % (len([n for n in os.listdir(FIX) if n.endswith('.json')]), bad))
    return bad


def main():
    if sys.argv[1:] == ["hand"]:
        hand_written()
        basis_forms()
        return 1 if replay_all() else 0
    port = free_port()
    url = "http://127.0.0.1:%d" % port
    anvil = subprocess.Popen(["anvil", "--port", str(port), "--hardfork", "prague", "--silent"],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                if S.qty(rpc(url, "eth_chainId", [])) == CHAIN:
                    break
            except Exception:
                time.sleep(0.1)
        else:
            raise RuntimeError("anvil did not come up")
        recorded(url)
    finally:
        anvil.terminate()
        anvil.wait()
    hand_written()
    basis_forms()
    return 1 if replay_all() else 0


if __name__ == "__main__":
    sys.exit(main())
