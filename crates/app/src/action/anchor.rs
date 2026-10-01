use super::*;

pub(super) fn record_work(shell: &mut Shell, note_md: &str, target: Option<&crate::anchorx::For>) -> Result<(String, Enqueued), crate::fault::Fault> {
    let content = shell
        .content
        .as_ref()
        .map(|c| c.digest)
        .ok_or_else(|| {
            crate::fault::Fault::known(
                crate::fault::Known::ContentShape,
                crate::lang::t(crate::lang::Key::Tail026).to_string(),
            )
        })?;
    let m = crate::anchorx::mode();
    let body = crate::anchorx::history_body_for(&content, &m, note_md, target);
    let id = append_entry(shell, zikaron::tokens::EntryType::History, body)?;
    let n = queue_it(shell, &id);
    // A signed file is recorded in the local index (file name and path).
    if let Some(c) = shell.content.clone().filter(|c| c.source == crate::anchorx::Source::File) {
        index_record(shell, &c, &id);
    }
    // When a git repository was anchored, record this commit: the passive indicator compares against it next
    // time.
    if let (Some(c), Some(rec)) = (shell.content.as_ref(), shell.settings.repo.clone()) {
        // The stored path is absolute (`home::kept`); the path given when taking content is resolved the same
        // way before comparing.
        let same = crate::home::kept(&c.subject).map(|p| p.display().to_string() == rec.path).unwrap_or(false);
        if c.source == crate::anchorx::Source::Git && same {
            if let Ok(h) = crate::gitx::head_of(std::path::Path::new(&rec.path)) {
                shell.commit_settings(|s| s.repo = Some(crate::settings::RepoRecord { path: rec.path, last_commit: h.commit }))?;
            }
        }
    }
    Ok((id, n))
}

/// Batch signing: one `history` per file, each signed, queued and recorded in the local index; the first
/// failure stops, returning those signed and which file it stopped at (signed ones are not rolled back: the
/// bytes are in the ledger, and taking them back would be a lie).
pub(super) fn record_files(
    shell: &mut Shell,
    note_md: &str,
    files: &[String],
    target: Option<&crate::anchorx::For>,
) -> (Vec<String>, Option<(usize, String, crate::fault::Fault)>, Enqueued) {
    let mut ids = Vec::new();
    let mut n = Enqueued { queued: shell.queue.len(), next: Next::Held };
    let m = crate::anchorx::mode();
    for (i, p) in files.iter().enumerate() {
        let one = crate::anchorx::of_file(std::path::Path::new(p.trim())).and_then(|c| {
            let body = crate::anchorx::history_body_for(&c.digest, &m, note_md, target);
            append_entry(shell, zikaron::tokens::EntryType::History, body).map(|id| (c, id))
        });
        match one {
            Ok((c, id)) => {
                n = queue_it(shell, &id);
                index_record(shell, &c, &id);
                ids.push(id);
            }
            Err(f) => return (ids, Some((i, p.clone(), f)), n),
        }
    }
    (ids, None, n)
}

/// Record in the local index after signing one. Failure to record does not take the entry back (it is in the
/// ledger); the trouble is still shown on the face.
pub(super) fn index_record(shell: &mut Shell, c: &crate::anchorx::Content, id: &str) {
    let row = (|| -> Result<crate::recordsx::Row, crate::fault::Fault> {
        let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
        let seq = home
            .ledger()?
            .pile()?
            .items
            .iter()
            .filter_map(|b| zikaron::entry::check(b).ok())
            .find(|e| e.id_hex().eq_ignore_ascii_case(id))
            .map(|e| e.seq)
            .unwrap_or(0);
        let path = std::path::Path::new(&c.subject);
        Ok(crate::recordsx::Row {
            root: crate::ledgerx::root_of(home)?,
            content: c.hex(),
            id: id.to_string(),
            seq,
            name: path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default(),
            // If the file is still there at signing, take its real path; if not (just moved), still make it
            // absolute through `home::kept`, never storing a relative path.
            path: match std::fs::canonicalize(path) {
                Ok(p) => p,
                Err(_) => crate::home::kept(&c.subject)?,
            }
            .display()
            .to_string(),
        })
    })()
    .and_then(|row| crate::recordsx::add(&crate::home::machine_dir()?, row));
    if let Err(f) = row {
        shell.faults.push(f);
    }
}

