use super::entry::{self as e2, Entry};
use super::tokens::{self as t2, EntryType, Key, Label};
use crate::audit as v1audit;
use crate::entry as v1entry;
use crate::hexfmt;
use crate::json::Value;
use crate::tokens::{EntryType as V1Type, FindingName, Token, Verdict};
use crate::trace;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorRec {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub tx: String,
    pub sender: String,
    pub hash: String,
    pub verdict: Verdict,
}

impl AnchorRec {
    fn key(&self) -> (u64, u64, String, String) {
        (self.chain_id, self.block_number, self.tx.clone(), self.hash.clone())
    }
}

struct ChainWindow {
    chain_id: u64,
    from: u64,
    to: u64,
    registries: Vec<String>,
    senders: Vec<String>,
}

pub struct Inputs {
    pub root: String,
    pub pile: Vec<Vec<u8>>,
    pub anchors: Vec<AnchorRec>,
    pub basis: Value,
}

fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
}

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

pub fn no_label() -> Value {
    obj(vec![(Key::Ok, Value::Bool(false)), (Key::Reason, s(t2::NO_LABEL))])
}

fn hex20_of(v: Option<&Value>) -> Option<String> {
    v?.as_str().filter(|x| hexfmt::is_hex20(x)).map(str::to_string)
}

fn hex32_of(v: Option<&Value>) -> Option<String> {
    v?.as_str().filter(|x| hexfmt::is_hex32(x)).map(str::to_string)
}

fn exact<'a>(v: &'a Value, keys: &[&str]) -> Option<Vec<&'a Value>> {
    let ms = match v {
        Value::Obj(ms) => ms,
        _ => return None,
    };
    if ms.len() != keys.len() || ms.iter().any(|(k, _)| !keys.contains(&k.as_str())) {
        return None;
    }
    keys.iter().map(|k| v.member(k)).collect()
}

fn hex20_table(v: &Value) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for a in v.as_arr()? {
        let x = hex20_of(Some(a))?;
        if out.last().is_some_and(|p| p.as_bytes() >= x.as_bytes()) {
            return None;
        }
        out.push(x);
    }
    Some(out)
}

fn check_basis(b: &Value) -> Option<Vec<ChainWindow>> {
    let [chains] = exact(b, &["chains"])?[..] else { return None };
    let mut out: Vec<ChainWindow> = Vec::new();
    for c in chains.as_arr()? {
        let [cid, fb, tb, regs, sends] = exact(c, &["chainId", "fromBlock", "toBlock", "registries", "senders"])?[..]
        else {
            return None;
        };
        let w = ChainWindow {
            chain_id: cid.as_int()?,
            from: fb.as_int()?,
            to: tb.as_int()?,
            registries: hex20_table(regs)?,
            senders: hex20_table(sends)?,
        };
        if w.from > w.to {
            return None;
        }
        if let Some(p) = out.last() {
            if (w.chain_id, w.from) <= (p.chain_id, p.from) {
                return None;
            }
            if w.chain_id == p.chain_id {
                if w.from <= p.to {
                    return None;
                }
                if w.from == p.to + 1 && w.registries == p.registries && w.senders == p.senders {
                    return None;
                }
            }
        }
        out.push(w);
    }
    Some(out)
}

fn reaches(chains: &[ChainWindow], a: &AnchorRec) -> bool {
    chains.iter().any(|w| {
        w.chain_id == a.chain_id
            && w.from <= a.block_number
            && a.block_number <= w.to
            && w.senders.iter().any(|x| *x == a.sender)
    })
}

