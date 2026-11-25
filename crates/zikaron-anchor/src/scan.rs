use crate::rpc::{Broke, Endpoint, Trouble};
use crate::tx::{self, Tx};
use zikaron::cryptox;
use zikaron::hexfmt;
use crate::wire::W;
use zikaron::json::Value;
use zikaron::tokens::Verdict;

pub const ANCHORED_SIGNATURE: &str = "Anchored(address,bytes32)";

pub const OVER_ASKS: &str = "OVER_ASKS";

pub fn topic0() -> [u8; 32] {
    cryptox::keccak256(ANCHORED_SIGNATURE.as_bytes())
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    ChainIdMismatch { declared: u64, served: String },
    NoEndpoint(u64),
    Unanswered { chain: u64, asked: Asked },
    TxNotItself(String),
    Malformed(Shape),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Asked {
    Declined { method: String, said: String, width: Option<(u64, u64)> },
    Broke { method: String, broke: Broke },
    NotServed(String),
    NotLogs,
    NoReceipt(String),
    Short,
    OverAsks { cap: u64, said: Option<String> },
}

impl Asked {
    pub fn evidence(&self) -> String {
        match self {
            Asked::Declined { method, said, .. } => format!("{method} {said}"),
            Asked::Broke { method, broke } => format!("{method} {}", broke.evidence()),
            Asked::NotServed(k) => format!("NOT_SERVED {k}"),
            Asked::NotLogs => "eth_getLogs NOT_LOGS".to_string(),
            Asked::NoReceipt(tx) => format!("eth_getTransactionReceipt NO_RECEIPT {tx}"),
            Asked::Short => "SHORT".to_string(),
            Asked::OverAsks { cap, said: None } => format!("eth_getLogs {OVER_ASKS} {cap}"),
            Asked::OverAsks { cap, said: Some(said) } => format!("eth_getLogs {OVER_ASKS} {cap} {said}"),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Shape {
    TxLacks { part: String, tx: String },
    NoTimestamp(u64),
    CodeNotBytes,
    Basis,
    Contradiction(String),
}

impl Shape {
    pub fn evidence(&self) -> String {
        match self {
            Shape::TxLacks { part, tx } => format!("TX_LACKS {part} {tx}"),
            Shape::NoTimestamp(n) => format!("eth_getBlockByNumber NO_TIMESTAMP {n}"),
            Shape::CodeNotBytes => "eth_getCode NOT_BYTES".to_string(),
            Shape::Basis => "BASIS_UNREAD".to_string(),
            Shape::Contradiction(k) => format!("CONTRADICTION {k}"),
        }
    }
}

impl Refusal {
    pub fn stands_aside(&self) -> bool {
        match self {
            Refusal::Unanswered { asked: Asked::Declined { said, .. }, .. } => crate::patience::stands_aside(&Trouble::Node(said.clone())),
            Refusal::Unanswered { asked: Asked::Broke { broke, .. }, .. } => crate::patience::stands_aside(&Trouble::Transport(broke.clone())),
            _ => false,
        }
    }

    pub fn cut(&self) -> bool {
        matches!(self, Refusal::Unanswered { asked: Asked::Declined { width: Some(_), .. }, .. })
    }

    pub fn over_asks(&self) -> Option<u64> {
        match self {
            Refusal::Unanswered { asked: Asked::OverAsks { cap, .. }, .. } => Some(*cap),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Refusal::ChainIdMismatch { .. } => "E_CHAIN_ID",
            Refusal::NoEndpoint(_) => "E_NO_ENDPOINT",
            Refusal::Unanswered { .. } => "E_UNANSWERED",
            Refusal::TxNotItself(_) => "E_TX_NOT_ITSELF",
            Refusal::Malformed(_) => "E_MALFORMED",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Refusal::ChainIdMismatch { declared, served } => format!("chain {declared} eth_chainId {served}"),
            Refusal::NoEndpoint(c) => format!("chain {c}"),
            Refusal::Unanswered { chain, asked } => format!("chain {chain} {}", asked.evidence()),
            Refusal::TxNotItself(h) => format!("tx {h}"),
            Refusal::Malformed(w) => w.evidence(),
        }
    }
}

pub fn endpoint_serves(served: &W, declared: u64) -> bool {
    chain_served(served) == Some(declared)
}

pub fn chain_served(served: &W) -> Option<u64> {
    tx::hex_qty(served)
}

pub fn serves_another(served: Option<u64>, given: u64) -> Option<u64> {
    served.filter(|s| *s != given)
}

#[derive(Clone, Debug)]
pub struct Window {
    pub chain_id: u64,
    pub from_block: u64,
    pub to_block: u64,
    pub registries: Vec<[u8; 20]>,
    pub senders: Vec<[u8; 20]>,
}

#[derive(Clone, Debug)]
pub struct Basis {
    pub windows: Vec<Window>,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct AnchorRec {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub tx: [u8; 32],
    pub sender: [u8; 20],
    pub hash: [u8; 32],
    pub verdict: Verdict,
}

pub struct Scanned {
    pub anchors: Vec<AnchorRec>,
    pub basis: Value,
    pub candidates: Vec<Claim>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Claim {
    pub chain_id: u64,
    pub block_number: u64,
    pub tx: [u8; 32],
    pub log_index: u64,
    pub sender: [u8; 20],
    pub hash: [u8; 32],
}

#[derive(Clone)]
pub enum Pick {
    All,
    Only(std::sync::Arc<dyn Fn(&[u8; 32]) -> bool + Send + Sync>),
}

impl Pick {
    pub fn only(choose: impl Fn(&[u8; 32]) -> bool + Send + Sync + 'static) -> Pick {
        Pick::Only(std::sync::Arc::new(choose))
    }

    fn tests(&self, chain: u64, w: &Window, log: &W, t0: &[u8; 32]) -> bool {
        match self {
            Pick::All => true,
            Pick::Only(f) => anchored_log(log, t0).is_some_and(|x| f(&x.hash) || claim_of(chain, w, log, t0).is_none()),
        }
    }
}

impl std::fmt::Debug for Pick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Pick::All => "Pick::All",
            Pick::Only(_) => "Pick::Only",
        })
    }
}

fn claim_of(chain: u64, w: &Window, log: &W, t0: &[u8; 32]) -> Option<Claim> {
    let f = anchored_log(log, t0)?;
    if f.block_number < w.from_block || f.block_number > w.to_block || !window_admits(w, &f.claimed) {
        return None;
    }
    let emitter = emitter_of(log)?;
    if !w.registries.iter().any(|r| *r == emitter) {
        return None;
    }
    let log_index = log.member("logIndex").and_then(tx::hex_qty)?;
    Some(Claim { chain_id: chain, block_number: f.block_number, tx: f.tx, log_index, sender: f.claimed, hash: f.hash })
}

pub fn candidates_in(fragment: &Value) -> Vec<Claim> {
    let Some(Value::Arr(xs)) = fragment.member("candidates") else { return Vec::new() };
    xs.iter()
        .filter_map(|c| {
            let int = |k: &str| match c.member(k) {
                Some(Value::Int(n)) => Some(*n),
                _ => None,
            };
            let h32s = |k: &str| c.member(k).and_then(Value::as_str).and_then(hexfmt::decode).and_then(|b| <[u8; 32]>::try_from(b).ok());
            Some(Claim {
                chain_id: int("chainId")?,
                block_number: int("blockNumber")?,
                tx: h32s("tx")?,
                log_index: int("logIndex")?,
                sender: c.member("sender").and_then(addr_of)?,
                hash: h32s("hash")?,
            })
        })
        .collect()
}

fn claim_value(c: &Claim) -> Value {
    Value::Obj(vec![
        ("blockNumber".into(), Value::Int(c.block_number)),
        ("chainId".into(), Value::Int(c.chain_id)),
        ("hash".into(), Value::Str(hexfmt::encode(&c.hash))),
        ("logIndex".into(), Value::Int(c.log_index)),
        ("sender".into(), Value::Str(hex20(&c.sender))),
        ("tx".into(), Value::Str(hexfmt::encode(&c.tx))),
    ])
}

pub fn joined(parts: &[Value], basis: Value) -> Value {
    let mut anchors = Vec::new();
    let mut candidates = Vec::new();
    for p in parts {
        if let Some(Value::Arr(xs)) = p.member("anchors") {
            anchors.extend(xs.iter().cloned());
        }
        if let Some(Value::Arr(xs)) = p.member("candidates") {
            candidates.extend(xs.iter().cloned());
        }
    }
    let mut members = vec![("anchors".to_string(), Value::Arr(anchors)), ("basis".to_string(), basis)];
    if !candidates.is_empty() {
        members.push(("candidates".to_string(), Value::Arr(candidates)));
    }
    Value::Obj(members)
}

pub fn ask_core_about_basis(basis_bytes: &[u8]) -> Option<Value> {
    let mut doc = Vec::new();
    doc.extend_from_slice(b"{\"anchors\":[],\"basis\":");
    doc.extend_from_slice(basis_bytes);
    doc.extend_from_slice(b",\"pile\":[],\"root\":\"0x0000000000000000000000000000000000000000\"}");
    let v = zikaron::json::parse_tests_1_3(&doc).ok()?;
    zikaron::v2::audit::audit_full(&v)?;
    Some(v.member("basis")?.clone())
}

pub fn no_label() -> Value {
    zikaron::v2::audit::no_label()
}

fn addr_of(v: &Value) -> Option<[u8; 20]> {
    let b = hexfmt::decode(v.as_str()?)?;
    if b.len() != 20 {
        return None;
    }
    let mut a = [0u8; 20];
    a.copy_from_slice(&b);
    Some(a)
}

fn h32(v: &W) -> Option<[u8; 32]> {
    let b = hexfmt::decode(v.as_str()?)?;
    if b.len() != 32 {
        return None;
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&b);
    Some(a)
}

pub fn read_basis(basis: &Value) -> Option<Basis> {
    let mut windows = Vec::new();
    for c in basis.member("chains")?.as_arr()? {
        windows.push(Window {
            chain_id: c.member("chainId")?.as_int()?,
            from_block: c.member("fromBlock")?.as_int()?,
            to_block: c.member("toBlock")?.as_int()?,
            registries: c.member("registries")?.as_arr()?.iter().map(addr_of).collect::<Option<_>>()?,
            senders: c.member("senders")?.as_arr()?.iter().map(addr_of).collect::<Option<_>>()?,
        });
    }
    Some(Basis { windows, value: basis.clone() })
}

pub fn hex_quantity(x: u64) -> String {
    format!("0x{x:x}")
}

fn hex20(a: &[u8; 20]) -> String {
    hexfmt::encode(a)
}

pub fn calldata_carries(calldata: &[u8], word: &[u8; 32]) -> bool {
    let mut base = 0usize;
    while base <= 4 {
        let mut off = base;
        while off + 32 <= calldata.len() {
            if &calldata[off..off + 32] == word {
                return true;
            }
            off += 32;
        }
        base += 4;
    }
    false
}

struct Chain<'a> {
    id: u64,
    ep: &'a mut dyn Endpoint,
}

impl<'a> Chain<'a> {
    fn ask(&mut self, method: &str, params: Vec<Value>) -> Result<W, Refusal> {
        crate::patience::ask(self.ep, method, &Value::Arr(params)).map_err(|t| self.refused(method, t))
    }

    fn refused(&self, method: &str, t: Trouble) -> Refusal {
        match t {
            Trouble::Node(e) => Refusal::Unanswered { chain: self.id, asked: Asked::Declined { method: method.to_string(), width: None, said: e } },
            Trouble::NotServed(k) => Refusal::Unanswered { chain: self.id, asked: Asked::NotServed(k) },
            Trouble::Transport(b) => Refusal::Unanswered { chain: self.id, asked: Asked::Broke { method: method.to_string(), broke: b } },
            Trouble::Contradiction(k) => Refusal::Malformed(Shape::Contradiction(k)),
        }
    }

    fn refused_log(&self, params: &Value, t: Trouble, cuttable: bool) -> Refusal {
        let width = (cuttable && matches!(&t, Trouble::Node(_)) && crate::patience::class_of(&t) == crate::patience::Class::Other).then(|| crate::patience::log_blocks("eth_getLogs", params)).flatten();
        if let (Some(_), Trouble::Node(words)) = (width, &t) {
            crate::action::heard(&self.ep.place(), words);
        }
        match self.refused("eth_getLogs", t) {
            Refusal::Unanswered { chain, asked: Asked::Declined { method, said, .. } } => Refusal::Unanswered { chain, asked: Asked::Declined { method, said, width } },
            other => other,
        }
    }

    fn tx_by_hash(&mut self, h: &[u8; 32]) -> Result<Option<Tx>, Refusal> {
        let v = self.ask("eth_getTransactionByHash", vec![Value::Str(hexfmt::encode(h))])?;
        if v.is_null() {
            return Ok(None);
        }
        match tx::read(&v) {
            Ok(t) if t.hash != *h => Err(Refusal::TxNotItself(hexfmt::encode(h))),
            Ok(t) => Ok(Some(t)),
            Err(tx::Bad::HashMismatch) => Err(Refusal::TxNotItself(hexfmt::encode(h))),
            Err(tx::Bad::Shape(w)) => Err(Refusal::Malformed(Shape::TxLacks { part: w.to_string(), tx: hexfmt::encode(h) })),
        }
    }

    fn status_one(&mut self, h: &[u8; 32]) -> Result<bool, Refusal> {
        if let Some(one) = crate::ahead::status_at(self.id, h) {
            return Ok(one);
        }
        let v = self.ask("eth_getTransactionReceipt", vec![Value::Str(hexfmt::encode(h))])?;
        if v.is_null() {
            return Err(Refusal::Unanswered { chain: self.id, asked: Asked::NoReceipt(hexfmt::encode(h)) });
        }
        Ok(tx::qty_u64(&v, "status") == Some(1))
    }

    fn block_timestamp(&mut self, n: u64) -> Result<u64, Refusal> {
        if let Some(t) = crate::ahead::time_at(self.id, n) {
            return Ok(t);
        }
        let v = self.ask("eth_getBlockByNumber", block_params(n))?;
        tx::qty_u64(&v, "timestamp").ok_or(Refusal::Malformed(Shape::NoTimestamp(n)))
    }

    fn verdict_at(&mut self, who: &[u8; 20], block: u64) -> Result<Verdict, Refusal> {
        let mut coded = false;
        if block > 0 {
            match self.code_at(who, block - 1)? {
                None => return Ok(Verdict::Unproven),
                Some(c) => coded |= c,
            }
        }
        match self.code_at(who, block)? {
            None => return Ok(Verdict::Unproven),
            Some(c) => coded |= c,
        }
        Ok(if coded { Verdict::Void } else { Verdict::Counted })
    }

    fn code_at(&mut self, who: &[u8; 20], block: u64) -> Result<Option<bool>, Refusal> {
        if let Some(coded) = crate::ahead::code_at(self.id, who, block) {
            return Ok(Some(coded));
        }
        code_at(self.ep, self.id, who, block)
    }
}

pub fn code_at(ep: &mut dyn Endpoint, chain: u64, who: &[u8; 20], block: u64) -> Result<Option<bool>, Refusal> {
    let params = vec![Value::Str(hex20(who)), Value::Str(hex_quantity(block))];
    match crate::patience::ask(ep, "eth_getCode", &Value::Arr(params)) {
        Ok(v) => {
            let b = v.as_str().and_then(hexfmt::decode).ok_or(Refusal::Malformed(Shape::CodeNotBytes))?;
            Ok(Some(!b.is_empty()))
        }
        Err(Trouble::Node(_)) => Ok(None),
        Err(Trouble::NotServed(k)) => Err(Refusal::Unanswered { chain, asked: Asked::NotServed(k) }),
        Err(Trouble::Transport(b)) => Err(Refusal::Unanswered { chain, asked: Asked::Broke { method: "eth_getCode".into(), broke: b } }),
        Err(Trouble::Contradiction(k)) => Err(Refusal::Malformed(Shape::Contradiction(k))),
    }
}

pub fn run(
    basis_bytes: &[u8],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
) -> Result<Result<Scanned, Value>, Refusal> {
    crate::seam();
    noting(basis_bytes, endpoints, &Known::default(), &[], &mut Vec::new(), &Pick::All).map(|r| r.map(|(s, _, _)| s))
}

pub type RecordKey = (u64, u64, [u8; 32], [u8; 32]);

pub type FactKey = (u64, [u8; 32], [u8; 32], u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    pub block_number: u64,
    pub sender: [u8; 20],
    pub hash: [u8; 32],
    pub emitter: [u8; 20],
    pub block_timestamp: u64,
    pub verdict: Verdict,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Known(pub std::collections::BTreeMap<FactKey, Fact>, pub std::collections::BTreeMap<FactKey, Fact>);

pub type Sightings = Vec<(FactKey, Fact)>;

fn fact_key(chain: u64, log: &W, tx: &[u8; 32]) -> Option<FactKey> {
    let block = log.member("blockHash").and_then(h32)?;
    let index = log.member("logIndex").and_then(tx::hex_qty)?;
    Some((chain, block, *tx, index))
}

pub type Emitters = std::collections::BTreeMap<RecordKey, std::collections::BTreeSet<[u8; 20]>>;

pub fn record_key(a: &AnchorRec) -> RecordKey {
    (a.chain_id, a.block_number, a.tx, a.hash)
}

pub fn run_noting(
    basis_bytes: &[u8],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
) -> Result<Result<(Scanned, Emitters), Value>, Refusal> {
    crate::seam();
    noting(basis_bytes, endpoints, &Known::default(), &[], &mut Vec::new(), &Pick::All).map(|r| r.map(|(s, e, _)| (s, e)))
}

pub fn run_knowing(
    basis_bytes: &[u8],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
    known: &Known,
) -> Result<Result<(Scanned, Emitters, Sightings), Value>, Refusal> {
    crate::seam();
    noting(basis_bytes, endpoints, known, &[], &mut Vec::new(), &Pick::All)
}

pub fn run_knowing_picked(
    basis_bytes: &[u8],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
    known: &Known,
    pick: &Pick,
) -> Result<Result<(Scanned, Emitters, Sightings), Value>, Refusal> {
    crate::seam();
    noting(basis_bytes, endpoints, known, &[], &mut Vec::new(), pick)
}

pub fn run_settling(
    basis_bytes: &[u8],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
    known: &Known,
    nodes: &[(u64, String)],
) -> Result<Result<(Scanned, Emitters, Sightings, Vec<crate::settled::Settling>), Value>, Refusal> {
    run_picked(basis_bytes, endpoints, known, nodes, &Pick::All)
}

pub fn run_picked(
    basis_bytes: &[u8],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
    known: &Known,
    nodes: &[(u64, String)],
    pick: &Pick,
) -> Result<Result<(Scanned, Emitters, Sightings, Vec<crate::settled::Settling>), Value>, Refusal> {
    crate::seam();
    let mut out = Vec::new();
    noting(basis_bytes, endpoints, known, nodes, &mut out, pick).map(|r| r.map(|(s, e, k)| (s, e, k, out)))
}

fn noting(
    basis_bytes: &[u8],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
    known: &Known,
    nodes: &[(u64, String)],
    settling: &mut Vec<crate::settled::Settling>,
    pick: &Pick,
) -> Result<Result<(Scanned, Emitters, Sightings), Value>, Refusal> {
    let Some(basis_value) = ask_core_about_basis(basis_bytes) else {
        return Ok(Err(no_label()));
    };
    let basis = read_basis(&basis_value).ok_or(Refusal::Malformed(Shape::Basis))?;

    let mut chains: Vec<u64> = basis.windows.iter().map(|w| w.chain_id).collect();
    chains.sort_unstable();
    chains.dedup();

    let mut anchors: Vec<AnchorRec> = Vec::new();
    let mut candidates: Vec<Claim> = Vec::new();
    let mut emitters = Emitters::new();
    let mut seen = Sightings::new();
    let t0 = topic0();

    for id in chains {
        let idx = endpoints
            .iter()
            .position(|(c, _)| *c == id)
            .ok_or(Refusal::NoEndpoint(id))?;
        let ep: &mut dyn Endpoint = &mut *endpoints[idx].1;
        let mut chain = Chain { id, ep };

        let served = chain.ask("eth_chainId", vec![])?;
        if !endpoint_serves(&served, id) {
            return Err(Refusal::ChainIdMismatch { declared: id, served: crate::wire::write(&served) });
        }

        let list = nodes.iter().find(|(c, _)| *c == id).map(|(_, l)| l.as_str());
        let filters: Vec<(LogFilter, u64, u64)> =
            basis.windows.iter().filter(|w| w.chain_id == id && !w.registries.is_empty() && !w.senders.is_empty()).map(|w| (anchor_filter(w, &t0), w.from_block, w.to_block)).collect();
        let reads: Vec<(&LogFilter, u64, u64)> = filters.iter().map(|(f, a, b)| (f, *a, *b)).collect();
        if reads.len() >= 2 {
            let asks = logs_ahead_asks(&*chain.ep, id, &reads, list);
            crate::ahead::ask_ahead(&mut *chain.ep, &asks);
        }

        for w in basis.windows.iter().filter(|w| w.chain_id == id) {
            if w.registries.is_empty() || w.senders.is_empty() {
                continue;
            }
            let over = match nodes.iter().find(|(c, _)| *c == id) {
                Some((_, list)) => Over::Agreed { nodes: list, out: &mut *settling },
                None => Over::Alone,
            };
            let logs = paged(&mut chain, &anchor_filter(w, &t0), w.from_block, w.to_block, over)?;
            let (logs, untested): (Vec<W>, Vec<W>) = logs.into_iter().partition(|l| pick.tests(id, w, l, &t0));
            candidates.extend(untested.iter().filter_map(|l| claim_of(id, w, l, &t0)));
            let (mut wants, mut receipts): (Vec<crate::ahead::Want>, Vec<[u8; 32]>) = (Vec::new(), Vec::new());
            for l in &logs {
                match wanted(chain.id, w, l, &t0, known) {
                    Some(Wanted::All(x)) => wants.push(x),
                    Some(Wanted::Receipt(tx)) if !receipts.contains(&tx) => receipts.push(tx),
                    _ => {}
                }
            }
            let share = crate::ahead::Share::of(&chain.ep.place(), list);
            crate::ahead::read_ahead_receipts(&mut *chain.ep, chain.id, &wants, &receipts, share);
            for log in &logs {
                let Some(rec) = registry_record(&mut chain, w, log, &t0, known, &mut seen)? else { continue };
                if let Some(at) = emitter_of(log) {
                    emitters.entry(record_key(&rec)).or_default().insert(at);
                }
                anchors.push(rec);
            }
        }
    }

    let anchors = zikaron::audit::ordered(anchors, |a, b| {
        (a.chain_id, a.block_number, a.tx, a.hash).cmp(&(b.chain_id, b.block_number, b.tx, b.hash))
    });
    let deduped = dedupe(anchors);
    candidates.sort();
    candidates.dedup();

    Ok(Ok((Scanned { anchors: deduped, basis: basis_value, candidates }, emitters, seen)))
}

pub fn served_single_chains(basis_bytes: &[u8], nodes: &[(u64, String)]) -> Vec<u64> {
    if !crate::settled::keeps() {
        return Vec::new();
    }
    let Some(basis) = zikaron::json::parse(basis_bytes).ok().as_ref().and_then(read_basis) else { return Vec::new() };
    let t0 = topic0();
    let mut out: Vec<u64> = basis
        .windows
        .iter()
        .filter(|w| !w.registries.is_empty() && !w.senders.is_empty())
        .filter_map(|w| {
            let (_, list) = nodes.iter().find(|(c, _)| *c == w.chain_id)?;
            subject_groups(&anchor_filter(w, &t0))
                .iter()
                .any(|g| {
                    let key = crate::settled::Key { chain: w.chain_id, nodes: crate::settled::spell_nodes(list), filter: filter_key(g) };
                    crate::settled::entry(&key).is_some_and(|e| e.single_source && e.from <= w.from_block)
                })
                .then_some(w.chain_id)
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn emitter_of(log: &W) -> Option<[u8; 20]> {
    let b = log.member("address").and_then(|a| a.as_str()).and_then(hexfmt::decode)?;
    b.try_into().ok()
}

fn padded(a: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(a);
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFilter {
    pub addresses: Vec<[u8; 20]>,
    pub topics: Vec<Topic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Topic {
    Any,
    One([u8; 32]),
    AnyOf(Vec<[u8; 32]>),
}

impl LogFilter {
    pub fn at(&self, from: u64, to: u64) -> Value {
        let topic = |t: &Topic| match t {
            Topic::Any => Value::Null,
            Topic::One(x) => Value::Str(hexfmt::encode(x)),
            Topic::AnyOf(xs) => Value::Arr(xs.iter().map(|x| Value::Str(hexfmt::encode(x))).collect()),
        };
        Value::Obj(vec![
            ("address".into(), Value::Arr(self.addresses.iter().map(|r| Value::Str(hex20(r))).collect())),
            ("fromBlock".into(), Value::Str(hex_quantity(from))),
            ("toBlock".into(), Value::Str(hex_quantity(to))),
            ("topics".into(), Value::Arr(self.topics.iter().map(topic).collect())),
        ])
    }
}

pub const SUBJECT_GROUP: usize = 32;

pub fn subject_groups(filter: &LogFilter) -> Vec<LogFilter> {
    let Some(at) = filter.topics.iter().position(|t| matches!(t, Topic::AnyOf(xs) if xs.len() > SUBJECT_GROUP)) else { return vec![filter.clone()] };
    let Topic::AnyOf(xs) = &filter.topics[at] else { return vec![filter.clone()] };
    let mut once: Vec<[u8; 32]> = Vec::new();
    for x in xs {
        if !once.contains(x) {
            once.push(*x);
        }
    }
    if once.len() <= SUBJECT_GROUP {
        return vec![filter.clone()];
    }
    once.chunks(SUBJECT_GROUP)
        .map(|g| {
            let mut f = filter.clone();
            f.topics[at] = Topic::AnyOf(g.to_vec());
            f
        })
        .collect()
}

fn merged(parts: Vec<Vec<W>>) -> Vec<W> {
    let mut all: Vec<W> = parts.into_iter().flatten().collect();
    let index = |l: &W| l.member("logIndex").and_then(crate::tx::hex_qty).unwrap_or(0);
    all.sort_by_key(|l| (crate::settled::block_of(l).unwrap_or(0), index(l)));
    all
}

pub fn lineage_filter(registries: &[[u8; 20]], senders: &[[u8; 20]]) -> LogFilter {
    LogFilter { addresses: registries.to_vec(), topics: vec![Topic::One(topic0()), Topic::AnyOf(senders.iter().map(padded).collect())] }
}

fn anchor_filter(w: &Window, t0: &[u8; 32]) -> LogFilter {
    LogFilter { addresses: w.registries.clone(), topics: vec![Topic::One(*t0), Topic::AnyOf(w.senders.iter().map(padded).collect())] }
}

pub fn logs_over_window(ep: &mut dyn Endpoint, chain: u64, filter: &LogFilter, from: u64, to: u64) -> Result<Vec<W>, Refusal> {
    crate::seam();
    let place = ep.place();
    let mut got = [paged(&mut Chain { id: chain, ep: &mut *ep }, filter, from, to, Over::Alone)];
    crate::action::cut_and_ask(&mut got, |_, _| vec![place.clone()], |_| false, |g| g.as_ref().err().is_some_and(Refusal::cut), |_, _| vec![paged(&mut Chain { id: chain, ep }, filter, from, to, Over::Alone)]);
    let [got] = got;
    got
}

pub fn logs_judged_over(ep: &mut dyn Endpoint, chain: u64, filter: &LogFilter, from: u64, to: u64, nodes: &str) -> Result<Vec<W>, Refusal> {
    crate::seam();
    paged(&mut Chain { id: chain, ep }, filter, from, to, Over::Judged { nodes })
}

pub fn log_asks(ep: &dyn Endpoint, chain: u64, filter: &LogFilter, from: u64, to: u64, nodes: Option<&str>) -> Vec<crate::rpc::Ask<'static>> {
    if to < from {
        return Vec::new();
    }
    let groups = subject_groups(filter);
    if groups.len() > 1 {
        return groups.iter().flat_map(|g| log_asks(ep, chain, g, from, to, nodes)).collect();
    }
    let place = ep.place();
    match reading_on(&place, chain, filter, from, nodes).and_then(|r| r.kept) {
        Some(e) => kept_asks(&place, &e, filter, from, to),
        None => whole_ask(&place, filter, from, to),
    }
}

pub fn logs_ahead<'e>(nodes: &mut [&mut (dyn Endpoint + Send + 'e)], chain: u64, reads: &[(&LogFilter, u64, u64)], nodes_list: Option<&str>) {
    crate::seam();
    if reads.len() < 2 || crate::action::current().is_none() {
        return;
    }
    let mut items: Vec<(String, Box<dyn Endpoint + Send>, Vec<crate::rpc::Ask<'static>>)> = Vec::new();
    let mut here: Vec<usize> = Vec::new();
    for (i, ep) in nodes.iter_mut().enumerate() {
        let asks = logs_ahead_asks(&**ep, chain, reads, nodes_list);
        match ep.another() {
            Some(other) => items.push((ep.place(), other, asks)),
            None => here.push(i),
        }
    }
    let done = crate::endpoints::until_enough(
        items,
        |(place, mut ep, asks)| {
            crate::ahead::ask_ahead(ep.as_mut(), &asks);
            place
        },
        String::new,
        |slots: &[Option<String>]| crate::endpoints::distinct_places(slots.iter().flatten().filter(|p| !p.is_empty()).map(String::as_str)) >= 2,
    );
    if crate::endpoints::distinct_places(done.iter().flatten().filter(|p| !p.is_empty()).map(String::as_str)) >= 2 {
        return;
    }
    for i in here {
        let asks = logs_ahead_asks(&*nodes[i], chain, reads, nodes_list);
        crate::ahead::ask_ahead(&mut *nodes[i], &asks);
    }
}

fn logs_ahead_asks(ep: &dyn Endpoint, chain: u64, reads: &[(&LogFilter, u64, u64)], nodes: Option<&str>) -> Vec<crate::rpc::Ask<'static>> {
    let mut asks: Vec<crate::rpc::Ask<'static>> = Vec::new();
    for (filter, from, to) in reads {
        for a in log_asks(ep, chain, filter, *from, *to, nodes) {
            if !asks.contains(&a) {
                asks.push(a);
            }
        }
    }
    asks
}

fn logs_question(filter: &LogFilter, from: u64, to: u64) -> Value {
    Value::Arr(vec![filter.at(from, to)])
}

fn block_params(n: u64) -> Vec<Value> {
    vec![Value::Str(hex_quantity(n)), Value::Bool(false)]
}

fn whole_ask(place: &str, filter: &LogFilter, from: u64, to: u64) -> Vec<crate::rpc::Ask<'static>> {
    if crate::patience::answered_width(place).is_some_and(|w| w < span_of(from, to)) {
        return Vec::new();
    }
    vec![("eth_getLogs", logs_question(filter, from, to))]
}

fn span_of(from: u64, to: u64) -> u64 {
    to.saturating_sub(from).saturating_add(1)
}

struct ReadingOn {
    key: crate::settled::Key,
    own: Option<crate::settled::Entry>,
    kept: Option<crate::settled::Entry>,
}

fn reading_on(place: &str, chain: u64, filter: &LogFilter, from: u64, nodes: Option<&str>) -> Option<ReadingOn> {
    use crate::settled::{self, Key};
    if !settled::keeps() {
        return None;
    }
    let list = match nodes {
        Some(n) => n.to_string(),
        None => settled::nodes_of([place]),
    };
    let key = Key { chain, nodes: settled::spell_nodes(&list), filter: filter_key(filter) };
    let own = settled::entry(&key).filter(|e| e.from <= from);
    let lent = if own.is_none() && nodes.is_none() { settled::lent(&key, from) } else { None };
    let kept = own.clone().or(lent);
    Some(ReadingOn { key, own, kept })
}

pub fn logs_agreed(nodes: Vec<Box<dyn Endpoint + Send>>, chain: u64, filter: &LogFilter, from: u64, to: u64) -> AgreedLogs {
    crate::seam();
    let places: Vec<String> = nodes.iter().map(|ep| ep.place()).collect();
    let list = crate::settled::nodes_of(places.iter().map(String::as_str));
    type Got = (Box<dyn Endpoint + Send>, Result<(Vec<W>, Vec<crate::settled::Settling>), Refusal>);
    let ask = |nodes: Vec<Box<dyn Endpoint + Send>>, places: Vec<String>| -> Vec<Option<Got>> {
        let (filter, list) = (filter.clone(), list.clone());
        crate::endpoints::until_enough(
            nodes,
            move |mut ep| {
                let mut out = Vec::new();
                let got = paged(&mut Chain { id: chain, ep: ep.as_mut() }, &filter, from, to, Over::Agreed { nodes: &list, out: &mut out }).map(|l| (l, out));
                Some((ep, got))
            },
            || None,
            move |slots: &[Option<Option<Got>>]| {
                let read: Vec<(&str, Vec<u8>)> = places.iter().zip(slots).filter_map(|(p, g)| match g {
                    Some(Some((_, Ok((logs, _))))) => Some((p.as_str(), logs_bytes(logs))),
                    _ => None,
                }).collect();
                crate::judge::two_places_alike(&read) || crate::action::read_while_cut(read.iter().map(|(p, _)| *p))
            },
        )
        .into_iter()
        .map(Option::flatten)
        .collect()
    };
    let mut got: Vec<Option<Got>> = ask(nodes, places.clone());
    crate::action::cut_and_ask(
        &mut got,
        |i, _| vec![places[i].clone()],
        |g| matches!(g, Some((_, Ok(_)))),
        |g| matches!(g, Some((_, Err(r))) if r.cut()),
        |idx, got| {
            let again: Vec<Box<dyn Endpoint + Send>> = idx.iter().filter_map(|i| got[*i].take().map(|(ep, _)| ep)).collect();
            let again_places: Vec<String> = idx.iter().map(|i| places[*i].clone()).collect();
            ask(again, again_places)
        },
    );
    let mut read: Vec<(String, Vec<W>, Vec<crate::settled::Settling>)> = Vec::new();
    let mut refused: Vec<(String, Refusal)> = Vec::new();
    for (g, place) in got.into_iter().zip(places) {
        match g {
            Some((_, Ok((logs, out)))) => read.push((place, logs, out)),
            Some((_, Err(r))) => refused.push((place, r)),
            None => {}
        }
    }
    let bytes: Vec<(&str, Vec<u8>)> = read.iter().map(|(p, l, _)| (p.as_str(), logs_bytes(l))).collect();
    let alike = bytes.iter().find(|(_, b)| crate::endpoints::distinct_places(bytes.iter().filter(|(_, x)| x == b).map(|(p, _)| *p)) >= 2).map(|(_, b)| b.clone());
    if let Some(b) = alike {
        let kept: Vec<(String, Vec<W>, Vec<crate::settled::Settling>)> = read.into_iter().filter(|(_, l, _)| logs_bytes(l) == b).collect();
        let runs: Vec<Vec<crate::settled::Settling>> = kept.iter().map(|(_, _, o)| o.clone()).collect();
        crate::settled::settle(&runs, &[]);
        let answered = kept.len();
        return AgreedLogs { answered, agreed: true, logs: kept.into_iter().next().map(|(_, l, _)| l).unwrap_or_default(), refused: Vec::new() };
    }
    let alike = read.iter().all(|(_, l, _)| crate::settled::same_logs(l, &read[0].1));
    let agreed = crate::endpoints::distinct_places(read.iter().map(|(p, _, _)| p.as_str())) >= 2 && alike;
    if !read.is_empty() && alike {
        let runs: Vec<Vec<crate::settled::Settling>> = read.iter().map(|(_, _, o)| o.clone()).collect();
        crate::settled::settle(&runs, &[]);
    }
    AgreedLogs { answered: read.len(), agreed, logs: read.into_iter().next().map(|(_, l, _)| l).unwrap_or_default(), refused }
}

fn logs_bytes(logs: &[W]) -> Vec<u8> {
    logs.iter().flat_map(|l| crate::wire::write(&crate::wire::project(l, &crate::settled::LOG_FACTS)).into_bytes().into_iter().chain(std::iter::once(b'\n'))).collect()
}

#[derive(Debug)]
pub struct AgreedLogs {
    pub logs: Vec<W>,
    pub answered: usize,
    pub agreed: bool,
    pub refused: Vec<(String, Refusal)>,
}

enum Over<'a> {
    Alone,
    Judged { nodes: &'a str },
    Agreed { nodes: &'a str, out: &'a mut Vec<crate::settled::Settling> },
}

impl Over<'_> {
    fn list(&self) -> Option<&str> {
        match self {
            Over::Alone => None,
            Over::Judged { nodes } | Over::Agreed { nodes, .. } => Some(nodes),
        }
    }
}

fn filter_key(filter: &LogFilter) -> String {
    let at = filter.at(0, 0);
    let kept = match at {
        Value::Obj(ms) => Value::Obj(ms.into_iter().filter(|(k, _)| k == "address" || k == "topics").collect()),
        other => other,
    };
    String::from_utf8(zikaron::json::canon_bytes(&kept)).unwrap_or_default()
}

fn paged(chain: &mut Chain, filter: &LogFilter, from: u64, to: u64, over: Over) -> Result<Vec<W>, Refusal> {
    use crate::settled::{self, Entry};
    if to < from {
        return Ok(Vec::new());
    }
    let groups = subject_groups(filter);
    if groups.len() > 1 {
        let mut over = over;
        let mut parts = Vec::new();
        for g in &groups {
            let each = match &mut over {
                Over::Alone => Over::Alone,
                Over::Judged { nodes } => Over::Judged { nodes },
                Over::Agreed { nodes, out } => Over::Agreed { nodes, out: &mut **out },
            };
            parts.push(paged(chain, g, from, to, each)?);
        }
        return Ok(merged(parts));
    }
    let place = chain.ep.place();
    let Some(ReadingOn { key, own, kept }) = reading_on(&place, chain.id, filter, from, over.list()) else {
        return asked(chain, filter, from, to);
    };
    if let Some(e) = &kept {
        let asks = kept_asks(&place, e, filter, from, to);
        crate::ahead::ask_ahead(chain.ep, &asks);
    }
    let held = kept.and_then(|e| rechecked(chain, e));
    let in_range = |l: &W| settled::block_of(l).is_some_and(|b| (from..=to).contains(&b));
    let (mut logs, ask_from) = match &held {
        Some(e) => (e.logs.iter().filter(|l| in_range(l)).cloned().collect::<Vec<W>>(), from.max(e.settled.saturating_add(1))),
        None => (Vec::new(), from),
    };
    let fetched = if ask_from <= to { asked(chain, filter, ask_from, to)? } else { Vec::new() };
    logs.extend(fetched.iter().cloned());
    let settle_to = to.saturating_sub(settled::MARGIN);
    let (base_from, base_logs, base_settled) = match &held {
        Some(e) => (e.from, e.logs.clone(), Some(e.settled)),
        None => (from, Vec::new(), None),
    };
    let carried = base_settled.is_none_or(|s| ask_from == s.saturating_add(1) && settle_to > s);
    let proposed = if carried && settle_to >= base_from {
        let mut kept = base_logs;
        let mut readable = true;
        for l in &fetched {
            match settled::block_of(l) {
                Some(b) if b <= settle_to => kept.push(l.clone()),
                Some(_) => {}
                None => readable = false,
            }
        }
        readable.then_some(Entry { from: base_from, settled: settle_to, logs: kept, single_source: true })
    } else {
        None
    };
    let base = held.as_ref().filter(|_| own.is_some()).map(|e| (e.from, e.settled));
    match over {
        Over::Alone => {
            if let Some(e) = proposed {
                settled::keep_on(&key, e, base);
            }
        }
        Over::Agreed { out, .. } => out.push(settled::Settling { key, entry: proposed, served_single: held.is_some_and(|e| e.single_source), base }),
        Over::Judged { .. } => {}
    }
    Ok(logs)
}

fn kept_asks(place: &str, e: &crate::settled::Entry, filter: &LogFilter, from: u64, to: u64) -> Vec<crate::rpc::Ask<'static>> {
    let mut asks = Vec::new();
    if let Some(last) = e.logs.iter().filter(|l| l.member("blockHash").and_then(W::as_str).is_some()).filter_map(crate::settled::block_of).max() {
        asks.push(("eth_getBlockByNumber", Value::Arr(block_params(last))));
    }
    let ask_from = from.max(e.settled.saturating_add(1));
    if ask_from <= to {
        asks.extend(whole_ask(place, filter, ask_from, to));
    }
    asks
}

