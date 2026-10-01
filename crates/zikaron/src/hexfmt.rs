//! Hex spelling in one place: `0x` prefix, lowercase digits, the hex20 / hex32 / hex65 shapes (law §1).

/// One byte as two lowercase hex digits, high nibble first (law §1).
pub fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

/// Law objects (hex20 / hex32 / hex65, §1) accept lowercase only.
fn lower_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// The measurement transport encoding (HARNESS audit input `pile` and `evidence.calldata`) accepts both
/// cases. Law objects keep the law's spelling; the transport encoding is an agreement between harnesses, and
/// case is accepted on that side.
fn any_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// `0x` plus an even number of hex digits (either case), decoded to bytes; missing or uppercase prefix, odd
/// length or a non-hex character gives `None`.
pub fn decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.len() < 2 || b[0] != b'0' || b[1] != b'x' {
        return None;
    }
    let body = &b[2..];
    if body.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() / 2);
    for pair in body.chunks(2) {
        out.push((any_digit(pair[0])? << 4) | any_digit(pair[1])?);
    }
    Some(out)
}

fn is_form(s: &str, digits: usize) -> bool {
    let b = s.as_bytes();
    b.len() == digits + 2
        && b[0] == b'0'
        && b[1] == b'x'
        && b[2..].iter().all(|&c| lower_digit(c).is_some())
}

/// hex20: `0x` plus 40 lowercase hex digits (law §1).
pub fn is_hex20(s: &str) -> bool {
    is_form(s, 40)
}

/// hex32: `0x` plus 64 lowercase hex digits (law §1).
pub fn is_hex32(s: &str) -> bool {
    is_form(s, 64)
}

/// hex65: `0x` plus 130 lowercase hex digits (law §1).
pub fn is_hex65(s: &str) -> bool {
    is_form(s, 130)
}

/// Transport encoding (HARNESS `pile`): lowercase `0x` plus an even number of hex digits of either case; the
/// empty string is `0x`; `0X` is outside the shape. Not one of the three law shapes.
pub fn is_bytes(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2
        && b[0] == b'0'
        && b[1] == b'x'
        && b[2..].len() % 2 == 0
        && b[2..].iter().all(|&c| any_digit(c).is_some())
}

/// Transport encoding (HARNESS `evidence.calldata`, law §9.2): `0x` plus an even number of lowercase hex
/// digits; empty calldata is `0x`.
pub fn is_lower_bytes(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2
        && b[0] == b'0'
        && b[1] == b'x'
        && b[2..].len() % 2 == 0
        && b[2..].iter().all(|&c| lower_digit(c).is_some())
}

/// A private key scalar on the command line (HARNESS `sign` `<privkey-hex>`): exactly 64 hex digits of either
/// case, optionally after `0x` or `0X`; anything else is misuse. The range (1 to n − 1) is judged by cryptox.
pub fn scalar32(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    let rest = if b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X') { &b[2..] } else { b };
    if rest.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in rest.chunks(2).enumerate() {
        out[i] = (any_digit(pair[0])? << 4) | any_digit(pair[1])?;
    }
    Some(out)
}
