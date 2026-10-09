//! Neutralize an ELF file's `.comment` section, which names the toolchains that built it (compiler, linker,
//! and on a cross build the package manager they came from). It is not loaded at run time, so its bytes are
//! zeroed in place: size and layout are unchanged and nothing reveals the build machine.
//!
//! Every offset and size comes from the file, so all arithmetic on them is checked: an overflow or a range past
//! the end is refused by name and the file is left untouched (nothing is zeroed until every header is read).

/// Zero the `.comment` section of the 64-bit little-endian ELF image `b`. Returns the number of bytes zeroed
/// (0 with no such section or no section headers). Refused: shorter than the header; not ELF; not 64-bit
/// little-endian; section header entries under 64 bytes; extended section numbering; the section header table,
/// section name table, a name or a `.comment` body extending past the end (or offset plus size overflowing);
/// a name table index that is not a section; an unterminated name.
pub fn neutral_comment(b: &mut [u8]) -> Result<usize, String> {
    if b.len() < 64 || &b[0..4] != b"\x7fELF" {
        return Err("not an ELF file".into());
    }
    if b[4] != 2 || b[5] != 1 {
        return Err("only 64-bit little-endian ELF is handled".into());
    }
    let comments = comment_ranges(b)?;
    let mut zeroed = 0;
    for (off, end) in comments {
        b[off..end].fill(0);
        zeroed += end - off;
    }
    Ok(zeroed)
}

/// `at + len`, when it fits and stays within `limit`.
fn within(at: usize, len: usize, limit: usize) -> Option<usize> {
    at.checked_add(len).filter(|end| *end <= limit)
}

