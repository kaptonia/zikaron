//! Local data at rest: everything the app keeps on this machine that is not on chain is sealed under the
//! local data key, and read and written only through this module.
//!
//! The local data key D is derived from the vault's master key M (`keybox::local_key`: HKDF-SHA256 with info
//! `zikaron/local/v1`). Each file is sealed separately with XChaCha20-Poly1305 and a fresh random 24-byte
//! nonce. D exists only while the vault is unlocked: when locked, every read and write here fails with
//! `LOCKED` (the master key is wiped on lock, so D cannot be derived).
//!
//! The sealed file kinds are the closed set [`Doc`]. Left plain, and never read through here: files needed
//! before unlocking (the home pointer and chosen-home record, the vault file, whose contents are already
//! encrypted, lock files, the writer mark), exports meant for other people (record bundles, key files, grant
//! files, badges, mirrors, backups), and outside files read by views that need no unlock.
//!
//! ─── File identity ───
//!
//! A file's identity is its owner and its place. The owner of a machine-directory file is the machine; the
//! owner of a home file is the home's number, a random value drawn when the home is created and kept in the
//! home's sealed label ([`Doc::HomeLabel`], which also records which identity the home belongs to, if any).
//! The place is the file's kind and its logical name inside the home or machine directory ([`logical_rel`]),
//! never its path or on-disk name, so moving the data folder, renaming files under a new master key, or
//! restoring onto another machine keeps every identity valid without resealing.
//!
//! ─── Envelope ───
//!
//! [`MAGIC_V2`] · algorithm (1 byte, [`ALG`] or [`ALG_KEYED`]) · kind length (1 byte) · kind · format version
//! (2 bytes, big-endian) · identity digest (16 bytes: 8 from the owner, 8 from the place's logical name;
//! [`Ident::digest`]) · nonce (24 bytes) · ciphertext and tag. The whole head, nonce included, is the AEAD
//! additional data, so a changed head byte, or a file moved to another place or home, does not open. The
//! owner and place are repeated as text inside the ciphertext and checked after opening.
//!
//! Reading accepts both this envelope and the older [`MAGIC`] one (kind and version bound, no identity),
//! permanently. A file that cannot be read fails with `LOCAL_SEAL`, whose details name one of six reasons
//! ([`Unread`]): not sealed, truncated head, another kind, a newer version, another file (identity digest
//! mismatch), or does not open (another key, or altered). The digest is compared before decrypting, so a
//! swapped file and a wrong key are distinguishable.
//!
//! Writing produces only the new envelope. There is no bulk migration: a file moves to the new envelope when
//! it is next written, and ledger entries, which are never rewritten, stay as they are. A home without a label
//! (from an older version) gets one the first time a writer opens or writes into it. An existing file that
//! cannot be read is never overwritten ([`put`]): the error tells the user to move it aside so the next write
//! recreates it.
//!
//! Known limits: putting an older copy of a file back in its place is not detected (that would need a counter
//! stored elsewhere; the ledger has its signature chain). Swapping two whole homes, labels included, is not
//! detected by the envelope (opening a home checks its label's owner against the opening identity,
//! `check_label`). There is no downgrade: a file this version writes cannot be read by older versions (they
//! refuse it and leave it untouched).
//!
//! Writes seal first and land through `home::put_at` (temporary name, then atomic rename, mode 0600), so an
//! interruption never leaves a half-written file over a good one. Ledger entries are stored in the store
//! crate's ledger directory as sealed bytes (the store treats them as opaque); [`Ledger`] opens them on read.

use crate::fault::{classify, Fault, Known};
use crate::home::Home;
use crate::keybox::LocalKey;
use std::path::{Path, PathBuf};
use zikaron_store::{EntryName, LedgerDir, Pile, Skip, Stored, Survey};

/// Magic bytes of the old envelope (read only) and of the current one (written).
pub use zikaron_glue::sealed::{MAGIC, MAGIC_V2};
const NONCE: usize = 24;
/// Length of the identity digest in the envelope head.
pub const DIGEST: usize = 16;
/// Algorithm 1: XChaCha20-Poly1305 with an unkeyed place half in the identity digest (also the old envelope's
/// cipher). Read only; [`ALG_KEYED`] is written.
pub const ALG: u8 = 1;
/// Algorithm 2, the one written: XChaCha20-Poly1305 with the place half of the identity digest keyed (HMAC
/// under a key derived from the local data key, [`PLACE_KEY_INFO`]), so the heads on a locked disk cannot be
/// matched against public logical names (entry and grant ids). Future algorithms take new numbers under the
/// same magic.
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
    /// Machine directory: machine settings staged by a restore (the file itself stays plain, see
    /// `machine.rs`); only ever a staged file.
    Machine,
    /// Machine directory: a change's plan (what moves and where it was), staged with the change; only ever a
    /// staged file.
    Plan,
    /// Home `settings/`: the home's label (its number, and which identity it belongs to).
    HomeLabel,
    /// Machine directory: what the last whole-machine backup holds (`backup/index.json`, `backup`).
    BackupIndex,
}

impl Doc {
    pub const ALL: [Doc; 19] = [
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
        matches!(self, Doc::Registry | Doc::KitLinks | Doc::Records | Doc::Checked | Doc::Machine | Doc::Plan | Doc::BackupIndex)
    }

    /// Whether this kind's on-disk name is derived with the names key from its content (its place comes from
    /// the content, [`logical_rel`]); other kinds have fixed names.
    pub fn keyed(self) -> bool {
        matches!(self, Doc::Entry | Doc::Held | Doc::Verdict | Doc::KeptGrant | Doc::TermsDoc | Doc::TermsRecord)
    }

