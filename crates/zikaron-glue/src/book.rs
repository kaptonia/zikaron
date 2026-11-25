use std::collections::{BTreeMap, BTreeSet};
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::Token;
use zikaron::v2::tokens::Label;
use zikaron_anchor::scan::Pick;

pub use crate::recordcheck::FirstAnchor;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Law {
    One,
    Two,
}

impl Law {
    pub fn spec(self) -> &'static str {
        match self {
            Law::One => zikaron::tokens::SPEC,
            Law::Two => zikaron::v2::tokens::SPEC,
        }
    }

    pub fn domain(self) -> &'static str {
        match self {
            Law::One => zikaron::tokens::Domain::Entry.as_str(),
            Law::Two => zikaron::v2::tokens::Domain::Entry.as_str(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Genesis,
    History,
    Grant,
    Revocation,
    Succession,
    Migration,
    Annotation,
    Retraction,
    Adoption,
    Other,
    Face,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        use zikaron::tokens::EntryType as T;
        match self {
            Kind::Genesis => T::Genesis.as_str(),
            Kind::History => T::History.as_str(),
            Kind::Grant => T::Grant.as_str(),
            Kind::Revocation => T::Revocation.as_str(),
            Kind::Succession => T::Succession.as_str(),
            Kind::Migration => "migration",
            Kind::Annotation => T::Annotation.as_str(),
            Kind::Retraction => crate::retraction::ENTRY_TYPE,
            Kind::Adoption => T::Adoption.as_str(),
            Kind::Other => "",
            Kind::Face => crate::face::ENTRY_TYPE,
        }
    }

    pub fn of_literal(t: &str) -> Kind {
        match Kind::of_v2(zikaron::v2::tokens::EntryType::of(t)) {
            Kind::Other if t == crate::face::ENTRY_TYPE => Kind::Face,
            k => k,
        }
    }

    fn of_v1(e: &zikaron::entry::Entry) -> Kind {
        use zikaron::tokens::EntryType as T;
        match e.kind {
            T::Genesis => Kind::Genesis,
            T::History => Kind::History,
            T::Grant => Kind::Grant,
            T::Revocation => Kind::Revocation,
            T::Adoption => Kind::Adoption,
            T::Succession => Kind::Succession,
            T::Annotation => Kind::Annotation,
            T::Other if e.entry_type == Kind::Retraction.as_str() => Kind::Retraction,
            T::Other => Kind::Other,
        }
    }

    fn of_v2(t: zikaron::v2::tokens::EntryType) -> Kind {
        use zikaron::v2::tokens::EntryType as T;
        match t {
            T::Genesis => Kind::Genesis,
            T::History => Kind::History,
            T::Grant => Kind::Grant,
            T::Revocation => Kind::Revocation,
            T::Succession => Kind::Succession,
            T::Migration => Kind::Migration,
            T::Annotation => Kind::Annotation,
            T::Retraction => Kind::Retraction,
            T::Other => Kind::Other,
        }
    }

    pub fn may_be_named(self) -> bool {
        self != Kind::Adoption
    }

    pub fn goes_alone(self) -> bool {
        matches!(self, Kind::Succession | Kind::Migration)
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub law: Law,
    pub bytes: Vec<u8>,
    pub id: [u8; 32],
    pub value: Value,
    pub entry_type: String,
    pub kind: Kind,
    pub author: String,
    pub seq: u64,
    pub prev: Option<String>,
    pub body: Value,
}

impl Entry {
    pub fn id_hex(&self) -> String {
        hexfmt::encode(&self.id)
    }

    pub fn continues(&self) -> Option<(String, String)> {
        if self.law != Law::Two || self.kind != Kind::Genesis {
            return None;
        }
        let c = self.body.member("continues")?;
        let root = c.member("root")?.as_str()?.to_string();
        let id = c.member("entry_id")?.as_str()?.to_string();
        Some((root, id))
    }

    pub fn to(&self) -> Option<&str> {
        if self.kind != Kind::Succession {
            return None;
        }
        self.body.member("to")?.as_str()
    }

    pub fn closes(&self) -> bool {
        self.law == Law::One
            && self.to().is_some_and(|t| t.eq_ignore_ascii_case(&self.author))
            && self.body.member("kind").and_then(Value::as_str) == Some(CLOSING_KIND)
    }

    pub fn hands_off(&self) -> bool {
        let Some(to) = self.to() else { return false };
        match self.law {
            Law::Two => self.body.member("kind").and_then(Value::as_str) == Some(HANDOVER_KIND),
            Law::One => !to.eq_ignore_ascii_case(&self.author) && !self.closes(),
        }
    }

    fn of_v1(e: zikaron::entry::Entry) -> Entry {
        let kind = Kind::of_v1(&e);
        Entry {
            law: Law::One,
            bytes: e.bytes,
            id: e.id,
            value: e.value,
            entry_type: e.entry_type,
            kind,
            author: e.author,
            seq: e.seq,
            prev: e.prev,
            body: e.body,
        }
    }

    fn of_v2(e: zikaron::v2::entry::Entry) -> Entry {
        Entry {
            law: Law::Two,
            bytes: e.bytes,
            id: e.id,
            value: e.value,
            kind: match Kind::of_v2(e.kind) {
                Kind::Other if e.entry_type == crate::face::ENTRY_TYPE => Kind::Face,
                k => k,
            },
            entry_type: e.entry_type,
            author: e.author,
            seq: e.seq,
            prev: e.prev,
            body: e.body,
        }
    }
}

fn law_of(b: &[u8]) -> Law {
    let one = format!("\"spec\":\"{}\"", zikaron::tokens::SPEC);
    if b.windows(one.len()).any(|w| w == one.as_bytes()) {
        if let Ok(v) = json::parse(b) {
            if v.member("spec").and_then(Value::as_str) == Some(zikaron::tokens::SPEC) {
                return Law::One;
            }
        }
    }
    Law::Two
}

pub fn check(b: &[u8]) -> Result<Entry, Token> {
    match law_of(b) {
        Law::One => zikaron::entry::check(b).map(Entry::of_v1),
        Law::Two => zikaron::v2::entry::check(b).map(Entry::of_v2),
    }
}

pub fn entry_id(b: &[u8]) -> [u8; 32] {
    zikaron::entry::entry_id(b)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoRoot {
    Empty,
    Forked(usize),
}

fn root_named(e: &Entry) -> Option<String> {
    if e.seq != 0 || e.kind != Kind::Genesis {
        return None;
    }
    Some(match e.continues() {
        Some((root, _)) => root,
        None => e.author.clone(),
    })
}

pub fn root_of(items: &[Vec<u8>]) -> Result<String, NoRoot> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for b in items {
        if let Ok(e) = check(b) {
            if let Some(r) = root_named(&e) {
                found.insert(r);
            }
        }
    }
    match found.len() {
        0 => Err(NoRoot::Empty),
        1 => Ok(found.into_iter().next().unwrap_or_default()),
        n => Err(NoRoot::Forked(n)),
    }
}

pub fn root_entry(items: &[Vec<u8>]) -> Option<Entry> {
    items.iter().filter_map(|b| check(b).ok()).filter(|e| e.seq == 0 && e.kind == Kind::Genesis).min_by_key(|e| (e.law, e.id))
}

pub fn senders_of(items: &[Vec<u8>]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for b in items {
        let Ok(e) = check(b) else { continue };
        if let Some(to) = e.to() {
            out.insert(to.to_string());
        }
        out.insert(e.author);
    }
    out.into_iter().collect()
}

pub fn ledger_of(items: &[Vec<u8>], want: &str, author: &str) -> Option<Vec<Vec<u8>>> {
    if items.iter().any(|b| hexfmt::encode(&entry_id(b)).eq_ignore_ascii_case(want) && crate::notices::carries(b)) {
        return Some(items.to_vec());
    }
    let entries: Vec<Entry> = items.iter().filter_map(|b| check(b).ok()).collect();
    let by_id: BTreeMap<String, &Entry> = entries.iter().map(|e| (e.id_hex(), e)).collect();
    let up = |e: &Entry| -> Option<String> { e.prev.clone().or_else(|| e.continues().map(|(_, id)| id)) };
    let head_of = |e: &Entry| -> String {
        let mut at = e;
        for _ in 0..=entries.len() {
            match up(at).as_ref().and_then(|p| by_id.get(p)) {
                Some(next) => at = next,
                None => break,
            }
        }
        at.id_hex()
    };
    let mut books: Vec<(String, Vec<Vec<u8>>)> = Vec::new();
    for e in &entries {
        let h = head_of(e);
        match books.iter_mut().find(|(k, _)| *k == h) {
            Some((_, v)) => v.push(e.bytes.clone()),
            None => books.push((h, vec![e.bytes.clone()])),
        }
    }
    let holds = |v: &Vec<Vec<u8>>| v.iter().any(|b| hexfmt::encode(&entry_id(b)).eq_ignore_ascii_case(want));
    if let Some((_, v)) = books.iter().find(|(_, v)| holds(v)) {
        return Some(v.clone());
    }
    books.into_iter().find(|(_, v)| senders_of(v).iter().any(|a| a.eq_ignore_ascii_case(author))).map(|(_, v)| v)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    New,
    Continued,
    Pending,
    Old,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    Root(NoRoot),
    Fragment,
    Domain(String),
    NoLabel,
}

#[derive(Clone, Debug)]
pub struct Ledger {
    pub root: String,
    pub form: Form,
    pub label: Label,
    pub report: Option<Value>,
    pub old_report: Option<Value>,
    pub entries: Vec<Entry>,
    pub old_len: usize,
    pub lineage: BTreeSet<String>,
    pub office: Option<String>,
    pub retired: BTreeSet<String>,
    pub first: BTreeMap<String, FirstAnchor>,
}

impl Ledger {
    pub fn rows(&self, name: &str) -> Vec<Value> {
        let list = |r: &Option<Value>| -> Vec<Value> { r.as_ref().and_then(|r| r.member(name)).and_then(Value::as_arr).cloned().unwrap_or_default() };
        let mut out = list(&self.old_report);
        out.extend(list(&self.report));
        out
    }

    pub fn merged(&self) -> Value {
        use zikaron::v2::tokens::Key as K;
        let list = |k: K| (k.as_str().to_string(), Value::Arr(self.rows(k.as_str())));
        let basis = self
            .report
            .as_ref()
            .and_then(|r| r.member(K::Basis.as_str()))
            .or_else(|| self.old_report.as_ref().and_then(|r| r.member(K::Basis.as_str())))
            .cloned()
            .unwrap_or(Value::Null);
        let continues = self.report.as_ref().and_then(|r| r.member(K::Continues.as_str())).cloned().unwrap_or(Value::Null);
        Value::Obj(vec![
            (K::Root.as_str().to_string(), Value::Str(self.root.clone())),
            (K::Basis.as_str().to_string(), basis),
            (K::Entries.as_str().to_string(), Value::Int(self.entries.len() as u64)),
            list(K::Findings),
            list(K::Missing),
            list(K::Anchored),
            list(K::Unanchored),
            list(K::Excluded),
            list(K::Retired),
            list(K::UnknownType),
            list(K::Malformed),
            list(K::Unproven),
            list(K::Void),
            list(K::Discarded),
            (K::Continues.as_str().to_string(), continues),
            (K::LabelKey.as_str().to_string(), Value::Str(self.label.as_str().to_string())),
        ])
    }

    pub fn anchored_ids(&self) -> BTreeSet<String> {
        self.rows("anchored").iter().filter_map(|r| r.member("entry_id").and_then(Value::as_str).map(str::to_ascii_lowercase)).collect()
    }

    pub fn may_write(&self, key: &str) -> bool {
        self.form != Form::Old
            && self.office.as_deref().is_some_and(|o| o.eq_ignore_ascii_case(key))
            && !self.retired.iter().any(|r| r.eq_ignore_ascii_case(key))
    }

    pub fn head(&self) -> Option<&Entry> {
        let new = &self.entries[self.old_len..];
        let top = new.iter().map(|e| e.seq).max()?;
        new.iter().find(|e| e.seq == top)
    }

    pub fn position(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.id_hex().eq_ignore_ascii_case(id))
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id_hex().eq_ignore_ascii_case(id))
    }

    pub fn is_old(&self, id: &str) -> bool {
        self.position(id).is_some_and(|p| p < self.old_len)
    }

    pub fn successions(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.kind == Kind::Succession)
    }

    pub fn joined(&self) -> bool {
        let old = &self.entries[..self.old_len];
        self.entries[self.old_len..].iter().filter(|e| e.continues().is_some_and(|(r, _)| r.eq_ignore_ascii_case(&self.root))).all(|g| {
            let named = g.continues().map(|(_, id)| id).unwrap_or_default();
            old.iter().any(|s| s.id_hex().eq_ignore_ascii_case(&named) && s.closes() && s.author.eq_ignore_ascii_case(&g.author))
        })
    }

    pub fn handed_off_old(&self, key: &str) -> bool {
        self.entries[..self.old_len].iter().any(|e| e.law == Law::One && e.author.eq_ignore_ascii_case(key) && e.to().is_some_and(|t| !t.eq_ignore_ascii_case(key)))
    }

    pub fn may_succeed(&self, to: &str) -> bool {
        let bare = |k: &str| k.trim().trim_start_matches("0x").to_ascii_lowercase();
        let t = bare(to);
        bare(&self.root) != t && !self.lineage.iter().any(|k| bare(k) == t)
    }

    pub fn closing_by(&self, author: &str) -> Option<&Entry> {
        self.entries[..self.old_len].iter().find(|e| e.closes() && e.author.eq_ignore_ascii_case(author))
    }
}

