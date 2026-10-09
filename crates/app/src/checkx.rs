//! Grant check page. Needs no identity and mirrors the CLI: takes a payload or a grant file and shows the
//! six checks as three-state lights plus an overall verdict. A multi-hop chain gets one row of lights per hop
//! and one verdict for the chain. PARTIAL is its own state, and the basis is shown with the verdict.
//!
//! Judging lives only in the kit crate (kit law §10): `zikaron_kit::check::grant_check` for the six checks
//! and `zikaron_kit::check::chain_check` for chains, the same code as the CLI's `check-grant` /
//! `chain-check` and the vault re-check. The same bytes, audit input and `now` give byte-identical result
//! objects on the page and in the CLI. This module only lays out the kit's result object and never folds
//! PARTIAL into green or red.
//!
//! No permissions: it reads no keychain, writes no archive and sends no transaction. Input is read-only
//! (payload bytes, grant files, ledger directories), the chain is read-only (anchor scans agreed across
//! endpoints, single sources flagged), and verdicts live only in memory. It works without a home: endpoints
//! and basis are entered on the page and not saved to settings.
//!
//! Absence is not guilt: without a ledger directory only checks one and five can run; the others answer
//! UNKNOWN and the kit judges PARTIAL. A ledger with no genesis cannot build the audit input
//! (`auditx::root_of` answers NO_GENESIS), so that hop says "no input" and stays undecided. An unreachable
//! chain is reported in the basis column and does not turn the verdict red.
//!
//! QR input: the badge packer encodes the payload text as a QR code; scanning it yields the same
//! `zikaron-grant:` text, which can be pasted here and is decoded by the same `badge::decode`.

use crate::fault::{Fault, Known};
use std::path::PathBuf;
use zikaron::json::Value;
use zikaron_kit::tokens::Key;

/// Input form; the same closed set as the vault (`payloadx::Form`).
pub use crate::payloadx::Form;

/// The parsed hops, from the root (the payload's segment order is the chain order, as the kit reads it).
#[derive(Clone, Debug)]
pub struct Hops {
    pub form: Form,
    pub hops: Vec<Vec<u8>>,
    /// For a grant file: its path and the opened bundle (its ledger supplies the "record bundle" level, its
    /// pointer the "publish address" level).
    pub file: Option<(PathBuf, crate::grantfilex::Opened)>,
}

/// Parses the input via `payloadx` (shared with the vault): payload text and files are decoded by the kit
/// crate, grant files are opened and verified, entry files must pass the core entry check and be a grant.
/// Empty input is refused by name.
pub fn hops_of(typed: &str) -> Result<Hops, Fault> {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
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
    /// A ledger was given but the input could not be built; carries the refusal (NO_GENESIS and others,
    /// from `auditx::root_of`).
    Refused(String),
}

/// One hop's reading, taken from the kit crate's result object; nothing is re-judged here.
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
    /// Block time of this grant's first anchor in the issuer's ledger, from this check's fragment.
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

/// The result of one judgment.
#[derive(Clone, Debug)]
pub struct Judged {
    pub hops: Vec<HopRead>,
    /// Only with multiple hops.
    pub chain: Option<ChainRead>,
    /// Overall verdict: the hop's verdict for one hop, the chain check's for several (GREEN / PARTIAL /
    /// FAIL, unchanged from the kit).
    pub verdict: String,
    /// The kit crate's result object, unchanged (kit law §10.3 for one hop, §10.5 for several); this is what
    /// the CLI prints.
    pub value: Value,
}

/// Judges the hops. Each hop has an optional audit input (like the CLI's `--hop <file>=<input file>`); one
/// hop goes to `grant_check`, several to `chain_check`. Nothing is judged here.
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

/// Verdict color. PARTIAL keeps its own color: green only for GREEN, red only for FAIL, amber for PARTIAL,
/// grey for anything else (empty, unrecognized). The page's lights are mapped only here, so a partial pass
/// can never show as green.
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

/// Color of a check's three states: PASS green, FAIL red, UNKNOWN grey (no amber: a single check has no
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

/// Where `now` came from: injected, chain time, or absent.
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

/// The basis the verdict was judged against, shown with the verdict.
#[derive(Clone, Debug)]
pub struct Basis {
    pub chain: u64,
    pub registry: String,
    pub from_block: u64,
    pub to_block: u64,
    pub senders: Vec<String>,
    /// How many endpoints were asked.
    pub asked: usize,
    /// The answer came from a single source (flagged by the endpoint rule).
    pub single_source: bool,
    pub unanswered: Vec<String>,
    /// How many anchor records the fragment scan found.
    pub anchors: usize,
}

