//! Web3 Secret Storage (keystore V3) files: the only form in which an identity key is written to disk.
//!
//! One file per key: a user-set password, scrypt plus AES-128-CTR, and the geth `UTC--` file name. Plaintext
//! private keys and recovery words never land on disk; the plaintext lives only in memory (`key::Secret`,
//! zeroed when dropped) and every byte written goes through `cryptx::aes128_ctr`. scrypt and AES come from
//! `cryptx`, the only app module allowed third-party cryptography.
//!
//! The MAC is `keccak256(dk[16..32] ‖ ciphertext)`, checked before decrypting, so a wrong password is reported
//! as such instead of as an address mismatch after decrypting garbage.

use crate::fault::{Fault, Known};
use crate::key::{Address, Secret};
use crate::cryptx;
use zikaron::cryptox;
use zikaron::hexfmt;
use zikaron::json::{self, Value};

/// KDF and cipher names, verbatim from the V3 specification. Files are written with `scrypt` ([`KDF`]); both
/// derivations the specification names are read (`scrypt`, and `pbkdf2` with [`PRF`]), so a file exported by
/// another wallet with either opens.
pub const KDF: &str = "scrypt";
pub const KDF_PBKDF2: &str = "pbkdf2";
pub const PRF: &str = "hmac-sha256";
pub const CIPHER: &str = "aes-128-ctr";
pub const VERSION: u64 = 3;
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

    fn ok(&self) -> bool {
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

/// A keystore V3's bytes and its address.
pub struct Keystore {
    pub json: Vec<u8>,
    pub address: Address,
    pub file_name: String,
}

fn rand(n: usize) -> Result<Vec<u8>, Fault> {
    crate::key::random(n)
}

/// Hex without `0x` (these fields are bare in the V3 specification).
fn bare(b: &[u8]) -> String {
    hexfmt::encode(b).trim_start_matches("0x").to_string()
}

fn unbare(s: &str) -> Option<Vec<u8>> {
    hexfmt::decode(&format!("0x{s}"))
}

/// UUID v4, with version and variant bits set per RFC 4122.
fn uuid() -> Result<String, Fault> {
    let mut b = rand(16)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = bare(&b);
    Ok(format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]))
}

/// The `UTC--<time>--<address>` file name (geth convention). The caller supplies the time, which keeps this
/// testable.
pub fn file_name(address: &Address, unix_secs: u64) -> String {
    format!("UTC--{}.000000000Z--{}", utc(unix_secs), bare(&address.0))
}

/// A Unix time formatted as `2026-09-08T10-37-00`. Also used by the UI to show when the last backup was made.
pub fn utc(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = crate::when::civil(unix_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}")
}


/// Encrypt. Returns V3 bytes, the address and the file name it should have.
pub fn encrypt(secret: &Secret, password: &str, p: Params, unix_secs: u64) -> Result<Keystore, Fault> {
    // Traced here so direct calls that bypass `apply` (tests, the CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::H2);
    if !p.ok() {
        return Err(Fault::known(Known::KeystoreParams, format!("n={} r={} p={}", p.n, p.r, p.p)));
    }
    let address = secret
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
    let salt = rand(32)?;
    let iv: [u8; 16] = rand(16)?.try_into().expect("十六字节");
    let id = uuid()?;
    encrypt_with(secret, &address, password, p, &salt, &iv, &id, unix_secs)
}

/// The deterministic half of encryption. `encrypt` passes salt, iv and id from system entropy; tests pass fixed
/// values to compare output bytes across dependency changes.
#[allow(clippy::too_many_arguments)]
fn encrypt_with(
    secret: &Secret,
    address: &Address,
    password: &str,
    p: Params,
    salt: &[u8],
    iv: &[u8; 16],
    id: &str,
    unix_secs: u64,
) -> Result<Keystore, Fault> {
    let address = *address;
    let mut dk = [0u8; DKLEN];
    if !cryptx::scrypt(password.as_bytes(), salt, p.n, p.r, p.p, &mut dk) {
        return Err(Fault::known(Known::KeystoreParams, format!("n={} r={} p={}", p.n, p.r, p.p)));
    }
    let ekey: [u8; 16] = dk[..16].try_into().expect("十六字节");
    // No plaintext passes this layer: the private key leaves `Secret` only through `Secret::ciphered`, which
    // returns ciphertext (see the `key` module header).
    let cipher_text = secret.ciphered(&ekey, iv);
    let mac = mac_of(&dk, &cipher_text);
    let json = write(&address, salt, iv, &cipher_text, &mac, p, id);
    Ok(Keystore { json, address, file_name: file_name(&address, unix_secs) })
}

