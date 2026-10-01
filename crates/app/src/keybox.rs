//! The key vault: every identity's keys on this machine, encrypted in one file in the machine directory.
//!
//! The system keychain identifies an app by each build's code hash; without a developer certificate carrying
//! a Team ID, every rebuild or upgrade asks for authorization again. So the app uses a local passcode
//! instead. A passcode does not stop offline brute force once the data directory is copied; it stops "whoever
//! holds this machine can sign".
//!
//! One master key, two ways to open it. Every key in the vault is encrypted with one random master key, which
//! is sealed twice:
//!
//! 1. The passcode seal: an eight-character passcode through a memory-hard derivation (scrypt, standard
//! parameters, about one second) gives the sealing key.
//! 2. A recovery seal per identity: a recovery-word identity derives from its 16 bytes of entropy, an
//! existing-key identity from its 32-byte private key (recovered from the exported keystore V3 file and its
//! password).
//!
//! So after a forgotten passcode, or five failures that lock the vault, a person with the recovery words or
//! the key file can still open it; opening that way reseals the passcode seal and resets the failure count.
//!
//! The lock is real: once opened, the master key lives only in memory (one `Mutex` in this file) and is wiped
//! on lock, exit or idle timeout. While locked, [`get`] refuses with `LOCKED`: the action layer's table stops
//! key-using actions first, and this is the second barrier.
//!
//! Failures are written to disk: each one increments a counter that survives restarts; at [`WRONG_LIMIT`]
//! only recovery remains. Successful recovery sets a new passcode and resets the count. Wrong recovery words
//! are refused and change neither the counter nor the vault: recovery must not become another way to be
//! locked out.
//!
//! No plain private key or seed is in the vault file: bytes go through `cryptx::aes128_ctr` with a keccak MAC
//! (the keystore V3 construction, [`crate::keystore::mac_of`]). Only the app's existing cryptographic
//! primitives are used.
//!
//! Plain text held in hand (the master key, derived keys, unsealed bytes) lives in [`Plain`] and [`Key32`]
//! and is wiped when dropped (`zikaron_ui::secret::wipe`); `MASTER` is wiped on lock. Keys returned by `get`
//! are wiped by the receiver (`key.rs`, `identity::words_of`). Temporary stack copies the compiler makes when
//! moving an array, and the primitives' own working memory, are outside what this file can wipe.
//!
//! Shape 2 binds each seal's identity into its MAC: with only the ciphertext covered (shape 1), someone able
//! to write the file could swap the two seats' slot ciphertexts and the author seat would sign with the
//! grantee key. In shape 2 each seal has a `bind` field covered by the master key over kind, slot name or
//! identity id, IV, derivation parameters, salt and ciphertext, so a swap, a changed IV or changed parameters
//! are refused by name. Slots carry only `bind` (a shape 2 slot with `mac` is malformed, so `bind` cannot be
//! stripped to forge a shape 1 file); passcode and recovery seals also keep `mac` (covered by the derived
//! key, which checks the passcode or recovery secret). Shape 1 files still open and are upgraded in place
//! when the master key is in hand.
//!
//! Derivation floor: the vault records its derivation parameters; below this binary's shipped parameters it
//! is refused with `KDF_BELOW_FLOOR` (not counted as a failure), and the person is guided to reseal at the
//! floor with the passcode or a recovery secret ([`reseal`], [`recover`]). Nothing is ever sealed under
//! parameters below the floor.

use crate::fault::{classify, Fault, Known};
use zikaron::hexfmt;
use zikaron::json::{self, Value};

/// Vault file shape. Shape 3: no plain name in the file leads to an identity on the chain (slot names keyed by
/// the names key, account names sealed inside the slots, the primary identity's id sealed under the master
/// key; only its kind stays plain). An older binary reading this shape refuses it by name.
pub const SHAPE: &str = "zikaron-desk/keybox/3";
/// Shape 2 (binds each seal's identity into its MAC; names plain): still readable, upgraded in place.
pub const SHAPE_V2: &str = "zikaron-desk/keybox/2";
/// Shape 1 (older vaults): still readable, upgraded in place on a successful unlock.
pub const SHAPE_V1: &str = "zikaron-desk/keybox/1";

/// A seal's IV length in bytes (AES-128-CTR).
pub const IV_LEN: usize = 16;

/// The vault file's member names (top level, each seal, the primary record, each slot, the derivation
/// parameters). One name, one home: the reader and the writer here, and anything that reads the file's raw
/// members, spell them only through this table.
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
    pub const N: &str = "n";
    pub const R: &str = "r";
    pub const P: &str = "p";
}
/// Passcode length: eight characters, each an ASCII letter or digit, case-sensitive.
pub const PIN_LEN: usize = 8;
/// Domain of the bind field (separate from the shape 1 `mac`, so neither can pose as the other).
const BIND_DOMAIN: &[u8] = b"zikaron-keybox/2";
/// Failures before the vault locks.
pub const WRONG_LIMIT: u64 = 5;

/// Master key length (the first 16 bytes are the key, the last 16 feed the MAC, as in keystore V3).
const MASTER_BYTES: usize = 32;

/// The master key once opened. Memory only; wiped on lock, home change or exit.
static MASTER: std::sync::Mutex<Option<[u8; MASTER_BYTES]>> = std::sync::Mutex::new(None);

/// The vault's state now. Closed: the lock screen and the test hooks branch on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// No vault on this machine yet (first run).
    Absent,
    /// A vault, locked, with tries left.
    Locked { wrong: u64 },
    /// A vault, locked, with all tries used: only recovery remains.
    LockedOut,
    /// Open.
    Open,
}

impl State {
    /// Whether the gate is on screen. Answered per member: locked and locked-out cover the window (the shell
    /// is not drawn meanwhile); no vault yet (no passcode set) and open do not.
    ///
    /// Answering "not open means the gate is up" would treat "no vault" as locked: on a new machine the shell
    /// would not draw while the gate draws only when locked, leaving a blank page with neither wizard nor
    /// gate. The shell, the wizard and the cards all ask this one table.
    pub fn gate_up(self) -> bool {
        match self {
            State::Absent => false,
            State::Locked { .. } => true,
            State::LockedOut => true,
            State::Open => false,
        }
    }

    /// Whether a key can be had now: only when open. Key-using actions (`action::needs_key`), the lamps and
    /// whether to suppress "vault locked" in toasts all ask it.
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

/// One seal: salt, IV, ciphertext and two MACs.
///
/// `mac`: covered by the key that opens this seal (a derived key for passcode and recovery seals; the master
/// key for shape 1 slots). `bind`: shape 2 only, covered by the master key over kind, identity, IV,
/// derivation parameters, salt and ciphertext.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sealed {
    salt: Vec<u8>,
    iv: [u8; IV_LEN],
    ct: Vec<u8>,
    mac: Option<[u8; 32]>,
    bind: Option<[u8; 32]>,
}

/// Which shape the vault file has.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Form {
    V1,
    V2,
    #[default]
    V3,
}

