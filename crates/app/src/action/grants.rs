use super::*;

/// Queue an entry that is outside the queue for anchoring.
///
/// The queueing rule is the same as the recording path (`queue_it`'s disk write): read, change and write
/// under one lock. Three preconditions: the pen in hand (broken chain, handed over and read-only each have
/// their refusal), the entry really in this ledger, and not yet anchored. Already queued is not queued again
/// (the face reads its current position itself).
pub(super) fn queue_entry(shell: &mut Shell, id: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    shell.may_write_entries()?;
    let id = id.trim().to_string();
    if !zikaron::hexfmt::is_hex32(&id) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, id));
    }
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // Is the entry really in this ledger. The same reading as annotation (`ledgerx::detail` reads the disk
    // once): pointing nowhere is refused by name, so no queued name points at a nonexistent entry.
    crate::ledgerx::detail(home, &id).map_err(|_| {
        crate::fault::Fault::known(crate::fault::Known::SubjectMissing, id.clone())
    })?;
    // Is it not yet anchored. Two sources answer this, each once; either saying anchored refuses (both named,
    // neither posing as the other):
    //
    // 1. What the queue file itself records: written on removal, so it survives restarts and home copies. The
    // queueing entry point asks it (`Queue::push` returns `Pushed::Anchored`), so every queueing path gets it.
    // 2. The last self-audit report (read from the chain): it covers anchors made from other machines and
    // copies, but says nothing when no audit ran or the report is stale. So it is only the second source;
    // alone it cannot say everything.
    //
    // With only the second source, living only in the face's light, a stale or missing report would still
    // show the button and queue the entry.
    if let Some(a) = shell.audit.as_ref() {
        if crate::ledgerx::anchored_of(&a.report).iter().any(|(h, _)| *h == id) {
            return Err(crate::fault::Fault::known(crate::fault::Known::AlreadyAnchored, id));
        }
    }
    if shell.queue.anchored_here(&id) {
        return Err(crate::fault::Fault::known(crate::fault::Known::AlreadyAnchored, id));
    }
    // A deleted entry no longer goes on chain as a record (the retraction rule, read the same way as the
    // reading convention): queueing again is refused by name, and the sentence states only the ledger fact
    // ("this record is deleted"), never a chain fact that did not happen.
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

/// Run one overlap check.
///
/// If the register has been read, use the copy in hand (pure computation, no disk): the face's as-you-type
/// check calls this on every keystroke, and walking the ledger directory on each would move disk reads back
/// into the frame. Only when never read is it read once now.
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
    // Clear the old reading first: with bad window cells this pass leaves midway, and the face must not keep
    // the count from the previous three cells.
    shell.clash.clear();
    let window = window_of(from, to)?;
    shell.clash = crate::grantx::conflicts(&rows, work, window);
    Ok(shell.clash.len())
}
