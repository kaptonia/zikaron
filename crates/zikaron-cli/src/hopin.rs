use crate::args::{self, Args};
use crate::codes::Reason;
use crate::out;
use zikaron::json::{self, Value};
use zikaron_glue::{book, notices};

pub struct Given {
    pub grant: Vec<u8>,
    pub input: Option<Value>,
    pub stated: Option<String>,
}

fn given_notices(a: &Args) -> Vec<Value> {
    let read: Vec<Value> = a.many("notices").iter().map(|p| json::parse_tests_1_3(&args::slurp(p)).unwrap_or(Value::Null)).collect();
    if notices::once_each(&read).is_err() {
        out::misuse(Reason::Args, out::Subject::flag("notices"), out::Said::NoticesTwice);
    }
    read
}

fn endpoints(a: &Args) -> Vec<zikaron_send::node::Endpoint> {
    let given = a.many_at("endpoint");
    if given.is_empty() {
        return Vec::new();
    }
    crate::chain::endpoint_specs(&given).iter().map(|(c, u)| zikaron_send::node::Endpoint::at(*c, zikaron_send::node::NodeAddr::new(u.as_str()))).collect()
}

fn reads_a_chain(input: &Value) -> bool {
    input.member("basis").and_then(|b| b.member("chains")).and_then(Value::as_arr).is_some_and(|c| !c.is_empty())
}

struct Wanted {
    chain: u64,
    registries: Vec<String>,
    from_block: u64,
    senders: Vec<String>,
}

fn member_window(chain: u64, grant: &[u8], root: &str, n: Option<&Value>) -> Option<Wanted> {
    let row = zikaron_anchor::deploy::ROWS.iter().find(|r| r.chain_id == chain)?;
    let mut senders = notices::lineage(grant, root, n);
    senders.extend(book::check(grant).ok().map(|e| e.author));
    senders.sort();
    senders.dedup();
    Some(Wanted { chain, registries: notices::registries_of(chain, row.registry), from_block: row.from_block, senders })
}

fn read_hops(eps: &[zikaron_send::node::Endpoint], wanted: &[Wanted], decisions: &mut [book::Decision], facts: &dyn zikaron_send::Facts, told: &mut Vec<String>) -> Result<Vec<Value>, Option<String>> {
    let mut windows: Vec<(u64, u64, &Wanted)> = Vec::new();
    for w in wanted {
        let mine: Vec<zikaron_send::node::Endpoint> = eps.iter().filter(|e| e.chain == w.chain).cloned().collect();
        if mine.is_empty() {
            let line = format!("{} {}: {}", out::Said::AnchorsUnread.text(), w.chain, zikaron_send::notices::Unread::NoNode.code());
            told.push(line.clone());
            return Err(Some(line));
        }
        match zikaron_send::node::head_block(&mine, w.chain) {
            Ok((head, _)) => windows.push((w.chain, head.max(w.from_block), w)),
            Err(t) => {
                let line = format!("{} {}: {}", out::Said::AnchorsUnread.text(), w.chain, crate::sealedsend::said(&t.why));
                told.push(line.clone());
                return Err(Some(line));
            }
        }
    }
    let ws: Vec<zikaron_glue::basis::Window> = windows.iter().map(|(chain, to, w)| zikaron_glue::basis::Window { chain: *chain, registries: &w.registries, from_block: w.from_block, to_block: *to, senders: &w.senders }).collect();
    let basis = json::canon_bytes(&zikaron_glue::basis::basis_of_all(&ws));
    let chains: Vec<u64> = wanted.iter().map(|w| w.chain).collect();
    let mine: Vec<zikaron_send::node::Endpoint> = eps.iter().filter(|e| chains.contains(&e.chain)).cloned().collect();
    loop {
        let fragment = match zikaron_send::agree::scan_picked(&mine, &basis, facts, &book::pick_of(decisions)) {
            Ok(zikaron_send::agree::Scan::Basis(got)) => got.fragment,
            Ok(zikaron_send::agree::Scan::NoLabel { .. }) => return Err(None),
            Err(t) => {
                let line = format!("{} {}: {}", out::Said::AnchorsUnread.text(), chains.first().copied().unwrap_or(0), crate::sealedsend::said(&t.why));
                told.push(line.clone());
                return Err(Some(line));
            }
        };
        let mut inputs = Vec::with_capacity(decisions.len());
        let mut more = false;
        for d in decisions.iter_mut() {
            match book::fast_input(d, &fragment) {
                Ok(book::Fast::Ready(i)) => inputs.push(i),
                Ok(book::Fast::More) => more = true,
                Err(_) => return Err(None),
            }
        }
        if !more {
            return Ok(inputs);
        }
    }
}

