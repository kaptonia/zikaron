//! The read side across networks: the main network and every read-only network (`readnets`), for the paths
//! that read someone else's material (the verify page, others' ledgers, the grant check, due diligence).
//!
//! Every network names a chain, a registry and a start block. Networks on one chain form one window: their
//! registries are the union, the start block the smallest, and the window ends at that chain's current head.
//! Each chain is read on its own, so one chain that cannot be read leaves the others standing: it is named
//! and left out, and the pass's basis is exactly the chains that were read.
//!
//! Fingerprint gate (read-only networks only): a read-only network's registry is an address the person typed,
//! often taken from what a kit's author said. A contract at that address that is not the pinned build could
//! emit "any sender anchored any hash". So before a read-only network is read, its nodes are asked for the
//! code at the registry: its keccak must equal the pinned build's ([`crate::pinned::CODE_HASH`]) on every node
//! that answers. Otherwise the network is marked "fingerprint mismatch", unused this pass, and no anchor is
//! taken from it. It is asked every time; no answer is cached. The main network and paths that read only it
//! ask nothing more: this gate admits read-only networks to the read side and changes nothing the law reads.
//!
//! Callers come here only when the read-only table holds a network. With an empty table each path asks
//! exactly what a single-network read asks, in the same order, from the same basis bytes.

use crate::auditx::{Ground, Scan};
use crate::chainx::Endpoint;
use crate::fault::{Fault, Known};
use crate::key::Address;
use crate::lang::Key;
use crate::readnets::Net;
use zikaron::json::Value;
use zikaron_anchor::scan::Emitters;

/// How a read-only network read: the four marks its row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    /// Every node that answered gave the pinned code, and more than one answered.
    Agreed(usize),
    /// One node answered, with the pinned code.
    Single,
    /// No node answered (or none answered for this chain).
    Down,
    /// A node answered with code that is not the pinned build.
    Fingerprint,
}

impl Reading {
    /// Whether the network may be read this pass.
    pub fn admits(self) -> bool {
        matches!(self, Reading::Agreed(_) | Reading::Single)
    }

    /// The reading's code as written to files (the result file's `missed`).
    pub fn code(self) -> &'static str {
        match self {
            Reading::Agreed(_) => "agreed",
            Reading::Single => "single",
            Reading::Down => "down",
            Reading::Fingerprint => "fingerprint",
        }
    }

    /// The mark's text.
    pub fn key(self) -> Key {
        match self {
            Reading::Agreed(_) => Key::BadgeAgreed,
            Reading::Single => Key::BadgeSingle,
            Reading::Down => Key::BadgeDown,
            Reading::Fingerprint => Key::BadgeFingerprint,
        }
    }
}

/// The fingerprint gate for one read-only network. Each node is asked its chain id, then the code at the
/// registry; a node that does not answer, or answers for another chain, does not count.
pub fn gate(net: &Net) -> Reading {
    gate_said(net).0
}

/// [`gate`], also returning why no node counted when none did (`Down`): the error of the first problem that
/// reached a node (classified by `chainx::said_fault`), else UNREACHABLE, with every node's message in its
/// tail. A node counts only when it serves the chain and returns the code at the registry as hex.
pub fn gate_said(net: &Net) -> (Reading, Option<Fault>) {
    gate_said_against(net, crate::pinned::CODE_HASH)
}

/// [`gate_said`] against a given code hash (`0x` and 64 lowercase hex digits): the product passes its pin;
/// tests pass the hash of a code of their own, so every form of the gate is reached without the pinned build.
pub fn gate_said_against(net: &Net, pinned: &str) -> (Reading, Option<Fault>) {
    let mut same = 0usize;
    let (mut first, mut words): (Option<Fault>, Vec<String>) = (None, Vec::new());
    for url in &net.nodes {
        let Some(mut ep) = crate::chainx::endpoint_at(url) else {
            words.push(format!("{}: not a node address", zikaron_net::sayable(url)));
            continue;
        };
        // Both questions use patience (`zikaron_anchor::patience::ask`, as every read does): a node that
        // rate-limits once is waited out, not counted as down.
        let served = match zikaron_anchor::patience::ask(ep.as_mut(), "eth_chainId", &Value::Arr(Vec::new())) {
            Ok(s) => s,
            Err(t) => {
                first.get_or_insert(crate::chainx::said_fault(url, &t));
                words.push(crate::chainx::trouble_said(url, &t));
                continue;
            }
        };
        if !zikaron_anchor::scan::endpoint_serves(&served, net.chain_id) {
            words.push(format!("{}: eth_chainId {}", zikaron_net::sayable(url), served.as_str().unwrap_or("?")));
            continue;
        }
        let params = Value::Arr(vec![Value::Str(net.registry.hex()), Value::Str("latest".to_string())]);
        let code = match zikaron_anchor::patience::ask(ep.as_mut(), "eth_getCode", &params) {
            Ok(c) => c,
            Err(t) => {
                first.get_or_insert(crate::chainx::said_fault(url, &t));
                words.push(crate::chainx::trouble_said(url, &t));
                continue;
            }
        };
        let Some(bytes) = code.as_str().and_then(zikaron::hexfmt::decode) else {
            words.push(format!("{}: eth_getCode answered no code", zikaron_net::sayable(url)));
            continue;
        };
        if zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(&bytes)) != pinned {
            return (Reading::Fingerprint, None);
        }
        same += 1;
    }
    match same {
        0 => {
            let k = first.and_then(|f| f.which()).unwrap_or(Known::Unreachable);
            (Reading::Down, Some(Fault::known(k, words.join(" · "))))
        }
        1 => (Reading::Single, None),
        n => (Reading::Agreed(n), None),
    }
}

