use zikaron::json::{canon_bytes, Value};

#[derive(Clone, Debug)]
pub struct Reading {
    pub fragment: Value,
    pub sources: Vec<String>,
    pub single_source: bool,
    pub single_source_chains: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct Disagreement {
    pub sources: Vec<String>,
    pub fragments: Vec<Vec<u8>>,
}

pub fn agree_over(runs: Vec<(String, Value)>, thin: Vec<u64>) -> Result<Reading, Disagreement> {
    crate::seam();
    let places: Vec<Vec<String>> = runs.iter().map(|(n, _)| vec![n.clone()]).collect();
    agree_among(runs, &places, thin)
}

pub fn agree_among(runs: Vec<(String, Value)>, places: &[Vec<String>], thin: Vec<u64>) -> Result<Reading, Disagreement> {
    let sources: Vec<String> = runs.iter().map(|(n, _)| n.clone()).collect();
    let bytes: Vec<Vec<u8>> = runs.iter().map(|(_, v)| canon_bytes(v)).collect();
    let all_same = bytes.windows(2).all(|w| w[0] == w[1]);
    let kept: Vec<usize> = if all_same {
        (0..runs.len()).collect()
    } else {
        match two_alike(&bytes, places) {
            Some(k) => k,
            None => return Err(Disagreement { sources, fragments: bytes }),
        }
    };
    let fragment = kept.first().and_then(|i| runs.get(*i)).map(|(_, v)| v.clone()).unwrap_or(Value::Null);
    let sources: Vec<String> = kept.iter().filter_map(|i| sources.get(*i).cloned()).collect();
    let mut thin = thin;
    thin.sort_unstable();
    thin.dedup();
    Ok(Reading { fragment, sources, single_source: !thin.is_empty(), single_source_chains: thin })
}

pub fn two_alike(bytes: &[Vec<u8>], places: &[Vec<String>]) -> Option<Vec<usize>> {
    let mut groups: Vec<(&[u8], Vec<usize>)> = Vec::new();
    for (i, b) in bytes.iter().enumerate() {
        match groups.iter_mut().find(|(g, _)| *g == b.as_slice()) {
            Some((_, at)) => at.push(i),
            None => groups.push((b.as_slice(), vec![i])),
        }
    }
    let two: Vec<Vec<usize>> = groups
        .into_iter()
        .map(|(_, at)| at)
        .filter(|at| distinct_places(at.iter().flat_map(|i| places.get(*i).into_iter().flatten()).map(String::as_str)) >= 2)
        .collect();
    match two.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

pub fn left_out(names: &[String], reading: &Reading) -> Vec<String> {
    names.iter().filter(|n| !reading.sources.contains(n)).cloned().collect()
}

pub fn project(answer: &Value, facts: &[&str]) -> Value {
    match answer {
        Value::Obj(m) => Value::Obj(
            facts
                .iter()
                .filter_map(|f| m.iter().find(|(k, _)| k == f).map(|(k, v)| (k.clone(), v.clone())))
                .collect(),
        ),
        other => other.clone(),
    }
}

pub fn only_behind(row: &[Value], more: &[Value], block_of: impl Fn(&Value) -> Option<u64>) -> bool {
    if row.len() >= more.len() {
        return false;
    }
    let theirs: Vec<Vec<u8>> = more.iter().map(canon_bytes).collect();
    let mine: Vec<Vec<u8>> = row.iter().map(canon_bytes).collect();
    if !mine.iter().all(|m| theirs.contains(m)) {
        return false;
    }
    let last = row.iter().filter_map(&block_of).max();
    more.iter().zip(&theirs).filter(|(_, b)| !mine.contains(b)).all(|(o, _)| match (block_of(o), last) {
        (Some(b), Some(l)) => b > l,
        (Some(_), None) => true,
        (None, _) => false,
    })
}

pub fn agree(runs: Vec<(String, Value)>) -> Result<Reading, Disagreement> {
    let thin = if distinct_places(runs.iter().map(|(n, _)| n.as_str())) < 2 { vec![0] } else { Vec::new() };
    agree_over(runs, thin)
}

pub fn place_of(url: &str) -> String {
    crate::rest::place(url).unwrap_or_else(|| zikaron_net::place_key(url))
}

pub fn places_of<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let names: Vec<&str> = names.into_iter().collect();
    let rest_of = |n: &str| crate::rest::place(n).or_else(|| n.trim().starts_with("rest://").then(|| n.trim().to_string()));
    let rest: Vec<String> = names.iter().filter_map(|n| rest_of(n)).collect();
    names.iter().map(|n| rest_of(n).or_else(|| crate::rest::place_beside(n, &rest)).unwrap_or_else(|| zikaron_net::place_key(n))).collect()
}

pub fn distinct_places<'a>(names: impl IntoIterator<Item = &'a str>) -> usize {
    let mut keys = places_of(names);
    keys.sort();
    keys.dedup();
    keys.len()
}

