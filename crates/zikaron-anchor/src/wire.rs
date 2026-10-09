//! The transport-side JSON reader: what nodes say is outside the law's §3 universe.
//!
//! The core's reader reads the §3 value domain: integers in `[0, 2^53)`, no negatives, no fractions, no
//! exponents. Nodes speak outside it (`{"code":-32000}` is how they decline), so reading them with the core's
//! reader would call a valid recording unreadable JSON. The two readers read two languages: this one never
//! produces canonical bytes (only the core does); it reads node answers, and fragments to be handed on are
//! still written by the core's `canon_bytes`.
//!
//! Numbers keep their source text ([`W::Num`]). A caller that needs an integer asks for one (`as_u64`), and
//! text that does not fit is absent. A fractional field nobody reads does not break a recording, and a
//! fractional field that is read is never silently truncated.
//!
//! Each value records its source span. The basis goes to the core as written: rewriting it would cure shape
//! faults (unsorted members, duplicate keys) that the core must see. So the basis is cut from the source by
//! its span.

/// A transport-side value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Body {
    Null,
    Bool(bool),
    /// Number source text (negatives, fractions and exponents kept as is).
    Num(String),
    Str(String),
    Arr(Vec<W>),
    Obj(Vec<(String, W)>),
}

/// A value with its span in the source.
///
/// The span is provenance, not identity: the same answer in two places of a recording is one answer. Equality
/// compares `body` only and is written by hand; a derived one would compare offsets and read two identical
/// answers as a contradiction on replay.
#[derive(Clone, Eq, Debug)]
pub struct W {
    pub body: Body,
    pub start: usize,
    pub end: usize,
}

impl PartialEq for W {
    fn eq(&self, other: &W) -> bool {
        self.body == other.body
    }
}

impl W {
    pub fn of(body: Body) -> W {
        W { body, start: 0, end: 0 }
    }
    pub fn member(&self, k: &str) -> Option<&W> {
        match &self.body {
            Body::Obj(ms) => ms.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match &self.body {
            Body::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[W]> {
        match &self.body {
            Body::Arr(a) => Some(a),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self.body, Body::Null)
    }
    /// Decimal text as u64; negatives, fractions and out-of-range values are absent.
    pub fn as_u64(&self) -> Option<u64> {
        match &self.body {
            Body::Num(t) => t.parse::<u64>().ok(),
            _ => None,
        }
    }
    /// The source bytes of this value (for taking the basis as written).
    pub fn raw<'a>(&self, src: &'a [u8]) -> &'a [u8] {
        src.get(self.start..self.end).unwrap_or(&[])
    }
}

/// Read a whole transport JSON document.
pub fn parse(b: &[u8]) -> Option<W> {
    let mut p = P { b, i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != b.len() {
        return None;
    }
    Some(v)
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Option<()> {
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Some(())
        } else {
            None
        }
    }
    fn lit(&mut self, s: &[u8]) -> Option<()> {
        if self.b.get(self.i..self.i + s.len()) == Some(s) {
            self.i += s.len();
            Some(())
        } else {
            None
        }
    }
    fn value(&mut self, depth: usize) -> Option<W> {
        if depth > 256 {
            return None;
        }
        self.ws();
        let start = self.i;
        let body = match *self.b.get(self.i)? {
            b'n' => {
                self.lit(b"null")?;
                Body::Null
            }
            b't' => {
                self.lit(b"true")?;
                Body::Bool(true)
            }
            b'f' => {
                self.lit(b"false")?;
                Body::Bool(false)
            }
            b'"' => Body::Str(self.string()?),
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.eat(b']').is_some() {
                    return Some(W { body: Body::Arr(items), start, end: self.i });
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.ws();
                    if self.eat(b',').is_some() {
                        continue;
                    }
                    self.eat(b']')?;
                    break;
                }
                Body::Arr(items)
            }
            b'{' => {
                self.i += 1;
                let mut ms: Vec<(String, W)> = Vec::new();
                self.ws();
                if self.eat(b'}').is_some() {
                    return Some(W { body: Body::Obj(ms), start, end: self.i });
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    self.eat(b':')?;
                    let v = self.value(depth + 1)?;
                    ms.push((k, v));
                    self.ws();
                    if self.eat(b',').is_some() {
                        continue;
                    }
                    self.eat(b'}')?;
                    break;
                }
                Body::Obj(ms)
            }
            _ => Body::Num(self.number()?),
        };
        Some(W { body, start, end: self.i })
    }