fn rechecked(chain: &mut Chain, e: crate::settled::Entry) -> Option<crate::settled::Entry> {
    let mut blocks: Vec<(u64, String)> = e.logs.iter().filter_map(|l| Some((crate::settled::block_of(l)?, l.member("blockHash")?.as_str()?.to_ascii_lowercase()))).collect();
    blocks.sort();
    blocks.dedup();
    let Some(last) = blocks.last().map(|(n, _)| *n) else { return Some(e) };
    for (n, hash) in blocks.iter().rev() {
        let said = chain.ask("eth_getBlockByNumber", block_params(*n)).ok()?;
        if said.member("hash").and_then(W::as_str).is_some_and(|h| h.eq_ignore_ascii_case(hash)) {
            if *n == last {
                return Some(e);
            }
            let logs = e.logs.iter().filter(|l| crate::settled::block_of(l).is_some_and(|b| b <= *n)).cloned().collect();
            return Some(crate::settled::Entry { settled: *n, logs, ..e });
        }
    }
    None
}

fn asked(chain: &mut Chain, filter: &LogFilter, from: u64, to: u64) -> Result<Vec<W>, Refusal> {
    if to < from {
        return Ok(Vec::new());
    }
    let place = chain.ep.place();
    let span = span_of(from, to);
    let known = crate::patience::answered_width(&place).filter(|w| *w < span);
    if known.is_none() && !crate::action::splits(&place) {
        return match crate::patience::ask_again_once(chain.ep, "eth_getLogs", &logs_question(filter, from, to)) {
            Ok(answered) => answered.as_arr().map(<[W]>::to_vec).ok_or(Refusal::Unanswered { chain: chain.id, asked: Asked::NotLogs }),
            Err(t) => Err(chain.refused_log(&logs_question(filter, from, to), t, true)),
        };
    }
    match known {
        Some(w) => pieces(chain, filter, from, to, w, 0),
        None => pieces(chain, filter, from, to, span.div_ceil(2), 1),
    }
}

