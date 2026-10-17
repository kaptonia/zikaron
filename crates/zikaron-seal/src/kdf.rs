//! The scrypt parameters of keystore V3 and their bounds, and its MAC: shared by the key vault's seals (here) and
//! the desktop's key files.

use zikaron::cryptox;

/// Derived key length: the first sixteen bytes are the key, the last sixteen feed the MAC.
pub const DKLEN: usize = 32;

/// The three scrypt parameters. Both levels are compliant V3: the app writes the standard level; the light
/// level is for code that runs many passes (tests).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Params {
    pub n: usize,
    pub r: usize,
    pub p: usize,
}

/// The scrypt cost floor (geth's standard level) for the key files this app writes ([`Params::standard`]) and,
/// outside the test hooks, for the key vault's own derivation (`keybox::params`).
pub const BACKUP_FLOOR: Params = Params { n: 262_144, r: 8, p: 1 };

impl Params {
    /// The level the app writes (geth's standard parameters): [`BACKUP_FLOOR`].
    pub fn standard() -> Params {
        BACKUP_FLOOR
    }

    /// geth's light parameters: compliant, but much cheaper to derive.
    pub fn light() -> Params {
        Params { n: 4_096, r: 8, p: 6 }
    }

    pub fn ok(&self) -> bool {
        in_range(self.n, self.r, self.p)
    }
}

/// Accepted scrypt parameters: memory `128·r·n` at most 2^30 bytes, `p` from 1 to 16, total work `128·r·n·p`
/// at most 2^32 bytes, `r` at least 1 (and `n` a power of two from 2, as scrypt requires). The parameters come
/// from an untrusted file: without a ceiling a file could demand terabytes of memory or days of work, and a
/// ceiling set at this app's own level would refuse other wallets' compliant files. The same bounds apply to
/// every reader of scrypt parameters (key files, the key store, whole-machine backups). The app itself writes
/// [`Params::standard`] (see `backup::params`).
pub const MAX_MEMORY: u128 = 1 << 30;
pub const MAX_WORK: u128 = 1 << 32;
pub const MAX_P: usize = 16;
pub const MIN_R: usize = 1;
/// Accepted `pbkdf2` round count `c`: 1 to 2^24. Other wallets write 262,144; the ceiling keeps the work an
/// untrusted file can demand to seconds.
pub const MIN_C: u64 = 1;
pub const MAX_C: u64 = 1 << 24;

/// `["c"]` when `pbkdf2`'s round count is out of bounds, empty otherwise.
pub fn c_out_of_bounds(c: u64) -> Vec<&'static str> {
    if (MIN_C..=MAX_C).contains(&c) { Vec::new() } else { vec!["c"] }
}

/// Whether `n` has the shape scrypt requires (a power of two from 2).
pub fn n_shaped(n: usize) -> bool {
    n >= 2 && n & (n - 1) == 0
}

/// Whether `n` and `r` have the shape scrypt itself requires together: `n` a power of two from 2, and below
/// 2^(16·r) (a larger `n` cannot be derived at all, whatever the bounds say).
pub fn n_shaped_for(n: usize, r: usize) -> bool {
    // A zero `r` is reported by `out_of_bounds`, not here.
    n_shaped(n) && (r == 0 || r >= 4 || (n as u128) < (1u128 << (16 * r as u32)))
}

/// Which scrypt bounds these parameters break (`r`, `p`, `memory`, `work`), empty when none.
pub fn out_of_bounds(n: usize, r: usize, p: usize) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if r < MIN_R {
        bad.push("r");
    }
    if !(1..=MAX_P).contains(&p) {
        bad.push("p");
    }
    // Checked products: a file's numbers can be arbitrarily large; an overflowing product is out of bounds,
    // never wrapped into range.
    let memory = 128u128.checked_mul(r as u128).and_then(|x| x.checked_mul(n as u128));
    if memory.map(|m| m > MAX_MEMORY).unwrap_or(true) {
        bad.push("memory");
    }
    if memory.and_then(|m| m.checked_mul(p as u128)).map(|w| w > MAX_WORK).unwrap_or(true) {
        bad.push("work");
    }
    bad
}

/// Whether the parameters have a valid shape and are within bounds.
pub fn in_range(n: usize, r: usize, p: usize) -> bool {
    n_shaped_for(n, r) && out_of_bounds(n, r, p).is_empty()
}

/// MAC: `keccak256(dk[16..32] ‖ ciphertext)`. Public so the wrong-password check can be tested on its own.
pub fn mac_of(dk: &[u8; DKLEN], cipher_text: &[u8]) -> [u8; 32] {
    let mut m = Vec::with_capacity(16 + cipher_text.len());
    m.extend_from_slice(&dk[16..32]);
    m.extend_from_slice(cipher_text);
    cryptox::keccak256(&m)
}