/// Which kind a seal is (the first field mixed into `bind`).
#[derive(Clone, Copy)]
enum Kind {
    Pin,
    Recovery,
    Slot,
    /// The primary identity's id, sealed under the master key (shape 3).
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

/// Bytes of plain text held in hand, wiped when dropped (an unsealed master key, a slot's key).
pub struct Plain(Vec<u8>);

impl Plain {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for Plain {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// A 32-byte plain key, wiped when dropped (the master key, a derived key; returned by `derive` and
/// `master_now`).
struct Key32([u8; 32]);

impl Drop for Key32 {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// The vault on disk.
#[derive(Clone, Debug, Default)]
struct Book {
    form: Form,
    pin: Option<Sealed>,
    /// This vault's own derivation parameters. Absent in vaults written before they were recorded (opened
    /// with this pass's [`params`]).
    kdf: Option<crate::keystore::Params>,
    /// Recovery seals by label: shapes 1 and 2 label each by identity id; shape 3 has at most one, the
    /// primary's, labelled [`PRIMARY_LABEL`].
    recovery: Vec<(String, Sealed)>,
    slots: Vec<Slot>,
    wrong: u64,
    /// The primary identity: the only one whose recovery seal opens the vault. `None` in vaults written before
    /// this cell existed (settled on the first unlock, see [`settle_primary`]).
    primary: Option<Primary>,
    /// Shape 3: this vault's salt for member marks, new with each new master key (see [`member_tag`]).
    marks: Option<[u8; 32]>,
}

/// One key slot. Shapes 1 and 2: `name` is the account name in plain text. Shape 3: `name` is keyed by the
/// names key (`names::Logical::Slot`) and the account name is sealed inside with the secret; `tag` is the
/// member mark of the secret (see [`member_tag`]).
#[derive(Clone, Debug)]
struct Slot {
    name: String,
    sealed: Sealed,
    tag: Option<[u8; 32]>,
}

/// The primary identity as the file holds it. Shapes 1 and 2: `id` plain. Shape 3: `id` sealed under the
/// master key (`id_seal`), and `tag` is the member mark of its secret, so recovery can tell the primary's
/// secret from another identity's while locked without the file naming anyone.
#[derive(Clone, Debug)]
struct Primary {
    id: Option<String>,
    kind: PrimaryKind,
    tag: Option<[u8; 32]>,
    id_seal: Option<Sealed>,
}

/// The label of shape 3's one recovery seal.
const PRIMARY_LABEL: &str = "primary";

/// The member mark of a secret (a recovery-word identity's entropy, an existing key's private key): keyed by
/// the secret itself, so it says nothing to anyone without that secret, and a secret handed in at recovery
/// finds its own mark. Salted by the vault's own salt, drawn anew with each new master key, so no mark is
/// shared between the vault before a master key change and the one after it.
fn member_tag(secret: &[u8], marks: &[u8; 32]) -> [u8; 32] {
    let mut msg = b"zikaron/keybox/member/v1".to_vec();
    msg.push(0);
    msg.extend_from_slice(marks);
    crate::cryptx::hmac_sha256(secret, &msg)
}

/// This vault's salt for member marks, drawn when a shape 3 vault has none yet (a vault being made).
fn marks_of(book: &mut Book) -> Result<[u8; 32], Fault> {
    match book.marks {
        Some(m) => Ok(m),
        None => {
            let m = new_marks()?;
            book.marks = Some(m);
            Ok(m)
        }
    }
}

/// A new salt for member marks.
fn new_marks() -> Result<[u8; 32], Fault> {
    let v = rand(32)?;
    let mut m = [0u8; 32];
    m.copy_from_slice(&v);
    Ok(m)
}

/// A slot's sealed bytes in shape 3: the account name (length first) and the secret.
fn slot_plain(account: &str, secret: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 + account.len() + secret.len());
    v.extend_from_slice(&(account.len() as u16).to_be_bytes());
    v.extend_from_slice(account.as_bytes());
    v.extend_from_slice(secret);
    v
}

/// Split a shape 3 slot's opened bytes into account name and secret.
fn slot_split(plain: &[u8]) -> Option<(String, Vec<u8>)> {
    if plain.len() < 2 {
        return None;
    }
    let n = u16::from_be_bytes([plain[0], plain[1]]) as usize;
    let account = std::str::from_utf8(plain.get(2..2 + n)?).ok()?.to_string();
    Some((account, plain.get(2 + n..)?.to_vec()))
}

/// Which kind of identity the primary is: it decides what the lock screen asks for. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrimaryKind {
    /// A recovery-word identity: twelve words recover the passcode.
    Words,
    /// An imported-key identity: its key file and the file's password recover the passcode.
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

/// Where the vault file lives: the machine directory, named per account base (a test vault and the shipped
/// one never see each other).
pub fn path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(crate::places::keybox_file()))
}

// The vault file lock.

/// A vault lock held in hand: while it lives the lock is held; when it is dropped (or the process ends) the
/// kernel releases it.
///
/// The failure count is read, changed and written back. Recording the attempt before deriving narrows the
/// race but leaves a window: N processes started together would each read the same count and each write back
/// one more, turning five tries into N × 5. Holding the lock over the whole read-modify-write removes that
/// class.
///
/// The lock comes from the kernel (`flock` on an open descriptor), so there is no stale lock (as in
/// `lock.rs`). This lock waits (`lock::grab_waiting`): a passcode attempt should queue, not be judged wrong
/// because another process is changing the vault. The vault lock and a home's writer lock are separate: the
/// vault lives in the machine directory, a home in its own place.
pub struct Held {
    _file: std::fs::File,
}

/// Where the lock file lives (the vault's directory; the name is built by [`crate::places`]).
fn lock_path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(crate::places::keybox_lock_file()))
}

/// Take the vault lock, waiting until it is available; creates the directory first if missing.
///
/// Every vault change (set passcode, unlock, change passcode, recover, add or drop a recovery seal, put or
/// drop a slot, reset an empty vault) starts here, and [`write_book`] takes a reference to the lock, so
/// writing without the lock cannot be written.
fn lock_book() -> Result<Held, Fault> {
    let p = lock_path()?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
    }
    let mut o = std::fs::OpenOptions::new();
    o.create(true).read(true).write(true).truncate(false);
    // The lock file is owner-only too (like the vault file and the registry). It holds no bytes, but anyone
    // who can open it can lock it: `flock` needs only an open descriptor, read-only included. At 0644,
    // another account on the same machine could hold the lock forever while this side waits, and the passcode
    // gate would hang.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let file = o.open(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
    if !crate::lock::grab_waiting(&file) {
        return Err(Fault::known(Known::KeyboxLocked, p.display().to_string()));
    }
    Ok(Held { _file: file })
}

