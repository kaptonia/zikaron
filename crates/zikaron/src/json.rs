//! Canonical form (law §3): value domain, integers, the two string character sets, canonical bytes,
//! round-trip identity on acceptance. The parser is our own (no third-party JSON); faults are named by the
//! earliest triggering byte (§3.5 test 2) through one left-to-right pass that stops at the first fault.

use crate::tokens::CanonToken as Token;
use crate::trace;

/// Value domain of law §3.1: null, true, false, integers, strings, arrays, objects. No floats.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(u64),
    Str(String),
    Arr(Vec<Value>),
    /// Members keep input order, duplicates included: test 3 must see duplicates.
    Obj(Vec<(String, Value)>),
}

/// Law §3.2: integer upper bound 2^53 − 1.
pub const MAX_INT: u64 = (1u64 << 53) - 1;

/// Law §3.5 test 2: a container opened at depth 129 is E_DEPTH; the root container is depth 1.
const MAX_DEPTH: usize = 128;

impl Value {
    pub fn member(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(ms) => ms.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<u64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Arr(a) => Some(a),
            _ => None,
        }
    }
    pub fn is_obj(&self) -> bool {
        matches!(self, Value::Obj(_))
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    depth: usize,
    /// How surrogate pairs are read. Entries (law §3.3) accept no surrogate escape; the audit-input reader
    /// (HARNESS five restrictions) reads an escaped pair as the one scalar RFC 8259 says it spells, and still
    /// refuses a lone surrogate.
    pairs: bool,
}

fn utf8_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

fn hexdigit(c: u8) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as u32),
        b'a'..=b'f' => Some((c - b'a' + 10) as u32),
        b'A'..=b'F' => Some((c - b'A' + 10) as u32),
        _ => None,
    }
}

