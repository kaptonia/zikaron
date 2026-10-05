use super::*;

/// Enter one seat of an identity. Open that seat's home (without writing the machine pointer: identity homes
/// are recorded in the register, and the pointer only follows the person's choice of home), align the seat in
/// settings, ask for the key once, and reread the identity table.
pub(super) fn enter(shell: &mut Shell, row: &crate::identity::Row, seat: crate::roles::Role) -> Result<(), crate::fault::Fault> {
    // An empty seat has no home to open: the seat still switches and readings are reread, but opening the
    // home is skipped. After that `signing_key` answers "this home does not belong to the current seat", so
    // not one byte can be signed from this seat, and the face line says "unoccupied" and how to fill it.
    let home = row.home(seat);
    if let Some(root) = home.as_ref() {
        open_home_at(shell, &root.display().to_string(), false)?;
    }
    // An empty seat has no home, and this lands on the shell: the home and lock are released, and each page
    // speaks from the "no home" state. The seat is set only on the shell, never written into the other seat's
    // home (a home's settings belong to its own seat); the register still records that cell.
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

/// The last step of creating an identity. The copy check is judged in the frame at once (mistakes are said at
/// once); deriving both seats' keys and sealing the recovery credential run in the background (on the
/// interface thread each identity creation froze the window for a second), and on landing [`vault_landed`]
/// continues into the home. `network` is the identity's network, recorded on its row in the same pass.
pub(super) fn confirm_identity(shell: &mut Shell, answers: &[(usize, crate::secret::Secret)], label: &str, network: &str) -> Result<Applied, crate::fault::Fault> {
    let Some(f) = shell.new_words.as_ref() else {
        return Err(crate::fault::Fault::known(crate::fault::Known::PhraseConfirm, String::new()));
    };
    crate::identity::confirm(f, answers)?;
    let network = network_choice(network)?;
    // The label is written to the register when this pass lands (`identity::rename` is the one owner); empty
    // means the cell is not written.
    shell.new_label = Some(label.trim().to_string());
    let before = registered()?;
    let f = f.twin();
    let role = shell.settings.role;
    Ok(vault(shell, move || {
        // Words confirmed by copying count as one form of backup.
        let row = crate::register::change(role, |reg| {
            let row = crate::identity::add_words(reg, &f, true)?;
            crate::identity::choose_network(reg, &row.id, &network)
        })?;
        let restored = before.contains(&row.id);
        Ok(crate::task::Vault::Identity { row, restored, fresh: true })
    }))
}

/// The network an identity is made with, judged in the frame: a row of the known deployments table, or
/// "custom"; anything else is refused by name before the background pass (and before any key is made).
fn network_choice(name: &str) -> Result<String, crate::fault::Fault> {
    if !crate::deploy::is_choice(name) {
        return Err(crate::fault::Fault::known(crate::fault::Known::IdentitiesShape, name.to_string()));
    }
    Ok(name.to_string())
}

/// The identity pass landed: enter its first occupied seat (key and home switched together) and answer
/// "created".
pub(super) fn identity_landed(shell: &mut Shell, row: crate::identity::Row, restored: bool, fresh: bool) -> Applied {
    if fresh {
        shell.new_words = None;
    }
    // An imported identity's homes get the "not fetched" mark: importing is restoring (words, private key and
    // key file alike), and the key may have written elsewhere; a freshly created identity (`fresh`) has no
    // ledger elsewhere and gets no mark. For an identity restored onto this machine (not in the register
    // before; `restored`, which only refills missing slots of an existing identity, does not count), both
    // seats' homes are marked, whether or not the home already has a ledger (it may be older than the
    // identity deletion, and writing may have continued elsewhere). The mark comes off one way only: this
    // seat's ledger checked against this key's anchors on chain with nothing missing (`check_tail`), whatever
    // the ledger came from; an empty ledger and a key with no anchors pass at once. If marking fails, it is
    // treated as marked (read-only, writing not opened).
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
    // Label: the cell taken by the create and import paths, written to the register when landing; empty means
    // not written. Failing to write only leaves the face without a name (it carries no weight), so it records
    // a trouble and continues, without failing the whole identity landing.
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
            // The marks come off only by the tail check, whatever the ledger's source: started now when
            // basis and nodes are set (an empty ledger and a key with no anchors pass on the spot), or when
            // they are set later.
            tail_if_due(shell);
            Applied::IdentityMade { id: row.id.clone(), seats: seats_of(&row), restored }
        }
        Err(f) => shell.trouble(f),
    }
}

