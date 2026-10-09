use super::*;

/// What the action layer does once a starting window's shell is up. With the vault open (only the test hooks
/// start that way) it opens the home, takes its lock, and reads the anchor and identity table; when locked,
/// none of this happens yet (local data is sealed) and `Shell::after_unlock` does it right after unlocking.
/// Returns the opened home root (empty when none). The window keeps no copy of this rule.
pub fn start(shell: &mut Shell) -> String {
    if !shell.unlocked() {
        return String::new();
    }
    let root = boot_home(shell);
    if let Err(f) = shell.refresh_anchor() {
        shell.faults.push(f);
    }
    apply(shell, Action::ReadIdentities);
    root
}

/// The startup pass: land on a seat, then open its home. Window startup and the test hooks both use it.
///
/// It first lands on a seat (`identity::land_at_boot`: an empty seat lands on the first occupied one, the same
/// rule as switching identity), then opens that seat's home (`identity::home_now`); if the home's settings
/// record a different seat, the landed seat is recorded. Without a register, the home comes from the
/// three-level resolution. Returns the place it tried to open (opened or not; empty when there is nothing to
/// open), which the window uses to prefill the "open data directory" field.
pub fn boot_home(shell: &mut Shell) -> String {
    let landed = match crate::register::change_listed(crate::identity::land_at_boot).map(Option::flatten) {
        Ok(l) => l,
        Err(f) => {
            shell.faults.push(f);
            None
        }
    };
    let root = match crate::register::read().and_then(|listed| crate::identity::home_now(listed.as_ref())) {
        Ok(Some(root)) => root.display().to_string(),
        Ok(None) => String::new(),
        Err(f) => {
            shell.faults.push(f);
            String::new()
        }
    };
    if !root.is_empty() {
        let _ = apply(shell, Action::OpenHome { root: root.clone() });
    }
    if let Some((_, seat)) = landed {
        if shell.settings.role != seat {
            if shell.home.is_some() {
                if let Err(f) = shell.commit_settings(|s| s.role = seat) {
                    shell.faults.push(f);
                }
            } else {
                shell.settings.role = seat;
            }
        }
    }
    root
}

/// Opens set-aside old data for reading (one of `Shell::aside`). The current home is remembered for returning,
/// and the machine pointer is not written. The old data keeps its read-only mark, so nothing is written there
/// or exported from it.
pub(super) fn view_old(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let at = std::path::PathBuf::from(root);
    let old = shell.aside.iter().find(|a| crate::home::same_place(&a.path, &at)).cloned().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::FileMissing, root.to_string()))?;
    let back = shell.old_view.as_ref().map(|v| v.back.clone()).or_else(|| shell.home.as_ref().map(|h| h.root().to_path_buf()));
    let mode = open_home_at(shell, root, false)?;
    shell.old_view = back.map(|back| crate::shell::OldView { back, at: old.at });
    Ok(mode)
}

/// Returns from old data to the home that was open before.
pub(super) fn leave_old(shell: &mut Shell) -> Result<(String, crate::lock::Mode), crate::fault::Fault> {
    let v = shell.old_view.clone().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let root = v.back.display().to_string();
    let mode = open_home_at(shell, &root, false)?;
    shell.old_view = None;
    Ok((root, mode))
}

/// "Write from this machine": the open home's writer mark names another machine, so this instance opened it
/// read-only. This rewrites the mark to name this machine and makes this instance the writer (the kernel lock
/// is already held). Only an explicit press does this; nothing takes a home over automatically or after a
/// timeout. Once the mark is rewritten the result is "writer" whatever follows (a later label problem is
/// reported alongside).
pub(super) fn take_writer(shell: &mut Shell) -> Result<(String, crate::lock::Mode), crate::fault::Fault> {
    let no_home = || crate::fault::Fault::known(crate::fault::Known::NoHome, crate::lang::t(crate::lang::Key::Tail005).to_string());
    let home = shell.home.as_ref().ok_or_else(no_home)?;
    let lock = shell.lock.as_mut().ok_or_else(no_home)?;
    let root = home.root().display().to_string();
    if lock.mode() != crate::lock::Mode::OtherMachine {
        return Ok((root, lock.mode()));
    }
    let as_seat = opened_as(home.root());
    let as_seat = as_seat.as_ref().map(|(i, s)| (i.as_str(), *s));
    // A home labelled for another identity or seat is not taken over: refused by name before the mark moves.
    crate::local::check_label(home.root(), as_seat, false)?;
    lock.take_over(home)?;
    // From here the mark names this machine and cannot be undone, so later failures are not reported as the
    // take-over failing. A home label that cannot be created or read is reported alongside.
    let label = crate::local::check_label(home.root(), as_seat, true);
    if let Err(f) = label {
        shell.faults.push(f);
    }
    Ok((root, crate::lock::Mode::Writer))
}

