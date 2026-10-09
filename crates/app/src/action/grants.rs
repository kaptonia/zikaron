use super::*;

/// Queues an existing ledger entry for anchoring.
///
/// Uses the same queueing rule as the recording path (`queue_it`): read, change and write under one lock.
/// Preconditions: this instance may write (broken chain, handed over and read-only each have their own
/// refusal), the entry is in this ledger, and it is not yet anchored. An entry already queued is not queued
/// again.
pub(super) fn queue_entry(shell: &mut Shell, id: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    shell.may_write_entries()?;
    let id = id.trim().to_string();
    if !zikaron::hexfmt::is_hex32(&id) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, id));
    }
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // The entry must be in this ledger (`ledgerx::detail`, as for annotations), so no queued id points at a
    // nonexistent entry.
    crate::ledgerx::detail(home, &id).map_err(|_| {
        crate::fault::Fault::known(crate::fault::Known::SubjectMissing, id.clone())
    })?;
    // The entry must not be anchored yet. Two sources are asked, and either one saying "anchored" refuses:
    //
    // 1. The queue file's own record, written on removal, so it survives restarts and home copies.
    // `Queue::push` checks it (returning `Pushed::Anchored`), so every queueing path gets it.
    // 2. The last self-audit report (from the chain): it covers anchors made from other machines and copies,
    // but says nothing when no audit ran and may be stale, so it cannot be the only source.
    if let Some(a) = shell.audit.as_ref() {
        if crate::ledgerx::anchored_of(&a.report).iter().any(|(h, _)| *h == id) {
            return Err(crate::fault::Fault::known(crate::fault::Known::AlreadyAnchored, id));
        }
    }
    if shell.queue.anchored_here(&id) {
        return Err(crate::fault::Fault::known(crate::fault::Known::AlreadyAnchored, id));
    }
    // A deleted entry is no longer anchored as a record, so queueing it again is refused by name. The message
    // states only the ledger fact ("this record is deleted"), never a claim about the chain.
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    let rows = crate::ledgerx::table(home, None, &queued)?.rows;
    if crate::retractx::read(&rows).is_deleted(&id) {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::EntryRefused,
            crate::lang::t(crate::lang::Key::Tail230).to_string(),
        ));
    }
    if shell.queue.has(&id) {
        return Ok((id, Enqueued { queued: shell.queue.len(), next: Next::Held }));
    }
    let n = queue_it(shell, &id);
    Ok((id, n))
}

/// Runs one overlap check.
///
/// Uses the register copy in hand when it has been read (no disk access), because the UI calls this on every
/// keystroke. Only a register that was never read is read once now.
pub(super) fn check_clash(
    shell: &mut Shell,
    work: &str,
    from: &str,
    to: &str,
) -> Result<usize, crate::fault::Fault> {
    let rows = match shell.grants.clone() {
        Some(rows) => rows,
        None => {
            let home = shell.home.as_ref().ok_or_else(|| {
                crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
            })?;
            crate::grantx::table(home, &shell.settings.exclusive)?
        }
    };
    // Clear the old result first, so invalid window fields cannot leave the previous count on screen.
    shell.clash.clear();
    let window = window_of(from, to)?;
    shell.clash = crate::grantx::conflicts(&rows, work, window);
    Ok(shell.clash.len())
}
