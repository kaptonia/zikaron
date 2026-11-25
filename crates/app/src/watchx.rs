use crate::shell::Page;
use zikaron::json::Value;

pub const SOON_SECS: u64 = 7 * 86_400;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Seat {
    Author,
    Grantee,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Unanchored,
    QueueBacklog,
    WindowExpiring,
    AuditLabel,
    HoldingExpiring,
    Revoked,
    Handed,
    UpstreamRed,
    Backup,
}

impl Item {
    pub const AUTHOR: [Item; 5] = [Item::Unanchored, Item::QueueBacklog, Item::WindowExpiring, Item::Backup, Item::AuditLabel];
    pub const GRANTEE: [Item; 5] = [Item::HoldingExpiring, Item::Revoked, Item::Backup, Item::Handed, Item::UpstreamRed];

    pub fn as_str(self) -> &'static str {
        match self {
            Item::Unanchored => "unanchored",
            Item::QueueBacklog => "queue_backlog",
            Item::WindowExpiring => "window_expiring",
            Item::Backup => "backup",
            Item::AuditLabel => "audit_label",
            Item::HoldingExpiring => "holding_expiring",
            Item::Revoked => "revoked",
            Item::Handed => "handed",
            Item::UpstreamRed => "upstream_red",
        }
    }

    pub fn seat(self) -> Seat {
        if self != Item::Backup && Item::AUTHOR.contains(&self) {
            Seat::Author
        } else {
            Seat::Grantee
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Light {
    Ok,
    Warn,
    Bad,
    Unknown,
}

impl Light {
    pub fn as_str(self) -> &'static str {
        match self {
            Light::Ok => "ok",
            Light::Warn => "warn",
            Light::Bad => "bad",
            Light::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Say {
    UnanchoredRead,
    Unanchored,
    Queue,
    WindowRead,
    NoChainTime,
    Window,
    BackupRead,
    BackupNever,
    BackupBehind,
    BackupFailed,
    AuditRun,
    AuditGaps,
    AuditUnavailable,
    AuditBroken,
    HoldingRead,
    HoldingNoNow,
    HoldingExpired,
    HoldingSoon,
    Revoked,
    Handed,
    UpstreamRed,
}

impl Say {
    pub fn as_str(self) -> &'static str {
        match self {
            Say::UnanchoredRead => "unanchored_read",
            Say::Unanchored => "unanchored",
            Say::Queue => "queue",
            Say::WindowRead => "window_read",
            Say::NoChainTime => "no_chain_time",
            Say::Window => "window",
            Say::BackupRead => "backup_read",
            Say::BackupNever => "backup_never",
            Say::BackupBehind => "backup_behind",
            Say::BackupFailed => "backup_failed",
            Say::AuditRun => "audit_run",
            Say::AuditGaps => "audit_gaps",
            Say::AuditUnavailable => "audit_unavailable",
            Say::AuditBroken => "audit_broken",
            Say::HoldingRead => "holding_read",
            Say::HoldingNoNow => "holding_no_now",
            Say::HoldingExpired => "holding_expired",
            Say::HoldingSoon => "holding_soon",
            Say::Revoked => "revoked",
            Say::Handed => "handed",
            Say::UpstreamRed => "upstream_red",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gap {
    pub say: Say,
    pub n: u64,
    pub secs: Option<u64>,
    pub go: Page,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Episode {
    Until(u64),
    At(String),
    Head(String),
}

impl Episode {
    pub fn kind(&self) -> &'static str {
        match self {
            Episode::Until(_) => "until",
            Episode::At(_) => "at",
            Episode::Head(_) => "head",
        }
    }

    pub fn value(&self) -> String {
        match self {
            Episode::Until(n) => n.to_string(),
            Episode::At(id) | Episode::Head(id) => id.to_ascii_lowercase(),
        }
    }

    pub fn as_key(&self) -> String {
        format!("{}-{}", self.kind(), self.value())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ring {
    pub subject: String,
    pub episode: Episode,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub item: Item,
    pub light: Light,
    pub n: u64,
    pub detail: String,
    pub gap: Option<Gap>,
    pub rings: Vec<Ring>,
}

pub fn needing_attention(rows: Vec<Row>) -> Vec<Row> {
    rows.into_iter().filter(|r| r.item != Item::Backup && matches!(r.light, Light::Warn | Light::Bad)).collect()
}

fn row(item: Item, light: Light, n: u64, detail: String, gap: Option<Gap>) -> Row {
    Row { item, light, n, detail, gap, rings: Vec::new() }
}

fn broken_at(report: &Value) -> String {
    let mut ids: Vec<String> = crate::auditx::rows_of(report, zikaron::v2::tokens::Key::Findings)
        .iter()
        .filter(|r| matches!(member(r, zikaron::v2::tokens::Key::Hard.as_str()), Some(Value::Bool(true))))
        .filter_map(|r| match member(r, zikaron::v2::tokens::Key::EntryId.as_str()) {
            Some(Value::Str(x)) => Some(x.to_ascii_lowercase()),
            _ => None,
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids.join("+")
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

fn table_len(report: &Value, k: zikaron::v2::tokens::Key) -> u64 {
    crate::auditx::rows_of(report, k).len() as u64
}

fn backup_row(backup: Option<&crate::machine::Backed>, items_now: Option<u64>, failed: Option<u64>) -> Row {
    let gap = |say: Say, n: u64| Some(Gap { say, n, secs: None, go: Page::Archive });
    if let Some(at) = failed {
        let mut r = row(Item::Backup, Light::Bad, 1, at.to_string(), gap(Say::BackupFailed, 1));
        r.rings = vec![Ring { subject: "backup".to_string(), episode: Episode::At(format!("failed {at}")) }];
        return r;
    }
    match (backup, items_now) {
        (None, _) => {
            let mut r = row(Item::Backup, Light::Unknown, 0, String::new(), gap(Say::BackupNever, 1));
            r.rings = vec![Ring { subject: "backup".to_string(), episode: Episode::At("never".to_string()) }];
            r
        }
        (Some(_), None) => row(Item::Backup, Light::Unknown, 0, String::new(), gap(Say::BackupRead, 1)),
        (Some(b), Some(_)) => {
            let behind = crate::machine::backup_behind(backup, items_now).unwrap_or(0);
            let mut r = row(Item::Backup, if behind == 0 { Light::Ok } else { Light::Warn }, behind, b.at.to_string(), if behind > 0 { gap(Say::BackupBehind, behind) } else { None });
            if behind > 0 {
                r.rings = vec![Ring { subject: "backup".to_string(), episode: Episode::At(b.at.to_string()) }];
            }
            r
        }
    }
}

pub fn unqueued(rows: Option<&[crate::ledgerx::Row]>, queued: usize) -> Option<u64> {
    let r = rows?;
    let d = crate::queue::density(Some(r), queued);
    let uncovered = |x: &&crate::ledgerx::Row| !d.covers(x);
    Some(r.iter().filter(uncovered).filter(|x| x.lamp == crate::ledgerx::Lamp::Landed).count() as u64)
}

pub fn author(
    rows: Option<&[crate::ledgerx::Row]>,
    queued: usize,
    grants: Option<&[crate::grantx::Row]>,
    backup: (Option<&crate::machine::Backed>, Option<u64>, Option<u64>),
    audit: Option<(&str, &Value, &[String])>,
    verdict: Option<crate::shell::Verdict>,
    now: Option<u64>,
) -> Vec<Row> {
    crate::trace::mark(crate::feature::Feature::P2);
    let gap = |say: Say, n: u64, secs: Option<u64>, go: Page| Some(Gap { say, n, secs, go });
    let mut out: Vec<Row> = Vec::new();
    out.push(match rows {
        None => row(Item::Unanchored, Light::Unknown, 0, String::new(), gap(Say::UnanchoredRead, 1, None, Page::Ledger)),
        Some(r) => {
            let d = crate::queue::density(Some(r), queued);
            let landed = unqueued(Some(r), queued).unwrap_or(0);
            row(
                Item::Unanchored,
                if d.behind == 0 { Light::Ok } else { Light::Warn },
                d.behind as u64,
                format!("{} / {}", d.anchored_through.map(|x| x.to_string()).unwrap_or_default(), d.head_seq.map(|x| x.to_string()).unwrap_or_default()),
                if d.behind == 0 {
                    None
                } else if landed > 0 {
                    gap(Say::Unanchored, landed, None, Page::Anchoring)
                } else if queued > 0 {
                    gap(Say::Queue, queued as u64, None, Page::Queue)
                } else {
                    gap(Say::Queue, d.behind as u64, None, Page::Queue)
                },
            )
        }
    });
    out.push(row(
        Item::QueueBacklog,
        if queued == 0 { Light::Ok } else { Light::Warn },
        queued as u64,
        String::new(),
        if queued > 0 { gap(Say::Queue, queued as u64, None, Page::Queue) } else { None },
    ));
    out.push(match (grants, now) {
        (None, _) => row(Item::WindowExpiring, Light::Unknown, 0, String::new(), gap(Say::WindowRead, 1, None, Page::Grants)),
        (Some(_), None) => row(Item::WindowExpiring, Light::Unknown, 0, String::new(), gap(Say::NoChainTime, 1, None, Page::Audit)),
        (Some(g), Some(t)) => {
            let soon: Vec<(String, u64)> = g
                .iter()
                .filter(|x| !x.revoked)
                .filter_map(|x| x.window.map(|(_, to)| (x.id.clone(), to)))
                .filter(|(_, to)| *to > t && *to - t <= SOON_SECS)
                .collect();
            let nearest = soon.iter().map(|(_, to)| *to - t).min();
            let rings: Vec<Ring> = soon.iter().map(|(id, to)| Ring { subject: id.clone(), episode: Episode::Until(*to) }).collect();
            let mut r = row(
                Item::WindowExpiring,
                if soon.is_empty() { Light::Ok } else { Light::Warn },
                soon.len() as u64,
                nearest.map(|s| s.to_string()).unwrap_or_default(),
                if soon.is_empty() { None } else { gap(Say::Window, soon.len() as u64, nearest, Page::Grant) },
            );
            r.rings = rings;
            r
        }
    });
    out.push(backup_row(backup.0, backup.1, backup.2));
    out.push(match (audit, verdict) {
        (None, _) => row(Item::AuditLabel, Light::Unknown, 0, String::new(), gap(Say::AuditRun, 1, None, Page::Audit)),
        (Some((label, _, _)), Some(crate::shell::Verdict::HandedOn(_))) => row(Item::AuditLabel, Light::Ok, 0, label.to_string(), None),
        (Some((label, report, unanswered)), Some(crate::shell::Verdict::Normal | crate::shell::Verdict::Abnormal) | None) => {
            use zikaron::v2::tokens::Label;
            if label == Label::Complete.as_str() {
                row(Item::AuditLabel, Light::Ok, 0, label.to_string(), None)
            } else if label == Label::Gaps.as_str() {
                let n = table_len(report, zikaron::v2::tokens::Key::Missing);
                row(Item::AuditLabel, Light::Bad, n, label.to_string(), gap(Say::AuditGaps, n.max(1), None, Page::Audit))
            } else if label == Label::Unproven.as_str() {
                let n = unanswered.len() as u64;
                row(Item::AuditLabel, Light::Warn, n, label.to_string(), gap(Say::AuditUnavailable, n.max(1), None, Page::Identity))
            } else {
                let mut r = row(Item::AuditLabel, Light::Bad, 1, label.to_string(), gap(Say::AuditBroken, 1, None, Page::Mirror));
                r.rings = vec![Ring { subject: label.to_string(), episode: Episode::At(broken_at(report)) }];
                r
            }
        }
    });
    out
}

pub fn grantee(
    cards: Option<&[crate::vaultx::Card]>,
    alarms: &[crate::sentinelx::Alarm],
    now: Option<u64>,
    backup: (Option<&crate::machine::Backed>, Option<u64>, Option<u64>),
) -> Vec<Row> {
    crate::trace::mark(crate::feature::Feature::P2);
    let gap = |say: Say, n: u64, secs: Option<u64>, go: Page| Some(Gap { say, n, secs, go });
    let mut out: Vec<Row> = Vec::new();
    out.push(match cards {
        None => row(Item::HoldingExpiring, Light::Unknown, 0, String::new(), gap(Say::HoldingRead, 1, None, Page::Vault)),
        Some(c) if c.is_empty() => row(Item::HoldingExpiring, Light::Ok, 0, String::new(), None),
        Some(c) => {
            let _ = now;
            let mut soon: Vec<(String, u64)> = Vec::new();
            let mut expired: Vec<(String, u64)> = Vec::new();
            let mut nearest: Option<u64> = None;
            let mut unknown = 0u64;
            for x in c {
                match (&x.countdown, x.window) {
                    (crate::vaultx::Countdown::Live { remaining }, Some((_, to))) if *remaining <= SOON_SECS => {
                        soon.push((x.id.clone(), to));
                        nearest = Some(nearest.map(|n| n.min(*remaining)).unwrap_or(*remaining));
                    }
                    (crate::vaultx::Countdown::Expired { .. }, Some((_, to))) => expired.push((x.id.clone(), to)),
                    (crate::vaultx::Countdown::NoNow, _) => unknown += 1,
                    _ => {}
                }
            }
            let mut rings: Vec<Ring> = soon.iter().chain(expired.iter()).map(|(id, to)| Ring { subject: id.clone(), episode: Episode::Until(*to) }).collect();
            rings.dedup_by(|a, b| a == b);
            let mut r = if !expired.is_empty() {
                row(Item::HoldingExpiring, Light::Bad, expired.len() as u64, String::new(), gap(Say::HoldingExpired, expired.len() as u64, None, Page::Relicense))
            } else if !soon.is_empty() {
                row(Item::HoldingExpiring, Light::Warn, soon.len() as u64, nearest.map(|s| s.to_string()).unwrap_or_default(), gap(Say::HoldingSoon, soon.len() as u64, nearest, Page::Relicense))
            } else if unknown > 0 {
                row(Item::HoldingExpiring, Light::Unknown, unknown, String::new(), gap(Say::HoldingNoNow, unknown, None, Page::Sentinel))
            } else {
                row(Item::HoldingExpiring, Light::Ok, 0, String::new(), None)
            };
            r.rings = rings;
            r
        }
    });
    let revoked = alarms.iter().filter(|a| a.kind == crate::sentinelx::Kind::Revoked).count() as u64;
    out.push(row(
        Item::Revoked,
        if revoked == 0 { Light::Ok } else { Light::Bad },
        revoked,
        String::new(),
        if revoked > 0 { gap(Say::Revoked, revoked, None, Page::Vault) } else { None },
    ));
    out.push(backup_row(backup.0, backup.1, backup.2));
    let handed = alarms.iter().filter(|a| a.kind == crate::sentinelx::Kind::Handed).count() as u64;
    out.push(row(
        Item::Handed,
        if handed == 0 { Light::Ok } else { Light::Warn },
        handed,
        String::new(),
        if handed > 0 { gap(Say::Handed, handed, None, Page::Upstreams) } else { None },
    ));
    out.push(match cards {
        None => row(Item::UpstreamRed, Light::Unknown, 0, String::new(), gap(Say::HoldingRead, 1, None, Page::Vault)),
        Some(c) => {
            let mut authors: Vec<Ring> = c
                .iter()
                .filter(|x| x.failed.iter().any(|f| f == zikaron_kit::tokens::Check::BrokenLedger.as_str()))
                .map(|x| Ring {
                    subject: x.author.to_ascii_lowercase(),
                    episode: Episode::Head(x.upstream_head.clone().unwrap_or_default()),
                })
                .collect();
            authors.sort_by(|a, b| a.subject.cmp(&b.subject));
            authors.dedup_by(|a, b| a.subject == b.subject);
            let n = authors.len() as u64;
            let mut r = row(
                Item::UpstreamRed,
                if n == 0 { Light::Ok } else { Light::Bad },
                n,
                String::new(),
                if n > 0 { gap(Say::UpstreamRed, n, None, Page::Upstreams) } else { None },
            );
            r.rings = authors;
            r
        }
    });
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub key: String,
    pub item: Item,
    pub subject: String,
    pub episode: Episode,
}

pub fn notices(rows: &[Row]) -> Vec<Notice> {
    let mut out: Vec<Notice> = Vec::new();
    for r in rows {
        for ring in &r.rings {
            out.push(Notice {
                key: format!("p2:{}:{}:{}", r.item.as_str(), ring.subject.to_ascii_lowercase(), ring.episode.as_key()),
                item: r.item,
                subject: ring.subject.clone(),
                episode: ring.episode.clone(),
            });
        }
    }
    out
}

pub fn to_ring(rows: Vec<Row>) -> Vec<Notice> {
    notices(&needing_attention(rows))
}

pub fn fresh(all: Vec<Notice>, already: &[String]) -> (Vec<Notice>, Vec<String>) {
    let mut out: Vec<Notice> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for n in all {
        if already.iter().any(|k| *k == n.key) || keys.iter().any(|k| *k == n.key) {
            continue;
        }
        keys.push(n.key.clone());
        out.push(n);
    }
    (out, keys)
}

pub fn is_network(f: &crate::fault::Fault) -> bool {
    use crate::fault::Known;
    let head = f.said().split(':').next().unwrap_or("");
    Known::NETWORK.iter().any(|k| k.as_str() == head)
}
