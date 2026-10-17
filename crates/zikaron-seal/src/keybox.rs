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
//! 2. The primary identity's recovery seals, one for every secret that derives its signing keys: a
//! recovery-word identity's 16 bytes of entropy and each of its two seat keys, an existing-key identity's
//! 32-byte private key (a seat key or an existing key is recovered from its exported keystore V3 file and
//! the file's password).
//!
//! So after a forgotten passcode, or five failures that lock the vault, a person with the primary's recovery
//! words or one of its key files can still open it; opening that way reseals the passcode seal and resets the
//! failure count.
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
//! No plain private key or seed is in the vault file: bytes go through `crypto::aes128_ctr` with a keccak MAC
//! (the keystore V3 construction, [`crate::kdf::mac_of`]). Only the app's existing cryptographic
//! primitives are used.
//!
//! Plain text held in hand (the master key, derived keys, unsealed bytes) lives in [`Plain`] and [`Key32`]
//! and is wiped when dropped (`zikaron_os::wipe`); `MASTER` is wiped on lock. Keys returned by `get`
//! are wiped by the receiver (`key.rs`, `identity::words_of`). Temporary stack copies the compiler makes when
//! moving an array, and the primitives' own working memory, are outside what this file can wipe.
//!
//! Each seal binds its identity into its MAC: a `bind` field covered by the master key over kind, slot name or
//! label, IV, derivation parameters, salt and ciphertext, so a swapped slot (the author seat signing with the
//! grantee key), a changed IV or changed parameters are refused by name. Slots carry only `bind`; passcode and
//! recovery seals also keep `mac` (covered by the derived key, which checks the passcode or recovery secret).
//! This version reads only the shape it writes ([`SHAPE`]); a vault in any other shape, an earlier one
//! included, is refused by name and not touched.
//!
//! Derivation floor: the vault records its derivation parameters; below this binary's shipped parameters it
//! is refused with `KDF_BELOW_FLOOR` (not counted as a failure), and the person is guided to reseal at the
//! floor with the passcode or a recovery secret ([`reseal`], [`recover`]). Nothing is ever sealed under
//! parameters below the floor.

use crate::kdf::Params;
use crate::trouble::{Code, Trouble};
use std::path::{Path, PathBuf};
use zikaron::hexfmt;
use zikaron::json::{self, Value};

/// Vault file shape, the only one read: no plain name in the file leads to an identity on the chain (slot names
/// keyed by the names key, account names sealed inside the slots, the primary identity's id sealed under the
/// master key; only its kind stays plain). Any other shape is refused by name (`KEYBOX_SHAPE`).
pub const SHAPE: &str = "zikaron-desk/keybox/3";
/// A shape an earlier version wrote: kept as a literal of the family register only (no other product may take
/// it); this version refuses it like any shape not its own.
pub const SHAPE_V2: &str = "zikaron-desk/keybox/2";
/// The first shape an earlier version wrote: kept as a literal of the family register only; refused.
pub const SHAPE_V1: &str = "zikaron-desk/keybox/1";

/// A seal's IV length in bytes (AES-128-CTR).
pub const IV_LEN: usize = 16;

/// The vault file's member names (top level, each seal, the primary record, each slot, the derivation
/// parameters). The reader and writer here, and anything that reads the file's raw members, spell them only
/// through this table.
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
/// Passcode length: eight characters, each an ASCII letter or digit, case-sensitive.
pub const PIN_LEN: usize = 8;
/// Domain of the bind field (separate from the shape 1 `mac`, so neither can pose as the other).
pub const BIND_DOMAIN: &[u8] = b"zikaron-keybox/2";
/// Domain of a secret's member mark ([`member_tag`]).
pub const MEMBER_INFO: &[u8] = b"zikaron/keybox/member/v1";
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
    /// Bytes to be held and wiped when dropped.
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
    pin: Option<Sealed>,
    /// This vault's own derivation parameters, recorded when it is made (a vault without them is refused by
    /// name).
    kdf: Option<Params>,
    /// The primary's recovery seals, one per label of its kind ([`labels`]), in that order.
    recovery: Vec<Recovery>,
    slots: Vec<Slot>,
    wrong: u64,
    /// The primary identity: the only one whose recovery seal opens the vault. A vault with a recovery seal
    /// and no primary is refused by name.
    primary: Option<Primary>,
    /// This vault's salt for member marks, new with each new master key (see [`member_tag`]).
    marks: Option<[u8; 32]>,
}