/// "Verify a file": compute the digest now and read the current home's ledger now; anchors are told from this
/// pass's audit report and fragment.
pub(super) fn verify_file(shell: &mut Shell, path: &str) -> Result<crate::recordsx::Verdict, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let report = shell.audit.as_ref().map(|a| &a.report);
    let fragment = shell.audit.as_ref().map(|a| &a.fragment);
    crate::recordsx::verify(home, &crate::home::machine_dir()?, std::path::Path::new(path.trim()), report, fragment, shell.remembered.as_ref())
}

pub(super) fn set_kit_link(shell: &mut Shell, path: &str, link: &str) -> Result<Option<String>, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let room = home.dir(crate::home::Slot::Kits);
    let root = crate::ledgerx::root_of(home)?;
    let got = crate::kitsindex::set_link(&crate::home::machine_dir()?, path, link, &root, &room, shell.settings.publish.as_deref())?;
    shell.reread_kits();
    Ok(got)
}

pub(super) fn drop_kit_copy(shell: &mut Shell, path: &str) -> Result<(crate::kitsindex::Row, bool), crate::fault::Fault> {
    let got = crate::kitsindex::drop_copy(&crate::home::machine_dir()?, path)?;
    shell.reread_kits();
    Ok(got)
}

/// Which entry a dropped path goes to. Recognized by reading the disk now: a directory containing `.git`
/// takes the git path, other directories the directory path, regular files the file path; none of the three
/// is refused by name.
///
/// This recognition lives in the action layer, not the frame: the window side reads no disk at all (checked
/// by the self-check suite).
pub(super) fn take_dropped(
    shell: &mut Shell,
    path: &str,
) -> Result<(crate::anchorx::Source, String), crate::fault::Fault> {
    let p = std::path::Path::new(path.trim());
    let md = std::fs::symlink_metadata(p)
        .map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
    let source = if md.is_dir() {
        if p.join(".git").exists() {
            crate::anchorx::Source::Git
        } else {
            crate::anchorx::Source::Dir
        }
    } else if md.is_file() {
        crate::anchorx::Source::File
    } else {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NotAdoptable,
            crate::lang::filln(crate::lang::Key::Tail027, &[&(p.display()).to_string()]),
        ));
    };
    let c = crate::anchorx::of(source, p)?;
    let hex = c.hex();
    shell.content = Some(c);
    shell.flow = crate::anchorx::Flow::default();
    Ok((source, hex))
}

pub(super) fn register_repo(shell: &mut Shell, path: &str) -> Result<String, crate::fault::Fault> {
    // Made absolute before saving (`home::kept`): starting from another directory next time still reads the
    // registered place.
    let p = crate::home::kept(path)?.display().to_string();
    // Open it once for real before registering: a path that cannot be opened, once registered, would only
    // turn red the next time the page opens.
    let h = crate::gitx::head_of(std::path::Path::new(&p))?;
    let last = shell
        .settings
        .repo
        .as_ref()
        .filter(|r| r.path == p)
        .map(|r| r.last_commit.clone())
        .unwrap_or_default();
    shell.commit_settings(|s| s.repo = Some(crate::settings::RepoRecord { path: p.clone(), last_commit: last }))?;
    shell.repo_since = Some((h.commit, None));
    Ok(p)
}

pub(super) fn check_repo(shell: &mut Shell) -> Result<(String, Option<usize>), crate::fault::Fault> {
    let rec = shell.settings.repo.clone().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::FileMissing, crate::lang::t(crate::lang::Key::Tail028).to_string())
    })?;
    // The reading side does not drift with the current directory: a stored relative path (older files) is
    // refused by name, never resolved against the current directory.
    let s = crate::anchorx::since(&crate::home::landing(&rec.path)?, &rec.last_commit)?;
    shell.repo_since = Some((s.head.clone(), s.grew));
    Ok((s.head, s.grew))
}
