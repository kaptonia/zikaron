use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::PathBuf;
use zikaron_glue::book::{self as entry, Entry};
use zikaron::json::Value;

pub fn take(typed: &str) -> Result<Vec<Vec<u8>>, Fault> {
    Ok(crate::payloadx::take(typed)?.1)
}

pub fn decode_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, Fault> {
    crate::payloadx::decode(payload)
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

pub fn admit(bytes: &[u8]) -> Result<Entry, Fault> {
    crate::trace::mark(crate::feature::Feature::D6);
    let e = entry::check(bytes).map_err(Fault::entry_refused)?;
    if e.kind != zikaron_glue::book::Kind::Grant {
        return Err(Fault::known(Known::NotAGrant, e.kind.as_str().to_string()));
    }
    Ok(e)
}

pub fn path_of(home: &Home, id: &str) -> Result<PathBuf, crate::fault::Fault> {
    crate::trace::mark(crate::feature::Feature::D6);
    Ok(home.dir(Slot::GrantsHeld).join(format!("{}{}", held_stem(id)?, zikaron_store::layout::ENTRY_SUFFIX)))
}

pub fn held_stem(id: &str) -> Result<String, crate::fault::Fault> {
    Ok(crate::names::key()?.name(crate::names::Logical::Held(id)))
}

pub fn store(home: &Home, bytes: &[u8]) -> Result<String, Fault> {
    let e = admit(bytes)?;
    let id = e.id_hex();
    let at = path_of(home, &id)?;
    if at.exists() {
        return Err(Fault::known(Known::AlreadyHeld, id));
    }
    if let Some(d) = at.parent() {
        std::fs::create_dir_all(d).map_err(|x| crate::fault::classify(&x, &d.display().to_string()))?;
    }
    crate::local::land(&at, crate::local::Doc::Held, bytes)?;
    Ok(id)
}

pub fn store_all(home: &Home, items: &[&Vec<u8>]) -> Result<Vec<String>, Fault> {
    crate::trace::mark(crate::feature::Feature::D6);
    let key = crate::keybox::local_key()?;
    let mut staged: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    for b in items {
        let e = admit(b)?;
        let id = e.id_hex();
        let at = path_of(home, &id)?;
        if at.exists() {
            for (_, part, _) in &staged {
                let _ = std::fs::remove_file(part);
            }
            return Err(Fault::known(Known::AlreadyHeld, id));
        }
        if let Some(d) = at.parent() {
            std::fs::create_dir_all(d).map_err(|x| crate::fault::classify(&x, &d.display().to_string()))?;
        }
        let part = at.with_extension(format!("part-{}", std::process::id()));
        let sealed = crate::local::seal_with(&key, &crate::local::ident_at(&at, crate::local::Doc::Held, b)?, b)?;
        if let Err(t) = zikaron_glue::landing::land_bytes(&part, &sealed) {
            for (_, p, _) in &staged {
                let _ = std::fs::remove_file(p);
            }
            let _ = std::fs::remove_file(&part);
            return Err(Fault::known(Known::Landing, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail220, &[&(t.code()).to_string(), &(t.subject()).to_string(), &(part.display()).to_string()]))));
        }
        staged.push((id, part, at));
    }
    let mut landed: Vec<String> = Vec::new();
    let mut rest = staged.into_iter();
    while let Some((id, part, at)) = rest.next() {
        if let Err(x) = zikaron_os::replace(&part, &at) {
            let _ = std::fs::remove_file(&part);
            for (_, p, _) in rest {
                let _ = std::fs::remove_file(p);
            }
            let f = crate::fault::classify(&x, &at.display().to_string());
            return Err(Fault::known(Known::Landing, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail221, &[&(f.said()).to_string(), &(f.tail()).to_string(), &(landed.join(" ")).to_string()]))));
        }
        landed.push(id);
    }
    Ok(landed)
}

fn held_copy(home: &Home, hop: &[u8]) -> Option<(String, Option<Vec<Vec<u8>>>, String)> {
    let e = entry::check(hop).ok()?;
    let (want, author) = (e.id_hex(), e.author.clone());
    let held = crate::supplyx::kept_copy(&home.dir(Slot::GrantsHeld).join(crate::grantfilex::KEPT), &want, &author, &mut Vec::new()).map(|s| s.items);
    Some((want, held, author))
}