pub fn read_inputs(v: &Value) -> Option<Inputs> {
    if !v.is_obj() {
        return None;
    }
    let root = hex20_of(v.member("root"))?;
    let mut pile: Vec<Vec<u8>> = Vec::new();
    for x in v.member("pile")?.as_arr()? {
        let hx = x.as_str()?;
        if !hexfmt::is_bytes(hx) {
            return None;
        }
        pile.push(hexfmt::decode(hx)?);
    }
    let mut anchors: Vec<AnchorRec> = Vec::new();
    let mut seen: HashMap<(u64, u64, String, String), usize> = HashMap::new();
    for x in v.member("anchors")?.as_arr()? {
        if !x.is_obj() {
            return None;
        }
        let rec = AnchorRec {
            chain_id: x.member("chainId")?.as_int()?,
            block_number: x.member("blockNumber")?.as_int()?,
            block_timestamp: x.member("blockTimestamp")?.as_int()?,
            tx: hex32_of(x.member("tx"))?,
            sender: hex20_of(x.member("sender"))?,
            hash: hex32_of(x.member("hash"))?,
            verdict: Verdict::parse(x.member("verdict")?.as_str()?)?,
        };
        match seen.get(&rec.key()) {
            Some(&i) => {
                let p = &anchors[i];
                if p.block_timestamp != rec.block_timestamp || p.sender != rec.sender || p.verdict != rec.verdict {
                    return None;
                }
            }
            None => {
                seen.insert(rec.key(), anchors.len());
                anchors.push(rec);
            }
        }
    }
    let basis = v.member("basis")?.clone();
    let chains = check_basis(&basis)?;
    if !anchors.iter().all(|a| reaches(&chains, a)) {
        return None;
    }
    Some(Inputs { root, pile, anchors, basis })
}

fn eid_hex(b: &[u8]) -> String {
    hexfmt::encode(&v1entry::entry_id(b))
}

struct Parsed<E> {
    entries: Vec<E>,
    malformed: BTreeMap<String, Token>,
}

fn parse_pile<E>(pile: &[Vec<u8>], check: impl Fn(&[u8]) -> Result<E, Token>) -> Parsed<E> {
    let mut seen: HashSet<&[u8]> = HashSet::new();
    let mut entries = Vec::new();
    let mut malformed = BTreeMap::new();
    for b in pile {
        if !seen.insert(b.as_slice()) {
            continue;
        }
        match check(b) {
            Ok(e) => entries.push(e),
            Err(tok) => {
                malformed.insert(eid_hex(b), tok);
            }
        }
    }
    Parsed { entries, malformed }
}

fn line<'a, E>(start: &'a E, by_id: &HashMap<&str, &'a E>, id: impl Fn(&E) -> &str, prev: impl Fn(&E) -> Option<&str>) -> Vec<&'a E> {
    let mut out: Vec<&'a E> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut cur = Some(start);
    while let Some(c) = cur {
        if !seen.insert(id(c)) {
            break;
        }
        out.push(c);
        cur = prev(c).and_then(|p| by_id.get(p).copied());
    }
    out
}

fn line2<'a>(start: &'a Entry, by_id: &HashMap<&str, &'a Entry>) -> Vec<&'a Entry> {
    line(start, by_id, |e| e.id_hex.as_str(), |e| e.prev.as_deref())
}