/// MAC: `keccak256(dk[16..32] ‖ ciphertext)`. Public so the wrong-password check can be tested on its own.
pub fn mac_of(dk: &[u8; DKLEN], cipher_text: &[u8]) -> [u8; 32] {
    let mut m = Vec::with_capacity(16 + cipher_text.len());
    m.extend_from_slice(&dk[16..32]);
    m.extend_from_slice(cipher_text);
    cryptox::keccak256(&m)
}

fn write(
    address: &Address,
    salt: &[u8],
    iv: &[u8; 16],
    cipher_text: &[u8],
    mac: &[u8; 32],
    p: Params,
    id: &str,
) -> Vec<u8> {
    let obj = |m: Vec<(&str, Value)>| {
        Value::Obj(m.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    };
    let s = |x: String| Value::Str(x);
    let doc = obj(vec![
        ("address", s(bare(&address.0))),
        (
            "crypto",
            obj(vec![
                ("cipher", s(CIPHER.to_string())),
                ("ciphertext", s(bare(cipher_text))),
                ("cipherparams", obj(vec![("iv", s(bare(iv)))])),
                ("kdf", s(KDF.to_string())),
                (
                    "kdfparams",
                    obj(vec![
                        ("dklen", Value::Int(DKLEN as u64)),
                        ("n", Value::Int(p.n as u64)),
                        ("p", Value::Int(p.p as u64)),
                        ("r", Value::Int(p.r as u64)),
                        ("salt", s(bare(salt))),
                    ]),
                ),
                ("mac", s(bare(mac))),
            ]),
        ),
        ("id", s(id.to_string())),
        ("version", Value::Int(VERSION)),
    ]);
    // Member order, integer form and escaping all come from the core's canonical JSON writer.
    json::canon_bytes(&doc)
}

/// The fields of a V3 file as read, so a refusal can name the wrong field instead of just "bad format".
pub struct Shape {
    pub version: u64,
    pub kdf: String,
    pub cipher: String,
    /// `pbkdf2`'s pseudo-random function and round count, as written (read only for that derivation).
    pub prf: String,
    pub c: u64,
    pub c_written: bool,
    pub n: usize,
    pub r: usize,
    pub p: usize,
    pub dklen: usize,
    pub salt_len: usize,
    pub iv_len: usize,
    /// The `address` field as written. It is optional in V3 (geth writes it, foundry does not), so "absent"
    /// and "present but malformed" are kept apart.
    pub address_text: String,
    pub address: Option<Address>,
    /// `r` and `p` are present and whole numbers (an explicit 0 is caught by the bounds check).
    pub r_written: bool,
    pub p_written: bool,
    /// The `mac` and `ciphertext` members as written: absent (or not text, or empty) is `None`; text that is
    /// not hex is `Some(None)`; hex is its length in bytes.
    pub mac_len: Option<Option<usize>>,
    pub ciphertext_len: Option<Option<usize>>,
}

fn field<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(name, _)| name == k).map(|(_, x)| x),
        _ => None,
    }
}