fn pieces(chain: &mut Chain, filter: &LogFilter, from: u64, to: u64, width: u64, asked_before: u64) -> Result<Vec<W>, Refusal> {
    let place = chain.ep.place();
    let cap = crate::action::asks_cap();
    let (mut width, mut used, mut at) = (width.max(1), asked_before, from);
    let mut said = crate::action::heard_from(&place);
    let mut out = Vec::new();
    while at <= to {
        if crate::endpoints::stopped() {
            return Err(Refusal::Unanswered { chain: chain.id, asked: Asked::Short });
        }
        let left = span_of(at, to);
        if used.saturating_add(left.div_ceil(width)) > cap {
            return Err(Refusal::Unanswered { chain: chain.id, asked: Asked::OverAsks { cap, said } });
        }
        let end = at.saturating_add(width.min(left) - 1);
        let q = logs_question(filter, at, end);
        used += 1;
        match crate::patience::ask(chain.ep, "eth_getLogs", &q) {
            Ok(answered) => {
                out.extend(answered.as_arr().map(<[W]>::to_vec).ok_or(Refusal::Unanswered { chain: chain.id, asked: Asked::NotLogs })?);
                if end < to {
                    crate::patience::keep_answered(&place, width);
                }
                if end == u64::MAX {
                    break;
                }
                at = end + 1;
            }
            Err(Trouble::Node(words)) if end > at && crate::patience::class_of(&Trouble::Node(words.clone())) == crate::patience::Class::Other => {
                crate::action::heard(&place, &words);
                said = Some(words);
                width = span_of(at, end).div_ceil(2);
            }
            Err(t) => return Err(chain.refused_log(&q, t, false)),
        }
    }
    Ok(out)
}

