//! RLP, Ethereum's byte layer (no third-party chain crates).
//!
//! It encodes values (anchoring and transaction-hash recomputation need it) and decodes bytes (headers and
//! MPT proof nodes need it). It has no meaning beyond length prefixes: addresses, quantities and lists are
//! byte strings here.

/// One decoded RLP value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Item {
    Bytes(Vec<u8>),
    List(Vec<Item>),
}

impl Item {
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Item::Bytes(b) => Some(b),
            Item::List(_) => None,
        }
    }
    pub fn list(&self) -> Option<&[Item]> {
        match self {
            Item::List(v) => Some(v),
            Item::Bytes(_) => None,
        }
    }
    /// A quantity: a big-endian byte string without leading zeros read as u64; over eight bytes gives `None`.
    pub fn u64(&self) -> Option<u64> {
        let b = self.bytes()?;
        if b.len() > 8 || (b.len() > 1 && b[0] == 0) {
            return None;
        }
        let mut x = 0u64;
        for byte in b {
            x = (x << 8) | *byte as u64;
        }
        Some(x)
    }
}

/// RLP encoding of a byte string.
pub fn bytes(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len() + 9);
    if b.len() == 1 && b[0] < 0x80 {
        out.push(b[0]);
    } else if b.len() < 56 {
        out.push(0x80 + b.len() as u8);
        out.extend_from_slice(b);
    } else {
        let l = be(b.len() as u64);
        out.push(0xb7 + l.len() as u8);
        out.extend_from_slice(&l);
        out.extend_from_slice(b);
    }
    out
}

/// RLP list of already encoded items.
pub fn list(items: &[Vec<u8>]) -> Vec<u8> {
    let n: usize = items.iter().map(|x| x.len()).sum();
    let mut out = Vec::with_capacity(n + 9);
    if n < 56 {
        out.push(0xc0 + n as u8);
    } else {
        let l = be(n as u64);
        out.push(0xf7 + l.len() as u8);
        out.extend_from_slice(&l);
    }
    for i in items {
        out.extend_from_slice(i);
    }
    out
}

/// RLP encoding of a quantity: big-endian, no leading zeros, zero as the empty string.
pub fn quantity(x: u64) -> Vec<u8> {
    bytes(&be(x))
}

/// RLP encoding of a big number already given as big-endian bytes without leading zeros.
pub fn scalar(b: &[u8]) -> Vec<u8> {
    let mut i = 0;
    while i < b.len() && b[i] == 0 {
        i += 1;
    }
    bytes(&b[i..])
}

fn be(x: u64) -> Vec<u8> {
    if x == 0 {
        return Vec::new();
    }
    let b = x.to_be_bytes();
    let mut i = 0;
    while b[i] == 0 {
        i += 1;
    }
    b[i..].to_vec()
}

/// Nesting bound. RLP bytes come from others (kits, recordings, node answers) and decoding is mutually
/// recursive: without a bound, fifty thousand nested empty lists would overflow the offline verifier's stack.
/// 128 levels hold everything this grammar reads (the deepest is the receipt log list, four levels).
pub const MAX_DEPTH: usize = 128;

/// Decode one RLP value and the number of bytes it used.
pub fn decode(b: &[u8]) -> Option<(Item, usize)> {
    decode_at(b, 0)
}

fn decode_at(b: &[u8], depth: usize) -> Option<(Item, usize)> {
    if depth > MAX_DEPTH {
        return None;
    }
    let first = *b.first()?;
    match first {
        0x00..=0x7f => Some((Item::Bytes(vec![first]), 1)),
        0x80..=0xb7 => {
            let n = (first - 0x80) as usize;
            let body = b.get(1..1 + n)?;
            // A single byte below 0x80 is its own canonical encoding, without a prefix.
            if n == 1 && body[0] < 0x80 {
                return None;
            }
            Some((Item::Bytes(body.to_vec()), 1 + n))
        }
        0xb8..=0xbf => {
            let ln = (first - 0xb7) as usize;
            let n = len_of(b.get(1..1 + ln)?)?;
            if n < 56 {
                return None;
            }
            let body = b.get(1 + ln..1 + ln + n)?;
            Some((Item::Bytes(body.to_vec()), 1 + ln + n))
        }
        0xc0..=0xf7 => {
            let n = (first - 0xc0) as usize;
            let items = decode_items(b.get(1..1 + n)?, depth + 1)?;
            Some((Item::List(items), 1 + n))
        }
        0xf8..=0xff => {
            let ln = (first - 0xf7) as usize;
            let n = len_of(b.get(1..1 + ln)?)?;
            if n < 56 {
                return None;
            }
            let items = decode_items(b.get(1 + ln..1 + ln + n)?, depth + 1)?;
            Some((Item::List(items), 1 + ln + n))
        }
    }
}

/// Decode one RLP value that must use the whole input.
pub fn decode_all(b: &[u8]) -> Option<Item> {
    let (v, n) = decode(b)?;
    if n == b.len() {
        Some(v)
    } else {
        None
    }
}

fn decode_items(mut b: &[u8], depth: usize) -> Option<Vec<Item>> {
    let mut out = Vec::new();
    while !b.is_empty() {
        let (v, n) = decode_at(b, depth)?;
        out.push(v);
        b = &b[n..];
    }
    Some(out)
}

fn len_of(b: &[u8]) -> Option<usize> {
    if b.is_empty() || b[0] == 0 || b.len() > 8 {
        return None;
    }
    let mut x = 0usize;
    for byte in b {
        x = x.checked_shl(8)?.checked_add(*byte as usize)?;
    }
    Some(x)
}
