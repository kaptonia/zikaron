use crate::fault::{Fault, Known};
use crate::home::Home;
use zikaron::json::Value;
use zikaron_glue::book::Kind as EntryType;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub grantee: String,
    pub work: String,
    pub terms: String,
    pub history: String,
    pub from: String,
    pub to: String,
    pub scope_md: String,
    pub upstream: String,
    pub ledger: String,
    pub revocation: Option<crate::sendx::Member>,
    pub supersedes: String,
}

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
    if !d.ledger.trim().is_empty() {
        body.push(("ledger".to_string(), Value::Str(d.ledger.trim().to_ascii_lowercase())));
    }
    if let Some(m) = d.revocation {
        body.push((zikaron_kit::v2::tokens::Key::Revocation.as_str().to_string(), m.value()));
    }
    if !d.supersedes.trim().is_empty() {
        body.push(("supersedes".to_string(), Value::Str(d.supersedes.trim().to_ascii_lowercase())));
    }
    let (f, t) = (d.from.trim(), d.to.trim());
    match (f.is_empty(), t.is_empty()) {
        (true, true) => {}
        (false, false) => {
            let num = |s: &str, what: &str| -> Result<u64, Fault> {
                s.parse::<u64>().map_err(|_| {
                    Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail155, &[&(what).to_string(), &format!("{:?}", s)])))
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
        _ => {
            return Err(Fault::known(
                Known::FieldMissing,
                crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail158).to_string()),
            ))
        }
    }
    body.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Value::Obj(body))
}

