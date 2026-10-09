use super::*;

pub(super) fn open_entry(shell: &mut Shell, id: &str) -> Result<(String, usize), crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let d = crate::ledgerx::detail(home, id)?;
    Ok((d.id, d.bytes.len()))
}

/// Writes an annotation. Published bytes never change; a correction is an annotation.
pub(super) fn annotate(shell: &mut Shell, subject: &str, note_md: &str) -> Result<String, crate::fault::Fault> {
    let subject = subject.trim().to_string();
    let note_md = note_md.trim().to_string();
    if note_md.is_empty() {
        // `note_md` is required; refuse an empty one here so the person sees why before signing.
        return Err(crate::fault::Fault::known(
            crate::fault::Known::EntryRefused,
            crate::lang::t(crate::lang::Key::Tail019).to_string(),
        ));
    }
    let subject = if subject.is_empty() {
        None
    } else {
        // The annotated entry must be in this ledger: check the disk once and refuse by name if it is absent,
        // so no dangling annotation reaches the chain.
        let home = shell.home.as_ref().ok_or_else(|| {
            crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
        })?;
        // Check the entry id's shape first (`0x` plus 64 lowercase hex digits): a subject written any other way
        // is refused for its shape, never looked up leniently or reported absent.
        if !zikaron::hexfmt::is_hex32(&subject) {
            return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, subject));
        }
        crate::ledgerx::detail(home, &subject).map_err(|_| {
            crate::fault::Fault::known(crate::fault::Known::SubjectMissing, subject.clone())
        })?;
        Some(subject)
    };
    let body = crate::anchorx::annotation_body(subject.as_deref(), &note_md);
    append_entry(shell, zikaron::tokens::EntryType::Annotation, body)
}

/// Deletes a record (the app's retraction convention, `retractx`). The ledger is append-only: a new entry is
/// written and the original record stays as it was.
///
/// Only the valid form is written: pointing at a `history` entry in this ledger that is not already deleted.
/// Invalid forms (other tools might write them) are still read as they are, but cannot be produced here. If
/// the deleted record is still in the anchor queue it is removed (it will no longer be anchored as a record);
/// the delete entry itself is queued as usual.
pub(super) fn retract(shell: &mut Shell, subject: &str, note_md: &str) -> Result<(String, bool, Enqueued, bool), crate::fault::Fault> {
    let subject = subject.trim().to_string();
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    let rows = crate::ledgerx::table(home, None, &queued)?.rows;
    // The rule lives in `zikaron_glue::retraction::may_retract` (also used by the command line's `retract`):
    // the subject is well formed, is a history entry in this ledger, and is not already deleted. Otherwise it
    // is refused by name, so an invalid delete is never written.
    let target = crate::retractx::may_retract(&rows, &subject).map_err(|why| {
        use crate::retractx::Invalid;
        match why {
            // Not shaped like an entry id is a shape error; well formed but not here is absent.
            Invalid::Shape => crate::fault::Fault::known(crate::fault::Known::ContentShape, subject.clone()),
            Invalid::NotInLedger => crate::fault::Fault::known(crate::fault::Known::SubjectMissing, subject.clone()),
            Invalid::NotAWork => crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::t(crate::lang::Key::Tail229).to_string()),
            Invalid::Repeated => crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::t(crate::lang::Key::Tail230).to_string()),
        }
    })?;
    // Whether the retracted item was published. Submitted, included, recorded as anchored in the queue file,
    // or anchored per the last report means published: the delete entry must be queued so chain readers know
    // the item no longer counts. Queued, reverted, refused before broadcast, or out of the queue with neither
    // source saying anchored means unpublished: the pair stays local, and any later anchored entry bounds
    // their existence through `prev`, so nothing is lost. The rule lives only in `queue::Queue::published`
    // (the ledger table's status lights use it too).
    let report_ids: Vec<String> = shell
        .audit
        .as_ref()
        .map(|a| crate::ledgerx::anchored_of(&a.report).into_iter().map(|(h, _)| h).collect())
        .unwrap_or_default();
    let published = shell.queue.published(&target, &report_ids);
    // If publication cannot be determined, do not guess. Judging "unpublished" needs an authoritative source:
    // the last audit report or the queue file knowing this entry. With neither (another machine, restored
    // from backup, a fresh queue file), assuming unpublished could keep the delete local while the item is in
    // fact on chain, and chain readers would never see the delete. So it is refused by name and the person
    // syncs first. Only a home that can anchor is checked: a ledger without a chain never publishes, its
    // deletes stay local anyway, and blocking would leave nothing to sync with.
    let can_anchor = shell.settings.chain_id.is_some() && !shell.settings.endpoints.is_empty();
    if can_anchor && !published && shell.audit.is_none() && !shell.queue.has(&target) {
        return Err(crate::fault::Fault::known(crate::fault::Known::NotAudited, target.clone()));
    }
    let in_flight = shell.queue.step_of(&target).is_some_and(crate::queue::Step::in_flight);
    let body = crate::retractx::body(&target, note_md);
    let id = append_typed(shell, crate::retractx::ENTRY_TYPE, body)?;
    // Record first, then touch the queue, so a failed recording leaves the queue untouched. Once recorded, the
    // deleted record is no longer anchored as a record (a transaction already submitted and awaiting its
    // receipt is left to finish).
    let dropped = match shell.home.as_ref() {
        Some(h) if !in_flight => match crate::queue::amend(h, |q| q.drop_ids(std::slice::from_ref(&target))) {
            Ok((n, q)) => {
                shell.queue = q;
                n > 0
            }
            Err(f) => {
                shell.faults.push(f);
                false
            }
        },
        _ => false,
    };
    let n = if published { queue_it(shell, &id) } else { Enqueued { queued: shell.queue.len(), next: Next::Held } };
    Ok((id, dropped, n, !published))
}

