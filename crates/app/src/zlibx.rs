struct Bits<'a> {
    src: &'a [u8],
    at: usize,
    acc: u32,
    n: u32,
}

impl<'a> Bits<'a> {
    fn new(src: &'a [u8]) -> Bits<'a> {
        Bits { src, at: 0, acc: 0, n: 0 }
    }

    fn need(&mut self, want: u32) -> Option<()> {
        while self.n < want {
            let b = *self.src.get(self.at)? as u32;
            self.at += 1;
            self.acc |= b << self.n;
            self.n += 8;
        }
        Some(())
    }

    fn take(&mut self, want: u32) -> Option<u32> {
        if want == 0 {
            return Some(0);
        }
        self.need(want)?;
        let v = self.acc & ((1u32 << want) - 1);
        self.acc >>= want;
        self.n -= want;
        Some(v)
    }

    fn align(&mut self) {
        let drop = self.n % 8;
        self.acc >>= drop;
        self.n -= drop;
    }

    fn byte(&mut self) -> Option<u8> {
        if self.n >= 8 {
            let v = (self.acc & 0xff) as u8;
            self.acc >>= 8;
            self.n -= 8;
            return Some(v);
        }
        let b = *self.src.get(self.at)?;
        self.at += 1;
        Some(b)
    }
}

struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Option<Huff> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            if l as usize > 15 {
                return None;
            }
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut left: i32 = 1;
        for l in 1..16 {
            left <<= 1;
            left -= counts[l] as i32;
            if left < 0 {
                return None;
            }
        }
        let mut offs = [0u16; 16];
        for l in 1..15 {
            offs[l + 1] = offs[l] + counts[l];
        }
        let mut symbols = vec![0u16; lengths.iter().filter(|l| **l > 0).count()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l > 0 {
                symbols[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Some(Huff { counts, symbols })
    }

    fn decode(&self, b: &mut Bits) -> Option<u16> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= b.take(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return self.symbols.get((index + (code - first)) as usize).copied();
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        None
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn copy_back(out: &mut Vec<u8>, dist: usize, len: usize) -> Option<()> {
    if dist == 0 || dist > out.len() {
        return None;
    }
    let start = out.len() - dist;
    for i in 0..len {
        let b = out[start + i];
        out.push(b);
    }
    Some(())
}

fn fixed_tables() -> Option<(Huff, Huff)> {
    let mut lit = [0u8; 288];
    for (i, l) in lit.iter_mut().enumerate() {
        *l = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    let dist = [5u8; 30];
    Some((Huff::new(&lit)?, Huff::new(&dist)?))
}

pub fn inflate(src: &[u8], cap: usize) -> Option<Vec<u8>> {
    let mut b = Bits::new(src);
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.take(1)?;
        let kind = b.take(2)?;
        match kind {
            0 => {
                b.align();
                let l0 = b.byte()? as usize;
                let l1 = b.byte()? as usize;
                let n0 = b.byte()? as usize;
                let n1 = b.byte()? as usize;
                let len = l0 | (l1 << 8);
                let nlen = n0 | (n1 << 8);
                if len != (!nlen & 0xffff) {
                    return None;
                }
                if out.len() + len > cap {
                    return None;
                }
                for _ in 0..len {
                    out.push(b.byte()?);
                }
            }
            1 | 2 => {
                let (lit, dist) = if kind == 1 {
                    fixed_tables()?
                } else {
                    let hlit = b.take(5)? as usize + 257;
                    let hdist = b.take(5)? as usize + 1;
                    let hclen = b.take(4)? as usize + 4;
                    if hlit > 286 || hdist > 30 {
                        return None;
                    }
                    let mut cl = [0u8; 19];
                    for i in 0..hclen {
                        cl[ORDER[i]] = b.take(3)? as u8;
                    }
                    let clh = Huff::new(&cl)?;
                    let mut lengths = vec![0u8; hlit + hdist];
                    let mut i = 0;
                    while i < lengths.len() {
                        let sym = clh.decode(&mut b)?;
                        match sym {
                            0..=15 => {
                                lengths[i] = sym as u8;
                                i += 1;
                            }
                            16 => {
                                if i == 0 {
                                    return None;
                                }
                                let prev = lengths[i - 1];
                                let n = 3 + b.take(2)? as usize;
                                for _ in 0..n {
                                    *lengths.get_mut(i)? = prev;
                                    i += 1;
                                }
                            }
                            17 => {
                                let n = 3 + b.take(3)? as usize;
                                for _ in 0..n {
                                    *lengths.get_mut(i)? = 0;
                                    i += 1;
                                }
                            }
                            18 => {
                                let n = 11 + b.take(7)? as usize;
                                for _ in 0..n {
                                    *lengths.get_mut(i)? = 0;
                                    i += 1;
                                }
                            }
                            _ => return None,
                        }
                    }
                    (Huff::new(&lengths[..hlit])?, Huff::new(&lengths[hlit..])?)
                };
                loop {
                    let sym = lit.decode(&mut b)?;
                    if sym < 256 {
                        if out.len() + 1 > cap {
                            return None;
                        }
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let i = sym as usize - 257;
                        if i >= LEN_BASE.len() {
                            return None;
                        }
                        let len = LEN_BASE[i] as usize + b.take(LEN_EXTRA[i] as u32)? as usize;
                        let d = dist.decode(&mut b)? as usize;
                        if d >= DIST_BASE.len() {
                            return None;
                        }
                        let distance =
                            DIST_BASE[d] as usize + b.take(DIST_EXTRA[d] as u32)? as usize;
                        if out.len() + len > cap {
                            return None;
                        }
                        copy_back(&mut out, distance, len)?;
                    }
                }
            }
            _ => return None,
        }
        if last == 1 {
            return Some(out);
        }
    }
}

pub fn inflate_zlib(src: &[u8], cap: usize) -> Option<Vec<u8>> {
    let cmf = *src.first()?;
    let flg = *src.get(1)?;
    if cmf & 0x0f != 8 {
        return None;
    }
    if ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return None;
    }
    if flg & 0x20 != 0 {
        return None;
    }
    inflate(&src[2..], cap)
}