#[derive(Clone, Debug)]
pub struct LogFacts {
    pub claimed: [u8; 20],
    pub hash: [u8; 32],
    pub tx: [u8; 32],
    pub block_number: u64,
}

pub fn anchored_log(log: &W, t0: &[u8; 32]) -> Option<LogFacts> {
    let topics = log.member("topics")?.as_arr()?;
    if topics.len() != 3 {
        return None;
    }
    if log.member("data")?.as_str()? != "0x" {
        return None;
    }
    let (a, b, c) = (h32(&topics[0])?, h32(&topics[1])?, h32(&topics[2])?);
    if &a != t0 || b[..12].iter().any(|x| *x != 0) {
        return None;
    }
    let mut claimed = [0u8; 20];
    claimed.copy_from_slice(&b[12..]);
    Some(LogFacts {
        claimed,
        hash: c,
        tx: log.member("transactionHash").and_then(h32)?,
        block_number: log.member("blockNumber").and_then(tx::hex_qty)?,
    })
}

fn no_tx(tx: &[u8; 32]) -> Refusal {
    Refusal::Malformed(Shape::TxLacks { part: "transaction".into(), tx: hexfmt::encode(tx) })
}

pub fn author_key(ep: &mut dyn Endpoint, chain: u64, registry: &[u8; 20], author: &[u8; 20], from: u64, to: u64) -> Result<Option<[u8; 65]>, Refusal> {
    crate::seam();
    let t0 = topic0();
    let emitters = crate::revoke::registries(chain, &[*registry]);
    let filter = LogFilter { addresses: emitters.clone(), topics: vec![Topic::One(t0), Topic::One(padded(author))] };
    let mut c = Chain { id: chain, ep };
    for log in paged(&mut c, &filter, from, to, Over::Alone)? {
        let Some(f) = anchored_log(&log, &t0) else { continue };
        if &f.claimed != author || !emitter_of(&log).is_some_and(|e| emitters.contains(&e)) || f.block_number < from || f.block_number > to {
            continue;
        }
        let id = c.id;
        let Some(t) = crate::action::proven(id, &f.tx, || c.tx_by_hash(&f.tx))? else { return Err(no_tx(&f.tx)) };
        if t.sender != Some(*author) || t.chain_id != Some(chain) {
            continue;
        }
        if let Some(key) = t.pubkey.filter(|k| cryptox::address_of_pubkey(k) == *author) {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

pub fn tx_proved(ep: &mut dyn Endpoint, chain: u64, hash: &[u8; 32]) -> Result<Option<Tx>, Refusal> {
    crate::seam();
    let mut c = Chain { id: chain, ep };
    crate::action::proven(chain, hash, || c.tx_by_hash(hash))
}

pub fn accessions_under(ep: &mut dyn Endpoint, chain: u64, book: &[u8; 20], root: &[u8; 20], from: u64, to: u64) -> Result<Vec<Value>, Refusal> {
    crate::seam();
    let mut c = Chain { id: chain, ep };
    let served = c.ask("eth_chainId", vec![])?;
    if !endpoint_serves(&served, chain) {
        return Err(Refusal::ChainIdMismatch { declared: chain, served: crate::wire::write(&served) });
    }
    if to < from {
        return Ok(Vec::new());
    }
    let t0 = cryptox::keccak256(crate::revoke::ACCEDED_SIGNATURE.as_bytes());
    let filter = LogFilter { addresses: vec![*book], topics: vec![Topic::One(t0), Topic::Any, Topic::Any, Topic::One(padded(root))] };
    let logs = asked(&mut c, &filter, from, to)?;
    let mut asks: Vec<crate::rpc::Ask<'_>> = Vec::new();
    let under_root = |log: &W| emitter_of(log) == Some(*book) && form_topics(log, &t0, 4).and_then(|t| address_topic(&t[3])) == Some(*root);
    for log in logs.iter().filter(|l| under_root(l)) {
        let (Some(txh), Some((n, _, _))) = (log.member("transactionHash").and_then(h32), position_of(log)) else { continue };
        if (from..=to).contains(&n) {
            asks.push(("eth_getTransactionReceipt", by_hash(&txh)));
            asks.push(("eth_getBlockByNumber", Value::Arr(block_params(n))));
        }
    }
    asks.sort_by(|a, b| (a.0, zikaron::json::canon_bytes(&a.1)).cmp(&(b.0, zikaron::json::canon_bytes(&b.1))));
    asks.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    crate::ahead::ask_ahead(c.ep, &asks);
    let mut recs: Vec<((u64, u64, u64), Value)> = Vec::new();
    for log in &logs {
        if emitter_of(log) != Some(*book) {
            continue;
        }
        let Some(topics) = form_topics(log, &t0, 4) else { continue };
        let (Some(holder), Some(to_key), Some(of)) = (address_topic(&topics[1]), address_topic(&topics[2]), address_topic(&topics[3])) else { continue };
        if of != *root {
            continue;
        }
        let txh = log.member("transactionHash").and_then(h32);
        let (Some(at), Some(txh)) = (position_of(log), txh) else {
            return Err(Refusal::Malformed(Shape::TxLacks { part: "log position".into(), tx: txh.map(|h| hexfmt::encode(&h)).unwrap_or_default() }));
        };
        if at.0 < from || at.0 > to || !c.status_one(&txh)? {
            continue;
        }
        let time = c.block_timestamp(at.0)?;
        recs.push((
            at,
            Value::Obj(vec![
                ("blockNumber".into(), Value::Int(at.0)),
                ("blockTimestamp".into(), Value::Int(time)),
                ("chainId".into(), Value::Int(chain)),
                ("holder".into(), Value::Str(hex20(&holder))),
                ("logIndex".into(), Value::Int(at.2)),
                ("root".into(), Value::Str(hex20(&of))),
                ("to".into(), Value::Str(hex20(&to_key))),
                ("tx".into(), Value::Str(hexfmt::encode(&txh))),
                ("txIndex".into(), Value::Int(at.1)),
            ]),
        ));
    }
    recs.sort_by(|a, b| a.0.cmp(&b.0));
    recs.dedup_by(|a, b| a.0 == b.0);
    Ok(recs.into_iter().map(|(_, v)| v).collect())
}

pub fn window_admits(w: &Window, claimed: &[u8; 20]) -> bool {
    w.senders.contains(claimed)
}

enum Wanted {
    All(crate::ahead::Want),
    Receipt([u8; 32]),
}

fn wanted(chain: u64, w: &Window, log: &W, t0: &[u8; 32], known: &Known) -> Option<Wanted> {
    let f = anchored_log(log, t0)?;
    if f.block_number < w.from_block || f.block_number > w.to_block || !window_admits(w, &f.claimed) {
        return None;
    }
    let emitter: [u8; 20] = emitter_of(log)?;
    if !w.registries.iter().any(|r| *r == emitter) {
        return None;
    }
    let key = fact_key(chain, log, &f.tx);
    let said_then = |k: &Fact| k.block_number == f.block_number && k.sender == f.claimed && k.hash == f.hash && k.emitter == emitter;
    if key.and_then(|k| known.0.get(&k)).is_some_and(said_then) {
        return None;
    }
    if key.and_then(|k| known.1.get(&k)).is_some_and(said_then) {
        return Some(Wanted::Receipt(f.tx));
    }
    let block = log.member("blockHash").and_then(h32)?;
    Some(Wanted::All(crate::ahead::Want { tx: f.tx, block, number: f.block_number, who: f.claimed, hash: f.hash }))
}

fn registry_record(
    chain: &mut Chain,
    w: &Window,
    log: &W,
    t0: &[u8; 32],
    known: &Known,
    seen: &mut Sightings,
) -> Result<Option<AnchorRec>, Refusal> {
    let Some(f) = anchored_log(log, t0) else { return Ok(None) };
    let (claimed, hash, txh, bn) = (f.claimed, f.hash, f.tx, f.block_number);
    if bn < w.from_block || bn > w.to_block {
        return Ok(None);
    }
    let Some(addr) = log.member("address").and_then(|a| a.as_str()).and_then(hexfmt::decode) else {
        return Ok(None);
    };
    if addr.len() != 20 || !w.registries.iter().any(|r| r[..] == addr[..]) {
        return Ok(None);
    }
    let mut emitter = [0u8; 20];
    emitter.copy_from_slice(&addr);
    let key = fact_key(chain.id, log, &txh);
    let said_then = |f: &&Fact| f.block_number == bn && f.sender == claimed && f.hash == hash && f.emitter == emitter;
    if let Some(f) = key.and_then(|k| known.0.get(&k)).filter(said_then) {
        if !window_admits(w, &claimed) {
            return Ok(None);
        }
        return Ok(Some(AnchorRec { chain_id: chain.id, block_number: bn, block_timestamp: f.block_timestamp, tx: txh, sender: claimed, hash, verdict: f.verdict }));
    }
    if let Some((k, f)) = key.and_then(|k| known.1.get(&k).filter(said_then).map(|f| (k, *f))) {
        if !window_admits(w, &claimed) || !chain.status_one(&txh)? {
            return Ok(None);
        }
        crate::ahead::note_proved(k);
        seen.push((k, f));
        return Ok(Some(AnchorRec { chain_id: chain.id, block_number: bn, block_timestamp: f.block_timestamp, tx: txh, sender: claimed, hash, verdict: f.verdict }));
    }

    let _at = crate::ahead::reading_at(log.member("blockHash").and_then(h32));
    let id = chain.id;
    let Some(t) = crate::action::proven(id, &txh, || chain.tx_by_hash(&txh))? else { return Ok(None) };
    if !chain.status_one(&txh)? {
        return Ok(None);
    }
    if !window_admits(w, &claimed) {
        return Ok(None);
    }
    if !calldata_carries(&t.input, &hash) {
        return Ok(None);
    }
    if t.sender != Some(claimed) || t.chain_id != Some(chain.id) || t.to.is_none() {
        return Ok(None);
    }
    let block_timestamp = chain.block_timestamp(bn)?;
    let verdict = chain.verdict_at(&claimed, bn)?;
    if let (Some(k), false) = (key, verdict == Verdict::Unproven) {
        if crate::ahead::proved_here(chain.id, &claimed, bn) {
            crate::ahead::note_proved(k);
        }
        seen.push((k, Fact { block_number: bn, sender: claimed, hash, emitter, block_timestamp, verdict }));
    }
    Ok(Some(AnchorRec {
        chain_id: chain.id,
        block_number: bn,
        block_timestamp,
        tx: txh,
        sender: claimed,
        hash,
        verdict,
    }))
}

fn position_of(log: &W) -> Option<(u64, u64, u64)> {
    Some((log.member("blockNumber").and_then(tx::hex_qty)?, log.member("transactionIndex").and_then(tx::hex_qty)?, log.member("logIndex").and_then(tx::hex_qty)?))
}

fn form_topics(log: &W, t0: &[u8; 32], n: usize) -> Option<Vec<[u8; 32]>> {
    let topics: Vec<[u8; 32]> = log.member("topics")?.as_arr()?.iter().map(h32).collect::<Option<_>>()?;
    (topics.len() == n && &topics[0] == t0 && log.member("data")?.as_str()? == "0x").then_some(topics)
}

fn address_topic(t: &[u8; 32]) -> Option<[u8; 20]> {
    t[..12].iter().all(|x| *x == 0).then(|| t[12..].try_into().ok()).flatten()
}

fn by_position(mut rs: Vec<((u64, u64, u64), Value)>) -> Value {
    rs.sort_by(|a, b| a.0.cmp(&b.0));
    rs.dedup_by(|a, b| a.0 == b.0);
    Value::Arr(rs.into_iter().map(|(_, v)| v).collect())
}

pub fn notice_input(ep: &mut dyn Endpoint, v: &crate::revoke::Venue, to: u64) -> Result<Value, Refusal> {
    crate::seam();
    read_notices(ep, v, to, None).map(|(n, _)| n)
}

pub const REVOCATIONS: &str = "revocations";

#[derive(Debug)]
pub struct NoticesRead {
    pub read: Vec<(usize, Value)>,
    pub refused: Vec<(usize, Refusal)>,
}

pub fn notices_agreed(nodes: Vec<Box<dyn Endpoint + Send>>, v: &crate::revoke::Venue, to: u64) -> NoticesRead {
    crate::seam();
    let places: Vec<String> = nodes.iter().map(|ep| ep.place()).collect();
    let list = crate::settled::nodes_of(places.iter().map(String::as_str));
    let mut out = NoticesRead { read: Vec::new(), refused: Vec::new() };
    let mut runs: Vec<Vec<crate::settled::Settling>> = Vec::new();
    type Got = Result<(Value, Vec<crate::settled::Settling>), Refusal>;
    let spare: Vec<Option<Box<dyn Endpoint + Send>>> = nodes.iter().map(|ep| ep.another()).collect();
    let chain = v.chain;
    let ask = |nodes: Vec<Box<dyn Endpoint + Send>>, at: Vec<String>| -> Vec<Option<Got>> {
        let (venue, list) = (v.clone(), list.clone());
        crate::endpoints::until_enough(
            nodes,
            move |mut ep| read_notices(ep.as_mut(), &venue, to, Some(&list)),
            move || Err(Refusal::Unanswered { chain, asked: Asked::Short }),
            move |slots: &[Option<Got>]| {
                let read: Vec<(&str, Vec<u8>)> = at.iter().zip(slots).filter_map(|(p, g)| match g {
                    Some(Ok((n, _))) => Some((p.as_str(), zikaron::json::canon_bytes(n))),
                    _ => None,
                }).collect();
                crate::judge::two_places_alike(&read) || crate::action::read_while_cut(read.iter().map(|(p, _)| *p))
            },
        )
    };
    let mut got = ask(nodes, places.clone());
    crate::action::cut_and_ask(
        &mut got,
        |i, _| vec![places[i].clone()],
        |g| matches!(g, Some(Ok(_))),
        |g| matches!(g, Some(Err(r)) if r.cut()),
        |idx, got| {
            let (mut eps, mut at, mut slot) = (Vec::new(), Vec::new(), Vec::new());
            for (j, i) in idx.iter().enumerate() {
                if let Some(ep) = spare[*i].as_ref().and_then(|e| e.another()) {
                    eps.push(ep);
                    at.push(places[*i].clone());
                    slot.push(j);
                }
            }
            let mut redo: Vec<Option<Got>> = idx.iter().map(|i| got[*i].take()).collect();
            for (j, g) in slot.into_iter().zip(ask(eps, at)) {
                redo[j] = g;
            }
            redo
        },
    );
    for (i, r) in got.into_iter().enumerate() {
        match r {
            Some(Ok((n, settling))) => {
                out.read.push((i, n));
                runs.push(settling);
            }
            Some(Err(r)) => out.refused.push((i, r)),
            None => {}
        }
    }
    let bytes: Vec<Vec<u8>> = out.read.iter().map(|(_, n)| zikaron::json::canon_bytes(n)).collect();
    let alike = bytes.iter().find(|b| crate::endpoints::distinct_places(out.read.iter().zip(&bytes).filter(|(_, x)| x == b).map(|((i, _), _)| places[*i].as_str())) >= 2).cloned();
    if let Some(b) = alike {
        let keep: Vec<bool> = bytes.iter().map(|x| *x == b).collect();
        let runs: Vec<Vec<crate::settled::Settling>> = runs.into_iter().zip(&keep).filter(|(_, k)| **k).map(|(r, _)| r).collect();
        crate::settled::settle_judged(&runs);
        out.read = out.read.into_iter().zip(keep).filter(|(_, k)| *k).map(|(r, _)| r).collect();
        out.refused.clear();
        return out;
    }
    let alike = out.read.first().is_some_and(|(_, first)| {
        let b = zikaron::json::canon_bytes(first);
        out.read.iter().all(|(_, n)| zikaron::json::canon_bytes(n) == b)
    });
    if alike && out.refused.iter().all(|(_, r)| r.stands_aside()) {
        crate::settled::settle_judged(&runs);
    }
    out
}

type Rec = ((u64, u64, u64), Value);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Notice,
    Accession,
}

impl Form {
    fn word(self) -> &'static str {
        match self {
            Form::Notice => "notice",
            Form::Accession => "accession",
        }
    }
}

fn judged_key(chain: u64, nodes: &str, filter: &LogFilter, form: Form) -> crate::settled::Key {
    let mut ms: Vec<(String, Value)> = match filter.at(0, 0) {
        Value::Obj(ms) => ms.into_iter().filter(|(k, _)| k == "address" || k == "topics").collect(),
        _ => Vec::new(),
    };
    ms.push(("judged".into(), Value::Str(form.word().into())));
    ms.sort_by(|a, b| a.0.cmp(&b.0));
    crate::settled::Key { chain, nodes: crate::settled::spell_nodes(nodes), filter: String::from_utf8(zikaron::json::canon_bytes(&Value::Obj(ms))).unwrap_or_default() }
}

fn record_position(r: &Value) -> Option<(u64, u64, u64)> {
    let at = |k: &str| r.member(k).and_then(Value::as_int);
    Some((at("blockNumber")?, at("txIndex")?, at("logIndex")?))
}

fn read_notices(ep: &mut dyn Endpoint, v: &crate::revoke::Venue, to: u64, nodes: Option<&str>) -> Result<(Value, Vec<crate::settled::Settling>), Refusal> {
    let mut chain = Chain { id: v.chain, ep };
    let served = chain.ask("eth_chainId", vec![])?;
    if !endpoint_serves(&served, v.chain) {
        return Err(Refusal::ChainIdMismatch { declared: v.chain, served: crate::wire::write(&served) });
    }
    let from = v.from_block.min(to);
    let book_from = v.book_from_block.min(to);
    let int = |n: u64| Value::Int(n);
    let s = |x: String| Value::Str(x);
    let mut settling = Vec::new();

    let revoked = cryptox::keccak256(crate::revoke::REVOKED_SIGNATURE.as_bytes());
    let notice_filter = LogFilter { addresses: vec![v.contract], topics: vec![Topic::One(revoked)] };
    let acceded = cryptox::keccak256(crate::revoke::ACCEDED_SIGNATURE.as_bytes());
    let accession_filter = LogFilter { addresses: vec![v.book], topics: vec![Topic::One(acceded)] };

    let place = chain.ep.place();
    let mut asks = form_asks(&place, v.chain, &notice_filter, Form::Notice, from, to, nodes);
    for a in form_asks(&place, v.chain, &accession_filter, Form::Accession, book_from, to, nodes) {
        if !asks.contains(&a) {
            asks.push(a);
        }
    }
    crate::ahead::ask_ahead(chain.ep, &asks);

    let notices = judged_form(&mut chain, &notice_filter, Form::Notice, from, to, nodes, &mut settling, &mut |c, logs| notices_of(c, v, &revoked, from, to, logs))?;
    let accessions = judged_form(&mut chain, &accession_filter, Form::Accession, book_from, to, nodes, &mut settling, &mut |c, logs| accessions_of(c, v, &acceded, book_from, to, logs))?;

    let n = Value::Obj(vec![
        ("accessions".into(), by_position(accessions)),
        (
            "basis".into(),
            Value::Obj(vec![
                ("book".into(), s(hex20(&v.book))),
                ("bookFromBlock".into(), int(book_from)),
                ("chainId".into(), int(v.chain)),
                ("fromBlock".into(), int(from)),
                ("revocationContract".into(), s(hex20(&v.contract))),
                ("toBlock".into(), int(to)),
            ]),
        ),
        (REVOCATIONS.into(), by_position(notices)),
    ]);
    Ok((n, settling))
}

enum FormOn {
    Whole,
    Held(crate::settled::Entry),
    Kept(crate::settled::Entry),
}

fn form_on(chain: u64, filter: &LogFilter, form: Form, from: u64, nodes: &str) -> (crate::settled::Key, FormOn) {
    use crate::settled::judgement;
    let key = judged_key(chain, nodes, filter, form);
    let usable = |e: &crate::settled::Entry| e.from <= from && e.logs.iter().all(|l| judgement(l).is_some());
    let on = match crate::action::judged_held(&key).filter(|e| e.from <= from) {
        Some(e) if usable(&e) => FormOn::Held(e),
        Some(_) => FormOn::Whole,
        None => crate::settled::judged_entry(&key).filter(usable).map_or(FormOn::Whole, FormOn::Kept),
    };
    (key, on)
}

fn form_asks(place: &str, chain: u64, filter: &LogFilter, form: Form, from: u64, to: u64, nodes: Option<&str>) -> Vec<crate::rpc::Ask<'static>> {
    if to < from {
        return Vec::new();
    }
    let Some(nodes) = nodes else { return whole_ask(place, filter, from, to) };
    match form_on(chain, filter, form, from, nodes).1 {
        FormOn::Whole => whole_ask(place, filter, from, to),
        FormOn::Held(e) => {
            let ask_from = from.max(e.settled.saturating_add(1));
            if ask_from <= to {
                whole_ask(place, filter, ask_from, to)
            } else {
                Vec::new()
            }
        }
        FormOn::Kept(e) => kept_asks(place, &e, filter, from, to),
    }
}

