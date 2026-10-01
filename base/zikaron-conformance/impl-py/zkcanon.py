"""zikaron/1 section 3: canonical form.

A byte-level JSON reader and canonicalizer written directly against the law's
text.  The stdlib `json` module is deliberately not used here.
"""

MAX_INT = (1 << 53) - 1
MAX_DEPTH = 128


class Fault(Exception):
    """A rejection carrying exactly one token from the closed set of s10."""

    def __init__(self, token):
        super().__init__(token)
        self.token = token


class Obj:
    """A JSON object as an ordered list of (key, value) pairs.

    Pairs, not a dict, so that s3.5 test 3 (duplicate keys) can be decided.
    """

    __slots__ = ("pairs",)

    def __init__(self, pairs):
        self.pairs = list(pairs)

    def keys(self):
        return [k for k, _ in self.pairs]

    def dict(self):
        return dict(self.pairs)

    def __repr__(self):
        return "Obj(%r)" % (self.pairs,)


# --------------------------------------------------------------------------
# s3.2 integers
# --------------------------------------------------------------------------

def int_literal(run):
    """Return the integer value of `run` under s3.2, or None."""
    if not run:
        return None
    for c in run:
        if c < 0x30 or c > 0x39:
            return None
    if len(run) > 1 and run[0] == 0x30:
        return None
    if len(run) > 16:            # 2^53 - 1 has 16 decimal digits
        return None
    v = int(run)
    if v > MAX_INT:
        return None
    return v


# --------------------------------------------------------------------------
# s3.5 test 2: RFC 8259 JSON-text, with the law's E_DEPTH / E_NUMBER /
# surrogate overlays.  The parse runs strictly left to right and stops at the
# first fault, so the fault named is always the one whose triggering byte comes
# earliest in b.
# --------------------------------------------------------------------------

_NUMSTART = b"-+.0123456789"
_NUMRUN = b"0123456789+-.eE"
_HEXD = b"0123456789abcdefABCDEF"


class _P:
    __slots__ = ("b", "i", "n")

    def __init__(self, b):
        self.b = b
        self.i = 0
        self.n = len(b)

    def ws(self):
        b, n, i = self.b, self.n, self.i
        while i < n:
            c = b[i]
            if c == 0x20 or c == 0x09 or c == 0x0A or c == 0x0D:
                i += 1
            else:
                break
        self.i = i

    def text(self):
        self.ws()
        v = self.value(0)
        self.ws()
        if self.i != self.n:
            raise Fault("E_JSON")
        return v

    def value(self, depth):
        if self.i >= self.n:
            raise Fault("E_JSON")
        c = self.b[self.i]
        if c == 0x7B:                       # {
            if depth + 1 > MAX_DEPTH:
                raise Fault("E_DEPTH")
            return self.obj(depth + 1)
        if c == 0x5B:                       # [
            if depth + 1 > MAX_DEPTH:
                raise Fault("E_DEPTH")
            return self.arr(depth + 1)
        if c == 0x22:                       # "
            return self.string()
        if c == 0x74:                       # t
            return self.lit(b"true", True)
        if c == 0x66:                       # f
            return self.lit(b"false", False)
        if c == 0x6E:                       # n
            return self.lit(b"null", None)
        if c in _NUMSTART:
            return self.number()
        raise Fault("E_JSON")

    def lit(self, word, val):
        b, i, n = self.b, self.i, self.n
        for k in range(len(word)):
            if i + k >= n or b[i + k] != word[k]:
                raise Fault("E_JSON")
        self.i = i + len(word)
        return val

    def number(self):
        b, n = self.b, self.n
        start = i = self.i
        while i < n and b[i] in _NUMRUN:
            i += 1
        v = int_literal(b[start:i])
        if v is None:
            raise Fault("E_NUMBER")
        self.i = i
        return v

    def string(self):
        b, n = self.b, self.n
        i = self.i + 1
        parts = []
        while True:
            start = i
            while i < n:
                c = b[i]
                if c == 0x22 or c == 0x5C or c < 0x20:
                    break
                i += 1
            if i > start:
                # A run boundary is a byte that is ", \ or < 0x20; none of
                # those is a UTF-8 continuation byte, so no sequence is split.
                parts.append(b[start:i].decode("utf-8"))
            if i >= n:
                raise Fault("E_JSON")
            c = b[i]
            if c == 0x22:
                self.i = i + 1
                return "".join(parts)
            if c < 0x20:
                raise Fault("E_JSON")
            i += 1                          # consume the backslash
            if i >= n:
                raise Fault("E_JSON")
            e = b[i]
            if e == 0x22:
                parts.append('"'); i += 1
            elif e == 0x5C:
                parts.append("\\"); i += 1
            elif e == 0x2F:
                parts.append("/"); i += 1
            elif e == 0x62:
                parts.append("\b"); i += 1
            elif e == 0x66:
                parts.append("\f"); i += 1
            elif e == 0x6E:
                parts.append("\n"); i += 1
            elif e == 0x72:
                parts.append("\r"); i += 1
            elif e == 0x74:
                parts.append("\t"); i += 1
            elif e == 0x75:
                if i + 4 >= n:
                    raise Fault("E_JSON")
                h = b[i + 1:i + 5]
                for ch in h:
                    if ch not in _HEXD:
                        raise Fault("E_JSON")
                cp = int(h.decode("ascii"), 16)
                if 0xD800 <= cp <= 0xDFFF:
                    raise Fault("E_JSON")
                parts.append(chr(cp))
                i += 5
            else:
                raise Fault("E_JSON")

    def obj(self, depth):
        self.i += 1
        pairs = []
        self.ws()
        if self.i < self.n and self.b[self.i] == 0x7D:
            self.i += 1
            return Obj(pairs)
        while True:
            self.ws()
            if self.i >= self.n:
                raise Fault("E_JSON")
            if self.b[self.i] != 0x22:
                raise Fault("E_JSON")
            k = self.string()
            self.ws()
            if self.i >= self.n or self.b[self.i] != 0x3A:
                raise Fault("E_JSON")
            self.i += 1
            self.ws()
            v = self.value(depth)
            pairs.append((k, v))
            self.ws()
            if self.i >= self.n:
                raise Fault("E_JSON")
            c = self.b[self.i]
            if c == 0x2C:
                self.i += 1
                continue
            if c == 0x7D:
                self.i += 1
                return Obj(pairs)
            raise Fault("E_JSON")

    def arr(self, depth):
        self.i += 1
        items = []
        self.ws()
        if self.i < self.n and self.b[self.i] == 0x5D:
            self.i += 1
            return items
        while True:
            self.ws()
            items.append(self.value(depth))
            self.ws()
            if self.i >= self.n:
                raise Fault("E_JSON")
            c = self.b[self.i]
            if c == 0x2C:
                self.i += 1
                continue
            if c == 0x5D:
                self.i += 1
                return items
            raise Fault("E_JSON")