fn text(v: &Value, k: &str) -> String {
    match field(v, k) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn num(v: &Value, k: &str) -> u64 {
    match field(v, k) {
        Some(Value::Int(n)) => *n,
        _ => 0,
    }
}

/// Read a file's shape. Unparsable JSON or a missing section is refused with a named reason.
pub fn shape(bytes: &[u8]) -> Result<Shape, Fault> {
    let v = json::parse(bytes)
        .map_err(|t| Fault::known(Known::KeystoreShape, crate::lang::filln(crate::lang::Key::Tail165, &[&format!("{:?}", t)])))?;
    let c = field(&v, "crypto")
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::t(crate::lang::Key::Tail166).to_string()))?;
    let kp = field(c, "kdfparams")
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::t(crate::lang::Key::Tail167).to_string()))?;
    let cp = field(c, "cipherparams")
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::t(crate::lang::Key::Tail168).to_string()))?;
    Ok(Shape {
        version: num(&v, "version"),
        kdf: text(c, "kdf"),
        cipher: text(c, "cipher"),
        prf: text(kp, "prf"),
        c: num(kp, "c"),
        c_written: matches!(field(kp, "c"), Some(Value::Int(_))),
        n: num(kp, "n") as usize,
        r: num(kp, "r") as usize,
        p: num(kp, "p") as usize,
        dklen: num(kp, "dklen") as usize,
        salt_len: unbare(&text(kp, "salt")).map(|x| x.len()).unwrap_or(0),
        iv_len: unbare(&text(cp, "iv")).map(|x| x.len()).unwrap_or(0),
        address_text: text(&v, "address"),
        address: Address::parse(&format!("0x{}", text(&v, "address"))),
        r_written: matches!(field(kp, "r"), Some(Value::Int(_))),
        p_written: matches!(field(kp, "p"), Some(Value::Int(_))),
        mac_len: hex_member(c, "mac"),
        ciphertext_len: hex_member(c, "ciphertext"),
    })
}

/// A hex member as written: absent, not text or empty is `None`; not hex is `Some(None)`; else its length.
fn hex_member(v: &Value, k: &str) -> Option<Option<usize>> {
    match field(v, k) {
        Some(Value::Str(s)) if !s.is_empty() => Some(unbare(s).map(|x| x.len())),
        _ => None,
    }
}

impl Shape {
    /// Whether this file derives by `pbkdf2` (else by `scrypt`, or by a name this reader does not know).
    pub fn pbkdf2(&self) -> bool {
        self.kdf == KDF_PBKDF2
    }

    /// The names of every non-compliant field or bound, empty when the file complies.
    pub fn compliant(&self) -> Vec<&'static str> {
        let mut bad = self.unshaped();
        bad.extend(self.over());
        bad
    }

    /// The derivation parameters out of bounds: scrypt's, or `pbkdf2`'s round count.
    pub fn over(&self) -> Vec<&'static str> {
        if self.pbkdf2() { c_out_of_bounds(self.c) } else { out_of_bounds(self.n, self.r, self.p) }
    }

    /// The parameters as shown in an out-of-bounds refusal.
    fn params_said(&self) -> String {
        if self.pbkdf2() { format!("c={}", self.c) } else { format!("n={} r={} p={}", self.n, self.r, self.p) }
    }

    /// Fields whose form is not recognised (parameter bounds are checked separately): version, kdf, cipher;
    /// for `scrypt`, `n` not a power of two or `r`/`p` missing; for `pbkdf2`, `prf` other than `hmac-sha256` or
    /// `c` missing; dklen, salt, iv; `mac` or `ciphertext` missing, empty or not 32 bytes; a malformed address.
    /// A 32-byte key always encrypts to 32 bytes, so any other ciphertext length could never open; it is
    /// reported before the derivation work, never as a wrong password. Non-hex `mac` or `ciphertext` is
    /// reported where it is read, also before the work.
    pub fn unshaped(&self) -> Vec<&'static str> {
        let mut bad = Vec::new();
        let scrypt = self.kdf == KDF;
        if scrypt && !self.r_written {
            bad.push("r");
        }
        if scrypt && !self.p_written {
            bad.push("p");
        }
        if self.pbkdf2() && self.prf != PRF {
            bad.push("prf");
        }
        if self.pbkdf2() && !self.c_written {
            bad.push("c");
        }
        if !matches!(self.mac_len, Some(Some(32)) | Some(None)) {
            bad.push("mac");
        }
        if !matches!(self.ciphertext_len, Some(Some(32)) | Some(None)) {
            bad.push("ciphertext");
        }
        if self.version != VERSION {
            bad.push("version");
        }
        if !scrypt && !self.pbkdf2() {
            bad.push("kdf");
        }
        if self.cipher != CIPHER {
            bad.push("cipher");
        }
        if scrypt && !n_shaped_for(self.n, self.r) {
            bad.push("n");
        }
        if self.dklen != DKLEN {
            bad.push("dklen");
        }
        if self.salt_len == 0 {
            bad.push("salt");
        }
        if self.iv_len != 16 {
            bad.push("iv");
        }
        // `address` is optional (foundry's files lack it); only a present but malformed one fails. The key's
        // address is computed from the decrypted key, not taken from this field.
        if !self.address_text.is_empty() && self.address.is_none() {
            bad.push("address");
        }
        bad
    }
}