#[allow(clippy::too_many_arguments)]
fn judged_form(
    chain: &mut Chain,
    filter: &LogFilter,
    form: Form,
    from: u64,
    to: u64,
    nodes: Option<&str>,
    settling: &mut Vec<crate::settled::Settling>,
    judge: &mut dyn FnMut(&mut Chain, &[W]) -> Result<Vec<Option<Rec>>, Refusal>,
) -> Result<Vec<Rec>, Refusal> {
    use crate::settled::{block_of, judged_log, judgement, Entry, Settling, MARGIN};
    let Some(nodes) = nodes else {
        let logs = asked(chain, filter, from, to)?;
        return Ok(judge(chain, &logs)?.into_iter().flatten().collect());
    };
    let (key, on) = form_on(chain.id, filter, form, from, nodes);
    let held = match on {
        FormOn::Held(e) => Some(e),
        FormOn::Kept(e) => {
            let asks = kept_asks(&chain.ep.place(), &e, filter, from, to);
            crate::ahead::ask_ahead(chain.ep, &asks);
            rechecked(chain, e)
        }
        FormOn::Whole => None,
    };
    let in_range = |l: &W| block_of(l).is_some_and(|b| (from..=to).contains(&b));
    let mut records: Vec<Rec> = held
        .iter()
        .flat_map(|e| e.logs.iter())
        .filter(|l| in_range(l))
        .filter_map(|l| judgement(l).flatten())
        .filter_map(|r| Some((record_position(&r)?, r)))
        .collect();
    let ask_from = held.as_ref().map_or(from, |e| from.max(e.settled.saturating_add(1)));
    let fetched = if ask_from <= to { asked(chain, filter, ask_from, to)? } else { Vec::new() };
    let judged = judge(chain, &fetched)?;
    records.extend(judged.iter().flatten().cloned());

    let mut settle_to = to.saturating_sub(MARGIN);
    let mut settleable = true;
    for (l, j) in fetched.iter().zip(&judged) {
        let undecided = j.as_ref().is_some_and(|(_, r)| r.member("verdict").and_then(Value::as_str) == Some(Verdict::Unproven.as_str()));
        match (undecided, block_of(l)) {
            (false, _) => {}
            (true, Some(b)) if b > 0 => settle_to = settle_to.min(b - 1),
            (true, _) => settleable = false,
        }
    }
    let (base_from, base_logs, base_settled) = match &held {
        Some(e) => (e.from, e.logs.clone(), Some(e.settled)),
        None => (from, Vec::new(), None),
    };
    let carried = base_settled.is_none_or(|s| ask_from == s.saturating_add(1) && settle_to > s);
    let entry = if settleable && carried && settle_to >= base_from {
        let mut kept = base_logs;
        let mut readable = true;
        for (l, j) in fetched.iter().zip(&judged) {
            match block_of(l) {
                Some(b) if b <= settle_to => kept.push(judged_log(l, j.as_ref().map(|(_, r)| r))),
                Some(_) => {}
                None => readable = false,
            }
        }
        readable.then_some(Entry { from: base_from, settled: settle_to, logs: kept, single_source: true })
    } else {
        None
    };
    settling.push(Settling { key, entry, served_single: held.as_ref().is_some_and(|e| e.single_source), base: held.as_ref().map(|e| (e.from, e.settled)) });
    Ok(records)
}

