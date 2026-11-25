use crate::rpc::{Broke, Endpoint, ProofGap, Trouble};
use crate::wire::W;
use std::collections::{BTreeMap, BTreeSet};
use zikaron::json::Value;

pub const BATCH_MAX: usize = 20;

pub const HEADER_FIELDS: [&str; 23] = [
    "parentHash",
    "sha3Uncles",
    "miner",
    "stateRoot",
    "transactionsRoot",
    "receiptsRoot",
    "logsBloom",
    "difficulty",
    "number",
    "gasLimit",
    "gasUsed",
    "timestamp",
    "extraData",
    "mixHash",
    "nonce",
    "baseFeePerGas",
    "withdrawalsRoot",
    "blobGasUsed",
    "excessBlobGas",
    "parentBeaconBlockRoot",
    "requestsHash",
    "blockAccessListHash",
    "slotNumber",
];

pub const HEADER_REQUIRED: usize = 15;

const HEADER_SCALARS: [&str; 9] = ["difficulty", "number", "gasLimit", "gasUsed", "timestamp", "baseFeePerGas", "blobGasUsed", "excessBlobGas", "slotNumber"];

const RECEIPT: &str = "eth_getTransactionReceipt";
const BLOCK: &str = "eth_getBlockByHash";
const BY_NUMBER: &str = "eth_getBlockByNumber";
const PROOF: &str = "eth_getProof";
const TX: &str = "eth_getTransactionByHash";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub number: u64,
    pub timestamp: u64,
    pub state_root: [u8; 32],
    pub parent: [u8; 32],
}

