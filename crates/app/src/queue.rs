//! The anchor queue: entries recorded but not yet anchored.
//!
//! It lives in the home's `settings/` directory as canonical JSON (the core's `json`), so copying a home
//! carries its queue along.
//!
//! ─── No miscounting ───
//!
//! The risk is an entry sent but not removed, or removed without being sent. There is one way in
//! ([`Queue::push`]) and two ways out, each with its reason: a successful chain receipt goes through
//! [`Queue::anchored_out`] (removed and recorded as anchored; [`Queue::included_out`] also records the block),
//! called only by [`settle`]; a deleted entry goes through [`Queue::drop_ids`] (removed, nothing recorded).
//! Anything that failed to send stays queued for retry because no other removal path exists.
//!
//! ─── Only the receipt path records "anchored" ───
//!
//! The `anchored` list states a chain fact. If one exit recorded "anchored" for every caller, a retraction
//! would mark an entry that never reached the chain as anchored, and re-queueing it would wrongly answer
//! "already on chain". So only the receipt path's exit writes `anchored`.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

/// The queue file's name.
pub const FILE: &str = "queue.json";

/// A queued entry's sending state. Entries included in a block are recorded separately ([`Block`]). The
/// status light, entry card, record card, queue page and watch line all derive from this (through
/// `ledgerx::Lamp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Queued, not sent.
    Queued,
    /// Broadcast, and the node returned the matching transaction hash: waiting for the receipt. Saved to disk,
    /// so after a restart the same transaction is awaited, never resent. `nonce` is the batch's signed nonce
    /// (`None` in files from older versions); it distinguishes a batch no node holds any more from one whose
    /// nonce another transaction used (`action::confirm_batch`).
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
    /// Its state (files from older versions lack it; read as [`Step::Queued`]).
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
    /// Included transactions with chain and block number, recorded by the receipt path; files from older
    /// versions lack this and read it as empty.
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

