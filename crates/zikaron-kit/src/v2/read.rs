use super::doc::{self, Accepted};
use super::tokens::{self as t2, Key};
use crate::tokens::{ChainToken, Check, CheckVerdict, FailKind, Reason, State};
use std::collections::{BTreeSet, HashMap, HashSet};
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron::tokens::{FindingName, Verdict};
use zikaron::trace;
use zikaron::v2::audit::{self as a2, AnchorRec};
use zikaron::v2::entry as e2;
use zikaron::v2::tokens::{EntryType as V2Type, Label};

fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub seq: u64,
    pub prev: Option<String>,
    pub entry_type: String,
    pub body: Value,
}

pub struct View {
    pub root: String,
    pub basis: Value,
    pub label: Label,
    pub ledger: Vec<Row>,
    by_id: HashMap<String, usize>,
    pub lineage: BTreeSet<String>,
    findings: HashMap<String, BTreeSet<String>>,
    retired: HashSet<String>,
    pub continued: bool,
    bounds: HashMap<String, u64>,
    unproven_reach: HashSet<String>,
    counted: Vec<AnchorRec>,
    cont: HashSet<String>,
    anchors: Vec<AnchorRec>,
    spine: bool,
}

fn reach(from: usize, ledger: &[Row], by_id: &HashMap<String, usize>, cont: &HashSet<String>) -> HashSet<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack = vec![from];
    while let Some(i) = stack.pop() {
        let cur = &ledger[i];
        if !seen.insert(cur.id.clone()) {
            continue;
        }
        if let Some(n) = cur.prev.as_ref().and_then(|p| by_id.get(p)) {
            stack.push(*n);
        }
        if cont.contains(&cur.id) {
            if let Some(n) = cur.body.member("continues").and_then(|c| c.member("entry_id")).and_then(Value::as_str).and_then(|x| by_id.get(x)) {
                stack.push(*n);
            }
        }
    }
    seen
}

fn finding_ids(report: &Value, out: &mut HashMap<String, BTreeSet<String>>) {
    for f in report.member("findings").and_then(Value::as_arr).into_iter().flatten() {
        if let (Some(id), Some(name)) = (f.member("entry_id").and_then(Value::as_str), f.member("name").and_then(Value::as_str)) {
            out.entry(id.to_string()).or_default().insert(name.to_string());
        }
    }
}

fn v1_ledger_ids(report: &Value) -> HashSet<String> {
    let mut ids: HashSet<String> = HashSet::new();
    for r in report.member("anchored").and_then(Value::as_arr).into_iter().flatten() {
        if let Some(id) = r.member("entry_id").and_then(Value::as_str) {
            ids.insert(id.to_string());
        }
    }
    for x in report.member("unanchored").and_then(Value::as_arr).into_iter().flatten() {
        if let Some(id) = x.as_str() {
            ids.insert(id.to_string());
        }
    }
    ids
}

pub fn load_view(input: &Value, extra: Option<&[u8]>) -> Option<View> {
    let mut input = input.clone();
    if let Some(g) = extra {
        let Value::Obj(ms) = &mut input else { return None };
        let pile = ms.iter_mut().find(|(k, _)| k == "pile").map(|(_, v)| v)?;
        let Value::Arr(items) = pile else { return None };
        items.push(Value::Str(zikaron::hexfmt::encode(g)));
    }
    let res = a2::audit_full(&input)?;
    let received = a2::read_inputs(&input)?;
    let mut view = view_of(&res);
    view.spine = res.new.ledger.iter().any(|e| e.kind == V2Type::Genesis && e.author == res.new.seed)
        || received.pile.iter().any(|b| {
            view.retired.contains(&hexfmt::encode(&zikaron::entry::entry_id(b)))
                && e2::check(b).is_ok_and(|e| e.kind == V2Type::Genesis && e.author == res.new.seed)
        });
    view.anchors = received.anchors;
    Some(view)
}

