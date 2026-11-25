use crate::fault::{Fault, Known};
use crate::home::Home;
use zikaron::json::Value;
use zikaron_glue::book::Kind as EntryType;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lamp {
    Anchored,
    Included,
    Submitted,
    Queued,
    Reverted,
    Refused,
    Landed,
    Deleted,
    LocalDeletion,
    Remembered,
    RememberedStale,
    ChainUnread,
    Sealed,
}

impl Lamp {
    pub const ALL: [Lamp; 13] = [
        Lamp::Anchored,
        Lamp::Included,
        Lamp::Submitted,
        Lamp::Queued,
        Lamp::Reverted,
        Lamp::Refused,
        Lamp::Landed,
        Lamp::Deleted,
        Lamp::LocalDeletion,
        Lamp::Remembered,
        Lamp::RememberedStale,
        Lamp::ChainUnread,
        Lamp::Sealed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Lamp::Anchored => "anchored",
            Lamp::Included => "included",
            Lamp::Submitted => "submitted",
            Lamp::Queued => "queued",
            Lamp::Reverted => "reverted",
            Lamp::Refused => "refused",
            Lamp::Landed => "landed",
            Lamp::Deleted => "deleted",
            Lamp::LocalDeletion => "local-deletion",
            Lamp::Remembered => "remembered",
            Lamp::RememberedStale => "remembered-stale",
            Lamp::ChainUnread => "chain-unread",
            Lamp::Sealed => "sealed",
        }
    }

    pub fn confirmed(self) -> bool {
        matches!(self, Lamp::Anchored | Lamp::Remembered | Lamp::RememberedStale)
    }

    pub fn in_queue(self) -> bool {
        matches!(self, Lamp::Queued | Lamp::Submitted | Lamp::Reverted | Lamp::Refused)
    }

    pub fn local(self) -> bool {
        matches!(self, Lamp::Deleted | Lamp::LocalDeletion)
    }

    pub fn stays_off(self) -> bool {
        self.local() || self == Lamp::Sealed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub law: zikaron_glue::book::Law,
    pub seq: u64,
    pub kind: EntryType,
    pub id: String,
    pub prev: Option<String>,
    pub author: String,
    pub summary: String,
    pub lamp: Lamp,
    pub tx: Option<(u64, String)>,
    pub bytes: usize,
    pub work: Option<String>,
    pub facts: Facts,
    pub anchored_at: Option<u64>,
}