fn succession_to(e: &Entry) -> Option<&str> {
    e.body.member("to").and_then(Value::as_str)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retirement {
    pub at: u64,
    pub seq: u64,
}

pub fn retirements(entries: &[Entry], anchors: &[AnchorRec]) -> HashMap<String, Retirement> {
    let by_id: HashMap<&str, &Entry> = entries.iter().map(|e| (e.id_hex.as_str(), e)).collect();
    let mut ret: HashMap<String, Retirement> = HashMap::new();
    for a in anchors {
        let Some(e) = by_id.get(a.hash.as_str()) else { continue };
        if !matches!(e.kind, EntryType::Succession | EntryType::Migration) {
            continue;
        }
        let now = Retirement { at: a.block_timestamp, seq: e.seq };
        ret.entry(e.author.clone()).and_modify(|r| *r = (*r).min_by_key_pair(now)).or_insert(now);
    }
    ret
}

impl Retirement {
    fn min_by_key_pair(self, o: Retirement) -> Retirement {
        if (o.at, o.seq) < (self.at, self.seq) { o } else { self }
    }
}

pub fn timely_set(entries: &[Entry], anchors: &[AnchorRec], ret: &HashMap<String, Retirement>) -> HashSet<String> {
    let by_id: HashMap<&str, &Entry> = entries.iter().map(|e| (e.id_hex.as_str(), e)).collect();
    let mut first: HashMap<&str, u64> = HashMap::new();
    for a in anchors {
        if let Some((k, _)) = by_id.get_key_value(a.hash.as_str()) {
            let t = first.entry(*k).or_insert(a.block_timestamp);
            *t = (*t).min(a.block_timestamp);
        }
    }
    let mut bound: HashMap<&str, u64> = HashMap::new();
    for (f, t) in &first {
        for e in line2(by_id[f], &by_id) {
            let b = bound.entry(e.id_hex.as_str()).or_insert(*t);
            *b = (*b).min(*t);
        }
    }
    entries
        .iter()
        .filter(|e| match ret.get(&e.author) {
            None => true,
            Some(r) => bound.get(e.id_hex.as_str()).is_some_and(|b| *b < r.at || (*b == r.at && e.seq <= r.seq)),
        })
        .map(|e| e.id_hex.clone())
        .collect()
}

pub fn lineage(seed: &str, entries: &[Entry], timely: &HashSet<String>) -> BTreeSet<String> {
    let mut lin: BTreeSet<String> = BTreeSet::new();
    lin.insert(seed.to_string());
    loop {
        let mut grew = false;
        for e in entries {
            if e.kind == EntryType::Succession && timely.contains(&e.id_hex) && lin.contains(&e.author) {
                if let Some(to) = succession_to(e) {
                    grew |= lin.insert(to.to_string());
                }
            }
        }
        if !grew {
            return lin;
        }
    }
}

fn has_retired(a: &AnchorRec, ret: &HashMap<String, Retirement>, by_id: &HashMap<&str, &Entry>) -> bool {
    let Some(r) = ret.get(&a.sender) else { return false };
    a.block_timestamp > r.at
        || (a.block_timestamp == r.at && by_id.get(a.hash.as_str()).is_some_and(|e| e.author == a.sender && e.seq > r.seq))
}

struct Finding {
    position: u64,
    name: FindingName,
    entry_id: String,
    second: String,
    hard: bool,
    extra: Vec<(Key, Value)>,
}

impl Finding {
    fn sort_key(&self) -> (u64, &[u8], &[u8], &[u8]) {
        (self.position, self.name.as_str().as_bytes(), self.entry_id.as_bytes(), self.second.as_bytes())
    }

    fn to_value(&self) -> Value {
        let mut ms = vec![
            (Key::Name, s(self.name.as_str())),
            (Key::Position, Value::Int(self.position)),
            (Key::EntryId, s(&self.entry_id)),
            (Key::Hard, Value::Bool(self.hard)),
        ];
        ms.extend(self.extra.iter().cloned());
        obj(ms)
    }
}

fn walk(seed: &str, order: &[&Entry]) -> (Vec<Finding>, bool) {
    let mut by_seq: HashMap<u64, Vec<&str>> = HashMap::new();
    for e in order {
        by_seq.entry(e.seq).or_default().push(e.id_hex.as_str());
    }
    let mut findings = Vec::new();
    let mut expected: u64 = 0;
    let mut auth = seed.to_string();
    let mut closed = false;
    let mut certain = true;
    let mut last: Option<u64> = None;
    let mut gap = false;
    let next = |x: u64| if x == crate::json::MAX_INT { x } else { x + 1 };
    let mut i = 0;
    while i < order.len() {
        let seq = order[i].seq;
        let mut j = i;
        while j < order.len() && order[j].seq == seq {
            j += 1;
        }
        let group = &order[i..j];
        for e in group {
            if e.seq == expected {
                expected = next(e.seq);
            } else if last != Some(e.seq) {
                findings.push(Finding {
                    position: e.seq,
                    name: FindingName::SeqGap,
                    entry_id: e.id_hex.clone(),
                    second: String::new(),
                    hard: false,
                    extra: vec![(Key::Expected, Value::Int(expected)), (Key::Actual, Value::Int(e.seq))],
                });
                gap = true;
                expected = next(e.seq);
                certain = false;
            }
            last = Some(e.seq);
            if e.seq >= 1 {
                if let Some(pred) = by_seq.get(&(e.seq - 1)) {
                    if !pred.iter().any(|p| Some(*p) == e.prev.as_deref()) {
                        findings.push(Finding {
                            position: e.seq,
                            name: FindingName::PrevMismatch,
                            entry_id: e.id_hex.clone(),
                            second: String::new(),
                            hard: true,
                            extra: vec![(Key::SeqKey, Value::Int(e.seq))],
                        });
                    }
                }
            }
            if e.seq >= 1 && (closed || e.author != auth) {
                findings.push(Finding {
                    position: e.seq,
                    name: FindingName::AuthorityMismatch,
                    entry_id: e.id_hex.clone(),
                    second: String::new(),
                    hard: certain,
                    extra: vec![(Key::SeqKey, Value::Int(e.seq)), (Key::Certain, Value::Bool(certain))],
                });
            }
            if e.seq == 0 && e.author != seed {
                findings.push(Finding {
                    position: 0,
                    name: FindingName::RootMismatch,
                    entry_id: e.id_hex.clone(),
                    second: String::new(),
                    hard: true,
                    extra: vec![(Key::SeqKey, Value::Int(0)), (Key::Actual, s(&e.author))],
                });
            }
        }
        if !closed {
            if group.iter().any(|e| e.kind == EntryType::Migration && e.author == auth) {
                closed = true;
            } else if let Some(sx) = group
                .iter()
                .filter(|e| e.kind == EntryType::Succession && e.author == auth)
                .min_by(|a, b| a.id.cmp(&b.id))
            {
                if let Some(to) = succession_to(sx) {
                    auth = to.to_string();
                }
            }
        }
        i = j;
    }
    (findings, gap)
}

fn equivocations(order: &[&Entry]) -> Vec<Finding> {
    let mut out = Vec::new();
    for a in 0..order.len() {
        for b in (a + 1)..order.len() {
            let (x, y) = (order[a], order[b]);
            if x.id == y.id {
                continue;
            }
            let same_prev = matches!((&x.prev, &y.prev), (Some(p), Some(q)) if p == q);
            if x.seq == y.seq || same_prev {
                let (lo, hi) = if x.id < y.id { (x, y) } else { (y, x) };
                let seq = x.seq.min(y.seq);
                out.push(Finding {
                    position: seq,
                    name: FindingName::Equivocation,
                    entry_id: lo.id_hex.clone(),
                    second: hi.id_hex.clone(),
                    hard: true,
                    extra: vec![(Key::SeqKey, Value::Int(seq)), (Key::A, s(&lo.id_hex)), (Key::B, s(&hi.id_hex))],
                });
            }
        }
    }
    out
}

pub struct Segment {
    pub seed: String,
    pub ledger: Vec<Entry>,
    pub lineage: BTreeSet<String>,
    pub kept: Vec<AnchorRec>,
    pub label: Label,
}

fn by_block(a: &&AnchorRec, b: &&AnchorRec) -> std::cmp::Ordering {
    (a.chain_id, a.block_number, a.tx.as_bytes()).cmp(&(b.chain_id, b.block_number, b.tx.as_bytes()))
}

fn audit_segment(
    root: &str,
    seed: &str,
    pile: &[Vec<u8>],
    anchors: &[AnchorRec],
    basis: &Value,
    continues: Value,
    whole_pile_ids: &HashSet<String>,
) -> (Value, Segment) {
    let parsed = parse_pile(pile, e2::check);
    let entries = parsed.entries;
    let ret = retirements(&entries, anchors);
    let timely = timely_set(&entries, anchors, &ret);
    let lin = lineage(seed, &entries, &timely);

    let mut ledger: Vec<&Entry> =
        entries.iter().filter(|e| lin.contains(&e.author) && timely.contains(&e.id_hex)).collect();
    ledger.sort_by(|a, b| (a.seq, a.id).cmp(&(b.seq, b.id)));
    let seq_order = |a: &&Entry, b: &&Entry| (a.seq, a.id).cmp(&(b.seq, b.id));
    let mut excluded: Vec<&Entry> = entries.iter().filter(|e| !lin.contains(&e.author)).collect();
    excluded.sort_by(seq_order);
    let mut retired: Vec<&Entry> =
        entries.iter().filter(|e| lin.contains(&e.author) && !timely.contains(&e.id_hex)).collect();
    retired.sort_by(seq_order);

    let by_id: HashMap<&str, &Entry> = entries.iter().map(|e| (e.id_hex.as_str(), e)).collect();
    let (mut discarded, kept): (Vec<&AnchorRec>, Vec<&AnchorRec>) =
        anchors.iter().partition(|a| !lin.contains(&a.sender) || has_retired(a, &ret, &by_id));
    let counted: Vec<&AnchorRec> = kept.iter().copied().filter(|a| a.verdict == Verdict::Counted).collect();
    let counted_hashes: BTreeSet<&str> = counted.iter().map(|a| a.hash.as_str()).collect();

    let (mut findings, gap) = walk(seed, &ledger);
    findings.extend(equivocations(&ledger));
    findings.sort_by(|p, q| p.sort_key().cmp(&q.sort_key()));
    findings.dedup_by(|q, p| p.sort_key() == q.sort_key());
    let any_hard = findings.iter().any(|f| f.hard);

    let have: BTreeSet<&str> = ledger.iter().map(|e| e.id_hex.as_str()).collect();
    let mut by_hash: HashMap<&str, Vec<&AnchorRec>> = HashMap::new();
    for a in &counted {
        by_hash.entry(a.hash.as_str()).or_default().push(a);
    }
    let records = |h: &str, with_time: bool| -> Value {
        let mut rs = by_hash.get(h).cloned().unwrap_or_default();
        rs.sort_by(by_block);
        Value::Arr(
            rs.iter()
                .map(|a| {
                    let mut ms = vec![(Key::ChainId, Value::Int(a.chain_id)), (Key::BlockNumber, Value::Int(a.block_number))];
                    if with_time {
                        ms.push((Key::BlockTimestamp, Value::Int(a.block_timestamp)));
                    }
                    ms.push((Key::Tx, s(&a.tx)));
                    ms.push((Key::Sender, s(&a.sender)));
                    obj(ms)
                })
                .collect(),
        )
    };
    let missing: Vec<Value> = counted_hashes
        .iter()
        .filter(|h| !have.contains(*h))
        .map(|h| obj(vec![(Key::Hash, s(h)), (Key::Anchors, records(h, false))]))
        .collect();
    let anchored: Vec<Value> = have
        .iter()
        .filter(|h| counted_hashes.contains(*h))
        .map(|h| obj(vec![(Key::EntryId, s(h)), (Key::Anchors, records(h, true))]))
        .collect();
    let unanchored: Vec<Value> = have.iter().filter(|h| !counted_hashes.contains(*h)).map(|h| s(h)).collect();

    let seq_rows = |es: &[&Entry]| -> Value {
        Value::Arr(
            es.iter()
                .map(|e| obj(vec![(Key::SeqKey, Value::Int(e.seq)), (Key::Author, s(&e.author)), (Key::EntryId, s(&e.id_hex))]))
                .collect(),
        )
    };
    let mut unknown: Vec<&Entry> = ledger.iter().copied().filter(|e| e.kind == EntryType::Other).collect();
    unknown.sort_by(|a, b| (a.seq, a.entry_type.as_bytes(), a.id).cmp(&(b.seq, b.entry_type.as_bytes(), b.id)));
    let unknown_rows: Vec<Value> = unknown
        .iter()
        .map(|e| obj(vec![(Key::SeqKey, Value::Int(e.seq)), (Key::EntryTypeKey, s(&e.entry_type)), (Key::EntryId, s(&e.id_hex))]))
        .collect();
    let malformed_rows: Vec<Value> = parsed
        .malformed
        .iter()
        .map(|(h, tok)| obj(vec![(Key::EntryId, s(h)), (Key::TokenKey, s(tok.as_str()))]))
        .collect();
    let verdict_rows = |want: Verdict| -> Vec<&AnchorRec> {
        let mut rs: Vec<&AnchorRec> = kept.iter().copied().filter(|a| a.verdict == want).collect();
        rs.sort_by(|a, b| {
            (a.hash.as_bytes(), a.chain_id, a.block_number, a.tx.as_bytes()).cmp(&(b.hash.as_bytes(), b.chain_id, b.block_number, b.tx.as_bytes()))
        });
        rs
    };
    let row = |a: &&AnchorRec| {
        obj(vec![
            (Key::Hash, s(&a.hash)),
            (Key::ChainId, Value::Int(a.chain_id)),
            (Key::BlockNumber, Value::Int(a.block_number)),
            (Key::Tx, s(&a.tx)),
            (Key::Sender, s(&a.sender)),
        ])
    };
    let unproven = verdict_rows(Verdict::Unproven);
    let void = verdict_rows(Verdict::Void);
    discarded.sort_by(|a, b| {
        (a.sender.as_bytes(), a.chain_id, a.block_number, a.tx.as_bytes(), a.hash.as_bytes()).cmp(&(
            b.sender.as_bytes(),
            b.chain_id,
            b.block_number,
            b.tx.as_bytes(),
            b.hash.as_bytes(),
        ))
    });
    let discarded_rows: Vec<Value> = discarded
        .iter()
        .map(|a| {
            obj(vec![
                (Key::ChainId, Value::Int(a.chain_id)),
                (Key::BlockNumber, Value::Int(a.block_number)),
                (Key::BlockTimestamp, Value::Int(a.block_timestamp)),
                (Key::Tx, s(&a.tx)),
                (Key::Sender, s(&a.sender)),
                (Key::Hash, s(&a.hash)),
                (Key::VerdictKey, s(a.verdict.as_str())),
            ])
        })
        .collect();

    let unproven_open = unproven.iter().any(|a| !whole_pile_ids.contains(&a.hash) && !counted_hashes.contains(a.hash.as_str()));
    let label = if any_hard {
        Label::BrokenChain
    } else if unproven_open {
        Label::Unproven
    } else if gap || !missing.is_empty() {
        Label::Gaps
    } else {
        Label::Complete
    };

    let report = obj(vec![
        (Key::Root, s(root)),
        (Key::Basis, basis.clone()),
        (Key::Entries, Value::Int(ledger.len() as u64)),
        (Key::Findings, Value::Arr(findings.iter().map(Finding::to_value).collect())),
        (Key::Missing, Value::Arr(missing)),
        (Key::Anchored, Value::Arr(anchored)),
        (Key::Unanchored, Value::Arr(unanchored)),
        (Key::Excluded, seq_rows(&excluded)),
        (Key::Retired, seq_rows(&retired)),
        (Key::UnknownType, Value::Arr(unknown_rows)),
        (Key::Malformed, Value::Arr(malformed_rows)),
        (Key::Unproven, Value::Arr(unproven.iter().map(row).collect())),
        (Key::Void, Value::Arr(void.iter().map(row).collect())),
        (Key::Discarded, Value::Arr(discarded_rows)),
        (Key::Continues, continues),
        (Key::LabelKey, s(label.as_str())),
    ]);
    let seg = Segment {
        seed: seed.to_string(),
        ledger: ledger.iter().map(|e| (*e).clone()).collect(),
        lineage: lin,
        kept: kept.into_iter().cloned().collect(),
        label,
    };
    (report, seg)
}

pub struct OldSegment {
    pub bytes: BTreeSet<Vec<u8>>,
    pub records: Vec<AnchorRec>,
    pub report: Value,
    pub entries: Vec<v1entry::Entry>,
    pub lineage: BTreeSet<String>,
    pub ledger: BTreeSet<String>,
}

pub struct Continuation {
    pub genesis: Entry,
    pub succession: v1entry::Entry,
    pub cut: u64,
    pub old: OldSegment,
}

fn rec_value(a: &AnchorRec) -> Value {
    Value::Obj(vec![
        ("chainId".into(), Value::Int(a.chain_id)),
        ("blockNumber".into(), Value::Int(a.block_number)),
        ("blockTimestamp".into(), Value::Int(a.block_timestamp)),
        ("tx".into(), s(&a.tx)),
        ("sender".into(), s(&a.sender)),
        ("hash".into(), s(&a.hash)),
        ("verdict".into(), s(a.verdict.as_str())),
    ])
}

fn v1_audit(root: &str, old: &BTreeSet<Vec<u8>>, recs: &[AnchorRec], basis: &Value) -> Option<v1audit::Outcome> {
    let chains = basis.member("chains")?.clone();
    let input = Value::Obj(vec![
        ("root".into(), s(root)),
        ("pile".into(), Value::Arr(old.iter().map(|b| s(&hexfmt::encode(b))).collect())),
        ("anchors".into(), Value::Arr(recs.iter().map(rec_value).collect())),
        ("unavailable".into(), Value::Arr(vec![])),
        ("evidence".into(), Value::Arr(vec![])),
        (
            "basis".into(),
            Value::Obj(vec![
                ("chains".into(), chains),
                ("bareTx".into(), Value::Arr(vec![])),
                ("adoptionChains".into(), Value::Arr(vec![])),
            ]),
        ),
    ]);
    v1audit::audit_full(&input)
}

fn v1_line<'a>(start: &'a v1entry::Entry, by_id: &HashMap<String, &'a v1entry::Entry>) -> Vec<&'a v1entry::Entry> {
    let mut out = Vec::new();
    let mut seen: HashSet<[u8; 32]> = HashSet::new();
    let mut cur = Some(start);
    while let Some(c) = cur {
        if !seen.insert(c.id) {
            break;
        }
        out.push(c);
        cur = c.prev.as_ref().and_then(|p| by_id.get(p).copied());
    }
    out
}

struct V1Pile<'a> {
    by_raw: HashMap<&'a [u8], v1entry::Entry>,
    by_id: HashMap<String, &'a [u8]>,
}

