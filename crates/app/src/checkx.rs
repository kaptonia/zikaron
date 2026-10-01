//! Grant check page. A tab that needs no identity, with a matching CLI: take a payload or a grant file,
//! show the six checks each as a three-state light plus an overall verdict; multiple hops get one row of
//! lights per hop and one verdict for the chain. PARTIAL is a state of its own, and the basis is shown with
//! the verdict.
//!
//! ─── Judged in one place only ───
//!
//! The six checks belong to `zikaron_kit::check::grant_check` and the chain check to
//! `zikaron_kit::check::chain_check` (kit law §10), the same implementation as the CLI's `check-grant` /
//! `chain-check` and the vault re-check: the same bytes, the same audit input and the same `now` give
//! byte-identical result objects on the tab and in the CLI. This layer judges no check; it only lays the kit
//! crate's result object out for the face. PARTIAL is the kit crate's own state, and nothing here folds it
//! into green or red.
//!
//! ─── Zero permissions ───
//!
//! This file reads no keychain, writes no archive and sends no transaction: input is read-only (payload
//! bytes, grant files, ledger directories), the chain is read-only (anchor scans through
//! `auditx::scan_agreed`, converged by the endpoint rule, single sources flagged), and verdicts live only in
//! memory. It works without a home: endpoints and basis are filled in on the page and not saved to settings.
//!
//! ─── Absence is not guilt ───
//!
//! Without a ledger directory only checks one and five can be done; the others answer UNKNOWN and the kit
//! crate judges PARTIAL. A ledger with no genesis cannot build the audit input (`auditx::root_of` answers
//! NO_GENESIS, read the same way as the record verifier); that hop says "no input" and the kit crate still reads
//! it as undecided. An unreachable chain says so in the basis column, and the verdict does not turn red for
//! it.
//!
//! ─── QR entry ───
//!
//! The badge packer encodes the payload text into a QR code; scanning it yields exactly that
//! `zikaron-grant:` text, and pasting it here takes the payload form. Entry and exit match: the same text,
//! the same `badge::decode`.

use crate::fault::{Fault, Known};
use std::path::PathBuf;
use zikaron::json::Value;
use zikaron_kit::tokens::Key;

/// Which form the input is: the same closed table as the vault (`payloadx::Form`).
pub use crate::payloadx::Form;

/// The hops taken in, from the root (the payload's segment order is the chain order, as the kit crate reads
/// it).
#[derive(Clone, Debug)]
pub struct Hops {
    pub form: Form,
    pub hops: Vec<Vec<u8>>,
    /// When the input is a grant file: its path and the opened bundle (its ledger is supply at the "record
    /// bundle" level, its pointer at the "publish address" level).
    pub file: Option<(PathBuf, crate::grantfilex::Opened)>,
}

/// Take in. The reading lives in `payloadx` only (shared with the vault): payload text and payload files
/// are decoded by the kit crate; grant files are opened and verified; entry files pass the core's thirteen
/// steps and must be a grant. Empty is refused by name.
pub fn hops_of(typed: &str) -> Result<Hops, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, the
    // CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::P1);
    let taken = crate::payloadx::take_full(typed)?;
    let (form, hops, file) = (taken.form, taken.hops, taken.file);
    if form == Form::EntryFile {
        let bytes = &hops[0];
        let e = zikaron::entry::check(bytes).map_err(Fault::entry_refused)?;
        if e.kind != zikaron::tokens::EntryType::Grant {
            return Err(Fault::known(Known::NotAGrant, e.kind.as_str().to_string()));
        }
    }
    Ok(Hops { form, hops, file })
}

/// How a hop's audit input was obtained. Closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputFrom {
    /// No ledger given: the kit crate reads it as undecided (absence is not guilt).
    NoLedger,
    /// A ledger was given and the input was built.
    Ledger { entries: usize },
    /// A ledger was given and the input could not be built, by name (NO_GENESIS and others, as
    /// `auditx::root_of` reads them).
    Refused(String),
}