/// One key slot: `name` is keyed by the names key (`names::Logical::Slot`) and the account name is sealed
/// inside with the secret; `tag` is the member mark of the secret (see [`member_tag`]).
#[derive(Clone, Debug)]
struct Slot {
    name: String,
    sealed: Sealed,
    tag: [u8; 32],
}

/// The primary identity as the file holds it: its id sealed under the master key (`id_seal`) and its kind
/// plain.
#[derive(Clone, Debug)]
struct Primary {
    kind: PrimaryKind,
    id_seal: Sealed,
}

/// One recovery seal of the primary: its label, `tag` the member mark of the secret it opens with (so recovery
/// finds the seal a secret opens, and tells the primary's secret from another identity's, while locked and
/// without the file naming anyone), and the master key sealed under that secret.
#[derive(Clone, Debug)]
struct Recovery {
    label: String,
    tag: [u8; 32],
    sealed: Sealed,
}

/// The label of the primary's own secret's seal (a recovery-word identity's entropy, an existing key), and of
/// the sealed primary id.
const PRIMARY_LABEL: &str = "primary";

/// The labels of a primary's recovery seals, in order, by its kind (a closed table): the primary's own secret,
/// then for a recovery-word identity each seat key (author, grantee), so its key files recover too.
fn labels(kind: PrimaryKind) -> &'static [&'static str] {
    match kind {
        PrimaryKind::Words => &[PRIMARY_LABEL, "primary-author", "primary-grantee"],
        PrimaryKind::KeyFile => &[PRIMARY_LABEL],
    }
}

/// The primary's recovery seals, sealed anew under `mk`: one per secret, each under a fresh salt. `secrets` are
/// every secret a primary of `kind` recovers with, in [`labels`] order (the desktop derives a recovery-word
/// identity's seat keys from its entropy); one too few or too many is refused by name.
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

/// The member mark of a secret (a recovery-word identity's entropy, an existing key's private key): keyed by
/// the secret itself, so it says nothing to anyone without that secret, and a secret handed in at recovery
/// finds its own mark. Salted by the vault's own salt, drawn anew with each new master key, so no mark is
/// shared between the vault before a master key change and the one after it.
fn member_tag(secret: &[u8], marks: &[u8; 32]) -> [u8; 32] {
    let mut msg = MEMBER_INFO.to_vec();
    msg.push(0);
    msg.extend_from_slice(marks);
    crate::crypto::hmac_sha256(secret, &msg)
}

/// This vault's salt for member marks, drawn when a shape 3 vault has none yet (a vault being made).
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

/// A new salt for member marks.
fn new_marks() -> Result<[u8; 32], Trouble> {
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

/// Where a vault lives: its directory (the machine directory), its file name and its lock file's name (both
/// named per account base, `crate::places`). Each end resolves these its own way and hands them in; nothing here
/// looks for a machine directory.
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

    /// The vault file.
    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.file)
    }

    /// The directory it lives in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Its file name.
    pub fn file(&self) -> &str {
        &self.file
    }

    fn lock_path(&self) -> PathBuf {
        self.dir.join(&self.lock)
    }
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

