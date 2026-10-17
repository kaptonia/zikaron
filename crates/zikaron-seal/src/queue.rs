//! The anchor queue's model, one for both ends: its rows and steps, its one way in ([`Queue::push`]) and its ways
//! out, and its bytes (canonical JSON) read and written once ([`Queue::parse`], [`Queue::to_bytes`]). Where the
//! file lives, its seal, the in-process lock and the words a refusal is said in are each end's (the desktop's
//! `queue`; the command line's sealed write, which only pushes).
//!
//! ─── No miscounting ───
//!
//! The risk is an entry sent but not removed, or removed without being sent. There is one way in
//! ([`Queue::push`]) and three ways out, each with its reason: a successful chain receipt goes through
//! [`Queue::anchored_out`] (removed and recorded as anchored; [`Queue::included_out`] also records the block),
//! called by the receipt path and by [`Queue::seen_out`] (an entry a self-audit read on chain while it still
//! waited here, another writer having anchored it); a deleted entry goes through [`Queue::drop_ids`] (removed,
//! nothing recorded). Anything that failed to send stays queued for retry because no other removal path
//! exists.
//!
//! ─── Only the receipt path records "anchored" ───
//!
//! The `anchored` list states a chain fact. If one exit recorded "anchored" for every caller, a retraction
//! would mark an entry that never reached the chain as anchored, and re-queueing it would wrongly answer
//! "already on chain". So only the exit that states a chain fact writes `anchored`, reached from the receipt path
//! and from a self-audit's reading.

use crate::trouble::{Code, Trouble};
use zikaron::json::{self, Value};

/// The queue file's name.
pub const FILE: &str = "queue.json";

/// A queued entry's sending state. Entries included in a block are recorded separately ([`Block`]). The
/// status light, entry card, record card, queue page and watch line all derive from this (the desktop's
/// `ledgerx::Lamp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Queued, not sent.
    Queued,
    /// Broadcast, and the node returned the matching transaction hash: waiting for the receipt. Saved to disk,
    /// so after a restart the same transaction is awaited, never resent. `nonce` is the batch's signed nonce,
    /// always written and required when read; it distinguishes a batch no node holds any more from one whose
    /// nonce another transaction used (the desktop's `action::confirm_batch`).
    Submitted { tx: String, chain: u64, nonce: Option<u64> },
    /// Included, but the receipt status is not 1 (law §9.1: not an anchor). Stays queued and can be
    /// resent.
    Reverted { tx: String, chain: u64 },
    /// Refused before broadcast (insufficient balance, node refusal, unreachable): never sent. Stays queued
    /// and can be resent.
    Refused { said: String },
    /// Broadcast, then resent by the user with higher fees at the same nonce. All the batch's transactions
    /// (oldest first) are awaited, and whichever is included counts (one nonce, so at most one can be). Saved
    /// to disk like `Submitted`: after a restart all are awaited, none resent. `nonce` as in `Submitted`.
    Resent { txs: Vec<String>, chain: u64, nonce: Option<u64> },
}

/// The most times one batch is resent with higher fees (its first transaction and at most this many more).
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

    /// Whether this entry's transaction is out and awaited (submitted or resent): such entries are never picked
    /// for another batch, never counted as sendable, and count as published.
    pub fn in_flight(&self) -> bool {
        matches!(self, Step::Submitted { .. } | Step::Resent { .. })
    }

    /// The transactions awaited for this entry (oldest first) and their chain; `None` when none is out.
    pub fn awaited(&self) -> Option<(Vec<String>, u64)> {
        match self {
            Step::Submitted { tx, chain, .. } => Some((vec![tx.clone()], *chain)),
            Step::Resent { txs, chain, .. } => Some((txs.clone(), *chain)),
            _ => None,
        }
    }

    /// The nonce the awaited transactions were signed with, when recorded.
    pub fn nonce(&self) -> Option<u64> {
        match self {
            Step::Submitted { nonce, .. } | Step::Resent { nonce, .. } => *nonce,
            _ => None,
        }
    }
}

/// Values of the `state` member in the queue file.
pub mod state {
    pub const QUEUED: &str = "queued";
    pub const SUBMITTED: &str = "submitted";
    pub const REVERTED: &str = "reverted";
    pub const REFUSED: &str = "refused";
    pub const RESENT: &str = "resent";
}

/// Member names in the queue file (top level, rows and blocks).
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
}

/// One queued entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    /// Entry id (hex32, law §2.1).
    pub id: String,
    /// When it was queued (only for ordering and display, never for decisions).
    pub at: u64,
    /// Its state.
    pub step: Step,
}

/// An included transaction (receipt status 1): entry, transaction, chain and block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub id: String,
    pub tx: String,
    pub chain: u64,
    pub block: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    pub items: Vec<Queued>,
    /// The entries this file records as anchored.
    ///
    /// Without it, "is it on chain" could only be answered by the queue (useless after removal) and the last
    /// self-audit report (absent after a restart). An already anchored entry could then be queued and anchored
    /// again, costing gas twice. Recording it here survives restarts and home copies.
    pub anchored: Vec<String>,
    /// Included transactions with chain and block number, recorded by the receipt path.
    pub blocks: Vec<Block>,
}

