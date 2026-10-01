//! Signing API for the anchor key. Only the two legal domains are exposed: entry and co-signature.
//!
//! ─── Why this is structure, not a reminder ───
//!
//! Law §5.7: the anchor key never holds funds, never delegates, never carries code; the interface never
//! offers a way to send funds or delegate with the anchor key. In code, the dangerous form is a general
//! signing function, `sign(bytes)` or `sign(domain, bytes)`. With it, `personal_sign` and 7702 authorizations
//! are one string away at the caller, and "nobody fill in that string" is a reminder, not structure.
//!
//! The design has two layers:
//!
//! 1. The domain cell's type is closed ([`Law`]), not `&str`. It wraps the core's closed type and has no
//! other constructor in this file, so "pass a domain string in" has no form at compile time, not even a
//! misspelled one.
//!
//! 2. One face, one function: two faces, two functions, each with its domain fixed in its body. So signing another
//! domain is not merely forbidden; there is no function to call, and "choose the domain by a parameter" has
//! no form either.
//!
//! [`Face`] is the table of faces and domains: two faces, two domains, one to one. The law has a closed set
//! of four domains; this desk signs only its two legal ones (kit law's fpm and ack domains are signed by the
//! command-line `fpm-sign` and `ack-sign`; this desk has no face for them). [`domains`] is computed from this
//! table, so an extra domain shows up on the face and in the self-check suite the same day.
//!
//! The private key bytes are lent once by the `key` layer ([`crate::key::Secret::with_sign_key`]), and the
//! loan covers exactly the call in [`seal`]: this file does not keep them and cannot get them a second time
//! (see the `key` file header, "five exits").

/// The anchor key layer. The file is still `src/key.rs` (see the re-export in `lib.rs`); it is made a child
/// for one reason only: the key-lending exit is visible only to this file.
#[path = "key.rs"]
pub mod key;

use crate::fault::{Fault, Known};
use crate::key::Secret;
use zikaron::cryptox;
use zikaron::entry as k1;
use zikaron::hexfmt;

/// The closed type of domains. Each constructor wraps a closed type from the law; a free string has no form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Law {
    /// Law §5.2's two domains (the core's closed type).
    Core(zikaron::tokens::Domain),
}

impl Law {
    /// This domain's literal. Taken from the law's closed type; this layer spells nothing itself.
    pub fn as_str(self) -> &'static str {
        match self {
            Law::Core(d) => d.as_str(),
        }
    }
}

/// The two faces this key will sign. Closed, only these two, one to one with the two domains.
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
            // The product's own names do not share a form with the law's words. This domain's real literal
            // `zikaron/1-adoption` comes from the core's closed type (see [`Face::law`]) and is spelled
            // nowhere; the short name is this layer's own word.
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

    /// Recognize a face by short name. Unrecognized gives `None`: a third name has nowhere to go.
    pub fn parse(short: &str) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.as_str() == short)
    }
}

/// Sign once, returning `0x` plus 130 hex digits (r‖s‖v, law §5.4).
///
/// The third argument takes [`Law`]: the union of the law's closed types, not a string.
fn seal(s: &Secret, preimage: &[u8], domain: Law) -> Result<String, Fault> {
    let digest = digest_under(preimage, domain);
    // Every call site of the primitive that actually signs is in this file. The key layer lends the private
    // key bytes once, for exactly this statement; this file does not keep them and cannot get them twice.
    //
    // The third member returned is already v (27 or 28), not the recovery id: adding 27 again here would make
    // the law judge `SigV` at once. The CLI's `entry::sign` is the same precedent.
    let (r, sv, v) = s
        .with_sign_key(|k| cryptox::sign_digest(k, &digest))
        .ok_or_else(|| Fault::known(Known::SignFailed, format!("domain {}", domain.as_str())))?;
    let mut out = Vec::with_capacity(65);
    out.extend_from_slice(&r);
    out.extend_from_slice(&sv);
    out.push(v);
    Ok(hexfmt::encode(&out))
}

/// The thirty-two bytes actually signed after the domain prefix. Computed only here (law §5.2 to §5.3 belongs
/// to the core).
///
/// The signing statement (`seal`) and the copy handed out for outside verification in tests read the same
/// place; computing the digest in two places would leave "what is verified outside is what was signed inside"
/// without support.
fn digest_under(preimage: &[u8], domain: Law) -> [u8; 32] {
    k1::presig_and_digest(preimage, domain.as_str()).1
}

/// As above, with the domain taken from the face (a closed table; callers cannot pass in a free string).
///
/// Tests hand it to an outside signature verifier (exactly what `cast wallet verify --no-hash` needs), so
/// they do not have to recompute the core's law.
pub fn digest_as(preimage: &[u8], face: Face) -> [u8; 32] {
    digest_under(preimage, face.law())
}