/// Take the vault lock, waiting until it is available; creates the directory first if missing.
///
/// Every vault change (set passcode, unlock, change passcode, recover, add or drop a recovery seal, put or
/// drop a slot, reset an empty vault) starts here, and [`write_book`] takes a reference to the lock, so a
/// write without the lock does not compile.
fn lock_book(v: &Vault) -> Result<Held, Trouble> {
    let p = v.lock_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| Trouble::io(&e, &d.display().to_string()))?;
    }
    let mut o = zikaron_os::Options::new();
    o.create(true).read(true).write(true).truncate(false);
    // The lock file is owner-only too (like the vault file and the registry). It holds no bytes, but anyone
    // who can open it can lock it: a lock needs only an open handle, read-only included. Readable by others,
    // another account on the same machine could hold the lock forever while this side waits, and the passcode
    // gate would hang.
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

/// Read one seal: slots and the primary's id only `bind`; passcode and recovery seals both.
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
    // Only the shape this version writes: an earlier shape is refused like any other, by name, untouched.
    match text(&v, member::SHAPE).as_str() {
        SHAPE => {}
        other => return Err(Trouble::new(Code::KeyboxShape, other.to_string())),
    }
    // A missing or malformed failure count is refused by name. Reading it as 0 would let someone able to edit
    // the file clear the count by deleting a field, silently. Nothing in the vault changes; it just does not
    // open.
    let wrong = match v.member(member::WRONG) {
        Some(Value::Int(n)) => *n,
        _ => return Err(Trouble::new(Code::KeyboxShape, member::WRONG.to_string())),
    };
    let mut book = Book { wrong, ..Book::default() };
    // The vault states its own derivation parameters. All three are read and checked against bounds
    // (`kdf::in_range`, the same gate as keystore V3 files): out of bounds is refused by name, so a file
    // cannot make derivation use terabytes of memory. A vault without this section is refused by name: this
    // version writes it into every vault it makes.
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
        // The kind plain, the id sealed.
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
    // Recovery seals open the vault only as the primary's, exactly one per label of its kind, in order; a
    // primary with no seals is one whose identity was deleted from under it, which this version never writes.
    // Anything else is refused by name.
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

/// Writes the vault. The first argument is the lock in hand, so a write without the lock does not compile
/// (the tests also check that every write is under the lock).
fn write_book(v: &Vault, _held: &Held, b: &Book) -> Result<(), Trouble> {
    write_book_as(v, b, &v.file)
}

/// The vault's bytes under a name in the machine directory (the vault itself, or the staged next vault).
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
    // One way to write, and it is not in this file. The vault is rewritten whole on every change, so
    // replacing the old file is intended; a direct write, cut off by a power loss or kill, would leave half a
    // file and every key and recovery seal unreadable. Landing lives in `land::put_at`, which also sets
    // permissions, the atomic rename and the unique temporary name.
    crate::land::put_at(&v.dir, name, &bytes)
}

/// Which derivation parameters to use. The shipped binary always uses the standard ones (memory-hard, about
/// one second); the test hooks set light ones (the test profile, milliseconds), since a test run performs
/// hundreds of derivations.
///
/// As with [`crate::places`]: set at most once, and only the test hooks set it; the window never calls it
/// (the tests scan for this), so the shipped app always runs the standard parameters.
static KDF: std::sync::OnceLock<Params> = std::sync::OnceLock::new();

/// The lightest parameters, for tests: within bounds (`kdf::in_range`) at nearly no cost. Passcode
/// strength is carried by the shipped parameters; tests exercise passcode behavior, not derivation time.
pub const PROBE_KDF: Params = Params { n: 2, r: 8, p: 1 };

/// Set light parameters once (test hooks only). `None` means [`PROBE_KDF`]; given parameters are set as given
/// (tests use this to check that the same passcode still opens when the vault's recorded parameters differ
/// from this pass's). Out of bounds is never set. A second call returns false, never silently replacing the
/// first.
pub fn set_light_kdf(p: Option<Params>) -> bool {
    let p = p.unwrap_or(PROBE_KDF);
    if !crate::kdf::in_range(p.n, p.r, p.p) {
        return false;
    }
    KDF.set(p).is_ok()
}

/// This pass's derivation parameters.
pub fn params() -> Params {
    *KDF.get_or_init(Params::standard)
}

