//! Signing API for the anchor key. Only the two legal domains are exposed: entry and co-signature.
//!
//! `zikaron/1` §5.7: the anchor key never holds funds, never delegates and never carries code, and the
//! interface never offers a way to send funds or delegate with it. In code, the dangerous form is a general signing
//! function, `sign(bytes)` or `sign(domain, bytes)`: with it, `personal_sign` and EIP-7702 authorizations are
//! one string away at the caller, and "nobody pass that string" is a convention, not structure.
//!
//! So the design has two layers:
//!
//! 1. The domain type is closed ([`Law`]), not `&str`. It wraps the core's closed type and has no other
//!    constructor, so passing a domain string in cannot even be written, misspelled or not.
//! 2. One function per face, each with its domain fixed in its body. Signing another domain is not merely
//!    forbidden: there is no function to call, and no parameter chooses a domain.
//!
//! [`Face`] maps the two faces one to one onto the two domains. The specs define four domains; this desk signs
//! only its two (`zikaron.kit/1`'s fpm and ack domains are signed by the CLI's `fpm-sign` and `ack-sign`).
//! [`domains`] is computed from this table, so an added domain shows up in the UI and the self-check suite at
//! once.
//!
//! The private key bytes are lent by the `key` layer ([`crate::key::Secret::with_sign_key`]) for exactly the
//! call in [`seal`]: this file does not keep them and cannot get them a second time (see the `key` module
//! header, "five exits").

/// The anchor key layer. The file is still `src/key.rs` (re-exported in `lib.rs`); it is a child module only
/// so that the key-lending exit is visible to this file alone.
#[path = "key.rs"]
pub mod key;

use crate::fault::{Fault, Known};
use crate::key::Secret;
use zikaron::cryptox;
use zikaron::entry as k1;
use zikaron::hexfmt;

/// The closed domain type. Each variant wraps a closed type from the core; a free string cannot be expressed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Law {
    /// The two domains of `zikaron/1` §5.2 (the core's closed type).
    Core(zikaron::tokens::Domain),
}

impl Law {
    /// This domain's literal, taken from the core's closed type; this layer spells nothing itself.
    pub fn as_str(self) -> &'static str {
        match self {
            Law::Core(d) => d.as_str(),
        }
    }
}

/// The two faces this key signs, one to one with the two domains.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    /// Entry (legal domain `zikaron/1`).
    Entry,
    /// Co-signature (legal domain `zikaron/1-adoption`).
    Adoption,
}

impl Face {
    pub const ALL: [Face; 2] = [Face::Entry, Face::Adoption];

    pub fn as_str(self) -> &'static str {
        match self {
            Face::Entry => "entry",
            // Product names do not reuse the spec's words. This domain's real literal `zikaron/1-adoption`
            // comes from the core's closed type (see [`Face::law`]); the short name is this layer's own.
            Face::Adoption => "cosign",
        }
    }

    /// Which domain this face signs. The face-to-domain mapping exists only here.
    pub fn law(self) -> Law {
        match self {
            Face::Entry => Law::Core(zikaron::tokens::Domain::Entry),
            Face::Adoption => Law::Core(zikaron::tokens::Domain::Adoption),
        }
    }

    /// The literal of this face's domain.
    pub fn domain(self) -> &'static str {
        self.law().as_str()
    }

    /// Parse a face from its short name. Unknown names give `None`.
    pub fn parse(short: &str) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.as_str() == short)
    }
}

/// Sign once, returning `0x` plus 130 hex digits (r‖s‖v, `zikaron/1` §5.4).
///
/// The third argument takes [`Law`]: the union of the core's closed domain types, not a string.
fn seal(s: &Secret, preimage: &[u8], domain: Law) -> Result<String, Fault> {
    let digest = digest_under(preimage, domain);
    // Every call to the primitive that actually signs is in this file. The key layer lends the private key
    // bytes for exactly this statement; this file does not keep them and cannot get them twice.
    //
    // The third returned member is already v (27 or 28), not the recovery id: adding 27 again would fail the
    // core's `SigV` check. The CLI's `entry::sign` does the same.
    let (r, sv, v) = s
        .with_sign_key(|k| cryptox::sign_digest(k, &digest))
        .ok_or_else(|| Fault::known(Known::SignFailed, format!("domain {}", domain.as_str())))?;
    let mut out = Vec::with_capacity(65);
    out.extend_from_slice(&r);
    out.extend_from_slice(&sv);
    out.push(v);
    Ok(hexfmt::encode(&out))
}

/// The 32 bytes actually signed after the domain prefix. Computed only here (`zikaron/1` §5.2 to §5.3 is the
/// core's).
///
/// The signing statement (`seal`) and the copy handed to tests for outside verification use this one
/// function; computing the digest in two places would leave "what is verified outside is what was signed
/// inside" unsupported.
fn digest_under(preimage: &[u8], domain: Law) -> [u8; 32] {
    k1::presig_and_digest(preimage, domain.as_str()).1
}

/// As `digest_under`, with the domain taken from the face (a closed table; callers cannot pass a free string).
///
/// Tests hand it to an outside signature verifier (what `cast wallet verify --no-hash` needs), so they need
/// not recompute the core's rules.
pub fn digest_as(preimage: &[u8], face: Face) -> [u8; 32] {
    digest_under(preimage, face.law())
}

