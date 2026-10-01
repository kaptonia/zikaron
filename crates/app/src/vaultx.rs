//! Grant vault. Import grant documents (files or payloads), verify their signatures and store them; the
//! bytes are the credential, and losing the vault is losing the contract.
//!
//! ─── Take, admit, store, re-check: each has its own owner ───
//!
//! Take: a file, or a `zikaron-grant:` payload; decoding belongs to the kit crate's `badge::decode` (kit law
//! §6: over cap, bad segment, not an entry, not a grant, upstream on the first segment and broken links each
//! have a token, never partially rendered); this layer decodes not one byte.
//! Admit: the core's thirteen steps (`entry::check`) plus this vault's shape gate: it is a grant.
//! Store: into the home's `grants-held/`, named by the store crate's entry names (id without 0x plus
//! `.entry`), so the reader and the verifier read it with their name-based reading; writing goes only through
//! the glue crate's landing path, and an existing file is refused.
//! Re-check: the six checks belong to the kit crate's `check::grant_check` (four consumers, one
//! implementation), the audit input to `input::assemble` (through `auditx::input_of`), the upstream label to
//! the core; window countdowns use only chain time and now.
//!
//! ─── Four malicious payload forms, each refused by name and never stored ───
//!
//! Over cap (`E_BADGE_CAP`), bad signature (the core's sig token carried out through `E_BADGE_ENTRY`, or a
//! raw file that fails the thirteen steps), false upstream (upstream on the first segment is
//! `E_BADGE_INCOMPLETE`), self-reference and broken links (when the byte chain between segments does not
//! link, `E_BADGE_LINK`: a segment's upstream can only be the previous segment's id; pointing back at itself
//! or elsewhere does not link). When taking refuses, storing has nothing to store. An entry's id covers its
//! own body, so a single grant whose upstream equals its own id cannot be written, and no extra shape gate is
//! needed.
//!
//! ─── Mirroring covers this directory ───
//!
//! The mirror's bundle export and restore carry every file under `grants-held/` along with its digest (`held_rows` is
//! where the files are listed).

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::PathBuf;
use zikaron::entry::{self, Entry};
use zikaron::json::Value;

/// Take. Pasted payload text is decoded; a path reads that file, decoding it when it holds a payload and
/// otherwise treating it as entry bytes. Returns one or more grants' bytes (one per segment of a multi-hop
/// payload).
pub fn take(typed: &str) -> Result<Vec<Vec<u8>>, Fault> {
    // The reading lives only in `payloadx` (shared with the grant check page): a payload in a file is decoded
    // after removing ASCII whitespace.
    Ok(crate::payloadx::take(typed)?.1)
}

/// The payload is decoded by the kit crate; refusals are kit law tokens, with segment number and inner token.
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

/// Admit. The core's thirteen steps; it is a grant.
pub fn admit(bytes: &[u8]) -> Result<Entry, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::D6);
    let e = entry::check(bytes).map_err(Fault::entry_refused)?;
    if e.kind != zikaron::tokens::EntryType::Grant {
        return Err(Fault::known(Known::NotAGrant, e.kind.as_str().to_string()));
    }
    Ok(e)
}

/// Where one vault item lands. One name, one home: the same shape of name as ledger entries, keyed by the
/// names key (`names`), so locked it does not say the grant.
pub fn path_of(home: &Home, id: &str) -> Result<PathBuf, crate::fault::Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::D6);
    Ok(home.dir(Slot::GrantsHeld).join(format!("{}{}", held_stem(id)?, zikaron_store::layout::ENTRY_SUFFIX)))
}

/// The keyed stem a held grant's files share (its entry and its verdict cache).
pub fn held_stem(id: &str) -> Result<String, crate::fault::Fault> {
    Ok(crate::names::key()?.name(crate::names::Logical::Held(id)))
}

/// Store. Written only after admission; an existing one is refused by name; writing goes only through the
/// glue crate's landing. Returns the id.
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
    // Sealed (`local::Doc::Held`): the credential bytes are local data.
    crate::local::land(&at, crate::local::Doc::Held, bytes)?;
    Ok(id)
}

