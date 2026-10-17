//! The queue on disk, kept by the sender for every end: its one read-modify-write under one lock ([`amend`]), the
//! receipt path ([`settle`], the only caller of the ways out that record "anchored") and the queue following the
//! ledger and the chain ([`follow`]). Its model and bytes are the sealed layer's (`zikaron_seal::queue`); where
//! the file lives and its seal are each end's ([`End::read_queue`], [`End::write_queue`]).

use crate::End;
use zikaron_seal::queue::Queue;

/// Serializes queue file changes within one process.
///
/// Queueing on one thread and removal on another (after a successful receipt) each read, modify and write back;
/// unserialized, the later write would restore a stale queue and silently lose the newly queued entry. Across
/// processes, the home's writer lock applies.
static AMEND: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The only way the queue on disk is changed: read, modify and write under one lock, returning the queue as
/// written so an end's copy always comes from here.
pub fn amend<E: End, T>(end: &E, f: impl FnOnce(&mut Queue) -> T) -> Result<(T, Queue), E::Fail> {
    let _g = AMEND.lock().unwrap_or_else(|e| e.into_inner());
    let mut q = end.read_queue()?;
    let out = f(&mut q);
    end.write_queue(&q)?;
    Ok((out, q))
}

/// The queue as it is on disk now, read under the same lock (no write).
pub fn read<E: End>(end: &E) -> Result<Queue, E::Fail> {
    let _g = AMEND.lock().unwrap_or_else(|e| e.into_inner());
    end.read_queue()
}

/// The only removal decision: entries are removed only when the receipt succeeded; otherwise nothing changes.
///
/// `anchored` comes from the receipt (per law §9.1, a receipt status other than 1 is not an anchor), and there is
/// no other path to removal, so anything that failed to send stays queued for retry.
///
/// The new queue is returned along with the count. This runs in the background while an end keeps a copy;
/// updating only the disk would leave the anchored entries in that copy, so sending again would anchor them
/// twice (paying gas twice) and later queueing would write them back.
///
/// `at` is where the transaction was included (hash, chain id, block number); when given, the block is
/// recorded in the same write.
pub fn settle<E: End>(end: &E, ids: &[String], anchored: bool, at: Option<&Inclusion>) -> Result<(usize, Queue), E::Fail> {
    if !anchored {
        return Ok((0, read(end)?));
    }
    amend(end, |q| match at {
        Some(i) => q.included_out(ids, &i.tx, i.chain, i.block),
        None => q.anchored_out(ids),
    })
}

/// The queue follows the ledger and the chain: entries a self-audit read on chain that still wait here leave it,
/// recorded as anchored (`Queue::seen_out`); entries owed an anchor that are neither queued nor on chain join it
/// (`Queue::push`, which refuses one recorded as anchored). One read-modify-write under [`amend`]; the queue as
/// written comes back for the end's copy.
pub fn follow<E: End>(end: &E, on_chain: &[String], owed: &[String], at: u64) -> Result<Queue, E::Fail> {
    amend(end, |q| {
        q.seen_out(on_chain);
        for id in owed {
            let _ = q.push(id, at);
        }
    })
    .map(|(_, q)| q)
}

/// Where an included transaction landed, for recording the block number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inclusion {
    pub tx: String,
    pub chain: u64,
    pub block: u64,
}