pub fn newer_than_held(home: &Home, hop: &[u8], ledger: &[Vec<u8>]) -> bool {
    let Some((want, held, author)) = held_copy(home, hop) else { return false };
    let Some(candidate) = crate::auditx::ledger_of(ledger, &want, &author) else { return false };
    crate::supplyx::rank(&candidate, &want) > crate::supplyx::rank(&held.unwrap_or_default(), &want)
}

pub fn keep_newer(home: &Home, h: &Held, upstream: &[Vec<u8>]) -> Result<bool, Fault> {
    let Some((want, held, author)) = held_copy(home, &h.bytes) else { return Ok(false) };
    let Some(candidate) = crate::auditx::ledger_of(upstream, &want, &author) else { return Ok(false) };
    let wanted = match &held {
        None => true,
        Some(k) => crate::supplyx::rank(&candidate, &want).0 && !crate::supplyx::rank(k, &want).0,
    };
    if !wanted {
        return Ok(false);
    }
    let chain = crate::badgex::chain_for(&crate::badgex::pool(home)?, &h.id)?;
    let (bytes, _, kit_id) = crate::grantfilex::built(crate::grantfilex::parts(&chain, &candidate, &[], None)?)?;
    crate::grantfilex::keep(home, &bytes, &kit_id)?;
    Ok(true)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub id: String,
    pub bytes: Vec<u8>,
    pub author: String,
    pub grantee: String,
    pub work: String,
    pub terms: String,
    pub window: Option<(u64, u64)>,
    pub upstream: Option<String>,
    pub seq: u64,
}

pub fn held(home: &Home) -> Result<Vec<Held>, Fault> {
    held_all(home).map(|(h, _)| h)
}