fn rand(n: usize) -> Result<Vec<u8>, Fault> {
    use std::io::Read;
    let mut h = std::fs::File::open(crate::key::ENTROPY)
        .map_err(|e| classify(&e, crate::key::ENTROPY))?;
    let mut b = vec![0u8; n];
    h.read_exact(&mut b).map_err(|e| classify(&e, crate::key::ENTROPY))?;
    Ok(b)
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

/// Read one seal. Shape 1: every seal needs `mac` and forbids `bind`. Shape 2: slots only `bind`; passcode
/// and recovery seals need both.
fn seal_of(v: &Value, form: Form, kind: Kind) -> Option<Sealed> {
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
    let want = match (form, kind) {
        (Form::V1, _) => mac.is_some() && bind.is_none(),
        (Form::V2 | Form::V3, Kind::Slot | Kind::PrimaryId) => mac.is_none() && bind.is_some(),
        (Form::V2 | Form::V3, _) => mac.is_some() && bind.is_some(),
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

fn read_book() -> Result<Option<Book>, Fault> {
    let p = path()?;
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    };
    let v = json::parse(&bytes)
        .map_err(|t| Fault::known(Known::KeyboxShape, format!("{}: {t:?}", p.display())))?;
    let form = match text(&v, member::SHAPE).as_str() {
        SHAPE => Form::V3,
        SHAPE_V2 => Form::V2,
        SHAPE_V1 => Form::V1,
        other => return Err(Fault::known(Known::KeyboxShape, other.to_string())),
    };
    // A missing or malformed failure count is refused by name. Reading it as 0 would let someone able to edit
    // the file clear the count by deleting a field, silently. Nothing in the vault changes; it just does not
    // open.
    let wrong = match v.member(member::WRONG) {
        Some(Value::Int(n)) => *n,
        _ => return Err(Fault::known(Known::KeyboxShape, member::WRONG.to_string())),
    };
    let mut book = Book { form, wrong, ..Book::default() };
    // The vault states its own derivation parameters. All three are read and checked against bounds
    // (`keystore::in_range`, the same gate as keystore V3 files): out of bounds is refused by name, so a file
    // cannot make derivation use terabytes of memory. A vault without this section predates it and opens with
    // this pass's `params()` (it was sealed under them).
    if let Some(k) = v.member(member::KDF) {
        let num = |name: &str| -> Option<usize> {
            match k.member(name) {
                Some(Value::Int(n)) => usize::try_from(*n).ok(),
                _ => None,
            }
        };
        let (Some(n), Some(r), Some(pp)) = (num(member::N), num(member::R), num(member::P)) else {
            return Err(Fault::known(Known::KeyboxShape, member::KDF.to_string()));
        };
        if !crate::keystore::in_range(n, r, pp) {
            return Err(Fault::known(Known::KeyboxShape, format!("kdf n={n} r={r} p={pp}")));
        }
        book.kdf = Some(crate::keystore::Params { n, r, p: pp });
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
    if form == Form::V3 {
        book.marks = Some(tag32(v.member(member::MARKS)).ok().flatten().ok_or_else(|| Fault::known(Known::KeyboxShape, member::MARKS.to_string()))?);
    }
    if let Some(p) = v.member(member::PRIMARY) {
        let kind = PrimaryKind::parse(&text(p, member::KIND));
        let bad = || Fault::known(Known::KeyboxShape, member::PRIMARY.to_string());
        book.primary = Some(match (form, kind) {
            // Shape 3: the kind and the member mark plain, the id sealed.
            (Form::V3, Some(kind)) => Primary {
                id: None,
                kind,
                tag: Some(tag32(p.member(member::TAG)).ok().flatten().ok_or_else(bad)?),
                id_seal: Some(p.member(member::ID).and_then(|x| seal_of(x, form, Kind::PrimaryId)).ok_or_else(bad)?),
            },
            (_, Some(kind)) if !text(p, member::ID).is_empty() => Primary { id: Some(text(p, member::ID)), kind, tag: None, id_seal: None },
            _ => return Err(bad()),
        });
    }
    if let Some(p) = v.member(member::PIN) {
        book.pin = seal_of(p, form, Kind::Pin);
        if book.pin.is_none() {
            return Err(Fault::known(Known::KeyboxShape, member::PIN.to_string()));
        }
    }
    if let Some(Value::Arr(a)) = v.member(member::RECOVERY) {
        for one in a {
            // Shape 3 labels its one seal itself; older shapes by identity id.
            let id = if form == Form::V3 { PRIMARY_LABEL.to_string() } else { text(one, member::ID) };
            let Some(s) = seal_of(one, form, Kind::Recovery) else {
                return Err(Fault::known(Known::KeyboxShape, format!("recovery {id}")));
            };
            book.recovery.push((id, s));
        }
        if form == Form::V3 && book.recovery.len() > 1 {
            return Err(Fault::known(Known::KeyboxShape, member::RECOVERY.to_string()));
        }
    }
    if let Some(Value::Arr(a)) = v.member(member::SLOTS) {
        for one in a {
            let name = if form == Form::V3 { text(one, member::NAME) } else { text(one, member::ACCOUNT) };
            let Some(sealed) = seal_of(one, form, Kind::Slot) else {
                return Err(Fault::known(Known::KeyboxShape, format!("slot {name}")));
            };
            let tag = match (form, tag32(one.member(member::TAG))) {
                (Form::V3, Ok(Some(t))) => Some(t),
                (Form::V3, _) => return Err(Fault::known(Known::KeyboxShape, format!("slot {name}"))),
                _ => None,
            };
            book.slots.push(Slot { name, sealed, tag });
        }
    }
    Ok(Some(book))
}

/// Write the vault. The first argument is the lock in hand: without it the call cannot be made, so writing
/// without the lock cannot be written (the tests also count that every write is under the lock).
fn write_book(_held: &Held, b: &Book) -> Result<(), Fault> {
    write_book_as(b, &crate::places::keybox_file())
}

/// The vault's bytes under a name in the machine directory (the vault itself, or the staged next vault).
fn write_book_as(b: &Book, name: &str) -> Result<(), Fault> {
    let p = path()?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
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
    let v3 = b.form == Form::V3;
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
        let mut f: Vec<(String, Value)> = vec![(member::KIND.into(), Value::Str(pr.kind.as_str().to_string()))];
        if v3 {
            if let (Some(t), Some(sl)) = (&pr.tag, &pr.id_seal) {
                f.push((member::ID.into(), seal_value(sl)));
                f.push((member::TAG.into(), Value::Str(bare(t))));
            }
        } else if let Some(id) = &pr.id {
            f.push((member::ID.into(), Value::Str(id.clone())));
        }
        f.sort_by(|x, y| x.0.cmp(&y.0));
        m.push((member::PRIMARY.into(), Value::Obj(f)));
    }
    m.push((
        member::RECOVERY.into(),
        Value::Arr(
            b.recovery
                .iter()
                .map(|(id, s)| if v3 { seal_value(s) } else { with(seal_value(s), vec![(member::ID.into(), Value::Str(id.clone()))]) })
                .collect(),
        ),
    ));
    m.push((member::SHAPE.into(), Value::Str(match b.form {
        Form::V3 => SHAPE,
        Form::V2 => SHAPE_V2,
        Form::V1 => SHAPE_V1,
    }.to_string())));
    m.push((
        member::SLOTS.into(),
        Value::Arr(
            b.slots
                .iter()
                .map(|sl| {
                    if v3 {
                        let mut extra = vec![(member::NAME.into(), Value::Str(sl.name.clone()))];
                        if let Some(t) = &sl.tag {
                            extra.push((member::TAG.into(), Value::Str(bare(t))));
                        }
                        with(seal_value(&sl.sealed), extra)
                    } else {
                        with(seal_value(&sl.sealed), vec![(member::ACCOUNT.into(), Value::Str(sl.name.clone()))])
                    }
                })
                .collect(),
        ),
    ));
    if let (true, Some(mk)) = (v3, &b.marks) {
        m.push((member::MARKS.into(), Value::Str(bare(mk))));
    }
    m.push((member::WRONG.into(), Value::Int(b.wrong)));
    m.sort_by(|a, b| a.0.cmp(&b.0));
    let bytes = json::canon_bytes(&Value::Obj(m));
    // One way to write, and it is not in this file. The vault is rewritten whole on every change, so
    // replacing the old file is intended; a direct write, cut off by a power loss or kill, would leave half a
    // file and every key and recovery seal unreadable. Landing lives in `home::put_at`, which also sets
    // permissions, the atomic rename and the unique temporary name.
    let dir = crate::home::machine_dir()?;
    crate::home::put_at(&dir, name, &bytes)
}

/// Which derivation parameters to use. The shipped binary always uses the standard ones (memory-hard, about
/// one second); the test hooks set light ones (the test profile, milliseconds), since a test run performs
/// hundreds of derivations.
///
/// As with [`crate::places`]: set at most once, and only the test hooks set it; the window never calls it
/// (the tests scan for this), so the shipped app always runs the standard parameters.
static KDF: std::sync::OnceLock<crate::keystore::Params> = std::sync::OnceLock::new();

/// The lightest parameters, for tests: within bounds (`keystore::in_range`) at nearly no cost. Passcode
/// strength is carried by the shipped parameters; tests exercise passcode behavior, not derivation time.
pub const PROBE_KDF: crate::keystore::Params = crate::keystore::Params { n: 2, r: 8, p: 1 };

/// Set light parameters once (test hooks only). `None` means [`PROBE_KDF`]; given parameters are set as given
/// (tests use this to check that the same passcode still opens when the vault's recorded parameters differ
/// from this pass's). Out of bounds is never set. A second call returns false, never silently replacing the
/// first.
pub fn set_light_kdf(p: Option<crate::keystore::Params>) -> bool {
    let p = p.unwrap_or(PROBE_KDF);
    if !crate::keystore::in_range(p.n, p.r, p.p) {
        return false;
    }
    KDF.set(p).is_ok()
}

/// This pass's derivation parameters.
pub fn params() -> crate::keystore::Params {
    *KDF.get_or_init(crate::keystore::Params::standard)
}

/// Wipe plain text in hand (master key, returned keys, derived keys). The zeroing lives in
/// `zikaron_ui::secret::wipe`.
fn wipe(b: &mut [u8]) {
    zikaron_ui::secret::wipe(b);
}

/// Which parameters this vault uses: its recorded section when present, otherwise this pass's [`params`]
/// (older vaults were sealed under them).
fn kdf_of(b: &Book) -> crate::keystore::Params {
    b.kdf.unwrap_or_else(params)
}

/// The derivation floor: this binary's shipped parameters (standard for the window, the light ones the test
/// hooks set for tests).
pub fn floor() -> crate::keystore::Params {
    params()
}

/// Whether parameters are below the floor: memory (n·r) or work (n·r·p), either below is below.
pub fn below_floor(k: crate::keystore::Params) -> bool {
    let f = floor();
    let mem = |p: crate::keystore::Params| p.n.saturating_mul(p.r);
    let work = |p: crate::keystore::Params| p.n.saturating_mul(p.r).saturating_mul(p.p);
    mem(k) < mem(f) || work(k) < work(f)
}

fn kdf_text(k: crate::keystore::Params) -> String {
    format!("n={} r={} p={}", k.n, k.r, k.p)
}

/// Whether this vault can be used: recorded parameters below the floor are refused by name (neither sealing
/// nor opening under a lowered file).
fn floor_gate(b: &Book) -> Result<(), Fault> {
    let k = kdf_of(b);
    if below_floor(k) {
        return Err(Fault::known(Known::KdfBelowFloor, format!("{} < {}", kdf_text(k), kdf_text(floor()))));
    }
    Ok(())
}

fn derive(p: crate::keystore::Params, secret: &[u8], salt: &[u8]) -> Result<Key32, Fault> {
    let mut dk = Key32([0u8; 32]);
    if !crate::cryptx::scrypt(secret, salt, p.n, p.r, p.p, &mut dk.0) {
        return Err(Fault::known(Known::KeystoreParams, format!("n={} r={} p={}", p.n, p.r, p.p)));
    }
    Ok(dk)
}

/// The bind field: the last 16 bytes of the master key cover kind, identity, IV, parameters, salt and
/// ciphertext (variable-length fields carry their lengths, so the concatenation is unambiguous).
fn bind_of(mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: crate::keystore::Params, s: &Sealed) -> [u8; 32] {
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

/// Whether a seal's identity matches (shape 2 only; shape 1 has no such field and answers true).
fn bound(mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: crate::keystore::Params, s: &Sealed, form: Form) -> bool {
    match form {
        Form::V1 => true,
        Form::V2 | Form::V3 => s.bind == Some(bind_of(mk, kind, label, kdf, s)),
    }
}

/// Seal bytes with a key (`mac` by this key; `bind` added by the master key).
fn seal(dk: &[u8; 32], salt: Vec<u8>, plain: &[u8]) -> Result<Sealed, Fault> {
    let iv_b = rand(16)?;
    let mut iv = [0u8; IV_LEN];
    iv.copy_from_slice(&iv_b);
    let mut ct = plain.to_vec();
    let mut key = [0u8; 16];
    key.copy_from_slice(&dk[0..16]);
    crate::cryptx::aes128_ctr(&key, &iv, &mut ct);
    wipe(&mut key);
    let mac = crate::keystore::mac_of(dk, &ct);
    Ok(Sealed { salt, iv, ct, mac: Some(mac), bind: None })
}

/// Seal the master key (passcode and recovery seals): `mac` by the derived key, `bind` by the master key.
fn seal_master(dk: &Key32, salt: Vec<u8>, mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: crate::keystore::Params, form: Form) -> Result<Sealed, Fault> {
    let mut s = seal(&dk.0, salt, mk)?;
    if form != Form::V1 {
        s.bind = Some(bind_of(mk, kind, label, kdf, &s));
    }
    Ok(s)
}

/// Seal a slot: shape 2 keeps only `bind` (by the master key); shape 1 keeps `mac`.
fn seal_slot(mk: &[u8; MASTER_BYTES], account: &str, kdf: crate::keystore::Params, form: Form, plain: &[u8]) -> Result<Sealed, Fault> {
    let salt = rand(32)?;
    let mut s = seal(mk, salt, plain)?;
    if form != Form::V1 {
        s.mac = None;
        s.bind = Some(bind_of(mk, Kind::Slot, account, kdf, &s));
    }
    Ok(s)
}

/// Open a seal. A MAC mismatch gives `None` (verify first, then decrypt: a wrong passcode stops at the MAC).
/// Shape 2 slots have no `mac`; their check is `bind` (the caller asks [`bound`] first).
fn unseal(dk: &[u8; 32], s: &Sealed) -> Option<Plain> {
    if let Some(mac) = s.mac {
        if crate::keystore::mac_of(dk, &s.ct) != mac {
            return None;
        }
    }
    let mut out = s.ct.clone();
    let mut key = [0u8; 16];
    key.copy_from_slice(&dk[0..16]);
    crate::cryptx::aes128_ctr(&key, &s.iv, &mut out);
    wipe(&mut key);
    Some(Plain(out))
}

/// Read opened bytes as a master key (`None` on wrong length).
fn master_of(p: Plain) -> Option<Key32> {
    if p.0.len() != MASTER_BYTES {
        return None;
    }
    let mut mk = Key32([0u8; MASTER_BYTES]);
    mk.0.copy_from_slice(&p.0);
    Some(mk)
}

/// The master key in memory now (a copy, wiped when dropped). Refused by name while locked.
fn master_now() -> Result<Key32, Fault> {
    let m = MASTER.lock().unwrap_or_else(|e| e.into_inner());
    match m.as_ref() {
        Some(k) => Ok(Key32(*k)),
        None => Err(Fault::known(Known::Locked, String::new())),
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

/// A lock asked for while local writes are in flight: the vault answers locked from now on (so nothing that
/// asks `state` sees it open, whatever rereads it), while the master key stays in memory for the writes to
/// land; `lock` wipes it when they have. A correct passcode before then ends it (`hold`).
static CLOSING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Begin a lock that completes when the local writes in flight land (see `CLOSING`).
pub fn begin_lock() {
    CLOSING.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Lock: the master key is wiped at once (lock, home change, exit and idle timeout all come here).
pub fn lock() {
    CLOSING.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut m = MASTER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mk) = m.as_mut() {
        wipe(mk);
    }
    *m = None;
    PIN_DIGITS_ONLY.store(0, std::sync::atomic::Ordering::SeqCst);
}

/// Whether this session's passcode is digits only (0 unknown, 1 yes, 2 no). Memory only, recorded on
/// successful open, set, change or recovery, forgotten on lock. The settings hint "your passcode is digits
/// only; consider adding letters" reads it (old passcodes keep working; changing is not forced).
static PIN_DIGITS_ONLY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn note_pin(pin: &str) {
    let v = if pin.chars().all(|c| c.is_ascii_digit()) { 1 } else { 2 };
    PIN_DIGITS_ONLY.store(v, std::sync::atomic::Ordering::SeqCst);
}

/// Whether this session's passcode is digits only (`None` when unknown).
pub fn pin_digits_only() -> Option<bool> {
    match PIN_DIGITS_ONLY.load(std::sync::atomic::Ordering::SeqCst) {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// Upgrade shape 1 to shape 2 in place (while the master key is in hand). Each slot must pass its shape 1
/// `mac` first; any mismatch stops the upgrade (that slot is still refused by name when read; upgrading does
/// not give a bad slot a valid binding) and returns false. Otherwise each seal gets `bind` and slots drop
/// `mac`.
fn upgrade(book: &mut Book, mk: &[u8; MASTER_BYTES]) -> bool {
    if book.form != Form::V1 {
        return false;
    }
    for sl in &book.slots {
        match sl.sealed.mac {
            Some(mac) if crate::keystore::mac_of(mk, &sl.sealed.ct) == mac => {}
            _ => return false,
        }
    }
    let kdf = kdf_of(book);
    if let Some(p) = book.pin.as_mut() {
        p.bind = Some(bind_of(mk, Kind::Pin, "", kdf, p));
    }
    for (id, s) in book.recovery.iter_mut() {
        s.bind = Some(bind_of(mk, Kind::Recovery, id, kdf, s));
    }
    for sl in book.slots.iter_mut() {
        sl.sealed.mac = None;
        sl.sealed.bind = Some(bind_of(mk, Kind::Slot, &sl.name, kdf, &sl.sealed));
    }
    book.form = Form::V2;
    true
}

/// Upgrade shapes 1 and 2 to shape 3 in place (the master key in hand): every slot is resealed with its
/// account name inside under a name keyed by the names key, and marked with its secret's member mark; the
/// primary's id is sealed under the master key and marked with its secret's mark; the one recovery seal is
/// relabelled. Not one key is lost: a slot that does not open stops the upgrade and the file stays as it was.
/// A vault whose primary is not settled yet (older vaults: [`settle_primary`] runs first) waits.
fn upgrade_v3(book: &mut Book, mk: &[u8; MASTER_BYTES]) -> Result<bool, Fault> {
    if book.form == Form::V3 {
        return Ok(false);
    }
    if book.form == Form::V1 && !upgrade(book, mk) {
        return Ok(false);
    }
    if book.primary.is_none() && !book.recovery.is_empty() {
        return Ok(false);
    }
    let kdf = kdf_of(book);
    let nk = names_of(mk);
    let marks = new_marks()?;
    let mut slots: Vec<Slot> = Vec::with_capacity(book.slots.len());
    let mut opened: Vec<(String, Vec<u8>)> = Vec::new();
    for sl in &book.slots {
        if !bound(mk, Kind::Slot, &sl.name, kdf, &sl.sealed, book.form) {
            return Err(Fault::known(Known::KeyboxSlot, sl.name.clone()));
        }
        let Some(mut plain) = unseal(mk, &sl.sealed) else {
            return Err(Fault::known(Known::KeyboxSlot, sl.name.clone()));
        };
        opened.push((sl.name.clone(), std::mem::take(&mut plain.0)));
    }
    for (account, secret) in opened.iter() {
        let name = nk.name(crate::names::Logical::Slot(account));
        let sealed = seal_slot(mk, &name, kdf, Form::V3, &slot_plain(account, secret))?;
        slots.push(Slot { name, sealed, tag: Some(member_tag(secret, &marks)) });
    }
    // The primary as the older shape names it (plain), before it is sealed below.
    let primary_id = book.primary.as_ref().and_then(|p| p.id.clone());
    if let Some(pr) = book.primary.as_mut() {
        let id = pr.id.clone().unwrap_or_default();
        // The primary's secret: its seed slot (a recovery-word identity) or its own key's slot (a key file).
        let base = crate::places::key_account().to_string();
        let want = match pr.kind {
            PrimaryKind::Words => format!("{base}-seed-{id}"),
            PrimaryKind::KeyFile => format!("{base}-{id}"),
        };
        let secret = opened.iter().find(|(a, _)| a.eq_ignore_ascii_case(&want)).map(|(_, s)| s.clone());
        let Some(secret) = secret else {
            for (_, s) in opened.iter_mut() {
                wipe(s);
            }
            return Err(Fault::known(Known::KeychainMissing, want));
        };
        pr.tag = Some(member_tag(&secret, &marks));
        let salt = rand(32)?;
        let mut sealed = seal(mk, salt, id.as_bytes())?;
        sealed.mac = None;
        sealed.bind = Some(bind_of(mk, Kind::PrimaryId, PRIMARY_LABEL, kdf, &sealed));
        pr.id_seal = Some(sealed);
        pr.id = None;
    }
    for (_, s) in opened.iter_mut() {
        wipe(s);
    }
    // Shape 3 holds the primary's seal only: of an older vault's seals (labelled by id) the primary's is kept
    // and relabelled; any other identity's is dropped (it could open the vault through a secret that is not
    // the primary's, and a second seal is not a shape 3 vault).
    book.recovery.retain(|(label, _)| primary_id.as_deref().map(|p| label.eq_ignore_ascii_case(p)).unwrap_or(false));
    for (label, s) in book.recovery.iter_mut() {
        *label = PRIMARY_LABEL.to_string();
        s.bind = Some(bind_of(mk, Kind::Recovery, PRIMARY_LABEL, kdf, s));
    }
    book.slots = slots;
    book.marks = Some(marks);
    book.form = Form::V3;
    Ok(true)
}

/// Upgrade this vault to shape 3 now (the first opening after upgrading, after the primary is settled). Only
/// while open. Answers whether it upgraded.
pub fn upgrade_names() -> Result<bool, Fault> {
    let mk = master_now()?;
    let held = lock_book()?;
    let Some(mut book) = read_book()? else { return Ok(false) };
    if !upgrade_v3(&mut book, &mk.0)? {
        return Ok(false);
    }
    write_book(&held, &book)?;
    Ok(true)
}

/// The vault's state now. One disk read; unreadable is refused by name.
pub fn state() -> Result<State, Fault> {
    let open = MASTER.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    let Some(book) = read_book()? else {
        return Ok(State::Absent);
    };
    if book.pin.is_none() {
        return Ok(State::Absent);
    }
    // Locked-out outranks open. The count on disk is this machine's record; the master key in memory is this
    // session's convenience. When they disagree the disk wins, or using up all tries in an open session would
    // leave a screen that says open until the next start finds it locked.
    if book.wrong >= WRONG_LIMIT {
        return Ok(State::LockedOut);
    }
    if open && !CLOSING.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(State::Open);
    }
    Ok(State::Locked { wrong: book.wrong })
}

/// How many tries are left.
pub fn tries_left() -> Result<u64, Fault> {
    let wrong = read_book()?.map(|b| b.wrong).unwrap_or(0);
    Ok(WRONG_LIMIT.saturating_sub(wrong))
}

// Passcode rules.

/// The ways a passcode breaks the rules. Closed, one sentence each.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PinTrouble {
    /// Not eight ASCII letters or digits.
    Shape,
    /// All eight the same (judged for any passcode).
    AllSame,
    /// A sequence, rising or falling (judged only for all-digit passcodes).
    Run,
    /// Date-like (a valid date as year-month-day, day-month-year or month-day-year; judged only for all-digit
    /// passcodes).
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

/// Whether eight characters follow the rules: `None` when they do, otherwise the rule broken (the screen
/// speaks by it).
///
/// Shape: eight characters, each an ASCII letter or digit, case-sensitive (about 47.6 bits; digits only about
/// 26.6). All-same is refused for any passcode; sequences and dates only for all-digit ones. The order is the
/// rule: shape first, then all-same, then sequence, then date.
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
    // Three readings: year-month-day, day-month-year, month-day-year. Any valid date is refused.
    if valid_ymd(n(0, 4), n(4, 2), n(6, 2))
        || valid_ymd(n(4, 4), n(2, 2), n(0, 2))
        || valid_ymd(n(4, 4), n(0, 2), n(2, 2))
    {
        return Some(PinTrouble::DateLike);
    }
    None
}

// Open, close, change.

/// Set the passcode for the first time: a random master key, sealed with the passcode, the vault written. A
/// vault that already has a passcode is refused by name.
pub fn set_pin(pin: &str) -> Result<(), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::H2);
    if let Some(t) = pin_trouble(pin) {
        return Err(Fault::known(Known::PinShape, t.as_str().to_string()));
    }
    let held = lock_book()?;
    let mut book = read_book()?.unwrap_or_default();
    if book.pin.is_some() {
        return Err(Fault::known(Known::PinSet, String::new()));
    }
    if book.form == Form::V3 {
        marks_of(&mut book)?;
    }
    // One vault, one set of derivation parameters, fixed when the vault is created and written into it. Every
    // later seal (passcode, recovery, slot) uses them, so making the shipped parameters harder later still
    // opens old vaults by their own section without migration; only new vaults use the new parameters.
    if book.kdf.is_none() {
        book.kdf = Some(params());
    }
    floor_gate(&book)?;
    let mut mk_v = rand(MASTER_BYTES)?;
    let mut mk = Key32([0u8; MASTER_BYTES]);
    mk.0.copy_from_slice(&mk_v);
    wipe(&mut mk_v);
    // A vault without a passcode can only be empty or a test vault the test hooks filled with slots: slots stay,
    // and the shape is as read (shape 2 when there was no vault).
    let salt = rand(32)?;
    let kdf = kdf_of(&book);
    let dk = derive(kdf, pin.as_bytes(), &salt)?;
    book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf, book.form)?);
    book.wrong = 0;
    write_book(&held, &book)?;
    hold(&mk);
    note_pin(pin);
    Ok(())
}

/// Unlock. Correct: the master key goes into memory and the count resets. Wrong: the count increments on disk
/// and the answer says how many tries remain. With all tries used only recovery remains (this is still
/// refused by name without trying the passcode). Recorded parameters below the floor are refused with
/// `KDF_BELOW_FLOOR` and not counted (the file was lowered; the passcode is not at fault).
pub fn unlock(pin: &str) -> Result<(), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::H2);
    let held = lock_book()?;
    unlock_held(&held, pin, false)
}