pub fn thin_chains(places: &[(u64, String)], chains: &[u64]) -> Vec<u64> {
    let mut thin: Vec<u64> = chains
        .iter()
        .copied()
        .filter(|c| distinct_places(places.iter().filter(|(x, _)| x == c).map(|(_, u)| u.as_str())) < 2)
        .collect();
    thin.sort_unstable();
    thin.dedup();
    thin
}

impl Reading {
    pub fn value(&self) -> Value {
        Value::Obj(vec![
            ("fragment".into(), self.fragment.clone()),
            ("singleSource".into(), Value::Bool(self.single_source)),
            (
                "singleSourceChains".into(),
                Value::Arr(self.single_source_chains.iter().map(|c| Value::Int(*c)).collect()),
            ),
            ("sources".into(), Value::Arr(self.sources.iter().map(|s| Value::Str(s.clone())).collect())),
        ])
    }
}

thread_local! {
    static STOPS: std::cell::RefCell<Vec<std::sync::Arc<std::sync::atomic::AtomicBool>>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn stopped() -> bool {
    STOPS.with(|s| s.borrow().iter().any(|m| m.load(std::sync::atomic::Ordering::SeqCst)))
}

fn under<R>(marks: Vec<std::sync::Arc<std::sync::atomic::AtomicBool>>, work: impl FnOnce() -> R) -> R {
    struct Back(Vec<std::sync::Arc<std::sync::atomic::AtomicBool>>);
    impl Drop for Back {
        fn drop(&mut self) {
            let before = std::mem::take(&mut self.0);
            STOPS.with(|s| *s.borrow_mut() = before);
        }
    }
    let before = STOPS.with(|s| std::mem::replace(&mut *s.borrow_mut(), marks));
    let _back = Back(before);
    work()
}

pub fn until_enough<T, R>(items: Vec<T>, work: impl Fn(T) -> R + Send + Sync + 'static, lost: impl Fn() -> R, mut enough: impl FnMut(&[Option<R>]) -> bool) -> Vec<Option<R>>
where
    T: Send + 'static,
    R: Send + 'static,
{
    use std::sync::{atomic::AtomicBool, mpsc, Arc, Mutex};
    let n = items.len();
    let mut slots: Vec<Option<R>> = (0..n).map(|_| None).collect();
    if n == 1 {
        slots[0] = items.into_iter().next().map(work);
        return slots;
    }
    if n == 0 {
        return slots;
    }
    let mark = Arc::new(AtomicBool::new(false));
    let mut marks = STOPS.with(|s| s.borrow().clone());
    marks.push(mark.clone());
    let work = Arc::new(work);
    let action = crate::action::current().map(|a| a.worker());
    let (tx, rx) = mpsc::channel::<(usize, Option<R>)>();
    let mut here: Vec<Arc<Mutex<Option<T>>>> = Vec::new();
    let mut here_at: Vec<usize> = Vec::new();
    let mut started = 0usize;
    for (i, t) in items.into_iter().enumerate() {
        let slot = Arc::new(Mutex::new(Some(t)));
        let (work, action, marks, tx, taken) = (work.clone(), action.clone(), marks.clone(), tx.clone(), slot.clone());
        let thread = std::thread::Builder::new().name(format!("zikaron-ask-{i}")).spawn(move || {
            let item = taken.lock().unwrap_or_else(|e| e.into_inner()).take();
            let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::action::within(action, || under(marks, || item.map(|t| work(t))))));
            let _ = tx.send((i, got.ok().flatten()));
        });
        match thread {
            Ok(_) => started += 1,
            Err(_) => {
                here.push(slot);
                here_at.push(i);
            }
        }
    }
    drop(tx);
    let stop = |slots: Vec<Option<R>>| {
        mark.store(true, std::sync::atomic::Ordering::SeqCst);
        slots
    };
    for (slot, i) in here.into_iter().zip(here_at) {
        let item = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| under(marks.clone(), || item.map(|t| work(t)))));
        slots[i] = Some(got.ok().flatten().unwrap_or_else(&lost));
        if enough(&slots) {
            return stop(slots);
        }
    }
    for _ in 0..started {
        let Ok((i, got)) = rx.recv() else { break };
        slots[i] = Some(got.unwrap_or_else(&lost));
        if enough(&slots) {
            return stop(slots);
        }
    }
    stop(slots)
}

