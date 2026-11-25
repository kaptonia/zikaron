use crate::codes::{Key, Reason};
use crate::out::{self, s, Answer, Said};
use std::path::{Path, PathBuf};
use zikaron_seal::envelope::{self as env, Doc, Expect, Ident, Owner};
use zikaron_seal::keybox::{self, LocalKey, Vault};
use zikaron_seal::names::NameKey;
use zikaron_seal::{Code, Trouble};

pub fn ledger_given(given: &str) -> String {
    let Ok(at) = std::path::absolute(Path::new(given)) else { return given.to_string() };
    if env::label_path(&at).is_file() && !env::label_path(zikaron_os::parent_dir(&at)).is_file() {
        return at.join(env::LEDGER_DIR).display().to_string();
    }
    given.to_string()
}

pub fn is_home(ledger: &str) -> bool {
    env::label_path(&home_of(ledger)).is_file()
}

pub struct Opened {
    home: PathBuf,
    machine: PathBuf,
    account: String,
    ledger: zikaron_seal::ledger::Ledger,
    local: LocalKey,
    names: NameKey,
    number: String,
    pub key: [u8; 32],
    pub author: String,
    pub items: Vec<Vec<u8>>,
    lock: Option<zikaron_os::LockFile>,
}

pub const TAKE_BACK_WAIT: std::time::Duration = std::time::Duration::from_secs(60);
pub const TAKE_BACK_STEP: std::time::Duration = std::time::Duration::from_millis(100);

pub fn refused(t: &Trouble) -> Answer {
    if let Some(a) = crate::named::credential(t) {
        return a;
    }
    let members = vec![(Key::Token, s(&word(t.code))), (Key::Detail, s(&t.tail))];
    match t.code {
        Code::HomeHeld => out::denied(Reason::HomeHeld, members).telling(Said::HomeHeld.text().to_string()),
        Code::HomeOtherMachine => out::denied(Reason::HomeOtherMachine, members).telling(Said::HomeOtherMachine.text().to_string()),
        Code::HandedOff => handed_off(&t.tail),
        _ => out::denied(Reason::Unreadable, members),
    }
}

pub fn handed_off(handoff: &str) -> Answer {
    crate::named::of(zikaron_glue::door::Own::HandedOff, "", handoff, false, "")
}

