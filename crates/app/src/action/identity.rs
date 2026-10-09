use super::*;

/// Enters one seat of an identity: opens that seat's home (without writing the machine pointer, since identity
/// homes are recorded in the register), aligns the seat in settings, reads the key once, and rereads the
/// identity table.
pub(super) fn enter(shell: &mut Shell, row: &crate::identity::Row, seat: crate::roles::Role) -> Result<(), crate::fault::Fault> {
    // An empty seat has no home to open: the seat still switches and readings are reread, but no home opens.
    // `signing_key` then refuses because the home does not belong to the current seat, so nothing can be
    // signed from it, and the UI says the seat is unoccupied and how to fill it.
    let home = row.home(seat);
    if let Some(root) = home.as_ref() {
        open_home_at(shell, &root.display().to_string(), false)?;
    }
    // With no home the shell releases the home and lock, and each page shows the "no home" state. The seat is
    // set only on the shell, never written into the other seat's home (a home's settings belong to its own
    // seat); the register still records it.
    if home.is_none() {
        shell.close_home();
        shell.settings.role = seat;
    } else if shell.settings.role != seat {
        shell.commit_settings(|s| s.role = seat)?;
    }
    shell.refresh_anchor()?;
    shell.seat_identities(Some(crate::register::view(seat)?));
    Ok(())
}

/// The last step of creating an identity. The copy check runs on the UI thread so mistakes show at once;
/// deriving both seats' keys and sealing the recovery credential run in the background (on the UI thread they
/// freeze the window for about a second), and [`vault_landed`] continues into the home when it lands.
/// `network` is recorded on the identity's row in the same pass.
pub(super) fn confirm_identity(shell: &mut Shell, answers: &[(usize, crate::secret::Secret)], label: &str, network: &str) -> Result<Applied, crate::fault::Fault> {
    let Some(f) = shell.new_words.as_ref() else {
        return Err(crate::fault::Fault::known(crate::fault::Known::PhraseConfirm, String::new()));
    };
    crate::identity::confirm(f, answers)?;
    let network = network_choice(network)?;
    // The label is written to the register when this pass lands (by `identity::rename`); empty means nothing
    // is written.
    shell.new_label = Some(label.trim().to_string());
    let before = registered()?;
    let f = f.twin();
    let role = shell.settings.role;
    let label = label.trim().to_string();
    Ok(vault(shell, move || {
        // Words confirmed by copying count as a backup.
        let row = crate::register::change(role, |reg| {
            let row = crate::identity::add_words(reg, &f, true, &label)?;
            crate::identity::choose_network(reg, &row.id, &network)
        })?;
        let restored = before.contains(&row.id);
        Ok(crate::task::Vault::Identity { row, restored, fresh: true })
    }))
}

/// The network an identity is created with, checked on the UI thread: a row of the known deployments table, or
/// "custom". Anything else is refused by name before the background pass (and before any key is made).
fn network_choice(name: &str) -> Result<String, crate::fault::Fault> {
    if !crate::deploy::is_choice(name) {
        return Err(crate::fault::Fault::known(crate::fault::Known::IdentitiesShape, name.to_string()));
    }
    Ok(name.to_string())
}

