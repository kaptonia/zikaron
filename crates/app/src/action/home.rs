use super::*;

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

pub(super) fn set_machine_aside(shell: &mut Shell) -> Applied {
    let now = crate::shell::Vault::of(crate::keybox::state());
    let damaged = matches!(now, crate::shell::Vault::Damaged(_));
    shell.vault = now;
    if !damaged {
        let at = crate::home::machine_dir().map(|p| p.display().to_string()).unwrap_or_default();
        return shell.trouble(crate::fault::Fault::known(crate::fault::Known::KeyboxNotEmpty, at));
    }
    let day = crate::when::day((shell.clock)());
    match crate::home::set_machine_aside(&day) {
        Ok(a) => {
            if let Err(f) = crate::home::settle_machine() {
                shell.faults.push(f);
            }
            shell.after_lock();
            Applied::MachineAside { from: a.from.display().to_string(), to: a.to.display().to_string() }
        }
        Err(f) => shell.trouble(f),
    }
}

pub(super) fn view_old(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let at = std::path::PathBuf::from(root);
    let old = shell.aside.iter().find(|a| crate::home::same_place(&a.path, &at)).cloned().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::FileMissing, root.to_string()))?;
    let back = shell.old_view.as_ref().map(|v| v.back.clone()).or_else(|| shell.home.as_ref().map(|h| h.root().to_path_buf()));
    let mode = open_home_at(shell, root, false)?;
    shell.old_view = back.map(|back| crate::shell::OldView { back, at: old.at });
    Ok(mode)
}

pub(super) fn leave_old(shell: &mut Shell) -> Result<(String, crate::lock::Mode), crate::fault::Fault> {
    let v = shell.old_view.clone().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let root = v.back.display().to_string();
    let mode = open_home_at(shell, &root, false)?;
    shell.old_view = None;
    Ok((root, mode))
}

pub(super) fn take_writer(shell: &mut Shell) -> Result<(String, crate::lock::Mode), crate::fault::Fault> {
    let no_home = || crate::fault::Fault::known(crate::fault::Known::NoHome, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail005).to_string()));
    let home = shell.home.as_ref().ok_or_else(no_home)?;
    let lock = shell.lock.as_mut().ok_or_else(no_home)?;
    let root = home.root().display().to_string();
    if lock.mode() != crate::lock::Mode::OtherMachine {
        return Ok((root, lock.mode()));
    }
    let as_seat = opened_as(home.root());
    let as_seat = as_seat.as_ref().map(|(i, s)| (i.as_str(), *s));
    crate::local::check_label(home.root(), as_seat, false)?;
    lock.take_over(home)?;
    let label = crate::local::check_label(home.root(), as_seat, true);
    if let Err(f) = label {
        shell.faults.push(f);
    }
    Ok((root, crate::lock::Mode::Writer))
}

fn opened_as(root: &std::path::Path) -> Option<(String, crate::roles::Role)> {
    let (row, seat) = crate::register::now_row_listed().ok().flatten()?;
    row.home(seat).filter(|h| crate::home::same_place(h, root)).map(|_| (row.id.clone(), seat))
}

pub fn reopen_here(shell: &mut Shell, root: &std::path::Path) -> Result<crate::lock::Mode, crate::fault::Fault> {
    shell.lock = None;
    open_home_at(shell, &root.display().to_string(), false)
}

pub(super) fn change_home(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let at = crate::home::landing(root)?;
    crate::home::may_open_at(&at)?;
    let Some((id, seat, left)) = seat_home_left(&at)? else { return open_home(shell, root) };
    let was = follow(&id, seat, &left, &at, &at.display().to_string())?;
    match open_home_at(shell, &at.display().to_string(), true) {
        Ok(mode) => Ok(mode),
        Err(f) => {
            if let Some(was) = was {
                if let Err(back) = follow(&id, seat, &at, &left, &was) {
                    shell.faults.push(back);
                }
            }
            Err(f)
        }
    }
}

fn seat_home_left(at: &std::path::Path) -> Result<Option<(String, crate::roles::Role, std::path::PathBuf)>, crate::fault::Fault> {
    let Some((row, seat)) = crate::register::now_row_listed()? else { return Ok(None) };
    Ok(row.home(seat).filter(|h| !crate::home::same_place(h, at)).map(|h| (row.id.clone(), seat, h)))
}

fn follow(id: &str, seat: crate::roles::Role, from: &std::path::Path, to: &std::path::Path, value: &str) -> Result<Option<String>, crate::fault::Fault> {
    let was = crate::register::change_listed(|reg| Ok(crate::identity::seat_home_set(reg, id, seat, value)))?.flatten();
    crate::own::home_follows(from, to)?;
    Ok(was)
}

pub(super) fn open_home(shell: &mut Shell, root: &str) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let root = crate::home::landing(root)?;
    let root = root.display().to_string();
    let root = root.as_str();
    let identity_home = crate::register::now_row_listed()
        .ok()
        .flatten()
        .and_then(|(row, seat)| row.home(seat))
        .map(|h| h == std::path::Path::new(root.trim()))
        .unwrap_or(false);
    open_home_at(shell, root, !identity_home)
}

pub(super) fn open_home_at(shell: &mut Shell, root: &str, pointer: bool) -> Result<crate::lock::Mode, crate::fault::Fault> {
    let o = opening(root, pointer, &OpeningAsk::of(shell))?;
    Ok(opened(shell, o))
}

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

pub struct Opening {
    home: crate::home::Home,
    lock: Option<crate::lock::Lock>,
    mode: crate::lock::Mode,
    read: crate::shell::Hydrated,
    network_trouble: Option<crate::fault::Fault>,
    took_network: bool,
    retook: bool,
    writes: bool,
}

