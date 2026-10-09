//! Watch and notifications: every row of both seats' watch tables; notifications with deduplication, one
//! alert per deadline; the status line.
//!
//! This layer touches no disk and no network: every row is computed from readings the shell already has
//! (ledger table, queue, grant table, mirror state, self-audit report, cards, sentinel alarms), each owned
//! elsewhere (ledger view, anchoring queue, grant register, mirror and restore, self-audit clock, grant vault,
//! revocation sentinel). A gap comes with an action: a row does not only light a lamp, it carries an action
//! with a number and the page to go to. A row with no reading says "not read yet" and points to the page that
//! can read it, never passing grey off as green.
//!
//! The two expiry rows (the author's window, the grantee's holding) use only the chain's current time:
//! without it they say "no chain time yet" and point to the pass that can fetch it (self-audit, re-check),
//! never falling back to the local clock.
//!
//! Each row that should alert has a key (`p2:<row>:<subject>:<episode>`), and alerted keys go into the
//! settings' `alarmed` (the revocation sentinel's record, not a separate one): the same deadline alerts once,
//! and a changed deadline (renewal, new grant) is a new key and alerts again. Revocation and succession alerts
//! belong to the revocation sentinel and are not repeated here. Network errors are not notifications: they go
//! to the status line (`Shell::status`).

use crate::shell::Page;
use zikaron::json::Value;

/// The expiry threshold in seconds: closer than this to the deadline counts as soon.
pub const SOON_SECS: u64 = 7 * 86_400;

/// Which seat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Seat {
    Author,
    Grantee,
}

/// Watch table rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    // Author seat
    Unanchored,
    QueueBacklog,
    WindowExpiring,
    AuditLabel,
    // Grantee seat
    HoldingExpiring,
    Revoked,
    Handed,
    UpstreamRed,
    // Both seats: the whole-machine backup (never written, or ledger entries and held grants came after it).
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

/// Lights. The same four states as the widget library's `Dot`, but this layer does not depend on the library
/// (it is a reading, not a drawing).
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

/// The form of an action sentence. Each row picks its sentence from its own state, and the UI only turns it
/// into text (key table). A UI guessing the sentence from (row, light, number) would read "a mirror exists but
/// is one entry behind" as "no mirror yet".
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
    /// The sentence's name; the UI assembles the text from the key table.
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

/// A gap's action: which sentence, a number, optional seconds, the page to go to. The UI assembles the text
/// from the key table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gap {
    pub say: Say,
    pub n: u64,
    pub secs: Option<u64>,
    pub go: Page,
}

/// The identity of an episode: the part of the alert key that says which occurrence this is. An episode
/// alerts once; a new episode alerts again. Each row defines its own episode identity, and keys are assembled
/// only in [`notices`].
///
/// If each row chose this part ad hoc (a constant, an entry count, a block head), a constant would alert once
/// and stay silent forever, and a moving reading would alert on every change. As a closed type, a row that
/// wants to alert must say which episode the alert belongs to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Episode {
    /// The deadline family: the deadline itself (chain time). Renewal is a new episode; recomputing the same
    /// deadline stays the same episode.
    Until(u64),
    /// The break family: which entries it broke at (ids of the findings the core marks hard). Repaired in
    /// place and broken again is another episode: the entry count can stay the same, while the ids at the
    /// break change with the bytes.
    At(String),
    /// The upstream family: the id of the upstream ledger's head entry. Any change to the upstream, even one
    /// entry or one character, is a new episode.
    Head(String),
}

impl Episode {
    /// Which family this episode belongs to; used by key assembly and the UI.
    pub fn kind(&self) -> &'static str {
        match self {
            Episode::Until(_) => "until",
            Episode::At(_) => "at",
            Episode::Head(_) => "head",
        }
    }

    /// The episode's value, unchanged (ids always lowercase: the two cases of one id are the same episode).
    pub fn value(&self) -> String {
        match self {
            Episode::Until(n) => n.to_string(),
            Episode::At(id) | Episode::Head(id) => id.to_ascii_lowercase(),
        }
    }

    /// The text of this part of the key. Family name first, so numbers from two families never collide.
    pub fn as_key(&self) -> String {
        format!("{}-{}", self.kind(), self.value())
    }
}

