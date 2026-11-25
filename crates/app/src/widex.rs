use crate::auditx::{Ground, Scan};
use crate::chainx::Endpoint;
use crate::fault::{Fault, Known};
use crate::key::Address;
use crate::lang::Key;
use crate::readnets::Net;
use zikaron::json::Value;
use zikaron_anchor::scan::Emitters;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    Agreed(usize),
    Single,
    Down,
    Fingerprint,
}

impl Reading {
    pub fn admits(self) -> bool {
        matches!(self, Reading::Agreed(_) | Reading::Single)
    }

    pub fn code(self) -> &'static str {
        match self {
            Reading::Agreed(_) => "agreed",
            Reading::Single => "single",
            Reading::Down => "down",
            Reading::Fingerprint => "fingerprint",
        }
    }

    pub fn key(self) -> Key {
        match self {
            Reading::Agreed(_) => Key::BadgeAgreed,
            Reading::Single => Key::BadgeSingle,
            Reading::Down => Key::BadgeDown,
            Reading::Fingerprint => Key::BadgeFingerprint,
        }
    }
}

pub fn gate(net: &Net) -> Reading {
    gate_said(net).0
}

pub fn gate_said(net: &Net) -> (Reading, Option<Fault>) {
    gate_said_against(net, crate::pinned::CODE_HASH)
}

pub fn gate_said_against(net: &Net, pinned: &str) -> (Reading, Option<Fault>) {
    let (reading, said, _) = gate_counted_against(net, pinned);
    (reading, said)
}