    /// For kinds whose plaintext is canonical JSON, the error for malformed content (returned by readers, and
    /// by [`seal_at`] before writing); `None` for kinds that are not canonical JSON (kept grant files, terms
    /// documents, plans).
    pub fn canonical_shape(self) -> Option<Known> {
        match self {
            Doc::Registry => Some(Known::IdentitiesShape),
            Doc::KitLinks | Doc::Records | Doc::Checked | Doc::Settings | Doc::FirstWindow | Doc::LastAudit | Doc::Unfetched | Doc::Verdict | Doc::TermsRecord | Doc::HomeLabel | Doc::BackupIndex => Some(Known::SettingsShape),
            Doc::Queue => Some(Known::QueueShape),
            Doc::Held | Doc::Entry => Some(Known::ContentShape),
            Doc::Machine => Some(Known::MachineShape),
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

    /// The reason in a `LOCAL_SEAL` error (`None` for any other error).
    pub fn of(f: &Fault) -> Option<Unread> {
        if f.which() != Some(Known::LocalSeal) {
            return None;
        }
        let first = f.tail().split(' ').next().unwrap_or_default();
        Unread::ALL.into_iter().find(|u| u.word() == first)
    }

    /// The `LOCAL_SEAL` error for this reason: the reason's word, then the kind and what was being read.
    pub fn fault(self, doc: Doc, what: &str) -> Fault {
        Fault::known(Known::LocalSeal, format!("{} · {} {what}", self.word(), doc.tag()))
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

    fn parse(t: &str) -> Option<Owner> {
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
fn is_home_number(n: &str) -> bool {
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
    /// the head, which is the additional data). Unkeyed under [`ALG`]; under [`ALG_KEYED`] the first 8 bytes of
    /// `HMAC-SHA256(K, PLACE_DOMAIN ‖ 0 ‖ logical)`, with `K` derived from the local data key `key` under
    /// [`PLACE_KEY_INFO`].
    fn place_half(logical: &str, alg: u8, key: &[u8; 32]) -> [u8; DIGEST / 2] {
        if alg == ALG {
            return half_of(PLACE_DOMAIN, &[logical.as_bytes()]);
        }
        let mut k = [0u8; 32];
        crate::cryptx::hkdf_sha256(key, PLACE_KEY_INFO.as_bytes(), &mut k);
        let mut msg = PLACE_DOMAIN.as_bytes().to_vec();
        msg.push(0);
        msg.extend_from_slice(logical.as_bytes());
        let mac = crate::cryptx::hmac_sha256(&k, &msg);
        zikaron_ui::secret::wipe(&mut k);
        let mut out = [0u8; DIGEST / 2];
        out.copy_from_slice(&mac[..DIGEST / 2]);
        out
    }

    /// The unkeyed identity digest ([`ALG`]): 8 owner bytes, then 8 place bytes. Read only;
    /// [`Ident::digest_keyed`] is written.
    pub fn digest(&self) -> [u8; DIGEST] {
        self.digest_as(ALG, &[0u8; 32])
    }

    /// The keyed identity digest ([`ALG_KEYED`]): 8 owner bytes, then 8 place bytes keyed under the local data
    /// key `key`.
    pub fn digest_keyed(&self, key: &[u8; 32]) -> [u8; DIGEST] {
        self.digest_as(ALG_KEYED, key)
    }

    fn digest_as(&self, alg: u8, key: &[u8; 32]) -> [u8; DIGEST] {
        let mut d = [0u8; DIGEST];
        d[..DIGEST / 2].copy_from_slice(&self.owner.half());
        d[DIGEST / 2..].copy_from_slice(&Ident::place_half(&self.logical, alg, key));
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
    v2: bool,
    tag: &'a [u8],
    version: u16,
    /// The algorithm number ([`ALG`] or [`ALG_KEYED`]; the old envelope reads as [`ALG`]).
    alg: u8,
    digest: [u8; DIGEST],
    nonce: [u8; NONCE],
    /// The additional data: the old envelope's head without its nonce, or the current envelope's whole head.
    aad: &'a [u8],
    sealed: &'a [u8],
}

fn take<'a>(bytes: &'a [u8], i: &mut usize, n: usize) -> Result<&'a [u8], Unread> {
    let s = bytes.get(*i..*i + n).ok_or(Unread::Truncated)?;
    *i += n;
    Ok(s)
}

fn parse_head(bytes: &[u8]) -> Result<Head<'_>, Unread> {
    let (v2, mut i) = if bytes.starts_with(MAGIC_V2) {
        (true, MAGIC_V2.len())
    } else if bytes.starts_with(MAGIC) {
        (false, MAGIC.len())
    } else {
        return Err(Unread::NotSealed);
    };
    let alg = if v2 { take(bytes, &mut i, 1)?[0] } else { ALG };
    if alg != ALG && alg != ALG_KEYED {
        return Err(Unread::NewerVersion);
    }
    let len = take(bytes, &mut i, 1)?[0] as usize;
    let tag = take(bytes, &mut i, len)?;
    let v = take(bytes, &mut i, 2)?;
    let version = u16::from_be_bytes([v[0], v[1]]);
    let mut digest = [0u8; DIGEST];
    if v2 {
        digest.copy_from_slice(take(bytes, &mut i, DIGEST)?);
    }
    let aad_end = i;
    let mut nonce = [0u8; NONCE];
    nonce.copy_from_slice(take(bytes, &mut i, NONCE)?);
    // An AEAD tag is 16 bytes: anything shorter is truncated.
    if bytes.len() < i + 16 {
        return Err(Unread::Truncated);
    }
    let aad = if v2 { &bytes[..i] } else { &bytes[..aad_end] };
    Ok(Head { v2, tag, version, alg, digest, nonce, aad, sealed: &bytes[i..] })
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
pub fn envelope(key: &[u8; 32], id: &Ident, nonce: &[u8; NONCE], plain: &[u8]) -> Result<Vec<u8>, Fault> {
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
    let ct = crate::cryptx::xchacha_seal(key, nonce, &out, &body);
    zikaron_ui::secret::wipe(&mut body);
    out.extend_from_slice(&ct.ok_or_else(|| Fault::known(Known::LocalSeal, id.doc.tag().to_string()))?);
    Ok(out)
}

/// The head prefix for this kind up to its version: magic, algorithm ([`ALG_KEYED`]), kind, version. Opening
/// compares it in full, using the algorithm the file names ([`ALG`] is accepted too); this is what binds a
/// file to its kind.
fn head(doc: Doc) -> Vec<u8> {
    head_alg(doc, ALG_KEYED)
}

fn head_alg(doc: Doc, alg: u8) -> Vec<u8> {
    let tag = doc.tag().as_bytes();
    let mut h = MAGIC_V2.to_vec();
    h.push(alg);
    h.push(tag.len() as u8);
    h.extend_from_slice(tag);
    h.extend_from_slice(&doc.version().to_be_bytes());
    h
}

/// The old envelope's head for this kind (read only).
fn head_v1(doc: Doc) -> Vec<u8> {
    let tag = doc.tag().as_bytes();
    let mut h = MAGIC.to_vec();
    h.push(tag.len() as u8);
    h.extend_from_slice(tag);
    h.extend_from_slice(&doc.version().to_be_bytes());
    h
}

/// Open under raw key bytes, checking against `ex` (see the module header for the order of checks). Accepts
/// both envelopes; the old one carries no identity and is checked for kind and version only.
pub fn unseal(key: &[u8; 32], ex: &Expect, bytes: &[u8], what: &str) -> Result<Vec<u8>, Fault> {
    unseal_headed(key, ex, &head(ex.doc), bytes, what)
}

/// [`unseal`] with the head prefix the file must start with (`h`, the expected kind's [`head`]).
fn unseal_headed(key: &[u8; 32], ex: &Expect, h: &[u8], bytes: &[u8], what: &str) -> Result<Vec<u8>, Fault> {
    let no = |u: Unread| u.fault(ex.doc, what);
    let p = parse_head(bytes).map_err(no)?;
    // Head mismatch: another kind, a newer version, or something this format never wrote.
    let expected = match (p.v2, p.alg == ALG_KEYED) {
        (true, true) => h.to_vec(),
        (true, false) => head_alg(ex.doc, p.alg),
        (false, _) => head_v1(ex.doc),
    };
    if !bytes.starts_with(&expected) {
        return Err(no(if p.tag != ex.doc.tag().as_bytes() {
            Unread::OtherKind
        } else if p.version > ex.doc.version() {
            Unread::NewerVersion
        } else {
            Unread::Unopenable
        }));
    }
    let h = p;
    if !h.v2 {
        return crate::cryptx::xchacha_open(key, &h.nonce, h.aad, h.sealed).ok_or_else(|| no(Unread::Unopenable));
    }
    // Check the digest first, so a swapped file is reported before any key is tried.
    let Some(owner) = &ex.owner else { return Err(no(Unread::Swapped)) };
    if h.digest[..DIGEST / 2] != owner.half() {
        return Err(no(Unread::Swapped));
    }
    let fixed = (!ex.doc.keyed()).then_some(ex.rel.as_deref()).flatten();
    // Check the place half before decrypting. A keyed half also differs under a wrong key, so a mismatch is
    // resolved by trying to open: if it does not open, it is another key (or altered); if it opens, it is
    // another file.
    let place_differs = fixed.is_some_and(|rel| h.digest[DIGEST / 2..] != Ident::place_half(rel, h.alg, key));
    // An unkeyed half that differs means another file, whatever the key.
    if place_differs && h.alg == ALG {
        return Err(no(Unread::Swapped));
    }
    let mut body = crate::cryptx::xchacha_open(key, &h.nonce, h.aad, h.sealed).ok_or_else(|| no(Unread::Unopenable))?;
    if place_differs {
        zikaron_ui::secret::wipe(&mut body);
        return Err(no(Unread::Swapped));
    }
    let checked = (|| -> Result<Vec<u8>, Unread> {
        let (o, logical, plain) = unframe(&body).ok_or(Unread::Unopenable)?;
        let id = Ident { owner: Owner::parse(&o).ok_or(Unread::Swapped)?, doc: ex.doc, logical };
        if &id.owner != owner || id.digest_as(h.alg, key) != h.digest {
            return Err(Unread::Swapped);
        }
        if ex.doc.keyed() {
            // A keyed kind's place comes from its content: the logical name inside must match the content, and
            // the on-disk name must match the logical name (keyed under the names key, or unkeyed as written
            // before names were keyed).
            let from_content = logical_rel(ex.doc, ex.rel.as_deref().unwrap_or(&id.logical), plain).map_err(|_| Unread::Swapped)?;
            if from_content != id.logical {
                return Err(Unread::Swapped);
            }
            if let Some(rel) = &ex.rel {
                let keyed = crate::names::key().ok().and_then(|nk| disk_rel(ex.doc, &id.logical, &nk).ok());
                if *rel != id.logical && keyed.as_deref() != Some(rel.as_str()) {
                    return Err(Unread::Swapped);
                }
            }
        }
        Ok(plain.to_vec())
    })();
    zikaron_ui::secret::wipe(&mut body);
    checked.map_err(no)
}

/// Whether sealed bytes open under raw key bytes, regardless of kind or identity (only tells which key a file
/// is under; the plaintext is wiped).
pub fn opens_under_key(key: &[u8; 32], bytes: &[u8]) -> bool {
    match parse_head(bytes) {
        Ok(h) => crate::cryptx::xchacha_open(key, &h.nonce, h.aad, h.sealed).map(|mut b| zikaron_ui::secret::wipe(&mut b)).is_some(),
        Err(_) => false,
    }
}

/// A staged copy's plaintext under the change's new key (`None` when it does not open under that key or is
/// another kind). Its identity was set when this process staged it, so settling only checks the key.
fn open_staged(key: &LocalKey, doc: Doc, bytes: &[u8]) -> Option<Vec<u8>> {
    let h = parse_head(bytes).ok()?;
    if h.tag != doc.tag().as_bytes() {
        return None;
    }
    let mut body = crate::cryptx::xchacha_open(key.bytes(), &h.nonce, h.aad, h.sealed)?;
    if !h.v2 {
        return Some(body);
    }
    let out = unframe(&body).map(|(_, _, p)| p.to_vec());
    zikaron_ui::secret::wipe(&mut body);
    out
}

fn nonce() -> Result<[u8; NONCE], Fault> {
    let mut n = [0u8; NONCE];
    crate::key::fill_random(&mut n)?;
    Ok(n)
}

/// Whether bytes carry a sealed envelope (a plain file left by an older version does not).
pub fn is_sealed(bytes: &[u8]) -> bool {
    zikaron_glue::sealed::is_sealed(bytes)
}

/// Seal under a given key (the vault's, or a new master key's while staging a change) with identity `id`.
pub fn seal_with(key: &LocalKey, id: &Ident, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    envelope(key.bytes(), id, &nonce()?, plain)
}

/// Open under a given key, checking against `ex`.
pub fn open_with(key: &LocalKey, ex: &Expect, bytes: &[u8], what: &str) -> Result<Vec<u8>, Fault> {
    let doc = ex.doc;
    let h = head(doc);
    unseal_headed(key.bytes(), ex, &h, bytes, what)
}

// ───────────────────────── Owners and places ─────────────────────────

/// A path without its staged suffix (a staged copy's place is the one it will take).
fn unstaged_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    PathBuf::from(unstaged(&s).unwrap_or(&s))
}

fn rel_under(path: &Path, root: &Path) -> Option<String> {
    let r = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// A file's root (the machine directory, or its home) and its path relative to that root. A home's root is
/// the nearest ancestor under which the relative path is a valid place for the kind (`home_doc`); a file with
/// no such ancestor is refused as `Swapped`.
pub fn place(path: &Path, doc: Doc) -> Result<(PathBuf, String), Fault> {
    let path = unstaged_path(path);
    if doc.in_machine() {
        let m = crate::home::machine_dir()?;
        let rel = rel_under(&path, &m).unwrap_or_else(|| path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        return Ok((m, rel));
    }
    for root in path.ancestors().skip(1).take(5) {
        if let Some(rel) = rel_under(&path, root) {
            if home_doc(&rel) == Some(doc) {
                return Ok((root.to_path_buf(), rel));
            }
        }
    }
    Err(Unread::Swapped.fault(doc, &path.display().to_string()))
}

/// Home labels read in this process, keyed by home, with the label file's bytes at read time. A label file
/// whose bytes changed since (a restore, a key change) is read again; comparing bytes rather than length and
/// mtime catches same-length rewrites on volumes with coarse timestamps. The file is small, so reading it is
/// cheap; the cache saves unsealing it.
static LABELS: std::sync::Mutex<Vec<(PathBuf, Vec<u8>, String)>> = std::sync::Mutex::new(Vec::new());

fn label_stamp(root: &Path) -> Option<Vec<u8>> {
    std::fs::read(label_path(root)).ok()
}

fn known_label(root: &Path) -> Option<String> {
    let at = std::fs::canonicalize(root).ok()?;
    let stamp = label_stamp(root)?;
    LABELS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(p, s, _)| *p == at && *s == stamp).map(|(_, _, n)| n.clone())
}

fn remember_label(root: &Path, n: &str) {
    if let (Ok(at), Some(stamp)) = (std::fs::canonicalize(root), label_stamp(root)) {
        let mut l = LABELS.lock().unwrap_or_else(|e| e.into_inner());
        l.retain(|(p, _, _)| *p != at);
        l.push((at, stamp, n.to_string()));
    }
}

/// Forget the labels read in this process (for tests that put another label in place).
pub fn forget_labels() {
    LABELS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// A home's label: its number, and the identity and role it belongs to (`None`: no one's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub number: String,
    pub whose: Option<(String, crate::roles::Role)>,
}

impl Label {
    /// A new label with a fresh random number.
    pub fn new(whose: Option<(String, crate::roles::Role)>) -> Result<Label, Fault> {
        let mut n = [0u8; 16];
        crate::key::fill_random(&mut n)?;
        Ok(Label { number: zikaron::hexfmt::encode(&n).trim_start_matches("0x").to_ascii_lowercase(), whose })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        use zikaron::json::Value;
        let whose = match &self.whose {
            Some((id, seat)) => Value::Obj(vec![("identity".into(), Value::Str(id.clone())), ("seat".into(), Value::Str(seat.as_str().into()))]),
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
                let seat = crate::roles::Role::ALL.into_iter().find(|r| Some(r.as_str()) == o.member("seat").and_then(|s| s.as_str()))?;
                Some((id, seat))
            }
            _ => return None,
        };
        Some(Label { number, whose })
    }
}

/// The path of a home's label file.
pub fn label_path(root: &Path) -> PathBuf {
    root.join(crate::home::Slot::Settings.as_str()).join(LABEL_FILE)
}

/// The label's logical name inside its home.
fn label_rel() -> String {
    format!("{}/{LABEL_FILE}", crate::home::Slot::Settings.as_str())
}

/// The identity of every home label (the same for all, since it is read before the home's number is known).
pub fn label_ident() -> Ident {
    Ident { owner: Owner::Label, doc: Doc::HomeLabel, logical: label_rel() }
}

/// Read a home's label (`None` for a home from an older version that has none yet).
pub fn read_label(root: &Path) -> Result<Option<Label>, Fault> {
    let at = label_path(root);
    let bytes = match std::fs::read(&at) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(classify(&e, &at.display().to_string())),
    };
    let key = crate::keybox::local_key()?;
    let ex = Expect { owner: Some(Owner::Label), doc: Doc::HomeLabel, rel: Some(label_rel()) };
    let plain = open_with(&key, &ex, &bytes, &at.display().to_string())?;
    let l = Label::parse(&plain).ok_or_else(|| Fault::known(Known::SettingsShape, format!("{} {}", Doc::HomeLabel.tag(), at.display())))?;
    remember_label(root, &l.number);
    Ok(Some(l))
}

/// The identity and role whose home this is according to the register, if any.
fn whose_by_register(root: &Path) -> Option<(String, crate::roles::Role)> {
    let reg = crate::register::read().ok().flatten()?;
    for row in &reg.rows {
        for seat in crate::roles::Role::ALL {
            if row.home(seat).map(|h| crate::home::same_place(&h, root)).unwrap_or(false) {
                return Some((row.id.clone(), seat));
            }
        }
    }
    None
}

/// Create and write a home's label (a fresh number, owner from the register). Only a writer does this, the
/// first time it opens or writes into a home without one.
fn make_label(root: &Path) -> Result<Label, Fault> {
    let l = Label::new(whose_by_register(root))?;
    put_sealed(&label_path(root), &label_ident(), &l.to_bytes())?;
    remember_label(root, &l.number);
    Ok(l)
}

/// A writer's label for a home: the existing one, or a new one. Creation is serialized within the process and
/// re-checked under the lock, so two concurrent first writers (the window and a background task) never give a
/// home two numbers.
pub fn ensure_label(root: &Path) -> Result<Label, Fault> {
    static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    if let Some(l) = read_label(root)? {
        return Ok(l);
    }
    let _one = MAKING.lock().unwrap_or_else(|e| e.into_inner());
    match read_label(root)? {
        Some(l) => Ok(l),
        None => make_label(root),
    }
}

/// On opening a home: read its label (a writer creates one if missing) and check its owner against the
/// identity and role it is opened as. A label for another identity or role is refused as `Swapped`, and
/// nothing is read from that home.
pub fn check_label(root: &Path, opened_as: Option<(&str, crate::roles::Role)>, writer: bool) -> Result<(), Fault> {
    let l = if writer { Some(ensure_label(root)?) } else { read_label(root)? };
    if let (Some(Label { whose: Some((id, seat)), .. }), Some((want_id, want_seat))) = (&l, opened_as) {
        let same = id.trim_start_matches("0x").eq_ignore_ascii_case(want_id.trim_start_matches("0x")) && *seat == want_seat;
        if !same {
            return Err(Unread::Swapped.fault(Doc::HomeLabel, &format!("{} · {id} {}", root.display(), seat.as_str())));
        }
    }
    Ok(())
}

/// A writer's home number: cached from this process, read from the label, or from a newly created label.
fn home_number(root: &Path) -> Result<String, Fault> {
    if let Some(n) = known_label(root) {
        return Ok(n);
    }
    Ok(ensure_label(root)?.number)
}

/// The owner expected for a home's files (`None` when the home has no label).
fn home_owner(root: &Path) -> Result<Option<Owner>, Fault> {
    if let Some(n) = known_label(root) {
        return Ok(Some(Owner::Home(n)));
    }
    Ok(read_label(root)?.map(|l| Owner::Home(l.number)))
}

/// What a read of the file at `path` expects.
pub fn expect_at(path: &Path, doc: Doc) -> Result<Expect, Fault> {
    if doc == Doc::HomeLabel {
        return Ok(Expect { owner: Some(Owner::Label), doc, rel: Some(label_rel()) });
    }
    let (root, rel) = place(path, doc)?;
    let owner = if doc.in_machine() { Some(Owner::Machine) } else { home_owner(&root)? };
    Ok(Expect { owner, doc, rel: Some(rel) })
}

/// What reading an entry from a ledger directory expects (its home is the directory's parent; its place comes
/// from its content).
pub fn expect_entry(ledger_dir: &Path) -> Result<Expect, Fault> {
    let root = ledger_dir.parent().unwrap_or(ledger_dir);
    Ok(Expect { owner: home_owner(root)?, doc: Doc::Entry, rel: None })
}

/// What a read of a found file expects (owner from its home's label).
pub fn expect_found(f: &Found) -> Result<Expect, Fault> {
    expect_at(&f.at, f.doc)
}

/// The identity for a write to `path`: its owner (creating the home's label if missing) and its place (from
/// the content for keyed kinds).
pub fn ident_at(path: &Path, doc: Doc, plain: &[u8]) -> Result<Ident, Fault> {
    if doc == Doc::HomeLabel {
        return Ok(label_ident());
    }
    let (root, rel) = place(path, doc)?;
    let owner = if doc.in_machine() { Owner::Machine } else { Owner::Home(home_number(&root)?) };
    Ok(Ident { owner, doc, logical: logical_rel(doc, &rel, plain)? })
}

/// The identity of a file in a home whose owner is already known (used when a change stages files under a new
/// key into homes it created or moved).
pub fn ident_in(owner: Owner, doc: Doc, rel: &str, plain: &[u8]) -> Result<Ident, Fault> {
    Ok(Ident { owner, doc, logical: logical_rel(doc, rel, plain)? })
}

// ───────────────────────── Reading and writing ─────────────────────────

/// Seal new local data with identity `id` under the vault's key (`LOCKED` while locked). Every write passes
/// here, so canonical JSON kinds are validated with the same parser that will read them back: bytes it would
/// refuse (an integer above the canonical limit, for example) fail with the kind's shape error and are never
/// written (`Doc::canonical_shape`).
pub fn seal(id: &Ident, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    let doc = id.doc;
    if let Some(shape) = doc.canonical_shape() {
        if let Err(t) = zikaron::json::parse(plain) {
            return Err(Fault::known(shape, format!("{} {t:?}", doc.tag())));
        }
    }
    seal_with(&crate::keybox::local_key()?, id, plain)
}

/// Seal data about to be written at `path`, with the identity [`ident_at`] gives.
pub fn seal_at(path: &Path, doc: Doc, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    seal(&ident_at(path, doc, plain)?, plain)
}

/// Read a sealed file: `None` when absent, `LOCKED` while locked, `LOCAL_SEAL` with its reason ([`Unread`])
/// when it cannot be read.
pub fn read(path: &Path, doc: Doc) -> Result<Option<Vec<u8>>, Fault> {
    // A missing file has nothing to protect, so "absent" is answered even while locked (writing still needs
    // the key).
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(classify(&e, &path.display().to_string())),
    };
    let key = crate::keybox::local_key()?;
    open_with(&key, &expect_at(path, doc)?, &bytes, &path.display().to_string()).map(Some)
}

/// The error for a write over an existing unreadable file: the reason, the file, and the fix (move it aside;
/// the next write recreates it). The file is not touched.
fn not_over(f: Fault, at: &Path) -> Fault {
    match Unread::of(&f) {
        Some(u) => Fault::known(Known::LocalSeal, format!("{} · {}", u.word(), crate::lang::filln(crate::lang::Key::TailUnreadKept, &[&at.display().to_string()]))),
        None => f,
    }
}

/// Refuse to overwrite an existing file at `path` that cannot be read; a missing or readable file passes.
/// Only `NotFound` counts as missing: a file the system will not hand over (permissions, another owner, I/O
/// failure) counts as unreadable and is refused too.
fn not_over_unread(path: &Path, doc: Doc) -> Result<(), Fault> {
    let have = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(not_over(crate::fault::classify(&e, &path.display().to_string()), path)),
    };
    let key = crate::keybox::local_key()?;
    let ex = expect_at(path, doc)?;
    open_with(&key, &ex, &have, &path.display().to_string()).map(|_| ()).map_err(|f| not_over(f, path))
}