/// One hop's reading. Every cell is read from the kit crate's result object; this layer does not re-judge.
#[derive(Clone, Debug)]
pub struct HopRead {
    pub id: String,
    pub author: String,
    pub grantee: String,
    pub work: String,
    pub upstream: Option<String>,
    pub window: Option<(u64, u64)>,
    /// GREEN / PARTIAL / FAIL.
    pub verdict: String,
    /// The six checks: (token, state).
    pub lights: Vec<(String, String)>,
    pub failed: Vec<String>,
    pub input: InputFrom,
    /// The terms digest the grant records (hex32).
    pub terms: String,
    /// First-anchor block time of this grant in the issuer's ledger, from this check's fragment.
    pub anchored_at: Option<u64>,
}

/// The whole chain's reading (only with more than one hop).
#[derive(Clone, Debug)]
pub struct ChainRead {
    pub verdict: String,
    pub token: String,
    pub failing_kind: String,
    pub failing_index: Option<u64>,
    /// Per link: true, false, undecided.
    pub links: Vec<Option<bool>>,
}

fn text(v: &Value, k: &str) -> Option<String> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

/// The six checks: (token, state), read from the kit crate's result object.
pub fn lights(checked: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(Value::Arr(rows)) = member(checked, Key::Checks.as_str()) else { return out };
    for r in rows {
        out.push((text(r, Key::Token.as_str()).unwrap_or_default(), text(r, Key::State.as_str()).unwrap_or_default()));
    }
    out
}

