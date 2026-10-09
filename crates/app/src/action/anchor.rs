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
    // For an anchored git repository, remember this commit; the passive indicator compares against it later.
    if let (Some(c), Some(rec)) = (shell.content.as_ref(), shell.settings.repo.clone()) {
        // The stored path is absolute (`home::kept`); resolve the content's path the same way before comparing.
        let same = crate::home::kept(&c.subject).map(|p| p.display().to_string() == rec.path).unwrap_or(false);
        if c.source == crate::anchorx::Source::Git && same {
            if let Ok(h) = crate::gitx::head_of(std::path::Path::new(&rec.path)) {
                shell.commit_settings(|s| s.repo = Some(crate::settings::RepoRecord { path: rec.path, last_commit: h.commit }))?;
            }
        }
    }
    Ok((id, n))
}

/// The background half of recording files (`Kind::Record`): fingerprints each file in order, stopping at the
/// first that cannot be read (nothing after it will be recorded).
pub(super) fn hash_files(files: &[String]) -> Vec<(String, Result<crate::anchorx::Content, crate::fault::Fault>)> {
    let mut out = Vec::new();
    for p in files {
        let c = crate::anchorx::of_file(std::path::Path::new(p.trim()));
        let stop = c.is_err();
        out.push((p.clone(), c));
        if stop {
            break;
        }
    }
    out
}

/// The UI-thread half of recording files: one history entry per fingerprinted file, in order. The first
/// failure stops there: earlier files are recorded, it and later ones are not.
pub(super) fn record_files(
    shell: &mut Shell,
    note_md: &str,
    files: Vec<(String, Result<crate::anchorx::Content, crate::fault::Fault>)>,
    target: Option<&crate::anchorx::For>,
) -> (Vec<String>, Option<(usize, String, crate::fault::Fault)>, Enqueued) {
    let mut ids = Vec::new();
    let mut n = Enqueued { queued: shell.queue.len(), next: Next::Held };
    let m = crate::anchorx::mode();
    for (i, (p, c)) in files.into_iter().enumerate() {
        let one = c.and_then(|c| {
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

/// Adds a signed entry to the local index. A failure here does not undo the entry (it is already in the
/// ledger), but it is still reported.
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
            // Use the real path if the file still exists; otherwise (just moved) make it absolute through
            // `home::kept`. A relative path is never stored.
            path: match std::fs::canonicalize(path) {
                Ok(p) => crate::home::plain_path(p),
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

/// Classifies a dropped or picked path from its metadata: a directory containing `.git` is a git repository,
/// other directories are folders, regular files are files, and anything else is refused by name. The
/// fingerprint is computed later in the background (`Kind::Take`).
///
/// This lives in the action layer because the window side never reads the disk.
pub(super) fn dropped_source(path: &str) -> Result<(crate::anchorx::Source, std::path::PathBuf), crate::fault::Fault> {
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
    Ok((source, p.to_path_buf()))
}

/// A taken content with its fingerprint becomes the shell's content. The previous three-step flow is reset,
/// so a half-completed flow never stays on screen.
pub(super) fn took_landed(shell: &mut Shell, source: crate::anchorx::Source, c: crate::anchorx::Content) -> super::Applied {
    let hex = c.hex();
    shell.content = Some(c);
    shell.flow = crate::anchorx::Flow::default();
    super::Applied::Took { source, hex }
}

pub(super) fn register_repo(shell: &mut Shell, path: &str) -> Result<String, crate::fault::Fault> {
    // Made absolute before saving (`home::kept`), so starting from another directory still finds it.
    let p = crate::home::kept(path)?.display().to_string();
    // Open it once before registering, so a bad path is refused now instead of failing on the next visit.
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
    // A stored relative path (written by older versions) is refused by name, never resolved against the
    // current directory.
    let s = crate::anchorx::since(&crate::home::landing(&rec.path)?, &rec.last_commit)?;
    shell.repo_since = Some((s.head.clone(), s.grew));
    Ok((s.head, s.grew))
}