pub fn word(c: Code) -> String {
    let mut out = String::new();
    for (i, ch) in format!("{c:?}").chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

struct Unlocked {
    machine: PathBuf,
    account: String,
    vault: Vault,
    local: LocalKey,
    names: NameKey,
}

fn unlock(now: u64) -> Result<Unlocked, Trouble> {
    let machine = zikaron_os::machine::here().map(|(m, _)| m).map_err(|_| Trouble::new(Code::CredentialMissing, String::new()))?;
    let account = zikaron_seal::credential::find(&machine)?;
    let pass = std::env::var_os(zikaron_seal::credential::PASS_ENV).map(|v| v.into_string().unwrap_or_else(|_| String::from("\u{0}")));
    let mk = zikaron_seal::credential::open(&machine, &account, now, pass.as_deref())?;
    drop(pass);
    let mk: [u8; 32] = mk.bytes().try_into().map_err(|_| Trouble::new(Code::CredentialShape, account.clone()))?;
    keybox::hold_master(&mk);
    let vault = Vault::new(machine.clone(), zikaron_seal::places::keybox_file_for(&account), zikaron_seal::places::keybox_lock_file_for(&account));
    if !keybox::master_is_this(&vault)? {
        zikaron_seal::credential::remove(&machine, &account)?;
        return Err(Trouble::new(Code::CredentialShape, format!("{} master", zikaron_seal::credential::path(&machine, &account).display())));
    }
    let local = keybox::local_key()?;
    let names = keybox::name_key()?;
    zikaron_anchor::action::keep_in(std::sync::Arc::new(zikaron_send::checked::Logs(zikaron_send::checked::Sealed(machine.clone()))));
    Ok(Unlocked { machine, account, vault, local, names })
}

pub fn machine_here() -> Result<PathBuf, Trouble> {
    let (m, _) = zikaron_os::machine::here().map_err(|_| Trouble::new(Code::MachineShape, String::new()))?;
    std::fs::create_dir_all(&m).map_err(|e| Trouble::io(&e, &m.display().to_string()))?;
    Ok(m)
}

fn home_of(ledger: &str) -> PathBuf {
    zikaron_os::parent_dir(Path::new(ledger)).to_path_buf()
}

pub fn open(ledger: &str, now: u64) -> Result<Opened, Trouble> {
    crate::seam();
    let ledger_dir = PathBuf::from(ledger);
    let home = home_of(ledger);
    let Unlocked { machine, account, vault, local, names } = unlock(now)?;
    let lock = take_lock(&home, &machine)?;
    if let Some(u) = zikaron_os::user_home() {
        let _ = zikaron_os::machine::settle_pointer(&u, &machine);
    }
    let label = env::read_label(&home, &local)?.ok_or_else(|| Trouble::new(Code::LocalSeal, env::label_path(&home).display().to_string()))?;
    let author = office_of(&home, &machine, &account, &local, &names, &label)?;
    let key = key_in(&vault, &account, &author)?.ok_or_else(|| Trouble::new(Code::KeyboxSlot, author.clone()))?;
    let ledger = zikaron_seal::ledger::Ledger::open(&ledger_dir)?;
    let items = ledger.pile(&keys(&local, &names, &label.number))?.items;
    Ok(Opened { home, machine, account, ledger, local, names, number: label.number, key, author, items, lock: Some(lock) })
}

fn office_of(home: &Path, machine: &Path, account: &str, local: &LocalKey, names: &NameKey, label: &env::Label) -> Result<String, Trouble> {
    let book = register_of(machine, account, local, names)?;
    let (identity, seat) = match (label.whose.clone(), book.current.clone()) {
        (Some(w), _) => w,
        (None, Some((id, seat))) => (id, seat.to_string()),
        (None, None) => return Err(Trouble::new(Code::IdentitiesShape, env::label_path(home).display().to_string())),
    };
    let line = book.lines.iter().find(|l| l.id == identity).ok_or_else(|| Trouble::new(Code::IdentitiesShape, identity.clone()))?;
    let address = line.office(&seat).map(|o| o.current).ok_or_else(|| Trouble::new(Code::IdentitiesShape, format!("{identity} {seat}")))?;
    Ok(zikaron::hexfmt::encode(&address))
}

fn last_anchored_at(local: &LocalKey, names: &NameKey, home: &Path, number: &str) -> Result<Option<zikaron_seal::lastaudit::Anchored>, Trouble> {
    let rel = format!("{}/{}", env::SETTINGS_DIR, zikaron_seal::lastaudit::FILE);
    match sealed_at(local, names, home, &rel, Doc::LastAudit, Owner::Home(number.to_string()))? {
        None => Ok(None),
        Some(plain) => zikaron_seal::lastaudit::parse(&plain).map(Some).map_err(|w| Trouble::new(Code::SettingsShape, format!("{rel}: {w}"))),
    }
}

fn terms_records_at(local: &LocalKey, names: &NameKey, home: &Path, number: &str) -> Result<Vec<zikaron_seal::terms::Record>, Trouble> {
    use zikaron_seal::terms::{RECORD_PREFIX, RECORD_SUFFIX};
    let room = format!("{}/{}", env::KITS_DIR, zikaron_glue::grantfile::TERMS_ROOM);
    let listing = match std::fs::read_dir(home.join(&room)) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Trouble::io(&e, &home.join(&room).display().to_string())),
    };
    let mut out = Vec::new();
    for e in listing.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.starts_with(RECORD_PREFIX) && name.ends_with(RECORD_SUFFIX)) {
            continue;
        }
        let rel = format!("{room}/{name}");
        let at = home.join(&rel);
        let raw = std::fs::read(&at).map_err(|e| Trouble::io(&e, &at.display().to_string()))?;
        let ex = Expect { owner: Some(Owner::Home(number.to_string())), doc: Doc::TermsRecord, rel: Some(rel.clone()) };
        if let Some(r) = zikaron_seal::terms::record_of(&env::open_with(local, &ex, &raw, &rel, Some(names))?) {
            out.push(r);
        }
    }
    out.sort_by(|a, b| a.grant.cmp(&b.grant));
    Ok(out)
}