pub fn bare_record() -> Option<zikaron_send::checked::Record<zikaron_send::checked::Plain>> {
    if zikaron_anchor::action::keeper().is_some() {
        return None;
    }
    let m = zikaron_send::sent::machine().ok()?;
    zikaron_anchor::action::keep_in(std::sync::Arc::new(zikaron_send::checked::Logs(zikaron_send::checked::Plain(m.clone()))));
    Some(zikaron_send::checked::Record(zikaron_send::checked::Plain(m)))
}

fn root_and_pile(input: &Value) -> Option<(String, Vec<Vec<u8>>)> {
    let root = input.member("root")?.as_str()?.to_string();
    let pile = input.member("pile")?.as_arr()?.iter().map(|x| x.as_str().and_then(zikaron::hexfmt::decode)).collect::<Option<_>>()?;
    Some((root, pile))
}

pub fn hops(a: &Args, given: Vec<Given>, book_key: &dyn Fn(&[u8], Option<&Value>) -> Option<String>) -> Hops {
    let files = given_notices(a);
    let eps = endpoints(a);
    let record = if eps.is_empty() { None } else { bare_record() };
    let facts: &dyn zikaron_send::Facts = match &record {
        Some(r) => r,
        None => &zikaron_send::Bare,
    };
    let mut told: Vec<String> = Vec::new();
    let mut unread_notices: Vec<(u64, String)> = Vec::new();
    let mut read: Vec<(u64, Option<Value>)> = Vec::new();
    if !eps.is_empty() {
        for h in &given {
            let Some(chain) = notices::chain_of(&h.grant) else { continue };
            if read.iter().any(|(c, _)| *c == chain) {
                continue;
            }
            match zikaron_send::notices::read(&eps, chain) {
                Ok(r) => read.push((chain, Some(r.value))),
                Err(u) => {
                    let why = match &u {
                        zikaron_send::notices::Unread::Trouble(t) => format!("{} {}", u.code(), crate::sealedsend::said(&t.why)),
                        zikaron_send::notices::Unread::Contract(g) => format!("{} {}", u.code(), g.code()),
                        other => other.code().to_string(),
                    };
                    let line = format!("{} {chain}: {why}", out::Said::NoticesUnread.text());
                    told.push(line.clone());
                    unread_notices.push((chain, line));
                    read.push((chain, None));
                }
            }
        }
    }
    let mut out_hops: Vec<(Vec<u8>, Option<Value>, Option<Value>)> = Vec::with_capacity(given.len());
    let mut undecided: Vec<(usize, book::Undecided)> = Vec::new();
    let mut chain_gaps: Vec<Option<book::Gap>> = given.iter().map(|_| eps.is_empty().then_some(book::Gap::NoNode)).collect();
    let mut members: Vec<(Wanted, Vec<(usize, book::Decision)>)> = Vec::new();
    let mut plain: Vec<(usize, book::Decision, Vec<String>)> = Vec::new();
    for (k, h) in given.iter().enumerate() {
        let member_chain = notices::chain_of(&h.grant);
        let from_chain = member_chain.and_then(|c| read.iter().find(|(x, _)| *x == c)).map(|(_, v)| v.clone());
        let n: Option<Value> = match from_chain {
            Some(v) => v,
            None => notices::pick(&h.grant, &files).map(|i| files[i].clone()),
        };
        out_hops.push((h.grant.clone(), h.input.clone(), n.clone()));
        let unread = h.input.as_ref().is_none_or(|i| !reads_a_chain(i));
        if eps.is_empty() || !unread {
            continue;
        }
        let carried = notices::carries(&h.grant);
        let (root, pile) = match h.input.as_ref().and_then(root_and_pile) {
            Some((r, p)) => (h.stated.clone().unwrap_or(r), p),
            None if !carried => continue,
            None => (h.stated.clone().or_else(|| notices::root_for(&h.grant, n.as_ref())).unwrap_or_default(), Vec::new()),
        };
        let window = match member_chain {
            Some(chain) => match member_window(chain, &h.grant, &root, n.as_ref()) {
                Some(w) => Some(w),
                None => continue,
            },
            None if carried => continue,
            None => None,
        };
        let d = match book::decision_set(&book::Hold { grant: &h.grant, root: &root, pile: &pile }) {
            Ok(d) => d,
            Err(u) => {
                undecided.push((k, u));
                out_hops[k].1 = None;
                continue;
            }
        };
        match window {
            Some(w) => match members.iter_mut().find(|(x, _)| x.chain == w.chain && x.registries == w.registries && x.senders == w.senders) {
                Some((_, ds)) => ds.push((k, d)),
                None => members.push((w, vec![(k, d)])),
            },
            None => {
                let mut senders = zikaron_glue::basis::senders_of(&pile);
                senders.extend(book::check(&h.grant).ok().map(|e| e.author));
                senders.push(root.to_ascii_lowercase());
                plain.push((k, d, senders));
            }
        }
    }
    for (w, ds) in members {
        let (ks, mut decisions): (Vec<usize>, Vec<book::Decision>) = ds.into_iter().unzip();
        match read_hops(&eps, std::slice::from_ref(&w), &mut decisions, facts, &mut told) {
            Ok(inputs) => {
                for (k, i) in ks.into_iter().zip(inputs) {
                    out_hops[k].1 = Some(i);
                }
            }
            Err(said) => {
                for k in ks {
                    chain_gaps[k] = said.clone().map(book::Gap::ChainUnread);
                }
            }
        }
    }
    if !plain.is_empty() {
        let mut senders: Vec<String> = plain.iter().flat_map(|(_, _, s)| s.iter().map(|x| x.to_ascii_lowercase())).collect();
        senders.sort();
        senders.dedup();
        let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).filter(|c| zikaron_anchor::deploy::ROWS.iter().any(|r| r.chain_id == *c)).collect();
        chains.sort_unstable();
        chains.dedup();
        let wanted: Vec<Wanted> = chains
            .iter()
            .filter_map(|c| zikaron_anchor::deploy::ROWS.iter().find(|r| r.chain_id == *c).map(|row| Wanted { chain: *c, registries: notices::registries_of(*c, row.registry), from_block: row.from_block, senders: senders.clone() }))
            .collect();
        if !wanted.is_empty() {
            let (ks, mut decisions): (Vec<usize>, Vec<book::Decision>) = plain.into_iter().map(|(k, d, _)| (k, d)).unzip();
            match read_hops(&eps, &wanted, &mut decisions, facts, &mut told) {
                Ok(inputs) => {
                    for (k, i) in ks.into_iter().zip(inputs) {
                        out_hops[k].1 = Some(i);
                    }
                }
                Err(said) => {
                    for k in ks {
                        chain_gaps[k] = said.clone().map(book::Gap::ChainUnread);
                    }
                }
            }
        }
    }
    for (grant, input, _) in out_hops.iter_mut() {
        let stands = input.as_ref().and_then(|i| i.member("pile")).and_then(Value::as_arr).map(|p| p.iter().filter_map(|x| x.as_str().and_then(zikaron::hexfmt::decode)).collect::<Vec<_>>());
        if let Some(copy) = stands {
            if !notices::copy_stands(grant, &copy, book_key(grant, input.as_ref()).as_deref()) {
                *input = None;
            }
        }
    }
    let time_chain = given.iter().filter_map(|h| notices::chain_of(&h.grant)).find(|c| eps.iter().any(|e| e.chain == *c)).or_else(|| eps.iter().map(|e| e.chain).find(|c| zikaron_anchor::deploy::ROWS.iter().any(|r| r.chain_id == *c)));
    let notices_said: Vec<Option<String>> = given.iter().map(|h| notices::chain_of(&h.grant).and_then(|c| unread_notices.iter().find(|(x, _)| *x == c)).map(|(_, w)| w.clone())).collect();
    Hops { hops: out_hops, told, undecided, chain_gaps, notices_said, time: Time { given: a.now(), eps, chain: time_chain, read: std::cell::OnceCell::new() } }
}

