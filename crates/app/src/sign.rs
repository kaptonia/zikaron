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

/// Where this broadcast went: hash, how many nodes were asked, which one took it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Broadcast {
    pub hash: [u8; 32],
    /// How many times the transaction (`eth_sendRawTransaction`) was submitted in total (including backoff
    /// retries at the same place).
    pub asked: usize,
    /// The endpoint that took it.
    pub taken: crate::chainx::NodeAddr,
    /// The nonce it was signed at, when this send chose it (`anchor_send`); a resend's is the batch's own.
    pub nonce: Option<u64>,
}

/// Send an anchoring transaction using the whole endpoint table. The key-lending exit (`with_tx_key`) is
/// visible only to this file and called only here: signing and submission both happen inside this loan.
///
/// Using only the first endpoint for nonce and broadcast, with no retry or failover, would leave a whole
/// batch stuck when the preset public nodes rate-limit. So:
/// 1. Sign once: the pending nonce is asked of every endpoint at once (each with the patience table, so a
///    rate limit is waited out there) and the largest is taken; an endpoint that does not answer is skipped.
/// 2. Submit the same signed bytes endpoint by endpoint; a matching echo is success. Network-kind refusals
///    (rate limit, no answer, timeout, bad answer, access refused: `fault::Known::NETWORK`) move to the next
///    endpoint (a rate limit first retries the same one per `backoff`). The nonce is fixed and the hash
///    identical, so there is no double spend.
/// 3. A node's substantive verdict (insufficient balance, nonce used, underpriced, execution reverted…)
///    would be the same everywhere, so it is returned at once without asking further.
/// 4. Only when no endpoint took it is it refused, with the last answer.
///
/// `backoff` is the list of backoff durations from the caller (a shell field tests can change); how the
/// transaction is signed and broadcast belongs to the anchoring crate. `_pass` is the exit gate's
/// [`crate::exitgate::Pass`]: there is no way to the chain except through the gate.
#[allow(clippy::too_many_arguments)]
pub fn anchor_send(
    _pass: &crate::exitgate::Pass,
    s: &Secret,
    urls: &[crate::chainx::NodeAddr],
    chain: u64,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    fees: zikaron_anchor::send::Fees,
    backoff: &[std::time::Duration],
) -> Result<Broadcast, Fault> {
    // The first node in its safe-to-display form, as a refusal's evidence names it.
    let first = urls.first().map(crate::chainx::NodeAddr::said).unwrap_or_default();
    let open = |url: &crate::chainx::NodeAddr| {
        crate::chainx::endpoint_at(url.for_transport()).ok_or_else(|| {
            Fault::known(Known::SettingsShape, crate::chainx::address_said(url.for_transport()))
        })
    };
    use crate::chainx::Next;
    let mut last: Option<Fault> = None;
    // Step 1: sign once. The pending nonce is asked of every endpoint at once, each with the patience table
    // (`zikaron_anchor::patience`: a rate limit is waited out there, never here), and the largest is taken (a
    // node that is behind would hand out a nonce already used). An endpoint that did not answer is skipped; a
    // node's substantive verdict refuses at once; if none answers, the last answer is returned.
    crate::task::stage_at(crate::task::Kind::Anchor, 0);
    let mut signed: Option<(Vec<u8>, [u8; 32], u64)> = None;
    let from = s.address().map(|a| a.0);
    if let Some(from) = from {
        let params = zikaron_anchor::send::nonce_params(&from);
        let mut nodes: Vec<(String, Box<dyn zikaron_anchor::rpc::Endpoint + Send>)> = Vec::new();
        let mut words: Vec<(usize, Fault)> = Vec::new();
        for (i, url) in urls.iter().enumerate() {
            match open(url) {
                Ok(h) => nodes.push((url.for_transport().to_string(), h)),
                Err(f) => words.push((i, f)),
            }
        }
        let answers = zikaron_anchor::endpoints::ask_each(nodes, "eth_getTransactionCount", &params);
        let judged = zikaron_anchor::judge::judge("eth_getTransactionCount", &params, answers);
        match judged {
            Ok(j) => {
                let taker = j.sources.first().cloned().unwrap_or_default();
                match j.value.as_str().and_then(crate::chainx::wei).and_then(|n| u64::try_from(n).ok()) {
                    Some(nonce) => match signed_at(s, chain, registry, hashes, None, fees, nonce) {
                        Ok((raw, hash)) => signed = Some((raw, hash, nonce)),
                        Err(t) => return Err(crate::chainx::said_fault(&taker, &t)),
                    },
                    None => last = Some(crate::chainx::said_fault(&taker, &zikaron_anchor::rpc::Trouble::Transport("nonce 读不出".into()))),
                }
            }
            Err(zikaron_anchor::judge::NoReading::NoneAnswered(missing)) => {
                for (url, m) in missing {
                    let t = match m {
                        zikaron_anchor::judge::Missing::Trouble(t) => t,
                        zikaron_anchor::judge::Missing::Unreadable(_) => zikaron_anchor::rpc::Trouble::Transport("nonce 读不出".into()),
                    };
                    let f = crate::chainx::said_fault(&url, &t);
                    let i = urls.iter().position(|u| u.for_transport() == url).unwrap_or(usize::MAX);
                    words.push((i, f));
                }
                words.sort_by_key(|(i, _)| *i);
                if let Some((_, f)) = words.iter().find(|(_, f)| crate::chainx::next_after(f) == Next::Stop) {
                    return Err(f.clone());
                }
                last = words.pop().map(|(_, f)| f);
            }
            // Taking the maximum, the answers never disagree.
            Err(_) => last = Some(Fault::known(Known::Unreachable, first.clone())),
        }
    }
    let Some((raw, hash, nonce)) = signed else {
        return Err(last.unwrap_or_else(|| Fault::known(Known::NoEndpoint, first)));
    };
    broadcast(urls, chain, &raw, hash, backoff, last).map(|b| Broadcast { nonce: Some(nonce), ..b })
}

