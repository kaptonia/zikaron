//! Grant vault: import grant documents (files or payloads), verify their signatures and store them. The
//! bytes are the credential: losing the vault means losing the contract.
//!
//! Each step has its own owner:
//!
//! * Take: a file or a `zikaron-grant:` payload. Decoding belongs to the kit crate's `badge::decode` (kit law
//!   §6: over cap, bad segment, not an entry, not a grant, upstream on the first segment and broken links each
//!   have a token and are never partially rendered); this layer decodes nothing itself.
//! * Admit: the core's thirteen steps (`entry::check`) plus this vault's check that it is a grant.
//! * Store: into the home's `grants-held/`, named like the store crate's entry names (id without 0x plus
//!   `.entry`), so the reader and the verifier can read it by name. Writing goes only through the glue
//!   crate's landing path, and an existing file is refused.
//! * Re-check: the six checks belong to the kit crate's `check::grant_check` (four consumers, one
//!   implementation), the audit input to `input::assemble` (via `auditx::input_of`), the upstream label to
//!   the core; window countdowns use only chain time.
//!
//! Four malicious payload forms are refused and never stored: over cap (`E_BADGE_CAP`); bad signature (the
//! core's signature token carried out through `E_BADGE_ENTRY`, or a raw file that fails the thirteen steps);
//! false upstream (upstream on the first segment is `E_BADGE_INCOMPLETE`); self-reference and broken links
//! (`E_BADGE_LINK` when the byte chain between segments does not link: a segment's upstream can only be the
//! previous segment's id). When taking refuses, there is nothing to store. An entry's id covers its own body,
//! so a single grant whose upstream equals its own id cannot be written, and no extra check is needed.
//!
//! Mirror bundle export and restore carry every file under `grants-held/` with its digest ([`held_rows`]
//! lists them).

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::PathBuf;
use zikaron::entry::{self, Entry};
use zikaron::json::Value;

/// Take. Pasted payload text is decoded; a path reads that file, decoding it when it holds a payload and
/// otherwise treating it as entry bytes. Returns one or more grants' bytes (one per segment of a multi-hop
/// payload).
pub fn take(typed: &str) -> Result<Vec<Vec<u8>>, Fault> {
    // Parsing lives only in `payloadx` (shared with the grant check page): a payload in a file is decoded
    // after removing ASCII whitespace.
    Ok(crate::payloadx::take(typed)?.1)
}

/// Decode a payload with the kit crate; refusals are kit law tokens, with segment number and inner token.
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

/// Admit: the core's thirteen steps, and it must be a grant.
pub fn admit(bytes: &[u8]) -> Result<Entry, Fault> {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, CLI) are traced
    // too.
    crate::trace::mark(crate::feature::Feature::D6);
    let e = entry::check(bytes).map_err(Fault::entry_refused)?;
    if e.kind != zikaron::tokens::EntryType::Grant {
        return Err(Fault::known(Known::NotAGrant, e.kind.as_str().to_string()));
    }
    Ok(e)
}

/// Where one vault item is stored: the same name shape as ledger entries, keyed by the names key (`names`),
/// so a locked vault does not reveal which grant it is.
pub fn path_of(home: &Home, id: &str) -> Result<PathBuf, crate::fault::Fault> {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, CLI) are traced
    // too.
    crate::trace::mark(crate::feature::Feature::D6);
    Ok(home.dir(Slot::GrantsHeld).join(format!("{}{}", held_stem(id)?, zikaron_store::layout::ENTRY_SUFFIX)))
}

/// The keyed stem a held grant's files share (its entry and its verdict cache).
pub fn held_stem(id: &str) -> Result<String, crate::fault::Fault> {
    Ok(crate::names::key()?.name(crate::names::Logical::Held(id)))
}

/// Store. Written only after admission; an existing item is refused; writing goes only through the glue
/// crate's landing. Returns the id.
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