/// The seats a row occupies and their addresses (the copy handed out in the answer).
pub(super) fn seats_of(row: &crate::identity::Row) -> Vec<(crate::roles::Role, Address)> {
    row.seats().into_iter().filter_map(|s| row.address(s).map(|a| (s, a))).collect()
}

/// Which identities are in the register now. An import matching one of them refills the missing slots
/// (`identity::restored`), not a new identity.
pub(super) fn registered() -> Result<Vec<String>, crate::fault::Fault> {
    Ok(crate::register::read()?.map(|r| r.rows.into_iter().map(|x| x.id).collect()).unwrap_or_default())
}

/// Import an identity. `seat` is which seat an existing key occupies (only that one; the other seat stays
/// empty). The recovery-words branch still derives both seats and lands on its first occupied seat
/// (`Row::first_seat`), so in that form `seat` is used only to read the current identity table, not to decide
/// the landing; the face line therefore appears only for the two existing-key forms.
pub(super) fn import_identity(shell: &mut Shell, form: ImportForm, seat: crate::roles::Role, label: &str, network: &str) -> Result<Applied, crate::fault::Fault> {
    // The key file's cells are judged in the frame, as exporting one is (`key_file_checks`): a mistyped
    // password is said at once, before the background pass.
    if let ImportForm::PrivateKey { keyfile: Some(out), .. } = &form {
        key_file_checks(&out.password, &out.again, &out.dir)?;
    }
    let network = network_choice(network)?;
    shell.new_label = Some(label.trim().to_string());
    let before = registered()?;
    // Key extraction (the keystore's scrypt, word derivation) and sealing the recovery credential run in the
    // background; landing is the same as `confirm_identity`.
    Ok(vault(shell, move || {
        let row = imported(form, seat, &network)?;
        let restored = before.contains(&row.id);
        Ok(crate::task::Vault::Identity { row, restored, fresh: false })
    }))
}

/// The disk and vault work of importing (run in the background pass). The row records `network` when it has
/// none (an identity imported again keeps the one it had).
pub(super) fn imported(form: ImportForm, seat: crate::roles::Role, network: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let row = match form {
        // Importing from words: the person holds the words, which is one form of backup.
        ImportForm::Words(text) => {
            let f = crate::identity::from_words(text.expose())?;
            crate::register::change(seat, |reg| {
                let row = crate::identity::add_words(reg, &f, true)?;
                crate::identity::choose_network(reg, &row.id, network)
            })?
        }
        ImportForm::PrivateKey { key, keyfile } => {
            let s = crate::key::from_hex(key.expose()).ok_or_else(|| Fault::known(Known::KeyMalformed, String::new()))?;
            // An import that makes the primary identity (the store has none yet) lands its key file first,
            // in this pass: the primary is the one identity that reopens the store after a forgotten
            // passcode, and a bare key has no other way back. Nothing is enrolled before the file is read
            // back (`landed_check`); without the cells it is refused by name. A secondary import that names
            // a key file lands it too; one that names none is as before.
            let becomes_primary = crate::keybox::primary()?.is_none();
            let landed = match keyfile {
                Some(out) => {
                    let dir = key_file_checks(&out.password, &out.again, &out.dir)?;
                    Some(write_key_file(&s, &out.password, &dir)?.0)
                }
                None if becomes_primary => return Err(Fault::known(Known::PrimaryNoKeyFile, String::new())),
                None => None,
            };
            crate::register::change(seat, |reg| {
                let row = crate::identity::add_existing(reg, &s, seat, landed.is_some())?;
                if let Some(at) = landed.as_deref() {
                    crate::identity::mark(reg, &row.id, false, true, Some(at))?;
                }
                crate::identity::choose_network(reg, &row.id, network)
            })?
        }
        // Importing from a keystore file: the person holds the file, which is one form of backup.
        ImportForm::Keystore { path, password } => {
            let p = path.trim();
            let bytes = std::fs::read(p).map_err(|e| crate::fault::classify(&e, p))?;
            let s = crate::keystore::decrypt(&bytes, password.expose())?;
            crate::register::change(seat, |reg| {
                let row = crate::identity::add_existing(reg, &s, seat, true)?;
                crate::identity::choose_network(reg, &row.id, network)
            })?
        }
    };
    Ok(row)
}

