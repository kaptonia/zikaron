//! The envelope every local file is sealed in, the names those files take on disk, and a home's label: one
//! implementation for the desktop and the command line (the desktop's `local` module walks and places files;
//! this is how each one is sealed, opened and named).
//!
//! ─── Envelope ───
//!
//! [`MAGIC_V2`] · algorithm (1 byte, [`ALG_KEYED`]) · kind length (1 byte) · kind · format version (2 bytes,
//! big-endian) · identity digest (16 bytes: 8 from the owner, 8 from the place's logical name keyed under the
//! local data key; [`Ident::digest_keyed`]) · nonce (24 bytes) · ciphertext and tag. The whole head, nonce
//! included, is the AEAD additional data, so a changed head byte, or a file moved to another place or home, does
//! not open. The owner and place are repeated as text inside the ciphertext and checked after opening.
//!
//! Reading accepts only this envelope under the algorithm written ([`ALG_KEYED`]); anything else is refused as
//! not sealed by this version's terms. A file that cannot be read is refused `LOCAL_SEAL`, whose evidence starts
//! with one of six reasons ([`Unread`]). The digest is compared before decrypting, so a swapped file and a wrong
//! key are distinguishable.
//!
//! ─── Names on disk ───
//!
//! A file's logical path names it by content (`ledger/<entry id>.entry`, `grants-held/<grant id>.entry`, ...);
//! the on-disk path replaces those ids with names derived from the names key ([`disk_rel`]). Only this module
//! translates between the two.

use crate::keybox::LocalKey;
use crate::names::NameKey;
use crate::trouble::{Code, Coded, Trouble};
use std::path::{Path, PathBuf};

/// A home's directories, by name (the desktop's `home::Slot` reads them here).
pub const LEDGER_DIR: &str = "ledger";
pub const KITS_DIR: &str = "kits";
pub const GRANTS_HELD_DIR: &str = "grants-held";
pub const SETTINGS_DIR: &str = "settings";
/// A home's writer lock file, in its settings room: only ever locked (the kernel's lock), never read for a
/// decision. The desktop holds it while it writes the home; the command line takes it for one write.
pub const WRITER_LOCK: &str = "writer.lock";
/// A held grant's verdict file suffix (beside the held grant, same stem).
pub const VERDICT_SUFFIX: &str = ".verdict.json";
/// The directory, under `grants-held/`, of kept grant files.
pub const KEPT_DIR: &str = "files";
/// The words of the two seats a home belongs to (the desktop's roles read them here).
pub const SEATS: [&str; 2] = ["author", "grantee"];
/// The suffix of a staged file (a change's file sealed under the next master key, beside its place).
pub const NEXT: &str = ".zk-next";

/// The staged copy's file name (the name plus [`NEXT`]).
pub fn staged_name(name: &str) -> String {
    format!("{name}{NEXT}")
}

/// Whether a path (or a relative path) names a staged copy.
pub fn is_staged(p: &str) -> bool {
    p.ends_with(NEXT)
}

/// A staged copy's path without its staged suffix (`None` when it is not one).
pub fn unstaged(p: &str) -> Option<&str> {
    p.strip_suffix(NEXT)
}

/// Magic bytes of the envelope an earlier version wrote (refused, kept as a family literal) and of the one
/// read and written.
pub use zikaron_glue::sealed::{MAGIC, MAGIC_V2};
/// The nonce length of the cipher (XChaCha20-Poly1305).
pub const NONCE: usize = 24;
/// Length of the identity digest in the envelope head.
pub const DIGEST: usize = 16;
/// Algorithm 2, the one read and written (an earlier version's algorithm 1, unkeyed, is refused as not
/// sealed): XChaCha20-Poly1305 with the place half of the identity digest keyed (HMAC under a key derived from
/// the local data key, [`PLACE_KEY_INFO`]), so the heads on a locked disk cannot be matched against public
/// logical names (entry and grant ids). Future algorithms take new numbers under the same magic.
pub const ALG_KEYED: u8 = 2;
/// HKDF info for the key that keys the place half (derived from the local data key).
pub const PLACE_KEY_INFO: &str = "zikaron-local/place-key";
/// Domain separator for the owner half of the identity digest.
pub const OWNER_DOMAIN: &str = "zikaron-local/owner";
/// Domain separator for the place half of the identity digest.
pub const PLACE_DOMAIN: &str = "zikaron-local/place";
/// The home label's file name, in the home's `settings/` directory.
pub const LABEL_FILE: &str = "home-label.json";