pub fn anchored_on(report: &Value, chain: u64) -> BTreeSet<String> {
    let on = |row: &Value| matches!(row.member("anchors"), Some(Value::Arr(rs)) if rs.iter().any(|r| matches!(r.member("chainId"), Some(Value::Int(c)) if chain == 0 || *c == chain)));
    let mut out = BTreeSet::new();
    let mut take = |rows: &[Value]| {
        for r in rows.iter().filter(|r| on(r)) {
            if let Some(id) = r.member("entry_id").and_then(|v| v.as_str()) {
                out.insert(id.to_ascii_lowercase());
            }
        }
    };
    if let Some(Value::Arr(rows)) = report.member("anchored") {
        take(rows);
    }
    if let Some(Value::Arr(rows)) = report.member("continues").and_then(|c| c.member("report")).and_then(|r| r.member("anchored")) {
        take(rows);
    }
    out
}

pub fn unanchored_in(l: &Ledger, chain: u64) -> Vec<String> {
    let mut on = BTreeSet::new();
    for r in [l.report.as_ref(), l.old_report.as_ref()].into_iter().flatten() {
        on.extend(anchored_on(r, chain));
    }
    l.entries.iter().map(|e| e.id_hex().to_ascii_lowercase()).filter(|id| !on.contains(id)).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sender {
    Counted,
    Retired,
    Outside,
}

impl Sender {
    pub fn as_str(self) -> &'static str {
        match self {
            Sender::Counted => "COUNTED",
            Sender::Retired => "RETIRED",
            Sender::Outside => "NOT_IN_LINEAGE",
        }
    }
}

