//! QR code (the image of the badge). Byte mode, error correction level L, version chosen automatically,
//! mask fixed at zero.
//!
//! A minimal implementation of ISO/IEC 18004: only the path the badge needs. The version table holds only
//! level L (kit law §6's cap of 2953 is exactly the capacity of version 40 level L byte mode, so this table
//! and that cap share a source). The mask is fixed at number zero: every mask produces a valid code, and
//! choosing a mask only affects readability, not correctness.
//!
//! The output is a square matrix (true is black); rendering it as SVG belongs to `badgex`; this file writes
//! nothing to disk.

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

/// Encode a code. `None` when it does not fit (the kit crate's cap has refused long before).
pub fn encode(bytes: &[u8]) -> Option<Code> {
    let v = version_for(bytes.len())?;
    let size = 17 + 4 * v;
    let mut m = vec![vec![false; size]; size];
    let mut f = vec![vec![false; size]; size];  // function pattern reservation

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
    // Data: zigzag placement, mask zero.
    let codewords = interleave(&data_stream(bytes, v), v);
    let mut bits: Vec<bool> = Vec::with_capacity(codewords.len() * 8);
    for cw in &codewords {
        for i in (0..8).rev() {
            bits.push((cw >> i) & 1 == 1);
        }
    }
    let mut k = 0usize;
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
                if f[r][c] {
                    continue;
                }
                let bit = if k < bits.len() { bits[k] } else { false };
                k += 1;
                let masked = if (r + c) % 2 == 0 { !bit } else { bit };
                m[r][c] = masked;
            }
        }
        upward = !upward;
        col -= 2;
    }
    // Format information (mask zero).
    let fb = format_bits(0);
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
    // Version information.
    if v >= 7 {
        let vb = version_bits(v as u32);
        for i in 0..18 {
            let on = (vb >> i) & 1 == 1;
            let (a, b) = (i / 3, i % 3);
            m[a][size - 11 + b] = on;
            m[size - 11 + b][a] = on;
        }
    }
    Some(Code { version: v, size, modules: m })
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
