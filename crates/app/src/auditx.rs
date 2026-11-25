use crate::chainx::Endpoint;
use crate::fault::{Fault, Known};
use crate::home::Home;
use crate::key::Address;
use zikaron::json::Value;
use zikaron::v2::tokens::{Key, Label};
use zikaron_anchor::rpc;
use zikaron_glue::book;
use zikaron_anchor::scan::{self, Scanned};

pub fn complete() -> &'static str {
    Label::Complete.as_str()
}

pub fn whole(label: &str, asked: usize, unanswered: &[String]) -> bool {
    zikaron_send::keep::whole(label, asked, unanswered.len())
}

pub struct Verdict {
    pub label: String,
    pub complete: bool,
    pub report: Value,
    pub book: Option<book::Ledger>,
    pub entries: usize,
    pub unanswered: Vec<String>,
    pub unproven: Vec<zikaron_send::Why>,
    pub asked: usize,
    pub single_source: bool,
    pub fragment: Value,
    pub root: String,
}

impl Verdict {
    pub fn tag(&self) -> Option<Label> {
        Label::parse(&self.label)
    }

    pub fn broken(&self) -> bool {
        self.tag() == Some(Label::BrokenChain)
    }

    pub fn lacks_anchored(&self) -> bool {
        !rows_of(&self.report, Key::Missing).is_empty()
    }
}

pub const ITEMS: [Key; 16] = [
    Key::Root,
    Key::Basis,
    Key::Entries,
    Key::Findings,
    Key::Missing,
    Key::Anchored,
    Key::Unanchored,
    Key::Excluded,
    Key::Retired,
    Key::UnknownType,
    Key::Malformed,
    Key::Unproven,
    Key::Void,
    Key::Discarded,
    Key::Continues,
    Key::LabelKey,
];

pub struct Item {
    pub key: &'static str,
    pub count: Option<usize>,
    pub said: String,
}

pub fn items(report: &Value) -> Vec<Item> {
    ITEMS
        .into_iter()
        .map(|k| {
            let name = k.as_str();
            match member(report, name) {
                Some(Value::Arr(a)) => Item { key: name, count: Some(a.len()), said: String::new() },
                Some(Value::Str(s)) => Item { key: name, count: None, said: s.clone() },
                Some(Value::Int(n)) => Item { key: name, count: None, said: n.to_string() },
                Some(Value::Obj(m)) => Item { key: name, count: Some(m.len()), said: String::new() },
                Some(other) => Item { key: name, count: None, said: format!("{other:?}") },
                None => Item { key: name, count: None, said: String::new() },
            }
        })
        .collect()
}

pub fn rows_of(report: &Value, k: Key) -> Vec<Value> {
    match member(report, k.as_str()) {
        Some(Value::Arr(a)) => a.clone(),
        _ => Vec::new(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unknown {
    pub convention: usize,
    pub faces: usize,
    pub left: usize,
}

pub fn unknown_of(report: &Value, rows: &[crate::ledgerx::Row]) -> Unknown {
    let count = items(report).iter().find(|i| i.key == Key::UnknownType.as_str()).and_then(|i| i.count).unwrap_or(0);
    let convention = crate::retractx::old_retractions(rows);
    let faces = rows.iter().filter(|r| r.law == zikaron_glue::book::Law::Two && r.kind == zikaron_glue::book::Kind::Face).count();
    Unknown { convention, faces, left: count.saturating_sub(convention + faces) }
}

pub fn entry_ids(report: &Value, k: Key) -> std::collections::BTreeSet<String> {
    rows_of(report, k)
        .iter()
        .filter_map(|r| match r {
            Value::Str(id) => Some(id.to_ascii_lowercase()),
            other => member(other, "entry_id").and_then(|v| v.as_str()).map(str::to_ascii_lowercase),
        })
        .collect()
}

pub fn held_on(report: &Value, chain: u64, id: &str, by: &str) -> bool {
    if anchored_on(report, chain).contains(&id.to_ascii_lowercase()) {
        return true;
    }
    let text = |row: &Value, k: &str| member(row, k).and_then(|v| v.as_str()).map(str::to_ascii_lowercase);
    matches!(member(report, "discarded"), Some(Value::Arr(rows)) if rows.iter().any(|r| {
        (chain == 0 || matches!(member(r, "chainId"), Some(Value::Int(c)) if *c == chain))
            && text(r, "hash").as_deref() == Some(id.to_ascii_lowercase().as_str())
            && text(r, "sender").as_deref() == Some(by.to_ascii_lowercase().as_str())
    }))
}

pub fn anchored_on(report: &Value, chain: u64) -> std::collections::BTreeSet<String> {
    book::anchored_on(report, chain)
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

fn empty_basis() -> Value {
    Value::Obj(vec![("chains".to_string(), Value::Arr(Vec::new()))])
}

fn no_root(n: book::NoRoot) -> Fault {
    match n {
        book::NoRoot::Empty => Fault::known(Known::NoGenesis, "这本账里没有创世条目".to_string()),
        book::NoRoot::Forked(n) => Fault::known(Known::ForkedRoot, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail079, &[&(n).to_string()]))),
    }
}

fn unread(u: book::Unread) -> Fault {
    match u {
        book::Unread::Root(n) => no_root(n),
        book::Unread::Fragment => Fault::known(Known::AuditInput, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail080).to_string())),
        book::Unread::Domain(t) => Fault::known(Known::AuditInput, crate::lang::both_of(|| format!("{} · {t}", crate::lang::t(crate::lang::Key::Tail081)))),
        book::Unread::NoLabel => Fault::known(Known::AuditInput, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail081).to_string())),
    }
}