#[allow(clippy::type_complexity)]
pub fn ask_each(nodes: Vec<(String, Box<dyn crate::rpc::Endpoint + Send>)>, method: &str, params: &Value, mut enough: impl FnMut(&[(String, Option<&Result<crate::wire::W, crate::rpc::Trouble>>)]) -> bool) -> Vec<(String, Option<Result<crate::wire::W, crate::rpc::Trouble>>)> {
    let names: Vec<String> = nodes.iter().map(|(n, _)| n.clone()).collect();
    let (m, p) = (method.to_string(), params.clone());
    let seen = names.clone();
    until_enough(
        nodes,
        move |(_, mut ep)| crate::patience::ask(ep.as_mut(), &m, &p),
        || Err(crate::rpc::Trouble::Transport(crate::rpc::Broke::Short)),
        |slots| {
            let view: Vec<(String, Option<&Result<crate::wire::W, crate::rpc::Trouble>>)> = seen.iter().cloned().zip(slots.iter().map(Option::as_ref)).collect();
            enough(&view)
        },
    )
    .into_iter()
    .zip(names)
    .map(|(got, name)| (name, got))
    .collect()
}

#[allow(clippy::type_complexity)]
pub fn all_in(got: Vec<(String, Option<Result<crate::wire::W, crate::rpc::Trouble>>)>) -> Vec<(String, Result<crate::wire::W, crate::rpc::Trouble>)> {
    got.into_iter().map(|(n, a)| (n, a.unwrap_or(Err(crate::rpc::Trouble::Transport(crate::rpc::Broke::Short))))).collect()
}

#[allow(clippy::type_complexity)]
pub fn every_answer(_: &[(String, Option<&Result<crate::wire::W, crate::rpc::Trouble>>)]) -> bool {
    false
}

pub fn every<S>(_: &[S]) -> bool {
    false
}

pub const REST_FIRST: std::time::Duration = std::time::Duration::from_secs(2);
pub const REST_MOST: std::time::Duration = std::time::Duration::from_secs(30);

struct Rest {
    until: std::time::Instant,
    length: std::time::Duration,
}

struct Limit {
    last: std::time::Instant,
    said: crate::rpc::Trouble,
    rest: Option<Rest>,
    run: Option<(usize, std::time::Instant)>,
}

static LIMITS: std::sync::Mutex<std::collections::BTreeMap<(String, String), Limit>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

thread_local! {
    static LIMIT_AHEAD: std::cell::Cell<std::time::Duration> = const { std::cell::Cell::new(std::time::Duration::ZERO) };
}

fn limits() -> std::sync::MutexGuard<'static, std::collections::BTreeMap<(String, String), Limit>> {
    LIMITS.lock().unwrap_or_else(|e| e.into_inner())
}

static LIMIT_AHEAD_ALL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn limit_clock() -> std::time::Instant {
    std::time::Instant::now() + LIMIT_AHEAD.with(std::cell::Cell::get) + std::time::Duration::from_millis(LIMIT_AHEAD_ALL.load(std::sync::atomic::Ordering::SeqCst))
}

pub fn move_limit_clock_all(d: std::time::Duration) {
    LIMIT_AHEAD_ALL.fetch_add(u64::try_from(d.as_millis()).unwrap_or(u64::MAX), std::sync::atomic::Ordering::SeqCst);
}

pub fn move_limit_clock(d: std::time::Duration) {
    LIMIT_AHEAD.with(|a| a.set(a.get() + d));
}

pub fn forget_limits() {
    limits().clear();
}

pub fn forget_limits_of(place: &str) {
    limits().retain(|(p, _), _| p != place);
}

fn key(place: &str, method: &str) -> (String, String) {
    (place.to_string(), method.to_string())
}