fn kept_ledgers_at(local: &LocalKey, names: &NameKey, home: &Path, number: &str) -> Result<Vec<Vec<Vec<u8>>>, Trouble> {
    let room = format!("{}/{}", env::GRANTS_HELD_DIR, env::KEPT_DIR);
    let listing = match std::fs::read_dir(home.join(&room)) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Trouble::io(&e, &home.join(&room).display().to_string())),
    };
    let mut paths: Vec<String> = listing.filter_map(Result::ok).filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    paths.sort();
    let mut out = Vec::new();
    for name in paths {
        let rel = format!("{room}/{name}");
        if let Some(plain) = sealed_at(local, names, home, &rel, Doc::KeptGrant, Owner::Home(number.to_string()))? {
            if let Ok(o) = zikaron_glue::grantfile::open(&plain) {
                out.push(o.ledger);
            }
        }
    }
    Ok(out)
}

pub struct Reading {
    home: PathBuf,
    machine: PathBuf,
    account: String,
    local: LocalKey,
    names: NameKey,
    number: String,
    pub items: Vec<Vec<u8>>,
    pub own: Option<String>,
}

pub fn read(ledger: &str, now: u64) -> Result<Reading, Trouble> {
    crate::seam();
    let home = home_of(ledger);
    let Unlocked { machine, account, local, names, .. } = unlock(now)?;
    this_machines(&home, &machine)?;
    let label = env::read_label(&home, &local)?.ok_or_else(|| Trouble::new(Code::LocalSeal, env::label_path(&home).display().to_string()))?;
    let items = zikaron_seal::ledger::Ledger::open(Path::new(ledger))?.pile(&keys(&local, &names, &label.number))?.items;
    let own = office_of(&home, &machine, &account, &local, &names, &label).ok();
    Ok(Reading { home, machine, account, local, names, number: label.number, items, own })
}

pub(crate) fn this_machines(home: &Path, machine: &Path) -> Result<(), Trouble> {
    match zikaron_seal::writer::mark_read(home) {
        zikaron_seal::writer::Mark::None => Ok(()),
        zikaron_seal::writer::Mark::Machine(m) if m == zikaron_seal::writer::this_machine(machine)? => Ok(()),
        zikaron_seal::writer::Mark::Machine(_) => Err(Trouble::new(Code::HomeOtherMachine, zikaron_seal::writer::mark_path(home).display().to_string())),
        zikaron_seal::writer::Mark::Unread(u) => Err(Trouble::new(Code::HomeOtherMachine, u.tail)),
    }
}

fn register_of(machine: &Path, account: &str, local: &LocalKey, names: &NameKey) -> Result<zikaron_seal::identities::Book, Trouble> {
    let register_name = zikaron_seal::places::registry_file_for(account);
    let register_at = machine.join(&register_name);
    let raw = std::fs::read(&register_at).map_err(|e| Trouble::io(&e, &register_at.display().to_string()))?;
    let ex = Expect { owner: Some(Owner::Machine), doc: Doc::Registry, rel: Some(register_name.clone()) };
    let plain = env::open_with(local, &ex, &raw, &register_name, Some(names))?;
    zikaron_seal::identities::Book::parse(&plain).map_err(|w| Trouble::new(Code::IdentitiesShape, w))
}

fn key_in(vault: &Vault, account: &str, author: &str) -> Result<Option<[u8; 32]>, Trouble> {
    let Some(mut secret) = keybox::get(vault, &zikaron_seal::places::key_slot_for(account, author))? else { return Ok(None) };
    let key: Result<[u8; 32], _> = secret.as_slice().try_into();
    zikaron_os::wipe(&mut secret);
    let key = key.map_err(|_| Trouble::new(Code::KeyMalformed, author.to_string()))?;
    if crate::entry::address(&key).as_deref() != Some(author) {
        return Err(Trouble::new(Code::KeyMalformed, author.to_string()));
    }
    Ok(Some(key))
}

pub fn identity_keys(ledger: &str, now: u64) -> Result<(Vec<[u8; 32]>, Option<Trouble>), Trouble> {
    crate::seam();
    let home = home_of(ledger);
    let Unlocked { machine, account, vault, local, names } = unlock(now)?;
    this_machines(&home, &machine)?;
    let book = register_of(&machine, &account, &local, &names)?;
    let (mut keys, mut passed) = (Vec::new(), None);
    for line in &book.lines {
        for seat in env::SEATS {
            let (Some(root), Some(office)) = (line.address(seat), line.office(seat)) else { continue };
            let mut held: Vec<[u8; 20]> = vec![root];
            for a in std::iter::once(office.current).chain(office.former.iter().copied()) {
                if !held.contains(&a) {
                    held.push(a);
                }
            }
            for address in held {
                match key_in(&vault, &account, &zikaron::hexfmt::encode(&address)) {
                    Ok(Some(k)) => {
                        keys.push(k);
                    }
                    Ok(None) => {}
                    Err(t) if t.code.one_slot() => {
                        passed.get_or_insert(t);
                    }
                    Err(t) => return Err(t),
                }
            }
        }
    }
    Ok((keys, passed))
}