/// A network this pass did not read, named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missed {
    pub chain_id: u64,
    pub registry: Address,
    /// The start block of the network not read.
    pub from_block: u64,
    /// Its name in the UI.
    pub name: String,
    /// `Down` or `Fingerprint`.
    pub reading: Reading,
}

/// One pass across networks.
#[derive(Clone, Debug)]
pub struct Wide {
    /// The fragment of every chain read, as one: anchors by chain, then the basis of exactly those chains.
    pub fragment: Value,
    pub anchors: usize,
    pub asked: usize,
    pub single_source: bool,
    pub unanswered: Vec<String>,
    /// Which registry each registry-form record came from.
    pub emitters: Emitters,
    /// The networks not read this pass, each named.
    pub missed: Vec<Missed>,
}

/// How each chain's window is asked: at its first node (as `auditx::scan_once`), or at every node until they
/// agree (as `auditx::scan_agreed`). Agreement is used by the grant check page, which is read-only and scans
/// without this machine's record of checked facts (`auditx::Facts::Bare`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    First,
    Agreed,
}

/// One chain's window, from every network on it.
struct Window {
    chain_id: u64,
    from_block: u64,
    registries: Vec<Address>,
    eps: Vec<Endpoint>,
    /// The networks it came from: (registry, name, start block), for naming it if it is not read.
    from: Vec<(Address, String, u64)>,
}

/// One chain's window as a basis (law §9.4's three tables), in the member order the canonical byte rules
/// require. With one registry it equals `auditx::basis_of`'s bytes.
fn basis_of(w: &Window, to_block: u64, senders: &[String]) -> Value {
    let window = Value::Obj(vec![
        ("chainId".to_string(), Value::Int(w.chain_id)),
        ("fromBlock".to_string(), Value::Int(w.from_block)),
        ("registries".to_string(), Value::Arr(w.registries.iter().map(|r| Value::Str(r.hex())).collect())),
        ("senders".to_string(), Value::Arr(senders.iter().map(|s| Value::Str(s.clone())).collect())),
        ("toBlock".to_string(), Value::Int(to_block)),
    ]);
    Value::Obj(vec![
        ("adoptionChains".to_string(), Value::Arr(Vec::new())),
        ("bareTx".to_string(), Value::Arr(Vec::new())),
        ("chains".to_string(), Value::Arr(vec![window])),
    ])
}

fn arr<'a>(v: &'a Value, k: &str) -> Vec<Value> {
    match v.member(k) {
        Some(Value::Arr(a)) => a.clone(),
        _ => Vec::new(),
    }
}

/// The windows of one pass: the main network (when configured) and every read-only network the gate admits,
/// merged per chain, by ascending chain id. Networks the gate turns away are named in `missed`.
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
                // One network per registry: a read-only row that is the main network itself joins the main's
                // entry (its earlier start block kept), so a window not read names it once.
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
    }
    for n in nets {
        // A read-only network that is the main network itself (the read-only table is per machine, the main
        // network per home, so a duplicate can remain after a home change) joins the main window when the gate
        // admits it, and is never reported as missed: the main network already reads that registry.
        let is_main = main.map(|(_, g)| n.is(g.chain, &g.registry)).unwrap_or(false);
        let reading = gate(n);
        if is_main && !reading.admits() {
            continue;
        }
        if !reading.admits() {
            missed.push(Missed { chain_id: n.chain_id, registry: n.registry, from_block: n.from_block, name: n.name(), reading });
            continue;
        }
        add(n.chain_id, n.registry, n.from_block, n.endpoints(), n.name());
    }
    for w in out.iter_mut() {
        w.registries.sort_by_key(|r| r.0);
    }
    out.sort_by_key(|w| w.chain_id);
    out
}