pub fn view_of(res: &a2::Outcome) -> View {
    let mut ledger: Vec<Row> = res
        .new
        .ledger
        .iter()
        .map(|e| Row { id: e.id_hex.clone(), seq: e.seq, prev: e.prev.clone(), entry_type: e.entry_type.clone(), body: e.body.clone() })
        .collect();
    let mut counted: Vec<&AnchorRec> = res.new.kept.iter().filter(|a| a.verdict == Verdict::Counted).collect();
    let mut unproven: Vec<&AnchorRec> = res.new.kept.iter().filter(|a| a.verdict == Verdict::Unproven).collect();
    let mut lineage = res.new.lineage.clone();
    let mut findings: HashMap<String, BTreeSet<String>> = HashMap::new();
    finding_ids(&res.report, &mut findings);
    let mut cont: HashSet<String> = HashSet::new();
    if let Some(c) = &res.continuation {
        let old_ids = v1_ledger_ids(&c.old.report);
        ledger.extend(c.old.entries.iter().filter(|e| old_ids.contains(&e.id_hex())).map(|e| Row {
            id: e.id_hex(),
            seq: e.seq,
            prev: e.prev.clone(),
            entry_type: e.entry_type.clone(),
            body: e.body.clone(),
        }));
        for a in &c.old.records {
            if c.old.lineage.contains(&a.sender) {
                match a.verdict {
                    Verdict::Counted => counted.push(a),
                    Verdict::Unproven => unproven.push(a),
                    Verdict::Void => {}
                }
            }
        }
        lineage.extend(c.old.lineage.iter().cloned());
        finding_ids(&c.old.report, &mut findings);
        let sid = c.succession.id_hex();
        cont = res
            .new
            .ledger
            .iter()
            .filter(|e| {
                e.seq == 0
                    && e.kind == V2Type::Genesis
                    && e.body.member("continues").filter(|x| x.is_obj()).and_then(|x| x.member("entry_id")).and_then(Value::as_str)
                        == Some(sid.as_str())
            })
            .map(|e| e.id_hex.clone())
            .collect();
    }
    let by_id: HashMap<String, usize> = ledger.iter().enumerate().map(|(i, r)| (r.id.clone(), i)).collect();
    let retired: HashSet<String> = res
        .report
        .member("retired")
        .and_then(Value::as_arr)
        .into_iter()
        .flatten()
        .filter_map(|r| r.member("entry_id").and_then(Value::as_str).map(str::to_string))
        .collect();

    let mut bounds: HashMap<String, u64> = HashMap::new();
    let mut cache: HashMap<&str, HashSet<String>> = HashMap::new();
    for a in &counted {
        let Some(&f) = by_id.get(&a.hash) else { continue };
        let r = cache.entry(a.hash.as_str()).or_insert_with(|| reach(f, &ledger, &by_id, &cont));
        for id in r.iter() {
            let b = bounds.entry(id.clone()).or_insert(a.block_timestamp);
            *b = (*b).min(a.block_timestamp);
        }
    }
    let mut unproven_reach: HashSet<String> = HashSet::new();
    for a in &unproven {
        if let Some(&f) = by_id.get(&a.hash) {
            unproven_reach.extend(reach(f, &ledger, &by_id, &cont));
        }
    }
    let counted = counted.into_iter().cloned().collect();
    View {
        root: res.root.clone(),
        basis: res.basis.clone(),
        label: res.label,
        ledger,
        by_id,
        lineage,
        findings,
        retired,
        continued: res.continuation.is_some(),
        bounds,
        unproven_reach,
        counted,
        cont,
        anchors: Vec::new(),
        spine: false,
    }
}

impl View {
    pub fn bound(&self, id: &str) -> Option<u64> {
        self.bounds.get(id).copied()
    }

    pub fn first_anchor(&self, id: &str) -> Option<&AnchorRec> {
        let t = self.bound(id)?;
        self.counted
            .iter()
            .filter(|a| a.block_timestamp == t)
            .filter(|a| {
                self.by_id
                    .get(&a.hash)
                    .is_some_and(|&f| reach(f, &self.ledger, &self.by_id, &self.cont).contains(id))
            })
            .min_by(|x, y| (x.chain_id, x.block_number, x.tx.as_bytes()).cmp(&(y.chain_id, y.block_number, y.tx.as_bytes())))
    }


