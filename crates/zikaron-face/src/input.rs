use std::collections::{BTreeMap, BTreeSet};
use zikaron::hexfmt;
use zikaron::json::{self, MAX_INT, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Counted,
    Unproven,
    Void,
}

impl Verdict {
    fn of(s: &str) -> Option<Verdict> {
        match s {
            "counted" => Some(Verdict::Counted),
            "UNPROVEN" => Some(Verdict::Unproven),
            "VOID" => Some(Verdict::Void),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Anchor {
    pub chain: u64,
    pub block: u64,
    pub time: u64,
    pub tx: String,
    pub sender: String,
    pub hash: String,
    pub verdict: Verdict,
}

#[derive(Clone, Debug)]
pub struct Accession {
    pub chain: u64,
    pub block: u64,
    pub time: u64,
    pub tx_index: u64,
    pub log_index: u64,
    pub tx: String,
    pub holder: String,
    pub to: String,
    pub root: String,
}

#[derive(Clone, Debug)]
pub struct Input {
    pub root: String,
    pub chains: Value,
    pub anchors: Vec<Anchor>,
    pub accessions: Vec<Accession>,
    pub pile: Vec<Vec<u8>>,
}

fn need(c: bool) -> Result<(), NoTable> {
    if c { Ok(()) } else { Err(NoTable) }
}

fn exact<'a>(v: &'a Value, keys: &[&str]) -> Result<&'a Value, NoTable> {
    let Value::Obj(ms) = v else { return Err(NoTable) };
    need(ms.len() == keys.len() && keys.iter().all(|k| ms.iter().any(|(m, _)| m == k)))?;
    Ok(v)
}

fn member<'a>(v: &'a Value, k: &str) -> Result<&'a Value, NoTable> {
    v.member(k).ok_or(NoTable)
}

fn uint(v: &Value, k: &str) -> Result<u64, NoTable> {
    match member(v, k)? {
        Value::Int(i) if *i <= MAX_INT => Ok(*i),
        _ => Err(NoTable),
    }
}

fn hex20(v: &Value) -> Result<String, NoTable> {
    match v {
        Value::Str(s) if hexfmt::is_hex20(s) => Ok(s.clone()),
        _ => Err(NoTable),
    }
}

fn hex32(v: &Value) -> Result<String, NoTable> {
    match v {
        Value::Str(s) if hexfmt::is_hex32(s) => Ok(s.clone()),
        _ => Err(NoTable),
    }
}

fn arr<'a>(v: &'a Value, k: &str) -> Result<&'a Vec<Value>, NoTable> {
    member(v, k)?.as_arr().ok_or(NoTable)
}

const INPUT: [&str; 5] = ["root", "chains", "anchors", "accessions", "pile"];
const CHAIN: [&str; 6] = ["chainId", "fromBlock", "toBlock", "registries", "book", "bookFromBlock"];
const ANCHOR: [&str; 7] = ["chainId", "blockNumber", "blockTimestamp", "tx", "sender", "hash", "verdict"];
const ACCESSION: [&str; 9] =
    ["chainId", "blockNumber", "blockTimestamp", "txIndex", "logIndex", "tx", "holder", "to", "root"];

struct Span {
    from: u64,
    to: u64,
    book_from: Option<u64>,
}

pub fn load(raw: &[u8]) -> Result<Input, NoTable> {
    let v = json::parse_tests_1_3(raw).map_err(|_| NoTable)?;
    from_value(&v)
}

pub fn load_pair(raw: &[u8]) -> Result<(Input, Input), NoTable> {
    let v = json::parse_tests_1_3(raw).map_err(|_| NoTable)?;
    match v.as_arr().map(Vec::as_slice) {
        Some([a, b]) => Ok((from_value(a)?, from_value(b)?)),
        _ => Err(NoTable),
    }
}

