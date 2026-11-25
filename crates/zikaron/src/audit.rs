use crate::entry::{self, Entry};
use crate::hexfmt;
use crate::json::Value;
use crate::tokens::{EntryType, FindingName, Key, Label, Token, Verdict};
use crate::trace;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
struct AnchorRec {
    chain_id: u64,
    block_number: u64,
    block_timestamp: u64,
    tx: String,
    sender: String,
    hash: String,
    verdict: Verdict,
}

#[derive(Clone, Debug)]
pub struct EvidenceRec {
    chain_id: u64,
    tx: String,
    sender: String,
    calldata: Vec<u8>,
}

struct Basis {
    chains: Vec<ChainWindow>,
    bare: HashSet<(u64, String)>,
    adoption_chains: Vec<u64>,
}

struct ChainWindow {
    chain_id: u64,
    from: u64,
    to: u64,
    registries: Vec<String>,
    senders: Vec<String>,
}

impl Basis {
    fn reaches(&self, a: &AnchorRec) -> bool {
        if self.bare.contains(&(a.chain_id, a.tx.clone())) {
            return true;
        }
        self.chains.iter().any(|w| {
            w.chain_id == a.chain_id
                && w.from <= a.block_number
                && a.block_number <= w.to
                && w.senders.iter().any(|x| *x == a.sender)
        })
    }
}

struct Inputs {
    root: String,
    pile: Vec<Vec<u8>>,
    anchors: Vec<AnchorRec>,
    unavailable: Vec<String>,
    evidence: Vec<EvidenceRec>,
    basis: Value,
    adoption_chains: Vec<u64>,
}

fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect(),
    )
}

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

pub fn no_label() -> Value {
    obj(vec![
        (Key::Ok, Value::Bool(false)),
        (Key::Reason, s(Label::NoLabel.as_str())),
    ])
}

fn as_hex20(v: Option<&Value>) -> Option<String> {
    let x = v?.as_str()?;
    if hexfmt::is_hex20(x) {
        Some(x.to_string())
    } else {
        None
    }
}

fn as_hex32(v: Option<&Value>) -> Option<String> {
    let x = v?.as_str()?;
    if hexfmt::is_hex32(x) {
        Some(x.to_string())
    } else {
        None
    }
}

fn as_int(v: Option<&Value>) -> Option<u64> {
    v?.as_int()
}

