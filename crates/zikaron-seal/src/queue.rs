use crate::trouble::{Code, Trouble};
use zikaron::json::{self, Value};

pub const FILE: &str = "queue.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Queued,
    Submitted { tx: String, chain: u64, nonce: Option<u64> },
    Reverted { tx: String, chain: u64 },
    Refused { said: String },
    Resent { txs: Vec<String>, chain: u64, nonce: Option<u64> },
}

pub const RESENDS_MAX: usize = 3;

impl Step {
    pub fn as_str(&self) -> &'static str {
        match self {
            Step::Queued => state::QUEUED,
            Step::Submitted { .. } => state::SUBMITTED,
            Step::Reverted { .. } => state::REVERTED,
            Step::Refused { .. } => state::REFUSED,
            Step::Resent { .. } => state::RESENT,
        }
    }

    pub fn in_flight(&self) -> bool {
        matches!(self, Step::Submitted { .. } | Step::Resent { .. })
    }

    pub fn awaited(&self) -> Option<(Vec<String>, u64)> {
        match self {
            Step::Submitted { tx, chain, .. } => Some((vec![tx.clone()], *chain)),
            Step::Resent { txs, chain, .. } => Some((txs.clone(), *chain)),
            _ => None,
        }
    }

    pub fn nonce(&self) -> Option<u64> {
        match self {
            Step::Submitted { nonce, .. } | Step::Resent { nonce, .. } => *nonce,
            _ => None,
        }
    }
}

pub mod state {
    pub const QUEUED: &str = "queued";
    pub const SUBMITTED: &str = "submitted";
    pub const REVERTED: &str = "reverted";
    pub const REFUSED: &str = "refused";
    pub const RESENT: &str = "resent";
}

pub mod member {
    pub const QUEUED: &str = "queued";
    pub const ANCHORED: &str = "anchored";
    pub const BLOCKS: &str = "blocks";
    pub const ID: &str = "id";
    pub const AT: &str = "at";
    pub const STATE: &str = "state";
    pub const TX: &str = "tx";
    pub const CHAIN: &str = "chain";
    pub const SAID: &str = "said";
    pub const BLOCK: &str = "block";
    pub const TXS: &str = "txs";
    pub const NONCE: &str = "nonce";
    pub const ON: &str = "on";
    pub const PASSED: &str = "passed";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub law: zikaron_glue::book::Law,
    pub seq: u64,
    pub alone: bool,
    pub carried_on: Option<u64>,
}

impl Place {
    pub fn of(e: &zikaron_glue::book::Entry) -> Place {
        Place { law: e.law, seq: e.seq, alone: e.kind.goes_alone(), carried_on: None }
    }

    pub fn of_pile(items: &[Vec<u8>]) -> std::collections::BTreeMap<String, Place> {
        let entries: Vec<zikaron_glue::book::Entry> = items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).collect();
        let seats = zikaron_glue::face::seats_of(&entries);
        entries
            .iter()
            .map(|e| {
                let id = e.id_hex().to_ascii_lowercase();
                let carried_on = seats.get(&id).copied();
                (id, Place { carried_on, ..Place::of(e) })
            })
            .collect()
    }

    pub fn carried(&self, chain: u64) -> bool {
        self.carried_on == Some(chain)
    }
}

pub fn places_of(items: &[Vec<u8>]) -> std::collections::BTreeMap<String, Place> {
    Place::of_pile(items)
}