/// One check's full result, as brought back by the background pass.
#[derive(Clone, Debug)]
pub struct Checked {
    pub form: Form,
    pub judged: Judged,
    /// The scanned basis on success; otherwise the refusal's evidence text (not configured, unreachable,
    /// rejected by the basis check).
    pub basis: Result<Basis, String>,
    /// The refusal when the scan failed; its code separates the grey-light gaps (no node configured versus
    /// chain not read).
    pub basis_why: Option<Fault>,
    /// Files in the issuer's ledger location that could not be read as entries, each named.
    pub refused: Vec<crate::verifyx::Rejected>,
    /// Per hop: which supply level the ledger came from and where, and which levels failed before it
    /// (`supplyx`, fixed order).
    pub found: Vec<Source>,
    pub now: Option<u64>,
    pub now_from: NowFrom,
    /// Whether the supplied file is the granted work.
    pub file: Side,
    /// Whether the supplied terms file is the one the grant records.
    pub terms: Side,
    /// Networks this pass could not read (`widex`), each named; empty with no read-only network.
    pub missed: Vec<crate::widex::Missed>,
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

/// True when this hop's ledger came from this machine and all six lights are green; the "change…" source
/// cell is then not offered, since there is no gap to fill.
pub fn source_settled(c: &Checked, hop: usize) -> bool {
    let local = matches!(c.found.get(hop).and_then(|f| f.from.as_ref()), Some((crate::supplyx::Level::Local, _)));
    let green = c.judged.hops.get(hop).map(|h| !h.lights.is_empty() && h.lights.iter().all(|(_, s)| light_tone(s) == Tone::Green)).unwrap_or(false);
    local && green
}

/// One hop's ledger source.
#[derive(Clone, Debug, Default)]
pub struct Source {
    /// Which level supplied the ledger and where; `None` when none of the four levels has it (shown as
    /// "ledger source: none").
    pub from: Option<(crate::supplyx::Level, String)>,
    /// The publish address level: how many files were fetched by the manifest.
    pub files: Option<usize>,
    /// Which levels failed on the way (level, named refusal).
    pub misses: Vec<(crate::supplyx::Level, Fault)>,
    /// Ids of the entries that level supplied; empty with no supply.
    pub entry_ids: Vec<[u8; 32]>,
}

/// What a grey light is missing. The page's action sentence (what is missing and where to get it) comes
/// from it, keeping grey distinct from red.
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
    /// A ledger exists and every network was read, but the grant is not anchored yet: verify again in a few
    /// minutes. If a network was skipped this pass it is `ChainUnread` instead, naming the skipped networks,
    /// since the anchor may be there.
    NotYetAnchored,
    /// The chain's current time could not be read (needed for the validity-window check).
    NoTime,
}