/// Reseal at the floor: when the recorded parameters are below the floor, open the passcode seal at the
/// floor, then write the floor back and reseal the passcode seal under it. That opening counts in the same
/// failure record. A vault not below the floor unlocks normally.
pub fn reseal(pin: &str) -> Result<(), Fault> {
    let held = lock_book()?;
    unlock_held(&held, pin, true)
}

/// The trouble when a reseal did not land (only after a successful open): taken where the unlock lands and
/// said once on screen. It is kept here instead of returned as an error because the vault did open; an error
/// would count a correct passcode as wrong.
static RESEAL_TROUBLE: std::sync::Mutex<Option<Fault>> = std::sync::Mutex::new(None);

fn note_reseal_trouble(f: Fault) {
    if let Ok(mut g) = RESEAL_TROUBLE.lock() {
        *g = Some(f);
    }
}

/// Take the last reseal trouble (empty after taking).
pub fn take_reseal_trouble() -> Option<Fault> {
    RESEAL_TROUBLE.lock().ok().and_then(|mut g| g.take())
}


/// The unlock, run under the vault lock. Changing the passcode takes the lock first and then comes here, so
/// the process never waits for itself. `at_floor`: open at the floor and write the floor back (the reseal
/// path); otherwise recorded parameters below the floor are refused by name.
fn unlock_held(held: &Held, pin: &str, at_floor: bool) -> Result<(), Fault> {
    let Some(mut book) = read_book()? else {
        return Err(Fault::known(Known::KeyboxMissing, String::new()));
    };
    let Some(sealed) = book.pin.clone() else {
        return Err(Fault::known(Known::KeyboxMissing, String::new()));
    };
    if book.wrong >= WRONG_LIMIT {
        return Err(Fault::known(Known::LockedOut, String::new()));
    }
    let low = below_floor(kdf_of(&book));
    if low && !at_floor {
        floor_gate(&book)?;
    }
    // The reseal path opens at the floor: the seals were made at the floor (only the recorded section was
    // lowered), so opening at the lowered parameters would fail.
    let kdf = if low { floor() } else { kdf_of(&book) };
    // Record the attempt first, then try. Incrementing only after the one-second derivation would let N
    // processes each read the same count and each write back one more, turning five tries into N × 5. So the
    // attempt is recorded on disk before deriving and cleared only on success.
    book.wrong += 1;
    write_book(held, &book)?;
    let dk = derive(kdf, pin.as_bytes(), &sealed.salt)?;
    let opened = unseal(&dk.0, &sealed).and_then(master_of);
    match opened {
        Some(mk) => {
            // An identity mismatch is refused by name (shape 2): the passcode was right (the derived key's
            // MAC passed) but this seal was altered (IV, parameters or the whole seal). The attempt is
            // cleared and the master key does not enter memory.
            if !bound(&mk.0, Kind::Pin, "", kdf, &sealed, book.form) {
                book.wrong = 0;
                let _ = write_book(held, &book);
                return Err(Fault::known(Known::KeyboxSlot, member::PIN.to_string()));
            }
            // A correct passcode opens first and clears the record after. If clearing cannot be written (disk
            // full, read-only directory), answering an error would count a correct passcode as wrong, and
            // five of those would lock the owner out. So it answers success; the recorded attempt stays on
            // disk and is cleared on the next successful open (it only makes the gate ask once more).
            hold(&mk);
            note_pin(pin);
            book.wrong = 0;
            if low {
                // Reseal: write the floor back, reseal the passcode seal at the floor (new salt), and rebind
                // every seal to the new record. A successful open does not return an error even if these
                // steps cannot be written (disk full, read-only directory, derivation failing): the seal
                // stays under the old record, the attempt is still cleared, and the next open reseals again.
                let mut resealed = book.clone();
                resealed.kdf = Some(floor());
                let done = (|| -> Result<(), Fault> {
                    let salt = rand(32)?;
                    let dk2 = derive(floor(), pin.as_bytes(), &salt)?;
                    resealed.pin = Some(seal_master(&dk2, salt, &mk.0, Kind::Pin, "", floor(), resealed.form)?);
                    rebind(&mut resealed, &mk.0);
                    write_book(held, &resealed)
                })();
                if let Err(f) = done {
                    // The reseal failed: write the cleared record (old parameters) once as best effort and
                    // hand the trouble to the shell. The vault did open, so no error; the failure must be
                    // visible, or the next start would ask the same thing with nobody saying why.
                    let _ = write_book(held, &book);
                    note_reseal_trouble(f);
                }
                return Ok(());
            }
            // Upgrade shape 1 to shape 2 in place (the master key is in hand).
            let _ = upgrade(&mut book, &mk.0);
            let _ = write_book(held, &book);
            Ok(())
        }
        None => {
            // On the final failure, take the key back at once: an open session that uses up all tries ends
            // there, the master key is wiped, and the next frame shows the locked-out card.
            if book.wrong >= WRONG_LIMIT {
                lock();
            }
            Err(Fault::known(Known::PinWrong, WRONG_LIMIT.saturating_sub(book.wrong).to_string()))
        }
    }
}

