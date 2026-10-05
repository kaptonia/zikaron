//! keystore V3: the only thing written to disk for an identity key file backup.
//!
//! Requirements: a user-set password, scrypt plus AES-128-CTR, the `UTC--` naming convention; plaintext private
//! keys and recovery words never land on disk; BIP-39 generates only identity keys.
//!
//! This code produces file backups of identity keys (one file per key). scrypt and AES come from the `cryptx`
//! boundary (the only app module allowed third-party cryptography).
//!
//! ─── Why only this lands on disk ───
//!
//! This file is the key's exit from this machine, and what passes an exit must be ciphertext: every path this
//! file writes goes through `cryptx::aes128_ctr`, and the plaintext lives only in memory (`key::Secret`,
//! zeroed when out of scope).
//!
//! ─── Three testable behaviors ───
//!
//! 1. Round trip: decrypting with the password gives the same address;
//! 2. V3 parameter compliance: `version` is 3, `kdf` is scrypt, `cipher` is aes-128-ctr, `n` is a power of
//! two, `dklen` is 32, salt and iv have the right lengths;
//! 3. Cross-implementation import: an independent implementation opening the same file gets the same address
//! (this file provides the file and the address).
//!
//! The MAC is `keccak256(dk[16..32] ‖ ciphertext)` (from the core's `cryptox`), so a wrong password fails
//! here first, instead of decrypting garbage with a wrong key and then saying "the address does not match".

use crate::fault::{Fault, Known};
use crate::key::{Address, Secret};
use crate::cryptx;
use zikaron::cryptox;
use zikaron::hexfmt;
use zikaron::json::{self, Value};

/// Names of the KDF and cipher, verbatim from the V3 specification. One name, one home.
pub const KDF: &str = "scrypt";
pub const CIPHER: &str = "aes-128-ctr";
pub const VERSION: u64 = 3;
/// Derived key length: the first sixteen bytes are the key, the last sixteen feed the MAC.
pub const DKLEN: usize = 32;

/// The three scrypt parameters. Both levels are compliant V3: the standard level is what the product uses,
/// the light level is for places that run many passes (tests); the two have the same shape and differ only
/// in n.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Params {
    pub n: usize,
    pub r: usize,
    pub p: usize,
}

impl Params {
    /// The level the product uses (the same numbers as geth's standard).
    pub fn standard() -> Params {
        Params { n: 262_144, r: 8, p: 1 }
    }

    /// The light level (the same numbers as geth's light). Compliant, just without the slower half.
    pub fn light() -> Params {
        Params { n: 4_096, r: 8, p: 6 }
    }

    fn ok(&self) -> bool {
        in_range(self.n, self.r, self.p)
    }
}

/// The closed range accepted for scrypt parameters, the family's table: memory `128·r·n` at most 2^30 bytes,
/// parallelism `p` from 1 to 16, total work `128·r·n·p` at most 2^32 bytes, `r` at least 1 (and `n` a power of
/// two from 2, which is the shape scrypt itself requires). These come from a file someone provides; checking
/// only "power of two, non-zero" would let a file demand terabytes of memory or days of computation (the
/// victim is whoever opens a bad file), and a bound set at one tool's own level would refuse another tool's
/// compliant file (the victim is whoever brings one). The same table bounds every reader of scrypt parameters:
/// key files, the key store, whole-machine backups. What the product itself writes stays at the standard
/// level ([`Params::standard`]; see `backup::params`).
pub const MAX_MEMORY: u128 = 1 << 30;
pub const MAX_WORK: u128 = 1 << 32;
pub const MAX_P: usize = 16;
pub const MIN_R: usize = 1;

/// Whether `n` has the shape scrypt requires (a power of two from 2).
pub fn n_shaped(n: usize) -> bool {
    n >= 2 && n & (n - 1) == 0
}

/// Whether `n` and `r` have the shape scrypt itself requires together: `n` a power of two from 2, and below
/// 2^(16·r) (a larger `n` cannot be derived at all, whatever the bounds say).
pub fn n_shaped_for(n: usize, r: usize) -> bool {
    // `r` itself is a bound (at least 1): a zero `r` is said there, not here.
    n_shaped(n) && (r == 0 || r >= 4 || (n as u128) < (1u128 << (16 * r as u32)))
}

/// Which of the family's bounds these parameters break (`r`, `p`, `memory`, `work`), empty when none.
pub fn out_of_bounds(n: usize, r: usize, p: usize) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if r < MIN_R {
        bad.push("r");
    }
    if !(1..=MAX_P).contains(&p) {
        bad.push("p");
    }
    // Checked products: a file's numbers can be as large as its integers go; a product that does not fit is
    // out of bounds, never wrapped round into range.
    let memory = 128u128.checked_mul(r as u128).and_then(|x| x.checked_mul(n as u128));
    if memory.map(|m| m > MAX_MEMORY).unwrap_or(true) {
        bad.push("memory");
    }
    if memory.and_then(|m| m.checked_mul(p as u128)).map(|w| w > MAX_WORK).unwrap_or(true) {
        bad.push("work");
    }
    bad
}

/// Whether the three are within the closed range (shape and bounds asked in one place).
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

/// The `UTC--<time>--<address>` file name (geth convention). The time is given by the caller: this file does
/// not ask the clock, so it is testable and never asks the clock twice for two names.
pub fn file_name(address: &Address, unix_secs: u64) -> String {
    format!("UTC--{}.000000000Z--{}", utc(unix_secs), bare(&address.0))
}

/// A Unix second read as `2026-09-08T10-37-00`.
///
/// The only civil-time conversion (one name, one home): the file name needs it, and so does the face saying
/// "when was the last one".
pub fn utc(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = crate::when::civil(unix_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}")
}