fn failed_of(checked: &Value) -> Vec<String> {
    match member(checked, Key::Failed.as_str()) {
        Some(Value::Arr(a)) => a
            .iter()
            .filter_map(|x| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn hop_read(bytes: &[u8], checked: &Value, input: InputFrom) -> HopRead {
    let e = zikaron::entry::check(bytes).ok();
    let body = |k: &str| e.as_ref().and_then(|e| text(&e.body, k)).unwrap_or_default();
    HopRead {
        id: e.as_ref().map(|e| e.id_hex()).unwrap_or_default(),
        author: e.as_ref().map(|e| e.author.clone()).unwrap_or_default(),
        grantee: body("grantee"),
        work: body("work"),
        upstream: e.as_ref().and_then(|e| text(&e.body, "upstream")),
        window: e.as_ref().and_then(|e| crate::grantx::window_of(&e.body)),
        verdict: text(checked, Key::Verdict.as_str()).unwrap_or_default(),
        lights: lights(checked),
        failed: failed_of(checked),
        input,
        terms: body("terms"),
        anchored_at: None,
    }
}

/// The output of one judgment.
#[derive(Clone, Debug)]
pub struct Judged {
    pub hops: Vec<HopRead>,
    /// Only with multiple hops.
    pub chain: Option<ChainRead>,
    /// Overall verdict: one hop gives that hop's verdict, several give the chain check's. GREEN / PARTIAL /
    /// FAIL, the kit crate's three states unchanged.
    pub verdict: String,
    /// The kit crate's result object, unchanged: kit law §10.3's object for one hop, §10.5's for several.
    /// This is what the CLI prints.
    pub value: Value,
}

/// Judge. Each hop has an optional audit input (the same shape as the CLI's `--hop <file>=<input file>`); one
/// hop goes to `grant_check`, several to `chain_check`. This layer judges nothing itself.
pub fn judge(hops: &[Vec<u8>], inputs: &[Option<Value>], froms: &[InputFrom], now: Option<u64>) -> Judged {
    crate::trace::mark(crate::feature::Feature::P1);
    let from_of = |i: usize| froms.get(i).cloned().unwrap_or(InputFrom::NoLedger);
    if hops.len() == 1 {
        let checked = zikaron_kit::check::grant_check(&hops[0], inputs.first().and_then(|x| x.as_ref()), now);
        let read = hop_read(&hops[0], &checked.value, from_of(0));
        return Judged { verdict: read.verdict.clone(), hops: vec![read], chain: None, value: checked.value };
    }
    let k2: Vec<zikaron_kit::check::Hop> = hops
        .iter()
        .enumerate()
        .map(|(i, g)| zikaron_kit::check::Hop { grant: g, input: inputs.get(i).cloned().flatten() })
        .collect();
    let value = zikaron_kit::check::chain_check(&k2, now);
    let per: Vec<Value> = match member(&value, Key::Hops.as_str()) {
        Some(Value::Arr(a)) => a.clone(),
        _ => Vec::new(),
    };
    let reads: Vec<HopRead> = hops
        .iter()
        .enumerate()
        .map(|(i, g)| hop_read(g, per.get(i).unwrap_or(&Value::Null), from_of(i)))
        .collect();
    let links: Vec<Option<bool>> = match member(&value, Key::Links.as_str()) {
        Some(Value::Arr(a)) => a
            .iter()
            .map(|x| match x {
                Value::Bool(b) => Some(*b),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let failing = member(&value, Key::Failing.as_str());
    let chain = ChainRead {
        verdict: text(&value, Key::Verdict.as_str()).unwrap_or_default(),
        token: text(&value, Key::Token.as_str()).unwrap_or_default(),
        failing_kind: failing.and_then(|f| text(f, Key::Kind.as_str())).unwrap_or_default(),
        failing_index: failing.and_then(|f| member(f, Key::Index.as_str())).and_then(|x| match x {
            Value::Int(n) => Some(*n),
            _ => None,
        }),
        links,
    };
    Judged { verdict: chain.verdict.clone(), hops: reads, chain: Some(chain), value }
}

/// Verdict to color. Closed, and PARTIAL is a state of its own: green only for GREEN, red only for FAIL,
/// yellow for PARTIAL, gray for anything else (empty, unrecognized). The face's lights are translated only
/// here, so "partial pass shown as green" cannot be written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Green,
    Amber,
    Red,
    Grey,
}

impl Tone {
    pub fn as_str(self) -> &'static str {
        match self {
            Tone::Green => "green",
            Tone::Amber => "amber",
            Tone::Red => "red",
            Tone::Grey => "grey",
        }
    }
}

/// Color of the overall verdict and of one hop's verdict.
pub fn tone(verdict: &str) -> Tone {
    use zikaron_kit::tokens::CheckVerdict;
    if verdict == CheckVerdict::Green.as_str() {
        Tone::Green
    } else if verdict == CheckVerdict::Fail.as_str() {
        Tone::Red
    } else if verdict == CheckVerdict::Partial.as_str() {
        Tone::Amber
    } else {
        Tone::Grey
    }
}

/// Color of a check's three states: PASS green, FAIL red, UNKNOWN gray (no yellow: a single check has no
/// "partial").
pub fn light_tone(state: &str) -> Tone {
    use zikaron_kit::tokens::State;
    if state == State::Pass.as_str() {
        Tone::Green
    } else if state == State::Fail.as_str() {
        Tone::Red
    } else {
        Tone::Grey
    }
}

/// Where `now` comes from. Closed (only chain time and an injected now).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NowFrom {
    Injected,
    Chain,
    Absent,
}

impl NowFrom {
    pub fn as_str(self) -> &'static str {
        match self {
            NowFrom::Injected => "injected",
            NowFrom::Chain => "chain",
            NowFrom::Absent => "absent",
        }
    }
}

/// The basis column: which basis the verdict was made against. Built by the face and shown with the verdict.
#[derive(Clone, Debug)]
pub struct Basis {
    pub chain: u64,
    pub registry: String,
    pub from_block: u64,
    pub to_block: u64,
    pub senders: Vec<String>,
    /// How many endpoints were asked.
    pub asked: usize,
    /// Single-source answer (given by the endpoint rule).
    pub single_source: bool,
    pub unanswered: Vec<String>,
    /// How many anchor records the fragment scan found.
    pub anchors: usize,
}

/// One check's reading. Every cell is what the background pass brought back.
#[derive(Clone, Debug)]
pub struct Checked {
    pub form: Form,
    pub judged: Judged,
    /// Basis: the one scanned when the scan succeeded; otherwise the evidence words of the named refusal (not
    /// configured, unreachable, rejected by the law check; the bytes are unchanged).
    pub basis: Result<Basis, String>,
    /// The refusal itself when the scan failed (a gray light's gap is split by its code: no node configured
    /// and chain unread are different things).
    pub basis_why: Option<Fault>,
    /// Files in the issuer's ledger location that could not be read as entries, each named.
    pub refused: Vec<crate::verifyx::Rejected>,
    /// Per hop: which level the ledger came from and where, and which levels failed on the way (`supplyx`, in
    /// fixed order).
    pub found: Vec<Source>,
    pub now: Option<u64>,
    pub now_from: NowFrom,
    /// Whether the supplied file is the granted work.
    pub file: Side,
    /// Whether the supplied terms file is the one the grant records.
    pub terms: Side,
}

/// One supplied file held against the grant. Closed set.
#[derive(Clone, Debug)]
pub enum Side {
    /// Nothing was supplied; the grant verdict stands on its own.
    NotGiven,
    /// Read and compared by the delivery check (`deliveryx::check`).
    Compared(crate::deliveryx::Checked),
    /// The file could not be read or the grant records no digest to compare with.
    Refused(Fault),
}

impl Side {
    pub fn as_str(&self) -> &'static str {
        match self {
            Side::NotGiven => "not-given",
            Side::Compared(c) if c.matched() => "match",
            Side::Compared(_) => "mismatch",
            Side::Refused(_) => "refused",
        }
    }
}

/// Hold an optional file against a recorded digest. Empty path is "not given".
pub fn side(path: &str, want: &str) -> Side {
    if path.trim().is_empty() {
        return Side::NotGiven;
    }
    match crate::deliveryx::check(std::path::Path::new(path.trim()), want) {
        Ok(c) => Side::Compared(c),
        Err(f) => Side::Refused(f),
    }
}

/// The one-line outcome of a check with its file and terms. Closed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Summary {
    /// The grant passed all six checks and every supplied file matched.
    AllMatch,
    /// This many items do not match (a failed grant counts as one).
    Mismatched(usize),
    /// Nothing mismatched, but the grant verdict is open or a supplied file could not be read.
    Incomplete,
}