fn by_hash(h: &[u8; 32]) -> Value {
    Value::Arr(vec![Value::Str(hexfmt::encode(h))])
}

fn once(mut txs: Vec<[u8; 32]>) -> Vec<[u8; 32]> {
    txs.sort_unstable();
    txs.dedup();
    txs
}

fn notices_of(chain: &mut Chain, v: &crate::revoke::Venue, t0: &[u8; 32], from: u64, to: u64, logs: &[W]) -> Result<Vec<Option<Rec>>, Refusal> {
    struct Cand {
        at: (u64, u64, u64),
        txh: [u8; 32],
        sender: [u8; 20],
        grant: [u8; 32],
    }
    let cands: Vec<Option<Cand>> = logs
        .iter()
        .map(|log| {
            if emitter_of(log) != Some(v.contract) {
                return None;
            }
            let (topics, at, txh) = (form_topics(log, t0, 3)?, position_of(log)?, log.member("transactionHash").and_then(h32)?);
            let sender = address_topic(&topics[1])?;
            (at.0 >= from && at.0 <= to).then_some(Cand { at, txh, sender, grant: topics[2] })
        })
        .collect();
    let txs = once(cands.iter().flatten().map(|c| c.txh).collect());
    let mut codes: Vec<Value> = Vec::new();
    for c in cands.iter().flatten() {
        for b in [c.at.0.checked_sub(1), Some(c.at.0)].into_iter().flatten() {
            let q = Value::Arr(vec![Value::Str(hex20(&c.sender)), Value::Str(hex_quantity(b))]);
            if crate::ahead::code_at(chain.id, &c.sender, b).is_none() && !codes.contains(&q) {
                codes.push(q);
            }
        }
    }
    let rest: Vec<crate::rpc::Ask<'_>> = txs.iter().map(|h| ("eth_getTransactionReceipt", by_hash(h))).chain(codes.into_iter().map(|q| ("eth_getCode", q))).collect();
    crate::ahead::ask_txs_ahead(&mut *chain.ep, chain.id, &txs, &rest, false);
    let mut passed: Vec<Option<&Cand>> = Vec::with_capacity(cands.len());
    for c in &cands {
        let Some(c) = c else {
            passed.push(None);
            continue;
        };
        let id = chain.id;
        let Some(t) = crate::action::proven(id, &c.txh, || chain.tx_by_hash(&c.txh))? else { return Err(no_tx(&c.txh)) };
        let ok = chain.status_one(&c.txh)? && t.sender == Some(c.sender) && t.chain_id == Some(chain.id) && t.to.is_some() && calldata_carries(&t.input, &c.grant);
        passed.push(ok.then_some(c));
    }
    let mut out = Vec::with_capacity(passed.len());
    for c in passed {
        let Some(c) = c else {
            out.push(None);
            continue;
        };
        let verdict = chain.verdict_at(&c.sender, c.at.0)?;
        out.push(Some((
            c.at,
            Value::Obj(vec![
                ("blockNumber".into(), Value::Int(c.at.0)),
                ("chainId".into(), Value::Int(chain.id)),
                ("grant".into(), Value::Str(hexfmt::encode(&c.grant))),
                ("logIndex".into(), Value::Int(c.at.2)),
                ("sender".into(), Value::Str(hex20(&c.sender))),
                ("tx".into(), Value::Str(hexfmt::encode(&c.txh))),
                ("txIndex".into(), Value::Int(c.at.1)),
                ("verdict".into(), Value::Str(verdict.as_str().into())),
            ]),
        )));
    }
    Ok(out)
}

fn accessions_of(chain: &mut Chain, v: &crate::revoke::Venue, t0: &[u8; 32], from: u64, to: u64, logs: &[W]) -> Result<Vec<Option<Rec>>, Refusal> {
    let cands: Vec<Option<((u64, u64, u64), [u8; 32], [[u8; 20]; 3])>> = logs
        .iter()
        .map(|log| {
            if emitter_of(log) != Some(v.book) {
                return None;
            }
            let (topics, at, txh) = (form_topics(log, t0, 4)?, position_of(log)?, log.member("transactionHash").and_then(h32)?);
            let keys = [address_topic(&topics[1])?, address_topic(&topics[2])?, address_topic(&topics[3])?];
            (at.0 >= from && at.0 <= to).then_some((at, txh, keys))
        })
        .collect();
    let txs = once(cands.iter().flatten().map(|c| c.1).collect());
    crate::ahead::ask_ahead(&mut *chain.ep, &txs.iter().map(|h| ("eth_getTransactionReceipt", by_hash(h))).collect::<Vec<_>>());
    let mut out = Vec::with_capacity(cands.len());
    for c in cands {
        let Some((at, txh, [holder, to_key, root])) = c else {
            out.push(None);
            continue;
        };
        if !chain.status_one(&txh)? {
            out.push(None);
            continue;
        }
        out.push(Some((
            at,
            Value::Obj(vec![
                ("blockNumber".into(), Value::Int(at.0)),
                ("chainId".into(), Value::Int(chain.id)),
                ("holder".into(), Value::Str(hex20(&holder))),
                ("logIndex".into(), Value::Int(at.2)),
                ("root".into(), Value::Str(hex20(&root))),
                ("to".into(), Value::Str(hex20(&to_key))),
                ("txIndex".into(), Value::Int(at.1)),
            ]),
        )));
    }
    Ok(out)
}

pub fn dedupe(anchors: Vec<AnchorRec>) -> Vec<AnchorRec> {
    let mut out: Vec<AnchorRec> = Vec::new();
    for a in anchors {
        let key = (a.chain_id, a.block_number, a.tx, a.hash);
        if out.last().map(|p| (p.chain_id, p.block_number, p.tx, p.hash)) == Some(key) {
            continue;
        }
        out.push(a);
    }
    out
}

pub fn fragment(s: &Scanned) -> Value {
    crate::seam();
    let anchors = s
        .anchors
        .iter()
        .map(|a| {
            Value::Obj(vec![
                ("blockNumber".into(), Value::Int(a.block_number)),
                ("blockTimestamp".into(), Value::Int(a.block_timestamp)),
                ("chainId".into(), Value::Int(a.chain_id)),
                ("hash".into(), Value::Str(hexfmt::encode(&a.hash))),
                ("sender".into(), Value::Str(hex20(&a.sender))),
                ("tx".into(), Value::Str(hexfmt::encode(&a.tx))),
                ("verdict".into(), Value::Str(a.verdict.as_str().into())),
            ])
        })
        .collect();
    let mut members = vec![("anchors".into(), Value::Arr(anchors)), ("basis".into(), s.basis.clone())];
    if !s.candidates.is_empty() {
        members.push(("candidates".into(), Value::Arr(s.candidates.iter().map(claim_value).collect())));
    }
    Value::Obj(members)
}

#[cfg(test)]
mod notice_tests;

#[cfg(test)]
mod ahead_logs_tests;