/// Encrypt. Returns V3 bytes, the address and the file name it should have.
pub fn encrypt(secret: &Secret, password: &str, p: Params, unix_secs: u64) -> Result<Keystore, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, the
    // CLI) are traced too.
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

/// The deterministic half of encryption. Salt, iv and id are given by the caller: `encrypt` takes them from
/// system entropy, and tests feed fixed values for byte comparison across dependency changes (same key,
/// password, salt and iv give byte-identical output).
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
    // Not one plaintext byte passes this layer. The private key's only way out is `Secret::ciphered`, which
    // hands out ciphertext already through `cryptx::aes128_ctr` (see the exits in the `key` file header).
    let cipher_text = secret.ciphered(&ekey, iv);
    let mac = mac_of(&dk, &cipher_text);
    let json = write(&address, salt, iv, &cipher_text, &mac, p, id);
    Ok(Keystore { json, address, file_name: file_name(&address, unix_secs) })
}

/// MAC: `keccak256(dk[16..32] ‖ ciphertext)`. Public, because "a wrong password fails here first" needs a
/// point that can be tested on its own.
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
    // The byte form comes from the core's canonicalizer: member order, integer spelling and escaping are all
    // its own; this file invents none.
    json::canon_bytes(&doc)
}

/// Whether this file's shape is V3. Each field named: say which field is wrong, never just "bad format".
pub struct Shape {
    pub version: u64,
    pub kdf: String,
    pub cipher: String,
    pub n: usize,
    pub r: usize,
    pub p: usize,
    pub dklen: usize,
    pub salt_len: usize,
    pub iv_len: usize,
    /// The original text of the `address` field. It is optional in V3: geth writes it, foundry does not. So
    /// "absent" and "present but malformed" are recorded separately.
    pub address_text: String,
    pub address: Option<Address>,
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

/// Read this file's shape. Unreadable is refused by name.
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
        n: num(kp, "n") as usize,
        r: num(kp, "r") as usize,
        p: num(kp, "p") as usize,
        dklen: num(kp, "dklen") as usize,
        salt_len: unbare(&text(kp, "salt")).map(|x| x.len()).unwrap_or(0),
        iv_len: unbare(&text(cp, "iv")).map(|x| x.len()).unwrap_or(0),
        address_text: text(&v, "address"),
        address: Address::parse(&format!("0x{}", text(&v, "address"))),
    })
}

impl Shape {
    /// Whether the V3 parameters comply. Computed now, item by item.
    pub fn compliant(&self) -> Vec<&'static str> {
        let mut bad = self.unshaped();
        bad.extend(out_of_bounds(self.n, self.r, self.p));
        bad
    }

    /// The fields whose form this reader does not recognise (not the parameter bounds): version, kdf, cipher,
    /// `n` not a power of two, dklen, salt, iv, a malformed address.
    pub fn unshaped(&self) -> Vec<&'static str> {
        let mut bad = Vec::new();
        if self.version != VERSION {
            bad.push("version");
        }
        if self.kdf != KDF {
            bad.push("kdf");
        }
        if self.cipher != CIPHER {
            bad.push("cipher");
        }
        if !n_shaped_for(self.n, self.r) {
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
        // `address` may be absent (it is a convenience field in the specification, not required: foundry's
        // files lack it). Only present but malformed fails; the key's address is ultimately computed from the
        // decrypted key, not taken from this field.
        if !self.address_text.is_empty() && self.address.is_none() {
            bad.push("address");
        }
        bad
    }
}

/// Decrypt. Check the MAC first, then decrypt; a wrong password fails at the MAC step, never decrypting
/// garbage and then saying the address does not match.
pub fn decrypt(bytes: &[u8], password: &str) -> Result<Secret, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, the
    // CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::H2);
    let s = shape(bytes)?;
    // A form this reader does not know, then parameters out of the family's bounds: two refusals by two names,
    // both before any derivation work (and apart from a wrong password, which only the work can tell).
    let bad = s.unshaped();
    if !bad.is_empty() {
        return Err(Fault::known(Known::KeystoreShape, crate::lang::filln(crate::lang::Key::Tail169, &[&(bad.join(" ")).to_string()])));
    }
    let over = out_of_bounds(s.n, s.r, s.p);
    if !over.is_empty() {
        return Err(Fault::known(Known::KeystoreParams, format!("n={} r={} p={} · {}", s.n, s.r, s.p, over.join(" "))));
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
    if !cryptx::scrypt(password.as_bytes(), &salt, s.n, s.r, s.p, &mut dk) {
        return Err(Fault::known(Known::KeystoreParams, format!("n={} r={} p={}", s.n, s.r, s.p)));
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
    // If that field is present, it must match the decrypted key: a mismatch means the file contradicts
    // itself, and silently acting on one half of a self-contradictory file is the worst form.
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

    /// For comparison across dependency changes: same key, password, salt, iv and id give the bytes written.
    /// The file is written only when `KS_FIXED_OUT` is given; otherwise only the round trip is checked.
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

/// Three strength levels. A reminder only: no password is blocked by it (there is no weak password gate).
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

/// Estimated entropy (bits). A pure function: estimated entropy = effective length × log2(character pool).
///
/// The pool adds up by the classes used: digits 10, lowercase 26, uppercase 26, ASCII symbols 33 (including
/// space), any other character class 100. Effective length: each run of repeats (`aaaa`) or sequences
/// (`1234`, `dcba`, step always ±1) counts as only 2 characters.
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
    // Effective length: split the string into runs of "the same character repeated" or "sequence with step
    // ±1"; runs longer than 2 count as 2.
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

/// Three reading levels: weak < 50 bits, fair 50 to 69 bits, strong ≥ 70 bits.
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