/// Law §3.2: decimal, unsigned, no leading zero, within [0, 2^53 − 1].
fn integer_of(run: &[u8]) -> Option<u64> {
    if run.is_empty() || !run.iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if run.len() > 1 && run[0] == b'0' {
        return None;
    }
    if run.len() > 16 {
        return None;
    }
    let mut v: u64 = 0;
    for c in run {
        v = v * 10 + (c - b'0') as u64;
    }
    if v > MAX_INT {
        None
    } else {
        Some(v)
    }
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(c) = self.b.get(self.i) {
            match c {
                b' ' | b'\t' | b'\n' | b'\r' => self.i += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn value(&mut self) -> Result<Value, Token> {
        match self.peek() {
            None => Err(Token::Json),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'n') => self.literal(b"null", Value::Null),
            // Law §3.1: a value starting with `-`, `+`, `.` or a digit is a number position.
            Some(b'-') | Some(b'+') | Some(b'.') | Some(b'0'..=b'9') => self.number(),
            Some(_) => Err(Token::Json),
        }
    }

    fn literal(&mut self, word: &[u8], v: Value) -> Result<Value, Token> {
        if self.b[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(Token::Json)
        }
    }

    fn number(&mut self) -> Result<Value, Token> {
        let start = self.i;
        while let Some(c) = self.b.get(self.i) {
            match c {
                b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E' => self.i += 1,
                _ => break,
            }
        }
        match integer_of(&self.b[start..self.i]) {
            Some(v) => Ok(Value::Int(v)),
            None => Err(Token::Number),
        }
    }

    fn string(&mut self) -> Result<String, Token> {
        self.i += 1;  // Opening quote.
        let mut out = String::new();
        loop {
            let c = *self.b.get(self.i).ok_or(Token::Json)?;
            match c {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    let e = *self.b.get(self.i).ok_or(Token::Json)?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let h = self.b.get(self.i..self.i + 4).ok_or(Token::Json)?;
                            let mut cp: u32 = 0;
                            for &d in h {
                                cp = cp * 16 + hexdigit(d).ok_or(Token::Json)?;
                            }
                            self.i += 4;
                            if (0xD800..=0xDFFF).contains(&cp) {
                                // Law §3.3: in an entry, a \u escape of a surrogate code point is E_JSON,
                                // whatever follows.
                                if !self.pairs {
                                    return Err(Token::Json);
                                }
                                // Audit reader: a high surrogate followed by a `\u` low surrogate is one
                                // scalar; a lone one is refused.
                                if !(0xD800..=0xDBFF).contains(&cp) {
                                    return Err(Token::Json);
                                }
                                let tail = self.b.get(self.i..self.i + 6).ok_or(Token::Json)?;
                                if tail[0] != b'\\' || tail[1] != b'u' {
                                    return Err(Token::Json);
                                }
                                let mut lo: u32 = 0;
                                for &d in &tail[2..6] {
                                    lo = lo * 16 + hexdigit(d).ok_or(Token::Json)?;
                                }
                                if !(0xDC00..=0xDFFF).contains(&lo) {
                                    return Err(Token::Json);
                                }
                                self.i += 6;
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            }
                            out.push(char::from_u32(cp).ok_or(Token::Json)?);
                        }
                        _ => return Err(Token::Json),
                    }
                }
                // RFC 8259: no unescaped U+0000-U+001F inside a string.
                0x00..=0x1f => return Err(Token::Json),
                _ => {
                    let n = utf8_len(c);
                    let chunk = self.b.get(self.i..self.i + n).ok_or(Token::Json)?;
                    let s = std::str::from_utf8(chunk).map_err(|_| Token::Utf8)?;
                    let ch = s.chars().next().ok_or(Token::Json)?;
                    out.push(ch);
                    self.i += n;
                }
            }
        }
    }

    fn object(&mut self) -> Result<Value, Token> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Token::Depth);
        }
        self.i += 1; // `{`
        let mut members: Vec<(String, Value)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Value::Obj(members));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(Token::Json);
            }
            let k = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(Token::Json);
            }
            self.i += 1;
            self.skip_ws();
            let v = self.value()?;
            members.push((k, v));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Value::Obj(members));
                }
                _ => return Err(Token::Json),
            }
        }
    }

    fn array(&mut self) -> Result<Value, Token> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Token::Depth);
        }
        self.i += 1; // `[`
        let mut items: Vec<Value> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Value::Arr(items));
        }
        loop {
            self.skip_ws();
            let v = self.value()?;
            items.push(v);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    self.depth -= 1;
                    return Ok(Value::Arr(items));
                }
                _ => return Err(Token::Json),
            }
        }
    }
}

/// Law §3.5 tests 1 and 2: valid UTF-8 and one JSON text.
pub fn parse(b: &[u8]) -> Result<Value, Token> {
    parse_with(b, false)
}

fn parse_with(b: &[u8], pairs: bool) -> Result<Value, Token> {
    trace::mark(trace::K1);
    if std::str::from_utf8(b).is_err() {
        return Err(Token::Utf8);
    }
    let mut p = Parser { b, i: 0, depth: 0, pairs };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.i != b.len() {
        return Err(Token::Json);
    }
    Ok(v)
}