fn sealed_at(local: &LocalKey, names: &NameKey, dir: &Path, rel: &str, doc: Doc, owner: Owner) -> Result<Option<Vec<u8>>, Trouble> {
    let at = dir.join(rel);
    let raw = match std::fs::read(&at) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Trouble::io(&e, &at.display().to_string())),
    };
    let ex = Expect { owner: Some(owner), doc, rel: Some(rel.to_string()) };
    env::open_with(local, &ex, &raw, rel, Some(names)).map(Some)
}

fn hold_rest_keys(g: &zikaron_seal::ground::Ground, read: impl FnOnce() -> Result<zikaron_seal::machine::Cli, Trouble>) -> Result<(), Trouble> {
    if !g.rest_asked().is_empty() {
        zikaron_anchor::rest::hold_keys(read()?.rest_keys.0);
    }
    Ok(())
}

fn ground_of(local: &LocalKey, names: &NameKey, home: &Path, number: &str) -> Result<zikaron_seal::ground::Ground, Trouble> {
    let rel = format!("{}/{}", env::SETTINGS_DIR, zikaron_seal::ground::SETTINGS_FILE);
    match sealed_at(local, names, home, &rel, Doc::Settings, Owner::Home(number.to_string()))? {
        None => Ok(zikaron_seal::ground::Ground::default()),
        Some(plain) => {
            let v = zikaron::json::parse(&plain).map_err(|t| Trouble::new(Code::SettingsShape, format!("{rel}: {t:?}")))?;
            let g = zikaron_seal::ground::of_settings(&v);
            zikaron_anchor::action::take_asks_cap(g.log_asks.unwrap_or(zikaron_anchor::action::ASKS_DEFAULT));
            Ok(g)
        }
    }
}

impl Reading {
    fn sealed(&self, dir: &Path, rel: &str, doc: Doc, owner: Owner) -> Result<Option<Vec<u8>>, Trouble> {
        sealed_at(&self.local, &self.names, dir, rel, doc, owner)
    }

    pub fn ground(&self) -> Result<zikaron_seal::ground::Ground, Trouble> {
        let g = ground_of(&self.local, &self.names, &self.home, &self.number)?;
        hold_rest_keys(&g, || self.machine_settings())?;
        Ok(g)
    }

    pub fn machine_settings(&self) -> Result<zikaron_seal::machine::Cli, Trouble> {
        zikaron_seal::machine::read(&self.machine, &self.account, &self.local, &self.names)
    }

