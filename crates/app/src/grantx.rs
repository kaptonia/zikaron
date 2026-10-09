//! Grant drafter, and the grant register with the double-sale gate.
//!
//! The drafter decides nothing: a grant's field format follows law §6.3 and is judged by the core's
//! thirteen-step entry check. This module only lays a few cells out as an object; a mistake is refused by
//! the core at once, and the error shown is the spec's token, not a message made up here.

use crate::fault::{Fault, Known};
use crate::home::Home;
use zikaron::json::Value;
use zikaron::tokens::EntryType;

// ───────────────────────── Drafting (law §6.3) ─────────────────────────

/// The form's cells. Empty optional cells are left out of the body (law §6.10: extra members belong to
/// readings; giving fewer is not an error).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub grantee: String,
    pub work: String,
    pub terms: String,
    pub history: String,
    pub from: String,
    pub to: String,
    pub scope_md: String,
    /// The kit reading (treated as data by law §6.10); the page labels it "kit reading, not base grammar".
    pub upstream: String,
}

/// Lays out law §6.3's body. Key names are JSON keys, not spec tokens (as in `entryx`).
///
/// Format checks (hex20 / hex32 / from ≤ to) belong to the core's entry check and are not repeated here.
pub fn grant_body(d: &Draft) -> Result<Value, Fault> {
    let need = |s: &str, what: &str| -> Result<String, Fault> {
        let t = s.trim();
        if t.is_empty() {
            return Err(Fault::known(Known::FieldMissing, what.to_string()));
        }
        Ok(t.to_string())
    };
    let mut body: Vec<(String, Value)> = vec![
        ("grantee".to_string(), Value::Str(need(&d.grantee, "grantee")?)),
        ("terms".to_string(), Value::Str(need(&d.terms, "terms")?)),
        ("work".to_string(), Value::Str(need(&d.work, "work")?)),
    ];
    if !d.history.trim().is_empty() {
        body.push((
            crate::entryx::HISTORY.to_string(),
            Value::Str(d.history.trim().to_string()),
        ));
    }
    if !d.scope_md.trim().is_empty() {
        body.push(("scope_md".to_string(), Value::Str(d.scope_md.to_string())));
    }
    if !d.upstream.trim().is_empty() {
        body.push(("upstream".to_string(), Value::Str(d.upstream.trim().to_string())));
    }
    let (f, t) = (d.from.trim(), d.to.trim());
    match (f.is_empty(), t.is_empty()) {
        (true, true) => {}
        (false, false) => {
            let num = |s: &str, what: &str| -> Result<u64, Fault> {
                s.parse::<u64>().map_err(|_| {
                    Fault::known(Known::SettingsShape, crate::lang::filln(crate::lang::Key::Tail155, &[&(what).to_string(), &format!("{:?}", s)]))
                })
            };
            body.push((
                "window".to_string(),
                Value::Obj(vec![
                    ("from".to_string(), Value::Int(num(f, crate::lang::t(crate::lang::Key::GrantFrom))?)),
                    ("to".to_string(), Value::Int(num(t, crate::lang::t(crate::lang::Key::GrantTo))?)),
                ]),
            ));
        }
        // The window is one object whose two cells are both present or both absent (law §6.3). With only
        // half given, say so at once instead of letting the core report a message about object members.
        _ => {
            return Err(Fault::known(
                Known::FieldMissing,
                crate::lang::t(crate::lang::Key::Tail158).to_string(),
            ))
        }
    }
    body.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Value::Obj(body))
}

/// Drafts a grant's bytes.
///
/// Recording and queueing are separate steps in the action, each with its own status colour.
pub fn draft(
    secret: &crate::key::Secret,
    d: &Draft,
    head: (u64, String),
) -> Result<crate::entryx::Sealed, Fault> {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
    crate::trace::mark(crate::feature::Feature::W7);
    let body = grant_body(d)?;
    crate::entryx::seal(
        secret,
        EntryType::Grant.as_str(),
        head.0 + 1,
        Some(&head.1),
        body,
    )
}

// ───────────────────────── Register and double-sale gate ─────────────────────────

/// One row of the register.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub seq: u64,
    pub grantee: String,
    pub work: String,
    pub terms: String,
    pub window: Option<(u64, u64)>,
    /// Whether this grant carries the local exclusive flag (this machine's bookkeeping, not in the entry
    /// bytes). The double-sale gate reads it.
    pub exclusive: bool,
    /// Where exclusivity comes from: the record made at signing, or the older list in the settings file (no
    /// terms document).
    pub exclusive_from: crate::termsx::Exclusive,
    /// The terms document kept at signing (relative path under `kits`); `None` when there is none.
    pub doc: Option<String>,
    /// The display name of that document (`termsx::Record::shown_name`).
    pub doc_name: Option<String>,
    /// Whether this ledger has a revocation referencing it.
    pub revoked: bool,
}