pub fn sender_now(l: &Ledger, who: &str) -> Sender {
    let bare = |s: &str| s.trim().trim_start_matches("0x").to_ascii_lowercase();
    let who = bare(who);
    if l.retired.iter().any(|k| bare(k) == who) {
        Sender::Retired
    } else if l.lineage.iter().any(|k| bare(k) == who) {
        Sender::Counted
    } else {
        Sender::Outside
    }
}

pub fn empty_fragment() -> Value {
    Value::Obj(vec![
        ("anchors".to_string(), Value::Arr(Vec::new())),
        ("basis".to_string(), Value::Obj(vec![("chains".to_string(), Value::Arr(Vec::new()))])),
    ])
}

fn hex_pile(items: &[Vec<u8>]) -> Vec<String> {
    items.iter().map(|b| hexfmt::encode(b)).collect()
}

pub fn input_of(root: &str, items: &[Vec<u8>], fragment: &Value) -> Result<Value, Unread> {
    let input = zikaron_anchor::input::assemble(fragment, root, &hex_pile(items)).ok_or(Unread::Fragment)?;
    json::parse(&json::canon_bytes(&input)).map_err(|t| Unread::Domain(format!("{t:?}")))?;
    Ok(input)
}

fn v1_basis(basis: &Value) -> Value {
    let Value::Obj(members) = basis else { return basis.clone() };
    let mut out = members.clone();
    for k in ["adoptionChains", "bareTx"] {
        if basis.member(k).is_none() {
            out.push((k.to_string(), Value::Arr(Vec::new())));
        }
    }
    Value::Obj(out)
}

fn v1_read(root: &str, bytes: &[Vec<u8>], anchors: &[Value], basis: &Value, evidence: &[Value]) -> Option<zikaron::audit::Outcome> {
    let input = Value::Obj(vec![
        ("anchors".into(), Value::Arr(anchors.to_vec())),
        ("basis".into(), v1_basis(basis)),
        ("evidence".into(), Value::Arr(evidence.to_vec())),
        ("pile".into(), Value::Arr(hex_pile(bytes).into_iter().map(Value::Str).collect())),
        ("root".into(), Value::Str(root.to_string())),
        ("unavailable".into(), Value::Arr(Vec::new())),
    ]);
    zikaron::audit::audit_full(&input)
}

fn office_after(seed: &str, ledger: &[Entry]) -> Option<String> {
    let mut at = Some(seed.to_string());
    let mut moved_at: Option<u64> = None;
    for e in ledger {
        let Some(cur) = at.clone() else { break };
        if !e.author.eq_ignore_ascii_case(&cur) || moved_at == Some(e.seq) {
            continue;
        }
        match e.kind {
            Kind::Succession => {
                if let Some(to) = e.to() {
                    at = Some(to.to_string());
                    moved_at = Some(e.seq);
                }
            }
            Kind::Migration if e.law == Law::Two => at = None,
            _ => {}
        }
    }
    at
}

fn v1_first(outcome: &zikaron::audit::Outcome, anchors: &[Value]) -> BTreeMap<String, FirstAnchor> {
    let fragment = Value::Obj(vec![("anchors".to_string(), Value::Arr(anchors.to_vec()))]);
    crate::recordcheck::first_anchors(outcome, &fragment).into_iter().collect()
}