/// After the recorded parameters change, every shape 2 or 3 seal's binding is recomputed (shape 1 has no such
/// field and is upgraded first).
fn rebind(book: &mut Book, mk: &[u8; MASTER_BYTES]) {
    if book.form == Form::V1 && !upgrade(book, mk) {
        return;
    }
    let kdf = kdf_of(book);
    if let Some(p) = book.pin.as_mut() {
        p.bind = Some(bind_of(mk, Kind::Pin, "", kdf, p));
    }
    for (id, s) in book.recovery.iter_mut() {
        s.bind = Some(bind_of(mk, Kind::Recovery, id, kdf, s));
    }
    for sl in book.slots.iter_mut() {
        sl.sealed.bind = Some(bind_of(mk, Kind::Slot, &sl.name, kdf, &sl.sealed));
    }
    if let Some(sealed) = book.primary.as_mut().and_then(|p| p.id_seal.as_mut()) {
        sealed.bind = Some(bind_of(mk, Kind::PrimaryId, PRIMARY_LABEL, kdf, sealed));
    }
}

/// Change the passcode: the old one must match (a mismatch counts like a wrong passcode), then the same
/// master key is resealed under the new one.
pub fn change_pin(old: &str, new: &str) -> Result<(), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::H2);
    if let Some(t) = pin_trouble(new) {
        return Err(Fault::known(Known::PinShape, t.as_str().to_string()));
    }
    // One lock covers "match the old, then switch to the new": another process writing between the two steps
    // would leave a half-old, half-new vault. The lock is taken before the unlock (`unlock_held`), so the
    // process never waits for itself.
    let held = lock_book()?;
    unlock_held(&held, old, false)?;
    let mk = master_now()?;
    let mut book = read_book()?.ok_or_else(|| Fault::known(Known::KeyboxMissing, String::new()))?;
    floor_gate(&book)?;
    let salt = rand(32)?;
    let kdf = kdf_of(&book);
    let dk = derive(kdf, new.as_bytes(), &salt)?;
    book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf, book.form)?);
    book.wrong = 0;
    write_book(&held, &book)?;
    note_pin(new);
    Ok(())
}