/// A row's state badge now. Closed set of five.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Badge {
    /// Within the window.
    Live,
    /// Expired.
    Expired,
    /// The window has not opened yet. Kept apart from "expired": calling an exclusive license that starts
    /// next month "expired" would tell the user the opposite of the truth.
    NotYet,
    /// Revoked.
    Revoked,
    /// No reading: no window, or no usable now yet (deadlines use only chain time and an injected now).
    Unknown,
}

impl Badge {
    pub const ALL: [Badge; 5] =
        [Badge::Live, Badge::Expired, Badge::NotYet, Badge::Revoked, Badge::Unknown];

    pub fn as_str(self) -> &'static str {
        match self {
            Badge::Live => "live",
            Badge::Expired => "expired",
            Badge::NotYet => "not_yet",
            Badge::Revoked => "revoked",
            Badge::Unknown => "unknown",
        }
    }
}

impl Row {
    /// This row's badge. `now` is passed in from outside (deadline and window decisions use only chain time
    /// and an injected now); without `now` there is no reading, never a guessed "within the window".
    pub fn badge(&self, now: Option<u64>) -> Badge {
        if self.revoked {
            return Badge::Revoked;
        }
        match (self.window, now) {
            (Some((a, b)), Some(n)) => {
                if n < a {
                    Badge::NotYet
                } else if n <= b {
                    Badge::Live
                } else {
                    Badge::Expired
                }
            }
            _ => Badge::Unknown,
        }
    }
}

fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        _ => None,
    }
}

/// A grant's window (law §6.3); also used for grants in other people's ledgers.
pub fn window_of(body: &Value) -> Option<(u64, u64)> {
    let Value::Obj(m) = body else { return None };
    let (_, w) = m.iter().find(|(k, _)| k == "window")?;
    let Value::Obj(ws) = w else { return None };
    let g = |k: &str| -> Option<u64> {
        ws.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Int(n) => Some(*n),
            _ => None,
        })
    };
    Some((g("from")?, g("to")?))
}

/// Reads the register once: one row per grant in the ledger. A grant counts as revoked when a revocation in
/// this ledger references it (law §6.4: only the issuing ledger can revoke). Exclusivity and documents come
/// from the issuance record (`termsx`, written once at signing); `legacy` is the older exclusive list in the
/// settings file (read-only, kept for data written by older versions).
pub fn table(home: &Home, legacy: &[String]) -> Result<Vec<Row>, Fault> {
    let records = crate::termsx::records(home)?;
    let survey = home
        .ledger()?
        .survey()?;
    let mut entries: Vec<zikaron::entry::Entry> = Vec::new();
    for b in &survey.items {
        if let Ok(e) = zikaron::entry::check(b) {
            entries.push(e);
        }
    }
    let revoked: Vec<String> = entries
        .iter()
        .filter(|e| e.kind == EntryType::Revocation)
        .filter_map(|e| text(&e.body, "grant").map(|s| s.to_string()))
        .collect();
    let mut rows: Vec<Row> = entries
        .iter()
        .filter(|e| e.kind == EntryType::Grant)
        .map(|e| {
            let id = zikaron::hexfmt::encode(&e.id);
            let from = crate::termsx::exclusive_of(&records, legacy, &id);
            Row {
                grantee: text(&e.body, "grantee").unwrap_or("").to_string(),
                work: text(&e.body, "work").unwrap_or("").to_string(),
                terms: text(&e.body, "terms").unwrap_or("").to_string(),
                window: window_of(&e.body),
                exclusive: from != crate::termsx::Exclusive::No,
                exclusive_from: from,
                doc: records.iter().find(|r| r.grant.eq_ignore_ascii_case(&id)).and_then(|r| r.doc.clone()),
                doc_name: records.iter().find(|r| r.grant.eq_ignore_ascii_case(&id)).and_then(|r| r.shown_name().map(str::to_string)),
                revoked: revoked.iter().any(|r| *r == id),
                seq: e.seq,
                id,
            }
        })
        .collect();
    rows.sort_by(|a, b| (b.seq, &a.id).cmp(&(a.seq, &b.id)));
    Ok(rows)
}

/// Filter by grantee or record. Empty means no limit; prefix match, case ignored.
pub fn filter(rows: &[Row], grantee: &str, work: &str) -> Vec<Row> {
    let g = grantee.trim().to_ascii_lowercase();
    let w = work.trim().to_ascii_lowercase();
    rows.iter()
        .filter(|r| g.is_empty() || r.grantee.to_ascii_lowercase().starts_with(&g))
        .filter(|r| w.is_empty() || r.work.to_ascii_lowercase().starts_with(&w))
        .cloned()
        .collect()
}

/// Whether two windows overlap. A side without a window counts as "forever" (law §6.3: what a grant without
/// a window means is up to the terms), so it overlaps anything.
pub fn overlaps(a: Option<(u64, u64)>, b: Option<(u64, u64)>) -> bool {
    match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => a0 <= b1 && b0 <= a1,
        _ => true,
    }
}