/// Appends a new entry: read the head, seal the envelope, record it.
///
/// The invariant "seq follows the head and prev points at the head" is enforced here and only here;
/// annotations and history entries share this code, so they never follow different heads.
pub(super) fn append_entry(
    shell: &mut Shell,
    kind: zikaron::tokens::EntryType,
    body: zikaron::json::Value,
) -> Result<String, crate::fault::Fault> {
    append_typed(shell, kind.as_str(), body)
}

/// Like `append_entry`, with the entry type given as a raw string (the type list is open, so types beyond the
/// seven built-in ones can only be written this way).
pub(super) fn append_typed(
    shell: &mut Shell,
    entry_type: &str,
    body: zikaron::json::Value,
) -> Result<String, crate::fault::Fault> {
    shell.may_write_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let head = crate::ledgerx::head(home)?.ok_or_else(|| {
        crate::fault::Fault::known(
            crate::fault::Known::NoGenesis,
            crate::lang::t(crate::lang::Key::Tail020).to_string(),
        )
    })?;
    let secret = signing_key(shell, crate::sign::Use::Sign(crate::sign::Face::Entry))?;
    shell.flow.sign = crate::anchorx::Step::Waiting;
    shell.flow.land = crate::anchorx::Step::Waiting;
    shell.flow.queue = crate::anchorx::Step::Waiting;
    let sealed = match crate::entryx::seal(&secret, entry_type, head.0 + 1, Some(&head.1), body) {
        Ok(x) => {
            shell.flow.sign = crate::anchorx::Step::Done;
            x
        }
        Err(f) => {
            shell.flow.sign = crate::anchorx::Step::Failed;
            return Err(f);
        }
    };
    land_sealed(shell, sealed)
}