pub fn from_value(v: &Value) -> Result<Input, NoTable> {
    exact(v, &INPUT)?;
    let root = hex20(member(v, "root")?)?;

    let chains = member(v, "chains")?;
    let mut spans: BTreeMap<u64, Span> = BTreeMap::new();
    let mut last: Option<u64> = None;
    for c in chains.as_arr().ok_or(NoTable)? {
        exact(c, &CHAIN)?;
        let id = uint(c, "chainId")?;
        let from = uint(c, "fromBlock")?;
        let to = uint(c, "toBlock")?;
        need(from <= to)?;
        let mut prev: Option<String> = None;
        for r in arr(c, "registries")? {
            let r = hex20(r)?;
            need(prev.as_ref().is_none_or(|p| *p < r))?;
            prev = Some(r);
        }
        let book_from = match (member(c, "book")?, member(c, "bookFromBlock")?) {
            (Value::Null, Value::Null) => None,
            (Value::Null, _) | (_, Value::Null) => return Err(NoTable),
            (book, _) => {
                hex20(book)?;
                let b = uint(c, "bookFromBlock")?;
                need(b <= to)?;
                Some(b)
            }
        };
        need(last.is_none_or(|l| l < id))?;
        last = Some(id);
        spans.insert(id, Span { from, to, book_from });
    }

    let mut clock: BTreeMap<(u64, u64), u64> = BTreeMap::new();
    let mut tick = |chain: u64, block: u64, time: u64| -> Result<(), NoTable> {
        need(*clock.entry((chain, block)).or_insert(time) == time)
    };

    let mut anchors = Vec::new();
    let mut seen: BTreeMap<(u64, u64, String, String), (u64, String, Verdict)> = BTreeMap::new();
    for r in arr(v, "anchors")? {
        let Value::Obj(ms) = r else { return Err(NoTable) };
        need(ANCHOR.iter().all(|k| ms.iter().any(|(m, _)| m == k)))?;
        let verdict = member(r, "verdict")?.as_str().and_then(Verdict::of).ok_or(NoTable)?;
        let a = Anchor {
            chain: uint(r, "chainId")?,
            block: uint(r, "blockNumber")?,
            time: uint(r, "blockTimestamp")?,
            tx: hex32(member(r, "tx")?)?,
            sender: hex20(member(r, "sender")?)?,
            hash: hex32(member(r, "hash")?)?,
            verdict,
        };
        let span = spans.get(&a.chain).ok_or(NoTable)?;
        need(span.from <= a.block && a.block <= span.to)?;
        let key = (a.chain, a.block, a.tx.clone(), a.hash.clone());
        let rest = (a.time, a.sender.clone(), a.verdict);
        match seen.get(&key) {
            Some(was) => {
                need(*was == rest)?;
                continue;
            }
            None => {
                seen.insert(key, rest);
            }
        }
        tick(a.chain, a.block, a.time)?;
        anchors.push(a);
    }

    let mut accessions = Vec::new();
    let mut seen: BTreeSet<(u64, u64, u64, u64)> = BTreeSet::new();
    for r in arr(v, "accessions")? {
        exact(r, &ACCESSION)?;
        let x = Accession {
            chain: uint(r, "chainId")?,
            block: uint(r, "blockNumber")?,
            time: uint(r, "blockTimestamp")?,
            tx_index: uint(r, "txIndex")?,
            log_index: uint(r, "logIndex")?,
            tx: hex32(member(r, "tx")?)?,
            holder: hex20(member(r, "holder")?)?,
            to: hex20(member(r, "to")?)?,
            root: hex20(member(r, "root")?)?,
        };
        let span = spans.get(&x.chain).ok_or(NoTable)?;
        let book_from = span.book_from.ok_or(NoTable)?;
        need(book_from <= x.block && x.block <= span.to)?;
        need(seen.insert((x.chain, x.block, x.tx_index, x.log_index)))?;
        tick(x.chain, x.block, x.time)?;
        accessions.push(x);
    }

    let mut before: BTreeMap<u64, u64> = BTreeMap::new();
    for (&(chain, _), &time) in &clock {
        need(before.get(&chain).is_none_or(|&t| t <= time))?;
        before.insert(chain, time);
    }

    let mut pile = Vec::new();
    for p in arr(v, "pile")? {
        let Value::Str(s) = p else { return Err(NoTable) };
        pile.push(hexfmt::decode(s).ok_or(NoTable)?);
    }

    Ok(Input { root, chains: chains.clone(), anchors, accessions, pile })
}