pub fn note_limited(place: &str, method: &str, said: &crate::rpc::Trouble) {
    let now = limit_clock();
    let mut l = limits();
    match l.get_mut(&key(place, method)) {
        Some(x) => {
            x.last = now;
            x.said = said.clone();
        }
        None => {
            l.insert(key(place, method), Limit { last: now, said: said.clone(), rest: None, run: None });
        }
    }
}

pub fn limited_since(place: &str, method: &str, since: std::time::Instant) -> bool {
    limits().get(&key(place, method)).is_some_and(|x| x.last >= since)
}

pub fn limited_in_action(place: &str, method: &str) -> bool {
    crate::action::opened().is_some_and(|t| limited_since(place, method, t))
}

pub fn resting(place: &str, method: &str) -> Option<crate::rpc::Trouble> {
    let now = limit_clock();
    limits().get(&key(place, method)).filter(|x| x.rest.as_ref().is_some_and(|r| now < r.until)).map(|x| x.said.clone())
}

pub fn rest(place: &str, method: &str, said: &crate::rpc::Trouble) {
    note_limited(place, method, said);
    let now = limit_clock();
    let mut l = limits();
    if let Some(x) = l.get_mut(&key(place, method)) {
        begin_rest(x, now);
    }
}

fn begin_rest(x: &mut Limit, now: std::time::Instant) {
    if x.rest.as_ref().is_none_or(|r| now >= r.until) {
        let length = x.rest.as_ref().map_or(REST_FIRST, |r| r.length);
        x.rest = Some(Rest { until: now + length, length });
        x.run = None;
    }
}

pub fn limits_rest(count: usize, since: std::time::Duration, pauses: &[std::time::Duration]) -> bool {
    count > pauses.len() && since >= pauses.iter().sum::<std::time::Duration>()
}

pub(crate) enum Gate {
    Open,
    Probe,
    Rests(crate::rpc::Trouble),
}

pub(crate) fn gate(place: &str, method: &str) -> Gate {
    let now = limit_clock();
    let mut l = limits();
    let Some(x) = l.get_mut(&key(place, method)) else { return Gate::Open };
    let said = x.said.clone();
    let Some(r) = x.rest.as_mut() else { return Gate::Open };
    if now < r.until {
        return Gate::Rests(said);
    }
    r.until = now + r.length;
    Gate::Probe
}

pub(crate) fn heard(place: &str, method: &str, got: &Result<crate::wire::W, crate::rpc::Trouble>, probe: bool) {
    heard_in(place, method, got, probe, true);
}

fn heard_in(place: &str, method: &str, got: &Result<crate::wire::W, crate::rpc::Trouble>, probe: bool, in_row: bool) {
    match got {
        Ok(_) => {
            if let Some(x) = limits().get_mut(&key(place, method)) {
                x.rest = None;
                x.run = None;
            }
        }
        Err(t) if crate::patience::class_of(t) == crate::patience::Class::RateLimited => {
            if probe {
                note_limited(place, method, t);
                let now = limit_clock();
                if let Some(r) = limits().get_mut(&key(place, method)).and_then(|x| x.rest.as_mut()) {
                    r.length = (r.length * 2).min(REST_MOST);
                    r.until = now + r.length;
                }
            } else {
                limited_in_a_row(place, method, t, in_row);
            }
        }
        Err(_) if probe => {
            if let Some(x) = limits().get_mut(&key(place, method)) {
                x.rest = None;
                x.run = None;
            }
        }
        Err(_) => {}
    }
}

fn limited_in_a_row(place: &str, method: &str, said: &crate::rpc::Trouble, in_row: bool) {
    let pauses = crate::patience::pauses(crate::patience::Class::RateLimited);
    let now = limit_clock();
    let mut l = limits();
    let x = l.entry(key(place, method)).or_insert_with(|| Limit { last: now, said: said.clone(), rest: None, run: None });
    x.last = now;
    x.said = said.clone();
    if !in_row || x.rest.is_some() {
        return;
    }
    let (count, from) = x.run.map_or((1, now), |(n, from)| (n + 1, from));
    x.run = Some((count, from));
    if limits_rest(count, now.saturating_duration_since(from), &pauses) {
        begin_rest(x, now);
    }
}

pub const LACK_FOR: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

static LACKS: std::sync::Mutex<std::collections::BTreeMap<(String, String), crate::settled::Lack>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

