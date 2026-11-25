use crate::wire::W;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use zikaron::json::{self, Value};

pub const MARGIN: u64 = 64;

pub const FORM: &str = "zikaron.settled-logs/2";

pub const NOTES: &str = "notes";

pub const ANSWERED_WIDTHS: &str = "answeredWidths";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unkept {
    Locked,
    KeyChanged,
    Said(String),
}

impl From<String> for Unkept {
    fn from(why: String) -> Unkept {
        Unkept::Said(why)
    }
}

pub trait Keep: Send + Sync {
    fn load(&self, part: &str) -> Option<Vec<u8>>;
    fn save(&self, part: &str, plain: &[u8]) -> Result<(), Unkept>;
    fn parts(&self) -> Vec<String> {
        Vec::new()
    }
    fn hold(&self) -> Option<Box<dyn std::any::Any>> {
        None
    }
    fn public(&self) -> bool {
        false
    }
}

static TURN: Mutex<()> = Mutex::new(());

static UNKEPT: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn save_said(k: &dyn Keep, part: &str, plain: &[u8]) {
    if let Err(Unkept::Said(why)) = k.save(part, plain) {
        let mut held = UNKEPT.lock().unwrap_or_else(|e| e.into_inner());
        match held.iter_mut().find(|(p, _)| p == part) {
            Some(one) => one.1 = why,
            None => held.push((part.to_string(), why)),
        }
    }
}

pub fn unkept() -> Vec<String> {
    std::mem::take(&mut *UNKEPT.lock().unwrap_or_else(|e| e.into_inner())).into_iter().map(|(part, why)| format!("{part}: {why}")).collect()
}

pub fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn keeper() -> Option<Arc<dyn Keep>> {
    crate::action::keeper()
}

fn judged_keeper() -> Option<Arc<dyn Keep>> {
    crate::action::judged_keeper()
}

pub fn keeps() -> bool {
    keeper().is_some()
}

pub fn spell(place: &str) -> String {
    let public = keeper().is_some_and(|k| k.public()) || judged_keeper().is_some_and(|k| k.public());
    if !public {
        return place.to_string();
    }
    let d = zikaron::cryptox::sha256(place.as_bytes());
    format!("sha256:{}", zikaron::hexfmt::encode(&d[..16]).trim_start_matches("0x"))
}

pub fn spell_nodes(nodes: &str) -> String {
    nodes.split(' ').filter(|p| !p.is_empty()).map(spell).collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub chain: u64,
    pub nodes: String,
    pub filter: String,
}

pub fn nodes_of<'a>(places: impl IntoIterator<Item = &'a str>) -> String {
    let mut p: Vec<&str> = places.into_iter().collect();
    p.sort_unstable();
    p.dedup();
    p.join(" ")
}

pub fn places_in(nodes: &str) -> usize {
    nodes.split(' ').filter(|p| !p.is_empty()).count()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub from: u64,
    pub settled: u64,
    pub logs: Vec<W>,
    pub single_source: bool,
}

