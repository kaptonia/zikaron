use super::*;

pub(super) fn confirm_identity(shell: &mut Shell, answers: &[(usize, crate::secret::Secret)], label: &str, network: &str) -> Result<Applied, crate::fault::Fault> {
    let Some(f) = shell.new_words.as_ref() else {
        return Err(crate::fault::Fault::known(crate::fault::Known::PhraseConfirm, String::new()));
    };
    crate::identity::confirm(f, answers)?;
    let network = network_choice(network)?;
    shell.new_label = Some(label.trim().to_string());
    let before = registered()?;
    let was = current_seat()?;
    let f = f.twin();
    let label = label.trim().to_string();
    Ok(vault(shell, move || {
        let row = crate::register::change(|reg| {
            let row = crate::identity::add_words(reg, &f, true, &label)?;
            crate::identity::choose_network(reg, &row.id, &network)
        })?;
        let restored = before.contains(&row.id);
        Ok(crate::task::Vault::Identity { row, restored, fresh: true, was })
    }))
}

fn network_choice(name: &str) -> Result<String, crate::fault::Fault> {
    if !crate::deploy::is_choice(name) {
        return Err(crate::fault::Fault::known(crate::fault::Known::IdentitiesShape, name.to_string()));
    }
    Ok(name.to_string())
}

pub(super) fn identity_landed(shell: &mut Shell, row: crate::identity::Row, restored: bool, fresh: bool, was: Option<(String, crate::roles::Role)>) -> Applied {
    if fresh {
        shell.new_words = None;
    }
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
    let mut row = row;
    if let Some(label) = shell.new_label.take().filter(|l| !l.trim().is_empty()) {
        match crate::register::change(|reg| crate::identity::rename(reg, &row.id, &label)) {
            Ok(fresh_row) => row = fresh_row,
            Err(f) => shell.faults.push(f),
        }
    }
    let landed = row.seat_here_or_first(shell.settings.role);
    match enter(shell, &row, landed, Back::To(was)) {
        Ok(()) => {
            if mark_failed && shell.unfetched.is_none() {
                shell.unfetched = Some(crate::restorex::State::Unfetched);
            }
            tail_if_due(shell);
            Applied::IdentityMade { id: row.id.clone(), seats: seats_of(&row), restored }
        }
        Err(f) => shell.trouble(f),
    }
}

pub(super) fn seats_of(row: &crate::identity::Row) -> Vec<(crate::roles::Role, Address)> {
    row.seats().into_iter().filter_map(|s| row.address(s).map(|a| (s, a))).collect()
}

fn current_seat() -> Result<Option<(String, crate::roles::Role)>, crate::fault::Fault> {
    Ok(crate::register::read()?.and_then(|r| r.current))
}

pub(super) fn registered() -> Result<Vec<String>, crate::fault::Fault> {
    Ok(crate::register::read()?.map(|r| r.rows.into_iter().map(|x| x.id).collect()).unwrap_or_default())
}

pub(super) fn import_identity(shell: &mut Shell, form: ImportForm, seat: crate::roles::Role, label: &str, network: &str) -> Result<Applied, crate::fault::Fault> {
    if let ImportForm::PrivateKey { keyfile: Some(out), .. } = &form {
        key_file_checks(&out.password, &out.again, &out.dir)?;
    }
    let (network, book) = if network == crate::deploy::UNDECIDED {
        (None, crate::machine::pick(&shell.machine))
    } else {
        let n = network_choice(network)?;
        (Some(n.clone()), n)
    };
    shell.new_label = Some(label.trim().to_string());
    let before = registered()?;
    let was = current_seat()?;
    let label = label.trim().to_string();
    Ok(vault(shell, move || {
        let row = imported_on(form, seat, &book, network.as_deref(), &label)?;
        let restored = before.contains(&row.id);
        Ok(crate::task::Vault::Identity { row, restored, fresh: false, was })
    }))
}

