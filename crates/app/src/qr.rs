//! QR codes for badges: byte mode, error correction level L, smallest fitting version, mask chosen by
//! penalty, and every symbol read back before it is returned.
//!
//! A minimal implementation of ISO/IEC 18004 covering only what badges need. The version table holds only
//! level L (the 2953-byte payload cap of kit law §6 is exactly version 40 level L byte-mode
//! capacity). Every mask gives a valid code, but not an equally readable one: all eight are drawn and the one
//! with the lowest penalty is kept (§7.8.3, [`penalty`]). The symbol is then decoded by this module's own
//! reader ([`decode`]: format and version information, unmasking, every block's error correction, the byte
//! segment); one that does not return its bytes is never handed out ([`NotMade::SelfCheck`]).
//!
//! The output is a square matrix (true is black); SVG rendering is done by `badgex`. Nothing is written to
//! disk.

/// Per version: error correction codewords per block; per group (block count, data codewords per block).
/// Level L.
const EC_L: [(u8, [(u8, u16); 2]); 40] = [
    (7, [(1, 19), (0, 0)]),
    (10, [(1, 34), (0, 0)]),
    (15, [(1, 55), (0, 0)]),
    (20, [(1, 80), (0, 0)]),
    (26, [(1, 108), (0, 0)]),
    (18, [(2, 68), (0, 0)]),
    (20, [(2, 78), (0, 0)]),
    (24, [(2, 97), (0, 0)]),
    (30, [(2, 116), (0, 0)]),
    (18, [(2, 68), (2, 69)]),
    (20, [(4, 81), (0, 0)]),
    (24, [(2, 92), (2, 93)]),
    (26, [(4, 107), (0, 0)]),
    (30, [(3, 115), (1, 116)]),
    (22, [(5, 87), (1, 88)]),
    (24, [(5, 98), (1, 99)]),
    (28, [(1, 107), (5, 108)]),
    (30, [(5, 120), (1, 121)]),
    (28, [(3, 113), (4, 114)]),
    (28, [(3, 107), (5, 108)]),
    (28, [(4, 116), (4, 117)]),
    (28, [(2, 111), (7, 112)]),
    (30, [(4, 121), (5, 122)]),
    (30, [(6, 117), (4, 118)]),
    (26, [(8, 106), (4, 107)]),
    (28, [(10, 114), (2, 115)]),
    (30, [(8, 122), (4, 123)]),
    (30, [(3, 117), (10, 118)]),
    (30, [(7, 116), (7, 117)]),
    (30, [(5, 115), (10, 116)]),
    (30, [(13, 115), (3, 116)]),
    (30, [(17, 115), (0, 0)]),
    (30, [(17, 115), (1, 116)]),
    (30, [(13, 115), (6, 116)]),
    (30, [(12, 121), (7, 122)]),
    (30, [(6, 121), (14, 122)]),
    (30, [(17, 122), (4, 123)]),
    (30, [(4, 122), (18, 123)]),
    (30, [(20, 117), (4, 118)]),
    (30, [(19, 118), (6, 119)]),
];

/// Alignment pattern center coordinates (from version 2).
const ALIGN: [&[usize]; 41] = [
    &[], &[], &[6, 18], &[6, 22], &[6, 26], &[6, 30], &[6, 34], &[6, 22, 38], &[6, 24, 42], &[6, 26, 46],
    &[6, 28, 50], &[6, 30, 54], &[6, 32, 58], &[6, 34, 62], &[6, 26, 46, 66], &[6, 26, 48, 70],
    &[6, 26, 50, 74], &[6, 30, 54, 78], &[6, 30, 56, 82], &[6, 30, 58, 86], &[6, 34, 62, 90],
    &[6, 28, 50, 72, 94], &[6, 26, 50, 74, 98], &[6, 30, 54, 78, 102], &[6, 28, 54, 80, 106],
    &[6, 32, 58, 84, 110], &[6, 30, 58, 86, 114], &[6, 34, 62, 90, 118], &[6, 26, 50, 74, 98, 122],
    &[6, 30, 54, 78, 102, 126], &[6, 26, 52, 78, 104, 130], &[6, 30, 56, 82, 108, 134],
    &[6, 34, 60, 86, 112, 138], &[6, 30, 58, 86, 114, 142], &[6, 34, 62, 90, 118, 146],
    &[6, 30, 54, 78, 102, 126, 150], &[6, 24, 50, 76, 102, 128, 154], &[6, 28, 54, 80, 106, 132, 158],
    &[6, 32, 58, 84, 110, 136, 162], &[6, 26, 54, 82, 110, 138, 166], &[6, 30, 58, 86, 114, 142, 170],
];