pub fn root_of(items: &[Vec<u8>]) -> Result<String, Fault> {
    book::root_of(items).map_err(no_root)
}

pub fn ledger_of(items: &[Vec<u8>], want: &str, author: &str) -> Option<Vec<Vec<u8>>> {
    book::ledger_of(items, want, author)
}

pub fn offline(home: &Home) -> Result<Verdict, Fault> {
    let ledger = home.ledger()?;
    let pile = ledger
        .pile()?;
    ask(&pile.items, &Scanned { anchors: Vec::new(), basis: empty_basis(), candidates: Vec::new() })
}

pub fn offline_items(items: &[Vec<u8>]) -> Result<Verdict, Fault> {
    ask(items, &Scanned { anchors: Vec::new(), basis: empty_basis(), candidates: Vec::new() })
}

pub fn ask(items: &[Vec<u8>], scanned: &Scanned) -> Result<Verdict, Fault> {
    ask_from(items, &scan::fragment(scanned), Vec::new(), 0, false)
}

pub fn ask_from(
    items: &[Vec<u8>],
    fragment: &Value,
    unanswered: Vec<String>,
    asked: usize,
    single_source: bool,
) -> Result<Verdict, Fault> {
    let read = book::read(items, fragment).map_err(unread)?;
    let label = read.label.as_str().to_string();
    Ok(Verdict {
        complete: label == complete(),
        label,
        report: read.merged(),
        entries: items.len(),
        root: read.root.clone(),
        book: Some(read),
        unanswered,
        unproven: Vec::new(),
        asked,
        single_source,
        fragment: fragment.clone(),
    })
}

pub fn input_of(items: &[Vec<u8>], fragment: &Value) -> Result<Value, Fault> {
    let root = root_of(items)?;
    book::input_of(&root, items, fragment).map_err(unread)
}

pub fn input_for(grant: &[u8], items: &[Vec<u8>], fragment: &Value, stated: Option<&str>, notices: Option<&Value>) -> Result<Value, Fault> {
    let root = match stated {
        Some(r) => r.to_ascii_lowercase(),
        None => zikaron_glue::notices::root_for(grant, notices).ok_or_else(|| Fault::known(Known::NotAGrant, String::new()))?,
    };
    book::input_of(&root, items, fragment).map_err(unread)
}

pub fn read(items: &[Vec<u8>], fragment: &Value) -> Result<book::Ledger, Fault> {
    book::read(items, fragment).map_err(unread)
}

pub use zikaron_glue::recordcheck::FirstAnchor;

pub fn first_anchored(items: &[Vec<u8>], fragment: &Value) -> Option<Vec<(String, u64)>> {
    let read = read(items, fragment).ok()?;
    Some(read.first.iter().map(|(id, f)| (id.clone(), f.block_timestamp)).collect())
}