/// The sealed file kinds. Each has a tag and a format version, bound into the envelope head.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Doc {
    /// Machine directory: the identity register.
    Registry,
    /// Machine directory: hand-filled kit links (`kits/links.json`).
    KitLinks,
    /// Machine directory: the local index of signed files (`records/index.json`).
    Records,
    /// Machine directory: chain facts already checked, shared across endpoints (`checked/facts.json`,
    /// `checkedx`).
    Checked,
    /// Home `settings/`: the settings file.
    Settings,
    /// Home `settings/`: the anchor queue.
    Queue,
    /// Home `settings/`: the first-run checklist.
    FirstWindow,
    /// Home `settings/`: the last audit's anchored set.
    LastAudit,
    /// Home `settings/`: the restored identity's read-only mark.
    Unfetched,
    /// Home `grants-held/`: a held grant's entry bytes.
    Held,
    /// Home `grants-held/`: a held grant's last verdict.
    Verdict,
    /// Home `grants-held/files/`: a kept grant file.
    KeptGrant,
    /// Home `kits/terms/<digest>/`: a terms document received at signing.
    TermsDoc,
    /// Home `kits/terms/`: a grant's issuance record.
    TermsRecord,
    /// Home `ledger/`: one ledger entry.
    Entry,
    /// Machine directory: the plain machine settings staged by a restore (the file itself stays plain, see the
    /// desktop's `machine`); only ever a staged file.
    Machine,
    /// Machine directory: a change's plan (what moves and where it was), staged with the change; only ever a
    /// staged file.
    Plan,
    /// Home `settings/`: the home's label (its number, and which identity it belongs to).
    HomeLabel,
    /// Machine directory: what the last whole-machine backup holds (`backup/index.json`, `backup`).
    BackupIndex,
    /// Machine directory: the machine settings that are not read before unlocking (auto-lock, proxy, the
    /// command line's switches, the homes used, backup records; `places::machine_file_for`).
    MachineSealed,
}

impl Doc {
    pub const ALL: [Doc; 20] = [
        Doc::Registry,
        Doc::KitLinks,
        Doc::Records,
        Doc::Checked,
        Doc::Settings,
        Doc::Queue,
        Doc::FirstWindow,
        Doc::LastAudit,
        Doc::Unfetched,
        Doc::Held,
        Doc::Verdict,
        Doc::KeptGrant,
        Doc::TermsDoc,
        Doc::TermsRecord,
        Doc::Entry,
        Doc::Machine,
        Doc::Plan,
        Doc::HomeLabel,
        Doc::BackupIndex,
        Doc::MachineSealed,
    ];

    /// The kind tag bound into the envelope head.
    pub fn tag(self) -> &'static str {
        match self {
            Doc::Registry => "registry",
            Doc::KitLinks => "kit-links",
            Doc::Records => "records",
            Doc::Checked => "checked",
            Doc::Settings => "settings",
            Doc::Queue => "queue",
            Doc::FirstWindow => "first-window",
            Doc::LastAudit => "last-audit",
            Doc::Unfetched => "unfetched",
            Doc::Held => "held",
            Doc::Verdict => "verdict",
            Doc::KeptGrant => "kept-grant",
            Doc::TermsDoc => "terms-doc",
            Doc::TermsRecord => "terms-record",
            Doc::Entry => "entry",
            Doc::Machine => "machine",
            Doc::Plan => "plan",
            Doc::HomeLabel => "home-label",
            Doc::BackupIndex => "backup-index",
            Doc::MachineSealed => "machine-sealed",
        }
    }

    /// The format version of the sealed plaintext (every kind starts at 1).
    pub fn version(self) -> u16 {
        1
    }

    pub fn from_tag(t: &str) -> Option<Doc> {
        Doc::ALL.into_iter().find(|d| d.tag() == t)
    }

    /// Whether this kind lives in the machine directory (owned by the machine); all other kinds live in a home.
    pub fn in_machine(self) -> bool {
        matches!(self, Doc::Registry | Doc::KitLinks | Doc::Records | Doc::Checked | Doc::Machine | Doc::Plan | Doc::BackupIndex | Doc::MachineSealed)
    }

    /// Whether this kind's on-disk name is derived with the names key from its content (its place comes from
    /// the content, [`logical_rel`]); other kinds have fixed names.
    pub fn keyed(self) -> bool {
        matches!(self, Doc::Entry | Doc::Held | Doc::Verdict | Doc::KeptGrant | Doc::TermsDoc | Doc::TermsRecord)
    }

    /// For kinds whose plaintext is canonical JSON, the error for malformed content (returned by readers, and
    /// by [`seal_at`] before writing); `None` for kinds that are not canonical JSON (kept grant files, terms
    /// documents, plans).
    pub fn canonical_shape(self) -> Option<Code> {
        match self {
            Doc::Registry => Some(Code::IdentitiesShape),
            Doc::KitLinks | Doc::Records | Doc::Checked | Doc::Settings | Doc::FirstWindow | Doc::LastAudit | Doc::Unfetched | Doc::Verdict | Doc::TermsRecord | Doc::HomeLabel | Doc::BackupIndex => Some(Code::SettingsShape),
            Doc::Queue => Some(Code::QueueShape),
            Doc::Held | Doc::Entry => Some(Code::ContentShape),
            Doc::Machine | Doc::MachineSealed => Some(Code::MachineShape),
            Doc::KeptGrant | Doc::TermsDoc | Doc::Plan => None,
        }
    }
}

