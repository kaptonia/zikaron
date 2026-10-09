//! Revocation sentinel: watches the issuer ledger of every vault item. A revocation raises an alarm; a
//! succession adds a yellow note.
//!
//! The sentinel runs no scan of its own. The vault's periodic re-check scans through the anchoring crate and
//! gets the core's report and the kit crate's six checks for every item; this layer only reads the resulting
//! cards. `REVOKED` among the failures raises a revocation alarm, and a succession in the upstream ledger adds
//! the "ledger changed hands" note. The meaning is read on the holder's side (the issuer only writes
//! entries); judging stays with the core and kit crates.
//!
//! Each event for a grant alerts once: alerted keys are stored in settings (`alarmed`), so restarting the app
//! does not alert again. The red and yellow notes on cards still show (they are state, not notifications).

use crate::vaultx::Card;

/// The two kinds of alarm.
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
    /// For a change of hands: the new holder.
    pub to: Option<String>,
    /// Chain time at the pass that found it.
    pub at: Option<u64>,
}

/// Deduplication key: each event for a grant alerts once. For a change of hands the key includes the
/// recipient, so the same ledger changing hands again alerts again (without it, a second handover would never
/// alert).
pub fn key_of(a: &Alarm) -> String {
    match &a.to {
        Some(to) => format!("{}:{}:{}", a.kind.as_str(), a.grant.to_ascii_lowercase(), to.to_ascii_lowercase()),
        None => format!("{}:{}", a.kind.as_str(), a.grant.to_ascii_lowercase()),
    }
}

/// Whom the upstream ledger was handed to by the latest succession after this grant (seq `after`), if any.
/// Reads only entries the core accepted (via `diligx::successions`). Grants signed after a succession were
/// signed by the new key, so that handover gets no note for them.
pub fn handed_after(upstream: &[Vec<u8>], after: u64) -> Option<String> {
    crate::diligx::successions(upstream).iter().filter(|x| x.seq > after).last().map(|x| x.to.clone())
}

/// What to alert after a re-check pass. Looks only at cards; keys already alerted are skipped. Returns (new
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