/// A code: side length and matrix (true is black).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Code {
    pub version: usize,
    pub size: usize,
    /// The mask the symbol is drawn with (0 to 7), the one [`make`] found to score lowest.
    pub mask: u8,
    pub modules: Vec<Vec<bool>>,
}

fn data_codewords(v: usize) -> usize {
    let (_, groups) = EC_L[v - 1];
    groups.iter().map(|(n, k)| *n as usize * *k as usize).sum()
}

/// How many bytes byte mode holds at level L.
pub fn capacity(v: usize) -> usize {
    let bits = data_codewords(v) * 8;
    let count_bits = if v <= 9 { 8 } else { 16 };
    (bits - 4 - count_bits) / 8
}

/// The smallest version that holds this many bytes; `None` when none does.
pub fn version_for(len: usize) -> Option<usize> {
    (1..=40).find(|v| capacity(*v) >= len)
}

// ── GF(256), polynomial 0x11d ──

fn gf_tables() -> ([u8; 256], [u8; 512]) {
    let mut log = [0u8; 256];
    let mut exp = [0u8; 512];
    let mut x: u16 = 1;
    for i in 0..255 {
        exp[i] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if x & 0x100 != 0 {
            x ^= 0x11d;
        }
    }
    for i in 255..512 {
        exp[i] = exp[i - 255];
    }
    (log, exp)
}

fn gf_mul(a: u8, b: u8, log: &[u8; 256], exp: &[u8; 512]) -> u8 {
    if a == 0 || b == 0 {
        0
    } else {
        exp[log[a as usize] as usize + log[b as usize] as usize]
    }
}

/// Generator polynomial (monic, degree n).
fn generator(n: usize, log: &[u8; 256], exp: &[u8; 512]) -> Vec<u8> {
    let mut g = vec![1u8];
    for i in 0..n {
        let mut next = vec![0u8; g.len() + 1];
        for (j, c) in g.iter().enumerate() {
            next[j] ^= *c;
            next[j + 1] ^= gf_mul(*c, exp[i], log, exp);
        }
        g = next;
    }
    g
}

fn ec_codewords(data: &[u8], n: usize, log: &[u8; 256], exp: &[u8; 512]) -> Vec<u8> {
    let g = generator(n, log, exp);
    let mut rem = vec![0u8; n];
    for &d in data {
        let lead = rem[0] ^ d;
        rem.remove(0);
        rem.push(0);
        if lead != 0 {
            for (j, c) in g.iter().skip(1).enumerate() {
                rem[j] ^= gf_mul(lead, *c, log, exp);
            }
        }
    }
    rem
}

struct Bits {
    bits: Vec<bool>,
}

impl Bits {
    fn push(&mut self, value: u32, n: usize) {
        for i in (0..n).rev() {
            self.bits.push((value >> i) & 1 == 1);
        }
    }
}

/// Data codewords: mode, count, bytes, terminator, byte alignment, padding.
fn data_stream(bytes: &[u8], v: usize) -> Vec<u8> {
    let total = data_codewords(v);
    let mut b = Bits { bits: Vec::new() };
    b.push(0b0100, 4);
    b.push(bytes.len() as u32, if v <= 9 { 8 } else { 16 });
    for &x in bytes {
        b.push(x as u32, 8);
    }
    let cap = total * 8;
    let term = (cap - b.bits.len()).min(4);
    b.push(0, term);
    while b.bits.len() % 8 != 0 {
        b.bits.push(false);
    }
    let mut out: Vec<u8> = b
        .bits
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |acc, &bit| (acc << 1) | bit as u8))
        .collect();
    let pads = [0xec, 0x11];
    let mut k = 0;
    while out.len() < total {
        out.push(pads[k % 2]);
        k += 1;
    }
    out
}

