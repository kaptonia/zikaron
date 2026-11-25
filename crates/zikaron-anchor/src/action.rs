use crate::settled::Keep;
use crate::wire::W;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use zikaron::json::Value;

type Question = (String, String, Vec<u8>);

#[derive(Default)]
struct Inner {
    keeper: Mutex<Option<Arc<dyn Keep>>>,
    answers: Mutex<BTreeMap<Question, (W, bool)>>,
    once: Mutex<BTreeMap<Question, W>>,
    facts: Mutex<BTreeMap<&'static str, crate::scan::Known>>,
    proven: Mutex<BTreeMap<(u64, [u8; 32]), Slot>>,
    read_out: std::sync::Condvar,
    floors: Mutex<BTreeMap<String, u64>>,
    broadcast_heads: Mutex<BTreeMap<[u8; 32], u64>>,
    ahead: Mutex<crate::ahead::Held>,
    ahead_moved: std::sync::Condvar,
    ahead_turn: Mutex<()>,
    judged: Mutex<BTreeMap<crate::settled::Key, crate::settled::Entry>>,
    judged_keeper: Mutex<Option<Arc<dyn Keep>>>,
    opened: OpenedAt,
    split: Mutex<BTreeMap<String, usize>>,
    heard: Mutex<BTreeMap<String, String>>,
    asks: Mutex<Option<u64>>,
    behind: bool,
}

#[derive(Default)]
struct Lived {
    record: Mutex<Record>,
    front: FrontMark,
}

#[derive(Clone)]
enum Life {
    Owner(Arc<Lived>),
    Worker(std::sync::Weak<Lived>),
}

static FRONT: Mutex<usize> = Mutex::new(0);
static FRONT_GONE: std::sync::Condvar = std::sync::Condvar::new();

#[derive(Default)]
struct FrontMark(std::sync::atomic::AtomicBool);

impl Drop for FrontMark {
    fn drop(&mut self) {
        if self.0.load(std::sync::atomic::Ordering::SeqCst) {
            let mut n = FRONT.lock().unwrap_or_else(|e| e.into_inner());
            *n = n.saturating_sub(1);
            FRONT_GONE.notify_all();
        }
    }
}

