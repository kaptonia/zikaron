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

fn lower_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

fn any_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

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

pub fn is_hex20(s: &str) -> bool {
    is_form(s, 40)
}

pub fn is_hex32(s: &str) -> bool {
    is_form(s, 64)
}

pub fn is_hex65(s: &str) -> bool {
    is_form(s, 130)
}

pub fn is_bytes(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2
        && b[0] == b'0'
        && b[1] == b'x'
        && b[2..].len() % 2 == 0
        && b[2..].iter().all(|&c| any_digit(c).is_some())
}

pub fn is_lower_bytes(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2
        && b[0] == b'0'
        && b[1] == b'x'
        && b[2..].len() % 2 == 0
        && b[2..].iter().all(|&c| lower_digit(c).is_some())
}

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