/// Split into blocks, compute error correction, interleave.
fn interleave(data: &[u8], v: usize) -> Vec<u8> {
    let (log, exp) = gf_tables();
    let (ec_n, groups) = EC_L[v - 1];
    let mut blocks: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut at = 0usize;
    for (n, k) in groups.iter() {
        for _ in 0..*n {
            let d = data[at..at + *k as usize].to_vec();
            at += *k as usize;
            let e = ec_codewords(&d, ec_n as usize, &log, &exp);
            blocks.push((d, e));
        }
    }
    let max_d = blocks.iter().map(|(d, _)| d.len()).max().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..max_d {
        for (d, _) in &blocks {
            if i < d.len() {
                out.push(d[i]);
            }
        }
    }
    for i in 0..ec_n as usize {
        for (_, e) in &blocks {
            out.push(e[i]);
        }
    }
    out
}

/// BCH remainder (format 15/5 uses 0x537, version 18/6 uses 0x1f25).
fn bch(value: u32, bits: usize, poly: u32, poly_bits: usize) -> u32 {
    let mut v = value << (poly_bits - 1);
    for i in (0..bits).rev() {
        if v & (1 << (i + poly_bits - 1)) != 0 {
            v ^= poly << i;
        }
    }
    v
}

fn format_bits(mask: u32) -> u32 {
    // Level L's two bits are 01.
    let data = (0b01 << 3) | mask;
    let rem = bch(data, 5, 0x537, 11);
    ((data << 10) | rem) ^ 0x5412
}

fn version_bits(v: u32) -> u32 {
    let rem = bch(v, 6, 0x1f25, 13);
    (v << 12) | rem
}

/// Why a code could not be made.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotMade {
    /// More bytes than version 40 level L holds (the kit crate's payload cap normally rejects this earlier).
    TooLong,
    /// The drawn symbol does not decode back to its bytes ([`decode`]), so it is not handed out.
    SelfCheck,
}

/// The function patterns of a version (finders, separators, alignment, timing, the dark module) drawn, and
/// which modules they and the format and version information take (`true`: not a data module).
fn base(v: usize) -> (Vec<Vec<bool>>, Vec<Vec<bool>>) {
    let size = 17 + 4 * v;
    let mut m = vec![vec![false; size]; size];
    let mut f = vec![vec![false; size]; size];
    let set = |m: &mut Vec<Vec<bool>>, f: &mut Vec<Vec<bool>>, r: usize, c: usize, on: bool| {
        m[r][c] = on;
        f[r][c] = true;
    };
    // Finder patterns and separators.
    for &(r0, c0) in &[(0usize, 0usize), (0, size - 7), (size - 7, 0)] {
        for r in 0..7 {
            for c in 0..7 {
                let on = r == 0 || r == 6 || c == 0 || c == 6 || (2..=4).contains(&r) && (2..=4).contains(&c);
                set(&mut m, &mut f, r0 + r, c0 + c, on);
            }
        }
    }
    for i in 0..8 {
        // the three separators
        set(&mut m, &mut f, 7, i, false);
        set(&mut m, &mut f, i, 7, false);
        set(&mut m, &mut f, 7, size - 1 - i, false);
        set(&mut m, &mut f, i, size - 8, false);
        set(&mut m, &mut f, size - 8, i, false);
        set(&mut m, &mut f, size - 1 - i, 7, false);
    }
    // Alignment patterns.
    let centers = ALIGN[v];
    for &r in centers {
        for &c in centers {
            if f[r][c] {
                continue;  // skip where it collides with a finder pattern
            }
            for dr in 0..5 {
                for dc in 0..5 {
                    let on = dr == 0 || dr == 4 || dc == 0 || dc == 4 || (dr == 2 && dc == 2);
                    set(&mut m, &mut f, r - 2 + dr, c - 2 + dc, on);
                }
            }
        }
    }
    // Timing patterns.
    for i in 8..size - 8 {
        if !f[6][i] {
            set(&mut m, &mut f, 6, i, i % 2 == 0);
        }
        if !f[i][6] {
            set(&mut m, &mut f, i, 6, i % 2 == 0);
        }
    }
    // Dark module and format information reservation.
    set(&mut m, &mut f, size - 8, 8, true);
    for i in 0..9 {
        if i != 6 {
            f[8][i] = true;
            f[i][8] = true;
        }
    }
    for i in 0..8 {
        f[8][size - 1 - i] = true;
        f[size - 1 - i][8] = true;
    }
    // Version information reservation.
    if v >= 7 {
        for i in 0..6 {
            for j in 0..3 {
                f[i][size - 11 + j] = true;
                f[size - 11 + j][i] = true;
            }
        }
    }
    (m, f)
}