pub fn block_of(log: &W) -> Option<u64> {
    log.member("blockNumber").and_then(crate::tx::hex_qty)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lack {
    pub said: String,
    pub at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Width {
    pub blocks: u64,
    pub at: u64,
}

impl Width {
    pub const FOR: u64 = 24 * 60 * 60;

    pub fn stands(&self, now: u64) -> bool {
        now.checked_sub(self.at).is_some_and(|age| age <= Width::FOR)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    pub lacks: BTreeMap<(String, String), Lack>,
    pub widths: BTreeMap<String, Width>,
    pub carried: BTreeMap<(u64, [u8; 32]), Option<Vec<u8>>>,
}

impl Notes {
    pub fn render(&self) -> Vec<u8> {
        let lacks: Vec<Value> = self
            .lacks
            .iter()
            .map(|((n, m), l)| Value::Obj(vec![("at".into(), Value::Int(l.at)), ("method".into(), Value::Str(m.clone())), ("node".into(), Value::Str(n.clone())), ("said".into(), Value::Str(l.said.clone()))]))
            .collect();
        let mut members = vec![("form".into(), Value::Str(FORM.into())), ("lacks".into(), Value::Arr(lacks))];
        if !self.carried.is_empty() {
            let carried = self
                .carried
                .iter()
                .map(|((chain, tx), entry)| {
                    Value::Obj(vec![
                        ("chainId".into(), Value::Int(*chain)),
                        ("entry".into(), entry.as_ref().map_or(Value::Null, |b| Value::Str(zikaron::hexfmt::encode(b)))),
                        ("tx".into(), Value::Str(zikaron::hexfmt::encode(tx))),
                    ])
                })
                .collect();
            members.insert(0, ("carried".into(), Value::Arr(carried)));
        }
        if !self.widths.is_empty() {
            let widths = self
                .widths
                .iter()
                .map(|(n, w)| Value::Obj(vec![("at".into(), Value::Int(w.at)), ("blocks".into(), Value::Int(w.blocks)), ("node".into(), Value::Str(n.clone()))]))
                .collect();
            members.push((ANSWERED_WIDTHS.into(), Value::Arr(widths)));
        }
        json::canon_bytes(&Value::Obj(members))
    }

    pub fn parse(b: &[u8]) -> Option<Notes> {
        let v = json::parse(b).ok()?;
        if v.member("form").and_then(Value::as_str) != Some(FORM) {
            return None;
        }
        let mut out = Notes::default();
        for l in v.member("lacks").and_then(|x| x.as_arr()).map_or(&[][..], |v| v.as_slice()) {
            let (Some(n), Some(m)) = (l.member("node").and_then(Value::as_str), l.member("method").and_then(Value::as_str)) else { continue };
            let key = (n.to_string(), m.to_string());
            if let (Some(said), Some(at)) = (l.member("said").and_then(Value::as_str), int(l.member("at"))) {
                if crate::patience::lacks_the_method(m, &crate::rpc::Trouble::Node(said.to_string())) {
                    out.lacks.insert(key, Lack { said: said.to_string(), at });
                }
            }
        }
        for w in v.member(ANSWERED_WIDTHS).and_then(|x| x.as_arr()).map_or(&[][..], |v| v.as_slice()) {
            if let (Some(n), Some(blocks), Some(at)) = (w.member("node").and_then(Value::as_str), int(w.member("blocks")), int(w.member("at"))) {
                if blocks > 0 {
                    out.widths.insert(n.to_string(), Width { blocks, at });
                }
            }
        }
        for c in v.member("carried").and_then(|x| x.as_arr()).map_or(&[][..], |v| v.as_slice()) {
            let tx: Option<[u8; 32]> = c.member("tx").and_then(Value::as_str).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok());
            let entry = match c.member("entry") {
                Some(Value::Null) => Some(None),
                Some(Value::Str(h)) => zikaron::hexfmt::decode(h).map(Some),
                _ => None,
            };
            if let (Some(chain), Some(tx), Some(entry)) = (int(c.member("chainId")), tx, entry) {
                out.carried.insert((chain, tx), entry);
            }
        }
        Some(out)
    }
}

fn int(x: Option<&Value>) -> Option<u64> {
    match x {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

pub fn part_of(key: &Key) -> String {
    let named = json::canon_bytes(&Value::Obj(vec![("chainId".into(), Value::Int(key.chain)), ("filter".into(), Value::Str(key.filter.clone())), ("nodes".into(), Value::Str(key.nodes.clone()))]));
    let d = zikaron::cryptox::sha256(&named);
    zikaron::hexfmt::encode(&d[..16]).trim_start_matches("0x").to_string()
}

pub fn render_entry(key: &Key, e: &Entry) -> Vec<u8> {
    json::canon_bytes(&Value::Obj(vec![
        ("chainId".into(), Value::Int(key.chain)),
        ("filter".into(), Value::Str(key.filter.clone())),
        ("form".into(), Value::Str(FORM.into())),
        ("from".into(), Value::Int(e.from)),
        ("logs".into(), Value::Arr(e.logs.iter().map(|l| Value::Str(crate::wire::write(l))).collect())),
        ("nodes".into(), Value::Str(key.nodes.clone())),
        ("settled".into(), Value::Int(e.settled)),
        ("singleSource".into(), Value::Bool(e.single_source)),
    ]))
}

pub fn parse_entry(b: &[u8]) -> Option<(Key, Entry)> {
    let e = json::parse(b).ok()?;
    if e.member("form").and_then(Value::as_str) != Some(FORM) {
        return None;
    }
    let key = Key { chain: int(e.member("chainId"))?, nodes: e.member("nodes")?.as_str()?.to_string(), filter: e.member("filter")?.as_str()?.to_string() };
    let (from, settled) = (int(e.member("from"))?, int(e.member("settled"))?);
    let single_source = match e.member("singleSource") {
        Some(Value::Bool(b)) => *b,
        _ => return None,
    };
    let logs: Vec<W> = e.member("logs")?.as_arr()?.iter().map(|l| crate::wire::parse(l.as_str()?.as_bytes())).collect::<Option<_>>()?;
    if settled < from || logs.iter().any(|l| !block_of(l).is_some_and(|b| (from..=settled).contains(&b))) {
        return None;
    }
    Some((key, Entry { from, settled, logs, single_source }))
}

fn load_entry(k: &dyn Keep, key: &Key) -> Option<Entry> {
    k.load(&part_of(key)).and_then(|b| parse_entry(&b)).filter(|(named, _)| named == key).map(|(_, e)| e)
}

fn load_notes(k: &dyn Keep) -> Notes {
    k.load(NOTES).and_then(|b| Notes::parse(&b)).unwrap_or_default()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Book {
    pub entries: BTreeMap<Key, Entry>,
    pub notes: Notes,
}

pub fn read_whole(k: &dyn Keep) -> Book {
    let mut b = Book { entries: BTreeMap::new(), notes: load_notes(k) };
    for part in k.parts() {
        if part == NOTES {
            continue;
        }
        if let Some((key, e)) = k.load(&part).and_then(|x| parse_entry(&x)).filter(|(key, _)| part_of(key) == part) {
            b.entries.insert(key, e);
        }
    }
    b
}

pub fn book() -> Option<Book> {
    keeper().map(|k| read_whole(&*k))
}

pub fn entry(key: &Key) -> Option<Entry> {
    let k = keeper()?;
    crate::action::entry(key, || load_entry(&*k, key))
}

pub(crate) fn judged_entry(key: &Key) -> Option<Entry> {
    let k = judged_keeper()?;
    crate::action::entry(key, || load_entry(&*k, key))
}

pub fn notes() -> Option<Notes> {
    let k = keeper()?;
    Some(crate::action::notes(|| load_notes(&*k)))
}

fn change_notes(f: impl FnOnce(&mut Notes)) {
    let Some(k) = keeper() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let _held = k.hold();
    let mut n = load_notes(&*k);
    let was = n.clone();
    f(&mut n);
    if n != was {
        save_said(&*k, NOTES, &n.render());
    }
    crate::action::notes_now(&n);
}

pub fn carried_kept(chain: u64, tx: &[u8; 32]) -> Option<Option<Vec<u8>>> {
    notes()?.carried.get(&(chain, *tx)).cloned()
}

pub fn keep_carried(found: &[((u64, [u8; 32]), Option<Vec<u8>>)]) {
    if crate::endpoints::stopped() {
        return;
    }
    if found.is_empty() {
        return;
    }
    change_notes(|n| {
        for (k, e) in found {
            n.carried.insert(*k, e.clone());
        }
    });
}

pub fn keep(key: &Key, e: Entry) {
    if crate::endpoints::stopped() {
        return;
    }
    keep_on(key, e, None);
}

pub fn keep_on(key: &Key, e: Entry, base: Option<(u64, u64)>) {
    if crate::endpoints::stopped() {
        return;
    }
    keep_on_in(keeper(), key, e, base)
}

fn keep_on_in(k: Option<Arc<dyn Keep>>, key: &Key, e: Entry, base: Option<(u64, u64)>) {
    let Some(k) = k else { return };
    if let Some(p) = crate::action::pend(Pending { keeper: k, key: key.clone(), entry: e, base }) {
        flush(vec![p]);
    }
}

pub(crate) fn carried_on(kept: Option<&Entry>, e: &Entry, base: Option<(u64, u64)>) -> Option<Entry> {
    if let Some((from, settled)) = base {
        if !kept.is_some_and(|h| h.from == from && h.settled >= settled) {
            return None;
        }
    }
    match kept {
        Some(h) if h.from <= e.from && h.settled >= e.settled => None,
        _ => Some(e.clone()),
    }
}

pub struct Pending {
    pub(crate) keeper: Arc<dyn Keep>,
    pub(crate) key: Key,
    pub(crate) entry: Entry,
    pub(crate) base: Option<(u64, u64)>,
}

pub(crate) fn flush(pending: Vec<Pending>) {
    if pending.is_empty() {
        return;
    }
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut keepers: Vec<Arc<dyn Keep>> = Vec::new();
    for p in &pending {
        if !keepers.iter().any(|k| Arc::ptr_eq(k, &p.keeper)) {
            keepers.push(p.keeper.clone());
        }
    }
    for k in keepers {
        let _held = k.hold();
        let mine: Vec<&Pending> = pending.iter().filter(|p| Arc::ptr_eq(&p.keeper, &k)).collect();
        let mut saved: Vec<&Key> = Vec::new();
        let mut keys: Vec<&Key> = Vec::new();
        for p in &mine {
            if !keys.contains(&&p.key) {
                keys.push(&p.key);
            }
        }
        for key in keys {
            let mut kept = load_entry(&*k, key);
            let fresh = kept.is_none();
            let mut changed = false;
            for p in mine.iter().filter(|p| &p.key == key) {
                if let Some(e) = carried_on(kept.as_ref(), &p.entry, p.base) {
                    kept = Some(e);
                    changed = true;
                }
            }
            if let (true, Some(e)) = (changed, &kept) {
                save_said(&*k, &part_of(key), &render_entry(key, e));
                if fresh {
                    saved.push(key);
                }
            }
        }
        for key in saved.drain(..) {
            let at = positions_of(key);
            let mut lists = load_positions(&*k, &at);
            if lists.insert(key.nodes.clone()) {
                save_said(&*k, &part_of(&at), &render_positions(&at, &lists));
            }
        }
    }
}

const POSITIONS: &str = "*";

fn positions_of(key: &Key) -> Key {
    Key { chain: key.chain, nodes: POSITIONS.to_string(), filter: key.filter.clone() }
}

fn load_positions(k: &dyn Keep, at: &Key) -> std::collections::BTreeSet<String> {
    let Some(v) = k.load(&part_of(at)).and_then(|b| json::parse(&b).ok()) else { return Default::default() };
    let named = v.member("form").and_then(Value::as_str) == Some(FORM) && int(v.member("chainId")) == Some(at.chain) && v.member("filter").and_then(Value::as_str) == Some(at.filter.as_str());
    if !named {
        return Default::default();
    }
    v.member("positions").and_then(Value::as_arr).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

fn render_positions(at: &Key, lists: &std::collections::BTreeSet<String>) -> Vec<u8> {
    json::canon_bytes(&Value::Obj(vec![
        ("chainId".into(), Value::Int(at.chain)),
        ("filter".into(), Value::Str(at.filter.clone())),
        ("form".into(), Value::Str(FORM.into())),
        ("positions".into(), Value::Arr(lists.iter().map(|l| Value::Str(l.clone())).collect())),
    ]))
}

pub fn lent(key: &Key, from: u64) -> Option<Entry> {
    let k = keeper()?;
    let lists = load_positions(&*k, &positions_of(key));
    lists
        .into_iter()
        .filter(|l| *l != key.nodes)
        .filter_map(|nodes| load_entry(&*k, &Key { chain: key.chain, nodes, filter: key.filter.clone() }))
        .filter(|e| e.from <= from)
        .max_by_key(|e| e.settled)
}

pub fn keep_lack(place: &str, method: &str, lack: Lack, since: u64) {
    change_notes(|n| {
        n.lacks.retain(|_, l| l.at >= since);
        n.lacks.insert((place.to_string(), method.to_string()), lack);
    });
}

pub fn keep_width(place: &str, w: Width, now: u64) {
    if crate::endpoints::stopped() {
        return;
    }
    change_notes(|n| {
        n.widths.retain(|_, x| x.stands(now));
        n.widths.insert(place.to_string(), w);
    });
}

#[derive(Clone, Debug)]
pub struct Settling {
    pub key: Key,
    pub entry: Option<Entry>,
    pub served_single: bool,
    pub base: Option<(u64, u64)>,
}

pub fn served_single(runs: &[Vec<Settling>]) -> Vec<u64> {
    let mut out: Vec<u64> = runs.iter().flatten().filter(|s| s.served_single).map(|s| s.key.chain).collect();
    out.sort_unstable();
    out.dedup();
    out
}

pub(crate) const LOG_FACTS: [&str; 8] = ["address", "blockHash", "blockNumber", "data", "logIndex", "topics", "transactionHash", "transactionIndex"];

pub(crate) fn same_logs(a: &[W], b: &[W]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| crate::wire::write(&crate::wire::project(x, &LOG_FACTS)) == crate::wire::write(&crate::wire::project(y, &LOG_FACTS)))
}

pub fn settle(runs: &[Vec<Settling>], held_back: &[u64]) {
    if crate::endpoints::stopped() {
        return;
    }
    for (key, e, base) in decided(runs, held_back) {
        keep_on(&key, e, base);
    }
}

pub(crate) fn settle_judged(runs: &[Vec<Settling>]) {
    if crate::endpoints::stopped() {
        return;
    }
    for (key, e, base) in decided(runs, &[]) {
        crate::action::judged_keep(&key, e.clone());
        keep_on_in(judged_keeper(), &key, e, base);
    }
}

fn decided(runs: &[Vec<Settling>], held_back: &[u64]) -> Vec<(Key, Entry, Option<(u64, u64)>)> {
    let mut out = Vec::new();
    let mut keys: Vec<&Key> = runs.iter().flatten().filter(|s| s.entry.is_some()).map(|s| &s.key).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let settlings: Vec<&Settling> = runs.iter().filter_map(|r| r.iter().find(|s| &s.key == key)).filter(|s| s.entry.is_some()).collect();
        let said: Vec<&Entry> = settlings.iter().filter_map(|s| s.entry.as_ref()).collect();
        let Some(first) = said.first() else { continue };
        let base = settlings.first().and_then(|s| s.base);
        if places_in(&key.nodes) < 2 {
            out.push((key.clone(), Entry { single_source: true, ..(*first).clone() }, base));
            continue;
        }
        let read_it = runs.iter().filter(|r| r.iter().any(|s| &s.key == key)).count();
        let alike = said.len() == read_it && said.iter().all(|e| e.from == first.from && e.settled == first.settled && same_logs(&e.logs, &first.logs));
        if alike && !held_back.contains(&key.chain) {
            let single_source = read_it < 2 || settlings.iter().any(|s| s.served_single);
            out.push((key.clone(), Entry { single_source, ..(*first).clone() }, base));
        }
    }
    out
}

const JUDGED: &str = "judged";

pub(crate) fn judged_log(log: &W, record: Option<&Value>) -> W {
    let mut kept = crate::wire::project(log, &LOG_FACTS);
    let judged = match record {
        Some(r) => crate::wire::parse(&json::canon_bytes(r)).unwrap_or(W::of(crate::wire::Body::Null)),
        None => W::of(crate::wire::Body::Null),
    };
    if let crate::wire::Body::Obj(ms) = &mut kept.body {
        ms.push((JUDGED.to_string(), judged));
    }
    kept
}

pub(crate) fn judgement(kept: &W) -> Option<Option<Value>> {
    let j = kept.member(JUDGED)?;
    if j.is_null() {
        return Some(None);
    }
    crate::wire::to_core(j).filter(|v| matches!(v, Value::Obj(_))).map(Some)
}


#[cfg(test)]
mod tests {
    #[test]
    fn a_part_not_written_is_said_once_with_why() {
        struct Full;
        impl super::Keep for Full {
            fn load(&self, _part: &str) -> Option<Vec<u8>> {
                None
            }
            fn save(&self, _part: &str, _plain: &[u8]) -> Result<(), super::Unkept> {
                Err(super::Unkept::Said("no space left on device".into()))
            }
        }
        struct Fine;
        impl super::Keep for Fine {
            fn load(&self, _part: &str) -> Option<Vec<u8>> {
                None
            }
            fn save(&self, _part: &str, _plain: &[u8]) -> Result<(), super::Unkept> {
                Ok(())
            }
        }
        super::save_said(&Fine, "fine-part-641", b"x");
        assert!(!super::unkept().iter().any(|w| w.starts_with("fine-part-641")), "a part written leaves nothing");
        super::save_said(&Full, "full-part-641", b"x");
        super::save_said(&Full, "full-part-641", b"y");
        let said = super::unkept();
        assert!(said.iter().any(|w| w == "full-part-641: no space left on device"), "the part and why: {said:?}");
        assert_eq!(said.iter().filter(|w| w.starts_with("full-part-641")).count(), 1, "one part failed twice is said as one: {said:?}");
        assert!(!super::unkept().iter().any(|w| w.starts_with("full-part-641")), "said once");
    }

    #[test]
    fn a_part_not_written_for_the_store_locked_is_not_said() {
        struct Locked;
        impl super::Keep for Locked {
            fn load(&self, _part: &str) -> Option<Vec<u8>> {
                None
            }
            fn save(&self, _part: &str, _plain: &[u8]) -> Result<(), super::Unkept> {
                Err(super::Unkept::Locked)
            }
        }
        super::save_said(&Locked, "locked-part-649", b"x");
        assert!(!super::unkept().iter().any(|w| w.starts_with("locked-part-649")), "a lock's write is not said");
    }
    use super::*;

    fn log(block: u64) -> W {
        crate::wire::parse(format!("{{\"blockNumber\":\"0x{block:x}\",\"data\":\"0x\",\"logIndex\":\"0x0\"}}").as_bytes()).expect("a log")
    }

    #[derive(Default)]
    pub(crate) struct Held {
        pub(crate) parts: Mutex<BTreeMap<String, Vec<u8>>>,
        pub(crate) saves: Mutex<Vec<String>>,
        pub(crate) read_bytes: std::sync::atomic::AtomicUsize,
    }

    impl Keep for Held {
        fn load(&self, part: &str) -> Option<Vec<u8>> {
            let b = self.parts.lock().unwrap_or_else(|e| e.into_inner()).get(part).cloned();
            self.read_bytes.fetch_add(b.as_ref().map_or(0, Vec::len), std::sync::atomic::Ordering::SeqCst);
            b
        }
        fn save(&self, part: &str, plain: &[u8]) -> Result<(), super::Unkept> {
            self.saves.lock().unwrap_or_else(|e| e.into_inner()).push(part.to_string());
            self.parts.lock().unwrap_or_else(|e| e.into_inner()).insert(part.to_string(), plain.to_vec());
            Ok(())
        }
        fn parts(&self) -> Vec<String> {
            self.parts.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect()
        }
    }

    #[test]
    fn each_part_reads_back_what_it_wrote_and_a_width_is_no_lack() {
        let k = |n: &str| Key { chain: 1, nodes: n.into(), filter: "{}".into() };
        let good = Entry { from: 5, settled: 100, logs: vec![log(7), log(99)], single_source: false };
        assert_eq!(parse_entry(&render_entry(&k("a b"), &good)), Some((k("a b"), good.clone())));
        assert!(parse_entry(&render_entry(&k("c"), &Entry { from: 5, settled: 50, logs: vec![log(60)], single_source: true })).is_none(), "a log past the settled block: no entry");
        let other = String::from_utf8(render_entry(&k("a b"), &good)).unwrap().replace(FORM, "zikaron.settled-logs/1");
        assert!(parse_entry(other.as_bytes()).is_none(), "another form");
        assert_ne!(part_of(&k("a b")), part_of(&k("a c")));
        assert_eq!(part_of(&k("a b")).len(), 32);
        let held = Held::default();
        let _ = held.save(&part_of(&k("a c")), &render_entry(&k("a b"), &good));
        assert!(load_entry(&held, &k("a c")).is_none(), "a part naming another key is none for this one");

        let mut n = Notes::default();
        n.lacks.insert(("a".into(), "eth_getRawTransactionByHash".into()), Lack { said: "{\"code\":-32601}".into(), at: 7 });
        n.widths.insert("a".into(), Width { blocks: 15_000, at: 9 });
        let written = json::parse(&n.render()).expect("json");
        let listed = written.member("lacks").and_then(|x| x.as_arr()).expect("the list");
        assert_eq!(listed.len(), 1, "the width is not in the lacks' list");
        assert_eq!(Notes::parse(&n.render()), Some(n.clone()));
    }

    #[test]
    fn a_record_of_refused_widths_an_earlier_version_kept_reads_as_none_and_is_written_over_with_no_word() {
        let old = format!(r#"{{"form":"{FORM}","lacks":[],"refusedWidths":[{{"at":9,"blocks":30001,"node":"http://old-form.test","said":"range too large"}},{{"at":9,"from":0,"node":"http://old-form.test","said":"range too large","to":30000}}]}}"#);
        let keep = Arc::new(Held::default());
        keep.parts.lock().unwrap_or_else(|e| e.into_inner()).insert(NOTES.into(), old.into_bytes());
        let _a = crate::action::open(Some(keep.clone()));
        assert!(notes().is_some_and(|n| n.widths.is_empty()), "the earlier form is read as none");
        assert_eq!(crate::patience::answered_width("http://old-form.test"), None, "and starts no read narrower");
        crate::patience::keep_answered("http://old-form.test", 5_000);
        let saved = keep.parts.lock().unwrap_or_else(|e| e.into_inner()).get(NOTES).cloned().expect("written");
        let back = Notes::parse(&saved).expect("the notes read");
        assert_eq!(back.widths.get("http://old-form.test").map(|w| w.blocks), Some(5_000));
        assert!(!String::from_utf8_lossy(&saved).contains("refusedWidths"), "the earlier form is not carried");
        assert!(!unkept().iter().any(|w| w.starts_with(NOTES)), "reading and writing over it said nothing");
        crate::patience::forget_answered("http://old-form.test");
    }

    #[test]
    fn a_lack_kept_for_a_rate_limit_and_the_old_marks_are_let_go_as_they_are_read() {
        let old = br#"{"form":"FORM","lacks":[{"at":7,"method":"eth_getRawTransactionByHash","node":"a","said":"{\"code\":-32601,\"message\":\"the method does not exist\"}"},{"at":8,"method":"eth_getLogs","node":"a","said":"HTTP 429 too many requests"},{"at":8,"method":"eth_getProof","node":"b","said":"{\"code\":-32005,\"message\":\"rate limit exceeded\"}"},{"at":9,"by":"7-1","limited":"{\"message\":\"rate limit\"}","method":"eth_getBlockByHash","node":"a"},{"answered":true,"method":"eth_getLogs","node":"b"}],"refusedWidths":[{"at":9,"from":0,"node":"a","said":"range too large","to":30000},{"at":9,"blocks":30001,"node":"b","said":"range too large"}]}"#;
        let old = String::from_utf8(old.to_vec()).unwrap().replace("FORM", FORM);
        let n = Notes::parse(old.as_bytes()).expect("the notes read on");
        assert_eq!(n.lacks.keys().cloned().collect::<Vec<_>>(), vec![("a".to_string(), "eth_getRawTransactionByHash".to_string())], "only the node's word that it has no such method");
        assert!(n.widths.is_empty(), "a refused width kept by an earlier version is read as none");
        let again = json::parse(&n.render()).expect("json");
        assert_eq!(again.member("lacks").and_then(|x| x.as_arr()).map(|l| l.len()), Some(1), "written back without them");
        assert!(again.member("refusedWidths").is_none(), "and without the refused widths");
    }

    #[test]
    fn kept_lacks_naming_a_range_are_let_go_for_logs_alone() {
        let words = "{\"code\":-32000,\"message\":\"ranges over 10000 blocks are not supported on freetier\"}";
        let mut n = Notes::default();
        n.lacks.insert(("a".into(), "eth_getLogs".into()), Lack { said: words.into(), at: 7 });
        n.lacks.insert(("a".into(), "eth_getBlockReceipts".into()), Lack { said: words.into(), at: 7 });
        let back = Notes::parse(&n.render()).expect("the notes read");
        assert_eq!(back.lacks.keys().cloned().collect::<Vec<_>>(), vec![("a".to_string(), "eth_getBlockReceipts".to_string())]);
    }

    #[test]
    fn an_action_writes_each_key_it_settled_once_as_it_ends() {
        let held = Arc::new(Held::default());
        let key = |f: &str| Key { chain: 1, nodes: "a".into(), filter: f.into() };
        for i in 0..50 {
            let _ = held.save(&part_of(&key(&format!("{{\"other\":{i}}}"))), &render_entry(&key(&format!("{{\"other\":{i}}}")), &Entry { from: 1, settled: 9_000, logs: (1..200).map(log).collect(), single_source: false }));
        }
        held.saves.lock().unwrap().clear();
        {
            let _action = crate::action::open(Some(held.clone()));
            let before = held.read_bytes.load(std::sync::atomic::Ordering::SeqCst);
            assert!(entry(&key("{}")).is_none());
            let read = held.read_bytes.load(std::sync::atomic::Ordering::SeqCst) - before;
            assert_eq!(read, 0, "reading a key not kept reads nothing of the others");
            keep(&key("{}"), Entry { from: 5, settled: 50, logs: vec![log(7)], single_source: true });
            keep_on(&key("{}"), Entry { from: 5, settled: 90, logs: vec![log(7), log(60)], single_source: true }, Some((5, 50)));
            keep(&key("{\"x\":1}"), Entry { from: 1, settled: 10, logs: Vec::new(), single_source: false });
            assert!(held.saves.lock().unwrap().is_empty(), "nothing written while the action lasts");
            assert_eq!(entry(&key("{}")).map(|e| e.settled), Some(90), "read in the action: what it settled");
        }
        let saves = held.saves.lock().unwrap().clone();
        let entries: Vec<String> = [key("{}"), key("{\"x\":1}")].iter().map(part_of).collect();
        let lists: Vec<String> = [key("{}"), key("{\"x\":1}")].iter().map(|k| part_of(&positions_of(k))).collect();
        let mut got_entries: Vec<String> = saves.iter().filter(|p| entries.contains(p)).cloned().collect();
        let mut got_lists: Vec<String> = saves.iter().filter(|p| lists.contains(p)).cloned().collect();
        got_entries.sort();
        got_lists.sort();
        let (mut want_entries, mut want_lists) = (entries.clone(), lists.clone());
        want_entries.sort();
        want_lists.sort();
        assert_eq!(got_entries, want_entries, "each key it settled written once");
        assert_eq!(got_lists, want_lists, "each filter's list of node lists written once");
        assert_eq!(saves.len(), entries.len() + lists.len(), "nothing written but those: {saves:?}");
        assert_eq!(load_entry(&*held, &key("{}")).map(|e| e.settled), Some(90));
    }

    #[test]
    fn a_read_carried_on_from_an_entry_deleted_meanwhile_is_not_kept() {
        let held = Arc::new(Held::default());
        let key = Key { chain: 1, nodes: "a".into(), filter: "{}".into() };
        {
            let _action = crate::action::open(Some(held.clone()));
            keep(&key, Entry { from: 5, settled: 50, logs: vec![log(7)], single_source: true });
        }
        assert_eq!(load_entry(&*held, &key).map(|e| e.settled), Some(50));
        {
            let _action = crate::action::open(Some(held.clone()));
            assert_eq!(entry(&key).map(|e| e.settled), Some(50));
            {
                let _turn = turn();
                held.parts.lock().unwrap().clear();
            }
            keep_on(&key, Entry { from: 5, settled: 90, logs: vec![log(7), log(60)], single_source: true }, Some((5, 50)));
        }
        assert!(load_entry(&*held, &key).is_none(), "the deleted entry is not written back under the read carried on from it");
        for (e, base, settled) in [(Entry { from: 5, settled: 90, logs: vec![log(60)], single_source: true }, None, Some(90)), (Entry { from: 5, settled: 120, logs: vec![log(60)], single_source: true }, Some((5, 90)), Some(120))] {
            let _action = crate::action::open(Some(held.clone()));
            keep_on(&key, e, base);
            drop(_action);
            assert_eq!(load_entry(&*held, &key).map(|e| e.settled), settled);
        }
    }

    #[test]
    fn what_a_transaction_carries_is_kept_and_notes_without_it_carry_none() {
        let mut n = Notes::default();
        assert!(!String::from_utf8_lossy(&n.render()).contains("carried"), "no member while none is kept");
        n.carried.insert((1, [0xaa; 32]), Some(b"entry".to_vec()));
        n.carried.insert((1, [0xbb; 32]), None);
        assert_eq!(Notes::parse(&n.render()), Some(n));
    }
}