fn imported_on(form: ImportForm, seat: crate::roles::Role, book: &str, network: Option<&str>, label: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    let chosen = |reg: &mut crate::identity::Registry, row: crate::identity::Row| match network {
        Some(n) => crate::identity::choose_network(reg, &row.id, n),
        None => Ok(row),
    };
    use crate::fault::{Fault, Known};
    let row = match form {
        ImportForm::Words(text) => {
            let f = crate::identity::from_words(text.expose())?;
            let roots = crate::identity::derived_pair(&f).map(|(a, g)| book_roots(book, [a, g])).unwrap_or([None, None]);
            crate::register::change(|reg| {
                let row = crate::identity::add_words_rooted(reg, &f, true, label, roots)?;
                chosen(reg, row)
            })?
        }
        ImportForm::PrivateKey { key, keyfile } => {
            let s = crate::key::from_hex(key.expose()).ok_or_else(|| Fault::known(Known::KeyMalformed, String::new()))?;
            let root = book_root(book, &s);
            let becomes_primary = crate::keybox::primary()?.is_none();
            if let (Some(_), Some(addr)) = (&keyfile, root.or(s.address())) {
                let reg = crate::register::view()?;
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
            crate::register::change(|reg| {
                let row = crate::identity::add_existing_rooted(reg, &s, seat, landed.is_some(), label, root)?;
                if let Some(at) = landed.as_deref() {
                    crate::identity::mark(reg, &row.id, false, true, Some(at))?;
                }
                chosen(reg, row)
            })?
        }
        ImportForm::Keystore { path, password } => {
            let p = path.trim();
            let bytes = std::fs::read(p).map_err(|e| crate::fault::classify(&e, p))?;
            let s = crate::keystore::decrypt_typed(&bytes, &password)?;
            let root = book_root(book, &s);
            crate::register::change(|reg| {
                let row = crate::identity::add_existing_rooted(reg, &s, seat, true, label, root)?;
                chosen(reg, row)
            })?
        }
    };
    Ok(row)
}

fn book_root(name: &str, s: &crate::key::Secret) -> Option<Address> {
    let [root] = book_roots(name, [s.address()?]);
    root
}

fn book_roots<const N: usize>(name: &str, keys: [Address; N]) -> [Option<Address>; N] {
    let Some(d) = crate::deploy::named(name) else { return [None; N] };
    let mine: Vec<crate::chainx::Endpoint> = crate::home::machine_dir().ok().and_then(|m| crate::readnets::read(&m).ok()).unwrap_or_default().iter().filter(|n| n.chain_id == d.chain_id).flat_map(|n| n.endpoints()).collect();
    let eps: Vec<crate::chainx::Endpoint> = if mine.is_empty() { d.endpoint_specs().iter().filter_map(|s| crate::chainx::Endpoint::parse(s)).collect() } else { mine };
    crate::chainx::speak();
    let Ok(zikaron_send::book::State::At(at)) = zikaron_send::book::state(&eps, d.chain_id) else { return [None; N] };
    keys.map(|k| zikaron_send::book::address(&eps, d.chain_id, &at, zikaron_anchor::book::Call::Root, &k.0).ok().map(Address).filter(|r| *r != k))
}

pub(super) fn delete_identity(shell: &mut Shell, id: &str) -> Result<String, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let reg = crate::register::view()?;
    let row = reg.find(id).cloned().ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let handed = !row.backed() && {
        let mut items: Vec<Vec<u8>> = Vec::new();
        for s in row.seats() {
            let Some(home) = row.home(s) else { continue };
            let dir = home.join(crate::home::Slot::Ledger.as_str());
            if dir.is_dir() {
                items.extend(crate::local::Ledger::open(&dir)?.survey()?.items);
            }
        }
        row.seats()
            .into_iter()
            .filter_map(|s| row.current(s))
            .any(|a| crate::succeedx::handed_over(&items, Some(a)).is_some())
    };
    let stood = shell.identities.as_ref().and_then(crate::identity::now_row).is_some_and(|(r, _)| r.id.eq_ignore_ascii_case(id));
    let (gone, next) = crate::register::change(|reg| crate::identity::delete(reg, id, handed))?;
    match next {
        Some((r, s)) if stood => {
            if let Err(f) = enter(shell, &r, s, Back::Gone) {
                shell.faults.push(f);
            }
        }
        Some((r, s)) => enter(shell, &r, s, Back::Was)?,
        None => {
            shell.refresh_anchor()?;
            shell.seat_identities(Some(crate::register::view()?));
        }
    }
    Ok(gone.id)
}

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

pub(super) fn same_twice(pin: &crate::secret::Secret, again: &crate::secret::Secret) -> Result<(), crate::fault::Fault> {
    if pin != again {
        return Err(crate::fault::Fault::known(crate::fault::Known::PasswordsDiffer, String::new()));
    }
    Ok(())
}

