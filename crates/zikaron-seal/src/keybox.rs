use crate::kdf::Params;
use crate::trouble::{Code, Trouble};
use std::path::{Path, PathBuf};
use zikaron::hexfmt;
use zikaron::json::{self, Value};

pub const SHAPE: &str = "zikaron-desk/keybox/3";
pub const SHAPE_V2: &str = "zikaron-desk/keybox/2";
pub const SHAPE_V1: &str = "zikaron-desk/keybox/1";

pub const IV_LEN: usize = 16;

pub mod member {
    pub const SHAPE: &str = "shape";
    pub const WRONG: &str = "wrong";
    pub const KDF: &str = "kdf";
    pub const MARKS: &str = "marks";
    pub const PRIMARY: &str = "primary";
    pub const PIN: &str = "pin";
    pub const RECOVERY: &str = "recovery";
    pub const SLOTS: &str = "slots";
    pub const CT: &str = "ct";
    pub const IV: &str = "iv";
    pub const SALT: &str = "salt";
    pub const MAC: &str = "mac";
    pub const BIND: &str = "bind";
    pub const ID: &str = "id";
    pub const TAG: &str = "tag";
    pub const NAME: &str = "name";
    pub const ACCOUNT: &str = "account";
    pub const KIND: &str = "kind";
    pub const LABEL: &str = "label";
    pub const N: &str = "n";
    pub const R: &str = "r";
    pub const P: &str = "p";
}
pub const PIN_LEN: usize = 8;
pub const BIND_DOMAIN: &[u8] = b"zikaron-keybox/2";
pub const MEMBER_INFO: &[u8] = b"zikaron/keybox/member/v1";
pub const WRONG_LIMIT: u64 = 5;

const MASTER_BYTES: usize = 32;

static MASTER: std::sync::Mutex<Option<[u8; MASTER_BYTES]>> = std::sync::Mutex::new(None);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Absent,
    Locked { wrong: u64 },
    LockedOut,
    Open,
}

impl State {
    pub fn gate_up(self) -> bool {
        match self {
            State::Absent => false,
            State::Locked { .. } => true,
            State::LockedOut => true,
            State::Open => false,
        }
    }

