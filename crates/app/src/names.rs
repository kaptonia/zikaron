//! Names on disk: the one place a name that would say something on the chain is made.
//!
//! Locked, nothing on this machine may lead to an identity on the chain: not a file's bytes (sealed, `local`)
//! and not its name. Every name built from an address, an identity id, an entry id, a grant id or a terms
//! digest is keyed here by the names key NK (`keybox::name_key`, derived from the master key): the same master
//! key always gives the same name for the same thing (a deleted identity imported again finds its two homes),
//! and a new master key renames everything. The names that stay plain are a closed table of fixed room and
//! file names; anything not in it goes through here.

/// What a name is made from. Closed: a new kind of named thing is added here, never spelled at its call site.
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
            // An account name is taken as it is (it is not an id).
            Logical::Slot(a) => return a.to_string(),
        };
        v
    }

    /// How many hex digits the name keeps: a home keeps the shape of an address (40), everything else the shape
    /// of an entry id (64), so the store's and the walkers' shape checks still hold.
    fn width(&self) -> usize {
        match self {
            Logical::Home(_) => 40,
            _ => 64,
        }
    }
}

/// The names key in hand, wiped when dropped.
pub struct NameKey([u8; 32]);

impl NameKey {
    pub(crate) fn new(k: [u8; 32]) -> NameKey {
        NameKey(k)
    }

    /// The name on disk of one logical thing under this key (lowercase hex).
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

/// The names key of the vault now (refused with `LOCKED` while locked).
pub fn key() -> Result<NameKey, crate::fault::Fault> {
    crate::keybox::name_key()
}

/// Whether a directory name has the shape of a home name (40 lowercase hex digits): the layout rule, used where
/// the register cannot be read (a restore from the lock card).
pub fn is_home_name(n: &str) -> bool {
    n.len() == 40 && n.bytes().all(|b| b.is_ascii_hexdigit())
}