# --------------------------------------------------------------------------
# s3.5 tests 3, 4, 5
# --------------------------------------------------------------------------

def _check_dup(v):
    stack = [v]
    while stack:
        x = stack.pop()
        if isinstance(x, Obj):
            seen = set()
            for k, val in x.pairs:
                if k in seen:
                    raise Fault("E_DUP_KEY")
                seen.add(k)
                stack.append(val)
        elif isinstance(x, list):
            stack.extend(x)


def _skeleton(s):
    for ch in s:
        o = ord(ch)
        if o < 0x20 or o > 0x7E:
            return False
    return True


def _check_keys(v):
    stack = [v]
    while stack:
        x = stack.pop()
        if isinstance(x, Obj):
            for k, val in x.pairs:
                if len(k) == 0 or not _skeleton(k):
                    raise Fault("E_KEY_CHARSET")
                stack.append(val)
        elif isinstance(x, list):
            stack.extend(x)


def _check_values(v, prose):
    stack = [(v, prose)]
    while stack:
        x, p = stack.pop()
        if isinstance(x, Obj):
            for k, val in x.pairs:
                stack.append((val, p or k.endswith("_md")))
        elif isinstance(x, list):
            for e in x:
                stack.append((e, p))
        elif isinstance(x, str):
            if not p and not _skeleton(x):
                raise Fault("E_VALUE_CHARSET")


# --------------------------------------------------------------------------
# s3.4 canonical bytes
# --------------------------------------------------------------------------

_ESC = {
    0x22: b'\\"', 0x5C: b"\\\\", 0x08: b"\\b", 0x09: b"\\t",
    0x0A: b"\\n", 0x0C: b"\\f", 0x0D: b"\\r",
}


def canon_string(s):
    out = bytearray(b'"')
    for ch in s:
        o = ord(ch)
        e = _ESC.get(o)
        if e is not None:
            out += e
        elif o < 0x20:
            out += b"\\u00" + b"%02x" % o
        else:
            out += ch.encode("utf-8")
    out += b'"'
    return bytes(out)


def _emit(v, out):
    if v is None:
        out += b"null"
    elif v is True:
        out += b"true"
    elif v is False:
        out += b"false"
    elif isinstance(v, int):
        out += str(v).encode("ascii")
    elif isinstance(v, str):
        out += canon_string(v)
    elif isinstance(v, list):
        out += b"["
        first = True
        for e in v:
            if not first:
                out += b","
            first = False
            _emit(e, out)
        out += b"]"
    elif isinstance(v, Obj):
        out += b"{"
        first = True
        for k, val in sorted(v.pairs, key=lambda kv: kv[0].encode("utf-8")):
            if not first:
                out += b","
            first = False
            out += canon_string(k)
            out += b":"
            _emit(val, out)
        out += b"}"
    else:
        raise TypeError("not a value of the universe: %r" % (v,))


def canon(v):
    out = bytearray()
    _emit(v, out)
    return bytes(out)


# --------------------------------------------------------------------------
# s3.5 entry points
# --------------------------------------------------------------------------

def parse15(b):
    """s3.5 tests 1 through 5.  Returns the parsed value or raises Fault."""
    try:
        b.decode("utf-8")
    except UnicodeDecodeError:
        raise Fault("E_UTF8")
    v = _P(b).text()
    _check_dup(v)
    _check_keys(v)
    _check_values(v, False)
    return v


def accept(b):
    """s3.5 in full.  Returns the parsed value or raises Fault."""
    v = parse15(b)
    if canon(v) != b:
        raise Fault("E_NOT_CANONICAL")
    return v