    pub fn keys_ready(self) -> bool {
        match self {
            State::Absent => false,
            State::Locked { .. } => false,
            State::LockedOut => false,
            State::Open => true,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            State::Absent => "absent",
            State::Locked { .. } => "locked",
            State::LockedOut => "locked-out",
            State::Open => "open",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Sealed {
    salt: Vec<u8>,
    iv: [u8; IV_LEN],
    ct: Vec<u8>,
    mac: Option<[u8; 32]>,
    bind: Option<[u8; 32]>,
}

#[derive(Clone, Copy)]
enum Kind {
    Pin,
    Recovery,
    Slot,
    PrimaryId,
}

impl Kind {
    fn tag(self) -> &'static [u8] {
        match self {
            Kind::Pin => b"pin",
            Kind::Recovery => b"recovery",
            Kind::Slot => b"slot",
            Kind::PrimaryId => b"primary-id",
        }
    }
}

pub struct Plain(Vec<u8>);

impl Plain {
    pub fn new(b: Vec<u8>) -> Plain {
        Plain(b)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for Plain {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

struct Key32([u8; 32]);

impl Drop for Key32 {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

#[derive(Clone, Debug, Default)]
struct Book {
    pin: Option<Sealed>,
    kdf: Option<Params>,
    recovery: Vec<Recovery>,
    slots: Vec<Slot>,
    wrong: u64,
    primary: Option<Primary>,
    marks: Option<[u8; 32]>,
}

#[derive(Clone, Debug)]
struct Slot {
    name: String,
    sealed: Sealed,
    tag: [u8; 32],
}

#[derive(Clone, Debug)]
struct Primary {
    kind: PrimaryKind,
    id_seal: Sealed,
}

#[derive(Clone, Debug)]
struct Recovery {
    label: String,
    tag: [u8; 32],
    sealed: Sealed,
}

const PRIMARY_LABEL: &str = "primary";

fn labels(kind: PrimaryKind) -> &'static [&'static str] {
    match kind {
        PrimaryKind::Words => &[PRIMARY_LABEL, "primary-author", "primary-grantee"],
        PrimaryKind::KeyFile => &[PRIMARY_LABEL],
    }
}

fn recovery_seals(mk: &[u8; MASTER_BYTES], kind: PrimaryKind, secrets: &[Plain], kdf: Params, marks: &[u8; 32]) -> Result<Vec<Recovery>, Trouble> {
    if secrets.len() != labels(kind).len() {
        return Err(Trouble::new(Code::KeyboxShape, member::RECOVERY.to_string()));
    }
    let mut out = Vec::new();
    for (label, sec) in labels(kind).iter().zip(secrets) {
        let salt = rand(32)?;
        let dk = derive(kdf, sec.bytes(), &salt)?;
        out.push(Recovery { label: label.to_string(), tag: member_tag(sec.bytes(), marks), sealed: seal_master(&dk, salt, mk, Kind::Recovery, label, kdf)? });
    }
    Ok(out)
}

fn member_tag(secret: &[u8], marks: &[u8; 32]) -> [u8; 32] {
    let mut msg = MEMBER_INFO.to_vec();
    msg.push(0);
    msg.extend_from_slice(marks);
    crate::crypto::hmac_sha256(secret, &msg)
}

fn marks_of(book: &mut Book) -> Result<[u8; 32], Trouble> {
    match book.marks {
        Some(m) => Ok(m),
        None => {
            let m = new_marks()?;
            book.marks = Some(m);
            Ok(m)
        }
    }
}

fn new_marks() -> Result<[u8; 32], Trouble> {
    let v = rand(32)?;
    let mut m = [0u8; 32];
    m.copy_from_slice(&v);
    Ok(m)
}

fn slot_plain(account: &str, secret: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 + account.len() + secret.len());
    v.extend_from_slice(&(account.len() as u16).to_be_bytes());
    v.extend_from_slice(account.as_bytes());
    v.extend_from_slice(secret);
    v
}

fn slot_split(plain: &[u8]) -> Option<(String, Vec<u8>)> {
    if plain.len() < 2 {
        return None;
    }
    let n = u16::from_be_bytes([plain[0], plain[1]]) as usize;
    let account = std::str::from_utf8(plain.get(2..2 + n)?).ok()?.to_string();
    Some((account, plain.get(2 + n..)?.to_vec()))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrimaryKind {
    Words,
    KeyFile,
}

impl PrimaryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PrimaryKind::Words => "words",
            PrimaryKind::KeyFile => "key-file",
        }
    }

    fn parse(s: &str) -> Option<PrimaryKind> {
        match s {
            "words" => Some(PrimaryKind::Words),
            "key-file" => Some(PrimaryKind::KeyFile),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Vault {
    dir: PathBuf,
    file: String,
    lock: String,
}

impl Vault {
    pub fn new(dir: PathBuf, file: String, lock: String) -> Vault {
        Vault { dir, file, lock }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.file)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    fn lock_path(&self) -> PathBuf {
        self.dir.join(&self.lock)
    }
}

pub struct Held {
    _file: std::fs::File,
}

fn lock_book(v: &Vault) -> Result<Held, Trouble> {
    let p = v.lock_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| Trouble::io(&e, &d.display().to_string()))?;
    }
    let mut o = zikaron_os::Options::new();
    o.create(true).read(true).write(true).truncate(false);
    zikaron_os::owner_only(&mut o);
    let file = o.open(&p).map_err(|e| Trouble::io(&e, &p.display().to_string()))?;
    if !zikaron_os::lock_wait(&file) {
        return Err(Trouble::new(Code::KeyboxLocked, p.display().to_string()));
    }
    Ok(Held { _file: file })
}

fn rand(n: usize) -> Result<Vec<u8>, Trouble> {
    crate::trouble::random(n)
}

fn bare(b: &[u8]) -> String {
    hexfmt::encode(b).trim_start_matches("0x").to_string()
}

fn unbare(s: &str) -> Option<Vec<u8>> {
    hexfmt::decode(&format!("0x{s}"))
}

fn text(v: &Value, k: &str) -> String {
    match v.member(k) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn seal_of(v: &Value, kind: Kind) -> Option<Sealed> {
    let salt = unbare(&text(v, member::SALT))?;
    let iv_b = unbare(&text(v, member::IV))?;
    let ct = unbare(&text(v, member::CT))?;
    let tag32 = |k: &str| -> Result<Option<[u8; 32]>, ()> {
        match v.member(k) {
            None => Ok(None),
            Some(Value::Str(x)) => {
                let b = unbare(x).ok_or(())?;
                let mut out = [0u8; 32];
                if b.len() != 32 {
                    return Err(());
                }
                out.copy_from_slice(&b);
                Ok(Some(out))
            }
            Some(_) => Err(()),
        }
    };
    let mac = tag32(member::MAC).ok()?;
    let bind = tag32(member::BIND).ok()?;
    if iv_b.len() != IV_LEN || salt.is_empty() || ct.is_empty() {
        return None;
    }
    let want = match kind {
        Kind::Slot | Kind::PrimaryId => mac.is_none() && bind.is_some(),
        _ => mac.is_some() && bind.is_some(),
    };
    if !want {
        return None;
    }
    let mut iv = [0u8; IV_LEN];
    iv.copy_from_slice(&iv_b);
    Some(Sealed { salt, iv, ct, mac, bind })
}

fn seal_value(s: &Sealed) -> Value {
    let mut m = vec![
        (member::CT.into(), Value::Str(bare(&s.ct))),
        (member::IV.into(), Value::Str(bare(&s.iv))),
        (member::SALT.into(), Value::Str(bare(&s.salt))),
    ];
    if let Some(mac) = &s.mac {
        m.push((member::MAC.into(), Value::Str(bare(mac))));
    }
    if let Some(b) = &s.bind {
        m.push((member::BIND.into(), Value::Str(bare(b))));
    }
    m.sort_by(|a: &(String, Value), b: &(String, Value)| a.0.cmp(&b.0));
    Value::Obj(m)
}

fn read_book(v: &Vault) -> Result<Option<Book>, Trouble> {
    let p = v.path();
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Trouble::io(&e, &p.display().to_string())),
    };
    let v = json::parse(&bytes)
        .map_err(|t| Trouble::new(Code::KeyboxShape, format!("{}: {t:?}", p.display())))?;
    match text(&v, member::SHAPE).as_str() {
        SHAPE => {}
        other => return Err(Trouble::new(Code::KeyboxShape, other.to_string())),
    }
    let wrong = match v.member(member::WRONG) {
        Some(Value::Int(n)) => *n,
        _ => return Err(Trouble::new(Code::KeyboxShape, member::WRONG.to_string())),
    };
    let mut book = Book { wrong, ..Book::default() };
    let Some(k) = v.member(member::KDF) else {
        return Err(Trouble::new(Code::KeyboxShape, member::KDF.to_string()));
    };
    {
        let num = |name: &str| -> Option<usize> {
            match k.member(name) {
                Some(Value::Int(n)) => usize::try_from(*n).ok(),
                _ => None,
            }
        };
        let (Some(n), Some(r), Some(pp)) = (num(member::N), num(member::R), num(member::P)) else {
            return Err(Trouble::new(Code::KeyboxShape, member::KDF.to_string()));
        };
        if !crate::kdf::in_range(n, r, pp) {
            return Err(Trouble::new(Code::KeyboxShape, format!("kdf n={n} r={r} p={pp}")));
        }
        book.kdf = Some(Params { n, r, p: pp });
    }
    let tag32 = |x: Option<&Value>| -> Result<Option<[u8; 32]>, ()> {
        match x {
            None => Ok(None),
            Some(Value::Str(h)) => {
                let b = unbare(h).ok_or(())?;
                <[u8; 32]>::try_from(b.as_slice()).map(Some).map_err(|_| ())
            }
            Some(_) => Err(()),
        }
    };
    book.marks = Some(tag32(v.member(member::MARKS)).ok().flatten().ok_or_else(|| Trouble::new(Code::KeyboxShape, member::MARKS.to_string()))?);
    if let Some(p) = v.member(member::PRIMARY) {
        let bad = || Trouble::new(Code::KeyboxShape, member::PRIMARY.to_string());
        book.primary = Some(Primary {
            kind: PrimaryKind::parse(&text(p, member::KIND)).ok_or_else(bad)?,
            id_seal: p.member(member::ID).and_then(|x| seal_of(x, Kind::PrimaryId)).ok_or_else(bad)?,
        });
    }
    if let Some(p) = v.member(member::PIN) {
        book.pin = seal_of(p, Kind::Pin);
        if book.pin.is_none() {
            return Err(Trouble::new(Code::KeyboxShape, member::PIN.to_string()));
        }
    }
    if let Some(Value::Arr(a)) = v.member(member::RECOVERY) {
        for one in a {
            let label = text(one, member::LABEL);
            let (Some(sealed), Ok(Some(tag))) = (seal_of(one, Kind::Recovery), tag32(one.member(member::TAG))) else {
                return Err(Trouble::new(Code::KeyboxShape, format!("recovery {label}")));
            };
            book.recovery.push(Recovery { label, tag, sealed });
        }
    }
    let have: Vec<&str> = book.recovery.iter().map(|r| r.label.as_str()).collect();
    match &book.primary {
        Some(p) if have == labels(p.kind) => {}
        None if have.is_empty() => {}
        Some(_) => return Err(Trouble::new(Code::KeyboxShape, member::RECOVERY.to_string())),
        None => return Err(Trouble::new(Code::KeyboxShape, member::PRIMARY.to_string())),
    }
    if let Some(Value::Arr(a)) = v.member(member::SLOTS) {
        for one in a {
            let name = text(one, member::NAME);
            let Some(sealed) = seal_of(one, Kind::Slot) else {
                return Err(Trouble::new(Code::KeyboxShape, format!("slot {name}")));
            };
            let Ok(Some(tag)) = tag32(one.member(member::TAG)) else {
                return Err(Trouble::new(Code::KeyboxShape, format!("slot {name}")));
            };
            book.slots.push(Slot { name, sealed, tag });
        }
    }
    Ok(Some(book))
}

fn write_book(v: &Vault, _held: &Held, b: &Book) -> Result<(), Trouble> {
    write_book_as(v, b, &v.file)
}

fn write_book_as(v: &Vault, b: &Book, name: &str) -> Result<(), Trouble> {
    let p = v.path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| Trouble::io(&e, &d.display().to_string()))?;
    }
    let with = |base: Value, extra: Vec<(String, Value)>| -> Value {
        match base {
            Value::Obj(mut f) => {
                f.extend(extra);
                f.sort_by(|x, y| x.0.cmp(&y.0));
                Value::Obj(f)
            }
            other => other,
        }
    };
    let mut m: Vec<(String, Value)> = Vec::new();
    if let Some(k) = &b.kdf {
        m.push((
            member::KDF.into(),
            Value::Obj(vec![
                (member::N.into(), Value::Int(k.n as u64)),
                (member::P.into(), Value::Int(k.p as u64)),
                (member::R.into(), Value::Int(k.r as u64)),
            ]),
        ));
    }
    if let Some(pin) = &b.pin {
        m.push((member::PIN.into(), seal_value(pin)));
    }
    if let Some(pr) = &b.primary {
        let mut f: Vec<(String, Value)> = vec![
            (member::KIND.into(), Value::Str(pr.kind.as_str().to_string())),
            (member::ID.into(), seal_value(&pr.id_seal)),
        ];
        f.sort_by(|x, y| x.0.cmp(&y.0));
        m.push((member::PRIMARY.into(), Value::Obj(f)));
    }
    m.push((
        member::RECOVERY.into(),
        Value::Arr(
            b.recovery
                .iter()
                .map(|r| with(seal_value(&r.sealed), vec![(member::LABEL.into(), Value::Str(r.label.clone())), (member::TAG.into(), Value::Str(bare(&r.tag)))]))
                .collect(),
        ),
    ));
    m.push((member::SHAPE.into(), Value::Str(SHAPE.to_string())));
    m.push((
        member::SLOTS.into(),
        Value::Arr(
            b.slots
                .iter()
                .map(|sl| with(seal_value(&sl.sealed), vec![(member::NAME.into(), Value::Str(sl.name.clone())), (member::TAG.into(), Value::Str(bare(&sl.tag)))]))
                .collect(),
        ),
    ));
    if let Some(mk) = &b.marks {
        m.push((member::MARKS.into(), Value::Str(bare(mk))));
    }
    m.push((member::WRONG.into(), Value::Int(b.wrong)));
    m.sort_by(|a, b| a.0.cmp(&b.0));
    let bytes = json::canon_bytes(&Value::Obj(m));
    crate::land::put_at(&v.dir, name, &bytes)
}