/// The identity pass landed: enter its first occupied seat (key and home switch together) and report it
/// created.
pub(super) fn identity_landed(shell: &mut Shell, row: crate::identity::Row, restored: bool, fresh: bool) -> Applied {
    if fresh {
        shell.new_words = None;
    }
    // An imported identity's homes get the "not fetched" mark: importing (from words, private key or key file)
    // is restoring, and the key may have written elsewhere. A freshly created identity (`fresh`) has no ledger
    // elsewhere and gets no mark. For an identity new to this machine's register (`restored` only refills
    // missing slots of an existing identity and does not count), both seats' homes are marked, whether or not
    // they already have a ledger (it may predate an identity deletion, and writing may have continued
    // elsewhere). The mark comes off only when the seat's ledger is checked against the key's on-chain anchors
    // with nothing missing (`check_tail`); an empty ledger and a key with no anchors pass at once. If marking
    // fails, the home is treated as marked (read-only).
    let mut mark_failed = false;
    if !fresh && !restored {
        for seat in row.seats() {
            if let Some(dir) = row.home(seat) {
                if let Err(f) = crate::home::Home::open_or_create(&dir).and_then(|h| crate::restorex::mark(&h)) {
                    shell.faults.push(f);
                    mark_failed = true;
                }
            }
        }
    }
    // The label from the create and import paths is written to the register on landing (empty means not
    // written). A failure only leaves the identity unnamed, so it is reported without failing the landing.
    let mut row = row;
    if let Some(label) = shell.new_label.take().filter(|l| !l.trim().is_empty()) {
        match crate::register::change(shell.settings.role, |reg| crate::identity::rename(reg, &row.id, &label)) {
            Ok(fresh_row) => row = fresh_row,
            Err(f) => shell.faults.push(f),
        }
    }
    let landed = row.first_seat();
    match enter(shell, &row, landed) {
        Ok(()) => {
            if mark_failed && shell.unfetched.is_none() {
                shell.unfetched = Some(crate::restorex::State::Unfetched);
            }
            // The marks come off only through the tail check: it starts now if basis and nodes are set (an
            // empty ledger and a key with no anchors pass at once), or later when they are.
            tail_if_due(shell);
            Applied::IdentityMade { id: row.id.clone(), seats: seats_of(&row), restored }
        }
        Err(f) => shell.trouble(f),
    }
}

/// The seats a row occupies, with their addresses.
pub(super) fn seats_of(row: &crate::identity::Row) -> Vec<(crate::roles::Role, Address)> {
    row.seats().into_iter().filter_map(|s| row.address(s).map(|a| (s, a))).collect()
}

/// The identity ids currently in the register. An import matching one refills its missing slots
/// (`identity::restored`) instead of creating a new identity.
pub(super) fn registered() -> Result<Vec<String>, crate::fault::Fault> {
    Ok(crate::register::read()?.map(|r| r.rows.into_iter().map(|x| x.id).collect()).unwrap_or_default())
}

/// Imports an identity. `seat` is the seat an existing key occupies (only that one; the other stays empty).
/// The recovery-words form derives both seats and lands on the first occupied one (`Row::first_seat`), so
/// there `seat` only selects which identity table to read, not where to land.
pub(super) fn import_identity(shell: &mut Shell, form: ImportForm, seat: crate::roles::Role, label: &str, network: &str) -> Result<Applied, crate::fault::Fault> {
    // The key file fields are checked on the UI thread, as on export (`key_file_checks`), so a mistyped
    // password is reported before the background pass.
    if let ImportForm::PrivateKey { keyfile: Some(out), .. } = &form {
        key_file_checks(&out.password, &out.again, &out.dir)?;
    }
    let network = network_choice(network)?;
    shell.new_label = Some(label.trim().to_string());
    let before = registered()?;
    // Key extraction (keystore scrypt, word derivation) and sealing the recovery credential run in the
    // background; landing is the same as for `confirm_identity`.
    let label = label.trim().to_string();
    Ok(vault(shell, move || {
        let row = imported(form, seat, &network, &label)?;
        let restored = before.contains(&row.id);
        Ok(crate::task::Vault::Identity { row, restored, fresh: false })
    }))
}

