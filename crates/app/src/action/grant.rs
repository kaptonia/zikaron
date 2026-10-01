use super::*;

pub(super) fn draft_grant(
    shell: &mut Shell,
    d: &crate::grantx::Draft,
    exclusive: bool,
    terms_file: Option<&str>,
) -> Result<(String, Enqueued), crate::fault::Fault> {
    // Double-sale gate. Same record, overlapping windows, an existing grant with the local exclusive flag:
    // when all three hold, do not sign; the collisions stay in `clash`, and the face's modal lists them one
    // by one.
    let flags = shell.settings.exclusive.clone();
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // A deleted record cannot be the subject of a new grant (this desk's reading convention): refused when
    // all of this record's anchor entries in this ledger are deleted.
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
    // Check the terms document before signing: a received document's digest must equal the one to be signed
    // into the grant; otherwise refuse at once and do not sign. Keeping it happens after signing
    // (`termsx::keep` checks again); this removes "signed while the document does not match" before signing.
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
    // Queue before local bookkeeping. The bytes are already in the ledger, and queueing is their only path to
    // the chain. With the exclusive flag in between, an unwritable settings file would leave the whole action
    // at that `?`: the grant recorded but never queued, and a retry would collide as Conflict with the grant
    // just recorded. This closes the "recorded but not queued" form; what is loosened is the exclusive flag,
    // which is local bookkeeping: failing to record it only removes one local check next time, and no legal
    // guarantee depends on it.
    let n = queue_it(shell, &id);
    // The issuance record, written once at signing (`termsx`): exclusivity, terms digest, document location.
    // Refused when present, with no action to change it afterwards (a flag in the settings list could be
    // flipped by one button after issuing, and the buyer of exclusivity could not prevent it); that list is
    // demoted to read-only history.
    if let Some(home) = shell.home.as_ref() {
        if let Err(f) = crate::termsx::keep(home, &id, &d.terms, exclusive, doc.as_deref()) {
            shell.faults.push(f);
        }
    }
    shell.stale_grants();
    Ok((id, n))
}

/// Read the form's two window cells as a pair of numbers. Both present or both absent (law §6.3).
pub(super) fn window_of(from: &str, to: &str) -> Result<Option<(u64, u64)>, crate::fault::Fault> {
    let (f, t) = (from.trim(), to.trim());
    match (f.is_empty(), t.is_empty()) {
        (true, true) => Ok(None),
        (false, false) => {
            let num = |s: &str| -> Result<u64, crate::fault::Fault> {
                s.parse::<u64>().map_err(|_| {
                    crate::fault::Fault::known(
                        crate::fault::Known::SettingsShape,
                        crate::lang::filln(crate::lang::Key::Tail044, &[&format!("{:?}", s)]),
                    )
                })
            };
            Ok(Some((num(f)?, num(t)?)))
        }
        _ => Err(crate::fault::Fault::known(
            crate::fault::Known::FieldMissing,
            crate::lang::t(crate::lang::Key::Tail045).to_string(),
        )),
    }
}