impl Queue {
    /// Read the queue. No file means an empty queue ("never queued" is not an error).
    pub fn read(home: &Home) -> Result<Queue, Fault> {
        let p = home.dir(Slot::Settings).join(FILE);
        // Sealed (`local::Doc::Queue`). Locked or unreadable is an error, never treated as empty, or the next
        // write would drop queued entries.
        let Some(bytes) = crate::local::read(&p, crate::local::Doc::Queue)? else {
            return Ok(Queue::default());
        };
        let v = json::parse(&bytes)
            .map_err(|t| Fault::known(Known::QueueShape, format!("{}: {t:?}", p.display())))?;
        let Some(Value::Arr(rows)) = member(&v, member::QUEUED) else {
            return Err(Fault::known(Known::QueueShape, crate::lang::filln(crate::lang::Key::Tail200, &[&(p.display()).to_string()])));
        };
        let mut items = Vec::new();
        for r in rows {
            let id = match member(r, member::ID) {
                Some(Value::Str(s)) if zikaron::hexfmt::is_hex32(s) => s.clone(),
                _ => {
                    return Err(Fault::known(
                        Known::QueueShape,
                        crate::lang::filln(crate::lang::Key::Tail201, &[&(p.display()).to_string()]),
                    ))
                }
            };
            let at = match member(r, member::AT) {
                Some(Value::Int(n)) => *n,
                _ => 0,
            };
            let text = |k: &str| match member(r, k) {
                Some(Value::Str(s)) => Some(s.clone()),
                _ => None,
            };
            let chain = match member(r, member::CHAIN) {
                Some(Value::Int(n)) => Some(*n),
                _ => None,
            };
            // The batch's nonce: absent in files from older versions; if present it must be an integer.
            let nonce = match member(r, member::NONCE) {
                None => Ok(None),
                Some(Value::Int(n)) => Ok(Some(*n)),
                Some(_) => Err(()),
            };
            // `state`: absent (older files) reads as queued; unknown or incomplete is a shape error, never
            // guessed.
            let bad = || Fault::known(Known::QueueShape, crate::lang::filln(crate::lang::Key::Tail201, &[&(p.display()).to_string()]));
            let step = match text(member::STATE).as_deref() {
                None | Some(state::QUEUED) => Step::Queued,
                Some(state::SUBMITTED) => match (text(member::TX), chain, nonce) {
                    (Some(tx), Some(chain), Ok(nonce)) if zikaron::hexfmt::is_hex32(&tx) => Step::Submitted { tx, chain, nonce },
                    _ => return Err(bad()),
                },
                Some(state::REVERTED) => match (text(member::TX), chain) {
                    (Some(tx), Some(chain)) if zikaron::hexfmt::is_hex32(&tx) => Step::Reverted { tx, chain },
                    _ => return Err(bad()),
                },
                Some(state::REFUSED) => Step::Refused { said: text(member::SAID).unwrap_or_default() },
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
                Some(_) => return Err(bad()),
            };
            items.push(Queued { id, at, step });
        }
        // `anchored`: absent in files from older versions; read as empty, not an error.
        let mut anchored: Vec<String> = Vec::new();
        if let Some(Value::Arr(rows)) = member(&v, member::ANCHORED) {
            for r in rows {
                match r {
                    Value::Str(x) if zikaron::hexfmt::is_hex32(x) => anchored.push(x.clone()),
                    _ => {
                        return Err(Fault::known(
                            Known::QueueShape,
                            crate::lang::filln(crate::lang::Key::Tail201, &[&(p.display()).to_string()]),
                        ))
                    }
                }
            }
        }
        // `blocks`: absent in files from older versions; read as empty.
        let mut blocks: Vec<Block> = Vec::new();
        if let Some(Value::Arr(rows)) = member(&v, member::BLOCKS) {
            for r in rows {
                let (Some(Value::Str(id)), Some(Value::Str(tx)), Some(Value::Int(chain)), Some(Value::Int(block))) =
                    (member(r, member::ID), member(r, member::TX), member(r, member::CHAIN), member(r, member::BLOCK))
                else {
                    return Err(Fault::known(
                        Known::QueueShape,
                        crate::lang::filln(crate::lang::Key::Tail201, &[&(p.display()).to_string()]),
                    ));
                };
                blocks.push(Block { id: id.clone(), tx: tx.clone(), chain: *chain, block: *block });
            }
        }
        Ok(Queue { items, anchored, blocks })
    }

    /// Write the queue, replacing the previous one.
    pub fn write(&self, home: &Home) -> Result<(), Fault> {
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
        let bytes = json::canon_bytes(&Value::Obj(vec![
            (member::ANCHORED.to_string(), Value::Arr(anchored)),
            (member::BLOCKS.to_string(), Value::Arr(blocks)),
            (member::QUEUED.to_string(), Value::Arr(rows)),
        ]));
        // `local::put` seals, writes aside, then renames, so a half-written queue never lands on disk.
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Queue, &bytes)
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

    /// A successful chain receipt: remove and record as anchored. Only [`settle`] calls this.
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

    /// Set the state of these entries (only entries still queued are affected).
    pub fn mark(&mut self, ids: &[String], step: Step) {
        for q in self.items.iter_mut() {
            if ids.iter().any(|x| x == &q.id) {
                q.step = step.clone();
            }
        }
    }

    /// Included with receipt status 1: remove, record as anchored, and record the block. Only [`settle`] calls
    /// this in the app.
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
    pub fn hashes(ids: &[String]) -> Result<Vec<[u8; 32]>, Fault> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let b = zikaron::hexfmt::decode(id)
                .ok_or_else(|| Fault::known(Known::ContentShape, id.clone()))?;
            if b.len() != 32 {
                return Err(Fault::known(Known::ContentShape, id.clone()));
            }
            let mut h = [0u8; 32];
            h.copy_from_slice(&b);
            out.push(h);
        }
        Ok(out)
    }
}