fn validate_basis(b: &Value) -> Option<Basis> {
    let ms = match b {
        Value::Obj(ms) => ms,
        _ => return None,
    };
    let allowed = ["chains", "bareTx", "adoptionChains"];
    if ms.len() != 3 || !allowed.iter().all(|k| ms.iter().any(|(mk, _)| mk == k)) {
        return None;
    }
    if ms.iter().any(|(k, _)| !allowed.contains(&k.as_str())) {
        return None;
    }

    let hex20_list = |v: Option<&Value>| -> Option<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        for a in v?.as_arr()? {
            let x = as_hex20(Some(a))?;
            if let Some(prev) = out.last() {
                if prev.as_bytes() >= x.as_bytes() {
                    return None;
                }
            }
            out.push(x);
        }
        Some(out)
    };

    let chains_arr = b.member("chains")?.as_arr()?;
    let mut chains: Vec<ChainWindow> = Vec::with_capacity(chains_arr.len());
    for c in chains_arr {
        let cm = match c {
            Value::Obj(m) => m,
            _ => return None,
        };
        let keys = ["chainId", "fromBlock", "toBlock", "registries", "senders"];
        if cm.len() != 5 || !keys.iter().all(|k| cm.iter().any(|(mk, _)| mk == k)) {
            return None;
        }
        let w = ChainWindow {
            chain_id: as_int(c.member("chainId"))?,
            from: as_int(c.member("fromBlock"))?,
            to: as_int(c.member("toBlock"))?,
            registries: hex20_list(c.member("registries"))?,
            senders: hex20_list(c.member("senders"))?,
        };
        if w.from > w.to {
            return None;
        }
        if let Some(p) = chains.last() {
            if (p.chain_id, p.from) >= (w.chain_id, w.from) {
                return None;
            }
            if p.chain_id == w.chain_id {
                if p.to >= w.from {
                    return None;
                }
                if p.to + 1 == w.from && p.registries == w.registries && p.senders == w.senders {
                    return None;
                }
            }
        }
        chains.push(w);
    }

    let bare_arr = b.member("bareTx")?.as_arr()?;
    let mut bare: HashSet<(u64, String)> = HashSet::new();
    let mut last_bare: Option<(u64, String)> = None;
    for x in bare_arr {
        let xm = match x {
            Value::Obj(m) => m,
            _ => return None,
        };
        let keys = ["chainId", "tx"];
        if xm.len() != 2 || !keys.iter().all(|k| xm.iter().any(|(mk, _)| mk == k)) {
            return None;
        }
        let key = (as_int(x.member("chainId"))?, as_hex32(x.member("tx"))?);
        if let Some(p) = &last_bare {
            if (p.0, p.1.as_bytes()) >= (key.0, key.1.as_bytes()) {
                return None;
            }
        }
        last_bare = Some(key.clone());
        bare.insert(key);
    }

    let ac = b.member("adoptionChains")?.as_arr()?;
    let mut adoption_chains: Vec<u64> = Vec::new();
    for x in ac {
        let xm = match x {
            Value::Obj(m) => m,
            _ => return None,
        };
        let keys = ["chainId", "throughBlock"];
        if xm.len() != 2 || !keys.iter().all(|k| xm.iter().any(|(mk, _)| mk == k)) {
            return None;
        }
        let id = as_int(x.member("chainId"))?;
        as_int(x.member("throughBlock"))?;
        if let Some(p) = adoption_chains.last() {
            if *p >= id {
                return None;
            }
        }
        adoption_chains.push(id);
    }
    Some(Basis {
        chains,
        bare,
        adoption_chains,
    })
}

fn validate_inputs(v: &Value) -> Option<Inputs> {
    if !v.is_obj() {
        return None;
    }
    let root = as_hex20(v.member("root"))?;

    let mut pile: Vec<Vec<u8>> = Vec::new();
    for x in v.member("pile")?.as_arr()? {
        let hx = x.as_str()?;
        if !hexfmt::is_bytes(hx) {
            return None;
        }
        pile.push(hexfmt::decode(hx)?);
    }

    let basis = v.member("basis")?.clone();
    let parsed = validate_basis(&basis)?;

    let mut anchors: Vec<AnchorRec> = Vec::new();
    let mut seen_anchor: HashMap<(u64, u64, String, String), usize> = HashMap::new();
    for x in v.member("anchors")?.as_arr()? {
        if !x.is_obj() {
            return None;
        }
        let rec = AnchorRec {
            chain_id: as_int(x.member("chainId"))?,
            block_number: as_int(x.member("blockNumber"))?,
            block_timestamp: as_int(x.member("blockTimestamp"))?,
            tx: as_hex32(x.member("tx"))?,
            sender: as_hex20(x.member("sender"))?,
            hash: as_hex32(x.member("hash"))?,
            verdict: Verdict::parse(x.member("verdict")?.as_str()?)?,
        };
        if !parsed.reaches(&rec) {
            return None;
        }
        let key = (rec.chain_id, rec.block_number, rec.tx.clone(), rec.hash.clone());
        match seen_anchor.get(&key) {
            Some(&i) => {
                let prior: &AnchorRec = &anchors[i];
                if prior.block_timestamp != rec.block_timestamp
                    || prior.sender != rec.sender
                    || prior.verdict != rec.verdict
                {
                    return None;
                }
            }
            None => {
                seen_anchor.insert(key, anchors.len());
                anchors.push(rec);
            }
        }
    }

    let mut unavailable: Vec<String> = Vec::new();
    let mut seen_unavailable: HashSet<String> = HashSet::new();
    for x in v.member("unavailable")?.as_arr()? {
        let h = as_hex32(Some(x))?;
        if seen_unavailable.insert(h.clone()) {
            unavailable.push(h);
        }
    }

    let mut evidence: Vec<EvidenceRec> = Vec::new();
    let mut seen_evidence: HashMap<(u64, String), usize> = HashMap::new();
    for x in v.member("evidence")?.as_arr()? {
        if !x.is_obj() {
            return None;
        }
        let cd = x.member("calldata")?.as_str()?;
        if !hexfmt::is_lower_bytes(cd) {
            return None;
        }
        let rec = EvidenceRec {
            chain_id: as_int(x.member("chainId"))?,
            tx: as_hex32(x.member("tx"))?,
            sender: as_hex20(x.member("sender"))?,
            calldata: hexfmt::decode(cd)?,
        };
        if !parsed.adoption_chains.contains(&rec.chain_id) {
            return None;
        }
        let key = (rec.chain_id, rec.tx.clone());
        match seen_evidence.get(&key) {
            Some(&i) => {
                let prior: &EvidenceRec = &evidence[i];
                if prior.sender != rec.sender || prior.calldata != rec.calldata {
                    return None;
                }
            }
            None => {
                seen_evidence.insert(key, evidence.len());
                evidence.push(rec);
            }
        }
    }

    Some(Inputs {
        root,
        pile,
        anchors,
        unavailable,
        evidence,
        basis,
        adoption_chains: parsed.adoption_chains,
    })
}