#[cfg(test)]
mod width_tests;

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn a_transaction_that_is_not_the_one_asked_is_refused_as_not_itself() {
        let asked = [0x01u8; 32];
        let unsigned = |hash: String| move |_: u64, _: u64, _: usize| -> Result<W, Trouble> {
            Ok(crate::wire::parse(format!(r#"{{"hash":"{hash}","input":"0x","r":"0x0","s":"0x0","type":"0x0"}}"#).as_bytes()).expect("a transaction"))
        };
        let mut n = logs(unsigned(format!("0x{}", "02".repeat(32))));
        let got = Chain { id: 1, ep: &mut n }.tx_by_hash(&asked);
        assert!(matches!(got, Err(Refusal::TxNotItself(_))), "another transaction: {got:?}");
        let mut n = logs(unsigned(hexfmt::encode(&asked)));
        let got = Chain { id: 1, ep: &mut n }.tx_by_hash(&asked);
        assert!(matches!(got, Ok(Some(_))), "its own: {got:?}");
    }

    #[test]
    fn a_receipt_is_each_nodes_own_word() {
        let h = [0x03u8; 32];
        let mut n = logs(|_, _, _| Ok(crate::wire::parse(br#"{"blockNumber":"0x5","status":"0x0"}"#).expect("a receipt")));
        assert_eq!(Chain { id: 1, ep: &mut n }.status_one(&h).ok(), Some(false), "status 0: did not succeed, its own word");
        let mut n = logs(|_, _, _| Ok(crate::wire::parse(b"null").expect("null")));
        let got = Chain { id: 1, ep: &mut n }.status_one(&h);
        assert!(matches!(got, Err(Refusal::Unanswered { asked: Asked::NoReceipt(_), .. })), "no receipt: {got:?}");
    }

    struct NoTx {
        contract: [u8; 20],
        sender: [u8; 20],
    }

    impl Endpoint for NoTx {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            let filter_t0 = match params {
                Value::Arr(ps) => ps.first().and_then(|f| f.member("topics")).and_then(Value::as_arr).and_then(|t| t.first()).and_then(Value::as_str).map(str::to_string),
                _ => None,
            };
            let text = match method {
                "eth_chainId" => "\"0x1\"".to_string(),
                "eth_getLogs" => {
                    let t0 = filter_t0.unwrap_or_default();
                    let revoked = hexfmt::encode(&cryptox::keccak256(crate::revoke::REVOKED_SIGNATURE.as_bytes()));
                    if t0 != revoked {
                        return Ok(W::of(crate::wire::Body::Arr(Vec::new())));
                    }
                    format!(
                        r#"[{{"address":"{}","blockNumber":"0x5","transactionIndex":"0x0","logIndex":"0x0","transactionHash":"0x{}","topics":["{}","0x{}{}","0x{}"],"data":"0x"}}]"#,
                        hexfmt::encode(&self.contract),
                        "ab".repeat(32),
                        revoked,
                        "00".repeat(12),
                        hexfmt::encode(&self.sender).trim_start_matches("0x"),
                        "cd".repeat(32)
                    )
                }
                "eth_getTransactionByHash" => "null".to_string(),
                "eth_getTransactionReceipt" => r#"{"status":"0x1","logs":[]}"#.to_string(),
                _ => "null".to_string(),
            };
            Ok(crate::wire::parse(text.as_bytes()).expect("json"))
        }
        fn name(&self) -> String {
            "no-tx".into()
        }
        fn place(&self) -> String {
            "no-tx".into()
        }
    }

    #[test]
    fn a_notice_whose_transaction_is_not_given_is_no_notice_input() {
        let contract = [0x50; 20];
        let v = crate::revoke::Venue { chain: 1, contract, from_block: 0, book: [0x60; 20], book_from_block: 0 };
        let got = notice_input(&mut NoTx { contract, sender: [0x22; 20] }, &v, 10);
        assert!(matches!(&got, Err(Refusal::Malformed(Shape::TxLacks { part, .. })) if part == "transaction"), "refused naming the transaction: {got:?}");
    }

    struct Logs<F: FnMut(u64, u64, usize) -> Result<W, Trouble>> {
        rule: F,
        asked: Vec<(u64, u64)>,
        methods: Vec<String>,
        name: String,
    }

    struct Shared<E>(std::sync::Arc<std::sync::Mutex<E>>);

    impl<E> Clone for Shared<E> {
        fn clone(&self) -> Self {
            Shared(self.0.clone())
        }
    }

    impl<E: Endpoint> Endpoint for Shared<E> {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            self.0.lock().unwrap().call(method, params)
        }
        fn name(&self) -> String {
            self.0.lock().unwrap().name()
        }
        fn place(&self) -> String {
            self.0.lock().unwrap().place()
        }
    }

    impl<F: FnMut(u64, u64, usize) -> Result<W, Trouble>> Endpoint for Logs<F> {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            self.methods.push(method.to_string());
            let at = |k: &str| -> u64 {
                match params {
                    Value::Arr(ps) => match ps.first() {
                        Some(Value::Obj(f)) => f.iter().find(|(n, _)| n == k).and_then(|(_, v)| v.as_str()).and_then(|h| u64::from_str_radix(h.trim_start_matches("0x"), 16).ok()).unwrap_or(0),
                        _ => 0,
                    },
                    _ => 0,
                }
            };
            let (from, to) = (at("fromBlock"), at("toBlock"));
            let n = self.asked.len();
            self.asked.push((from, to));
            (self.rule)(from, to, n)
        }
        fn name(&self) -> String {
            self.name.clone()
        }
        fn place(&self) -> String {
            self.name.clone()
        }
    }

    fn logs<F: FnMut(u64, u64, usize) -> Result<W, Trouble>>(rule: F) -> Logs<F> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let name = format!("logs-{}", N.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        Logs { rule, asked: Vec::new(), methods: Vec::new(), name }
    }

    fn window(from: u64, to: u64) -> Window {
        Window { chain_id: 1, from_block: from, to_block: to, registries: vec![[0x11; 20]], senders: vec![[0x22; 20]] }
    }

    fn empty() -> Result<W, Trouble> {
        Ok(W::of(crate::wire::Body::Arr(Vec::new())))
    }

    fn node_says(code: i64, message: &str) -> Trouble {
        Trouble::Node(format!("{{\"code\":{code},\"message\":\"{message}\"}}"))
    }

    fn scan_window<F: FnMut(u64, u64, usize) -> Result<W, Trouble>>(node: &mut Logs<F>, w: &Window) -> Result<Vec<W>, Refusal> {
        logs_over_window(node, w.chain_id, &anchor_filter(w, &topic0()), w.from_block, w.to_block)
    }

    fn no_pauses() {
        crate::patience::set_waits(Some(std::time::Duration::ZERO));
    }

    #[test]
    fn a_log_filter_is_written_position_by_position() {
        let f = LogFilter { addresses: Vec::new(), topics: vec![Topic::Any, Topic::One([0x33; 32]), Topic::AnyOf(vec![[0x44; 32], [0x55; 32]])] };
        let got = String::from_utf8(zikaron::json::canon_bytes(&f.at(0, u64::MAX))).expect("text");
        let word = |b: u8| format!("\"0x{}\"", format!("{b:02x}").repeat(32));
        assert_eq!(
            got,
            format!("{{\"address\":[],\"fromBlock\":\"0x0\",\"toBlock\":\"0xffffffffffffffff\",\"topics\":[null,{},[{},{}]]}}", word(0x33), word(0x44), word(0x55))
        );
        assert_eq!(String::from_utf8(zikaron::json::canon_bytes(&LogFilter { addresses: Vec::new(), topics: Vec::new() }.at(5, 5))).expect("text"), "{\"address\":[],\"fromBlock\":\"0x5\",\"toBlock\":\"0x5\",\"topics\":[]}");
        let w = window(1, 9);
        let anchor = anchor_filter(&w, &topic0());
        let mut padded_sender = [0u8; 32];
        padded_sender[12..].copy_from_slice(&[0x22; 20]);
        assert_eq!(anchor, LogFilter { addresses: vec![[0x11; 20]], topics: vec![Topic::One(topic0()), Topic::AnyOf(vec![padded_sender])] });
    }

    #[test]
    fn a_log_read_is_one_question_for_the_whole_range_and_a_width_refusal_is_read_in_pieces() {
        no_pauses();
        let mut node = logs(|_, _, _| empty());
        assert!(scan_window(&mut node, &window(0, 1_208_000)).is_ok());
        assert_eq!(node.asked, vec![(0, 1_208_000)]);
        let sepolia = "{\"data\":{\"code\":\"ErrGetLogsExceededMaxAllowedRange\",\"message\":\"getLogs request exceeded max allowed range\",\"details\":{\"requestRange\":1208001,\"maxAllowedRange\":30000}},\"code\":-32012,\"message\":\"getLogs request exceeded max allowed range\"}";
        let arbitrum = |f: u64, t: u64| format!("{{\"code\":-32602,\"message\":\"query spans {} blocks ({f} to {t}), but only 30000 are allowed for this request; narrow the block range\"}}", t - f + 1);
        let words: [std::sync::Arc<dyn Fn(u64, u64) -> String + Send + Sync>; 2] = [std::sync::Arc::new(move |_: u64, _: u64| sepolia.to_string()), std::sync::Arc::new(arbitrum)];
        for said in words {
            let mut node = logs(move |f, t, _| if t - f + 1 > 30_000 { Err(Trouble::Node(said(f, t))) } else { empty() });
            assert!(scan_window(&mut node, &window(0, 1_208_000)).is_ok(), "read in pieces");
            assert_eq!(node.asked[..2], [(0, 1_208_000), (0, 1_208_000)], "{:?}", &node.asked[..2]);
            assert!(lays(&node.asked, 30_000, 0, 1_208_000), "the pieces lay the window end to end: {:?}", node.asked);
        }
    }

    fn lays(asked: &[(u64, u64)], cap: u64, from: u64, to: u64) -> bool {
        let mut next = from;
        for (a, b) in asked.iter().filter(|(a, b)| b - a < cap) {
            if *a != next {
                return false;
            }
            next = b + 1;
        }
        next == to + 1
    }

    #[test]
    fn a_refusal_naming_no_range_is_passed_on_and_the_range_kept() {
        no_pauses();
        let busy = "service temporarily unavailable";
        let mut node = logs(move |_, _, _| Err(node_says(-32603, busy)));
        let refused = scan_window(&mut node, &window(1, 10_000));
        match &refused {
            Err(Refusal::Unanswered { asked: Asked::OverAsks { cap, said: Some(said) }, .. }) => assert!(*cap == crate::action::asks_cap() && said.contains(busy), "{said}"),
            other => panic!("{other:?}"),
        }
        assert!(refused.as_ref().err().map(Refusal::detail).is_some_and(|d| d.contains(OVER_ASKS) && d.contains(busy)), "{refused:?}");
        assert_eq!(node.asked[..2], [(1, 10_000); 2]);
        assert!(node.asked[2..].iter().all(|(f, t)| t - f + 1 < 10_000), "halved after the whole range: {:?}", node.asked);
        node.asked.clear();
        assert!(scan_window(&mut node, &window(1, 10_000)).is_err());
        assert_eq!(node.asked[..2], [(1, 10_000); 2], "the next read asks it whole");
        let mut node = logs(|_, _, n| if n == 0 { Err(node_says(-32603, busy)) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 10_000)).is_ok(), "answered when asked once more");
        assert_eq!(node.asked, vec![(1, 10_000); 2]);
    }

    #[test]
    fn the_scans_other_questions_are_asked_once_when_refused() {
        no_pauses();
        let busy = || Err(node_says(-32603, "service temporarily unavailable"));
        let mut node = logs(move |_, _, _| busy());
        assert_eq!(code_at(&mut node, 1, &[0x22; 20], 5), Ok(None));
        assert_eq!(node.methods, vec!["eth_getCode".to_string()]);
        let mut node = logs(move |_, _, _| busy());
        assert!(Chain { id: 1, ep: &mut node }.tx_by_hash(&[0x44; 32]).is_err());
        assert_eq!(node.methods, vec!["eth_getTransactionByHash".to_string()]);
    }

    #[test]
    fn an_answer_that_is_not_a_log_list_is_refused_never_empty() {
        let mut node = logs(|_, _, _| Ok(W::of(crate::wire::Body::Null)));
        assert!(matches!(scan_window(&mut node, &window(1, 10)), Err(Refusal::Unanswered { asked: Asked::NotLogs, .. })));
    }

    #[test]
    fn a_window_to_the_last_block_is_one_question() {
        let mut node = logs(|_, _, _| empty());
        assert!(scan_window(&mut node, &window(0, u64::MAX)).is_ok());
        assert_eq!(node.asked, vec![(0, u64::MAX)]);
        let mut node = logs(|_, _, _| empty());
        assert!(scan_window(&mut node, &window(u64::MAX, u64::MAX)).is_ok());
        assert_eq!(node.asked.len(), 1);
    }

    #[test]
    fn a_rate_limited_node_is_waited_out_and_its_range_kept() {
        no_pauses();
        let pauses = crate::said::RATE_BACKOFF.len();
        let mut node = logs(|_, _, _| Err(node_says(-32005, "rate limit: max 25 requests per second")));
        assert!(scan_window(&mut node, &window(1, 1000)).is_err());
        assert_eq!(node.asked, vec![(1, 1000); 1 + pauses], "the same range, once and once after each pause");
        let mut node = logs(|_, _, n| if n == 0 { Err(node_says(-32005, "daily request limit exceeded")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 1000)).is_ok());
        assert_eq!(node.asked, vec![(1, 1000); 2]);
        let mut node = logs(|_, _, _| Err(crate::rpc::status("https://n.example", 429)));
        assert!(scan_window(&mut node, &window(1, 1000)).is_err());
        assert_eq!(node.asked, vec![(1, 1000); 1 + pauses]);
        let mut node = logs(|f, t, _| if t - f + 1 > 500 { Err(node_says(-32005, "query returned more than 10000 results")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 1000)).is_ok(), "read in pieces");
        assert_eq!(node.asked[..2], [(1, 1000), (1, 1000)], "a refusal naming results is no rate limit: {:?}", &node.asked[..2]);
        assert!(lays(&node.asked[2..], 500, 1, 1000), "the pieces lay the window end to end: {:?}", node.asked);
    }

    #[test]
    fn a_refusals_evidence_is_machine_form_in_no_language() {
        let cjk = |x: &str| x.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
        let declined = Refusal::Unanswered { chain: 1, asked: Asked::Declined { method: "eth_getLogs".into(), said: "{\"code\":-32000,\"message\":\"busy\"}".into(), width: None } };
        assert_eq!(declined.detail(), "chain 1 eth_getLogs {\"code\":-32000,\"message\":\"busy\"}");
        let late = Refusal::Unanswered {
            chain: 11155111,
            asked: Asked::Broke { method: "eth_getCode".into(), broke: Broke::Late { node: "http://n:1".into(), deadline: std::time::Duration::from_secs(5) } },
        };
        assert_eq!(late.detail(), "chain 11155111 eth_getCode TIMEOUT http://n:1 5000ms");
        let all = [
            declined,
            late,
            Refusal::ChainIdMismatch { declared: 1, served: "\"0x5\"".into() },
            Refusal::NoEndpoint(7),
            Refusal::Unanswered { chain: 1, asked: Asked::NotServed("eth_getLogs [..]".into()) },
            Refusal::Unanswered { chain: 1, asked: Asked::NotLogs },
            Refusal::Unanswered { chain: 1, asked: Asked::NoReceipt("0xab".into()) },
            Refusal::Unanswered { chain: 1, asked: Asked::Short },
            Refusal::TxNotItself("0xab".into()),
            Refusal::Malformed(Shape::TxLacks { part: "input".into(), tx: "0xab".into() }),
            Refusal::Malformed(Shape::NoTimestamp(9)),
            Refusal::Malformed(Shape::CodeNotBytes),
            Refusal::Malformed(Shape::Basis),
            Refusal::Malformed(Shape::Contradiction("k".into())),
        ];
        for r in &all {
            assert!(!r.detail().is_empty() && !cjk(&r.detail()), "{r:?}: {}", r.detail());
        }
    }

    #[test]
    fn a_transaction_the_node_does_not_have_is_no_anchor() {
        let t0 = topic0();
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(&[0x22; 20]);
        let log = crate::wire::parse(
            format!(
                "{{\"address\":\"{}\",\"blockNumber\":\"0x5\",\"data\":\"0x\",\"logIndex\":\"0x0\",\"topics\":[\"{}\",\"{}\",\"0x{}\"],\"transactionHash\":\"0x{}\"}}",
                hex20(&[0x11; 20]),
                hexfmt::encode(&t0),
                hexfmt::encode(&padded),
                "33".repeat(32),
                "44".repeat(32)
            )
            .as_bytes(),
        )
        .expect("a log");
        let mut node = logs(|_, _, _| Ok(W::of(crate::wire::Body::Null)));
        let mut chain = Chain { id: 1, ep: &mut node };
        let got = registry_record(&mut chain, &window(1, 10), &log, &t0, &Known::default(), &mut Vec::new());
        assert!(matches!(got, Ok(None)));
        assert_eq!(node.methods, vec!["eth_getTransactionByHash".to_string()]);
    }

    struct Kept(std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>);

    impl crate::settled::Keep for Kept {
        fn load(&self, part: &str) -> Option<Vec<u8>> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).get(part).cloned()
        }
        fn save(&self, part: &str, plain: &[u8]) -> Result<(), crate::settled::Unkept> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(part.to_string(), plain.to_vec());
            Ok(())
        }
    }

    struct Laid {
        hashes: std::collections::BTreeMap<u64, String>,
        logs: Vec<u64>,
        posts: std::sync::Arc<std::sync::Mutex<usize>>,
        asked: std::sync::Arc<std::sync::Mutex<Vec<(String, Option<(u64, u64)>)>>>,
    }

    impl Laid {
        fn answer(&self, method: &str, params: &Value) -> Result<W, Trouble> {
            let first = params.as_arr().and_then(|p| p.first());
            let n = |v: Option<&Value>| v.and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok());
            let range = (method == "eth_getLogs").then(|| (n(first.and_then(|f| f.member("fromBlock"))).unwrap_or(0), n(first.and_then(|f| f.member("toBlock"))).unwrap_or(0)));
            self.asked.lock().unwrap().push((method.to_string(), range));
            let text = match method {
                "eth_getBlockByNumber" => {
                    let b = n(first).unwrap_or(0);
                    format!(r#"{{"hash":"{}","number":"0x{b:x}","timestamp":"0x1"}}"#, self.hashes.get(&b).cloned().unwrap_or_default())
                }
                "eth_getLogs" => {
                    let (from, to) = range.unwrap_or((0, 0));
                    let logs: Vec<String> = self
                        .logs
                        .iter()
                        .filter(|b| (from..=to).contains(*b))
                        .map(|b| format!(r#"{{"address":"0x{}","blockHash":"{}","blockNumber":"0x{b:x}","data":"0x","logIndex":"0x0","topics":[],"transactionHash":"0x{b:064x}","transactionIndex":"0x0"}}"#, "11".repeat(20), self.hashes.get(b).cloned().unwrap_or_default()))
                        .collect();
                    format!("[{}]", logs.join(","))
                }
                _ => "null".into(),
            };
            Ok(crate::wire::parse(text.as_bytes()).expect("json"))
        }
    }

    impl Endpoint for Laid {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            *self.posts.lock().unwrap() += 1;
            self.answer(method, params)
        }
        fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
            *self.posts.lock().unwrap() += 1;
            Some(Ok(asks.iter().map(|(m, p)| Some(self.answer(m, p))).collect()))
        }
        fn name(&self) -> String {
            "laid".into()
        }
        fn place(&self) -> String {
            "laid".into()
        }
    }

    #[test]
    fn a_kept_reads_check_and_tail_go_together_and_a_replaced_block_reads_as_a_whole_read() {
        no_pauses();
        let keep: std::sync::Arc<dyn crate::settled::Keep> = std::sync::Arc::new(Kept(std::sync::Mutex::new(Default::default())));
        let filter = LogFilter { addresses: vec![[0x11; 20]], topics: Vec::new() };
        let hashes = |replaced_from: u64| (0..=300u64).map(|b| (b, format!("0x{}{b:062x}", if b >= replaced_from { "bb" } else { "aa" }))).collect::<std::collections::BTreeMap<u64, String>>();
        let laid = |hashes, logs: Vec<u64>| Laid { hashes, logs, posts: Default::default(), asked: Default::default() };
        let read = |node: Laid, keeper: Option<std::sync::Arc<dyn crate::settled::Keep>>| {
            let (posts, asked) = (node.posts.clone(), node.asked.clone());
            let got = {
                let _action = crate::action::open(keeper);
                let mut ep = crate::endpoints::remembering("laid", Box::new(node));
                paged(&mut Chain { id: 1, ep: ep.as_mut() }, &filter, 0, 200, Over::Alone).expect("read")
            };
            let mut seen: Vec<(u64, String)> = got.iter().map(|l| (crate::settled::block_of(l).unwrap_or(0), l.member("blockHash").and_then(W::as_str).unwrap_or_default().to_string())).collect();
            seen.sort();
            let asked = asked.lock().unwrap().clone();
            (seen, *posts.lock().unwrap(), asked)
        };
        let _ = read(laid(hashes(u64::MAX), vec![5, 7]), Some(keep.clone()));
        let (seen, posts, asked) = read(laid(hashes(u64::MAX), vec![5, 7, 150]), Some(keep.clone()));
        assert_eq!(seen.iter().map(|(b, _)| *b).collect::<Vec<_>>(), [5, 7, 150]);
        assert_eq!(posts, 1, "the check and the tail in one post: {asked:?}");
        assert_eq!(asked.iter().filter_map(|(_, r)| *r).collect::<Vec<_>>(), [(137, 200)], "the logs after the kept part, once");
        let after = vec![5, 7, 50, 160];
        let (kept_read, _, asked) = read(laid(hashes(7), after.clone()), Some(keep.clone()));
        let (whole, _, _) = read(laid(hashes(7), after), None);
        assert_eq!(kept_read, whole, "log for log, a read with nothing kept: {asked:?}");
        assert!(asked.iter().any(|(_, r)| *r == Some((6, 200))), "read again from the block after the last that matched: {asked:?}");
    }

    fn at_seven(from: u64, to: u64) -> Result<W, Trouble> {
        let log = "{\"address\":\"0x1111111111111111111111111111111111111111\",\"blockNumber\":\"0x7\",\"data\":\"0x\",\"logIndex\":\"0x0\",\"topics\":[]}";
        Ok(crate::wire::parse(if (from..=to).contains(&7) { format!("[{log}]") } else { "[]".to_string() }.as_bytes()).expect("logs"))
    }

    #[test]
    fn a_stretch_two_endpoints_bore_alike_is_served_to_a_table_of_three() {
        no_pauses();
        let keep: std::sync::Arc<dyn crate::settled::Keep> = std::sync::Arc::new(Kept(std::sync::Mutex::new(Default::default())));
        let nodes = crate::settled::nodes_of(["http://a.test", "http://b.test", "rest://row.test"]);
        let filter = anchor_filter(&window(0, 1000), &topic0());
        let bounded = |f: u64, t: u64, _: usize| if t - f + 1 > 100 { Err(node_says(-32012, "getLogs request exceeded max allowed range")) } else { at_seven(f, t) };
        {
            let _action = crate::action::open(Some(keep.clone()));
            let (mut a, mut row, mut b) = (logs(|f, t, _| at_seven(f, t)), logs(|f, t, _| at_seven(f, t)), logs(bounded));
            let (mut by_a, mut by_row, mut by_b) = (Vec::new(), Vec::new(), Vec::new());
            paged(&mut Chain { id: 1, ep: &mut a }, &filter, 0, 1000, Over::Agreed { nodes: &nodes, out: &mut by_a }).expect("the whole range");
            paged(&mut Chain { id: 1, ep: &mut row }, &filter, 0, 1000, Over::Agreed { nodes: &nodes, out: &mut by_row }).expect("the whole range, in two parts");
            assert!(paged(&mut Chain { id: 1, ep: &mut b }, &filter, 0, 1000, Over::Agreed { nodes: &nodes, out: &mut by_b }).is_err(), "the bounded node refuses it");
            assert!(by_b.is_empty(), "a round that refused settles nothing");
            crate::settled::settle(&[by_a, by_row], &[]);
        }
        let _next = crate::action::open(Some(keep));
        let mut b = logs(bounded);
        let mut out = Vec::new();
        let got = paged(&mut Chain { id: 1, ep: &mut b }, &filter, 0, 1000, Over::Agreed { nodes: &nodes, out: &mut out }).expect("served, and the blocks after answered");
        assert_eq!(b.asked, vec![(1000 - crate::settled::MARGIN + 1, 1000)], "only the blocks after what the table settled");
        assert_eq!(got.len(), 1, "the settled log served");
        assert!(out.iter().all(|s| !s.served_single), "served an agreed entry: not single-source");
    }

    #[test]
    fn any_logs_are_read_in_one_agreed_round_and_kept_once_alike() {
        no_pauses();
        let keep: std::sync::Arc<dyn crate::settled::Keep> = std::sync::Arc::new(Kept(std::sync::Mutex::new(Default::default())));
        let filter = LogFilter { addresses: vec![[0x22; 20]], topics: vec![Topic::One([0x33; 32])] };
        let named = |mut n: Logs<fn(u64, u64, usize) -> Result<W, Trouble>>, place: &str| {
            n.name = place.to_string();
            n
        };
        let seven: fn(u64, u64, usize) -> Result<W, Trouble> = |f, t, _| at_seven(f, t);
        {
            let _action = crate::action::open(Some(keep.clone()));
            let (a, b) = (named(logs(seven), "http://a.test"), named(logs(seven), "http://b.test"));
            let got = logs_agreed(vec![Box::new(a), Box::new(b)], 1, &filter, 0, 1000);
            assert!(got.agreed && got.answered == 2 && got.refused.is_empty() && got.logs.len() == 1, "{got:?}");
        }
        {
            let _next = crate::action::open(Some(keep.clone()));
            let a = Shared(std::sync::Arc::new(std::sync::Mutex::new(named(logs(seven), "http://a.test"))));
            let b = named(logs(seven), "http://b.test");
            let got = logs_agreed(vec![Box::new(a.clone()), Box::new(b)], 1, &filter, 0, 1000);
            assert!(got.agreed && got.logs.len() == 1);
            assert_eq!(a.0.lock().unwrap().asked, vec![(1000 - crate::settled::MARGIN + 1, 1000)], "served what was kept, asked only after");
        }
        let _other = crate::action::open(Some(keep));
        let (a, odd, down) = (logs(|f, t, _| at_seven(f, t)), logs(|_, _, _| Ok(crate::wire::parse(b"[]").expect("none"))), logs(|_, _, _| Err(node_says(-32000, "down"))));
        let filter2 = LogFilter { addresses: vec![[0x44; 20]], topics: Vec::new() };
        let got = logs_agreed(vec![Box::new(a), Box::new(odd), Box::new(down)], 1, &filter2, 0, 1000);
        assert!(!got.agreed && got.answered == 2 && got.refused.len() == 1 && got.logs.len() == 1, "{got:?}");
    }
}