fn v2_first(outcome: &zikaron::v2::audit::Outcome) -> BTreeMap<String, FirstAnchor> {
    let view = zikaron_kit::v2::read::view_of(outcome);
    view.ledger
        .iter()
        .filter_map(|r| {
            let a = view.first_anchor(&r.id)?;
            Some((
                r.id.clone(),
                FirstAnchor {
                    chain_id: a.chain_id,
                    block_number: a.block_number,
                    block_timestamp: a.block_timestamp,
                    tx: a.tx.clone(),
                    hash: a.hash.clone(),
                    registry: None,
                },
            ))
        })
        .collect()
}

fn retired_of(new_entries: &[zikaron::v2::entry::Entry], input: &Value) -> BTreeSet<String> {
    let Some(inputs) = zikaron::v2::audit::read_inputs(input) else { return BTreeSet::new() };
    zikaron::v2::audit::retirements(new_entries, &inputs.anchors).into_keys().collect()
}

fn hash_of(r: &Value) -> Option<String> {
    r.member("hash").and_then(Value::as_str).map(str::to_ascii_lowercase)
}

pub fn read(items: &[Vec<u8>], fragment: &Value) -> Result<Ledger, Unread> {
    let root = root_of(items).map_err(Unread::Root)?;
    read_under(&root, items, fragment)
}