/// Why a sealed file could not be read, named in a `LOCAL_SEAL` error's details ([`Unread::of`] reads it
/// back). The error code, message and suggested fix are the same for all six.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unread {
    /// Not a sealed file (a plain file left by an older version, or anything else).
    NotSealed,
    /// The magic is present but the head is truncated.
    Truncated,
    /// Sealed as another kind.
    OtherKind,
    /// Sealed by a newer version (a format version or an algorithm this version does not know).
    NewerVersion,
    /// Another file: its identity (owner and place) is not the one expected.
    Swapped,
    /// Does not open: another key, or a byte altered.
    Unopenable,
}

impl Unread {
    pub const ALL: [Unread; 6] = [Unread::NotSealed, Unread::Truncated, Unread::OtherKind, Unread::NewerVersion, Unread::Swapped, Unread::Unopenable];

    /// The reason's word, the first word of a `LOCAL_SEAL` error's details.
    pub fn word(self) -> &'static str {
        match self {
            Unread::NotSealed => "not-sealed",
            Unread::Truncated => "truncated",
            Unread::OtherKind => "other-kind",
            Unread::NewerVersion => "newer-version",
            Unread::Swapped => "swapped",
            Unread::Unopenable => "unopenable",
        }
    }

    /// The reason in a `LOCAL_SEAL` refusal (`None` for any other), from either end's error.
    pub fn of<F: Coded>(f: &F) -> Option<Unread> {
        if f.sealed_code() != Some(Code::LocalSeal) {
            return None;
        }
        let first = f.sealed_tail().split(' ').next().unwrap_or_default();
        Unread::ALL.into_iter().find(|u| u.word() == first)
    }

    /// The `LOCAL_SEAL` refusal for this reason: the reason's word, then the kind and what was being read. Either
    /// end's error is made from it (`From<Trouble>`).
    pub fn fault(self, doc: Doc, what: &str) -> Trouble {
        Trouble::new(Code::LocalSeal, format!("{} · {} {what}", self.word(), doc.tag()))
    }
}

/// Who owns a sealed file: the machine, a home (by its number), or a home's label itself (opened before the
/// home's number is known).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
    Machine,
    Home(String),
    Label,
}

impl Owner {
    /// The owner as text (hashed into the digest, and repeated inside the ciphertext).
    pub fn text(&self) -> String {
        match self {
            Owner::Machine => "machine".to_string(),
            Owner::Home(n) => format!("home:{n}"),
            Owner::Label => "home-label".to_string(),
        }
    }

    pub fn parse(t: &str) -> Option<Owner> {
        match t {
            "machine" => Some(Owner::Machine),
            "home-label" => Some(Owner::Label),
            _ => t.strip_prefix("home:").filter(|n| is_home_number(n)).map(|n| Owner::Home(n.to_string())),
        }
    }

    /// The owner half of the identity digest.
    fn half(&self) -> [u8; DIGEST / 2] {
        half_of(OWNER_DOMAIN, &[self.text().as_bytes()])
    }
}