/// Store several, all or none. Each is admitted first and staged beside its place in the same directory
/// (`.part`); only when all are staged is each renamed into place. If staging fails, every staged file is
/// removed and nothing remains on disk; if the renaming stage fails, it names which ones landed.
pub fn store_all(home: &Home, items: &[&Vec<u8>]) -> Result<Vec<String>, Fault> {
    crate::trace::mark(crate::feature::Feature::D6);
    // Sealing needs the vault open: refused before anything is staged.
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
        let sealed = crate::local::seal_with(&key, crate::local::Doc::Held, b)?;
        if let Err(t) = zikaron_glue::landing::land_bytes(&part, &sealed) {
            for (_, p, _) in &staged {
                let _ = std::fs::remove_file(p);
            }
            let _ = std::fs::remove_file(&part);
            return Err(Fault::known(Known::Landing, crate::lang::filln(crate::lang::Key::Tail220, &[&(t.code()).to_string(), &(t.subject()).to_string(), &(part.display()).to_string()])));
        }
        staged.push((id, part, at));
    }
    let mut landed: Vec<String> = Vec::new();
    let mut rest = staged.into_iter();
    while let Some((id, part, at)) = rest.next() {
        if let Err(x) = std::fs::rename(&part, &at) {
            let _ = std::fs::remove_file(&part);
            for (_, p, _) in rest {
                let _ = std::fs::remove_file(p);
            }
            let f = crate::fault::classify(&x, &at.display().to_string());
            return Err(Fault::known(Known::Landing, crate::lang::filln(crate::lang::Key::Tail221, &[&(f.said()).to_string(), &(f.tail()).to_string(), &(landed.join(" ")).to_string()])));
        }
        landed.push(id);
    }
    Ok(landed)
}

/// One vault item.
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

/// List the vault. Only files whose names match the store crate's entry names and that pass admission; sorted
/// by id, so two passes over one vault give the same list.
pub fn held(home: &Home) -> Result<Vec<Held>, Fault> {
    held_all(home).map(|(h, _)| h)
}