/// The gap behind an UNKNOWN light, derived without guessing from the result (supply source, whether the
/// basis was built, unreadable files, time source). `None` for any other state. BAD_SIG always has an
/// answer and is not covered.
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
    let missed_gap = || (!x.missed.is_empty()).then(|| Gap::ChainUnread(crate::widex::named(&x.missed)));
    if token == C::Unanchored.as_str() {
        return chain_gap().or_else(ledger_gap).or_else(missed_gap).or(Some(Gap::NotYetAnchored));
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

/// The whole check, run on a background thread: fetch supply, scan the chain, judge.
///
/// Without `ground` configured, or with empty `eps`, the basis column names the refusal and the kit reads
/// the verdict as undecided. Each hop's ledger is resolved level by level (`supplyx::find`: this machine,
/// vault, record bundle, publish address) and assembled by `auditx::input_of`, refused by name when it
/// cannot be. All hops share the fragment scanned in this pass; it is empty when the scan fails.
pub fn run(
    hops: Hops,
    mut shelf: crate::supplyx::Shelf,
    eps: Vec<crate::chainx::Endpoint>,
    ground: Result<crate::auditx::Ground, Fault>,
    injected: Option<u64>,
    chain: Option<u64>,
) -> Checked {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
    crate::trace::mark(crate::feature::Feature::P1);
    // now: injected first, else chain time, else absent. Chain time needs only a chain id, so it is asked
    // even when the basis cannot be built. The basis's chain id wins over the one the caller derived from
    // endpoints and settings (`action::check_payload`).
    let chain_id: Option<u64> = ground.as_ref().ok().map(|g| g.chain).or(chain);
    let (now, now_from) = match injected {
        Some(n) => (Some(n), NowFrom::Injected),
        None => match chain_id.and_then(|c| crate::chainx::head_time(&eps, c).ok()) {
            Some((t, _, _)) => (Some(t), NowFrom::Chain),
            None => (None, NowFrom::Absent),
        },
    };
    // A grant file's ledger is the "record bundle" level and its pointer the "publish address" level.
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
    // Basis senders: the union of every hop's ledger lineage and each hop's author, sorted and deduplicated.
    // Refusal evidence keeps two forms: configuration and validation refusals use `said`, chain queries and
    // scans use `evidence`.
    let basis: Result<(Basis, Value, Vec<crate::widex::Missed>), (Fault, bool)> = (|| {
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
        if !shelf.reads.is_empty() {
            // Read across networks the same way as every path that reads someone else's material
            // (`widex::scan`): the main network, if configured, is one window that fails by name like any
            // other chain, and every read-only network is read separately, each to agreement. An unconfigured
            // main network (no registry, no chain id anywhere) is no window at all, as on the verify page; a
            // mistyped setting is refused by name, never treated as unconfigured.
            let main = match &ground {
                Ok(g) => Some(g),
                Err(f) if f.which() == Some(Known::NoRegistry) || (f.which() == Some(Known::NoChainId) && f.tail().is_empty()) => None,
                Err(f) => return Err((f.clone(), false)),
            };
            crate::task::stage_at(crate::task::Kind::Check, 1);
            let w = crate::widex::scan(main.map(|g| (&eps[..], g)), &shelf.reads, &senders, crate::widex::Ask::Agreed).map_err(|f| (f, true))?;
            let g = crate::widex::carrier(main, &shelf.reads);
            // The main window's end as read (its start when it was not read).
            let to_block = match w.fragment.member("basis").and_then(|b| b.member("chains")) {
                Some(Value::Arr(c)) => c
                    .iter()
                    .filter(|x| x.member("chainId") == Some(&Value::Int(g.chain)))
                    .filter_map(|x| match x.member("toBlock") {
                        Some(Value::Int(n)) => Some(*n),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(g.from_block),
                _ => g.from_block,
            };
            return Ok((
                Basis {
                    chain: g.chain,
                    registry: g.registry.hex(),
                    from_block: g.from_block,
                    to_block,
                    senders,
                    asked: w.asked,
                    single_source: w.single_source,
                    unanswered: w.unanswered,
                    anchors: anchors_in(&w.fragment),
                },
                w.fragment,
                w.missed,
            ));
        }
        let mut g = ground.clone().map_err(|f| (f, false))?;
        if eps.is_empty() {
            return Err((Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail103)), false));
        }
        let (head, _) = crate::chainx::head_block(&eps, g.chain).map_err(|f| (f, true))?;
        g.to_block = head.max(g.from_block);
        g.senders = senders;
        crate::task::stage_at(crate::task::Kind::Check, 1);
        // No permissions: this page touches no local file, so it scans without the checked-facts cache.
        match crate::auditx::scan_agreed_with(&eps, &g, crate::auditx::Facts::Bare).map_err(|f| (f, true))? {
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
                Vec::new(),
            )),
            crate::auditx::Scan::NoLabel { .. } => Err((Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail083)), false)),
        }
    })();
    let (basis, basis_why, fragment, missed) = match basis {
        Ok((b, f, m)) => (Ok(b), None, f, m),
        Err((f, long)) => (Err(if long { f.evidence() } else { f.said().to_string() }), Some(f), crate::auditx::empty_fragment(), Vec::new()),
    };
    // A network skipped this pass joins the basis as a window with no registry, so the kit's coverage rule
    // sees the basis does not cover it and the grant is not called unanchored for want of an unread chain.
    // With nothing missed the fragment is unchanged.
    let senders = basis.as_ref().map(|b| b.senders.clone()).unwrap_or_default();
    let fragment = crate::widex::with_unread_windows(&fragment, &missed, &senders);
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
    Checked { form: hops.form, judged, basis, basis_why, refused, found, now, now_from, file: Side::NotGiven, terms: Side::NotGiven, missed }
}