/// Sum up a check: grant verdict, file and terms together.
pub fn summary(x: &Checked) -> Summary {
    let sides = [&x.file, &x.terms];
    let bad = sides.iter().filter(|s| matches!(s, Side::Compared(c) if !c.matched())).count() + usize::from(tone(&x.judged.verdict) == Tone::Red);
    if bad > 0 {
        return Summary::Mismatched(bad);
    }
    if tone(&x.judged.verdict) == Tone::Green && !sides.iter().any(|s| matches!(s, Side::Refused(_))) {
        Summary::AllMatch
    } else {
        Summary::Incomplete
    }
}

/// Compare the supplied work file and terms file with the presented grant (the last hop).
pub fn with_sides(mut x: Checked, file: &str, terms: &str) -> Checked {
    let (work, recorded) = x.judged.hops.last().map(|h| (h.work.clone(), h.terms.clone())).unwrap_or_default();
    crate::task::stage_at(crate::task::Kind::Check, 3);
    x.file = side(file, &work);
    crate::task::stage_at(crate::task::Kind::Check, 4);
    x.terms = side(terms, &recorded);
    x
}

impl Checked {
    /// The equivalent CLI verb: `check-grant` for one hop, `chain-check` for several.
    pub fn verb(&self) -> &'static str {
        if self.judged.hops.len() > 1 {
            "chain-check"
        } else {
            "check-grant"
        }
    }
}

/// This hop's ledger source is settled: supply came from this machine and all six lights are green. Then the
/// "change…" cell is not offered (there is no gap to fill, and opening it would only show a permanently empty
/// input).
pub fn source_settled(c: &Checked, hop: usize) -> bool {
    let local = matches!(c.found.get(hop).and_then(|f| f.from.as_ref()), Some((crate::supplyx::Level::Local, _)));
    let green = c.judged.hops.get(hop).map(|h| !h.lights.is_empty() && h.lights.iter().all(|(_, s)| light_tone(s) == Tone::Green)).unwrap_or(false);
    local && green
}

/// One hop's ledger source.
#[derive(Clone, Debug, Default)]
pub struct Source {
    /// Which level supply came from and where; `None` when none of the four levels has it (the face says
    /// "ledger source: none").
    pub from: Option<(crate::supplyx::Level, String)>,
    /// The publish address level: how many files were fetched by the manifest.
    pub files: Option<usize>,
    /// Which levels failed on the way (level, named refusal).
    pub misses: Vec<(crate::supplyx::Level, Fault)>,
    /// The ids of the entries that level supplied (read port: what came back, entry by entry; empty with no
    /// supply).
    pub entry_ids: Vec<[u8; 32]>,
}

/// What a gray light is missing. Closed: the face's action sentence comes from it (what is missing and where
/// to get it), keeping gray apart from red.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gap {
    /// None of the four levels has the issuer's ledger.
    NoLedger,
    /// A ledger exists but the audit input cannot be built (no genesis, a fork), with the refusal's words.
    LedgerRefused(String),
    /// No node or registry contract configured (go to settings).
    NoNode,
    /// Configured, but this pass could not read the chain, with the refusal's words.
    ChainUnread(String),
    /// A ledger exists and the chain was read, but this one is not yet anchored: verify again in a few
    /// minutes.
    NotYetAnchored,
    /// The chain's current time could not be read (for the window check).
    NoTime,
}