fn half_of(domain: &str, parts: &[&[u8]]) -> [u8; DIGEST / 2] {
    let mut msg = domain.as_bytes().to_vec();
    for p in parts {
        msg.push(0);
        msg.extend_from_slice(p);
    }
    let d = zikaron::cryptox::sha256(&msg);
    let mut out = [0u8; DIGEST / 2];
    out.copy_from_slice(&d[..DIGEST / 2]);
    out
}

/// A home number's shape: 32 lowercase hex digits.
pub fn is_home_number(n: &str) -> bool {
    n.len() == 32 && n.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A sealed file's identity: its owner and its place (kind and logical name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ident {
    pub owner: Owner,
    pub doc: Doc,
    pub logical: String,
}

impl Ident {
    /// The place half of the identity digest, from the logical name only (the kind is already bound as part of
    /// the head, which is the additional data): the first 8 bytes of `HMAC-SHA256(K, PLACE_DOMAIN ‖ 0 ‖
    /// logical)`, with `K` derived from the local data key `key` under [`PLACE_KEY_INFO`].
    pub fn place_half(logical: &str, key: &[u8; 32]) -> [u8; DIGEST / 2] {
        let mut k = [0u8; 32];
        crate::crypto::hkdf_sha256(key, PLACE_KEY_INFO.as_bytes(), &mut k);
        let mut msg = PLACE_DOMAIN.as_bytes().to_vec();
        msg.push(0);
        msg.extend_from_slice(logical.as_bytes());
        let mac = crate::crypto::hmac_sha256(&k, &msg);
        zikaron_os::wipe(&mut k);
        let mut out = [0u8; DIGEST / 2];
        out.copy_from_slice(&mac[..DIGEST / 2]);
        out
    }

    /// The identity digest ([`ALG_KEYED`]): 8 owner bytes, then 8 place bytes keyed under the local data key
    /// `key`.
    pub fn digest_keyed(&self, key: &[u8; 32]) -> [u8; DIGEST] {
        let mut d = [0u8; DIGEST];
        d[..DIGEST / 2].copy_from_slice(&self.owner.half());
        d[DIGEST / 2..].copy_from_slice(&Ident::place_half(&self.logical, key));
        d
    }
}

/// What a read expects: the owner (`None` when the home has no label, so no current-envelope file can belong
/// to it), the kind, and the file's path inside its home or the machine directory when known (`None` for an
/// entry from the store's listing, whose place comes from its content).
#[derive(Clone, Debug)]
pub struct Expect {
    pub owner: Option<Owner>,
    pub doc: Doc,
    pub rel: Option<String>,
}

/// A parsed head.
struct Head<'a> {
    tag: &'a [u8],
    version: u16,
    digest: [u8; DIGEST],
    nonce: [u8; NONCE],
    /// The additional data: the whole head.
    aad: &'a [u8],
    sealed: &'a [u8],
}

fn take<'a>(bytes: &'a [u8], i: &mut usize, n: usize) -> Result<&'a [u8], Unread> {
    let s = bytes.get(*i..*i + n).ok_or(Unread::Truncated)?;
    *i += n;
    Ok(s)
}

/// The head of the envelope this version writes. Bytes without its magic (a plain file, or the first envelope
/// an earlier version wrote) are not sealed; an algorithm below the one written (an earlier version's) is not
/// sealed either, one above it is a newer version's.
fn parse_head(bytes: &[u8]) -> Result<Head<'_>, Unread> {
    if !bytes.starts_with(MAGIC_V2) {
        return Err(Unread::NotSealed);
    }
    let mut i = MAGIC_V2.len();
    let alg = take(bytes, &mut i, 1)?[0];
    if alg < ALG_KEYED {
        return Err(Unread::NotSealed);
    }
    if alg > ALG_KEYED {
        return Err(Unread::NewerVersion);
    }
    let len = take(bytes, &mut i, 1)?[0] as usize;
    let tag = take(bytes, &mut i, len)?;
    let v = take(bytes, &mut i, 2)?;
    let version = u16::from_be_bytes([v[0], v[1]]);
    let mut digest = [0u8; DIGEST];
    digest.copy_from_slice(take(bytes, &mut i, DIGEST)?);
    let mut nonce = [0u8; NONCE];
    nonce.copy_from_slice(take(bytes, &mut i, NONCE)?);
    // An AEAD tag is 16 bytes: anything shorter is truncated.
    if bytes.len() < i + 16 {
        return Err(Unread::Truncated);
    }
    Ok(Head { tag, version, digest, nonce, aad: &bytes[..i], sealed: &bytes[i..] })
}