pub fn draft(
    secret: &crate::key::Secret,
    d: &Draft,
    head: (u64, String),
    ledger: &str,
) -> Result<crate::entryx::Sealed, Fault> {
    crate::trace::mark(crate::feature::Feature::W7);
    let body = grant_body(&Draft { ledger: ledger.to_string(), ..d.clone() })?;
    crate::entryx::seal(
        secret,
        EntryType::Grant.as_str(),
        head.0 + 1,
        Some(&head.1),
        body,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub seq: u64,
    pub grantee: String,
    pub work: String,
    pub terms: String,
    pub window: Option<(u64, u64)>,
    pub exclusive: bool,
    pub exclusive_from: crate::termsx::Exclusive,
    pub doc: Option<String>,
    pub doc_name: Option<String>,
    pub revoked: bool,
    pub unnoticed: bool,
    pub member: bool,
    pub superseded: bool,
    pub undecided: Option<Undecided>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Undecided {
    Unread(u64, String),
    Uncounted(u64),
    NotApplying(Option<u64>),
}

impl Undecided {
    pub fn code(&self) -> &'static str {
        match self {
            Undecided::Unread(..) => "unread",
            Undecided::Uncounted(_) => "uncounted",
            Undecided::NotApplying(_) => "not-applying",
        }
    }

    pub fn chain(&self) -> Option<u64> {
        match self {
            Undecided::Unread(c, _) => Some(*c),
            Undecided::Uncounted(c) => Some(*c),
            Undecided::NotApplying(c) => *c,
        }
    }

    pub fn words(&self) -> String {
        let on = |key: crate::lang::Key, c: Option<u64>| match c {
            Some(c) => format!("{} {c}", crate::lang::t(key)),
            None => crate::lang::t(key).to_string(),
        };
        match self {
            Undecided::Unread(_, w) => w.clone(),
            Undecided::Uncounted(c) => on(crate::lang::Key::NoticesUncounted, Some(*c)),
            Undecided::NotApplying(c) => on(crate::lang::Key::NoticesNotApplying, *c),
        }
    }
}

impl Row {
    pub fn migratable(&self, now: Option<u64>) -> bool {
        let ended = matches!((self.window, now), (Some((_, to)), Some(n)) if n > to);
        !self.member && !self.superseded && !self.revoked && !self.unnoticed && !ended
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Badge {
    Live,
    Expired,
    NotYet,
    Revoked,
    Unnoticed,
    Unknown,
}

impl Badge {
    pub const ALL: [Badge; 6] =
        [Badge::Live, Badge::Expired, Badge::NotYet, Badge::Revoked, Badge::Unnoticed, Badge::Unknown];

    pub fn as_str(self) -> &'static str {
        match self {
            Badge::Live => "live",
            Badge::Expired => "expired",
            Badge::NotYet => "not_yet",
            Badge::Revoked => "revoked",
            Badge::Unnoticed => "unnoticed",
            Badge::Unknown => "unknown",
        }
    }
}

impl Row {
    pub fn badge(&self, now: Option<u64>) -> Badge {
        if self.revoked {
            return Badge::Revoked;
        }
        if self.unnoticed {
            return Badge::Unnoticed;
        }
        if self.undecided.is_some() {
            return Badge::Unknown;
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

pub fn window_of(body: &Value) -> Option<(u64, u64)> {
    zikaron_seal::terms::window_of(body)
}

pub fn table(home: &Home) -> Result<Vec<Row>, Fault> {
    let records = crate::termsx::records(home)?;
    let survey = home
        .ledger()?
        .survey()?;
    let standing = crate::sendx::standing(&survey.items, &crate::sendx::record::noticed());
    let revocations_of = |items: &[Vec<u8>]| -> Vec<String> {
        items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).filter(|e| e.kind == EntryType::Revocation).filter_map(|e| text(&e.body, "grant").map(|s| s.to_string())).collect()
    };
    let revoked = revocations_of(&standing);
    let mut entries: Vec<zikaron_glue::book::Entry> = Vec::new();
    for b in &survey.items {
        if let Ok(e) = zikaron_glue::book::check(b) {
            entries.push(e);
        }
    }
    let superseded: Vec<String> = entries.iter().filter(|e| e.kind == EntryType::Grant && e.body.member(zikaron_kit::v2::tokens::Key::Revocation.as_str()).is_some()).filter_map(|e| text(&e.body, "supersedes").map(str::to_ascii_lowercase)).collect();
    let mut rows: Vec<Row> = entries
        .iter()
        .filter(|e| e.kind == EntryType::Grant)
        .map(|e| {
            let id = zikaron::hexfmt::encode(&e.id);
            let from = crate::termsx::exclusive_of(&records, &id);
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
                unnoticed: !revoked.iter().any(|r| *r == id) && !crate::sendx::unsettled(&survey.items, std::slice::from_ref(&id)).is_empty(),
                member: e.body.member(zikaron_kit::v2::tokens::Key::Revocation.as_str()).is_some(),
                superseded: superseded.contains(&id),
                undecided: None,
                seq: e.seq,
                id,
            }
        })
        .collect();
    rows.sort_by(|a, b| (b.seq, &a.id).cmp(&(a.seq, &b.id)));
    Ok(rows)
}

pub fn filter(rows: &[Row], grantee: &str, work: &str) -> Vec<Row> {
    let g = grantee.trim().to_ascii_lowercase();
    let w = work.trim().to_ascii_lowercase();
    rows.iter()
        .filter(|r| g.is_empty() || r.grantee.to_ascii_lowercase().starts_with(&g))
        .filter(|r| w.is_empty() || r.work.to_ascii_lowercase().starts_with(&w))
        .cloned()
        .collect()
}

pub fn overlaps(a: Option<(u64, u64)>, b: Option<(u64, u64)>) -> bool {
    zikaron_seal::terms::overlaps(a, b)
}

pub fn conflicts(rows: &[Row], work: &str, window: Option<(u64, u64)>) -> Vec<Row> {
    let held: Vec<zikaron_seal::terms::Held> = rows.iter().map(|r| zikaron_seal::terms::Held { id: r.id.clone(), work: r.work.clone(), window: r.window, revoked: r.revoked, exclusive: r.exclusive }).collect();
    let ids = zikaron_seal::terms::clashes(&held, work, window);
    rows.iter().filter(|r| ids.contains(&r.id)).cloned().collect()
}

pub fn revocation_body(grant: &str, case: &str) -> Result<Value, Fault> {
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

pub struct Story {
    pub grant: Option<Row>,
    pub revocations: Vec<(String, Option<String>)>,
}

pub fn story(home: &Home, grant_id: &str) -> Result<Story, Fault> {
    crate::trace::mark(crate::feature::Feature::W9);
    let want = grant_id.trim().to_string();
    let survey = home
        .ledger()?
        .survey()?;
    let mut revocations: Vec<(String, Option<String>)> = Vec::new();
    for b in &survey.items {
        let Ok(e) = zikaron_glue::book::check(b) else { continue };
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
    let rows = table(home)?;
    Ok(Story { grant: rows.into_iter().find(|r| r.id == want), revocations })
}

pub fn draft_ready(grantee: &str, work: &str, terms: &str, upstream: &str, preset_days: u32, chain_now: Option<u64>) -> bool {
    let mixed_case = [grantee, terms, upstream].iter().any(|x| x.trim().strip_prefix("0x").map(|h| h.bytes().any(|b| b.is_ascii_uppercase())).unwrap_or(false));
    let preset_unfilled = preset_days > 0 && chain_now.is_none();
    !grantee.trim().is_empty() && !grantee_shape_bad(grantee) && !work.trim().is_empty() && !terms.trim().is_empty() && !mixed_case && !preset_unfilled
}

pub fn grantee_shape_bad(grantee: &str) -> bool {
    let g = grantee.trim();
    let mixed = g.strip_prefix("0x").is_some_and(|h| h.bytes().any(|b| b.is_ascii_uppercase()));
    !mixed && crate::addressedx::not_address(g, crate::addressedx::Takes::Address)
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

    #[test]
    fn a_grantee_that_is_no_address_is_said_and_not_signed() {
        for bad in ["0x12zz", "0x2da333b436f845d65c9a9d7bea9a819062e3c2fz", "0x2da333b436f845d65c9a9d7bea9a819062e3c2", "0x2da333b436f845d65c9a9d7bea9a819062e3c2ff00", "2da333b436f845d65c9a9d7bea9a819062e3c2ff"] {
            assert!(super::grantee_shape_bad(bad), "{bad}");
            assert!(!draft_ready(bad, W, T, "", 0, Some(1)), "{bad}");
        }
        assert!(!super::grantee_shape_bad(G));
        assert!(!super::grantee_shape_bad(""));
        assert!(!super::grantee_shape_bad("0x2dA333B436F845D65C9a9D7beA9A819062e3c2ff"));
    }
}