pub fn first_anchors(items: &[Vec<u8>], fragment: &Value) -> Option<Vec<(String, FirstAnchor)>> {
    let read = read(items, fragment).ok()?;
    Some(read.first.into_iter().collect())
}

pub fn name_registry(first: &mut FirstAnchor, emitters: &zikaron_anchor::scan::Emitters) {
    let h32 = |s: &str| -> Option<[u8; 32]> { zikaron::hexfmt::decode(s)?.try_into().ok() };
    let (Some(tx), Some(hash)) = (h32(&first.tx), h32(&first.hash)) else { return };
    if let Some(set) = emitters.get(&(first.chain_id, first.block_number, tx, hash)) {
        first.registry = set.iter().next().map(|a| zikaron::hexfmt::encode(a));
    }
}

pub fn revocation_contract(chain: u64, at: &str) -> bool {
    zikaron_anchor::revoke::at(chain).is_some_and(|r| zikaron::hexfmt::decode(at).is_some_and(|a| a.as_slice() == r.as_slice()))
}

pub fn empty_fragment() -> Value {
    scan::fragment(&Scanned { anchors: Vec::new(), basis: empty_basis(), candidates: Vec::new() })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ground {
    pub chain: u64,
    pub registry: Address,
    pub from_block: u64,
    pub to_block: u64,
    pub senders: Vec<String>,
    pub earlier: Vec<Window>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub chain: u64,
    pub registry: Address,
    pub from_block: u64,
    pub to_block: u64,
}

impl Ground {
    pub fn only(&self, w: Option<&Window>) -> Ground {
        match w {
            None => Ground { earlier: Vec::new(), ..self.clone() },
            Some(w) => Ground { chain: w.chain, registry: w.registry, from_block: w.from_block, to_block: w.to_block, senders: self.senders.clone(), earlier: Vec::new() },
        }
    }
}

pub fn senders_of(items: &[Vec<u8>]) -> Vec<String> {
    zikaron_glue::basis::senders_of(items)
}

pub fn basis_of(g: &Ground) -> Value {
    let registries = zikaron_glue::notices::registries_of(g.chain, &g.registry.hex());
    zikaron_glue::basis::basis_of(&zikaron_glue::basis::Window { chain: g.chain, registries: &registries, from_block: g.from_block, to_block: g.to_block, senders: &g.senders })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facts {
    Local,
    Bare,
}

impl Facts {
    fn known_single(self) -> scan::Known {
        match self {
            Facts::Local => crate::checkedx::read_single(),
            Facts::Bare => scan::Known::default(),
        }
    }
}

pub struct Scanned1 {
    pub anchors: usize,
    pub asked: usize,
    pub fragment: zikaron::json::Value,
}

pub fn scan_once(eps: &[Endpoint], g: &Ground) -> Result<Scanned1, Fault> {
    scan_once_noting(eps, g).map(|(s, _)| s)
}

pub fn scan_once_noting(eps: &[Endpoint], g: &Ground) -> Result<(Scanned1, scan::Emitters), Fault> {
    scan_once_heard(eps, g).map(|(s, e, _)| (s, e)).map_err(Fault::from)
}

pub fn scan_once_heard(eps: &[Endpoint], g: &Ground) -> Result<(Scanned1, scan::Emitters, Vec<String>), Unread> {
    scan_table(eps, &zikaron::json::canon_bytes(&basis_of(g)))
}

pub fn scan_first(eps: &[Endpoint], basis_bytes: &[u8]) -> Result<(Scanned1, scan::Emitters), Fault> {
    scan_table(eps, basis_bytes).map(|(s, e, _)| (s, e)).map_err(Fault::from)
}

pub struct Unread {
    pub fault: Fault,
    pub unanswered: Vec<String>,
}

impl From<Unread> for Fault {
    fn from(u: Unread) -> Fault {
        u.fault
    }
}

pub fn scan_table(eps: &[Endpoint], basis_bytes: &[u8]) -> Result<(Scanned1, scan::Emitters, Vec<String>), Unread> {
    scan_table_picked(eps, basis_bytes, &scan::Pick::All)
}

pub fn scan_table_picked(eps: &[Endpoint], basis_bytes: &[u8], pick: &scan::Pick) -> Result<(Scanned1, scan::Emitters, Vec<String>), Unread> {
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();
    type Read = (Scanned1, scan::Emitters);
    let round = |k: usize, asked: &mut usize| -> Option<(Vec<(u64, String)>, Result<Result<Read, ()>, scan::Refusal>)> {
        let (mut used, mut https): (Vec<(u64, String)>, Vec<(u64, Box<dyn rpc::Endpoint>)>) = (Vec::new(), Vec::new());
        for c in &chains {
            crate::chainx::speak();
            let Some(url) = zikaron_send::agree::nth_for(eps, *c, k) else { continue };
            if let Some(h) = zikaron_send::node::endpoint_at(&url) {
                used.push((*c, url));
                https.push((*c, h));
            }
        }
        if https.is_empty() {
            return None;
        }
        *asked += https.len();
        let mut handed: Vec<(u64, &mut dyn rpc::Endpoint)> = https.iter_mut().map(|(c, h)| (*c, &mut **h as &mut dyn rpc::Endpoint)).collect();
        let got = match scan::run_knowing_picked(basis_bytes, &mut handed, &Facts::Local.known_single(), pick) {
            Ok(Ok((s, emitters, seen))) => {
                let _ = crate::checkedx::add_single(&seen);
                Ok(Ok((Scanned1 { anchors: s.anchors.len() + s.candidates.len(), asked: 0, fragment: scan::fragment(&s) }, emitters)))
            }
            Ok(Err(_)) => Ok(Err(())),
            Err(r) => Err(r),
        };
        Some((used, got))
    };
    let no_label = || Fault::known(Known::AuditInput, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail083).to_string()));
    type Tried = (usize, Vec<(u64, String)>, Result<Result<Read, ()>, scan::Refusal>);
    let words = |tried: &[Tried]| tried.iter().filter_map(|(_, used, r)| r.as_ref().err().map(|r| refused_words(used, r))).collect::<Vec<String>>();
    let mut tried: Vec<Tried> = Vec::new();
    let mut asked = 0usize;
    for k in 0..zikaron_send::agree::rounds(eps) {
        match round(k, &mut asked) {
            None => {}
            Some((_, Ok(Ok((mut s, emitters))))) => {
                s.asked = asked;
                return Ok((s, emitters, words(&tried)));
            }
            Some((_, Ok(Err(())))) => return Err(Unread { fault: no_label(), unanswered: words(&tried) }),
            Some((used, Err(r))) => tried.push((k, used, Err(r))),
        }
    }
    if tried.is_empty() {
        let fault = Fault::known(Known::NoEndpoint, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail082).to_string()));
        return Err(Unread { fault, unanswered: Vec::new() });
    }
    zikaron_anchor::action::cut_and_ask(
        &mut tried,
        |_, (_, used, _)| used.iter().map(|(_, u)| zikaron_anchor::endpoints::place_of(u)).collect(),
        |(_, _, r)| r.is_ok(),
        |(_, _, r)| r.as_ref().err().is_some_and(scan::Refusal::cut),
        |idx, tried| {
            let mut out = Vec::new();
            for i in idx {
                let k = tried[*i].0;
                let Some((used, got)) = round(k, &mut asked) else {
                    let kept = match &tried[*i].2 {
                        Err(r) => Err(r.clone()),
                        Ok(_) => Err(scan::Refusal::Unanswered { chain: 0, asked: scan::Asked::Short }),
                    };
                    out.push((k, tried[*i].1.clone(), kept));
                    continue;
                };
                let read = got.is_ok();
                out.push((k, used, got));
                if read {
                    break;
                }
            }
            out
        },
    );
    if let Some(i) = tried.iter().position(|(_, _, r)| r.is_ok()) {
        let before = words(&tried[..i]);
        return match tried.swap_remove(i).2 {
            Ok(Ok((mut s, emitters))) => {
                s.asked = asked;
                Ok((s, emitters, before))
            }
            _ => Err(Unread { fault: no_label(), unanswered: before }),
        };
    }
    let said = words(&tried);
    let refused: Vec<scan::Refusal> = tried.into_iter().filter_map(|(_, _, r)| r.err()).collect();
    Err(Unread { fault: Fault::scan_refused(said.join("; "), &refused), unanswered: said })
}