pub fn at_boundary(ids: &[String], places: &std::collections::BTreeMap<String, Place>) -> bool {
    let mine: Vec<Place> = ids.iter().filter_map(|id| places.get(&id.to_ascii_lowercase()).copied()).collect();
    if mine.len() < ids.len() || mine.iter().any(|p| p.alone) {
        return true;
    }
    let Some(first) = mine.iter().min_by_key(|p| (p.law, p.seq)) else { return false };
    places.values().filter(|p| (p.law, p.seq) < (first.law, first.seq)).max_by_key(|p| (p.law, p.seq)).is_some_and(|p| p.alone)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    pub id: String,
    pub at: u64,
    pub step: Step,
    pub chain: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub id: String,
    pub tx: String,
    pub chain: u64,
    pub block: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    pub chain: u64,
    pub items: Vec<Queued>,
    pub anchored: Vec<(String, u64)>,
    pub blocks: Vec<Block>,
    pub passed: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Retracting {
    pub publish: bool,
    pub drop: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pushed {
    Queued,
    InQueue,
    Anchored,
}

impl Pushed {
    pub fn as_str(self) -> &'static str {
        match self {
            Pushed::Queued => "queued",
            Pushed::InQueue => "in-queue",
            Pushed::Anchored => "anchored",
        }
    }

    pub fn landed(self) -> bool {
        matches!(self, Pushed::Queued)
    }
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Shape {
    Json(String),
    NoQueued,
    BadRow,
    Missing(&'static str),
}

fn on(row: u64, c: u64) -> bool {
    row == c || row == 0 || c == 0
}

impl Queue {
    pub fn parse(bytes: &[u8], home: u64) -> Result<Queue, Shape> {
        let v = json::parse(bytes).map_err(|t| Shape::Json(format!("{t:?}")))?;
        let Some(Value::Arr(rows)) = member(&v, member::QUEUED) else {
            return Err(Shape::NoQueued);
        };
        let mut items = Vec::new();
        for r in rows {
            let id = match member(r, member::ID) {
                Some(Value::Str(s)) if zikaron::hexfmt::is_hex32(s) => s.clone(),
                _ => {
                    return Err(Shape::BadRow)
                }
            };
            let bad = || Shape::BadRow;
            let at = match member(r, member::AT) {
                Some(Value::Int(n)) => *n,
                _ => return Err(bad()),
            };
            let text = |k: &str| match member(r, k) {
                Some(Value::Str(s)) => Some(s.clone()),
                _ => None,
            };
            let chain = match member(r, member::CHAIN) {
                Some(Value::Int(n)) => Some(*n),
                _ => None,
            };
            let nonce = match member(r, member::NONCE) {
                Some(Value::Int(n)) => Ok(Some(*n)),
                _ => Err(()),
            };
            let step = match text(member::STATE).as_deref() {
                Some(state::QUEUED) => Step::Queued,
                Some(state::SUBMITTED) => match (text(member::TX), chain, nonce) {
                    (Some(tx), Some(chain), Ok(nonce)) if zikaron::hexfmt::is_hex32(&tx) => Step::Submitted { tx, chain, nonce },
                    _ => return Err(bad()),
                },
                Some(state::REVERTED) => match (text(member::TX), chain) {
                    (Some(tx), Some(chain)) if zikaron::hexfmt::is_hex32(&tx) => Step::Reverted { tx, chain },
                    _ => return Err(bad()),
                },
                Some(state::REFUSED) => Step::Refused { said: text(member::SAID).ok_or_else(bad)? },
                Some(state::RESENT) => {
                    let txs: Option<Vec<String>> = match member(r, member::TXS) {
                        Some(Value::Arr(xs)) => xs.iter().map(|x| match x {
                            Value::Str(t) if zikaron::hexfmt::is_hex32(t) => Some(t.clone()),
                            _ => None,
                        }).collect(),
                        _ => None,
                    };
                    match (txs, chain, nonce) {
                        (Some(txs), Some(chain), Ok(nonce)) if (2..=1 + RESENDS_MAX).contains(&txs.len()) => Step::Resent { txs, chain, nonce },
                        _ => return Err(bad()),
                    }
                }
                _ => return Err(bad()),
            };
            let on = match member(r, member::ON) {
                None => home,
                Some(Value::Int(0)) => home,
                Some(Value::Int(n)) => *n,
                Some(_) => return Err(bad()),
            };
            items.push(Queued { id, at, step, chain: on });
        }
        let missing = Shape::Missing;
        let mut anchored: Vec<(String, u64)> = Vec::new();
        let Some(Value::Arr(rows)) = member(&v, member::ANCHORED) else { return Err(missing(member::ANCHORED)) };
        {
            for r in rows {
                match r {
                    Value::Str(x) if zikaron::hexfmt::is_hex32(x) => anchored.push((x.clone(), home)),
                    Value::Obj(_) => match (member(r, member::ID), member(r, member::ON)) {
                        (Some(Value::Str(x)), Some(Value::Int(c))) if zikaron::hexfmt::is_hex32(x) => anchored.push((x.clone(), if *c == 0 { home } else { *c })),
                        _ => return Err(Shape::BadRow),
                    },
                    _ => {
                        return Err(Shape::BadRow)
                    }
                }
            }
        }
        let mut blocks: Vec<Block> = Vec::new();
        let Some(Value::Arr(rows)) = member(&v, member::BLOCKS) else { return Err(missing(member::BLOCKS)) };
        {
            for r in rows {
                let (Some(Value::Str(id)), Some(Value::Str(tx)), Some(Value::Int(chain)), Some(Value::Int(block))) =
                    (member(r, member::ID), member(r, member::TX), member(r, member::CHAIN), member(r, member::BLOCK))
                else {
                    return Err(Shape::BadRow);
                };
                blocks.push(Block { id: id.clone(), tx: tx.clone(), chain: *chain, block: *block });
            }
        }
        let mut passed: Vec<String> = Vec::new();
        match member(&v, member::PASSED) {
            None => {}
            Some(Value::Arr(rows)) => {
                for r in rows {
                    match r {
                        Value::Str(x) if zikaron::hexfmt::is_hex32(x) => passed.push(x.clone()),
                        _ => return Err(Shape::BadRow),
                    }
                }
            }
            Some(_) => return Err(Shape::BadRow),
        }
        Ok(Queue { chain: home, items, anchored, blocks, passed })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let rows: Vec<Value> = self
            .items
            .iter()
            .map(|q| {
                let mut m = vec![
                    (member::AT.to_string(), Value::Int(q.at)),
                    (member::ID.to_string(), Value::Str(q.id.clone())),
                    (member::ON.to_string(), Value::Int(q.chain)),
                    (member::STATE.to_string(), Value::Str(q.step.as_str().to_string())),
                ];
                match &q.step {
                    Step::Queued => {}
                    Step::Submitted { tx, chain, .. } | Step::Reverted { tx, chain } => {
                        m.push((member::CHAIN.to_string(), Value::Int(*chain)));
                        m.push((member::TX.to_string(), Value::Str(tx.clone())));
                    }
                    Step::Refused { said } => m.push((member::SAID.to_string(), Value::Str(said.clone()))),
                    Step::Resent { txs, chain, .. } => {
                        m.push((member::CHAIN.to_string(), Value::Int(*chain)));
                        m.push((member::TXS.to_string(), Value::Arr(txs.iter().map(|t| Value::Str(t.clone())).collect())));
                    }
                }
                if let Some(n) = q.step.nonce() {
                    m.push((member::NONCE.to_string(), Value::Int(n)));
                }
                m.sort_by(|a, b| a.0.cmp(&b.0));
                Value::Obj(m)
            })
            .collect();
        let anchored: Vec<Value> =
            self.anchored.iter().map(|(x, c)| Value::Obj(vec![(member::ID.to_string(), Value::Str(x.clone())), (member::ON.to_string(), Value::Int(*c))])).collect();
        let blocks: Vec<Value> = self
            .blocks
            .iter()
            .map(|b| {
                Value::Obj(vec![
                    (member::BLOCK.to_string(), Value::Int(b.block)),
                    (member::CHAIN.to_string(), Value::Int(b.chain)),
                    (member::ID.to_string(), Value::Str(b.id.clone())),
                    (member::TX.to_string(), Value::Str(b.tx.clone())),
                ])
            })
            .collect();
        let mut members = vec![
            (member::ANCHORED.to_string(), Value::Arr(anchored)),
            (member::BLOCKS.to_string(), Value::Arr(blocks)),
            (member::QUEUED.to_string(), Value::Arr(rows)),
        ];
        if !self.passed.is_empty() {
            members.push((member::PASSED.to_string(), Value::Arr(self.passed.iter().map(|x| Value::Str(x.clone())).collect())));
        }
        json::canon_bytes(&Value::Obj(members))
    }

    pub fn push(&mut self, id: &str, at: u64) -> Pushed {
        self.push_on(id, self.chain, at)
    }

    pub fn push_on(&mut self, id: &str, chain: u64, at: u64) -> Pushed {
        if self.anchored_on(id, chain) {
            return Pushed::Anchored;
        }
        if self.items.iter().any(|q| q.id == id && on(q.chain, chain)) {
            return Pushed::InQueue;
        }
        self.items.push(Queued { id: id.to_string(), at, step: Step::Queued, chain });
        Pushed::Queued
    }

    pub fn push_placed(&mut self, id: &str, alone: bool, carried_on: Option<u64>, others: &[u64], at: u64) -> Pushed {
        match carried_on {
            Some(chain) => self.push_on(id, chain, at),
            None => self.push_entry(id, alone, others, at),
        }
    }

    pub fn push_entry(&mut self, id: &str, alone: bool, others: &[u64], at: u64) -> Pushed {
        let here = self.push(id, at);
        if !alone {
            return here;
        }
        let mut took = false;
        let here_chain = self.chain;
        for c in others.iter().filter(|c| **c != here_chain) {
            took |= self.push_on(id, *c, at).landed();
        }
        if took && !here.landed() {
            Pushed::Queued
        } else {
            here
        }
    }

    pub fn retracting(&self, target: &str, last: Option<&[String]>, can_anchor: bool) -> Option<Retracting> {
        let publish = self.published(target, last.unwrap_or_default());
        if can_anchor && !publish && last.is_none() && !self.has(target) {
            return None;
        }
        Some(Retracting { publish, drop: !self.step_of(target).is_some_and(Step::in_flight) })
    }

    pub fn retracted(&mut self, target: &str, delete: &str, r: Retracting) -> usize {
        let dropped = if r.drop { self.drop_ids(&[target.to_string()]) } else { 0 };
        if !r.publish {
            self.drop_ids(&[delete.to_string()]);
        }
        dropped
    }

    pub fn drop_ids(&mut self, ids: &[String]) -> usize {
        let before = self.items.len();
        self.items.retain(|q| !ids.iter().any(|x| x == &q.id));
        before - self.items.len()
    }

    pub fn anchored_out(&mut self, ids: &[String]) -> usize {
        self.anchored_out_on(ids, self.chain)
    }

    pub fn anchored_out_on(&mut self, ids: &[String], chain: u64) -> usize {
        let before = self.items.len();
        self.items.retain(|q| !(on(q.chain, chain) && ids.iter().any(|x| x == &q.id)));
        for id in ids {
            if !self.anchored_on(id, chain) {
                self.anchored.push((id.clone(), chain));
            }
        }
        before - self.items.len()
    }

    pub fn seen_out(&mut self, ids: &[String]) -> usize {
        self.seen_out_on(ids, self.chain)
    }

    pub fn seen_out_on(&mut self, ids: &[String], chain: u64) -> usize {
        let not_out: Vec<String> = ids.iter().filter(|id| !self.step_on(id, chain).is_some_and(Step::in_flight)).cloned().collect();
        self.anchored_out_on(&not_out, chain)
    }

    pub fn mark(&mut self, ids: &[String], step: Step) -> usize {
        self.mark_on(ids, self.chain, step)
    }

    pub fn mark_on(&mut self, ids: &[String], chain: u64, step: Step) -> usize {
        let mut n = 0;
        for q in self.items.iter_mut() {
            if on(q.chain, chain) && ids.iter().any(|x| x == &q.id) && !q.step.in_flight() {
                q.step = step.clone();
                n += 1;
            }
        }
        n
    }

    pub fn moved_by(&mut self, ids: &[String], txs: &[String], chain: u64, step: Step) -> usize {
        let mut n = 0;
        for q in self.items.iter_mut() {
            if on(q.chain, chain) && ids.iter().any(|x| x == &q.id) && q.step.awaited().is_some_and(|(t, c)| t == txs && c == chain) {
                q.step = step.clone();
                n += 1;
            }
        }
        n
    }

    pub fn included_out(&mut self, ids: &[String], tx: &str, chain: u64, block: u64) -> usize {
        let n = self.anchored_out_on(ids, chain);
        for id in ids {
            self.blocks.retain(|b| !(&b.id == id && b.chain == chain));
            self.blocks.push(Block { id: id.clone(), tx: tx.to_string(), chain, block });
        }
        n
    }

    pub fn step_of(&self, id: &str) -> Option<&Step> {
        self.step_on(id, self.chain)
    }

    pub fn step_on(&self, id: &str, chain: u64) -> Option<&Step> {
        self.items.iter().find(|q| q.id == id && on(q.chain, chain)).map(|q| &q.step)
    }

    pub fn block_of(&self, id: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id && b.chain == self.chain).or_else(|| self.blocks.iter().find(|b| b.id == id))
    }

    pub fn submitted(&self) -> Vec<(Vec<String>, u64, Vec<String>)> {
        let mut out: Vec<(Vec<String>, u64, Vec<String>)> = Vec::new();
        for q in &self.items {
            if let Some((txs, chain)) = q.step.awaited() {
                match out.iter_mut().find(|(t, c, _)| *t == txs && *c == chain) {
                    Some((_, _, ids)) => ids.push(q.id.clone()),
                    None => out.push((txs, chain, vec![q.id.clone()])),
                }
            }
        }
        out
    }

    pub fn published(&self, id: &str, report_anchored: &[String]) -> bool {
        self.items.iter().any(|q| q.id == id && q.step.in_flight())
            || self.blocks.iter().any(|b| b.id == id)
            || self.anchored.iter().any(|(x, _)| x == id)
            || report_anchored.iter().any(|x| x == id)
    }

    pub fn anchored_here(&self, id: &str) -> bool {
        self.anchored_on(id, self.chain)
    }

    pub fn anchored_on(&self, id: &str, chain: u64) -> bool {
        self.anchored.iter().any(|(x, c)| x == id && on(*c, chain))
    }

    pub fn anchored_ids(&self) -> Vec<String> {
        self.anchored.iter().filter(|(_, c)| on(*c, self.chain)).map(|(x, _)| x.clone()).collect()
    }

    pub fn sent_ids(&self) -> Vec<String> {
        self.items.iter().filter(|q| q.step.in_flight()).map(|q| q.id.clone()).chain(self.anchored.iter().map(|(x, _)| x.clone())).collect()
    }

    pub fn has(&self, id: &str) -> bool {
        self.items.iter().any(|q| q.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn take_ids(&self, n: usize, place: impl Fn(&str) -> Option<Place>) -> Vec<String> {
        match self.next_batch(n, place) {
            Some((chain, ids)) if chain == self.chain => ids,
            _ => Vec::new(),
        }
    }

    pub fn next_batch(&self, n: usize, place: impl Fn(&str) -> Option<Place>) -> Option<(u64, Vec<String>)> {
        let lane_of = |q: &Queued| if q.chain == 0 { self.chain } else { q.chain };
        let carried = |q: &Queued| place(&q.id).is_some_and(|p| p.carried(lane_of(q)));
        if let Some(b) = self.batch_of(n, &place, |q| !carried(q)) {
            return Some(b);
        }
        let mut faces: Vec<(&Queued, Place)> = self.items.iter().filter(|q| carried(q) && !q.step.in_flight()).filter_map(|q| place(&q.id).map(|p| (q, p))).collect();
        faces.sort_by(|(a, pa), (b, pb)| (pa.law, pa.seq, &a.id).cmp(&(pb.law, pb.seq, &b.id)));
        faces.first().map(|(q, _)| (lane_of(q), vec![q.id.clone()]))
    }

    fn batch_of(&self, n: usize, place: &impl Fn(&str) -> Option<Place>, keep: impl Fn(&Queued) -> bool) -> Option<(u64, Vec<String>)> {
        let key = |q: &Queued, p: &Option<Place>| match p {
            Some(x) => (0u8, x.law, x.seq, 0u64, q.id.clone()),
            None => (1u8, zikaron_glue::book::Law::Two, 0, q.at, q.id.clone()),
        };
        let mut order: Vec<(&Queued, Option<Place>)> = self.items.iter().filter(|q| keep(q)).map(|q| (q, place(&q.id))).collect();
        order.sort_by(|(a, pa), (b, pb)| key(a, pa).cmp(&key(b, pb)).then(((a.chain != self.chain), a.chain).cmp(&((b.chain != self.chain), b.chain))));
        let (first, _) = order.first()?;
        let first_key = key(first, &place(&first.id));
        let lane_of = |q: &Queued| if q.chain == 0 { self.chain } else { q.chain };
        let mut lanes: Vec<u64> = order.iter().filter(|(q, p)| key(q, p) == first_key).map(|(q, _)| lane_of(q)).collect();
        lanes.dedup();
        for lane in lanes {
            let mut run: Vec<&Queued> = Vec::new();
            for (q, p) in order.iter() {
                let alone = p.is_none_or(|p| p.alone);
                if lane_of(q) != lane {
                    if key(q, p) != first_key {
                        break;
                    }
                    continue;
                }
                if alone && !run.is_empty() {
                    break;
                }
                run.push(q);
                if alone {
                    break;
                }
            }
            let ids: Vec<String> = run.into_iter().filter(|q| !q.step.in_flight()).take(n).map(|q| q.id.clone()).collect();
            if !ids.is_empty() {
                return Some((lane, ids));
            }
        }
        None
    }

    pub fn sendable(&self) -> usize {
        self.items.iter().filter(|q| !q.step.in_flight()).count()
    }

    pub fn hashes(ids: &[String]) -> Result<Vec<[u8; 32]>, Trouble> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let b = zikaron::hexfmt::decode(id)
                .ok_or_else(|| Trouble::new(Code::ContentShape, id.clone()))?;
            if b.len() != 32 {
                return Err(Trouble::new(Code::ContentShape, id.clone()));
            }
            let mut h = [0u8; 32];
            h.copy_from_slice(&b);
            out.push(h);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod seen_tests {
    use super::*;

    #[test]
    fn a_batch_never_crosses_a_succession() {
        use zikaron_glue::book::Law;
        let ids = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let place = |id: &str| match id {
            "old1" => Some(Place { law: Law::One, seq: 1, alone: false, carried_on: None }),
            "old2" => Some(Place { law: Law::One, seq: 2, alone: false, carried_on: None }),
            "close" => Some(Place { law: Law::One, seq: 3, alone: true, carried_on: None }),
            "gen" => Some(Place { law: Law::Two, seq: 0, alone: false, carried_on: None }),
            "note" => Some(Place { law: Law::Two, seq: 1, alone: false, carried_on: None }),
            _ => None,
        };
        let mut q = Queue::default();
        for (i, id) in ["gen", "stray", "close", "old2", "note", "old1"].iter().enumerate() {
            let _ = q.push(id, i as u64);
        }
        assert_eq!(q.take_ids(10, place), ids(&["old1", "old2"]));
        assert_eq!(q.take_ids(1, place), ids(&["old1"]));
        q.mark(&ids(&["old1"]), Step::Submitted { tx: "0x01".into(), chain: 1, nonce: Some(0) });
        assert_eq!(q.take_ids(10, place), ids(&["old2"]), "the rest of the batch out may go");
        q.mark(&ids(&["old2"]), Step::Submitted { tx: "0x02".into(), chain: 1, nonce: Some(1) });
        assert!(q.take_ids(10, place).is_empty(), "nothing after a batch still out");
        q.anchored_out(&ids(&["old1", "old2"]));
        assert_eq!(q.take_ids(10, place), ids(&["close"]), "the succession alone");
        q.anchored_out(&ids(&["close"]));
        assert_eq!(q.take_ids(10, place), ids(&["gen", "note"]));
        q.anchored_out(&ids(&["gen", "note"]));
        assert_eq!(q.take_ids(10, place), ids(&["stray"]), "unplaced: alone, last");
        let all: std::collections::BTreeMap<String, Place> = ["old1", "old2", "close", "gen", "note"].iter().filter_map(|x| place(x).map(|p| (x.to_string(), p))).collect();
        assert!(!at_boundary(&ids(&["old1", "old2"]), &all));
        assert!(at_boundary(&ids(&["close"]), &all));
        assert!(at_boundary(&ids(&["gen"]), &all));
        assert!(!at_boundary(&ids(&["note"]), &all));
        assert!(at_boundary(&ids(&["stray"]), &all), "not placed: held");
    }

    #[test]
    fn a_succession_goes_on_every_chain_before_anything_after_it() {
        use zikaron_glue::book::Law;
        let hex = |x: &str| match x {
            "e1" => format!("0x{}", "e1".repeat(32)),
            "s" => format!("0x{}", "5a".repeat(32)),
            _ => format!("0x{}", "e3".repeat(32)),
        };
        let ids = |xs: &[&str]| xs.iter().map(|x| hex(x)).collect::<Vec<_>>();
        let place = |id: &str| match id {
            x if x == hex("e1") => Some(Place { law: Law::Two, seq: 1, alone: false, carried_on: None }),
            x if x == hex("s") => Some(Place { law: Law::Two, seq: 2, alone: true, carried_on: None }),
            x if x == hex("e3") => Some(Place { law: Law::Two, seq: 3, alone: false, carried_on: None }),
            _ => None,
        };
        let (a, b) = (11u64, 22u64);
        let mut q = Queue { chain: b, ..Queue::default() };
        let _ = q.push(&hex("e3"), 3);
        let _ = q.push(&hex("s"), 2);
        let _ = q.push_on(&hex("s"), a, 2);
        let _ = q.push(&hex("e1"), 1);
        assert_eq!(q.next_batch(10, place), Some((b, ids(&["e1"]))));
        q.included_out(&ids(&["e1"]), "0x01", b, 5);
        assert_eq!(q.next_batch(10, place), Some((b, ids(&["s"]))), "the anchoring chain first");
        q.mark_on(&ids(&["s"]), b, Step::Submitted { tx: "0x02".into(), chain: b, nonce: Some(1) });
        assert_eq!(q.next_batch(10, place), Some((a, ids(&["s"]))), "the other chain's row of the same entry");
        q.included_out(&ids(&["s"]), "0x02", b, 6);
        assert_eq!(q.next_batch(10, place), Some((a, ids(&["s"]))), "e3 waits for every chain");
        q.included_out(&ids(&["s"]), "0x03", a, 9);
        assert_eq!(q.next_batch(10, place), Some((b, ids(&["e3"]))));
        assert!(q.anchored_on(&hex("s"), a) && q.anchored_on(&hex("s"), b) && !q.anchored_on(&hex("e1"), a));
        assert_eq!(q.push_on(&hex("e1"), a, 7), Pushed::Queued, "anchored on one chain is no record on another");
        let back = Queue::parse(&q.to_bytes(), b).expect("reads");
        assert_eq!(back.items, q.items);
        assert_eq!(back.anchored, q.anchored);
        let old = br#"{"anchored":["0x1111111111111111111111111111111111111111111111111111111111111111"],"blocks":[],"queued":[{"at":1,"id":"0x2222222222222222222222222222222222222222222222222222222222222222","state":"queued"}]}"#;
        let old = Queue::parse(old, a).expect("an earlier file reads");
        assert_eq!(old.items[0].chain, a);
        assert_eq!(old.anchored[0].1, a);
    }

    #[test]
    fn a_face_entry_goes_alone_on_its_chain_and_cuts_no_batch() {
        use zikaron_glue::book::Law;
        let hex = |x: &str| format!("0x{}", x.repeat(32));
        let (a, h, c) = (11u64, 22u64, 33u64);
        let place = |id: &str| match id {
            x if x == hex("e1") => Some(Place { law: Law::Two, seq: 1, alone: false, carried_on: None }),
            x if x == hex("f2") => Some(Place { law: Law::Two, seq: 2, alone: false, carried_on: Some(h) }),
            x if x == hex("e3") => Some(Place { law: Law::Two, seq: 3, alone: false, carried_on: None }),
            _ => None,
        };
        let mut q = Queue { chain: a, ..Queue::default() };
        let _ = q.push(&hex("e1"), 1);
        let _ = q.push_on(&hex("f2"), h, 2);
        let _ = q.push(&hex("e3"), 3);
        assert_eq!(q.next_batch(10, place), Some((a, vec![hex("e1"), hex("e3")])), "the ordinary entries, one batch");
        q.included_out(&[hex("e1"), hex("e3")], "0x01", a, 5);
        assert_eq!(q.next_batch(10, place), Some((h, vec![hex("f2")])), "then the face entry alone, on its chain");
        assert!(place(&hex("f2")).is_some_and(|p| p.carried(h) && !p.carried(c)));
        let mut moved = Queue { chain: c, ..Queue::default() };
        for (id, at) in [("e1", 1), ("f2", 2), ("e3", 3)] {
            let _ = moved.push_on(&hex(id), c, at);
        }
        assert_eq!(moved.next_batch(10, place), Some((c, vec![hex("e1"), hex("f2"), hex("e3")])), "as a hash on another chain");
    }

    #[test]
    fn an_entry_is_queued_where_its_place_puts_it() {
        use zikaron_glue::book::Law;
        let hex = |x: &str| format!("0x{}", x.repeat(32));
        let (a, h) = (11u64, 22u64);
        let mut q = Queue { chain: a, ..Queue::default() };
        let face = Place { law: Law::Two, seq: 2, alone: false, carried_on: Some(h) };
        let succession = Place { law: Law::Two, seq: 3, alone: true, carried_on: None };
        for (id, p, at) in [("f2", face, 1), ("c3", succession, 2)] {
            let _ = q.push_placed(&hex(id), p.alone, p.carried_on, &[h], at);
        }
        let _ = q.push_placed(&hex("e4"), false, None, &[h], 3);
        let at = |id: &str| -> Vec<u64> { q.items.iter().filter(|x| x.id == hex(id)).map(|x| x.chain).collect() };
        assert_eq!(at("f2"), vec![h], "the face entry on its chain alone");
        assert_eq!(at("c3"), vec![a, h], "a succession on every chain of the basis");
        assert_eq!(at("e4"), vec![a], "an ordinary entry on the anchoring chain");
    }

    #[test]
    fn an_entry_seen_on_chain_leaves_unless_its_transaction_is_out() {
        let mut q = Queue::default();
        let _ = q.push("aa", 1);
        let _ = q.push("bb", 2);
        q.mark(&["bb".to_string()], Step::Submitted { tx: "0x01".into(), chain: 1, nonce: Some(0) });
        let gone = q.seen_out(&["aa".to_string(), "bb".to_string(), "cc".to_string()]);
        assert_eq!(gone, 1);
        assert!(!q.has("aa") && q.has("bb"), "{q:?}");
        assert_eq!(q.anchored, vec![("aa".to_string(), 0), ("cc".to_string(), 0)]);
        assert_eq!(q.push("cc", 3), Pushed::Anchored);
    }

    #[test]
    fn mark_leaves_a_batch_in_flight_to_the_one_who_sent_it() {
        let ids = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let sub = Step::Submitted { tx: "0x01".into(), chain: 1, nonce: Some(4) };
        let resent = Step::Resent { txs: vec!["0x02".into(), "0x03".into()], chain: 1, nonce: Some(5) };
        let mut q = Queue::default();
        for (i, id) in ["q", "f", "r", "s", "t"].iter().enumerate() {
            let _ = q.push(id, i as u64);
        }
        assert_eq!(q.mark(&ids(&["f"]), Step::Refused { said: "X".into() }), 1);
        assert_eq!(q.mark(&ids(&["r"]), Step::Reverted { tx: "0x09".into(), chain: 1 }), 1);
        assert_eq!(q.mark(&ids(&["s"]), sub.clone()), 1);
        assert_eq!(q.mark(&ids(&["t"]), resent.clone()), 1);
        let refused = Step::Refused { said: "Y".into() };
        assert_eq!(q.mark(&ids(&["q", "f", "r", "s", "t"]), refused.clone()), 3, "queued, refused and reverted change");
        assert_eq!(q.step_of("s"), Some(&sub), "a submitted batch is not overwritten");
        assert_eq!(q.step_of("t"), Some(&resent), "a resent batch is not overwritten");
        assert_eq!(q.moved_by(&ids(&["s"]), &ids(&["0x09"]), 1, Step::Queued), 0, "other transactions move nothing");
        assert_eq!(q.moved_by(&ids(&["s"]), &ids(&["0x01"]), 2, Step::Queued), 0, "another chain moves nothing");
        assert_eq!(q.moved_by(&ids(&["q"]), &ids(&["0x01"]), 1, Step::Queued), 0, "an entry with nothing out is not moved");
        assert_eq!(q.moved_by(&ids(&["s", "t"]), &ids(&["0x01"]), 1, Step::Queued), 1, "its own transactions move it");
        assert_eq!(q.step_of("s"), Some(&Step::Queued));
        assert_eq!(q.moved_by(&ids(&["t"]), &ids(&["0x02", "0x03"]), 1, sub.clone()), 1);
        assert_eq!(q.step_of("t"), Some(&sub));
    }
}