/// Entry. The domain is fixed in the body; callers cannot pass another.
pub fn sign_entry(s: &Secret, preimage: &[u8]) -> Result<String, Fault> {
    seal(s, preimage, Law::Core(zikaron::tokens::Domain::Entry))
}

/// Co-signature. The domain is fixed in the body.
pub fn sign_adoption(s: &Secret, preimage: &[u8]) -> Result<String, Fault> {
    seal(s, preimage, Law::Core(zikaron::tokens::Domain::Adoption))
}

/// The signing port the sender asks through (`zikaron_send::batch::Signer`): the only place an anchoring
/// transaction is signed. The key is lent here alone (`with_tx_key`, visible to this file only), to
/// `send::sign_at` (a registry call to `registry` with the hashes, or the given call data), at the given nonce;
/// asking the nonce, broadcasting and every refusal are the sender's.
impl zikaron_send::batch::Signer for Secret {
    fn from(&self) -> Option<[u8; 20]> {
        self.address().map(|a| a.0)
    }

    fn sign(&self, chain: u64, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], input: Option<Vec<u8>>, fees: zikaron_anchor::send::Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), zikaron_anchor::rpc::Trouble> {
        self.with_tx_key(|k| zikaron_anchor::send::sign_at(k, chain, zikaron_anchor::send::Form::Registry, registry, hashes, input, fees, nonce))
    }
}

/// Sign by face. The window and tests go through here; this is the only face-to-function mapping.
pub fn sign_as(s: &Secret, preimage: &[u8], face: Face) -> Result<String, Fault> {
    match face {
        Face::Entry => sign_entry(s, preimage),
        Face::Adoption => sign_adoption(s, preimage),
    }
}

/// Every domain this key signs, computed from the face table. The UI and the self-check suite both use it,
/// so an added domain is visible at once.
pub fn domains() -> Vec<&'static str> {
    Face::ALL.iter().map(|f| f.domain()).collect()
}

// ───────────────────────── Seat × domain ─────────────────────────

/// What the key is used for this time: one variant per signing face, plus sending an anchoring transaction.
///
/// Derived from [`Face`], so an added domain adds a variant here (the self-check suite counts both and
/// compares).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Use {
    /// Sign a face.
    Sign(Face),
    /// Send an anchoring transaction (signed through this key's port to the sender, `zikaron_send::batch::Signer`).
    /// Not a signing domain: it signs a transaction, with the same key.
    Anchor,
}

impl Use {
    /// All uses: one per face, plus anchoring.
    pub fn all() -> Vec<Use> {
        Face::ALL.iter().map(|f| Use::Sign(*f)).chain([Use::Anchor]).collect()
    }

    /// This member's short name (the face's short name, or `anchor`).
    pub fn as_str(self) -> &'static str {
        match self {
            Use::Sign(f) => f.as_str(),
            Use::Anchor => "anchor",
        }
    }
}

/// Whether `seat` may use this key for `u`. Every cell of `Role::ALL` × [`Use::all`] has exactly one answer.
///
/// If fetching the signing key only asked "does the current seat own the open home" and not "which domain is
/// this", the grantee seat could sign co-signatures, and readers of the chain could not tell what each seat
/// signed. A seat check inside each signing function would be forgotten with the next domain. Instead the
/// domain-to-seat mapping lives only here: every key user names its use and passes it in, and this table
/// decides (refusals use `SEAT_DOMAIN`).
///
/// * Entry: both seats. "Its own ledger" is enforced by a separate check at key fetching (the current seat
///   must own the open home, see `action::signing_key`), so relicensing (`zikaron.kit/1` §10.4: the grantee
///   opens their own ledger and signs their own entries) works, and signing into someone else's ledger is
///   impossible.
/// * Co-signature: the author seat's domain. The product currently only verifies co-signatures and never
///   signs them (`adoptx::cosigned` verifies the counterpart's), so no product path reaches this cell; it is
///   still answered in the table.
/// * Anchoring: both seats, each anchoring the queued entries of its own ledger.
pub fn seat_may(seat: crate::roles::Role, u: Use) -> bool {
    use crate::roles::Role;
    match (seat, u) {
        // Entry: each seat signs its own ledger.
        (Role::Author, Use::Sign(Face::Entry)) | (Role::Grantee, Use::Sign(Face::Entry)) => true,
        // Co-signature: author seat only.
        (Role::Author, Use::Sign(Face::Adoption)) => true,
        (Role::Grantee, Use::Sign(Face::Adoption)) => false,
        // Anchoring: each seat anchors its own ledger.
        (Role::Author, Use::Anchor) | (Role::Grantee, Use::Anchor) => true,
    }
}

/// Which seat this use belongs to (the reverse lookup, so a refusal can say which seat to switch to). Uses
/// open to both seats return `None`.
pub fn seat_for(u: Use) -> Option<crate::roles::Role> {
    let allowed: Vec<crate::roles::Role> = crate::roles::Role::ALL.into_iter().filter(|r| seat_may(*r, u)).collect();
    match allowed.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}