fn refused_words(used: &[(u64, String)], r: &scan::Refusal) -> String {
    let chain = match r {
        scan::Refusal::Unanswered { chain, .. } | scan::Refusal::NoEndpoint(chain) => Some(*chain),
        scan::Refusal::ChainIdMismatch { declared, .. } => Some(*declared),
        scan::Refusal::TxNotItself(_) | scan::Refusal::Malformed(_) => None,
    };
    let node = chain.and_then(|c| used.iter().find(|(x, _)| *x == c)).or(used.first()).map(|(_, u)| zikaron_net::sayable(u));
    match node {
        Some(n) => format!("{n} {}", Fault::scan_tail(r)),
        None => Fault::scan_tail(r),
    }
}

pub struct Agreed {
    pub fragment: Value,
    pub unanswered: Vec<String>,
    pub unproven: Vec<zikaron_send::Why>,
    pub asked: usize,
    pub single_source: bool,
}

pub enum Scan {
    Basis(Agreed),
    NoLabel { report: Value, unanswered: Vec<String>, asked: usize },
}

pub fn scan_agreed(eps: &[Endpoint], g: &Ground) -> Result<Scan, Fault> {
    scan_agreed_with(eps, g, Facts::Local)
}

pub fn scan_agreed_with(eps: &[Endpoint], g: &Ground, facts: Facts) -> Result<Scan, Fault> {
    scan_agreed_bytes_with(eps, &zikaron::json::canon_bytes(&basis_of(g)), facts)
}