/// Wipe plain text in hand (master key, returned keys, derived keys): the platform piece's one wipe.
fn wipe(b: &mut [u8]) {
    zikaron_os::wipe(b);
}

/// Which parameters this vault uses: its recorded section when present, otherwise this pass's [`params`]
/// (older vaults were sealed under them).
fn kdf_of(b: &Book) -> Params {
    b.kdf.unwrap_or_else(params)
}

/// The derivation floor: this binary's shipped parameters (standard for the window, the light ones the test
/// hooks set for tests).
pub fn floor() -> Params {
    params()
}

/// Whether parameters are below the floor: memory (n·r) or work (n·r·p), either below is below.
pub fn below_floor(k: Params) -> bool {
    let f = floor();
    let mem = |p: Params| p.n.saturating_mul(p.r);
    let work = |p: Params| p.n.saturating_mul(p.r).saturating_mul(p.p);
    mem(k) < mem(f) || work(k) < work(f)
}

fn kdf_text(k: Params) -> String {
    format!("n={} r={} p={}", k.n, k.r, k.p)
}

/// Whether this vault can be used: recorded parameters below the floor are refused by name (neither sealing
/// nor opening under a lowered file).
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

/// The bind field: the last 16 bytes of the master key cover kind, identity, IV, parameters, salt and
/// ciphertext (variable-length fields carry their lengths, so the concatenation is unambiguous).
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

/// Whether a seal's identity matches.
fn bound(mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: Params, s: &Sealed) -> bool {
    s.bind == Some(bind_of(mk, kind, label, kdf, s))
}

/// Seal bytes with a key (`mac` by this key; `bind` added by the master key).
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

/// Seal the master key (passcode and recovery seals): `mac` by the derived key, `bind` by the master key.
fn seal_master(dk: &Key32, salt: Vec<u8>, mk: &[u8; MASTER_BYTES], kind: Kind, label: &str, kdf: Params) -> Result<Sealed, Trouble> {
    let mut s = seal(&dk.0, salt, mk)?;
    s.bind = Some(bind_of(mk, kind, label, kdf, &s));
    Ok(s)
}

/// Seal a slot: only `bind` (by the master key).
fn seal_slot(mk: &[u8; MASTER_BYTES], account: &str, kdf: Params, plain: &[u8]) -> Result<Sealed, Trouble> {
    let salt = rand(32)?;
    let mut s = seal(mk, salt, plain)?;
    s.mac = None;
    s.bind = Some(bind_of(mk, Kind::Slot, account, kdf, &s));
    Ok(s)
}

/// Open a seal. A MAC mismatch gives `None` (verify first, then decrypt: a wrong passcode stops at the MAC).
/// Slots have no `mac`; their check is `bind` (the caller asks [`bound`] first).
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

/// Hold a master key opened elsewhere (the command line opens it with its credential, `credential`): from now on
/// this process reads the vault as open. The bytes are copied; the caller wipes its own.
pub fn hold_master(mk: &[u8; 32]) {
    hold(&Key32(*mk));
}