pub(super) fn switch_identity(shell: &mut Shell, id: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    // Land on its first occupied seat: an identity occupying only the user seat still lands on the user seat
    // after switching.
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
    // The handover question includes the ledgers in both seats' homes.
    //
    // Looking only at the recorder seat's home would hurt someone who used both seats: after deleting the
    // identity, the user seat's ledger (held grants) could never be signed again, and a succession in it
    // would never have been considered when judging "can it be deleted". So the ledger question is read seat
    // by seat for every seat this row occupies, with the closed seat table given by `Row::seats`, not
    // hard-coded to one seat here.
    let handed = !row.backed() && {
        let mut items: Vec<Vec<u8>> = Vec::new();
        for s in row.seats() {
            let Some(home) = row.home(s) else { continue };
            let dir = home.join(crate::home::Slot::Ledger.as_str());
            // Sealed entries: read through `local::Ledger`, never as raw bytes.
            // A seat never opened has no ledger yet; one that is there but cannot be read is refused by name
            // (read as empty, it would turn "handed over" into "not backed up").
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

/// Which of a row's seats have a ledger with entries (what the delete sheet warns will stay behind), read from
/// disk once by the action layer, never by the window. A seat whose ledger is there but cannot be read counts
/// as having entries (the warning shows) and its refusal is handed back to be said, never read as "none".
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

/// The passcode must match both times (the rule itself is judged by `keybox::pin_trouble`, one owner).
pub(super) fn same_twice(pin: &crate::secret::Secret, again: &crate::secret::Secret) -> Result<(), crate::fault::Fault> {
    if pin != again {
        return Err(crate::fault::Fault::known(crate::fault::Known::PasswordsDiffer, String::new()));
    }
    Ok(())
}

/// The wizard's network step. The choice is the network of the identity the wizard made: recorded on the
/// current identity's row (both its seats take it), and in the machine-level settings file as this machine's
/// last choice (which choice the wizard and the new-identity sheet select first; the row name exists only
/// there, the literals always come from the table). The current home follows the choice unless the person
/// configured its network by hand: a row fills it, "custom" leaves it without a network, to be filled in
/// settings. Returns whether this home now has the row's network.
pub(super) fn choose_network(shell: &mut Shell, name: &str) -> Result<bool, crate::fault::Fault> {
    let mut m = crate::machine::read()?;
    m.network = Some(name.to_string());
    crate::machine::write(&m)?;
    shell.machine = crate::machine::read()?;
    // The machine cell is settled, so a later step failing is not a whole failure: returning an error would
    // make the face say "choosing the network failed" while the machine's choice did change. Each later
    // failure records a trouble and says plainly what was not done.
    // The shell's copy of the table is reread with it: the wizard's network step reads the current identity's
    // choice from there.
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
    // A home whose network came from a choice records which (`Settings::network`); one the person configured
    // by hand records none and is not touched.
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
    // Unreadable means leave (as with the two network-choosing exits): writing an empty copy back would erase
    // this machine's chosen network together with the file's other cells, while the face said "set".
    let mut m = crate::machine::read()?;
    m.auto_lock = on;
    m.auto_lock_secs = secs;
    crate::machine::write(&m)?;
    shell.machine = crate::machine::read()?;
    Ok((shell.machine.auto_lock, shell.machine.auto_lock_secs))
}

/// Write a whole-machine backup (after the passcode gate): the password at least eight characters and typed
/// twice alike, the folder absolute; the package is collected, sealed, written and read back in the background.
pub(super) fn export_backup(shell: &mut Shell, password: crate::secret::Secret, again: &crate::secret::Secret, dir: &str) -> Result<Spawned, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // The same refusal as a key file's (see `key_file_checks`): the backup is never sealed under the head of a
    // password longer than the secret block.
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

/// Write the appearance to machine settings. A name outside the closed table is refused by name and the file
/// is untouched; unreadable settings are not overwritten.
pub(super) fn set_appearance(shell: &mut Shell, appearance: &str) -> Result<String, crate::fault::Fault> {
    let mut m = crate::machine::read()?;
    m.appearance = Some(appearance.to_string());
    crate::machine::write(&m)?;
    shell.machine = crate::machine::read()?;
    Ok(shell.machine.appearance.clone().unwrap_or_default())
}

pub(super) fn reveal_words(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // One passcode check, one reveal. The passcode question is taken by the gate in `apply`
    // (`Action::pin_asked`): these three actions go through the same gate and the same record.
    let seat = shell.settings.role;
    // "Which row is current" is answered only by `identity::now_row` (the backup path asks the same).
    let view = crate::register::view(seat)?;
    let id = crate::identity::now_row(&view)
        .map(|(r, _)| r.id)
        .ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    shell.words = Some(crate::identity::words_of(&view, &id)?);
    Ok(())
}

/// Name. Changes only the register's weightless text cell.
pub(super) fn name_identity(shell: &mut Shell, id: &str, label: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    let seat = shell.settings.role;
    let row = crate::register::change(seat, |reg| crate::identity::rename(reg, id, label))?;
    shell.seat_identities(Some(crate::register::view(seat)?));
    Ok(row)
}

/// The cells of a key file to write, judged before anything is written: the password twice alike, at least
/// the minimum, and a landing folder (`home::landing`). Answers the folder as it will be written to. Exporting
/// a key file and importing a key that becomes primary ask this one place.
pub(super) fn key_file_checks(password: &crate::secret::Secret, again: &crate::secret::Secret, dir: &str) -> Result<String, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // A password longer than the secret block keeps only its head: the key file would be sealed under a
    // password the person never typed in full (and another wallet given the whole one would not open it).
    if password.overflowed() || again.overflowed() {
        return Err(Fault::known(Known::PasswordLong, crate::secret::CAP.to_string()));
    }
    if password != again {
        return Err(Fault::known(Known::PasswordsDiffer, String::new()));
    }
    // The minimum counts characters only (the strength reading only reminds; there is no weak password gate;
    // see `strength`).
    if password.chars() < crate::backup::PASSWORD_MIN {
        return Err(Fault::known(Known::PasswordShort, password.chars().to_string()));
    }
    // An empty or relative landing place is refused before going to the background and writing the key file
    // (`home::landing`).
    Ok(crate::home::landing(dir)?.display().to_string())
}

/// Write `secret` as a key file under `password` into `dir` and read it back (`landed_check`). Answers where
/// it landed and the address it carries. Runs in a background pass (the scrypt standard level). Exporting a
/// key file and importing a key that becomes primary write through this one place.
pub(super) fn write_key_file(secret: &crate::key::Secret, password: &crate::secret::Secret, dir: &str) -> Result<(String, Address), crate::fault::Fault> {
    let ks = crate::keystore::encrypt(secret, password.expose(), Params::standard(), now_secs())?;
    std::fs::create_dir_all(dir).map_err(|e| crate::fault::classify(&e, dir))?;
    let path = std::path::Path::new(dir).join(&ks.file_name);
    // The key file is readable only by its owner: it is ciphertext, but another account on the same
    // machine reading it byte for byte could brute-force its password offline. The same depth as the
    // vault file's 0600; what is handed to the counterpart still follows the environment.
    zikaron_glue::landing::land_bytes_for(zikaron_glue::landing::Readers::Owner, &path, &ks.json)
        .map_err(|t| crate::fault::Fault::of_landing(t))?;
    // Read it back after writing and compare once.
    //
    // Inferring "backed up" from "the landing step returned no error" fails when the disk is full or
    // read-only, the landing place was swapped, or the written bytes do not match the ones in hand; if
    // any of those returns no error, the register gets a false flag, and a person who deletes the
    // identity relying on it loses the key forever (the victim: an author who pressed backup and then
    // deleted the identity). So the owner of "backed up" is the file on disk, not the previous step's
    // exit code: unreadable, wrong shape or wrong address is refused by name as `BACKUP_NOT_LANDED`, and
    // this pass does not count as a backup.
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
    // The scrypt standard level runs in the background: the key is taken from the key vault in the frame and
    // moved into this pass; the frame keeps running and the card says it is encrypting. After landing, the
    // "backed up" record is written when the shell receives the message.
    Ok(shell.tasks.spawn(Kind::Keystore, move || {
        let (path, address) = write_key_file(&secret, &password, &dir)?;
        Ok(Done::Keystore(crate::task::Keystore::BackedUp { path, address, id, seat }))
    }))
}

/// Set that test hook once (tests only). Setting again always returns false, never silently replacing it.
pub fn set_landed_tamper(f: fn(&std::path::Path)) -> bool {
    LANDED_TAMPER.set(f).is_ok()
}

/// Whether the key file really landed on disk. Each form named: unreadable, not keystore V3 shape, address
/// not this key's.
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
