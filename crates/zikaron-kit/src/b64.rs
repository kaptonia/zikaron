//! base64url (RFC 4648 §5), unpadded and canonical: the unused bits of the last character are zero (kit law
//! §1). The one place it is encoded and decoded.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn value(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a' + 26) as u32),
        b'0'..=b'9' => Some((c - b'0' + 52) as u32),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 4 + 2) / 3);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

/// Decode; empty input, a byte outside the alphabet, length ≡ 1 (mod 4) or nonzero unused bits give `None`
/// (kit law §6.2 step 3a).
pub fn decode(s: &[u8]) -> Option<Vec<u8>> {
    if s.is_empty() || s.len() % 4 == 1 {
        return None;
    }
    let mut vals = Vec::with_capacity(s.len());
    for &c in s {
        vals.push(value(c)?);
    }
    // The unused bits of the last character must be zero.
    match s.len() % 4 {
        2 => {
            if vals[vals.len() - 1] & 0x0f != 0 {
                return None;
            }
        }
        3 => {
            if vals[vals.len() - 1] & 0x03 != 0 {
                return None;
            }
        }
        _ => {}
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    for chunk in vals.chunks(4) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, v)| acc | (v << (18 - 6 * i)));
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}
