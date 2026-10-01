//! Anchor queue. One file, one owner.
//!
//! The queue is this machine's bookkeeping: which entries are recorded but not yet anchored. It lives in the
//! home's `settings` room, in canonical value form (the core's `json`), so copying a home carries the queue
//! along (any copy is equivalent).
//!
//! ─── No room for miscounting ───
//!
//! The risk is a miscounted queue: sent but not removed, or removed without being sent. The design has one
//! way in ([`Queue::push`]) and two ways out, each recording its reason: a chain receipt saying it succeeded
//! goes through [`Queue::anchored_out`] (removed and recorded as anchored; with the block the transaction
//! landed in, [`Queue::included_out`]; in the product only [`settle`] calls either); a deleted entry goes
//! through [`Queue::drop_ids`] (removed, nothing recorded). What failed to send stays, so "stays queued for
//! retry" holds because no other path exists.
//!
//! ─── Only the path that knows records "anchored" ───
//!
//! The `anchored` cell is a statement about a chain fact. A single exit that recorded "anchored" for every
//! caller would, when retraction used it, mark an entry that never reached the chain as anchored; re-queueing
//! it later would answer "already on chain", and the queue file would carry false evidence. So different exit
//! reasons record different things: writing `anchored` lives only in the receipt path's own exit, and no
//! other exit can touch a byte of it.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

/// The queue file's name. One name, one home.
pub const FILE: &str = "queue.json";

/// Which step an entry is at. Queued entries each have one member; entries included in a block are recorded
/// separately ([`Block`]). Closed: the status light, entry card, record card, queue page and watch line all
/// read it (through `ledgerx::Lamp`), instead of each page assembling its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Queued, not sent.
    Queued,
    /// Broadcast, and the node's echo matches the transaction: waiting for the receipt. This step is saved to
    /// disk, so after a restart the same transaction is awaited and never resent.
    Submitted { tx: String, chain: u64 },
    /// Included, but the receipt status is not 1 (law §9.1: not an anchor). Stays queued and can be resent.
    Reverted { tx: String, chain: u64 },
    /// Refused before broadcast (insufficient balance, node refusal, unreachable): never sent. Stays queued
    /// and can be resent.
    Refused { said: String },
}

impl Step {
    pub fn as_str(&self) -> &'static str {
        match self {
            Step::Queued => state::QUEUED,
            Step::Submitted { .. } => state::SUBMITTED,
            Step::Reverted { .. } => state::REVERTED,
            Step::Refused { .. } => state::REFUSED,
        }
    }
}

/// The step cell's words in the queue file. One name, one home: `Step::as_str`, the reader and anything
/// reading the file's raw cells spell them only here.
pub mod state {
    pub const QUEUED: &str = "queued";
    pub const SUBMITTED: &str = "submitted";
    pub const REVERTED: &str = "reverted";
    pub const REFUSED: &str = "refused";
}

/// The queue file's member names (top level, each row, each block). One name, one home.
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
}

/// One queued entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    /// Entry id (law §2.1's hex32).
    pub id: String,
    /// When it was queued (used only for ordering and display, never for a decision).
    pub at: u64,
    /// Which step it is at (older files lack this cell and read as [`Step::Queued`]).
    pub step: Step,
}

/// The transaction that was included (receipt status 1): which entry, which transaction, which chain, which
/// block.
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
    /// The entries this file itself records as anchored.
    ///
    /// Otherwise only two things could answer "is it on chain": whether it is still queued (not after
    /// removal) and the last self-audit report (silent if no audit ran or the report is stale). After a
    /// restart the queue is empty and there is no report, so an already anchored entry could be queued again
    /// and anchored twice on chain (the person pays gas twice). So the removal records the fact in the queue
    /// file, which survives restarts and home copies.
    pub anchored: Vec<String>,
    /// The included transactions: the receipt path records transaction, chain and block number; older files
    /// lack this cell and read it as empty.
    pub blocks: Vec<Block>,
}