/// The byte ranges of every `.comment` section with bytes in the file, every header read and checked first.
fn comment_ranges(b: &[u8]) -> Result<Vec<(usize, usize)>, String> {
    let n = b.len();
    let field = |o: usize, w: usize| -> Result<u64, String> {
        let end = within(o, w, n).ok_or_else(|| format!("a {w}-byte field at {o} runs past the end"))?;
        let mut x = [0u8; 8];
        x[..w].copy_from_slice(&b[o..end]);
        Ok(u64::from_le_bytes(x))
    };
    let size = |v: u64, what: &str| usize::try_from(v).map_err(|_| format!("{what} {v} does not fit this machine"));
    let shoff = size(field(0x28, 8)?, "the section header offset")?;
    let shentsize = size(field(0x3A, 2)?, "the section header entry size")?;
    let shnum = size(field(0x3C, 2)?, "the section count")?;
    let shstrndx = size(field(0x3E, 2)?, "the name table index")?;
    if shoff == 0 {
        return Ok(Vec::new());
    }
    if shnum == 0 || shstrndx == 0xffff {
        return Err("extended section numbering (more sections than the header counts) is not handled".into());
    }
    if shentsize < 64 {
        return Err(format!("section header entries of {shentsize} bytes (64 wanted)"));
    }
    let table = shnum.checked_mul(shentsize).ok_or("the section header table's size does not fit")?;
    within(shoff, table, n).ok_or_else(|| format!("the section header table ({shnum} × {shentsize} at {shoff}) runs past the end"))?;
    if shstrndx >= shnum {
        return Err(format!("the name table index {shstrndx} is not one of the {shnum} sections"));
    }
    // Each header lies within the table (checked above), so these sums fit.
    let sh = |i: usize| shoff + i * shentsize;
    let names_off = size(field(sh(shstrndx) + 0x18, 8)?, "the name table offset")?;
    let names_size = size(field(sh(shstrndx) + 0x20, 8)?, "the name table size")?;
    let names_end = within(names_off, names_size, n).ok_or("the name table runs past the end")?;
    const SHT_NOBITS: u64 = 8;
    let mut out = Vec::new();
    for i in 0..shnum {
        let name = size(field(sh(i), 4)?, "a section's name offset")?;
        let at = names_off.checked_add(name).filter(|a| *a < names_end).ok_or_else(|| format!("section {i}'s name lies outside the name table"))?;
        let len = b[at..names_end].iter().position(|c| *c == 0).ok_or_else(|| format!("section {i}'s name has no end"))?;
        if &b[at..at + len] != b".comment" || field(sh(i) + 4, 4)? == SHT_NOBITS {
            continue;
        }
        let off = size(field(sh(i) + 0x18, 8)?, "the .comment offset")?;
        let body = size(field(sh(i) + 0x20, 8)?, "the .comment size")?;
        let end = within(off, body, n).ok_or("the .comment section runs past the end")?;
        out.push((off, end));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small ELF64 image: header, a `.comment` body, a name table, and three section headers (null, the name
    /// table, `.comment`), laid out as a linker would.
    fn image() -> Vec<u8> {
        let mut b = vec![0u8; 64];
        b[0..4].copy_from_slice(b"\x7fELF");
        b[4] = 2;
        b[5] = 1;
        let comment = b"rustc version 1.97.1\0";
        let names = b"\0.shstrtab\0.comment\0";
        let comment_off = b.len();
        b.extend_from_slice(comment);
        let names_off = b.len();
        b.extend_from_slice(names);
        let shoff = b.len();
        let header = |name: u32, kind: u32, off: usize, size: usize| {
            let mut h = vec![0u8; 64];
            h[0..4].copy_from_slice(&name.to_le_bytes());
            h[4..8].copy_from_slice(&kind.to_le_bytes());
            h[0x18..0x20].copy_from_slice(&(off as u64).to_le_bytes());
            h[0x20..0x28].copy_from_slice(&(size as u64).to_le_bytes());
            h
        };
        b.extend(header(0, 0, 0, 0));
        b.extend(header(1, 3, names_off, names.len()));
        b.extend(header(11, 1, comment_off, comment.len()));
        b[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes());
        b[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes());
        b[0x3C..0x3E].copy_from_slice(&3u16.to_le_bytes());
        b[0x3E..0x40].copy_from_slice(&1u16.to_le_bytes());
        b
    }

    fn set(b: &mut [u8], at: usize, bytes: &[u8]) {
        b[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn shoff(b: &[u8]) -> usize {
        u64::from_le_bytes(b[0x28..0x30].try_into().expect("8")) as usize
    }

    #[test]
    fn the_comment_is_zeroed_in_place() {
        let mut b = image();
        let len = b.len();
        assert_eq!(neutral_comment(&mut b), Ok(21));
        assert_eq!(b.len(), len, "the size kept");
        assert!(!b.windows(5).any(|w| w == b"rustc"), "the toolchain words gone");
        // No section headers: nothing to do.
        let mut none = image();
        set(&mut none, 0x28, &0u64.to_le_bytes());
        assert_eq!(neutral_comment(&mut none), Ok(0));
        // A `.comment` with no bytes in the file (NOBITS) is skipped.
        let mut nobits = image();
        let at = shoff(&nobits) + 2 * 64 + 4;
        set(&mut nobits, at, &8u32.to_le_bytes());
        assert_eq!(neutral_comment(&mut nobits), Ok(0));
    }

    /// Every refused form fails by name and leaves the file unchanged.
    #[test]
    fn every_offset_past_the_end_or_out_of_range_is_refused_and_nothing_is_touched() {
        let base = image();
        let sh = shoff(&base);
        // The name table is the 20 bytes before the section headers; its last byte ends `.comment`.
        let names_off = sh - 20;
        let cases: Vec<(&str, Vec<u8>, &str)> = vec![
            ("short", base[..63].to_vec(), "not an ELF"),
            ("not ELF", { let mut b = base.clone(); b[0] = 0; b }, "not an ELF"),
            ("32-bit", { let mut b = base.clone(); b[4] = 1; b }, "64-bit little-endian"),
            ("big-endian", { let mut b = base.clone(); b[5] = 2; b }, "64-bit little-endian"),
            ("the table offset at the top of the address space", { let mut b = base.clone(); set(&mut b, 0x28, &u64::MAX.to_le_bytes()); b }, "runs past the end"),
            ("the table past the end", { let mut b = base.clone(); set(&mut b, 0x3C, &4u16.to_le_bytes()); b }, "runs past the end"),
            ("small entries", { let mut b = base.clone(); set(&mut b, 0x3A, &40u16.to_le_bytes()); b }, "64 wanted"),
            ("extended numbering", { let mut b = base.clone(); set(&mut b, 0x3C, &0u16.to_le_bytes()); b }, "extended section numbering"),
            ("the name index at its escape", { let mut b = base.clone(); set(&mut b, 0x3E, &0xffffu16.to_le_bytes()); b }, "extended section numbering"),
            ("the name index not a section", { let mut b = base.clone(); set(&mut b, 0x3E, &3u16.to_le_bytes()); b }, "not one of the 3 sections"),
            ("the name table at the top", { let mut b = base.clone(); set(&mut b, sh + 64 + 0x18, &u64::MAX.to_le_bytes()); b }, "name table runs past the end"),
            ("the name table too long", { let mut b = base.clone(); set(&mut b, sh + 64 + 0x20, &4096u64.to_le_bytes()); b }, "name table runs past the end"),
            ("a name outside the table", { let mut b = base.clone(); set(&mut b, sh + 2 * 64, &u32::MAX.to_le_bytes()); b }, "outside the name table"),
            ("a name with no end", { let mut b = base.clone(); b[names_off + 19] = b'x'; b }, "no end"),
            ("the comment at the top", { let mut b = base.clone(); set(&mut b, sh + 2 * 64 + 0x18, &u64::MAX.to_le_bytes()); b }, ".comment section runs past the end"),
            ("the comment too long", { let mut b = base.clone(); set(&mut b, sh + 2 * 64 + 0x20, &(u64::MAX - 1).to_le_bytes()); b }, ".comment section runs past the end"),
        ];
        for (form, mut b, said) in cases {
            let before = b.clone();
            let got = neutral_comment(&mut b);
            assert!(got.as_ref().err().map(|e| e.contains(said)).unwrap_or(false), "{form}: {got:?}");
            assert_eq!(b, before, "{form}: the file left as it was");
        }
    }
}