/// The disk and vault work of importing (runs in the background). The row records `network` only if it has
/// none (re-importing an identity keeps its network).
pub(super) fn imported(form: ImportForm, seat: crate::roles::Role, network: &str, label: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let row = match form {
        // Importing from words: the person holds the words, which counts as a backup.
        ImportForm::Words(text) => {
            let f = crate::identity::from_words(text.expose())?;
            crate::register::change(seat, |reg| {
                let row = crate::identity::add_words(reg, &f, true, label)?;
                crate::identity::choose_network(reg, &row.id, network)
            })?
        }
        ImportForm::PrivateKey { key, keyfile } => {
            let s = crate::key::from_hex(key.expose()).ok_or_else(|| Fault::known(Known::KeyMalformed, String::new()))?;
            // An import that creates the primary identity (the store has none yet) writes its key file first:
            // the primary is the only identity that can reopen the store after a forgotten passcode, and a
            // bare key has no other recovery path. Nothing is enrolled before the file is read back
            // (`landed_check`); without the key file fields it is refused by name. A secondary import that
            // names a key file writes it too.
            let becomes_primary = crate::keybox::primary()?.is_none();
            // Check against the register before writing a key file, so an import refused by name leaves no file
            // behind (`identity::meets`, rechecked when adding below).
            if let (Some(_), Some(addr)) = (&keyfile, s.address()) {
                // The same register view the adding below sees (`register::change` reads it via
                // `identity::view`; a machine with no register yet lists its account-base key).
                let reg = crate::register::view(seat)?;
                crate::identity::meets(&reg, &crate::identity::Keys::One { seat, addr }, label)?;
            }
            let landed = match keyfile {
                Some(out) => {
                    let dir = key_file_checks(&out.password, &out.again, &out.dir)?;
                    Some(write_key_file(&s, &out.password, &dir)?.0)
                }
                None if becomes_primary => return Err(Fault::known(Known::PrimaryNoKeyFile, String::new())),
                None => None,
            };
            crate::register::change(seat, |reg| {
                let row = crate::identity::add_existing(reg, &s, seat, landed.is_some(), label)?;
                if let Some(at) = landed.as_deref() {
                    crate::identity::mark(reg, &row.id, false, true, Some(at))?;
                }
                crate::identity::choose_network(reg, &row.id, network)
            })?
        }
        // Importing from a keystore file: the person holds the file, which counts as a backup.
        ImportForm::Keystore { path, password } => {
            let p = path.trim();
            let bytes = std::fs::read(p).map_err(|e| crate::fault::classify(&e, p))?;
            let s = crate::keystore::decrypt_typed(&bytes, &password)?;
            crate::register::change(seat, |reg| {
                let row = crate::identity::add_existing(reg, &s, seat, true, label)?;
                crate::identity::choose_network(reg, &row.id, network)
            })?
        }
    };
    Ok(row)
}

pub(super) fn switch_identity(shell: &mut Shell, id: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    // Land on its first occupied seat, so an identity occupying only the user seat lands there.
    let seat = crate::register::view(shell.settings.role)?
        .find(id)
        .map(|r| r.first_seat())
        .unwrap_or(crate::roles::Role::Author);
    let row = crate::register::change(seat, |reg| crate::identity::switch(reg, id, seat))?;
    enter(shell, &row, seat)?;
    Ok(row)
}

pub(super) fn delete_identity(shell: &mut Shell, id: &str) -> Result<String, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let seat = shell.settings.role;
    let reg = crate::register::view(seat)?;
    let row = reg.find(id).cloned().ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    // The handover check covers the ledgers in every seat's home. Checking only the recorder seat would ignore a
    // succession in the user seat's ledger (held grants), which could never be signed again after deletion. So
    // every seat the row occupies is read, using the closed seat list from `Row::seats`.
    let handed = !row.backed() && {
        let mut items: Vec<Vec<u8>> = Vec::new();
        for s in row.seats() {
            let Some(home) = row.home(s) else { continue };
            let dir = home.join(crate::home::Slot::Ledger.as_str());
            // Sealed entries are read through `local::Ledger`, never as raw bytes. A seat never opened has no
            // ledger yet; one that exists but cannot be read is refused by name (reading it as empty would turn
            // "handed over" into "not backed up").
            if dir.is_dir() {
                items.extend(crate::local::Ledger::open(&dir)?.survey()?.items);
            }
        }
        row.seats()
            .into_iter()
            .filter_map(|s| row.address(s))
            .any(|a| crate::succeedx::handed_over(&items, Some(a)).is_some())
    };
    let (gone, next) = crate::register::change(seat, |reg| crate::identity::delete(reg, id, handed))?;
    match next {
        Some((r, s)) => enter(shell, &r, s)?,
        None => {
            shell.refresh_anchor()?;
            shell.seat_identities(Some(crate::register::view(seat)?));
        }
    }
    Ok(gone.id)
}