/// The data modules in their zigzag order (two columns at a time from the right, up then down, the timing
/// column skipped): where each data bit goes, and where [`decode`] reads it from.
fn data_order(f: &[Vec<bool>]) -> Vec<(usize, usize)> {
    let size = f.len();
    let mut out = Vec::new();
    let mut col = size as isize - 1;
    let mut upward = true;
    while col > 0 {
        if col == 6 {
            col -= 1;
        }
        for step in 0..size {
            let r = if upward { size - 1 - step } else { step };
            for dc in 0..2 {
                let c = (col - dc) as usize;
                if !f[r][c] {
                    out.push((r, c));
                }
            }
        }
        upward = !upward;
        col -= 2;
    }
    out
}

/// Whether mask `k` (ISO/IEC 18004 table 10) flips the module at row `r`, column `c`.
fn masked(k: u8, r: usize, c: usize) -> bool {
    match k {
        0 => (r + c) % 2 == 0,
        1 => r % 2 == 0,
        2 => c % 3 == 0,
        3 => (r + c) % 3 == 0,
        4 => (r / 2 + c / 3) % 2 == 0,
        5 => (r * c) % 2 + (r * c) % 3 == 0,
        6 => ((r * c) % 2 + (r * c) % 3) % 2 == 0,
        _ => ((r + c) % 2 + (r * c) % 3) % 2 == 0,
    }
}

/// Write the format information (level L, mask `k`) in both of its places.
fn put_format(m: &mut [Vec<bool>], k: u8) {
    let size = m.len();
    let fb = format_bits(k as u32);
    let bit = |i: usize| (fb >> i) & 1 == 1;  // i is the bit index, 14 the most significant
    // First copy: top left, most significant first.
    for i in 0..6 {
        m[8][i] = bit(14 - i);
    }
    m[8][7] = bit(8);
    m[8][8] = bit(7);
    m[7][8] = bit(6);
    for i in 0..6 {
        m[5 - i][8] = bit(5 - i);
    }
    // Second copy: the lower-left column (high seven bits) and the upper-right row (low eight bits).
    for i in 0..7 {
        m[size - 1 - i][8] = bit(14 - i);
    }
    for i in 0..8 {
        m[8][size - 8 + i] = bit(7 - i);
    }
}