/// The master key in hand, lent to `f` (the desktop seals it under a command-line credential's key). Refused by
/// name while locked.
pub fn with_master<R>(f: impl FnOnce(&[u8; 32]) -> R) -> Result<R, Trouble> {
    let mk = master_now()?;
    Ok(f(&mk.0))
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

/// The vault's state now. One disk read; unreadable is refused by name.
pub fn state(v: &Vault) -> Result<State, Trouble> {
    let open = MASTER.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    let Some(book) = read_book(v)? else {
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
pub fn tries_left(v: &Vault) -> Result<u64, Trouble> {
    let wrong = read_book(v)?.map(|b| b.wrong).unwrap_or(0);
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
    // A vault without a passcode can only be empty or a test vault the test hooks filled with slots: slots stay.
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

/// Unlock. Correct: the master key goes into memory and the count resets. Wrong: the count increments on disk
/// and the answer says how many tries remain. With all tries used only recovery remains (this is still
/// refused by name without trying the passcode). Recorded parameters below the floor are refused with
/// `KDF_BELOW_FLOOR` and not counted (the file was lowered; the passcode is not at fault).
pub fn unlock(v: &Vault, pin: &str) -> Result<(), Trouble> {
    let held = lock_book(v)?;
    unlock_held(v, &held, pin, false)
}

/// Check a passcode without opening the vault: the gate in front of an action that asks for the passcode again
/// on an open vault (showing the words, deleting an identity, exporting a key or a backup, signing for another)
/// only confirms who is at the machine. The tries count as for [`unlock`] (recorded before deriving, cleared on
/// a correct passcode, the last failure locks) and are refused the same way (no vault, locked out, recorded
/// parameters below the floor), but the master key never enters memory and a lock that is pending or done stays
/// as it is: a correct passcode checked here reopens nothing.
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
    // Recorded before deriving, as in `unlock_held`, so parallel checks cannot each spend the same try.
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

/// Reseal at the floor: when the recorded parameters are below the floor, open the passcode seal at the
/// floor, then write the floor back and reseal the passcode seal under it. That opening counts in the same
/// failure record. A vault not below the floor unlocks normally.
pub fn reseal(v: &Vault, pin: &str) -> Result<(), Trouble> {
    let held = lock_book(v)?;
    unlock_held(v, &held, pin, true)
}

/// The trouble when a reseal did not land (only after a successful open): taken where the unlock lands and
/// said once on screen. It is kept here instead of returned as an error because the vault did open; an error
/// would count a correct passcode as wrong.
static RESEAL_TROUBLE: std::sync::Mutex<Option<Trouble>> = std::sync::Mutex::new(None);

fn note_reseal_trouble(f: Trouble) {
    if let Ok(mut g) = RESEAL_TROUBLE.lock() {
        *g = Some(f);
    }
}

/// Take the last reseal trouble (empty after taking).
pub fn take_reseal_trouble() -> Option<Trouble> {
    RESEAL_TROUBLE.lock().ok().and_then(|mut g| g.take())
}


/// The unlock, run under the vault lock. Changing the passcode takes the lock first and then comes here, so
/// the process never waits for itself. `at_floor`: open at the floor and write the floor back (the reseal
/// path); otherwise recorded parameters below the floor are refused by name.
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
    // The reseal path opens at the floor: the seals were made at the floor (only the recorded section was
    // lowered), so opening at the lowered parameters would fail.
    let kdf = if low { floor() } else { kdf_of(&book) };
    // Record the attempt first, then try. Incrementing only after the one-second derivation would let N
    // processes each read the same count and each write back one more, turning five tries into N × 5. So the
    // attempt is recorded on disk before deriving and cleared only on success.
    book.wrong += 1;
    write_book(v, held, &book)?;
    let dk = derive(kdf, pin.as_bytes(), &sealed.salt)?;
    let opened = unseal(&dk.0, &sealed).and_then(master_of);
    match opened {
        Some(mk) => {
            // An identity mismatch is refused by name (shape 2): the passcode was right (the derived key's
            // MAC passed) but this seal was altered (IV, parameters or the whole seal). The attempt is
            // cleared and the master key does not enter memory.
            if !bound(&mk.0, Kind::Pin, "", kdf, &sealed) {
                book.wrong = 0;
                let _ = write_book(v, held, &book);
                return Err(Trouble::new(Code::KeyboxSlot, member::PIN.to_string()));
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
                let done = (|| -> Result<(), Trouble> {
                    let salt = rand(32)?;
                    let dk2 = derive(floor(), pin.as_bytes(), &salt)?;
                    resealed.pin = Some(seal_master(&dk2, salt, &mk.0, Kind::Pin, "", floor())?);
                    rebind(&mut resealed, &mk.0);
                    write_book(v, held, &resealed)
                })();
                if let Err(f) = done {
                    // The reseal failed: write the cleared record (old parameters) once as best effort and
                    // hand the trouble to the shell. The vault did open, so no error; the failure must be
                    // visible, or the next start would ask the same thing with nobody saying why.
                    let _ = write_book(v, held, &book);
                    note_reseal_trouble(f);
                }
                return Ok(());
            }
            let _ = write_book(v, held, &book);
            Ok(())
        }
        None => {
            // On the final failure, take the key back at once: an open session that uses up all tries ends
            // there, the master key is wiped, and the next frame shows the locked-out card.
            if book.wrong >= WRONG_LIMIT {
                lock();
            }
            Err(Trouble::new(Code::PinWrong, WRONG_LIMIT.saturating_sub(book.wrong).to_string()))
        }
    }
}

/// After the recorded parameters change, every seal's binding is recomputed.
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

/// Change the passcode: the old one must match (a mismatch counts like a wrong passcode), then the same
/// master key is resealed under the new one.
pub fn change_pin(v: &Vault, old: &str, new: &str) -> Result<(), Trouble> {
    if let Some(t) = pin_trouble(new) {
        return Err(Trouble::new(Code::PinShape, t.as_str().to_string()));
    }
    // One lock covers "match the old, then switch to the new": another process writing between the two steps
    // would leave a half-old, half-new vault. The lock is taken before the unlock (`unlock_held`), so the
    // process never waits for itself.
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

/// Recover: open the master key with one of the primary identity's secrets, set a new passcode, reset the
/// count.
///
/// The secret is a recovery-word identity's 16 bytes of entropy or one of its seat keys, or an existing-key
/// identity's 32-byte private key; its member mark finds the one primary's seal it opens. A secret of another
/// identity on this machine is refused by name with `RECOVERY_NOT_PRIMARY`; one that matches nothing with
/// `RECOVERY_NO_MATCH`. Either way neither the vault nor the count changes (wrong words must not lock the
/// person in further). With recorded parameters below the floor, it opens at the floor and writes the floor
/// back (the recovery reseal path).
pub fn recover(v: &Vault, secret: &[u8], new_pin: &str) -> Result<(), Trouble> {
    if let Some(t) = pin_trouble(new_pin) {
        return Err(Trouble::new(Code::PinShape, t.as_str().to_string()));
    }
    let held = lock_book(v)?;
    let mut book = read_book(v)?.ok_or_else(|| Trouble::new(Code::KeyboxMissing, String::new()))?;
    let low = below_floor(kdf_of(&book));
    let kdf = if low { floor() } else { kdf_of(&book) };
    // Which of the primary's seals does this secret open? Told by the member mark (the file names nobody).
    let mark = member_tag(secret, &book.marks.unwrap_or_default());
    let Some(r) = book.recovery.iter().find(|r| r.tag == mark) else {
        let here = book.slots.iter().any(|sl| sl.tag == mark);
        let k = if here { Code::RecoveryNotPrimary } else { Code::RecoveryNoMatch };
        return Err(Trouble::new(k, String::new()));
    };
    let dk = derive(kdf, secret, &r.sealed.salt)?;
    // A seal whose identity does not match does not open: a relabelled seal or one with an altered IV cannot
    // yield a trustworthy master key.
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

/// The primary identity (id and kind). Shape 3 keeps the id sealed under the master key: this needs the vault
/// open (`LOCKED` otherwise); the kind alone is [`primary_kind`], readable while locked.
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

/// The primary identity's kind, readable while locked (the lock screen offers words or a key file by it).
pub fn primary_kind(v: &Vault) -> Result<Option<PrimaryKind>, Trouble> {
    Ok(read_book(v)?.and_then(|b| b.primary).map(|p| p.kind))
}

/// Record the recovery seals of the primary identity (one per secret, [`labels`]; `secrets` in that order, the
/// primary's own first). The vault has one primary identity and only its seals open the vault: when there is
/// none yet (the first identity built on this machine), this identity becomes primary, its seals are written
/// and the header records it; when this identity is already primary its seals are replaced (refilling a
/// machine). Any other identity is secondary and gets no seal (answers false). Only while open (the master key
/// is in hand).
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
    // Nothing is sealed under parameters below the floor: if the record was lowered during this session, no
    // seal is written.
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

/// Whether the vault still holds anything to lose: true when any recovery seal or key slot exists; false for
/// a vault with only a passcode and no identity yet.
///
/// The lock screen's "reset the vault" key and `reset_empty`'s refusal both ask this one function, so the key
/// never appears for a press that would be refused. Slots count: a slot is a key that could be lost.
pub fn recoverable(v: &Vault) -> Result<bool, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(false) };
    Ok(!book.recovery.is_empty() || !book.slots.is_empty())
}

/// Delete an empty vault entirely. A machine with a passcode set and no identity yet that fails five times is
/// locked out, and with no recovery seal or slot neither recovery path can open it: there is no way back,
/// though there is nothing to lose. This is that way out.
///
/// With any slot or recovery seal present (`recoverable`) it is refused with `KEYBOX_NOT_EMPTY` and the file
/// is unchanged. An empty vault is deleted and the master key wiped at once; `state()` then answers `Absent`:
/// the shell draws, the wizard returns to step 1.
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

/// Whether an identity still has its recovery seals (only the primary has them).
pub fn has_recovery(v: &Vault, id: &str) -> Result<bool, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(false) };
    Ok(!book.recovery.is_empty() && primary(v)?.map(|(p, _)| p.eq_ignore_ascii_case(id)).unwrap_or(false))
}

/// Drop an identity's recovery seals (they go with the identity when deleted).
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

/// The local data key D, derived from the master key now (HKDF-SHA256 under [`LOCAL_INFO`]); wiped when
/// dropped. Refused by name with `LOCKED` while locked: no local file is read or written without it.
pub fn local_key() -> Result<LocalKey, Trouble> {
    let mk = master_now()?;
    Ok(local_of(&mk.0))
}

/// The HKDF info string of the local data key (`<app>/local/v1`).
pub const LOCAL_INFO: &[u8] = b"zikaron/local/v1";

fn local_of(mk: &[u8; MASTER_BYTES]) -> LocalKey {
    let mut d = LocalKey([0u8; 32]);
    // 32 bytes is always within what HKDF can expand.
    let _ = crate::crypto::hkdf_sha256(mk, LOCAL_INFO, &mut d.0);
    d
}

/// The names key NK, derived from the master key now (HKDF-SHA256 under [`NAMES_INFO`]): every name on disk
/// that would otherwise say an identity, an entry or a grant is keyed by it (`names`). It follows the master
/// key: the same master key gives the same names, a new master key gives new ones. Refused while locked.
pub fn name_key() -> Result<crate::names::NameKey, Trouble> {
    let mk = master_now()?;
    Ok(names_of(&mk.0))
}

/// The HKDF info string of the names key (`<app>/names/v1`).
pub const NAMES_INFO: &[u8] = b"zikaron/names/v1";

fn names_of(mk: &[u8; MASTER_BYTES]) -> crate::names::NameKey {
    let mut n = [0u8; 32];
    let _ = crate::crypto::hkdf_sha256(mk, NAMES_INFO, &mut n);
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
// unlock the desktop's `local::settle_pending` renames the ones that open under the vault's key and removes the ones
// that do not, so a cut at any moment leaves either the whole old state or the whole new one.

/// The suffix of a staged file.
pub use crate::envelope::NEXT;

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

/// Build a vault in memory: `pin` seals the master key; `primary` (id, kind, every secret it recovers with in
/// [`labels`] order) gets its recovery seals; every `slots` entry is sealed under the master key. The count
/// starts at zero. Nothing is written.
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
    // Names keyed by the new master key's names key, member marks salted anew.
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

/// Write the new vault beside the vault (`<vault>.zk-next`), under the vault lock.
pub fn stage_new(v: &Vault, nb: &NewBook) -> Result<(), Trouble> {
    let _held = lock_book(v)?;
    write_book_as(v, &nb.book, &next_name(v))
}

fn next_name(v: &Vault) -> String {
    crate::envelope::staged_name(&v.file)
}

/// The moment the new vault takes effect: the staged vault is renamed over the vault, and its master key is
/// held (the old one wiped).
pub fn commit_new(v: &Vault, nb: NewBook) -> Result<(), Trouble> {
    let _held = lock_book(v)?;
    let from = v.dir.join(next_name(v));
    let to = v.path();
    zikaron_os::rename_over(&from, &to).map_err(|u| Trouble::io(&u.error, &u.at.display().to_string()))?;
    hold(&nb.mk);
    PIN_DIGITS_ONLY.store(0, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// A staged vault left by a cut before its commit is not the vault: it is removed (the commit is the rename,
/// so a staged vault still on disk never took effect). Answers whether one was removed.
pub fn drop_staged(v: &Vault) -> Result<bool, Trouble> {
    let _held = lock_book(v)?;
    let p = v.dir.join(next_name(v));
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Trouble::io(&e, &p.display().to_string())),
    }
}

// Slots.

/// The slot of `account` in a vault: by its keyed name.
fn slot_name(mk: &[u8; MASTER_BYTES], account: &str) -> String {
    names_of(mk).name(crate::names::Logical::Slot(account))
}

/// The name the vault files `account`'s slot under, by the same lookup `get` and `put` use (tests take it
/// from here rather than computing it). Refused by name while locked.
pub fn slot_name_of(v: &Vault, account: &str) -> Result<Option<String>, Trouble> {
    let mk = master_now()?;
    if read_book(v)?.is_none() {
        return Ok(None);
    }
    Ok(Some(slot_name(&mk.0, account)))
}

/// Put one in. A slot with the same name is dropped first, so this places it exactly. Refused by name while
/// locked.
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

/// Take one out. No such slot is `None`; locked is refused with `LOCKED`. A MAC mismatch (the `bind`, bound to
/// the slot name) is refused with `KEYBOX_SLOT`: if the two seats' ciphertexts were swapped, taking this
/// seat's name would return the other seat's key, and that stops here; shape 3 also checks the account name
/// sealed inside.
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
        // MAC mismatch: this slot does not match the master key (the file was altered, or two vaults were
        // mixed).
        return Err(Trouble::new(Code::KeyboxSlot, account.to_string()));
    };
    // The returned copy is wiped by the receiver (`key.rs`, `identity::words_of`).
    match slot_split(plain.bytes()) {
        Some((a, secret)) if a == account => Ok(Some(secret)),
        _ => Err(Trouble::new(Code::KeyboxSlot, account.to_string())),
    }
}

/// Drop one. True when dropped, false when absent. Shape 3 finds the slot by its keyed name, so this needs the
/// vault open (deleting an identity asks the passcode anyway).
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

/// Drop every slot of this vault (test vaults, swept whole: every slot of a test account's vault belongs to the
/// test). Needs no master key. Answers how many.
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

/// How many slots the vault holds (counting needs no master key).
pub fn slot_count(v: &Vault) -> Result<usize, Trouble> {
    Ok(read_book(v)?.map(|b| b.slots.len()).unwrap_or(0))
}

/// The account names in the vault now, kept sealed inside the slots: this needs the vault open.
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

/// Whether a slot exists (not decrypted). Naming it needs the vault open.
pub fn present(v: &Vault, account: &str) -> Result<bool, Trouble> {
    let Some(book) = read_book(v)? else { return Ok(false) };
    let name = slot_name(&master_now()?.0, account);
    Ok(book.slots.iter().any(|sl| sl.name == name))
}

/// Whether the master key in hand is this vault's: its passcode seal is bound under it. No passcode is derived
/// and no try is counted. The command line asks this of a credential's master key before reading anything under
/// it, so a credential issued before a new master key took effect is refused by name, not read as damage.
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