impl<'a> V1Pile<'a> {
    fn new(pile: &'a [Vec<u8>]) -> Self {
        let mut by_raw: HashMap<&'a [u8], v1entry::Entry> = HashMap::new();
        for b in pile {
            if by_raw.contains_key(b.as_slice()) {
                continue;
            }
            if let Ok(e) = v1entry::check(b) {
                by_raw.insert(b.as_slice(), e);
            }
        }
        let by_id = by_raw.iter().map(|(b, e)| (e.id_hex(), *b)).collect();
        V1Pile { by_raw, by_id }
    }

    fn get_id(&self, id: &str) -> Option<&v1entry::Entry> {
        self.by_raw.get(self.by_id.get(id)?)
    }

    fn id_map(&self) -> HashMap<String, &v1entry::Entry> {
        self.by_raw.values().map(|e| (e.id_hex(), e)).collect()
    }
}

fn old_segment(
    root: &str,
    pile: &[Vec<u8>],
    anchors: &[AnchorRec],
    basis: &Value,
    sx: &v1entry::Entry,
    cut: u64,
    v1: &V1Pile,
) -> Option<OldSegment> {
    let by_id = v1.id_map();
    let mut old: BTreeSet<Vec<u8>> = BTreeSet::new();
    for e in v1_line(sx, &by_id) {
        old.insert(e.bytes.clone());
    }
    let early: HashSet<&str> = anchors.iter().filter(|a| a.block_timestamp <= cut).map(|a| a.hash.as_str()).collect();
    let distinct: BTreeSet<&[u8]> = pile.iter().map(Vec::as_slice).collect();
    for b in distinct {
        if early.contains(eid_hex(b).as_str()) {
            old.insert(b.to_vec());
            if let Some(e) = v1.by_raw.get(b) {
                for x in v1_line(e, &by_id) {
                    old.insert(x.bytes.clone());
                }
            }
        }
    }
    let by_bytes = v1_audit(root, &old, &[], basis)?;
    let ledger: BTreeSet<String> = by_bytes.ledger.iter().map(|e| e.id_hex()).collect();
    let records: Vec<AnchorRec> =
        anchors.iter().filter(|a| a.block_timestamp <= cut || ledger.contains(&a.hash)).cloned().collect();
    let outcome = v1_audit(root, &old, &records, basis)?;
    let entries: Vec<v1entry::Entry> = old.iter().filter_map(|b| v1.by_raw.get(b.as_slice()).cloned()).collect();
    Some(OldSegment {
        bytes: old,
        records,
        report: outcome.report,
        entries,
        lineage: by_bytes.lineage.into_iter().collect(),
        ledger,
    })
}

