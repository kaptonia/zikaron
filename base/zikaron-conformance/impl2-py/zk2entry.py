"""zikaron/2 sections 4, 5 and 6: envelope, signing, bodies.

The canonical form (section 3) and the curve arithmetic (section 5) are the
zikaron/1 core's own files, imported from ../impl-py; this file states what
zikaron/2 adds or changes: the spec literal, the single domain, and the
eight body tables.
"""

import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(_HERE), "impl-py"))

from zkcanon import Fault, Obj, accept, canon                       # noqa: E402
from zkcrypto import eip191_digest, message, sha256                 # noqa: E402
from zkentry import (SEVEN, SEVEN_SET, Entry, check_sig, is_hex,    # noqa: E402
                     is_int, is_prose, is_token)

SPEC = "zikaron/2"
DOMAIN_ENTRY = "zikaron/2"
KNOWN_TYPES = frozenset(("genesis", "history", "grant", "revocation",
                         "succession", "migration", "annotation", "retraction"))


# --------------------------------------------------------------------------
# s6 body tables
# --------------------------------------------------------------------------

def _bad():
    raise Fault("E_BODY_FIELD")


def _obj_rows(v, required, optional):
    """An object field whose named members carry forms; members beyond
    them are permitted (s6.10)."""
    if not isinstance(v, Obj):
        _bad()
    d = v.dict()
    for k, test in required:
        if k not in d or not test(d[k]):
            _bad()
    for k, test in optional:
        if k in d and not test(d[k]):
            _bad()
    return d


def _hex20(v):
    return is_hex(v, 20)


def _hex32(v):
    return is_hex(v, 32)


def _check_body(etype, body):
    d = body.dict()

    def req(k, test):
        if k not in d or not test(d[k]):
            _bad()

    def opt(k, test):
        if k in d and not test(d[k]):
            _bad()

    if etype == "genesis":
        req("statement_md", is_prose)
        if "continues" in d:
            _obj_rows(d["continues"], [("root", _hex20), ("entry_id", _hex32)], [])
    elif etype == "history":
        req("content", _hex32)
        if "mode" not in d:
            _bad()
        _obj_rows(d["mode"], [("mark", is_token), ("toolchain", _hex32)], [])
        opt("note_md", is_prose)
        if "for" in d:
            _obj_rows(d["for"], [("app", is_token), ("identity", _hex20), ("ref", is_token)],
                      [("seat", is_token)])
    elif etype == "grant":
        req("grantee", _hex20)
        req("work", _hex32)
        req("terms", _hex32)
        opt("history", _hex32)
        if "window" in d:
            wd = _obj_rows(d["window"], [("from", is_int), ("to", is_int)], [])
            if wd["from"] > wd["to"]:
                _bad()
        opt("scope_md", is_prose)
        opt("upstream", _hex32)
        opt("ledger", _hex20)
    elif etype == "revocation":
        req("grant", _hex32)
        opt("case", _hex32)
    elif etype == "succession":
        req("to", _hex20)
        req("kind", is_token)
        req("effective", is_int)
        req("statement_md", is_prose)
    elif etype == "migration":
        req("spec", is_token)
        req("statement_md", is_prose)
    elif etype == "annotation":
        opt("subject", _hex32)
        req("note_md", is_prose)
    elif etype == "retraction":
        req("subject", _hex32)
        opt("note_md", is_prose)
    # s6.9: an unknown type has only the s6.10 test, already met by step 9.


def entry_digest(b6):
    return eip191_digest(message(DOMAIN_ENTRY, sha256(b6)))


# --------------------------------------------------------------------------
# s4.3 the thirteen steps
# --------------------------------------------------------------------------

def validate(b):
    """Decide zikaron/2 s4.3 over the byte string `b`.  Raises Fault, or
    returns an Entry (the zikaron/1 core's record class)."""
    v = accept(b)                                                   # 1
    if not isinstance(v, Obj):                                      # 2
        raise Fault("E_ENVELOPE")
    d = v.dict()
    for k in SEVEN:                                                 # 3
        if k not in d:
            raise Fault("E_ENVELOPE_MISSING")
    for k in d:
        if k not in SEVEN_SET:
            raise Fault("E_ENVELOPE_CLOSED")
    if not (isinstance(d["spec"], str) and d["spec"] == SPEC):      # 4
        raise Fault("E_SPEC")
    if not is_token(d["entryType"]):                                # 5
        raise Fault("E_ENTRYTYPE")
    if not is_hex(d["author"], 20):                                 # 6
        raise Fault("E_AUTHOR")
    if not is_int(d["seq"]):                                        # 7
        raise Fault("E_SEQ")
    prev = d["prev"]
    if not (prev is None or is_hex(prev, 32)):                      # 8
        raise Fault("E_PREV")
    if (prev is None) != (d["seq"] == 0):
        raise Fault("E_PREV_SEQ")
    if not isinstance(d["body"], Obj):                              # 9
        raise Fault("E_BODY")
    if not is_hex(d["sig"], 65):                                    # 10
        raise Fault("E_SIG_FORM")
    if (d["entryType"] == "genesis") != (d["seq"] == 0):            # 11
        raise Fault("E_GENESIS_PLACE")
    _check_body(d["entryType"], d["body"])                          # 12
    b6 = canon(Obj([(k, val) for k, val in v.pairs if k != "sig"]))  # 13
    check_sig(entry_digest(b6), d["sig"], d["author"])
    return Entry(b, v, "0x" + sha256(b).hex(), d["entryType"], d["author"],
                 d["seq"], prev, d["body"])