/// Seal `plain` as `id` and write it at `path`, replacing an existing file only if it can be read; an
/// unreadable file is refused and left untouched.
fn put_sealed(path: &Path, id: &Ident, plain: &[u8]) -> Result<(), Fault> {
    // Validate the content before touching the disk.
    let sealed = seal(id, plain)?;
    not_over_unread(path, id.doc)?;
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(&dir, &name, &sealed)
}

/// Write a sealed file, replacing any existing one, through `home::put_at`. An existing file that cannot be
/// read is never overwritten (refused, untouched, with the fix in the message).
pub fn put(dir: &Path, name: &str, doc: Doc, plain: &[u8]) -> Result<(), Fault> {
    let at = dir.join(name);
    let sealed = seal(&ident_at(&at, doc, plain)?, plain)?;
    not_over_unread(&at, doc)?;
    crate::home::put_at(dir, name, &sealed)
}

/// Write a sealed write-once file (terms documents, issuance records, held grants, kept grant files); the glue
/// crate's `land_bytes` refuses if a file is already there.
pub fn land(path: &Path, doc: Doc, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal_at(path, doc, plain)?;
    zikaron_glue::landing::land_bytes(path, &sealed).map_err(|t| Fault::of_landing(t))
}

fn store(t: zikaron_store::Trouble) -> Fault {
    Fault::known(Known::Ledger, format!("{t:?}"))
}

/// The marker for empty entry files (zero bytes under an entry's name) in a strict read's error. The store
/// skips such files as interrupted writes; a strict read here refuses them, because a pile missing that entry
/// would look complete downstream and audit differently (a disk fault or sync tool can empty a file too).
pub const EMPTY: &str = "EMPTY";

/// A strict read's error naming the empty entry files (`EMPTY` and each on-disk name).
pub fn empty_fault(names: &[String]) -> Fault {
    Fault::known(Known::Ledger, format!("{EMPTY} {}", names.join(" ")))
}

/// Names of the empty entry files a lenient read skipped (`IN_FLIGHT` under an entry's name).
pub fn empty_in(skipped: &[Skip]) -> Vec<String> {
    skipped
        .iter()
        .filter(|k| k.why == zikaron_store::Why::InFlight && zikaron_store::layout::classify(&k.name) == zikaron_store::Kind::Entry)
        .map(|k| k.name.clone())
        .collect()
}

/// A home's ledger: the store crate's ledger directory holding sealed entries. All ledger access goes through
/// here, and the entry bytes returned are decrypted.
pub struct Ledger {
    dir: LedgerDir,
}

impl Ledger {
    /// Open a ledger directory (created when missing) whose entries are sealed.
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<Ledger, Fault> {
        Ok(Ledger { dir: LedgerDir::open_or_create(root).map_err(store)? })
    }

    /// Open an existing ledger directory (another role's home, read-only use).
    pub fn open(root: impl Into<PathBuf>) -> Result<Ledger, Fault> {
        Ok(Ledger { dir: LedgerDir::open(root).map_err(store)? })
    }

    pub fn root(&self) -> &Path {
        self.dir.root()
    }

    /// The underlying store directory (for layout reports and sweeps, which do not read entry bytes).
    pub fn store(&self) -> &LedgerDir {
        &self.dir
    }

    /// The home this ledger is in (the ledger directory's parent), which owns its entries.
    fn home_root(&self) -> PathBuf {
        self.root().parent().map(Path::to_path_buf).unwrap_or_else(|| self.root().to_path_buf())
    }

    /// What reading one of this ledger's entries expects (`file`: its on-disk name, when known).
    fn expect(&self, file: Option<&str>) -> Result<Expect, Fault> {
        Ok(Expect { owner: home_owner(&self.home_root())?, doc: Doc::Entry, rel: file.map(|f| format!("{}/{f}", crate::home::Slot::Ledger.as_str())) })
    }

    /// Strict read: the decrypted audit pile. Anything the store cannot account for, any empty entry file
    /// ([`EMPTY`]), or any entry that does not open fails the whole read.
    pub fn pile(&self) -> Result<Pile, Fault> {
        let key = crate::keybox::local_key()?;
        let ex = self.expect(None)?;
        let p = self.dir.pile().map_err(store)?;
        let empty = self.empty_entries()?;
        if !empty.is_empty() {
            return Err(empty_fault(&empty));
        }
        let mut items = Vec::with_capacity(p.items.len());
        for (i, b) in p.items.iter().enumerate() {
            items.push(open_with(&key, &ex, b, &format!("{} #{i}", self.root().display()))?);
        }
        Ok(Pile { items })
    }

    /// Zero-byte files under an entry's name in this ledger, sorted by name.
    fn empty_entries(&self) -> Result<Vec<String>, Fault> {
        let root = self.root();
        let listing = std::fs::read_dir(root).map_err(|e| classify(&e, &root.display().to_string()))?;
        let mut out = Vec::new();
        for item in listing {
            let item = item.map_err(|e| classify(&e, &root.display().to_string()))?;
            let name = item.file_name().to_string_lossy().into_owned();
            if zikaron_store::layout::classify(&name) != zikaron_store::Kind::Entry {
                continue;
            }
            // `symlink_metadata`: a link is treated as itself, as the store does.
            if std::fs::symlink_metadata(item.path()).is_ok_and(|m| m.is_file() && m.len() == 0) {
                out.push(name);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Lenient read: every entry that opens, plus each skipped file with its reason (entries that do not open
    /// are listed as unreadable).
    pub fn survey(&self) -> Result<Survey, Fault> {
        let key = crate::keybox::local_key()?;
        let ex = self.expect(None)?;
        let s = self.dir.survey().map_err(store)?;
        let mut out = Survey { items: Vec::with_capacity(s.items.len()), skipped: s.skipped };
        for (i, b) in s.items.iter().enumerate() {
            match open_with(&key, &ex, b, "") {
                Ok(p) => out.items.push(p),
                Err(_) => zikaron_store::ledger::note_skip(&mut out.skipped, &format!("sealed #{i}"), zikaron_store::Why::Unreadable),
            }
        }
        out.skipped.sort_by(|a: &Skip, b: &Skip| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Read and decrypt one entry by its logical file name (`<entry id>.entry`). The on-disk name is keyed
    /// (`names`); callers only deal in entry ids.
    pub fn read_named(&self, file: &str) -> Result<Vec<u8>, Fault> {
        let key = crate::keybox::local_key()?;
        let disk = disk_entry_file(file)?;
        let b = self.dir.read_named(&disk).map_err(store)?;
        open_with(&key, &self.expect(Some(&disk))?, &b, file)
    }

    /// Seal one entry for this ledger under the vault's key (for staging several before writing any).
    pub fn seal_entry(&self, plain: &[u8]) -> Result<Vec<u8>, Fault> {
        let owner = Owner::Home(home_number(&self.home_root())?);
        seal(&ident_in(owner, Doc::Entry, "", plain)?, plain)
    }

    /// Append one entry. The store refuses overwrites and sees only opaque bytes, and each seal uses a fresh
    /// nonce, so equality is checked here on the decrypted bytes: the same entry again is a no-op, a different
    /// one under the same name is refused with nothing changed. A task whose home changed since it started
    /// (`task::ticket_void`) writes nothing.
    pub fn append(&self, name: &EntryName, plain: &[u8]) -> Result<Stored, Fault> {
        if crate::task::ticket_void() {
            return Err(crate::task::void_ticket_fault(self.root()));
        }
        // `name` is the entry's id; the on-disk name is derived with the names key (`names`).
        let name = &disk_entry_name(name.as_str())?;
        let file = zikaron_store::layout::entry_file_name(name);
        match self.dir.read_named(&file) {
            Ok(have) => {
                let key = crate::keybox::local_key()?;
                let opened = open_with(&key, &self.expect(Some(&file))?, &have, &file)?;
                if zikaron_store::ledger::identical(&opened, plain) {
                    return Ok(Stored::AlreadyThere);
                }
                return Err(store(zikaron_store::Trouble::named(zikaron_store::Code::Conflict, &file)));
            }
            Err(t) if t.code == zikaron_store::Code::Absent => {}
            Err(t) => return Err(store(t)),
        }
        let sealed = self.seal_entry(plain)?;
        self.dir.append(name, &sealed).map_err(store)
    }
}

/// An entry's on-disk name from its id, under the vault's current names key.
fn disk_entry_name(id: &str) -> Result<EntryName, Fault> {
    let nk = crate::names::key()?;
    entry_name_under(&nk, id)
}

/// An entry's on-disk name under a given names key.
pub fn entry_name_under(nk: &crate::names::NameKey, id: &str) -> Result<EntryName, Fault> {
    EntryName::parse(&nk.name(crate::names::Logical::Entry(id))).ok_or_else(|| Fault::known(Known::Ledger, id.to_string()))
}

/// An entry's path in a ledger directory (name derived with the names key; needs the vault unlocked).
pub fn entry_path(ledger: &Path, id: &str) -> Result<PathBuf, Fault> {
    Ok(ledger.join(zikaron_store::layout::entry_file_name(&disk_entry_name(id)?)))
}

/// The on-disk file name for a logical entry file name (`<id>.entry`).
fn disk_entry_file(file: &str) -> Result<String, Fault> {
    let id = zikaron_store::layout::parse_entry_file(file).ok_or_else(|| Fault::known(Known::Ledger, file.to_string()))?;
    Ok(zikaron_store::layout::entry_file_name(&disk_entry_name(id.as_str())?))
}

/// A home's ledger (how `Home` hands it out).
pub fn ledger_of(home: &Home) -> Result<Ledger, Fault> {
    Ledger::open_or_create(home.dir(crate::home::Slot::Ledger))
}

// ───────────────────────── Where local data lies ─────────────────────────
//
// One walker lists every sealed file on this machine: the machine directory's sealed files and every home
// (each identity's role homes from the register, plus the machine's current home if it is not one of them).
// Plaintext migration, resealing under a new master key, settling staged files and the whole-machine backup
// all use the same list, so none can miss a file another sees.

/// Who a home belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Whose {
    /// The machine directory itself.
    Machine,
    /// An identity's home for one role.
    Seat { id: String, seat: crate::roles::Role },
    /// A home with no identity (the chosen or default home of a machine without a register row for it).
    Loose,
    /// A data folder this machine used before and no longer points at (`machine::Machine::homes`), numbered
    /// in the order remembered.
    Kept { n: usize },
    /// A home a deleted identity left on disk (`identity::Registry::left`).
    Left { id: String, seat: crate::roles::Role },
}

/// One sealed (or not yet sealed) local data file.
#[derive(Clone, Debug)]
pub struct Found {
    pub at: PathBuf,
    /// Its path inside its home or the machine directory, `/`-separated.
    pub rel: String,
    pub doc: Doc,
    pub whose: Whose,
}

/// The kind of a machine-directory file by its relative path (`None`: not sealed local data).
pub fn machine_doc(rel: &str) -> Option<Doc> {
    if rel == crate::places::registry_file() {
        return Some(Doc::Registry);
    }
    if rel == format!("{}/{}", crate::kitsindex::DIR, crate::kitsindex::OVERRIDES) {
        return Some(Doc::KitLinks);
    }
    if rel == format!("{}/{}", crate::recordsx::DIR, crate::recordsx::FILE) {
        return Some(Doc::Records);
    }
    if rel == format!("{}/{}", crate::checkedx::DIR, crate::checkedx::FILE) {
        return Some(Doc::Checked);
    }
    if rel == crate::backup::index_rel() {
        return Some(Doc::BackupIndex);
    }
    None
}

/// The kind of a home file by its relative path (`None`: not sealed local data, e.g. record bundles exported
/// into `kits/`, lock files, temporary files).
pub fn home_doc(rel: &str) -> Option<Doc> {
    use crate::home::Slot;
    let parts: Vec<&str> = rel.split('/').collect();
    let settings = Slot::Settings.as_str();
    match parts.as_slice() {
        [s, f] if *s == settings && *f == crate::settings::FILE => Some(Doc::Settings),
        [s, f] if *s == settings && *f == crate::queue::FILE => Some(Doc::Queue),
        [s, f] if *s == settings && *f == crate::wizard::FILE => Some(Doc::FirstWindow),
        [s, f] if *s == settings && *f == crate::lastread::ANCHORED_FILE => Some(Doc::LastAudit),
        [s, f] if *s == settings && *f == crate::restorex::FILE => Some(Doc::Unfetched),
        [s, f] if *s == settings && *f == LABEL_FILE => Some(Doc::HomeLabel),
        [l, f] if *l == Slot::Ledger.as_str() && zikaron_store::layout::parse_entry_file(f).is_some() => Some(Doc::Entry),
        [g, f] if *g == Slot::GrantsHeld.as_str() && crate::lastread::is_cache(f) => Some(Doc::Verdict),
        [g, rest @ ..] if *g == Slot::GrantsHeld.as_str() => crate::vaultx::held_doc(&rest.join("/")),
        [k, t, f] if *k == Slot::Kits.as_str() && *t == crate::termsx::ROOM && f.starts_with("grant-") && f.ends_with(".json") => Some(Doc::TermsRecord),
        [k, t, d, _] if *k == Slot::Kits.as_str() && *t == crate::termsx::ROOM && d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()) => Some(Doc::TermsDoc),
        _ => None,
    }
}

fn walk(root: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) {
    let at = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    let Ok(listing) = std::fs::read_dir(&at) else { return };
    for e in listing.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        match std::fs::symlink_metadata(e.path()) {
            Ok(m) if m.is_dir() => walk(root, &r, out),
            Ok(m) if m.is_file() => out.push((e.path(), r)),
            _ => {}
        }
    }
}

/// Like `walk`, but fails on a directory that cannot be listed (a change that reseals every file must not
/// skip one it cannot see).
fn walk_strict(root: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) -> Result<(), Fault> {
    let at = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    let listing = std::fs::read_dir(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
    for e in listing {
        let e = e.map_err(|x| classify(&x, &at.display().to_string()))?;
        let name = e.file_name().to_string_lossy().to_string();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        match std::fs::symlink_metadata(e.path()) {
            Ok(m) if m.is_dir() => walk_strict(root, &r, out)?,
            Ok(m) if m.is_file() => out.push((e.path(), r)),
            Ok(_) => {}
            Err(x) => return Err(classify(&x, &e.path().display().to_string())),
        }
    }
    Ok(())
}

/// Every home on this machine: each identity's role homes (from the register, which must be readable under
/// this vault's key), then the machine's current home if it is not one of them. Homes not on disk are left out
/// (a role never opened has none yet).
pub fn homes() -> Result<Vec<(Whose, PathBuf)>, Fault> {
    homes_of(crate::register::read()?, false)
}

/// This machine's homes found from plain records and the directory layout, without the sealed register: the
/// home the pointer names, the folders the machine settings remember, and next to each of them (and in the
/// machine directory) every role home laid out as `identity_home` does (`<place>/<identity>/<seat>`). A
/// whole-machine restore from the lock screen cannot read the register (it is locked under the key being
/// replaced), so this is how it finds every former home.
pub fn homes_on_record() -> Vec<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    if let Ok(h) = crate::home::where_is() {
        places.push(h);
    }
    if let Ok(m) = crate::machine::read() {
        places.extend(m.homes.iter().map(PathBuf::from));
    }
    let mut bases: Vec<PathBuf> = places.iter().filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
    if let Ok(m) = crate::home::machine_dir() {
        bases.push(m);
    }
    let is_id = |n: &str| n.len() == 40 && n.chars().all(|c| c.is_ascii_hexdigit());
    let mut out = places;
    for b in bases {
        let Ok(list) = std::fs::read_dir(&b) else { continue };
        for e in list.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !is_id(&name) || !e.path().is_dir() {
                continue;
            }
            for seat in crate::roles::Role::ALL {
                let h = e.path().join(seat.as_str());
                if h.is_dir() {
                    out.push(h);
                }
            }
        }
    }
    let mut uniq: Vec<PathBuf> = Vec::new();
    for p in out {
        if !uniq.iter().any(|x| crate::home::same_place(x, &p)) {
            uniq.push(p);
        }
    }
    uniq
}

/// The register as stored: sealed, or plain if an older version left it (used only by passes that run before
/// plain files are sealed: settling and migration).
fn registry_any() -> Result<Option<crate::identity::Registry>, Fault> {
    let at = crate::register::path()?;
    match std::fs::read(&at) {
        Ok(b) if !is_sealed(&b) => Ok(Some(
            crate::identity::Registry::parse(&b).map_err(|why| Fault::known(Known::IdentitiesShape, format!("{}: {why}", at.display())))?,
        )),
        _ => crate::register::read(),
    }
}

/// With `strict`, a registered home whose parent directory is also missing (a detached disk, a moved folder)
/// is an error instead of being skipped; a home whose parent exists but which was never created is still
/// skipped. Two roles sharing one home (older register rows) count it once.
fn homes_of(reg: Option<crate::identity::Registry>, strict: bool) -> Result<Vec<(Whose, PathBuf)>, Fault> {
    let mut out: Vec<(Whose, PathBuf)> = Vec::new();
    if let Some(reg) = &reg {
        for row in &reg.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    if out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
                        continue;
                    }
                    if h.is_dir() {
                        out.push((Whose::Seat { id: row.id.clone(), seat }, h));
                    } else if strict && !h.parent().map(Path::is_dir).unwrap_or(false) {
                        return Err(Fault::known(Known::HomeUnreachable, h.display().to_string()));
                    }
                }
            }
        }
        // Homes left by deleted identities: included when on disk, never an error.
        for (id, seat, home) in &reg.left {
            let h = PathBuf::from(home);
            if h.is_dir() && !out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
                out.push((Whose::Left { id: id.clone(), seat: *seat }, h));
            }
        }
    }
    if let Ok(h) = crate::home::where_is() {
        if h.is_dir() && !out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
            out.push((Whose::Loose, h));
        }
    }
    // Folders this machine used before: missing with the parent present means the user removed them; missing
    // with the parent gone too is an error when strict (as for role homes).
    let remembered = match crate::machine::read() {
        Ok(m) => m.homes,
        Err(f) if strict => return Err(f),
        Err(_) => Vec::new(),
    };
    for (n, h) in remembered.iter().map(PathBuf::from).enumerate() {
        if out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
            continue;
        }
        if h.is_dir() {
            out.push((Whose::Kept { n }, h));
        } else if strict && !h.parent().map(Path::is_dir).unwrap_or(false) {
            return Err(Fault::known(Known::HomeUnreachable, h.display().to_string()));
        }
    }
    Ok(out)
}