static KDF: std::sync::OnceLock<Params> = std::sync::OnceLock::new();

pub const PROBE_KDF: Params = Params { n: 2, r: 8, p: 1 };

pub fn set_light_kdf(p: Option<Params>) -> bool {
    let p = p.unwrap_or(PROBE_KDF);
    if !crate::kdf::in_range(p.n, p.r, p.p) {
        return false;
    }
    KDF.set(p).is_ok()
}

pub fn params() -> Params {
    *KDF.get_or_init(Params::standard)
}

fn wipe(b: &mut [u8]) {
    zikaron_os::wipe(b);
}

fn kdf_of(b: &Book) -> Params {
    b.kdf.unwrap_or_else(params)
}

pub fn floor() -> Params {
    params()
}

pub fn below_floor(k: Params) -> bool {
    let f = floor();
    let mem = |p: Params| p.n.saturating_mul(p.r);
    let work = |p: Params| p.n.saturating_mul(p.r).saturating_mul(p.p);
    mem(k) < mem(f) || work(k) < work(f)
}

fn kdf_text(k: Params) -> String {
    format!("n={} r={} p={}", k.n, k.r, k.p)
}

fn floor_gate(b: &Book) -> Result<(), Trouble> {
    let k = kdf_of(b);
    if below_floor(k) {
        return Err(Trouble::new(Code::KdfBelowFloor, format!("{} < {}", kdf_text(k), kdf_text(floor()))));
    }
    Ok(())
}