/// Which of a row's seats have a ledger with entries (the delete dialog warns these stay behind), read from
/// disk by the action layer, never by the window. A seat whose ledger exists but cannot be read counts as
/// having entries (the warning shows), and its error is returned for display, never read as "none".
pub fn seats_with_entries(row: &crate::identity::Row) -> (Vec<crate::roles::Role>, Vec<crate::fault::Fault>) {
    let mut seats = Vec::new();
    let mut troubles = Vec::new();
    for s in row.seats() {
        let Some(home) = row.home(s) else { continue };
        let dir = home.join(crate::home::Slot::Ledger.as_str());
        if !dir.is_dir() {
            continue;
        }
        match crate::local::Ledger::open(&dir).and_then(|l| l.store().layout().map_err(|t| crate::fault::Fault::known(crate::fault::Known::Ledger, format!("{t:?}")))) {
            Ok(l) if l.entries > 0 => seats.push(s),
            Ok(_) => {}
            Err(f) => {
                seats.push(s);
                troubles.push(f);
            }
        }
    }
    (seats, troubles)
}

/// The passcode must match both times (the passcode rules themselves live in `keybox::pin_trouble`).
pub(super) fn same_twice(pin: &crate::secret::Secret, again: &crate::secret::Secret) -> Result<(), crate::fault::Fault> {
    if pin != again {
        return Err(crate::fault::Fault::known(crate::fault::Known::PasswordsDiffer, String::new()));
    }
    Ok(())
}

/// The wizard's network step. The choice becomes the network of the identity the wizard created: recorded on
/// the current identity's row (both seats share it) and in the machine settings as this machine's last choice
/// (preselected by the wizard and the new-identity dialog; only the row name is stored, the values always come
/// from the table). The current home follows the choice unless its network was configured by hand: a known row
/// fills it, "custom" leaves it without a network to be filled in settings. Returns whether this home now has
/// the row's network.
pub(super) fn choose_network(shell: &mut Shell, name: &str) -> Result<bool, crate::fault::Fault> {
    shell.machine = crate::machine::update(|m| m.network = Some(name.to_string()))?;
    // The machine choice is saved, so a later failure is not a total failure: returning an error would say
    // choosing failed while the machine's choice did change. Each later failure is recorded as a trouble.
    // The shell's identity table is reread too, since the wizard reads the current identity's choice from it.
    let recorded = crate::register::now_row_listed().and_then(|now| match now {
        Some((row, _)) => crate::register::change_listed(|reg| crate::identity::set_network(reg, &row.id, name))
            .and_then(|_| crate::register::view(shell.settings.role))
            .map(|v| shell.seat_identities(Some(v))),
        None => Ok(()),
    });
    if let Err(f) = recorded {
        shell.trouble(f);
    }
    let s = &shell.settings;
    let bare = s.chain_id.is_none() && s.registry.is_none() && s.endpoints.is_empty();
    // A home whose network came from a choice records which (`Settings::network`); one configured by hand
    // records none and is left alone.
    let from_a_choice = bare || s.network.is_some();
    if shell.home.is_none() || !shell.writable() || !from_a_choice {
        return Ok(false);
    }
    let done = match crate::deploy::named(name) {
        Some(d) => adopt_network(shell, crate::deploy::Network::Known(d)).map(|()| true),
        None => clear_network(shell).map(|()| false),
    };
    match done {
        Ok(filled) => Ok(filled),
        Err(f) => {
            shell.trouble(f);
            Ok(false)
        }
    }
}

pub(super) fn set_auto_lock(shell: &mut Shell, on: bool, secs: u64) -> Result<(bool, u64), crate::fault::Fault> {
    // If the machine file is unreadable, stop: writing an empty copy back would erase this machine's chosen
    // network and the file's other fields while the UI said "set".
    shell.machine = crate::machine::update(|m| {
        m.auto_lock = on;
        m.auto_lock_secs = secs;
    })?;
    Ok((shell.machine.auto_lock, shell.machine.auto_lock_secs))
}