/// The machine directory's sealed files.
pub fn machine_files() -> Result<Vec<Found>, Fault> {
    let m = crate::home::machine_dir()?;
    let mut all = Vec::new();
    walk(&m, "", &mut all);
    let mut out: Vec<Found> = all
        .into_iter()
        .filter_map(|(at, rel)| machine_doc(&rel).map(|doc| Found { at, rel, doc, whose: Whose::Machine }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

/// One home's sealed files.
pub fn home_files(root: &Path, whose: &Whose) -> Vec<Found> {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    let mut out: Vec<Found> = all
        .into_iter()
        .filter_map(|(at, rel)| home_doc(&rel).map(|doc| Found { at, rel, doc, whose: whose.clone() }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

/// Every sealed file on this machine (machine directory first, then each home). Strict: an unreachable home or
/// an unlistable directory is an error, never skipped, because resealing under a new master key and the
/// whole-machine backup use this list, and a missed file would be lost with the old key.
pub fn all_files() -> Result<Vec<Found>, Fault> {
    let m = crate::home::machine_dir()?;
    let mut mine = Vec::new();
    walk_strict(&m, "", &mut mine)?;
    let mut out: Vec<Found> = mine
        .into_iter()
        .filter_map(|(at, rel)| machine_doc(&rel).map(|doc| Found { at, rel, doc, whose: Whose::Machine }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    for (w, h) in homes_of(crate::register::read()?, true)? {
        let mut all = Vec::new();
        walk_strict(&h, "", &mut all)?;
        let mut here: Vec<Found> = all
            .into_iter()
            .filter_map(|(at, rel)| home_doc(&rel).map(|doc| Found { at, rel, doc, whose: w.clone() }))
            .collect();
        here.sort_by(|a, b| a.rel.cmp(&b.rel));
        out.extend(here);
    }
    Ok(out)
}

// ───────────────────────── Names on disk: logical and keyed ─────────────────────────
//
// A file's logical path names it by content (`ledger/<entry id>.entry`, `grants-held/<grant id>.entry`,
// `kits/terms/<digest>/<digest>`...); backups carry it, and a change of names starts from it. The on-disk
// path replaces those ids with names derived from the names key (`names`). Only this section translates
// between the two.

fn hex_id(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b).trim_start_matches("0x").to_ascii_lowercase()
}

/// A file's logical path, from its on-disk path and decrypted bytes. Kinds with fixed names keep theirs.
pub fn logical_rel(doc: Doc, rel: &str, plain: &[u8]) -> Result<String, Fault> {
    use crate::home::Slot;
    let shape = || Fault::known(Known::LocalSeal, format!("{} {rel}", doc.tag()));
    let text_of = |k: &str| -> Option<String> {
        match zikaron::json::parse(plain).ok()?.member(k) {
            Some(zikaron::json::Value::Str(x)) => Some(x.trim().trim_start_matches("0x").to_ascii_lowercase()),
            _ => None,
        }
    };
    Ok(match doc {
        Doc::Entry => format!("{}/{}{}", Slot::Ledger.as_str(), hex_id(&zikaron::entry::entry_id(plain)), zikaron_store::layout::ENTRY_SUFFIX),
        Doc::Held => format!("{}/{}{}", Slot::GrantsHeld.as_str(), hex_id(&zikaron::entry::entry_id(plain)), zikaron_store::layout::ENTRY_SUFFIX),
        Doc::Verdict => {
            // The grant id is inside; caches from older versions were named by the grant id instead.
            let id = match text_of("grant") {
                Some(g) => g,
                None => rel.rsplit('/').next().and_then(|n| n.strip_suffix(crate::lastread::VERDICT_SUFFIX)).map(str::to_string).ok_or_else(shape)?,
            };
            format!("{}/{id}{}", Slot::GrantsHeld.as_str(), crate::lastread::VERDICT_SUFFIX)
        }
        Doc::KeptGrant => {
            let kit = crate::grantfilex::open_bytes(plain)?.kit_id;
            format!("{}/{}/{}.{}", Slot::GrantsHeld.as_str(), crate::grantfilex::KEPT, kit.trim().trim_start_matches("0x").to_ascii_lowercase(), zikaron_glue::container::EXT)
        }
        Doc::TermsDoc => {
            let d = hex_id(&zikaron_kit::doc::doc_id(plain));
            format!("{}/{}/{d}/{d}", Slot::Kits.as_str(), crate::termsx::ROOM)
        }
        Doc::TermsRecord => format!("{}/{}/grant-{}.json", Slot::Kits.as_str(), crate::termsx::ROOM, text_of("grant").ok_or_else(shape)?),
        _ => rel.to_string(),
    })
}

/// The on-disk path for a logical path under a names key.
pub fn disk_rel(doc: Doc, logical: &str, nk: &crate::names::NameKey) -> Result<String, Fault> {
    use crate::home::Slot;
    use crate::names::Logical as L;
    let parts: Vec<&str> = logical.split('/').collect();
    let bad = || Fault::known(Known::LocalSeal, format!("{} {logical}", doc.tag()));
    let stem = |f: &str, suffix: &str| -> Result<String, Fault> { f.strip_suffix(suffix).map(str::to_string).ok_or_else(bad) };
    // A kind that is not keyed keeps its logical name. A keyed kind (`Doc::keyed`) is renamed by its arm below;
    // one with no arm is refused by name, never left under a revealing name.
    if !doc.keyed() {
        return Ok(logical.to_string());
    }
    Ok(match (doc, parts.as_slice()) {
        (Doc::Entry, [_, f]) => format!("{}/{}{}", Slot::Ledger.as_str(), nk.name(L::Entry(&stem(f, zikaron_store::layout::ENTRY_SUFFIX)?)), zikaron_store::layout::ENTRY_SUFFIX),
        (Doc::Held, [_, f]) => format!("{}/{}{}", Slot::GrantsHeld.as_str(), nk.name(L::Held(&stem(f, zikaron_store::layout::ENTRY_SUFFIX)?)), zikaron_store::layout::ENTRY_SUFFIX),
        (Doc::Verdict, [_, f]) => format!("{}/{}{}", Slot::GrantsHeld.as_str(), nk.name(L::Held(&stem(f, crate::lastread::VERDICT_SUFFIX)?)), crate::lastread::VERDICT_SUFFIX),
        (Doc::KeptGrant, [_, _, f]) => {
            let ext = format!(".{}", zikaron_glue::container::EXT);
            format!("{}/{}/{}{ext}", Slot::GrantsHeld.as_str(), crate::grantfilex::KEPT, nk.name(L::Kept(&stem(f, &ext)?)))
        }
        (Doc::TermsDoc, [_, _, d, _]) => format!("{}/{}/{}/{}", Slot::Kits.as_str(), crate::termsx::ROOM, nk.name(L::TermsDir(d)), nk.name(L::TermsDoc(d))),
        (Doc::TermsRecord, [_, _, f]) => {
            let g = f.strip_prefix("grant-").and_then(|x| x.strip_suffix(".json")).ok_or_else(bad)?;
            format!("{}/{}/grant-{}.json", Slot::Kits.as_str(), crate::termsx::ROOM, nk.name(L::TermsRecord(g)))
        }
        _ => return Err(bad()),
    })
}

/// A role home's path under a names key: the same base, the identity's keyed directory name, the role. A home
/// not laid out as `<base>/<40 hex>/<seat>` keeps its path.
pub fn home_under(nk: &crate::names::NameKey, id: &str, seat: crate::roles::Role, at: &Path) -> PathBuf {
    let seat_ok = at.file_name().map(|n| n == seat.as_str()).unwrap_or(false);
    let named = at.parent().and_then(|p| p.file_name()).map(|n| crate::names::is_home_name(&n.to_string_lossy())).unwrap_or(false);
    match (seat_ok && named, at.parent().and_then(Path::parent)) {
        (true, Some(base)) => base.join(crate::home::home_dir_name(nk, id)).join(seat.as_str()),
        _ => at.to_path_buf(),
    }
}

/// What a names change staged: where each file will be, and what it leaves behind.
#[derive(Default, Debug)]
pub struct Restaged {
    /// Staged files (their paths once committed).
    pub to: Vec<PathBuf>,
    /// Paths removed once the change takes effect (files that moved to another name).
    pub gone: Vec<PathBuf>,
    /// Directories this change created (removed if it never takes effect).
    pub made: Vec<PathBuf>,
}

/// Stage every local file on this machine for a key or names change: opened under `old`, sealed under `new`,
/// and named under `nk` (role home directories renamed too, and the register's home paths rewritten), each
/// written beside its new path as `<file>.zk-next`. Nothing is replaced yet; the plan (what goes, what was
/// created) is staged too, sealed under `new`, for the commit pass (`settle_pending`). Machine settings are
/// not touched.
pub fn restage(old: &LocalKey, new: &LocalKey, nk: &crate::names::NameKey) -> Result<Restaged, Fault> {
    // Give every home a label first: files keep their owner across the change, and a home from an older
    // version gets its number now, under the current key, staged with the rest.
    let files = {
        for (_, h) in homes_of(crate::register::read()?, true)? {
            ensure_label(&h)?;
        }
        all_files()?
    };
    let reg = crate::register::read()?;
    // Each role home (and each home left by a deleted identity) with its new path.
    let mut homes: Vec<(PathBuf, PathBuf)> = Vec::new();
    if let Some(r) = &reg {
        for row in &r.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    homes.push((h.clone(), home_under(nk, &row.id, seat, &h)));
                }
            }
        }
        for (id, seat, h) in &r.left {
            let h = PathBuf::from(h);
            homes.push((h.clone(), home_under(nk, id, *seat, &h)));
        }
    }
    let moved_home = |p: &str| -> String {
        homes.iter().find(|(a, _)| crate::home::same_place(a, Path::new(p))).map(|(_, b)| b.display().to_string()).unwrap_or_else(|| p.to_string())
    };
    let mut out = Restaged::default();
    // Create each moving home's directory layout at its new path first, recording it in the plan before
    // creating it.
    for (a, b) in &homes {
        if a != b && a.is_dir() && !b.exists() {
            let top = identity_dir_of(b).filter(|d| !d.exists()).unwrap_or_else(|| b.clone());
            if !out.made.contains(&top) {
                out.made.push(top);
                stage_plan(PLAN_MADE, &out.made)?;
            }
            crate::home::lay(b)?;
        }
    }
    for f in files {
        let raw = std::fs::read(&f.at).map_err(|e| classify(&e, &f.at.display().to_string()))?;
        let ex = expect_found(&f)?;
        let mut plain = open_with(old, &ex, &raw, &f.rel)?;
        let owner = ex.owner.clone().ok_or_else(|| Unread::Swapped.fault(f.doc, &f.rel))?;
        // Keep the file's owner; its place comes from its content.
        let stage_next = |to: &Path, key: &LocalKey, doc: Doc, plain: &[u8]| ident_in(owner.clone(), doc, &f.rel, plain).and_then(|id| stage_next(to, key, &id, plain));
        if f.doc == Doc::Registry {
            // The register lists home paths: rewrite them to the new paths.
            let mut r = crate::identity::Registry::parse(&plain).map_err(|w| Fault::known(Known::IdentitiesShape, w))?;
            for row in r.rows.iter_mut() {
                row.author_home = if row.author_home.is_empty() { String::new() } else { moved_home(&row.author_home) };
                row.grantee_home = if row.grantee_home.is_empty() { String::new() } else { moved_home(&row.grantee_home) };
            }
            for (_, _, h) in r.left.iter_mut() {
                *h = moved_home(h);
            }
            plain = r.to_bytes();
        }
        if f.doc == Doc::Verdict {
            // A cache from an older version is named only by its grant: add the grant inside before the name is
            // keyed.
            let lr = logical_rel(Doc::Verdict, &f.rel, &plain)?;
            let id = lr.rsplit('/').next().and_then(|n| n.strip_suffix(crate::lastread::VERDICT_SUFFIX)).unwrap_or_default().to_string();
            plain = crate::lastread::with_grant(&plain, &id)?;
        }
        let root = root_of(&f);
        let new_root = homes.iter().find(|(a, _)| crate::home::same_place(a, &root)).map(|(_, b)| b.clone()).unwrap_or_else(|| root.clone());
        let to = new_root.join(disk_rel(f.doc, &logical_rel(f.doc, &f.rel, &plain)?, nk)?);
        if let Some(d) = to.parent() {
            if !d.exists() {
                // The topmost directory this change creates (removed whole if the change never takes effect).
                let mut top = d.to_path_buf();
                while let Some(up) = top.parent() {
                    if up.exists() {
                        break;
                    }
                    top = up.to_path_buf();
                }
                if !out.made.contains(&top) {
                    // Record it before creating it, so an interruption from here on removes it at the next
                    // unlock.
                    out.made.push(top);
                    stage_plan(PLAN_MADE, &out.made)?;
                }
                std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
            }
        }
        let put = stage_next(&to, new, f.doc, &plain);
        zikaron_ui::secret::wipe(&mut plain);
        put?;
        if to != f.at {
            out.gone.push(f.at.clone());
        }
        out.to.push(to);
    }
    stage_plan(PLAN_MADE, &out.made)?;
    stage_plan_sealed(PLAN_GONE, new, &out.gone.iter().map(|p| p.display().to_string()).collect::<Vec<_>>())?;
    let moved: Vec<String> = homes.iter().filter(|(a, b)| a != b && a.is_dir()).map(|(a, b)| format!("{}\t{}", a.display(), b.display())).collect();
    stage_plan_sealed(PLAN_MOVED, new, &moved)?;
    Ok(out)
}

/// The root (home or machine directory) of a found file: its path minus its relative path.
fn root_of(f: &Found) -> PathBuf {
    let mut root = f.at.clone();
    for _ in f.rel.split('/') {
        root.pop();
    }
    root
}

/// Whether any local file is not at its name under the vault's current names key: data from an older version
/// (names that reveal ids), or a names change interrupted halfway.
fn names_out_of_date(nk: &crate::names::NameKey) -> Result<bool, Fault> {
    let key = crate::keybox::local_key()?;
    let reg = crate::register::read()?;
    if let Some(r) = &reg {
        for row in &r.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    if home_under(nk, &row.id, seat, &h) != h {
                        return Ok(true);
                    }
                }
            }
        }
        for (id, seat, h) in &r.left {
            let h = PathBuf::from(h);
            if home_under(nk, id, *seat, &h) != h {
                return Ok(true);
            }
        }
    }
    for f in all_files()? {
        if f.doc.keyed() {
            let raw = std::fs::read(&f.at).map_err(|e| classify(&e, &f.at.display().to_string()))?;
            let mut plain = open_with(&key, &expect_found(&f)?, &raw, &f.rel)?;
            let want = disk_rel(f.doc, &logical_rel(f.doc, &f.rel, &plain)?, nk)?;
            let old_verdict = f.doc == Doc::Verdict && !plain.windows(7).any(|w| w == b"\"grant\"");
            zikaron_ui::secret::wipe(&mut plain);
            if want != f.rel || old_verdict {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// On the first unlock after upgrading (or after an interrupted names change): move every local file whose
/// name still reveals an identity, entry or grant id to its keyed name. Uses the same staged pass as a master
/// key change (old names are removed only once every new file is staged), so an interruption at any point is
/// resumed at the next unlock and no file is lost. Returns how many files moved.
pub fn migrate_names() -> Result<usize, Fault> {
    let nk = crate::names::key()?;
    // First bring in what older versions left outside the register (homes of deleted identities) and outside
    // the homes (set-aside trees under their old paths).
    if let Err(f) = flatten_old_aside() {
        note_trouble(f);
    }
    // Clear id-named strays (an empty role directory, files this key cannot open) first, so what remains is a
    // role home that `adopt_left_homes` can list and the renaming below can rename.
    match crate::keybox::local_key().and_then(|k| clear_strays(&k, true)) {
        Ok((0, _)) => {}
        Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
        Err(f) => note_trouble(f),
    }
    if let Err(f) = adopt_left_homes() {
        note_trouble(f);
    }
    if !names_out_of_date(&nk)? {
        return Ok(0);
    }
    let key = crate::keybox::local_key()?;
    let m = crate::home::machine_dir()?;
    stage_plan(PLAN_OPEN, &[])?;
    let staged = match restage(&key, &key, &nk) {
        Ok(s) => s,
        Err(f) => {
            // Never took effect: drop what was staged (now, or at the next unlock).
            let _ = settle_pending();
            return Err(f);
        }
    };
    cut_point(Cut::BeforeCommit)?;
    let n = staged.gone.len();
    std::fs::remove_file(plan_path(&m, PLAN_OPEN)).map_err(|e| classify(&e, PLAN_OPEN))?;
    cut_point(Cut::AfterCommit)?;
    let s = settle_pending()?;
    if s.held > 0 {
        return Err(Fault::known(Known::MigrateMismatch, format!("{} held", s.held)));
    }
    Ok(n)
}

// ───────────────────────── Migrating plain files ─────────────────────────

static MIGRATE_FAULT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Make one file's sealed copy read back different during migration (test hooks only, to show a mismatch
/// stops migration with every plain file kept).
pub fn set_migrate_fault(at: PathBuf) -> bool {
    MIGRATE_FAULT.set(at).is_ok()
}

/// The suffix of a sealed copy written beside a plain file during migration.
pub const SEALED_BESIDE: &str = ".zk-sealed";

/// What a migration pass did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Migrated {
    /// How many plain files were sealed.
    pub sealed: usize,
}

fn beside(at: &Path, suffix: &str) -> PathBuf {
    let mut s = at.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Seal a list of plain files, all or none. Each gets a sealed copy beside it, which is read back, opened and
/// compared byte for byte with the plain file; only when all match is each copy renamed over its plain file.
/// On the first failure every copy is removed, no plain file is deleted, and the error names the file
/// (`MIGRATE_MISMATCH`).
fn seal_plain(list: &[Found]) -> Result<usize, Fault> {
    let key = crate::keybox::local_key()?;
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    let undo = |done: &[(PathBuf, PathBuf)]| {
        for (_, copy) in done {
            let _ = std::fs::remove_file(copy);
        }
    };
    // Labels first, so each home's number is known before its other files are sealed.
    let mut list: Vec<&Found> = list.iter().collect();
    list.sort_by_key(|f| f.doc != Doc::HomeLabel);
    for f in list.iter().filter(|f| f.doc == Doc::HomeLabel) {
        if let Some(l) = std::fs::read(&f.at).ok().filter(|b| !is_sealed(b)).and_then(|b| Label::parse(&b)) {
            remember_label(&root_of(f), &l.number);
        }
    }
    for f in list {
        let plain = std::fs::read(&f.at).map_err(|e| classify(&e, &f.at.display().to_string()))?;
        if is_sealed(&plain) {
            continue;
        }
        let copy = beside(&f.at, SEALED_BESIDE);
        let step = (|| -> Result<(), Fault> {
            let sealed = seal_with(&key, &ident_at(&f.at, f.doc, &plain)?, &plain)?;
            // The envelope makes an entry longer; one that would exceed the ledger's size cap is refused here
            // rather than written and making the whole ledger unreadable.
            if f.doc == Doc::Entry && sealed.len() > zikaron_store::ENTRY_MAX {
                return Err(Fault::known(Known::Ledger, format!("{}: {} > {}", f.at.display(), sealed.len(), zikaron_store::ENTRY_MAX)));
            }
            let dir = copy.parent().map(Path::to_path_buf).unwrap_or_default();
            let name = copy.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            crate::home::put_at(&dir, &name, &sealed)?;
            let mut back = std::fs::read(&copy).map_err(|e| classify(&e, &copy.display().to_string()))?;
            // Injected read-back fault for test hooks (never set in the shipped app).
            if MIGRATE_FAULT.get().map(|p| p == &f.at).unwrap_or(false) {
                if let Some(b) = back.last_mut() {
                    *b ^= 1;
                }
            }
            // A copy that does not open is as much a mismatch as one that opens to other bytes.
            let opened = expect_at(&f.at, f.doc)
                .and_then(|ex| open_with(&key, &ex, &back, &f.rel))
                .map_err(|_| Fault::known(Known::MigrateMismatch, f.at.display().to_string()))?;
            if opened != plain {
                return Err(Fault::known(Known::MigrateMismatch, f.at.display().to_string()));
            }
            Ok(())
        })();
        if let Err(e) = step {
            let _ = std::fs::remove_file(&copy);
            undo(&done);
            // Keep the specific reason (too large, unreadable, mismatch) and name the file, rather than folding
            // everything into "did not match" and misleading the user about the cause.
            let named = match e.which() {
                Some(Known::MigrateMismatch) | None => e,
                Some(k) => Fault::known(k, format!("{} · {}", f.at.display(), e.tail())),
            };
            return Err(named);
        }
        done.push((f.at.clone(), copy));
    }
    for (at, copy) in &done {
        crate::home::rename_over(copy, at)?;
    }
    Ok(done.len())
}

/// On the first unlock after upgrading: seal every plain local file on this machine in one all-or-nothing pass
/// (the register may still be plain and is read as is to find the homes). Already sealed files are skipped,
/// so a second pass does nothing. Leftover sealed copies from an interrupted pass are removed first (their
/// plain files were never deleted).
pub fn migrate_plain() -> Result<Migrated, Fault> {
    let _ = crate::keybox::local_key()?;
    let m = crate::home::machine_dir()?;
    drop_leftovers(&m);
    let mut list = machine_files()?;
    for (w, h) in homes_of(registry_any()?, false)? {
        drop_leftovers(&h);
        list.extend(home_files(&h, &w));
    }
    Ok(Migrated { sealed: seal_plain(&list)? })
}

fn drop_leftovers(root: &Path) {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    for (at, rel) in all {
        if rel.ends_with(SEALED_BESIDE) {
            let _ = std::fs::remove_file(at);
        }
    }
}

// ───────────────────────── Files under a master key that is gone ─────────────────────────

/// The directory in the machine directory where files sealed under a lost master key are moved.
pub const SET_ASIDE: &str = "set-aside";

/// Before a new passcode creates a new master key on a machine whose key store is gone (reset, or file lost):
/// sealed files under the machine directory, the current home and previously used folders can never open
/// again, and one left in place would stop its home from opening. Each is moved unchanged into a `set-aside`
/// directory (numbered if taken) in the machine directory. Returns how many and the set-aside directory
/// (`None` when there were none).
pub fn set_aside_sealed() -> Result<(usize, Option<PathBuf>), Fault> {
    let mut roots: Vec<PathBuf> = vec![crate::home::machine_dir()?];
    if let Ok(h) = crate::home::where_is() {
        roots.push(h);
    }
    if let Ok(m) = crate::machine::read() {
        roots.extend(m.homes.iter().map(PathBuf::from));
    }
    set_aside_in(roots, None)
}

/// After a master key change has taken effect (a whole-machine restore): every sealed file under `roots` that
/// does not open under the new key was sealed under the replaced key and can never open again. Each is moved
/// aside the same way, never deleted (the roots are what a restore leaves behind: former homes, and files of
/// the kept home and machine directory that the backup did not have).
pub fn set_aside_unopenable(roots: Vec<PathBuf>, key: &LocalKey) -> Result<(usize, Option<PathBuf>), Fault> {
    set_aside_in(roots, Some(key))
}

/// Whether sealed bytes open under this key, regardless of kind.
fn opens_under(key: &LocalKey, bytes: &[u8]) -> bool {
    opens_under_key(key.bytes(), bytes)
}

fn set_aside_in(roots: Vec<PathBuf>, keep: Option<&LocalKey>) -> Result<(usize, Option<PathBuf>), Fault> {
    let m = crate::home::machine_dir()?;
    // Homes in use now (named by the register under the new key) are never removed, whatever they hold.
    let live: Vec<PathBuf> = match keep {
        Some(_) => homes_of(crate::register::read().ok().flatten(), false).map(|h| h.into_iter().map(|(_, p)| p).collect()).unwrap_or_default(),
        None => Vec::new(),
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut sealed: Vec<PathBuf> = Vec::new();
    let mut seats: Vec<PathBuf> = Vec::new();
    for root in roots {
        if !root.is_dir() || seen.iter().any(|x| crate::home::same_place(x, &root)) {
            continue;
        }
        seen.push(root.clone());
        let mut all = Vec::new();
        walk(&root, "", &mut all);
        let here: Vec<PathBuf> = all
            .into_iter()
            // Skip what an earlier pass set aside, and staged files (they belong to a change in progress).
            .filter(|(_, rel)| !rel.split('/').any(|c| c.starts_with(SET_ASIDE)) && !is_staged(&rel))
            .filter(|(at, _)| match std::fs::read(at) {
                Ok(b) => is_sealed(&b) && keep.map(|k| !opens_under(k, &b)).unwrap_or(true),
                Err(_) => false,
            })
            .map(|(at, _)| at)
            .collect();
        for at in here {
            if !sealed.iter().any(|x| x == &at) {
                sealed.push(at);
            }
        }
        if identity_dir_of(&root).is_some() && !live.iter().any(|h| crate::home::same_place(h, &root)) {
            seats.push(root);
        }
    }
    // One directory in the machine directory, files numbered flat: original names (which may reveal identity,
    // entry or grant ids) are not kept. Created only when needed.
    let mut to: Option<PathBuf> = None;
    let mut n = 0usize;
    let mut put = |at: &Path, to: &mut Option<PathBuf>| -> Result<(), Fault> {
        if to.is_none() {
            let mut p = m.join(SET_ASIDE);
            let mut i = 2u32;
            while p.exists() {
                p = m.join(format!("{SET_ASIDE}-{i}"));
                i += 1;
            }
            std::fs::create_dir_all(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
            *to = Some(p);
        }
        n += 1;
        let dest = to.as_ref().map(|p| p.join(format!("{n}"))).unwrap_or_default();
        move_across(at, &dest)
    };
    for at in &sealed {
        put(at, &mut to)?;
    }
    // A former role home is removed whole, with its identity directory: its remaining unsealed files (lock
    // file, exported record bundles) are set aside with the rest, numbered on, never deleted. A home that still
    // holds sealed files the current key opens is not a former home and stays.
    for root in &seats {
        let mut rest = Vec::new();
        walk(root, "", &mut rest);
        if rest.iter().any(|(at, _)| std::fs::read(at).map(|b| is_sealed(&b)).unwrap_or(false)) {
            continue;
        }
        for (at, rel) in rest {
            if !is_staged(&rel) {
                put(&at, &mut to)?;
            }
        }
        prune_empty_under(root);
        if std::fs::remove_dir(root).is_ok() {
            if let Some(d) = root.parent() {
                let _ = std::fs::remove_dir(d);
            }
        }
    }
    Ok((sealed.len(), to))
}

// ───────────────────────── Staged files of a master key change ─────────────────────────

/// The path of a plan file in the machine directory (`<name>` plus the staged suffix). Everything that
/// stages, reads or removes a plan, including test hooks, uses this.
pub fn plan_path(machine: &Path, name: &str) -> PathBuf {
    staged_path(&machine.join(name))
}

/// A file's staged copy path: beside it with the staged suffix (`keybox::NEXT`). This and [`staged_name`] are
/// the only places that add the suffix.
pub fn staged_path(at: &Path) -> PathBuf {
    beside(at, crate::keybox::NEXT)
}

/// The staged copy's file name (the name plus the staged suffix).
pub fn staged_name(name: &str) -> String {
    format!("{name}{}", crate::keybox::NEXT)
}

/// Whether a path (or a relative path) names a staged copy.
pub fn is_staged(p: &str) -> bool {
    p.ends_with(crate::keybox::NEXT)
}

/// A staged copy's path without its staged suffix (`None` when it is not one).
pub fn unstaged(p: &str) -> Option<&str> {
    p.strip_suffix(crate::keybox::NEXT)
}

/// Write a plan file (see [`plan_path`]).
fn put_plan(machine: &Path, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    let at = plan_path(machine, name);
    let file = at.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(machine, &file, bytes)
}

/// Staged content meaning "remove this file" (a whole-machine restore removes machine files the backup does
/// not have).
pub const GONE: &[u8] = b"zikaron-local/gone";

/// Points where a master key change can be stopped deliberately by a test hook, to show an interruption
/// leaves a consistent state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    /// Everything staged, vault not yet renamed: the change has not taken effect.
    BeforeCommit,
    /// Vault renamed, staged files not yet settled: the change took effect and the next unlock settles it.
    AfterCommit,
}

static CUT: std::sync::OnceLock<Cut> = std::sync::OnceLock::new();

/// Set a cut point once. Only test hooks call this (as with `places::set`), so the shipped app never stops
/// midway on purpose.
pub fn set_cut(c: Cut) -> bool {
    CUT.set(c).is_ok()
}

/// Stop here if a test hook set this cut point: return an error without cleaning up, leaving exactly what a
/// power loss at this point would.
pub fn cut_point(c: Cut) -> Result<(), Fault> {
    if CUT.get() == Some(&c) {
        return Err(Fault::known(Known::RestorePartial, format!("cut {c:?}")));
    }
    Ok(())
}

/// Whether a failure is a test hook's cut (`cut_point`). The process must not clean up after it, since a real
/// power loss would not get the chance.
pub fn is_cut(f: &Fault) -> bool {
    CUT.get().is_some() && f.which() == Some(Known::RestorePartial) && f.tail().starts_with("cut ")
}

/// Remove every staged file on this machine (a change that failed before taking effect).
pub fn drop_staged_everywhere() -> Result<(), Fault> {
    drop_staged_machine()?;
    for (_, h) in homes()? {
        let mut all = Vec::new();
        walk(&h, "", &mut all);
        for (at, rel) in all {
            if is_staged(&rel) {
                let _ = std::fs::remove_file(at);
            }
        }
    }
    Ok(())
}

/// Undo a change that failed before taking effect: remove the directories it created and every staged file.
pub fn drop_change() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    for d in read_plan(&m, PLAN_MADE) {
        if d.is_dir() {
            let _ = std::fs::remove_dir_all(&d);
        }
    }
    drop_staged_everywhere()
}

/// Remove the machine directory's staged files (a change that failed before taking effect).
pub fn drop_staged_machine() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    let mut all = Vec::new();
    walk(&m, "", &mut all);
    for (at, rel) in all {
        if is_staged(&rel) {
            let _ = std::fs::remove_file(at);
        }
    }
    Ok(())
}

/// Stage one file sealed under a new key: `<file>.zk-next` beside it (see `keybox::NEXT`).
pub fn stage_next(at: &Path, key: &LocalKey, id: &Ident, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal_with(key, id, plain)?;
    let next = staged_path(at);
    let dir = next.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = next.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(&dir, &name, &sealed)
}

/// A change's plan files, staged in the machine directory (plain text, one path per line) and removed with
/// the staged files: `PLAN_MADE` lists homes a restore created in new places (removed if the change never
/// takes effect); `PLAN_ASIDE` lists roots whose files the new key cannot open (set aside once it takes
/// effect). The same pass (`settle_pending`) carries out the plan, whether in the same run or at the next
/// unlock after an interruption.
pub const PLAN_MADE: &str = "change-made-homes";
pub const PLAN_ASIDE: &str = "change-set-aside";
/// For a names change: the paths removed once it takes effect (files now at their new names), and each home
/// that moved (`<old>\t<new>`; its other contents follow and the old directory is removed). Both are sealed
/// under the new key, since they contain on-disk names.
pub const PLAN_GONE: &str = "change-gone";
pub const PLAN_MOVED: &str = "change-moved-homes";
/// A names change under the same key (a migration) has no vault rename to commit it. This marker exists while
/// staging; removing it is the commit.
pub const PLAN_OPEN: &str = "change-open";

/// Stage one plan file (replacing any staged under that name).
pub fn stage_plan(name: &str, paths: &[PathBuf]) -> Result<(), Fault> {
    let lines: String = paths.iter().map(|p| format!("{}\n", p.display())).collect();
    put_plan(&crate::home::machine_dir()?, name, lines.as_bytes())
}

/// Stage one plan file sealed under `key` (the change's new key).
pub fn stage_plan_sealed(name: &str, key: &LocalKey, lines: &[String]) -> Result<(), Fault> {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let sealed = seal_with(key, &Ident { owner: Owner::Machine, doc: Doc::Plan, logical: name.to_string() }, text.as_bytes())?;
    put_plan(&crate::home::machine_dir()?, name, &sealed)
}

/// A sealed plan's lines (empty when missing or not openable under `key`, i.e. the change never took effect).
fn read_plan_sealed(machine: &Path, name: &str, key: &LocalKey) -> Vec<String> {
    let at = plan_path(&machine, name);
    let Ok(raw) = std::fs::read(&at) else { return Vec::new() };
    match open_with(key, &Expect { owner: Some(Owner::Machine), doc: Doc::Plan, rel: Some(name.to_string()) }, &raw, name) {
        Ok(mut plain) => {
            let out = String::from_utf8_lossy(&plain).lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
            zikaron_ui::secret::wipe(&mut plain);
            out
        }
        Err(_) => Vec::new(),
    }
}

fn plan_there(machine: &Path, name: &str) -> bool {
    plan_path(&machine, name).exists()
}

/// Remove empty directories upward from `from` (itself included), stopping below `stop` or at the first
/// directory that holds anything.
fn prune_up(from: &Path, stop: &Path) {
    let mut at = from.to_path_buf();
    while at.starts_with(stop) && at != stop {
        if std::fs::remove_dir(&at).is_err() {
            break;
        }
        match at.parent() {
            Some(p) => at = p.to_path_buf(),
            None => break,
        }
    }
}

/// Remove every empty directory under `root` (itself kept).
fn prune_empty_under(root: &Path) {
    let Ok(list) = std::fs::read_dir(root) else { return };
    for e in list.flatten() {
        let p = e.path();
        if std::fs::symlink_metadata(&p).map(|m| m.is_dir()).unwrap_or(false) {
            prune_empty_under(&p);
            let _ = std::fs::remove_dir(&p);
        }
    }
}

/// A role home's identity directory (`<base>/<40 hex>/<seat>` → `<base>/<40 hex>`), or `None` for a home laid
/// out otherwise.
fn identity_dir_of(home: &Path) -> Option<PathBuf> {
    let seat = home.file_name()?.to_string_lossy().to_string();
    let parent = home.parent()?;
    let named = crate::names::is_home_name(&parent.file_name()?.to_string_lossy());
    (named && crate::roles::Role::ALL.iter().any(|r| r.as_str() == seat)).then(|| parent.to_path_buf())
}

/// After a names change moved a home: move its remaining contents (non-sealed files such as exported record
/// bundles) to the new path unless already there, then remove the old directory, and its identity directory
/// if left empty. Returns how many items could not be moved or removed.
fn carry_home(old: &Path, new: &Path) -> usize {
    let mut failed = 0usize;
    let mut rest = Vec::new();
    walk(old, "", &mut rest);
    let mut clash: Vec<PathBuf> = Vec::new();
    for (at, rel) in rest {
        if is_staged(&rel) {
            continue;
        }
        let to = new.join(&rel);
        if to.exists() {
            // Already present at the new path: set this one aside rather than leave it under the old name.
            clash.push(at);
            continue;
        }
        let moved = to.parent().map(|d| std::fs::create_dir_all(d).is_ok()).unwrap_or(true) && move_across(&at, &to).is_ok();
        if !moved {
            failed += 1;
        }
    }
    if !clash.is_empty() && aside_flat(&clash).is_err() {
        failed += clash.len();
    }
    prune_empty_under(old);
    match std::fs::remove_dir(old) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => failed += 1,
    }
    if let Some(d) = identity_dir_of(old) {
        let _ = std::fs::remove_dir(&d);
    }
    failed
}

/// Move one file, possibly across volumes (e.g. from a home on another disk into the machine directory's
/// set-aside). Renames when possible; across volumes it copies, syncs, then removes the source, so an
/// interruption leaves one or both copies whole.
fn move_across(from: &Path, to: &Path) -> Result<(), Fault> {
    let n = MOVES.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    if let Some(MoveFault::FailOnce(k)) = MOVE_FAULT.get() {
        if *k == n {
            return Err(classify(&std::io::Error::other("injected"), &to.display().to_string()));
        }
    }
    let renamed = if matches!(MOVE_FAULT.get(), Some(MoveFault::CrossDevice)) {
        Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices))
    } else {
        std::fs::rename(from, to)
    };
    match renamed {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let say = |e: std::io::Error| classify(&e, &to.display().to_string());
            let mut src = std::fs::File::open(from).map_err(say)?;
            let mut dst = std::fs::OpenOptions::new().write(true).create_new(true).open(to).map_err(say)?;
            std::io::copy(&mut src, &mut dst).map_err(say)?;
            dst.sync_all().map_err(say)?;
            std::fs::remove_file(from).map_err(|e| classify(&e, &from.display().to_string()))
        }
        Err(e) => Err(classify(&e, &to.display().to_string())),
    }
}

