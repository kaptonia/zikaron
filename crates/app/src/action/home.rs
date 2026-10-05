use super::*;

/// What a starting window asks of the action layer once the shell is up: with the vault open (only the test
/// hooks start that way) the home opens, its lock is taken, the anchor and the identity table are read; locked, none
/// of that happens now (local data is sealed) and `Shell::after_unlock` does it right after unlocking. Answers
/// the home root opened (empty when none). The window keeps no copy of this rule.
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

/// The startup pass: land on a seat, open the home. Window startup and the test hooks' startup both go
/// through here.
///
/// First land on a seat (`identity::land_at_boot`: standing on an empty seat lands on the first occupied
/// seat, the same rule as switching identity), then open that seat's home (`identity::home_now`); if the seat
/// recorded in that home's settings differs from the seat landed on, the landed one is recorded. Without a
/// register, the home is resolved by the archive's three levels. Returns the place this pass tried to open
/// (whether or not it opened; an empty string when there is nothing to open); the window prefills the "open
/// data directory" cell with it.
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

/// Open old data to read (one of `Shell::aside`): the home open now is remembered, the pointer is not
/// written. The old data keeps its read-only mark, so nothing is written there and nothing leaves from it.
pub(super) fn view_old(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let at = std::path::PathBuf::from(root);
    let old = shell.aside.iter().find(|a| crate::home::same_place(&a.path, &at)).cloned().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::FileMissing, root.to_string()))?;
    let back = shell.old_view.as_ref().map(|v| v.back.clone()).or_else(|| shell.home.as_ref().map(|h| h.root().to_path_buf()));
    let mode = open_home_at(shell, root, false)?;
    shell.old_view = back.map(|back| crate::shell::OldView { back, at: old.at });
    Ok(mode)
}

/// Come back from old data to the home open before it.
pub(super) fn leave_old(shell: &mut Shell) -> Result<(String, crate::lock::Mode), crate::fault::Fault> {
    let v = shell.old_view.clone().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let root = v.back.display().to_string();
    let mode = open_home_at(shell, &root, false)?;
    shell.old_view = None;
    Ok((root, mode))
}

/// After the home in this place was swapped for a fresh one (set aside, fetched into): open it again, as it is
/// now (the writer lock taken on the fresh home).
pub fn reopen_here(shell: &mut Shell, root: &std::path::Path) -> Result<crate::lock::Mode, crate::fault::Fault> {
    shell.lock = None;
    open_home_at(shell, &root.display().to_string(), false)
}

/// "Change data folder": the person points at a folder, and a home opens there only when it is a home already
/// or nothing is there (`home::may_open_at`); anything else is refused by name with nothing written (a
/// command-line ledger folder is taken over by adopting it in place, not by opening it as a home). Only the
/// person's choice is asked this: a home the product opens by itself (the one remembered at start, a seat's)
/// is the product's own, and a stray file a person later drops in it must not lock them out of their data.
pub(super) fn change_home(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let at = crate::home::landing(root)?;
    crate::home::may_open_at(&at)?;
    open_home(shell, root)
}

pub(super) fn open_home(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    // The place the person gave is checked for shape first (`home::landing`): empty or relative is refused
    // before laying out rooms or writing the pointer.
    let root = crate::home::landing(root)?;
    let root = root.display().to_string();
    let root = root.as_str();
    // An identity's home does not write the machine pointer (as with `enter`): opening the current identity's
    // seat home from the register at startup goes through here too, and writing here would change the
    // machine pointer to that identity's home on every open. The pointer only follows a home the person chose
    // without an identity register.
    let identity_home = crate::register::now_row_listed()
        .ok()
        .flatten()
        .and_then(|(row, seat)| row.home(seat))
        .map(|h| h == std::path::Path::new(root.trim()))
        .unwrap_or(false);
    open_home_at(shell, root, !identity_home)
}