/// One alert a row should raise: to whom (subject), which episode.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ring {
    pub subject: String,
    pub episode: Episode,
}

/// One row of the watch table.
#[derive(Clone, Debug)]
pub struct Row {
    pub item: Item,
    pub light: Light,
    /// This row's main number (how many entries, items or places).
    pub n: u64,
    /// A one-line reading (label name, state name), shown unchanged in the UI.
    pub detail: String,
    /// The action sentence when the light is not green; `None` when green.
    pub gap: Option<Gap>,
    /// The alerts to raise. Empty means this row does not alert.
    pub rings: Vec<Ring>,
}

fn row(item: Item, light: Light, n: u64, detail: String, gap: Option<Gap>) -> Row {
    Row { item, light, n, detail, gap, rings: Vec::new() }
}

/// Where it broke: the ids of the findings the core marked `hard` in the report, sorted, deduplicated and
/// joined. Which findings are hard is the core's decision; this layer re-judges none.
///
/// BROKEN_CHAIN is set by having a hard finding (law §8.7 item 15), so there is always at least one. If none
/// can be read, this returns an empty string; the row still alerts once and not again (the empty string is
/// the same episode).
fn broken_at(report: &Value) -> String {
    let mut ids: Vec<String> = crate::auditx::rows_of(report, zikaron::tokens::Key::Findings)
        .iter()
        .filter(|r| matches!(member(r, zikaron::tokens::Key::Hard.as_str()), Some(Value::Bool(true))))
        .filter_map(|r| match member(r, zikaron::tokens::Key::EntryId.as_str()) {
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

/// How many rows one table in the self-audit report has.
fn table_len(report: &Value, k: zikaron::tokens::Key) -> u64 {
    crate::auditx::rows_of(report, k).len() as u64
}

/// The whole-machine backup row (both seats), four colours: a last backup attempt that was not written or not
/// read back is red (`failed`, its time is the episode); never backed up is grey (one alert); ledger entries
/// and held grants added since the last backup are amber with the count (one alert per backup: the episode is
/// the last backup's time); nothing behind is green. A count not measured yet is grey too, undecided, never
/// passing for "nothing behind".
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

/// Author seat, five rows. All readings come from the parameters.
pub fn author(
    rows: Option<&[crate::ledgerx::Row]>,
    queued: usize,
    grants: Option<&[crate::grantx::Row]>,
    backup: (Option<&crate::machine::Backed>, Option<u64>, Option<u64>),
    audit: Option<(&str, &Value, &[String])>,
    now: Option<u64>,
) -> Vec<Row> {
    crate::trace::mark(crate::feature::Feature::P2);
    let gap = |say: Say, n: u64, secs: Option<u64>, go: Page| Some(Gap { say, n, secs, go });
    let mut out: Vec<Row> = Vec::new();
    // Unanchored: entries no anchor covers (transitively, as the anchoring queue reads it: anchoring seq N
    // covers ≤N); a covered entry whose light is not green is not a gap. Recorded but not yet queued points to
    // the anchoring desk; all queued points to the queue.
    out.push(match rows {
        None => row(Item::Unanchored, Light::Unknown, 0, String::new(), gap(Say::UnanchoredRead, 1, None, Page::Ledger)),
        Some(r) => {
            let d = crate::queue::density(Some(r), queued);
            let uncovered = |x: &&crate::ledgerx::Row| d.anchored_through.map(|n| x.seq > n).unwrap_or(true);
            let landed = r.iter().filter(uncovered).filter(|x| x.lamp == crate::ledgerx::Lamp::Landed).count() as u64;
            row(
                Item::Unanchored,
                if d.behind == 0 { Light::Ok } else { Light::Warn },
                d.behind as u64,
                format!("{} / {}", d.anchored_through.map(|x| x.to_string()).unwrap_or_default(), d.head_seq.map(|x| x.to_string()).unwrap_or_default()),
                if d.behind == 0 {
                    None
                } else if landed > 0 {
                    gap(Say::Unanchored, landed, None, Page::Anchoring)
                } else {
                    gap(Say::Queue, d.behind as u64, None, Page::Queue)
                },
            )
        }
    });
    // Queue backlog.
    out.push(row(
        Item::QueueBacklog,
        if queued == 0 { Light::Ok } else { Light::Warn },
        queued as u64,
        String::new(),
        if queued > 0 { gap(Say::Queue, queued as u64, None, Page::Queue) } else { None },
    ));
    // Window expiring: chain time only.
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
    // The whole-machine backup.
    out.push(backup_row(backup.0, backup.1, backup.2));
    // Self-audit label. The broken-chain alert's episode is the ids at the break: repaired in place and
    // broken again (same entry count) is another episode and alerts again.
    out.push(match audit {
        None => row(Item::AuditLabel, Light::Unknown, 0, String::new(), gap(Say::AuditRun, 1, None, Page::Audit)),
        Some((label, report, unanswered)) => {
            use zikaron::tokens::Label;
            if label == Label::Complete.as_str() {
                row(Item::AuditLabel, Light::Ok, 0, label.to_string(), None)
            } else if label == Label::Gaps.as_str() {
                let n = table_len(report, zikaron::tokens::Key::Missing);
                row(Item::AuditLabel, Light::Bad, n, label.to_string(), gap(Say::AuditGaps, n.max(1), None, Page::Audit))
            } else if label == Label::Unavailable.as_str() {
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

/// Grantee seat, five rows.
pub fn grantee(
    cards: Option<&[crate::vaultx::Card]>,
    alarms: &[crate::sentinelx::Alarm],
    now: Option<u64>,
    backup: (Option<&crate::machine::Backed>, Option<u64>, Option<u64>),
) -> Vec<Row> {
    crate::trace::mark(crate::feature::Feature::P2);
    let gap = |say: Say, n: u64, secs: Option<u64>, go: Page| Some(Gap { say, n, secs, go });
    let mut out: Vec<Row> = Vec::new();
    // Holding expiring: each card counts down on its own; a card without chain time leaves only that card
    // undecided, and the others still alert. Expired ones alert too (one key per deadline: one that alerted
    // while expiring does not alert again; one that never alerted does once expired).
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
    // Revocation and succession alarms (the revocation sentinel's readings; the alerts belong to it).
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
    // Upstream turned red: the upstream ledger's audit label is BROKEN_CHAIN, or BROKEN_LEDGER failed among
    // the six checks. The alert's episode is the upstream ledger's head entry id, so any change, even with the
    // same entry count, is a new episode.
    out.push(match cards {
        None => row(Item::UpstreamRed, Light::Unknown, 0, String::new(), gap(Say::HoldingRead, 1, None, Page::Vault)),
        Some(c) => {
            let mut authors: Vec<Ring> = c
                .iter()
                .filter(|x| {
                    x.upstream_label == zikaron::tokens::Label::BrokenChain.as_str()
                        || x.failed.iter().any(|f| f == zikaron_kit::tokens::Check::BrokenLedger.as_str())
                })
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

/// One notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The deduplication key: `p2:<row>:<subject>:<episode>`, assembled only here.
    pub key: String,
    pub item: Item,
    pub subject: String,
    pub episode: Episode,
}

/// The keys to alert. How many alerts a row raises is set by its `rings`: one key per subject and episode.
/// The key's four parts are assembled only here, so what goes in each part is not decided row by row.
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

/// Deduplicate. Keys already alerted do not alert again; returns (the new alerts this pass, their keys).
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

/// Whether a fault is network-related (network errors go to the status line, not to notifications).
pub fn is_network(f: &crate::fault::Fault) -> bool {
    use crate::fault::Known;
    let head = f.said().split(':').next().unwrap_or("");
    // The member table lives in one place (`fault::NETWORK`): the status line and dispatching read the same
    // table.
    Known::NETWORK.iter().any(|k| k.as_str() == head)
}