/// A light's gap. Answers only for the UNKNOWN state, from four things in the result object (where supply
/// came from, whether the basis was built, unreadable files, where the time came from), without guessing.
/// BAD_SIG always has an answer and is not here.
pub fn gap(x: &Checked, hop: usize, token: &str, state: &str) -> Option<Gap> {
    use zikaron_kit::tokens::{Check as C, State};
    if state != State::Unknown.as_str() {
        return None;
    }
    let input = x.judged.hops.get(hop).map(|h| h.input.clone()).unwrap_or(InputFrom::NoLedger);
    let ledger_gap = || match &input {
        InputFrom::NoLedger => Some(Gap::NoLedger),
        InputFrom::Refused(why) => Some(Gap::LedgerRefused(why.clone())),
        InputFrom::Ledger { .. } => None,
    };
    let chain_gap = || match (&x.basis, &x.basis_why) {
        (Ok(_), _) => None,
        (Err(_), Some(f)) if matches!(f.which(), Some(Known::NoEndpoint | Known::NoRegistry | Known::NoChainId)) => Some(Gap::NoNode),
        (Err(said), _) => Some(Gap::ChainUnread(said.clone())),
    };
    if token == C::Unanchored.as_str() {
        return chain_gap().or_else(ledger_gap).or(Some(Gap::NotYetAnchored));
    }
    if token == C::Expired.as_str() {
        return match (x.now_from, chain_gap()) {
            (NowFrom::Absent, Some(Gap::NoNode)) => Some(Gap::NoNode),
            _ => Some(Gap::NoTime),
        };
    }
    if token == C::BrokenLedger.as_str() || token == C::NotInLedger.as_str() || token == C::Revoked.as_str() {
        return ledger_gap();
    }
    None
}

/// The injected now: a decimal integer; empty means none injected.
pub fn now_of(typed: &str) -> Result<Option<u64>, Fault> {
    let t = typed.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<u64>()
        .map(Some)
        .map_err(|_| Fault::known(Known::SettingsShape, crate::lang::filln(crate::lang::Key::Tail102, &[&format!("{:?}", t)])))
}

fn anchors_in(fragment: &Value) -> usize {
    match member(fragment, "anchors") {
        Some(Value::Arr(a)) => a.len(),
        _ => 0,
    }
}