/// Open a home. `pointer` true means a home the person chose in the product (writes the machine pointer);
/// false when entering a seat of an identity.
pub(super) fn open_home_at(shell: &mut Shell, root: &str, pointer: bool) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let home = crate::home::Home::open_or_create(root)?;
    // What counts as "laid out" is that the rooms really exist on disk, not that the previous statement
    // reported no error. `lay` has already read back once; this is the second owner: what opening a home
    // hands out must survive being asked again on the spot.
    let missing = home.missing();
    if !missing.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::CannotLay,
            crate::lang::filln(crate::lang::Key::Tail016, &[&(home.root().display()).to_string(), &(missing.join(" ")).to_string()]),
        ));
    }
    // Re-entering the home whose writer lock is in hand: reuse it, instead of opening a second descriptor and
    // judging itself a reader.
    let lock = if crate::lock::holds_writer(shell.lock.as_ref(), &home) { None } else { Some(crate::lock::take(&home)?) };
    let mode = lock.as_ref().map(|l| l.mode()).unwrap_or(crate::lock::Mode::Writer);
    // Steps that can fail come before the shell is touched: unreadable settings or an unwritable pointer
    // leave with the shell untouched. Loading the new home's settings before writing the pointer would, on
    // pointer failure, show the old home while holding the new home's settings, and the next settings change
    // would be written into the old home.
    crate::settings::Settings::read(&home)?;
    // The pointer only follows the person's choice: a home named by an environment variable does not write
    // the pointer (see `home::named_by_env`).
    if pointer && !crate::home::named_by_env(home.root()) {
        crate::home::write_pointer(home.root())?;
    }
    // A different home: everything read from the previous one is void, and tasks following the source move to
    // a new generation.
    shell.source_changed(crate::shell::Source::Home);
    // The copies on disk are loaded in one place (settings, endpoints, anchor queue, first-window checklist).
    // Written but never read back is the "the face is empty after reopening while the disk is full" kind; a
    // broken read is named at once, never swallowed as an empty copy (see `Shell::hydrate`).
    shell.hydrate(&home)?;
    shell.home = Some(home);
    if let Some(l) = lock {
        // Moving to another home: the new lock is settled, and the old lock is released on replacement.
        shell.lock = Some(l);
    }
    // Three writes a writer makes on opening, and a reader never does (a reader's home is another writer's),
    // nor does opening old data to read (`view_old`: the set-aside data is only looked at, whoever holds its
    // lock). Each failure after the shell was touched returns no error: the home is already the new one and
    // the lock is in hand, so returning an error would make the face say "opening the home failed" while the
    // app already stands in the new home. A failure records a trouble, and the face says plainly what the home
    // has.
    let old_data = shell.home.as_ref().map(|h| shell.aside.iter().any(|a| crate::home::same_place(&a.path, h.root()))).unwrap_or(false);
    if mode.writable() && !old_data {
        // Nodes a table row shipped with before, letter for letter: the row's nodes of today, saved once.
        if let Err(f) = renew_shipped_nodes(shell) {
            shell.trouble(f);
        }
        // Identities made before they chose a network: recorded this machine's row once.
        if let Err(f) = record_machine_network(shell) {
            shell.trouble(f);
        }
        // A home without a network takes the network of the identity it belongs to (both seats of an identity
        // take the same one); once it has one, nothing is filled back. A home no identity owns, an identity
        // that chose none, or chose "custom" with nothing filled in yet: nothing is filled, and the face says
        // plainly that no network is configured.
        if let Err(f) = take_identity_network(shell) {
            shell.trouble(f);
        }
    }
    // Measure once after opening: the face cells then have real numbers, and the disk walk happens in the
    // background, not in the frame.
    if let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) {
        shell.tasks.spawn(Kind::Archive, move || measure(&root));
    }
    // Read the vault once too: imported but the vault looking empty is a form of silent failure.
    shell.held = None;
    shell.tasks.forget(Kind::Held);
    let _ = list_held(shell);
    // For a ledger recorded before genesis was queued, queue the root once.
    backfill_root(shell);
    // A transaction the queue file records as "submitted": opening the home resumes waiting for its receipt,
    // without resending.
    resume(shell);
    Ok(mode)
}

/// The open home's nodes are an earlier pair of a known row (`deploy::shipped_before`): they become that row's
/// nodes of today, saved once; every other cell stays. Nodes that differ in any letter or order were set by a
/// person and are not touched.
fn renew_shipped_nodes(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    let Some(d) = crate::deploy::shipped_before(&shell.settings.endpoints) else { return Ok(()) };
    let eps: Vec<crate::chainx::Endpoint> = d.endpoint_specs().iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).collect();
    shell.commit_settings(|s| s.endpoints = eps.iter().map(|e| e.spec()).collect())?;
    endpoints_changed(shell, eps);
    Ok(())
}

/// Rows made before identities chose a network record none; their homes took the row this machine chose when
/// they were first opened. Each such row is recorded that row, once (`identity::backfill_network` records
/// only on a row with none): a seat not yet opened then takes it like any identity's, and a later choice on
/// this machine changes no row. A machine that chose none, or chose "custom", records nothing; nor does a row
/// whose homes already hold another chain (recording it would put the two seats on two chains).
fn record_machine_network(shell: &Shell) -> Result<(), crate::fault::Fault> {
    let Some(d) = shell.machine.network.as_deref().and_then(crate::deploy::named) else { return Ok(()) };
    crate::register::change_listed(|reg| {
        crate::identity::backfill_network(reg, d, |row| row.seats().into_iter().filter_map(|s| row.home(s)).all(|h| holds_no_other_chain(&h, d)));
        Ok(())
    })?;
    Ok(())
}