/// The current envelope's plaintext: the owner and the logical name, each prefixed by a 2-byte length, then
/// the file's bytes.
fn frame(id: &Ident, plain: &[u8]) -> Vec<u8> {
    let o = id.owner.text();
    let mut out = Vec::with_capacity(4 + o.len() + id.logical.len() + plain.len());
    for part in [o.as_bytes(), id.logical.as_bytes()] {
        out.extend_from_slice(&(part.len() as u16).to_be_bytes());
        out.extend_from_slice(part);
    }
    out.extend_from_slice(plain);
    out
}

fn unframe(b: &[u8]) -> Option<(String, String, &[u8])> {
    let mut i = 0usize;
    let mut text = || -> Option<String> {
        let n = u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as usize;
        let s = std::str::from_utf8(b.get(i + 2..i + 2 + n)?).ok()?.to_string();
        i += 2 + n;
        Some(s)
    };
    let owner = text()?;
    let logical = text()?;
    Some((owner, logical, &b[i..]))
}

/// Build the envelope under raw key bytes with a given nonce (test vectors reproduce it byte for byte). Writes
/// go through [`seal_with`], which draws a random nonce.
pub fn envelope(key: &[u8; 32], id: &Ident, nonce: &[u8; NONCE], plain: &[u8]) -> Result<Vec<u8>, Trouble> {
    let tag = id.doc.tag().as_bytes();
    let mut out = Vec::with_capacity(MAGIC_V2.len() + 4 + tag.len() + DIGEST + NONCE + plain.len() + 64);
    out.extend_from_slice(MAGIC_V2);
    out.push(ALG_KEYED);
    out.push(tag.len() as u8);
    out.extend_from_slice(tag);
    out.extend_from_slice(&id.doc.version().to_be_bytes());
    out.extend_from_slice(&id.digest_keyed(key));
    out.extend_from_slice(nonce);
    let mut body = frame(id, plain);
    let ct = crate::crypto::xchacha_seal(key, nonce, &out, &body);
    zikaron_os::wipe(&mut body);
    out.extend_from_slice(&ct.ok_or_else(|| Trouble::new(Code::LocalSeal, id.doc.tag().to_string()))?);
    Ok(out)
}

/// The head prefix for this kind up to its version: magic, algorithm ([`ALG_KEYED`]), kind, version. Opening
/// compares it in full; this is what binds a file to its kind.
fn head(doc: Doc) -> Vec<u8> {
    let tag = doc.tag().as_bytes();
    let mut h = MAGIC_V2.to_vec();
    h.push(ALG_KEYED);
    h.push(tag.len() as u8);
    h.extend_from_slice(tag);
    h.extend_from_slice(&doc.version().to_be_bytes());
    h
}

/// Open under raw key bytes, checking against `ex` (see the module header for the order of checks).
pub fn unseal(key: &[u8; 32], ex: &Expect, bytes: &[u8], what: &str, nk: Option<&NameKey>) -> Result<Vec<u8>, Trouble> {
    unseal_headed(key, ex, &head(ex.doc), bytes, what, nk)
}

