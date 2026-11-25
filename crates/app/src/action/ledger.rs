use super::*;

pub(super) fn open_entry(shell: &mut Shell, id: &str) -> Result<(String, usize), crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let d = crate::ledgerx::detail(home, id)?;
    Ok((d.id, d.bytes.len()))
}

pub(super) fn annotate(shell: &mut Shell, subject: &str, note_md: &str) -> Result<String, crate::fault::Fault> {
    let subject = subject.trim().to_string();
    let note_md = note_md.trim().to_string();
    if note_md.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::EntryRefused,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail019).to_string()),
        ));
    }
    let subject = if subject.is_empty() {
        None
    } else {
        let home = shell.home.as_ref().ok_or_else(|| {
            crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
        })?;
        let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
        let rows = crate::ledgerx::table(home, None, &queued)?.rows;
        zikaron_glue::retraction::annotation::may_name(&crate::retractx::lines(&rows), &subject).map_err(|why| {
            let f = match why {
                zikaron_glue::retraction::annotation::Invalid::Shape => crate::fault::Fault::known(crate::fault::Known::ContentShape, subject.clone()),
                zikaron_glue::retraction::annotation::Invalid::NotInLedger => crate::fault::Fault::known(crate::fault::Known::SubjectMissing, subject.clone()),
                zikaron_glue::retraction::annotation::Invalid::NotATarget => crate::fault::Fault::known(crate::fault::Known::NotATarget, subject.clone()),
            };
            f.ruled(zikaron_glue::door::Rule::Annotation, Some(why.token()), Some(subject.clone()), None)
        })?;
        Some(subject)
    };
    let body = crate::anchorx::annotation_body(subject.as_deref(), &note_md);
    let id = append_entry(shell, zikaron_glue::book::Kind::Annotation, body)?;
    queue_it(shell, &id);
    Ok(id)
}

pub(super) fn retract(shell: &mut Shell, subject: &str, note_md: &str) -> Result<(String, bool, Enqueued, bool), crate::fault::Fault> {
    let subject = subject.trim().to_string();
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    let rows = crate::ledgerx::table(home, None, &queued)?.rows;
    let target = crate::retractx::may_retract(&rows, &subject).map_err(|why| {
        use crate::retractx::Invalid;
        let f = match why {
            Invalid::Shape => crate::fault::Fault::known(crate::fault::Known::ContentShape, subject.clone()),
            Invalid::NotInLedger => crate::fault::Fault::known(crate::fault::Known::SubjectMissing, subject.clone()),
            Invalid::NotAWork => crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail229).to_string())),
            Invalid::Repeated => crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail230).to_string())),
        };
        f.ruled(zikaron_glue::door::Rule::Retraction, Some(why.token()), None, None)
    })?;
    let last: Option<Vec<String>> = shell.remembered.as_ref().map(|a| a.rows.iter().map(|(id, _)| id.clone()).collect());
    let plan = shell
        .queue
        .retracting(&target, last.as_deref(), shell.settings.ground().can_anchor())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NotAudited, target.clone()).ruled(zikaron_glue::door::Rule::Retraction, Some(zikaron_glue::door::RETRACT_NOT_AUDITED), Some(target.clone()), None))?;
    let published = plan.publish;
    let body = crate::retractx::body(&target, note_md);
    let id = append_typed(shell, crate::retractx::ENTRY_TYPE, body)?;
    let dropped = match shell.home.as_ref() {
        Some(h) => match crate::queue::amend(h, |q| q.retracted(&target, &id, plan)) {
            Ok((n, q)) => {
                shell.queue = q;
                n > 0
            }
            Err(f) => {
                shell.faults.push(f);
                false
            }
        },
        None => false,
    };
    let n = if published { queue_it(shell, &id) } else { Enqueued { queued: shell.queue.len(), next: Next::Held } };
    Ok((id, dropped, n, !published))
}

pub(super) fn append_entry(
    shell: &mut Shell,
    kind: zikaron_glue::book::Kind,
    body: zikaron::json::Value,
) -> Result<String, crate::fault::Fault> {
    append_typed(shell, kind.as_str(), body)
}

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
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail020).to_string()),
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

pub(super) fn land_sealed(
    shell: &mut Shell,
    sealed: crate::entryx::Sealed,
) -> Result<String, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let ledger = home.ledger()?;
    if let Ok(pile) = ledger.pile() {
        match zikaron_glue::book::new_findings(&pile.items, &sealed.bytes, None) {
            Ok(names) if names.is_empty() => {}
            Ok(names) => {
                let f = crate::fault::Fault::known(crate::fault::Known::WouldBreak, names.join(" ")).ruled(zikaron_glue::door::Rule::WouldBreak, None, Some(sealed.id.clone()), Some(names));
                return Err(if shell.handed.is_some() { f.worded(Some(crate::lang::Key::FaultWhatHandedOver), Some(crate::lang::Key::FaultNextHandedOver)) } else { f });
            }
            Err(zikaron_glue::book::Barred::Signer { counted, key, office }) => {
                return Err(crate::fault::Fault::known(crate::fault::Known::WouldBreak, format!("key={key} office={office}")).ruled(zikaron_glue::door::Rule::WouldBreak, Some(counted.as_str()), Some(sealed.id.clone()), Some(Vec::new())))
            }
            Err(zikaron_glue::book::Barred::Tip(_)) => return Err(crate::fault::Fault::known(crate::fault::Known::ForkedRoot, home.root().display().to_string())),
        }
    }
    let bare = sealed.id.trim_start_matches("0x");
    let name = zikaron_store::EntryName::parse(bare).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::Ledger, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail012, &[&(bare).to_string()])))
    })?;
    match ledger.append(&name, &sealed.bytes) {
        Ok(_) => {
            shell.flow.land = crate::anchorx::Step::Done;
            shell.landed = Some((sealed.id.clone(), sealed.seq));
            shell.book_changed();
            shell.count_here();
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
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail020).to_string()),
        )
    })?;
    let secret = signing_key(shell, crate::sign::Use::Sign(crate::sign::Face::Entry))?;
    shell.flow = crate::anchorx::Flow::default();
    Ok((head, secret))
}

pub(super) fn queue_it(shell: &mut Shell, id: &str) -> Enqueued {
    let (landed, n) = enqueue(shell, id);
    shell.flow.queue = if landed { crate::anchorx::Step::Done } else { crate::anchorx::Step::Failed };
    n
}

pub(super) fn enqueue(shell: &mut Shell, id: &str) -> (bool, Enqueued) {
    let Some(home) = shell.home.as_ref() else {
        return (false, Enqueued { queued: shell.queue.len(), next: Next::Held });
    };
    let at = now_secs();
    let place = home.ledger().ok().and_then(|l| l.pile().ok()).map(|p| zikaron_seal::queue::places_of(&p.items)).and_then(|m| m.get(&id.to_ascii_lowercase()).copied());
    let others: Vec<u64> = shell.basis_chains().into_iter().skip(1).map(|(c, _, _)| c).collect();
    let landed = match crate::queue::amend(home, |q| q.push_placed(id, place.is_some_and(|p| p.alone), place.and_then(|p| p.carried_on), &others, at)) {
        Ok((said, q)) => {
            shell.queue = q;
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
    let next = if !landed {
        Next::Held
    } else if shell.settings.auto_anchor {
        Next::Send
    } else {
        Next::Wait
    };
    (landed, Enqueued { queued: shell.queue.len(), next })
}