/// The answer to queueing. Closed: three forms, one sentence each, spoken by the caller per form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pushed {
    /// Queued.
    Queued,
    /// Already queued (each entry is queued once).
    InQueue,
    /// This file records it as anchored: after a restart the queue is empty, and this cell remains.
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

    /// Whether it was actually queued this time (older callers still ask this).
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
    /// Read. No file means an empty queue: "never queued" is not an error.
    pub fn read(home: &Home) -> Result<Queue, Fault> {
        let p = home.dir(Slot::Settings).join(FILE);
        // Sealed (`local::Doc::Queue`): no file is an empty queue; locked or not opening is refused by name
        // (never read as empty: the next write would drop queued entries).
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
            // The step cell: absent means an older file, read as queued; unrecognized or incomplete is a
            // shape error, never guessed.
            let bad = || Fault::known(Known::QueueShape, crate::lang::filln(crate::lang::Key::Tail201, &[&(p.display()).to_string()]));
            let step = match text(member::STATE).as_deref() {
                None | Some(state::QUEUED) => Step::Queued,
                Some(state::SUBMITTED) => match (text(member::TX), chain) {
                    (Some(tx), Some(chain)) if zikaron::hexfmt::is_hex32(&tx) => Step::Submitted { tx, chain },
                    _ => return Err(bad()),
                },
                Some(state::REVERTED) => match (text(member::TX), chain) {
                    (Some(tx), Some(chain)) if zikaron::hexfmt::is_hex32(&tx) => Step::Reverted { tx, chain },
                    _ => return Err(bad()),
                },
                Some(state::REFUSED) => Step::Refused { said: text(member::SAID).unwrap_or_default() },
                Some(_) => return Err(bad()),
            };
            items.push(Queued { id, at, step });
        }
        // The anchored entries: queue files written before this cell existed lack it and read it as empty,
        // not an error.
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
        // The included transactions: older files lack this cell and read it as empty.
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

    /// Write. Overwriting the old queue is intended.
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
                    Step::Submitted { tx, chain } | Step::Reverted { tx, chain } => {
                        m.push((member::CHAIN.to_string(), Value::Int(*chain)));
                        m.push((member::TX.to_string(), Value::Str(tx.clone())));
                    }
                    Step::Refused { said } => m.push((member::SAID.to_string(), Value::Str(said.clone()))),
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
        // Writing to disk has one method (`local::put`: sealed, written aside, then renamed). A half-written queue has no
        // place on disk.
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Queue, &bytes)
    }

    /// Queue. Each entry is queued once, and an entry recorded as anchored can never be queued again.
    ///
    /// This is the on-disk owner of "is it on chain": the queueing entry point asks it, so every queueing path
    /// (queued after recording, queued by hand on the page) asks the same question without each keeping its
    /// own record.
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

    /// Remove only, record nothing. The reason this entry will no longer be anchored is not "anchored" (today
    /// the only path is retraction: a deleted record leaves the queue), so this exit does not touch
    /// `anchored`: it states no chain fact and speaks for no one.
    pub fn drop_ids(&mut self, ids: &[String]) -> usize {
        let before = self.items.len();
        self.items.retain(|q| !ids.iter().any(|x| x == &q.id));
        before - self.items.len()
    }

    /// A chain receipt says it succeeded: remove, and record as anchored. Only [`settle`] calls it (the
    /// receipt path knows this).
    ///
    /// Removal and recording the fact are two sides of one event here. Split in two, there would be a frame
    /// where it was removed but not recorded, and after that frame it could be queued again; so both happen
    /// in this exit, and `anchored` is written only here.
    pub fn anchored_out(&mut self, ids: &[String]) -> usize {
        let n = self.drop_ids(ids);
        for id in ids {
            if !self.anchored.iter().any(|x| x == id) {
                self.anchored.push(id.clone());
            }
        }
        n
    }

    /// Which step these entries reached: changes only entries still queued, nothing else.
    pub fn mark(&mut self, ids: &[String], step: Step) {
        for q in self.items.iter_mut() {
            if ids.iter().any(|x| x == &q.id) {
                q.step = step.clone();
            }
        }
    }

    /// Included with receipt status 1: remove, record as anchored, and record the block the transaction
    /// landed in. In the product only [`settle`] calls it (the only exit).
    pub fn included_out(&mut self, ids: &[String], tx: &str, chain: u64, block: u64) -> usize {
        let n = self.anchored_out(ids);
        for id in ids {
            self.blocks.retain(|b| &b.id != id);
            self.blocks.push(Block { id: id.clone(), tx: tx.to_string(), chain, block });
        }
        n
    }

    /// Which step this entry is at now (`None` when not queued).
    pub fn step_of(&self, id: &str) -> Option<&Step> {
        self.items.iter().find(|q| q.id == id).map(|q| &q.step)
    }

    /// Which block the transaction landed in (`None` when not recorded).
    pub fn block_of(&self, id: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Submitted transactions still waiting for a receipt (the question asked at startup to resume waiting;
    /// entries of the same transaction grouped together).
    pub fn submitted(&self) -> Vec<(String, u64, Vec<String>)> {
        let mut out: Vec<(String, u64, Vec<String>)> = Vec::new();
        for q in &self.items {
            if let Step::Submitted { tx, chain } = &q.step {
                match out.iter_mut().find(|(t, c, _)| t == tx && c == chain) {
                    Some((_, _, ids)) => ids.push(q.id.clone()),
                    None => out.push((tx.clone(), *chain, vec![q.id.clone()])),
                }
            }
        }
        out
    }

    /// Whether this entry was published: submitted, included, recorded as anchored in this file, or anchored
    /// per the last report. Queued, reverted, refused before broadcast, or out of the queue with neither
    /// source saying anchored all count as unpublished. One owner: the delete path asks it.
    pub fn published(&self, id: &str, report_anchored: &[String]) -> bool {
        matches!(self.step_of(id), Some(Step::Submitted { .. }))
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

    /// Ids of the first `n` (the batch picks these). Submitted entries waiting for a receipt are not picked:
    /// their transaction is already on its way, and sending again would anchor the same batch twice.
    pub fn take_ids(&self, n: usize) -> Vec<String> {
        self.items
            .iter()
            .filter(|q| !matches!(q.step, Step::Submitted { .. }))
            .take(n)
            .map(|q| q.id.clone())
            .collect()
    }

    /// How many can be sent (submitted entries do not count).
    pub fn sendable(&self) -> usize {
        self.items.iter().filter(|q| !matches!(q.step, Step::Submitted { .. })).count()
    }

    /// Turn a batch of ids into 32-byte hashes (the form anchoring needs). Unrecognized refuses the whole
    /// batch; half is never sent.
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

/// The lock for changing the queue file within one process.
///
/// Two paths change it at the same time: queueing in the frame (one entry queued per record) and removal in
/// the background (a batch removed when the receipt succeeds). Each reads, changes and writes back, so the
/// later one would overwrite the earlier with a stale table, and the just-queued entry would vanish from disk
/// and shell with nothing reporting it. Across processes there is the home's writer lock (one writer per
/// home), so this lock covers only these two in-process paths.
static AMEND: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The only place the queue on disk is changed. Read, change and write happen under one lock, and it returns
/// the table after writing, so the shell's copy has no other source (see `action::queue_it` and [`settle`]).
pub fn amend<T>(home: &Home, f: impl FnOnce(&mut Queue) -> T) -> Result<(T, Queue), Fault> {
    let _g = AMEND.lock().unwrap_or_else(|e| e.into_inner());
    let mut q = Queue::read(home)?;
    let out = f(&mut q);
    q.write(home)?;
    Ok((out, q))
}

/// The only removal decision. Removed only when the receipt succeeded; otherwise not one entry moves.
///
/// "What failed to send stays queued for retry" depends on this parameter: `anchored` comes from the anchoring
/// crate's `Sent::anchored()` (law §9.1 says both forms with status other than 1 are not anchors; that is the
/// law's statement, not the shell's), and besides this one `if` there is no other path to removal.
///
/// After removal the new queue is returned too. This runs on a background thread while the shell keeps a copy
/// of the queue; changing only the disk would leave that copy with the anchored entries, and sending "this
/// batch" a second time would anchor the same bytes twice (paying gas twice), while any later queueing would
/// write the removed entries back to disk. So this returns the new table, not a count, and the shell's copy
/// has no other source.
///
/// `at` is where the transaction was included (transaction hash, chain id, block number): when given, removal
/// also records the block, still one disk write.
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

/// Where an included transaction landed: used by the exit to record the block number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inclusion {
    pub tx: String,
    pub chain: u64,
    pub block: u64,
}

/// The two numbers in the density note at the top of the page: the largest anchored seq, which transitively
/// covers every entry at or below it.
///
/// This layer only lays out the numbers; the sentence lives in the key table, and "cadence is unrelated
/// to validity" is law §9.6's reading, not a decision here.
pub struct Density {
    /// The largest seq among anchored entries.
    pub anchored_through: Option<u64>,
    /// The largest seq in the ledger now.
    pub head_seq: Option<u64>,
    /// How many are not yet anchored, counted transitively: anchoring seq N covers every entry at or below N
    /// (law §9.6), so this counts entries with seq above `anchored_through`, not "entries whose light is not
    /// green". Counting one by one would have the page say "anchored through 9", "head is 9" and "9 to go" at
    /// once, contradicting itself.
    pub behind: usize,
}

/// Compute the three numbers. Reads the table already read, with no disk access: this is what the frame asks.
///
/// Before the table is read, all three cells have no reading (not zero): "not read yet" and "zero" differ on
/// the face.
pub fn density(rows: Option<&[crate::ledgerx::Row]>, queued: usize) -> Density {
    let Some(rows) = rows else {
        return Density { anchored_through: None, head_seq: None, behind: queued };
    };
    let anchored_through = rows
        .iter()
        .filter(|r| r.lamp == crate::ledgerx::Lamp::Anchored)
        .map(|r| r.seq)
        .max();
    // The deleted pairs on this machine are not owed: keeping them locally is intended, and any later
    // anchored entry bounds their existence along `prev` (law §9.6). Counting them would make the watch line
    // say "not anchored" forever.
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
