use super::*;

pub(super) fn genesis(shell: &mut Shell, statement: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    shell.may_hold_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let survey = home
        .ledger()?
        .survey()?;
    if !survey.items.is_empty() || !survey.skipped.is_empty() {
        let checked: Vec<Option<zikaron_glue::book::Entry>> =
            survey.items.iter().map(|b| zikaron_glue::book::check(b).ok()).collect();
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
                crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail009, &[&(roots.len()).to_string()])),
            ));
        }
        let refused = checked.iter().filter(|e| e.is_none()).count();
        let entries = checked.len() - refused;
        return Err(crate::fault::Fault::known(
            crate::fault::Known::Ledger,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail010, &[&(checked.len() + survey.skipped.len()).to_string(), &(entries).to_string(), &(refused).to_string(), &(survey.skipped.len()).to_string()])),
        ));
    }
    shell.may_write_entries()?;
    let secret = signing_key(shell, crate::sign::Use::Sign(crate::sign::Face::Entry))?;
    let sealed = crate::entryx::genesis(&secret, statement)?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let ledger = home.ledger()?;
    let bare = sealed.id.trim_start_matches("0x");
    let name = zikaron_store::EntryName::parse(bare).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::Ledger, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail012, &[&(bare).to_string()])))
    })?;
    ledger
        .append(&name, &sealed.bytes)?;
    shell.book_changed();
    shell.count_here();
    let (_, n) = enqueue(shell, &sealed.id);
    Ok((sealed.id, n))
}

pub(super) fn backfill_root(shell: &mut Shell) {
    if !shell.rooted || shell.unfetched.is_some() || shell.settings.chain_id.is_none() {
        return;
    }
    let known: Option<Vec<String>> = match (shell.audit.as_ref(), shell.remembered.as_ref()) {
        (Some(a), _) if crate::auditx::whole(&a.label, a.asked, &a.unanswered) => Some(crate::ledgerx::anchored_of(&a.report).into_iter().map(|(h, _)| h).collect()),
        (None, Some(r)) if r.whole => Some(r.rows.iter().map(|(h, _)| h.clone()).collect()),
        _ => None,
    };
    let Some(known) = known else { return };
    let Some(root) = shell.kits_root.clone() else { return };
    if known.iter().any(|id| id.eq_ignore_ascii_case(&root)) || shell.queue.has(&root) || shell.queue.anchored_here(&root) {
        return;
    }
    if shell.may_write_entries().is_err() {
        return;
    }
    let _ = enqueue(shell, &root);
}

pub(super) fn adopt(shell: &mut Shell, dir: &str) -> Result<crate::firstrun::Adopted, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    match shell.lock.as_ref() {
        Some(l) if l.mode().writable() => {}
        Some(l) => return Err(crate::fault::Fault::known(crate::fault::Known::ReadOnly, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()])))),
        None => return Err(crate::fault::Fault::known(crate::fault::Known::NoHome, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail005).to_string()))),
    }
    let got = crate::firstrun::adopt(std::path::Path::new(dir), home);
    shell.source_changed(crate::shell::Source::Ledger);
    shell.book_changed();
    shell.count_here();
    remeasure(shell);
    let got = got?;
    shell.pen = if got.complete {
        crate::auditx::Pen::Granted
    } else {
        crate::auditx::Pen::Held
    };
    shell.reconciled = Some((got.label.clone(), got.complete, got.linked));
    tail_if_due(shell);
    Ok(got)
}

pub(super) fn remeasure(shell: &mut Shell) -> Option<crate::task::Spawned> {
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf())?;
    let mark = shell.book_mark;
    let spawned = shell.tasks.spawn(Kind::Archive, move || measure(&root, mark));
    if matches!(spawned, crate::task::Spawned::Started) {
        shell.archive_mark = Some(mark);
    }
    Some(spawned)
}

pub(super) fn measure(root: &std::path::Path, mark: u64) -> Result<Done, crate::fault::Fault> {
    crate::task::stage_at(crate::task::Kind::Archive, 0);
    let home = crate::home::Home::open(root)?;
    let bytes = home.usage()?;
    let (items, skipped, records) = match home.ledger().and_then(|l| {
        l.survey()
    }) {
        Ok(s) => {
            let records = s.items.iter().filter(|b| zikaron_glue::book::check(b).map(|e| e.kind == zikaron_glue::book::Kind::History).unwrap_or(false)).count();
            (s.items.len(), s.skipped.len(), records)
        }
        Err(_) => (0, 0, 0),
    };
    let machine_items = crate::machine::read().and_then(|m| crate::backup::measured(m.backup.as_ref()));
    let rec = crate::settings::Settings::read(&home)?.mirror;
    let mirror = crate::mirror::status(rec.as_ref());
    let backed = crate::backup::content_now().ok();
    Ok(Done::Archive { bytes, items, skipped, mirror, records, machine_items, backed, mark })
}
