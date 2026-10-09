//! On-disk names for anything that could be linked to on-chain data.
//!
//! While locked, nothing on this machine may lead to an on-chain identity: not file contents (sealed, see
//! `local`) and not file names. Every name derived from an address, identity id, entry id, grant id or terms
//! digest is an HMAC under the names key NK (`keybox::name_key`, derived from the master key). The same master
//! key always gives the same name for the same thing (so a deleted identity imported again finds its homes),
//! and a new master key renames everything. Only fixed directory and file names stay plain.

/// What a name is derived from. New kinds of named things are added here, never built at the call site.
#[derive(Clone, Copy, Debug)]
pub enum Logical<'a> {
    /// An identity's home directory (by identity id): `<place>/<name>/<seat>`.
    Home(&'a str),
    /// A ledger entry file (by entry id).
    Entry(&'a str),
    /// A held grant in the vault (by grant id); its verdict cache takes the same stem.
    Held(&'a str),
    /// A kept grant file (by bundle id).
    Kept(&'a str),
    /// A terms document's directory and file (by terms digest).
    TermsDir(&'a str),
    TermsDoc(&'a str),
    /// A grant's issuance record (by grant id).
    TermsRecord(&'a str),
    /// A key store slot (by account name).
    Slot(&'a str),
}

impl Logical<'_> {
    fn tag(&self) -> &'static [u8] {
        match self {
            Logical::Home(_) => b"home",
            Logical::Entry(_) => b"entry",
            Logical::Held(_) => b"held",
            Logical::Kept(_) => b"kept",
            Logical::TermsDir(_) => b"terms-dir",
            Logical::TermsDoc(_) => b"terms-doc",
            Logical::TermsRecord(_) => b"terms-record",
            Logical::Slot(_) => b"slot",
        }
    }

    fn value(&self) -> String {
        let v = match self {
            Logical::Home(x) | Logical::Entry(x) | Logical::Held(x) | Logical::Kept(x) | Logical::TermsDir(x) | Logical::TermsDoc(x) | Logical::TermsRecord(x) => x.trim().trim_start_matches("0x").to_ascii_lowercase(),
            // An account name is used as is (it is not an id).
            Logical::Slot(a) => return a.to_string(),
        };
        v
    }

    /// How many hex digits the name keeps: 40 for a home (address-shaped), 64 otherwise (entry-id-shaped), so
    /// existing shape checks in the store and directory walkers still hold.
    fn width(&self) -> usize {
        match self {
            Logical::Home(_) => 40,
            _ => 64,
        }
    }
}

/// The names key, wiped when dropped.
pub struct NameKey([u8; 32]);

impl NameKey {
    pub(crate) fn new(k: [u8; 32]) -> NameKey {
        NameKey(k)
    }

    /// The on-disk name of one logical item under this key: truncated `HMAC-SHA256(NK, tag ‖ 0 ‖ value)`, as
    /// lowercase hex.
    pub fn name(&self, l: Logical) -> String {
        let mut msg = Vec::with_capacity(64);
        msg.extend_from_slice(l.tag());
        msg.push(0);
        msg.extend_from_slice(l.value().as_bytes());
        let mac = crate::cryptx::hmac_sha256(&self.0, &msg);
        let hex = zikaron::hexfmt::encode(&mac);
        hex.trim_start_matches("0x")[..l.width()].to_string()
    }
}

impl Drop for NameKey {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            *b = 0;
        }
    }
}

/// The vault's current names key (`LOCKED` while locked).
pub fn key() -> Result<NameKey, crate::fault::Fault> {
    crate::keybox::name_key()
}

/// Whether a directory name has the shape of a home name (40 hex digits), for when the register cannot be
/// read (a restore from the lock screen).
pub fn is_home_name(n: &str) -> bool {
    n.len() == 40 && n.bytes().all(|b| b.is_ascii_hexdigit())
}
