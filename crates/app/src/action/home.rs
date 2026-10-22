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
/// entering an identity's seat. The disk half ([`opening`]) and the shell half ([`opened`]) run here back to back;
/// switching seat runs the first in the background and the second where it lands.
pub(super) fn open_home_at(shell: &mut Shell, root: &str, pointer: bool) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let o = opening(root, pointer, &OpeningAsk::of(shell))?;
    Ok(opened(shell, o))
}

/// What the disk half of opening a home takes from the shell, so it can run off the window's thread: the writer
/// lock held now (to keep it when the home is the one it locks) and whether this machine writes alone.
#[derive(Clone, Debug)]
pub(super) struct OpeningAsk {
    held: Option<std::path::PathBuf>,
    alone: bool,
}

impl OpeningAsk {
    pub(super) fn of(shell: &Shell) -> OpeningAsk {
        OpeningAsk { held: crate::lock::writer_path(shell.lock.as_ref()).map(|p| p.to_path_buf()), alone: shell.machine.cli_alone.is_some() }
    }
}

/// A home opened on disk and not yet in the shell: the home, its writer lock (none when the one held is kept),
/// the mode it opened in, and what the shell's copies will hold (`shell::Hydrated`).
pub struct Opening {
    home: crate::home::Home,
    lock: Option<crate::lock::Lock>,
    mode: crate::lock::Mode,
    read: crate::shell::Hydrated,
    /// Why a bare home could not take its owner's network ([`take_identity_network`]), said once the shell stands
    /// in the home; whether it took one (its settings written and read again here).
    network_trouble: Option<crate::fault::Fault>,
    took_network: bool,
    /// Whether the writes a writer makes on opening are made (a writer, not viewing old data).
    writes: bool,
}

impl Opening {
    /// The seat written into the home's settings when they name another, before the shell moves in (entering a seat:
    /// a home's settings name the seat it belongs to). A home this process only reads refuses it by name, as saving
    /// settings is refused there (`Shell::may_save_settings`).
    pub(super) fn seat_written(&mut self, seat: crate::roles::Role) -> Result<(), crate::fault::Fault> {
        if self.read.settings.role == seat {
            return Ok(());
        }
        if !self.mode.writable() {
            let holder = self.lock.as_ref().map(|l| l.holder().to_string()).unwrap_or_default();
            return Err(crate::fault::Fault::known(crate::fault::Known::ReadOnly, crate::lang::filln(crate::lang::Key::Tail006, &[&holder])));
        }
        let mut next = self.read.settings.clone();
        next.role = seat;
        next.write(&self.home)?;
        self.read.settings = next;
        Ok(())
    }
}

impl std::fmt::Debug for Opening {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opening").field("home", &self.home.root()).field("mode", &self.mode).finish()
    }
}

