//! The `.comment` section of an ELF file names the toolchains that built it (the compiler, the linker, and on
//! a cross build the package manager they came from). It is not loaded at run time; its bytes are set to zero
//! in place, so the file keeps its size and layout and says nothing of the machine that built it.

/// Zero the `.comment` section of the 64-bit little-endian ELF image `b`. Returns how many bytes were
/// zeroed (0 when the file has no such section).
pub fn neutral_comment(b: &mut [u8]) -> Result<usize, String> {
    if b.len() < 64 || &b[0..4] != b"\x7fELF" {
        return Err("not an ELF file".into());
    }
    if b[4] != 2 || b[5] != 1 {
        return Err("only 64-bit little-endian ELF is handled".into());
    }
    let u16_at = |b: &[u8], o: usize| -> Result<u64, String> {
        b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as u64).ok_or_else(|| format!("short read at {o}"))
    };
    let u32_at = |b: &[u8], o: usize| -> Result<u64, String> {
        b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as u64).ok_or_else(|| format!("short read at {o}"))
    };
    let u64_at = |b: &[u8], o: usize| -> Result<u64, String> {
        b.get(o..o + 8).map(|x| u64::from_le_bytes(x.try_into().unwrap_or([0; 8]))).ok_or_else(|| format!("short read at {o}"))
    };
    let shoff = u64_at(b, 0x28)? as usize;
    let shentsize = u16_at(b, 0x3A)? as usize;
    let shnum = u16_at(b, 0x3C)? as usize;
    let shstrndx = u16_at(b, 0x3E)? as usize;
    if shoff == 0 || shnum == 0 {
        return Ok(0);
    }
    let sh = |i: usize| shoff + i * shentsize;
    let names_off = u64_at(b, sh(shstrndx) + 0x18)? as usize;
    let mut zeroed = 0;
    for i in 0..shnum {
        let name = u32_at(b, sh(i))? as usize;
        let at = names_off + name;
        let end = b[at..].iter().position(|c| *c == 0).map(|n| at + n).ok_or("section name runs off the file")?;
        if &b[at..end] != b".comment" {
            continue;
        }
        let off = u64_at(b, sh(i) + 0x18)? as usize;
        let size = u64_at(b, sh(i) + 0x20)? as usize;
        let body = b.get_mut(off..off + size).ok_or(".comment runs off the file")?;
        body.fill(0);
        zeroed += size;
    }
    Ok(zeroed)
}