static LACKS_TAKEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn clock() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn stands(l: &crate::settled::Lack, now: u64) -> bool {
    now.checked_sub(l.at).is_some_and(|age| age <= LACK_FOR.as_secs())
}

fn lacks(now: u64) -> std::sync::MutexGuard<'static, std::collections::BTreeMap<(String, String), crate::settled::Lack>> {
    let mut g = LACKS.lock().unwrap_or_else(|e| e.into_inner());
    if !LACKS_TAKEN.load(std::sync::atomic::Ordering::SeqCst) {
        if let Some(notes) = crate::settled::notes() {
            for (key, l) in notes.lacks {
                if stands(&l, now) {
                    g.entry(key).or_insert(l);
                }
            }
            LACKS_TAKEN.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    g
}

pub fn lacking(place: &str, method: &str) -> Option<String> {
    let place = &crate::settled::spell(place);
    let now = clock();
    lacks(now).get(&(place.to_string(), method.to_string())).filter(|l| stands(l, now)).map(|l| l.said.clone())
}

pub fn remember_lack(place: &str, method: &str, said: &str) {
    let place = &crate::settled::spell(place);
    let now = clock();
    let lack = crate::settled::Lack { said: said.to_string(), at: now };
    lacks(now).insert((place.to_string(), method.to_string()), lack.clone());
    crate::settled::keep_lack(place, method, lack, now.saturating_sub(LACK_FOR.as_secs()));
}

pub fn forget_lacks() {
    LACKS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    LACKS_TAKEN.store(false, std::sync::atomic::Ordering::SeqCst);
}

pub struct Remembering {
    at: String,
    ep: Box<dyn crate::rpc::Endpoint + Send>,
}

pub fn remembering(at: &str, ep: Box<dyn crate::rpc::Endpoint + Send>) -> Box<dyn crate::rpc::Endpoint + Send> {
    Box::new(Remembering { at: at.to_string(), ep })
}

impl Remembering {
    fn asked(&mut self, method: &str, params: &Value, ask: impl FnOnce(&mut dyn crate::rpc::Endpoint) -> Result<crate::wire::W, crate::rpc::Trouble>) -> Result<crate::wire::W, crate::rpc::Trouble> {
        if method == crate::rpc::BROADCAST {
            let tx = match params {
                Value::Arr(ps) => ps.first().and_then(Value::as_str).and_then(zikaron::hexfmt::decode).filter(|raw| !raw.is_empty()).map(|raw| zikaron::cryptox::keccak256(&raw)),
                _ => None,
            };
            let head = crate::action::head_of(&self.at);
            let out = ask(self.ep.as_mut());
            crate::action::sent_on(tx.as_ref(), head);
            return out;
        }
        if stopped() {
            return Err(crate::rpc::Trouble::Transport(crate::rpc::Broke::Short));
        }
        let place = self.ep.place();
        if let Some(said) = lacking(&place, method) {
            return Err(crate::rpc::Trouble::Node(said));
        }
        if let Some(w) = crate::action::answer_for(&self.at, method, params) {
            return Ok(w);
        }
        let probe = match gate(&place, method) {
            Gate::Rests(t) => return Err(t),
            Gate::Probe => true,
            Gate::Open => false,
        };
        crate::action::give_way();
        let out = ask(self.ep.as_mut());
        heard(&place, method, &out, probe);
        match &out {
            Ok(w) => crate::action::note(&self.at, method, params, w),
            Err(crate::rpc::Trouble::Node(said)) if over_blocks_by_code(method, params, said) => {}
            Err(t @ crate::rpc::Trouble::Node(said)) if crate::patience::lacks_the_method(method, t) => remember_lack(&place, method, said),
            Err(_) => {}
        }
        out
    }
}

fn over_blocks_by_code(method: &str, params: &Value, said: &str) -> bool {
    crate::patience::log_span(method, params).is_some_and(|n| n > 1) && crate::said::lack_said(said) == Some(crate::said::LackSaid::Code)
}

impl crate::rpc::Endpoint for Remembering {
    fn call(&mut self, method: &str, params: &Value) -> Result<crate::wire::W, crate::rpc::Trouble> {
        self.asked(method, params, |ep| ep.call(method, params))
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, crate::rpc::Trouble>> {
        if stopped() {
            return Some(Err(crate::rpc::Trouble::Transport(crate::rpc::Broke::Short)));
        }
        let place = self.ep.place();
        let mut out: crate::rpc::Batched = asks.iter().map(|(m, p)| crate::action::answered(&self.at, m, p).map(Ok)).collect();
        let mut left: Vec<usize> = (0..asks.len()).filter(|i| out[*i].is_none() && !crate::action::held_ahead(&self.at, asks[*i].0, &asks[*i].1) && asks[*i].0 != crate::rpc::BROADCAST && lacking(&place, asks[*i].0).is_none()).collect();
        let mut probes: Vec<bool> = Vec::new();
        left.retain(|i| match gate(&place, asks[*i].0) {
            Gate::Rests(t) => {
                out[*i] = Some(Err(t));
                false
            }
            g => {
                probes.push(matches!(g, Gate::Probe));
                true
            }
        });
        if left.is_empty() {
            return Some(Ok(out));
        }
        let asked: Vec<crate::rpc::Ask<'_>> = left.iter().map(|i| (asks[*i].0, asks[*i].1.clone())).collect();
        crate::action::give_way();
        let got = match self.ep.call_batch(&asked)? {
            Ok(g) => g,
            Err(t) => {
                for (i, probe) in left.iter().zip(&probes) {
                    heard_in(&place, asks[*i].0, &Err(t.clone()), *probe, false);
                }
                return Some(Err(t));
            }
        };
        for ((i, a), probe) in left.into_iter().zip(got).zip(probes) {
            if let Some(answer) = &a {
                heard_in(&place, asks[i].0, answer, probe, false);
            }
            if let Some(Ok(w)) = &a {
                let (method, params) = &asks[i];
                crate::action::noted_once(&self.at, method, params, w);
            }
            out[i] = a;
        }
        Some(Ok(out))
    }
    fn another(&self) -> Option<Box<dyn crate::rpc::Endpoint + Send>> {
        Some(remembering(&self.at, self.ep.another()?))
    }
    fn call_within(&mut self, method: &str, params: &Value, within: std::time::Duration) -> Result<crate::wire::W, crate::rpc::Trouble> {
        self.asked(method, params, |ep| ep.call_within(method, params, within))
    }
    fn waits_out_limits(&self, method: &str) -> bool {
        self.ep.waits_out_limits(method)
    }
    fn asks_a_node(&self) -> bool {
        self.ep.asks_a_node()
    }
    fn name(&self) -> String {
        self.ep.name()
    }
    fn place(&self) -> String {
        self.ep.place()
    }
}

#[cfg(test)]
mod limit_tests;

#[cfg(test)]
mod lack_tests {
    use super::*;

    #[test]
    fn a_lack_stands_a_day() {
        let l = |at: u64| crate::settled::Lack { said: String::new(), at };
        let day = LACK_FOR.as_secs();
        assert!(stands(&l(1000), 1000));
        assert!(stands(&l(1000), 1000 + day));
        assert!(!stands(&l(1000), 1000 + day + 1));
        assert!(!stands(&l(1000), 999));
    }
}

#[cfg(test)]
mod behind_tests {
    use super::*;

    fn log(block: u64, tag: &str) -> Value {
        Value::Obj(vec![("blockNumber".into(), Value::Str(format!("0x{block:x}"))), ("tag".into(), Value::Str(tag.into()))])
    }

    fn block(v: &Value) -> Option<u64> {
        v.member("blockNumber").and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok())
    }

    #[test]
    fn a_rest_rows_logs_are_only_behind_when_they_stop_short() {
        let full = vec![log(5, "a"), log(7, "b"), log(9, "c")];
        assert!(only_behind(&full[..2], &full, block), "the last block missing");
        assert!(only_behind(&full[..1], &full, block), "the last two blocks missing");
        assert!(only_behind(&[], &full, block), "none given yet");
        assert!(!only_behind(&full, &full, block), "the same answer is not behind");
        assert!(!only_behind(&[log(5, "a"), log(9, "c")], &full, block), "a log missing in the middle contradicts");
        assert!(!only_behind(&[log(5, "a"), log(6, "x")], &full, block), "a log the other does not give contradicts");
        assert!(!only_behind(&[log(5, "a"), log(7, "b")], &[log(5, "a"), log(7, "b"), log(7, "c")], block), "a block of its own given in part");
    }
}