pub fn read_under(root: &str, items: &[Vec<u8>], fragment: &Value) -> Result<Ledger, Unread> {
    let input = input_of(root, items, fragment)?;
    let basis = fragment.member("basis").cloned().unwrap_or(Value::Null);
    let anchors: Vec<Value> = fragment.member("anchors").and_then(Value::as_arr).cloned().unwrap_or_default();
    let parsed: Vec<Entry> = items.iter().filter_map(|b| check(b).ok()).collect();
    let v2_all: Vec<zikaron::v2::entry::Entry> = items.iter().filter_map(|b| zikaron::v2::entry::check(b).ok()).collect();
    let continuing = parsed.iter().any(|e| e.continues().is_some_and(|(r, _)| r.eq_ignore_ascii_case(root)));
    let has_v1 = parsed.iter().any(|e| e.law == Law::One);
    let has_v2_genesis = parsed.iter().any(|e| e.law == Law::Two && e.seq == 0 && e.kind == Kind::Genesis && e.author.eq_ignore_ascii_case(root));

    if has_v1 && !continuing && !has_v2_genesis {
        let evidence = fragment.member("evidence").and_then(Value::as_arr).cloned().unwrap_or_default();
        let old_out = v1_read(root, items, &anchors, &basis, &evidence).ok_or(Unread::NoLabel)?;
        let old: Vec<Entry> = old_out.ledger.iter().cloned().map(Entry::of_v1).collect();
        return Ok(Ledger {
            root: root.to_string(),
            form: Form::Old,
            label: Label::of_v1(old_out.label.as_str()).unwrap_or(Label::BrokenChain),
            report: None,
            old_report: Some(old_out.report.clone()),
            old_len: old.len(),
            lineage: old_out.lineage.iter().cloned().collect(),
            office: office_after(root, &old),
            retired: BTreeSet::new(),
            first: v1_first(&old_out, &anchors),
            entries: old,
        });
    }

    let outcome = zikaron::v2::audit::audit_full(&input).ok_or(Unread::NoLabel)?;
    if let Some(c) = &outcome.continuation {
        let mut old: Vec<Entry> =
            c.old.entries.iter().filter(|e| c.old.ledger.contains(&e.id_hex())).cloned().map(Entry::of_v1).collect();
        old.sort_by(|a, b| (a.seq, a.id).cmp(&(b.seq, b.id)));
        let new: Vec<Entry> = outcome.new.ledger.iter().cloned().map(Entry::of_v2).collect();
        let mut lineage: BTreeSet<String> = outcome.new.lineage.clone();
        lineage.extend(c.old.lineage.iter().cloned());
        let office = office_after(&outcome.new.seed, &new);
        let old_len = old.len();
        let mut entries = old;
        entries.extend(new);
        return Ok(Ledger {
            root: root.to_string(),
            form: Form::Continued,
            label: outcome.label,
            report: Some(outcome.report.clone()),
            old_report: Some(c.old.report.clone()),
            old_len,
            lineage,
            office,
            retired: retired_of(&v2_all, &input),
            first: v2_first(&outcome),
            entries,
        });
    }
    if !has_v1 || !continuing {
        let new: Vec<Entry> = outcome.new.ledger.iter().cloned().map(Entry::of_v2).collect();
        return Ok(Ledger {
            root: root.to_string(),
            form: Form::New,
            label: outcome.label,
            report: Some(outcome.report.clone()),
            old_report: None,
            old_len: 0,
            lineage: outcome.new.lineage.clone(),
            office: office_after(&outcome.new.seed, &new),
            retired: retired_of(&v2_all, &input),
            first: v2_first(&outcome),
            entries: new,
        });
    }

    let v1_bytes: Vec<Vec<u8>> = parsed.iter().filter(|e| e.law == Law::One).map(|e| e.bytes.clone()).collect();
    let v1_ids: BTreeSet<String> = v1_bytes.iter().map(|b| hexfmt::encode(&entry_id(b))).collect();
    let (old_anchors, new_anchors): (Vec<Value>, Vec<Value>) = anchors.iter().cloned().partition(|r| hash_of(r).is_some_and(|h| v1_ids.contains(&h)));
    let old_out = v1_read(root, &v1_bytes, &old_anchors, &basis, &[]).ok_or(Unread::NoLabel)?;
    let old_label = Label::of_v1(old_out.label.as_str()).unwrap_or(Label::BrokenChain);
    let old: Vec<Entry> = old_out.ledger.iter().cloned().map(Entry::of_v1).collect();
    let mut first = v1_first(&old_out, &old_anchors);
    let old_len = old.len();
    let new_items: Vec<Vec<u8>> = items.iter().filter(|b| !v1_ids.contains(&hexfmt::encode(&entry_id(b)))).cloned().collect();
    let new_fragment = Value::Obj(vec![("anchors".to_string(), Value::Arr(new_anchors)), ("basis".to_string(), basis)]);
    let seed = parsed
        .iter()
        .filter(|e| e.continues().is_some_and(|(r, _)| r.eq_ignore_ascii_case(root)))
        .min_by_key(|e| e.id)
        .map(|e| e.author.clone())
        .unwrap_or_else(|| root.to_string());
    let new_input = input_of(&seed, &new_items, &new_fragment)?;
    let new_out = zikaron::v2::audit::audit_full(&new_input).ok_or(Unread::NoLabel)?;
    let new: Vec<Entry> = new_out.new.ledger.iter().cloned().map(Entry::of_v2).collect();
    first.extend(v2_first(&new_out));
    let mut lineage: BTreeSet<String> = new_out.new.lineage.clone();
    lineage.extend(old_out.lineage.iter().cloned());
    let office = office_after(&new_out.new.seed, &new);
    let mut entries = old;
    entries.extend(new);
    Ok(Ledger {
        root: root.to_string(),
        form: Form::Pending,
        label: new_out.label.min(old_label),
        report: Some(new_out.report.clone()),
        old_report: Some(old_out.report.clone()),
        old_len,
        lineage,
        office,
        retired: retired_of(&v2_all, &new_input),
        first,
        entries,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoTip {
    Absent,
    Forked,
    NotMoved,
}

pub fn tip(root: &str, items: &[Vec<u8>]) -> Result<(u64, String), NoTip> {
    let read = read_under(root, items, &empty_fragment()).map_err(|_| NoTip::Absent)?;
    if read.form == Form::Old {
        return Err(NoTip::NotMoved);
    }
    let new = &read.entries[read.old_len..];
    let Some(top) = new.iter().map(|e| e.seq).max() else { return Err(NoTip::Absent) };
    let mut ids: Vec<String> = new.iter().filter(|e| e.seq == top).map(|e| e.id_hex()).collect();
    ids.sort();
    ids.dedup();
    match ids.len() {
        1 => Ok((top + 1, ids.remove(0))),
        _ => Err(NoTip::Forked),
    }
}

pub fn new_findings(items: &[Vec<u8>], candidate: &[u8], named: Option<&str>) -> Result<Vec<String>, Barred> {
    let mut all = items.to_vec();
    all.push(candidate.to_vec());
    let root = match (named, root_of(&all)) {
        (Some(r), _) => r.to_string(),
        (None, Ok(r)) => r,
        (None, Err(NoRoot::Forked(_))) => return Err(Barred::Tip(NoTip::Forked)),
        (None, Err(_)) => return Ok(Vec::new()),
    };
    let found = |read: Option<&Ledger>| -> Vec<(String, String)> {
        let text = |f: &Value, k: &str| f.member(k).and_then(Value::as_str).unwrap_or("").to_string();
        read.map(|r| r.rows("findings").iter().map(|f| (text(f, "name"), text(f, "entry_id"))).collect()).unwrap_or_default()
    };
    let with = read_under(&root, &all, &empty_fragment()).ok();
    if let (Some(l), Ok(e)) = (&with, check(candidate)) {
        let counted = sender_now(l, &e.author);
        if counted != Sender::Counted {
            return Err(Barred::Signer { counted, key: e.author, office: l.office.clone().unwrap_or_default() });
        }
    }
    let before = found(read_under(&root, items, &empty_fragment()).ok().as_ref());
    let mut names: Vec<String> = found(with.as_ref()).into_iter().filter(|f| !before.contains(f)).map(|(n, _)| n).collect();
    names.sort();
    names.dedup();
    Ok(names)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Barred {
    Tip(NoTip),
    Signer { counted: Sender, key: String, office: String },
}

fn v1_input(input: &Value) -> Value {
    let basis = input.member("basis").cloned().unwrap_or(Value::Null);
    Value::Obj(vec![
        ("anchors".into(), input.member("anchors").cloned().unwrap_or(Value::Arr(Vec::new()))),
        ("basis".into(), v1_basis(&basis)),
        ("evidence".into(), Value::Arr(Vec::new())),
        ("pile".into(), input.member("pile").cloned().unwrap_or(Value::Arr(Vec::new()))),
        ("root".into(), input.member("root").cloned().unwrap_or(Value::Null)),
        ("unavailable".into(), Value::Arr(Vec::new())),
    ])
}

pub fn behind_book(copy: &[Vec<u8>], key: Option<&str>) -> bool {
    let bare = |k: &str| k.trim().trim_start_matches("0x").to_ascii_lowercase();
    key.is_some_and(|k| !copy.is_empty() && !senders_of(copy).iter().any(|x| bare(x) == bare(k)))
}

pub fn under_book<'a>(input: Option<&'a Value>, key: Option<&str>) -> Option<&'a Value> {
    let copy: Vec<Vec<u8>> = input.and_then(|i| i.member("pile")).and_then(Value::as_arr).map(|a| a.iter().filter_map(|x| x.as_str().and_then(hexfmt::decode)).collect()).unwrap_or_default();
    input.filter(|_| !behind_book(&copy, key))
}

pub fn grant_check(g: &[u8], input: Option<&Value>, notices: Option<&Value>, now: Option<u64>) -> Value {
    let view = input.and_then(|i| zikaron_kit::v2::read::load_view(i, Some(g)));
    if under_old_law(g, view.as_ref()) {
        let v1 = input.map(v1_input);
        return zikaron_kit::check::grant_check(g, v1.as_ref(), now).value;
    }
    let notices = notices.and_then(zikaron_kit::v2::read::load_notices);
    zikaron_kit::v2::read::grant_check(g, view.as_ref(), notices.as_ref(), now).result
}

pub fn withdrawing_notice(g: &[u8], notices: &Value) -> Option<(u64, u64, u64)> {
    let n = zikaron_kit::v2::read::load_notices(notices)?;
    zikaron_kit::v2::read::withdrawing_notice(g, None, Some(&n))
}

fn under_old_law(g: &[u8], view: Option<&zikaron_kit::v2::read::View>) -> bool {
    let carries = zikaron_kit::v2::doc::accept(g).is_ok_and(|e| zikaron_kit::v2::read::member_of(&e).carried);
    check(g).is_ok_and(|e| e.law == Law::One) && !carries && !view.is_some_and(|v| v.continued)
}

pub fn chain_check(hops: &[(Vec<u8>, Option<Value>, Option<Value>)], now: Option<u64>) -> Value {
    use zikaron_kit::v2::read::{load_notices, load_view, Hop};
    let read: Vec<Hop> = hops
        .iter()
        .map(|(g, i, n)| (g.clone(), i.as_ref().and_then(|i| load_view(i, Some(g))), n.as_ref().and_then(load_notices)))
        .collect();
    if !read.is_empty() && read.iter().all(|(g, v, _)| under_old_law(g, v.as_ref())) {
        let inputs: Vec<Option<Value>> = hops.iter().map(|(_, i, _)| i.as_ref().map(v1_input)).collect();
        let old: Vec<zikaron_kit::check::Hop> = hops.iter().zip(inputs).map(|((g, _, _), input)| zikaron_kit::check::Hop { grant: g, input }).collect();
        return zikaron_kit::check::chain_check(&old, now);
    }
    zikaron_kit::v2::read::chain_check(&read, now)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Undecided {
    OldLaw,
    Continued,
}

pub const NO_NODE_WORDS: (&str, &str) = ("未配置节点", "No nodes configured");

pub const OVER_ASKS_WORDS: (&str, &str) = ("各节点都没能在问数上限之内读完这一段。", "No node read this range within the limit on questions.");

pub const OVER_ASKS_NEXT_WORDS: (&str, &str) = ("可到设置「网络」调大「节点最大问数上限」,或添一台读得了整段的节点。", "Raise “Most questions per node” in Settings › Network, or add a node that reads the whole range.");
pub const NO_LEDGER_WORDS: (&str, &str) = ("缺少签发者账本", "Issuer ledger missing");
pub const LEDGER_REFUSED_WORDS: (&str, &str) = ("签发者账本读不成", "Issuer ledger unreadable");
pub const CHAIN_UNREAD_WORDS: (&str, &str) = ("暂时读不到链", "Chain unreadable right now");
pub const NOT_YET_ANCHORED_WORDS: (&str, &str) = ("未确认上链", "Not yet confirmed on chain");
pub const NO_TIME_WORDS: (&str, &str) = ("读不到链上时间", "Chain time unavailable");
pub const LEDGER_BEHIND_WORDS: (&str, &str) = ("签发者账本有更新", "Issuer ledger has updates");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gap {
    NoLedger,
    LedgerRefused(String),
    NoNode,
    ChainUnread(String),
    NotYetAnchored,
    NoTime,
    LedgerBehind,
    NoticesUnread(String),
    Undecided(Undecided),
}

impl Gap {
    pub fn chain_unread(&self) -> bool {
        matches!(self, Gap::NoNode | Gap::ChainUnread(_) | Gap::NoTime | Gap::NoticesUnread(_))
    }

    pub fn code(&self) -> &'static str {
        match self {
            Gap::NoLedger => "NO_LEDGER",
            Gap::LedgerRefused(_) => "LEDGER_UNREADABLE",
            Gap::NoNode => "NO_NODE",
            Gap::ChainUnread(_) => "CHAIN_UNREAD",
            Gap::NotYetAnchored => "NOT_YET_ANCHORED",
            Gap::NoTime => "NO_CHAIN_TIME",
            Gap::LedgerBehind => "LEDGER_BEHIND",
            Gap::NoticesUnread(_) => "NOTICES_UNREAD",
            Gap::Undecided(u) => u.code(),
        }
    }

    pub fn words(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Gap::NoLedger => Some(NO_LEDGER_WORDS),
            Gap::LedgerRefused(_) => Some(LEDGER_REFUSED_WORDS),
            Gap::NoNode => Some(NO_NODE_WORDS),
            Gap::ChainUnread(_) => Some(CHAIN_UNREAD_WORDS),
            Gap::NotYetAnchored => Some(NOT_YET_ANCHORED_WORDS),
            Gap::NoTime => Some(NO_TIME_WORDS),
            Gap::LedgerBehind => Some(LEDGER_BEHIND_WORDS),
            Gap::NoticesUnread(_) | Gap::Undecided(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerRead {
    None,
    Refused(String),
    Read { behind: bool },
    Undecided(Undecided),
}

pub fn gap_of(token: &str, state: &str, ledger: &LedgerRead, chain: Option<Gap>, missed: Option<String>, no_time: bool) -> Option<Gap> {
    use zikaron_kit::tokens::{Check as C, State};
    if state != State::Unknown.as_str() {
        return None;
    }
    let ledger_gap = || match ledger {
        LedgerRead::None => Some(Gap::NoLedger),
        LedgerRead::Refused(why) => Some(Gap::LedgerRefused(why.clone())),
        LedgerRead::Read { behind: true } => Some(Gap::LedgerBehind),
        LedgerRead::Read { .. } => None,
        LedgerRead::Undecided(u) => Some(Gap::Undecided(*u)),
    };
    if let LedgerRead::Undecided(u) = ledger {
        return (token != C::Expired.as_str()).then_some(Gap::Undecided(*u));
    }
    if token == C::Unanchored.as_str() {
        return chain.or_else(ledger_gap).or_else(|| missed.map(Gap::ChainUnread)).or(Some(Gap::NotYetAnchored));
    }
    if token == C::Expired.as_str() {
        return match (no_time, chain) {
            (true, Some(Gap::NoNode)) => Some(Gap::NoNode),
            _ => Some(Gap::NoTime),
        };
    }
    if token == C::BrokenLedger.as_str() || token == C::NotInLedger.as_str() || token == C::Revoked.as_str() {
        return ledger_gap();
    }
    None
}

pub fn notices_gap_of(said: Option<&str>, token: &str, state: &str) -> Option<Gap> {
    use zikaron_kit::tokens::{Check as C, State};
    let reads_notices = [C::NotInLedger.as_str(), C::Unanchored.as_str(), C::Revoked.as_str()].contains(&token);
    said.filter(|_| state == State::Unknown.as_str() && reads_notices).map(|w| Gap::NoticesUnread(w.to_string()))
}

impl Undecided {
    pub fn code(self) -> &'static str {
        match self {
            Undecided::OldLaw => "OLD_LAW",
            Undecided::Continued => "CONTINUED",
        }
    }
}

pub struct Hold<'a> {
    pub grant: &'a [u8],
    pub root: &'a str,
    pub pile: &'a [Vec<u8>],
}

#[derive(Clone, Debug)]
pub struct Decision {
    grant: Vec<u8>,
    root: String,
    pile: Vec<Vec<u8>>,
    prevs: BTreeMap<[u8; 32], Option<[u8; 32]>>,
    retiring: BTreeSet<[u8; 32]>,
    line: BTreeSet<[u8; 32]>,
    carries: bool,
    added: BTreeSet<[u8; 32]>,
}

fn h32(x: &str) -> Option<[u8; 32]> {
    hexfmt::decode(x).and_then(|b| <[u8; 32]>::try_from(b).ok())
}

pub fn decision_set(hold: &Hold) -> Result<Decision, Undecided> {
    let entries: Vec<Entry> = hold.pile.iter().chain(std::iter::once(&hold.grant.to_vec())).filter_map(|b| check(b).ok()).collect();
    let same = |a: &str, b: &str| a.trim_start_matches("0x").eq_ignore_ascii_case(b.trim_start_matches("0x"));
    let continued = entries.iter().any(|g| {
        g.seq == 0
            && g.continues().is_some_and(|(root, id)| {
                same(&root, hold.root) && entries.iter().any(|x| x.closes() && same(&x.id_hex(), &id) && same(&x.author, &g.author))
            })
    });
    if continued {
        return Err(Undecided::Continued);
    }
    let carries = crate::notices::carries(hold.grant);
    if check(hold.grant).is_ok_and(|e| e.law == Law::One) && !carries {
        return Err(Undecided::OldLaw);
    }
    let grant_id = entry_id(hold.grant);
    let two: Vec<&Entry> = entries.iter().filter(|e| e.law == Law::Two).collect();
    let prevs: BTreeMap<[u8; 32], Option<[u8; 32]>> = two.iter().map(|e| (e.id, e.prev.as_deref().and_then(h32))).collect();
    let retiring = two.iter().filter(|e| matches!(e.kind, Kind::Succession | Kind::Migration)).map(|e| e.id).collect();
    let mut line: BTreeSet<[u8; 32]> = prevs.keys().copied().filter(|f| reaches(&prevs, *f, |x| x == grant_id)).collect();
    line.insert(grant_id);
    Ok(Decision { grant: hold.grant.to_vec(), root: hold.root.to_string(), pile: hold.pile.to_vec(), prevs, retiring, line, carries, added: BTreeSet::new() })
}

fn reaches(prevs: &BTreeMap<[u8; 32], Option<[u8; 32]>>, from: [u8; 32], hit: impl Fn([u8; 32]) -> bool) -> bool {
    let mut at = Some(from);
    let mut seen = BTreeSet::new();
    while let Some(x) = at {
        if !prevs.contains_key(&x) || !seen.insert(x) {
            return false;
        }
        if hit(x) {
            return true;
        }
        at = prevs.get(&x).copied().flatten();
    }
    false
}

impl Decision {
    fn tests(&self, hash: &[u8; 32]) -> bool {
        self.retiring.contains(hash) || self.line.contains(hash) || self.added.contains(hash) || (!self.carries && !self.prevs.contains_key(hash))
    }

    pub fn pick(&self) -> Pick {
        pick_of(std::slice::from_ref(self))
    }
}

pub fn pick_of(hops: &[Decision]) -> Pick {
    let hops = hops.to_vec();
    Pick::only(move |hash| hops.iter().any(|d| d.tests(hash)))
}

#[derive(Clone, Debug)]
pub enum Fast {
    Ready(Value),
    More,
}

pub fn fast_input(d: &mut Decision, fragment: &Value) -> Result<Fast, Unread> {
    let input = input_of(&d.root, &d.pile, fragment)?;
    let untested = zikaron_anchor::scan::candidates_in(fragment);
    if untested.is_empty() {
        return Ok(Fast::Ready(input));
    }
    let mut with_grant = d.pile.clone();
    if !with_grant.contains(&d.grant) {
        with_grant.push(d.grant.clone());
    }
    let Some(outcome) = input_of(&d.root, &with_grant, fragment).ok().and_then(|i| zikaron::v2::audit::audit_full(&i)) else {
        return Ok(Fast::Ready(input));
    };
    let report = &outcome.report;
    let ids = |list: &str, member: Option<&str>| -> BTreeSet<[u8; 32]> {
        report.member(list).and_then(Value::as_arr).map(|xs| xs.iter().filter_map(|x| match member { Some(m) => x.member(m), None => Some(x) }).filter_map(Value::as_str).filter_map(h32).collect()).unwrap_or_default()
    };
    let retired = ids("retired", Some("entry_id"));
    let mut have = ids("anchored", Some("entry_id"));
    have.extend(ids("unanchored", None));
    let need: BTreeSet<[u8; 32]> = untested
        .iter()
        .map(|c| c.hash)
        .filter(|h| reaches(&d.prevs, *h, |x| retired.contains(&x)) || (!d.carries && !have.contains(h)))
        .collect();
    if need.is_empty() {
        return Ok(Fast::Ready(input));
    }
    d.added.extend(need);
    Ok(Fast::More)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KitCounts {
    pub kit_id: String,
    pub entries: usize,
    pub files: usize,
    pub proofs: usize,
    pub invalid: usize,
}

pub fn kit_counts(dir: &std::path::Path) -> Result<KitCounts, (String, Option<String>)> {
    use zikaron_kit::v2::dispatch::{read_kit_dir, KitReading};
    match read_kit_dir(dir) {
        KitReading::Kit1(zikaron_kit::kitdir::KitVerdict::Ok { kit_id, entries, files, proofs, invalid }) => {
            Ok(KitCounts { kit_id: hexfmt::encode(&kit_id), entries, files, proofs, invalid: invalid.len() })
        }
        KitReading::Kit1(zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject }) => Err((verdict.as_str().to_string(), subject)),
        KitReading::Kit2(zikaron_kit::v2::kit::KitVerdict::Ok { kit_id, entries, files, invalid, .. }) => Ok(KitCounts { kit_id, entries, files, proofs: 0, invalid: invalid.len() }),
        KitReading::Kit2(zikaron_kit::v2::kit::KitVerdict::Fail { token, subject }) => Err((token.as_str().to_string(), subject)),
    }
}

pub fn kit_verdict(dir: &std::path::Path) -> Result<String, (String, Option<String>)> {
    kit_counts(dir).map(|c| c.kit_id)
}

pub mod envelope {
    pub const SPEC: &str = "spec";
    pub const ENTRY_TYPE: &str = "entryType";
    pub const AUTHOR: &str = "author";
    pub const SEQ: &str = "seq";
    pub const PREV: &str = "prev";
    pub const BODY: &str = "body";
    pub const SIG: &str = "sig";
}

pub fn six(law: Law, author: &str, entry_type: &str, seq: u64, prev: Option<&str>, body: Value) -> Value {
    Value::Obj(vec![
        (envelope::SPEC.to_string(), Value::Str(law.spec().to_string())),
        (envelope::ENTRY_TYPE.to_string(), Value::Str(entry_type.to_string())),
        (envelope::AUTHOR.to_string(), Value::Str(author.to_string())),
        (envelope::SEQ.to_string(), Value::Int(seq)),
        (envelope::PREV.to_string(), prev.map(|p| Value::Str(p.to_string())).unwrap_or(Value::Null)),
        (envelope::BODY.to_string(), body),
    ])
}

pub fn preimage(six: &Value) -> Vec<u8> {
    zikaron::entry::b6_bytes(six)
}

pub enum Sealed<E> {
    Signer(E),
    Refused(Token),
    Done(Entry),
}

pub fn seal<E>(
    law: Law,
    author: &str,
    entry_type: &str,
    seq: u64,
    prev: Option<&str>,
    body: Value,
    sign: impl FnOnce(&[u8], &str) -> Result<String, E>,
) -> Sealed<E> {
    let six = six(law, author, entry_type, seq, prev, body);
    let sig = match sign(&preimage(&six), law.domain()) {
        Ok(s) => s,
        Err(e) => return Sealed::Signer(e),
    };
    let Value::Obj(mut members) = six else { return Sealed::Refused(Token::Envelope) };
    members.push((envelope::SIG.to_string(), Value::Str(sig)));
    let bytes = json::canon_bytes(&Value::Obj(members));
    match check(&bytes) {
        Ok(e) if e.law == law => Sealed::Done(e),
        Ok(_) => Sealed::Refused(Token::Spec),
        Err(t) => Sealed::Refused(t),
    }
}

pub const HANDOVER_KIND: &str = "handover";

pub const CLOSING_KIND: &str = zikaron::v2::tokens::CLOSING_KIND;

pub fn closing_body(key: &str, effective: u64, statement_md: &str) -> Value {
    Value::Obj(vec![
        ("effective".to_string(), Value::Int(effective)),
        ("kind".to_string(), Value::Str(CLOSING_KIND.to_string())),
        ("statement_md".to_string(), Value::Str(statement_md.to_string())),
        ("to".to_string(), Value::Str(key.to_string())),
    ])
}

pub fn genesis_body(statement_md: &str, continues: Option<(&str, &str)>) -> Value {
    let mut m = Vec::new();
    if let Some((root, id)) = continues {
        m.push((
            "continues".to_string(),
            Value::Obj(vec![("entry_id".to_string(), Value::Str(id.to_string())), ("root".to_string(), Value::Str(root.to_string()))]),
        ));
    }
    m.push(("statement_md".to_string(), Value::Str(statement_md.to_string())));
    Value::Obj(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_office_moves_once_per_position_and_a_migration_closes_it() {
        let mk = |seq: u64, kind: Kind, author: &str, to: Option<&str>, id: u8| Entry {
            law: Law::Two,
            bytes: Vec::new(),
            id: [id; 32],
            value: Value::Null,
            entry_type: kind.as_str().to_string(),
            kind,
            author: author.to_string(),
            seq,
            prev: None,
            body: match to {
                Some(t) => Value::Obj(vec![("to".to_string(), Value::Str(t.to_string()))]),
                None => Value::Obj(Vec::new()),
            },
        };
        let a = "0x00000000000000000000000000000000000000aa";
        let b = "0x00000000000000000000000000000000000000bb";
        let c = "0x00000000000000000000000000000000000000cc";
        let ledger = vec![mk(0, Kind::Genesis, a, None, 1), mk(1, Kind::Succession, a, Some(b), 2), mk(1, Kind::Succession, a, Some(c), 3)];
        assert_eq!(office_after(a, &ledger).as_deref(), Some(b));
        let closed = vec![mk(0, Kind::Genesis, a, None, 1), mk(1, Kind::Migration, a, None, 2)];
        assert_eq!(office_after(a, &closed), None);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Move {
    Close { seq: u64, prev: String, body: Value },
    Genesis { closing: String, body: Value },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoMove {
    Moved,
    TipAbsent,
    TipForked,
}

pub fn next_move(read: &Ledger, author: &str, effective: u64, statement: Option<&str>) -> Result<Move, NoMove> {
    if read.form != Form::Old {
        return Err(NoMove::Moved);
    }
    let old = &read.entries;
    let top = old.iter().map(|e| e.seq).max().ok_or(NoMove::TipAbsent)?;
    let tips: Vec<&Entry> = old.iter().filter(|e| e.seq == top).collect();
    let [tip] = tips.as_slice() else { return Err(NoMove::TipForked) };
    if tip.closes() && tip.author.eq_ignore_ascii_case(author) {
        return Ok(Move::Genesis { closing: tip.id_hex(), body: genesis_for(read, &tip.id_hex(), statement) });
    }
    Ok(Move::Close { seq: top + 1, prev: tip.id_hex(), body: closing_body(author, effective, statement.unwrap_or(CLOSING_KIND)) })
}

pub fn genesis_for(read: &Ledger, closing: &str, statement: Option<&str>) -> Value {
    let said = statement.map(str::to_string).or_else(|| {
        read.entries
            .iter()
            .find(|e| e.law == Law::One && e.seq == 0 && e.kind == Kind::Genesis)
            .and_then(|g| g.body.member("statement_md"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    genesis_body(&said.unwrap_or_default(), Some((&read.root, closing)))
}
