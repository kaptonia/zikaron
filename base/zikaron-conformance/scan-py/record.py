#!/usr/bin/env python3
"""Record one scan against live nodes into a fixture of the shape
zikaron-core/fixtures/README.md describes.

    record.py <scenario.json> <out.json>

The scenario names the basis, the adoption elements to look for evidence on,
and one JSON-RPC endpoint per chainId:

    {"basis": {...}, "adoptions": [...], "rpc": {"31337": "http://127.0.0.1:8545"}}

The scan is scan_replay's own, run over a live endpoint that writes down every
exchange; the fixture's `expected` is the fragment that scan produced, and the
fixture is replayed once before it is written so that the recording is known
to reproduce it with no node."""

import json
import os
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import scan_replay as S  # noqa: E402


class Live(object):
    def __init__(self, urls):
        self.urls = {}
        for k, v in urls.items():
            self.urls[int(k)] = v
        self.log = {}
        self.seen = set()

    def _post(self, url, method, params):
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method,
                           "params": params}).encode("utf-8")
        req = urllib.request.Request(url, data=body,
                                     headers={"content-type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read().decode("utf-8"))

    def call(self, chain_id, method, params):
        url = self.urls.get(int(chain_id))
        if url is None:
            raise S.Loud("no endpoint for chain %s" % chain_id)
        resp = self._post(url, method, params)
        ex = {"method": method, "params": params}
        if resp.get("error") is not None:
            err = resp["error"]
            ex["error"] = {"code": err.get("code"), "message": err.get("message")}
        else:
            ex["result"] = resp.get("result")
        key = (str(chain_id), json.dumps(ex, sort_keys=True))
        if key not in self.seen:
            self.seen.add(key)
            self.log.setdefault(str(chain_id), []).append(ex)
        if "error" in ex:
            raise S.RpcError(ex["error"]["code"], ex["error"]["message"])
        return ex["result"]


def scan_with(basis, adoptions, rec):
    if not S.valid_basis(basis):
        return {"ok": False, "reason": "NO_LABEL"}
    sc = S.Scanner(basis, rec)
    anchors = {}
    sc.scan_registries(anchors)
    sc.scan_bare(anchors)
    evidence = sc.scan_evidence(adoptions)
    return {
        "anchors": [anchors[k] for k in sorted(anchors.keys())],
        "basis": basis,
        "evidence": [evidence[k] for k in sorted(evidence.keys())],
    }


def record(scenario):
    basis = scenario["basis"]
    adoptions = scenario.get("adoptions") or []
    live = Live(scenario["rpc"])
    # The endpoint is bound to the chain its basis names (9.4): ask each chain
    # who it is before reading anything from it.
    chain_ids = set()
    for c in basis.get("chains", []) if isinstance(basis, dict) else []:
        chain_ids.add(c.get("chainId"))
    for t in basis.get("bareTx", []) if isinstance(basis, dict) else []:
        chain_ids.add(t.get("chainId"))
    for o in basis.get("adoptionChains", []) if isinstance(basis, dict) else []:
        chain_ids.add(o.get("chainId"))
    for cid in sorted(x for x in chain_ids if isinstance(x, int)):
        if cid not in live.urls:
            continue
        got = live.call(cid, "eth_chainId", [])
        if S.qty(got) != cid:
            raise S.Loud("endpoint for chain %d answers eth_chainId %s" % (cid, got))
    answer = scan_with(basis, adoptions, live)
    fixture = {
        "adoptions": adoptions,
        "basis": basis,
        "expected": S.canon(answer).decode("utf-8"),
        "rpc": live.log,
    }
    replay = S.canon(S.run(fixture)).decode("utf-8")
    if replay != fixture["expected"]:
        raise S.Loud("the recording does not replay to the fragment it produced")
    return fixture


def main(argv):
    if len(argv) != 3:
        sys.stderr.write("usage: record.py <scenario.json> <out.json>\n")
        return 2
    with open(argv[1], "rb") as fh:
        scenario = json.loads(fh.read().decode("utf-8"))
    fixture = record(scenario)
    with open(argv[2], "w") as fh:
        json.dump(fixture, fh, indent=2, sort_keys=True)
        fh.write("\n")
    sys.stdout.write(fixture["expected"] + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