/// Sign a stuck batch's resend with higher fees: the same transaction (its nonce, recipient and call data,
/// read back from the chain, `task::Stuck`) signed again with `fees` by the same key; nothing is sent. A
/// different key is refused before anything is signed (`AddressMismatch`). The caller records the hash, then
/// sends the bytes ([`bump_broadcast`]), so a resend that went out is always one the queue watches.
pub fn bump_sign(_pass: &crate::exitgate::Pass, s: &Secret, urls: &[crate::chainx::NodeAddr], stuck: &crate::task::Stuck, fees: zikaron_anchor::send::Fees) -> Result<(Vec<u8>, [u8; 32]), Fault> {
    let first = urls.first().map(|u| u.for_transport().to_string()).unwrap_or_default();
    if s.address().map(|a| a.0) != Some(stuck.from) {
        return Err(Fault::known(Known::AddressMismatch, zikaron::hexfmt::encode(&stuck.from)));
    }
    crate::task::stage_at(crate::task::Kind::Anchor, 0);
    signed_at(s, stuck.chain, Some(stuck.to), &[], Some(stuck.input.clone()), fees, stuck.nonce).map_err(|t| crate::chainx::said_fault(&first, &t))
}

/// Send a signed resend through the single broadcast path ([`broadcast`]), once.
pub fn bump_broadcast(_pass: &crate::exitgate::Pass, urls: &[crate::chainx::NodeAddr], chain: u64, raw: &[u8], hash: [u8; 32], backoff: &[std::time::Duration]) -> Result<Broadcast, Fault> {
    broadcast(urls, chain, raw, hash, backoff, None)
}

/// The only place an anchoring transaction is signed: the key is lent here alone, to `send::sign_at` (a
/// registry call to `registry` with the hashes, or the given call data), at the given nonce.
fn signed_at(s: &Secret, chain: u64, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], input: Option<Vec<u8>>, fees: zikaron_anchor::send::Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), zikaron_anchor::rpc::Trouble> {
    s.with_tx_key(|k| zikaron_anchor::send::sign_at(k, chain, zikaron_anchor::send::Form::Registry, registry, hashes, input, fees, nonce))
}

/// Submit signed bytes endpoint by endpoint (every anchoring transaction goes out here, once per signing):
/// the first endpoint that echoes the hash took it; one that answers "already pending" or "nonce used" is
/// asked by hash whether it knows this very transaction.
fn broadcast(urls: &[crate::chainx::NodeAddr], chain: u64, raw: &[u8], hash: [u8; 32], backoff: &[std::time::Duration], last: Option<Fault>) -> Result<Broadcast, Fault> {
    // The first node in its safe-to-display form, as a refusal's evidence names it.
    let first = urls.first().map(crate::chainx::NodeAddr::said).unwrap_or_default();
    let mut last = last;
    let open = |url: &crate::chainx::NodeAddr| crate::chainx::endpoint_at(url.for_transport()).ok_or_else(|| Fault::known(Known::SettingsShape, crate::chainx::address_said(url.for_transport())));
    use crate::chainx::Next;
    let waits: Vec<std::time::Duration> = std::iter::once(std::time::Duration::ZERO).chain(backoff.iter().copied()).collect();
    // Step 2: submit the same bytes endpoint by endpoint.
    crate::task::stage_at(crate::task::Kind::Anchor, 1);
    let mut asked = 0usize;
    // An earlier endpoint already received these bytes (it answered with a network-kind refusal, and the
    // bytes may be in its pool).
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
            match zikaron_anchor::send::submit(http.as_mut(), raw, &hash) {
                Ok(()) => return Ok(Broadcast { hash, asked, taken: url.clone(), nonce: None }),
                Err(t) => {
                    // What the node said, classified only in `chainx::said_fault`.
                    let f = crate::chainx::said_fault(url.for_transport(), &t);
                    // "Already pending" or "nonce used": ask once by hash; if a node knows the transaction it
                    // was taken (then wait for the receipt), otherwise refuse as before.
                    if crate::chainx::already_taken(&f) && crate::chainx::tx_known(urls, chain, &hash) {
                        return Ok(Broadcast { hash, asked, taken: url.clone(), nonce: None });
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
    // Submitted but no endpoint answered (timeout, disconnect): the bytes may be in some pool. Ask once by
    // hash; known means taken (the receipt decides), unknown means refuse. Refusing outright would make the
    // next pass anchor again with a new nonce, and if the earlier transaction was also included the batch would
    // be anchored twice.
    if handed && crate::chainx::tx_known(urls, chain, &hash) {
        let Some(taken) = urls.first().cloned() else { return Err(last.unwrap_or_else(|| Fault::known(Known::NoEndpoint, first))) };
        return Ok(Broadcast { hash, asked, taken, nonce: None });
    }
    Err(last.unwrap_or_else(|| Fault::known(Known::NoEndpoint, first)))
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
    /// Send an anchoring transaction ([`anchor_send`]). Not a signing domain: it signs a transaction, with
    /// the same key.
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