/// Decrypt with a typed password. A password longer than `secret::CAP` bytes is refused before any work,
/// as on the write side; a truncated password would only be reported as a wrong one.
pub fn decrypt_typed(bytes: &[u8], password: &crate::secret::Secret) -> Result<Secret, Fault> {
    if password.overflowed() {
        return Err(Fault::known(Known::PasswordLong, crate::secret::CAP.to_string()));
    }
    decrypt(bytes, password.expose())
}

/// Decrypt. The MAC is checked first, so a wrong password fails there instead of decrypting garbage and
/// reporting an address mismatch.
pub fn decrypt(bytes: &[u8], password: &str) -> Result<Secret, Fault> {
    // Traced here so direct calls that bypass `apply` (tests, the CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::H2);
    let s = shape(bytes)?;
    // Unknown form, then out-of-bounds parameters: two distinct refusals, both before any derivation work and
    // distinct from a wrong password, which only the derivation can reveal.
    let bad = s.unshaped();
    if !bad.is_empty() {
        return Err(Fault::known(Known::KeystoreShape, crate::lang::filln(crate::lang::Key::Tail169, &[&(bad.join(" ")).to_string()])));
    }
    let over = out_of_bounds(s.n, s.r, s.p);
    // A pbkdf2 file is bounded by its round count instead.
    let over = if s.pbkdf2() { c_out_of_bounds(s.c) } else { over };
    if !over.is_empty() {
        return Err(Fault::known(Known::KeystoreParams, format!("{} · {}", s.params_said(), over.join(" "))));
    }
    let v = json::parse(bytes)
        .map_err(|t| Fault::known(Known::KeystoreShape, crate::lang::filln(crate::lang::Key::Tail165, &[&format!("{:?}", t)])))?;
    let c = field(&v, "crypto").expect("形已过");
    let kp = field(c, "kdfparams").expect("形已过");
    let cp = field(c, "cipherparams").expect("形已过");
    let salt = unbare(&text(kp, "salt"))
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::t(crate::lang::Key::Tail170).to_string()))?;
    let iv: [u8; 16] = unbare(&text(cp, "iv"))
        .and_then(|x| x.try_into().ok())
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::t(crate::lang::Key::Tail171).to_string()))?;
    let cipher_text = unbare(&text(c, "ciphertext"))
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::t(crate::lang::Key::Tail172).to_string()))?;
    let want = unbare(&text(c, "mac"))
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::t(crate::lang::Key::Tail173).to_string()))?;

    let mut dk = [0u8; DKLEN];
    // The round count is within `MAX_C` here (the bounds passed), so it fits the derivation's 32 bits.
    let derived = if s.pbkdf2() {
        u32::try_from(s.c).map(|c| cryptx::pbkdf2_sha256(password.as_bytes(), &salt, c, &mut dk)).unwrap_or(false)
    } else {
        cryptx::scrypt(password.as_bytes(), &salt, s.n, s.r, s.p, &mut dk)
    };
    if !derived {
        return Err(Fault::known(Known::KeystoreParams, s.params_said()));
    }
    let got = mac_of(&dk, &cipher_text);
    if got.as_slice() != want.as_slice() {
        return Err(Fault::known(Known::BadPassword, crate::lang::t(crate::lang::Key::Tail174).to_string()));
    }
    let mut clear = cipher_text.clone();
    let ekey: [u8; 16] = dk[..16].try_into().expect("十六字节");
    cryptx::aes128_ctr(&ekey, &iv, &mut clear);
    let raw: [u8; 32] = clear
        .as_slice()
        .try_into()
        .map_err(|_| Fault::known(Known::KeystoreShape, crate::lang::filln(crate::lang::Key::Tail175, &[&(clear.len()).to_string()])))?;
    let secret = Secret::take(raw)
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail176).to_string()))?;
    // A present `address` must match the decrypted key: a mismatch means the file contradicts itself, and
    // silently trusting either half would be wrong.
    if let Some(claimed) = s.address {
        let got = secret
            .address()
            .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
        if got != claimed {
            return Err(Fault::known(
                Known::AddressMismatch,
                crate::lang::filln(crate::lang::Key::Tail177, &[&(claimed.hex()).to_string(), &(got.hex()).to_string()]),
            ));
        }
    }
    Ok(secret)
}

