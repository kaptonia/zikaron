use super::*;

pub(super) fn open_entry(shell: &mut Shell, id: &str) -> Result<(String, usize), crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let d = crate::ledgerx::detail(home, id)?;
    Ok((d.id, d.bytes.len()))
}

/// Write an annotation (law §6.8). Published bytes never change; a correction is an annotation.
pub(super) fn annotate(shell: &mut Shell, subject: &str, note_md: &str) -> Result<String, crate::fault::Fault> {
    let subject = subject.trim().to_string();
    let note_md = note_md.trim().to_string();
    if note_md.is_empty() {
        // §6.8's `note_md` is required. The law would refuse an empty one, and the person should see this
        // sentence before pressing.
        return Err(crate::fault::Fault::known(
            crate::fault::Known::EntryRefused,
            crate::lang::t(crate::lang::Key::Tail019).to_string(),
        ));
    }
    let subject = if subject.is_empty() {
        None
    } else {
        // The annotated entry must be in this ledger. Law §6.8 says `subject` is "the entry_id of an entry in
        // this ledger", and "is it there" beyond the format is a reading; this reads the disk once and
        // refuses by name when it points nowhere, so no dangling annotation lands on chain.
        let home = shell.home.as_ref().ok_or_else(|| {
            crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
        })?;
        crate::ledgerx::detail(home, &subject).map_err(|_| {
            crate::fault::Fault::known(crate::fault::Known::SubjectMissing, subject.clone())
        })?;
        Some(subject)
    };
    let body = crate::anchorx::annotation_body(subject.as_deref(), &note_md);
    append_entry(shell, zikaron::tokens::EntryType::Annotation, body)
}

/// Delete a record (this desk's reading convention, `retractx`). The ledger is append-only: a new entry is
/// written and the original record stays as it was.
///
/// This desk writes only the valid form: pointing at a `history` in this ledger that was not deleted before.
/// Invalid forms are read as they are by the reading (other tools might write them), and this place offers no
/// path for them. If the deleted record is still in the anchor queue it is removed (it will no longer be
/// anchored as a record); the delete entry itself is queued as usual.
pub(super) fn retract(shell: &mut Shell, subject: &str, note_md: &str) -> Result<(String, bool, Enqueued, bool), crate::fault::Fault> {
    let subject = subject.trim().to_string();
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    let rows = crate::ledgerx::table(home, None, &queued)?.rows;
    // The production rule lives in the convention table (`zikaron_glue::retraction::may_retract`; the command
    // line's `retract` asks the same): the subject is well formed, is a history in this ledger, and has not
    // been deleted. Otherwise refused by name, and a delete that would read as invalid is never written.
    let target = crate::retractx::may_retract(&rows, &subject).map_err(|why| {
        use crate::retractx::Invalid;
        match why {
            Invalid::Shape | Invalid::NotInLedger => crate::fault::Fault::known(crate::fault::Known::SubjectMissing, subject.clone()),
            Invalid::NotAWork => crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::t(crate::lang::Key::Tail229).to_string()),
            Invalid::Repeated => crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::t(crate::lang::Key::Tail230).to_string()),
        }
    })?;
    // Rule: whether the retracted item was published. Submitted, included, recorded as anchored in the
    // queue file, or anchored per the last report means published: the delete entry must be queued so chain
    // readers know that item no longer counts. Queued, reverted, refused before broadcast, or out of the
    // queue with neither source saying anchored means unpublished: the pair stays local, and any later
    // anchored entry bounds their existence along `prev` (law §9.6), so nothing is lost legally. The
    // rule lives only in `queue::Queue::published` (the ledger table's lights ask it too).
    let report_ids: Vec<String> = shell
        .audit
        .as_ref()
        .map(|a| crate::ledgerx::anchored_of(&a.report).into_iter().map(|(h, _)| h).collect())
        .unwrap_or_default();
    let published = shell.queue.published(&target, &report_ids);
    // When publication cannot be told, do not guess: judging "unpublished" needs an authoritative reading,
    // the last audit report or the queue file knowing this entry. With neither (another machine, restored
    // from backup, a fresh queue file), defaulting to unpublished would keep the delete only locally, while
    // the item was in fact published on chain, and chain readers would never see the delete. Without a
    // reading it is refused by name, so the person syncs first. Only a home that can anchor asks this: a
    // local ledger without a chain never publishes, deletes stay local anyway, and blocking it would make the
    // path unusable (with nowhere to "go sync").
    let can_anchor = shell.settings.chain_id.is_some() && !shell.settings.endpoints.is_empty();
    if can_anchor && !published && shell.audit.is_none() && !shell.queue.has(&target) {
        return Err(crate::fault::Fault::known(crate::fault::Known::NotAudited, target.clone()));
    }
    let in_flight = matches!(shell.queue.step_of(&target), Some(crate::queue::Step::Submitted { .. }));
    let body = crate::retractx::body(&target, note_md);
    let id = append_typed(shell, crate::retractx::ENTRY_TYPE, body)?;
    // Record first, then touch the queue. If recording fails, the queue is untouched; once recorded, the
    // deleted record no longer goes on chain as a record (a submitted transaction waiting for its receipt is
    // already on its way and is left to get its receipt).
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

