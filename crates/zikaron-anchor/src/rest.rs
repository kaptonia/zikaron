use crate::rpc::{self, Broke, Limits, Trouble};
use crate::wire::{self, Body, W};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use zikaron::json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Etherscan,
    Blockscout,
    Like,
}

impl Service {
    pub fn of(host: &str) -> Service {
        let h = host.trim().to_ascii_lowercase();
        if h == "etherscan.io" || h.ends_with(".etherscan.io") {
            Service::Etherscan
        } else if h == "blockscout.com" || h.ends_with(".blockscout.com") {
            Service::Blockscout
        } else {
            Service::Like
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Service::Etherscan => "etherscan",
            Service::Blockscout => "blockscout",
            Service::Like => "etherscan-like",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotABase {
    Scheme,
    Query,
    Address(zikaron_net::NotAnAddress),
}

pub fn base_of(text: &str) -> Result<zikaron_net::Target, NotABase> {
    let t = text.trim();
    let target = zikaron_net::read_address(t).map_err(NotABase::Address)?;
    if target.scheme != zikaron_net::Scheme::Https {
        return Err(NotABase::Scheme);
    }
    if t.contains('?') || t.contains('#') {
        return Err(NotABase::Query);
    }
    Ok(target)
}

pub fn service_key(base: &str) -> Option<String> {
    base_of(base).ok().map(|t| t.authority())
}

pub const MARK: &str = "#rest=";

pub fn lane(base: &str, chain: u64) -> String {
    format!("{}{MARK}{chain}", base.trim())
}

pub fn of_lane(url: &str) -> Option<(&str, u64)> {
    let (base, chain) = url.trim().rsplit_once(MARK)?;
    Some((base, chain.parse().ok()?))
}

fn place_at(authority: &str) -> String {
    format!("rest://{authority}")
}

pub fn place(url: &str) -> Option<String> {
    let (base, _) = of_lane(url)?;
    service_key(base).map(|a| place_at(&a))
}

pub fn place_beside(url: &str, rest: &[String]) -> Option<String> {
    let at = place_at(&zikaron_net::parse(url)?.authority());
    rest.contains(&at).then_some(at)
}

static HELD: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

static SOURCE: std::sync::OnceLock<fn() -> Vec<(String, String)>> = std::sync::OnceLock::new();

pub fn keys_from(source: fn() -> Vec<(String, String)>) {
    let _ = SOURCE.set(source);
}

pub fn hold_keys(keys: Vec<(String, String)>) {
    *HELD.lock().unwrap_or_else(|e| e.into_inner()) = keys;
}

static FILE: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub fn hold_file_keys(keys: Vec<(String, String)>) {
    *FILE.lock().unwrap_or_else(|e| e.into_inner()) = keys;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeysFileBad {
    Unreadable,
    NotUtf8,
    Shape,
}

pub fn keys_file(path: &std::path::Path) -> Result<Vec<(String, String)>, KeysFileBad> {
    let bytes = std::fs::read(path).map_err(|_| KeysFileBad::Unreadable)?;
    let text = String::from_utf8(bytes).map_err(|_| KeysFileBad::NotUtf8)?;
    let mut keys = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let mut parts = line.split_whitespace();
        let (Some(service), Some(key), None) = (parts.next(), parts.next(), parts.next()) else { return Err(KeysFileBad::Shape) };
        keys.push((service.to_ascii_lowercase(), key.to_string()));
    }
    Ok(keys)
}

fn held_for(keys: &[(String, String)], service: &str) -> Option<String> {
    let by = |want: &str| keys.iter().find(|(s, k)| s.eq_ignore_ascii_case(want) && !k.is_empty()).map(|(_, k)| k.clone());
    by(service).or_else(|| by(Service::of(service).word()))
}

pub fn key_of(service: &str) -> Option<String> {
    let keys = match SOURCE.get() {
        Some(f) => f(),
        None => HELD.lock().unwrap_or_else(|e| e.into_inner()).clone(),
    };
    held_for(&keys, service).or_else(|| file_key(service))
}

fn file_key(service: &str) -> Option<String> {
    held_for(&FILE.lock().unwrap_or_else(|e| e.into_inner()), service)
}

pub fn file_gives(service: &str) -> bool {
    file_key(service).is_some()
}

static PASSED: Mutex<BTreeSet<(String, u64)>> = Mutex::new(BTreeSet::new());

static INSTANCES: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

pub const ETHERSCAN_PATH: &str = "/v2/api";

pub const PAGE: usize = 1000;
pub const PAGES: usize = 10;
pub const BLOCKSCOUT_LOGS: usize = 1000;

pub struct Rest {
    base: String,
    target: zikaron_net::Target,
    chain: u64,
    service: Service,
    key: Option<String>,
    limits: Limits,
    said: Box<dyn Fn(zikaron_net::Fail) -> Trouble + Send>,
    next_id: u64,
}

impl Rest {
    pub fn open(url: &str, said: impl Fn(zikaron_net::Fail) -> Trouble + Send + 'static) -> Option<Rest> {
        let (base, chain) = of_lane(url)?;
        let target = base_of(base).ok()?;
        let service = Service::of(&target.host);
        let key = key_of(&target.authority());
        Some(Rest { base: base.trim().to_string(), target, chain, service, key, limits: Limits::from_env(), said: Box::new(said), next_id: 1 })
    }

    fn authority(&self) -> String {
        self.target.authority()
    }

    fn unserved(&self, method: &str) -> Trouble {
        Trouble::Transport(Broke::Unserved { node: zikaron_net::sayable(&self.base), method: method.to_string() })
    }

    fn shapeless(&self) -> Trouble {
        rpc::shapeless(&self.base)
    }

    fn through_rpc(&self) -> bool {
        self.service == Service::Blockscout || INSTANCES.lock().unwrap_or_else(|e| e.into_inner()).contains(&self.authority())
    }

    fn ask(&mut self, method: &str, params: &Value, limits: Limits) -> Result<W, Trouble> {
        let Some(key) = self.key.clone() else {
            return Err(Trouble::Transport(Broke::NoKey { node: zikaron_net::sayable(&self.base) }));
        };
        let got = if self.through_rpc() {
            self.json_rpc(&key, method, params, &limits)
        } else {
            match self.scan_api(&key, method, params, &limits) {
                Err(t) if self.service == Service::Like && no_module(&t) => {
                    INSTANCES.lock().unwrap_or_else(|e| e.into_inner()).insert(self.authority());
                    self.json_rpc(&key, method, params, &limits)
                }
                other => other,
            }
        };
        if got.is_ok() {
            PASSED.lock().unwrap_or_else(|e| e.into_inner()).insert((self.authority(), self.chain));
        }
        got
    }

    fn rpc_target(&self, key: &str) -> zikaron_net::Target {
        let path = if self.target.host == "api.blockscout.com" {
            format!("/{}/json-rpc", self.chain)
        } else {
            let p = self.target.path.trim_end_matches('/');
            if p.ends_with("/api") { format!("{p}/eth-rpc") } else { format!("{p}/api/eth-rpc") }
        };
        zikaron_net::Target { path: format!("{path}?apikey={}", encode(key)), ..self.target.clone() }
    }

    fn json_rpc(&mut self, key: &str, method: &str, params: &Value, limits: &Limits) -> Result<W, Trouble> {
        let filtered;
        let params = if method == "eth_getLogs" {
            filtered = one_address(params).ok_or_else(|| self.unserved(method))?;
            &filtered
        } else {
            params
        };
        let id = self.next_id;
        self.next_id += 1;
        let target = self.rpc_target(key);
        let said = &self.said;
        let got = rpc::ask_node(&self.base, &target, limits, id, method, params, &|f| said(f))?;
        if method == "eth_getLogs" && got.as_arr().is_some_and(|a| a.len() >= BLOCKSCOUT_LOGS) {
            return Err(more_than(BLOCKSCOUT_LOGS));
        }
        Ok(got)
    }

    fn form_path(&self) -> String {
        let p = self.target.path.trim_end_matches('/');
        match (p.is_empty(), self.service) {
            (true, Service::Etherscan) => ETHERSCAN_PATH.to_string(),
            (true, _) => "/".to_string(),
            (false, _) => p.to_string(),
        }
    }

    fn get(&self, key: &str, query: &[(&str, String)], limits: &Limits, logs: bool) -> Result<W, Trouble> {
        let mut all: Vec<(&str, String)> = vec![("chainid", self.chain.to_string())];
        all.extend(query.iter().cloned());
        all.push(("apikey", key.to_string()));
        let form = all.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect::<Vec<_>>().join("&");
        let once = query.iter().any(|(k, v)| *k == "action" && v == rpc::BROADCAST);
        pace(&self.authority());
        let got = if once {
            zikaron_net::post(&zikaron_net::Target { path: self.form_path(), ..self.target.clone() }, "application/x-www-form-urlencoded", form.as_bytes(), limits, zikaron_net::Ask::Once)
        } else {
            zikaron_net::get(&zikaron_net::Target { path: format!("{}?{form}", self.form_path()), ..self.target.clone() }, limits)
        }
        .map_err(|f| (self.said)(f))?;
        let read = self.read(&got, logs);
        if let Err(Trouble::Node(words)) = &read {
            if let Some(per_sec) = rate_named(words) {
                learn_rate(&self.authority(), per_sec);
            }
        }
        read
    }

    fn read(&self, got: &zikaron_net::Answer, logs: bool) -> Result<W, Trouble> {
        let Some(v) = wire::parse(&got.body) else {
            return Err(if (200..300).contains(&got.status) { Trouble::Transport(Broke::NotJson) } else { rpc::status(&self.base, got.status) });
        };
        if let Some(e) = v.member("error").filter(|e| !e.is_null()) {
            return Err(Trouble::Node(wire::write(e)));
        }
        let result = v.member("result");
        match (v.member("status").and_then(W::as_str), result) {
            (Some("1"), Some(r)) => Ok(r.clone()),
            (Some("0"), Some(r)) if logs && r.as_arr().is_some() => Ok(r.clone()),
            (Some("0"), Some(W { body: Body::Str(words), .. })) => Err(refused(words, v.member("message").and_then(W::as_str))),
            (Some("0"), _) => Err(refused(v.member("message").and_then(W::as_str).unwrap_or(""), None)),
            (None, Some(r)) if (200..300).contains(&got.status) => Ok(r.clone()),
            _ if !(200..300).contains(&got.status) => Err(rpc::status(&self.base, got.status)),
            _ => Err(self.shapeless()),
        }
    }

    fn paced(&self, key: &str, query: &[(&str, String)], limits: &Limits, logs: bool) -> Result<W, Trouble> {
        let pauses = crate::patience::pauses(crate::patience::Class::RateLimited);
        let mut n = 0usize;
        loop {
            match self.get(key, query, limits, logs) {
                Err(t) if crate::said::says_rate_limited(&t) && n < pauses.len() => {
                    if !pauses[n].is_zero() {
                        std::thread::sleep(pauses[n]);
                    }
                    n += 1;
                }
                other => return other,
            }
        }
    }

    fn proxy(&self, key: &str, action: &str, more: Vec<(&'static str, String)>, limits: &Limits) -> Result<W, Trouble> {
        let mut q: Vec<(&str, String)> = vec![("module", "proxy".into()), ("action", action.into())];
        q.extend(more);
        self.get(key, &q, limits, false)
    }

    fn scan_api(&self, key: &str, method: &str, params: &Value, limits: &Limits) -> Result<W, Trouble> {
        let ps: &[Value] = match params {
            Value::Arr(a) => a,
            _ => return Err(self.unserved(method)),
        };
        let text = |i: usize| match ps.get(i) {
            Some(Value::Str(s)) => Some(s.clone()),
            _ => None,
        };
        let no = || self.unserved(method);
        match method {
            "eth_chainId" if ps.is_empty() => {
                if !PASSED.lock().unwrap_or_else(|e| e.into_inner()).contains(&(self.authority(), self.chain)) {
                    self.proxy(key, "eth_blockNumber", Vec::new(), limits)?;
                }
                Ok(W::of(Body::Str(format!("0x{:x}", self.chain))))
            }
            "eth_blockNumber" if ps.is_empty() => {
                let indexed = self.indexed_head(key, limits);
                let node = self.proxy(key, method, Vec::new(), limits)?;
                let at = node.as_str().and_then(|h| u64::from_str_radix(h.trim_start_matches("0x"), 16).ok());
                match (indexed, at) {
                    (Some(i), Some(n)) if i < n => Ok(W::of(Body::Str(format!("0x{i:x}")))),
                    _ => Ok(node),
                }
            }
            "eth_gasPrice" if ps.is_empty() => self.proxy(key, method, Vec::new(), limits),
            "eth_getBlockByNumber" => match (text(0), ps.get(1), ps.len()) {
                (Some(tag), Some(Value::Bool(full)), 2) => self.proxy(key, method, vec![("tag", tag), ("boolean", full.to_string())], limits),
                _ => Err(no()),
            },
            "eth_getTransactionByHash" | "eth_getTransactionReceipt" => match (text(0), ps.len()) {
                (Some(h), 1) => self.proxy(key, method, vec![("txhash", h)], limits),
                _ => Err(no()),
            },
            "eth_getTransactionCount" | "eth_getCode" => match (text(0), text(1), ps.len()) {
                (Some(a), Some(tag), 2) => self.proxy(key, method, vec![("address", a), ("tag", tag)], limits),
                _ => Err(no()),
            },
            rpc::BROADCAST => match (text(0), ps.len()) {
                (Some(hex), 1) => self.proxy(key, method, vec![("hex", hex)], limits),
                _ => Err(no()),
            },
            "eth_call" => match (call_members(ps.first(), &["to", "data"]), text(1), ps.len()) {
                (Some(mut m), Some(tag), 2) => {
                    m.push(("tag", tag));
                    self.proxy(key, method, m, limits)
                }
                _ => Err(no()),
            },
            "eth_estimateGas" => match (call_members(ps.first(), &["to", "data", "value", "gas", "gasPrice"]), ps.len()) {
                (Some(m), 1) => self.proxy(key, method, m, limits),
                _ => Err(no()),
            },
            "eth_getBalance" => match (text(0), text(1).as_deref(), ps.len()) {
                (Some(a), Some("latest"), 2) => {
                    let r = self.get(key, &[("module", "account".into()), ("action", "balance".into()), ("address", a), ("tag", "latest".into())], limits, false)?;
                    let wei = r.as_str().and_then(|d| d.parse::<u128>().ok()).ok_or_else(|| self.shapeless())?;
                    Ok(W::of(Body::Str(format!("0x{wei:x}"))))
                }
                _ => Err(no()),
            },
            "eth_getLogs" => match (ps.first(), ps.len()) {
                (Some(f), 1) => self.logs(key, f, limits).unwrap_or_else(|| Err(no())),
                _ => Err(no()),
            },
            _ => Err(no()),
        }
    }

    fn indexed_head(&self, key: &str, limits: &Limits) -> Option<u64> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
        let q = [("module", "block".to_string()), ("action", "getblocknobytime".to_string()), ("timestamp", now.to_string()), ("closest", "before".to_string())];
        self.get(key, &q, limits, false).ok()?.as_str()?.parse().ok()
    }

    fn logs(&self, key: &str, filter: &Value, limits: &Limits) -> Option<Result<W, Trouble>> {
        let block = |k: &str| match filter.member(k) {
            Some(Value::Str(s)) => s.strip_prefix("0x").filter(|b| !b.is_empty() && b.len() <= 16).and_then(|b| u64::from_str_radix(b, 16).ok()),
            _ => None,
        };
        let (from, to) = (block("fromBlock")?, block("toBlock")?);
        let mut addresses: Vec<Option<String>> = match filter.member("address") {
            Some(Value::Str(a)) => vec![Some(a.clone())],
            Some(Value::Arr(a)) => a.iter().map(|x| x.as_str().map(|a| Some(a.to_string()))).collect::<Option<_>>()?,
            None | Some(Value::Null) => Vec::new(),
            _ => return None,
        };
        if addresses.is_empty() {
            addresses.push(None);
        }
        let mut positions: Vec<Option<Vec<String>>> = Vec::new();
        match filter.member("topics") {
            None | Some(Value::Null) => {}
            Some(Value::Arr(ts)) if ts.len() <= 4 => {
                for t in ts {
                    positions.push(match t {
                        Value::Null => None,
                        Value::Str(s) => Some(vec![s.clone()]),
                        Value::Arr(xs) if !xs.is_empty() => Some(xs.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<_>>()?),
                        _ => return None,
                    });
                }
            }
            _ => return None,
        }
        if addresses == [None] && positions.iter().all(Option::is_none) {
            return None;
        }
        let mut picks: Vec<Vec<(usize, String)>> = vec![Vec::new()];
        for (i, p) in positions.iter().enumerate() {
            if let Some(choices) = p {
                picks = picks.iter().flat_map(|done| choices.iter().map(move |c| [done.clone(), vec![(i, c.clone())]].concat())).collect();
            }
        }
        let mut found: Vec<W> = Vec::new();
        for a in &addresses {
            for pick in &picks {
                let mut q: Vec<(&str, String)> = vec![("module", "logs".into()), ("action", "getLogs".into())];
                if let Some(a) = a {
                    q.push(("address", a.clone()));
                }
                q.extend([("fromBlock", from.to_string()), ("toBlock", to.to_string())]);
                for (i, t) in pick {
                    q.push((TOPICS[*i], t.clone()));
                }
                for (x, (i, _)) in pick.iter().enumerate() {
                    for (j, _) in &pick[x + 1..] {
                        q.push((pair(*i, *j)?, "and".into()));
                    }
                }
                q.push(("offset", PAGE.to_string()));
                for page in 1..=PAGES {
                    let mut asked = q.clone();
                    asked.push(("page", page.to_string()));
                    let got = match self.paced(key, &asked, limits, true) {
                        Ok(g) => g,
                        Err(t) => return Some(Err(t)),
                    };
                    let Some(items) = got.as_arr() else { return Some(Err(self.shapeless())) };
                    let full = items.len() >= PAGE;
                    found.extend(items.iter().map(as_rpc_log));
                    if !full {
                        break;
                    }
                    if page == PAGES {
                        return Some(Err(more_than(PAGE * PAGES)));
                    }
                }
            }
        }
        Some(Ok(W::of(Body::Arr(in_chain_order(found)))))
    }
}

static PACE: Mutex<BTreeMap<String, (std::time::Duration, std::time::Instant)>> = Mutex::new(BTreeMap::new());

pub const ETHERSCAN_FREE_PER_SEC: u32 = 3;

fn pace(authority: &str) {
    if Service::of(authority.split(':').next().unwrap_or(authority)) == Service::Etherscan {
        let gap = std::time::Duration::from_millis(1100 / u64::from(ETHERSCAN_FREE_PER_SEC));
        PACE.lock().unwrap_or_else(|e| e.into_inner()).entry(authority.to_string()).or_insert((gap, std::time::Instant::now()));
    }
    let wait = {
        let mut p = PACE.lock().unwrap_or_else(|e| e.into_inner());
        let Some((gap, next)) = p.get_mut(authority) else { return };
        let now = std::time::Instant::now();
        let at = (*next).max(now);
        *next = at + *gap;
        at - now
    };
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }
}

fn learn_rate(authority: &str, per_sec: u32) {
    let gap = std::time::Duration::from_millis(1100 / u64::from(per_sec.max(1)));
    let mut p = PACE.lock().unwrap_or_else(|e| e.into_inner());
    let next = std::time::Instant::now() + gap;
    p.entry(authority.to_string()).and_modify(|e| e.0 = e.0.max(gap)).or_insert((gap, next));
}

pub fn paced_gap(authority: &str) -> Option<std::time::Duration> {
    PACE.lock().unwrap_or_else(|e| e.into_inner()).get(authority).map(|(gap, _)| *gap)
}

fn rate_named(words: &str) -> Option<u32> {
    let low = words.to_lowercase();
    let at = low.find("/sec)")?;
    let open = low[..at].rfind('(')?;
    low[open + 1..at].trim().parse().ok().filter(|n| *n > 0)
}

const TOPICS: [&str; 4] = ["topic0", "topic1", "topic2", "topic3"];

fn pair(i: usize, j: usize) -> Option<&'static str> {
    match (i, j) {
        (0, 1) => Some("topic0_1_opr"),
        (0, 2) => Some("topic0_2_opr"),
        (0, 3) => Some("topic0_3_opr"),
        (1, 2) => Some("topic1_2_opr"),
        (1, 3) => Some("topic1_3_opr"),
        (2, 3) => Some("topic2_3_opr"),
        _ => None,
    }
}

const LOG_QUANTITIES: [&str; 3] = ["blockNumber", "logIndex", "transactionIndex"];

fn as_rpc_log(log: &W) -> W {
    match &log.body {
        Body::Obj(ms) => {
            let mut out: Vec<(String, W)> = ms
                .iter()
                .filter(|(k, _)| !NOT_LOG_MEMBERS.contains(&k.as_str()))
                .map(|(k, v)| {
                    let k = if k == "timeStamp" { "blockTimestamp".to_string() } else { k.clone() };
                    match &v.body {
                        Body::Str(s) if s == "0x" && LOG_QUANTITIES.contains(&k.as_str()) => (k, W::of(Body::Str("0x0".into()))),
                        _ => (k, v.clone()),
                    }
                })
                .collect();
            if !out.iter().any(|(k, _)| k == "removed") {
                out.push(("removed".to_string(), W::of(Body::Bool(false))));
            }
            out.sort_by(|a, b| a.0.cmp(&b.0));
            W::of(Body::Obj(out))
        }
        _ => log.clone(),
    }
}

const NOT_LOG_MEMBERS: [&str; 2] = ["gasPrice", "gasUsed"];

fn in_chain_order(logs: Vec<W>) -> Vec<W> {
    let at = |w: &W, k: &str| w.member(k).and_then(crate::tx::hex_qty).unwrap_or(u64::MAX);
    let id = |w: &W| (w.member("transactionHash").and_then(W::as_str).unwrap_or("").to_ascii_lowercase(), at(w, "logIndex"));
    let mut seen: BTreeSet<(String, u64)> = BTreeSet::new();
    let mut out: Vec<W> = logs.into_iter().filter(|w| seen.insert(id(w))).collect();
    out.sort_by_key(|w| (at(w, "blockNumber"), at(w, "logIndex")));
    out
}

fn call_members(call: Option<&Value>, takes: &[&'static str]) -> Option<Vec<(&'static str, String)>> {
    let Some(Value::Obj(ms)) = call else { return None };
    ms.iter()
        .map(|(k, v)| match (takes.iter().find(|t| **t == k.as_str()), v) {
            (Some(t), Value::Str(s)) => Some((*t, s.clone())),
            _ => None,
        })
        .collect()
}

fn refused(words: &str, message: Option<&str>) -> Trouble {
    let low = words.to_ascii_lowercase();
    let w = if low.contains("limit reached") && !low.contains("rate limit") {
        format!("rate limit: {words}")
    } else if low.contains("apikey") && !low.contains("api key") {
        format!("api key: {words}")
    } else if words.is_empty() {
        message.unwrap_or("").to_string()
    } else {
        words.to_string()
    };
    Trouble::Node(wire::write(&W::of(Body::Obj(vec![("message".into(), W::of(Body::Str(w)))]))))
}

pub(crate) fn more_than(n: usize) -> Trouble {
    Trouble::Node(wire::write(&W::of(Body::Obj(vec![("message".into(), W::of(Body::Str(format!("more than {n} results"))))]))))
}

fn no_module(t: &Trouble) -> bool {
    matches!(t, Trouble::Node(e) if e.to_ascii_lowercase().contains("unknown module"))
}

fn encode(v: &str) -> String {
    let mut out = String::new();
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl rpc::Endpoint for Rest {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        self.ask(method, params, self.limits)
    }
    fn waits_out_limits(&self, method: &str) -> bool {
        method == "eth_getLogs" && !self.through_rpc()
    }
    fn call_within(&mut self, method: &str, params: &Value, within: std::time::Duration) -> Result<W, Trouble> {
        self.ask(method, params, rpc::within(self.limits, within))
    }
    fn name(&self) -> String {
        zikaron_net::sayable(&self.base)
    }
    fn place(&self) -> String {
        place_at(&self.authority())
    }
}

fn one_address(params: &Value) -> Option<Value> {
    let Value::Arr(ps) = params else { return Some(params.clone()) };
    let one = |v: &Value| match v {
        Value::Arr(xs) if xs.len() == 1 => xs[0].clone(),
        other => other.clone(),
    };
    let mut out = Vec::with_capacity(ps.len());
    for p in ps {
        let Value::Obj(members) = p else {
            out.push(p.clone());
            continue;
        };
        let mut m = Vec::with_capacity(members.len());
        for (k, v) in members {
            let v = match (k.as_str(), v) {
                ("address", Value::Arr(xs)) if xs.len() > 1 => return None,
                ("address", v) => one(v),
                ("topics", Value::Arr(ts)) => Value::Arr(ts.iter().map(one).collect()),
                (_, v) => v.clone(),
            };
            m.push((k.clone(), v));
        }
        out.push(Value::Obj(m));
    }
    Some(Value::Arr(out))
}

pub fn unserved(t: &Trouble) -> bool {
    matches!(t, Trouble::Transport(Broke::Unserved { .. }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_log_filter_goes_to_blockscout_with_one_address() {
        let s = |x: &str| Value::Str(x.to_string());
        let f = |address: Value| Value::Arr(vec![Value::Obj(vec![("address".into(), address), ("fromBlock".into(), s("0x1")), ("topics".into(), Value::Arr(vec![Value::Arr(vec![s("0xaa")]), Value::Null, s("0xbb")]))])]);
        let want = Value::Arr(vec![Value::Obj(vec![("address".into(), s("0x5d")), ("fromBlock".into(), s("0x1")), ("topics".into(), Value::Arr(vec![s("0xaa"), Value::Null, s("0xbb")]))])]);
        assert_eq!(super::one_address(&f(Value::Arr(vec![s("0x5d")]))), Some(want.clone()), "一个地址的列表即那一个地址");
        assert_eq!(super::one_address(&f(s("0x5d"))), Some(want));
        assert_eq!(super::one_address(&f(Value::Arr(vec![s("0x5d"), s("0x6e")]))), None, "两个地址不问、不拆");
    }

    use super::*;
    use crate::rpc::Endpoint as _;
    use std::io::{Read, Write};
    use std::sync::Arc;

    type Asked = Arc<Mutex<Vec<(String, String, String)>>>;

    fn service(answer: impl Fn(&str, &str, &str) -> (u16, String) + Send + Sync + 'static) -> (u16, Asked) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().expect("addr").port();
        let asked: Asked = Arc::new(Mutex::new(Vec::new()));
        let (seen, answer) = (asked.clone(), Arc::new(answer));
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let (seen, answer) = (seen.clone(), answer.clone());
                std::thread::spawn(move || {
                    let mut s = s;
                    let mut got = Vec::new();
                    let mut buf = [0u8; 8192];
                    let head_end = loop {
                        match s.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => got.extend_from_slice(&buf[..n]),
                        }
                        if let Some(i) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                    };
                    let head = String::from_utf8_lossy(&got[..head_end]).to_string();
                    let len: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
                    while got.len() < head_end + len {
                        match s.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => got.extend_from_slice(&buf[..n]),
                        }
                    }
                    let body = String::from_utf8_lossy(&got[head_end..]).to_string();
                    let mut line = head.lines().next().unwrap_or("").split(' ');
                    let (method, target) = (line.next().unwrap_or("").to_string(), line.next().unwrap_or("").to_string());
                    let (status, text) = answer(&method, &target, &body);
                    seen.lock().unwrap_or_else(|e| e.into_inner()).push((method, target, body));
                    let _ = s.write_all(format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).as_bytes());
                });
            }
        });
        (port, asked)
    }

    fn row(port: u16, path: &str, service: Service, key: Option<&str>, chain: u64) -> Rest {
        let base = format!("http://127.0.0.1:{port}{path}");
        let target = zikaron_net::parse(&base).expect("an address");
        let said_base = base.clone();
        Rest { base, target, chain, service, key: key.map(str::to_string), limits: Limits::from_env(), said: Box::new(move |f| rpc::transport_said(&said_base, &f)), next_id: 1 }
    }

    fn arr(items: &[Value]) -> Value {
        Value::Arr(items.to_vec())
    }
    fn s(x: &str) -> Value {
        Value::Str(x.to_string())
    }

    #[test]
    fn a_log_from_the_logs_module_reads_as_a_nodes_log() {
        let es = crate::wire::parse(br#"{"address":"0x5d","topics":["0xaa"],"data":"0x01","blockNumber":"0xb8475b","blockHash":"0xc5","timeStamp":"0x6aedd7c0","gasPrice":"0x3b9aca00","gasUsed":"0x22398","logIndex":"0x","transactionHash":"0x10","transactionIndex":"0x18"}"#).unwrap();
        let node = crate::wire::parse(br#"{"address":"0x5d","blockHash":"0xc5","blockNumber":"0xb8475b","blockTimestamp":"0x6aedd7c0","data":"0x01","logIndex":"0x0","removed":false,"topics":["0xaa"],"transactionHash":"0x10","transactionIndex":"0x18"}"#).unwrap();
        assert_eq!(crate::wire::write(&as_rpc_log(&es)), crate::wire::write(&node));
    }

    #[test]
    fn a_service_that_says_its_rate_is_asked_under_it() {
        assert_eq!(rate_named(r#"{"message":"Max calls per sec rate limit reached (3/sec)"}"#), Some(3));
        assert_eq!(rate_named("rate limit"), None);
        assert_eq!(paced_gap("learnt.example.test"), None, "a service never paced has no gap");
        learn_rate("learnt.example.test", 3);
        assert_eq!(paced_gap("learnt.example.test"), Some(std::time::Duration::from_millis(1100 / 3)), "three a second is kept a tenth under it");
        learn_rate("paced.example.test", 10);
        let t = std::time::Instant::now();
        for _ in 0..3 {
            pace("paced.example.test");
        }
        assert!(t.elapsed() >= std::time::Duration::from_millis(300), "three questions at ten a second take at least 0.3 s: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        pace("unpaced.example.test");
        assert!(t.elapsed() < std::time::Duration::from_millis(50), "a service that never said its rate is not held");
    }

    #[test]
    fn an_etherscan_host_is_paced_from_its_first_question() {
        crate::patience::set_waits(Some(std::time::Duration::ZERO));
        let host = "paced-first.etherscan.io";
        let t = std::time::Instant::now();
        pace(host);
        assert!(t.elapsed() < std::time::Duration::from_millis(50), "the first goes at once: {:?}", t.elapsed());
        pace(host);
        pace(host);
        let took = t.elapsed();
        assert!(took >= std::time::Duration::from_millis(2 * 330) && took < std::time::Duration::from_secs(2), "the next two wait their turns: {took:?}");
    }

    #[test]
    fn a_rate_limit_on_a_log_page_is_waited_out_in_one_layer() {
        crate::patience::set_waits(Some(std::time::Duration::ZERO));
        let (port, asked) = service(|_, _, _| (200, r#"{"status":"0","message":"NOTOK","result":"Max calls per sec rate limit reached (3/sec)"}"#.to_string()));
        let mut r = row(port, "/v2/api", Service::Etherscan, Some("KEY-1"), 11155111);
        let filter = Value::Obj(vec![("address".into(), s("0x5d")), ("fromBlock".into(), s("0x1")), ("toBlock".into(), s("0x2")), ("topics".into(), arr(&[arr(&[s("0xaa")])]))]);
        assert!(rpc::Endpoint::waits_out_limits(&r, "eth_getLogs"));
        let got = crate::patience::ask(&mut r, "eth_getLogs", &arr(&[filter]));
        assert!(crate::said::says_rate_limited(&got.expect_err("still limited")), "refused as limited");
        let pages = asked.lock().unwrap().iter().filter(|(_, t, _)| t.contains("module=logs")).count();
        assert_eq!(pages, 3, "the page and its two pauses, never the whole question again");
    }

    #[test]
    fn a_log_question_naming_no_address_is_asked_by_its_topics() {
        let (port, asked) = service(|_, _, _| (200, r#"{"status":"1","message":"OK","result":[]}"#.to_string()));
        let mut r = row(port, "/v2/api", Service::Etherscan, Some("KEY-1"), 11155111);
        let filter = Value::Obj(vec![("address".into(), arr(&[])), ("fromBlock".into(), s("0x1")), ("toBlock".into(), s("0x2")), ("topics".into(), arr(&[arr(&[s("0xaa")])]))]);
        rpc::Endpoint::call(&mut r, "eth_getLogs", &arr(&[filter])).expect("asked by topics");
        let (_, target, _) = asked.lock().unwrap().last().cloned().expect("asked");
        let qs = query(&target);
        assert_eq!((q(&qs, "topic0"), q(&qs, "address")), (Some("0xaa"), None));
        let neither = Value::Obj(vec![("address".into(), arr(&[])), ("fromBlock".into(), s("0x1")), ("toBlock".into(), s("0x2"))]);
        assert!(unserved(&rpc::Endpoint::call(&mut r, "eth_getLogs", &arr(&[neither])).expect_err("nothing to narrow it")));
    }

    #[test]
    fn a_key_file_gives_a_key_the_settings_lack_and_the_settings_key_wins() {
        hold_keys(vec![("set.example.test".into(), "FROM-SETTINGS".into())]);
        hold_file_keys(vec![("set.example.test".into(), "FROM-FILE".into()), ("unset.example.test".into(), "FILE-BY-HOST".into()), ("etherscan".into(), "FILE-BY-WORD".into())]);
        assert_eq!(key_of("set.example.test").as_deref(), Some("FROM-SETTINGS"), "the settings' key wins");
        assert_eq!(key_of("unset.example.test").as_deref(), Some("FILE-BY-HOST"));
        assert_eq!(key_of("api.etherscan.io").as_deref(), Some("FILE-BY-WORD"), "by the service's word");
        assert_eq!(key_of("other.example.test"), None);
        hold_file_keys(Vec::new());
        hold_keys(Vec::new());
    }

    #[test]
    fn a_keys_file_reads_whole_or_is_refused_by_name() {
        let dir = std::env::temp_dir().join(format!("zk-anchor-keys-file-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let at = |name: &str, bytes: &[u8]| {
            let p = dir.join(name);
            std::fs::write(&p, bytes).expect("written");
            p
        };
        let good = at("good", "# 钥匙\n\nAPI.Etherscan.io  KEY-1\n  etherscan-like KEY-2  \n".as_bytes());
        assert_eq!(keys_file(&good), Ok(vec![("api.etherscan.io".into(), "KEY-1".into()), ("etherscan-like".into(), "KEY-2".into())]));
        assert_eq!(keys_file(&dir.join("none")), Err(KeysFileBad::Unreadable));
        assert_eq!(keys_file(&at("bytes", &[0xff, 0xfe, b'\n'])), Err(KeysFileBad::NotUtf8));
        for line in ["etherscan", "etherscan KEY more"] {
            assert_eq!(keys_file(&at("shape", format!("api.etherscan.io KEY-1\n{line}\n").as_bytes())), Err(KeysFileBad::Shape), "{line}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_etherscan_rows_head_is_its_index_head_when_lower() {
        for (index, want) in [(r#"{"status":"1","message":"OK","result":"14"}"#, "0xe"), (r#"{"status":"1","message":"OK","result":"20"}"#, "0x10"), (r#"{"status":"0","message":"NOTOK","result":"Error!"}"#, "0x10")] {
            let index = index.to_string();
            let (port, asked) = service(move |_, target, _| if target.contains("action=getblocknobytime") { (200, index.clone()) } else { (200, r#"{"jsonrpc":"2.0","id":1,"result":"0x10"}"#.to_string()) });
            let mut r = row(port, "/v2/api", Service::Etherscan, Some("KEY-1"), 11155111);
            assert_eq!(r.call("eth_blockNumber", &arr(&[])).expect("head").as_str(), Some(want));
            let targets: Vec<String> = asked.lock().unwrap().iter().map(|(_, t, _)| t.clone()).collect();
            assert!(targets.iter().any(|t| t.contains("module=block") && t.contains("closest=before")), "{targets:?}");
        }
    }

    fn query(target: &str) -> Vec<(String, String)> {
        target.split_once('?').map(|(_, q)| q).unwrap_or("").split('&').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    fn q<'a>(qs: &'a [(String, String)], k: &str) -> Option<&'a str> {
        qs.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn a_base_url_reads_as_https_with_no_query_and_names_its_service() {
        assert!(base_of("https://api.etherscan.io/v2/api").is_ok());
        assert_eq!(base_of("http://api.etherscan.io/v2/api").err(), Some(NotABase::Scheme), "a key goes over https alone");
        assert_eq!(base_of("https://api.etherscan.io/v2/api?apikey=K").err(), Some(NotABase::Query), "a key pasted into the Base URL would land in the ledger's settings");
        assert_eq!(base_of("https://api.etherscan.io/#x").err(), Some(NotABase::Query));
        assert!(matches!(base_of("api.etherscan.io"), Err(NotABase::Address(_))));
        assert_eq!(Service::of("api.etherscan.io"), Service::Etherscan);
        assert_eq!(Service::of("API-SEPOLIA.Etherscan.io"), Service::Etherscan);
        assert_eq!(Service::of("eth.blockscout.com"), Service::Blockscout);
        assert_eq!(Service::of("api.blockscout.com"), Service::Blockscout);
        assert_eq!(Service::of("notetherscan.io"), Service::Like, "a host only ending in the letters is another service");
        assert_eq!(Service::of("explorer.example"), Service::Like);
        let l = lane("https://api.etherscan.io/v2/api", 11155111);
        assert_eq!(of_lane(&l), Some(("https://api.etherscan.io/v2/api", 11155111)));
        assert_eq!(of_lane("https://node.example/rpc"), None, "a JSON-RPC node is no REST row");
        assert_eq!(service_key("https://API.Etherscan.io:443/v2/api"), Some("api.etherscan.io".to_string()), "one service however it is written");
    }

    #[test]
    fn one_service_is_one_place_and_a_json_rpc_node_on_its_host_joins_it() {
        let a = lane("https://eth.blockscout.com/api", 1);
        let b = lane("https://eth.blockscout.com/other", 1);
        assert_eq!(place(&a), place(&b));
        assert_eq!(crate::endpoints::distinct_places([a.as_str(), b.as_str()]), 1);
        let rpc = "https://eth.blockscout.com/api/eth-rpc";
        assert_eq!(crate::endpoints::distinct_places([rpc, a.as_str()]), 1, "the same Blockscout as a JSON-RPC node and as a REST row is one witness");
        assert_eq!(crate::endpoints::distinct_places(["https://node.example/eth/k1", "https://node.example/eth/k2"]), 2, "JSON-RPC nodes keep their whole address");
        assert_eq!(crate::endpoints::distinct_places(["https://node.example/eth/k1", lane("https://api.etherscan.io/v2/api", 1).as_str()]), 2);
        assert_eq!(crate::endpoints::place_of(rpc), zikaron_net::place_key(rpc), "a JSON-RPC node's own place is its whole address");
    }

    #[test]
    fn each_proxy_question_is_asked_with_its_own_members_the_chain_and_the_key() {
        let (port, asked) = service(|_, _, _| (200, r#"{"jsonrpc":"2.0","id":1,"result":"0x10"}"#.to_string()));
        let mut r = row(port, "/v2/api", Service::Etherscan, Some("KEY-1"), 11155111);
        let addr = "0x00000000000000000000000000000000000000aa";
        let hash = format!("0x{}", "ab".repeat(32));
        let cases: Vec<(&str, Value, Vec<(&str, &str)>)> = vec![
            ("eth_blockNumber", arr(&[]), vec![]),
            ("eth_gasPrice", arr(&[]), vec![]),
            ("eth_getBlockByNumber", arr(&[s("0x5"), Value::Bool(false)]), vec![("tag", "0x5"), ("boolean", "false")]),
            ("eth_getTransactionByHash", arr(&[s(&hash)]), vec![("txhash", &hash)]),
            ("eth_getTransactionReceipt", arr(&[s(&hash)]), vec![("txhash", &hash)]),
            ("eth_getTransactionCount", arr(&[s(addr), s("pending")]), vec![("address", addr), ("tag", "pending")]),
            ("eth_getCode", arr(&[s(addr), s("0x5")]), vec![("address", addr), ("tag", "0x5")]),
            ("eth_call", arr(&[Value::Obj(vec![("data".into(), s("0x01")), ("to".into(), s(addr))]), s("latest")]), vec![("to", addr), ("data", "0x01"), ("tag", "latest")]),
            ("eth_estimateGas", arr(&[Value::Obj(vec![("data".into(), s("0x01")), ("to".into(), s(addr))])]), vec![("to", addr), ("data", "0x01")]),
        ];
        for (m, params, want) in &cases {
            let got = r.call(m, params).unwrap_or_else(|t| panic!("{m}: {t:?}"));
            assert_eq!(got.as_str(), Some("0x10"), "{m}");
            let (method, target, _) = asked.lock().unwrap().last().cloned().expect("asked");
            let qs = query(&target);
            assert_eq!(method, "GET", "{m}");
            assert!(target.starts_with("/v2/api?"), "{m}: {target}");
            assert_eq!((q(&qs, "module"), q(&qs, "action"), q(&qs, "chainid"), q(&qs, "apikey")), (Some("proxy"), Some(*m), Some("11155111"), Some("KEY-1")), "{m}");
            for (k, v) in want {
                assert_eq!(q(&qs, k), Some(*v), "{m} {k}");
            }
        }
        r.call(rpc::BROADCAST, &arr(&[s("0x02f8")])).expect("broadcast");
        let (method, target, body) = asked.lock().unwrap().last().cloned().expect("asked");
        assert_eq!((method.as_str(), target.as_str()), ("POST", "/v2/api"));
        let qs = query(&format!("?{body}"));
        assert_eq!((q(&qs, "action"), q(&qs, "hex"), q(&qs, "apikey")), (Some(rpc::BROADCAST), Some("0x02f8"), Some("KEY-1")));
    }

    #[test]
    fn what_the_service_cannot_answer_as_asked_is_not_served_and_never_asked() {
        let (port, asked) = service(|_, t, _| (200, if t.contains("module=account") { r#"{"status":"1","message":"OK","result":"1000000000000000000"}"# } else { r#"{"status":"1","message":"OK","result":"0x1"}"# }.to_string()));
        let mut r = row(port, "/v2/api", Service::Etherscan, Some("K"), 1);
        let addr = "0x00000000000000000000000000000000000000aa";
        let from = Value::Obj(vec![("from".into(), s(addr)), ("to".into(), s(addr))]);
        for (m, params) in [
            ("eth_getBalance", arr(&[s(addr), s("0x10")])),
            ("eth_estimateGas", arr(&[from.clone()])),
            ("eth_call", arr(&[from, s("latest")])),
            ("eth_feeHistory", arr(&[s("0x5"), s("0x10"), arr(&[Value::Int(50)])])),
            ("eth_getProof", arr(&[])),
        ] {
            match r.call(m, &params) {
                Err(t @ Trouble::Transport(Broke::Unserved { .. })) => assert!(unserved(&t)),
                other => panic!("{m}: {other:?}"),
            }
        }
        assert!(asked.lock().unwrap().is_empty(), "a question not served asks nothing");
        let latest = r.call("eth_getBalance", &arr(&[s(addr), s("latest")])).expect("latest balance");
        assert_eq!(latest.as_str(), Some("0xde0b6b3a7640000"));
    }

    #[test]
    fn the_services_refusals_read_as_rate_limiting_credentials_or_its_own_words() {
        let (port, _) = service(|_, t, _| {
            let words = match q(&query(t), "action") {
                Some("eth_blockNumber") => "Max calls per sec rate limit reached (5/sec)",
                Some("eth_gasPrice") => "Max daily limit reached",
                Some("eth_getCode") => "Missing/Invalid API Key",
                Some("eth_getTransactionCount") => "Invalid apikey",
                _ => "Free API access is not supported for this chain",
            };
            (200, format!(r#"{{"status":"0","message":"NOTOK","result":"{words}"}}"#))
        });
        let mut r = row(port, "/v2/api", Service::Etherscan, Some("K"), 8453);
        let addr = "0x00000000000000000000000000000000000000aa";
        let rate = |t: &Trouble| crate::said::says_rate_limited(t);
        let auth = |t: &Trouble| crate::said::refusal(t) == Some(crate::said::Refusal::Auth);
        let t = r.call("eth_blockNumber", &arr(&[])).expect_err("limited");
        assert!(rate(&t), "{t:?}");
        let t = r.call("eth_gasPrice", &arr(&[])).expect_err("limited");
        assert!(rate(&t), "{t:?}");
        let t = r.call("eth_getCode", &arr(&[s(addr), s("latest")])).expect_err("bad key");
        assert!(auth(&t), "{t:?}");
        let t = r.call("eth_getTransactionCount", &arr(&[s(addr), s("latest")])).expect_err("bad key");
        assert!(auth(&t), "{t:?}");
        let t = r.call("eth_getTransactionReceipt", &arr(&[s("0x00")])).expect_err("refused");
        assert!(matches!(&t, Trouble::Node(e) if e.contains("not supported for this chain")) && !rate(&t) && !auth(&t), "{t:?}");
    }

    #[test]
    fn a_row_with_no_key_here_asks_nothing() {
        let (port, asked) = service(|_, _, _| (200, r#"{"status":"1","message":"OK","result":"0x1"}"#.to_string()));
        let mut r = row(port, "/v2/api", Service::Etherscan, None, 1);
        for m in ["eth_blockNumber", "eth_chainId"] {
            assert!(matches!(r.call(m, &arr(&[])), Err(Trouble::Transport(Broke::NoKey { .. }))), "{m}");
        }
        assert!(asked.lock().unwrap().is_empty());
        assert_eq!(rpc::Trouble::Transport(Broke::NoKey { node: "https://a.example".into() }), Trouble::Transport(Broke::NoKey { node: "https://a.example".into() }));
        assert_eq!(Broke::NoKey { node: "https://a.example".into() }.evidence(), "NO_KEY_HERE https://a.example");
    }

    #[test]
    fn the_chain_id_is_the_chain_asked_once_a_question_about_it_passed() {
        let (port, asked) = service(|_, t, _| match q(&query(t), "chainid") {
            Some("1") => (200, r#"{"jsonrpc":"2.0","id":1,"result":"0x10"}"#.to_string()),
            _ => (200, r#"{"status":"0","message":"NOTOK","result":"Free API access is not supported for this chain"}"#.to_string()),
        });
        let mut one = row(port, "/v2/api", Service::Etherscan, Some("K"), 1);
        assert_eq!(one.call("eth_chainId", &arr(&[])).expect("chain 1").as_str(), Some("0x1"));
        assert_eq!(asked.lock().unwrap().len(), 1, "one real question first");
        assert_eq!(one.call("eth_chainId", &arr(&[])).expect("chain 1").as_str(), Some("0x1"));
        assert_eq!(asked.lock().unwrap().len(), 1, "once it passed, the chain is known");
        let mut other = row(port, "/v2/api", Service::Etherscan, Some("K"), 8453);
        assert!(matches!(other.call("eth_chainId", &arr(&[])), Err(Trouble::Node(_))), "a chain the service refuses is no chain id");
    }

    #[test]
    fn a_log_question_is_split_by_address_and_topic_and_read_to_its_last_page() {
        let addrs = ["0x00000000000000000000000000000000000000a1", "0x00000000000000000000000000000000000000a2"];
        let t0 = format!("0x{}", "11".repeat(32));
        let senders = [format!("0x{}", "21".repeat(32)), format!("0x{}", "22".repeat(32))];
        let log = |block: u64, index: &str, tx: &str| format!(r#"{{"address":"x","blockHash":"0x{}","blockNumber":"0x{block:x}","data":"0x","logIndex":"{index}","topics":[],"transactionHash":"{tx}","transactionIndex":"0x"}}"#, "cc".repeat(32));
        let page_of = move |t: &str| -> String {
            let qs = query(t);
            let (a, t1, page) = (q(&qs, "address").unwrap_or("").to_string(), q(&qs, "topic1").unwrap_or("").to_string(), q(&qs, "page").unwrap_or("").to_string());
            let items: Vec<String> = match (a.ends_with("a1"), t1.ends_with("21"), page.as_str()) {
                (true, true, "1") => (0..PAGE).map(|i| log(10 + i as u64, &format!("0x{:x}", i % 3), &format!("0x{:064x}", i))).collect(),
                (true, true, "2") => vec![log(5, "0x", "0xaaa"), log(5, "0x1", "0xbbb"), log(2000, "0x0", "0xccc")],
                _ => Vec::new(),
            };
            if items.is_empty() { r#"{"status":"0","message":"No records found","result":[]}"#.to_string() } else { format!(r#"{{"status":"1","message":"OK","result":[{}]}}"#, items.join(",")) }
        };
        let (port, asked) = service(move |_, t, _| (200, page_of(t)));
        let mut r = row(port, "/v2/api", Service::Etherscan, Some("K"), 1);
        let filter = Value::Obj(vec![
            ("address".into(), arr(&addrs.map(s))),
            ("fromBlock".into(), s("0x0")),
            ("toBlock".into(), s("0x1000")),
            ("topics".into(), arr(&[s(&t0), arr(&senders.clone().map(|x| s(&x)))])),
        ]);
        let got = r.call("eth_getLogs", &arr(&[filter])).expect("logs");
        let logs = got.as_arr().expect("a list");
        assert_eq!(logs.len(), PAGE + 3);
        let at = |w: &W, k: &str| w.member(k).and_then(crate::tx::hex_qty).expect(k);
        assert_eq!((at(&logs[0], "blockNumber"), at(&logs[0], "logIndex")), (5, 0), "in chain order, a bare 0x read as zero");
        assert_eq!(logs[0].member("transactionIndex").and_then(W::as_str), Some("0x0"));
        assert!(logs.windows(2).all(|w| (at(&w[0], "blockNumber"), at(&w[0], "logIndex")) <= (at(&w[1], "blockNumber"), at(&w[1], "logIndex"))));
        let asked = asked.lock().unwrap().clone();
        assert_eq!(asked.len(), 5);
        for (_, t, _) in &asked {
            let qs = query(t);
            assert_eq!((q(&qs, "module"), q(&qs, "action"), q(&qs, "fromBlock"), q(&qs, "toBlock"), q(&qs, "offset")), (Some("logs"), Some("getLogs"), Some("0"), Some("4096"), Some("1000")));
            assert_eq!((q(&qs, "topic0"), q(&qs, "topic0_1_opr")), (Some(t0.as_str()), Some("and")));
        }
    }

    #[test]
    fn past_the_last_page_a_log_question_is_refused_naming_the_results() {
        let full = format!(r#"{{"status":"1","message":"OK","result":[{}]}}"#, vec![r#"{"blockNumber":"0x1","logIndex":"0x0","transactionHash":"0x1"}"#; PAGE].join(","));
        let (port, asked) = service(move |_, _, _| (200, full.clone()));
        let mut r = row(port, "/v2/api", Service::Etherscan, Some("K"), 1);
        let filter = Value::Obj(vec![("address".into(), s("0x00000000000000000000000000000000000000a1")), ("fromBlock".into(), s("0x0")), ("toBlock".into(), s("0x100000"))]);
        let t = r.call("eth_getLogs", &arr(&[filter])).expect_err("too many");
        assert_eq!(t, more_than(PAGE * PAGES));
        assert_eq!(asked.lock().unwrap().len(), PAGES);
        assert!(crate::patience::names_range(match &t { Trouble::Node(e) => e, _ => "" }), "it names the results, so it is never waited out as a rate limit");
        assert!(unserved(&r.call("eth_getLogs", &arr(&[Value::Obj(vec![("fromBlock".into(), s("latest")), ("toBlock".into(), s("latest")), ("address".into(), s("0x1"))])])).expect_err("latest")));
        assert!(unserved(&r.call("eth_getLogs", &arr(&[Value::Obj(vec![("fromBlock".into(), s("0x0")), ("toBlock".into(), s("0x1"))])])).expect_err("no address")));
    }

    #[test]
    fn a_service_without_the_proxy_module_is_asked_through_its_json_rpc() {
        let (port, asked) = service(|m, t, body| {
            if m == "GET" {
                return (200, r#"{"message":"Unknown module","result":null,"status":"0"}"#.to_string());
            }
            if !t.starts_with("/api/eth-rpc?apikey=K") {
                return (404, "{}".to_string());
            }
            if body.contains("eth_gasPrice") {
                return (429, r#"{"message":"Too Many Requests"}"#.to_string());
            }
            let id = body.split("\"id\":").nth(1).and_then(|x| x.split(',').next()).unwrap_or("1").to_string();
            if body.contains("eth_getLogs") {
                let logs = vec![r#"{"blockNumber":"0x1"}"#; BLOCKSCOUT_LOGS].join(",");
                return (200, format!(r#"{{"jsonrpc":"2.0","id":{id},"result":[{logs}]}}"#));
            }
            (200, format!(r#"{{"jsonrpc":"2.0","id":{id},"result":"0xaa36a7"}}"#))
        });
        let mut r = row(port, "/api", Service::Like, Some("K"), 11155111);
        assert_eq!(r.call("eth_chainId", &arr(&[])).expect("its own chain id").as_str(), Some("0xaa36a7"));
        let first = asked.lock().unwrap().clone();
        assert_eq!(first.iter().map(|(m, _, _)| m.as_str()).collect::<Vec<_>>(), vec!["GET", "POST"], "asked as Etherscan once, then through its JSON-RPC");
        assert_eq!(r.call("eth_blockNumber", &arr(&[])).expect("head").as_str(), Some("0xaa36a7"));
        assert_eq!(asked.lock().unwrap().last().map(|(m, _, _)| m.clone()), Some("POST".to_string()), "known now: straight to its JSON-RPC");
        let t = r.call("eth_gasPrice", &arr(&[])).expect_err("429");
        assert!(crate::said::says_rate_limited(&t) && rpc::status_of(&t) == Some(429), "{t:?}");
        assert_eq!(r.call("eth_getLogs", &arr(&[Value::Obj(vec![])])).expect_err("cut"), more_than(BLOCKSCOUT_LOGS));
    }

    #[test]
    fn a_blockscout_rows_json_rpc_is_its_chains_or_its_instances() {
        let api = Rest::open(&lane("https://api.blockscout.com", 8453), |f| rpc::transport_said("x", &f)).expect("a row");
        assert_eq!(api.rpc_target("K").path, "/8453/json-rpc?apikey=K");
        let inst = Rest::open(&lane("https://eth.blockscout.com/api/", 1), |f| rpc::transport_said("x", &f)).expect("a row");
        assert_eq!(inst.rpc_target("K").path, "/api/eth-rpc?apikey=K");
        let bare = Rest::open(&lane("https://eth.blockscout.com", 1), |f| rpc::transport_said("x", &f)).expect("a row");
        assert_eq!(bare.rpc_target("K/+").path, "/api/eth-rpc?apikey=K%2F%2B", "the key is one query value");
        assert_eq!((api.name(), inst.place()), ("https://api.blockscout.com".to_string(), "rest://eth.blockscout.com".to_string()));
        assert!(Rest::open(&lane("http://eth.blockscout.com/api", 1), |f| rpc::transport_said("x", &f)).is_none(), "a Base URL that does not read opens nothing");
    }

    #[test]
    fn a_recording_through_a_rest_row_holds_no_byte_of_the_key() {
        let (port, _) = service(|_, _, _| (200, r#"{"jsonrpc":"2.0","id":1,"result":"0x10"}"#.to_string()));
        let key = "SECRET-KEY-0123456789";
        let mut rec = rpc::Recorder::new(Box::new(row(port, "/v2/api", Service::Etherscan, Some(key), 1)));
        rec.call("eth_blockNumber", &arr(&[])).expect("head");
        rec.call("eth_getCode", &arr(&[s("0x00000000000000000000000000000000000000aa"), s("latest")])).expect("code");
        let _ = rec.call("eth_feeHistory", &arr(&[])).expect_err("not served");
        let bytes: String = rec.exchanges.iter().map(wire::write).collect();
        assert_eq!(rec.exchanges.len(), 2, "a question not served is no answer of the node's, not recorded");
        assert!(!bytes.contains(key) && !bytes.contains("apikey"), "{bytes}");
        assert!(!rec.name().contains(key) && !rec.place().contains(key));
    }
}