/// The disk half of opening a home: every check and read, the writer lock taken, the pointer written; the shell is
/// not touched. Any refusal leaves the place as it was found, and dropping what this returns lets the lock go.
pub(super) fn opening(root: &str, pointer: bool, ask: &OpeningAsk) -> Result<Opening, crate::fault::Fault> {
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
    let lock = if crate::lock::holds_writer_at(ask.held.as_deref(), &home) { None } else { Some(crate::lock::take_as(&home, ask.alone)?) };
    let mode = lock.as_ref().map(|l| l.mode()).unwrap_or(crate::lock::Mode::Writer);
    // Check the label (local data format, version 2) against the identity and seat it is opened as; a writer
    // gives a home without one its label now.
    crate::local::check_label(home.root(), opened_as(home.root()).as_ref().map(|(i, s)| (i.as_str(), *s)), mode.writable())?;
    // Steps that can fail come before the shell is touched, so unreadable settings or an unwritable pointer
    // leave it unchanged. Loading the new settings before writing the pointer would, if the pointer write
    // failed, show the old home with the new home's settings, and the next settings change would be written
    // into the old home. The on-disk copies are read in one place (`shell::Hydrated::read`): settings that do not
    // read refuse the opening; any other file that does not read is shown as a trouble.
    let read = crate::shell::Hydrated::read(&home)?;
    // A home named by an environment variable does not write the pointer (see `home::named_by_env`).
    if pointer && !crate::home::named_by_env(home.root()) {
        crate::home::write_pointer(home.root())?;
    }
    // The writes a writer makes on opening, which a reader never does (a reader's home belongs to another writer),
    // nor does viewing old data (`view_old`): decided here, once, for both halves. A temporary file a write left
    // when it was cut (a power loss mid-write) is removed now (the writer holds the lock, so none is in flight, and
    // strict reads after this meet only the home's own names); a home with no network takes its owner's
    // ([`take_identity_network`]). Every write of opening is made here, none where the shell moves in.
    let old_data = read.aside.iter().any(|a| crate::home::same_place(&a.path, home.root()));
    let mut read = read;
    let mut writes = false;
    let (mut network_trouble, mut took_network) = (None, false);
    if mode.writable() && !old_data {
        writes = true;
        crate::home::sweep_temps(home.root());
        let bare = read.settings.chain_id.is_none() && read.settings.registry.is_none() && read.settings.endpoints.is_empty();
        match take_identity_network(&home, &mut read) {
            Ok(()) => took_network = bare && read.settings.chain_id.is_some(),
            Err(f) => network_trouble = Some(f),
        }
    }
    Ok(Opening { home, lock, mode, read, writes, network_trouble, took_network })
}

/// The shell half of opening a home: the shell moves into it (a new source), its copies installed, its lock in
/// place, and the passes after opening started in the background. It cannot fail: everything that can was done in
/// the disk half ([`opening`]), so a seat switch whose register already moved always lands whole.
pub(super) fn opened(shell: &mut Shell, o: Opening) -> crate::lock::Mode {
    let Opening { home, lock, mode, read, writes, network_trouble, took_network } = o;
    // A different home: everything read from the previous one is invalid, and source-bound tasks move to a new
    // generation.
    shell.source_changed(crate::shell::Source::Home);
    shell.install(read);
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
    // The writes a writer makes on opening were made in [`opening`]. A failure there returns no error here: the shell
    // already stands in the new home with its lock, so an error would wrongly say opening failed; it is recorded as a
    // trouble instead. A home that took its owner's network asks about its tail now, as a network chosen does.
    if writes {
        if let Some(f) = network_trouble {
            shell.trouble(f);
        }
        if took_network {
            tail_if_due(shell);
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
    mode
}

/// If the home being opened has no network (no chain, registry or nodes) and an identity on this machine owns it, it
/// takes that identity's network (`identity::Row::network_now`): written into its settings and read again into
/// `read`, in the disk half of opening ([`opening`]), off the window's thread when switching seat. A home with a
/// network is never changed here: it may hold a ledger anchored on that chain. A home no identity owns, or an
/// identity that chose none or chose "custom" without filling it in, gets nothing, and the UI says no network is
/// configured.
fn take_identity_network(home: &crate::home::Home, read: &mut crate::shell::Hydrated) -> Result<(), crate::fault::Fault> {
    let s = &read.settings;
    if s.chain_id.is_some() || s.registry.is_some() || !s.endpoints.is_empty() {
        return Ok(());
    }
    let Some(reg) = crate::register::read()? else { return Ok(()) };
    let Some((row, _)) = crate::identity::owner_of(&reg, home.root()) else { return Ok(()) };
    let Some(n) = row.network_now() else { return Ok(()) };
    let mut next = read.settings.clone();
    network_into(&mut next, n);
    next.write(home)?;
    // Which chain each node serves is cached per process; nodes saved are asked again (as `commit_settings` does).
    crate::chainx::forget_serving();
    *read = crate::shell::Hydrated::read(home)?;
    Ok(())
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
    let lock = crate::lock::take_as(&moved, shell.machine.cli_alone.is_some())?;
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