/// Append a new entry. Three steps: ask for the head, seal the envelope through the thirteen steps, record.
///
/// "seq follows the head and prev points at the head" rests here, and only here: annotations and history entries
/// go through the same code, so the two paths never follow different heads.
pub(super) fn append_entry(
    shell: &mut Shell,
    kind: zikaron::tokens::EntryType,
    body: zikaron::json::Value,
) -> Result<String, crate::fault::Fault> {
    append_typed(shell, kind.as_str(), body)
}

/// As `append_entry`, with the type given as its raw word (law §6.9's open enumeration: types beyond the
/// seven can only be said this way).
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

/// The recording step, one owner. Annotations, history entries and grants all go through it, so the three
/// paths never each write their own "how bytes go into the ledger".
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
            // The ledger moved one step: the last self-audit report describes the ledger before this entry.
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

/// The three things before appending: is the pen held, where is the head, is the key present.
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

/// Queue right after recording. A failed queueing does not take back the recorded entry: the bytes are
/// already in the ledger, and taking them back would be a lie; this step only marks the queue cell red, and
/// the person queues again.
///
/// The record flow's cell (sign, record, queue) is lit here; queueing itself is [`enqueue`].
pub(super) fn queue_it(shell: &mut Shell, id: &str) -> Enqueued {
    let (landed, n) = enqueue(shell, id);
    shell.flow.queue = if landed { crate::anchorx::Step::Done } else { crate::anchorx::Step::Failed };
    n
}

/// Queueing itself: write to disk, speak per form, and decide send or wait next. Returns (queued or already
/// queued, what to do after queueing). The record flow's three cells are not here: genesis and the root
/// backfill at home opening use this exit too, and they are not records; lighting that cell would make the
/// records page say "queued" before any record was signed.
pub(super) fn enqueue(shell: &mut Shell, id: &str) -> (bool, Enqueued) {
    let Some(home) = shell.home.as_ref() else {
        return (false, Enqueued { queued: shell.queue.len(), next: Next::Held });
    };
    // The table on disk is changed in only one place (`queue::amend`: read, change and write under one lock).
    // Changing the shell's copy and writing it back, while the background removes entries at the same time,
    // would let two stale tables overwrite each other, and the just-queued entry would vanish from disk and
    // shell together. The table returned is the on-disk state after writing.
    let at = now_secs();
    let landed = match crate::queue::amend(home, |q| q.push(id, at)) {
        Ok((said, q)) => {
            shell.queue = q;
            // Each form speaks its own sentence. Queued and already queued both count as this step done;
            // "this file records it as anchored" is another matter: queueing it again would only anchor the
            // same entry again, so this step turns red and says so by name (a boolean from `push` would
            // quietly treat this form as success).
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
    // Send or wait next is answered here by the "auto anchor" cell: all nine queueing paths share this exit;
    // the return value is a closed table that every path must hand on, so the compiler forces handling when a
    // new anchoring path is added.
    let next = if !landed {
        Next::Held
    } else if shell.settings.auto_anchor {
        Next::Send
    } else {
        Next::Wait
    };
    (landed, Enqueued { queued: shell.queue.len(), next })
}