fn find_continuation(
    inp: &Inputs,
    v2_entries: &[Entry],
    v1: &V1Pile,
) -> Result<Option<Continuation>, ()> {
    let mut by_hash: HashMap<&str, Vec<&AnchorRec>> = HashMap::new();
    for a in &inp.anchors {
        by_hash.entry(a.hash.as_str()).or_default().push(a);
    }
    let mut best: Option<Continuation> = None;
    for g in v2_entries {
        if g.seq != 0 || g.kind != EntryType::Genesis {
            continue;
        }
        let Some(c) = g.body.member("continues").filter(|c| c.is_obj()) else { continue };
        if c.member("root").and_then(Value::as_str) != Some(inp.root.as_str()) {
            continue;
        }
        let Some(sx) = c.member("entry_id").and_then(Value::as_str).and_then(|id| v1.get_id(id)) else { continue };
        if sx.kind != V1Type::Succession || sx.author != g.author {
            continue;
        }
        if sx.body.member("to").and_then(Value::as_str) != Some(g.author.as_str())
            || sx.body.member("kind").and_then(Value::as_str) != Some(t2::CLOSING_KIND)
        {
            continue;
        }
        let Some(recs) = by_hash.get(sx.id_hex().as_str()) else { continue };
        let Some(cut) = recs.iter().map(|a| a.block_timestamp).min() else { continue };
        let handed_off = v1.by_raw.values().any(|e| {
            e.kind == V1Type::Succession
                && e.author == sx.author
                && e.body.member("to").and_then(Value::as_str) != Some(sx.author.as_str())
                && by_hash.get(e.id_hex().as_str()).is_some_and(|rs| rs.iter().any(|a| a.block_timestamp < cut))
        });
        if handed_off {
            continue;
        }
        let old = old_segment(&inp.root, &inp.pile, &inp.anchors, &inp.basis, sx, cut, v1).ok_or(())?;
        if !old.ledger.contains(&sx.id_hex()) {
            continue;
        }
        let better = match &best {
            None => true,
            Some(b) => (cut, g.id) < (b.cut, b.genesis.id),
        };
        if better {
            best = Some(Continuation { genesis: g.clone(), succession: sx.clone(), cut, old });
        }
    }
    Ok(best)
}