#[cfg(test)]
mod face_tests {
    use super::*;

    struct Book {
        book: [u8; 20],
        root: [u8; 20],
        positioned: bool,
    }

    fn w(text: &str) -> W {
        crate::wire::parse(text.as_bytes()).expect("json")
    }

    fn topic(a: &[u8; 20]) -> String {
        format!("0x{}{}", "00".repeat(12), hexfmt::encode(a).trim_start_matches("0x"))
    }

    impl Endpoint for Book {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            let first = match params {
                Value::Arr(ps) => ps.first().cloned(),
                _ => None,
            };
            let t0 = hexfmt::encode(&cryptox::keccak256(crate::revoke::ACCEDED_SIGNATURE.as_bytes()));
            Ok(match method {
                "eth_chainId" => w("\"0x1\""),
                "eth_getLogs" => {
                    let log = |block: u64, tx: u8, root: &[u8; 20], positioned: bool| {
                        let at = if positioned { format!(r#""blockNumber":"0x{block:x}","transactionIndex":"0x0","logIndex":"0x{tx:x}","#) } else { String::new() };
                        format!(
                            r#"{{"address":"{}",{at}"transactionHash":"0x{}","topics":["{t0}","{}","{}","{}"],"data":"0x"}}"#,
                            hexfmt::encode(&self.book),
                            format!("{tx:02x}").repeat(32),
                            topic(&[0x31; 20]),
                            topic(&[0x32; 20]),
                            topic(root)
                        )
                    };
                    let mut logs = vec![log(5, 1, &self.root, true), log(6, 2, &self.root, true), log(7, 3, &[0x99; 20], true)];
                    if !self.positioned {
                        logs.push(log(8, 4, &self.root, false));
                    }
                    w(&format!("[{}]", logs.join(",")))
                }
                "eth_getTransactionReceipt" => {
                    let h = first.as_ref().and_then(Value::as_str).unwrap_or_default().to_string();
                    w(if h.starts_with("0x02") { r#"{"status":"0x0"}"# } else { r#"{"status":"0x1"}"# })
                }
                "eth_getBlockByNumber" => {
                    let n = first.as_ref().and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok()).unwrap_or(0);
                    w(&format!(r#"{{"number":"0x{n:x}","timestamp":"0x{:x}"}}"#, 1000 + n))
                }
                _ => w("null"),
            })
        }
        fn name(&self) -> String {
            "book".into()
        }
        fn place(&self) -> String {
            "book".into()
        }
    }

    struct Batching {
        inner: Book,
        batches: std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>>,
        singles: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Batching {
        fn answer(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            let got = self.inner.call(method, params)?;
            if method != "eth_getTransactionReceipt" {
                return Ok(got);
            }
            let tx = params.as_arr().and_then(|p| p.first()).and_then(Value::as_str).unwrap_or_default().to_string();
            let block = if tx.starts_with("0x02") { 6 } else { 5 };
            let status = got.member("status").and_then(W::as_str).unwrap_or("0x0").to_string();
            Ok(w(&format!(r#"{{"blockHash":"0x{}","blockNumber":"0x{block:x}","status":"{status}"}}"#, "cd".repeat(32))))
        }
    }

    impl Endpoint for Batching {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            self.singles.lock().unwrap().push(method.to_string());
            self.answer(method, params)
        }
        fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
            self.batches.lock().unwrap().push(asks.iter().map(|(m, _)| m.to_string()).collect());
            Some(Ok(asks.iter().map(|(m, p)| Some(self.answer(m, p))).collect()))
        }
        fn name(&self) -> String {
            "book".into()
        }
        fn place(&self) -> String {
            "book".into()
        }
    }

    #[test]
    fn a_face_reading_asks_its_accessions_receipts_and_blocks_in_one_batch() {
        let (book, root) = ([0x60; 20], [0x11; 20]);
        let (batches, singles) = (std::sync::Arc::new(std::sync::Mutex::new(Vec::new())), std::sync::Arc::new(std::sync::Mutex::new(Vec::new())));
        let node = Batching { inner: Book { book, root, positioned: true }, batches: batches.clone(), singles: singles.clone() };
        let _action = crate::action::open(None);
        let mut ep = crate::endpoints::remembering("book", Box::new(node));
        let got = accessions_under(ep.as_mut(), 1, &book, &root, 0, 100).expect("read");
        assert_eq!(got.len(), 1);
        let batches = batches.lock().unwrap().clone();
        assert_eq!(batches.len(), 1, "one batch: {batches:?}");
        let mut kinds = batches[0].clone();
        kinds.sort();
        assert_eq!(kinds, ["eth_getBlockByNumber", "eth_getBlockByNumber", "eth_getTransactionReceipt", "eth_getTransactionReceipt"], "two accessions under the root, each its receipt and its block");
        assert_eq!(singles.lock().unwrap().clone(), ["eth_chainId", "eth_getLogs"], "nothing asked again alone");
    }

    #[test]
    fn the_accessions_under_a_root_carry_nine_members_and_are_read_whole() {
        let (book, root) = ([0x60; 20], [0x11; 20]);
        let got = accessions_under(&mut Book { book, root, positioned: true }, 1, &book, &root, 0, 100).expect("read");
        assert_eq!(got.len(), 1, "block 6 failed, block 7 is another root's: {got:?}");
        let r = &got[0];
        let Value::Obj(ms) = r else { panic!("an object") };
        let names: Vec<&str> = ms.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["blockNumber", "blockTimestamp", "chainId", "holder", "logIndex", "root", "to", "tx", "txIndex"]);
        assert_eq!(r.member("blockTimestamp").and_then(Value::as_int), Some(1005));
        assert_eq!(r.member("root").and_then(Value::as_str), Some(hexfmt::encode(&root).as_str()));
        assert_eq!(r.member("to").and_then(Value::as_str), Some(hexfmt::encode(&[0x32u8; 20]).as_str()));
        assert_eq!(r.member("tx").and_then(Value::as_str), Some(format!("0x{}", "01".repeat(32)).as_str()));
        let got = accessions_under(&mut Book { book, root, positioned: false }, 1, &book, &root, 0, 100);
        assert!(matches!(got, Err(Refusal::Malformed(Shape::TxLacks { .. }))), "a log without its position: {got:?}");
        assert_eq!(accessions_under(&mut Book { book, root, positioned: true }, 1, &book, &root, 0, 4).map(|v| v.len()), Ok(0), "outside the range");
    }
}