/// The whole check. This runs on a background thread: fetch supply, scan the chain, judge.
///
/// With `ground` not configured the basis column says so by name and the kit crate reads the verdict as
/// undecided; an empty `eps` likewise. Each hop's ledger is resolved level by level (`supplyx::find`: this
/// machine, vault, record bundle, publish address, in fixed order) and assembled with `auditx::input_of`,
/// refused by name when it cannot be; the fragment is the one this pass scanned (shared by all hops), empty
/// when the scan fails.
pub fn run(
    hops: Hops,
    mut shelf: crate::supplyx::Shelf,
    eps: Vec<crate::chainx::Endpoint>,
    ground: Result<crate::auditx::Ground, Fault>,
    injected: Option<u64>,
    chain: Option<u64>,
) -> Checked {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, the
    // CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::P1);
    // now: an injected one first; otherwise ask the chain; with neither, absent. Chain time needs only a
    // chain id: ask even when the basis cannot be built (no endpoints). The caller gives the chain id (it
    // already worked it out from endpoints and settings, in `action::check_payload`); when the basis is
    // built, its id wins; only when neither has one is there none.
    let chain_id: Option<u64> = ground.as_ref().ok().map(|g| g.chain).or(chain);
    let (now, now_from) = match injected {
        Some(n) => (Some(n), NowFrom::Injected),
        None => match chain_id.and_then(|c| crate::chainx::head_time(&eps, c).ok()) {
            Some((t, _, _)) => (Some(t), NowFrom::Chain),
            None => (None, NowFrom::Absent),
        },
    };
    // The input is a grant file: its ledger is the "record bundle" level, its pointer the "publish address"
    // level.
    if let Some((p, o)) = hops.file.as_ref() {
        if o.carries_ledger() {
            shelf.carried = Some((p.display().to_string(), o.ledger.clone()));
        }
        if shelf.pointer.is_none() {
            shelf.pointer = o.publish.clone();
        }
    }
    // Ledger bytes, resolved per hop, level by level.
    let mut piles: Vec<Option<Vec<Vec<u8>>>> = Vec::new();
    let mut refused: Vec<crate::verifyx::Rejected> = Vec::new();
    let mut froms: Vec<InputFrom> = Vec::new();
    let mut found: Vec<Source> = Vec::new();
    for (i, h) in hops.hops.iter().enumerate() {
        let f = crate::supplyx::find(&shelf, crate::supplyx::Want::Hop(h), i);
        match f.supply {
            Some(s) => {
                refused.extend(s.rejected.clone());
                piles.push(Some(s.items.clone()));
                let entry_ids = s.items.iter().map(|b| zikaron::entry::entry_id(b)).collect();
                found.push(Source { from: Some((s.level, s.place.clone())), files: s.files, misses: f.misses, entry_ids });
            }
            None => {
                piles.push(None);
                found.push(Source { from: None, files: None, misses: f.misses, entry_ids: Vec::new() });
            }
        }
        froms.push(InputFrom::NoLedger);
    }
    // Basis: the senders are the union of every hop's ledger lineage (sorted and deduplicated, one way to
    // assemble a scan). The refusal's evidence keeps its two forms: not-configured and law-check refusals use
    // `said`, chain queries and scans use `evidence` (bytes frozen).
    let basis: Result<(Basis, Value), (Fault, bool)> = (|| {
        let mut g = ground.clone().map_err(|f| (f, false))?;
        if eps.is_empty() {
            return Err((Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail103)), false));
        }
        let (head, _) = crate::chainx::head_block(&eps, g.chain).map_err(|f| (f, true))?;
        g.to_block = head.max(g.from_block);
        let mut senders: Vec<String> = Vec::new();
        for p in piles.iter().flatten() {
            senders.extend(crate::auditx::senders_of(p));
        }
        for h in &hops.hops {
            if let Ok(e) = zikaron::entry::check(h) {
                senders.push(e.author.clone());
            }
        }
        senders.sort();
        senders.dedup();
        g.senders = senders;
        crate::task::stage_at(crate::task::Kind::Check, 1);
        match crate::auditx::scan_agreed(&eps, &g).map_err(|f| (f, true))? {
            crate::auditx::Scan::Basis(a) => Ok((
                Basis {
                    chain: g.chain,
                    registry: g.registry.hex(),
                    from_block: g.from_block,
                    to_block: g.to_block,
                    senders: g.senders.clone(),
                    asked: a.asked,
                    single_source: a.single_source,
                    unanswered: a.unanswered,
                    anchors: anchors_in(&a.fragment),
                },
                a.fragment,
            )),
            crate::auditx::Scan::NoLabel { .. } => Err((Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail083)), false)),
        }
    })();
    let (basis, basis_why, fragment) = match basis {
        Ok((b, f)) => (Ok(b), None, f),
        Err((f, long)) => (Err(if long { f.evidence() } else { f.said().to_string() }), Some(f), crate::auditx::empty_fragment()),
    };
    // Input, per hop.
    let mut inputs: Vec<Option<Value>> = Vec::new();
    for (i, pile) in piles.iter().enumerate() {
        match pile {
            Some(items) => match crate::auditx::input_of(items, &fragment) {
                Ok(v) => {
                    froms[i] = InputFrom::Ledger { entries: items.len() };
                    inputs.push(Some(v));
                }
                Err(f) => {
                    froms[i] = InputFrom::Refused(f.evidence());
                    inputs.push(None);
                }
            },
            None => inputs.push(None),
        }
    }
    crate::task::stage_at(crate::task::Kind::Check, 2);
    let mut judged = judge(&hops.hops, &inputs, &froms, now);
    for (hop, pile) in judged.hops.iter_mut().zip(piles.iter()) {
        let Some(items) = pile else { continue };
        hop.anchored_at = crate::auditx::first_anchored(items, &fragment)
            .and_then(|at| at.into_iter().find(|(id, _)| id.eq_ignore_ascii_case(&hop.id)).map(|(_, t)| t));
    }
    Checked { form: hops.form, judged, basis, basis_why, refused, found, now, now_from, file: Side::NotGiven, terms: Side::NotGiven }
}
