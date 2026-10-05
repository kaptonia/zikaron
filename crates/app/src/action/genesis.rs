use super::*;

pub(super) fn genesis(shell: &mut Shell, statement: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    shell.may_write_entries()?;
    // One ledger, one root: genesis lands only in an empty ledger. Genesis is the ledger's first entry; if
    // the ledger holds anything at all (the pile from the store crate's lenient read, or skipped files), the
    // new one would not be first. So the precondition is that both the pile and the skipped list are empty,
    // and there is no second path to write it (the same rule as the CLI's `init`, both reading the store
    // crate and the core).
    //
    // Asking only "skipped list non-empty refuses, a seq 0 in the pile the core recognizes refuses, otherwise
    // write" is not enough: a file with an entry name but broken bytes is read into the pile by the store
    // (not the skipped list), the core refuses it and it is not counted, so a root would be written beside
    // it; a ledger with entries but no root likewise. So the check asks "is it empty".
    //
    // Refusals use this desk's existing words: the core recognizing seq 0 ids (one or several) →
    // ALREADY_ROOTED; any other non-empty → LEDGER. The evidence tail is a gap turned into an action
    // sentence: with the item count, saying how to get out from this home. All judging is in the store and
    // the core; this layer only counts.
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let survey = home
        .ledger()?
        .survey()?;
    if !survey.items.is_empty() || !survey.skipped.is_empty() {
        let checked: Vec<Option<zikaron::entry::Entry>> =
            survey.items.iter().map(|b| zikaron::entry::check(b).ok()).collect();
        // seq 0 entries the core recognizes, deduplicated by id (one entry stored under two names is one
        // root; two entries from the same key are two roots).
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
    // The root is queued right away: the same exit as records, grants and published revocations (`enqueue`,
    // the queueing body of `queue_it`). With auto anchor on it is sent with the queue; off, it stays queued
    // for manual sending (returning right after writing genesis would leave the root never anchored).
    let (_, n) = enqueue(shell, &sealed.id);
    Ok((sealed.id, n))
}

/// Queue an existing ledger's root once. A ledger recorded before genesis was queued has its root neither
/// queued nor anchored; without this, that ledger could never show an anchored root. Triggered once by the
/// home-opening verb, not by the wall clock.
///
/// Only on the side that can write this ledger (readers, broken chain, held pen, handed over, restored but
/// not fetched: none).
///
/// Only with a chain reading in hand: this home has a chain id, and there is this pass's audit report or the
/// last audit's set (the disk cache, filtered by the current chain id). Queuing without a reading would, for
/// a restored, fetched or adopted ledger whose root was anchored long ago while both local records are empty,
/// queue it again and pay gas again. Knowing nothing means no backfill; it waits until an audit has run and
/// the home is opened again. With a reading, any of "in the reading", "queued in the queue file", "recorded
/// as anchored in the queue file" means no backfill. Queueing goes through the same [`enqueue`]; it is not a
/// press by the person, so "send or wait next" shows no confirmation card, and the root stays queued for the
/// next anchoring.
pub(super) fn backfill_root(shell: &mut Shell) {
    if !shell.rooted || shell.unfetched.is_some() || shell.may_write_entries().is_err() || shell.settings.chain_id.is_none() {
        return;
    }
    // Only a whole reading counts (`auditx::whole`): in an incomplete one, "root not in the anchor set" does
    // not mean the root is unanchored, and backfilling would anchor it again.
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
    // Adoption changed this home's ledger (on success or failure): what was read from the ledger is reread
    // from the new source; the pen and the broken-chain banner are not touched here.
    shell.source_changed(crate::shell::Source::Ledger);
    remeasure(shell);
    let got = got?;
    // The pen follows what the self-audit after adoption said.
    shell.pen = if got.complete {
        crate::auditx::Pen::Granted
    } else {
        crate::auditx::Pen::Held
    };
    shell.reconciled = Some((got.label.clone(), got.complete, got.linked));
    // A ledger adopted in place opens for writing as any other does: once its tail is checked against the
    // chain (`check_tail`), after the self-audit above.
    tail_if_due(shell);
    Ok(got)
}

/// Measure again after the ledger changed (background; not started again while one is in flight, whose landed
/// reading is already after the change).
pub(super) fn remeasure(shell: &mut Shell) {
    if let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) {
        shell.tasks.spawn(Kind::Archive, move || measure(&root));
    }
}

/// Walk the disk to measure a home. Runs on a background thread, so it touches no egui.
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
    let machine_items = crate::backup::count_now().ok();
    // The state of the last exported bundle needs reading settings and verifying the whole bundle, both disk
    // reads, so it goes with this background pass rather than staying in the frame.
    let rec = crate::settings::Settings::read(&home)?.mirror;
    let mirror = crate::mirror::status(rec.as_ref());
    Ok(Done::Archive { bytes, items, skipped, mirror, records, machine_items })
}