fn derive(p: Params, secret: &[u8], salt: &[u8]) -> Result<Key32, Trouble> {
    let mut dk = Key32([0u8; 32]);
    if !crate::crypto::scrypt(secret, salt, p.n, p.r, p.p, &mut dk.0) {
        return Err(Trouble::new(Code::KeystoreParams, format!("n={} r={} p={}", p.n, p.r, p.p)));
    }
    Ok(dk)
}

fn bind_of(mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: Params, s: &Sealed) -> [u8; 32] {
    let mut m: Vec<u8> = Vec::with_capacity(128 + s.ct.len());
    m.extend_from_slice(&mk[16..32]);
    m.extend_from_slice(BIND_DOMAIN);
    let mut field = |b: &[u8]| {
        m.extend_from_slice(&(b.len() as u32).to_be_bytes());
        m.extend_from_slice(b);
    };
    field(kind.tag());
    field(label.as_bytes());
    field(&s.iv);
    field(&(kdf.n as u64).to_be_bytes());
    field(&(kdf.r as u64).to_be_bytes());
    field(&(kdf.p as u64).to_be_bytes());
    field(&s.salt);
    field(&s.ct);
    let out = zikaron::cryptox::keccak256(&m);
    wipe(&mut m);
    out
}

fn bound(mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: Params, s: &Sealed) -> bool {
    s.bind == Some(bind_of(mk, kind, label, kdf, s))
}

fn seal(dk: &[u8; 32], salt: Vec<u8>, plain: &[u8]) -> Result<Sealed, Trouble> {
    let iv_b = rand(16)?;
    let mut iv = [0u8; IV_LEN];
    iv.copy_from_slice(&iv_b);
    let mut ct = plain.to_vec();
    let mut key = [0u8; 16];
    key.copy_from_slice(&dk[0..16]);
    crate::crypto::aes128_ctr(&key, &iv, &mut ct);
    wipe(&mut key);
    let mac = crate::kdf::mac_of(dk, &ct);
    Ok(Sealed { salt, iv, ct, mac: Some(mac), bind: None })
}

fn seal_master(dk: &Key32, salt: Vec<u8>, mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: Params) -> Result<Sealed, Trouble> {
    let mut s = seal(&dk.0, salt, mk)?;
    s.bind = Some(bind_of(mk, kind, label, kdf, &s));
    Ok(s)
}

fn seal_slot(mk: &[u8; MASTER_BYTES], account: &str, kdf: Params, plain: &[u8]) -> Result<Sealed, Trouble> {
    let salt = rand(32)?;
    let mut s = seal(mk, salt, plain)?;
    s.mac = None;
    s.bind = Some(bind_of(mk, Kind::Slot, account, kdf, &s));
    Ok(s)
}

fn unseal(dk: &[u8; 32], s: &Sealed) -> Option<Plain> {
    if let Some(mac) = s.mac {
        if crate::kdf::mac_of(dk, &s.ct) != mac {
            return None;
        }
    }
    let mut out = s.ct.clone();
    let mut key = [0u8; 16];
    key.copy_from_slice(&dk[0..16]);
    crate::crypto::aes128_ctr(&key, &s.iv, &mut out);
    wipe(&mut key);
    Some(Plain(out))
}

fn master_of(p: Plain) -> Option<Key32> {
    if p.0.len() != MASTER_BYTES {
        return None;
    }
    let mut mk = Key32([0u8; MASTER_BYTES]);
    mk.0.copy_from_slice(&p.0);
    Some(mk)
}

fn master_now() -> Result<Key32, Trouble> {
    let m = MASTER.lock().unwrap_or_else(|e| e.into_inner());
    match m.as_ref() {
        Some(k) => Ok(Key32(*k)),
        None => Err(Trouble::new(Code::Locked, String::new())),
    }
}

fn hold(mk: &Key32) {
    CLOSING.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut m = MASTER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = m.as_mut() {
        wipe(old);
    }
    *m = Some(mk.0);
}

pub fn hold_master(mk: &[u8; 32]) {
    hold(&Key32(*mk));
}

pub fn with_master<R>(f: impl FnOnce(&[u8; 32]) -> R) -> Result<R, Trouble> {
    let mk = master_now()?;
    Ok(f(&mk.0))
}

static CLOSING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn begin_lock() {
    CLOSING.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn lock() {
    CLOSING.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut m = MASTER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mk) = m.as_mut() {
        wipe(mk);
    }
    *m = None;
    PIN_DIGITS_ONLY.store(0, std::sync::atomic::Ordering::SeqCst);
}

static PIN_DIGITS_ONLY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn note_pin(pin: &str) {
    let v = if pin.chars().all(|c| c.is_ascii_digit()) { 1 } else { 2 };
    PIN_DIGITS_ONLY.store(v, std::sync::atomic::Ordering::SeqCst);
}