impl Row {
    pub fn pos(&self) -> u64 {
        match self.law {
            zikaron_glue::book::Law::One => self.seq,
            zikaron_glue::book::Law::Two => (1u64 << 62) + self.seq,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub note: Option<String>,
    pub mark: Option<String>,
    pub grantee: Option<String>,
    pub work: Option<String>,
    pub window: Option<(u64, u64)>,
    pub subject: Option<String>,
    pub to: Option<String>,
    pub shape_ok: bool,
    pub raw_type: String,
    pub file: Option<String>,
    pub content: Option<String>,
    pub member: bool,
    pub face: Option<(&'static str, String, String)>,
}

impl Facts {
    pub fn record_name(&self) -> Option<String> {
        self.note.clone().filter(|n| !n.trim().is_empty()).or_else(|| self.file.clone().filter(|n| !n.trim().is_empty())).or_else(|| self.content.as_deref().and_then(content_short))
    }
}

pub fn facts(kind: EntryType, body: &Value) -> Facts {
    let note = |k: &str| text(body, k).map(|s| one_line(s, 48)).filter(|s| !s.is_empty());
    let mut f = Facts::default();
    match kind {
        EntryType::Genesis => f.note = note("statement_md"),
        EntryType::History => {
            f.note = note(crate::entryx::NOTE_MD);
            f.mark = member(body, "mode").and_then(|m| text(m, "mark").map(|x| x.to_string()));
            f.content = work_of(kind, body);
        }
        EntryType::Grant => {
            f.grantee = text(body, "grantee").map(|s| s.to_string());
            f.work = text(body, "work").map(|s| s.to_string());
            f.window = member(body, "window").and_then(|w| Some((int(w, "from")?, int(w, "to")?)));
            f.member = member(body, zikaron_kit::v2::tokens::Key::Revocation.as_str()).is_some();
        }
        EntryType::Revocation => f.subject = text(body, "grant").map(|s| s.to_string()),
        EntryType::Adoption => {}
        EntryType::Succession => f.to = text(body, "to").map(|s| s.to_string()),
        EntryType::Migration => f.note = note("statement_md"),
        EntryType::Annotation | EntryType::Retraction => {
            f.note = note(crate::entryx::NOTE_MD);
            f.subject = text(body, "subject").map(|s| s.to_string());
        }
        EntryType::Other => {}
        EntryType::Face => {
            let t = |k: &str| text(body, k).unwrap_or_default().to_string();
            f.face = zikaron_glue::face::shape_of(body).map(|shape| match shape {
                zikaron_glue::face::Shape::Home => ("home", int(body, "chain").map(|c| c.to_string()).unwrap_or_default(), String::new()),
                zikaron_glue::face::Shape::Account => ("account", t("platform"), t("handle")),
                zikaron_glue::face::Shape::Key => ("key", t("address"), t("role")),
                zikaron_glue::face::Shape::Persona => ("persona", String::new(), String::new()),
                zikaron_glue::face::Shape::Release => ("release", t("subject"), String::new()),
                zikaron_glue::face::Shape::Reroot => ("reroot", t("to"), String::new()),
                zikaron_glue::face::Shape::Origin => ("origin", t("from"), String::new()),
            });
        }
    }
    f
}

pub fn facts_of(e: &zikaron_glue::book::Entry) -> Facts {
    let mut f = facts(e.kind, &e.body);
    let line = zikaron_glue::retraction::Line::at(0, e);
    f.raw_type = line.raw_type;
    if zikaron_glue::retraction::is_retraction(e.kind, &e.entry_type) {
        f.subject = line.subject;
        f.shape_ok = line.shape_ok;
    }
    f
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    match member(v, k) {
        Some(Value::Str(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn int(v: &Value, k: &str) -> Option<u64> {
    match member(v, k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

pub fn short(h: &str) -> String {
    if h.chars().count() <= 12 {
        return h.to_string();
    }
    format!("{}…", h.chars().take(12).collect::<String>())
}

fn content_short(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("0x")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| format!("0x{}", &hex[..8].to_ascii_lowercase()))
}

pub fn work_of(kind: EntryType, body: &Value) -> Option<String> {
    zikaron_glue::recordcheck::work_of(kind, body)
}

pub fn summarize(kind: EntryType, body: &Value) -> String {
    let missing = "—".to_string();
    match kind {
        EntryType::Genesis => text(body, "statement_md")
            .map(|s| one_line(s, 60))
            .unwrap_or(missing),
        EntryType::History => {
            let c = text(body, "content").map(short).unwrap_or_else(|| "—".into());
            let mark = member(body, "mode")
                .and_then(|m| text(m, "mark").map(|x| x.to_string()))
                .unwrap_or_else(|| "—".into());
            format!("{c} · {mark}")
        }
        EntryType::Grant => {
            let g = text(body, "grantee").map(short).unwrap_or_else(|| "—".into());
            let w = match member(body, "window") {
                Some(w) => match (int(w, "from"), int(w, "to")) {
                    (Some(a), Some(b)) => format!("{a}–{b}"),
                    _ => "—".to_string(),
                },
                None => "∞".to_string(),
            };
            format!("{g} · {w}")
        }
        EntryType::Revocation => {
            let g = text(body, "grant").map(short).unwrap_or_else(|| "—".into());
            match text(body, "case") {
                Some(c) => format!("{g} · {}", short(c)),
                None => g,
            }
        }
        EntryType::Adoption => missing,
        EntryType::Migration => text(body, "spec").unwrap_or("—").to_string(),
        EntryType::Retraction => text(body, "subject").map(short).unwrap_or(missing),
        EntryType::Succession => {
            let k = text(body, "kind").unwrap_or("—");
            let to = text(body, "to").map(short).unwrap_or_else(|| "—".into());
            format!("{k} · {to}")
        }
        EntryType::Annotation => match text(body, "subject") {
            Some(s) => short(s),
            None => text(body, crate::entryx::NOTE_MD).map(|s| one_line(s, 48)).unwrap_or(missing),
        },
        EntryType::Other => missing,
        EntryType::Face => match zikaron_glue::face::shape_of(body) {
            Some(zikaron_glue::face::Shape::Home) => format!("home · {}", int(body, "chain").map(|c| c.to_string()).unwrap_or(missing)),
            Some(zikaron_glue::face::Shape::Account) => format!("account · {} · {}", text(body, "platform").unwrap_or("—"), text(body, "handle").unwrap_or("—")),
            Some(zikaron_glue::face::Shape::Key) => format!("key · {} · {}", text(body, "address").map(short).unwrap_or_else(|| "—".into()), text(body, "role").unwrap_or("—")),
            Some(zikaron_glue::face::Shape::Persona) => "persona".to_string(),
            Some(zikaron_glue::face::Shape::Release) => format!("release · {}", text(body, "subject").map(short).unwrap_or_else(|| "—".into())),
            Some(zikaron_glue::face::Shape::Reroot) => format!("reroot · {}", text(body, "to").map(short).unwrap_or_else(|| "—".into())),
            Some(zikaron_glue::face::Shape::Origin) => format!("origin · {}", text(body, "from").map(short).unwrap_or_else(|| "—".into())),
            None => missing,
        },
    }
}

fn one_line(s: &str, cap: usize) -> String {
    let one: String = s.lines().next().unwrap_or("").trim().to_string();
    if one.chars().count() <= cap {
        return one;
    }
    let cut: String = one.chars().take(cap).collect();
    format!("{cut}…")
}

pub fn anchored_of(report: &Value) -> Vec<(String, (u64, String))> {
    use zikaron::v2::tokens::Key;
    let mut out = Vec::new();
    let Some(Value::Arr(rows)) = member(report, Key::Anchored.as_str()) else {
        return out;
    };
    for r in rows {
        let Some(id) = text(r, Key::EntryId.as_str()) else { continue };
        let Some(Value::Arr(anchors)) = member(r, Key::Anchors.as_str()) else {
            continue;
        };
        let Some(first) = anchors.iter().min_by_key(|a| (int(a, Key::BlockTimestamp.as_str()).unwrap_or(u64::MAX), int(a, Key::BlockNumber.as_str()).unwrap_or(u64::MAX))) else { continue };
        let chain = int(first, Key::ChainId.as_str()).unwrap_or(0);
        let tx = text(first, Key::Tx.as_str()).unwrap_or("").to_string();
        out.push((id.to_string(), (chain, tx)));
    }
    out
}

pub fn anchors_of(report: &Value, id: &str) -> Vec<(u64, String)> {
    use zikaron::v2::tokens::Key;
    let Some(Value::Arr(rows)) = member(report, Key::Anchored.as_str()) else { return Vec::new() };
    let Some(r) = rows.iter().find(|r| text(r, Key::EntryId.as_str()).is_some_and(|x| x.eq_ignore_ascii_case(id.trim()))) else { return Vec::new() };
    let Some(Value::Arr(anchors)) = member(r, Key::Anchors.as_str()) else { return Vec::new() };
    let mut sorted: Vec<&Value> = anchors.iter().collect();
    sorted.sort_by_key(|a| (int(a, Key::BlockTimestamp.as_str()).unwrap_or(u64::MAX), int(a, Key::BlockNumber.as_str()).unwrap_or(u64::MAX)));
    let mut out: Vec<(u64, String)> = Vec::new();
    for a in sorted {
        let chain = int(a, Key::ChainId.as_str()).unwrap_or(0);
        if !out.iter().any(|(c, _)| *c == chain) {
            out.push((chain, text(a, Key::Tx.as_str()).unwrap_or("").to_string()));
        }
    }
    out
}

pub fn chains_checked(report: &Value) -> Vec<(u64, usize)> {
    use zikaron::v2::tokens::Key;
    let chains: Vec<u64> = match member(report, Key::Basis.as_str()).and_then(|b| member(b, "chains")) {
        Some(Value::Arr(cs)) => cs.iter().filter_map(|c| int(c, Key::ChainId.as_str())).collect(),
        _ => Vec::new(),
    };
    let rows = match member(report, Key::Anchored.as_str()) {
        Some(Value::Arr(rows)) => rows.clone(),
        _ => Vec::new(),
    };
    let mut out: Vec<(u64, usize)> = Vec::new();
    for c in chains {
        if out.iter().any(|(x, _)| *x == c) {
            continue;
        }
        let n = rows
            .iter()
            .filter(|r| match member(r, Key::Anchors.as_str()) {
                Some(Value::Arr(a)) => a.iter().any(|x| int(x, Key::ChainId.as_str()) == Some(c)),
                _ => false,
            })
            .count();
        out.push((c, n));
    }
    out
}

pub fn stamp(rows: &mut [Row], items: &[Vec<u8>], fragment: &Value) {
    let Some(at) = crate::auditx::first_anchored(items, fragment) else { return };
    for r in rows.iter_mut() {
        r.anchored_at = at.iter().find(|(h, _)| h.eq_ignore_ascii_case(&r.id)).map(|(_, t)| *t);
    }
}

pub struct Table {
    pub rows: Vec<Row>,
    pub strays: usize,
}

pub fn table(home: &Home, report: Option<&Value>, queued: &[String]) -> Result<Table, Fault> {
    let mut q = crate::queue::Queue::default();
    for id in queued {
        let _ = q.push(id, 0);
    }
    table_with(home, report, &q)
}

pub fn table_with(home: &Home, report: Option<&Value>, queue: &crate::queue::Queue) -> Result<Table, Fault> {
    table_remembering(home, report, queue, None, 0)
}

pub fn table_remembering(
    home: &Home,
    report: Option<&Value>,
    queue: &crate::queue::Queue,
    remembered: Option<&crate::lastread::Anchored>,
    now: u64,
) -> Result<Table, Fault> {
    let stale = remembered.map(|r| crate::lastread::stale(r.at, now)).unwrap_or(false);
    let ledger = home.ledger()?;
    let survey = ledger
        .survey()?;
    let anchored = report.map(anchored_of).unwrap_or_default();
    let mut rows: Vec<Row> = Vec::new();
    for b in &survey.items {
        let Ok(e) = zikaron_glue::book::check(b) else { continue };
        let id = zikaron::hexfmt::encode(&e.id);
        let tx = anchored.iter().find(|(h, _)| *h == id).map(|(_, t)| t.clone());
        let recalled = if tx.is_none() { remembered.and_then(|r| r.rows.iter().find(|(h, _)| h.eq_ignore_ascii_case(&id)).map(|(_, t)| t.clone())) } else { None };
        let lamp = match (tx.is_some(), recalled.is_some()) {
            (false, true) if stale => Lamp::RememberedStale,
            (false, true) => Lamp::Remembered,
            _ => lamp_of(&id, tx.is_some(), queue),
        };
        let tx = tx.or(recalled);
        let tx = tx.or_else(|| queue.block_of(&id).map(|b| (b.chain, b.tx.clone()))).or_else(|| match queue.step_of(&id) {
            Some(crate::queue::Step::Submitted { tx, chain, .. }) | Some(crate::queue::Step::Reverted { tx, chain }) => Some((*chain, tx.clone())),
            Some(crate::queue::Step::Resent { txs, chain, .. }) => txs.last().map(|t| (*chain, t.clone())),
            _ => None,
        });
        rows.push(Row {
            law: e.law,
            seq: e.seq,
            kind: e.kind,
            summary: summarize(e.kind, &e.body),
            work: work_of(e.kind, &e.body),
            facts: facts_of(&e),
            id,
            prev: e.prev.clone(),
            author: e.author.clone(),
            lamp,
            tx,
            bytes: b.len(),
            anchored_at: None,
        });
    }
    if let Ok(index) = crate::home::machine_dir().and_then(|m| crate::recordsx::read(&m)) {
        for r in rows.iter_mut().filter(|r| r.kind == EntryType::History) {
            r.facts.file = index.iter().find(|x| x.id.eq_ignore_ascii_case(&r.id)).map(|x| x.name.clone()).filter(|n| !n.is_empty());
        }
    }
    let report_ids: Vec<String> = anchored
        .iter()
        .map(|(h, _)| h.clone())
        .chain(remembered.map(|r| r.rows.iter().map(|(h, _)| h.clone()).collect::<Vec<_>>()).unwrap_or_default())
        .collect();
    let pairs = crate::retractx::pairs(&rows);
    for (retraction, subject) in pairs {
        if queue.published(&subject, &report_ids) {
            continue;
        }
        for r in rows.iter_mut() {
            if r.id == subject && !matches!(r.lamp, Lamp::Anchored | Lamp::Remembered | Lamp::RememberedStale | Lamp::Included | Lamp::Submitted) {
                r.lamp = Lamp::Deleted;
            }
            if r.id == retraction && matches!(r.lamp, Lamp::Landed) {
                r.lamp = Lamp::LocalDeletion;
            }
        }
    }
    rows.sort_by(|a, b| (b.law, b.seq, &a.id).cmp(&(a.law, a.seq, &b.id)));
    Ok(Table { rows, strays: survey.skipped.len() })
}

pub fn lamp_of(id: &str, in_report: bool, queue: &crate::queue::Queue) -> Lamp {
    if in_report {
        return Lamp::Anchored;
    }
    if queue.block_of(id).is_some() || queue.anchored_here(id) {
        return Lamp::Included;
    }
    match queue.step_of(id) {
        Some(crate::queue::Step::Queued) => Lamp::Queued,
        Some(crate::queue::Step::Submitted { .. }) | Some(crate::queue::Step::Resent { .. }) => Lamp::Submitted,
        Some(crate::queue::Step::Reverted { .. }) => Lamp::Reverted,
        Some(crate::queue::Step::Refused { .. }) => Lamp::Refused,
        None => Lamp::Landed,
    }
}

pub fn refusal_of<'a>(id: &str, queue: &'a crate::queue::Queue) -> Option<&'a str> {
    match queue.step_of(id) {
        Some(crate::queue::Step::Refused { said }) if !said.is_empty() => Some(said.as_str()),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Detail {
    pub id: String,
    pub bytes: Vec<u8>,
    pub prev: Option<String>,
    pub seq: u64,
    pub author: String,
    pub kind: EntryType,
    pub body: Value,
}

pub fn detail(home: &Home, id: &str) -> Result<Detail, Fault> {
    let ledger = home.ledger()?;
    let bare = id.trim().trim_start_matches("0x");
    let name = zikaron_store::EntryName::parse(bare)
        .ok_or_else(|| Fault::known(Known::ContentShape, id.to_string()))?;
    let file = zikaron_store::layout::entry_file_name(&name);
    let bytes = ledger
        .read_named(&file)?;
    let e = zikaron_glue::book::check(&bytes)
        .map_err(Fault::entry_refused)?;
    Ok(Detail {
        id: zikaron::hexfmt::encode(&e.id),
        prev: e.prev.clone(),
        seq: e.seq,
        author: e.author.clone(),
        kind: e.kind,
        body: e.body.clone(),
        bytes,
    })
}

pub fn rooted(home: &Home) -> bool {
    match head(home) {
        Ok(h) => h.is_some(),
        Err(f) => f.which() == Some(Known::NotMoved),
    }
}

pub fn head(home: &Home) -> Result<Option<(u64, String)>, Fault> {
    let ledger = home.ledger()?;
    let survey = ledger
        .survey()?;
    let empty = crate::local::empty_in(&survey.skipped);
    if !empty.is_empty() {
        return Err(crate::local::empty_fault(&empty));
    }
    if !survey.skipped.is_empty() {
        return Err(Fault::known(
            Known::Ledger,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail182, &[&(survey.skipped.len()).to_string()])),
        ));
    }
    for b in &survey.items {
        if zikaron_glue::book::check(b).is_err() {
            return Err(Fault::known(Known::Ledger, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail183).to_string())));
        }
    }
    let tip = |f: Fault| f.owned(crate::fault::Owned::Reason(zikaron_glue::door::Own::Tip));
    let root = match zikaron_glue::book::root_of(&survey.items) {
        Ok(r) => r,
        Err(zikaron_glue::book::NoRoot::Empty) => return Ok(None),
        Err(zikaron_glue::book::NoRoot::Forked(_)) => return Err(tip(Fault::known(Known::ForkedRoot, home.root().display().to_string()))),
    };
    match zikaron_glue::book::tip(&root, &survey.items) {
        Ok((next, prev)) => Ok(Some((next - 1, prev))),
        Err(zikaron_glue::book::NoTip::Absent) => Ok(None),
        Err(zikaron_glue::book::NoTip::NotMoved) => Err(tip(Fault::known(Known::NotMoved, home.root().display().to_string()))),
        Err(zikaron_glue::book::NoTip::Forked) => Err(tip(Fault::known(Known::Broken, root))),
    }
}

pub fn root_anchored(rows: &[Row]) -> Option<bool> {
    rows.iter().filter(|r| r.seq == 0 && r.kind == EntryType::Genesis).min_by_key(|r| r.law).map(|r| r.lamp.confirmed())
}

pub fn root_of(home: &Home) -> Result<String, Fault> {
    let pile = home.ledger()?.pile()?;
    zikaron_glue::book::root_entry(&pile.items)
        .map(|e| e.id_hex())
        .ok_or_else(|| Fault::known(Known::Ledger, home.root().display().to_string()))
}