/// Store several, all or none. Each is admitted first and staged beside its target in the same directory
/// (`.part`); only when all are staged is each renamed into place. If staging fails, every staged file is
/// removed and nothing remains; if renaming fails, the error names the ones that landed.
pub fn store_all(home: &Home, items: &[&Vec<u8>]) -> Result<Vec<String>, Fault> {
    crate::trace::mark(crate::feature::Feature::D6);
    // Sealing needs the vault open, so this fails before anything is staged.
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
            return Err(Fault::known(Known::Landing, crate::lang::filln(crate::lang::Key::Tail220, &[&(t.code()).to_string(), &(t.subject()).to_string(), &(part.display()).to_string()])));
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

/// List the vault, including items that fail admission. Files with entry names that fail admission go into a
/// second list by name (the bytes are the credential; a broken one must be noticed, never treated as
/// absent). No vault directory means an empty vault; an unreadable one is an error.
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
        // An unreadable file goes into the second list by name and the rest are listed as usual (one
        // unreadable file should not make the whole vault look empty).
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

/// Every file in the vault directory (relative path, opened bytes), for mirroring. Recursive, sorted by path.
/// Sealed files are opened according to their location (`<id>.entry` held grants, `files/` kept grant
/// files); anything else is carried as is.
pub fn held_rows(home: &Home) -> Result<Vec<(String, Vec<u8>)>, Fault> {
    let key = crate::keybox::local_key()?;
    let root = home.dir(Slot::GrantsHeld);
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(at) = stack.pop() {
        // An unreadable directory must be an error: a bundle missing a level is a subset that mirroring would
        // still verify green, and a restore would lose that level.
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
                // Opened and named by its logical name (the keyed on-disk name means nothing to a reader of
                // the bundle).
                let (rel, bytes) = match held_doc(&rel) {
                    Some(doc) if crate::local::is_sealed(&bytes) => {
                        let plain = crate::local::open_with(&key, &crate::local::expect_at(&p, doc)?, &bytes, &rel)?;
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

/// Window countdown, from chain time only: without the current chain time there is no reading.
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

/// A card: the six checks' three-state results and verdict, countdown, upstream label. Every field comes from
/// the re-check pass.
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
    /// Which half of this re-check failed (upstream unreadable, basis not built); empty when both succeeded.
    pub said: String,
    /// The block time of the upstream ledger's latest anchor (the largest in the fragment); `None` when there
    /// is none.
    pub latest_anchor: Option<u64>,
    /// Who the latest succession in the upstream ledger handed it to (the sentinel's yellow note "ledger
    /// changed hands"); `None` when there is none.
    pub handed: Option<String>,
    /// The id of the upstream ledger's head entry (the highest seq); `None` when the upstream bytes could not
    /// be obtained. The watch row uses it to identify an upstream state: any change to the upstream, even one
    /// entry or one character, is a new state.
    pub upstream_head: Option<String>,
    /// The audit input used by this pass's six checks, kept for the chain check (kit law §10.5) to reuse per
    /// hop; none when it could not be built.
    pub input: Option<Value>,
    /// Which level the upstream ledger came from and where (`supplyx`); none when no level has it.
    pub from: Option<(crate::supplyx::Level, String)>,
    /// First-anchor block time of this grant in the upstream ledger, from this pass's fragment.
    pub anchored_at: Option<u64>,
    /// The record's name in the issuer's ledger (the note of the entry that anchored the granted record);
    /// `None` when that ledger does not say or could not be read.
    pub record_name: Option<String>,
    /// The networks this pass could not read (the main network or a read-only one), each named on the card;
    /// anchors only they could show read as unknown, never as unanchored.
    pub missed: Vec<crate::widex::Missed>,
    /// The issuer ledger's statement (its genesis note); `None` when it does not say or could not be read.
    pub issuer_name: Option<String>,
}

/// The head id of a set of entries: the highest-seq entry among those passing the core's thirteen steps.
/// `None` for an empty set.
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

/// Re-check one item from the upstream bytes and fragment: the audit input is assembled, the six checks go to
/// the kit crate, the label to the core. With no upstream bytes only checks one and five can run (the others
/// answer UNKNOWN, verdict PARTIAL), and the card says so.
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
        missed: Vec::new(),
    }
}

/// One lineage's reading of the chains for the vault re-check: the fragment scanned (with a window that
/// covers nothing for each network not read), how many anchors it found, and the networks not read.
pub struct LineageRead {
    pub fragment: Value,
    pub anchors: usize,
    pub missed: Vec<crate::widex::Missed>,
}

/// The vault re-check over held grants, each with its upstream ledger's bytes (empty when not obtained).
/// Grants with the same upstream lineage (the senders the scan looks for: the author and the keys its ledger
/// handed to) share one scan, `read(senders)`. A grant without upstream bytes, or with no chain to read
/// (`read` is `None`), is checked on an empty fragment (its chain checks honestly unknown). A failed scan
/// leaves its cards on an empty fragment with the reason on each; networks a scan did not read are named on
/// each of its cards.
pub fn review_all(items: &[(Held, Vec<Vec<u8>>)], now: Option<u64>, read: Option<&mut dyn FnMut(&[String]) -> Result<LineageRead, String>>) -> Vec<Card> {
    let mut read = read;
    let mut by_lineage: Vec<(Vec<String>, Result<LineageRead, String>)> = Vec::new();
    let mut cards = Vec::with_capacity(items.len());
    for (h, up) in items {
        // The lineage as every read path takes it (`readerx::basis_for`): the senders the upstream ledger
        // names, else its author.
        let mut unread_author: Option<String> = None;
        let senders = if up.is_empty() {
            None
        } else {
            let mut s = crate::auditx::senders_of(up);
            if s.is_empty() {
                match crate::readerx::who(&h.author) {
                    Ok(a) => s = vec![a.hex()],
                    Err(f) => unread_author = Some(f.evidence()),
                }
            }
            (!s.is_empty()).then_some(s)
        };
        let got: Option<&Result<LineageRead, String>> = match (senders, read.as_mut()) {
            (Some(s), Some(r)) => {
                // Grouped by the set of senders; scanned with the lineage as computed.
                let mut key = s.clone();
                key.sort();
                key.dedup();
                let at = match by_lineage.iter().position(|(k, _)| *k == key) {
                    Some(i) => i,
                    None => {
                        let one = r(&s);
                        by_lineage.push((key, one));
                        by_lineage.len() - 1
                    }
                };
                Some(&by_lineage[at].1)
            }
            _ => None,
        };
        let empty = crate::auditx::empty_fragment();
        let (fragment, anchors, missed, failed) = match got {
            Some(Ok(l)) => (&l.fragment, l.anchors, l.missed.clone(), None),
            Some(Err(said)) => (&empty, 0, Vec::new(), Some(said.clone())),
            None => (&empty, 0, Vec::new(), unread_author),
        };
        let mut card = review(h, up, fragment, now, anchors);
        card.missed = missed;
        if let (Some(said), true) = (failed, card.said.is_empty()) {
            card.said = said;
        }
        cards.push(card);
    }
    cards
}

/// The granted record's name and the ledger's statement, read from the issuer's ledger: the note of the
/// history entry that anchored `work`, and the genesis note. Entries that fail the core's checks are ignored.
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

/// Group by issuer. A group's label and anchor age come from its cards (same upstream, same scan); no volume
/// numbers (no indexer).
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

/// Red-label groups first; the rest by issuer byte order, so two passes over one vault give the same list.
pub fn group_order(mut groups: Vec<Group>) -> Vec<Group> {
    groups.sort_by(|a, b| b.red.cmp(&a.red).then_with(|| a.author.cmp(&b.author)));
    groups
}

/// Which sealed kind a file in the vault directory is, by its location (`None`: not a sealed vault file).
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

    /// An upstream ledger of one genesis by the key `k`, and that key's address.
    fn ledger_of(k: u8) -> (Vec<Vec<u8>>, String) {
        let secret = crate::key::Secret::take([k; 32]).expect("a key");
        let g = crate::entryx::genesis(&secret, "upstream").expect("a genesis");
        (vec![g.bytes], secret.address().expect("its address").hex())
    }

    fn missed(name: &str) -> crate::widex::Missed {
        crate::widex::Missed { chain_id: 10, registry: crate::key::Address([0x22; 20]), from_block: 0, name: name.into(), reading: crate::widex::Reading::Down }
    }

    /// One scan per upstream lineage, shared by its grants; no scan without upstream bytes or a chain to read;
    /// a failed scan gives its reason on its cards; unread networks are named on that scan's cards; an
    /// unreadable author is reported.
    #[test]
    fn the_vault_is_read_once_per_lineage_and_networks_not_read_are_named() {
        let (one, a1) = ledger_of(0x41);
        let (two, a2) = ledger_of(0x42);
        let items = vec![(held('a', &a1), one.clone()), (held('b', &a1), one.clone()), (held('c', &a2), two.clone()), (held('d', &a1), Vec::new())];
        let mut asked: Vec<Vec<String>> = Vec::new();
        let a2c = a2.clone();
        let mut read = |s: &[String]| -> Result<LineageRead, String> {
            asked.push(s.to_vec());
            let other = s[0] == a2c;
            Ok(LineageRead { fragment: crate::auditx::empty_fragment(), anchors: if other { 2 } else { 1 }, missed: if other { vec![missed("OP Mainnet")] } else { Vec::new() } })
        };
        let cards = review_all(&items, None, Some(&mut read));
        assert_eq!(asked, vec![vec![a1.clone()], vec![a2.clone()]], "one scan per lineage, none for a grant without upstream");
        assert_eq!(cards.iter().map(|c| c.anchors).collect::<Vec<_>>(), vec![1, 1, 2, 0], "the same lineage reads the same scan");
        assert_eq!(cards.iter().map(|c| c.missed.len()).collect::<Vec<_>>(), vec![0, 0, 1, 0], "a network not read is named on its scan's cards");
        assert_eq!(cards[2].missed[0].name, "OP Mainnet");

        // No chain to read: nothing scanned, nothing said of a scan.
        let cards = review_all(&items[..1], None, None);
        assert_eq!((cards[0].anchors, cards[0].said.as_str(), cards[0].missed.len()), (0, "", 0));

        // A scan that fails: its cards on an empty fragment, each saying why.
        let mut failing = |_: &[String]| -> Result<LineageRead, String> { Err("UNREACHABLE:no node answered".into()) };
        let cards = review_all(&items[..2], None, Some(&mut failing));
        assert!(cards.iter().all(|c| c.said == "UNREACHABLE:no node answered" && c.anchors == 0), "{:?}", cards.iter().map(|c| c.said.clone()).collect::<Vec<_>>());

        // An author that does not read (and upstream bytes that name no sender): said, not scanned.
        let mut never = |_: &[String]| -> Result<LineageRead, String> { panic!("not scanned") };
        let cards = review_all(&[(held('e', "not an address"), vec![b"no entry".to_vec()])], None, Some(&mut never));
        assert!(!cards[0].said.is_empty());
    }
}