pub struct Outcome {
    pub report: Value,
    pub root: String,
    pub basis: Value,
    pub label: Label,
    pub new: Segment,
    pub continuation: Option<Continuation>,
}

pub fn audit(input: &Value) -> Value {
    match audit_full(input) {
        Some(o) => o.report,
        None => no_label(),
    }
}

pub fn audit_full(input: &Value) -> Option<Outcome> {
    trace::mark(t2::K3);
    let inp = read_inputs(input)?;
    let whole_ids: HashSet<String> = inp.pile.iter().map(|b| eid_hex(b)).collect();
    let v2_entries = parse_pile(&inp.pile, e2::check).entries;
    let v1 = V1Pile::new(&inp.pile);
    let cont = find_continuation(&inp, &v2_entries, &v1).ok()?;
    let Some(cont) = cont else {
        let (report, new) = audit_segment(&inp.root, &inp.root, &inp.pile, &inp.anchors, &inp.basis, Value::Null, &whole_ids);
        return Some(Outcome { label: new.label, report, root: inp.root, basis: inp.basis, new, continuation: None });
    };
    let new_pile: Vec<Vec<u8>> = inp
        .pile
        .iter()
        .filter(|b| !(cont.old.bytes.contains(*b) && v1.by_raw.contains_key(b.as_slice())))
        .cloned()
        .collect();
    let old_keys: HashSet<(u64, u64, String, String)> = cont.old.records.iter().map(AnchorRec::key).collect();
    let new_anchors: Vec<AnchorRec> = inp.anchors.iter().filter(|a| !old_keys.contains(&a.key())).cloned().collect();
    let continues = obj(vec![
        (Key::EntryId, s(&cont.succession.id_hex())),
        (Key::Cut, Value::Int(cont.cut)),
        (Key::Report, cont.old.report.clone()),
    ]);
    let (report, new) =
        audit_segment(&inp.root, &cont.genesis.author, &new_pile, &new_anchors, &inp.basis, continues, &whole_ids);
    let old_label = cont.old.report.member("label").and_then(Value::as_str).and_then(Label::of_v1)?;
    let label = new.label.min(old_label);
    let report = match report {
        Value::Obj(ms) => Value::Obj(
            ms.into_iter()
                .map(|(k, v)| if k == Key::LabelKey.as_str() { (k, s(label.as_str())) } else { (k, v) })
                .collect(),
        ),
        other => other,
    };
    Some(Outcome { report, root: inp.root, basis: inp.basis, label, new, continuation: Some(cont) })
}
