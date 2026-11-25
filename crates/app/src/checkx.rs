use crate::fault::{Fault, Known};
use std::path::PathBuf;
use zikaron::json::Value;
use zikaron_kit::tokens::Key;

pub use crate::payloadx::Form;

#[derive(Clone, Debug)]
pub struct Hops {
    pub form: Form,
    pub hops: Vec<Vec<u8>>,
    pub file: Option<(PathBuf, crate::grantfilex::Opened)>,
}

pub fn hops_of(typed: &str) -> Result<Hops, Fault> {
    crate::trace::mark(crate::feature::Feature::P1);
    let taken = crate::payloadx::take_full(typed)?;
    let (form, hops, file) = (taken.form, taken.hops, taken.file);
    if form == Form::EntryFile {
        let bytes = &hops[0];
        let e = zikaron_glue::book::check(bytes).map_err(Fault::entry_refused)?;
        if e.kind != zikaron_glue::book::Kind::Grant {
            return Err(Fault::known(Known::NotAGrant, e.kind.as_str().to_string()));
        }
    }
    Ok(Hops { form, hops, file })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputFrom {
    NoLedger,
    Ledger { entries: usize, lacks: bool },
    Refused(String),
    Undecided(zikaron_glue::book::Undecided),
}

#[derive(Clone, Debug)]
pub struct HopRead {
    pub id: String,
    pub author: String,
    pub grantee: String,
    pub work: String,
    pub upstream: Option<String>,
    pub window: Option<(u64, u64)>,
    pub verdict: String,
    pub lights: Vec<(String, String)>,
    pub failed: Vec<String>,
    pub input: InputFrom,
    pub terms: String,
    pub anchored_at: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct ChainRead {
    pub verdict: String,
    pub token: String,
    pub failing_kind: String,
    pub failing_index: Option<u64>,
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
    let e = zikaron_glue::book::check(bytes).ok();
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

#[derive(Clone, Debug)]
pub struct Judged {
    pub hops: Vec<HopRead>,
    pub chain: Option<ChainRead>,
    pub verdict: String,
    pub value: Value,
}

pub fn judge(hops: &[Vec<u8>], inputs: &[Option<Value>], froms: &[InputFrom], now: Option<u64>) -> Judged {
    judge_with(hops, inputs, &[], froms, now)
}

pub fn judge_with(hops: &[Vec<u8>], inputs: &[Option<Value>], notices: &[Option<Value>], froms: &[InputFrom], now: Option<u64>) -> Judged {
    crate::trace::mark(crate::feature::Feature::P1);
    let from_of = |i: usize| froms.get(i).cloned().unwrap_or(InputFrom::NoLedger);
    let notice_of = |i: usize| notices.get(i).cloned().flatten();
    if hops.len() == 1 {
        let value = zikaron_glue::book::grant_check(&hops[0], inputs.first().and_then(|x| x.as_ref()), notice_of(0).as_ref(), now);
        let read = hop_read(&hops[0], &value, from_of(0));
        return Judged { verdict: read.verdict.clone(), hops: vec![read], chain: None, value };
    }
    let k2: Vec<(Vec<u8>, Option<Value>, Option<Value>)> = hops.iter().enumerate().map(|(i, g)| (g.clone(), inputs.get(i).cloned().flatten(), notice_of(i))).collect();
    let value = zikaron_glue::book::chain_check(&k2, now);
    let per: Vec<Value> = match member(&value, Key::Hops.as_str()) {
        Some(Value::Arr(a)) => a.clone(),
        _ => Vec::new(),
    };
    let reads: Vec<HopRead> = hops
        .iter()
        .enumerate()
        .map(|(i, g)| hop_read(g, per.get(i).unwrap_or(&Value::Null), from_of(i)))
        .collect();
    let chain = chain_read(&value);
    Judged { verdict: chain.verdict.clone(), hops: reads, chain: Some(chain), value }
}

fn chain_read(value: &Value) -> ChainRead {
    let links: Vec<Option<bool>> = match member(value, Key::Links.as_str()) {
        Some(Value::Arr(a)) => a
            .iter()
            .map(|x| match x {
                Value::Bool(b) => Some(*b),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let failing = member(value, Key::Failing.as_str());
    ChainRead {
        verdict: text(value, Key::Verdict.as_str()).unwrap_or_default(),
        token: text(value, Key::Token.as_str()).unwrap_or_default(),
        failing_kind: failing.and_then(|f| text(f, Key::Kind.as_str())).unwrap_or_default(),
        failing_index: failing.and_then(|f| member(f, Key::Index.as_str())).and_then(|x| match x {
            Value::Int(n) => Some(*n),
            _ => None,
        }),
        links,
    }
}

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

pub use crate::chainx::NowFrom;

#[derive(Clone, Debug)]
pub struct Basis {
    pub chain: u64,
    pub registry: String,
    pub from_block: u64,
    pub to_block: u64,
    pub senders: Vec<String>,
    pub asked: usize,
    pub single_source: bool,
    pub unanswered: Vec<String>,
    pub anchors: usize,
}

#[derive(Clone, Debug)]
pub struct Checked {
    pub form: Form,
    pub judged: Judged,
    pub basis: Result<Basis, String>,
    pub basis_why: Option<Fault>,
    pub refused: Vec<crate::verifyx::Rejected>,
    pub found: Vec<Source>,
    pub now: Option<u64>,
    pub now_from: NowFrom,
    pub file: Side,
    pub terms: Side,
    pub missed: Vec<crate::widex::Missed>,
    pub takes: Option<(String, Option<Result<zikaron_glue::extract::Plan, zikaron_glue::extract::Refused>>)>,
    pub notes: crate::noticex::Notes,
    pub reading: Reading,
    pub undecided: Vec<(usize, zikaron_glue::book::Undecided)>,
    pub audit_key: crate::sendx::behind::Key,
    pub ledger: Option<(String, usize)>,
    pub record: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    Fast,
    Whole,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Audit {
    pub verdict: String,
    pub label: String,
    pub entries: usize,
    pub chain: u64,
    pub block: u64,
    pub at: Option<u64>,
    pub value: Value,
    pub anchored: Vec<Option<u64>>,
    pub ledgers: Vec<Option<(usize, bool)>>,
}

pub const AUDIT_FORM: &str = "zikaron.audit-result/1";

impl Audit {
    pub fn of(x: &Checked) -> Audit {
        let (label, entries) = x.ledger.clone().unwrap_or_default();
        let (chain, block) = x.basis.as_ref().map(|b| (b.chain, b.to_block)).unwrap_or((0, 0));
        let anchored = x.judged.hops.iter().map(|h| h.anchored_at).collect();
        let ledgers = x
            .judged
            .hops
            .iter()
            .map(|h| match h.input {
                InputFrom::Ledger { entries, lacks } => Some((entries, lacks)),
                _ => None,
            })
            .collect();
        Audit { verdict: x.judged.verdict.clone(), label, entries, chain, block, at: x.now, value: x.judged.value.clone(), anchored, ledgers }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut m = vec![
            ("block".to_string(), Value::Int(self.block)),
            ("chain".to_string(), Value::Int(self.chain)),
            ("entryCount".to_string(), Value::Int(self.entries as u64)),
            ("form".to_string(), Value::Str(AUDIT_FORM.to_string())),
            ("label".to_string(), Value::Str(self.label.clone())),
            ("verdict".to_string(), Value::Str(self.verdict.clone())),
        ];
        if let Some(t) = self.at {
            m.insert(0, ("at".to_string(), Value::Int(t)));
        }
        let opt = |n: Option<u64>| n.map(Value::Int).unwrap_or(Value::Null);
        m.push(("anchored".to_string(), Value::Arr(self.anchored.iter().map(|a| opt(*a)).collect())));
        m.push((
            "ledgers".to_string(),
            Value::Arr(self.ledgers.iter().map(|l| l.map(|(n, lacks)| Value::Arr(vec![Value::Int(n as u64), Value::Bool(lacks)])).unwrap_or(Value::Null)).collect()),
        ));
        m.push(("result".to_string(), self.value.clone()));
        zikaron::json::canon_bytes(&Value::Obj(m))
    }

    fn from_bytes(b: &[u8]) -> Option<Audit> {
        let v = zikaron::json::parse(b).ok()?;
        if v.member("form").and_then(Value::as_str) != Some(AUDIT_FORM) {
            return None;
        }
        let int = |k: &str| match v.member(k) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let text = |k: &str| v.member(k).and_then(Value::as_str).map(str::to_string);
        let anchored = v
            .member("anchored")?
            .as_arr()?
            .iter()
            .map(|a| match a {
                Value::Int(n) => Some(Some(*n)),
                Value::Null => Some(None),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let ledgers = v
            .member("ledgers")?
            .as_arr()?
            .iter()
            .map(|l| match l {
                Value::Null => Some(None),
                Value::Arr(p) => match p.as_slice() {
                    [Value::Int(n), Value::Bool(lacks)] => Some(Some((*n as usize, *lacks))),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let value = v.member("result")?.clone();
        Some(Audit { verdict: text("verdict")?, label: text("label")?, entries: int("entryCount")? as usize, chain: int("chain")?, block: int("block")?, at: int("at"), value, anchored, ledgers })
    }
}

pub fn as_shown(x: &Checked, audit: &crate::sendx::behind::State<Audit, Fault>) -> Checked {
    match audit {
        crate::sendx::behind::State::Done(a) => audited(x, a).unwrap_or_else(|| x.clone()),
        _ => x.clone(),
    }
}

pub fn audited(x: &Checked, a: &Audit) -> Option<Checked> {
    if x.undecided.is_empty() || a.anchored.len() != x.judged.hops.len() || a.ledgers.len() != x.judged.hops.len() {
        return None;
    }
    let mut y = x.clone();
    let several = y.judged.hops.len() > 1;
    let per: Vec<Value> = if several {
        match member(&a.value, Key::Hops.as_str()) {
            Some(Value::Arr(h)) if h.len() == y.judged.hops.len() => h.clone(),
            _ => return None,
        }
    } else {
        vec![a.value.clone()]
    };
    for (i, h) in y.judged.hops.iter_mut().enumerate() {
        h.verdict = text(&per[i], Key::Verdict.as_str()).unwrap_or_default();
        h.lights = lights(&per[i]);
        h.failed = failed_of(&per[i]);
        h.anchored_at = a.anchored[i];
        h.input = match a.ledgers[i] {
            Some((entries, lacks)) => InputFrom::Ledger { entries, lacks },
            None => InputFrom::NoLedger,
        };
    }
    y.judged.chain = several.then(|| chain_read(&a.value));
    y.judged.verdict = a.verdict.clone();
    y.judged.value = a.value.clone();
    y.undecided = Vec::new();
    y.ledger = Some((a.label.clone(), a.entries));
    y.reading = Reading::Whole;
    Some(y)
}

pub struct AuditStore;

impl crate::sendx::behind::Store<Audit> for AuditStore {
    fn load(&self, key: &crate::sendx::behind::Key) -> Option<Audit> {
        use crate::sendx::record::Room;
        crate::sendx::record::Desk.read(&crate::sendx::record::audit_file(key)).and_then(|b| Audit::from_bytes(&b))
    }
    fn save(&self, key: &crate::sendx::behind::Key, r: &Audit) {
        use crate::sendx::record::Room;
        let _ = crate::sendx::record::Desk.put(&crate::sendx::record::audit_file(key), &r.to_bytes());
    }
}

pub type Audits = crate::sendx::behind::Audits<Audit, Fault>;

pub fn audit_key(hops: &[Vec<u8>], roots: &[Option<String>], piles: &[Option<Vec<Vec<u8>>>], ground: Option<&crate::auditx::Ground>, nets: &[crate::readnets::Net]) -> crate::sendx::behind::Key {
    let root: Vec<u8> = roots.iter().map(|r| r.clone().unwrap_or_default()).collect::<Vec<_>>().join(",").into_bytes();
    let window = |chain: u64, registry: String, from: u64| Value::Obj(vec![("chainId".to_string(), Value::Int(chain)), ("fromBlock".to_string(), Value::Int(from)), ("registry".to_string(), Value::Str(registry))]);
    let mut chains: Vec<Value> = ground.map(|g| window(g.chain, g.registry.hex(), g.from_block)).into_iter().collect();
    chains.extend(nets.iter().map(|n| window(n.chain_id, n.registry.hex(), n.from_block)));
    let basis = zikaron::json::canon_bytes(&Value::Arr(chains));
    let hex = |b: &[u8]| Value::Str(zikaron::hexfmt::encode(b));
    let pile = zikaron::json::canon_bytes(&Value::Arr(
        hops.iter().zip(piles).map(|(g, p)| Value::Arr(std::iter::once(hex(g)).chain(p.iter().flatten().map(|e| hex(e))).collect())).collect(),
    ));
    crate::sendx::behind::Key::of(&root, &basis, &pile)
}

pub fn originals_of_file(p: &std::path::Path) -> Option<Result<zikaron_glue::extract::Plan, zikaron_glue::extract::Refused>> {
    let opened = if zikaron_glue::addressed::is_addressed_at(p) { Some(crate::addressedx::open(p).ok()?) } else { None };
    let out = zikaron_glue::read::unpack(opened.as_ref().map(|o| o.file.as_path()).unwrap_or(p)).ok()?;
    let d = out.dir.clone();
    let (b, _) = crate::verifyx::bytes_at(&crate::verifyx::Source::Kit(d.clone())).ok()?;
    let kit = crate::verifyx::verify_kit_at(&d);
    let held = zikaron_glue::extract::originals_in_dir(&d);
    kit.ok.then(|| zikaron_glue::extract::plan_in(&d, &b, &held))
}

#[derive(Clone, Debug)]
pub enum Side {
    NotGiven,
    Compared(crate::deliveryx::Checked),
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

pub fn side(path: &str, want: &str) -> Side {
    if path.trim().is_empty() {
        return Side::NotGiven;
    }
    match crate::deliveryx::check(std::path::Path::new(path.trim()), want) {
        Ok(c) => Side::Compared(c),
        Err(f) => Side::Refused(f),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Summary {
    AllMatch,
    Mismatched(usize),
    Incomplete,
    Undecided,
}

pub fn summary(x: &Checked) -> Summary {
    let sides = [&x.file, &x.terms];
    let bad = sides.iter().filter(|s| matches!(s, Side::Compared(c) if !c.matched())).count() + usize::from(tone(&x.judged.verdict) == Tone::Red);
    if bad > 0 {
        return Summary::Mismatched(bad);
    }
    if !x.undecided.is_empty() {
        return Summary::Undecided;
    }
    if tone(&x.judged.verdict) == Tone::Green && !sides.iter().any(|s| matches!(s, Side::Refused(_))) {
        Summary::AllMatch
    } else {
        Summary::Incomplete
    }
}

pub fn with_sides(mut x: Checked, file: &str, terms: &str) -> Checked {
    let (work, recorded) = x.judged.hops.last().map(|h| (h.work.clone(), h.terms.clone())).unwrap_or_default();
    crate::task::stage_at(crate::task::Kind::Check, 3);
    x.file = side(file, &work);
    crate::task::stage_at(crate::task::Kind::Check, 4);
    x.terms = side(terms, &recorded);
    x
}

impl Checked {
    pub fn verb(&self) -> &'static str {
        if self.judged.hops.len() > 1 {
            "chain-check"
        } else {
            "check-grant"
        }
    }
}

pub fn source_settled(c: &Checked, hop: usize) -> bool {
    let local = matches!(c.found.get(hop).and_then(|f| f.from.as_ref()), Some((crate::supplyx::Level::Local, _)));
    let green = c.judged.hops.get(hop).map(|h| !h.lights.is_empty() && h.lights.iter().all(|(_, s)| light_tone(s) == Tone::Green)).unwrap_or(false);
    local && green
}

#[derive(Clone, Debug, Default)]
pub struct Source {
    pub from: Option<(crate::supplyx::Level, String)>,
    pub files: Option<usize>,
    pub misses: Vec<(crate::supplyx::Level, Fault)>,
    pub entry_ids: Vec<[u8; 32]>,
}

pub use zikaron_glue::book::Gap;

pub fn gap(x: &Checked, hop: usize, token: &str, state: &str) -> Option<Gap> {
    if let Some(g) = notices_gap(&x.notes, hop, token, state) {
        return Some(g);
    }
    let input = x.judged.hops.get(hop).map(|h| h.input.clone()).unwrap_or(InputFrom::NoLedger);
    let chain = match (&x.basis, &x.basis_why) {
        (Ok(_), _) => None,
        (Err(_), Some(f)) if matches!(f.which(), Some(Known::NoEndpoint | Known::NoRegistry | Known::NoChainId)) => Some(Gap::NoNode),
        (Err(said), _) => Some(Gap::ChainUnread(said.clone())),
    };
    gap_of(token, state, &input, chain, &x.missed, x.now_from == NowFrom::Absent)
}

pub fn notices_gap(notes: &crate::noticex::Notes, hop: usize, token: &str, state: &str) -> Option<Gap> {
    notices_gap_of(notes.said.get(hop).and_then(|x| x.as_deref()), token, state)
}

pub fn notices_gap_of(said: Option<&str>, token: &str, state: &str) -> Option<Gap> {
    zikaron_glue::book::notices_gap_of(said, token, state)
}

pub fn gap_of(token: &str, state: &str, input: &InputFrom, chain: Option<Gap>, missed: &[crate::widex::Missed], no_time: bool) -> Option<Gap> {
    let ledger = match input {
        InputFrom::NoLedger => zikaron_glue::book::LedgerRead::None,
        InputFrom::Refused(why) => zikaron_glue::book::LedgerRead::Refused(why.clone()),
        InputFrom::Ledger { lacks, .. } => zikaron_glue::book::LedgerRead::Read { behind: *lacks },
        InputFrom::Undecided(u) => zikaron_glue::book::LedgerRead::Undecided(*u),
    };
    let missed = (!missed.is_empty()).then(|| crate::widex::named(missed));
    zikaron_glue::book::gap_of(token, state, &ledger, chain, missed, no_time)
}

pub fn now_of(typed: &str) -> Result<Option<u64>, Fault> {
    let t = typed.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<u64>()
        .map(Some)
        .map_err(|_| Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail102, &[&format!("{:?}", t)]))))
}

fn anchors_in(fragment: &Value) -> usize {
    let records = match member(fragment, "anchors") {
        Some(Value::Arr(a)) => a.len(),
        _ => 0,
    };
    records + zikaron_anchor::scan::candidates_in(fragment).len()
}

pub fn run(hops: Hops, shelf: crate::supplyx::Shelf, eps: Vec<crate::chainx::Endpoint>, ground: Result<crate::auditx::Ground, Fault>, injected: Option<u64>, chain: Option<u64>) -> Checked {
    run_as(hops, shelf, eps, ground, injected, chain, Reading::Fast)
}

pub fn run_whole(hops: Hops, shelf: crate::supplyx::Shelf, eps: Vec<crate::chainx::Endpoint>, ground: Result<crate::auditx::Ground, Fault>, injected: Option<u64>, chain: Option<u64>) -> Checked {
    run_as(hops, shelf, eps, ground, injected, chain, Reading::Whole)
}

pub fn run_as(
    hops: Hops,
    mut shelf: crate::supplyx::Shelf,
    eps: Vec<crate::chainx::Endpoint>,
    ground: Result<crate::auditx::Ground, Fault>,
    injected: Option<u64>,
    chain: Option<u64>,
    reading: Reading,
) -> Checked {
    crate::trace::mark(crate::feature::Feature::P1);
    let chain_id: Option<u64> = ground.as_ref().ok().map(|g| g.chain).or(chain);
    let (now, now_from) = crate::chainx::now_of(injected, &eps, chain_id);
    if let Some((p, o)) = hops.file.as_ref() {
        if o.carries_ledger() {
            shelf.carried = Some((p.display().to_string(), o.ledger.clone()));
        }
        if shelf.pointer.is_none() {
            shelf.pointer = o.publish.clone();
        }
    }
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
    let all_eps = crate::readnets::all_endpoints(&eps, &shelf.reads);
    let grants: Vec<&[u8]> = hops.hops.iter().map(|h| h.as_slice()).collect();
    let notes = crate::noticex::for_grants(&grants, &all_eps);
    let mut behind: Vec<bool> = vec![false; hops.hops.len()];
    let roots: Vec<Option<String>> = hops
        .hops
        .iter()
        .zip(piles.iter())
        .map(|(h, pile)| {
            pile.as_ref().and_then(|p| crate::auditx::root_of(p).ok()).or_else(|| {
                zikaron_glue::book::check(h).ok().map(|e| e.body.member("ledger").and_then(|v| v.as_str()).map(str::to_string).unwrap_or(e.author))
            })
        })
        .collect();
    let stated: Vec<Option<String>> = (0..hops.hops.len()).map(|i| hops.file.as_ref().and_then(|(_, o)| zikaron_glue::grantfile::hop_root(&o.hops, &o.ledger, o.root.as_deref(), i))).collect();
    let audit_key = audit_key(&hops.hops, &roots, &piles, ground.as_ref().ok(), &shelf.reads);
    let mut undecided: Vec<(usize, zikaron_glue::book::Undecided)> = Vec::new();
    let basis: Result<(Basis, Value, Vec<crate::widex::Missed>), (Fault, bool)> = (|| {
        let mut senders: Vec<String> = Vec::new();
        for p in piles.iter().flatten() {
            senders.extend(crate::auditx::senders_of(p));
        }
        for h in &hops.hops {
            if let Ok(e) = zikaron_glue::book::check(h) {
                senders.push(e.author.clone());
            }
        }
        let main_g = ground.as_ref().ok().filter(|_| !eps.is_empty()).map(|g| (&eps[..], g));
        let offer = crate::readerx::book_offer(main_g, &shelf.reads);
        for (i, (h, pile)) in hops.hops.iter().zip(piles.iter()).enumerate() {
            if let Some(r) = roots[i].clone() {
                let key = crate::readerx::book_key(&r, &offer);
                behind[i] = pile.as_ref().is_some_and(|p| !zikaron_glue::notices::copy_stands(h, p, key.as_deref()));
                senders = crate::readerx::with_key(senders, key.as_deref());
            }
            let n = notes.per.get(i).cloned().flatten();
            if let Some(r) = zikaron_glue::notices::root_for(h, n.as_ref()).filter(|_| zikaron_glue::notices::carries(h)) {
                senders.extend(zikaron_glue::notices::lineage(h, &r, n.as_ref()));
            }
        }
        senders.sort();
        senders.dedup();
        let mut decisions: Vec<zikaron_glue::book::Decision> = Vec::new();
        if reading == Reading::Fast {
            for (i, h) in hops.hops.iter().enumerate() {
                let carries = zikaron_glue::notices::carries(h);
                if behind[i] || (piles[i].is_none() && !carries) {
                    continue;
                }
                let root = stated[i].clone().or_else(|| roots[i].clone()).unwrap_or_default();
                let pile: &[Vec<u8>] = piles[i].as_deref().unwrap_or(&[]);
                match zikaron_glue::book::decision_set(&zikaron_glue::book::Hold { grant: h, root: &root, pile }) {
                    Ok(d) => decisions.push(d),
                    Err(u) => undecided.push((i, u)),
                }
            }
        }
        let read = |pick: &zikaron_anchor::scan::Pick| -> Result<(Basis, Value, Vec<crate::widex::Missed>), (Fault, bool)> {
            read_basis(&ground, &eps, &shelf.reads, &senders, pick, reading == Reading::Fast)
        };
        loop {
            let pick = match reading {
                Reading::Whole => zikaron_anchor::scan::Pick::All,
                Reading::Fast => zikaron_glue::book::pick_of(&decisions),
            };
            let got = read(&pick)?;
            if reading == Reading::Whole {
                return Ok(got);
            }
            let mut more = false;
            for d in decisions.iter_mut() {
                if let Ok(zikaron_glue::book::Fast::More) = zikaron_glue::book::fast_input(d, &got.1) {
                    more = true;
                }
            }
            if !more {
                return Ok(got);
            }
        }
    })();
    let (basis, basis_why, fragment, missed) = match basis {
        Ok((b, f, m)) => (Ok(b), None, f, m),
        Err((f, long)) => (Err(if long { f.evidence() } else { f.said().to_string() }), Some(f), crate::auditx::empty_fragment(), Vec::new()),
    };
    finish(hops, piles, froms, found, refused, notes, behind, stated, undecided, audit_key, reading, (basis, basis_why, fragment, missed), (now, now_from))
}

fn read_basis(
    ground: &Result<crate::auditx::Ground, Fault>,
    eps: &[crate::chainx::Endpoint],
    reads: &[crate::readnets::Net],
    senders: &[String],
    pick: &zikaron_anchor::scan::Pick,
    staged: bool,
) -> Result<(Basis, Value, Vec<crate::widex::Missed>), (Fault, bool)> {
    let senders: Vec<String> = senders.to_vec();
    let stage = |at: u8| {
        if staged {
            crate::task::stage_at(crate::task::Kind::Check, at);
        }
    };
    {
        if !reads.is_empty() {
            let main = match ground {
                Ok(g) => Some(g),
                Err(f) if f.which() == Some(Known::NoRegistry) || (f.which() == Some(Known::NoChainId) && f.tail().is_empty()) => None,
                Err(f) => return Err((f.clone(), false)),
            };
            stage(1);
            let w = crate::widex::scan_picked(main.map(|g| (eps, g)), reads, &senders, crate::widex::Ask::Agreed, pick).map_err(|f| (f, true))?;
            let g = crate::widex::carrier(main, reads);
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
            return Err((Fault::known(Known::NoEndpoint, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail103))), false));
        }
        let (head, _) = crate::chainx::head_block(eps, g.chain).map_err(|f| (f, true))?;
        g.to_block = head.max(g.from_block);
        g.senders = senders;
        stage(1);
        match crate::auditx::scan_agreed_picked(eps, &g, crate::auditx::Facts::Bare, pick).map_err(|f| (f, true))? {
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
            crate::auditx::Scan::NoLabel { .. } => Err((Fault::known(Known::AuditInput, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail083))), false)),
        }
    }
}

fn issuer_ledger(reading: Reading, hops: usize, piles: &[Option<Vec<Vec<u8>>>], inputs: &[Option<Value>], labels: &[Option<String>], fragment: &Value) -> Option<(String, usize)> {
    if reading != Reading::Whole {
        return None;
    }
    let last = hops.checked_sub(1)?;
    let items = piles.get(last)?.as_ref()?;
    inputs.get(last)?.as_ref()?;
    let label = match labels.get(last).cloned().flatten() {
        Some(l) => l,
        None => crate::auditx::ask_from(items, fragment, Vec::new(), 0, true).ok()?.label,
    };
    Some((label, items.len()))
}

#[allow(clippy::too_many_arguments)]
fn finish(
    hops: Hops,
    piles: Vec<Option<Vec<Vec<u8>>>>,
    mut froms: Vec<InputFrom>,
    found: Vec<Source>,
    refused: Vec<crate::verifyx::Rejected>,
    notes: crate::noticex::Notes,
    behind: Vec<bool>,
    stated: Vec<Option<String>>,
    undecided: Vec<(usize, zikaron_glue::book::Undecided)>,
    audit_key: crate::sendx::behind::Key,
    reading: Reading,
    (basis, basis_why, fragment, missed): (Result<Basis, String>, Option<Fault>, Value, Vec<crate::widex::Missed>),
    (now, now_from): (Option<u64>, NowFrom),
) -> Checked {
    let senders = basis.as_ref().map(|b| b.senders.clone()).unwrap_or_default();
    let fragment = crate::widex::with_unread_windows(&fragment, &missed, &senders);
    let mut inputs: Vec<Option<Value>> = Vec::new();
    let mut labels: Vec<Option<String>> = vec![None; piles.len()];
    for (i, pile) in piles.iter().enumerate() {
        if let Some((_, u)) = undecided.iter().find(|(h, _)| *h == i) {
            froms[i] = InputFrom::Undecided(*u);
            inputs.push(None);
            continue;
        }
        let g = &hops.hops[i];
        if zikaron_glue::notices::carries(g) && (pile.is_some() || basis.is_ok()) {
            match crate::auditx::input_for(g, pile.as_deref().unwrap_or(&[]), &fragment, stated[i].as_deref(), notes.per.get(i).cloned().flatten().as_ref()) {
                Ok(v) => {
                    if let Some(items) = pile {
                        froms[i] = InputFrom::Ledger { entries: items.len(), lacks: false };
                    }
                    inputs.push(Some(v));
                }
                Err(f) => {
                    froms[i] = InputFrom::Refused(f.evidence());
                    inputs.push(None);
                }
            }
            continue;
        }
        if behind.get(i).copied().unwrap_or(false) {
            froms[i] = InputFrom::Refused(crate::lang::t(crate::lang::Key::LedgerBehindBook).to_string());
            inputs.push(None);
            continue;
        }
        match pile {
            Some(items) => match crate::auditx::input_of(items, &fragment) {
                Ok(v) => {
                    let asked = crate::auditx::ask_from(items, &fragment, Vec::new(), 0, true).ok();
                    let lacks = asked.as_ref().is_some_and(|r| r.lacks_anchored());
                    labels[i] = asked.map(|r| r.label);
                    froms[i] = InputFrom::Ledger { entries: items.len(), lacks };
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
    let ledger = issuer_ledger(reading, hops.hops.len(), &piles, &inputs, &labels, &fragment);
    if reading == Reading::Fast {
        crate::task::stage_at(crate::task::Kind::Check, 2);
    }
    let mut judged = judge_with(&hops.hops, &inputs, &notes.per, &froms, now);
    for (hop, pile) in judged.hops.iter_mut().zip(piles.iter()) {
        let Some(items) = pile else { continue };
        hop.anchored_at = crate::auditx::first_anchored(items, &fragment)
            .and_then(|at| at.into_iter().find(|(id, _)| id.eq_ignore_ascii_case(&hop.id)).map(|(_, t)| t));
    }
    let takes = hops.file.as_ref().filter(|(_, o)| !o.contents.is_empty()).map(|(p, _)| (p.display().to_string(), originals_of_file(p)));
    let record = judged.hops.last().zip(piles.last()).and_then(|(h, p)| p.as_deref().and_then(|items| crate::vaultx::names_in(items, &h.work).0));
    Checked { form: hops.form, judged, basis, basis_why, refused, found, now, now_from, file: Side::NotGiven, terms: Side::NotGiven, missed, takes, notes, reading, undecided, audit_key, ledger, record }
}

#[cfg(test)]
mod issuer_ledger_tests {
    use super::*;

    #[test]
    fn the_issuer_ledger_is_read_once_after_either_branch() {
        let pile = Some(vec![b"a".to_vec(), b"b".to_vec()]);
        let input = Some(Value::Null);
        let label = Some("COMPLETE".to_string());
        let frag = Value::Null;
        assert_eq!(issuer_ledger(Reading::Whole, 1, &[pile.clone()], &[input.clone()], &[label.clone()], &frag), Some(("COMPLETE".into(), 2)));
        assert_eq!(issuer_ledger(Reading::Fast, 1, &[pile.clone()], &[input.clone()], &[label.clone()], &frag), None, "an audit's alone");
        assert_eq!(issuer_ledger(Reading::Whole, 1, &[pile.clone()], &[None], &[label.clone()], &frag), None, "no input made");
        assert_eq!(issuer_ledger(Reading::Whole, 1, &[None], &[input.clone()], &[label.clone()], &frag), None, "no ledger in hand");
        assert_eq!(issuer_ledger(Reading::Whole, 2, &[None, pile], &[None, input], &[None, label], &frag), Some(("COMPLETE".into(), 2)), "the last hop's");
    }
}
