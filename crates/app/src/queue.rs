//! The anchor queue: entries recorded but not yet anchored.
//!
//! It lives in the home's `settings/` directory, sealed (`local::Doc::Queue`), so copying a home carries its
//! queue along. Its model and bytes are the sealed layer's (`zikaron_seal::queue`), one with the command line's
//! sealed write; its one read-modify-write lock, the receipt path ([`settle`], the only caller of the ways out that
//! record "anchored") and the queue following the ledger ([`follow`]) are the sender's (`zikaron_send::keep`). What
//! stays here is where the file lives, reading and writing it under the home's seal ([`Kept`]), and the words a
//! refusal is said in.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
pub use zikaron_seal::queue::{member, state, Block, Pushed, Queue, Queued, Shape, Step, FILE, RESENDS_MAX};

/// The queue file on disk: read and written here, its bytes the sealed layer's.
pub trait Kept: Sized {
    /// Read the queue. No file means an empty queue ("never queued" is not an error). Bytes not in this version's
    /// shape are refused by name (`QUEUE_SHAPE`), never read with a member guessed.
    fn read(home: &Home) -> Result<Self, Fault>;
    /// Write the queue, replacing the previous one.
    fn write(&self, home: &Home) -> Result<(), Fault>;
}

impl Kept for Queue {
    fn read(home: &Home) -> Result<Queue, Fault> {
        let p = home.dir(Slot::Settings).join(FILE);
        // Sealed (`local::Doc::Queue`). Locked or unreadable is an error, never treated as empty, or the next
        // write would drop queued entries.
        let Some(bytes) = crate::local::read(&p, crate::local::Doc::Queue)? else {
            return Ok(Queue::default());
        };
        Queue::parse(&bytes).map_err(|s| unparsed(&p, s))
    }

    fn write(&self, home: &Home) -> Result<(), Fault> {
        // `local::put` seals, writes aside, then renames, so a half-written queue never lands on disk.
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Queue, &self.to_bytes())
    }
}

/// A queue file not in its shape, said with its place.
pub fn unparsed(p: &std::path::Path, s: Shape) -> Fault {
    let at = p.display().to_string();
    Fault::known(
        Known::QueueShape,
        match s {
            Shape::Json(t) => format!("{at}: {t}"),
            Shape::NoQueued => crate::lang::filln(crate::lang::Key::Tail200, &[&at]),
            Shape::BadRow => crate::lang::filln(crate::lang::Key::Tail201, &[&at]),
            Shape::Missing(k) => format!("{at}: {k}"),
        },
    )
}

pub use zikaron_send::keep::Inclusion;

/// The only way the queue on disk is changed: read, modify and write under the sender's one lock
/// (`zikaron_send::keep::amend`), returning the queue as written so the shell's copy always comes from there.
pub fn amend<T>(home: &Home, f: impl FnOnce(&mut Queue) -> T) -> Result<(T, Queue), Fault> {
    zikaron_send::keep::amend(&crate::sendx::Desk::of(home), f)
}

/// The only removal decision (`zikaron_send::keep::settle`): entries are removed only when the receipt succeeded.
pub fn settle(home: &Home, ids: &[String], anchored: bool, at: Option<&Inclusion>) -> Result<(usize, Queue), Fault> {
    zikaron_send::keep::settle(&crate::sendx::Desk::of(home), ids, anchored, at)
}

/// The queue follows the ledger and the chain (`zikaron_send::keep::follow`).
pub fn follow(home: &Home, on_chain: &[String], owed: &[String], at: u64) -> Result<Queue, Fault> {
    zikaron_send::keep::follow(&crate::sendx::Desk::of(home), on_chain, owed, at)
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