/// The penalty of a finished symbol (ISO/IEC 18004 §7.8.3), lower reads better: N1 runs of five or more
/// modules of one colour in a row or column (3, plus 1 per module past five), N2 each 2×2 block of one colour
/// (3), N3 each 1:1:3:1:1 finder-like pattern with four light modules on one side, the quiet zone counting
/// as light (40), N4 the distance of the dark share from half, in steps of five per cent (10 per step).
pub fn penalty(m: &[Vec<bool>]) -> u32 {
    let size = m.len();
    let mut score = 0u32;
    // N1 and N3, along rows then columns.
    for horizontal in [true, false] {
        for i in 0..size {
            let at = |j: usize| if horizontal { m[i][j] } else { m[j][i] };
            let mut run = 0usize;
            let mut colour = false;
            for j in 0..size {
                if j > 0 && at(j) == colour {
                    run += 1;
                } else {
                    if run >= 5 {
                        score += 3 + (run - 5) as u32;
                    }
                    colour = at(j);
                    run = 1;
                }
            }
            if run >= 5 {
                score += 3 + (run - 5) as u32;
            }
            // The line with four light modules of quiet zone at each end.
            let line: Vec<bool> = std::iter::repeat_n(false, 4).chain((0..size).map(at)).chain(std::iter::repeat_n(false, 4)).collect();
            let core = [true, false, true, true, true, false, true];
            for s in 0..=line.len() - 7 {
                if line[s..s + 7] != core {
                    continue;
                }
                let light_before = s >= 4 && line[s - 4..s].iter().all(|x| !x);
                let light_after = s + 11 <= line.len() && line[s + 7..s + 11].iter().all(|x| !x);
                if light_before || light_after {
                    score += 40;
                }
            }
        }
    }
    // N2.
    for r in 0..size - 1 {
        for c in 0..size - 1 {
            let x = m[r][c];
            if m[r][c + 1] == x && m[r + 1][c] == x && m[r + 1][c + 1] == x {
                score += 3;
            }
        }
    }
    // N4.
    let dark: usize = m.iter().map(|row| row.iter().filter(|x| **x).count()).sum();
    let total = size * size;
    let k = ((dark * 20).abs_diff(total * 10)).div_ceil(total).saturating_sub(1);
    score + 10 * k as u32
}

/// Draw the symbol for these bytes with mask `k`.
fn drawn(bytes: &[u8], v: usize, k: u8) -> Code {
    let (mut m, f) = base(v);
    let codewords = interleave(&data_stream(bytes, v), v);
    let order = data_order(&f);
    for (i, &(r, c)) in order.iter().enumerate() {
        let bit = codewords.get(i / 8).map(|cw| (cw >> (7 - i % 8)) & 1 == 1).unwrap_or(false);
        m[r][c] = bit ^ masked(k, r, c);
    }
    put_format(&mut m, k);
    // Version information.
    let size = m.len();
    if v >= 7 {
        let vb = version_bits(v as u32);
        for i in 0..18 {
            let on = (vb >> i) & 1 == 1;
            let (a, b) = (i / 3, i % 3);
            m[a][size - 11 + b] = on;
            m[size - 11 + b][a] = on;
        }
    }
    Code { version: v, size, mask: k, modules: m }
}

/// Make a code: the smallest version that holds the bytes, drawn with each of the eight masks, keeping the
/// lowest [`penalty`] (the lower mask number on a tie), then decoded with [`decode`]; a symbol that does not
/// decode back to these bytes is never returned.
pub fn make(bytes: &[u8]) -> Result<Code, NotMade> {
    let v = version_for(bytes.len()).ok_or(NotMade::TooLong)?;
    let code = (0..8u8).map(|k| drawn(bytes, v, k)).min_by_key(|c| (penalty(&c.modules), c.mask)).ok_or(NotMade::TooLong)?;
    match decode(&code) {
        Ok(back) if back == bytes => Ok(code),
        _ => Err(NotMade::SelfCheck),
    }
}

/// [`make`] with mask `k` forced regardless of penalty (every mask must still decode correctly).
pub fn make_with_mask(bytes: &[u8], k: u8) -> Result<Code, NotMade> {
    let v = version_for(bytes.len()).ok_or(NotMade::TooLong)?;
    let code = drawn(bytes, v, k % 8);
    match decode(&code) {
        Ok(back) if back == bytes => Ok(code),
        _ => Err(NotMade::SelfCheck),
    }
}

/// Encode bytes as a code. `None` when they do not fit or (never observed) the symbol does not decode back;
/// [`make`] says which.
pub fn encode(bytes: &[u8]) -> Option<Code> {
    make(bytes).ok()
}