pub fn gaps(h: &Hops, result: &Value) -> Option<Value> {
    let single = result.member("checks").is_some();
    let results: Vec<&Value> = if single { vec![result] } else { result.member("hops").and_then(Value::as_arr).map(|v| v.iter().collect()).unwrap_or_default() };
    let mut out = Vec::new();
    for (k, r) in results.into_iter().enumerate() {
        let ledger = match h.undecided.iter().find(|(i, _)| *i == k) {
            Some((_, u)) => book::LedgerRead::Undecided(*u),
            None if h.hops.get(k).is_some_and(|x| x.1.is_some()) => book::LedgerRead::Read { behind: false },
            None => book::LedgerRead::None,
        };
        for c in r.member("checks").and_then(Value::as_arr).map_or(&[][..], |v| v.as_slice()) {
            let (Some(n), Some(token), Some(state)) = (c.member("n").and_then(Value::as_int), c.member("token").and_then(Value::as_str), c.member("state").and_then(Value::as_str)) else { continue };
            let said = h.notices_said.get(k).and_then(|x| x.as_deref());
            let gap = book::notices_gap_of(said, token, state).or_else(|| book::gap_of(token, state, &ledger, h.chain_gaps.get(k).cloned().flatten(), None, h.now().is_none()));
            let Some(gap) = gap else { continue };
            let (zh, en) = match (&gap, gap.words()) {
                (_, Some((zh, en))) => (zh.to_string(), en.to_string()),
                (book::Gap::NoticesUnread(w), None) => (w.clone(), w.clone()),
                _ => continue,
            };
            let mut m = Vec::new();
            if !single {
                m.push(("hop".to_string(), Value::Int(k as u64)));
            }
            m.extend([("en".to_string(), Value::Str(en)), ("gap".to_string(), Value::Str(gap.code().to_string())), ("n".to_string(), Value::Int(n)), ("token".to_string(), Value::Str(token.to_string())), ("zh".to_string(), Value::Str(zh))]);
            m.sort_by(|x, y| x.0.cmp(&y.0));
            out.push(Value::Obj(m));
        }
    }
    (!out.is_empty()).then_some(Value::Arr(out))
}