    fn number(&mut self) -> Option<String> {
        let start = self.i;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        let digits = |p: &mut P| {
            let s = p.i;
            while matches!(p.b.get(p.i), Some(c) if c.is_ascii_digit()) {
                p.i += 1;
            }
            p.i > s
        };
        // The integer part is `0` or starts with 1 to 9 (RFC 8259 §6): `-032005` and `03` are not numbers.
        let int_at = self.i;
        if !digits(self) {
            return None;
        }
        if self.b.get(int_at) == Some(&b'0') && self.i - int_at > 1 {
            return None;
        }
        if self.b.get(self.i) == Some(&b'.') {
            self.i += 1;
            if !digits(self) {
                return None;
            }
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return None;
            }
        }
        Some(String::from_utf8_lossy(&self.b[start..self.i]).into_owned())
    }

    fn string(&mut self) -> Option<String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let c = *self.b.get(self.i)?;
            self.i += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.b.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let ch = if (0xd800..0xdc00).contains(&hi) {
                                self.lit(b"\\u")?;
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return None;
                                }
                                char::from_u32(0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00))?
                            } else {
                                char::from_u32(hi)?
                            };
                            out.push(ch);
                        }
                        _ => return None,
                    }
                }
                c if c < 0x20 => return None,
                c => {
                    // Multi-byte UTF-8 passes through as is.
                    let n = match c {
                        0x00..=0x7f => 0,
                        0xc0..=0xdf => 1,
                        0xe0..=0xef => 2,
                        0xf0..=0xf7 => 3,
                        _ => return None,
                    };
                    let seq = self.b.get(self.i - 1..self.i + n)?;
                    out.push_str(std::str::from_utf8(seq).ok()?);
                    self.i += n;
                }
            }
        }
    }

    fn hex4(&mut self) -> Option<u32> {
        let s = self.b.get(self.i..self.i + 4)?;
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(s).ok()?, 16).ok()
    }
}

/// One answer cut down, on the wire, to the facts a decision reads: an object keeps only the named members, in
/// the order named; any other answer stands whole (as `endpoints::project` does over law values). Cut before
/// [`to_core`]: a member no decision reads (a fee history's fractional `gasUsedRatio`, a number past the law's
/// ceiling) never makes the facts it does read unreadable.
pub fn project(w: &W, facts: &[&str]) -> W {
    match &w.body {
        Body::Obj(ms) => W { body: Body::Obj(facts.iter().filter_map(|f| ms.iter().find(|(k, _)| k == f).cloned()).collect()), start: w.start, end: w.end },
        _ => w.clone(),
    }
}

/// Transport value to a law §3 value; anything outside the §3 domain (negatives, fractions, out-of-range
/// integers) gives `None`.
pub fn to_core(w: &W) -> Option<zikaron::json::Value> {
    use zikaron::json::Value;
    Some(match &w.body {
        Body::Null => Value::Null,
        Body::Bool(b) => Value::Bool(*b),
        Body::Num(t) => {
            let n: u64 = t.parse().ok()?;
            if n > zikaron::json::MAX_INT {
                return None;
            }
            Value::Int(n)
        }
        Body::Str(s) => Value::Str(s.clone()),
        Body::Arr(a) => Value::Arr(a.iter().map(to_core).collect::<Option<_>>()?),
        Body::Obj(ms) => Value::Obj(ms.iter().map(|(k, v)| Some((k.clone(), to_core(v)?))).collect::<Option<_>>()?),
    })
}

/// Law §3 value to a transport value (for asking).
pub fn from_core(v: &zikaron::json::Value) -> W {
    use zikaron::json::Value;
    W::of(match v {
        Value::Null => Body::Null,
        Value::Bool(b) => Body::Bool(*b),
        Value::Int(i) => Body::Num(i.to_string()),
        Value::Str(s) => Body::Str(s.clone()),
        Value::Arr(a) => Body::Arr(a.iter().map(from_core).collect()),
        Value::Obj(ms) => Body::Obj(ms.iter().map(|(k, x)| (k.clone(), from_core(x))).collect()),
    })
}

/// Write transport bytes (for recordings): members in key byte order, numbers as their source text.
pub fn write(w: &W) -> String {
    let mut s = String::new();
    put(w, &mut s);
    s
}

fn put(w: &W, out: &mut String) {
    match &w.body {
        Body::Null => out.push_str("null"),
        Body::Bool(true) => out.push_str("true"),
        Body::Bool(false) => out.push_str("false"),
        Body::Num(t) => out.push_str(t),
        Body::Str(s) => quote(s, out),
        Body::Arr(a) => {
            out.push('[');
            for (i, v) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                put(v, out);
            }
            out.push(']');
        }
        Body::Obj(ms) => {
            let mut idx: Vec<&(String, W)> = ms.iter().collect();
            idx.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push('{');
            for (i, (k, v)) in idx.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote(k, out);
                out.push(':');
                put(v, out);
            }
            out.push('}');
        }
    }
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