pub fn pin_digits_only() -> Option<bool> {
    match PIN_DIGITS_ONLY.load(std::sync::atomic::Ordering::SeqCst) {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

pub fn state(v: &Vault) -> Result<State, Trouble> {
    let open = MASTER.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    let Some(book) = read_book(v)? else {
        return Ok(State::Absent);
    };
    if book.pin.is_none() {
        return Ok(State::Absent);
    }
    if book.wrong >= WRONG_LIMIT {
        return Ok(State::LockedOut);
    }
    if open && !CLOSING.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(State::Open);
    }
    Ok(State::Locked { wrong: book.wrong })
}

pub fn tries_left(v: &Vault) -> Result<u64, Trouble> {
    let wrong = read_book(v)?.map(|b| b.wrong).unwrap_or(0);
    Ok(WRONG_LIMIT.saturating_sub(wrong))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PinTrouble {
    Shape,
    AllSame,
    Run,
    DateLike,
}

impl PinTrouble {
    pub fn as_str(self) -> &'static str {
        match self {
            PinTrouble::Shape => "shape",
            PinTrouble::AllSame => "all-same",
            PinTrouble::Run => "run",
            PinTrouble::DateLike => "date-like",
        }
    }
}

fn valid_ymd(y: u32, m: u32, d: u32) -> bool {
    if !(1900..=2199).contains(&y) || !(1..=12).contains(&m) || d == 0 {
        return false;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    d <= days[(m - 1) as usize]
}

pub fn pin_trouble(pin: &str) -> Option<PinTrouble> {
    let cs: Vec<char> = pin.chars().collect();
    if cs.len() != PIN_LEN || !cs.iter().all(|c| c.is_ascii_alphanumeric()) {
        return Some(PinTrouble::Shape);
    }
    if cs.iter().all(|c| *c == cs[0]) {
        return Some(PinTrouble::AllSame);
    }
    let b: Vec<u32> = cs.iter().filter_map(|c| c.to_digit(10)).collect();
    if b.len() != PIN_LEN {
        return None;
    }
    let up = b.windows(2).all(|w| w[1] == (w[0] + 1) % 10);
    let down = b.windows(2).all(|w| w[0] == (w[1] + 1) % 10);
    if up || down {
        return Some(PinTrouble::Run);
    }
    let n = |i: usize, len: usize| -> u32 { pin[i..i + len].parse().unwrap_or(0) };
    if valid_ymd(n(0, 4), n(4, 2), n(6, 2))
        || valid_ymd(n(4, 4), n(2, 2), n(0, 2))
        || valid_ymd(n(4, 4), n(0, 2), n(2, 2))
    {
        return Some(PinTrouble::DateLike);
    }
    None
}

pub fn set_pin(v: &Vault, pin: &str) -> Result<(), Trouble> {
    if let Some(t) = pin_trouble(pin) {
        return Err(Trouble::new(Code::PinShape, t.as_str().to_string()));
    }
    let held = lock_book(v)?;
    let mut book = read_book(v)?.unwrap_or_default();
    if book.pin.is_some() {
        return Err(Trouble::new(Code::PinSet, String::new()));
    }
    marks_of(&mut book)?;
    if book.kdf.is_none() {
        book.kdf = Some(params());
    }
    floor_gate(&book)?;
    let mut mk_v = rand(MASTER_BYTES)?;
    let mut mk = Key32([0u8; MASTER_BYTES]);
    mk.0.copy_from_slice(&mk_v);
    wipe(&mut mk_v);
    let salt = rand(32)?;
    let kdf = kdf_of(&book);
    let dk = derive(kdf, pin.as_bytes(), &salt)?;
    book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf)?);
    book.wrong = 0;
    write_book(v, &held, &book)?;
    hold(&mk);
    note_pin(pin);
    Ok(())
}

pub fn unlock(v: &Vault, pin: &str) -> Result<(), Trouble> {
    let held = lock_book(v)?;
    unlock_held(v, &held, pin, false)
}

pub fn check_pin(v: &Vault, pin: &str) -> Result<(), Trouble> {
    let held = lock_book(v)?;
    let Some(mut book) = read_book(v)? else {
        return Err(Trouble::new(Code::KeyboxMissing, String::new()));
    };
    let Some(sealed) = book.pin.clone() else {
        return Err(Trouble::new(Code::KeyboxMissing, String::new()));
    };
    if book.wrong >= WRONG_LIMIT {
        return Err(Trouble::new(Code::LockedOut, String::new()));
    }
    floor_gate(&book)?;
    let kdf = kdf_of(&book);
    book.wrong += 1;
    write_book(v, &held, &book)?;
    let dk = derive(kdf, pin.as_bytes(), &sealed.salt)?;
    match unseal(&dk.0, &sealed).and_then(master_of) {
        Some(mk) => {
            let whole = bound(&mk.0, Kind::Pin, "", kdf, &sealed);
            drop(mk);
            book.wrong = 0;
            let _ = write_book(v, &held, &book);
            if !whole {
                return Err(Trouble::new(Code::KeyboxSlot, member::PIN.to_string()));
            }
            Ok(())
        }
        None => {
            if book.wrong >= WRONG_LIMIT {
                lock();
            }
            Err(Trouble::new(Code::PinWrong, WRONG_LIMIT.saturating_sub(book.wrong).to_string()))
        }
    }
}

pub fn reseal(v: &Vault, pin: &str) -> Result<(), Trouble> {
    let held = lock_book(v)?;
    unlock_held(v, &held, pin, true)
}

static RESEAL_TROUBLE: std::sync::Mutex<Option<Trouble>> = std::sync::Mutex::new(None);

fn note_reseal_trouble(f: Trouble) {
    if let Ok(mut g) = RESEAL_TROUBLE.lock() {
        *g = Some(f);
    }
}

pub fn take_reseal_trouble() -> Option<Trouble> {
    RESEAL_TROUBLE.lock().ok().and_then(|mut g| g.take())
}