    fn has(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    fn soft_authority(&self, id: &str) -> bool {
        self.findings.get(id).is_some_and(|n| n.contains(FindingName::AuthorityMismatch.as_str()))
    }

    fn covering(&self) -> bool {
        let Some(chains) = self.basis.member("chains").and_then(Value::as_arr) else { return false };
        !chains.is_empty()
            && chains.iter().all(|c| {
                let regs = c.member("registries").and_then(Value::as_arr);
                let senders: HashSet<&str> =
                    c.member("senders").and_then(Value::as_arr).into_iter().flatten().filter_map(Value::as_str).collect();
                regs.is_some_and(|r| !r.is_empty()) && self.lineage.iter().all(|k| senders.contains(k.as_str()))
            })
    }
}

pub type Position = (u64, u64, u64);

#[derive(Clone, Debug)]
struct NoticeRec {
    sender: String,
    grant: String,
    verdict: Verdict,
}

#[derive(Clone, Debug)]
struct BookRec {
    to: String,
    root: String,
}

#[derive(Clone, Debug)]
pub struct Notices {
    pub basis: Value,
    chain_id: u64,
    to_block: u64,
    contract: String,
    revocations: Vec<(Position, NoticeRec)>,
    accessions: Vec<(Position, BookRec)>,
}

fn exactly<'a>(v: &'a Value, keys: &[Key]) -> Option<&'a Value> {
    let Value::Obj(ms) = v else { return None };
    (ms.len() == keys.len() && keys.iter().all(|k| ms.iter().any(|(mk, _)| mk == k.as_str()))).then_some(v)
}

fn int_of(v: &Value, k: Key) -> Option<u64> {
    v.member(k.as_str()).and_then(Value::as_int)
}

fn hex_of(v: &Value, k: Key, test: fn(&str) -> bool) -> Option<String> {
    v.member(k.as_str()).and_then(Value::as_str).filter(|x| test(x)).map(str::to_string)
}

fn records<R>(arr: Option<&Value>, keys: &[Key], chain: u64, lo: u64, hi: u64, read: impl Fn(&Value) -> Option<R>) -> Option<Vec<(Position, R)>> {
    let mut out: Vec<(Position, R)> = Vec::new();
    let mut seen: HashSet<Position> = HashSet::new();
    for r in arr?.as_arr()? {
        let r = exactly(r, keys)?;
        let block = int_of(r, Key::BlockNumber)?;
        if int_of(r, Key::ChainId)? != chain || block < lo || block > hi {
            return None;
        }
        let at = (block, int_of(r, Key::TxIndex)?, int_of(r, Key::LogIndex)?);
        if !seen.insert(at) {
            return None;
        }
        out.push((at, read(r)?));
    }
    out.sort_by(|x, y| x.0.cmp(&y.0));
    Some(out)
}

pub fn load_notices(v: &Value) -> Option<Notices> {
    let v = exactly(v, &[Key::Basis, Key::Revocations, Key::Accessions])?;
    let b = exactly(
        v.member(Key::Basis.as_str())?,
        &[Key::ChainId, Key::FromBlock, Key::ToBlock, Key::RevocationContract, Key::Book, Key::BookFromBlock],
    )?;
    let chain_id = int_of(b, Key::ChainId)?;
    let from = int_of(b, Key::FromBlock)?;
    let to_block = int_of(b, Key::ToBlock)?;
    let book_from = int_of(b, Key::BookFromBlock)?;
    let contract = hex_of(b, Key::RevocationContract, hexfmt::is_hex20)?;
    hex_of(b, Key::Book, hexfmt::is_hex20)?;
    if from > to_block || book_from > to_block {
        return None;
    }
    let revocations = records(
        v.member(Key::Revocations.as_str()),
        &[Key::ChainId, Key::BlockNumber, Key::TxIndex, Key::LogIndex, Key::Tx, Key::Sender, Key::Grant, Key::Verdict],
        chain_id,
        from,
        to_block,
        |r| {
            hex_of(r, Key::Tx, hexfmt::is_hex32)?;
            Some(NoticeRec {
                sender: hex_of(r, Key::Sender, hexfmt::is_hex20)?,
                grant: hex_of(r, Key::Grant, hexfmt::is_hex32)?,
                verdict: Verdict::parse(r.member(Key::Verdict.as_str())?.as_str()?)?,
            })
        },
    )?;
    let accessions = records(
        v.member(Key::Accessions.as_str()),
        &[Key::ChainId, Key::BlockNumber, Key::TxIndex, Key::LogIndex, Key::Holder, Key::To, Key::Root],
        chain_id,
        book_from,
        to_block,
        |r| {
            hex_of(r, Key::Holder, hexfmt::is_hex20)?;
            Some(BookRec { to: hex_of(r, Key::To, hexfmt::is_hex20)?, root: hex_of(r, Key::Root, hexfmt::is_hex20)? })
        },
    )?;
    Some(Notices { basis: b.clone(), chain_id, to_block, contract, revocations, accessions })
}

