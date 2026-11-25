use zikaron::cryptox;

pub const DKLEN: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Params {
    pub n: usize,
    pub r: usize,
    pub p: usize,
}

pub const BACKUP_FLOOR: Params = Params { n: 262_144, r: 8, p: 1 };

impl Params {
    pub fn standard() -> Params {
        BACKUP_FLOOR
    }

    pub fn light() -> Params {
        Params { n: 4_096, r: 8, p: 6 }
    }

    pub fn ok(&self) -> bool {
        in_range(self.n, self.r, self.p)
    }
}

pub const MAX_MEMORY: u128 = 1 << 30;
pub const MAX_WORK: u128 = 1 << 32;
pub const MAX_P: usize = 16;
pub const MIN_R: usize = 1;
pub const MIN_C: u64 = 1;
pub const MAX_C: u64 = 1 << 24;

pub fn c_out_of_bounds(c: u64) -> Vec<&'static str> {
    if (MIN_C..=MAX_C).contains(&c) { Vec::new() } else { vec!["c"] }
}

pub fn n_shaped(n: usize) -> bool {
    n >= 2 && n & (n - 1) == 0
}

pub fn n_shaped_for(n: usize, r: usize) -> bool {
    n_shaped(n) && (r == 0 || r >= 4 || (n as u128) < (1u128 << (16 * r as u32)))
}

pub fn out_of_bounds(n: usize, r: usize, p: usize) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if r < MIN_R {
        bad.push("r");
    }
    if !(1..=MAX_P).contains(&p) {
        bad.push("p");
    }
    let memory = 128u128.checked_mul(r as u128).and_then(|x| x.checked_mul(n as u128));
    if memory.map(|m| m > MAX_MEMORY).unwrap_or(true) {
        bad.push("memory");
    }
    if memory.and_then(|m| m.checked_mul(p as u128)).map(|w| w > MAX_WORK).unwrap_or(true) {
        bad.push("work");
    }
    bad
}

pub fn in_range(n: usize, r: usize, p: usize) -> bool {
    n_shaped_for(n, r) && out_of_bounds(n, r, p).is_empty()
}

pub fn mac_of(dk: &[u8; DKLEN], cipher_text: &[u8]) -> [u8; 32] {
    let mut m = Vec::with_capacity(16 + cipher_text.len());
    m.extend_from_slice(&dk[16..32]);
    m.extend_from_slice(cipher_text);
    cryptox::keccak256(&m)
}