pub fn held_all(home: &Home) -> Result<(Vec<Held>, Vec<crate::verifyx::Rejected>), Fault> {
    let dir = home.dir(Slot::GrantsHeld);
    let mut out: Vec<Held> = Vec::new();
    let mut bad: Vec<crate::verifyx::Rejected> = Vec::new();
    let key = crate::keybox::local_key()?;
    let mut stack = vec![dir.clone()];
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    while let Some(at) = stack.pop() {
        let listing = match std::fs::read_dir(&at) {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && at == dir => return Ok((out, bad)),
            Err(e) => return Err(crate::fault::classify(&e, &at.display().to_string())),
        };
        for e in listing.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                files.push(p);
            }
        }
    }
    for p in files {
        let name = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        if zikaron_store::layout::parse_entry_file(&name).is_none() {
            continue;
        }
        let bytes = match std::fs::read(&p).map_err(|x| crate::fault::classify(&x, &p.display().to_string())).and_then(|b| crate::local::expect_at(&p, crate::local::Doc::Held).and_then(|ex| crate::local::open_with(&key, &ex, &b, &name))) {
            Ok(b) => b,
            Err(f) => {
                bad.push(crate::verifyx::Rejected::of_fault(name, &f));
                continue;
            }
        };
        let en = match admit(&bytes) {
            Ok(en) => en,
            Err(f) => {
                bad.push(crate::verifyx::Rejected::of_fault(name, &f));
                continue;
            }
        };
        out.push(Held {
            id: en.id_hex(),
            author: en.author.clone(),
            grantee: text(&en.body, "grantee").unwrap_or("").to_string(),
            work: text(&en.body, "work").unwrap_or("").to_string(),
            terms: text(&en.body, "terms").unwrap_or("").to_string(),
            window: crate::grantx::window_of(&en.body),
            upstream: text(&en.body, "upstream").map(|s| s.to_string()),
            seq: en.seq,
            bytes,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    bad.sort_by(|a, b| a.file.cmp(&b.file));
    Ok((out, bad))
}

pub fn held_rows(home: &Home) -> Result<Vec<(String, Vec<u8>)>, Fault> {
    let key = crate::keybox::local_key()?;
    let root = home.dir(Slot::GrantsHeld);
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(at) = stack.pop() {
        let listing = std::fs::read_dir(&at).map_err(|x| crate::fault::classify(&x, &at.display().to_string()))?;
        for e in listing.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                if p.file_name().map(|n| crate::lastread::is_cache(&n.to_string_lossy())).unwrap_or(false) {
                    continue;
                }
                let rel = p
                    .strip_prefix(&root)
                    .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/"))
                    .unwrap_or_default();
                let bytes = std::fs::read(&p).map_err(|x| crate::fault::classify(&x, &p.display().to_string()))?;
                let (rel, bytes) = match held_doc(&rel) {
                    Some(doc) => {
                        let plain = crate::local::open_with(&key, &crate::local::expect_at(&p, doc)?, &bytes, &rel)?;
                        let room = format!("{}/", Slot::GrantsHeld.as_str());
                        let logical = crate::local::logical_rel(doc, &format!("{room}{rel}"), &plain)?;
                        (logical.strip_prefix(&room).map(str::to_string).unwrap_or(logical), plain)
                    }
                    None => (rel, bytes),
                };
                out.push((rel, bytes));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Countdown {
    NoWindow,
    NoNow,
    NotYet { starts_in: u64 },
    Live { remaining: u64 },
    Expired { since: u64 },
}

pub fn countdown(window: Option<(u64, u64)>, now: Option<u64>) -> Countdown {
    match (window, now) {
        (None, _) => Countdown::NoWindow,
        (Some(_), None) => Countdown::NoNow,
        (Some((a, b)), Some(n)) => {
            if n < a {
                Countdown::NotYet { starts_in: a - n }
            } else if n <= b {
                Countdown::Live { remaining: b - n }
            } else {
                Countdown::Expired { since: n - b }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Card {
    pub id: String,
    pub author: String,
    pub grantee: String,
    pub work: String,
    pub window: Option<(u64, u64)>,
    pub upstream: Option<String>,
    pub verdict: String,
    pub checks: Value,
    pub failed: Vec<String>,
    pub upstream_entries: usize,
    pub anchors: usize,
    pub countdown: Countdown,
    pub said: String,
    pub latest_anchor: Option<u64>,
    pub handed: Option<String>,
    pub upstream_head: Option<String>,
    pub input: Option<Value>,
    pub from: Option<(crate::supplyx::Level, String)>,
    pub anchored_at: Option<u64>,
    pub record_name: Option<String>,
    pub missed: Vec<crate::widex::Missed>,
    pub lacks: bool,
    pub notices: Option<Value>,
    pub notices_said: Option<String>,
    pub undecided: Option<zikaron_glue::book::Undecided>,
    pub chain_unread: bool,
}

impl Card {
    pub fn told(&self) -> bool {
        told(&self.verdict, &self.gaps(), self.chain_unread)
    }

    pub fn gaps(&self) -> Vec<crate::checkx::Gap> {
        let input = match (&self.from, &self.input, self.undecided) {
            (_, _, Some(u)) => crate::checkx::InputFrom::Undecided(u),
            (None, _, None) => crate::checkx::InputFrom::NoLedger,
            (Some(_), Some(_), None) => crate::checkx::InputFrom::Ledger { entries: self.upstream_entries, lacks: self.lacks },
            (Some(_), None, None) => crate::checkx::InputFrom::Refused(self.said.clone()),
        };
        states(&self.checks)
            .iter()
            .filter_map(|(tok, st)| crate::checkx::notices_gap_of(self.notices_said.as_deref(), tok, st).or_else(|| crate::checkx::gap_of(tok, st, &input, None, &self.missed, false)))
            .collect()
    }
}

pub fn told(verdict: &str, gaps: &[crate::checkx::Gap], chain_unread: bool) -> bool {
    let unread = |g: &crate::checkx::Gap| g.chain_unread() || (chain_unread && *g == crate::checkx::Gap::NotYetAnchored);
    verdict != zikaron_kit::tokens::CheckVerdict::Partial.as_str() || gaps.is_empty() || !gaps.iter().all(unread)
}

#[cfg(test)]
mod told_tests {
    use crate::checkx::Gap;

    #[test]
    fn a_verdict_open_only_for_the_chain_unread_tells_nothing_yet() {
        use zikaron_kit::tokens::CheckVerdict as V;
        let (p, g, f) = (V::Partial.as_str(), V::Green.as_str(), V::Fail.as_str());
        assert!(!super::told(p, &[Gap::ChainUnread("1".into()), Gap::NoTime], false), "a network not read, no chain time");
        assert!(!super::told(p, &[Gap::NoNode], false), "no node");
        assert!(!super::told(p, &[Gap::NotYetAnchored, Gap::NoTime], true), "no chain asked: not anchored is no reading");
        assert!(super::told(p, &[Gap::NotYetAnchored], false), "the chain read whole, not anchored yet");
        assert!(super::told(p, &[Gap::NoLedger, Gap::ChainUnread("1".into())], false), "no ledger: a gap read");
        assert!(super::told(p, &[Gap::LedgerBehind], true), "the issuer's copy behind");
        assert!(super::told(g, &[], false) && super::told(f, &[Gap::NoNode], true), "green and failed");
    }
}

pub fn head_of(items: &[Vec<u8>]) -> Option<String> {
    items
        .iter()
        .filter_map(|b| zikaron_glue::book::check(b).ok())
        .max_by_key(|e| e.seq)
        .map(|e| e.id_hex())
}

pub fn states(checks: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Value::Obj(m) = checks else { return out };
    let Some((_, Value::Arr(rows))) = m.iter().find(|(k, _)| k == "checks") else { return out };
    for r in rows {
        let tok = text(r, "token").unwrap_or("").to_string();
        let st = text(r, "state").unwrap_or("").to_string();
        out.push((tok, st));
    }
    out
}

pub fn review(h: &Held, upstream: &[Vec<u8>], fragment: &Value, now: Option<u64>, anchors: usize) -> Card {
    review_with(h, upstream, fragment, now, anchors, None)
}

pub fn review_with(h: &Held, upstream: &[Vec<u8>], fragment: &Value, now: Option<u64>, anchors: usize, notices: Option<&Value>) -> Card {
    crate::trace::mark(crate::feature::Feature::D6);
    let latest_anchor = latest_anchor_of(fragment);
    let handed = crate::sentinelx::handed_after(upstream, &h.id);
    let upstream_head = head_of(upstream);
    let names = names_in(upstream, &h.work);
    let mut said = String::new();
    let mut lacks = false;
    let mut kept: Option<Value> = None;
    let checked = if zikaron_glue::notices::carries(&h.bytes) {
        match crate::auditx::input_for(&h.bytes, upstream, fragment, None, notices) {
            Ok(input) => {
                let c = zikaron_glue::book::grant_check(&h.bytes, Some(&input), notices, now);
                kept = Some(input);
                c
            }
            Err(f) => {
                said = f.evidence();
                zikaron_glue::book::grant_check(&h.bytes, None, notices, now)
            }
        }
    } else if upstream.is_empty() {
        zikaron_glue::book::grant_check(&h.bytes, None, None, now)
    } else {
        match crate::auditx::input_of(upstream, fragment) {
            Ok(input) => {
                if let Ok(v) = crate::auditx::ask_from(upstream, fragment, Vec::new(), 0, true) {
                    lacks = v.lacks_anchored();
                }
                let c = zikaron_glue::book::grant_check(&h.bytes, Some(&input), None, now);
                kept = Some(input);
                c
            }
            Err(f) => {
                said = f.evidence();
                zikaron_glue::book::grant_check(&h.bytes, None, None, now)
            }
        }
    };
    let failed = match &checked {
        Value::Obj(m) => m
            .iter()
            .find(|(k, _)| k == "failed")
            .and_then(|(_, v)| match v {
                Value::Arr(a) => Some(a.iter().filter_map(|x| match x {
                    Value::Str(s) => Some(s.clone()),
                    _ => None,
                }).collect()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    Card {
        id: h.id.clone(),
        author: h.author.clone(),
        grantee: h.grantee.clone(),
        work: h.work.clone(),
        window: h.window,
        upstream: h.upstream.clone(),
        verdict: checked.member(zikaron_kit::tokens::Key::Verdict.as_str()).and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        checks: checked,
        failed,
        upstream_entries: upstream.len(),
        anchors,
        countdown: countdown(h.window, now),
        said,
        latest_anchor,
        handed,
        upstream_head,
        input: kept,
        from: None,
        anchored_at: crate::auditx::first_anchored(upstream, fragment)
            .and_then(|at| at.into_iter().find(|(id, _)| id.eq_ignore_ascii_case(&h.id)).map(|(_, t)| t)),
        record_name: names.0,
        missed: Vec::new(),
        lacks,
        notices: notices.cloned(),
        notices_said: None,
        undecided: None,
        chain_unread: false,
    }
}

pub struct LineageRead {
    pub fragment: Value,
    pub anchors: usize,
    pub missed: Vec<crate::widex::Missed>,
}

pub fn review_all(items: &[(Held, Vec<Vec<u8>>)], now: Option<u64>, read: Option<&mut dyn FnMut(&[String], &zikaron_anchor::scan::Pick) -> Result<LineageRead, String>>, book: Option<&dyn Fn(&str) -> Option<String>>, notes: &crate::noticex::Notes) -> Vec<Card> {
    let mut read = read;
    struct Plan {
        senders: Option<Vec<String>>,
        behind: bool,
        decision: Option<zikaron_glue::book::Decision>,
        undecided: Option<zikaron_glue::book::Undecided>,
        unread_author: Option<String>,
    }
    let mut plans: Vec<Plan> = Vec::with_capacity(items.len());
    for (i, (h, up)) in items.iter().enumerate() {
        let mut unread_author: Option<String> = None;
        let mut behind = false;
        let n = notes.per.get(i).cloned().flatten();
        let member = zikaron_glue::notices::carries(&h.bytes);
        let lineage: Vec<String> = match zikaron_glue::notices::root_for(&h.bytes, n.as_ref()).filter(|_| member) {
            Some(r) => zikaron_glue::notices::lineage(&h.bytes, &r, n.as_ref()),
            None => Vec::new(),
        };
        let senders = if up.is_empty() && lineage.is_empty() {
            None
        } else if up.is_empty() {
            let mut s = lineage.clone();
            s.push(h.author.clone());
            s.sort();
            s.dedup();
            Some(s)
        } else {
            let mut s = crate::auditx::senders_of(up);
            if s.is_empty() {
                match crate::readerx::who(&h.author) {
                    Ok(a) => s = vec![a.hex()],
                    Err(f) => unread_author = Some(f.evidence()),
                }
            }
            if let (Some(b), false) = (book, s.is_empty()) {
                let root = crate::auditx::root_of(up).unwrap_or_else(|_| h.author.clone());
                let key = b(&root);
                behind = !zikaron_glue::notices::copy_stands(&h.bytes, up, key.as_deref());
                s = crate::readerx::with_key(s, key.as_deref());
            }
            s.extend(lineage.iter().cloned());
            s.sort();
            s.dedup();
            (!s.is_empty()).then_some(s)
        };
        let (mut decision, mut undecided) = (None, None);
        if !behind && (!up.is_empty() || member) {
            let root = if up.is_empty() {
                zikaron_glue::notices::root_for(&h.bytes, n.as_ref()).unwrap_or_else(|| h.author.clone())
            } else {
                crate::auditx::root_of(up).unwrap_or_else(|_| h.author.clone())
            };
            match zikaron_glue::book::decision_set(&zikaron_glue::book::Hold { grant: &h.bytes, root: &root, pile: up }) {
                Ok(d) => decision = Some(d),
                Err(u) => undecided = Some(u),
            }
        }
        plans.push(Plan { senders, behind, decision, undecided, unread_author });
    }
    let mut by_lineage: Vec<(Vec<String>, Result<LineageRead, String>)> = Vec::new();
    if let Some(r) = read.as_mut() {
        let mut keys: Vec<(Vec<String>, Vec<String>)> = Vec::new();
        for p in &plans {
            if let Some(s) = &p.senders {
                let mut key = s.clone();
                key.sort();
                key.dedup();
                if !keys.iter().any(|(k, _)| *k == key) {
                    keys.push((key, s.clone()));
                }
            }
        }
        for (key, senders) in keys {
            let mut decisions: Vec<zikaron_glue::book::Decision> = plans
                .iter()
                .filter(|p| p.senders.as_ref().is_some_and(|s| {
                    let mut k = s.clone();
                    k.sort();
                    k.dedup();
                    k == key
                }))
                .filter_map(|p| p.decision.clone())
                .collect();
            let got = loop {
                let one = r(&senders, &zikaron_glue::book::pick_of(&decisions));
                let Ok(l) = &one else { break one };
                let mut more = false;
                for d in decisions.iter_mut() {
                    if let Ok(zikaron_glue::book::Fast::More) = zikaron_glue::book::fast_input(d, &l.fragment) {
                        more = true;
                    }
                }
                if !more {
                    break one;
                }
            };
            by_lineage.push((key, got));
        }
    }
    let mut cards = Vec::with_capacity(items.len());
    for (i, ((h, up), p)) in items.iter().zip(plans).enumerate() {
        let n = notes.per.get(i).cloned().flatten();
        let Plan { senders, behind, undecided, unread_author, .. } = p;
        let got: Option<&Result<LineageRead, String>> = senders.and_then(|s| {
            let mut key = s;
            key.sort();
            key.dedup();
            by_lineage.iter().find(|(k, _)| *k == key).map(|(_, r)| r)
        });
        let empty = crate::auditx::empty_fragment();
        let (fragment, anchors, missed, failed) = match got {
            Some(Ok(l)) => (&l.fragment, l.anchors, l.missed.clone(), None),
            Some(Err(said)) => (&empty, 0, Vec::new(), Some(said.clone())),
            None => (&empty, 0, Vec::new(), unread_author),
        };
        let mut card = if behind || undecided.is_some() { review(h, &[], fragment, now, anchors) } else { review_with(h, up, fragment, now, anchors, n.as_ref()) };
        card.undecided = undecided;
        card.notices_said = notes.said.get(i).cloned().flatten();
        if behind {
            card.said = crate::lang::t(crate::lang::Key::LedgerBehindBook).to_string();
        }
        card.missed = missed;
        if let (Some(said), true) = (failed, card.said.is_empty()) {
            card.said = said;
        }
        cards.push(card);
    }
    cards
}

pub fn names_in(upstream: &[Vec<u8>], work: &str) -> (Option<String>, Option<String>) {
    use zikaron_glue::book::Kind as EntryType;
    let mut record = None;
    let mut issuer = None;
    for b in upstream {
        let Ok(e) = zikaron_glue::book::check(b) else { continue };
        match e.kind {
            EntryType::Genesis => issuer = crate::ledgerx::facts_of(&e).note,
            EntryType::History if crate::ledgerx::work_of(e.kind, &e.body).map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false) => {
                record = crate::ledgerx::facts_of(&e).record_name();
            }
            _ => {}
        }
    }
    (record, issuer)
}

pub fn latest_anchor_of(fragment: &Value) -> Option<u64> {
    let Value::Obj(m) = fragment else { return None };
    let Some((_, Value::Arr(anchors))) = m.iter().find(|(k, _)| k == "anchors") else { return None };
    anchors
        .iter()
        .filter_map(|a| match a {
            Value::Obj(x) => x.iter().find(|(k, _)| k == "blockTimestamp").and_then(|(_, v)| match v {
                Value::Int(n) => Some(*n),
                _ => None,
            }),
            _ => None,
        })
        .max()
}

pub fn anchor_age(latest: Option<u64>, now: Option<u64>) -> Option<u64> {
    match (latest, now) {
        (Some(t), Some(n)) => Some(n.saturating_sub(t)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Group {
    pub author: String,
    pub anchor_age: Option<u64>,
    pub red: bool,
    pub cards: Vec<Card>,
}

fn broken(c: &Card) -> bool {
    c.failed.iter().any(|f| f == zikaron_kit::tokens::Check::BrokenLedger.as_str())
}

pub fn groups(cards: &[Card], now: Option<u64>) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for c in cards {
        let key = c.author.to_ascii_lowercase();
        match out.iter_mut().find(|g| g.author == key) {
            Some(g) => {
                if g.anchor_age.is_none() {
                    g.anchor_age = anchor_age(c.latest_anchor, now);
                }
                g.red = g.red || broken(c);
                g.cards.push(c.clone());
            }
            None => out.push(Group { author: key, anchor_age: anchor_age(c.latest_anchor, now), red: broken(c), cards: vec![c.clone()] }),
        }
    }
    group_order(out)
}

pub fn group_order(mut groups: Vec<Group>) -> Vec<Group> {
    groups.sort_by(|a, b| b.red.cmp(&a.red).then_with(|| a.author.cmp(&b.author)));
    groups
}

pub fn held_doc(rel: &str) -> Option<crate::local::Doc> {
    if rel.starts_with(&format!("{}/", crate::grantfilex::KEPT)) {
        return Some(crate::local::Doc::KeptGrant);
    }
    if !rel.contains('/') && zikaron_store::layout::parse_entry_file(rel).is_some() {
        return Some(crate::local::Doc::Held);
    }
    None
}

#[cfg(test)]
mod review_tests {
    use super::*;

    fn held(id: char, author: &str) -> Held {
        Held {
            id: format!("0x{}", id.to_string().repeat(64)),
            bytes: b"not a grant".to_vec(),
            author: author.to_string(),
            grantee: String::new(),
            work: String::new(),
            terms: String::new(),
            window: None,
            upstream: None,
            seq: 1,
        }
    }

    fn ledger_of(k: u8) -> (Vec<Vec<u8>>, String) {
        let secret = crate::key::Secret::take([k; 32]).expect("a key");
        let g = crate::entryx::genesis(&secret, "upstream").expect("a genesis");
        (vec![g.bytes], secret.address().expect("its address").hex())
    }

    fn missed(name: &str) -> crate::widex::Missed {
        crate::widex::Missed { chain_id: 10, registry: crate::key::Address([0x22; 20]), from_block: 0, name: name.into(), reading: crate::widex::Reading::Down, nodes: 1 }
    }

    #[test]
    fn the_vault_is_read_once_per_lineage_and_networks_not_read_are_named() {
        let (one, a1) = ledger_of(0x41);
        let (two, a2) = ledger_of(0x42);
        let items = vec![(held('a', &a1), one.clone()), (held('b', &a1), one.clone()), (held('c', &a2), two.clone()), (held('d', &a1), Vec::new())];
        let mut asked: Vec<Vec<String>> = Vec::new();
        let a2c = a2.clone();
        let mut read = |s: &[String], _: &zikaron_anchor::scan::Pick| -> Result<LineageRead, String> {
            asked.push(s.to_vec());
            let other = s[0] == a2c;
            Ok(LineageRead { fragment: crate::auditx::empty_fragment(), anchors: if other { 2 } else { 1 }, missed: if other { vec![missed("OP Mainnet")] } else { Vec::new() } })
        };
        let cards = review_all(&items, None, Some(&mut read), None, &Default::default());
        assert_eq!(asked, vec![vec![a1.clone()], vec![a2.clone()]], "one scan per lineage, none for a grant without upstream");
        assert_eq!(cards.iter().map(|c| c.anchors).collect::<Vec<_>>(), vec![1, 1, 2, 0], "the same lineage reads the same scan");
        assert_eq!(cards.iter().map(|c| c.missed.len()).collect::<Vec<_>>(), vec![0, 0, 1, 0], "a network not read is named on its scan's cards");
        assert_eq!(cards[2].missed[0].name, "OP Mainnet");

        let cards = review_all(&items[..1], None, None, None, &Default::default());
        assert_eq!((cards[0].anchors, cards[0].said.as_str(), cards[0].missed.len()), (0, "", 0));

        let mut failing = |_: &[String], _: &zikaron_anchor::scan::Pick| -> Result<LineageRead, String> { Err("UNREACHABLE:no node answered".into()) };
        let cards = review_all(&items[..2], None, Some(&mut failing), None, &Default::default());
        assert!(cards.iter().all(|c| c.said == "UNREACHABLE:no node answered" && c.anchors == 0), "{:?}", cards.iter().map(|c| c.said.clone()).collect::<Vec<_>>());

        let mut never = |_: &[String], _: &zikaron_anchor::scan::Pick| -> Result<LineageRead, String> { panic!("not scanned") };
        let cards = review_all(&[(held('e', "not an address"), vec![b"no entry".to_vec()])], None, Some(&mut never), None, &Default::default());
        assert!(!cards[0].said.is_empty());
    }
}