/// [`unseal`] with the head prefix the file must start with (`h`, the expected kind's [`head`]). `nk`: the names
/// key the on-disk name of a keyed kind is checked under (`None`: no key in hand, so a named keyed file reads as
/// another file's place).
fn unseal_headed(key: &[u8; 32], ex: &Expect, h: &[u8], bytes: &[u8], what: &str, nk: Option<&NameKey>) -> Result<Vec<u8>, Trouble> {
    let no = |u: Unread| -> Trouble { u.fault(ex.doc, what) };
    let p = parse_head(bytes).map_err(no)?;
    // Head mismatch: another kind, a newer version, or something this format never wrote.
    if !bytes.starts_with(h) {
        return Err(no(if p.tag != ex.doc.tag().as_bytes() {
            Unread::OtherKind
        } else if p.version > ex.doc.version() {
            Unread::NewerVersion
        } else {
            Unread::Unopenable
        }));
    }
    let h = p;
    // Check the digest first, so a swapped file is reported before any key is tried.
    let Some(owner) = &ex.owner else { return Err(no(Unread::Swapped)) };
    if h.digest[..DIGEST / 2] != owner.half() {
        return Err(no(Unread::Swapped));
    }
    let fixed = (!ex.doc.keyed()).then_some(ex.rel.as_deref()).flatten();
    // Check the place half before decrypting. A keyed half also differs under a wrong key, so a mismatch is
    // resolved by trying to open: if it does not open, it is another key (or altered); if it opens, it is
    // another file.
    let place_differs = fixed.is_some_and(|rel| h.digest[DIGEST / 2..] != Ident::place_half(rel, key));
    let mut body = crate::crypto::xchacha_open(key, &h.nonce, h.aad, h.sealed).ok_or_else(|| no(Unread::Unopenable))?;
    if place_differs {
        zikaron_os::wipe(&mut body);
        return Err(no(Unread::Swapped));
    }
    let checked = (|| -> Result<Vec<u8>, Unread> {
        let (o, logical, plain) = unframe(&body).ok_or(Unread::Unopenable)?;
        let id = Ident { owner: Owner::parse(&o).ok_or(Unread::Swapped)?, doc: ex.doc, logical };
        if &id.owner != owner || id.digest_keyed(key) != h.digest {
            return Err(Unread::Swapped);
        }
        if ex.doc.keyed() {
            // A keyed kind's place comes from its content: the logical name inside must match the content, and
            // the on-disk name must be the logical name keyed under the names key (a name as an earlier version
            // wrote it, unkeyed, is another file's place).
            let from_content = logical_rel(ex.doc, ex.rel.as_deref().unwrap_or(&id.logical), plain).map_err(|_| Unread::Swapped)?;
            if from_content != id.logical {
                return Err(Unread::Swapped);
            }
            if let Some(rel) = &ex.rel {
                let keyed = nk.and_then(|nk| disk_rel(ex.doc, &id.logical, nk).ok());
                if keyed.as_deref() != Some(rel.as_str()) {
                    return Err(Unread::Swapped);
                }
            }
        }
        Ok(plain.to_vec())
    })();
    zikaron_os::wipe(&mut body);
    checked.map_err(no)
}

/// Whether sealed bytes open under raw key bytes, regardless of kind or identity (only tells which key a file
/// is under; the plaintext is wiped).
pub fn opens_under_key(key: &[u8; 32], bytes: &[u8]) -> bool {
    match parse_head(bytes) {
        Ok(h) => crate::crypto::xchacha_open(key, &h.nonce, h.aad, h.sealed).map(|mut b| zikaron_os::wipe(&mut b)).is_some(),
        Err(_) => false,
    }
}

/// A staged copy's plaintext under the change's new key (`None` when it does not open under that key or is
/// another kind). Its identity was set when this process staged it, so settling only checks the key.
pub fn open_staged(key: &LocalKey, doc: Doc, bytes: &[u8]) -> Option<Vec<u8>> {
    let h = parse_head(bytes).ok()?;
    if h.tag != doc.tag().as_bytes() {
        return None;
    }
    let mut body = crate::crypto::xchacha_open(key.bytes(), &h.nonce, h.aad, h.sealed)?;
    let out = unframe(&body).map(|(_, _, p)| p.to_vec());
    zikaron_os::wipe(&mut body);
    out
}

fn nonce() -> Result<[u8; NONCE], Trouble> {
    let mut n = [0u8; NONCE];
    crate::trouble::fill_random(&mut n)?;
    Ok(n)
}

/// Whether bytes carry the envelope this version writes (a plain file, or one an earlier version wrote, does not).
pub fn is_sealed(bytes: &[u8]) -> bool {
    zikaron_glue::sealed::is_sealed(bytes)
}

/// Seal under a given key (the vault's, or a new master key's while staging a change) with identity `id`.
pub fn seal_with(key: &LocalKey, id: &Ident, plain: &[u8]) -> Result<Vec<u8>, Trouble> {
    envelope(key.bytes(), id, &nonce()?, plain)
}

/// Open under a given key, checking against `ex` (`nk` as for [`unseal`]).
pub fn open_with(key: &LocalKey, ex: &Expect, bytes: &[u8], what: &str, nk: Option<&NameKey>) -> Result<Vec<u8>, Trouble> {
    let doc = ex.doc;
    let h = head(doc);
    unseal_headed(key.bytes(), ex, &h, bytes, what, nk)
}

