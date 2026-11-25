use crate::fault::{Fault, Known};
use crate::key::{Address, Secret};
use crate::cryptx;
use zikaron::hexfmt;
use zikaron::json::{self, Value};

pub const KDF: &str = "scrypt";
pub const KDF_PBKDF2: &str = "pbkdf2";
pub const PRF: &str = "hmac-sha256";
pub const CIPHER: &str = "aes-128-ctr";
pub const VERSION: u64 = 3;

pub use zikaron_seal::kdf::{c_out_of_bounds, in_range, mac_of, n_shaped, n_shaped_for, out_of_bounds, Params, BACKUP_FLOOR, DKLEN, MAX_C, MAX_MEMORY, MAX_P, MAX_WORK, MIN_C, MIN_R};

pub struct Keystore {
    pub json: Vec<u8>,
    pub address: Address,
    pub file_name: String,
}

fn rand(n: usize) -> Result<Vec<u8>, Fault> {
    crate::key::random(n)
}

fn bare(b: &[u8]) -> String {
    hexfmt::encode(b).trim_start_matches("0x").to_string()
}

fn unbare(s: &str) -> Option<Vec<u8>> {
    hexfmt::decode(&format!("0x{s}"))
}

fn uuid() -> Result<String, Fault> {
    let mut b = rand(16)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = bare(&b);
    Ok(format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]))
}

pub fn file_name(address: &Address, unix_secs: u64) -> String {
    format!("UTC--{}.000000000Z--{}", utc(unix_secs), bare(&address.0))
}

pub fn utc(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = crate::when::civil(unix_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}")
}

pub fn encrypt(secret: &Secret, password: &str, p: Params, unix_secs: u64) -> Result<Keystore, Fault> {
    crate::trace::mark(crate::feature::Feature::H2);
    if !p.ok() {
        return Err(Fault::known(Known::KeystoreParams, format!("n={} r={} p={}", p.n, p.r, p.p)));
    }
    let address = secret
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail013).to_string())))?;
    let salt = rand(32)?;
    let iv: [u8; 16] = rand(16)?.try_into().expect("十六字节");
    let id = uuid()?;
    encrypt_with(secret, &address, password, p, &salt, &iv, &id, unix_secs)
}

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
    let cipher_text = secret.ciphered(&ekey, iv);
    let mac = mac_of(&dk, &cipher_text);
    let json = write(&address, salt, iv, &cipher_text, &mac, p, id);
    Ok(Keystore { json, address, file_name: file_name(&address, unix_secs) })
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
    json::canon_bytes(&doc)
}

pub struct Shape {
    pub version: u64,
    pub kdf: String,
    pub cipher: String,
    pub prf: String,
    pub c: u64,
    pub c_written: bool,
    pub n: usize,
    pub r: usize,
    pub p: usize,
    pub dklen: usize,
    pub salt_len: usize,
    pub iv_len: usize,
    pub address_text: String,
    pub address: Option<Address>,
    pub r_written: bool,
    pub p_written: bool,
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

pub fn shape(bytes: &[u8]) -> Result<Shape, Fault> {
    let v = json::parse(bytes)
        .map_err(|t| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail165, &[&format!("{:?}", t)]))))?;
    let c = field(&v, "crypto")
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail166).to_string())))?;
    let kp = field(c, "kdfparams")
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail167).to_string())))?;
    let cp = field(c, "cipherparams")
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail168).to_string())))?;
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

fn hex_member(v: &Value, k: &str) -> Option<Option<usize>> {
    match field(v, k) {
        Some(Value::Str(s)) if !s.is_empty() => Some(unbare(s).map(|x| x.len())),
        _ => None,
    }
}

impl Shape {
    pub fn pbkdf2(&self) -> bool {
        self.kdf == KDF_PBKDF2
    }