impl Notices {
    pub fn lineage(&self, root: &str) -> Vec<String> {
        std::iter::once(root.to_string()).chain(self.accessions.iter().filter(|(_, r)| r.root == root).map(|(_, r)| r.to.clone())).collect()
    }

    fn in_lineage(&self, root: &str, key: &str) -> bool {
        key == root || self.accessions.iter().any(|(_, r)| r.root == root && r.to == key)
    }

    fn in_office_at(&self, root: &str, p: Position) -> String {
        self.accessions.iter().take_while(|(at, _)| *at < p).filter(|(_, r)| r.root == root).last().map(|(_, r)| r.to.clone()).unwrap_or_else(|| root.to_string())
    }

    fn in_office_at_block(&self, root: &str, b: u64) -> String {
        self.accessions.iter().take_while(|(at, _)| at.0 < b).filter(|(_, r)| r.root == root).last().map(|(_, r)| r.to.clone()).unwrap_or_else(|| root.to_string())
    }
}

pub struct Member {
    pub carried: bool,
    pub well_formed: Option<(u64, String)>,
}

pub fn member_of(g: &Accepted) -> Member {
    let Some(m) = g.body().member(Key::Revocation.as_str()) else { return Member { carried: false, well_formed: None } };
    let well_formed = exactly(m, &[Key::Chain, Key::At]).and_then(|m| Some((int_of(m, Key::Chain)?, hex_of(m, Key::At, hexfmt::is_hex20)?)));
    Member { carried: true, well_formed }
}

impl Member {
    pub fn applies(&self, n: Option<&Notices>) -> bool {
        matches!((&self.well_formed, n), (Some((chain, at)), Some(n)) if n.chain_id == *chain && n.contract == *at)
    }
}

fn author_of(g: &Accepted) -> &str {
    match g {
        Accepted::V1(e) => &e.author,
        Accepted::V2(e) => &e.author,
    }
}

pub fn derived_root(g: &Accepted, member: &Member, view: Option<&View>, n: Option<&Notices>) -> String {
    if member.applies(n) {
        if let Some((_, r)) = n.and_then(|n| n.accessions.iter().find(|(_, r)| r.to == author_of(g))) {
            return r.root.clone();
        }
    }
    if let Some(v) = view {
        return v.root.clone();
    }
    match g.body().member("ledger").and_then(Value::as_str) {
        Some(l) if hexfmt::is_hex20(l) => l.to_string(),
        _ => author_of(g).to_string(),
    }
}

pub struct Checked {
    pub result: Value,
    pub entry: Option<Accepted>,
}

fn member_checks(e: &Accepted, view: Option<&View>, n: Option<&Notices>, states: &mut [State; 6]) {
    let id = e.id_hex();
    let member = member_of(e);
    let applies = member.applies(n);
    let root = derived_root(e, &member, view, n);
    states[1] = match view {
        None => State::Unknown,
        Some(v) if v.label == Label::BrokenChain => State::Fail,
        Some(v) if !v.spine => State::Unknown,
        Some(_) => State::Pass,
    };
    match view {
        Some(v) if states[1] != State::Fail => {
            let standing: Vec<&AnchorRec> = match (applies, n, &member.well_formed) {
                (true, Some(n), Some((chain, _))) => v
                    .anchors
                    .iter()
                    .filter(|a| a.chain_id == *chain && a.hash == id && a.verdict == Verdict::Counted && a.block_number <= n.to_block)
                    .filter(|a| a.sender == n.in_office_at_block(&root, a.block_number))
                    .collect(),
                _ => Vec::new(),
            };
            let table_fails = !v.has(&id) && !v.retired.contains(&id) && !(e.is_v1() && !v.continued);
            let seated = applies && n.is_some_and(|n| n.in_lineage(&root, author_of(e)));
            states[2] = if states[1] == State::Pass && table_fails && !seated {
                State::Fail
            } else if !applies {
                State::Unknown
            } else if v.root != root {
                State::Fail
            } else if standing.iter().any(|a| a.sender == author_of(e)) {
                State::Pass
            } else if !standing.is_empty() {
                State::Fail
            } else {
                State::Unknown
            };
            states[3] = if applies && !standing.is_empty() { State::Pass } else { State::Unknown };
        }
        _ => {
            states[2] = State::Unknown;
            states[3] = State::Unknown;
        }
    }
    states[5] = notice_check(&id, &root, n.filter(|_| applies)).0;
}