/// Recover: open the master key with the primary identity's own secret, set a new passcode, reset the count.
///
/// The secret is a recovery-word identity's 16 bytes of entropy or an existing-key identity's 32-byte private
/// key; `who` is the identity it derives to and `who_slots` the slot names that identity would hold here. Only
/// the primary identity's seal is tried (a vault written before the primary was recorded tries every seal, as
/// it did). A secret of another identity on this machine is refused by name with `RECOVERY_NOT_PRIMARY`; one
/// that matches nothing with `RECOVERY_NO_MATCH`. Either way neither the vault nor the count changes (wrong
/// words must not lock the person in further). With recorded parameters below the floor, it opens at the
/// floor and writes the floor back (the recovery reseal path).
pub fn recover(secret: &[u8], new_pin: &str, who: &str, who_slots: &[String]) -> Result<(), Fault> {
    if let Some(t) = pin_trouble(new_pin) {
        return Err(Fault::known(Known::PinShape, t.as_str().to_string()));
    }
    let held = lock_book()?;
    let mut book = read_book()?.ok_or_else(|| Fault::known(Known::KeyboxMissing, String::new()))?;
    let low = below_floor(kdf_of(&book));
    let kdf = if low { floor() } else { kdf_of(&book) };
    // Is this secret the primary's? Shape 3 tells by the member mark (the file names nobody); older shapes by
    // the plain id and slot names.
    let mark = member_tag(secret, &book.marks.unwrap_or_default());
    let not_primary = match &book.primary {
        Some(Primary { tag: Some(t), .. }) => *t != mark,
        Some(Primary { id: Some(p), .. }) => !p.eq_ignore_ascii_case(who),
        _ => false,
    };
    if not_primary {
        let here = if book.form == Form::V3 {
            book.slots.iter().any(|sl| sl.tag == Some(mark))
        } else {
            book.slots.iter().any(|sl| who_slots.iter().any(|w| *w == sl.name))
        };
        let k = if here { Known::RecoveryNotPrimary } else { Known::RecoveryNoMatch };
        return Err(Fault::known(k, String::new()));
    }
    let primary = book.primary.as_ref().and_then(|p| p.id.clone());
    let mut opened: Option<Key32> = None;
    for (id, s) in book.recovery.iter() {
        if let Some(p) = &primary {
            if !p.eq_ignore_ascii_case(id) {
                continue;
            }
        }
        let dk = derive(kdf, secret, &s.salt)?;
        if let Some(mk) = unseal(&dk.0, s).and_then(master_of) {
            // Seals whose identity does not match are skipped (shape 2): a renumbered seal or one with an
            // altered IV cannot yield a trustworthy master key.
            if bound(&mk.0, Kind::Recovery, id, kdf, s, book.form) {
                opened = Some(mk);
                break;
            }
        }
    }
    let Some(mk) = opened else {
        return Err(Fault::known(Known::RecoveryNoMatch, String::new()));
    };
    if low {
        book.kdf = Some(floor());
    }
    let _ = upgrade(&mut book, &mk.0);
    if low {
        rebind(&mut book, &mk.0);
    }
    let salt = rand(32)?;
    let kdf = kdf_of(&book);
    let dk = derive(kdf, new_pin.as_bytes(), &salt)?;
    book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf, book.form)?);
    book.wrong = 0;
    write_book(&held, &book)?;
    hold(&mk);
    note_pin(new_pin);
    Ok(())
}

/// The primary identity (id and kind). Shape 3 keeps the id sealed under the master key: this needs the vault
/// open (`LOCKED` otherwise); the kind alone is [`primary_kind`], readable while locked.
pub fn primary() -> Result<Option<(String, PrimaryKind)>, Fault> {
    let Some(book) = read_book()? else { return Ok(None) };
    let Some(pr) = book.primary.clone() else { return Ok(None) };
    if let Some(id) = pr.id {
        return Ok(Some((id, pr.kind)));
    }
    let mk = master_now()?;
    let sealed = pr.id_seal.ok_or_else(|| Fault::known(Known::KeyboxShape, member::PRIMARY.to_string()))?;
    if !bound(&mk.0, Kind::PrimaryId, PRIMARY_LABEL, kdf_of(&book), &sealed, book.form) {
        return Err(Fault::known(Known::KeyboxSlot, member::PRIMARY.to_string()));
    }
    let plain = unseal(&mk.0, &sealed).ok_or_else(|| Fault::known(Known::KeyboxSlot, member::PRIMARY.to_string()))?;
    let id = String::from_utf8(plain.bytes().to_vec()).map_err(|_| Fault::known(Known::KeyboxShape, member::PRIMARY.to_string()))?;
    Ok(Some((id, pr.kind)))
}

/// The primary identity's kind, readable while locked (the lock screen offers words or a key file by it).
pub fn primary_kind() -> Result<Option<PrimaryKind>, Fault> {
    Ok(read_book()?.and_then(|b| b.primary).map(|p| p.kind))
}

/// Settle the primary identity in a vault written before it was recorded (the first unlock after upgrading).
/// `rows` are this machine's identities as (id, kind, created); the primary is the earliest recovery-word
/// identity, or the earliest one when there is no recovery-word identity. Every other recovery seal is
/// dropped. Answers the chosen id when this pass settled it (the screen says once that only the primary
/// identity recovers the passcode); `None` when already settled or when there is no identity with a seal.
pub fn settle_primary(rows: &[(String, PrimaryKind, String)]) -> Result<Option<String>, Fault> {
    let _mk = master_now()?;
    let held = lock_book()?;
    let Some(mut book) = read_book()? else { return Ok(None) };
    if book.primary.is_some() {
        return Ok(None);
    }
    let sealed: Vec<&(String, PrimaryKind, String)> =
        rows.iter().filter(|(id, _, _)| book.recovery.iter().any(|(x, _)| x.eq_ignore_ascii_case(id))).collect();
    let pick = sealed
        .iter()
        .filter(|(_, k, _)| *k == PrimaryKind::Words)
        .min_by(|a, b| a.2.cmp(&b.2))
        .or_else(|| sealed.iter().min_by(|a, b| a.2.cmp(&b.2)));
    let Some((id, kind, _)) = pick.map(|x| (*x).clone()) else { return Ok(None) };
    book.recovery.retain(|(x, _)| x.eq_ignore_ascii_case(&id));
    book.primary = Some(Primary { id: Some(id.clone()), kind, tag: None, id_seal: None });
    write_book(&held, &book)?;
    Ok(Some(id))
}