/// 1 · Entry. The domain is fixed in the body; callers cannot pass another.
pub fn sign_entry(s: &Secret, preimage: &[u8]) -> Result<String, Fault> {
    seal(s, preimage, Law::Core(zikaron::tokens::Domain::Entry))
}

/// 2 · Co-signature.
pub fn sign_adoption(s: &Secret, preimage: &[u8]) -> Result<String, Fault> {
    seal(s, preimage, Law::Core(zikaron::tokens::Domain::Adoption))
}

/// Where this broadcast went: hash, how many nodes were asked, which one took it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Broadcast {
    pub hash: [u8; 32],
    /// How many times the transaction (`eth_sendRawTransaction`) was submitted in total (including backoff
    /// retries at the same place).
    pub asked: usize,
    /// The endpoint that took it.
    pub taken: String,
}

/// Send an anchoring transaction with the whole endpoint table. The key-lending exit (`with_tx_key`) is
/// visible only to this file, and its only caller is here: signing and submission both happen inside this
/// loan.
///
/// Using only the first endpoint for nonce and broadcast, with no retry or failover, would leave a whole
/// batch stuck when the preset public nodes rate-limit. So:
/// 1. Sign once: the nonce is read endpoint by endpoint; when rate-limited, wait per `backoff` and ask the
/// same place again; when unreadable, move to the next.
/// 2. Submit the same signed bytes endpoint by endpoint; a matching echo is success. Network-kind refusals
/// (rate limit, no answer, timeout, bad answer, access refused… `fault::Known::NETWORK`) move to the next
/// place (rate limiting first retries the same place per `backoff`); the nonce is fixed and the hash
/// identical, so there is no double spend.
/// 3. A node's substantive verdict (insufficient balance, nonce used, underpriced, execution reverted…) would
/// be the same everywhere, so it is refused by name at once without asking further.
/// 4. Only when no place took it is it refused by name with the last answer.
///
/// `backoff` is the list of backoff durations, given by the caller (a shell cell, changeable by tests);
/// how the transaction is signed and broadcast belongs entirely to the anchoring crate.
#[allow(clippy::too_many_arguments)]
pub fn anchor_send(
    s: &Secret,
    urls: &[String],
    chain: u64,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    fees: zikaron_anchor::send::Fees,
    backoff: &[std::time::Duration],
) -> Result<Broadcast, Fault> {
    let first = urls.first().cloned().unwrap_or_default();
    let open = |url: &str| {
        crate::chainx::endpoint_at(url).ok_or_else(|| {
            Fault::known(Known::NoEndpoint, crate::lang::filln(crate::lang::Key::Tail216, &[&(url).to_string()]))
        })
    };
    use crate::chainx::Next;
    let waits: Vec<std::time::Duration> = std::iter::once(std::time::Duration::ZERO).chain(backoff.iter().copied()).collect();
    let mut last: Option<Fault> = None;
    // 1 · Sign once (nonce read endpoint by endpoint).
    crate::task::stage_at(crate::task::Kind::Anchor, 0);
    let mut signed: Option<(Vec<u8>, [u8; 32])> = None;
    'sign: for url in urls {
        let mut http = match open(url) {
            Ok(h) => h,
            Err(f) => {
                last = Some(f);
                continue;
            }
        };
        for w in &waits {
            if !w.is_zero() {
                std::thread::sleep(*w);
            }
            let got = s.with_tx_key(|k| {
                zikaron_anchor::send::sign_for(http.as_mut(), k, chain, zikaron_anchor::send::Form::Registry, registry, hashes, None, fees)
            });
            match got {
                Ok(x) => {
                    signed = Some(x);
                    break 'sign;
                }
                Err(t) => {
                    let f = crate::chainx::said_fault(url, &t);
                    match crate::chainx::next_after(&f) {
                        Next::Retry => last = Some(f),
                        Next::NextEndpoint => {
                            last = Some(f);
                            continue 'sign;
                        }
                        Next::Stop => return Err(f),
                    }
                }
            }
        }
    }
    let Some((raw, hash)) = signed else {
        return Err(last.unwrap_or_else(|| Fault::known(Known::NoEndpoint, first)));
    };
    // 2 · Submit the same bytes endpoint by endpoint.
    crate::task::stage_at(crate::task::Kind::Anchor, 1);
    let mut asked = 0usize;
    // Some earlier place already received these bytes (it answered with a network-kind refusal, and the bytes
    // may already be in its pool).
    let mut handed = false;
    for url in urls {
        let mut http = match open(url) {
            Ok(h) => h,
            Err(f) => {
                last = Some(f);
                continue;
            }
        };
        for w in &waits {
            if !w.is_zero() {
                std::thread::sleep(*w);
            }
            asked += 1;
            match zikaron_anchor::send::submit(http.as_mut(), &raw, &hash) {
                Ok(()) => return Ok(Broadcast { hash, asked, taken: url.clone() }),
                Err(t) => {
                    // What the node said, named by member: dispatch lives only in `chainx::said_fault`.
                    let f = crate::chainx::said_fault(url, &t);
                    // "Already pending" or "nonce used": ask once by hash; if a node knows the transaction it
                    // was taken (then wait for the receipt), otherwise refuse as before.
                    if crate::chainx::already_taken(&f) && crate::chainx::tx_known(urls, chain, &hash) {
                        return Ok(Broadcast { hash, asked, taken: url.clone() });
                    }
                    match crate::chainx::next_after(&f) {
                        Next::Retry => last = Some(f),
                        Next::NextEndpoint => {
                            handed = true;
                            last = Some(f);
                            break;
                        }
                        Next::Stop => return Err(f),
                    }
                }
            }
        }
    }
    // Submitted but no place answered (timeout, disconnect): the bytes may be in some pool. Ask once by hash;
    // known means taken (the receipt decides), unknown means refuse. Refusing outright would make the next
    // pass anchor again with a new nonce, and if the earlier transaction was also included the batch would be
    // anchored twice.
    if handed && crate::chainx::tx_known(urls, chain, &hash) {
        let taken = urls.first().cloned().unwrap_or_default();
        return Ok(Broadcast { hash, asked, taken });
    }
    Err(last.unwrap_or_else(|| Fault::known(Known::NoEndpoint, first)))
}

