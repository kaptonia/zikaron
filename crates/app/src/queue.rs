use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
pub use zikaron_seal::queue::{member, state, Block, Place, Pushed, Queue, Queued, Shape, Step, FILE, RESENDS_MAX};

pub fn place_in<'a>(rows: Option<&'a [crate::ledgerx::Row]>, seats: &'a std::collections::BTreeMap<String, u64>) -> impl Fn(&str) -> Option<Place> + 'a {
    move |id: &str| {
        let r = rows?.iter().find(|r| r.id.eq_ignore_ascii_case(id))?;
        let carried_on = (r.kind == zikaron_glue::book::Kind::Face).then(|| seats.get(&r.id.to_ascii_lowercase()).copied()).flatten();
        Some(Place { law: r.law, seq: r.seq, alone: r.kind.goes_alone(), carried_on })
    }
}

pub trait Kept: Sized {
    fn read(home: &Home) -> Result<Self, Fault>;
    fn write(&self, home: &Home) -> Result<(), Fault>;
}

impl Kept for Queue {
    fn read(home: &Home) -> Result<Queue, Fault> {
        crate::reads::touch();
        let p = home.dir(Slot::Settings).join(FILE);
        let chain = crate::settings::Settings::read(home)?.chain_id.unwrap_or(0);
        let Some(bytes) = crate::local::read(&p, crate::local::Doc::Queue)? else {
            return Ok(Queue { chain, ..Queue::default() });
        };
        Queue::parse(&bytes, chain).map_err(|s| unparsed(&p, s))
    }

    fn write(&self, home: &Home) -> Result<(), Fault> {
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Queue, &self.to_bytes())?;
        crate::reads::wrote(crate::reads::Copies::Queue);
        Ok(())
    }
}

pub fn unparsed(p: &std::path::Path, s: Shape) -> Fault {
    let at = p.display().to_string();
    Fault::known(
        Known::QueueShape,
        crate::lang::both_of(|| match &s {
            Shape::Json(t) => format!("{at}: {t}"),
            Shape::NoQueued => crate::lang::filln(crate::lang::Key::Tail200, &[&at]),
            Shape::BadRow => crate::lang::filln(crate::lang::Key::Tail201, &[&at]),
            Shape::Missing(k) => format!("{at}: {k}"),
        }),
    )
}

pub use zikaron_send::keep::Inclusion;

pub fn amend<T>(home: &Home, f: impl FnOnce(&mut Queue) -> T) -> Result<(T, Queue), Fault> {
    zikaron_send::keep::amend(&crate::sendx::Desk::of(home), f)
}

pub fn settle(home: &Home, ids: &[String], anchored: bool, at: Option<&Inclusion>) -> Result<(usize, Queue), Fault> {
    zikaron_send::keep::settle(&crate::sendx::Desk::of(home), ids, anchored, at)
}

pub fn follow(home: &Home, on_chain: &[(String, u64)], owed: &[(String, bool, Option<u64>)], others: &[u64], at: u64) -> Result<Queue, Fault> {
    zikaron_send::keep::follow(&crate::sendx::Desk::of(home), on_chain, owed, others, at)
}

pub use zikaron_send::keep::Owed;

pub fn uncovered(q: &Queue, pile: &[Vec<u8>], published: &[String]) -> Vec<String> {
    zikaron_send::keep::uncovered(q, pile, published)
}

pub fn caught_up(home: &Home, chain: u64, others: &[u64], at: u64, published: &[String], owed: Owed) -> Result<Queue, Fault> {
    zikaron_send::keep::caught_up(&crate::sendx::Desk::of(home), chain, others, at, published, |_| Some(owed))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastBatch {
    pub tx: String,
    pub chain: u64,
    pub stands: Stands,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stands {
    Awaited(usize),
    Included(usize),
    Ended { confirmed: bool, sent: usize, dropped: usize },
}

pub fn last_batch(q: &Queue, sent: Option<&crate::task::Done>) -> Option<LastBatch> {
    if let Some((txs, chain, ids)) = q.submitted().pop() {
        return txs.last().map(|tx| LastBatch { tx: tx.clone(), chain, stands: Stands::Awaited(ids.len()) });
    }
    if let Some(crate::task::Done::Anchored { tx, chain, confirmed, sent, dropped, .. }) = sent {
        return Some(LastBatch { tx: tx.clone(), chain: *chain, stands: Stands::Ended { confirmed: *confirmed, sent: *sent, dropped: *dropped } });
    }
    let top = q.blocks.iter().max_by_key(|b| b.block)?;
    let n = q.blocks.iter().filter(|b| b.tx == top.tx && b.chain == top.chain).count();
    Some(LastBatch { tx: top.tx.clone(), chain: top.chain, stands: Stands::Included(n) })
}

pub struct Density {
    pub anchored_through: Option<u64>,
    pub head_seq: Option<u64>,
    pub behind: usize,
    through: [Option<u64>; 2],
}

impl Density {
    pub fn covers(&self, r: &crate::ledgerx::Row) -> bool {
        self.through[segment(r)].is_some_and(|n| r.seq <= n)
    }
}

fn segment(r: &crate::ledgerx::Row) -> usize {
    match r.law {
        zikaron_glue::book::Law::One => 0,
        zikaron_glue::book::Law::Two => 1,
    }
}

pub fn density(rows: Option<&[crate::ledgerx::Row]>, queued: usize) -> Density {
    let Some(rows) = rows else {
        return Density { anchored_through: None, head_seq: None, behind: queued, through: [None, None] };
    };
    let mut through = [None, None];
    for r in rows.iter().filter(|r| r.lamp == crate::ledgerx::Lamp::Anchored) {
        let at = &mut through[segment(r)];
        *at = Some(at.map_or(r.seq, |n: u64| n.max(r.seq)));
    }
    let newest = rows.iter().map(segment).max().unwrap_or(1);
    let owed = |r: &&crate::ledgerx::Row| !r.lamp.stays_off();
    let d = Density { anchored_through: through[newest], head_seq: rows.iter().filter(|r| segment(r) == newest).map(|r| r.seq).max(), behind: 0, through };
    let behind = rows.iter().filter(|r| !d.covers(r)).filter(owed).count();
    Density { behind, ..d }
}