/// The identity and seat a home is opened as: the register's current row, if this home is that row's seat
/// home.
fn opened_as(root: &std::path::Path) -> Option<(String, crate::roles::Role)> {
    let (row, seat) = crate::register::now_row_listed().ok().flatten()?;
    row.home(seat).filter(|h| crate::home::same_place(h, root)).map(|_| (row.id.clone(), seat))
}

/// Reopens the home at this place after it was swapped for a fresh one (set aside, then fetched into), taking
/// the writer lock on the fresh home.
pub fn reopen_here(shell: &mut Shell, root: &std::path::Path) -> Result<crate::lock::Mode, crate::fault::Fault> {
    shell.lock = None;
    open_home_at(shell, &root.display().to_string(), false)
}

/// "Change data folder": a home opens at the chosen folder only if it is already a home or is empty
/// (`home::may_open_at`); anything else is refused by name with nothing written (a command-line ledger folder
/// is adopted in place instead). Only a person's choice is checked this way: homes the app opens itself (the
/// one remembered at start, a seat's) are its own, and a stray file dropped in later must not lock the person
/// out of their data.
pub(super) fn change_home(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let at = crate::home::landing(root)?;
    crate::home::may_open_at(&at)?;
    open_home(shell, root)
}

pub(super) fn open_home(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    // Shape-check the given place first (`home::landing`): empty or relative is refused before creating any
    // folder or writing the pointer.
    let root = crate::home::landing(root)?;
    let root = root.display().to_string();
    let root = root.as_str();
    // An identity's home does not write the machine pointer (as with `enter`): opening the current identity's
    // seat home at startup also goes through here, and writing would repoint the machine pointer on every
    // open. The pointer only follows a home the person chose without an identity register.
    let identity_home = crate::register::now_row_listed()
        .ok()
        .flatten()
        .and_then(|(row, seat)| row.home(seat))
        .map(|h| h == std::path::Path::new(root.trim()))
        .unwrap_or(false);
    open_home_at(shell, root, !identity_home)
}

/// Opens a home. `pointer` is true for a home the person chose (writes the machine pointer) and false when
/// entering an identity's seat.
pub(super) fn open_home_at(shell: &mut Shell, root: &str, pointer: bool) -> Result<crate::lock::Mode, crate::fault::Fault> {
    // Check the home's label before writing anything there (no folders, no lock, no writer mark): a label that
    // cannot be opened (a home copied from another machine, sealed under its key) or that names another
    // identity or seat refuses the opening by name and leaves the place untouched.
    let as_seat = opened_as(std::path::Path::new(root));
    crate::local::check_label(std::path::Path::new(root), as_seat.as_ref().map(|(i, s)| (i.as_str(), *s)), false)?;
    // An incompletely laid home is refused by `home::lay` (inside `open_or_create`), which reads the folders
    // back from disk and returns `CANNOT_LAY` when any is missing.
    let home = crate::home::Home::open_or_create(root)?;
    // Re-entering the home whose writer lock is already held: reuse it instead of opening a second descriptor
    // and judging itself a reader.
    let lock = if crate::lock::holds_writer(shell.lock.as_ref(), &home) { None } else { Some(crate::lock::take(&home)?) };
    let mode = lock.as_ref().map(|l| l.mode()).unwrap_or(crate::lock::Mode::Writer);
    // Check the label (local data format, version 2) against the identity and seat it is opened as; a writer
    // gives a home created by an older version its label now.
    crate::local::check_label(home.root(), opened_as(home.root()).as_ref().map(|(i, s)| (i.as_str(), *s)), mode.writable())?;
    // Steps that can fail come before the shell is touched, so unreadable settings or an unwritable pointer
    // leave it unchanged. Loading the new settings before writing the pointer would, if the pointer write
    // failed, show the old home with the new home's settings, and the next settings change would be written
    // into the old home.
    crate::settings::Settings::read(&home)?;
    // A home named by an environment variable does not write the pointer (see `home::named_by_env`).
    if pointer && !crate::home::named_by_env(home.root()) {
        crate::home::write_pointer(home.root())?;
    }
    // A different home: everything read from the previous one is invalid, and source-bound tasks move to a new
    // generation.
    shell.source_changed(crate::shell::Source::Home);
    // Load the on-disk copies in one place (settings, endpoints, anchor queue, first-run checklist). A read
    // failure is reported at once, never treated as an empty copy (see `Shell::hydrate`).
    shell.hydrate(&home)?;
    shell.home = Some(home);
    if let Some(l) = lock {
        // Switching homes: install the new lock; the old one is released when replaced.
        shell.lock = Some(l);
    }
    // A writer mark this version cannot read opened the home read-only (`lock::Mark::Unread`): reported by
    // name, and the mark is left until the person takes the home over.
    if let Some(f) = shell.lock.as_ref().and_then(|l| l.mark_trouble()).cloned() {
        shell.trouble(f);
    }
    // Three writes a writer makes on opening, which a reader never does (a reader's home belongs to another
    // writer), nor does viewing old data (`view_old`). Failures here return no error: the shell already stands
    // in the new home with its lock, so an error would wrongly say opening failed. Each failure is recorded as
    // a trouble instead.
    let old_data = shell.home.as_ref().map(|h| shell.aside.iter().any(|a| crate::home::same_place(&a.path, h.root()))).unwrap_or(false);
    if mode.writable() && !old_data {
        // Nodes exactly matching a set a known row shipped earlier are replaced with its current nodes, once.
        if let Err(f) = renew_shipped_nodes(shell) {
            shell.trouble(f);
        }
        // Identities created before identities chose a network get this machine's row recorded, once.
        if let Err(f) = record_machine_network(shell) {
            shell.trouble(f);
        }
        // A home without a network takes the network of the identity that owns it (both seats of an identity
        // share one); a home that has one is never changed. A home no identity owns, or an identity that chose
        // none or chose "custom" without filling it in, gets nothing, and the UI says no network is configured.
        if let Err(f) = take_identity_network(shell) {
            shell.trouble(f);
        }
    }
    // Measure once after opening so the UI shows real numbers; the disk walk runs in the background.
    if let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) {
        shell.tasks.spawn(Kind::Archive, move || measure(&root));
    }
    // List the vault once too, so imported items never look missing.
    shell.held = None;
    shell.tasks.forget(Kind::Held);
    let _ = list_held(shell);
    // For a ledger recorded before genesis was queued, queue the root once.
    backfill_root(shell);
    // For a transaction the queue file records as submitted, resume waiting for its receipt without resending.
    resume(shell);
    Ok(mode)
}