    pub fn place_mark(&self, s: &zikaron_seal::unfetched::State) -> Result<(), Trouble> {
        zikaron_seal::unfetched::write(&self.home, &self.local, &self.number, s)
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn terms_of(&self, grant: &str) -> Result<Option<(String, Vec<u8>)>, Trouble> {
        let owner = || Owner::Home(self.number.clone());
        let rel = zikaron_seal::terms::record_rel(&self.names, grant);
        let Some(plain) = self.sealed(&self.home, &rel, Doc::TermsRecord, owner())? else { return Ok(None) };
        let Some(record) = zikaron_seal::terms::record_of(&plain) else { return Ok(None) };
        let Some(doc) = record.doc.clone() else { return Ok(None) };
        let rel = zikaron_seal::terms::doc_rel_on_disk(&self.names, &record.terms);
        Ok(self.sealed(&self.home, &rel, Doc::TermsDoc, owner())?.map(|bytes| (doc, bytes)))
    }

    pub fn machine(&self) -> &Path {
        &self.machine
    }

    pub fn checked(&self) -> zikaron_send::checked::Record<zikaron_send::checked::Sealed> {
        zikaron_send::checked::Record(zikaron_send::checked::Sealed(self.machine.clone()))
    }

    pub fn exports_list(&self) -> Result<Vec<zikaron_seal::exports::Row>, Trouble> {
        let rel = format!("{}/{}", zikaron_seal::exports::DIR, zikaron_seal::exports::FILE);
        match self.sealed(&self.machine, &rel, Doc::Exports, Owner::Machine)? {
            None => Ok(Vec::new()),
            Some(plain) => zikaron_seal::exports::parse(&plain).map_err(|w| Trouble::new(Code::SettingsShape, format!("{rel}: {w}"))),
        }
    }

    pub fn write_exports_list(&self, rows: &[zikaron_seal::exports::Row]) -> Result<(), Trouble> {
        let rel = format!("{}/{}", zikaron_seal::exports::DIR, zikaron_seal::exports::FILE);
        let sealed = env::seal_checked(&self.local, &Ident { owner: Owner::Machine, doc: Doc::Exports, logical: rel }, &zikaron_seal::exports::render(rows))?;
        zikaron_seal::land::put_at(&self.machine.join(zikaron_seal::exports::DIR), zikaron_seal::exports::FILE, &sealed)
    }

    pub fn list_export(&self, kind: zikaron_seal::exports::Kind, path: &Path, now: u64) -> Result<(), Trouble> {
        let at = std::fs::canonicalize(path).map_err(|e| Trouble::io(&e, &path.display().to_string()))?;
        let rows = zikaron_seal::exports::listed(&self.exports_list()?, kind, &at, now).ok_or_else(|| Trouble::new(Code::Ledger, at.display().to_string()))?;
        self.write_exports_list(&rows)
    }

    pub fn last_anchored(&self) -> Result<Option<zikaron_seal::lastaudit::Anchored>, Trouble> {
        last_anchored_at(&self.local, &self.names, &self.home, &self.number)
    }

    pub fn kept_ledgers(&self) -> Result<Vec<Vec<Vec<u8>>>, Trouble> {
        kept_ledgers_at(&self.local, &self.names, &self.home, &self.number)
    }

    pub fn held_office(&self, root: &[u8; 20]) -> Result<Option<[u8; 20]>, Trouble> {
        let book = register_of(&self.machine, &self.account, &self.local, &self.names)?;
        let seats: Vec<([u8; 20], [u8; 20])> = book.lines.iter().flat_map(|l| env::SEATS.iter().filter_map(move |seat| Some((l.address(seat)?, l.office(seat)?.current)))).collect();
        let kept = kept_ledgers_at(&self.local, &self.names, &self.home, &self.number)?;
        Ok(zikaron_send::recipient::held(root, &seats, &kept))
    }

    pub fn read_networks(&self) -> Result<Vec<zikaron_seal::readnets::Row>, Trouble> {
        zikaron_seal::readnets::read(&self.machine)
    }

    pub fn held(&self) -> Result<Vec<Vec<u8>>, Trouble> {
        let root = self.home.join(env::GRANTS_HELD_DIR);
        let mut stack = vec![root.clone()];
        let mut out = Vec::new();
        while let Some(at) = stack.pop() {
            let listing = match std::fs::read_dir(&at) {
                Ok(l) => l,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && at == root => return Ok(out),
                Err(e) => return Err(Trouble::io(&e, &at.display().to_string())),
            };
            for e in listing.filter_map(Result::ok) {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                if zikaron_store::layout::parse_entry_file(&name).is_none() {
                    continue;
                }
                let Ok(rel) = p.strip_prefix(&self.home) else { continue };
                let rel = rel.to_string_lossy().replace('\\', "/");
                if let Ok(Some(plain)) = self.sealed(&self.home, &rel, Doc::Held, Owner::Home(self.number.clone())) {
                    out.push(plain);
                }
            }
        }
        Ok(out)
    }

    pub fn records(&self) -> Result<Vec<zikaron_seal::records::Row>, Trouble> {
        let rel = format!("{}/{}", zikaron_seal::records::DIR, zikaron_seal::records::FILE);
        match self.sealed(&self.machine, &rel, Doc::Records, Owner::Machine)? {
            None => Ok(Vec::new()),
            Some(plain) => zikaron_seal::records::parse(&plain).map_err(|w| Trouble::new(Code::SettingsShape, format!("{rel}: {w}"))),
        }
    }
}

fn keys<'a>(local: &'a LocalKey, names: &'a NameKey, number: &str) -> zikaron_seal::ledger::Keys<'a> {
    zikaron_seal::ledger::Keys { local, names, owner: Some(Owner::Home(number.to_string())) }
}

fn take_lock(home: &Path, machine: &Path) -> Result<zikaron_os::LockFile, Trouble> {
    let t = zikaron_seal::writer::take(home, machine)?;
    match t.took {
        zikaron_seal::writer::Took::Writer => Ok(t.held),
        zikaron_seal::writer::Took::Busy { .. } => Err(Trouble::new(Code::HomeHeld, home.display().to_string())),
        zikaron_seal::writer::Took::OtherMachine => Err(Trouble::new(Code::HomeOtherMachine, zikaron_seal::writer::mark_path(home).display().to_string())),
        zikaron_seal::writer::Took::Unread(u) => Err(Trouble::new(Code::HomeOtherMachine, u.tail)),
    }
}

impl Opened {
    pub fn ledger_root(&self) -> String {
        self.ledger.root().to_string_lossy().into_owned()
    }