fn unlock_held(v: &Vault, held: &Held, pin: &str, at_floor: bool) -> Result<(), Trouble> {
    let Some(mut book) = read_book(v)? else {
        return Err(Trouble::new(Code::KeyboxMissing, String::new()));
    };
    let Some(sealed) = book.pin.clone() else {
        return Err(Trouble::new(Code::KeyboxMissing, String::new()));
    };
    if book.wrong >= WRONG_LIMIT {
        return Err(Trouble::new(Code::LockedOut, String::new()));
    }
    let low = below_floor(kdf_of(&book));
    if low && !at_floor {
        floor_gate(&book)?;
    }
    let kdf = if low { floor() } else { kdf_of(&book) };
    book.wrong += 1;
    write_book(v, held, &book)?;
    let dk = derive(kdf, pin.as_bytes(), &sealed.salt)?;
    let opened = unseal(&dk.0, &sealed).and_then(master_of);
    match opened {
        Some(mk) => {
            if !bound(&mk.0, Kind::Pin, "", kdf, &sealed) {
                book.wrong = 0;
                let _ = write_book(v, held, &book);
                return Err(Trouble::new(Code::KeyboxSlot, member::PIN.to_string()));
            }
            hold(&mk);
            note_pin(pin);
            book.wrong = 0;
            if low {
                let mut resealed = book.clone();
                resealed.kdf = Some(floor());
                let done = (|| -> Result<(), Trouble> {
                    let salt = rand(32)?;
                    let dk2 = derive(floor(), pin.as_bytes(), &salt)?;
                    resealed.pin = Some(seal_master(&dk2, salt, &mk.0, Kind::Pin, "", floor())?);
                    rebind(&mut resealed, &mk.0);
                    write_book(v, held, &resealed)
                })();
                if let Err(f) = done {
                    let _ = write_book(v, held, &book);
                    note_reseal_trouble(f);
                }
                return Ok(());
            }
            let _ = write_book(v, held, &book);
            Ok(())
        }
        None => {
            if book.wrong >= WRONG_LIMIT {
                lock();
            }
            Err(Trouble::new(Code::PinWrong, WRONG_LIMIT.saturating_sub(book.wrong).to_string()))
        }
    }
}

fn rebind(book: &mut Book, mk: &[u8; MASTER_BYTES]) {
    let kdf = kdf_of(book);
    if let Some(p) = book.pin.as_mut() {
        p.bind = Some(bind_of(mk, Kind::Pin, "", kdf, p));
    }
    for r in book.recovery.iter_mut() {
        r.sealed.bind = Some(bind_of(mk, Kind::Recovery, &r.label, kdf, &r.sealed));
    }
    for sl in book.slots.iter_mut() {
        sl.sealed.bind = Some(bind_of(mk, Kind::Slot, &sl.name, kdf, &sl.sealed));
    }
    if let Some(p) = book.primary.as_mut() {
        p.id_seal.bind = Some(bind_of(mk, Kind::PrimaryId, PRIMARY_LABEL, kdf, &p.id_seal));
    }
}

pub fn change_pin(v: &Vault, old: &str, new: &str) -> Result<(), Trouble> {
    if let Some(t) = pin_trouble(new) {
        return Err(Trouble::new(Code::PinShape, t.as_str().to_string()));
    }
    let held = lock_book(v)?;
    unlock_held(v, &held, old, false)?;
    let mk = master_now()?;
    let mut book = read_book(v)?.ok_or_else(|| Trouble::new(Code::KeyboxMissing, String::new()))?;
    floor_gate(&book)?;
    let salt = rand(32)?;
    let kdf = kdf_of(&book);
    let dk = derive(kdf, new.as_bytes(), &salt)?;
    book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf)?);
    book.wrong = 0;
    write_book(v, &held, &book)?;
    note_pin(new);
    Ok(())
}

pub fn recover(v: &Vault, secret: &[u8], new_pin: &str) -> Result<(), Trouble> {
    if let Some(t) = pin_trouble(new_pin) {
        return Err(Trouble::new(Code::PinShape, t.as_str().to_string()));
    }
    let held = lock_book(v)?;
    let mut book = read_book(v)?.ok_or_else(|| Trouble::new(Code::KeyboxMissing, String::new()))?;
    let low = below_floor(kdf_of(&book));
    let kdf = if low { floor() } else { kdf_of(&book) };
    let mark = member_tag(secret, &book.marks.unwrap_or_default());
    let Some(r) = book.recovery.iter().find(|r| r.tag == mark) else {
        let here = book.slots.iter().any(|sl| sl.tag == mark);
        let k = if here { Code::RecoveryNotPrimary } else { Code::RecoveryNoMatch };
        return Err(Trouble::new(k, String::new()));
    };
    let dk = derive(kdf, secret, &r.sealed.salt)?;
    let Some(mk) = unseal(&dk.0, &r.sealed).and_then(master_of).filter(|mk| bound(&mk.0, Kind::Recovery, &r.label, kdf, &r.sealed)) else {
        return Err(Trouble::new(Code::RecoveryNoMatch, String::new()));
    };
    if low {
        book.kdf = Some(floor());
        rebind(&mut book, &mk.0);
    }
    let salt = rand(32)?;
    let kdf = kdf_of(&book);
    let dk = derive(kdf, new_pin.as_bytes(), &salt)?;
    book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf)?);
    book.wrong = 0;
    write_book(v, &held, &book)?;
    hold(&mk);
    note_pin(new_pin);
    Ok(())
}

pub fn primary(v: &Vault) -> Result<Option<(String, PrimaryKind)>, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(None) };
    let Some(pr) = book.primary.clone() else { return Ok(None) };
    let mk = master_now()?;
    let sealed = pr.id_seal;
    if !bound(&mk.0, Kind::PrimaryId, PRIMARY_LABEL, kdf_of(&book), &sealed) {
        return Err(Trouble::new(Code::KeyboxSlot, member::PRIMARY.to_string()));
    }
    let plain = unseal(&mk.0, &sealed).ok_or_else(|| Trouble::new(Code::KeyboxSlot, member::PRIMARY.to_string()))?;
    let id = String::from_utf8(plain.bytes().to_vec()).map_err(|_| Trouble::new(Code::KeyboxShape, member::PRIMARY.to_string()))?;
    Ok(Some((id, pr.kind)))
}