/// If the open home's nodes are a set a known row shipped earlier (`deploy::shipped_before`), replaces them with
/// that row's current nodes and saves once; other settings stay. Nodes that differ in any character or order
/// were set by a person and are left alone.
fn renew_shipped_nodes(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    let Some(d) = crate::deploy::shipped_before(&shell.settings.endpoints) else { return Ok(()) };
    let eps: Vec<crate::chainx::Endpoint> = d.endpoint_specs().iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).collect();
    shell.commit_settings(|s| s.endpoints = eps.iter().map(|e| e.spec()).collect())?;
    endpoints_changed(shell, eps);
    Ok(())
}

/// Identity rows created before identities chose a network record none; their homes took the deployment this
/// machine chose when first opened. Records that deployment on each such row, once
/// (`identity::backfill_network` only fills a row with none), so a seat not yet opened takes it like any
/// identity's, and a later choice on this machine changes no row. Nothing is recorded if the machine chose
/// none or "custom", or for a row whose homes already hold another chain (that would split its two seats
/// across two chains).
fn record_machine_network(shell: &Shell) -> Result<(), crate::fault::Fault> {
    let Some(d) = shell.machine.network.as_deref().and_then(crate::deploy::named) else { return Ok(()) };
    crate::register::change_listed(|reg| {
        crate::identity::backfill_network(reg, d, |row| row.seats().into_iter().filter_map(|s| row.home(s)).all(|h| holds_no_other_chain(&h, d)));
        Ok(())
    })?;
    Ok(())
}

/// True for a home that does not exist yet, or whose settings name no chain or this deployment's chain and
/// registry. A home whose settings cannot be read counts as holding another chain.
fn holds_no_other_chain(at: &std::path::Path, d: &crate::deploy::Deployment) -> bool {
    if !at.exists() {
        return true;
    }
    match crate::home::Home::open(at).and_then(|h| crate::settings::Settings::read(&h)) {
        Ok(s) => s.chain_id.is_none() || (s.chain_id == Some(d.chain_id) && s.registry == Some(d.registry_address())),
        Err(_) => false,
    }
}

/// If the open home has no network (no chain, registry or nodes) and an identity on this machine owns it, it
/// takes that identity's network (`identity::Row::network_now`). A home with a network is never changed here:
/// it may hold a ledger anchored on that chain.
fn take_identity_network(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    let s = &shell.settings;
    if s.chain_id.is_some() || s.registry.is_some() || !s.endpoints.is_empty() {
        return Ok(());
    }
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return Ok(()) };
    let Some(reg) = crate::register::read()? else { return Ok(()) };
    let Some((row, _)) = crate::identity::owner_of(&reg, &root) else { return Ok(()) };
    match row.network_now() {
        Some(n) => adopt_network(shell, n),
        None => Ok(()),
    }
}