pub fn give_way() {
    let Some(a) = current() else { return };
    if a.0.behind {
        let mut n = FRONT.lock().unwrap_or_else(|e| e.into_inner());
        if *n > 0 {
            WAITING.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            while *n > 0 {
                n = FRONT_GONE.wait(n).unwrap_or_else(|e| e.into_inner());
            }
            WAITING.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    } else if a.lived().is_some_and(|l| !l.front.0.swap(true, std::sync::atomic::Ordering::SeqCst)) {
        *FRONT.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    }
}

static WAITING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn giving_way() -> usize {
    WAITING.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn in_front() -> usize {
    *FRONT.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn is_behind() -> bool {
    current().is_some_and(|a| a.0.behind)
}

struct OpenedAt(std::time::Instant);

impl Default for OpenedAt {
    fn default() -> OpenedAt {
        OpenedAt(crate::endpoints::limit_clock())
    }
}

#[derive(Default)]
struct Record {
    notes: Option<crate::settled::Notes>,
    entries: BTreeMap<crate::settled::Key, Option<crate::settled::Entry>>,
    pending: Vec<crate::settled::Pending>,
}

impl Drop for Record {
    fn drop(&mut self) {
        crate::settled::flush(std::mem::take(&mut self.pending));
    }
}

#[derive(Clone)]
enum Slot {
    Asking,
    Proved(crate::tx::Tx),
    Each,
}

#[derive(Clone)]
pub struct Action(Arc<Inner>, Life);

impl Action {
    fn new(inner: Inner) -> Action {
        Action(Arc::new(inner), Life::Owner(Arc::new(Lived::default())))
    }

    fn lived(&self) -> Option<Arc<Lived>> {
        match &self.1 {
            Life::Owner(l) => Some(l.clone()),
            Life::Worker(w) => w.upgrade(),
        }
    }

    pub fn worker(&self) -> Action {
        let weak = match &self.1 {
            Life::Owner(l) => Arc::downgrade(l),
            Life::Worker(w) => w.clone(),
        };
        Action(self.0.clone(), Life::Worker(weak))
    }
}

thread_local! {
    static NOW: std::cell::RefCell<Option<Action>> = const { std::cell::RefCell::new(None) };
}

#[must_use]
pub struct Opened {
    before: Option<Action>,
}

impl Drop for Opened {
    fn drop(&mut self) {
        let before = self.before.take();
        NOW.with(|n| *n.borrow_mut() = before);
    }
}

pub fn open(keeper: Option<Arc<dyn Keep>>) -> Opened {
    let a = Action::new(Inner { keeper: Mutex::new(keeper), ..Inner::default() });
    let before = NOW.with(|n| n.borrow_mut().replace(a));
    Opened { before }
}

pub fn open_behind(keeper: Option<Arc<dyn Keep>>) -> Opened {
    let a = Action::new(Inner { keeper: Mutex::new(keeper), behind: true, ..Inner::default() });
    let before = NOW.with(|n| n.borrow_mut().replace(a));
    Opened { before }
}

pub fn fresh(keeper: Option<Arc<dyn Keep>>) -> Action {
    Action::new(Inner { keeper: Mutex::new(keeper), ..Inner::default() })
}

pub fn enter(action: Action) -> Opened {
    let before = NOW.with(|n| n.borrow_mut().replace(action));
    Opened { before }
}

pub fn current() -> Option<Action> {
    NOW.with(|n| n.borrow().clone())
}

pub fn split_widths(places: &[String]) -> Cutting {
    let own = current().is_none().then(|| open(None));
    let action = current();
    if let Some(a) = &action {
        let mut split = a.0.split.lock().unwrap_or_else(|e| e.into_inner());
        for p in places {
            *split.entry(p.clone()).or_insert(0) += 1;
        }
    }
    Cutting { action, places: places.to_vec(), _own: own }
}

pub const ASKS_DEFAULT: u64 = 256;
pub const ASKS_MIN: u64 = 1;
pub const ASKS_MAX: u64 = 1000;

static ASKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(ASKS_DEFAULT);

pub fn asks_within(n: u64) -> bool {
    (ASKS_MIN..=ASKS_MAX).contains(&n)
}

pub fn take_asks_cap(n: u64) {
    ASKS.store(if asks_within(n) { n } else { ASKS_DEFAULT }, std::sync::atomic::Ordering::SeqCst);
}

pub fn asks_cap() -> u64 {
    current().and_then(|a| *a.0.asks.lock().unwrap_or_else(|e| e.into_inner())).unwrap_or_else(|| ASKS.load(std::sync::atomic::Ordering::SeqCst))
}

pub fn ask_at_most(n: u64) {
    if let Some(a) = current() {
        *a.0.asks.lock().unwrap_or_else(|e| e.into_inner()) = Some(n.clamp(ASKS_MIN, ASKS_MAX));
    }
}

pub fn cut_again<T>(got: &[T], answered: impl Fn(&T) -> bool, refused: impl Fn(&T) -> bool) -> Vec<usize> {
    if got.iter().any(answered) {
        return Vec::new();
    }
    (0..got.len()).filter(|i| refused(&got[*i])).collect()
}

pub fn read_while_cut<'a>(places: impl IntoIterator<Item = &'a str>) -> bool {
    places.into_iter().any(splits)
}

pub fn cut_and_ask<G>(got: &mut [G], places: impl Fn(usize, &G) -> Vec<String>, answered: impl Fn(&G) -> bool, refused: impl Fn(&G) -> bool, ask: impl FnOnce(&[usize], &mut [G]) -> Vec<G>) {
    let again = cut_again(got, answered, refused);
    if again.is_empty() {
        return;
    }
    let spots: Vec<String> = again.iter().flat_map(|i| places(*i, &got[*i])).collect();
    let _cut = split_widths(&spots);
    let redo = ask(&again, got);
    for (i, g) in again.into_iter().zip(redo) {
        got[i] = g;
    }
}

#[must_use]
pub struct Cutting {
    action: Option<Action>,
    places: Vec<String>,
    _own: Option<Opened>,
}

impl Drop for Cutting {
    fn drop(&mut self) {
        if let Some(a) = &self.action {
            let mut split = a.0.split.lock().unwrap_or_else(|e| e.into_inner());
            for p in &self.places {
                if let Some(n) = split.get_mut(p) {
                    *n -= 1;
                    if *n == 0 {
                        split.remove(p);
                    }
                }
            }
        }
    }
}

pub fn splits(place: &str) -> bool {
    current().is_some_and(|a| a.0.split.lock().unwrap_or_else(|e| e.into_inner()).contains_key(place))
}

pub fn heard(place: &str, said: &str) {
    if let Some(a) = current() {
        a.0.heard.lock().unwrap_or_else(|e| e.into_inner()).insert(place.to_string(), said.to_string());
    }
}

pub fn heard_from(place: &str) -> Option<String> {
    current().and_then(|a| a.0.heard.lock().unwrap_or_else(|e| e.into_inner()).get(place).cloned())
}

pub fn opened() -> Option<std::time::Instant> {
    NOW.with(|n| n.borrow().as_ref().map(|a| a.0.opened.0))
}

pub fn within<R>(action: Option<Action>, work: impl FnOnce() -> R) -> R {
    let before = NOW.with(|n| std::mem::replace(&mut *n.borrow_mut(), action));
    let _back = Opened { before };
    work()
}

pub fn keep_in(k: Arc<dyn Keep>) {
    if let Some(a) = current() {
        *a.0.keeper.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
    }
}

pub fn keeper() -> Option<Arc<dyn Keep>> {
    current().and_then(|a| a.0.keeper.lock().unwrap_or_else(|e| e.into_inner()).clone())
}

pub fn keep_judged_in(k: Arc<dyn Keep>) {
    if let Some(a) = current() {
        *a.0.judged_keeper.lock().unwrap_or_else(|e| e.into_inner()) = Some(k);
    }
}

pub fn judged_keeper() -> Option<Arc<dyn Keep>> {
    keeper().or_else(|| current().and_then(|a| a.0.judged_keeper.lock().unwrap_or_else(|e| e.into_inner()).clone()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stands {
    Not,
    Pinned,
    Moving,
}

fn numbered(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Str(x)) if x.len() > 2 && x.starts_with("0x") && x[2..].bytes().all(|b| b.is_ascii_hexdigit()))
}

fn stands(method: &str, params: &Value, answer: &W) -> Stands {
    if answer.is_null() {
        return Stands::Not;
    }
    let ps: &[Value] = match params {
        Value::Arr(xs) => xs,
        _ => &[],
    };
    let mined = |k: &str| answer.member(k).and_then(W::as_str).is_some();
    let pinned = |yes: bool| if yes { Stands::Pinned } else { Stands::Not };
    match method {
        "eth_chainId" => Stands::Pinned,
        "eth_blockNumber" => Stands::Moving,
        "eth_getLogs" => pinned(answer.as_arr().is_some() && ps.first().is_some_and(|f| numbered(f.member("fromBlock")) && numbered(f.member("toBlock")))),
        "eth_getBlockByNumber" => pinned(numbered(ps.first())),
        "eth_getBlockByHash" => Stands::Pinned,
        "eth_getCode" | "eth_getBalance" | "eth_call" | "eth_getStorageAt" | "eth_getProof" => pinned(numbered(ps.last())),
        "eth_feeHistory" => pinned(numbered(ps.get(1))),
        "eth_estimateGas" => pinned(numbered(ps.get(1))),
        "eth_getTransactionByHash" => pinned(mined("blockHash")),
        "eth_getRawTransactionByHash" => pinned(raw_is(ps.first(), answer)),
        "eth_getTransactionReceipt" => pinned(mined("blockHash") && mined("blockNumber") && mined("status")),
        _ => Stands::Not,
    }
}

fn raw_is(asked: Option<&Value>, answer: &W) -> bool {
    let hash = asked.and_then(Value::as_str).and_then(zikaron::hexfmt::decode);
    let raw = answer.as_str().and_then(zikaron::hexfmt::decode);
    match (hash, raw) {
        (Some(h), Some(r)) => h.len() == 32 && !r.is_empty() && zikaron::cryptox::keccak256(&r)[..] == h[..],
        _ => false,
    }
}

fn question(at: &str, method: &str, params: &Value) -> Question {
    (at.to_string(), method.to_string(), zikaron::json::canon_bytes(params))
}

pub fn answered(at: &str, method: &str, params: &Value) -> Option<W> {
    let a = current()?;
    let held = a.0.answers.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((w, _)) = held.get(&question(at, method, params)) {
        return Some(w.clone());
    }
    if method != "eth_getLogs" {
        return None;
    }
    let (from, to) = logs_range(params)?;
    let rest = filter_rest(params)?;
    held.iter()
        .filter(|((node, m, _), _)| node == at && m == method)
        .find_map(|((_, _, wider), (w, _))| {
            let p = zikaron::json::parse(wider).ok()?;
            let (f, t) = logs_range(&p)?;
            (f <= from && to <= t && filter_rest(&p).as_ref() == Some(&rest)).then(|| logs_within(w, from, to))?
        })
}

fn logs_range(params: &Value) -> Option<(u64, u64)> {
    let f = params.as_arr()?.first()?;
    let n = |k: &str| numbered(f.member(k)).then(|| f.member(k).and_then(Value::as_str).and_then(|x| u64::from_str_radix(&x[2..], 16).ok()))?;
    Some((n("fromBlock")?, n("toBlock")?))
}

fn filter_rest(params: &Value) -> Option<Vec<u8>> {
    let xs = params.as_arr()?;
    if xs.len() != 1 {
        return None;
    }
    match &xs[0] {
        Value::Obj(ms) if ms.iter().all(|(k, _)| k != "blockHash") => Some(zikaron::json::canon_bytes(&Value::Obj(ms.iter().filter(|(k, _)| k != "fromBlock" && k != "toBlock").cloned().collect()))),
        _ => None,
    }
}

fn logs_within(answer: &W, from: u64, to: u64) -> Option<W> {
    let logs = answer.as_arr()?;
    let mut kept = Vec::new();
    for l in logs {
        let b = crate::settled::block_of(l)?;
        if (from..=to).contains(&b) {
            kept.push(crate::wire::write(l));
        }
    }
    crate::wire::parse(format!("[{}]", kept.join(",")).as_bytes())
}

pub fn answer_for(at: &str, method: &str, params: &Value) -> Option<W> {
    if let Some(w) = answered(at, method, params) {
        return Some(w);
    }
    let a = current()?;
    let w = a.0.once.lock().unwrap_or_else(|e| e.into_inner()).remove(&question(at, method, params));
    w
}

pub fn held_ahead(at: &str, method: &str, params: &Value) -> bool {
    answered(at, method, params).is_some() || current().is_some_and(|a| a.0.once.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&question(at, method, params)))
}

pub fn noted_once(at: &str, method: &str, params: &Value, answer: &W) {
    if late() {
        return;
    }
    note(at, method, params, answer);
    if stands(method, params, answer) == Stands::Not {
        if let Some(a) = current() {
            a.0.once.lock().unwrap_or_else(|e| e.into_inner()).insert(question(at, method, params), answer.clone());
        }
    }
}

pub fn note(at: &str, method: &str, params: &Value, answer: &W) {
    if late() {
        return;
    }
    let Some(a) = current() else { return };
    if method == "eth_getTransactionReceipt" {
        if let Some(block) = crate::tx::qty_u64(answer, "blockNumber") {
            landed(at, block);
        }
    }
    let moving = match stands(method, params, answer) {
        Stands::Not => return,
        Stands::Pinned => false,
        Stands::Moving => true,
    };
    a.0.answers.lock().unwrap_or_else(|e| e.into_inner()).insert(question(at, method, params), (answer.clone(), moving));
}

pub fn sent(at: &str, tx: Option<&[u8; 32]>) {
    sent_on(tx, head_of(at));
}

pub fn head_of(at: &str) -> Option<u64> {
    let a = current()?;
    let held = a.0.answers.lock().unwrap_or_else(|e| e.into_inner());
    held.iter().filter(|((node, m, _), (_, moving))| *moving && node == at && m == "eth_blockNumber").filter_map(|(_, (w, _))| height(w)).max()
}

pub fn sent_on(tx: Option<&[u8; 32]>, head: Option<u64>) {
    if let (Some(a), Some(tx), Some(head)) = (current(), tx, head) {
        let mut on = a.0.broadcast_heads.lock().unwrap_or_else(|e| e.into_inner());
        let was = on.entry(*tx).or_insert(head);
        *was = (*was).max(head);
    }
    new_head();
}

pub fn head_at_broadcast(tx: &[u8; 32]) -> Option<u64> {
    current().and_then(|a| a.0.broadcast_heads.lock().unwrap_or_else(|e| e.into_inner()).get(tx).copied())
}

pub(crate) fn height(w: &W) -> Option<u64> {
    w.as_str().and_then(crate::send::quantity)
}

pub fn new_head() {
    if let Some(a) = current() {
        a.0.answers.lock().unwrap_or_else(|e| e.into_inner()).retain(|_, (_, moving)| !*moving);
        a.0.once.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

pub fn after_a_beat<R>(pause: std::time::Duration, ask: impl FnOnce() -> R) -> R {
    if !pause.is_zero() {
        std::thread::sleep(pause);
    }
    new_head();
    ask()
}

pub fn landed(at: &str, block: u64) {
    if late() {
        return;
    }
    let Some(a) = current() else { return };
    {
        let mut f = a.0.floors.lock().unwrap_or_else(|e| e.into_inner());
        let was = f.entry(at.to_string()).or_insert(block);
        *was = (*was).max(block);
    }
    a.0.answers.lock().unwrap_or_else(|e| e.into_inner()).retain(|(_, m, _), (w, moving)| !*moving || m != "eth_blockNumber" || height(w).is_some_and(|h| h >= block));
}

pub fn floor_among<'a>(nodes: impl IntoIterator<Item = &'a str>) -> Option<u64> {
    let a = current()?;
    let f = a.0.floors.lock().unwrap_or_else(|e| e.into_inner());
    nodes.into_iter().filter_map(|n| f.get(n).copied()).max()
}

pub(crate) fn notes(load: impl FnOnce() -> crate::settled::Notes) -> crate::settled::Notes {
    let Some(l) = current().and_then(|a| a.lived()) else { return load() };
    let mut r = l.record.lock().unwrap_or_else(|e| e.into_inner());
    r.notes.get_or_insert_with(load).clone()
}

pub(crate) fn notes_now(n: &crate::settled::Notes) {
    if let Some(l) = current().and_then(|a| a.lived()) {
        l.record.lock().unwrap_or_else(|e| e.into_inner()).notes = Some(n.clone());
    }
}

pub(crate) fn entry(key: &crate::settled::Key, load: impl FnOnce() -> Option<crate::settled::Entry>) -> Option<crate::settled::Entry> {
    let Some(l) = current().and_then(|a| a.lived()) else { return load() };
    let mut r = l.record.lock().unwrap_or_else(|e| e.into_inner());
    r.entries.entry(key.clone()).or_insert_with(load).clone()
}

pub(crate) fn pend(p: crate::settled::Pending) -> Option<crate::settled::Pending> {
    if late() {
        return None;
    }
    let Some(a) = current() else { return Some(p) };
    let Some(l) = a.lived() else { return None };
    let mut r = l.record.lock().unwrap_or_else(|e| e.into_inner());
    let load = || p.keeper.load(&crate::settled::part_of(&p.key)).and_then(|b| crate::settled::parse_entry(&b)).filter(|(k, _)| *k == p.key).map(|(_, e)| e);
    let seen = r.entries.entry(p.key.clone()).or_insert_with(load);
    if let Some(e) = crate::settled::carried_on(seen.as_ref(), &p.entry, p.base) {
        *seen = Some(e);
    }
    r.pending.push(p);
    None
}

pub fn facts(kind: &'static str, load: impl FnOnce() -> crate::scan::Known) -> crate::scan::Known {
    let Some(a) = current() else { return load() };
    let mut held = a.0.facts.lock().unwrap_or_else(|e| e.into_inner());
    held.entry(kind).or_insert_with(load).clone()
}

pub fn facts_now() {
    if let Some(a) = current() {
        a.0.facts.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

pub fn proven<E>(chain: u64, hash: &[u8; 32], ask: impl FnOnce() -> Result<Option<crate::tx::Tx>, E>) -> Result<Option<crate::tx::Tx>, E> {
    let Some(a) = current() else { return ask() };
    let key = (chain, *hash);
    {
        let mut held = a.0.proven.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match held.get(&key).cloned() {
                Some(Slot::Proved(t)) => return Ok(Some(t)),
                Some(Slot::Each) => {
                    drop(held);
                    return ask();
                }
                Some(Slot::Asking) => held = a.0.read_out.wait(held).unwrap_or_else(|e| e.into_inner()),
                None => {
                    held.insert(key, Slot::Asking);
                    break;
                }
            }
        }
    }
    struct Settle<'a>(&'a Inner, (u64, [u8; 32]), Slot);
    impl Drop for Settle<'_> {
        fn drop(&mut self) {
            let mut held = self.0.proven.lock().unwrap_or_else(|e| e.into_inner());
            if !matches!(held.get(&self.1), Some(Slot::Proved(_))) || matches!(self.2, Slot::Proved(_)) {
                held.insert(self.1, self.2.clone());
            }
            drop(held);
            self.0.read_out.notify_all();
        }
    }
    let mut settle = Settle(&a.0, key, Slot::Each);
    let got = ask();
    if let Ok(Some(t)) = &got {
        if t.hash == *hash && !t.raw.is_empty() && !late() {
            settle.2 = Slot::Proved(t.clone());
        }
    }
    drop(settle);
    got
}

pub(crate) fn judged_held(key: &crate::settled::Key) -> Option<crate::settled::Entry> {
    current().and_then(|a| a.0.judged.lock().unwrap_or_else(|e| e.into_inner()).get(key).cloned())
}

pub(crate) fn judged_keep(key: &crate::settled::Key, e: crate::settled::Entry) {
    if late() {
        return;
    }
    let Some(a) = current() else { return };
    let mut held = a.0.judged.lock().unwrap_or_else(|e| e.into_inner());
    match held.get(key) {
        Some(h) if h.from <= e.from && h.settled >= e.settled => {}
        _ => {
            held.insert(key.clone(), e);
        }
    }
}

pub(crate) fn ahead<R>(f: impl FnOnce(&mut crate::ahead::Held) -> R) -> Option<R> {
    let a = current()?;
    let mut held = a.0.ahead.lock().unwrap_or_else(|e| e.into_inner());
    Some(f(&mut held))
}

pub(crate) fn ahead_moved<R>(f: impl FnOnce(&mut crate::ahead::Held) -> R) -> Option<R> {
    let a = current()?;
    let out = f(&mut a.0.ahead.lock().unwrap_or_else(|e| e.into_inner()));
    a.0.ahead_moved.notify_all();
    Some(out)
}

pub(crate) fn ahead_wait(ready: impl Fn(&crate::ahead::Held) -> bool) {
    let Some(a) = current() else { return };
    let mut held = a.0.ahead.lock().unwrap_or_else(|e| e.into_inner());
    while !ready(&held) {
        held = a.0.ahead_moved.wait(held).unwrap_or_else(|e| e.into_inner());
    }
}

pub(crate) fn proven_given(chain: u64, t: &crate::tx::Tx) {
    let Some(a) = current() else { return };
    if t.raw.is_empty() || late() {
        return;
    }
    let mut held = a.0.proven.lock().unwrap_or_else(|e| e.into_inner());
    let slot = held.entry((chain, t.hash)).or_insert(Slot::Asking);
    if matches!(slot, Slot::Asking) {
        *slot = Slot::Proved(t.clone());
    }
    drop(held);
    a.0.read_out.notify_all();
}

pub(crate) fn claim_ahead(chain: u64, hashes: &[[u8; 32]]) -> Claimed {
    let Some(a) = current() else { return Claimed { action: None, chain, hashes: Vec::new() } };
    let mut held = a.0.proven.lock().unwrap_or_else(|e| e.into_inner());
    let mut taken: Vec<[u8; 32]> = Vec::new();
    for h in hashes {
        if !held.contains_key(&(chain, *h)) && !taken.contains(h) {
            held.insert((chain, *h), Slot::Asking);
            taken.push(*h);
        }
    }
    drop(held);
    Claimed { action: Some(a), chain, hashes: taken }
}

pub(crate) struct Claimed {
    action: Option<Action>,
    chain: u64,
    hashes: Vec<[u8; 32]>,
}

impl Claimed {
    pub(crate) fn hashes(&self) -> &[[u8; 32]] {
        &self.hashes
    }

    pub(crate) fn proved(&mut self, t: &crate::tx::Tx) {
        let Some(a) = &self.action else { return };
        if t.raw.is_empty() || !self.hashes.contains(&t.hash) || late() {
            return;
        }
        self.hashes.retain(|h| *h != t.hash);
        a.0.proven.lock().unwrap_or_else(|e| e.into_inner()).insert((self.chain, t.hash), Slot::Proved(t.clone()));
        a.0.read_out.notify_all();
    }

    pub(crate) fn not_itself(&mut self, hash: &[u8; 32]) {
        let Some(a) = &self.action else { return };
        if !self.hashes.contains(hash) {
            return;
        }
        self.hashes.retain(|h| h != hash);
        a.0.proven.lock().unwrap_or_else(|e| e.into_inner()).insert((self.chain, *hash), Slot::Each);
        a.0.read_out.notify_all();
    }
}

impl Drop for Claimed {
    fn drop(&mut self) {
        let Some(a) = &self.action else { return };
        let mut held = a.0.proven.lock().unwrap_or_else(|e| e.into_inner());
        for h in &self.hashes {
            if matches!(held.get(&(self.chain, *h)), Some(Slot::Asking)) {
                held.remove(&(self.chain, *h));
            }
        }
        drop(held);
        a.0.read_out.notify_all();
    }
}

fn late() -> bool {
    crate::endpoints::stopped()
}

pub(crate) fn ahead_turn<R>(work: impl FnOnce() -> R) -> Option<R> {
    let a = current()?;
    let _turn = a.0.ahead_turn.lock().unwrap_or_else(|e| e.into_inner());
    Some(work())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_is_cut_where_nobody_answered_and_every_node_that_refused_is_cut() {
        #[derive(Clone, Copy, PartialEq)]
        enum A {
            Whole,
            Refused,
            Down,
        }
        let again = |got: &[A]| cut_again(got, |a| *a == A::Whole, |a| *a == A::Refused);
        assert_eq!(again(&[A::Down, A::Refused]), [1], "the node that answered with a refusal, beside one that is down");
        assert_eq!(again(&[A::Refused]), [0]);
        assert!(again(&[A::Whole, A::Refused]).is_empty(), "one answered the whole: nobody cut");
        assert_eq!(again(&[A::Refused, A::Down, A::Refused]), [0, 2], "every node that refused, whatever it said");
        assert!(again(&[A::Down, A::Down]).is_empty(), "nothing refused: nothing cut");
    }

    #[test]
    fn a_read_asked_again_gives_leave_to_cut_to_every_node_that_refused() {
        let _a = open(None);
        let mut got: Vec<Result<u8, bool>> = vec![Err(true), Err(true), Err(false)];
        let mut leave = Vec::new();
        let mut one_read = false;
        cut_and_ask(&mut got, |i, _| vec![format!("node-{i}")], |g| g.is_ok(), |g| *g == Err(true), |idx, _| {
            leave = ["node-0", "node-1", "node-2"].iter().map(|p| splits(p)).collect();
            one_read = read_while_cut(["node-1"]) && !read_while_cut(["node-2"]);
            idx.iter().map(|i| Ok(*i as u8)).collect()
        });
        assert_eq!(got, [Ok(0), Ok(1), Err(false)]);
        assert_eq!(leave, [true, true, false]);
        assert!(one_read, "while it is cut, one node that read the whole is enough");
        assert!(!splits("node-0"));
        let mut whole = vec![Ok(7u8), Err(true)];
        cut_and_ask(&mut whole, |i, _| vec![format!("whole-{i}")], |g| g.is_ok(), |g| *g == Err(true), |_, _| panic!("one answered the whole"));
        assert_eq!(whole, [Ok(7), Err(true)]);
    }

    #[test]
    fn the_cap_on_questions_is_one_to_a_thousand_and_an_action_may_hold_its_own() {
        assert!(!asks_within(0) && asks_within(1) && asks_within(ASKS_DEFAULT) && asks_within(1000) && !asks_within(1001));
        let _a = open(None);
        ask_at_most(3);
        assert_eq!(asks_cap(), 3);
        ask_at_most(5_000);
        assert_eq!(asks_cap(), ASKS_MAX);
    }

    #[test]
    fn every_read_of_logs_by_several_nodes_goes_through_the_one_entry() {
        const READERS: [&str; 9] = ["paged", "logs_judged_over", "held_among", "latest_among", "accessions_under", "read_notices", "read_one", "run_picked", "run_knowing_picked"];
        const ASKS_AT_ONCE: [&str; 2] = ["until_enough(", "each_node("];
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        let mut stack: Vec<std::path::PathBuf> = ["zikaron-anchor", "zikaron-send", "app"].iter().map(|c| crates.join(c).join("src")).collect();
        while let Some(at) = stack.pop() {
            for e in std::fs::read_dir(&at).expect("a source folder").filter_map(Result::ok) {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    files.push(p);
                }
            }
        }
        let code = |text: &str| -> String { text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n") };
        let mut defined = std::collections::BTreeSet::new();
        let mut red = Vec::new();
        for f in &files {
            let text = code(&std::fs::read_to_string(f).expect("a source file"));
            let here = f.ends_with("zikaron-anchor/src/action.rs");
            for word in ["cut_again(", "split_widths("] {
                if !here && text.contains(word) {
                    red.push(format!("{}: {word}", f.display()));
                }
            }
            let mut bodies: Vec<(String, String)> = Vec::new();
            for line in text.lines() {
                let t = line.trim_start();
                let head = ["pub(crate) fn ", "pub(super) fn ", "pub fn ", "fn "].iter().find_map(|p| t.strip_prefix(p));
                match head {
                    Some(rest) => {
                        let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                        defined.insert(name.clone());
                        bodies.push((name, String::new()));
                    }
                    None => {
                        if let Some((_, b)) = bodies.last_mut() {
                            b.push_str(line);
                            b.push('\n');
                        }
                    }
                }
            }
            for (name, body) in bodies {
                let at_once = ASKS_AT_ONCE.iter().any(|w| body.contains(w));
                let reads = READERS.iter().any(|r| body.contains(&format!("{r}(")));
                if at_once && reads && !body.contains("cut_and_ask(") {
                    red.push(format!("{}: {name}", f.display()));
                }
            }
        }
        for r in READERS {
            assert!(defined.contains(r), "a reader the pin names is no function: {r}");
        }
        assert!(red.is_empty(), "reads of logs by several nodes not through `cut_and_ask`: {red:?}");
    }

    #[test]
    fn only_a_transaction_rebuilt_to_the_asked_hash_is_every_rounds() {
        let h = [1u8; 32];
        let tx = |hash: [u8; 32], raw: Vec<u8>| crate::tx::Tx { hash, chain_id: None, sender: None, pubkey: None, to: None, input: Vec::new(), block_number: Some(1), tx_type: 2, raw };
        for (form, first) in [("another hash", tx([2u8; 32], vec![1])), ("no signature", tx(h, Vec::new()))] {
            let _a = open(None);
            let _ = proven::<()>(1, &h, || Ok(Some(first.clone())));
            let mut asked = false;
            let _ = proven::<()>(1, &h, || {
                asked = true;
                Ok(None)
            });
            assert!(asked, "{form}: the next round asks its own node");
        }
        let _a = open(None);
        let _ = proven::<()>(1, &h, || Ok(Some(tx(h, vec![1]))));
        let got = proven::<()>(1, &h, || panic!("a proved transaction is not asked again"));
        assert!(matches!(got, Ok(Some(_))), "a proved transaction is every round's");
    }

    struct Nowhere;

    impl Keep for Nowhere {
        fn load(&self, _part: &str) -> Option<Vec<u8>> {
            None
        }
        fn save(&self, _part: &str, _plain: &[u8]) -> Result<(), crate::settled::Unkept> { Ok(()) }
    }

    #[test]
    fn a_keeper_lives_in_the_action_it_was_opened_with_and_nowhere_else() {
        assert!(keeper().is_none(), "no action: no keeper");
        keep_in(Arc::new(Nowhere));
        assert!(keeper().is_none(), "no action to keep it in: none for the process");
        let outer = open(Some(Arc::new(Nowhere)));
        assert!(keeper().is_some());
        {
            let _inner = open(None);
            assert!(keeper().is_none(), "the inner action keeps nothing");
            let carried = current();
            let seen = std::thread::spawn(move || within(carried, || keeper().is_some())).join().expect("the thread");
            assert!(!seen, "a thread of the inner action keeps what it keeps");
        }
        assert!(keeper().is_some(), "the outer action back");
        let carried = current();
        assert!(std::thread::spawn(move || within(carried, || keeper().is_some())).join().expect("the thread"), "a thread of the action keeps its keeper");
        drop(outer);
        assert!(keeper().is_none(), "closed");
    }

    #[test]
    fn a_pinned_answer_is_the_actions_and_a_moving_one_is_asked_again() {
        let w = |t: &str| crate::wire::parse(t.as_bytes()).expect("an answer");
        let none = Value::Arr(Vec::new());
        let at_block = Value::Arr(vec![Value::Str("0x11".into()), Value::Str("0x5".into())]);
        let at_latest = Value::Arr(vec![Value::Str("0x11".into()), Value::Str("latest".into())]);
        note("n", "eth_chainId", &none, &w("\"0x1\""));
        assert!(answered("n", "eth_chainId", &none).is_none(), "no action: nothing held");
        let action = open(None);
        note("n", "eth_chainId", &none, &w("\"0x1\""));
        note("n", "eth_blockNumber", &none, &w("\"0x9\""));
        note("n", "eth_getCode", &at_block, &w("\"0x\""));
        note("n", "eth_getCode", &at_latest, &w("\"0x\""));
        note("n", "eth_getTransactionByHash", &none, &w("{\"blockHash\":null,\"hash\":\"0x1\"}"));
        note("n", "eth_getTransactionCount", &at_block, &w("\"0x1\""));
        assert_eq!(answered("n", "eth_chainId", &none), Some(w("\"0x1\"")));
        assert!(answered("m", "eth_chainId", &none).is_none(), "another node is asked itself");
        assert!(answered("n", "eth_getCode", &at_block).is_some(), "a numbered block pins the code");
        assert!(answered("n", "eth_getCode", &at_latest).is_none(), "latest moves");
        assert!(answered("n", "eth_getTransactionByHash", &none).is_none(), "a transaction not yet mined moves");
        assert!(answered("n", "eth_getTransactionCount", &at_block).is_none(), "a nonce is asked every time");
        assert!(answered("n", "eth_blockNumber", &none).is_some(), "the head is the action's");
        sent("n", Some(&[1; 32]));
        assert_eq!(head_at_broadcast(&[1; 32]), Some(9), "the head the node gave before the broadcast");
        assert!(answered("n", "eth_blockNumber", &none).is_none(), "a broadcast lets the head go");
        assert!(answered("n", "eth_getCode", &at_block).is_some(), "what a block pins stays");
        let carried = current();
        assert!(std::thread::spawn(move || within(carried, || answered("n", "eth_chainId", &Value::Arr(Vec::new())).is_some())).join().expect("the thread"), "every thread of the action reads its answers");
        drop(action);
        assert!(answered("n", "eth_chainId", &none).is_none(), "the next action asks again");
    }

    #[test]
    fn a_log_question_within_a_held_answer_is_answered_from_it() {
        let w = |t: &str| crate::wire::parse(t.as_bytes()).expect("an answer");
        let q = |from: &str, to: &str, address: &str| {
            Value::Arr(vec![Value::Obj(vec![
                ("address".into(), Value::Arr(vec![Value::Str(address.into())])),
                ("fromBlock".into(), Value::Str(from.into())),
                ("toBlock".into(), Value::Str(to.into())),
                ("topics".into(), Value::Arr(Vec::new())),
            ])])
        };
        let _action = open(None);
        note("n", "eth_getLogs", &q("0x1", "0x41", "0x11"), &w(r#"[{"blockNumber":"0x40","logIndex":"0x0"},{"blockNumber":"0x41","logIndex":"0x0"}]"#));
        assert_eq!(answered("n", "eth_getLogs", &q("0x1", "0x40", "0x11")), Some(w(r#"[{"blockNumber":"0x40","logIndex":"0x0"}]"#)), "up to an earlier block: its logs there");
        assert_eq!(answered("n", "eth_getLogs", &q("0x1", "0x41", "0x11")), Some(w(r#"[{"blockNumber":"0x40","logIndex":"0x0"},{"blockNumber":"0x41","logIndex":"0x0"}]"#)), "the same question");
        assert!(answered("n", "eth_getLogs", &q("0x1", "0x42", "0x11")).is_none(), "past the held answer");
        assert_eq!(answered("n", "eth_getLogs", &q("0x41", "0x41", "0x11")), Some(w(r#"[{"blockNumber":"0x41","logIndex":"0x0"}]"#)), "from a later block: its logs there");
        assert_eq!(answered("n", "eth_getLogs", &q("0x2", "0x3f", "0x11")), Some(w("[]")), "a range inside with no logs: none");
        assert!(answered("n", "eth_getLogs", &q("0x0", "0x40", "0x11")).is_none(), "from an earlier block");
        assert!(answered("n", "eth_getLogs", &q("0x1", "0x40", "0x12")).is_none(), "another filter");
        assert!(answered("m", "eth_getLogs", &q("0x1", "0x40", "0x11")).is_none(), "another node");
        let topics = |t: Value| Value::Arr(vec![Value::Obj(vec![("address".into(), Value::Arr(vec![Value::Str("0x11".into())])), ("fromBlock".into(), Value::Str("0x1".into())), ("toBlock".into(), Value::Str("0x40".into())), ("topics".into(), t)])]);
        assert!(answered("n", "eth_getLogs", &topics(Value::Arr(vec![Value::Str("0xaa".into())]))).is_none(), "a narrower topic: asked itself");
        let hash = Value::Arr(vec![Value::Obj(vec![("address".into(), Value::Arr(vec![Value::Str("0x11".into())])), ("blockHash".into(), Value::Str("0x99".into())), ("topics".into(), Value::Arr(Vec::new()))])]);
        assert!(answered("n", "eth_getLogs", &hash).is_none(), "a block by hash: asked itself");
        note("n", "eth_getLogs", &q("0x1", "0x50", "0x13"), &w(r#"[{"logIndex":"0x0"}]"#));
        assert!(answered("n", "eth_getLogs", &q("0x1", "0x40", "0x13")).is_none(), "a log that does not say its block: asked again");
    }

    #[test]
    fn a_raw_transaction_is_held_once_it_hashes_to_the_hash_asked() {
        let raw = vec![0x02u8, 0xf8, 0x01, 0x02];
        let hash = zikaron::cryptox::keccak256(&raw);
        let ask = |h: &[u8]| Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(h))]);
        let w = |t: String| crate::wire::parse(t.as_bytes()).expect("an answer");
        let _action = open(None);
        note("n", "eth_getRawTransactionByHash", &ask(&hash), &w(format!("\"{}\"", zikaron::hexfmt::encode(&raw))));
        assert!(answered("n", "eth_getRawTransactionByHash", &ask(&hash)).is_some(), "bytes of the hash asked: held");
        let other = [7u8; 32];
        note("n", "eth_getRawTransactionByHash", &ask(&other), &w(format!("\"{}\"", zikaron::hexfmt::encode(&raw))));
        assert!(answered("n", "eth_getRawTransactionByHash", &ask(&other)).is_none(), "bytes of another hash: asked again");
        note("n", "eth_getRawTransactionByHash", &ask(&[8u8; 32]), &w("\"0x\"".into()));
        assert!(answered("n", "eth_getRawTransactionByHash", &ask(&[8u8; 32])).is_none(), "empty bytes: asked again");
    }

    #[test]
    fn a_receipt_lets_go_only_the_heads_below_its_block() {
        let w = |t: &str| crate::wire::parse(t.as_bytes()).expect("an answer");
        let none = Value::Arr(Vec::new());
        let _action = open(None);
        note("ahead", "eth_blockNumber", &none, &w("\"0x20\""));
        note("behind", "eth_blockNumber", &none, &w("\"0x10\""));
        landed("ahead", 0x18);
        assert_eq!(floor_among(["ahead", "behind"]), Some(0x18));
        assert!(answered("ahead", "eth_blockNumber", &none).is_some(), "a head past the receipt's block stays");
        assert!(answered("behind", "eth_blockNumber", &none).is_none(), "a head below it is asked anew");
        landed("ahead", 0x20);
        assert!(answered("ahead", "eth_blockNumber", &none).is_some(), "a head at the receipt's block stays");
        landed("behind", 0x21);
        assert!(answered("ahead", "eth_blockNumber", &none).is_none(), "a receipt past every head lets it go");
        assert_eq!(floor_among(["ahead", "behind"]), Some(0x21), "the highest receipt's block of the chain's nodes");
    }

    #[test]
    fn a_chains_floor_is_its_own_nodes_receipts() {
        assert_eq!(floor_among(["high"]), None, "no action: no floor");
        let _action = open(None);
        landed("high", 0x1000);
        assert_eq!(floor_among(["low-a", "low-b"]), None, "another chain's receipt: no floor here");
        landed("low-b", 0x12);
        assert_eq!(floor_among(["low-a", "low-b"]), Some(0x12), "a receipt of one node of the chain holds every node of it");
        assert_eq!(floor_among(["high"]), Some(0x1000));
    }

    #[test]
    fn the_head_a_broadcast_went_out_on_is_its_transactions() {
        let w = |t: &str| crate::wire::parse(t.as_bytes()).expect("an answer");
        let none = Value::Arr(Vec::new());
        let _action = open(None);
        note("high", "eth_blockNumber", &none, &w("\"0x1000\""));
        note("low", "eth_blockNumber", &none, &w("\"0x10\""));
        sent("low", Some(&[1; 32]));
        assert_eq!(head_at_broadcast(&[1; 32]), Some(0x10), "the lower chain's own head, never the other chain's");
        note("high", "eth_blockNumber", &none, &w("\"0x1001\""));
        sent("high", Some(&[2; 32]));
        assert_eq!((head_at_broadcast(&[1; 32]), head_at_broadcast(&[2; 32])), (Some(0x10), Some(0x1001)), "each transaction its own");
        note("a", "eth_blockNumber", &none, &w("\"0x20\""));
        note("b", "eth_blockNumber", &none, &w("\"0x21\""));
        let (a, b) = (head_of("a"), head_of("b"));
        sent_on(Some(&[3; 32]), a);
        sent("b", Some(&[3; 32]));
        assert_eq!(head_of("b"), None, "the first broadcast let the heads go");
        sent_on(Some(&[3; 32]), b);
        sent("c", Some(&[3; 32]));
        assert_eq!(head_at_broadcast(&[3; 32]), Some(0x21), "the highest of the nodes that took it; a node holding none changes nothing");
        sent("high", None);
        assert_eq!(head_at_broadcast(&[4; 32]), None, "no broadcast of it: no base");
    }
}