/// Read every chain's window and put what was read together. `senders` is the lineage the path scans for (the
/// same on every chain). A chain whose head or window cannot be read is named and left out; when no chain is
/// read, the first chain's fault is the answer, its tail naming every network not read.
pub fn scan(main: Option<(&[Endpoint], &Ground)>, nets: &[Net], senders: &[String], ask: Ask) -> Result<Wide, Fault> {
    let mut missed: Vec<Missed> = Vec::new();
    let mut anchors: Vec<Value> = Vec::new();
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
                    let (s, e) = crate::auditx::scan_first(&w.eps, &bytes)?;
                    Ok((s.fragment, Some(e), s.asked, true, Vec::new()))
                }
                Ask::Agreed => match crate::auditx::scan_agreed_bytes_with(&w.eps, &bytes, crate::auditx::Facts::Bare)? {
                    Scan::Basis(a) => Ok((a.fragment, None, a.asked, a.single_source, a.unanswered)),
                    Scan::NoLabel { .. } => Err(Fault::known(Known::AuditInput, crate::lang::t(Key::Tail083).to_string())),
                },
            }
        })();
        match pass {
            Ok((fragment, e, n, single, said)) => {
                read_any = true;
                anchors.extend(arr(&fragment, "anchors"));
                if let Some(b) = fragment.member("basis") {
                    chains.extend(arr(b, "chains"));
                }
                emitters.extend(e.unwrap_or_default());
                asked += n;
                single_source |= single;
                unanswered.extend(said);
            }
            Err(f) => {
                unanswered.push(f.evidence());
                first.get_or_insert(f);
                for (registry, name, from_block) in w.from {
                    missed.push(Missed { chain_id: w.chain_id, registry, from_block, name, reading: Reading::Down });
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
    let fragment = Value::Obj(vec![
        ("anchors".to_string(), Value::Arr(anchors)),
        (
            "basis".to_string(),
            Value::Obj(vec![
                ("adoptionChains".to_string(), Value::Arr(Vec::new())),
                ("bareTx".to_string(), Value::Arr(Vec::new())),
                ("chains".to_string(), Value::Arr(chains)),
            ]),
        ),
        ("evidence".to_string(), Value::Arr(Vec::new())),
    ]);
    let count = arr(&fragment, "anchors").len();
    Ok(Wide { fragment, anchors: count, asked, single_source, unanswered, emitters, missed })
}

/// The basis handed to a judge for a pass that left networks out: every chain not read joins as a window with
/// no registry (from and to its network's start block; the senders the path scans for), so the judge's own
/// covering rule finds the basis does not cover, and nothing reads as unanchored for want of a chain not read.
/// A chain that was read for another of its networks gets that window just past its read one (the basis
/// allows no overlap on one chain). The windows read are left as they are; with nothing missed the fragment is
/// returned unchanged.
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

/// The networks a pass left out, in words: each one's name and its mark ("OP Mainnet unreachable"), joined.
pub fn named(missed: &[Missed]) -> String {
    missed.iter().map(|m| format!("{} {}", m.name, crate::lang::t(m.reading.key()))).collect::<Vec<_>>().join(" · ")
}

/// Whether a kit's stated anchoring point is among the networks this pass would read: the main network or a
/// read-only network with that chain and registry. A kit that states none is always read.
pub fn listed(at: Option<&crate::kitsindex::AnchoredOn>, main: Option<&Ground>, nets: &[Net]) -> bool {
    let Some(a) = at else { return true };
    let Some(reg) = Address::parse(&a.registry) else { return false };
    main.map(|g| g.chain == a.chain_id && g.registry == reg).unwrap_or(false) || nets.iter().any(|n| n.is(a.chain_id, &reg))
}

/// A basis carrying the senders a path computes when the main network is not configured: the first read-only
/// network's fields. The paths read its `senders`; the grant check also shows its chain, registry and start
/// block in the basis it displays (the main network's when configured, else that first read-only network's).
pub fn carrier(main: Option<&Ground>, nets: &[Net]) -> Ground {
    match (main, nets.first()) {
        (Some(g), _) => g.clone(),
        (None, Some(n)) => Ground { chain: n.chain_id, registry: n.registry, from_block: n.from_block, to_block: n.from_block, senders: Vec::new() },
        (None, None) => Ground { chain: 0, registry: Address([0u8; 20]), from_block: 0, to_block: 0, senders: Vec::new() },
    }
}