/// The UI-thread half before a move: whether this instance may move the home, and where to (checked, not
/// written). Returns the old root and the target; the copy runs in the background (`Kind::Migrate`,
/// [`copy_home`]) and [`migrate_landed`] finishes it.
pub(super) fn migrate_start(shell: &Shell, to: &str) -> Result<(std::path::PathBuf, std::path::PathBuf), crate::fault::Fault> {
    // Moving the home requires being its writer. Otherwise, with two instances open, the reader could copy the
    // tree, lock the copy and repoint this machine's home pointer at it while the writer kept appending to the
    // old one, hiding the writer's later entries. A broken chain or held pen does not block moving: the same
    // bytes move and nothing new is recorded.
    match shell.lock.as_ref() {
        None => {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::NoHome,
                crate::lang::t(crate::lang::Key::Tail005).to_string(),
            ))
        }
        Some(l) if !l.mode().writable() => {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::ReadOnly,
                crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()]),
            ))
        }
        Some(_) => {}
    }
    // Check the target before writing anything (`home::landing`): an empty or relative path is refused as
    // `PATH_RELATIVE`. Otherwise it would resolve against the process's current directory and the whole home
    // tree, with settings and writer lock, would be written there.
    let target = crate::home::landing(to)?;
    let Some(home) = shell.home.as_ref() else {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::FileMissing,
            crate::lang::t(crate::lang::Key::Tail017).to_string(),
        ));
    };
    // A target at or under this home is refused here, before the task starts and the home freezes
    // (`settings::migrate` checks the same for its other callers).
    crate::settings::outside_home(home.root(), &target)?;
    Ok((home.root().to_path_buf(), target))
}

/// Starts a move: the checks (`migrate_start`), then the whole tree is copied in a task (`Kind::Migrate`) with
/// the home frozen meanwhile (`swapping`), so nothing written now could end up only in the old place. The lock,
/// the register and the shell follow when it lands (`migrate_landed`).
pub(super) fn migrate_begin(shell: &mut Shell, to: &str) -> Applied {
    match migrate_start(shell, to) {
        Ok((old, target)) => match shell.tasks.spawn(Kind::Migrate, move || copy_home(&old, &target)) {
            Spawned::Started => {
                shell.swapping = true;
                Applied::Started(Kind::Migrate)
            }
            Spawned::InFlight => Applied::Refused(Kind::Migrate),
        },
        Err(f) => shell.trouble(f),
    }
}

/// The background half of a move: copies the whole tree to the new place and compares it, then writes the
/// pointer (`settings::migrate`; copy first, pointer after). The old place is never changed.
pub(super) fn copy_home(old: &std::path::Path, target: &std::path::Path) -> Result<crate::task::Done, crate::fault::Fault> {
    let from = crate::home::Home::open(old)?;
    let moved = crate::settings::migrate(&from, target)?;
    Ok(crate::task::Done::Copied { old: old.to_path_buf(), root: moved.root().to_path_buf() })
}

/// The UI-thread half after the home was copied: the lock moves to the new home, the register entry and the
/// record bundle index follow, and the shell opens it.
pub(super) fn migrate_landed(shell: &mut Shell, old: &std::path::Path, root: &std::path::Path) -> Result<String, crate::fault::Fault> {
    let old = old.to_path_buf();
    let moved = crate::home::Home::open(root)?;
    let root = moved.root().display().to_string();
    // The old place is left untouched; the person deletes it after moving. The lock moves to the new home.
    let lock = crate::lock::take(&moved)?;
    // Identity homes are recorded in the register: repoint the entry from the old place to the new one
    // (signing key ownership and the next startup both read it). The register is the last fallible step
    // before the shell is touched: if an earlier step fails, register and shell still point to the old place;
    // if it fails itself, the new lock is dropped and the shell keeps the old home open, so both still agree.
    crate::register::change_listed(|reg| Ok(crate::identity::rehome(reg, &old, moved.root())))?;
    // After the move succeeds, record bundle index rows under the old home are rebased to the new one. Failure
    // does not undo the move (the shell must follow the home); it is reported, and the index keeps pointing at
    // the old place, which is unchanged and still holds the bundles.
    if let Err(f) = crate::home::machine_dir().and_then(|m| crate::kitsindex::rebase(&m, &old, moved.root())) {
        shell.faults.push(f);
    }
    shell.home = Some(moved);
    shell.lock = Some(lock);
    shell.reread_kits();
    Ok(root)
}

pub(super) fn set_cap(shell: &mut Shell, bytes: u64) -> Result<u64, crate::fault::Fault> {
    if bytes == 0 {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::SettingsShape,
            crate::lang::t(crate::lang::Key::Tail018).to_string(),
        ));
    }
    shell.commit_settings(|s| s.cap_bytes = bytes)?;
    Ok(bytes)
}

/// The current Unix time in seconds. Used only for naming files, never for decisions (deadlines and windows use
/// only chain time and an injected now).
pub(super) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