/// A fault test hooks inject into file moves (as with `set_cut`): every move behaves as cross-volume, or the
/// n-th move in this process (counting from 1) fails once. Never set in the shipped app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveFault {
    CrossDevice,
    FailOnce(usize),
}

static MOVE_FAULT: std::sync::OnceLock<MoveFault> = std::sync::OnceLock::new();
static MOVES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Set the injected move fault once (test hooks only).
pub fn set_move_fault(f: MoveFault) -> bool {
    MOVE_FAULT.set(f).is_ok()
}

/// How many moves this process has made so far (tests pick which one fails).
pub fn moves_so_far() -> usize {
    MOVES.load(std::sync::atomic::Ordering::SeqCst)
}

/// Move these files into a new `set-aside` directory in the machine directory, numbered flat (kept, never
/// deleted, original names dropped). Returns the directory.
fn aside_flat(files: &[PathBuf]) -> Result<PathBuf, Fault> {
    let m = crate::home::machine_dir()?;
    let mut to = m.join(SET_ASIDE);
    let mut i = 2u32;
    while to.exists() {
        to = m.join(format!("{SET_ASIDE}-{i}"));
        i += 1;
    }
    std::fs::create_dir_all(&to).map_err(|e| classify(&e, &to.display().to_string()))?;
    for (n, at) in files.iter().enumerate() {
        if let Err(f) = move_across(at, &to.join(format!("{}", n + 1))) {
            // Remove the directory again if it is still empty.
            let _ = std::fs::remove_dir(&to);
            return Err(f);
        }
    }
    Ok(to)
}