/// Writes a whole-machine backup (after the passcode gate). The password must be at least
/// `backup::PASSWORD_MIN` characters and typed twice identically, and the folder absolute; the package is
/// collected, sealed, written and read back in the background.
pub(super) fn export_backup(shell: &mut Shell, password: crate::secret::Secret, again: &crate::secret::Secret, dir: &str) -> Result<Spawned, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // Same refusal as for a key file (see `key_file_checks`): a password longer than the secret buffer would be
    // truncated, and the backup must never be sealed under only its head.
    if password.overflowed() || again.overflowed() {
        return Err(Fault::known(Known::PasswordLong, crate::secret::CAP.to_string()));
    }
    if &password != again {
        return Err(Fault::known(Known::PasswordsDiffer, String::new()));
    }
    if password.chars() < crate::backup::PASSWORD_MIN {
        return Err(Fault::known(Known::PasswordShort, password.chars().to_string()));
    }
    let dir = crate::home::landing(dir)?;
    let now = (shell.clock)();
    Ok(shell.tasks.spawn(Kind::Backup, move || {
        std::fs::create_dir_all(&dir).map_err(|e| crate::fault::classify(&e, &dir.display().to_string()))?;
        let done = crate::backup::export(&dir, password.expose(), now)?;
        Ok(Done::BackupMade { path: done.path.display().to_string(), summary: done.summary })
    }))
}

/// Writes the appearance to machine settings. A name outside the closed set is refused by name and the file is
/// untouched; unreadable settings are not overwritten.
pub(super) fn set_appearance(shell: &mut Shell, appearance: &str) -> Result<String, crate::fault::Fault> {
    shell.machine = crate::machine::update(|m| m.appearance = Some(appearance.to_string()))?;
    Ok(shell.machine.appearance.clone().unwrap_or_default())
}

/// Writes this machine's proxy choice: `system`, `none`, or a proxy address parsed by the transport's own
/// parser (`zikaron_net::proxy_of`); an address it rejects is refused by name before anything is written.
pub(super) fn set_proxy(shell: &mut Shell, choice: &str) -> Result<String, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let c = choice.trim();
    let written = match c {
        crate::machine::proxy::SYSTEM | crate::machine::proxy::NONE => c.to_string(),
        typed => match zikaron_net::proxy_of(typed) {
            Ok(p) => p.spelled(),
            Err(zikaron_net::NotAProxy::Credentials) => return Err(Fault::known(Known::SettingsShape, crate::lang::filln(crate::lang::Key::TailProxyCredentials, &[typed]))),
            Err(zikaron_net::NotAProxy::Shape) => return Err(Fault::known(Known::SettingsShape, crate::lang::filln(crate::lang::Key::TailProxyShape, &[typed]))),
        },
    };
    shell.machine = crate::machine::update(|m| m.proxy = Some(written))?;
    Ok(shell.machine.proxy.clone().unwrap_or_default())
}

pub(super) fn reveal_words(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // One passcode check per reveal. The passcode is checked by the shared gate in `apply`
    // (`Action::pin_asked`).
    let seat = shell.settings.role;
    // Only `identity::now_row` decides which row is current (the backup path uses it too).
    let view = crate::register::view(seat)?;
    let id = crate::identity::now_row(&view)
        .map(|(r, _)| r.id)
        .ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    shell.words = Some(crate::identity::words_of(&view, &id)?);
    Ok(())
}

/// Renames an identity. Changes only the register's label, which carries no meaning.
pub(super) fn name_identity(shell: &mut Shell, id: &str, label: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    let seat = shell.settings.role;
    let row = crate::register::change(seat, |reg| crate::identity::rename(reg, id, label))?;
    shell.seat_identities(Some(crate::register::view(seat)?));
    Ok(row)
}

/// Checks the key file fields before anything is written: the password typed twice identically, at least the
/// minimum length, and an output folder (`home::landing`). Returns the folder to write to. Shared by key file
/// export and importing a key that becomes primary.
pub(super) fn key_file_checks(password: &crate::secret::Secret, again: &crate::secret::Secret, dir: &str) -> Result<String, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // A password longer than the secret buffer would be truncated: the key file would be sealed under a
    // password the person never typed in full (and another wallet given the whole one would not open it).
    if password.overflowed() || again.overflowed() {
        return Err(Fault::known(Known::PasswordLong, crate::secret::CAP.to_string()));
    }
    if password != again {
        return Err(Fault::known(Known::PasswordsDiffer, String::new()));
    }
    // The minimum counts characters only; the strength meter is advisory and there is no weak-password gate
    // (see `strength`).
    if password.chars() < crate::backup::PASSWORD_MIN {
        return Err(Fault::known(Known::PasswordShort, password.chars().to_string()));
    }
    // An empty or relative output folder is refused before any background work (`home::landing`).
    Ok(crate::home::landing(dir)?.display().to_string())
}