pub fn scan_agreed_bytes(eps: &[Endpoint], basis_bytes: &[u8]) -> Result<Scan, Fault> {
    scan_agreed_bytes_with(eps, basis_bytes, Facts::Local)
}

pub fn scan_agreed_bytes_with(eps: &[Endpoint], basis_bytes: &[u8], facts: Facts) -> Result<Scan, Fault> {
    scan_agreed_bytes_picked(eps, basis_bytes, facts, &zikaron_anchor::scan::Pick::All)
}

pub fn scan_agreed_picked(eps: &[Endpoint], g: &Ground, facts: Facts, pick: &zikaron_anchor::scan::Pick) -> Result<Scan, Fault> {
    scan_agreed_bytes_picked(eps, &zikaron::json::canon_bytes(&basis_of(g)), facts, pick)
}

pub fn scan_agreed_bytes_picked(eps: &[Endpoint], basis_bytes: &[u8], facts: Facts, pick: &zikaron_anchor::scan::Pick) -> Result<Scan, Fault> {
    crate::chainx::speak();
    let got = match facts {
        Facts::Local => zikaron_send::agree::scan_picked(eps, basis_bytes, &crate::sendx::Local, pick),
        Facts::Bare => zikaron_send::agree::scan_picked(eps, basis_bytes, &zikaron_send::Bare, pick),
    }?;
    let said = |w: &[zikaron_send::Why]| w.iter().map(crate::chainx::said_of).collect::<Vec<String>>();
    Ok(match got {
        zikaron_send::agree::Scan::Basis(a) => Scan::Basis(Agreed { fragment: a.fragment, unanswered: said(&a.unanswered), unproven: a.unproven, asked: a.asked, single_source: a.single_source }),
        zikaron_send::agree::Scan::NoLabel { report, unanswered, asked } => Scan::NoLabel { report, unanswered: said(&unanswered), asked },
    })
}