/// Where whole homes are set aside (in the machine directory, numbered; names reveal nothing about owners).
pub const ASIDE_HOMES: &str = "aside-homes";

/// Plan file for swapping a home for a fresh one (`<home>\t<old data path>`), present in the machine directory
/// while the swap runs (see `swap_aside`).
pub const PLAN_SWAP: &str = "change-swap-home";

/// Where the fresh replacement for a home is prepared: beside it, with the staged suffix.
pub fn staged_home(root: &Path) -> PathBuf {
    staged_path(root)
}

/// Prepare a fresh replacement for the home at `root` (see `action::fetch_aside`), staged beside it. Everything
/// except the ledger is copied as is (settings, held grants, record bundles; same key), and the read-only mark
/// is set, so writing is enabled only after the fetched ledger's tail is checked. The home at `root` is not
/// touched.
pub fn stage_fresh_home(root: &Path) -> Result<crate::home::Home, Fault> {
    let staged = staged_home(root);
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(|e| classify(&e, &staged.display().to_string()))?;
    }
    let home = crate::home::Home::open_or_create(&staged)?;
    // Same home, same path: keep its label (its number) so the copied files remain this home's. A home from an
    // older version has no label and gets one when written.
    if let Ok(bytes) = std::fs::read(label_path(root)) {
        crate::home::put_at(&home.dir(crate::home::Slot::Settings), LABEL_FILE, &bytes)?;
    }
    carry_settings(root, &home)?;
    for slot in [crate::home::Slot::GrantsHeld, crate::home::Slot::Kits] {
        copy_tree(&root.join(slot.as_str()), &home.dir(slot))?;
    }
    crate::restorex::write(&home, crate::restorex::State::Unfetched)?;
    Ok(home)
}