    pub fn append(&self, id: &str, bytes: &[u8], now: u64) -> Result<bool, Trouble> {
        let name = zikaron_store::layout::EntryName::parse(id.trim_start_matches("0x")).ok_or_else(|| Trouble::new(Code::Ledger, id.to_string()))?;
        let stored = self.ledger.append(&name, bytes, &keys(&self.local, &self.names, &self.number), &self.number)?;
        let written = matches!(stored, zikaron_store::ledger::Stored::Written);
        if written {
            self.queue(id, bytes, now)?;
        }
        Ok(written)
    }

    pub fn index_original(&self, row: zikaron_seal::records::Row) -> Result<(), Trouble> {
        use zikaron_seal::records::{self as records, DIR, FILE};
        let rel = format!("{DIR}/{FILE}");
        let dir = self.machine.join(DIR);
        let mut rows = match std::fs::read(dir.join(FILE)) {
            Ok(raw) => {
                let ex = Expect { owner: Some(Owner::Machine), doc: Doc::Records, rel: Some(rel.clone()) };
                let plain = env::open_with(&self.local, &ex, &raw, &rel, Some(&self.names))?;
                records::parse(&plain).map_err(|w| Trouble::new(Code::SettingsShape, format!("{rel}: {w}")))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(Trouble::io(&e, &dir.join(FILE).display().to_string())),
        };
        rows.retain(|r| r.id != row.id);
        rows.push(row);
        std::fs::create_dir_all(&dir).map_err(|e| Trouble::io(&e, &dir.display().to_string()))?;
        let sealed = env::seal_checked(&self.local, &Ident { owner: Owner::Machine, doc: Doc::Records, logical: rel }, &records::render(&rows))?;
        zikaron_seal::land::put_at(&dir, FILE, &sealed)
    }

    pub fn take(&self, id: &str, bytes: &[u8]) -> Result<bool, Trouble> {
        let name = zikaron_store::layout::EntryName::parse(id.trim_start_matches("0x")).ok_or_else(|| Trouble::new(Code::Ledger, id.to_string()))?;
        let stored = self.ledger.append(&name, bytes, &keys(&self.local, &self.names, &self.number), &self.number)?;
        Ok(matches!(stored, zikaron_store::ledger::Stored::Written))
    }

    fn queue(&self, id: &str, bytes: &[u8], now: u64) -> Result<(), Trouble> {
        let mut q = self.read_queue()?;
        let ground = self.ground()?;
        let place = zikaron_seal::queue::places_of(&self.pile()?).get(&format!("0x{}", id.trim_start_matches("0x").to_ascii_lowercase())).copied().or_else(|| zikaron_glue::book::check(bytes).ok().map(|e| zikaron_seal::queue::Place::of(&e)));
        q.push_placed(id, place.is_some_and(|p| p.alone), place.and_then(|p| p.carried_on), &ground.other_chains(), now);
        self.write_queue(&q)
    }

    pub fn read_queue(&self) -> Result<zikaron_seal::queue::Queue, Trouble> {
        use zikaron_seal::queue::{Queue, FILE};
        let rel = format!("{}/{FILE}", env::SETTINGS_DIR);
        match sealed_at(&self.local, &self.names, &self.home, &rel, Doc::Queue, Owner::Home(self.number.clone()))? {
            Some(plain) => Queue::parse(&plain, self.ground()?.chain_id.unwrap_or(0)).map_err(|w| Trouble::new(Code::QueueShape, format!("{rel}: {w:?}"))),
            None => Ok(Queue { chain: self.ground()?.chain_id.unwrap_or(0), ..Queue::default() }),
        }
    }

    pub fn write_queue(&self, q: &zikaron_seal::queue::Queue) -> Result<(), Trouble> {
        use zikaron_seal::queue::FILE;
        let rel = format!("{}/{FILE}", env::SETTINGS_DIR);
        let sealed = env::seal_checked(&self.local, &Ident { owner: Owner::Home(self.number.clone()), doc: Doc::Queue, logical: rel }, &q.to_bytes())?;
        zikaron_seal::land::put_at(&self.home.join(env::SETTINGS_DIR), FILE, &sealed)
    }

    pub fn retracting(&self, target: &str) -> Result<Option<zikaron_seal::queue::Retracting>, Trouble> {
        let last: Option<Vec<String>> = last_anchored_at(&self.local, &self.names, &self.home, &self.number)?.map(|a| a.rows.into_iter().map(|(id, _)| id).collect());
        Ok(self.read_queue()?.retracting(target, last.as_deref(), self.ground()?.can_anchor()))
    }

    pub fn last_anchored(&self) -> Result<Option<zikaron_seal::lastaudit::Anchored>, Trouble> {
        last_anchored_at(&self.local, &self.names, &self.home, &self.number)
    }

    pub fn terms_records(&self) -> Result<Vec<zikaron_seal::terms::Record>, Trouble> {
        terms_records_at(&self.local, &self.names, &self.home, &self.number)
    }

    pub fn amend_queue<T>(&self, f: impl FnOnce(&mut zikaron_seal::queue::Queue) -> T) -> Result<T, Trouble> {
        let mut q = self.read_queue()?;
        let t = f(&mut q);
        self.write_queue(&q)?;
        Ok(t)
    }

    pub fn pile(&self) -> Result<Vec<Vec<u8>>, Trouble> {
        Ok(self.ledger.pile(&keys(&self.local, &self.names, &self.number))?.items)
    }

    pub fn ground(&self) -> Result<zikaron_seal::ground::Ground, Trouble> {
        let g = ground_of(&self.local, &self.names, &self.home, &self.number)?;
        hold_rest_keys(&g, || self.machine_settings())?;
        Ok(g)
    }

    pub fn machine_settings(&self) -> Result<zikaron_seal::machine::Cli, Trouble> {
        zikaron_seal::machine::read(&self.machine, &self.account, &self.local, &self.names)
    }

    pub fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Trouble> {
        zikaron_seal::unfetched::read(&self.home, &self.local, &self.names, &self.number)
    }

    pub fn place_mark(&self, s: &zikaron_seal::unfetched::State) -> Result<(), Trouble> {
        zikaron_seal::unfetched::write(&self.home, &self.local, &self.number, s)
    }

    pub fn lift_mark(&self) -> Result<(), Trouble> {
        match self.lock {
            Some(_) => zikaron_seal::unfetched::clear(&self.home),
            None => Ok(()),
        }
    }

    pub fn machine(&self) -> &Path {
        &self.machine
    }

    pub fn checked(&self) -> zikaron_send::checked::Record<zikaron_send::checked::Sealed> {
        zikaron_send::checked::Record(zikaron_send::checked::Sealed(self.machine.clone()))
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn let_go(&mut self) {
        self.lock = None;
    }

    pub fn take_back(&mut self) -> Result<(), Trouble> {
        if self.lock.is_some() {
            return Ok(());
        }
        let until = std::time::Instant::now() + TAKE_BACK_WAIT;
        loop {
            match take_lock(&self.home, &self.machine) {
                Ok(l) => {
                    self.lock = Some(l);
                    return Ok(());
                }
                Err(t) if t.code == Code::HomeHeld && std::time::Instant::now() < until => std::thread::sleep(TAKE_BACK_STEP),
                Err(t) => return Err(t),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_ledgers_home_is_the_directory_it_is_in() {
        use std::path::Path;
        assert_eq!(super::home_of("ledger"), Path::new("."));
        assert_eq!(super::home_of("./ledger"), Path::new("."));
        assert_eq!(super::home_of("h/ledger"), Path::new("h"));
        assert_eq!(super::home_of("/abs/h/ledger"), Path::new("/abs/h"));
    }
}