/// Why a symbol could not be decoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotRead {
    /// The side is not 17 + 4·v for a version from 1 to 40, or a row is not that long.
    Size,
    /// Neither copy of the format information is within three bits of a valid one, or it is not level L.
    Format,
    /// The version information (from version 7) is within three bits of no version, or of another one.
    Version,
    /// A block's error correction does not check (the symbol is damaged; this reader corrects nothing).
    Damaged,
    /// The data is not one byte-mode segment that fits.
    Data,
}

/// Decode a symbol: format information (either copy, within three bits), the version (from the size, and from
/// version 7 also its version information), the data modules unmasked in order, every block's error
/// correction checked, and the single byte-mode segment's bytes. [`make`] runs this on every code it makes.
pub fn decode(code: &Code) -> Result<Vec<u8>, NotRead> {
    let m = &code.modules;
    let size = m.len();
    if size < 21 || (size - 17) % 4 != 0 || size > 177 || m.iter().any(|row| row.len() != size) {
        return Err(NotRead::Size);
    }
    let v = (size - 17) / 4;
    // Format information: the copy nearest a valid word, level L only.
    let read_bits = |places: &[(usize, usize)]| places.iter().fold(0u32, |acc, &(r, c)| (acc << 1) | m[r][c] as u32);
    let first: Vec<(usize, usize)> = (0..6).map(|i| (8, i)).chain([(8, 7), (8, 8), (7, 8)]).chain((0..6).rev().map(|i| (i, 8))).collect();
    let second: Vec<(usize, usize)> = (0..7).map(|i| (size - 1 - i, 8)).chain((0..8).map(|i| (8, size - 8 + i))).collect();
    let nearest = |word: u32| {
        (0..32u32)
            .map(|d| (((d << 10) | bch(d, 5, 0x537, 11)) ^ 0x5412, d))
            .map(|(valid, d)| ((valid ^ word).count_ones(), d))
            .min()
            .filter(|(dist, _)| *dist <= 3)
            .map(|(_, d)| d)
    };
    let data = nearest(read_bits(&first)).or_else(|| nearest(read_bits(&second))).ok_or(NotRead::Format)?;
    if data >> 3 != 0b01 {
        return Err(NotRead::Format);
    }
    let k = (data & 7) as u8;
    if v >= 7 {
        let copy = |swap: bool| (0..18).rev().fold(0u32, |acc, i| {
            let (a, b) = (i / 3, size - 11 + i % 3);
            (acc << 1) | if swap { m[b][a] } else { m[a][b] } as u32
        });
        let near = |word: u32| (7..=40u32).map(|x| ((version_bits(x) ^ word).count_ones(), x)).min().filter(|(d, _)| *d <= 3).map(|(_, x)| x);
        if near(copy(false)).or_else(|| near(copy(true))) != Some(v as u32) {
            return Err(NotRead::Version);
        }
    }
    let (_, f) = base(v);
    let order = data_order(&f);
    let mut codewords = vec![0u8; order.len() / 8];
    for (i, &(r, c)) in order.iter().enumerate().take(codewords.len() * 8) {
        if m[r][c] ^ masked(k, r, c) {
            codewords[i / 8] |= 1 << (7 - i % 8);
        }
    }
    // Deinterleave into blocks and check each block's error correction.
    let (log, exp) = gf_tables();
    let (ec_n, groups) = EC_L[v - 1];
    let lens: Vec<usize> = groups.iter().flat_map(|(n, k)| std::iter::repeat_n(*k as usize, *n as usize)).collect();
    let mut blocks: Vec<Vec<u8>> = lens.iter().map(|_| Vec::new()).collect();
    let mut at = 0usize;
    let max_d = lens.iter().copied().max().unwrap_or(0);
    for i in 0..max_d {
        for (b, len) in lens.iter().enumerate() {
            if i < *len {
                blocks[b].push(*codewords.get(at).ok_or(NotRead::Damaged)?);
                at += 1;
            }
        }
    }
    for _ in 0..ec_n {
        for block in blocks.iter_mut() {
            block.push(*codewords.get(at).ok_or(NotRead::Damaged)?);
            at += 1;
        }
    }
    let mut data_bytes = Vec::new();
    for (block, len) in blocks.iter().zip(&lens) {
        // The codeword polynomial vanishes at the generator's roots α^0 .. α^(ec_n - 1).
        for i in 0..ec_n as usize {
            let s = block.iter().fold(0u8, |acc, &x| gf_mul(acc, exp[i], &log, &exp) ^ x);
            if s != 0 {
                return Err(NotRead::Damaged);
            }
        }
        data_bytes.extend_from_slice(&block[..*len]);
    }
    // One byte-mode segment: mode, count, bytes.
    let bit = |i: usize| data_bytes.get(i / 8).map(|b| (b >> (7 - i % 8)) & 1 == 1);
    let take = |from: usize, n: usize| -> Option<usize> { (from..from + n).try_fold(0usize, |acc, i| Some((acc << 1) | bit(i)? as usize)) };
    if take(0, 4) != Some(0b0100) {
        return Err(NotRead::Data);
    }
    let count_bits = if v <= 9 { 8 } else { 16 };
    let n = take(4, count_bits).ok_or(NotRead::Data)?;
    let from = 4 + count_bits;
    (0..n).map(|j| take(from + 8 * j, 8).map(|x| x as u8)).collect::<Option<Vec<u8>>>().ok_or(NotRead::Data)
}