/// Copy every file under `from` to the same relative path under `to`, synced to disk. Owner-only files are
/// copied into files created owner-only (`zikaron_os::owner_only`), since a plain copy on some systems does
/// not carry over access lists and would leave the copy readable by others.
fn copy_tree(from: &Path, to: &Path) -> Result<(), Fault> {
    let mut files = Vec::new();
    walk(from, "", &mut files);
    for (at, rel) in files {
        let dest = to.join(&rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
        }
        let say = |e: std::io::Error| classify(&e, &dest.display().to_string());
        if zikaron_os::is_owner_only(&at).map_err(|e| classify(&e, &at.display().to_string()))? {
            let mut o = zikaron_os::Options::new();
            o.write(true).create(true).truncate(true);
            zikaron_os::owner_only(&mut o);
            let mut out = o.open(&dest).map_err(say)?;
            let mut src = std::fs::File::open(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
            std::io::copy(&mut src, &mut out).map_err(say)?;
        } else {
            std::fs::copy(&at, &dest).map_err(say)?;
        }
        zikaron_os::sync_file(&dest).map_err(say)?;
    }
    Ok(())
}

/// Swap the home at `root` for the fresh one staged beside it. The old home is moved whole to the machine
/// directory's `aside-homes/<n>` (numbered, not named by owner) and listed among the machine's data folders
/// (so it is resealed and renamed with every home, included in backups, and readable); the staged home takes
/// its place. The plan is written first and removed last; an interruption is finished at the next unlock
/// (`settle_swap`): forward if the old data is whole at its new path, back otherwise. Returns where the old
/// data went.
pub fn swap_aside(root: &Path) -> Result<PathBuf, Fault> {
    let m = crate::home::machine_dir()?;
    let room = m.join(ASIDE_HOMES);
    std::fs::create_dir_all(&room).map_err(|e| classify(&e, &room.display().to_string()))?;
    let mut n = 1u32;
    while room.join(format!("{n}")).exists() || room.join(format!("{n}{PARTIAL}")).exists() {
        n += 1;
    }
    let to = room.join(format!("{n}"));
    put_plan(&m, PLAN_SWAP, format!("{}\t{}\n", root.display(), to.display()).as_bytes())?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    crate::machine::update(|machine| {
        machine.homes.push(to.display().to_string());
        machine.aside_at.push((to.display().to_string(), now));
    })?;
    cut_point(Cut::BeforeCommit)?;
    move_dir(root, &to)?;
    cut_point(Cut::AfterCommit)?;
    crate::home::rename_over(&staged_home(root), root)?;
    let _ = std::fs::remove_file(plan_path(&m, PLAN_SWAP));
    Ok(to)
}

/// Suffix for a directory being copied across volumes; it is renamed only once complete.
const PARTIAL: &str = ".partial";

/// Move a whole directory: rename when possible; across volumes, copy into `<to>.partial`, rename to `to`,
/// then remove the original (an interruption leaves the original whole, or both whole).
fn move_dir(from: &Path, to: &Path) -> Result<(), Fault> {
    let injected = matches!(MOVE_FAULT.get(), Some(MoveFault::CrossDevice));
    match if injected { Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices)) } else { std::fs::rename(from, to) } {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let partial = beside(to, PARTIAL);
            if partial.exists() {
                std::fs::remove_dir_all(&partial).map_err(|e| classify(&e, &partial.display().to_string()))?;
            }
            copy_tree(from, &partial)?;
            crate::home::rename_over(&partial, to)?;
            std::fs::remove_dir_all(from).map_err(|e| classify(&e, &from.display().to_string()))
        }
        Err(e) => Err(classify(&e, &to.display().to_string())),
    }
}

/// Finish an interrupted swap (on unlock, before any home is read): if the old data is whole at its new path,
/// move the fresh home into place (forward); otherwise remove the staged home and any partial copy and unlist
/// the old data (back: the home stays as it was). Staged fresh homes no plan refers to are removed too.
pub fn settle_swap() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    let plan = plan_path(&m, PLAN_SWAP);
    if let Ok(text) = std::fs::read_to_string(&plan) {
        if let Some((root, to)) = text.trim().split_once('\t') {
            let (root, to) = (PathBuf::from(root), PathBuf::from(to));
            let staged = staged_home(&root);
            if staged.is_dir() {
                if to.is_dir() {
                    if root.exists() {
                        std::fs::remove_dir_all(&root).map_err(|e| classify(&e, &root.display().to_string()))?;
                    }
                    crate::home::rename_over(&staged, &root)?;
                } else {
                    let _ = std::fs::remove_dir_all(beside(&to, PARTIAL));
                    let _ = std::fs::remove_dir_all(&staged);
                    crate::machine::update(|machine| {
                        machine.homes.retain(|h| Path::new(h) != to);
                        machine.aside_at.retain(|(h, _)| Path::new(h) != to);
                    })?;
                }
            }
        }
        let _ = std::fs::remove_file(&plan);
    }
    // Remove fresh homes staged by a pass that stopped before writing its plan; they were never swapped in.
    // (A register that cannot be read now, still sealed under a key being replaced, is retried next unlock.)
    if let Ok(Some(reg)) = registry_any() {
        for row in &reg.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    let staged = staged_home(&h);
                    if staged.is_dir() && h.is_dir() {
                        let _ = std::fs::remove_dir_all(&staged);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Copy the settings (nodes, basis, publish address) from a home being set aside into its fresh replacement.
/// The queue is not copied: it belongs to the entries set aside and stays with them.
pub fn carry_settings(from_root: &Path, into: &crate::home::Home) -> Result<(), Fault> {
    let from = from_root.join(crate::home::Slot::Settings.as_str()).join(crate::settings::FILE);
    match read(&from, Doc::Settings)? {
        Some(plain) => put(&into.dir(crate::home::Slot::Settings), crate::settings::FILE, Doc::Settings, &plain),
        None => Ok(()),
    }
}

/// Whether a set-aside directory is already flat: no subdirectories, and every file named by a number (dot
/// files left by the system's file browser are ignored).
fn aside_is_flat(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|d| {
            d.flatten().all(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                !e.path().is_dir() && (name.starts_with('.') || name.bytes().all(|b| b.is_ascii_digit()))
            })
        })
        .unwrap_or(true)
}

/// Older versions set files aside inside each root under their original paths (which may contain entry or
/// grant ids). Flatten every such tree, in the machine directory and every home, into the machine directory's
/// `set-aside` (numbered) and remove it. Idempotent; an interruption leaves each file in one place or the
/// other.
fn flatten_old_aside() -> Result<usize, Fault> {
    let m = crate::home::machine_dir()?;
    let mut roots = vec![m.clone()];
    roots.extend(homes_of(crate::register::read()?, false)?.into_iter().map(|(_, h)| h));
    let mut n = 0usize;
    for root in roots {
        let Ok(list) = std::fs::read_dir(&root) else { continue };
        for e in list.flatten() {
            let p = e.path();
            if !e.file_name().to_string_lossy().starts_with(SET_ASIDE) || !p.is_dir() {
                continue;
            }
            if root == m && aside_is_flat(&p) {
                continue;
            }
            let mut files = Vec::new();
            walk(&p, "", &mut files);
            let files: Vec<PathBuf> = files.into_iter().map(|(at, _)| at).collect();
            if !files.is_empty() {
                aside_flat(&files)?;
                n += files.len();
            }
            prune_empty_under(&p);
            let _ = std::fs::remove_dir(&p);
        }
    }
    Ok(n)
}

/// Directories that contain homes: the machine directory, the parents of the current home and of previously
/// used folders, and the parent of every home the register names.
fn home_places() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Ok(m) = crate::home::machine_dir() {
        out.push(m);
    }
    if let Some(p) = crate::home::where_is().ok().and_then(|h| h.parent().map(Path::to_path_buf)) {
        out.push(p);
    }
    if let Ok(m) = crate::machine::read() {
        out.extend(m.homes.iter().filter_map(|h| Path::new(h).parent().map(Path::to_path_buf)));
    }
    for d in known_identity_dirs() {
        if let Some(p) = d.parent() {
            out.push(p.to_path_buf());
        }
    }
    let mut uniq: Vec<PathBuf> = Vec::new();
    for p in out {
        if !uniq.iter().any(|x| crate::home::same_place(x, &p)) {
            uniq.push(p);
        }
    }
    uniq
}

/// The identity directory of every home the register names (role homes, on disk or not, and homes left by
/// deleted identities), reading the register as stored.
fn known_identity_dirs() -> Vec<PathBuf> {
    let Ok(Some(reg)) = registry_any() else { return Vec::new() };
    let mut homes: Vec<PathBuf> = reg.rows.iter().flat_map(|r| crate::roles::Role::ALL.iter().filter_map(|s| r.home(*s)).collect::<Vec<_>>()).collect();
    homes.extend(reg.left.iter().map(|(_, _, h)| PathBuf::from(h)));
    homes.iter().filter_map(|h| identity_dir_of(h)).collect()
}