pub struct Hops {
    pub hops: Vec<(Vec<u8>, Option<Value>, Option<Value>)>,
    pub told: Vec<String>,
    pub undecided: Vec<(usize, book::Undecided)>,
    pub chain_gaps: Vec<Option<book::Gap>>,
    pub notices_said: Vec<Option<String>>,
    time: Time,
}

struct Time {
    given: Option<u64>,
    eps: Vec<zikaron_send::node::Endpoint>,
    chain: Option<u64>,
    read: std::cell::OnceCell<Option<u64>>,
}

impl Hops {
    pub fn refused(&self, chain: bool) -> Option<out::Answer> {
        if self.undecided.is_empty() {
            return None;
        }
        let shape = chain.then(|| book::chain_check(&self.hops, None));
        unchecked(&self.undecided, chain, shape.as_ref())
    }

    pub fn now(&self) -> Option<u64> {
        *self.time.read.get_or_init(|| zikaron_send::notices::now_of(self.time.given, &self.time.eps, self.time.chain).0)
    }
}

fn unchecked(undecided: &[(usize, book::Undecided)], chain: bool, checked: Option<&Value>) -> Option<out::Answer> {
    let (k, u) = undecided.first()?;
    if chain {
        let kind = checked.and_then(|v| v.member("failing")).and_then(|f| f.member("kind")).and_then(Value::as_str);
        use zikaron_kit::tokens::FailKind;
        if kind.is_some_and(|k| k == FailKind::Incomplete.as_str() || k == FailKind::Link.as_str()) {
            return None;
        }
    }
    let detail = if chain { format!("hop {k} {}", u.code()) } else { u.code().to_string() };
    Some(out::unanswered(Reason::Unchecked, vec![(crate::codes::Key::Detail, out::s(&detail))]))
}

pub fn telling(answer: out::Answer, told: Vec<String>) -> out::Answer {
    if told.is_empty() {
        answer
    } else {
        answer.telling(told.join("\n"))
    }
}