/// Test 3: the decoded keys of every object at any depth are distinct. Sorted then compared with neighbours;
/// pairwise comparison would grow with input size.
fn distinct_keys(v: &Value) -> Result<(), Token> {
    match v {
        Value::Obj(ms) => {
            let mut keys: Vec<&str> = ms.iter().map(|(k, _)| k.as_str()).collect();
            keys.sort_unstable();
            for i in 1..keys.len() {
                if keys[i - 1] == keys[i] {
                    return Err(Token::DupKey);
                }
            }
            for (_, val) in ms {
                distinct_keys(val)?;
            }
            Ok(())
        }
        Value::Arr(items) => {
            for it in items {
                distinct_keys(it)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn is_skeleton(s: &str) -> bool {
    s.bytes().all(|b| (0x20..=0x7e).contains(&b))
}

/// Test 4: keys are non-empty skeleton strings (law §3.3).
fn keys_ok(v: &Value) -> Result<(), Token> {
    match v {
        Value::Obj(ms) => {
            for (k, val) in ms {
                if k.is_empty() || !is_skeleton(k) {
                    return Err(Token::KeyCharset);
                }
                keys_ok(val)?;
            }
            Ok(())
        }
        Value::Arr(items) => {
            for it in items {
                keys_ok(it)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Test 5: string values outside prose subtrees are skeleton strings (law §3.3). A member whose key is `_md`
/// or ends in `_md` opens a prose subtree; its value and every string inside are prose strings.
fn values_ok(v: &Value, prose: bool) -> Result<(), Token> {
    match v {
        Value::Str(s) => {
            if !prose && !is_skeleton(s) {
                return Err(Token::ValueCharset);
            }
            Ok(())
        }
        Value::Arr(items) => {
            for it in items {
                values_ok(it, prose)?;
            }
            Ok(())
        }
        Value::Obj(ms) => {
            for (k, val) in ms {
                values_ok(val, prose || k.ends_with("_md"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Law §3.4: canonical bytes of a value.
pub fn canon_bytes(v: &Value) -> Vec<u8> {
    trace::mark(trace::K1);
    let mut out = Vec::new();
    write_canon(v, &mut out);
    out
}

fn write_canon(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Int(i) => out.extend_from_slice(i.to_string().as_bytes()),
        Value::Str(s) => write_string(s, out),
        Value::Arr(items) => {
            out.push(b'[');
            for (n, it) in items.iter().enumerate() {
                if n > 0 {
                    out.push(b',');
                }
                write_canon(it, out);
            }
            out.push(b']');
        }
        Value::Obj(ms) => {
            let mut idx: Vec<usize> = (0..ms.len()).collect();
            // Members sorted by the byte order of their decoded UTF-8 keys (law §3.4 item 5).
            idx.sort_by(|&a, &b| ms[a].0.as_bytes().cmp(ms[b].0.as_bytes()));
            out.push(b'{');
            for (n, &k) in idx.iter().enumerate() {
                if n > 0 {
                    out.push(b',');
                }
                write_string(&ms[k].0, out);
                out.push(b':');
                write_canon(&ms[k].1, out);
            }
            out.push(b'}');
        }
    }
}

fn write_string(s: &str, out: &mut Vec<u8>) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    out.push(b'"');
    for ch in s.chars() {
        match ch {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{09}' => out.extend_from_slice(b"\\t"),
            '\u{0a}' => out.extend_from_slice(b"\\n"),
            '\u{0c}' => out.extend_from_slice(b"\\f"),
            '\u{0d}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                let v = c as u32;
                out.extend_from_slice(b"\\u00");
                out.push(DIGITS[((v >> 4) & 0xf) as usize]);
                out.push(DIGITS[(v & 0xf) as usize]);
            }
            c => {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out.push(b'"');
}

/// The HARNESS audit-input reader: RFC 8259 with exactly five restrictions: valid UTF-8; every number a §3.2
/// integer; no duplicate member names; every string decodes to scalars (lone surrogates refused, pairs read
/// as one); depth 129 unreadable. That is §3.5 tests 1 to 3 plus the pair reading, without canonical form or
/// the two character sets.
pub fn parse_tests_1_3(b: &[u8]) -> Result<Value, Token> {
    let v = parse_with(b, true)?;
    distinct_keys(&v)?;
    Ok(v)
}

/// Law §3.5 tests 1 to 5 (without round-trip identity); what `zk1 canon` judges by.
pub fn parse_tests_1_5(b: &[u8]) -> Result<Value, Token> {
    let v = parse(b)?;
    distinct_keys(&v)?;
    keys_ok(&v)?;
    values_ok(&v, false)?;
    Ok(v)
}

/// All six tests of law §3.5: acceptance means round-trip identity.
pub fn accept(b: &[u8]) -> Result<Value, Token> {
    let v = parse_tests_1_5(b)?;
    if canon_bytes(&v) != b {
        return Err(Token::NotCanonical);
    }
    Ok(v)
}
