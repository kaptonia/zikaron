use super::*;

pub(super) fn queue_entry(shell: &mut Shell, id: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    shell.may_write_entries()?;
    let id = id.trim().to_string();
    if !zikaron::hexfmt::is_hex32(&id) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, id));
    }
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    crate::ledgerx::detail(home, &id).map_err(|_| {
        crate::fault::Fault::known(crate::fault::Known::SubjectMissing, id.clone())
    })?;
    if let Some(a) = shell.audit.as_ref() {
        if crate::ledgerx::anchored_of(&a.report).iter().any(|(h, _)| *h == id) {
            return Err(crate::fault::Fault::known(crate::fault::Known::AlreadyAnchored, id));
        }
        if a.mark == shell.book_mark && !crate::auditx::entry_ids(&a.report, zikaron::v2::tokens::Key::Unanchored).contains(&id.to_ascii_lowercase()) {
            return Err(crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::TailNotLedgerEntry).to_string())));
        }
    }
    if shell.queue.anchored_here(&id) {
        return Err(crate::fault::Fault::known(crate::fault::Known::AlreadyAnchored, id));
    }
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    let rows = crate::ledgerx::table(home, None, &queued)?.rows;
    if crate::retractx::read(&rows).is_deleted(&id) {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::EntryRefused,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail230).to_string()),
        ));
    }
    if shell.queue.has(&id) {
        return Ok((id, Enqueued { queued: shell.queue.len(), next: Next::Held }));
    }
    let n = queue_it(shell, &id);
    Ok((id, n))
}

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
            crate::grantx::table(home)?
        }
    };
    shell.clash.clear();
    shell.clash_unsettled = 0;
    let window = window_of(from, to)?;
    let (waiting, clash): (Vec<crate::grantx::Row>, Vec<crate::grantx::Row>) = crate::grantx::conflicts(&rows, work, window).into_iter().partition(|r| r.unnoticed);
    shell.clash = clash;
    shell.clash_unsettled = waiting.len();
    Ok(shell.clash.len())
}