/// A home that is not there yet, or whose settings name no chain, or name this row's chain and registry. A home
/// whose settings cannot be read counts as holding another: nothing is recorded over what cannot be seen.
fn holds_no_other_chain(at: &std::path::Path, d: &crate::deploy::Deployment) -> bool {
    if !at.exists() {
        return true;
    }
    match crate::home::Home::open(at).and_then(|h| crate::settings::Settings::read(&h)) {
        Ok(s) => s.chain_id.is_none() || (s.chain_id == Some(d.chain_id) && s.registry == Some(d.registry_address())),
        Err(_) => false,
    }
}

/// The open home has no network (no chain, no registry, no nodes) and an identity on this machine owns it:
/// it takes that identity's network (`identity::Row::network_now`). A home with a network is never changed
/// here: it may hold a ledger anchored on that chain.
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

/// The frame half before a move: whether this side may move the home, and where to (checked, not written).
/// Answers the old root and the landing place; the copy runs in the background (`Kind::Migrate`, with
/// [`copy_home`]) and [`migrate_landed`] follows it where it lands.
pub(super) fn migrate_start(shell: &Shell, to: &str) -> Result<(std::path::PathBuf, std::path::PathBuf), crate::fault::Fault> {
    // Moving the home asks "am I this home's writer".
    //
    // With no gate at all, when two instances have the home open, the reader side could copy the whole tree,
    // take the lock on the copy and point this machine's home pointer at the copy, while the writer kept
    // appending to the old one; afterwards the copy is opened, and none of the writer's later entries are
    // visible. Broken chain and held pen do not block moving: it moves where the same bytes live and creates
    // no new legal fact (any copy is equivalent). What is closed is exactly "the non-writer side can change
    // this machine's home pointer".
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
    // Check the landing place first, write later (`home::landing`): an empty string or relative path is
    // refused as `PATH_RELATIVE` before one byte is written. Otherwise a relative path would land relative to
    // the process's current directory, and the whole home tree with settings and writer lock would be written
    // there.
    let target = crate::home::landing(to)?;
    let Some(home) = shell.home.as_ref() else {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::FileMissing,
            crate::lang::t(crate::lang::Key::Tail017).to_string(),
        ));
    };
    // A new place at or under this home is refused here, before the task starts and the home freezes
    // (`settings::migrate` asks the same on its own, for its other callers).
    crate::settings::outside_home(home.root(), &target)?;
    Ok((home.root().to_path_buf(), target))
}

/// Starting a move: the checks (`migrate_start`), then the whole tree copied as a task (`Kind::Migrate`) with the
/// home frozen meanwhile (`swapping`), so nothing written to it now could end up only in the old place. The lock,
/// the register and the shell follow where it lands (`migrate_landed`).
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

/// The background half of a move: the whole tree copied to the new place and compared, then the pointer written
/// (`settings::migrate`; copy first, pointer after, as always). Not one byte of the old place changes.
pub(super) fn copy_home(old: &std::path::Path, target: &std::path::Path) -> Result<crate::task::Done, crate::fault::Fault> {
    let from = crate::home::Home::open(old)?;
    let moved = crate::settings::migrate(&from, target)?;
    Ok(crate::task::Done::Copied { old: old.to_path_buf(), root: moved.root().to_path_buf() })
}

/// The frame half after a move copied the home: the lock moves to the new home, the register's cell and the
/// record bundle index follow, and the shell opens it.
pub(super) fn migrate_landed(shell: &mut Shell, old: &std::path::Path, root: &std::path::Path) -> Result<String, crate::fault::Fault> {
    let old = old.to_path_buf();
    let moved = crate::home::Home::open(root)?;
    let root = moved.root().display().to_string();
    // Not one byte of the old place changes: the person deletes it after moving. The lock moves to the new
    // home.
    let lock = crate::lock::take(&moved)?;
    // Identity homes are recorded in the register: the cell pointing to the old place changes to the new one
    // (signing key ownership and the next startup both read it). The register is the last step before the
    // shell is touched that can fail: if any step before it fails, the register and shell still point to the
    // old place; if it fails itself, the new lock is released with this frame and the shell keeps the old
    // place open, so both sides still agree.
    crate::register::change_listed(|reg| Ok(crate::identity::rehome(reg, &old, moved.root())))?;
    // After the whole move succeeds, record bundle index rows whose paths are under the old home move to the
    // new home. Failure does not undo the move (the home has moved and the shell must follow); the refusal
    // goes to the trouble bar and the index keeps pointing at the old place (not one byte of it changed, and
    // the bundles are still there).
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

/// The current Unix seconds. Used only for naming files, never for a decision (deadlines and windows use only
/// chain time and an injected now).
pub(super) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