/// List the vault, including items that fail admission. Files with entry names that fail admission go by name
/// into a second list (the bytes are the credential; a broken one must be noticed, never treated as absent).
/// No vault directory means an empty vault; an unreadable one is refused by name.
pub fn held_all(home: &Home) -> Result<(Vec<Held>, Vec<crate::verifyx::Rejected>), Fault> {
    let dir = home.dir(Slot::GrantsHeld);
    let mut out: Vec<Held> = Vec::new();
    let mut bad: Vec<crate::verifyx::Rejected> = Vec::new();
    let key = crate::keybox::local_key()?;
    // Walk recursively (items restored by mirroring can carry subdirectories, and `held_rows` also collects
    // recursively: both sides must see the same files).
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
        // An unreadable file goes by name into the second list, and the others are listed as usual (one
        // unreadable file should not make the whole vault look "never run").
        let bytes = match std::fs::read(&p).map_err(|x| crate::fault::classify(&x, &p.display().to_string())).and_then(|b| crate::local::open_with(&key, crate::local::Doc::Held, &b, &name)) {
            Ok(b) => b,
            Err(f) => {
                bad.push(crate::verifyx::Rejected::plain(name, format!("{} · {}", f.said(), f.tail())));
                continue;
            }
        };
        let en = match admit(&bytes) {
            Ok(en) => en,
            Err(f) => {
                bad.push(crate::verifyx::Rejected::plain(name, format!("{} · {}", f.said(), f.tail())));
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

/// Every file in the vault directory (relative path, bytes opened), for mirroring to carry. Recursive, sorted
/// by path. Sealed files are opened by where they lie (`<id>.entry` held grants, `files/` kept grant files);
/// anything else goes as it is.
pub fn held_rows(home: &Home) -> Result<Vec<(String, Vec<u8>)>, Fault> {
    let key = crate::keybox::local_key()?;
    let root = home.dir(Slot::GrantsHeld);
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(at) = stack.pop() {
        // An unreadable level must be refused: a bundle missing a level is a subset, mirroring would still
        // verify it green, and a restore would lose that level.
        let listing = std::fs::read_dir(&at).map_err(|x| crate::fault::classify(&x, &at.display().to_string()))?;
        for e in listing.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                // The verdict cache is not vault content: it does not go into the bundle.
                if p.file_name().map(|n| crate::lastread::is_cache(&n.to_string_lossy())).unwrap_or(false) {
                    continue;
                }
                let rel = p
                    .strip_prefix(&root)
                    .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/"))
                    .unwrap_or_default();
                let bytes = std::fs::read(&p).map_err(|x| crate::fault::classify(&x, &p.display().to_string()))?;
                // Opened, and named by what it stands for (its name on disk is keyed and says nothing to a
                // reader of the bundle).
                let (rel, bytes) = match held_doc(&rel) {
                    Some(doc) if crate::local::is_sealed(&bytes) => {
                        let plain = crate::local::open_with(&key, doc, &bytes, &rel)?;
                        let room = format!("{}/", Slot::GrantsHeld.as_str());
                        let logical = crate::local::logical_rel(doc, &format!("{room}{rel}"), &plain)?;
                        (logical.strip_prefix(&room).map(str::to_string).unwrap_or(logical), plain)
                    }
                    _ => (rel, bytes),
                };
                out.push((rel, bytes));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Window countdown. Only chain time and now: without now there is no reading.
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

/// A card: the six checks' three states and verdict, countdown, upstream label. Every cell is what the
/// re-check pass brought back.
#[derive(Clone, Debug)]
pub struct Card {
    pub id: String,
    pub author: String,
    pub grantee: String,
    pub work: String,
    pub window: Option<(u64, u64)>,
    pub upstream: Option<String>,
    /// The kit crate's verdict (GREEN / PARTIAL / FAIL).
    pub verdict: String,
    /// The kit crate's six-check result object, unchanged.
    pub checks: Value,
    pub failed: Vec<String>,
    /// The upstream ledger's audit label (from the core); empty when the upstream bytes could not be
    /// obtained.
    pub upstream_label: String,
    pub upstream_entries: usize,
    pub anchors: usize,
    pub countdown: Countdown,
    /// Which half of this re-check failed (upstream unreadable, basis not built), by name; empty when both
    /// succeeded.
    pub said: String,
    /// The block time of the upstream ledger's latest anchor (the largest in the fragment); none when there
    /// is none.
    pub latest_anchor: Option<u64>,
    /// Who the latest succession in the upstream ledger handed it to (the sentinel's yellow note "ledger
    /// changed hands"); none when there is none.
    pub handed: Option<String>,
    /// The id of the upstream ledger's head entry now (the highest seq). None when the upstream bytes could
    /// not be obtained. The watch row uses it as the identity of "this upstream episode": the upstream
    /// growing by one entry or changing one character is a new episode.
    pub upstream_head: Option<String>,
    /// The audit input used by this pass's six checks, kept for the chain check (kit law §10.5) to reuse per
    /// hop; none when it could not be built.
    pub input: Option<Value>,
    /// Which level the upstream ledger came from and where (`supplyx`); none when no level has it.
    pub from: Option<(crate::supplyx::Level, String)>,
    /// First-anchor block time of this grant in the upstream ledger, from this pass's fragment.
    pub anchored_at: Option<u64>,
    /// The record's name in the issuer's ledger (the note of the entry that anchored the granted record);
    /// none when that ledger does not say or could not be read.
    pub record_name: Option<String>,
    /// The issuer ledger's statement (its genesis note); none when it does not say or could not be read.
    pub issuer_name: Option<String>,
}

/// The head id of a stack of bytes: the highest-seq entry among those passing the core's thirteen steps. None
/// for an empty stack.
pub fn head_of(items: &[Vec<u8>]) -> Option<String> {
    items
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .max_by_key(|e| e.seq)
        .map(|e| e.id_hex())
}

/// The six checks: (token, state). Read from the kit crate's result object; this layer does not re-judge.
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

/// Re-check one item. Upstream bytes and fragment go in: the audit input is assembled, the six checks go to
/// the kit crate, the label to the core. With empty upstream bytes only checks one and five can be done (the
/// others answer UNKNOWN, verdict PARTIAL), and it says so.
pub fn review(h: &Held, upstream: &[Vec<u8>], fragment: &Value, now: Option<u64>, anchors: usize) -> Card {
    crate::trace::mark(crate::feature::Feature::D6);
    let latest_anchor = latest_anchor_of(fragment);
    let handed = crate::sentinelx::handed_after(upstream, h.seq);
    let upstream_head = head_of(upstream);
    let names = names_in(upstream, &h.work);
    let mut said = String::new();
    let mut label = String::new();
    let mut kept: Option<Value> = None;
    let checked = if upstream.is_empty() {
        zikaron_kit::check::grant_check(&h.bytes, None, now)
    } else {
        match crate::auditx::input_of(upstream, fragment) {
            Ok(input) => {
                label = crate::auditx::ask_from(upstream, fragment, Vec::new(), 0, true)
                    .map(|v| v.label)
                    .unwrap_or_default();
                let c = zikaron_kit::check::grant_check(&h.bytes, Some(&input), now);
                kept = Some(input);
                c
            }
            Err(f) => {
                said = f.evidence();
                zikaron_kit::check::grant_check(&h.bytes, None, now)
            }
        }
    };
    let failed = match &checked.value {
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
        verdict: checked.verdict.as_str().to_string(),
        checks: checked.value,
        failed,
        upstream_label: label,
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
        issuer_name: names.1,
    }
}

/// The granted record's name and the ledger's statement, read from the issuer's ledger: the note of the
/// history entry that anchored `work`, and the genesis note. Entries that do not pass the core's checks say
/// nothing.
pub fn names_in(upstream: &[Vec<u8>], work: &str) -> (Option<String>, Option<String>) {
    use zikaron::tokens::EntryType;
    let mut record = None;
    let mut issuer = None;
    for b in upstream {
        let Ok(e) = zikaron::entry::check(b) else { continue };
        match e.kind {
            EntryType::Genesis => issuer = crate::ledgerx::facts_of(&e).note,
            EntryType::History if crate::ledgerx::work_of(e.kind, &e.body).map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false) => {
                record = crate::ledgerx::facts_of(&e).note;
            }
            _ => {}
        }
    }
    (record, issuer)
}

/// The block time of the latest anchor in the fragment. Reads the anchoring crate's fragment; no separate
/// scan.
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

/// Anchor age: the chain's current time minus the latest anchor's time. With either missing there is no
/// reading (chain time only).
pub fn anchor_age(latest: Option<u64>, now: Option<u64>) -> Option<u64> {
    match (latest, now) {
        (Some(t), Some(n)) => Some(n.saturating_sub(t)),
        _ => None,
    }
}

/// One group of the multi-upstream register: the items of one issuer.
#[derive(Clone, Debug)]
pub struct Group {
    pub author: String,
    /// That upstream's audit label (from the core); empty when not read.
    pub label: String,
    pub anchor_age: Option<u64>,
    /// The label is red (BROKEN_CHAIN).
    pub red: bool,
    pub cards: Vec<Card>,
}

/// Group by issuer. A group's label and anchor age come from its cards' readings (same upstream, same scan);
/// no volume numbers at all (no indexer).
pub fn groups(cards: &[Card], now: Option<u64>) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for c in cards {
        let key = c.author.to_ascii_lowercase();
        match out.iter_mut().find(|g| g.author == key) {
            Some(g) => {
                if g.label.is_empty() {
                    g.label = c.upstream_label.clone();
                }
                if g.anchor_age.is_none() {
                    g.anchor_age = anchor_age(c.latest_anchor, now);
                }
                g.red = g.red || c.upstream_label == zikaron::tokens::Label::BrokenChain.as_str();
                g.cards.push(c.clone());
            }
            None => out.push(Group {
                author: key,
                label: c.upstream_label.clone(),
                anchor_age: anchor_age(c.latest_anchor, now),
                red: c.upstream_label == zikaron::tokens::Label::BrokenChain.as_str(),
                cards: vec![c.clone()],
            }),
        }
    }
    group_order(out)
}

/// Red-label groups first (hard negative ordering); the rest by issuer byte order, so two passes over one
/// vault give the same list.
pub fn group_order(mut groups: Vec<Group>) -> Vec<Group> {
    groups.sort_by(|a, b| b.red.cmp(&a.red).then_with(|| a.author.cmp(&b.author)));
    groups
}

/// Which sealed kind a file in the vault room is, by where it lies (`None`: not a sealed vault file).
pub fn held_doc(rel: &str) -> Option<crate::local::Doc> {
    if rel.starts_with(&format!("{}/", crate::grantfilex::KEPT)) {
        return Some(crate::local::Doc::KeptGrant);
    }
    if !rel.contains('/') && zikaron_store::layout::parse_entry_file(rel).is_some() {
        return Some(crate::local::Doc::Held);
    }
    None
}