impl Opening {
    pub(super) fn seat_written(&mut self, seat: crate::roles::Role) -> Result<(), crate::fault::Fault> {
        if self.read.settings.role == seat {
            return Ok(());
        }
        if !self.mode.writable() {
            let holder = self.lock.as_ref().map(|l| l.holder().to_string()).unwrap_or_default();
            return Err(crate::fault::Fault::known(crate::fault::Known::ReadOnly, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail006, &[&holder]))));
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

pub(super) fn opening(root: &str, pointer: bool, ask: &OpeningAsk) -> Result<Opening, crate::fault::Fault> {
    let as_seat = opened_as(std::path::Path::new(root));
    crate::local::check_label(std::path::Path::new(root), as_seat.as_ref().map(|(i, s)| (i.as_str(), *s)), false)?;
    let home = crate::home::Home::open_or_create(root)?;
    let lock = if crate::lock::holds_writer_at(ask.held.as_deref(), &home) { None } else { Some(crate::lock::take_as(&home, ask.alone)?) };
    let mode = lock.as_ref().map(|l| l.mode()).unwrap_or(crate::lock::Mode::Writer);
    crate::local::check_label(home.root(), opened_as(home.root()).as_ref().map(|(i, s)| (i.as_str(), *s)), mode.writable())?;
    let read = crate::shell::Hydrated::read(&home)?;
    if pointer && !crate::home::named_by_env(home.root()) {
        crate::home::write_pointer(home.root())?;
    }
    let old_data = read.aside.iter().any(|a| crate::home::same_place(&a.path, home.root()));
    let mut read = read;
    let mut writes = false;
    let (mut network_trouble, mut took_network, mut retook) = (None, false, false);
    if mode.writable() && !old_data {
        writes = true;
        crate::home::sweep_temps(home.root());
        let bare = read.settings.chain_id.is_none() && read.settings.registry.is_none() && read.settings.endpoints.is_empty();
        match take_identity_network(&home, &mut read) {
            Ok(()) => took_network = bare && read.settings.chain_id.is_some(),
            Err(f) => network_trouble = Some(f),
        }
        retook = retake_on_opening(&home, &mut read);
    }
    Ok(Opening { home, lock, mode, read, writes, network_trouble, took_network, retook })
}

pub(super) fn opened(shell: &mut Shell, o: Opening) -> crate::lock::Mode {
    let Opening { home, lock, mode, read, writes, network_trouble, took_network, retook } = o;
    shell.source_changed(crate::shell::Source::Home);
    shell.install(read);
    shell.home = Some(home);
    if let Some(l) = lock {
        shell.lock = Some(l);
    }
    if let Some(f) = shell.lock.as_ref().and_then(|l| l.mark_trouble()).cloned() {
        shell.trouble(f);
    }
    if writes {
        if let Some(f) = network_trouble {
            shell.trouble(f);
        }
        if took_network || retook {
            tail_if_due(shell);
        }
    }
    let _ = remeasure(shell);
    shell.held = None;
    shell.tasks.forget(Kind::Held);
    let _ = list_held(shell);
    backfill_root(shell);
    resume(shell);
    mode
}

fn retake_on_opening(home: &crate::home::Home, read: &mut crate::shell::Hydrated) -> bool {
    if read.rooted {
        if let Err(f) = crate::restorex::forget_on(home) {
            read.troubles.push(f);
        }
        return false;
    }
    if read.unfetched.is_some() || !crate::restorex::moved(home, crate::restorex::On::of(&read.settings)) {
        return false;
    }
    if let Err(f) = crate::restorex::mark(home) {
        read.troubles.push(f);
    }
    read.unfetched = Some(crate::restorex::State::Unfetched);
    true
}

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
    crate::chainx::forget_serving();
    *read = crate::shell::Hydrated::read(home)?;
    Ok(())
}

pub(super) fn migrate_start(shell: &Shell, to: &str) -> Result<(std::path::PathBuf, std::path::PathBuf), crate::fault::Fault> {
    match shell.lock.as_ref() {
        None => {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::NoHome,
                crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail005).to_string()),
            ))
        }
        Some(l) if !l.mode().writable() => {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::ReadOnly,
                crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()])),
            ))
        }
        Some(_) => {}
    }
    let target = crate::home::landing(to)?;
    let Some(home) = shell.home.as_ref() else {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::FileMissing,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail017).to_string()),
        ));
    };
    crate::settings::outside_home(home.root(), &target)?;
    Ok((home.root().to_path_buf(), target))
}

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

pub(super) fn copy_home(old: &std::path::Path, target: &std::path::Path) -> Result<crate::task::Done, crate::fault::Fault> {
    let from = crate::home::Home::open(old)?;
    let moved = crate::settings::migrate(&from, target)?;
    Ok(crate::task::Done::Copied { old: old.to_path_buf(), root: moved.root().to_path_buf() })
}

pub(super) fn migrate_landed(shell: &mut Shell, old: &std::path::Path, root: &std::path::Path) -> Result<String, crate::fault::Fault> {
    let old = old.to_path_buf();
    let moved = crate::home::Home::open(root)?;
    let root = moved.root().display().to_string();
    let lock = crate::lock::take_as(&moved, shell.machine.cli_alone.is_some())?;
    crate::register::change_listed(|reg| Ok(crate::identity::rehome(reg, &old, moved.root())))?;
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
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail018).to_string()),
        ));
    }
    shell.commit_settings(|s| s.cap_bytes = bytes)?;
    Ok(bytes)
}

pub(super) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