/// Writes `secret` as a key file under `password` into `dir` and reads it back (`landed_check`). Returns the
/// path and the address it carries. Runs in the background (scrypt at the standard level). Shared by key file
/// export and importing a key that becomes primary.
pub(super) fn write_key_file(secret: &crate::key::Secret, password: &crate::secret::Secret, dir: &str) -> Result<(String, Address), crate::fault::Fault> {
    let ks = crate::keystore::encrypt(secret, password.expose(), Params::standard(), now_secs())?;
    std::fs::create_dir_all(dir).map_err(|e| crate::fault::classify(&e, dir))?;
    let path = std::path::Path::new(dir).join(&ks.file_name);
    // The key file is readable only by its owner: it is ciphertext, but another account on the same machine
    // could copy it and brute-force the password offline. Same protection as the vault file's 0600.
    zikaron_glue::landing::land_bytes_for(zikaron_glue::landing::Readers::Owner, &path, &ks.json)
        .map_err(|t| crate::fault::Fault::of_landing(t))?;
    // Read it back and compare: "backed up" is judged by the file on disk, not by the write returning no
    // error. A full or read-only disk, a swapped target or mismatched bytes can slip past the write, and a
    // false "backed up" flag could lead the person to delete the identity and lose the key forever.
    // Unreadable, wrong shape or wrong address is refused by name as `BACKUP_NOT_LANDED`, and the pass does
    // not count as a backup.
    if let Some(tamper) = LANDED_TAMPER.get() {
        tamper(&path);
    }
    landed_check(&path, ks.address)?;
    Ok((path.display().to_string(), ks.address))
}

pub(super) fn backup_key(shell: &mut Shell, password: crate::secret::Secret, again: &crate::secret::Secret, dir: &str) -> Result<Spawned, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let dir = key_file_checks(&password, again, dir)?;
    let seat = shell.settings.role;
    let view = crate::register::view(seat)?;
    let acct = crate::identity::account_now(&view)
        .ok_or_else(|| Fault::known(Known::SeatUnseated, crate::lang::t(crate::lang::Key::IdSeatEmpty).to_string()))?;
    let secret = crate::key::load(Some(&acct))?.ok_or_else(|| Fault::known(Known::KeychainMissing, acct.clone()))?;
    let id = crate::identity::now_row(&view).map(|(r, _)| r.id);
    // scrypt at the standard level runs in the background: the key is loaded from the key vault on the UI
    // thread and moved into the task while the UI shows that it is encrypting. The "backed up" record is
    // written when the shell receives the result.
    Ok(shell.tasks.spawn(Kind::Keystore, move || {
        let (path, address) = write_key_file(&secret, &password, &dir)?;
        Ok(Done::Keystore(crate::task::Keystore::BackedUp { path, address, id, seat }))
    }))
}

/// Installs the post-write tamper hook once (tests only). Returns false if already set; it is never replaced.
pub fn set_landed_tamper(f: fn(&std::path::Path)) -> bool {
    LANDED_TAMPER.set(f).is_ok()
}

/// Whether the key file really landed on disk. Each failure is named: unreadable, not keystore V3 shape, or an
/// address other than this key's.
pub(super) fn landed_check(path: &std::path::Path, want: Address) -> Result<(), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let at = path.display().to_string();
    let back = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &at))?;
    let shape = crate::keystore::shape(&back)
        .map_err(|f| Fault::known(Known::BackupNotLanded, format!("{at}: {}", f.said())))?;
    let bad = shape.compliant();
    if !bad.is_empty() {
        return Err(Fault::known(Known::BackupNotLanded, format!("{at}: {}", bad.join(" "))));
    }
    if shape.address != Some(want) {
        return Err(Fault::known(Known::BackupNotLanded, format!("{at}: {} ≠ {}", shape.address_text, want.hex())));
    }
    Ok(())
}