fn notice_check(id: &str, root: &str, n: Option<&Notices>) -> (State, Option<Position>) {
    let Some(n) = n else { return (State::Unknown, None) };
    let mut hit = State::Pass;
    for (at, r) in n.revocations.iter().filter(|(_, r)| r.grant == id) {
        if r.sender != n.in_office_at(root, *at) {
            continue;
        }
        if r.verdict == Verdict::Counted {
            return (State::Fail, Some(*at));
        }
        hit = State::Unknown;
    }
    (hit, None)
}

pub fn withdrawing_notice(g: &[u8], view: Option<&View>, notices: Option<&Notices>) -> Option<Position> {
    let e = doc::accept(g).ok().filter(Accepted::is_grant)?;
    let member = member_of(&e);
    if !member.carried || !member.applies(notices) {
        return None;
    }
    let root = derived_root(&e, &member, view, notices);
    notice_check(&e.id_hex(), &root, notices).1
}

pub fn grant_check(g: &[u8], view: Option<&View>, notices: Option<&Notices>, now: Option<u64>) -> Checked {
    trace::mark(t2::K4);
    let mut states = [State::Unknown; 6];
    let mut reasons: [Option<String>; 6] = Default::default();
    let mut entry: Option<Accepted> = None;
    match doc::accept(g) {
        Err(tok) => {
            states[0] = State::Fail;
            reasons[0] = Some(tok.as_str().to_string());
        }
        Ok(e) if !e.is_grant() => {
            states[0] = State::Fail;
            reasons[0] = Some(Reason::NotAGrant.as_str().to_string());
        }
        Ok(e) => {
            states[0] = State::Pass;
            entry = Some(e);
        }
    }
    if let Some(e) = entry.as_ref().filter(|e| member_of(e).carried) {
        member_checks(e, view, notices, &mut states);
    } else if let Some(e) = &entry {
        let id = e.id_hex();
        states[1] = match view {
            None => State::Unknown,
            Some(v) if v.label == Label::BrokenChain => State::Fail,
            Some(_) => State::Pass,
        };
        let c2_fail = states[1] == State::Fail;
        let mut gate46 = false;
        states[2] = match view {
            None => State::Unknown,
            Some(_) if c2_fail => State::Unknown,
            Some(v) if e.is_v1() && !v.continued => {
                gate46 = true;
                State::Unknown
            }
            Some(v) if !v.has(&id) => {
                gate46 = true;
                if v.retired.contains(&id) && v.label != Label::Complete { State::Unknown } else { State::Fail }
            }
            Some(v) if v.soft_authority(&id) => State::Unknown,
            Some(_) => State::Pass,
        };
        states[3] = match view {
            None => State::Unknown,
            Some(_) if c2_fail || gate46 => State::Unknown,
            Some(v) if v.bounds.contains_key(&id) => State::Pass,
            Some(v) if !v.covering() => State::Unknown,
            Some(v) if v.unproven_reach.contains(&id) => State::Unknown,
            Some(v) if v.label == Label::Complete => State::Fail,
            Some(_) => State::Unknown,
        };
        states[5] = match view {
            None => State::Unknown,
            Some(_) if c2_fail || gate46 => State::Unknown,
            Some(v) => {
                let revoked = v.ledger.iter().any(|x| {
                    x.entry_type == "revocation"
                        && x.body.member("grant").and_then(Value::as_str) == Some(id.as_str())
                        && !v.soft_authority(&x.id)
                });
                if revoked {
                    State::Fail
                } else if v.label == Label::Complete {
                    State::Pass
                } else {
                    State::Unknown
                }
            }
        };
    }
    if let Some(e) = &entry {
        states[4] = match e.body().member("window") {
            None => State::Pass,
            Some(_) if now.is_none() => State::Unknown,
            Some(w) => {
                let n = now.unwrap_or_default();
                let from = w.member("from").and_then(Value::as_int).unwrap_or_default();
                let to = w.member("to").and_then(Value::as_int).unwrap_or_default();
                if n < from || n > to { State::Fail } else { State::Pass }
            }
        };
    }
    let verdict = if states.iter().all(|x| *x == State::Pass) {
        CheckVerdict::Green
    } else if states.contains(&State::Fail) {
        CheckVerdict::Fail
    } else {
        CheckVerdict::Partial
    };
    let checks: Vec<Value> = Check::ALL
        .iter()
        .enumerate()
        .map(|(i, c)| {
            obj(vec![
                (Key::N, Value::Int(i as u64 + 1)),
                (Key::Token, s(c.as_str())),
                (Key::State, s(states[i].as_str())),
                (Key::Reason, reasons[i].as_deref().map(s).unwrap_or(Value::Null)),
            ])
        })
        .collect();
    let failed: Vec<Value> = Check::ALL.iter().enumerate().filter(|(i, _)| states[*i] == State::Fail).map(|(_, c)| s(c.as_str())).collect();
    let result = obj(vec![
        (Key::Verdict, s(verdict.as_str())),
        (Key::Basis, view.map(|v| v.basis.clone()).unwrap_or(Value::Null)),
        (Key::Notices, notices.map(|n| n.basis.clone()).unwrap_or(Value::Null)),
        (Key::Checks, Value::Arr(checks)),
        (Key::Failed, Value::Arr(failed)),
    ]);
    Checked { result, entry }
}