#[derive(Default)]
pub struct Held {
    headers: BTreeMap<(u64, [u8; 32]), Header>,
    codes: BTreeMap<(u64, [u8; 32], [u8; 20]), bool>,
    unproven: Vec<(u64, String, ProofGap)>,
    bearing: BTreeMap<u64, usize>,
    turning: BTreeMap<u64, usize>,
    txs_asked: BTreeSet<(String, u64, [u8; 32])>,
    tx_turns: BTreeMap<u64, BTreeSet<usize>>,
    offered: Option<BTreeMap<(u64, [u8; 32]), Vec<crate::proofs::AnchorProof>>>,
    statuses: BTreeSet<(u64, [u8; 32], [u8; 32])>,
    judged: BTreeMap<(u64, [u8; 32], [u8; 32]), Met>,
    by_hash: Vec<ByHash>,
    hashes_read: BTreeSet<[u8; 32]>,
    loading: bool,
    loads: usize,
    facts_proved: BTreeSet<crate::scan::FactKey>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Met {
    Used,
    Missing,
    Failed(crate::proofs::Gap),
}

pub fn offer(proofs: Vec<crate::proofs::AnchorProof>) {
    let _ = crate::action::ahead(|held| offered_into(held, proofs));
}

fn offered_into(held: &mut Held, proofs: Vec<crate::proofs::AnchorProof>) {
    let into = held.offered.get_or_insert_with(BTreeMap::new);
    for p in proofs {
        into.entry((p.chain_id, p.tx_hash())).or_default().push(p);
    }
}

type ByHash = std::sync::Arc<dyn Fn(&[u8; 32]) -> Option<Vec<crate::proofs::AnchorProof>> + Send + Sync>;

pub fn offer_by_hash(load: impl Fn(&[u8; 32]) -> Option<Vec<crate::proofs::AnchorProof>> + Send + Sync + 'static) {
    let _ = crate::action::ahead(|held| held.by_hash.push(std::sync::Arc::new(load)));
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub used: usize,
    pub missing: usize,
    pub failed: Vec<(u64, [u8; 32], crate::proofs::Gap)>,
    pub loads: usize,
}

pub fn tally() -> Tally {
    crate::action::ahead(|held| {
        let mut t = Tally { loads: held.loads, ..Tally::default() };
        for ((chain, _, tx), met) in &held.judged {
            match met {
                Met::Used => t.used += 1,
                Met::Missing => t.missing += 1,
                Met::Failed(g) => t.failed.push((*chain, *tx, *g)),
            }
        }
        t
    })
    .unwrap_or_default()
}

fn take_proofs(chain: u64, wants: &[Want]) {
    if wants.is_empty() {
        return;
    }
    let (loaders, hashes) = crate::action::ahead(|held| {
        if held.loading || held.by_hash.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let mut hashes: Vec<[u8; 32]> = wants.iter().map(|w| w.hash).filter(|h| !held.hashes_read.contains(h)).collect();
        hashes.sort_unstable();
        hashes.dedup();
        held.hashes_read.extend(hashes.iter().copied());
        held.loading = !hashes.is_empty();
        (held.by_hash.clone(), hashes)
    })
    .unwrap_or_default();
    if !hashes.is_empty() {
        let read: Vec<Option<Vec<crate::proofs::AnchorProof>>> = hashes.iter().flat_map(|h| loaders.iter().map(move |load| load(h))).collect();
        let _ = crate::action::ahead_moved(|held| {
            for rows in read.into_iter().flatten() {
                held.loads += 1;
                offered_into(held, rows);
            }
            held.loading = false;
        });
    }
    crate::action::ahead_wait(|held| !held.loading);
    let _ = crate::action::ahead(|held| {
        let Some(offered) = held.offered.as_ref() else { return Vec::new() };
        let mut proved = Vec::new();
        let mut met: Vec<((u64, [u8; 32], [u8; 32]), Met)> = Vec::new();
        for w in wants {
            let key = (chain, w.block, w.tx);
            if held.judged.contains_key(&key) || met.iter().any(|(k, _)| *k == key) {
                continue;
            }
            let outcome = match offered.get(&(chain, w.tx)) {
                None => Met::Missing,
                Some(ps) => {
                    let mut first_gap = None;
                    let mut ok = None;
                    for p in ps {
                        match crate::proofs::check(p, chain, w) {
                            Ok(pr) => {
                                ok = Some(pr);
                                break;
                            }
                            Err(g) => {
                                first_gap.get_or_insert(g);
                            }
                        }
                    }
                    match (ok, first_gap) {
                        (Some(pr), _) => {
                            proved.push((*w, pr));
                            Met::Used
                        }
                        (None, Some(g)) => Met::Failed(g),
                        (None, None) => Met::Missing,
                    }
                }
            };
            met.push((key, outcome));
        }
        for (w, pr) in &proved {
            held.headers.insert((chain, w.block), pr.header);
            if let Some((ph, p)) = pr.parent {
                held.headers.insert((chain, ph), p);
                if let Some(c) = pr.code_before {
                    held.codes.insert((chain, ph, w.who), c);
                }
            }
            if let Some(c) = pr.code_at {
                held.codes.insert((chain, w.block, w.who), c);
            }
            held.statuses.insert((chain, w.block, w.tx));
        }
        held.judged.extend(met);
        proved
    })
    .map(|proved| {
        for (_, pr) in proved {
            crate::action::proven_given(chain, &pr.tx);
        }
    });
}

pub(crate) fn status_at(chain: u64, tx: &[u8; 32]) -> Option<bool> {
    let at = AT.with(|a| a.get())?;
    crate::action::ahead(|held| held.statuses.contains(&(chain, at, *tx)).then_some(true)).flatten()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Want {
    pub tx: [u8; 32],
    pub block: [u8; 32],
    pub number: u64,
    pub who: [u8; 20],
    pub hash: [u8; 32],
}

pub fn header_rlp(block: &W) -> Option<Vec<u8>> {
    let mut items = Vec::with_capacity(HEADER_FIELDS.len());
    for (i, name) in HEADER_FIELDS.iter().enumerate() {
        if block.member(name).is_none_or(|v| v.is_null()) {
            if i < HEADER_REQUIRED {
                return None;
            }
            break;
        }
        let item = if HEADER_SCALARS.contains(name) { crate::rlp::scalar(&crate::tx::qty(block, name)?) } else { crate::rlp::bytes(&crate::tx::data(block, name)?) };
        items.push(item);
    }
    Some(crate::rlp::list(&items))
}

pub fn header_of(block: &W, hash: &[u8; 32]) -> Result<Header, ProofGap> {
    let rlp = header_rlp(block).ok_or(ProofGap::HeaderUnbuilt)?;
    if zikaron::cryptox::keccak256(&rlp) != *hash {
        return Err(ProofGap::HeaderHash);
    }
    let h32 = |k: &str| crate::tx::data(block, k).and_then(|b| <[u8; 32]>::try_from(b).ok());
    match (crate::tx::qty_u64(block, "number"), crate::tx::qty_u64(block, "timestamp"), h32("stateRoot"), h32("parentHash")) {
        (Some(number), Some(timestamp), Some(state_root), Some(parent)) => Ok(Header { number, timestamp, state_root, parent }),
        _ => Err(ProofGap::HeaderUnbuilt),
    }
}

pub fn code_in(root: &[u8; 32], who: &[u8; 20], proof: &[Vec<u8>]) -> Result<bool, ProofGap> {
    let key = zikaron::cryptox::keccak256(who);
    match verify(root, &key, proof).map_err(|()| ProofGap::AccountProof)? {
        None => Ok(false),
        Some(account) => {
            let item = crate::rlp::decode_all(&account).ok_or(ProofGap::AccountProof)?;
            let code_hash = item.list().filter(|l| l.len() == 4).and_then(|l| l[3].bytes()).filter(|b| b.len() == 32).ok_or(ProofGap::AccountProof)?;
            Ok(code_hash != zikaron::cryptox::keccak256(&[]))
        }
    }
}

enum Next {
    Empty,
    Hash([u8; 32]),
    Inline(crate::rlp::Item),
}

fn next_of(child: &crate::rlp::Item) -> Result<Next, ()> {
    match child {
        crate::rlp::Item::Bytes(b) if b.is_empty() => Ok(Next::Empty),
        crate::rlp::Item::Bytes(b) => <[u8; 32]>::try_from(b.as_slice()).map(Next::Hash).map_err(|_| ()),
        crate::rlp::Item::List(_) => Ok(Next::Inline(child.clone())),
    }
}

pub(crate) fn verify(root: &[u8; 32], key: &[u8], proof: &[Vec<u8>]) -> Result<Option<Vec<u8>>, ()> {
    let nibbles: Vec<u8> = key.iter().flat_map(|b| [b >> 4, b & 0x0f]).collect();
    let mut at = 0usize;
    let mut next = Next::Hash(*root);
    let mut given = proof.iter();
    loop {
        let node = match next {
            Next::Empty => return Ok(None),
            Next::Hash(h) => {
                let raw = given.next().ok_or(())?;
                if zikaron::cryptox::keccak256(raw) != h {
                    return Err(());
                }
                crate::rlp::decode_all(raw).ok_or(())?
            }
            Next::Inline(item) => item,
        };
        let items = node.list().ok_or(())?;
        match items.len() {
            17 => {
                if at == nibbles.len() {
                    let v = items[16].bytes().ok_or(())?;
                    return Ok((!v.is_empty()).then(|| v.to_vec()));
                }
                next = next_of(&items[nibbles[at] as usize])?;
                at += 1;
            }
            2 => {
                let path = items[0].bytes().ok_or(())?;
                let Some(first) = path.first() else { return Err(()) };
                let flag = first >> 4;
                if flag > 3 {
                    return Err(());
                }
                let mut p: Vec<u8> = Vec::with_capacity(path.len() * 2);
                if flag & 1 == 1 {
                    p.push(first & 0x0f);
                }
                for b in &path[1..] {
                    p.push(b >> 4);
                    p.push(b & 0x0f);
                }
                let rest = &nibbles[at..];
                if flag & 2 == 2 {
                    return if rest == p.as_slice() { Ok(Some(items[1].bytes().ok_or(())?.to_vec())) } else { Ok(None) };
                }
                if !rest.starts_with(&p) {
                    return Ok(None);
                }
                at += p.len();
                next = next_of(&items[1])?;
                if matches!(next, Next::Empty) {
                    return Err(());
                }
            }
            _ => return Err(()),
        }
    }
}

static UNBATCHED: std::sync::Mutex<BTreeSet<(String, String)>> = std::sync::Mutex::new(BTreeSet::new());

pub const WHOLE: &str = "*";

fn batches(place: &str, method: &str) -> bool {
    let u = UNBATCHED.lock().unwrap_or_else(|e| e.into_inner());
    !u.contains(&(place.to_string(), WHOLE.to_string())) && !u.contains(&(place.to_string(), method.to_string()))
}

fn refused(place: &str, t: &Trouble) {
    if refuses_batches(t) {
        UNBATCHED.lock().unwrap_or_else(|e| e.into_inner()).insert((place.to_string(), WHOLE.to_string()));
    }
}

fn refuses_batches(t: &Trouble) -> bool {
    use crate::patience::{class_of, Class};
    if class_of(t) != Class::Other {
        return false;
    }
    match t {
        Trouble::Node(_) => true,
        Trouble::Transport(Broke::Status { code, .. }) => (400..500).contains(code),
        Trouble::Transport(Broke::Shapeless { .. }) => true,
        _ => false,
    }
}

pub fn unbatched() -> Vec<(String, String)> {
    UNBATCHED.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
}

pub const PARALLEL_START: usize = 4;

struct Halved {
    n: usize,
    until: std::time::Instant,
    length: std::time::Duration,
}

static PARALLEL: std::sync::Mutex<BTreeMap<String, Halved>> = std::sync::Mutex::new(BTreeMap::new());

fn halvings() -> std::sync::MutexGuard<'static, BTreeMap<String, Halved>> {
    PARALLEL.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn parallel(place: &str) -> usize {
    let now = crate::endpoints::limit_clock();
    halvings().get(place).filter(|h| now < h.until).map_or(PARALLEL_START, |h| h.n)
}

fn halve(place: &str) {
    let now = crate::endpoints::limit_clock();
    let mut p = halvings();
    match p.get_mut(place) {
        Some(h) if now < h.until => {
            h.n = (h.n / 2).max(1);
            h.until = now + h.length;
        }
        Some(h) => {
            h.length = (h.length * 2).min(crate::endpoints::REST_MOST);
            h.n = (PARALLEL_START / 2).max(1);
            h.until = now + h.length;
        }
        None => {
            let length = crate::endpoints::REST_FIRST;
            p.insert(place.to_string(), Halved { n: (PARALLEL_START / 2).max(1), until: now + length, length });
        }
    }
}

fn cleared(place: &str) {
    let now = crate::endpoints::limit_clock();
    let mut p = halvings();
    if p.get(place).is_some_and(|h| now >= h.until) {
        p.remove(place);
    }
}

pub fn forget_halvings() {
    halvings().clear();
}

static NO_PROOF: std::sync::Mutex<BTreeSet<String>> = std::sync::Mutex::new(BTreeSet::new());

pub fn proves(place: &str, quick: bool) -> bool {
    !NO_PROOF.lock().unwrap_or_else(|e| e.into_inner()).contains(place) && batches(place, PROOF) && crate::endpoints::resting(place, PROOF).is_none() && !(quick && crate::endpoints::limited_in_action(place, PROOF))
}

fn gives_no_proof(place: &str) {
    NO_PROOF.lock().unwrap_or_else(|e| e.into_inner()).insert(place.to_string());
}

pub fn no_proof() -> Vec<String> {
    let mut all: BTreeSet<String> = NO_PROOF.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
    all.extend(resting_from(PROOF));
    all.into_iter().collect()
}

fn resting_from(method: &str) -> Vec<String> {
    let asked: Vec<String> = ASKED.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
    asked.into_iter().filter(|p| crate::endpoints::resting(p, method).is_some()).collect()
}

static ASKED: std::sync::Mutex<BTreeSet<String>> = std::sync::Mutex::new(BTreeSet::new());

pub fn gives_txs(place: &str, quick: bool) -> bool {
    crate::endpoints::resting(place, TX).is_none() && !(quick && crate::endpoints::limited_in_action(place, TX))
}

pub fn no_tx() -> Vec<String> {
    resting_from(TX)
}

fn gave_none(kind: &[(&Option<Result<W, Trouble>>, bool)]) -> bool {
    !kind.is_empty() && kind.iter().any(|(a, limited)| *limited || limited_here(a)) && !kind.iter().any(|(a, _)| value(a).is_some())
}

fn limit_in(kind: &[(&Option<Result<W, Trouble>>, bool)]) -> Trouble {
    kind.iter().find_map(|(a, _)| match a {
        Some(Err(t)) if crate::patience::class_of(t) == crate::patience::Class::RateLimited => Some(t.clone()),
        _ => None,
    })
    .unwrap_or_else(|| Trouble::Transport(Broke::Status { node: String::new(), code: 429 }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stopped {
    No,
    Limited,
    Other,
}

fn in_batches(ep: &mut dyn Endpoint, asks: &[crate::rpc::Ask<'_>]) -> Option<crate::rpc::Batched> {
    in_batches_said(ep, asks, false).got
}

struct Read {
    got: Option<crate::rpc::Batched>,
    stopped: Stopped,
    limited: Vec<bool>,
}

fn in_batches_said(ep: &mut dyn Endpoint, asks: &[crate::rpc::Ask<'_>], quick: bool) -> Read {
    let place = ep.place();
    ASKED.lock().unwrap_or_else(|e| e.into_inner()).insert(place.clone());
    let left: Vec<usize> = (0..asks.len()).filter(|i| batches(&place, asks[*i].0)).collect();
    if left.len() < 2 {
        return Read { got: None, stopped: Stopped::No, limited: vec![false; asks.len()] };
    }
    let laid: Vec<Vec<usize>> = left.chunks(BATCH_MAX).map(<[usize]>::to_vec).collect();
    let tables: Vec<std::sync::Mutex<std::vec::IntoIter<std::time::Duration>>> = laid.iter().map(|_| std::sync::Mutex::new(crate::patience::pauses(crate::patience::Class::RateLimited).into_iter())).collect();
    let mut ways: Vec<Box<dyn Endpoint + Send>> = Vec::new();
    while 1 + ways.len() < parallel(&place).min(laid.len()) {
        match ep.another() {
            Some(w) => ways.push(w),
            None => break,
        }
    }
    struct Part {
        idx: Vec<usize>,
        table: usize,
        split: u8,
    }
    #[derive(Default)]
    struct Cap {
        held: Option<Vec<usize>>,
        answered: bool,
        stop: bool,
    }
    struct Queue {
        parts: std::collections::VecDeque<Part>,
        stopped: Stopped,
        any: bool,
        out: usize,
        caps: Vec<Cap>,
    }
    let caps = laid.iter().map(|_| Cap::default()).collect();
    let queue = std::sync::Mutex::new(Queue { parts: laid.into_iter().enumerate().map(|(table, idx)| Part { idx, table, split: 0 }).collect(), stopped: Stopped::No, any: false, out: 0, caps });
    let limited: std::sync::Mutex<Vec<bool>> = std::sync::Mutex::new(vec![false; asks.len()]);
    let leave = |idx: &[usize]| {
        let mut l = limited.lock().unwrap_or_else(|e| e.into_inner());
        for i in idx {
            l[*i] = true;
        }
    };
    let halves = |q: &mut Queue, idx: &[usize], table: usize, split: u8| {
        if idx.len() >= 2 {
            let (a, b) = idx.split_at(idx.len() / 2);
            let next = split.saturating_add(1);
            q.parts.push_front(Part { idx: b.to_vec(), table, split: next });
            q.parts.push_front(Part { idx: a.to_vec(), table, split: next });
        } else {
            leave(idx);
        }
    };
    let rest_all = |idx: &[usize]| {
        let said = Trouble::Transport(Broke::Status { node: place.clone(), code: 429 });
        for i in idx {
            let t = crate::endpoints::resting(&place, asks[*i].0).unwrap_or_else(|| said.clone());
            crate::endpoints::rest(&place, asks[*i].0, &t);
        }
    };
    let back = std::sync::Condvar::new();
    let out: std::sync::Mutex<crate::rpc::Batched> = std::sync::Mutex::new((0..asks.len()).map(|_| None).collect());
    let lock_queue = || queue.lock().unwrap_or_else(|e| e.into_inner());
    let work = |way: usize, ep: &mut dyn Endpoint| loop {
        let part = {
            let mut q = lock_queue();
            loop {
                if q.stopped != Stopped::No || way >= parallel(&place) {
                    return;
                }
                if let Some(p) = q.parts.pop_front() {
                    q.out += 1;
                    break p;
                }
                if q.out == 0 {
                    return;
                }
                q = back.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        let done = |q: &mut Queue| {
            q.out -= 1;
            back.notify_all();
        };
        let idx: Vec<usize> = part.idx.iter().copied().filter(|i| batches(&place, asks[*i].0)).collect();
        if idx.is_empty() {
            done(&mut lock_queue());
            continue;
        }
        let these: Vec<crate::rpc::Ask<'_>> = idx.iter().map(|i| (asks[*i].0, asks[*i].1.clone())).collect();
        let got = match one_batch(ep, &place, &these, &tables[part.table], quick) {
            Ok((got, left)) => {
                let left_idx: Vec<usize> = left.iter().map(|k| idx[*k]).collect();
                leave(&left_idx);
                if !quick {
                    let dry: Vec<usize> = left_idx.iter().copied().filter(|i| !idx.iter().zip(&got).any(|(j, a)| asks[*j].0 == asks[*i].0 && value(a).is_some())).collect();
                    rest_all(&dry);
                }
                if quick && !left.is_empty() {
                    let mut q = lock_queue();
                    if q.stopped == Stopped::No {
                        q.stopped = Stopped::Limited;
                    }
                }
                got
            }
            Err(Stopped::Limited) if quick => {
                leave(&idx);
                let mut q = lock_queue();
                if q.stopped == Stopped::No {
                    q.stopped = Stopped::Limited;
                }
                done(&mut q);
                return;
            }
            Err(Stopped::Limited) => {
                let mut q = lock_queue();
                let t = part.table;
                match part.split {
                    0 => halves(&mut q, &idx, t, 0),
                    1 if q.caps[t].stop => leave(&idx),
                    1 if q.caps[t].answered => halves(&mut q, &idx, t, 1),
                    1 => match q.caps[t].held.take() {
                        Some(sibling) => {
                            q.caps[t].stop = true;
                            leave(&sibling);
                            leave(&idx);
                            rest_all(&sibling);
                            rest_all(&idx);
                        }
                        None => q.caps[t].held = Some(idx),
                    },
                    n if !q.caps[t].stop => halves(&mut q, &idx, t, n),
                    _ => leave(&idx),
                }
                done(&mut q);
                continue;
            }
            Err(why) => {
                let mut q = lock_queue();
                if q.stopped == Stopped::No {
                    q.stopped = why;
                }
                done(&mut q);
                return;
            }
        };
        let mut methods: Vec<&str> = these.iter().map(|(m, _)| *m).collect();
        methods.sort_unstable();
        methods.dedup();
        for m in methods {
            let of_m: Vec<&Option<Result<W, Trouble>>> = these.iter().zip(got.iter()).filter(|((x, _), _)| *x == m).map(|(_, a)| a).collect();
            if of_m.len() >= 2 && of_m.iter().all(|a| matches!(a, Some(Err(t @ Trouble::Node(_))) if refuses_batches(t))) {
                UNBATCHED.lock().unwrap_or_else(|e| e.into_inner()).insert((place.clone(), m.to_string()));
            }
        }
        let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
        for (i, a) in idx.into_iter().zip(got) {
            o[i] = a;
        }
        drop(o);
        let mut q = lock_queue();
        q.any = true;
        if part.split == 1 {
            let t = part.table;
            q.caps[t].answered = true;
            if let Some(h) = q.caps[t].held.take() {
                halves(&mut q, &h, t, 1);
            }
        }
        done(&mut q);
    };
    let action = crate::action::current();
    std::thread::scope(|s| {
        for (j, mut way) in ways.into_iter().enumerate() {
            let (work, action) = (&work, action.clone());
            let _ = std::thread::Builder::new().name(format!("zikaron-batch-{}", j + 1)).spawn_scoped(s, move || crate::action::within(action, || work(j + 1, way.as_mut())));
        }
        work(0, ep);
    });
    let q = queue.into_inner().unwrap_or_else(|e| e.into_inner());
    for c in &q.caps {
        if let Some(h) = &c.held {
            leave(h);
        }
    }
    let out = out.into_inner().unwrap_or_else(|e| e.into_inner());
    let limited = limited.into_inner().unwrap_or_else(|e| e.into_inner());
    let stopped = if q.stopped == Stopped::No && limited.iter().any(|l| *l) { Stopped::Limited } else { q.stopped };
    Read { got: q.any.then_some(out), stopped, limited }
}

fn limited_here(a: &Option<Result<W, Trouble>>) -> bool {
    matches!(a, Some(Err(t)) if crate::patience::class_of(t) == crate::patience::Class::RateLimited)
}

fn one_batch(ep: &mut dyn Endpoint, place: &str, part: &[crate::rpc::Ask<'_>], table: &std::sync::Mutex<std::vec::IntoIter<std::time::Duration>>, quick: bool) -> Result<(crate::rpc::Batched, Vec<usize>), Stopped> {
    let mut got = posted(ep, place, part, table, quick)?;
    let mut asked: Vec<usize> = (0..part.len()).collect();
    let mut stay: Vec<usize> = Vec::new();
    loop {
        let limited: Vec<usize> = asked.iter().copied().filter(|i| limited_here(&got[*i])).collect();
        for i in &limited {
            if let Some(Err(t)) = &got[*i] {
                crate::endpoints::note_limited(place, part[*i].0, t);
            }
        }
        if limited.is_empty() && stay.is_empty() {
            cleared(place);
        }
        let (again, resting): (Vec<usize>, Vec<usize>) = limited.into_iter().partition(|i| crate::endpoints::resting(place, part[*i].0).is_none());
        stay.extend(resting);
        let left = |again: Vec<usize>, stay: &[usize]| {
            let mut l: Vec<usize> = again.into_iter().chain(stay.iter().copied()).collect();
            l.sort_unstable();
            l
        };
        if again.is_empty() || quick {
            return Ok((got, left(again, &stay)));
        }
        halve(place);
        let Some(next) = table.lock().unwrap_or_else(|e| e.into_inner()).next() else {
            return Ok((got, left(again, &stay)));
        };
        let pause = crate::patience::waited(next);
        if !pause.is_zero() {
            std::thread::sleep(pause);
        }
        let these: Vec<crate::rpc::Ask<'_>> = again.iter().map(|i| (part[*i].0, part[*i].1.clone())).collect();
        match posted(ep, place, &these, table, false) {
            Ok(answers) => {
                for (i, a) in again.iter().zip(answers) {
                    got[*i] = a;
                }
                asked = again;
            }
            Err(_) => return Ok((got, left(again, &stay))),
        }
    }
}

fn posted(ep: &mut dyn Endpoint, place: &str, part: &[crate::rpc::Ask<'_>], table: &std::sync::Mutex<std::vec::IntoIter<std::time::Duration>>, quick: bool) -> Result<crate::rpc::Batched, Stopped> {
    loop {
        match ep.call_batch(part).ok_or(Stopped::Other)? {
            Ok(got) => return Ok(got),
            Err(t) if crate::patience::class_of(&t) == crate::patience::Class::RateLimited => {
                for (m, _) in part {
                    crate::endpoints::note_limited(place, m, &t);
                }
                halve(place);
                if quick {
                    return Err(Stopped::Limited);
                }
                let next = table.lock().unwrap_or_else(|e| e.into_inner()).next();
                let pause = crate::patience::waited(next.ok_or(Stopped::Limited)?);
                if !pause.is_zero() {
                    std::thread::sleep(pause);
                }
            }
            Err(t) => {
                refused(place, &t);
                return Err(Stopped::Other);
            }
        }
    }
}

fn asked_few(ep: &mut dyn Endpoint, asks: &[crate::rpc::Ask<'_>]) -> (crate::rpc::Batched, Stopped, Vec<bool>) {
    if let [(m, p)] = asks {
        let a = Some(ep.call(m, p));
        let limited = limited_here(&a);
        return (vec![a], Stopped::No, vec![limited]);
    }
    let r = in_batches_said(ep, asks, false);
    let limited = r.limited;
    (r.got.unwrap_or_else(|| (0..asks.len()).map(|_| None).collect()), r.stopped, limited)
}

fn value(a: &Option<Result<W, Trouble>>) -> Option<&W> {
    a.as_ref().and_then(|r| r.as_ref().ok()).filter(|w| !w.is_null())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Share {
    pub k: usize,
    pub n: usize,
}

impl Share {
    pub const WHOLE: Share = Share { k: 0, n: 1 };

    pub fn of(place: &str, list: Option<&str>) -> Share {
        let Some(list) = list else { return Share::WHOLE };
        let places: Vec<&str> = list.split(' ').filter(|p| !p.is_empty()).collect();
        match places.iter().position(|p| *p == place) {
            Some(k) if places.len() > 1 => Share { k, n: places.len() },
            _ => Share::WHOLE,
        }
    }

    fn holds(self, i: usize) -> bool {
        i % self.n.max(1) == self.k
    }
}

pub fn read_ahead(ep: &mut dyn Endpoint, chain: u64, wants: &[Want], share: Share) {
    read_ahead_receipts(ep, chain, wants, &[], share)
}

pub fn read_ahead_receipts(ep: &mut dyn Endpoint, chain: u64, wants: &[Want], receipts: &[[u8; 32]], share: Share) {
    if (wants.is_empty() && receipts.is_empty()) || crate::action::current().is_none() {
        return;
    }
    if wants.is_empty() {
        let asks: Vec<crate::rpc::Ask<'_>> = receipts.iter().map(|h| (RECEIPT, Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(h))]))).collect();
        let _ = in_batches(ep, &asks);
        return;
    }
    let bearing = Bearing::begin(chain);
    let turning = Turning::begin(chain);
    let tx_turn = (share.n > 1).then(|| TxTurn::begin(chain, share.k));
    take_proofs(chain, wants);
    let receipt_proved = |w: &Want| crate::action::ahead(|held| held.statuses.contains(&(chain, w.block, w.tx))).unwrap_or(false);
    let mut txs: Vec<[u8; 32]> = wants.iter().filter(|w| !receipt_proved(w)).map(|w| w.tx).chain(receipts.iter().copied()).collect();
    txs.sort_unstable();
    txs.dedup();
    let receipts: Vec<crate::rpc::Ask<'_>> = txs.iter().map(|h| (RECEIPT, Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(h))]))).collect();
    let _ = in_batches(ep, &receipts);
    let mut blocks: Vec<[u8; 32]> = Vec::new();
    for w in wants {
        if !blocks.contains(&w.block) {
            blocks.push(w.block);
        }
    }
    let holds = |w: &Want| blocks.iter().position(|b| *b == w.block).is_some_and(|i| share.holds(i));
    let mine: Vec<Want> = wants.iter().filter(|w| holds(w)).copied().collect();
    prove(ep, chain, &mine, wants, share.n > 1);
    drop(bearing);
    crate::action::ahead_wait(|held| held.bearing.get(&chain).copied().unwrap_or(0) == 0);
    let theirs: Vec<Want> = wants.iter().filter(|w| !holds(w)).copied().collect();
    let cands: Vec<[u8; 32]> = wants.iter().map(|w| w.tx).collect();
    if !theirs.is_empty() {
        let _ = crate::action::ahead_turn(|| prove(ep, chain, wants, wants, share.n > 1));
    }
    if let Some(turn) = tx_turn {
        crate::action::ahead_wait(|held| held.tx_turns.get(&chain).is_none_or(|ks| ks.iter().all(|k| *k >= share.k)));
        ask_txs_ahead(ep, chain, &cands, &[], true);
        drop(turn);
    }
    drop(turning);
    crate::action::ahead_wait(|held| held.turning.get(&chain).copied().unwrap_or(0) == 0);
    ask_txs_ahead(ep, chain, &cands, &still_asked(chain, wants), false);
}

struct TxTurn(u64, usize);

impl TxTurn {
    fn begin(chain: u64, k: usize) -> TxTurn {
        let _ = crate::action::ahead_moved(|held| held.tx_turns.entry(chain).or_default().insert(k));
        TxTurn(chain, k)
    }
}

impl Drop for TxTurn {
    fn drop(&mut self) {
        let (chain, k) = (self.0, self.1);
        let _ = crate::action::ahead_moved(|held| {
            if let Some(ks) = held.tx_turns.get_mut(&chain) {
                ks.remove(&k);
            }
        });
    }
}

struct Turning(u64);

impl Turning {
    fn begin(chain: u64) -> Turning {
        let _ = crate::action::ahead_moved(|held| *held.turning.entry(chain).or_default() += 1);
        Turning(chain)
    }
}

impl Drop for Turning {
    fn drop(&mut self) {
        let chain = self.0;
        let _ = crate::action::ahead_moved(|held| {
            let n = held.turning.entry(chain).or_default();
            *n = n.saturating_sub(1);
        });
    }
}

struct Bearing(u64);

impl Bearing {
    fn begin(chain: u64) -> Bearing {
        let _ = crate::action::ahead_moved(|held| *held.bearing.entry(chain).or_default() += 1);
        Bearing(chain)
    }
}

impl Drop for Bearing {
    fn drop(&mut self) {
        let chain = self.0;
        let _ = crate::action::ahead_moved(|held| {
            let n = held.bearing.entry(chain).or_default();
            *n = n.saturating_sub(1);
        });
    }
}

fn still_asked(chain: u64, wants: &[Want]) -> Vec<crate::rpc::Ask<'static>> {
    let mut asks: Vec<crate::rpc::Ask<'static>> = Vec::new();
    let mut put = |a: crate::rpc::Ask<'static>| {
        if !asks.contains(&a) {
            asks.push(a);
        }
    };
    let quantity = |n: u64| Value::Str(crate::scan::hex_quantity(n));
    for w in wants {
        let _at = reading_at(Some(w.block));
        if time_at(chain, w.number).is_none() {
            put((BY_NUMBER, Value::Arr(vec![quantity(w.number), Value::Bool(false)])));
        }
        for n in [w.number.checked_sub(1), Some(w.number)].into_iter().flatten() {
            if code_at(chain, &w.who, n).is_none() {
                put(("eth_getCode", Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(&w.who)), quantity(n)])));
            }
        }
    }
    asks
}

pub fn ask_ahead(ep: &mut dyn Endpoint, asks: &[crate::rpc::Ask<'_>]) {
    if crate::action::current().is_none() {
        return;
    }
    let _ = in_batches(ep, asks);
}

pub fn ask_txs_ahead(ep: &mut dyn Endpoint, chain: u64, txs: &[[u8; 32]], rest: &[crate::rpc::Ask<'_>], quick: bool) {
    if crate::action::current().is_none() {
        return;
    }
    let place = ep.place();
    let asked_here = |tx: &[u8; 32]| crate::action::ahead(|held| held.txs_asked.contains(&(place.clone(), chain, *tx))).unwrap_or(false);
    let wanted: Vec<[u8; 32]> = if gives_txs(&place, quick) { txs.iter().filter(|h| !asked_here(h)).copied().collect() } else { Vec::new() };
    let mut claim = crate::action::claim_ahead(chain, &wanted);
    let taken: Vec<[u8; 32]> = claim.hashes().to_vec();
    let _ = crate::action::ahead(|held| held.txs_asked.extend(taken.iter().map(|h| (place.clone(), chain, *h))));
    let asks: Vec<crate::rpc::Ask<'_>> = taken.iter().map(|h| (TX, Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(h))]))).chain(rest.iter().map(|(m, p)| (*m, p.clone()))).collect();
    if asks.is_empty() {
        return;
    }
    let (got, limited): (crate::rpc::Batched, Vec<bool>) = match asks.as_slice() {
        [(m, p)] if *m == TX => {
            let a = Some(if quick { ep.call(m, p) } else { crate::patience::ask(ep, m, p) });
            if let Some(Err(t)) = &a {
                if crate::patience::class_of(t) == crate::patience::Class::RateLimited {
                    crate::endpoints::note_limited(&place, TX, t);
                }
            }
            let l = limited_here(&a);
            (vec![a], vec![l])
        }
        _ => {
            let r = in_batches_said(ep, &asks, quick);
            let limited = r.limited;
            (r.got.unwrap_or_else(|| (0..asks.len()).map(|_| None).collect()), limited)
        }
    };
    for (h, a) in taken.iter().zip(&got) {
        let Some(w) = value(a) else { continue };
        match crate::tx::read(w) {
            Ok(t) if t.hash == *h && !t.raw.is_empty() => claim.proved(&t),
            _ => claim.not_itself(h),
        }
    }
    drop(claim);
    let kind: Vec<(&Option<Result<W, Trouble>>, bool)> = got.iter().zip(limited).take(taken.len()).collect();
    if !quick && gave_none(&kind) {
        let said = limit_in(&kind);
        crate::endpoints::rest(&place, TX, &said);
    }
}

fn prove(ep: &mut dyn Endpoint, chain: u64, wants: &[Want], all: &[Want], quick: bool) {
    let place = ep.place();
    let held_header = |h: &[u8; 32]| crate::action::ahead(|held| held.headers.get(&(chain, *h)).copied()).flatten();
    let blame = |gap: ProofGap| {
        let _ = crate::action::ahead(|held| held.unproven.push((chain, place.clone(), gap)));
    };
    let code_known = |hash: &[u8; 32], who: &[u8; 20]| crate::action::ahead(|held| held.codes.contains_key(&(chain, *hash, *who))).unwrap_or(true);
    let blocking = !(quick && crate::endpoints::limited_in_action(&place, BLOCK));
    let proving = blocking && proves(&place, quick);
    let mut blocks: Vec<([u8; 32], u64)> = Vec::new();
    let mut proofs: Vec<(u64, [u8; 20])> = Vec::new();
    let own_of_another = |number: u64, who: &[u8; 20]| all.iter().any(|o| o.number == number && o.who == *who);
    for w in wants {
        let header = held_header(&w.block);
        if blocking && header.is_none() && !blocks.contains(&(w.block, w.number)) {
            blocks.push((w.block, w.number));
        }
        if !proving {
            continue;
        }
        if !(header.is_some() && code_known(&w.block, &w.who)) && !proofs.contains(&(w.number, w.who)) {
            proofs.push((w.number, w.who));
        }
        let parent_code = header.is_some_and(|h| held_header(&h.parent).is_some() && code_known(&h.parent, &w.who));
        if w.number > 0 && !parent_code && !own_of_another(w.number - 1, &w.who) && !proofs.contains(&(w.number - 1, w.who)) {
            proofs.push((w.number - 1, w.who));
        }
    }
    let quantity = |n: u64| Value::Str(crate::scan::hex_quantity(n));
    let asks: Vec<crate::rpc::Ask<'_>> = blocks
        .iter()
        .map(|(h, _)| (BLOCK, Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(h)), Value::Bool(false)])))
        .chain(proofs.iter().map(|(n, who)| (PROOF, Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(who)), Value::Arr(Vec::new()), quantity(*n)]))))
        .collect();
    if asks.is_empty() {
        return;
    }
    let read = in_batches_said(ep, &asks, quick);
    let got = read.got.unwrap_or_else(|| (0..asks.len()).map(|_| None).collect());
    let (got_blocks, got_proofs) = got.split_at(blocks.len());
    let proofs_limited = &read.limited[blocks.len()..];
    for ((hash, number), a) in blocks.iter().zip(got_blocks) {
        let Some(block) = value(a) else { continue };
        if held_header(hash).is_some() {
            continue;
        }
        match header_of(block, hash) {
            Ok(h) if h.number == *number => {
                let _ = crate::action::ahead(|held| held.headers.insert((chain, *hash), h));
            }
            Ok(_) => blame(ProofGap::HeaderUnread),
            Err(gap) => blame(gap),
        }
    }
    let refused_own = got_proofs.iter().filter(|a| matches!(a, Some(Err(t @ Trouble::Node(_))) if refuses_batches(t))).count();
    let none_given = (refused_own >= 2 && refused_own == got_proofs.len()) || got_proofs.iter().any(|a| value(a).is_some_and(|w| w.member("accountProof").is_none()));
    let proof_kind: Vec<(&Option<Result<W, Trouble>>, bool)> = got_proofs.iter().zip(proofs_limited.iter().copied()).collect();
    if none_given {
        gives_no_proof(&place);
    } else if !quick && gave_none(&proof_kind) {
        let said = limit_in(&proof_kind);
        crate::endpoints::rest(&place, PROOF, &said);
    }
    let header_at = |number: u64, who: &[u8; 20]| -> Option<([u8; 32], Header)> {
        wants
            .iter()
            .filter(|w| w.who == *who)
            .filter_map(|w| held_header(&w.block).map(|h| (w.block, h)))
            .flat_map(|(hash, h)| [Some((hash, h)), held_header(&h.parent).map(|p| (h.parent, p))])
            .flatten()
            .find(|(_, h)| h.number == number)
    };
    let child_held = |number: u64| wants.iter().any(|w| w.number == number + 1 && held_header(&w.block).is_some_and(|h| held_header(&h.parent).is_none()));
    let mut parents: Vec<u64> = Vec::new();
    for ((number, who), a) in proofs.iter().zip(got_proofs) {
        if value(a).is_some() && header_at(*number, who).is_none() && child_held(*number) && !parents.contains(number) {
            parents.push(*number);
        }
    }
    if !parents.is_empty() {
        let asks: Vec<crate::rpc::Ask<'_>> = parents.iter().map(|n| (BY_NUMBER, Value::Arr(vec![quantity(*n), Value::Bool(false)]))).collect();
        let (got_parents, _, _) = asked_few(ep, &asks);
        for (number, a) in parents.iter().zip(&got_parents) {
            let Some(block) = value(a) else { continue };
            let children: Vec<Header> = wants.iter().filter(|w| w.number == number + 1).filter_map(|w| held_header(&w.block)).collect();
            let Some(child) = children.first() else { continue };
            if held_header(&child.parent).is_some() {
                continue;
            }
            match header_of(block, &child.parent) {
                Ok(h) if h.number == *number => {
                    let _ = crate::action::ahead(|held| held.headers.insert((chain, child.parent), h));
                }
                Ok(_) => blame(ProofGap::HeaderUnread),
                Err(gap) => blame(gap),
            }
        }
    }
    for ((number, who), a) in proofs.iter().zip(got_proofs) {
        let Some(answer) = value(a) else { continue };
        let Some((hash, header)) = header_at(*number, who) else { continue };
        if code_known(&hash, who) {
            continue;
        }
        let proof: Option<Vec<Vec<u8>>> = answer.member("accountProof").and_then(|p| p.as_arr()).and_then(|items| items.iter().map(|x| x.as_str().and_then(zikaron::hexfmt::decode)).collect());
        match proof.ok_or(ProofGap::NoAccountProof).and_then(|p| code_in(&header.state_root, who, &p)) {
            Ok(coded) => {
                let _ = crate::action::ahead(|held| held.codes.insert((chain, hash, *who), coded));
            }
            Err(gap) => blame(gap),
        }
    }
}

thread_local! {
    static AT: std::cell::Cell<Option<[u8; 32]>> = const { std::cell::Cell::new(None) };
}

pub(crate) fn reading_at(hash: Option<[u8; 32]>) -> impl Drop {
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            AT.with(|a| a.set(None));
        }
    }
    AT.with(|a| a.set(hash));
    Clear
}

pub(crate) fn proved_here(chain: u64, who: &[u8; 20], number: u64) -> bool {
    time_at(chain, number).is_some() && code_at(chain, who, number).is_some() && (number == 0 || code_at(chain, who, number - 1).is_some())
}

pub(crate) fn note_proved(key: crate::scan::FactKey) {
    let _ = crate::action::ahead(|held| held.facts_proved.insert(key));
}

pub fn fact_proved(key: &crate::scan::FactKey) -> bool {
    crate::action::ahead(|held| held.facts_proved.contains(key)).unwrap_or(false)
}

pub(crate) fn time_at(chain: u64, number: u64) -> Option<u64> {
    let at = AT.with(|a| a.get())?;
    crate::action::ahead(|held| held.headers.get(&(chain, at)).filter(|h| h.number == number).map(|h| h.timestamp)).flatten()
}

pub(crate) fn code_at(chain: u64, who: &[u8; 20], number: u64) -> Option<bool> {
    let at = AT.with(|a| a.get())?;
    crate::action::ahead(|held| {
        let h = held.headers.get(&(chain, at))?;
        let hash = if h.number == number { at } else if number + 1 == h.number { h.parent } else { return None };
        held.codes.get(&(chain, hash, *who)).copied()
    })
    .flatten()
}

pub fn unproven() -> Vec<(u64, String, ProofGap)> {
    crate::action::ahead(|held| held.unproven.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests;