#[cfg(test)]
mod fixed_bytes {
    use super::*;

    /// Encrypts with fixed inputs and checks the round trip. With `KS_FIXED_OUT` set, also writes the bytes
    /// for comparison across dependency changes.
    fn one(p: Params) -> Vec<u8> {
        let secret = Secret::take([0x42u8; 32]).expect("阶内");
        let address = secret.address().expect("地址");
        let ks = encrypt_with(&secret, &address, "zk-r9-口令", p, &[0x11u8; 32], &[0x22u8; 16], "00000000-0000-4000-8000-000000000000", 0)
            .expect("加密");
        let back = decrypt(&ks.json, "zk-r9-口令").expect("解密");
        assert_eq!(back.address(), Some(address));
        ks.json
    }

    #[test]
    fn ks_fixed_dump() {
        let light = one(Params::light());
        if let Ok(out) = std::env::var("KS_FIXED_OUT") {
            std::fs::write(format!("{out}-light.json"), &light).unwrap();
            std::fs::write(format!("{out}-standard.json"), one(Params::standard())).unwrap();
        }
    }
}


// ───────────────────────── Backup password strength reading ─────────────────────────

/// Password strength levels. Advisory only: no password is rejected for being weak.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strength {
    /// Estimated entropy below 50 bits.
    Weak,
    /// 50 to 69 bits.
    Fair,
    /// 70 bits and up.
    Strong,
}

impl Strength {
    pub fn as_str(self) -> &'static str {
        match self {
            Strength::Weak => "weak",
            Strength::Fair => "fair",
            Strength::Strong => "strong",
        }
    }
}

/// Estimated entropy in bits: effective length × log2(character pool).
///
/// The pool sums the classes used: digits 10, lowercase 26, uppercase 26, ASCII symbols 33 (including
/// space), anything else 100. In the effective length each run of repeats (`aaaa`) or ±1 sequences (`1234`,
/// `dcba`) counts as at most 2 characters.
pub fn entropy_bits(pw: &str) -> f64 {
    let cs: Vec<char> = pw.chars().collect();
    if cs.is_empty() {
        return 0.0;
    }
    let (mut digit, mut lower, mut upper, mut sym, mut other) = (false, false, false, false, false);
    for c in &cs {
        match c {
            '0'..='9' => digit = true,
            'a'..='z' => lower = true,
            'A'..='Z' => upper = true,
            c if c.is_ascii() => sym = true,
            _ => other = true,
        }
    }
    let pool = [(digit, 10.0), (lower, 26.0), (upper, 26.0), (sym, 33.0), (other, 100.0)]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| n)
        .sum::<f64>();
    // Effective length: runs of repeats or ±1 steps count as at most 2.
    let mut eff = 0usize;
    let mut i = 0usize;
    while i < cs.len() {
        let mut j = i + 1;
        if j < cs.len() {
            let step = cs[j] as i64 - cs[i] as i64;
            if step == 0 || step == 1 || step == -1 {
                while j + 1 < cs.len() && cs[j + 1] as i64 - cs[j] as i64 == step {
                    j += 1;
                }
                eff += (j - i + 1).min(2);
                i = j + 1;
                continue;
            }
        }
        eff += 1;
        i += 1;
    }
    eff as f64 * pool.log2()
}

/// Weak below 50 bits, fair from 50 to 69, strong from 70.
pub fn strength(pw: &str) -> Strength {
    let bits = entropy_bits(pw);
    if bits < 50.0 {
        Strength::Weak
    } else if bits < 70.0 {
        Strength::Fair
    } else {
        Strength::Strong
    }
}

#[cfg(test)]
mod strength_tests {
    use super::*;

    #[test]
    fn eight_digits_in_a_run_are_weak_and_a_long_mixed_phrase_is_strong() {
        assert_eq!(strength("12345678"), Strength::Weak);
        assert_eq!(strength("aaaaaaaaaaaaaaaaaaaa"), Strength::Weak);
        assert_eq!(strength("correct Horse battery 9 staple!"), Strength::Strong);
    }
}