pub fn primary_kind(v: &Vault) -> Result<Option<PrimaryKind>, Trouble> {
    Ok(read_book(v)?.and_then(|b| b.primary).map(|p| p.kind))
}

pub fn add_recovery(v: &Vault, id: &str, kind: PrimaryKind, secrets: &[Plain]) -> Result<bool, Trouble> {
    let mk = master_now()?;
    let held = lock_book(v)?;
    let mut book = read_book(v)?.unwrap_or_default();
    let marks = marks_of(&mut book)?;
    let secret = secrets.first().ok_or_else(|| Trouble::new(Code::KeyboxShape, member::RECOVERY.to_string()))?.bytes();
    let mark = member_tag(secret, &marks);
    if book.primary.is_some() && book.recovery.first().is_some_and(|r| r.tag != mark) {
        return Ok(false);
    }
    floor_gate(&book)?;
    let kdf = kdf_of(&book);
    book.recovery = recovery_seals(&mk.0, kind, secrets, kdf, &marks)?;
    let mut id_seal = seal(&mk.0, rand(32)?, id.as_bytes())?;
    id_seal.mac = None;
    id_seal.bind = Some(bind_of(&mk.0, Kind::PrimaryId, PRIMARY_LABEL, kdf, &id_seal));
    book.primary = Some(Primary { kind, id_seal });
    write_book(v, &held, &book)?;
    Ok(true)
}

pub fn recoverable(v: &Vault) -> Result<bool, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(false) };
    Ok(!book.recovery.is_empty() || !book.slots.is_empty())
}

pub fn reset_empty(v: &Vault) -> Result<(), Trouble> {
    let _held = lock_book(v)?;
    if recoverable(v)? {
        let book = read_book(v)?.unwrap_or_default();
        return Err(Trouble::new(
            Code::KeyboxNotEmpty,
            format!("slots={} seals={}", book.slots.len(), book.recovery.len()),
        ));
    }
    let p = v.path();
    match std::fs::remove_file(&p) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Trouble::io(&e, &p.display().to_string())),
    }
    lock();
    Ok(())
}

pub fn has_recovery(v: &Vault, id: &str) -> Result<bool, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(false) };
    Ok(!book.recovery.is_empty() && primary(v)?.map(|(p, _)| p.eq_ignore_ascii_case(id)).unwrap_or(false))
}

pub fn drop_recovery(v: &Vault, id: &str) -> Result<(), Trouble> {
    let is_primary = has_recovery(v, id)?;
    let held = lock_book(v)?;
    let Some(mut book) = read_book(v)? else { return Ok(()) };
    if !is_primary {
        return Ok(());
    }
    book.recovery.clear();
    book.primary = None;
    write_book(v, &held, &book)
}

pub fn local_key() -> Result<LocalKey, Trouble> {
    let mk = master_now()?;
    Ok(local_of(&mk.0))
}

pub const LOCAL_INFO: &[u8] = b"zikaron/local/v1";

fn local_of(mk: &[u8; MASTER_BYTES]) -> LocalKey {
    let mut d = LocalKey([0u8; 32]);
    let _ = crate::crypto::hkdf_sha256(mk, LOCAL_INFO, &mut d.0);
    d
}

pub fn name_key() -> Result<crate::names::NameKey, Trouble> {
    let mk = master_now()?;
    Ok(names_of(&mk.0))
}

pub const NAMES_INFO: &[u8] = b"zikaron/names/v1";

fn names_of(mk: &[u8; MASTER_BYTES]) -> crate::names::NameKey {
    let mut n = [0u8; 32];
    let _ = crate::crypto::hkdf_sha256(mk, NAMES_INFO, &mut n);
    let k = crate::names::NameKey::new(n);
    wipe(&mut n);
    k
}

pub struct LocalKey([u8; 32]);

impl LocalKey {
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for LocalKey {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

pub use crate::envelope::NEXT;

pub struct NewBook {
    book: Book,
    mk: Key32,
}

impl NewBook {
    pub fn local_key(&self) -> LocalKey {
        local_of(&self.mk.0)
    }

    pub fn name_key(&self) -> crate::names::NameKey {
        names_of(&self.mk.0)
    }
}

pub enum PinFor<'a> {
    New(&'a str),
    KeepFresh,
}

pub fn build_new(v: &Vault, pin: PinFor, primary: Option<(&str, PrimaryKind, &[Plain])>, slots: &[(String, Vec<u8>)]) -> Result<NewBook, Trouble> {
    if let PinFor::New(p) = pin {
        if let Some(t) = pin_trouble(p) {
            return Err(Trouble::new(Code::PinShape, t.as_str().to_string()));
        }
    }
    let old = read_book(v)?.unwrap_or_default();
    let mut book = Book { kdf: Some(old.kdf.unwrap_or_else(params)), ..Book::default() };
    if below_floor(kdf_of(&book)) {
        book.kdf = Some(floor());
    }
    let kdf = kdf_of(&book);
    let mk = match pin {
        PinFor::KeepFresh => {
            if !old.recovery.is_empty() || !old.slots.is_empty() || old.pin.is_none() {
                return Err(Trouble::new(Code::KeyboxNotEmpty, format!("slots={} seals={}", old.slots.len(), old.recovery.len())));
            }
            let mk = master_now()?;
            book.kdf = old.kdf;
            book.pin = old.pin.clone();
            mk
        }
        PinFor::New(p) => {
            let mut v = rand(MASTER_BYTES)?;
            let mut mk = Key32([0u8; MASTER_BYTES]);
            mk.0.copy_from_slice(&v);
            wipe(&mut v);
            let salt = rand(32)?;
            let dk = derive(kdf, p.as_bytes(), &salt)?;
            book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf)?);
            mk
        }
    };
    let kdf = kdf_of(&book);
    let nk = names_of(&mk.0);
    let marks = new_marks()?;
    book.marks = Some(marks);
    if let Some((id, kind, secrets)) = primary {
        book.recovery = recovery_seals(&mk.0, kind, secrets, kdf, &marks)?;
        let mut id_seal = seal(&mk.0, rand(32)?, id.as_bytes())?;
        id_seal.mac = None;
        id_seal.bind = Some(bind_of(&mk.0, Kind::PrimaryId, PRIMARY_LABEL, kdf, &id_seal));
        book.primary = Some(Primary { kind, id_seal });
    }
    for (a, plain) in slots {
        let name = nk.name(crate::names::Logical::Slot(a));
        let sealed = seal_slot(&mk.0, &name, kdf, &slot_plain(a, plain))?;
        book.slots.push(Slot { name, sealed, tag: member_tag(plain, &marks) });
    }
    book.wrong = 0;
    Ok(NewBook { book, mk })
}

