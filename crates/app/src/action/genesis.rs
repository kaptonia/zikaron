use super::*;

pub(super) fn genesis(shell: &mut Shell, statement: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    shell.may_write_entries()?;
    // One ledger, one root: genesis must be the first entry, so it is written only when both the pile (the
    // store's lenient read) and the skipped list are empty. The CLI's `init` applies the same rule.
    //
    // Checking only for a recognized seq-0 entry is not enough: a file with an entry name but broken bytes is
    // read into the pile, rejected by the core and not counted, so a root would be written beside it; the
    // same holds for a ledger with entries but no root.
    //
    // Refusals: one or more seq-0 entries recognized by the core gives `AlreadyRooted`; anything else
    // non-empty gives `Ledger`, with the item counts. All judging is in the store and the core; this layer
    // only counts.
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let survey = home
        .ledger()?
        .survey()?;
    if !survey.items.is_empty() || !survey.skipped.is_empty() {
        let checked: Vec<Option<zikaron::entry::Entry>> =
            survey.items.iter().map(|b| zikaron::entry::check(b).ok()).collect();
        // seq-0 entries the core recognizes, deduplicated by id (one entry stored under two names is one root;
        // two entries from the same key are two roots).
        let mut roots: Vec<String> = checked
            .iter()
            .flatten()
            .filter(|e| e.seq == 0)
            .map(|e| e.id_hex())
            .collect();
        roots.sort();
        roots.dedup();
        if !roots.is_empty() {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::AlreadyRooted,
                crate::lang::filln(crate::lang::Key::Tail009, &[&(roots.len()).to_string()]),
            ));
        }
        let refused = checked.iter().filter(|e| e.is_none()).count();
        let entries = checked.len() - refused;
        return Err(crate::fault::Fault::known(
            crate::fault::Known::Ledger,
            crate::lang::filln(crate::lang::Key::Tail010, &[&(checked.len() + survey.skipped.len()).to_string(), &(entries).to_string(), &(refused).to_string(), &(survey.skipped.len()).to_string()]),
        ));
    }
    let secret = signing_key(shell, crate::sign::Use::Sign(crate::sign::Face::Entry))?;
    let sealed = crate::entryx::genesis(&secret, statement)?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let ledger = home.ledger()?;
    let bare = sealed.id.trim_start_matches("0x");
    let name = zikaron_store::EntryName::parse(bare).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::Ledger, crate::lang::filln(crate::lang::Key::Tail012, &[&(bare).to_string()]))
    })?;
    ledger
        .append(&name, &sealed.bytes)?;
    shell.book_changed();
    // Queue the root right away, through the same path as records, grants and revocations (`enqueue`, the body
    // of `queue_it`). With auto-anchor on it is sent with the queue; otherwise it waits for manual sending.
    let (_, n) = enqueue(shell, &sealed.id);
    Ok((sealed.id, n))
}

/// Queues an existing ledger's root once. Ledgers created before genesis was queued automatically have a root
/// that was never queued or anchored; without this they could never show an anchored root. Runs once when a
/// home is opened, not on a timer.
///
/// Only on the side that can write this ledger (not for readers, a broken chain, a held pen, a handed-over
/// ledger, or a restored home not yet fetched).
///
/// Only with a chain reading in hand: this home has a chain id, and there is this session's audit report or
/// the last audit's cached set (filtered by the current chain id). Without a reading, a restored, fetched or
/// adopted ledger whose root was anchored long ago would be queued and paid for again, so nothing is
/// backfilled until an audit has run and the home is reopened. With a reading, the root is skipped if it is in
/// the reading, already queued, or recorded as anchored in the queue file. Queueing uses the same
/// [`enqueue`]; since the person pressed nothing, no confirmation card is shown and the root waits for the
/// next anchoring.
pub(super) fn backfill_root(shell: &mut Shell) {
    if !shell.rooted || shell.unfetched.is_some() || shell.may_write_entries().is_err() || shell.settings.chain_id.is_none() {
        return;
    }
    // Only a complete reading counts (`auditx::whole`): in an incomplete one, a root missing from the anchor
    // set may still be anchored, and backfilling would anchor it again.
    let known: Option<Vec<String>> = match (shell.audit.as_ref(), shell.remembered.as_ref()) {
        (Some(a), _) if crate::auditx::whole(&a.label, a.asked, &a.unanswered) => Some(crate::ledgerx::anchored_of(&a.report).into_iter().map(|(h, _)| h).collect()),
        (None, Some(r)) if r.whole => Some(r.rows.iter().map(|(h, _)| h.clone()).collect()),
        _ => None,
    };
    let Some(known) = known else { return };
    let Some(home) = shell.home.as_ref() else { return };
    let Ok(root) = crate::ledgerx::root_of(home) else { return };
    if known.iter().any(|id| id.eq_ignore_ascii_case(&root)) || shell.queue.has(&root) || shell.queue.anchored_here(&root) {
        return;
    }
    let _ = enqueue(shell, &root);
}

pub(super) fn adopt(shell: &mut Shell, dir: &str) -> Result<crate::firstrun::Adopted, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let got = crate::firstrun::adopt(std::path::Path::new(dir), home);
    // Adoption changed this home's ledger (whether or not it succeeded), so reread everything derived from it.
    // The pen and the broken-chain banner are not touched here.
    shell.source_changed(crate::shell::Source::Ledger);
    remeasure(shell);
    let got = got?;
    // The pen follows the self-audit run after adoption.
    shell.pen = if got.complete {
        crate::auditx::Pen::Granted
    } else {
        crate::auditx::Pen::Held
    };
    shell.reconciled = Some((got.label.clone(), got.complete, got.linked));
    // A ledger adopted in place opens for writing like any other: once its tail is checked against the chain
    // (`check_tail`), after the self-audit above.
    tail_if_due(shell);
    Ok(got)
}

/// Re-measures the home in the background after the ledger changed. A measurement already in flight is not
/// restarted, since its result already reflects the change.
pub(super) fn remeasure(shell: &mut Shell) {
    if let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) {
        shell.tasks.spawn(Kind::Archive, move || measure(&root));
    }
}

/// Walks the disk to measure a home. Runs on a background thread, so it never touches egui.
pub(super) fn measure(root: &std::path::Path) -> Result<Done, crate::fault::Fault> {
    crate::task::stage_at(crate::task::Kind::Archive, 0);
    let home = crate::home::Home::open(root)?;
    let bytes = home.usage()?;
    let (items, skipped, records) = match home.ledger().and_then(|l| {
        l.survey()
    }) {
        Ok(s) => {
            let records = s.items.iter().filter(|b| zikaron::entry::check(b).map(|e| e.kind == zikaron::tokens::EntryType::History).unwrap_or(false)).count();
            (s.items.len(), s.skipped.len(), records)
        }
        Err(_) => (0, 0, 0),
    };
    // The item count recorded at the last backup (`backup::measured`).
    let machine_items = crate::machine::read().and_then(|m| crate::backup::measured(m.backup.as_ref()));
    // The last exported bundle's state needs the settings and a full bundle verification, both disk reads, so
    // it is done in this background pass rather than on the UI thread.
    let rec = crate::settings::Settings::read(&home)?.mirror;
    let mirror = crate::mirror::status(rec.as_ref());
    Ok(Done::Archive { bytes, items, skipped, mirror, records, machine_items })
}
