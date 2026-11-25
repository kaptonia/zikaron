use crate::key::Address;
use zikaron::json::Value;
use zikaron_glue::book;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    SameWords,
    NewWords,
    Drawn,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::SameWords => "same-words",
            Source::NewWords => "new-words",
            Source::Drawn => "drawn",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Book {
    Off,
    Wrong,
    Read { pending: [u8; 20], current: [u8; 20], root: [u8; 20], own: [u8; 20] },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lack {
    NoRow,
    WrongCode,
}

impl Lack {
    pub fn as_str(self) -> &'static str {
        match self {
            Lack::NoRow => "no-row",
            Lack::WrongCode => "wrong-code",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chain {
    pub chain: u64,
    pub book: Book,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Behind,
    Owed { n: usize },
    Bookless { chains: Vec<(u64, Lack)> },
    Ready,
    Unread { chain: u64, written: bool },
    Declare { chain: u64 },
    Write,
    Anchoring { id: String },
    Accede { chain: u64 },
    Sweep { chain: u64, send: Option<u128>, then: Option<Box<Stage>> },
    Done,
    Unseated { n: Option<usize> },
    Unentered { chains: Vec<u64> },
    Declared { chain: u64, to: [u8; 20] },
    TakenUp { chain: u64, by: [u8; 20] },
}

pub fn standing(chains: &[Chain], basis: &[u64], root: &[u8; 20], office: &[u8; 20], before: &[u8; 20]) -> Option<Stage> {
    let read = || chains.iter().filter(|c| basis.contains(&c.chain));
    let taken = read().find_map(|c| match c.book {
        Book::Read { pending, current, root: of_root, .. } => match enrolment(of_root, current, pending, false, root, office) {
            Err(Closed::OtherKey(k)) if k != *before => Some(Stage::TakenUp { chain: c.chain, by: k }),
            _ => None,
        },
        _ => None,
    });
    taken.or_else(|| {
        read().find_map(|c| match c.book {
            Book::Read { own, .. } if declares(&own) => Some(Stage::Declared { chain: c.chain, to: own }),
            _ => None,
        })
    })
}

pub const WITHDRAWN: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xde, 0xad];

pub fn declares(to: &[u8; 20]) -> bool {
    *to != [0u8; 20] && *to != WITHDRAWN
}

impl Stage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Stage::Behind => "behind",
            Stage::Owed { .. } => "owed",
            Stage::Bookless { .. } => "bookless",
            Stage::Unread { .. } => "unread",
            Stage::Ready => "ready",
            Stage::Declare { .. } => "declare",
            Stage::Write => "write",
            Stage::Anchoring { .. } => "anchoring",
            Stage::Accede { .. } => "accede",
            Stage::Sweep { .. } => "sweep",
            Stage::Done => "done",
            Stage::Unseated { .. } => "unseated",
            Stage::Unentered { .. } => "unentered",
            Stage::Declared { .. } => "declared",
            Stage::TakenUp { .. } => "takenUp",
        }
    }

    pub fn step(&self) -> Option<usize> {
        match self {
            Stage::Behind | Stage::Owed { .. } | Stage::Bookless { .. } | Stage::Ready | Stage::Unseated { .. } | Stage::Unentered { .. } | Stage::Declared { .. } | Stage::TakenUp { .. } => None,
            Stage::Unread { written: true, .. } => Some(4),
            Stage::Unread { .. } | Stage::Declare { .. } => Some(1),
            Stage::Write => Some(2),
            Stage::Sweep { .. } => Some(3),
            Stage::Anchoring { .. } | Stage::Accede { .. } => Some(4),
            Stage::Done => Some(5),
        }
    }

    pub fn waits_on_chain(&self) -> bool {
        matches!(self, Stage::Bookless { .. } | Stage::Unread { .. } | Stage::Anchoring { .. } | Stage::Sweep { send: None, .. })
    }

    pub fn walking(&self) -> bool {
        matches!(self, Stage::Declare { .. } | Stage::Write | Stage::Anchoring { .. } | Stage::Accede { .. } | Stage::Sweep { .. })
    }
}

pub fn last_move(items: &[Vec<u8>], current: &str) -> Option<book::Entry> {
    let empty = book::empty_fragment();
    let read = book::read(items, &empty).ok()?;
    read.successions().filter(|e| e.law == book::Law::Two && e.to().is_some_and(|to| to.eq_ignore_ascii_case(current)) && !e.author.eq_ignore_ascii_case(current)).max_by_key(|e| e.seq).cloned()
}

pub fn moved(current: &Address, table: Option<(String, String)>, table_current: bool, written: Option<(String, String)>) -> Option<(String, String)> {
    let written = written.filter(|(_, by)| !by.eq_ignore_ascii_case(&current.hex()));
    match (table, written) {
        (Some(m), _) if table_current => Some(m),
        (m, w) => w.or(m),
    }
}

pub fn holder(current: Address, next: Option<Address>, given: Option<Address>, moved: Option<&(String, String)>) -> Address {
    match moved {
        Some((_, by)) if next.is_none() && given.is_none() => Address::parse(by).unwrap_or(current),
        _ => current,
    }
}

pub fn office_of(items: &[Vec<u8>]) -> Option<(String, u64, bool)> {
    let empty = book::empty_fragment();
    let read = book::read(items, &empty).ok()?;
    let office = read.office.clone()?;
    let mut at = read.entries.iter().filter(|e| e.seq == 0 && e.kind == book::Kind::Genesis).max_by_key(|e| e.law)?.author.clone();
    let (mut moves, mut rotation) = (0u64, false);
    let mut successions: Vec<&book::Entry> = read.successions().filter(|e| e.law == book::Law::Two).collect();
    successions.sort_by_key(|e| e.seq);
    for e in successions {
        if let Some(to) = e.to().filter(|to| e.author.eq_ignore_ascii_case(&at) && !to.eq_ignore_ascii_case(&at)) {
            at = to.to_string();
            moves += 1;
            rotation = e.body.member("kind").and_then(|v| v.as_str()) == Some(crate::succeedx::KIND_ROTATION);
        }
    }
    Some((office, moves, rotation))
}

pub struct Owing<'a> {
    pub on: &'a crate::chainx::OnChain,
    pub queued: &'a [String],
    pub own: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Faces {
    pub root: Option<[u8; 20]>,
    pub unseated: Option<usize>,
    pub home: Option<u64>,
    pub sealed: bool,
}

impl Faces {
    pub const NONE: Faces = Faces { root: None, unseated: Some(0), home: None, sealed: false };
}