fn gate_counted_against(net: &Net, pinned: &str) -> (Reading, Option<Fault>, Net) {
    use crate::chainx::{FingerprintGate as Gate, FingerprintMiss as Why};
    let (gate, counted) = crate::chainx::fingerprint_gate_counted(net.chain_id, &net.registry.0, &net.nodes, pinned);
    let on_counted = Net { nodes: net.nodes.iter().filter(|u| counted.contains(u)).cloned().collect(), ..net.clone() };
    let (reading, said) = match gate {
        Gate::Agreed(n) => (Reading::Agreed(n), None),
        Gate::Single => (Reading::Single, None),
        Gate::Fingerprint => (Reading::Fingerprint, None),
        Gate::Down(misses) => {
            let mut first: Option<Fault> = None;
            for m in &misses {
                if let Why::Trouble(t) = &m.why {
                    first.get_or_insert(crate::chainx::said_fault(m.url.as_str(), t));
                }
            }
            let words = crate::lang::both_of(|| {
                let mut words: Vec<String> = Vec::new();
                for m in &misses {
                    let url = m.url.as_str();
                    match &m.why {
                        Why::NotAddress => words.push(format!("{}: not a node address", zikaron_net::sayable(url))),
                        Why::Trouble(t) => words.push(crate::chainx::trouble_said(url, t)),
                        Why::OtherChain(said) => words.push(format!("{}: eth_chainId {said}", zikaron_net::sayable(url))),
                        Why::NoCode => words.push(format!("{}: eth_getCode answered no code", zikaron_net::sayable(url))),
                    }
                }
                words.join(" · ")
            });
            let f = match first.filter(|f| f.which().is_some()) {
                Some(f) => f.retold(words),
                None => Fault::known(Known::Unreachable, words),
            };
            (Reading::Down, Some(f))
        }
    };
    (reading, said, on_counted)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missed {
    pub chain_id: u64,
    pub registry: Address,
    pub from_block: u64,
    pub name: String,
    pub reading: Reading,
    pub nodes: usize,
}

impl Missed {
    pub fn key(&self) -> Key {
        match (self.reading, self.nodes) {
            (Reading::Down, 0) => Key::BadgeNoNode,
            (r, _) => r.key(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Wide {
    pub fragment: Value,
    pub anchors: usize,
    pub asked: usize,
    pub single_source: bool,
    pub unanswered: Vec<String>,
    pub emitters: Emitters,
    pub missed: Vec<Missed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    First,
    Agreed,
}

struct Window {
    chain_id: u64,
    from_block: u64,
    registries: Vec<Address>,
    eps: Vec<Endpoint>,
    from: Vec<(Address, String, u64)>,
}

fn basis_of(w: &Window, to_block: u64, senders: &[String]) -> Value {
    let window = Value::Obj(vec![
        ("chainId".to_string(), Value::Int(w.chain_id)),
        ("fromBlock".to_string(), Value::Int(w.from_block)),
        ("registries".to_string(), Value::Arr(w.registries.iter().map(|r| Value::Str(r.hex())).collect())),
        ("senders".to_string(), Value::Arr(senders.iter().map(|s| Value::Str(s.clone())).collect())),
        ("toBlock".to_string(), Value::Int(to_block)),
    ]);
    Value::Obj(vec![("chains".to_string(), Value::Arr(vec![window]))])
}

fn arr<'a>(v: &'a Value, k: &str) -> Vec<Value> {
    match v.member(k) {
        Some(Value::Arr(a)) => a.clone(),
        _ => Vec::new(),
    }
}

fn windows(main: Option<(&[Endpoint], &Ground)>, nets: &[Net], missed: &mut Vec<Missed>) -> Vec<Window> {
    let mut out: Vec<Window> = Vec::new();
    let mut add = |chain_id: u64, registry: Address, from_block: u64, eps: Vec<Endpoint>, name: String| {
        match out.iter_mut().find(|w| w.chain_id == chain_id) {
            Some(w) => {
                w.from_block = w.from_block.min(from_block);
                if !w.registries.contains(&registry) {
                    w.registries.push(registry);
                }
                for e in eps {
                    if !w.eps.iter().any(|x| x.url == e.url) {
                        w.eps.push(e);
                    }
                }
                match w.from.iter_mut().find(|(r, _, _)| *r == registry) {
                    Some(one) => one.2 = one.2.min(from_block),
                    None => w.from.push((registry, name, from_block)),
                }
            }
            None => out.push(Window { chain_id, from_block, registries: vec![registry], eps, from: vec![(registry, name, from_block)] }),
        }
    };
    if let Some((eps, g)) = main {
        let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == g.chain).cloned().collect();
        add(g.chain, g.registry, g.from_block, mine, crate::readnets::chain_name(g.chain, None));
        for w in &g.earlier {
            let theirs: Vec<Endpoint> = eps.iter().filter(|e| e.chain == w.chain).cloned().collect();
            add(w.chain, w.registry, w.from_block, theirs, crate::readnets::chain_name(w.chain, None));
        }
    }
    for n in nets {
        let is_main = main.map(|(_, g)| n.is(g.chain, &g.registry)).unwrap_or(false);
        let (reading, _, n) = gate_counted_against(n, crate::pinned::CODE_HASH);
        let n = &n;
        if is_main && !reading.admits() {
            continue;
        }
        if !reading.admits() {
            missed.push(Missed { chain_id: n.chain_id, registry: n.registry, from_block: n.from_block, name: n.name(), reading, nodes: n.nodes.len() });
            continue;
        }
        add(n.chain_id, n.registry, n.from_block, n.endpoints(), n.name());
    }
    for w in out.iter_mut() {
        let read: Vec<[u8; 20]> = w.registries.iter().map(|r| r.0).collect();
        w.registries = zikaron_anchor::revoke::registries(w.chain_id, &read).into_iter().map(Address).collect();
    }
    out.sort_by_key(|w| w.chain_id);
    out
}

pub fn scan(main: Option<(&[Endpoint], &Ground)>, nets: &[Net], senders: &[String], ask: Ask) -> Result<Wide, Fault> {
    scan_picked(main, nets, senders, ask, &zikaron_anchor::scan::Pick::All)
}

pub fn scan_picked(main: Option<(&[Endpoint], &Ground)>, nets: &[Net], senders: &[String], ask: Ask, pick: &zikaron_anchor::scan::Pick) -> Result<Wide, Fault> {
    let mut missed: Vec<Missed> = Vec::new();
    let mut read: Vec<Value> = Vec::new();
    let mut chains: Vec<Value> = Vec::new();
    let mut emitters = Emitters::new();
    let (mut asked, mut single_source, mut unanswered) = (0usize, false, Vec::new());
    let mut first: Option<Fault> = None;
    let mut read_any = false;
    for w in windows(main, nets, &mut missed) {
        let pass = (|| -> Result<(Value, Option<Emitters>, usize, bool, Vec<String>), Fault> {
            let (head, _) = crate::chainx::head_block(&w.eps, w.chain_id)?;
            let bytes = zikaron::json::canon_bytes(&basis_of(&w, head.max(w.from_block), senders));
            match ask {
                Ask::First => {
                    let (s, e, said) = crate::auditx::scan_table_picked(&w.eps, &bytes, pick).map_err(Fault::from)?;
                    Ok((s.fragment, Some(e), s.asked, true, said))
                }
                Ask::Agreed => match crate::auditx::scan_agreed_bytes_picked(&w.eps, &bytes, crate::auditx::Facts::Bare, pick)? {
                    Scan::Basis(a) => Ok((a.fragment, None, a.asked, a.single_source, a.unanswered)),
                    Scan::NoLabel { .. } => Err(Fault::known(Known::AuditInput, crate::lang::both_of(|| crate::lang::t(Key::Tail083).to_string()))),
                },
            }
        })();
        match pass {
            Ok((fragment, e, n, single, said)) => {
                read_any = true;
                if let Some(b) = fragment.member("basis") {
                    chains.extend(arr(b, "chains"));
                }
                read.push(fragment);
                emitters.extend(e.unwrap_or_default());
                asked += n;
                single_source |= single;
                unanswered.extend(said);
            }
            Err(f) => {
                unanswered.push(f.evidence());
                first.get_or_insert(f);
                let nodes = w.eps.len();
                for (registry, name, from_block) in w.from {
                    missed.push(Missed { chain_id: w.chain_id, registry, from_block, name, reading: Reading::Down, nodes });
                }
            }
        }
    }
    if !read_any {
        let names = named(&missed);
        return Err(match first {
            Some(f) => Fault::known(f.which().unwrap_or(Known::ScanRefused), format!("{} · {}", f.tail(), names)),
            None => Fault::known(Known::ScanRefused, names),
        });
    }
    let fragment = zikaron_anchor::scan::joined(&read, Value::Obj(vec![("chains".to_string(), Value::Arr(chains))]));
    let count = arr(&fragment, "anchors").len() + zikaron_anchor::scan::candidates_in(&fragment).len();
    Ok(Wide { fragment, anchors: count, asked, single_source, unanswered, emitters, missed })
}

pub fn with_unread_windows(fragment: &Value, missed: &[Missed], senders: &[String]) -> Value {
    if missed.is_empty() {
        return fragment.clone();
    }
    let basis = fragment.member("basis").cloned().unwrap_or(Value::Null);
    let mut chains = arr(&basis, "chains");
    let read_to = |chains: &[Value], chain: u64| -> Option<u64> {
        chains
            .iter()
            .filter(|w| w.member("chainId") == Some(&Value::Int(chain)) && matches!(w.member("registries"), Some(Value::Arr(r)) if !r.is_empty()))
            .filter_map(|w| match w.member("toBlock") {
                Some(Value::Int(n)) => Some(*n),
                _ => None,
            })
            .max()
    };
    let mut unread: Vec<(u64, u64)> = Vec::new();
    for m in missed {
        match unread.iter_mut().find(|(c, _)| *c == m.chain_id) {
            Some((_, from)) => *from = (*from).min(m.from_block),
            None => unread.push((m.chain_id, m.from_block)),
        }
    }
    for (chain, from) in unread {
        let at = read_to(&chains, chain).map(|to| to.saturating_add(1)).unwrap_or(from);
        chains.push(Value::Obj(vec![
            ("chainId".to_string(), Value::Int(chain)),
            ("fromBlock".to_string(), Value::Int(at)),
            ("registries".to_string(), Value::Arr(Vec::new())),
            ("senders".to_string(), Value::Arr(senders.iter().map(|s| Value::Str(s.clone())).collect())),
            ("toBlock".to_string(), Value::Int(at)),
        ]));
    }
    let key = |w: &Value| {
        let int = |k: &str| match w.member(k) {
            Some(Value::Int(n)) => *n,
            _ => 0,
        };
        (int("chainId"), int("fromBlock"))
    };
    chains.sort_by_key(key);
    let mut out: Vec<(String, Value)> = Vec::new();
    if let Value::Obj(m) = fragment {
        for (k, v) in m {
            if k == "basis" {
                let mut b: Vec<(String, Value)> = match &basis {
                    Value::Obj(x) => x.clone(),
                    _ => Vec::new(),
                };
                for (bk, bv) in b.iter_mut() {
                    if bk == "chains" {
                        *bv = Value::Arr(chains.clone());
                    }
                }
                out.push((k.clone(), Value::Obj(b)));
            } else {
                out.push((k.clone(), v.clone()));
            }
        }
    }
    Value::Obj(out)
}

pub fn named(missed: &[Missed]) -> String {
    missed.iter().map(|m| format!("{} {}", m.name, crate::lang::t(m.key()))).collect::<Vec<_>>().join(" · ")
}

pub fn listed(at: Option<&crate::kitsindex::AnchoredOn>, main: Option<&Ground>, nets: &[Net]) -> bool {
    let Some(a) = at else { return true };
    let Some(reg) = Address::parse(&a.registry) else { return false };
    main.map(|g| g.chain == a.chain_id && g.registry == reg).unwrap_or(false) || nets.iter().any(|n| n.is(a.chain_id, &reg))
}

pub fn carrier(main: Option<&Ground>, nets: &[Net]) -> Ground {
    match (main, nets.first()) {
        (Some(g), _) => g.clone(),
        (None, Some(n)) => Ground { chain: n.chain_id, registry: n.registry, from_block: n.from_block, to_block: n.from_block, senders: Vec::new(), earlier: Vec::new() },
        (None, None) => Ground { chain: 0, registry: Address([0u8; 20]), from_block: 0, to_block: 0, senders: Vec::new(), earlier: Vec::new() },
    }
}