/// Seal new local data with identity `id` under `key`. Every write of either end passes here, so canonical JSON
/// kinds are validated with the same parser that will read them back: bytes it would refuse fail with the kind's
/// shape code and are never written (`Doc::canonical_shape`).
pub fn seal_checked(key: &LocalKey, id: &Ident, plain: &[u8]) -> Result<Vec<u8>, Trouble> {
    let doc = id.doc;
    if let Some(shape) = doc.canonical_shape() {
        if let Err(t) = zikaron::json::parse(plain) {
            return Err(Trouble::new(shape, format!("{} {t:?}", doc.tag())));
        }
    }
    seal_with(key, id, plain)
}

// ───────────────────────── A home's label ─────────────────────────

/// A home's label: its number, and the identity and seat it belongs to (`None`: no one's). The seat by its word
/// ([`SEATS`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub number: String,
    pub whose: Option<(String, String)>,
}

impl Label {
    /// A new label with a fresh random number.
    pub fn new(whose: Option<(String, String)>) -> Result<Label, Trouble> {
        let mut n = [0u8; 16];
        crate::trouble::fill_random(&mut n)?;
        Ok(Label { number: zikaron::hexfmt::encode(&n).trim_start_matches("0x").to_ascii_lowercase(), whose })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        use zikaron::json::Value;
        let whose = match &self.whose {
            Some((id, seat)) => Value::Obj(vec![("identity".into(), Value::Str(id.clone())), ("seat".into(), Value::Str(seat.clone()))]),
            None => Value::Str("none".into()),
        };
        zikaron::json::canon_bytes(&Value::Obj(vec![("home".into(), Value::Str(self.number.clone())), ("owner".into(), whose)]))
    }

    pub fn parse(b: &[u8]) -> Option<Label> {
        use zikaron::json::Value;
        let v = zikaron::json::parse(b).ok()?;
        let number = v.member("home")?.as_str()?.to_string();
        if !is_home_number(&number) {
            return None;
        }
        let whose = match v.member("owner")? {
            Value::Str(s) if s == "none" => None,
            o @ Value::Obj(_) => {
                let id = o.member("identity")?.as_str()?.to_string();
                let seat = SEATS.iter().find(|r| Some(**r) == o.member("seat").and_then(|s| s.as_str()))?.to_string();
                Some((id, seat))
            }
            _ => return None,
        };
        Some(Label { number, whose })
    }
}

/// The path of a home's label file.
pub fn label_path(root: &Path) -> PathBuf {
    root.join(SETTINGS_DIR).join(LABEL_FILE)
}

/// The label's logical name inside its home.
pub fn label_rel() -> String {
    format!("{}/{LABEL_FILE}", SETTINGS_DIR)
}

/// The identity of every home label (the same for all, since it is read before the home's number is known).
pub fn label_ident() -> Ident {
    Ident { owner: Owner::Label, doc: Doc::HomeLabel, logical: label_rel() }
}

/// Read a home's label under `key` (`None` when it has none yet).
pub fn read_label(root: &Path, key: &LocalKey) -> Result<Option<Label>, Trouble> {
    let at = label_path(root);
    let bytes = match std::fs::read(&at) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Trouble::io(&e, &at.display().to_string())),
    };
    let ex = Expect { owner: Some(Owner::Label), doc: Doc::HomeLabel, rel: Some(label_rel()) };
    let plain = open_with(key, &ex, &bytes, &at.display().to_string(), None)?;
    let l = Label::parse(&plain).ok_or_else(|| Trouble::new(Code::SettingsShape, format!("{} {}", Doc::HomeLabel.tag(), at.display())))?;
    Ok(Some(l))
}

// ───────────────────────── Names on disk: logical and keyed ─────────────────────────

fn hex_id(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b).trim_start_matches("0x").to_ascii_lowercase()
}