pub fn scan_basis(eps: &[Endpoint], g: &Ground) -> Result<Scan, Fault> {
    if g.earlier.is_empty() {
        return scan_agreed(eps, g);
    }
    let windows: Vec<Ground> = std::iter::once(g.only(None)).chain(g.earlier.iter().map(|w| g.only(Some(w)))).collect();
    let (mut read, mut chains, mut unanswered, mut unproven) = (Vec::new(), Vec::new(), Vec::new(), Vec::<zikaron_send::Why>::new());
    let (mut asked, mut single_source, mut first) = (0usize, false, None);
    for w in &windows {
        let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == w.chain).cloned().collect();
        match scan_agreed(&mine, w) {
            Ok(Scan::Basis(a)) => {
                read.push(a.fragment.clone());
                if let Some(Value::Arr(xs)) = member(&a.fragment, "basis").and_then(|b| member(b, "chains")) {
                    chains.extend(xs.iter().cloned());
                }
                asked += a.asked;
                single_source |= a.single_source;
                unanswered.extend(a.unanswered);
                for u in a.unproven {
                    if !unproven.contains(&u) {
                        unproven.push(u);
                    }
                }
            }
            Ok(nolabel @ Scan::NoLabel { .. }) => return Ok(nolabel),
            Err(f) => {
                unanswered.push(format!("chainId={} {}", w.chain, f.evidence()));
                first.get_or_insert(f);
            }
        }
    }
    if chains.is_empty() {
        return Err(first.unwrap_or_else(|| Fault::known(Known::NoEndpoint, String::new())));
    }
    let fragment = zikaron_anchor::scan::joined(&read, Value::Obj(vec![("chains".to_string(), Value::Arr(chains))]));
    Ok(Scan::Basis(Agreed { fragment, unanswered, unproven, asked, single_source }))
}

pub fn online(home: &Home, eps: &[Endpoint], g: &Ground) -> Result<Verdict, Fault> {
    crate::trace::mark(crate::feature::Feature::W2);
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail021).to_string())));
    }
    let ledger = home.ledger()?;
    let pile = ledger
        .pile()?;
    match scan_basis(eps, g)? {
        Scan::NoLabel { report, unanswered, asked } => Ok(Verdict {
            label: String::new(),
            complete: false,
            report,
            entries: pile.items.len(),
            unanswered,
            unproven: Vec::new(),
            asked,
            single_source: true,
            fragment: empty_fragment(),
            root: root_of(&pile.items).unwrap_or_default(),
            book: None,
        }),
        Scan::Basis(a) => {
            let mut v = ask_from(&pile.items, &a.fragment, a.unanswered, a.asked, a.single_source)?;
            v.unproven = a.unproven;
            Ok(v)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pen {
    Granted,
    Held,
}

impl Pen {
    pub fn as_str(self) -> &'static str {
        match self {
            Pen::Granted => "granted",
            Pen::Held => "held",
        }
    }

    pub fn writable(self) -> bool {
        matches!(self, Pen::Granted)
    }
}

#[cfg(test)]
mod contract_name_tests {
    #[test]
    fn a_first_anchors_contract_is_named_by_what_it_is() {
        let sepolia = zikaron::hexfmt::encode(&zikaron_anchor::revoke::at(11_155_111).expect("a row"));
        assert!(super::revocation_contract(11_155_111, &sepolia));
        assert!(super::revocation_contract(11_155_111, &sepolia.to_ascii_uppercase().replacen("0X", "0x", 1)));
        assert!(!super::revocation_contract(11_155_111, &format!("0x{}", "11".repeat(20))), "a registry");
        assert!(!super::revocation_contract(31_337, &sepolia), "a chain with no revocation contract");
        assert!(!super::revocation_contract(11_155_111, "0x12"), "out of form");
    }
}

#[cfg(test)]
mod held_on_tests {
    #[test]
    fn a_retired_keys_own_record_is_on_chain() {
        let report = zikaron::json::parse(br#"{"anchored":[{"anchors":[{"chainId":7}],"entry_id":"0xab"}],"discarded":[{"chainId":9,"hash":"0xab","sender":"0x01"},{"chainId":8,"hash":"0xab","sender":"0x02"}]}"#).ok().expect("json");
        assert!(super::held_on(&report, 7, "0xAB", "0x01"), "in the report");
        assert!(super::held_on(&report, 9, "0xab", "0x01"), "set aside, its signer's");
        assert!(!super::held_on(&report, 8, "0xab", "0x01"), "set aside, another sender's");
        assert!(!super::held_on(&report, 10, "0xab", "0x01"), "not on that chain");
        assert!(!super::held_on(&report, 9, "0xcd", "0x01"), "another entry");
    }
}
