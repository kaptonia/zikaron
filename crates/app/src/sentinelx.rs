//! Revocation sentinel. Periodically scan the issuer ledger of every vault item; a revocation raises an
//! alarm, a succession a yellow note.
//!
//! ─── It reads the vault re-check ───
//!
//! The sentinel starts no scan of its own: the vault's periodic re-check scans the basis through
//! the anchoring crate, gets the core's report and the kit crate's six checks for every item; this layer only
//! reads that pass's cards: REVOKED among the failures raises a revocation alarm, and a succession in the
//! upstream ledger adds the "ledger changed hands" yellow note. The sentinel's semantics are read on this
//! side (the other side only writes entries); judging stays with the core and kit crates.
//!
//! ─── Notifications with hysteresis and deduplication ───
//!
//! The same event for the same grant alerts once: alerted keys are recorded in settings (`alarmed`), so
//! closing and reopening the app does not alert again; the red and yellow notes on cards still show (that is
//! state, not notification).

use crate::vaultx::Card;

/// The two kinds of alarm. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A revocation referencing a grant I hold has appeared.
    Revoked,
    /// The issuer's ledger changed hands (succession).
    Handed,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Revoked => "revoked",
            Kind::Handed => "handed",
        }
    }
}

/// One alarm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alarm {
    pub grant: String,
    pub author: String,
    pub kind: Kind,
    /// Whom it was handed to, for a change of hands.
    pub to: Option<String>,
    /// The chain's time at the pass that found it.
    pub at: Option<u64>,
}

/// The deduplication key: the same event for the same grant alerts once. A revocation is one event; the
/// "event" of a change of hands is whom it went to: the same ledger changing hands again is another event and
/// alerts again (a key without the recipient would leave the second change of hands forever silent).
pub fn key_of(a: &Alarm) -> String {
    match &a.to {
        Some(to) => format!("{}:{}:{}", a.kind.as_str(), a.grant.to_ascii_lowercase(), to.to_ascii_lowercase()),
        None => format!("{}:{}", a.kind.as_str(), a.grant.to_ascii_lowercase()),
    }
}

/// Whether the upstream ledger changed hands after this grant: whom the latest succession after this grant
/// (seq `after`) handed to. Reads entries the core accepted (through `diligx::successions`). Grants signed
/// after the succession were signed by the new key itself, so that handover is not an event for them and gets
/// no note.
pub fn handed_after(upstream: &[Vec<u8>], after: u64) -> Option<String> {
    crate::diligx::successions(upstream).iter().filter(|x| x.seq > after).last().map(|x| x.to.clone())
}

/// What to alert after a re-check pass. Looks only at cards; alerted keys do not alert again. Returns (new
/// alarms, their keys).
pub fn alarms(cards: &[Card], already: &[String], now: Option<u64>) -> (Vec<Alarm>, Vec<String>) {
    crate::trace::mark(crate::feature::Feature::D7);
    let mut out: Vec<Alarm> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for c in cards {
        let mut found: Vec<Alarm> = Vec::new();
        if c.failed.iter().any(|x| x == zikaron_kit::tokens::Check::Revoked.as_str()) {
            found.push(Alarm { grant: c.id.clone(), author: c.author.clone(), kind: Kind::Revoked, to: None, at: now });
        }
        if let Some(to) = &c.handed {
            found.push(Alarm { grant: c.id.clone(), author: c.author.clone(), kind: Kind::Handed, to: Some(to.clone()), at: now });
        }
        for a in found {
            let k = key_of(&a);
            if already.iter().any(|x| *x == k) || keys.iter().any(|x| *x == k) {
                continue;
            }
            keys.push(k);
            out.push(a);
        }
    }
    (out, keys)
}