fn unentered(basis: &[u64], chains: &[Chain], root: &[u8; 20], office: &Address) -> Vec<u64> {
    chains
        .iter()
        .filter(|c| basis.contains(&c.chain))
        .filter_map(|c| match c.book {
            Book::Read { current, root: of_root, .. } if !crate::chainx::book::Held { root: of_root, current }.holds(root, &office.0) => Some(c.chain),
            _ => None,
        })
        .collect()
}

fn faces_hold(basis: &[u64], chains: &[Chain], office: &Address, faces: &Faces) -> Option<Stage> {
    if faces.sealed {
        return None;
    }
    match faces.unseated {
        Some(0) => {}
        n => return Some(Stage::Unseated { n }),
    }
    let root = faces.root?;
    let chains = unentered(basis, chains, &root, office);
    (!chains.is_empty()).then_some(Stage::Unentered { chains })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enrol {
    Held,
    Declare,
    Accede,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closed {
    RootElsewhere([u8; 20]),
    OtherKey([u8; 20]),
    OfficeEntered,
}

pub fn enrolment(root_of_root: [u8; 20], current: [u8; 20], pending: [u8; 20], office_entered: bool, root: &[u8; 20], office: &[u8; 20]) -> Result<Enrol, Closed> {
    let held = crate::chainx::book::Held { root: root_of_root, current };
    match held.key_under(root) {
        None => Err(Closed::RootElsewhere(root_of_root)),
        Some(k) if k == *office => Ok(Enrol::Held),
        Some(k) if k != *root => Err(Closed::OtherKey(k)),
        Some(_) if pending == *office => Ok(Enrol::Accede),
        Some(_) if office_entered => Err(Closed::OfficeEntered),
        Some(_) => Ok(Enrol::Declare),
    }
}

pub fn owed(report: Option<&Value>, owing: &Owing, local: &std::collections::BTreeSet<String>) -> usize {
    let mut ids: std::collections::BTreeSet<String> = report.map(|r| crate::auditx::entry_ids(r, zikaron::v2::tokens::Key::Unanchored)).unwrap_or_default().into_iter().filter(|id| !owing.on.anywhere(id)).collect();
    ids.extend(owing.queued.iter().map(|id| id.to_ascii_lowercase()));
    ids.retain(|id| !local.contains(id) && owing.own.is_none_or(|o| !o.eq_ignore_ascii_case(id)));
    ids.len()
}

#[allow(clippy::too_many_arguments)]
pub fn stage(current: Address, next: Option<Address>, moved: Option<&str>, report: Option<&Value>, owing: &Owing, local: &std::collections::BTreeSet<String>, basis: &[u64], chains: &[Chain], faces: &Faces) -> Stage {
    if next.is_some() {
        if report.is_none() {
            return Stage::Behind;
        }
        let n = owed(report, owing, local);
        if n > 0 {
            return Stage::Owed { n };
        }
        if let Some(c) = basis.iter().find(|c| !chains.iter().any(|x| x.chain == **c)) {
            return Stage::Unread { chain: *c, written: false };
        }
        if let Some(b) = bookless(basis, chains) {
            return b;
        }
        if let Some(f) = faces_hold(basis, chains, &current, faces) {
            return f;
        }
        let next = next.unwrap_or(current);
        for c in chains {
            if let Book::Read { pending, .. } = c.book {
                if pending != next.0 {
                    return Stage::Declare { chain: c.chain };
                }
            }
        }
        return Stage::Write;
    }
    if let Some(moved) = moved {
        let id = moved.to_ascii_lowercase();
        let on_every = basis.iter().all(|c| owing.on.on(*c, &id));
        if !on_every {
            return Stage::Anchoring { id: moved.to_string() };
        }
        if let Some(c) = basis.iter().find(|c| !chains.iter().any(|x| x.chain == **c)) {
            return Stage::Unread { chain: *c, written: true };
        }
        let mut order: Vec<&Chain> = chains.iter().collect();
        order.sort_by_key(|c| Some(c.chain) != faces.home);
        for c in order {
            if let Book::Read { pending, current: now, .. } = c.book {
                if now != current.0 || pending == current.0 {
                    return Stage::Accede { chain: c.chain };
                }
            }
        }
        return Stage::Done;
    }
    if report.is_none() {
        return Stage::Behind;
    }
    match owed(report, owing, local) {
        0 => {}
        n => return Stage::Owed { n },
    }
    if let Some(c) = basis.iter().find(|c| !chains.iter().any(|x| x.chain == **c)) {
        return Stage::Unread { chain: *c, written: false };
    }
    bookless(basis, chains).or_else(|| faces_hold(basis, chains, &current, faces)).unwrap_or(Stage::Ready)
}

#[allow(clippy::too_many_arguments)]
pub fn stage_given(current: Address, to: Address, report: Option<&Value>, owing: &Owing, local: &std::collections::BTreeSet<String>, basis: &[u64], chains: &[Chain], faces: &Faces) -> Stage {
    stage(current, Some(to), None, report, owing, local, basis, chains, faces)
}

pub fn stage_handed(id: &str, by: &str, report: Option<&Value>, on: &crate::chainx::OnChain, basis: &[u64]) -> Stage {
    if basis.iter().all(|c| report.is_some_and(|r| crate::auditx::held_on(r, *c, id, by)) || on.on(*c, id)) {
        Stage::Done
    } else {
        Stage::Anchoring { id: id.to_string() }
    }
}

fn bookless(basis: &[u64], chains: &[Chain]) -> Option<Stage> {
    let lacking: Vec<(u64, Lack)> = chains
        .iter()
        .filter(|c| basis.contains(&c.chain))
        .filter_map(|c| match c.book {
            Book::Off => Some((c.chain, Lack::NoRow)),
            Book::Wrong => Some((c.chain, Lack::WrongCode)),
            Book::Read { .. } => None,
        })
        .collect();
    (!lacking.is_empty()).then_some(Stage::Bookless { chains: lacking })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fell {
    Declared { next: Address },
    Acceded { current: Address },
    Withdrawn,
}

pub fn step_waits(stage: &Stage, sending: Option<u64>) -> bool {
    match stage {
        Stage::Declare { chain } | Stage::Accede { chain } | Stage::Sweep { chain, .. } => sending == Some(*chain),
        _ => false,
    }
}

pub fn succession_out(id: &str, sending: bool, out: &[String]) -> bool {
    sending || out.iter().any(|o| o.eq_ignore_ascii_case(id))
}

pub fn after_fell(mut chains: Vec<Chain>, chain: u64, fell: Fell) -> Vec<Chain> {
    for c in chains.iter_mut().filter(|c| c.chain == chain) {
        match (fell, c.book) {
            (Fell::Declared { next }, Book::Read { current, root, .. }) => c.book = Book::Read { pending: next.0, current, root, own: next.0 },
            (Fell::Acceded { current }, Book::Read { root, .. }) => c.book = Book::Read { pending: [0; 20], current: current.0, root, own: [0; 20] },
            (Fell::Withdrawn, Book::Read { pending, current, root, .. }) => c.book = Book::Read { pending, current, root, own: WITHDRAWN },
            _ => {}
        }
    }
    chains
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Left {
    pub chain: u64,
    pub held: u128,
    pub cost: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftRead {
    pub old: crate::key::Address,
    pub left: Vec<Left>,
}

impl Left {
    pub fn sendable(&self) -> u128 {
        self.held.saturating_sub(self.cost)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Short {
    pub chain: Option<u64>,
    pub need: Option<u128>,
    pub base: Vec<(u64, u128)>,
}

impl Short {
    pub fn of(chain: Option<u64>, funds: Option<(u128, u128)>, fresh: &[(u64, u128)]) -> Short {
        let mut base: Vec<(u64, u128)> = fresh.to_vec();
        if let (Some(c), Some((_, have))) = (chain, funds) {
            base.retain(|(x, _)| *x != c);
            base.push((c, have));
        }
        Short { chain, need: funds.map(|(n, _)| n), base }
    }
}

pub fn short_now(short: &Short, fresh: &[(u64, u128)]) -> bool {
    let chains: Vec<u64> = match short.chain {
        Some(c) => vec![c],
        None if fresh.is_empty() => return true,
        None => fresh.iter().map(|(c, _)| *c).collect(),
    };
    chains.iter().any(|c| match (fresh.iter().find(|(x, _)| x == c).map(|(_, w)| *w), short.need, short.base.iter().find(|(x, _)| x == c).map(|(_, w)| *w)) {
        (None, _, _) => true,
        (Some(now), Some(need), _) => now < need,
        (Some(now), None, Some(then)) => now <= then,
        (Some(_), None, None) => true,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Settled {
    Sent { value: u128, tx: String },
    Short { held: u128, cost: u128 },
    Left { held: Option<u128> },
}

impl Settled {
    pub fn line(&self, id: &str, chain: u64) -> String {
        let id = id.to_ascii_lowercase();
        match self {
            Settled::Sent { value, tx } => format!("{id} {chain} sent {value} {tx}"),
            Settled::Short { held, cost } => format!("{id} {chain} short {held} {cost}"),
            Settled::Left { held: Some(held) } => format!("{id} {chain} left {held}"),
            Settled::Left { held: None } => format!("{id} {chain} left {UNREAD}"),
        }
    }
}

const UNREAD: &str = "-";

pub fn settled_of(lines: &[String], id: &str) -> Vec<(u64, Settled)> {
    lines
        .iter()
        .filter_map(|l| {
            let w: Vec<&str> = l.split(' ').collect();
            if !w.first()?.eq_ignore_ascii_case(id) {
                return None;
            }
            let chain = w.get(1)?.parse().ok()?;
            let n = |i: usize| w.get(i).and_then(|x| x.parse::<u128>().ok());
            let s = match (w.get(2).copied(), w.len()) {
                (Some("sent"), 5) => Settled::Sent { value: n(3)?, tx: w[4].to_string() },
                (Some("short"), 5) => Settled::Short { held: n(3)?, cost: n(4)? },
                (Some("left"), 4) if w[3] == UNREAD => Settled::Left { held: None },
                (Some("left"), 4) => Settled::Left { held: Some(n(3)?) },
                _ => return None,
            };
            Some((chain, s))
        })
        .collect()
}

pub fn sweep(basis: &[u64], left: &[Left], settled: &[(u64, Settled)], then: Option<&Stage>) -> Option<Stage> {
    let then = then.map(|s| Box::new(s.clone()));
    for c in basis {
        if settled.iter().any(|(x, _)| x == c) {
            continue;
        }
        match left.iter().find(|l| l.chain == *c) {
            Some(l) if l.sendable() == 0 => {}
            Some(l) => return Some(Stage::Sweep { chain: *c, send: Some(l.sendable()), then }),
            None => return Some(Stage::Sweep { chain: *c, send: None, then }),
        }
    }
    None
}

pub fn shorts(basis: &[u64], left: &[Left], settled: &[(u64, Settled)]) -> Vec<(u64, Settled)> {
    left.iter()
        .filter(|l| basis.contains(&l.chain) && l.sendable() == 0 && !settled.iter().any(|(c, _)| *c == l.chain))
        .map(|l| (l.chain, Settled::Short { held: l.held, cost: l.cost }))
        .collect()
}

pub fn fit(held: u128, value: u128, data: u128, now: zikaron_anchor::send::Fees) -> Option<zikaron_anchor::send::Fees> {
    let room = held.checked_sub(value)?.checked_sub(data)?;
    let gas = u128::from(now.gas_limit.max(1));
    let cap = (room / gas).min(u128::from(now.max_fee));
    let base = if now.from_chain { u128::from(now.max_fee.saturating_sub(now.priority) / 2) } else { u128::from(now.max_fee) };
    if value == 0 || cap < base {
        return None;
    }
    let cap = u64::try_from(cap).ok()?;
    Some(zikaron_anchor::send::Fees { max_fee: cap, priority: now.priority.min(cap), ..now })
}

pub const SWEEP_ASKS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    Send { value: u128, limit: u64 },
    Short { cost: u128 },
    Unsettled,
}

pub fn sweep_amount<E>(held: u128, fee: u64, data: u128, opening: (u128, u64), mut limit_at: impl FnMut(u128) -> Result<u64, E>) -> Result<Amount, E> {
    let (mut asked, mut limit) = opening;
    for n in 1..=SWEEP_ASKS {
        let cost = u128::from(limit).saturating_mul(u128::from(fee)).saturating_add(data);
        if cost >= held {
            return Ok(Amount::Short { cost });
        }
        let value = held - cost;
        if value == asked {
            return Ok(Amount::Send { value, limit });
        }
        if n == SWEEP_ASKS {
            break;
        }
        let at = limit_at(value)?;
        if at <= limit {
            return Ok(Amount::Send { value, limit: at });
        }
        (asked, limit) = (value, at);
    }
    Ok(Amount::Unsettled)
}

pub fn sweep_opening(held: u128) -> u128 {
    (held / 2).max(1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reverted {
    pub chain: u64,
    pub step: &'static str,
    pub tx: String,
    pub used: Option<u64>,
    pub limit: u64,
}

impl Reverted {
    pub fn out_of_gas(&self) -> bool {
        self.used == Some(self.limit)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn too_little_is_said_by_the_balance_read_not_by_the_last_refusal() {
        use super::{short_now, Short};
        let refused = Short::of(Some(7), Some((100, 40)), &[(7, 40)]);
        assert!(!short_now(&refused, &[(7, 100)]), "funded to what was needed: not said");
        assert!(short_now(&refused, &[(7, 99)]), "still below: said");
        assert!(short_now(&refused, &[(8, 1_000)]), "its chain not read since: said");
        let bare = Short::of(Some(7), None, &[(7, 40)]);
        assert!(!short_now(&bare, &[(7, 41)]), "no amount given, the balance rose: not said");
        assert!(short_now(&bare, &[(7, 40)]), "no amount given, nothing came: said");
        let batch = Short::of(None, Some((100, 0)), &[]);
        assert!(short_now(&batch, &[(7, 100), (8, 5)]), "a batch, one chain short: said");
        assert!(!short_now(&batch, &[(7, 100), (8, 100)]), "a batch, every chain funded: not said");
        assert!(short_now(&batch, &[]), "a batch, nothing read: said");
        assert_eq!(refused.base, vec![(7, 40)], "the balance then is what the refusal said");
    }

    #[test]
    fn a_step_waits_on_the_chain_only_for_its_own_transaction() {
        let declare = Stage::Declare { chain: 7 };
        assert!(step_waits(&declare, Some(7)));
        assert!(!step_waits(&declare, None), "a reading in flight is no step's transaction");
        assert!(!step_waits(&declare, Some(8)), "a transaction on another chain is not this step's");
        assert!(step_waits(&Stage::Accede { chain: 7 }, Some(7)));
        for s in [Stage::Write, Stage::Ready, Stage::Unread { chain: 7, written: false }, Stage::Anchoring { id: String::new() }, Stage::Done, Stage::Behind] {
            assert!(!step_waits(&s, Some(7)), "{s:?} sends no transaction of its own");
        }
    }

    #[test]
    fn the_succession_waits_on_the_chain_only_once_sent() {
        let id = "0x".to_string() + &"ab".repeat(32);
        assert!(!succession_out(&id, false, &[]), "queued, not sent");
        assert!(!succession_out(&id, false, &["0x".to_string() + &"cd".repeat(32)]), "another entry out is not this one");
        assert!(succession_out(&id, true, &[]), "the batch being sent");
        assert!(succession_out(&id, false, &[id.to_ascii_uppercase()]), "its transaction out, awaiting the receipt");
    }

    use super::*;
    use crate::chainx::OnChain;

    fn owing<'a>(on: &'a OnChain, queued: &'a [String]) -> Owing<'a> {
        Owing { on, queued, own: None }
    }

    #[test]
    fn each_step_is_read_off_what_stands() {
        let (k0, k1) = (Address([0x11; 20]), Address([0x22; 20]));
        let read = |pending: [u8; 20]| Book::Read { pending, current: k0.0, root: k0.0, own: pending };
        let chain = |book: Book| Chain { chain: 7, book };
        let r = zikaron::json::parse(br#"{"unanchored":[]}"#).ok().expect("json");
        let r = Some(&r);
        let none = OnChain::default();
        let (q0, q1, q2): (Vec<String>, Vec<String>, Vec<String>) = (vec![], vec!["0x0a".into()], vec!["0x0a".into(), "0x0b".into()]);
        let (o0, o1, o2) = (owing(&none, &q0), owing(&none, &q1), owing(&none, &q2));
        assert_eq!(stage(k0, Some(k1), None, None, &o0, &Default::default(), &[7], &[chain(read(k1.0))], &Faces::NONE), Stage::Behind, "no report in step: a self-audit first");
        assert_eq!(stage(k0, Some(k1), None, r, &o2, &Default::default(), &[7], &[], &Faces::NONE), Stage::Owed { n: 2 }, "two entries still queued");
        assert_eq!(stage(k0, Some(k1), None, r, &o0, &Default::default(), &[7], &[], &Faces::NONE), Stage::Unread { chain: 7, written: false }, "the chain not read: nothing written");
        assert_eq!(stage(k0, Some(k1), None, r, &o0, &Default::default(), &[7, 9], &[chain(read(k1.0))], &Faces::NONE), Stage::Unread { chain: 9, written: false }, "one chain of two read");
        assert_eq!(stage(k0, Some(k1), None, r, &o0, &Default::default(), &[7], &[chain(read([0; 20]))], &Faces::NONE), Stage::Declare { chain: 7 });
        assert_eq!(stage(k0, Some(k1), None, r, &o0, &Default::default(), &[7], &[chain(read(k1.0))], &Faces::NONE), Stage::Write, "told: the succession next, whatever the new key holds");
        assert_eq!(stage(k0, Some(k1), None, r, &o0, &Default::default(), &[7], &[chain(Book::Off)], &Faces::NONE), Stage::Bookless { chains: vec![(7, Lack::NoRow)] }, "no book: nothing written");
        assert_eq!(stage(k0, Some(k1), None, r, &o0, &Default::default(), &[7], &[chain(Book::Wrong)], &Faces::NONE), Stage::Bookless { chains: vec![(7, Lack::WrongCode)] }, "a book of other code: nothing written");
        let looked = |book: Book, c: u64| Chain { chain: c, book };
        assert_eq!(stage(k0, None, None, r, &o0, &Default::default(), &[7], &[], &Faces::NONE), Stage::Unread { chain: 7, written: false }, "no change starts before the book is read");
        assert_eq!(stage(k0, None, None, r, &o0, &Default::default(), &[7], &[looked(read([0; 20]), 7)], &Faces::NONE), Stage::Ready);
        assert_eq!(stage(k0, None, None, r, &o0, &Default::default(), &[7, 9], &[looked(read([0; 20]), 7), looked(Book::Off, 9)], &Faces::NONE), Stage::Bookless { chains: vec![(9, Lack::NoRow)] }, "one chain of two with no book row");
        assert_eq!(stage(k0, None, None, r, &o0, &Default::default(), &[7, 9], &[looked(Book::Wrong, 7), looked(Book::Off, 9)], &Faces::NONE), Stage::Bookless { chains: vec![(7, Lack::WrongCode), (9, Lack::NoRow)] }, "each chain named with why");
        assert_eq!(stage(k0, None, None, None, &o0, &Default::default(), &[7], &[], &Faces::NONE), Stage::Behind);
        assert_eq!(stage(k0, None, None, r, &o1, &Default::default(), &[7], &[], &Faces::NONE), Stage::Owed { n: 1 });
        let id = "0x".to_string() + &"ab".repeat(32);
        assert_eq!(stage(k1, None, Some(&id), None, &o0, &Default::default(), &[7], &[], &Faces::NONE), Stage::Anchoring { id: id.clone() }, "no report: not yet on chain");
        let report = zikaron::json::parse(format!(r#"{{"anchored":[{{"anchors":[{{"chainId":7}}],"entry_id":"{id}"}}]}}"#).as_bytes()).ok().expect("json");
        let on7 = OnChain::of(crate::auditx::anchored_on(&report, 7).into_iter().map(|x| (Some(7), x)), &Default::default());
        let r7 = owing(&on7, &q0);
        assert_eq!(stage(k1, None, Some(&id), Some(&report), &r7, &Default::default(), &[7, 9], &[], &Faces::NONE), Stage::Anchoring { id: id.clone() }, "one chain of two");
        let taken = |current: [u8; 20]| Chain { chain: 7, book: Book::Read { pending: [0; 20], current, root: k0.0, own: [0; 20] } };
        assert_eq!(stage(k1, None, Some(&id), Some(&report), &r7, &Default::default(), &[7], &[taken(k0.0)], &Faces::NONE), Stage::Accede { chain: 7 });
        assert_eq!(stage(k1, None, Some(&id), Some(&report), &r7, &Default::default(), &[7], &[taken(k1.0)], &Faces::NONE), Stage::Done);
        let named = Chain { chain: 7, book: Book::Read { pending: k1.0, current: k1.0, root: k0.0, own: k1.0 } };
        assert_eq!(stage(k1, None, Some(&id), Some(&report), &r7, &Default::default(), &[7], &[named], &Faces::NONE), Stage::Accede { chain: 7 });
        let given = |pending: [u8; 20]| Chain { chain: 7, book: Book::Read { pending, current: k0.0, root: k0.0, own: pending } };
        assert_eq!(stage_given(k0, k1, None, &o0, &Default::default(), &[7], &[given(k1.0)], &Faces::NONE), Stage::Behind);
        assert_eq!(stage_given(k0, k1, r, &o1, &Default::default(), &[7], &[given(k1.0)], &Faces::NONE), Stage::Owed { n: 1 });
        assert_eq!(stage_given(k0, k1, r, &o0, &Default::default(), &[7], &[], &Faces::NONE), Stage::Unread { chain: 7, written: false });
        assert_eq!(stage_given(k0, k1, r, &o0, &Default::default(), &[7], &[Chain { chain: 7, book: Book::Off }], &Faces::NONE), Stage::Bookless { chains: vec![(7, Lack::NoRow)] });
        assert_eq!(stage_given(k0, k1, r, &o0, &Default::default(), &[7], &[given([0; 20])], &Faces::NONE), Stage::Declare { chain: 7 });
        assert_eq!(stage_given(k0, k1, r, &o0, &Default::default(), &[7], &[given(k1.0)], &Faces::NONE), Stage::Write);
        assert_eq!(stage_handed(&id, &k0.hex(), None, &none, &[7]), Stage::Anchoring { id: id.clone() });
        assert_eq!(stage_handed(&id, &k0.hex(), Some(&report), &none, &[7]), Stage::Done);
        assert_eq!(stage_handed(&id, &k0.hex(), Some(&report), &none, &[7, 9]), Stage::Anchoring { id: id.clone() });
        let both = zikaron::json::parse(format!(r#"{{"anchored":[{{"anchors":[{{"chainId":7}}],"entry_id":"{id}"}}],"discarded":[{{"chainId":9,"hash":"{id}","sender":"{}"}}]}}"#, k0.hex()).as_bytes()).ok().expect("json");
        assert_eq!(stage_handed(&id, &k0.hex(), Some(&both), &none, &[7, 9]), Stage::Done);
        let unanchored = zikaron::json::parse(br#"{"unanchored":["0x01","0x02"]}"#).ok().expect("json");
        let local: std::collections::BTreeSet<String> = ["0x02".to_string()].into_iter().collect();
        assert_eq!(owed(Some(&unanchored), &o0, &local), 1);
        assert_eq!(owed(Some(&unanchored), &o0, &Default::default()), 2);
    }

    #[test]
    fn the_amount_shown_goes_its_fee_fitted_to_what_is_left() {
        use zikaron_anchor::send::Fees;
        let at = |base: u64, tip: u64| Fees { max_fee: base * 2 + tip, priority: tip, gas_limit: 21_000, from_chain: true };
        let shown = at(8, 50);
        let (value, data) = (1_000_000u128, 7u128);
        let held = value + 21_000 * 66 + data;
        assert_eq!(fit(held, value, data, shown), Some(shown), "as read: as shown");
        assert_eq!(fit(held, value, data, at(8, 1_000_000_000)), Some(Fees { max_fee: 66, priority: 66, ..at(8, 1_000_000_000) }), "the tip jumped: the fee takes what is left");
        assert_eq!(fit(held, value, data, at(33, 50)), Some(Fees { max_fee: 66, priority: 50, ..at(33, 50) }), "a base fee up to what is left still goes");
        assert_eq!(fit(held, value, data, at(80_000, 50)), None, "the base fee past what is left: nothing goes");
        assert_eq!(fit(held, 0, data, shown), None, "nothing to send");
        assert_eq!(fit(held, held + 1, 0, shown), None, "more than is held");
        assert_eq!(fit(held, value, held - value + 1, shown), None, "the data charge eats the rest");
        let fallback = Fees::fallback();
        let exact = value + u128::from(fallback.gas_limit) * u128::from(fallback.max_fee);
        assert_eq!(fit(exact, value, 0, fallback), Some(fallback), "a price not read, met: as it is");
        assert_eq!(fit(exact - 1, value, 0, fallback), None, "a price not read is never lowered");
        assert_eq!(fit(u128::MAX, u128::MAX - 21_000 * 66, 0, shown), Some(shown), "any balance");
    }

    #[test]
    fn what_is_owed_is_read_off_the_one_set() {
        let id = |b: u8| format!("0x{}", format!("{b:02x}").repeat(32));
        let unanchored = zikaron::json::parse(format!(r#"{{"unanchored":["{}","{}","{}"]}}"#, id(1), id(2), id(3)).as_bytes()).ok().expect("json");
        let mut q = zikaron_seal::queue::Queue { chain: 7, ..Default::default() };
        q.blocks.push(zikaron_seal::queue::Block { id: id(1), tx: id(9), chain: 7, block: 5 });
        let on = OnChain::of(Vec::new(), &q);
        let queued = vec![id(2), id(4)];
        assert_eq!(owed(Some(&unanchored), &owing(&on, &queued), &Default::default()), 3);
        assert_eq!(owed(Some(&unanchored), &owing(&OnChain::default(), &queued), &Default::default()), 4, "without the receipt the received one is owed");
        assert_eq!(owed(Some(&unanchored), &Owing { on: &on, queued: &queued, own: Some(&id(4).to_ascii_uppercase()) }, &Default::default()), 2);
        let (k0, k1) = (Address([0x11; 20]), Address([0x22; 20]));
        let empty = zikaron::json::parse(br#"{"unanchored":[]}"#).ok().expect("json");
        let only = vec![id(4)];
        let looked = Chain { chain: 7, book: Book::Read { pending: [0; 20], current: k0.0, root: [0x11; 20], own: [0; 20] } };
        assert_eq!(stage(k0, None, None, Some(&empty), &Owing { on: &on, queued: &only, own: Some(&id(4)) }, &Default::default(), &[7], &[looked], &Faces::NONE), Stage::Ready);
        assert_eq!(stage(k0, None, None, Some(&empty), &owing(&on, &only), &Default::default(), &[7], &[looked], &Faces::NONE), Stage::Owed { n: 1 }, "another change's entry is owed");
        let mut q2 = zikaron_seal::queue::Queue { chain: 7, ..Default::default() };
        q2.blocks.push(zikaron_seal::queue::Block { id: id(5), tx: id(9), chain: 7, block: 6 });
        let on2 = OnChain::of(Vec::new(), &q2);
        let none: Vec<String> = Vec::new();
        let taken = Chain { chain: 7, book: Book::Read { pending: [0; 20], current: k1.0, root: [0x11; 20], own: [0; 20] } };
        assert_eq!(stage(k1, None, Some(&id(5)), None, &owing(&on2, &none), &Default::default(), &[7], &[taken], &Faces::NONE), Stage::Done, "the receipt says it is on chain");
        assert_eq!(stage(k1, None, Some(&id(5)), None, &owing(&on2, &none), &Default::default(), &[7, 9], &[taken], &Faces::NONE), Stage::Anchoring { id: id(5) }, "one chain of two");
        assert_eq!(stage_handed(&id(5), &k0.hex(), None, &on2, &[7]), Stage::Done);
        assert_eq!(stage_handed(&id(5), &k0.hex(), None, &on2, &[7, 9]), Stage::Anchoring { id: id(5) });
    }

    #[test]
    fn a_change_waits_on_the_face_entries_and_the_books_and_takes_the_home_first() {
        let (k0, k1, root) = (Address([0x11; 20]), Address([0x22; 20]), [0x0a; 20]);
        let r = zikaron::json::parse(br#"{"unanchored":[]}"#).ok().expect("json");
        let none = OnChain::default();
        let q: Vec<String> = Vec::new();
        let o = owing(&none, &q);
        let held = |c: u64| Chain { chain: c, book: Book::Read { pending: [0; 20], current: k0.0, root, own: [0; 20] } };
        let faces = |unseated: Option<usize>| Faces { root: Some(root), unseated, home: None, sealed: false };
        let at = |chains: &[Chain], f: &Faces| stage(k0, None, None, Some(&r), &o, &Default::default(), &[7, 9], chains, f);
        assert_eq!(at(&[held(7), held(9)], &faces(Some(0))), Stage::Ready);
        assert_eq!(at(&[held(7), held(9)], &faces(Some(2))), Stage::Unseated { n: Some(2) });
        assert_eq!(at(&[held(7), held(9)], &faces(None)), Stage::Unseated { n: None }, "the table not read");
        let never = Chain { chain: 9, book: Book::Read { pending: [0; 20], current: root, root, own: [0; 20] } };
        assert_eq!(at(&[held(7), never], &faces(Some(0))), Stage::Unentered { chains: vec![9] }, "the root never entered on 9");
        let elsewhere = Chain { chain: 7, book: Book::Read { pending: [0; 20], current: k0.0, root: [0x0b; 20], own: [0; 20] } };
        assert_eq!(at(&[elsewhere, held(9)], &faces(Some(0))), Stage::Unentered { chains: vec![7] }, "the root under another root on 7");
        assert_eq!(at(&[held(7), never], &Faces::NONE), Stage::Ready, "no ledger root read: as before");
        let sealed = |unseated: Option<usize>| Faces { root: Some(root), unseated, home: None, sealed: true };
        assert_eq!(at(&[held(7), held(9)], &sealed(Some(2))), Stage::Ready, "unseated entries are not asked of a sealed seat");
        assert_eq!(at(&[held(7), never], &sealed(None)), Stage::Ready, "nor the books it never entered");
        assert_eq!(stage(k0, Some(k1), None, Some(&r), &o, &Default::default(), &[7], &[held(7)], &faces(Some(1))), Stage::Unseated { n: Some(1) });
        let id = format!("0x{}", "ab".repeat(32));
        let report = zikaron::json::parse(format!(r#"{{"anchored":[{{"anchors":[{{"chainId":7}},{{"chainId":9}}],"entry_id":"{id}"}}]}}"#).as_bytes()).ok().expect("json");
        let on = OnChain::of(crate::auditx::anchored_on(&report, 7).into_iter().map(|x| (Some(7), x)).chain(crate::auditx::anchored_on(&report, 9).into_iter().map(|x| (Some(9), x))), &Default::default());
        let r2 = owing(&on, &q);
        let old = |c: u64| Chain { chain: c, book: Book::Read { pending: [0; 20], current: k0.0, root, own: [0; 20] } };
        let home9 = Faces { root: Some(root), unseated: Some(0), home: Some(9), sealed: false };
        assert_eq!(stage(k1, None, Some(&id), Some(&report), &r2, &Default::default(), &[7, 9], &[old(7), old(9)], &home9), Stage::Accede { chain: 9 }, "the home first");
        assert_eq!(stage(k1, None, Some(&id), Some(&report), &r2, &Default::default(), &[7, 9], &[old(7), old(9)], &Faces::NONE), Stage::Accede { chain: 7 }, "no home: the basis's order");
        let taken9 = Chain { chain: 9, book: Book::Read { pending: [0; 20], current: k1.0, root, own: [0; 20] } };
        assert_eq!(stage(k1, None, Some(&id), Some(&report), &r2, &Default::default(), &[7, 9], &[old(7), taken9], &home9), Stage::Accede { chain: 7 }, "then the others");
    }

    #[test]
    fn entering_a_book_is_judged_once_on_both_views() {
        let (r, k, x) = ([0x0a; 20], [0x0b; 20], [0x0c; 20]);
        assert_eq!(enrolment(r, k, [0; 20], false, &r, &k), Ok(Enrol::Held));
        assert_eq!(enrolment(r, r, [0; 20], false, &r, &k), Ok(Enrol::Declare));
        assert_eq!(enrolment(r, r, k, false, &r, &k), Ok(Enrol::Accede), "declared already: the accession alone");
        assert_eq!(enrolment(x, k, [0; 20], false, &r, &k), Err(Closed::RootElsewhere(x)), "current(R) counts only while root(R) is R");
        assert_eq!(enrolment(r, x, [0; 20], false, &r, &k), Err(Closed::OtherKey(x)));
        assert_eq!(enrolment(r, r, [0; 20], true, &r, &k), Err(Closed::OfficeEntered));
    }

    #[test]
    fn what_the_books_say_of_a_seat_at_rest_is_judged_alone() {
        let (root, old, now, to, other) = ([0x0a; 20], [0x11; 20], [0x22; 20], [0x33; 20], [0x44; 20]);
        let book = |pending: [u8; 20], current: [u8; 20], own: [u8; 20]| Chain { chain: 7, book: Book::Read { pending, current, root, own } };
        assert_eq!(standing(&[book([0; 20], now, to)], &[7], &root, &now, &old), Some(Stage::Declared { chain: 7, to }), "moved before, a declaration of its own stands");
        assert_eq!(standing(&[book([0; 20], now, [0; 20])], &[7], &root, &now, &old), None);
        assert_eq!(standing(&[book([0; 20], now, WITHDRAWN)], &[7], &root, &now, &old), None, "withdrawn");
        assert_eq!(standing(&[book([0; 20], other, [0; 20])], &[7], &root, &now, &old), Some(Stage::TakenUp { chain: 7, by: other }));
        assert_eq!(standing(&[book([0; 20], root, to)], &[7], &root, &now, &now), Some(Stage::Declared { chain: 7, to }), "a book holding the root is no other key");
        assert_eq!(standing(&[book(now, old, [0; 20])], &[7], &root, &now, &old), None, "the record waits to be taken up from the key before");
        assert_eq!(standing(&[book([0; 20], now, to)], &[9], &root, &now, &old), None, "a chain outside the basis says nothing");
        assert_eq!(standing(&[Chain { chain: 7, book: Book::Off }], &[7], &root, &now, &old), None);
        let behind = after_fell(vec![book([0; 20], now, to)], 7, Fell::Withdrawn);
        assert_eq!(standing(&behind, &[7], &root, &now, &old), None, "a withdrawal that fell stands over a node behind it");
        assert_eq!(behind[0].book, Book::Read { pending: [0; 20], current: now, root, own: WITHDRAWN });
    }

    #[test]
    fn a_step_that_fell_stands_over_a_reading_behind_it() {
        let (k0, k1) = (Address([0x11; 20]), Address([0x22; 20]));
        let before = vec![
            Chain { chain: 7, book: Book::Read { pending: [0; 20], current: k0.0, root: [0x11; 20], own: [0; 20] } },
            Chain { chain: 9, book: Book::Read { pending: [0; 20], current: k0.0, root: [0x11; 20], own: [0; 20] } },
        ];
        let told = after_fell(before.clone(), 7, Fell::Declared { next: k1 });
        assert_eq!(told[0].book, Book::Read { pending: k1.0, current: k0.0, root: [0x11; 20], own: k1.0 });
        assert_eq!(told[1], before[1], "the other chain as read");
        let taken = after_fell(before.clone(), 7, Fell::Acceded { current: k1 });
        assert_eq!(taken[0].book, Book::Read { pending: [0; 20], current: k1.0, root: [0x11; 20], own: [0; 20] });
        let off = after_fell(vec![Chain { book: Book::Off, ..before[0] }], 7, Fell::Declared { next: k1 });
        assert_eq!(off[0].book, Book::Off, "a book not read is not made up");
        let both = after_fell(after_fell(before.clone(), 7, Fell::Declared { next: k1 }), 9, Fell::Declared { next: k1 });
        let r = zikaron::json::parse(br#"{"unanchored":[]}"#).ok().expect("json");
        let none: Vec<String> = Vec::new();
        assert_eq!(stage(k0, Some(k1), None, Some(&r), &owing(&OnChain::default(), &none), &Default::default(), &[7, 9], &both, &Faces::NONE), Stage::Write);
    }

    #[test]
    fn the_sheet_reads_the_chain_itself_where_the_change_waits_on_it() {
        let ahead = Some(Box::new(Stage::Anchoring { id: String::new() }));
        let waits = [
            Stage::Bookless { chains: vec![(7, Lack::NoRow)] },
            Stage::Unread { chain: 7, written: false },
            Stage::Anchoring { id: String::new() },
            Stage::Sweep { chain: 7, send: None, then: None },
            Stage::Sweep { chain: 7, send: None, then: ahead.clone() },
        ];
        for s in &waits {
            assert!(s.waits_on_chain(), "{s:?}");
        }
        let shown = [Stage::Sweep { chain: 7, send: Some(1), then: None }, Stage::Sweep { chain: 7, send: Some(1), then: ahead }];
        for s in [Stage::Behind, Stage::Owed { n: 1 }, Stage::Ready, Stage::Declare { chain: 7 }, Stage::Write, Stage::Accede { chain: 7 }, Stage::Done].into_iter().chain(shown) {
            assert!(!s.waits_on_chain(), "{s:?}");
        }
    }

    #[test]
    fn the_old_keys_balance_is_sent_on_once_for_each_chain() {
        let id = "0x".to_string() + &"ab".repeat(32);
        let left = |chain: u64, held: u128, cost: u128| Left { chain, held, cost };
        assert_eq!(left(7, 100, 30).sendable(), 70);
        assert_eq!(left(7, 30, 30).sendable(), 0, "all of it would go to the fee");
        assert_eq!(left(7, 10, 30).sendable(), 0);
        assert_eq!(left(7, u128::MAX, 1).sendable(), u128::MAX - 1, "any balance");
        assert_eq!(sweep(&[7], &[], &[], None), Some(Stage::Sweep { chain: 7, send: None, then: None }), "not read: read it");
        assert_eq!(sweep(&[7], &[left(7, 100, 30)], &[], None), Some(Stage::Sweep { chain: 7, send: Some(70), then: None }));
        let anchoring = Stage::Anchoring { id: id.clone() };
        assert_eq!(sweep(&[7], &[left(7, 100, 30)], &[], Some(&anchoring)), Some(Stage::Sweep { chain: 7, send: Some(70), then: Some(Box::new(anchoring.clone())) }), "ahead of the succession going on chain");
        assert_eq!(sweep(&[7], &[left(7, 10, 30)], &[], Some(&anchoring)), None, "too little to send: nothing asked");
        assert_eq!(shorts(&[7], &[left(7, 10, 30)], &[]), vec![(7, Settled::Short { held: 10, cost: 30 })]);
        assert_eq!(shorts(&[7], &[left(7, 100, 30)], &[]), Vec::new());
        assert_eq!(sweep(&[7, 9], &[left(7, 100, 30), left(9, 50, 10)], &[(7, Settled::Sent { value: 70, tx: "0x01".into() })], None), Some(Stage::Sweep { chain: 9, send: Some(40), then: None }), "each chain its own");
        let sent = Settled::Sent { value: 70, tx: "0x01".into() };
        assert_eq!(sweep(&[7], &[left(7, 900, 30)], &[(7, sent.clone())], None), None, "settled once: paid again, never asked again");
        assert_eq!(shorts(&[7], &[left(7, 10, 30)], &[(7, sent.clone())]), Vec::new(), "nor settled twice");
        assert_eq!(sweep(&[7], &[left(7, 100, 30)], &[(7, Settled::Left { held: Some(100) })], Some(&anchoring)), None, "left where it was");
        assert_eq!(sweep(&[9], &[left(7, 100, 30)], &[], None), Some(Stage::Sweep { chain: 9, send: None, then: None }), "a reading of another chain says nothing");
        let lines = vec![
            sent.line(&id.to_ascii_uppercase(), 7),
            Settled::Short { held: 10, cost: 30 }.line(&id, 9),
            Settled::Left { held: Some(u128::MAX) }.line(&id, 10),
            Settled::Left { held: None }.line(&id, 12),
            Settled::Left { held: Some(5) }.line(&("0x".to_string() + &"cd".repeat(32)), 7),
            format!("{id} 11 sent 70"),
            format!("{id} x left 5"),
            format!("{id} 13 left x"),
        ];
        assert_eq!(settled_of(&lines, &id), vec![(7, sent), (9, Settled::Short { held: 10, cost: 30 }), (10, Settled::Left { held: Some(u128::MAX) }), (12, Settled::Left { held: None })]);
        assert_eq!(settled_of(&[format!("{id} 7 left 0")], &id), vec![(7, Settled::Left { held: Some(0) })], "an old line, as written");
        assert!(Stage::Sweep { chain: 7, send: None, then: None }.walking());
        assert_eq!(Stage::Sweep { chain: 7, send: Some(1), then: None }.step(), Some(3), "after the write, before the chain");
        assert!(step_waits(&Stage::Sweep { chain: 7, send: Some(1), then: None }, Some(7)), "its own transfer out");
    }

    #[test]
    fn the_take_up_reads_one_succession_in_the_run_that_wrote_it_and_after_reopening() {
        let (root, new) = (Address([0x11; 20]), Address([0x22; 20]));
        let id = "0x".to_string() + &"ab".repeat(32);
        let succession = (id.clone(), root.hex());
        let older = ("0x".to_string() + &"cd".repeat(32), Address([0x33; 20]).hex());
        let same_run = moved(&new, None, false, Some(succession.clone()));
        assert_eq!(moved(&new, Some(older.clone()), false, Some(succession.clone())), same_run, "a stale table's older move gives way to this run's write");
        let reopened = moved(&new, Some(succession.clone()), true, None);
        assert_eq!(same_run, reopened);
        assert_eq!(same_run, Some(succession.clone()));
        for m in [&same_run, &reopened] {
            assert_eq!(holder(new, None, None, m.as_ref()), root, "taken up from the key that signed the succession");
        }
        let report = zikaron::json::parse(format!(r#"{{"anchored":[{{"anchors":[{{"chainId":7}}],"entry_id":"{id}"}}]}}"#).as_bytes()).ok().expect("json");
        let on = OnChain::of(crate::auditx::anchored_on(&report, 7).into_iter().map(|x| (Some(7), x)), &Default::default());
        let none: Vec<String> = Vec::new();
        let book = Chain { chain: 7, book: Book::Read { pending: new.0, current: root.0, root: [0x11; 20], own: new.0 } };
        for m in [&same_run, &reopened] {
            let moved_id = m.as_ref().map(|(i, _)| i.as_str());
            assert_eq!(stage(new, None, moved_id, Some(&report), &owing(&on, &none), &Default::default(), &[7], &[book], &Faces::NONE), Stage::Accede { chain: 7 });
        }
        assert_eq!(moved(&new, Some(older.clone()), true, Some(succession.clone())), Some(older));
        assert_eq!(moved(&new, None, false, Some((id, new.hex()))), None);
        assert_eq!(holder(new, Some(root), None, Some(&succession)), new);
        assert_eq!(holder(new, None, Some(root), Some(&succession)), new);
        assert_eq!(holder(root, None, None, None), root);
    }
    #[test]
    fn the_old_keys_transfer_is_asked_its_gas_at_the_amount_it_sends() {
        let limit = |v: u128| -> Result<u64, ()> { Ok(if v == 0 { 31_500 } else { 306_900 }) };
        let held = 20_000_000_000_000_000u128;
        let fee = 1_000_000u64;
        let mut asked = Vec::new();
        let opening = sweep_opening(held);
        let got = sweep_amount(held, fee, 7, (opening, limit(opening).unwrap()), |v| {
            asked.push(v);
            limit(v)
        });
        let value = held - 306_900 * u128::from(fee) - 7;
        assert_eq!(got, Ok(Amount::Send { value, limit: 306_900 }), "the gas of something, for something");
        assert_eq!(asked, vec![value], "the amount sent is the amount asked");
        assert!(opening < held && opening > 0, "the first ask is never everything held");
        let flat = |_: u128| -> Result<u64, ()> { Ok(31_500) };
        assert_eq!(sweep_amount(held, fee, 0, (opening, 31_500), flat), Ok(Amount::Send { value: held - 31_500 * u128::from(fee), limit: 31_500 }));
        let less = |v: u128| -> Result<u64, ()> { Ok(if v == opening { 40_000 } else { 31_500 }) };
        assert_eq!(sweep_amount(held, fee, 0, (opening, 40_000), less), Ok(Amount::Send { value: held - 40_000 * u128::from(fee), limit: 31_500 }));
        assert_eq!(sweep_amount(1_000, fee, 0, (500, 31_500), flat), Ok(Amount::Short { cost: 31_500 * u128::from(fee) }));
        assert_eq!(sweep_opening(1), 1);
        assert_eq!(sweep_opening(0), 1);
        let mut n = 0;
        let grows = |_: u128| -> Result<u64, ()> {
            n += 1;
            Ok(31_500 + 1_000 * n)
        };
        assert_eq!(sweep_amount(held, fee, 0, (opening, 31_500), grows), Ok(Amount::Unsettled));
        assert_eq!(n as usize, SWEEP_ASKS - 1, "asked SWEEP_ASKS times with the opening");
        assert_eq!(sweep_amount(held, fee, 0, (opening, 31_500), |_| Err::<u64, &str>("refused")), Err("refused"));
        let gone = Reverted { chain: 7, step: "sweep", tx: "0x01".into(), used: Some(31_500), limit: 31_500 };
        assert!(gone.out_of_gas());
        assert!(!Reverted { used: Some(21_000), ..gone.clone() }.out_of_gas());
        assert!(!Reverted { used: None, ..gone }.out_of_gas());
    }
}