pub fn ledger_link(u: &Accepted, d: &Accepted, view_d: Option<&View>, notices_d: Option<&Notices>) -> Option<bool> {
    if !doc::byte_link(u, d) {
        return Some(false);
    }
    let member = member_of(d);
    if view_d.is_none() && !(member.carried && member.applies(notices_d)) {
        return None;
    }
    Some(u.body().member("grantee").and_then(Value::as_str) == Some(derived_root(d, &member, view_d, notices_d).as_str()))
}

pub type Hop = (Vec<u8>, Option<View>, Option<Notices>);

pub fn chain_check(hops: &[Hop], now: Option<u64>) -> Value {
    trace::mark(t2::K4);
    let failing_value = |kind: FailKind, index: usize| obj(vec![(Key::Kind, s(kind.as_str())), (Key::Index, Value::Int(index as u64))]);
    if hops.is_empty() {
        return obj(vec![
            (Key::Verdict, s(CheckVerdict::Fail.as_str())),
            (Key::Hops, Value::Arr(vec![])),
            (Key::Links, Value::Arr(vec![])),
            (Key::Token, s(ChainToken::Empty.as_str())),
            (Key::Failing, failing_value(FailKind::Empty, 0)),
        ]);
    }
    let checked: Vec<Checked> = hops.iter().map(|(g, v, n)| grant_check(g, v.as_ref(), n.as_ref(), now)).collect();
    let links: Vec<Option<bool>> = (1..hops.len())
        .map(|k| match (&checked[k - 1].entry, &checked[k].entry) {
            (Some(u), Some(d)) => ledger_link(u, d, hops[k].1.as_ref(), hops[k].2.as_ref()),
            _ => None,
        })
        .collect();
    let verdict_of = |c: &Checked| c.result.member("verdict").and_then(Value::as_str).map(str::to_string);
    let mut failing: Option<(FailKind, usize, Option<ChainToken>)> = None;
    if checked[0].entry.as_ref().is_some_and(|e| e.body().member("upstream").is_some()) {
        failing = Some((FailKind::Incomplete, 0, Some(ChainToken::Incomplete)));
    } else if let Some(k) = links.iter().position(|l| *l == Some(false)) {
        failing = Some((FailKind::Link, k + 1, Some(ChainToken::Link)));
    } else if let Some(k) = checked.iter().position(|c| verdict_of(c).as_deref() == Some(CheckVerdict::Fail.as_str())) {
        failing = Some((FailKind::Hop, k, None));
    }
    let verdict = if failing.is_some() {
        CheckVerdict::Fail
    } else if checked.iter().all(|c| verdict_of(c).as_deref() == Some(CheckVerdict::Green.as_str())) && links.iter().all(|l| *l == Some(true)) {
        CheckVerdict::Green
    } else {
        CheckVerdict::Partial
    };
    obj(vec![
        (Key::Verdict, s(verdict.as_str())),
        (Key::Hops, Value::Arr(checked.iter().map(|c| c.result.clone()).collect())),
        (Key::Links, Value::Arr(links.iter().map(|l| l.map(Value::Bool).unwrap_or(Value::Null)).collect())),
        (Key::Token, failing.and_then(|f| f.2).map(|t| s(t.as_str())).unwrap_or(Value::Null)),
        (Key::Failing, failing.map(|f| failing_value(f.0, f.1)).unwrap_or(Value::Null)),
    ])
}