    pub fn compliant(&self) -> Vec<&'static str> {
        let mut bad = self.unshaped();
        bad.extend(self.over());
        bad
    }

    pub fn over(&self) -> Vec<&'static str> {
        if self.pbkdf2() { c_out_of_bounds(self.c) } else { out_of_bounds(self.n, self.r, self.p) }
    }

    fn params_said(&self) -> String {
        if self.pbkdf2() { format!("c={}", self.c) } else { format!("n={} r={} p={}", self.n, self.r, self.p) }
    }

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
        if !self.address_text.is_empty() && self.address.is_none() {
            bad.push("address");
        }
        bad
    }
}

pub fn decrypt_typed(bytes: &[u8], password: &crate::secret::Secret) -> Result<Secret, Fault> {
    if password.overflowed() {
        return Err(Fault::known(Known::PasswordLong, crate::secret::CAP.to_string()));
    }
    decrypt(bytes, password.expose())
}

pub fn decrypt(bytes: &[u8], password: &str) -> Result<Secret, Fault> {
    crate::trace::mark(crate::feature::Feature::H2);
    let s = shape(bytes)?;
    let bad = s.unshaped();
    if !bad.is_empty() {
        return Err(Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail169, &[&(bad.join(" ")).to_string()]))));
    }
    let over = out_of_bounds(s.n, s.r, s.p);
    let over = if s.pbkdf2() { c_out_of_bounds(s.c) } else { over };
    if !over.is_empty() {
        return Err(Fault::known(Known::KeystoreParams, crate::lang::both_of(|| format!("{} · {}", s.params_said(), over.join(" ")))));
    }
    let v = json::parse(bytes)
        .map_err(|t| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail165, &[&format!("{:?}", t)]))))?;
    let c = field(&v, "crypto").expect("形已过");
    let kp = field(c, "kdfparams").expect("形已过");
    let cp = field(c, "cipherparams").expect("形已过");
    let salt = unbare(&text(kp, "salt"))
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail170).to_string())))?;
    let iv: [u8; 16] = unbare(&text(cp, "iv"))
        .and_then(|x| x.try_into().ok())
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail171).to_string())))?;
    let cipher_text = unbare(&text(c, "ciphertext"))
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail172).to_string())))?;
    let want = unbare(&text(c, "mac"))
        .ok_or_else(|| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail173).to_string())))?;

    let mut dk = [0u8; DKLEN];
    let derived = if s.pbkdf2() {
        u32::try_from(s.c).map(|c| cryptx::pbkdf2_sha256(password.as_bytes(), &salt, c, &mut dk)).unwrap_or(false)
    } else {
        cryptx::scrypt(password.as_bytes(), &salt, s.n, s.r, s.p, &mut dk)
    };
    if !derived {
        return Err(Fault::known(Known::KeystoreParams, crate::lang::both_of(|| s.params_said())));
    }
    let got = mac_of(&dk, &cipher_text);
    if got.as_slice() != want.as_slice() {
        return Err(Fault::known(Known::BadPassword, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail174).to_string())));
    }
    let mut clear = cipher_text.clone();
    let ekey: [u8; 16] = dk[..16].try_into().expect("十六字节");
    cryptx::aes128_ctr(&ekey, &iv, &mut clear);
    let raw: [u8; 32] = clear
        .as_slice()
        .try_into()
        .map_err(|_| Fault::known(Known::KeystoreShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail175, &[&(clear.len()).to_string()]))))?;
    let secret = Secret::take(raw)
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail176).to_string())))?;
    if let Some(claimed) = s.address {
        let got = secret
            .address()
            .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail013).to_string())))?;
        if got != claimed {
            return Err(Fault::known(
                Known::AddressMismatch,
                crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail177, &[&(claimed.hex()).to_string(), &(got.hex()).to_string()])),
            ));
        }
    }
    Ok(secret)
}

#[cfg(test)]
mod fixed_bytes {
    use super::*;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strength {
    Weak,
    Fair,
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