/// Record the recovery seal of the primary identity. The vault has one primary identity and only its seal
/// opens the vault: when there is none yet (the first identity built on this machine), this identity becomes
/// primary, its seal is written and the header records it; when this identity is already primary its seal is
/// replaced (refilling a machine). Any other identity is secondary and gets no seal (answers false). Only while
/// open (the master key is in hand).
pub fn add_recovery(id: &str, kind: PrimaryKind, secret: &[u8]) -> Result<bool, Fault> {
    let mk = master_now()?;
    let held = lock_book()?;
    let mut book = read_book()?.unwrap_or_default();
    let marks = if book.form == Form::V3 { marks_of(&mut book)? } else { [0u8; 32] };
    let mark = member_tag(secret, &marks);
    match &book.primary {
        Some(Primary { tag: Some(t), .. }) if *t != mark => return Ok(false),
        Some(Primary { id: Some(p), .. }) if !p.eq_ignore_ascii_case(id) => return Ok(false),
        _ => {}
    }
    // Nothing is sealed under parameters below the floor: if the record was lowered during this session, this
    // seal is not written.
    floor_gate(&book)?;
    let salt = rand(32)?;
    let kdf = kdf_of(&book);
    let dk = derive(kdf, secret, &salt)?;
    if book.form == Form::V3 {
        let sealed = seal_master(&dk, salt, &mk.0, Kind::Recovery, PRIMARY_LABEL, kdf, book.form)?;
        book.recovery = vec![(PRIMARY_LABEL.to_string(), sealed)];
        let mut id_seal = seal(&mk.0, rand(32)?, id.as_bytes())?;
        id_seal.mac = None;
        id_seal.bind = Some(bind_of(&mk.0, Kind::PrimaryId, PRIMARY_LABEL, kdf, &id_seal));
        book.primary = Some(Primary { id: None, kind, tag: Some(mark), id_seal: Some(id_seal) });
    } else {
        let sealed = seal_master(&dk, salt, &mk.0, Kind::Recovery, id, kdf, book.form)?;
        book.recovery.retain(|(x, _)| x != id);
        book.recovery.push((id.to_string(), sealed));
        book.primary = Some(Primary { id: Some(id.to_string()), kind, tag: None, id_seal: None });
    }
    write_book(&held, &book)?;
    Ok(true)
}

/// Whether the vault still holds anything to lose: true when any recovery seal or key slot exists; false for
/// a vault with only a passcode and no identity yet.
///
/// The lock screen's "reset the vault" key and `reset_empty`'s refusal both ask this one function, so the key
/// never appears for a press that would be refused. Slots count: a slot is a key that could be lost.
pub fn recoverable() -> Result<bool, Fault> {
    let Some(book) = read_book()? else { return Ok(false) };
    Ok(!book.recovery.is_empty() || !book.slots.is_empty())
}

/// Delete an empty vault entirely. A machine with a passcode set and no identity yet that fails five times is
/// locked out, and with no recovery seal or slot neither recovery path can open it: there is no way back,
/// though there is nothing to lose. This is that way out.
///
/// With any slot or recovery seal present (`recoverable`) it is refused with `KEYBOX_NOT_EMPTY` and the file
/// is unchanged. An empty vault is deleted and the master key wiped at once; `state()` then answers `Absent`:
/// the shell draws, the wizard returns to step 1.
pub fn reset_empty() -> Result<(), Fault> {
    let _held = lock_book()?;
    if recoverable()? {
        let book = read_book()?.unwrap_or_default();
        return Err(Fault::known(
            Known::KeyboxNotEmpty,
            format!("slots={} seals={}", book.slots.len(), book.recovery.len()),
        ));
    }
    let p = path()?;
    match std::fs::remove_file(&p) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    }
    lock();
    Ok(())
}

/// Whether an identity still has its recovery seal (only the primary has one).
pub fn has_recovery(id: &str) -> Result<bool, Fault> {
    let Some(book) = read_book()? else { return Ok(false) };
    if book.form != Form::V3 {
        return Ok(book.recovery.iter().any(|(x, _)| x == id));
    }
    Ok(!book.recovery.is_empty() && primary()?.map(|(p, _)| p.eq_ignore_ascii_case(id)).unwrap_or(false))
}

/// Drop an identity's recovery seal (it goes with the identity when deleted).
pub fn drop_recovery(id: &str) -> Result<(), Fault> {
    let is_primary = has_recovery(id)?;
    let held = lock_book()?;
    let Some(mut book) = read_book()? else { return Ok(()) };
    if book.form != Form::V3 {
        book.recovery.retain(|(x, _)| x != id);
    } else if is_primary {
        book.recovery.clear();
        book.primary = None;
    } else {
        return Ok(());
    }
    write_book(&held, &book)
}

/// The local data key D, derived from the master key now (HKDF-SHA256 under [`LOCAL_INFO`]); wiped when
/// dropped. Refused by name with `LOCKED` while locked: no local file is read or written without it.
pub fn local_key() -> Result<LocalKey, Fault> {
    let mk = master_now()?;
    Ok(local_of(&mk.0))
}

/// The HKDF info string of the local data key (`<app>/local/v1`). One name, one home.
pub const LOCAL_INFO: &[u8] = b"zikaron/local/v1";

fn local_of(mk: &[u8; MASTER_BYTES]) -> LocalKey {
    let mut d = LocalKey([0u8; 32]);
    // 32 bytes is always within what HKDF can expand.
    let _ = crate::cryptx::hkdf_sha256(mk, LOCAL_INFO, &mut d.0);
    d
}

/// The names key NK, derived from the master key now (HKDF-SHA256 under [`NAMES_INFO`]): every name on disk
/// that would otherwise say an identity, an entry or a grant is keyed by it (`names`). It follows the master
/// key: the same master key gives the same names, a new master key gives new ones. Refused while locked.
pub fn name_key() -> Result<crate::names::NameKey, Fault> {
    let mk = master_now()?;
    Ok(names_of(&mk.0))
}

/// The HKDF info string of the names key (`<app>/names/v1`). One name, one home.
pub const NAMES_INFO: &[u8] = b"zikaron/names/v1";

fn names_of(mk: &[u8; MASTER_BYTES]) -> crate::names::NameKey {
    let mut n = [0u8; 32];
    let _ = crate::cryptx::hkdf_sha256(mk, NAMES_INFO, &mut n);
    let k = crate::names::NameKey::new(n);
    wipe(&mut n);
    k
}

/// The local data key in hand, wiped when dropped.
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

// A new master key: changing the primary identity and restoring from a backup.
//
// Both replace the master key M, so every seal and every local file is resealed. The new vault is built in
// memory ([`build_new`]), written beside the vault as `<vault>.zk-next` ([`stage_new`]), and becomes the vault
// by one rename ([`commit_new`]): that rename is the one moment the change takes effect. Local files are staged
// the same way (`<file>.zk-next`, sealed under the new key) before the commit and renamed after it; on the next
// unlock [`crate::local::settle_pending`] renames the ones that open under the vault's key and removes the ones
// that do not, so a cut at any moment leaves either the whole old state or the whole new one.

/// The suffix of a staged file. One name, one home.
pub const NEXT: &str = ".zk-next";

/// A vault built in memory with a new master key, not yet on disk.
pub struct NewBook {
    book: Book,
    mk: Key32,
}

impl NewBook {
    /// The local data key under the new master key (to seal the staged local files).
    pub fn local_key(&self) -> LocalKey {
        local_of(&self.mk.0)
    }

    /// The names key under the new master key (to name the staged local files).
    pub fn name_key(&self) -> crate::names::NameKey {
        names_of(&self.mk.0)
    }
}

/// How the new vault is sealed with a passcode.
pub enum PinFor<'a> {
    /// A new passcode (restoring on the locked card, or with the passcode given).
    New(&'a str),
    /// Keep this vault's passcode seal and master key (first run: the vault was made moments ago with no
    /// identity in it, so its master key is as new as a fresh one). Refused when the vault holds any seal or
    /// slot.
    KeepFresh,
}