/// The result of queueing an entry; the caller reports each form with its own message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pushed {
    /// Queued.
    Queued,
    /// Already queued (each entry is queued once).
    InQueue,
    /// Already recorded as anchored in this file (this survives restarts).
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

    /// Whether it was actually queued by this call.
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

/// Why queue bytes are not this version's queue. Each end says it with the file's place.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Not canonical JSON (the core's trouble).
    Json(String),
    /// No `queued` table.
    NoQueued,
    /// A row, an anchored id or a block not in its shape.
    BadRow,
    /// A required top-level member missing (its name).
    Missing(&'static str),
}

impl Queue {
    /// Read queue bytes. Every member this version writes is required: bytes missing one (an earlier version's,
    /// or altered) are refused ([`Shape`]), never read with a member guessed.
    pub fn parse(bytes: &[u8]) -> Result<Queue, Shape> {
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
            // The batch's nonce, required where a batch was signed (submitted, resent).
            let nonce = match member(r, member::NONCE) {
                Some(Value::Int(n)) => Ok(Some(*n)),
                _ => Err(()),
            };
            // `state`: required; unknown or incomplete is a shape error, never guessed.
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
                // Resent: 2 to `1 + RESENDS_MAX` transaction hashes; anything else is a shape error.
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
            items.push(Queued { id, at, step });
        }
        // `anchored` and `blocks`: required (empty lists when there are none).
        let missing = Shape::Missing;
        let mut anchored: Vec<String> = Vec::new();
        let Some(Value::Arr(rows)) = member(&v, member::ANCHORED) else { return Err(missing(member::ANCHORED)) };
        {
            for r in rows {
                match r {
                    Value::Str(x) if zikaron::hexfmt::is_hex32(x) => anchored.push(x.clone()),
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
        Ok(Queue { items, anchored, blocks })
    }

    /// The queue's bytes: canonical JSON, members in order.
    pub fn to_bytes(&self) -> Vec<u8> {
        let rows: Vec<Value> = self
            .items
            .iter()
            .map(|q| {
                let mut m = vec![
                    (member::AT.to_string(), Value::Int(q.at)),
                    (member::ID.to_string(), Value::Str(q.id.clone())),
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
        let anchored: Vec<Value> = self.anchored.iter().map(|x| Value::Str(x.clone())).collect();
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
        json::canon_bytes(&Value::Obj(vec![
            (member::ANCHORED.to_string(), Value::Arr(anchored)),
            (member::BLOCKS.to_string(), Value::Arr(blocks)),
            (member::QUEUED.to_string(), Value::Arr(rows)),
        ]))
    }

    /// Queue an entry. Each entry is queued once, and an entry recorded as anchored can never be queued again.
    ///
    /// Every queueing path (automatic after recording, manual from the page) goes through here, so they all
    /// share one record of what is on chain.
    pub fn push(&mut self, id: &str, at: u64) -> Pushed {
        if self.anchored.iter().any(|x| x == id) {
            return Pushed::Anchored;
        }
        if self.items.iter().any(|q| q.id == id) {
            return Pushed::InQueue;
        }
        self.items.push(Queued { id: id.to_string(), at, step: Step::Queued });
        Pushed::Queued
    }

    /// Remove entries without recording anything. Used when an entry will no longer be anchored for a reason
    /// other than being anchored (currently only deletion), so `anchored` is not touched.
    pub fn drop_ids(&mut self, ids: &[String]) -> usize {
        let before = self.items.len();
        self.items.retain(|q| !ids.iter().any(|x| x == &q.id));
        before - self.items.len()
    }

    /// A chain fact: remove and record as anchored. Reached from the receipt path (the desktop's `queue::settle`)
    /// and from a self-audit's reading ([`Queue::seen_out`]).
    ///
    /// Removal and recording happen together; split apart, there would be a moment where the entry was removed
    /// but not recorded and could be queued again. `anchored` is written only here.
    pub fn anchored_out(&mut self, ids: &[String]) -> usize {
        let n = self.drop_ids(ids);
        for id in ids {
            if !self.anchored.iter().any(|x| x == id) {
                self.anchored.push(id.clone());
            }
        }
        n
    }

    /// Entries a self-audit read on chain (the registry's logs carry them): those with no transaction of theirs
    /// out leave through [`Queue::anchored_out`], removed and recorded as anchored (another writer, the command
    /// line, put them on chain); one whose transaction is out is left to its receipt. Only the desktop's
    /// `queue::follow` calls this.
    pub fn seen_out(&mut self, ids: &[String]) -> usize {
        let not_out: Vec<String> =
            ids.iter().filter(|id| !self.step_of(id).is_some_and(Step::in_flight)).cloned().collect();
        self.anchored_out(&not_out)
    }

    /// Set the state of these entries, only where no transaction of theirs is out: an entry submitted or resent
    /// (in flight, [`Step::in_flight`]) keeps its step, so no writer overwrites a batch another has out. Returns
    /// how many changed. An entry's in-flight step changes only by [`Queue::moved_by`].
    pub fn mark(&mut self, ids: &[String], step: Step) -> usize {
        let mut n = 0;
        for q in self.items.iter_mut() {
            if ids.iter().any(|x| x == &q.id) && !q.step.in_flight() {
                q.step = step.clone();
                n += 1;
            }
        }
        n
    }

    /// Move a batch out in flight on to its next step: only these entries whose step awaits exactly `txs` on
    /// `chain` (the batch's own transactions, held by whoever sent them) change; an entry waiting on other
    /// transactions, or none, is left as it is. Returns how many changed. The receipt wait (included with another
    /// status, voided), a resend (its new transaction joining) and a broadcast refused outright come here.
    pub fn moved_by(&mut self, ids: &[String], txs: &[String], chain: u64, step: Step) -> usize {
        let mut n = 0;
        for q in self.items.iter_mut() {
            if ids.iter().any(|x| x == &q.id) && q.step.awaited().is_some_and(|(t, c)| t == txs && c == chain) {
                q.step = step.clone();
                n += 1;
            }
        }
        n
    }

    /// Included with receipt status 1: remove, record as anchored, and record the block. Only the receipt path (the
    /// desktop's `queue::settle`) calls this.
    pub fn included_out(&mut self, ids: &[String], tx: &str, chain: u64, block: u64) -> usize {
        let n = self.anchored_out(ids);
        for id in ids {
            self.blocks.retain(|b| &b.id != id);
            self.blocks.push(Block { id: id.clone(), tx: tx.to_string(), chain, block });
        }
        n
    }

    /// This entry's state (`None` when not queued).
    pub fn step_of(&self, id: &str) -> Option<&Step> {
        self.items.iter().find(|q| q.id == id).map(|q| &q.step)
    }

    /// The block this entry's transaction was included in (`None` when not recorded).
    pub fn block_of(&self, id: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Batches still waiting for a receipt, used at startup to resume waiting: each batch's transactions (oldest
    /// first), chain and entry ids.
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

    /// Whether this entry was published: in flight, included, recorded as anchored here, or anchored per the
    /// last report. Queued, reverted, refused, or out of the queue with neither source saying anchored count as
    /// unpublished. Used by the delete path.
    pub fn published(&self, id: &str, report_anchored: &[String]) -> bool {
        self.step_of(id).is_some_and(Step::in_flight)
            || self.block_of(id).is_some()
            || self.anchored_here(id)
            || report_anchored.iter().any(|x| x == id)
    }

    /// Whether this file records it as anchored.
    pub fn anchored_here(&self, id: &str) -> bool {
        self.anchored.iter().any(|x| x == id)
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

    /// Ids of the first `n` sendable entries, for the next batch. In-flight entries are skipped: sending them
    /// again would anchor the same batch twice.
    pub fn take_ids(&self, n: usize) -> Vec<String> {
        self.items
            .iter()
            .filter(|q| !q.step.in_flight())
            .take(n)
            .map(|q| q.id.clone())
            .collect()
    }

    /// How many can be sent (in-flight entries do not count).
    pub fn sendable(&self) -> usize {
        self.items.iter().filter(|q| !q.step.in_flight()).count()
    }

    /// Convert a batch of ids into 32-byte hashes for anchoring. Any invalid id fails the whole batch, so a
    /// partial batch is never sent.
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

    /// An entry a self-audit read on chain leaves the queue recorded as anchored, unless its transaction is out
    /// (left to its receipt, recorded then); one not queued is recorded too, so it can never be queued again.
    #[test]
    fn an_entry_seen_on_chain_leaves_unless_its_transaction_is_out() {
        let mut q = Queue::default();
        let _ = q.push("aa", 1);
        let _ = q.push("bb", 2);
        q.mark(&["bb".to_string()], Step::Submitted { tx: "0x01".into(), chain: 1, nonce: Some(0) });
        let gone = q.seen_out(&["aa".to_string(), "bb".to_string(), "cc".to_string()]);
        assert_eq!(gone, 1);
        assert!(!q.has("aa") && q.has("bb"), "{q:?}");
        assert_eq!(q.anchored, vec!["aa".to_string(), "cc".to_string()]);
        assert_eq!(q.push("cc", 3), Pushed::Anchored);
    }

    /// `mark` changes only entries with no transaction out; a batch in flight moves only by the transactions it
    /// awaits (`moved_by`). One line per form: queued, refused and reverted are marked; submitted and resent are
    /// not; `moved_by` with the batch's own transactions moves it, with other transactions, another chain or a
    /// queued entry moves nothing.
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