pub struct Finding {
    position: u64,
    name: FindingName,
    entry_id: String,
    second: String,
    hard: bool,
    extra: Vec<(Key, Value)>,
}

impl Finding {
    fn to_value(&self) -> Value {
        let mut ms: Vec<(String, Value)> = vec![
            (Key::Name.as_str().to_string(), s(self.name.as_str())),
            (Key::Position.as_str().to_string(), Value::Int(self.position)),
            (Key::EntryId.as_str().to_string(), s(&self.entry_id)),
            (Key::Hard.as_str().to_string(), Value::Bool(self.hard)),
        ];
        for (k, v) in &self.extra {
            ms.push((k.as_str().to_string(), v.clone()));
        }
        Value::Obj(ms)
    }
}

fn succession_to(e: &Entry) -> Option<String> {
    e.body.member("to")?.as_str().map(|x| x.to_string())
}

fn whole_set_lineage(root: &str, entries: &[Entry]) -> Vec<String> {
    let mut set = vec![root.to_string()];
    let mut have: HashSet<String> = HashSet::new();
    have.insert(root.to_string());
    loop {
        let mut grew = false;
        for e in entries {
            if e.kind == EntryType::Succession && have.contains(&e.author) {
                if let Some(to) = succession_to(e) {
                    if have.insert(to.clone()) {
                        set.push(to);
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            return set;
        }
    }
}

fn prefix_lineage(root: &str, ledger: &[&Entry], k: u64) -> HashSet<String> {
    let mut set: HashSet<String> = HashSet::new();
    set.insert(root.to_string());
    loop {
        let mut grew = false;
        for e in ledger {
            if e.kind == EntryType::Succession && e.seq < k && set.contains(&e.author) {
                if let Some(to) = succession_to(e) {
                    if set.insert(to) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            return set;
        }
    }
}

fn calldata_carries(calldata: &[u8], content: &[u8]) -> bool {
    if content.len() != 32 || calldata.len() < 32 {
        return false;
    }
    let mut base = 0usize;
    while base + 32 <= calldata.len() {
        for off in [base, base + 4] {
            if off + 32 <= calldata.len() && &calldata[off..off + 32] == content {
                return true;
            }
        }
        base += 32;
    }
    false
}

pub fn walk(root: &str, order: &[&Entry]) -> (Vec<Finding>, bool) {
    let ids: Vec<String> = order.iter().map(|e| e.id_hex()).collect();
    let mut by_seq: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, e) in order.iter().enumerate() {
        by_seq.entry(e.seq).or_default().push(i);
    }
    let mut findings: Vec<Finding> = Vec::new();
    let mut expected: u64 = 0;
    let mut auth: String = root.to_string();
    let mut certain = true;
    let mut prev_seq: Option<u64> = None;
    let mut gap_seen = false;

    let mut i = 0usize;
    while i < order.len() {
        let seq = order[i].seq;
        let mut j = i;
        while j < order.len() && order[j].seq == seq {
            j += 1;
        }
        for (n, e) in order[i..j].iter().enumerate() {
            let here = i + n;
            if e.seq != expected && prev_seq != Some(e.seq) {
                findings.push(Finding {
                    position: e.seq,
                    name: FindingName::SeqGap,
                    entry_id: ids[here].clone(),
                    second: String::new(),
                    hard: false,
                    extra: vec![
                        (Key::Expected, Value::Int(expected)),
                        (Key::Actual, Value::Int(e.seq)),
                    ],
                });
                expected = if e.seq == crate::json::MAX_INT { e.seq } else { e.seq + 1 };
                certain = false;
                gap_seen = true;
            } else if e.seq == expected {
                expected = if e.seq == crate::json::MAX_INT { e.seq } else { e.seq + 1 };
            }
            prev_seq = Some(e.seq);

            if e.seq >= 1 {
                let at_prev: &[usize] = by_seq.get(&(e.seq - 1)).map(|v| v.as_slice()).unwrap_or(&[]);
                if !at_prev.is_empty()
                    && !at_prev.iter().any(|&p| Some(&ids[p]) == e.prev.as_ref())
                {
                    findings.push(Finding {
                        position: e.seq,
                        name: FindingName::PrevMismatch,
                        entry_id: ids[here].clone(),
                        second: String::new(),
                        hard: true,
                        extra: vec![(Key::SeqKey, Value::Int(e.seq))],
                    });
                }
            }

            if e.seq >= 1 && e.author != auth {
                findings.push(Finding {
                    position: e.seq,
                    name: FindingName::AuthorityMismatch,
                    entry_id: ids[here].clone(),
                    second: String::new(),
                    hard: certain,
                    extra: vec![(Key::SeqKey, Value::Int(e.seq)), (Key::Certain, Value::Bool(certain))],
                });
            }

            if e.seq == 0 && e.author != root {
                findings.push(Finding {
                    position: 0,
                    name: FindingName::RootMismatch,
                    entry_id: ids[here].clone(),
                    second: String::new(),
                    hard: true,
                    extra: vec![(Key::SeqKey, Value::Int(0)), (Key::Actual, s(&e.author))],
                });
            }
        }
        let mut succs: Vec<&&Entry> = order[i..j]
            .iter()
            .filter(|e| e.kind == EntryType::Succession && e.author == auth)
            .collect();
        if !succs.is_empty() {
            succs.sort_by(|a, b| a.id.cmp(&b.id));
            if let Some(to) = succession_to(succs[0]) {
                auth = to;
            }
        }
        i = j;
    }
    (findings, gap_seen)
}

pub fn equivocations(order: &[&Entry]) -> Vec<Finding> {
    let ids: Vec<String> = order.iter().map(|e| e.id_hex()).collect();
    let mut findings = Vec::new();
    for a in 0..order.len() {
        for b in (a + 1)..order.len() {
            let (x, y) = (order[a], order[b]);
            if x.id == y.id {
                continue;
            }
            let same_seq = x.seq == y.seq;
            let same_prev = match (&x.prev, &y.prev) {
                (Some(p), Some(q)) => p == q,
                _ => false,
            };
            if same_seq || same_prev {
                let (lo, hi) = if x.id <= y.id { (a, b) } else { (b, a) };
                findings.push(Finding {
                    position: x.seq.min(y.seq),
                    name: FindingName::Equivocation,
                    entry_id: ids[lo].clone(),
                    second: ids[hi].clone(),
                    hard: true,
                    extra: vec![
                        (Key::SeqKey, Value::Int(x.seq.min(y.seq))),
                        (Key::A, s(&ids[lo])),
                        (Key::B, s(&ids[hi])),
                    ],
                });
            }
        }
    }
    findings
}

pub fn reconcile(
    order: &[&Entry],
    counted_hashes: &[String],
    unavailable: &[String],
) -> (Vec<String>, Vec<String>) {
    let have: Vec<String> = order.iter().map(|e| e.id_hex()).collect();
    let have_set: HashSet<&str> = have.iter().map(|x| x.as_str()).collect();
    let unavailable_set: HashSet<&str> = unavailable.iter().map(|x| x.as_str()).collect();
    let counted_set: HashSet<&str> = counted_hashes.iter().map(|x| x.as_str()).collect();
    let missing: Vec<String> = ordered(
        counted_hashes
            .iter()
            .filter(|h| !have_set.contains(h.as_str()) && !unavailable_set.contains(h.as_str()))
            .cloned()
            .collect(),
        |a, b| a.as_bytes().cmp(b.as_bytes()),
    );
    let unanchored: Vec<String> = ordered(
        have.iter().filter(|h| !counted_set.contains(h.as_str())).cloned().collect(),
        |a, b| a.as_bytes().cmp(b.as_bytes()),
    );
    (missing, unanchored)
}

pub fn adoption_unproven(
    root: &str,
    order: &[&Entry],
    adoption_chains: &[u64],
    evidence: &[EvidenceRec],
) -> Vec<(u64, String, u64)> {
    let mut rows: Vec<(u64, String, u64)> = Vec::new();
    let mut by_tx: HashMap<(u64, &str), &EvidenceRec> = HashMap::new();
    for r in evidence {
        by_tx.insert((r.chain_id, r.tx.as_str()), r);
    }
    for e in order {
        if e.kind != EntryType::Adoption {
            continue;
        }
        let anchors_arr = match e.body.member("anchors").and_then(|x| x.as_arr()) {
            Some(a) => a.clone(),
            None => continue,
        };
        let pl = prefix_lineage(root, order, e.seq);
        let attestor = entry::attestation_attestor(e);
        for (idx, el) in anchors_arr.iter().enumerate() {
            let chain_id = el.member("chainId").and_then(|x| x.as_int());
            let tx = el.member("tx").and_then(|x| x.as_str()).map(|x| x.to_string());
            let content = el
                .member("content")
                .and_then(|x| x.as_str())
                .and_then(hexfmt::decode);
            let proven = match (chain_id, tx, content) {
                (Some(cid), Some(txh), Some(c)) => {
                    let declared = adoption_chains.contains(&cid);
                    let rec = by_tx.get(&(cid, txh.as_str())).copied();
                    match (declared, rec) {
                        (true, Some(r)) => {
                            calldata_carries(&r.calldata, &c)
                                && (pl.contains(&r.sender)
                                    || attestor.as_deref() == Some(r.sender.as_str()))
                        }
                        _ => false,
                    }
                }
                _ => false,
            };
            if !proven {
                rows.push((e.seq, e.id_hex(), idx as u64));
            }
        }
    }
    ordered(rows, |a, b| (a.0, a.1.as_bytes(), a.2).cmp(&(b.0, b.1.as_bytes(), b.2)))
}

pub fn ordered<T>(mut rows: Vec<T>, by: impl Fn(&T, &T) -> Ordering) -> Vec<T> {
    rows.sort_by(by);
    rows
}

pub fn label_of(
    any_hard: bool,
    unavailable_empty: bool,
    unproven_unbacked: bool,
    gap_seen: bool,
    missing_empty: bool,
) -> Label {
    if any_hard {
        Label::BrokenChain
    } else if !unavailable_empty || unproven_unbacked {
        Label::Unavailable
    } else if gap_seen || !missing_empty {
        Label::Gaps
    } else {
        Label::Complete
    }
}

pub struct Outcome {
    pub report: Value,
    pub label: Label,
    pub basis: Value,
    pub root: String,
    pub ledger: Vec<Entry>,
    pub findings: Vec<FindingRow>,
    pub counted: Vec<CountedAnchor>,
    pub unproven: Vec<CountedAnchor>,
    pub lineage: Vec<String>,
}

pub struct FindingRow {
    pub name: FindingName,
    pub entry_id: String,
    pub hard: bool,
}

pub struct CountedAnchor {
    pub hash: String,
    pub block_timestamp: u64,
}

pub fn audit(input: &Value) -> Value {
    match audit_full(input) {
        Some(o) => o.report,
        None => no_label(),
    }
}

fn anchor_row(a: &AnchorRec, with_hash: bool, with_timestamp: bool, with_verdict: bool) -> Value {
    let mut ms: Vec<(Key, Value)> = Vec::with_capacity(7);
    if with_hash && !with_verdict {
        ms.push((Key::Hash, s(&a.hash)));
    }
    ms.push((Key::ChainId, Value::Int(a.chain_id)));
    ms.push((Key::BlockNumber, Value::Int(a.block_number)));
    if with_timestamp {
        ms.push((Key::BlockTimestamp, Value::Int(a.block_timestamp)));
    }
    ms.push((Key::Tx, s(&a.tx)));
    ms.push((Key::Sender, s(&a.sender)));
    if with_verdict {
        ms.push((Key::Hash, s(&a.hash)));
        ms.push((Key::VerdictKey, s(a.verdict.as_str())));
    }
    obj(ms)
}

pub fn audit_full(input: &Value) -> Option<Outcome> {
    trace::mark(trace::K1);
    let inp = validate_inputs(input)?;

    let mut pile_ids: HashSet<String> = HashSet::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen_entry: HashSet<[u8; 32]> = HashSet::new();
    let mut malformed: Vec<(String, Token)> = Vec::new();
    let mut seen_malformed: HashSet<String> = HashSet::new();
    for b in &inp.pile {
        let eid = hexfmt::encode(&entry::entry_id(b));
        pile_ids.insert(eid.clone());
        match entry::check(b) {
            Ok(e) => {
                if seen_entry.insert(e.id) {
                    entries.push(e);
                }
            }
            Err(tok) => {
                if seen_malformed.insert(eid.clone()) {
                    malformed.push((eid, tok));
                }
            }
        }
    }

    let lineage = whole_set_lineage(&inp.root, &entries);
    let in_lineage: HashSet<&str> = lineage.iter().map(|x| x.as_str()).collect();
    let ledger: Vec<&Entry> = entries.iter().filter(|e| in_lineage.contains(e.author.as_str())).collect();
    let excluded: Vec<&Entry> = entries.iter().filter(|e| !in_lineage.contains(e.author.as_str())).collect();

    let (trimmed, discarded): (Vec<&AnchorRec>, Vec<&AnchorRec>) = inp
        .anchors
        .iter()
        .partition(|a| in_lineage.contains(a.sender.as_str()));
    let counted: Vec<&AnchorRec> = trimmed.iter().copied().filter(|a| a.verdict == Verdict::Counted).collect();
    let mut counted_hashes: Vec<String> = Vec::new();
    let mut counted_set: HashSet<&str> = HashSet::new();
    for a in &counted {
        if counted_set.insert(a.hash.as_str()) {
            counted_hashes.push(a.hash.clone());
        }
    }

    let unavailable: Vec<String> = inp
        .unavailable
        .iter()
        .filter(|h| counted_set.contains(h.as_str()) && !pile_ids.contains(*h))
        .cloned()
        .collect();

    let order: Vec<&Entry> = ordered(ledger.clone(), |a, b| (a.seq, a.id).cmp(&(b.seq, b.id)));

    let (mut findings, gap_seen) = walk(&inp.root, &order);
    findings.extend(equivocations(&order));

    let findings = ordered(findings, |p, q| {
        (p.position, p.name.as_str().as_bytes(), p.entry_id.as_bytes(), p.second.as_bytes()).cmp(&(
            q.position,
            q.name.as_str().as_bytes(),
            q.entry_id.as_bytes(),
            q.second.as_bytes(),
        ))
    });
    let mut uniq: Vec<&Finding> = Vec::new();
    for f in &findings {
        match uniq.last() {
            Some(p)
                if p.position == f.position
                    && p.name == f.name
                    && p.entry_id == f.entry_id
                    && p.second == f.second => {}
            _ => uniq.push(f),
        }
    }

    let (missing, unanchored) = reconcile(&order, &counted_hashes, &unavailable);

    let mut by_hash: HashMap<&str, Vec<&AnchorRec>> = HashMap::new();
    for a in &counted {
        by_hash.entry(a.hash.as_str()).or_default().push(a);
    }
    let by_block = |a: &&AnchorRec, b: &&AnchorRec| {
        (a.chain_id, a.block_number, a.tx.as_bytes()).cmp(&(b.chain_id, b.block_number, b.tx.as_bytes()))
    };
    let records_of = |h: &str, with_timestamp: bool| -> Value {
        let rows = ordered(by_hash.get(h).cloned().unwrap_or_default(), by_block);
        Value::Arr(rows.iter().map(|a| anchor_row(a, false, with_timestamp, false)).collect())
    };

    let missing_rows: Vec<Value> = missing
        .iter()
        .map(|h| obj(vec![(Key::Hash, s(h)), (Key::Anchors, records_of(h, false))]))
        .collect();

    let anchored_ids = ordered(
        order.iter().map(|e| e.id_hex()).filter(|h| counted_set.contains(h.as_str())).collect::<Vec<String>>(),
        |a, b| a.as_bytes().cmp(b.as_bytes()),
    );
    let anchored_rows: Vec<Value> = anchored_ids
        .iter()
        .map(|h| obj(vec![(Key::EntryId, s(h)), (Key::Anchors, records_of(h, true))]))
        .collect();

    let exc = ordered(excluded.clone(), |a, b| (a.seq, a.id).cmp(&(b.seq, b.id)));
    let excluded_rows: Vec<Value> = exc
        .iter()
        .map(|e| {
            obj(vec![
                (Key::SeqKey, Value::Int(e.seq)),
                (Key::Author, s(&e.author)),
                (Key::EntryId, s(&e.id_hex())),
            ])
        })
        .collect();

    let adoption_values: Vec<Value> =
        adoption_unproven(&inp.root, &order, &inp.adoption_chains, &inp.evidence)
            .iter()
            .map(|(seq, id, idx)| {
                obj(vec![
                    (Key::SeqKey, Value::Int(*seq)),
                    (Key::EntryId, s(id)),
                    (Key::Index, Value::Int(*idx)),
                ])
            })
            .collect();

    let unknown = ordered(
        order.iter().copied().filter(|e| e.kind == EntryType::Other).collect::<Vec<&Entry>>(),
        |a, b| (a.seq, a.entry_type.as_bytes(), &a.id).cmp(&(b.seq, b.entry_type.as_bytes(), &b.id)),
    );
    let unknown_rows: Vec<Value> = unknown
        .iter()
        .map(|e| {
            obj(vec![
                (Key::SeqKey, Value::Int(e.seq)),
                (Key::EntryTypeKey, s(&e.entry_type)),
                (Key::EntryId, s(&e.id_hex())),
            ])
        })
        .collect();

    let malformed = ordered(malformed, |a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let malformed_rows: Vec<Value> = malformed
        .iter()
        .map(|(id, tok)| obj(vec![(Key::EntryId, s(id)), (Key::TokenKey, s(tok.as_str()))]))
        .collect();

    let unavail_sorted = ordered(unavailable.clone(), |a, b| a.as_bytes().cmp(b.as_bytes()));

    let verdict_rows = |want: Verdict| -> Vec<&AnchorRec> {
        ordered(
            trimmed.iter().copied().filter(|a| a.verdict == want).collect::<Vec<&AnchorRec>>(),
            |a, b| {
                (a.hash.as_bytes(), a.chain_id, a.block_number, a.tx.as_bytes()).cmp(&(
                    b.hash.as_bytes(),
                    b.chain_id,
                    b.block_number,
                    b.tx.as_bytes(),
                ))
            },
        )
    };
    let unproven_recs = verdict_rows(Verdict::Unproven);
    let void_recs = verdict_rows(Verdict::Void);
    let unproven_rows: Vec<Value> = unproven_recs.iter().map(|a| anchor_row(a, true, false, false)).collect();
    let void_rows: Vec<Value> = void_recs.iter().map(|a| anchor_row(a, true, false, false)).collect();

    let discarded = ordered(discarded, |a, b| {
        (a.sender.as_bytes(), a.chain_id, a.block_number, a.tx.as_bytes(), a.hash.as_bytes()).cmp(&(
            b.sender.as_bytes(),
            b.chain_id,
            b.block_number,
            b.tx.as_bytes(),
            b.hash.as_bytes(),
        ))
    });
    let discarded_rows: Vec<Value> = discarded.iter().map(|a| anchor_row(a, true, true, true)).collect();

    let unproven_unbacked = unproven_recs
        .iter()
        .any(|a| !pile_ids.contains(&a.hash) && !counted_set.contains(a.hash.as_str()));

    let label = label_of(
        uniq.iter().any(|f| f.hard),
        unavail_sorted.is_empty(),
        unproven_unbacked,
        gap_seen,
        missing_rows.is_empty(),
    );

    let report = obj(vec![
        (Key::Root, s(&inp.root)),
        (Key::Basis, inp.basis.clone()),
        (Key::Entries, Value::Int(order.len() as u64)),
        (Key::Findings, Value::Arr(uniq.iter().map(|f| f.to_value()).collect())),
        (Key::Missing, Value::Arr(missing_rows)),
        (Key::Anchored, Value::Arr(anchored_rows)),
        (Key::Unanchored, Value::Arr(unanchored.iter().map(|h| s(h)).collect())),
        (Key::Excluded, Value::Arr(excluded_rows)),
        (Key::AdoptionUnproven, Value::Arr(adoption_values)),
        (Key::UnknownType, Value::Arr(unknown_rows)),
        (Key::Malformed, Value::Arr(malformed_rows)),
        (Key::Unavailable, Value::Arr(unavail_sorted.iter().map(|h| s(h)).collect())),
        (Key::Unproven, Value::Arr(unproven_rows)),
        (Key::Void, Value::Arr(void_rows)),
        (Key::Discarded, Value::Arr(discarded_rows)),
        (Key::LabelKey, s(label.as_str())),
    ]);

    let flat = |recs: &[&AnchorRec]| -> Vec<CountedAnchor> {
        recs.iter()
            .map(|a| CountedAnchor {
                hash: a.hash.clone(),
                block_timestamp: a.block_timestamp,
            })
            .collect()
    };

    Some(Outcome {
        report,
        label,
        basis: inp.basis.clone(),
        root: inp.root.clone(),
        ledger: order.iter().map(|e| (*e).clone()).collect(),
        findings: uniq
            .iter()
            .map(|f| FindingRow {
                name: f.name,
                entry_id: f.entry_id.clone(),
                hard: f.hard,
            })
            .collect(),
        counted: flat(&counted),
        unproven: flat(&unproven_recs),
        lineage,
    })
}
