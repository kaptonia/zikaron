use super::*;

pub(super) fn draft_grant(
    shell: &mut Shell,
    d: &crate::grantx::Draft,
    exclusive: bool,
    terms_file: Option<&str>,
) -> Result<(String, Enqueued), crate::fault::Fault> {
    // Double-sale gate: if an existing grant on the same record has an overlapping window and the local
    // exclusive flag, do not sign. The collisions are kept in `clash` for the UI to list.
    let flags = shell.settings.exclusive.clone();
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // A deleted record cannot be the subject of a new grant: refused when all of this record's anchor entries
    // in this ledger are deleted.
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    if crate::retractx::work_deleted(&crate::ledgerx::table(home, None, &queued)?.rows, &d.work) {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::EntryRefused,
            crate::lang::t(crate::lang::Key::Tail231).to_string(),
        ));
    }
    let rows = crate::grantx::table(home, &flags)?;
    let window = window_of(&d.from, &d.to)?;
    let clash = crate::grantx::conflicts(&rows, &d.work, window);
    if !clash.is_empty() {
        let n = clash.len();
        shell.clash = clash;
        return Err(crate::fault::Fault::known(
            crate::fault::Known::Conflict,
            crate::lang::filln(crate::lang::Key::Tail043, &[&(n).to_string()]),
        ));
    }
    shell.clash.clear();
    // Check the terms document before signing: its digest must equal the terms digest signed into the grant,
    // or nothing is signed. `termsx::keep` checks again when storing it after signing.
    let doc = terms_file.map(str::trim).filter(|p| !p.is_empty()).map(std::path::PathBuf::from);
    if let Some(p) = doc.as_ref() {
        let c = crate::anchorx::of_file(p)?;
        if !c.hex().eq_ignore_ascii_case(d.terms.trim()) {
            return Err(crate::fault::Fault::known(crate::fault::Known::TermsMismatch, format!("{} ≠ {}", c.hex(), d.terms.trim())));
        }
    }
    let (head, secret) = ready_to_append(shell)?;
    let sealed = match crate::grantx::draft(&secret, d, head) {
        Ok(x) => {
            shell.flow.sign = crate::anchorx::Step::Done;
            x
        }
        Err(f) => {
            shell.flow.sign = crate::anchorx::Step::Failed;
            return Err(f);
        }
    };
    let id = land_sealed(shell, sealed)?;
    // Queue before local bookkeeping. The bytes are already in the ledger and queueing is their only path to
    // the chain; a failing bookkeeping write first would leave the grant recorded but never queued, and a
    // retry would then conflict with it. The exclusive flag is local bookkeeping only: losing it skips one
    // local check next time, and no guarantee depends on it.
    let n = queue_it(shell, &id);
    // The issuance record (`termsx`), written once at signing: exclusivity, terms digest, document location.
    // It cannot be rewritten later; a settings flag could be flipped after issuing without the buyer of
    // exclusivity being able to stop it, so the settings list is only read-only history.
    if let Some(home) = shell.home.as_ref() {
        if let Err(f) = crate::termsx::keep(home, &id, &d.terms, exclusive, doc.as_deref()) {
            shell.faults.push(f);
        }
    }
    shell.stale_grants();
    Ok((id, n))
}

/// Parses the form's two window fields as a pair of numbers. Both must be present or both absent.
pub(super) fn window_of(from: &str, to: &str) -> Result<Option<(u64, u64)>, crate::fault::Fault> {
    let (f, t) = (from.trim(), to.trim());
    match (f.is_empty(), t.is_empty()) {
        (true, true) => Ok(None),
        (false, false) => {
            let num = |s: &str| crate::fault::whole_within_ceiling(s, crate::lang::Key::Tail044);
            Ok(Some((num(f)?, num(t)?)))
        }
        _ => Err(crate::fault::Fault::known(
            crate::fault::Known::FieldMissing,
            crate::lang::t(crate::lang::Key::Tail045).to_string(),
        )),
    }
}