/// The single recording step: annotations, history entries and grants all write ledger bytes through it.
pub(super) fn land_sealed(
    shell: &mut Shell,
    sealed: crate::entryx::Sealed,
) -> Result<String, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let ledger = home.ledger()?;
    let bare = sealed.id.trim_start_matches("0x");
    let name = zikaron_store::EntryName::parse(bare).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::Ledger, crate::lang::filln(crate::lang::Key::Tail012, &[&(bare).to_string()]))
    })?;
    match ledger.append(&name, &sealed.bytes) {
        Ok(_) => {
            shell.flow.land = crate::anchorx::Step::Done;
            // The ledger moved on: the last self-audit report describes the ledger before this entry.
            shell.book_changed();
        }
        Err(t) => {
            shell.flow.land = crate::anchorx::Step::Failed;
            return Err(crate::fault::Fault::known(
                crate::fault::Known::Ledger,
                format!("{t:?}"),
            ));
        }
    }
    Ok(sealed.id)
}

/// The three checks before appending: may this instance write, where is the head, is the key present.
pub(super) fn ready_to_append(
    shell: &mut Shell,
) -> Result<((u64, String), crate::key::Secret), crate::fault::Fault> {
    shell.may_write_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let head = crate::ledgerx::head(home)?.ok_or_else(|| {
        crate::fault::Fault::known(
            crate::fault::Known::NoGenesis,
            crate::lang::t(crate::lang::Key::Tail020).to_string(),
        )
    })?;
    let secret = signing_key(shell, crate::sign::Use::Sign(crate::sign::Face::Entry))?;
    shell.flow = crate::anchorx::Flow::default();
    Ok((head, secret))
}

/// Queues right after recording. A failed queueing does not undo the recorded entry (the bytes are already in
/// the ledger); it only marks the queue step failed, and the person can queue again.
///
/// Sets the record flow's queue step; queueing itself is [`enqueue`].
pub(super) fn queue_it(shell: &mut Shell, id: &str) -> Enqueued {
    let (landed, n) = enqueue(shell, id);
    shell.flow.queue = if landed { crate::anchorx::Step::Done } else { crate::anchorx::Step::Failed };
    n
}

/// Queueing itself: write to disk, report each outcome, and decide whether to send or wait. Returns (queued or
/// already queued, what to do next). The record flow's steps are not set here: genesis and the root backfill
/// on home opening also use this path and are not records, so setting them would make the records page say
/// "queued" before any record was signed.
pub(super) fn enqueue(shell: &mut Shell, id: &str) -> (bool, Enqueued) {
    let Some(home) = shell.home.as_ref() else {
        return (false, Enqueued { queued: shell.queue.len(), next: Next::Held });
    };
    // The on-disk table is changed in only one place (`queue::amend`: read, change and write under one lock).
    // Writing back the shell's copy while the background removes entries would let two stale tables overwrite
    // each other, and the just-queued entry could vanish from both disk and shell. The returned table is the
    // on-disk state after writing.
    let at = now_secs();
    let landed = match crate::queue::amend(home, |q| q.push(id, at)) {
        Ok((said, q)) => {
            shell.queue = q;
            // Queued and already queued both complete this step. "Recorded as anchored" does not: queueing
            // again would only anchor the same entry twice, so the step fails and says so by name (a boolean
            // from `push` would quietly treat this as success).
            match said {
                crate::queue::Pushed::Queued | crate::queue::Pushed::InQueue => true,
                crate::queue::Pushed::Anchored => {
                    shell.faults.push(crate::fault::Fault::known(crate::fault::Known::AlreadyAnchored, id.to_string()));
                    false
                }
            }
        }
        Err(f) => {
            shell.faults.push(f);
            false
        }
    };
    // Send or wait is decided here by the "auto anchor" setting, since every queueing path goes through this
    // function. The result is a closed enum every path must pass on, so the compiler makes any new anchoring
    // path handle it.
    let next = if !landed {
        Next::Held
    } else if shell.settings.auto_anchor {
        Next::Send
    } else {
        Next::Wait
    };
    (landed, Enqueued { queued: shell.queue.len(), next })
}