/// Ensure nothing remains named by an id. In every directory that contains homes, a directory laid out like an
/// identity's homes (an id-named directory with role subdirectories) that the register does not know is a
/// stray, whatever its origin (an older version, an interruption, a manual copy). Its files that are plain or
/// do not open under the vault's key are set aside unchanged, numbered flat in the machine directory, and its
/// empty directories are removed. With `adopt`, a role directory whose local files open under the key is kept
/// for `adopt_left_homes` to list and the names change to rename; without it (after a change took effect,
/// when every home to keep is already registered) everything is set aside. Staged files of a change in
/// progress are left alone. Returns how many files were set aside, and where.
pub fn clear_strays(key: &LocalKey, adopt: bool) -> Result<(usize, Option<PathBuf>), Fault> {
    let known = known_identity_dirs();
    let seat_named = |p: &Path| crate::roles::Role::ALL.iter().any(|r| p.join(r.as_str()).is_dir());
    let mut aside: Vec<PathBuf> = Vec::new();
    let mut strays: Vec<PathBuf> = Vec::new();
    for place in home_places() {
        let Ok(list) = std::fs::read_dir(&place) else { continue };
        for e in list.flatten() {
            let d = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if !d.is_dir() || !crate::names::is_home_name(&name) || !seat_named(&d) || known.iter().any(|k| crate::home::same_place(k, &d)) {
                continue;
            }
            if strays.iter().any(|x| crate::home::same_place(x, &d)) {
                continue;
            }
            let mut files = Vec::new();
            walk(&d, "", &mut files);
            let opens = |at: &Path| std::fs::read(at).map(|b| is_sealed(&b) && opens_under(key, &b)).unwrap_or(false);
            // Within a role directory (`<seat>/<rel in the home>`), local files that open are kept for adoption,
            // and so are non-local files (lock file, exported bundles) when that role keeps at least one local
            // file; everything else is set aside.
            let in_seat = |rel: &str| -> Option<(String, String)> {
                let (seat, rest) = rel.split_once('/')?;
                crate::roles::Role::ALL.iter().any(|r| r.as_str() == seat).then(|| (seat.to_string(), rest.to_string()))
            };
            let seats_kept: Vec<String> = files
                .iter()
                .filter_map(|(at, rel)| in_seat(rel).filter(|(_, rest)| home_doc(rest).is_some() && opens(at)).map(|(seat, _)| seat))
                .collect();
            for (at, rel) in files {
                if is_staged(&rel) {
                    continue;
                }
                let keep = adopt
                    && match in_seat(&rel) {
                        Some((seat, rest)) => seats_kept.contains(&seat) && (home_doc(&rest).is_none() || opens(&at)),
                        None => false,
                    };
                if !keep {
                    aside.push(at);
                }
            }
            strays.push(d);
        }
    }
    let to = if aside.is_empty() { None } else { Some(aside_flat(&aside)?) };
    for d in &strays {
        prune_empty_under(d);
        let _ = std::fs::remove_dir(d);
    }
    Ok((aside.len(), to))
}

/// Role homes that older versions left on disk for deleted identities without recording them: laid out as
/// `<place>/<identity id>/<seat>` next to known homes, but neither registered nor listed as left. Each whose
/// local files all open under the vault's key is recorded as left, so it is renamed with the rest and found
/// again if the identity is imported again. Returns how many were recorded.
fn adopt_left_homes() -> Result<usize, Fault> {
    let Some(mut reg) = crate::register::read()? else { return Ok(0) };
    let key = crate::keybox::local_key()?;
    let mut known: Vec<PathBuf> = reg.rows.iter().flat_map(|r| crate::roles::Role::ALL.iter().filter_map(|s| r.home(*s)).collect::<Vec<_>>()).collect();
    known.extend(reg.left.iter().map(|(_, _, h)| PathBuf::from(h)));
    let mut added = 0usize;
    for h in homes_on_record() {
        let Some(d) = identity_dir_of(&h) else { continue };
        let Some(seat) = h.file_name().and_then(|n| crate::roles::Role::ALL.into_iter().find(|r| r.as_str() == n.to_string_lossy())) else { continue };
        if known.iter().any(|k| crate::home::same_place(k, &h)) {
            continue;
        }
        // Require at least one local file, all sealed and opening under this vault's key. A home of plain files
        // (deleted before sealing existed) or of another vault is not this machine's to rename.
        let mut files = Vec::new();
        walk(&h, "", &mut files);
        let local: Vec<&(PathBuf, String)> = files.iter().filter(|(_, rel)| home_doc(rel).is_some()).collect();
        let all_open = !local.is_empty()
            && local.iter().all(|(at, _)| match std::fs::read(at) {
                Ok(b) => is_sealed(&b) && opens_under(&key, &b),
                Err(_) => false,
            });
        if !all_open {
            continue;
        }
        let id = format!("0x{}", d.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default());
        reg.left.push((id, seat, h.display().to_string()));
        known.push(h);
        added += 1;
    }
    if added > 0 {
        crate::register::write(&reg)?;
    }
    Ok(added)
}

fn read_plan(machine: &Path, name: &str) -> Vec<PathBuf> {
    std::fs::read_to_string(plan_path(&machine, name))
        .map(|t| t.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default()
}

/// The outcome of settling staged files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled {
    /// Staged files that opened under the vault's key and replaced their targets (the change had taken effect).
    pub forward: usize,
    /// Staged files that did not (the change never took effect) and were removed.
    pub back: usize,
    /// Staged files of a change that took effect but could not replace their targets now: reported, and kept
    /// staged for the next unlock (they are the only copy under the new key).
    pub held: usize,
}

fn settle_in(root: &Path, doc_of: &dyn Fn(&str) -> Option<Doc>, key: &LocalKey, back_all: bool, out: &mut Settled, kept: &mut Vec<PathBuf>) -> Result<(), Fault> {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    for (at, rel) in all {
        let Some(base) = unstaged(&rel) else { continue };
        // Not a kind for this pass (a home can sit inside the machine directory); its own pass settles it.
        let Some(doc) = doc_of(base) else { continue };
        let target = beside_strip(&at);
        let opened = if back_all { None } else { std::fs::read(&at).ok().and_then(|b| open_staged(key, doc, &b)).map(|p| (doc, p)) };
        match opened {
            Some((Doc::Machine, plain)) => {
                // Machine settings are stored plain: the sealed staged copy proves it belongs to the change that
                // took effect, and its content is written plain.
                match crate::machine::write_bytes(&plain) {
                    Ok(()) => {
                        let _ = std::fs::remove_file(&at);
                        out.forward += 1;
                    }
                    // Report and continue, so one failure does not hold back the others. The file stays staged
                    // (the sweep below spares it) and is retried at the next unlock.
                    Err(f) => {
                        note_trouble(f);
                        kept.push(at.clone());
                        out.held += 1;
                    }
                }
            }
            Some((_, plain)) if plain == GONE => {
                // Staged as removed: the change deletes this file.
                let _ = std::fs::remove_file(&target);
                let _ = std::fs::remove_file(&at);
                out.forward += 1;
            }
            Some(_) => match crate::home::rename_over(&at, &target) {
                Ok(()) => out.forward += 1,
                Err(f) => {
                    note_trouble(f);
                    kept.push(at.clone());
                    out.held += 1;
                }
            },
            None => {
                let _ = std::fs::remove_file(&at);
                out.back += 1;
            }
        }
    }
    Ok(())
}

fn beside_strip(at: &Path) -> PathBuf {
    let s = at.as_os_str().to_string_lossy().to_string();
    PathBuf::from(unstaged(&s).unwrap_or(&s))
}

/// Where pruning stops for a removed path: the home (or machine directory) that contained it.
fn homes_stop(at: &Path, homes: &[(Whose, PathBuf)], machine: &Path) -> PathBuf {
    homes.iter().map(|(_, h)| h).find(|h| at.starts_with(h)).cloned().unwrap_or_else(|| {
        // A moved home is no longer listed: stop at its role directory, or the machine directory.
        at.ancestors().find(|a| identity_dir_of(a).is_some()).map(Path::to_path_buf).unwrap_or_else(|| machine.to_path_buf())
    })
}

/// Run on unlock, before anything local is read. A change stages its new vault first and renames it last, so
/// a staged vault still on disk means the change never took effect: it and every staged file are removed.
/// Otherwise the change took effect: each staged file that opens under the vault's key replaces its target,
/// and the rest are removed. The machine directory goes first (the register may be staged), then every home
/// the register names (read as stored, since this runs before an older version's plain files are sealed).
pub fn settle_pending() -> Result<Settled, Fault> {
    let key = crate::keybox::local_key()?;
    // Finish an interrupted swap first; its errors are reported but never block the settling below (a master
    // key change may still have its register staged).
    if let Err(f) = settle_swap() {
        note_trouble(f);
    }
    let m = crate::home::machine_dir()?;
    // A migration whose marker is still present never took effect either (`PLAN_OPEN`).
    let never = crate::keybox::drop_staged()? | plan_there(&m, PLAN_OPEN);
    let mut out = Settled::default();
    let machine_doc_or = |rel: &str| -> Option<Doc> {
        if rel == crate::machine::FILE {
            Some(Doc::Machine)
        } else {
            machine_doc(rel)
        }
    };
    let mut kept: Vec<PathBuf> = Vec::new();
    let made = read_plan(&m, PLAN_MADE);
    let aside = read_plan(&m, PLAN_ASIDE);
    let gone = if never { Vec::new() } else { read_plan_sealed(&m, PLAN_GONE, &key) };
    let moved = if never { Vec::new() } else { read_plan_sealed(&m, PLAN_MOVED, &key) };
    if never {
        // The change never took effect: homes it created in new places are unreferenced and are removed with
        // its other staged files.
        for d in &made {
            if d.is_dir() {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }
    settle_in(&m, &machine_doc_or, &key, never, &mut out, &mut kept)?;
    let homes = homes_of(registry_any()?, false)?;
    for (_, h) in &homes {
        settle_in(h, &home_doc, &key, never, &mut out, &mut kept)?;
    }
    if !never && (!gone.is_empty() || !moved.is_empty()) {
        // A names change took effect: remove the old paths, then move each moved home's remaining contents.
        // If any file was held above, skip this and keep the plan staged for the next unlock.
        let mut failed = 0usize;
        if out.held == 0 {
            for g in &gone {
                let at = PathBuf::from(g);
                match std::fs::remove_file(&at) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => failed += 1,
                }
                if let Some(d) = at.parent() {
                    let stop = homes_stop(&at, &homes, &m);
                    prune_up(d, &stop);
                }
            }
            for line in &moved {
                if let Some((a, b)) = line.split_once('\t') {
                    failed += carry_home(Path::new(a), Path::new(b));
                }
            }
        }
        if out.held > 0 || failed > 0 {
            // Not finished: report it and keep the plan staged so the next unlock runs it again (every step is
            // idempotent).
            if failed > 0 {
                note_trouble(Fault::known(Known::RestorePartial, format!("names · {failed}")));
            }
            for name in [PLAN_GONE, PLAN_MOVED] {
                kept.push(plan_path(&m, name));
            }
        }
    }
    let changed = !gone.is_empty() || !moved.is_empty() || !aside.is_empty();
    if !never && !aside.is_empty() {
        // The change took effect: set aside what the listed roots hold that the new key cannot open, reported
        // once.
        match set_aside_unopenable(aside, &key) {
            Ok((0, _)) => {}
            Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
            Err(f) => {
                // Report it and keep the plan staged; the next unlock sets aside what is left (idempotent).
                note_trouble(f);
                kept.push(plan_path(&m, PLAN_ASIDE));
            }
        }
    }
    // After a change that took effect and completed (names, master key, restore), no id-named directory that
    // is not a registered home may remain (`clear_strays`).
    if !never && changed && kept.is_empty() {
        match clear_strays(&key, false) {
            Ok((0, _)) => {}
            Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
            Err(f) => note_trouble(f),
        }
    }
    // Any staged file no pass claimed belongs to nothing that took effect (held files were claimed).
    let mut left = Vec::new();
    walk(&m, "", &mut left);
    for (_, h) in &homes {
        walk(h, "", &mut left);
    }
    for (at, rel) in left {
        if is_staged(&rel) && !kept.contains(&at) && std::fs::remove_file(&at).is_ok() {
            out.back += 1;
        }
    }
    Ok(out)
}

// ───────────────────────── Right after the vault opens ─────────────────────────

/// What the post-unlock passes have to report (taken once by the shell when the unlock completes).
static OPENED: std::sync::Mutex<Option<Opened>> = std::sync::Mutex::new(None);

/// What the post-unlock passes found.
#[derive(Clone, Debug, Default)]
pub struct Opened {
    pub settled: Settled,
    pub migrated: usize,
    /// Local files moved to their keyed names (on the first unlock of an older version's data).
    pub renamed: usize,
    /// The primary identity chosen for an older vault (shown once on screen).
    pub primary: Option<String>,
    pub troubles: Vec<Fault>,
}

/// Run right after the vault opens (unlock, reseal, recovery), in the same background task: settle staged
/// files of a master key change, seal plain files left by an older version, settle the primary identity of an
/// older vault, and key old file names. Errors are collected for the shell; none undoes the unlock.
pub fn after_open() {
    let mut o = Opened::default();
    match settle_pending() {
        Ok(s) => o.settled = s,
        Err(f) => o.troubles.push(f),
    }
    match migrate_plain() {
        Ok(m) => o.migrated = m.sealed,
        Err(f) => o.troubles.push(f),
    }
    match crate::register::read().and_then(|listed| crate::identity::settle_primary(listed.as_ref())) {
        Ok(p) => o.primary = p,
        Err(f) => o.troubles.push(f),
    }
    // Upgrade an older key store's layout in place (after the primary is settled) so its file names reveal no
    // identity (`keybox::upgrade_names`).
    if let Err(f) = crate::keybox::upgrade_names() {
        o.troubles.push(f);
    }
    // Then key local file names that still reveal identity, entry or grant ids (`migrate_names`; an
    // interrupted run resumes here at the next unlock).
    match migrate_names() {
        Ok(n) => o.renamed = n,
        Err(f) => o.troubles.push(f),
    }
    // Merge rather than replace, so troubles noted during the passes (`note_trouble`) are reported too.
    if let Ok(mut g) = OPENED.lock() {
        let was = g.get_or_insert_with(Opened::default);
        was.settled = o.settled;
        was.migrated = o.migrated;
        was.renamed = o.renamed;
        was.primary = o.primary;
        was.troubles.extend(o.troubles);
    }
}

/// Record a problem for the shell to report when the unlock completes (for example, a change that took effect
/// but could not settle every staged file yet; the next unlock settles them).
pub fn note_trouble(f: Fault) {
    if let Ok(mut g) = OPENED.lock() {
        g.get_or_insert_with(Opened::default).troubles.push(f);
    }
}

/// Take what the post-unlock passes found (empty afterwards).
pub fn take_opened() -> Option<Opened> {
    OPENED.lock().ok().and_then(|mut g| g.take())
}