/// The double-sale gate: fires only when the record matches, the windows overlap, and the existing grant
/// carries the local exclusive flag.
///
/// The third condition is the limit: terms are a hash, so exclusivity cannot be read from them by machine.
/// The gate enforces the author's own note of exclusivity at issuance, not a spec rule that two grants
/// exclude each other (law §6.3: two grants say nothing about each other). The page says so.
pub fn conflicts(rows: &[Row], work: &str, window: Option<(u64, u64)>) -> Vec<Row> {
    let w = work.trim().to_ascii_lowercase();
    rows.iter()
        .filter(|r| !r.revoked)
        .filter(|r| r.exclusive)
        .filter(|r| r.work.to_ascii_lowercase() == w)
        .filter(|r| overlaps(r.window, window))
        .cloned()
        .collect()
}

// ───────────────────────── Revocation flow ─────────────────────────

/// The body of `revocation` (law §6.4: `grant` required hex32, `case` optional hex32).
///
/// Who issues the `case` is up to the terms (law §6.4: a court judgment, an arbitration award or a case file
/// within the ecosystem all qualify); this module does not judge the issuer and only records the digest. A
/// revocation without `case` says exactly that: revoked, citing no decision.
pub fn revocation_body(grant: &str, case: &str) -> Result<Value, Fault> {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
    crate::trace::mark(crate::feature::Feature::W9);
    let g = grant.trim();
    if g.is_empty() {
        return Err(Fault::known(Known::FieldMissing, "grant".to_string()));
    }
    let mut body = vec![("grant".to_string(), Value::Str(g.to_string()))];
    let c = case.trim();
    if !c.is_empty() {
        body.push(("case".to_string(), Value::Str(c.to_string())));
    }
    body.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Value::Obj(body))
}

/// A grant's story: the grant and the revocations referencing it.
pub struct Story {
    pub grant: Option<Row>,
    /// (revocation id, the decision digest it cites).
    pub revocations: Vec<(String, Option<String>)>,
}

/// Links a grant with its revocations. Only the issuing ledger can revoke (law §6.4), so only this ledger
/// is searched.
pub fn story(home: &Home, grant_id: &str) -> Result<Story, Fault> {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
    crate::trace::mark(crate::feature::Feature::W9);
    let want = grant_id.trim().to_string();
    let survey = home
        .ledger()?
        .survey()?;
    let mut revocations: Vec<(String, Option<String>)> = Vec::new();
    for b in &survey.items {
        let Ok(e) = zikaron::entry::check(b) else { continue };
        if e.kind != EntryType::Revocation {
            continue;
        }
        if text(&e.body, "grant") != Some(want.as_str()) {
            continue;
        }
        revocations.push((
            zikaron::hexfmt::encode(&e.id),
            text(&e.body, "case").map(|s| s.to_string()),
        ));
    }
    revocations.sort();
    let rows = table(home, &[])?;
    Ok(Story { grant: rows.into_iter().find(|r| r.id == want), revocations })
}

/// Whether the draft form may be signed: grantee, record and terms fingerprint are present, none of the
/// three hex cells mixes cases (the core refuses checksum addresses by name), and a preset validity has chain
/// time to count from. Without chain time a preset has no window, and signing would write a grant with no
/// window at all, which is not what someone who chose "30 days" asked for.
pub fn draft_ready(grantee: &str, work: &str, terms: &str, upstream: &str, preset_days: u32, chain_now: Option<u64>) -> bool {
    let mixed_case = [grantee, terms, upstream].iter().any(|x| x.trim().strip_prefix("0x").map(|h| h.bytes().any(|b| b.is_ascii_uppercase())).unwrap_or(false));
    let preset_unfilled = preset_days > 0 && chain_now.is_none();
    !grantee.trim().is_empty() && !work.trim().is_empty() && !terms.trim().is_empty() && !mixed_case && !preset_unfilled
}

#[cfg(test)]
mod draft_ready_tests {
    use super::draft_ready;

    const G: &str = "0x2da333b436f845d65c9a9d7bea9a819062e3c2ff";
    const W: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
    const T: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";

    #[test]
    fn a_preset_without_chain_time_cannot_be_signed() {
        assert!(!draft_ready(G, W, T, "", 30, None));
        assert!(!draft_ready(G, W, T, "", 7, None));
    }

    #[test]
    fn a_preset_with_chain_time_can_be_signed() {
        assert!(draft_ready(G, W, T, "", 30, Some(1_790_000_000)));
    }

    #[test]
    fn custom_validity_does_not_need_chain_time() {
        assert!(draft_ready(G, W, T, "", 0, None));
    }

    #[test]
    fn the_other_conditions_still_hold() {
        assert!(!draft_ready("", W, T, "", 0, Some(1)));
        assert!(!draft_ready(G, "", T, "", 0, Some(1)));
        assert!(!draft_ready(G, W, "", "", 0, Some(1)));
        assert!(!draft_ready("0x2dA333B436F845D65C9a9D7beA9A819062e3c2ff", W, T, "", 0, Some(1)));
        assert!(!draft_ready(G, W, T, "0xABC", 0, Some(1)));
    }
}
