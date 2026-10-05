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

/// The background half of recording files (`Kind::Record`): each file's fingerprint, in the order given,
/// stopping at the first that cannot be read (nothing past it is computed, as nothing past it was recorded).
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

/// The frame half of recording files: one history entry per file whose fingerprint came back, in order; the
/// first refusal stops the batch there (the files before it are recorded, it and those after are not).
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

/// Which entry a dropped path goes to. Recognized by reading the disk now: a directory containing `.git`
/// takes the git path, other directories the directory path, regular files the file path; none of the three
/// is refused by name.
///
/// This recognition lives in the action layer, not the frame: the window side reads no disk at all (checked
/// by the self-check suite).
/// The frame half of taking a dropped or picked path: which kind of content it is (a file, a folder, a
/// repository), read from its metadata; the fingerprint is computed in the background (`Kind::Take`).
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

/// A content taken, its fingerprint computed: it is the shell's content now, and the previous three-step flow is
/// void, since a half-green flow left on screen would be a silent failure.
pub(super) fn took_landed(shell: &mut Shell, source: crate::anchorx::Source, c: crate::anchorx::Content) -> super::Applied {
    let hex = c.hex();
    shell.content = Some(c);
    shell.flow = crate::anchorx::Flow::default();
    super::Applied::Took { source, hex }
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
