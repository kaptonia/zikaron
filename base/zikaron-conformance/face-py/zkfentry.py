"""zikaron.face/1 sections 1 to 5: the face entry, the key's consent, the
marker, and the bytes carried with an anchor.

Acceptance of a byte string as an entry (zikaron/2 s4.3), the EIP-191 digest
and the signature tests (zikaron/2 s5.3 to s5.5) are the embedded zikaron/2
core's, imported from ../impl2-py and ../impl-py; this file states what
zikaron.face/1 adds: the type literal, the seven body shapes, the declaration
and its domain, and the carried entry.
"""

import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
_UP = os.path.dirname(_HERE)
sys.path.insert(0, os.path.join(_UP, "impl2-py"))
sys.path.insert(0, os.path.join(_UP, "impl-py"))

from zkcanon import Fault                                            # noqa: E402
from zkcrypto import eip191_digest, message, sha256                  # noqa: E402
from zkentry import check_sig, is_hex, is_int                        # noqa: E402
from zk2entry import validate                                        # noqa: E402

TYPE = "zikaron.face/1"
DOMAIN = "zikaron.face/1"
MARKER_PREFIX = "zikaron:"
SELECTOR_ANCHOR = bytes.fromhex("eecdf927")
SHAPES = ("home", "account", "key", "persona", "release", "reroot", "origin")

_LABEL_CHARS = frozenset("abcdefghijklmnopqrstuvwxyz0123456789-")


# --------------------------------------------------------------------------
# s1 notation
# --------------------------------------------------------------------------

def accept(b):
    """zikaron/2 s4.3 over `b`: the Entry, or Fault with that law's token."""
    return validate(b)


def is_label(v):
    return (isinstance(v, str) and 1 <= len(v) <= 32
            and all(c in _LABEL_CHARS for c in v))


def is_handle(v):
    return (isinstance(v, str) and 1 <= len(v) <= 128
            and v[0] != " " and v[-1] != " ")


# s1: the scalar values no name carries, as closed ranges; beside them the last
# two scalar values of every plane
_BARRED = (
    (0x0000, 0x0020), (0x007F, 0x00A0), (0x00AD, 0x00AD), (0x034F, 0x034F),
    (0x0600, 0x0605), (0x061C, 0x061C), (0x06DD, 0x06DD), (0x070F, 0x070F),
    (0x0890, 0x0891), (0x08E2, 0x08E2), (0x115F, 0x1160), (0x1680, 0x1680),
    (0x17B4, 0x17B5), (0x180B, 0x180F), (0x2000, 0x200F), (0x2028, 0x202F),
    (0x205F, 0x206F), (0x2800, 0x2800), (0x3000, 0x3000), (0x3164, 0x3164),
    (0xE000, 0xF8FF), (0xFDD0, 0xFDEF), (0xFE00, 0xFE0F), (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0), (0xFFF0, 0xFFFF), (0x110BD, 0x110BD), (0x110CD, 0x110CD),
    (0x13430, 0x1343F), (0x13441, 0x13442), (0x16FE4, 0x16FE4), (0x1BC9D, 0x1BC9D),
    (0x1BCA0, 0x1BCA3), (0x1D159, 0x1D159), (0x1D173, 0x1D17A),
    (0xE0000, 0xE0FFF), (0xF0000, 0x10FFFF),
)


def _barred(o):
    return (o & 0xFFFE) == 0xFFFE or any(lo <= o <= hi for lo, hi in _BARRED)


def is_name(v):
    return (isinstance(v, str) and 1 <= len(v) <= 32
            and not any(_barred(ord(c)) for c in v))


def _hex20(v):
    return is_hex(v, 20)


def _hex32(v):
    return is_hex(v, 32)


def _hex65(v):
    return is_hex(v, 65)


def _is(literal):
    return lambda v: isinstance(v, str) and v == literal


def _string(v):
    return isinstance(v, str)


# --------------------------------------------------------------------------
# s2.2 the seven shapes, closed
# --------------------------------------------------------------------------

_ROWS = {
    "home": ((("act", _is("home")), ("root", _hex20), ("chain", is_int)), ()),
    "account": ((("act", _is("bind")), ("root", _hex20), ("kind", _is("account")),
                 ("platform", is_label), ("handle", is_handle)),
                (("note_md", _string),)),
    "key": ((("act", _is("bind")), ("root", _hex20), ("kind", _is("key")),
             ("address", _hex20), ("role", is_label), ("sig", _hex65)),
            (("note_md", _string),)),
    "persona": ((("act", _is("bind")), ("root", _hex20), ("kind", _is("persona")),
                 ("name_md", is_name)), ()),
    "release": ((("act", _is("release")), ("root", _hex20), ("subject", _hex32)),
                (("note_md", _string),)),
    "reroot": ((("act", _is("reroot")), ("root", _hex20), ("to", _hex20)),
               (("note_md", _string),)),
    "origin": ((("act", _is("origin")), ("root", _hex20), ("from", _hex20)),
               (("note_md", _string),)),
}

# s2.2: the member a reroot or an origin row holds byte-unequal to its root
_OTHER = {"reroot": "to", "origin": "from"}


def _has_shape(d, required, optional):
    names = set(k for k, _ in required)
    for k, test in required:
        if k not in d or not test(d[k]):
            return False
    for k, test in optional:
        if k in d:
            names.add(k)
            if not test(d[k]):
                return False
    return set(d.keys()) == names


def shape_of(body):
    """The shape of an entry's body (s2.2), or None.  `body` is the body of
    an accepted entry, an object by zikaron/2 s4.3 step 9."""
    d = body.dict()
    for name in SHAPES:
        required, optional = _ROWS[name]
        if _has_shape(d, required, optional):
            if name in _OTHER and d[_OTHER[name]] == d["root"]:
                return None
            return name
    return None


def face_of(shape, d):
    """s2.3: the face an account, key or persona entry names."""
    if shape == "account":
        return ("account", d["platform"], d["handle"])
    if shape == "persona":
        return ("persona", d["name_md"])
    return ("key", d["address"], d["role"])


# --------------------------------------------------------------------------
# s3 the key's consent
# --------------------------------------------------------------------------

def declaration(root, address, role):
    """s3.1: T(r, a, l)."""
    return ("zikaron.face/1\nroot:%s\naddress:%s\nrole:%s"
            % (root, address, role)).encode("ascii")


def message_of(decl):
    """s3.2: D || 0x0A || hex32(sha256(T))."""
    return message(DOMAIN, sha256(decl))


def digest_of(decl):
    return eip191_digest(message_of(decl))


def consent_holds(d):
    """s3.3 over the members of a body of the key shape."""
    digest = digest_of(declaration(d["root"], d["address"], d["role"]))
    return check_sig(digest, d["sig"], d["address"], tokens=False)


# --------------------------------------------------------------------------
# s4 the marker
# --------------------------------------------------------------------------

def marker(root):
    return MARKER_PREFIX + root


# --------------------------------------------------------------------------
# s5.1 carried
# --------------------------------------------------------------------------

def carried(c):
    """The entry c[36:] where the calldata `c` carries one, else None."""
    if len(c) <= 36 or c[:4] != SELECTOR_ANCHOR:
        return None
    tail = c[36:]
    if sha256(tail) != c[4:36]:
        return None
    try:
        return accept(tail)
    except Fault:
        return None