pub(super) fn choose_network(shell: &mut Shell, name: &str) -> Result<bool, crate::fault::Fault> {
    shell.machine = crate::machine::update(|m| m.network = Some(name.to_string()))?;
    let recorded = crate::register::now_row_listed().and_then(|now| match now {
        Some((row, _)) => crate::register::change_listed(|reg| crate::identity::set_network(reg, &row.id, name))
            .and_then(|_| crate::register::view())
            .map(|v| shell.seat_identities(Some(v))),
        None => Ok(()),
    });
    if let Err(f) = recorded {
        shell.trouble(f);
    }
    let s = &shell.settings;
    let bare = s.chain_id.is_none() && s.registry.is_none() && s.endpoints.is_empty();
    let from_a_choice = bare || s.network.is_some();
    if shell.home.is_none() || !shell.settings_writable() || !from_a_choice {
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
    shell.machine = crate::machine::update(|m| {
        m.auto_lock = on;
        m.auto_lock_secs = secs;
    })?;
    Ok((shell.machine.auto_lock, shell.machine.auto_lock_secs))
}

pub(super) fn export_backup(shell: &mut Shell, password: crate::secret::Secret, again: &crate::secret::Secret, dir: &str) -> Result<Spawned, crate::fault::Fault> {
    crate::backup::password_ok(&password, again)?;
    let dir = crate::home::landing(dir)?;
    let now = (shell.clock)();
    Ok(shell.tasks.spawn(Kind::Backup, move || {
        let made = (|| {
            std::fs::create_dir_all(&dir).map_err(|e| crate::fault::classify(&e, &dir.display().to_string()))?;
            crate::backup::export(&dir, password.expose(), now)
        })();
        match made {
            Ok(done) => Ok(Done::BackupMade { path: done.path.display().to_string(), summary: done.summary }),
            Err(f) => {
                let _ = crate::machine::update(|m| m.backup_failed = Some(now));
                Err(f)
            }
        }
    }))
}

pub(super) fn set_appearance(shell: &mut Shell, appearance: &str) -> Result<String, crate::fault::Fault> {
    shell.machine = crate::machine::update(|m| m.appearance = Some(appearance.to_string()))?;
    Ok(shell.machine.appearance.clone().unwrap_or_default())
}

pub(super) fn set_proxy(shell: &mut Shell, choice: &str) -> Result<String, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let c = choice.trim();
    let shown_typed = zikaron_net::proxy_text_shown(c).0;
    let written = match c {
        crate::machine::proxy::SYSTEM | crate::machine::proxy::NONE => c.to_string(),
        typed => match zikaron_net::proxy_of(typed) {
            Ok(mut p) => {
                let before = shell.machine.proxy.as_deref().and_then(|w| zikaron_net::proxy_of(w).ok()).filter(|b| crate::proxycells::password_stays(b, &p));
                if let (Some(a), Some(b)) = (p.auth.as_mut(), before.and_then(|b| b.auth)) {
                    a.pass = b.pass;
                }
                p.written()
            }
            Err(zikaron_net::NotAProxy::Credentials) => return Err(Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailProxyCredentials, &[&shown_typed])))),
            Err(zikaron_net::NotAProxy::Shape) => return Err(Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailProxyShape, &[&shown_typed])))),
        },
    };
    shell.machine = crate::machine::update(|m| m.proxy = Some(written))?;
    Ok(shell.machine.proxy.as_deref().map(|w| zikaron_net::proxy_text_shown(w).0).unwrap_or_default())
}

pub(super) fn reveal_words(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let view = crate::register::view()?;
    let id = crate::identity::now_row(&view)
        .map(|(r, _)| r.id)
        .ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    shell.words = Some(crate::identity::words_of(&view, &id)?);
    Ok(())
}

pub(super) fn name_identity(shell: &mut Shell, id: &str, label: &str) -> Result<crate::identity::Row, crate::fault::Fault> {
    let row = crate::register::change(|reg| crate::identity::rename(reg, id, label))?;
    shell.seat_identities(Some(crate::register::view()?));
    Ok(row)
}

pub(super) fn key_file_checks(password: &crate::secret::Secret, again: &crate::secret::Secret, dir: &str) -> Result<String, crate::fault::Fault> {
    crate::backup::password_ok(password, again)?;
    Ok(crate::home::landing(dir)?.display().to_string())
}

pub(super) fn write_key_file(secret: &crate::key::Secret, password: &crate::secret::Secret, dir: &str) -> Result<(String, Address), crate::fault::Fault> {
    let ks = crate::keystore::encrypt(secret, password.expose(), Params::standard(), now_secs())?;
    std::fs::create_dir_all(dir).map_err(|e| crate::fault::classify(&e, dir))?;
    let path = std::path::Path::new(dir).join(&ks.file_name);
    zikaron_glue::landing::land_bytes_for(zikaron_glue::landing::Readers::Owner, &path, &ks.json)
        .map_err(|t| crate::fault::Fault::of_landing(t))?;
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
    let view = crate::register::view()?;
    let acct = crate::identity::account_now(&view)
        .ok_or_else(|| Fault::known(Known::SeatUnseated, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::IdSeatEmpty).to_string())))?;
    let secret = crate::key::load(Some(&acct))?.ok_or_else(|| Fault::known(Known::KeychainMissing, acct.clone()))?;
    let id = crate::identity::now_row(&view).map(|(r, _)| r.id);
    Ok(shell.tasks.spawn(Kind::Keystore, move || {
        let (path, address) = write_key_file(&secret, &password, &dir)?;
        Ok(Done::Keystore(crate::task::Keystore::BackedUp { path, address, id, seat }))
    }))
}

pub fn set_landed_tamper(f: fn(&std::path::Path)) -> bool {
    LANDED_TAMPER.set(f).is_ok()
}

pub(super) fn landed_check(path: &std::path::Path, want: Address) -> Result<(), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let at = path.display().to_string();
    let back = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &at))?;
    let shape = crate::keystore::shape(&back)
        .map_err(|f| Fault::known(Known::BackupNotLanded, crate::lang::both_of(|| format!("{at}: {}", f.said()))))?;
    let bad = shape.compliant();
    if !bad.is_empty() {
        return Err(Fault::known(Known::BackupNotLanded, format!("{at}: {}", bad.join(" "))));
    }
    if shape.address != Some(want) {
        return Err(Fault::known(Known::BackupNotLanded, format!("{at}: {} ≠ {}", shape.address_text, want.hex())));
    }
    Ok(())
}