pub fn stage_new(v: &Vault, nb: &NewBook) -> Result<(), Trouble> {
    let _held = lock_book(v)?;
    write_book_as(v, &nb.book, &next_name(v))
}

fn next_name(v: &Vault) -> String {
    crate::envelope::staged_name(&v.file)
}

pub fn commit_new(v: &Vault, nb: NewBook) -> Result<(), Trouble> {
    let _held = lock_book(v)?;
    let from = v.dir.join(next_name(v));
    let to = v.path();
    zikaron_os::rename_over(&from, &to).map_err(|u| Trouble::io(&u.error, &u.at.display().to_string()))?;
    hold(&nb.mk);
    PIN_DIGITS_ONLY.store(0, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

pub fn drop_staged(v: &Vault) -> Result<bool, Trouble> {
    let _held = lock_book(v)?;
    let p = v.dir.join(next_name(v));
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Trouble::io(&e, &p.display().to_string())),
    }
}

fn slot_name(mk: &[u8; MASTER_BYTES], account: &str) -> String {
    names_of(mk).name(crate::names::Logical::Slot(account))
}

pub fn slot_name_of(v: &Vault, account: &str) -> Result<Option<String>, Trouble> {
    let mk = master_now()?;
    if read_book(v)?.is_none() {
        return Ok(None);
    }
    Ok(Some(slot_name(&mk.0, account)))
}

pub fn put(v: &Vault, account: &str, secret: &[u8]) -> Result<(), Trouble> {
    let mk = master_now()?;
    let held = lock_book(v)?;
    let mut book = read_book(v)?.unwrap_or_default();
    floor_gate(&book)?;
    let name = slot_name(&mk.0, account);
    let (plain, tag) = (slot_plain(account, secret), member_tag(secret, &marks_of(&mut book)?));
    let sealed = seal_slot(&mk.0, &name, kdf_of(&book), &plain);
    let mut plain = plain;
    wipe(&mut plain);
    book.slots.retain(|sl| sl.name != name);
    book.slots.push(Slot { name, sealed: sealed?, tag });
    write_book(v, &held, &book)
}

pub fn get(v: &Vault, account: &str) -> Result<Option<Vec<u8>>, Trouble> {
    let mk = master_now()?;
    let Some(book) = read_book(v)? else { return Ok(None) };
    let name = slot_name(&mk.0, account);
    let Some(sl) = book.slots.iter().find(|sl| sl.name == name) else {
        return Ok(None);
    };
    if !bound(&mk.0, Kind::Slot, &name, kdf_of(&book), &sl.sealed) {
        return Err(Trouble::new(Code::KeyboxSlot, account.to_string()));
    }
    let Some(plain) = unseal(&mk.0, &sl.sealed) else {
        return Err(Trouble::new(Code::KeyboxSlot, account.to_string()));
    };
    match slot_split(plain.bytes()) {
        Some((a, secret)) if a == account => Ok(Some(secret)),
        _ => Err(Trouble::new(Code::KeyboxSlot, account.to_string())),
    }
}

pub fn drop_item(v: &Vault, account: &str) -> Result<bool, Trouble> {
    let held = lock_book(v)?;
    let Some(mut book) = read_book(v)? else { return Ok(false) };
    let name = slot_name(&master_now()?.0, account);
    let before = book.slots.len();
    book.slots.retain(|sl| sl.name != name);
    let gone = book.slots.len() != before;
    if gone {
        write_book(v, &held, &book)?;
    }
    Ok(gone)
}

pub fn drop_all_slots(v: &Vault) -> Result<usize, Trouble> {
    let held = lock_book(v)?;
    let Some(mut book) = read_book(v)? else { return Ok(0) };
    let n = book.slots.len();
    if n > 0 {
        book.slots.clear();
        write_book(v, &held, &book)?;
    }
    Ok(n)
}

pub fn slot_count(v: &Vault) -> Result<usize, Trouble> {
    Ok(read_book(v)?.map(|b| b.slots.len()).unwrap_or(0))
}

pub fn accounts(v: &Vault) -> Result<Vec<String>, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(Vec::new()) };
    let mk = master_now()?;
    let mut out = Vec::with_capacity(book.slots.len());
    for sl in &book.slots {
        let plain = unseal(&mk.0, &sl.sealed).ok_or_else(|| Trouble::new(Code::KeyboxSlot, sl.name.clone()))?;
        let (a, mut secret) = slot_split(plain.bytes()).ok_or_else(|| Trouble::new(Code::KeyboxSlot, sl.name.clone()))?;
        wipe(&mut secret);
        out.push(a);
    }
    Ok(out)
}

pub fn present(v: &Vault, account: &str) -> Result<bool, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(false) };
    let name = slot_name(&master_now()?.0, account);
    Ok(book.slots.iter().any(|sl| sl.name == name))
}

pub fn master_is_this(v: &Vault) -> Result<bool, Trouble> {
    let mk = master_now()?;
    let Some(book) = read_book(v)? else {
        return Err(Trouble::new(Code::KeyboxMissing, String::new()));
    };
    let Some(sealed) = book.pin.as_ref() else {
        return Err(Trouble::new(Code::KeyboxMissing, String::new()));
    };
    Ok(bound(&mk.0, Kind::Pin, "", kdf_of(&book), sealed))
}