/// Build a vault in memory: `pin` seals the master key; `primary` (id, kind, secret) gets the one recovery
/// seal; every `slots` entry is sealed under the master key. The count starts at zero. Nothing is written.
pub fn build_new(pin: PinFor, primary: Option<(&str, PrimaryKind, &[u8])>, slots: &[(String, Vec<u8>)]) -> Result<NewBook, Fault> {
    if let PinFor::New(p) = pin {
        if let Some(t) = pin_trouble(p) {
            return Err(Fault::known(Known::PinShape, t.as_str().to_string()));
        }
    }
    let old = read_book()?.unwrap_or_default();
    let mut book = Book { form: Form::V3, kdf: Some(old.kdf.unwrap_or_else(params)), ..Book::default() };
    if below_floor(kdf_of(&book)) {
        book.kdf = Some(floor());
    }
    let kdf = kdf_of(&book);
    let mk = match pin {
        PinFor::KeepFresh => {
            if !old.recovery.is_empty() || !old.slots.is_empty() || old.pin.is_none() {
                return Err(Fault::known(Known::KeyboxNotEmpty, format!("slots={} seals={}", old.slots.len(), old.recovery.len())));
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
            book.pin = Some(seal_master(&dk, salt, &mk.0, Kind::Pin, "", kdf, Form::V3)?);
            mk
        }
    };
    let kdf = kdf_of(&book);
    // Shape 3: names keyed by the new master key's names key, member marks salted anew.
    let nk = names_of(&mk.0);
    let marks = new_marks()?;
    book.marks = Some(marks);
    if let Some((id, kind, secret)) = primary {
        let salt = rand(32)?;
        let dk = derive(kdf, secret, &salt)?;
        book.recovery.push((PRIMARY_LABEL.to_string(), seal_master(&dk, salt, &mk.0, Kind::Recovery, PRIMARY_LABEL, kdf, Form::V3)?));
        let mut id_seal = seal(&mk.0, rand(32)?, id.as_bytes())?;
        id_seal.mac = None;
        id_seal.bind = Some(bind_of(&mk.0, Kind::PrimaryId, PRIMARY_LABEL, kdf, &id_seal));
        book.primary = Some(Primary { id: None, kind, tag: Some(member_tag(secret, &marks)), id_seal: Some(id_seal) });
    }
    for (a, plain) in slots {
        let name = nk.name(crate::names::Logical::Slot(a));
        let sealed = seal_slot(&mk.0, &name, kdf, Form::V3, &slot_plain(a, plain))?;
        book.slots.push(Slot { name, sealed, tag: Some(member_tag(plain, &marks)) });
    }
    book.wrong = 0;
    Ok(NewBook { book, mk })
}

/// Write the new vault beside the vault (`<vault>.zk-next`), under the vault lock.
pub fn stage_new(nb: &NewBook) -> Result<(), Fault> {
    let _held = lock_book()?;
    write_book_as(&nb.book, &next_name())
}

fn next_name() -> String {
    crate::local::staged_name(&crate::places::keybox_file())
}

/// The moment the new vault takes effect: the staged vault is renamed over the vault, and its master key is
/// held (the old one wiped).
pub fn commit_new(nb: NewBook) -> Result<(), Fault> {
    let _held = lock_book()?;
    let dir = crate::home::machine_dir()?;
    let from = dir.join(next_name());
    let to = dir.join(crate::places::keybox_file());
    crate::home::rename_over(&from, &to)?;
    hold(&nb.mk);
    PIN_DIGITS_ONLY.store(0, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// A staged vault left by a cut before its commit is not the vault: it is removed (the commit is the rename,
/// so a staged vault still on disk never took effect). Answers whether one was removed.
pub fn drop_staged() -> Result<bool, Fault> {
    let _held = lock_book()?;
    let p = crate::home::machine_dir()?.join(next_name());
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(classify(&e, &p.display().to_string())),
    }
}

// Slots.

/// The slot of `account` in a vault: by plain name (shapes 1 and 2) or by its keyed name (shape 3).
fn slot_name(book: &Book, mk: &[u8; MASTER_BYTES], account: &str) -> String {
    if book.form == Form::V3 {
        names_of(mk).name(crate::names::Logical::Slot(account))
    } else {
        account.to_string()
    }
}

/// The name the vault files `account`'s slot under, by the same lookup `get` and `put` use (read port: tests
/// take it from here rather than working it out). Refused by name while locked.
pub fn slot_name_of(account: &str) -> Result<Option<String>, Fault> {
    let mk = master_now()?;
    let Some(book) = read_book()? else { return Ok(None) };
    Ok(Some(slot_name(&book, &mk.0, account)))
}

/// Put one in. A slot with the same name is dropped first, so this places it exactly. Refused by name while
/// locked.
pub fn put(account: &str, secret: &[u8]) -> Result<(), Fault> {
    let mk = master_now()?;
    let held = lock_book()?;
    let mut book = read_book()?.unwrap_or_default();
    floor_gate(&book)?;
    let name = slot_name(&book, &mk.0, account);
    let (plain, tag) = if book.form == Form::V3 { (slot_plain(account, secret), Some(member_tag(secret, &marks_of(&mut book)?))) } else { (secret.to_vec(), None) };
    let sealed = seal_slot(&mk.0, &name, kdf_of(&book), book.form, &plain);
    let mut plain = plain;
    wipe(&mut plain);
    book.slots.retain(|sl| sl.name != name);
    book.slots.push(Slot { name, sealed: sealed?, tag });
    write_book(&held, &book)
}

/// Take one out. No such slot is `None`; locked is refused with `LOCKED`. A MAC mismatch (the `bind`, bound to
/// the slot name) is refused with `KEYBOX_SLOT`: if the two seats' ciphertexts were swapped, taking this
/// seat's name would return the other seat's key, and that stops here; shape 3 also checks the account name
/// sealed inside.
pub fn get(account: &str) -> Result<Option<Vec<u8>>, Fault> {
    let mk = master_now()?;
    let Some(book) = read_book()? else { return Ok(None) };
    let name = slot_name(&book, &mk.0, account);
    let Some(sl) = book.slots.iter().find(|sl| sl.name == name) else {
        return Ok(None);
    };
    if !bound(&mk.0, Kind::Slot, &name, kdf_of(&book), &sl.sealed, book.form) {
        return Err(Fault::known(Known::KeyboxSlot, account.to_string()));
    }
    let Some(mut plain) = unseal(&mk.0, &sl.sealed) else {
        // MAC mismatch: this slot does not match the master key (the file was altered, or two vaults were
        // mixed).
        return Err(Fault::known(Known::KeyboxSlot, account.to_string()));
    };
    if book.form != Form::V3 {
        // The returned copy is wiped by the receiver (`key.rs`, `identity::words_of`).
        return Ok(Some(std::mem::take(&mut plain.0)));
    }
    match slot_split(plain.bytes()) {
        Some((a, secret)) if a == account => Ok(Some(secret)),
        _ => Err(Fault::known(Known::KeyboxSlot, account.to_string())),
    }
}

/// Drop one. True when dropped, false when absent. Shape 3 finds the slot by its keyed name, so this needs the
/// vault open (deleting an identity asks the passcode anyway).
pub fn drop_item(account: &str) -> Result<bool, Fault> {
    let held = lock_book()?;
    let Some(mut book) = read_book()? else { return Ok(false) };
    let name = if book.form == Form::V3 { slot_name(&book, &master_now()?.0, account) } else { account.to_string() };
    let before = book.slots.len();
    book.slots.retain(|sl| sl.name != name);
    let gone = book.slots.len() != before;
    if gone {
        write_book(&held, &book)?;
    }
    Ok(gone)
}

/// Drop every slot of this vault (test vaults, swept whole: every slot of a test account's vault belongs to the
/// test). Needs no master key. Answers how many.
pub fn drop_all_slots() -> Result<usize, Fault> {
    let held = lock_book()?;
    let Some(mut book) = read_book()? else { return Ok(0) };
    let n = book.slots.len();
    if n > 0 {
        book.slots.clear();
        write_book(&held, &book)?;
    }
    Ok(n)
}

/// How many slots the vault holds (counting needs no master key).
pub fn slot_count() -> Result<usize, Fault> {
    Ok(read_book()?.map(|b| b.slots.len()).unwrap_or(0))
}

/// The account names in the vault now. Shape 3 keeps them sealed inside the slots: this needs the vault open.
pub fn accounts() -> Result<Vec<String>, Fault> {
    let Some(book) = read_book()? else { return Ok(Vec::new()) };
    if book.form != Form::V3 {
        return Ok(book.slots.iter().map(|sl| sl.name.clone()).collect());
    }
    let mk = master_now()?;
    let mut out = Vec::with_capacity(book.slots.len());
    for sl in &book.slots {
        let plain = unseal(&mk.0, &sl.sealed).ok_or_else(|| Fault::known(Known::KeyboxSlot, sl.name.clone()))?;
        let (a, mut secret) = slot_split(plain.bytes()).ok_or_else(|| Fault::known(Known::KeyboxSlot, sl.name.clone()))?;
        wipe(&mut secret);
        out.push(a);
    }
    Ok(out)
}

/// Whether a slot exists (not decrypted). Shape 3 needs the vault open to name it.
pub fn present(account: &str) -> Result<bool, Fault> {
    let Some(book) = read_book()? else { return Ok(false) };
    let name = if book.form == Form::V3 { slot_name(&book, &master_now()?.0, account) } else { account.to_string() };
    Ok(book.slots.iter().any(|sl| sl.name == name))
}