/// Render as SVG (one square per module, four-module quiet zone).
pub fn svg(code: &Code) -> String {
    let quiet = 4usize;
    let n = code.size + 2 * quiet;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {n} {n}\" shape-rendering=\"crispEdges\"><rect width=\"{n}\" height=\"{n}\" fill=\"#fff\"/><path fill=\"#000\" d=\""
    );
    for (r, row) in code.modules.iter().enumerate() {
        for (c, &on) in row.iter().enumerate() {
            if on {
                s.push_str(&format!("M{} {}h1v1h-1z", c + quiet, r + quiet));
            }
        }
    }
    s.push_str("\"/></svg>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every version's first and last length reads back (the count field widens at version 10; version
    /// information starts at 7); past version 40 there is no code.
    #[test]
    fn every_version_reads_back_at_both_its_ends() {
        for v in 1..=40 {
            let lo = if v == 1 { 0 } else { capacity(v - 1) + 1 };
            for n in [lo, capacity(v)] {
                let bytes: Vec<u8> = (0..n).map(|i| (i * 7 + v) as u8).collect();
                let code = make(&bytes).unwrap_or_else(|e| panic!("v{v} n{n}: {e:?}"));
                assert_eq!((code.version, code.size), (v, 17 + 4 * v), "n{n}");
                assert_eq!(decode(&code), Ok(bytes), "v{v} n{n}");
            }
        }
        assert_eq!(make(&vec![b'q'; capacity(40) + 1]), Err(NotMade::TooLong));
    }

    /// Each of the eight masks reads back, and the one taken scores lowest (the lower number on a tie).
    #[test]
    fn the_mask_taken_scores_lowest_and_every_mask_reads_back() {
        for payload in [b"zikaron-grant:A".to_vec(), vec![0u8; 200], (0..=255u8).collect::<Vec<u8>>()] {
            let chosen = make(&payload).expect("made");
            let scores: Vec<u32> = (0..8).map(|k| penalty(&make_with_mask(&payload, k).expect("reads back").modules)).collect();
            let best = (0..8u8).min_by_key(|k| (scores[*k as usize], *k)).expect("eight");
            assert_eq!(chosen.mask, best, "{scores:?}");
            for k in 0..8u8 {
                assert_eq!(decode(&make_with_mask(&payload, k).expect("made")), Ok(payload.clone()), "mask {k}");
            }
        }
    }

    /// The penalty's four rules on symbols whose score is counted by hand (21 × 21). All light: N1 798 (each
    /// of 42 lines one run of 21, 3 + 16), N2 1200 (400 blocks), N4 90 (no dark: nine steps from half). A
    /// checkerboard: nothing (no run, no block, no pattern, 221 dark of 441 within five per cent of half).
    /// All light but `1011101` at columns 4 to 10 of row 10: N1 772, N2 1152, N3 40 (light on both sides
    /// counts the place once), N4 90.
    #[test]
    fn the_penalty_is_the_standards_four_rules() {
        let light = vec![vec![false; 21]; 21];
        assert_eq!(penalty(&light), 798 + 1200 + 90);
        let board: Vec<Vec<bool>> = (0..21).map(|r| (0..21).map(|c| (r + c) % 2 == 0).collect()).collect();
        assert_eq!(penalty(&board), 0);
        let mut one = light.clone();
        for (c, on) in [true, false, true, true, true, false, true].into_iter().enumerate() {
            one[10][c + 4] = on;
        }
        assert_eq!(penalty(&one), 772 + 1152 + 40 + 90);
    }

    /// What does not read, by name: a damaged data module, a format information beyond three wrong bits in
    /// both copies or of another level, a version information beyond three wrong bits, a side of no version.
    /// Within three wrong bits each still reads.
    #[test]
    fn what_does_not_read_is_named() {
        let bytes = vec![b'z'; 160];
        let good = make(&bytes).expect("made");
        assert!(good.version >= 7, "version information in play");
        // A data module flipped.
        let (_, f) = base(good.version);
        let (r, c) = data_order(&f)[3];
        let mut d = good.clone();
        d.modules[r][c] = !d.modules[r][c];
        assert_eq!(decode(&d), Err(NotRead::Damaged));
        // Format: three bits in the first copy still read; four in both do not.
        let first = [(8usize, 0usize), (8, 1), (8, 2), (8, 3)];
        let size = good.size;
        let second = [(size - 1, 8usize), (size - 2, 8), (size - 3, 8), (size - 4, 8)];
        let mut three = good.clone();
        for &(r, c) in &first[..3] {
            three.modules[r][c] = !three.modules[r][c];
        }
        assert_eq!(decode(&three), Ok(bytes.clone()));
        let mut four = good.clone();
        for &(r, c) in first.iter().chain(second.iter()) {
            four.modules[r][c] = !four.modules[r][c];
        }
        assert_eq!(decode(&four), Err(NotRead::Format));
        // Another level (M): its format words in both places.
        let mut level_m = good.clone();
        let word = ((0b00u32 << 3 | good.mask as u32) << 10 | bch(0b00 << 3 | good.mask as u32, 5, 0x537, 11)) ^ 0x5412;
        let fb = format_bits(good.mask as u32);
        let positions: Vec<(usize, usize)> = (0..6).map(|i| (8, i)).chain([(8, 7), (8, 8), (7, 8)]).chain((0..6).rev().map(|i| (i, 8))).collect();
        let positions2: Vec<(usize, usize)> = (0..7).map(|i| (size - 1 - i, 8)).chain((0..8).map(|i| (8, size - 8 + i))).collect();
        for places in [&positions, &positions2] {
            for (j, &(r, c)) in places.iter().enumerate() {
                level_m.modules[r][c] = (word >> (14 - j)) & 1 == 1;
            }
        }
        assert_ne!(word, fb);
        assert_eq!(decode(&level_m), Err(NotRead::Format));
        // Version information: four wrong bits in both copies.
        let mut ver = good.clone();
        for i in 0..4 {
            let (a, b) = (i / 3, size - 11 + i % 3);
            ver.modules[a][b] = !ver.modules[a][b];
            ver.modules[b][a] = !ver.modules[b][a];
        }
        assert_eq!(decode(&ver), Err(NotRead::Version));
        // A side of no version, and a ragged row.
        let small = Code { version: 1, size: 20, mask: 0, modules: vec![vec![false; 20]; 20] };
        assert_eq!(decode(&small), Err(NotRead::Size));
        let mut ragged = good.clone();
        ragged.modules[5].pop();
        assert_eq!(decode(&ragged), Err(NotRead::Size));
    }
}