/// A file's logical path, from its on-disk path and decrypted bytes. Kinds with fixed names keep theirs.
pub fn logical_rel(doc: Doc, rel: &str, plain: &[u8]) -> Result<String, Trouble> {
        let shape = || Trouble::new(Code::LocalSeal, format!("{} {rel}", doc.tag()));
    let text_of = |k: &str| -> Option<String> {
        match zikaron::json::parse(plain).ok()?.member(k) {
            Some(zikaron::json::Value::Str(x)) => Some(x.trim().trim_start_matches("0x").to_ascii_lowercase()),
            _ => None,
        }
    };
    Ok(match doc {
        Doc::Entry => format!("{}/{}{}", LEDGER_DIR, hex_id(&zikaron::entry::entry_id(plain)), zikaron_store::layout::ENTRY_SUFFIX),
        Doc::Held => format!("{}/{}{}", GRANTS_HELD_DIR, hex_id(&zikaron::entry::entry_id(plain)), zikaron_store::layout::ENTRY_SUFFIX),
        Doc::Verdict => {
            // The grant id is inside; a verdict without it is not one this version wrote.
            let id = text_of("grant").ok_or_else(shape)?;
            format!("{}/{id}{}", GRANTS_HELD_DIR, VERDICT_SUFFIX)
        }
        Doc::KeptGrant => {
            // The kept file's kit id, read by the glue's one reading of a grant file; a file it does not read has no
            // place of its own.
            let kit = zikaron_glue::grantfile::open(plain).map_err(|_| shape())?.kit_id;
            format!("{}/{}/{}.{}", GRANTS_HELD_DIR, KEPT_DIR, hex_id(&kit), zikaron_glue::container::EXT)
        }
        Doc::TermsDoc => {
            let d = hex_id(&zikaron_kit::doc::doc_id(plain));
            format!("{}/{}/{d}/{d}", KITS_DIR, zikaron_glue::grantfile::TERMS_ROOM)
        }
        Doc::TermsRecord => format!("{}/{}/grant-{}.json", KITS_DIR, zikaron_glue::grantfile::TERMS_ROOM, text_of("grant").ok_or_else(shape)?),
        _ => rel.to_string(),
    })
}

/// The on-disk path for a logical path under a names key.
pub fn disk_rel(doc: Doc, logical: &str, nk: &NameKey) -> Result<String, Trouble> {
        use crate::names::Logical as L;
    let parts: Vec<&str> = logical.split('/').collect();
    let bad = || Trouble::new(Code::LocalSeal, format!("{} {logical}", doc.tag()));
    let stem = |f: &str, suffix: &str| -> Result<String, Trouble> { f.strip_suffix(suffix).map(str::to_string).ok_or_else(bad) };
    // A kind that is not keyed keeps its logical name. A keyed kind (`Doc::keyed`) is renamed by its arm below;
    // one with no arm is refused by name, never left under a revealing name.
    if !doc.keyed() {
        return Ok(logical.to_string());
    }
    Ok(match (doc, parts.as_slice()) {
        (Doc::Entry, [_, f]) => format!("{}/{}{}", LEDGER_DIR, nk.name(L::Entry(&stem(f, zikaron_store::layout::ENTRY_SUFFIX)?)), zikaron_store::layout::ENTRY_SUFFIX),
        (Doc::Held, [_, f]) => format!("{}/{}{}", GRANTS_HELD_DIR, nk.name(L::Held(&stem(f, zikaron_store::layout::ENTRY_SUFFIX)?)), zikaron_store::layout::ENTRY_SUFFIX),
        (Doc::Verdict, [_, f]) => format!("{}/{}{}", GRANTS_HELD_DIR, nk.name(L::Held(&stem(f, VERDICT_SUFFIX)?)), VERDICT_SUFFIX),
        (Doc::KeptGrant, [_, _, f]) => {
            let ext = format!(".{}", zikaron_glue::container::EXT);
            format!("{}/{}/{}{ext}", GRANTS_HELD_DIR, KEPT_DIR, nk.name(L::Kept(&stem(f, &ext)?)))
        }
        (Doc::TermsDoc, [_, _, d, _]) => format!("{}/{}/{}/{}", KITS_DIR, zikaron_glue::grantfile::TERMS_ROOM, nk.name(L::TermsDir(d)), nk.name(L::TermsDoc(d))),
        (Doc::TermsRecord, [_, _, f]) => {
            let g = f.strip_prefix("grant-").and_then(|x| x.strip_suffix(".json")).ok_or_else(bad)?;
            format!("{}/{}/grant-{}.json", KITS_DIR, zikaron_glue::grantfile::TERMS_ROOM, nk.name(L::TermsRecord(g)))
        }
        _ => return Err(bad()),
    })
}