/// Sign by face. The window and tests go through here: the face-to-function mapping is only this table
/// (one name, one home).
pub fn sign_as(s: &Secret, preimage: &[u8], face: Face) -> Result<String, Fault> {
    match face {
        Face::Entry => sign_entry(s, preimage),
        Face::Adoption => sign_adoption(s, preimage),
    }
}

/// Every domain this key recognizes, computed from the face table. The face and the self-check suite both ask
/// it: an extra domain in this table is visible the same day.
pub fn domains() -> Vec<&'static str> {
    Face::ALL.iter().map(|f| f.domain()).collect()
}

// ───────────────────────── Seat × domain ─────────────────────────

/// What the key is used for this time. Closed: one member for each signing face, plus sending an anchoring
/// transaction.
///
/// Computed from [`Face`], so an extra domain adds a member here the same day (the self-check suite counts
/// both and compares).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Use {
    /// Sign a face.
    Sign(Face),
    /// Send an anchoring transaction ([`anchor_send`]). Not a signing domain: it signs a transaction, with
    /// the same key.
    Anchor,
}

impl Use {
    /// Closed set of three: one per face, plus anchoring.
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

/// Which seat may use this key for this use. Closed (seat × domain: each cell of `Role::ALL` × [`Use::all`]
/// answers once).
///
/// ─── Why this table ───
///
/// If fetching the signing key only asked "does the current seat own the open home" and not "which domain is
/// this", the grantee seat could sign co-signatures, and people reading the chain could not tell what each
/// seat signed (the victims are readers of both seats' ledgers). Checking the seat once more inside signing
/// would be forgotten again with the next domain. Instead: the domain-to-seat mapping lives only
/// here; every key user names its use and passes it in, and this table allows it or refuses by name
/// (`SEAT_DOMAIN`).
///
/// ─── Each cell ───
///
/// * Entry: both seats. "Its own ledger" is carried by another question at key fetching (the current seat
/// must own the open home, see `action::signing_key`), so relicensing (kit/1 §10.4: the grantee opens their
/// own ledger and signs their own entries) still works, and signing into someone else's ledger has nowhere to
/// go.
/// * Co-signature: the recorder seat's domain. The product today only verifies and never signs
/// (`adoptx::cosigned` verifies the counterpart's), so no product path reaches this cell; it still answers
/// once in the table.
/// * Anchoring: both seats, each anchoring the queued entries of its own ledger.
pub fn seat_may(seat: crate::roles::Role, u: Use) -> bool {
    use crate::roles::Role;
    match (seat, u) {
        // Entry: each seat signs its own ledger.
        (Role::Author, Use::Sign(Face::Entry)) | (Role::Grantee, Use::Sign(Face::Entry)) => true,
        // Co-signature: recorder seat only.
        (Role::Author, Use::Sign(Face::Adoption)) => true,
        (Role::Grantee, Use::Sign(Face::Adoption)) => false,
        // Anchoring: each seat anchors its own ledger.
        (Role::Author, Use::Anchor) | (Role::Grantee, Use::Anchor) => true,
    }
}

/// Which seat this use belongs to (the other side of the closed table: a refusal must say which seat to go
/// to). Uses available to both seats return `None`.
pub fn seat_for(u: Use) -> Option<crate::roles::Role> {
    let allowed: Vec<crate::roles::Role> = crate::roles::Role::ALL.into_iter().filter(|r| seat_may(*r, u)).collect();
    match allowed.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}