/// Serializes queue file changes within one process.
///
/// Queueing on the UI thread and removal in the background (after a successful receipt) each read, modify
/// and write back; unserialized, the later write would restore a stale queue and silently lose the newly
/// queued entry. Across processes, the home's writer lock applies.
static AMEND: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The only way the queue on disk is changed: read, modify and write under one lock, returning the queue as
/// written so the shell's copy always comes from here (see `action::queue_it` and [`settle`]).
pub fn amend<T>(home: &Home, f: impl FnOnce(&mut Queue) -> T) -> Result<(T, Queue), Fault> {
    let _g = AMEND.lock().unwrap_or_else(|e| e.into_inner());
    let mut q = Queue::read(home)?;
    let out = f(&mut q);
    q.write(home)?;
    Ok((out, q))
}

/// The only removal decision: entries are removed only when the receipt succeeded; otherwise nothing changes.
///
/// `anchored` comes from the anchoring crate's `Sent::anchored()` (per law §9.1, a receipt status
/// other than 1 is not an anchor), and there is no other path to removal, so anything that failed to send
/// stays queued for retry.
///
/// The new queue is returned along with the count. This runs in the background while the shell keeps a copy;
/// updating only the disk would leave the anchored entries in that copy, so sending again would anchor them
/// twice (paying gas twice) and later queueing would write them back.
///
/// `at` is where the transaction was included (hash, chain id, block number); when given, the block is
/// recorded in the same write.
pub fn settle(home: &Home, ids: &[String], anchored: bool, at: Option<&Inclusion>) -> Result<(usize, Queue), Fault> {
    if !anchored {
        let _g = AMEND.lock().unwrap_or_else(|e| e.into_inner());
        let q = Queue::read(home)?;
        return Ok((0, q));
    }
    amend(home, |q| match at {
        Some(i) => q.included_out(ids, &i.tx, i.chain, i.block),
        None => q.anchored_out(ids),
    })
}

/// Where an included transaction landed, for recording the block number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inclusion {
    pub tx: String,
    pub chain: u64,
    pub block: u64,
}

/// The numbers in the anchoring density note at the top of the page. Anchoring seq N transitively covers every
/// entry at or below it (law §9.6), so anchoring cadence does not affect validity.
pub struct Density {
    /// The largest seq among anchored entries.
    pub anchored_through: Option<u64>,
    /// The largest seq in the ledger now.
    pub head_seq: Option<u64>,
    /// How many are not yet anchored, counted transitively: entries with seq above `anchored_through`, not
    /// entries whose lamp is not green. Counting individually could show "anchored through 9", "head is 9" and
    /// "9 to go" at once.
    pub behind: usize,
}

/// Compute the density from the already loaded table, with no disk access (called on the UI thread).
///
/// Before the table is loaded the seq numbers are `None`, not zero, so "not read yet" and "zero" look
/// different.
pub fn density(rows: Option<&[crate::ledgerx::Row]>, queued: usize) -> Density {
    let Some(rows) = rows else {
        return Density { anchored_through: None, head_seq: None, behind: queued };
    };
    let anchored_through = rows
        .iter()
        .filter(|r| r.lamp == crate::ledgerx::Lamp::Anchored)
        .map(|r| r.seq)
        .max();
    // Local deletion pairs are not owed an anchor: keeping them local is intended, and any later anchored entry
    // covers them along `prev` (law §9.6). Counting them would show "not anchored" forever.
    let owed = |r: &&crate::ledgerx::Row| !r.lamp.local();
    Density {
        anchored_through,
        head_seq: rows.iter().map(|r| r.seq).max(),
        behind: match anchored_through {
            Some(n) => rows.iter().filter(|r| r.seq > n).filter(owed).count(),
            None => rows.iter().filter(owed).count(),
        },
    }
}
