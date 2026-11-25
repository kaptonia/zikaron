use crate::fault::{classify, Fault, Known};
use crate::home::Home;
use crate::keybox::LocalKey;
use std::path::{Path, PathBuf};
use zikaron_seal::envelope as env;
use zikaron_store::{EntryName, Pile, Stored, Survey};

pub use env::{is_home_number, is_staged, opens_under_key, open_staged, staged_name, unstaged, Doc, Expect, Ident, Owner, Unread, ALG_KEYED, DIGEST, LABEL_FILE, MAGIC, MAGIC_V2, OWNER_DOMAIN, PLACE_DOMAIN, PLACE_KEY_INFO};

pub fn envelope(key: &[u8; 32], id: &Ident, nonce: &[u8; env::NONCE], plain: &[u8]) -> Result<Vec<u8>, Fault> {
    Ok(env::envelope(key, id, nonce, plain)?)
}

pub fn unseal(key: &[u8; 32], ex: &Expect, bytes: &[u8], what: &str) -> Result<Vec<u8>, Fault> {
    let nk = crate::names::key().ok();
    Ok(env::unseal(key, ex, bytes, what, nk.as_ref())?)
}

pub fn is_sealed(bytes: &[u8]) -> bool {
    zikaron_glue::sealed::is_sealed(bytes)
}

pub fn seal_with(key: &LocalKey, id: &Ident, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    Ok(env::seal_with(key, id, plain)?)
}

pub fn open_with(key: &LocalKey, ex: &Expect, bytes: &[u8], what: &str) -> Result<Vec<u8>, Fault> {
    let nk = crate::names::key().ok();
    Ok(env::open_with(key, ex, bytes, what, nk.as_ref())?)
}

fn unstaged_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    PathBuf::from(unstaged(&s).unwrap_or(&s))
}

fn rel_under(path: &Path, root: &Path) -> Option<String> {
    let r = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

pub fn place(path: &Path, doc: Doc) -> Result<(PathBuf, String), Fault> {
    let path = unstaged_path(path);
    if doc.in_machine() {
        let m = crate::home::machine_dir()?;
        let rel = rel_under(&path, &m).unwrap_or_else(|| path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        return Ok((m, rel));
    }
    for root in path.ancestors().skip(1).take(5) {
        if let Some(rel) = rel_under(&path, root) {
            if home_doc(&rel) == Some(doc) {
                return Ok((root.to_path_buf(), rel));
            }
        }
    }
    Err(Unread::Swapped.fault(doc, &path.display().to_string()).into())
}

static LABELS: std::sync::Mutex<Vec<(PathBuf, Vec<u8>, String)>> = std::sync::Mutex::new(Vec::new());

fn label_stamp(root: &Path) -> Option<Vec<u8>> {
    std::fs::read(label_path(root)).ok()
}

fn known_label(root: &Path) -> Option<String> {
    let at = std::fs::canonicalize(root).ok()?;
    let stamp = label_stamp(root)?;
    LABELS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(p, s, _)| *p == at && *s == stamp).map(|(_, _, n)| n.clone())
}

fn remember_label(root: &Path, n: &str) {
    if let (Ok(at), Some(stamp)) = (std::fs::canonicalize(root), label_stamp(root)) {
        let mut l = LABELS.lock().unwrap_or_else(|e| e.into_inner());
        l.retain(|(p, _, _)| *p != at);
        l.push((at, stamp, n.to_string()));
    }
}

pub fn forget_labels() {
    LABELS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub number: String,
    pub whose: Option<(String, crate::roles::Role)>,
}

impl Label {
    pub fn new(whose: Option<(String, crate::roles::Role)>) -> Result<Label, Fault> {
        let l = env::Label::new(None)?;
        Ok(Label { number: l.number, whose })
    }

    fn sealed(&self) -> env::Label {
        env::Label { number: self.number.clone(), whose: self.whose.as_ref().map(|(id, seat)| (id.clone(), seat.as_str().to_string())) }
    }

    fn of(l: env::Label) -> Option<Label> {
        let whose = match l.whose {
            Some((id, seat)) => Some((id, crate::roles::Role::ALL.into_iter().find(|r| r.as_str() == seat)?)),
            None => None,
        };
        Some(Label { number: l.number, whose })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.sealed().to_bytes()
    }

    pub fn parse(b: &[u8]) -> Option<Label> {
        Label::of(env::Label::parse(b)?)
    }
}

pub use env::{label_ident, label_path};

fn label_rel() -> String {
    env::label_rel()
}

pub fn read_label(root: &Path) -> Result<Option<Label>, Fault> {
    let key = crate::keybox::local_key()?;
    let Some(l) = env::read_label(root, &key)? else { return Ok(None) };
    let at = label_path(root);
    let l = Label::of(l).ok_or_else(|| Fault::known(Known::SettingsShape, format!("{} {}", Doc::HomeLabel.tag(), at.display())))?;
    remember_label(root, &l.number);
    Ok(Some(l))
}

fn whose_by_register(root: &Path) -> Option<(String, crate::roles::Role)> {
    let reg = crate::register::read().ok().flatten()?;
    for row in &reg.rows {
        for seat in crate::roles::Role::ALL {
            if row.home(seat).map(|h| crate::home::same_place(&h, root)).unwrap_or(false) {
                return Some((row.id.clone(), seat));
            }
        }
    }
    None
}

fn make_label(root: &Path) -> Result<Label, Fault> {
    let l = Label::new(whose_by_register(root))?;
    put_sealed(&label_path(root), &label_ident(), &l.to_bytes())?;
    remember_label(root, &l.number);
    Ok(l)
}

pub fn ensure_label(root: &Path) -> Result<Label, Fault> {
    static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    if let Some(l) = read_label(root)? {
        return Ok(l);
    }
    let _one = MAKING.lock().unwrap_or_else(|e| e.into_inner());
    match read_label(root)? {
        Some(l) => Ok(l),
        None => make_label(root),
    }
}

pub fn check_label(root: &Path, opened_as: Option<(&str, crate::roles::Role)>, writer: bool) -> Result<(), Fault> {
    let l = if writer { Some(ensure_label(root)?) } else { read_label(root)? };
    if let (Some(Label { whose: Some((id, seat)), .. }), Some((want_id, want_seat))) = (&l, opened_as) {
        let same = id.trim_start_matches("0x").eq_ignore_ascii_case(want_id.trim_start_matches("0x")) && *seat == want_seat;
        if !same {
            return Err(Unread::Swapped.fault(Doc::HomeLabel, &format!("{} · {id} {}", root.display(), seat.as_str())).into());
        }
    }
    Ok(())
}

fn home_number(root: &Path) -> Result<String, Fault> {
    if let Some(n) = known_label(root) {
        return Ok(n);
    }
    Ok(ensure_label(root)?.number)
}

fn home_owner(root: &Path) -> Result<Option<Owner>, Fault> {
    if let Some(n) = known_label(root) {
        return Ok(Some(Owner::Home(n)));
    }
    Ok(read_label(root)?.map(|l| Owner::Home(l.number)))
}

pub fn expect_at(path: &Path, doc: Doc) -> Result<Expect, Fault> {
    if doc == Doc::HomeLabel {
        return Ok(Expect { owner: Some(Owner::Label), doc, rel: Some(label_rel()) });
    }
    let (root, rel) = place(path, doc)?;
    let owner = if doc.in_machine() { Some(Owner::Machine) } else { home_owner(&root)? };
    Ok(Expect { owner, doc, rel: Some(rel) })
}

pub fn expect_entry(ledger_dir: &Path) -> Result<Expect, Fault> {
    let root = ledger_dir.parent().unwrap_or(ledger_dir);
    Ok(Expect { owner: home_owner(root)?, doc: Doc::Entry, rel: None })
}

pub fn expect_found(f: &Found) -> Result<Expect, Fault> {
    expect_at(&f.at, f.doc)
}

pub fn ident_at(path: &Path, doc: Doc, plain: &[u8]) -> Result<Ident, Fault> {
    if doc == Doc::HomeLabel {
        return Ok(label_ident());
    }
    let (root, rel) = place(path, doc)?;
    let owner = if doc.in_machine() { Owner::Machine } else { Owner::Home(home_number(&root)?) };
    Ok(Ident { owner, doc, logical: logical_rel(doc, &rel, plain)? })
}

pub fn ident_in(owner: Owner, doc: Doc, rel: &str, plain: &[u8]) -> Result<Ident, Fault> {
    Ok(Ident { owner, doc, logical: logical_rel(doc, rel, plain)? })
}

pub fn seal(id: &Ident, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    Ok(env::seal_checked(&crate::keybox::local_key()?, id, plain)?)
}

pub fn seal_at(path: &Path, doc: Doc, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    seal(&ident_at(path, doc, plain)?, plain)
}

pub fn read(path: &Path, doc: Doc) -> Result<Option<Vec<u8>>, Fault> {
    crate::reads::touch();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(classify(&e, &path.display().to_string())),
    };
    let key = crate::keybox::local_key()?;
    open_with(&key, &expect_at(path, doc)?, &bytes, &path.display().to_string()).map(Some)
}

fn not_over(f: Fault, at: &Path) -> Fault {
    match Unread::of(&f) {
        Some(u) => Fault::known(Known::LocalSeal, crate::lang::both_of(|| format!("{} · {}", u.word(), crate::lang::filln(crate::lang::Key::TailUnreadKept, &[&at.display().to_string()])))),
        None => f,
    }
}

fn not_over_unread(path: &Path, doc: Doc) -> Result<(), Fault> {
    let have = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(not_over(crate::fault::classify(&e, &path.display().to_string()), path)),
    };
    let key = crate::keybox::local_key()?;
    let ex = expect_at(path, doc)?;
    open_with(&key, &ex, &have, &path.display().to_string()).map(|_| ()).map_err(|f| not_over(f, path))
}

fn put_sealed(path: &Path, id: &Ident, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal(id, plain)?;
    not_over_unread(path, id.doc)?;
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(&dir, &name, &sealed)
}

pub fn put(dir: &Path, name: &str, doc: Doc, plain: &[u8]) -> Result<(), Fault> {
    crate::reads::touch();
    let at = dir.join(name);
    let sealed = seal(&ident_at(&at, doc, plain)?, plain)?;
    not_over_unread(&at, doc)?;
    crate::home::put_at(dir, name, &sealed)
}

pub fn land(path: &Path, doc: Doc, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal_at(path, doc, plain)?;
    zikaron_glue::landing::land_bytes(path, &sealed).map_err(|t| Fault::of_landing(t))
}

pub use zikaron_seal::ledger::{empty_in, EMPTY};

pub fn empty_fault(names: &[String]) -> Fault {
    zikaron_seal::ledger::empty_trouble(names).into()
}

pub struct Ledger {
    inner: zikaron_seal::ledger::Ledger,
}

impl Ledger {
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<Ledger, Fault> {
        Ok(Ledger { inner: zikaron_seal::ledger::Ledger::open_or_create(root)? })
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Ledger, Fault> {
        Ok(Ledger { inner: zikaron_seal::ledger::Ledger::open(root)? })
    }

    pub fn root(&self) -> &Path {
        self.inner.root()
    }

    pub fn store(&self) -> &zikaron_store::LedgerDir {
        self.inner.store()
    }

    pub fn pile(&self) -> Result<Pile, Fault> {
        let (local, names) = (crate::keybox::local_key()?, crate::names::key()?);
        let owner = home_owner(&self.inner.home_root())?;
        Ok(self.inner.pile(&zikaron_seal::ledger::Keys { local: &local, names: &names, owner })?)
    }

    pub fn survey(&self) -> Result<Survey, Fault> {
        let (local, names) = (crate::keybox::local_key()?, crate::names::key()?);
        let owner = home_owner(&self.inner.home_root())?;
        Ok(self.inner.survey(&zikaron_seal::ledger::Keys { local: &local, names: &names, owner })?)
    }

    pub fn read_named(&self, file: &str) -> Result<Vec<u8>, Fault> {
        let (local, names) = (crate::keybox::local_key()?, crate::names::key()?);
        let owner = home_owner(&self.inner.home_root())?;
        Ok(self.inner.read_named(file, &zikaron_seal::ledger::Keys { local: &local, names: &names, owner })?)
    }

    pub fn seal_entry(&self, plain: &[u8]) -> Result<Vec<u8>, Fault> {
        let number = home_number(&self.inner.home_root())?;
        Ok(self.inner.seal_entry(plain, &crate::keybox::local_key()?, &number)?)
    }

    pub fn append(&self, name: &EntryName, plain: &[u8]) -> Result<Stored, Fault> {
        if crate::task::ticket_void() {
            return Err(crate::task::void_ticket_fault(self.root()));
        }
        let (local, names) = (crate::keybox::local_key()?, crate::names::key()?);
        let root = self.inner.home_root();
        let number = home_number(&root)?;
        let owner = home_owner(&root)?;
        Ok(self.inner.append(name, plain, &zikaron_seal::ledger::Keys { local: &local, names: &names, owner }, &number)?)
    }
}

fn disk_entry_name(id: &str) -> Result<EntryName, Fault> {
    let nk = crate::names::key()?;
    entry_name_under(&nk, id)
}

pub fn entry_name_under(nk: &crate::names::NameKey, id: &str) -> Result<EntryName, Fault> {
    Ok(zikaron_seal::ledger::entry_name_under(nk, id)?)
}

pub fn entry_path(ledger: &Path, id: &str) -> Result<PathBuf, Fault> {
    Ok(ledger.join(zikaron_store::layout::entry_file_name(&disk_entry_name(id)?)))
}

pub fn ledger_of(home: &Home) -> Result<Ledger, Fault> {
    Ledger::open_or_create(home.dir(crate::home::Slot::Ledger))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Whose {
    Machine,
    Seat { id: String, seat: crate::roles::Role },
    Loose,
    Kept { n: usize },
    Left { id: String, seat: crate::roles::Role },
}

#[derive(Clone, Debug)]
pub struct Found {
    pub at: PathBuf,
    pub rel: String,
    pub doc: Doc,
    pub whose: Whose,
}

pub fn machine_doc(rel: &str) -> Option<Doc> {
    if rel == crate::places::registry_file() {
        return Some(Doc::Registry);
    }
    if rel == format!("{}/{}", crate::kitsindex::DIR, crate::kitsindex::OVERRIDES) {
        return Some(Doc::KitLinks);
    }
    if rel == format!("{}/{}", crate::recordsx::DIR, crate::recordsx::FILE) {
        return Some(Doc::Records);
    }
    if rel == format!("{}/{}", crate::kitsindex::DIR, crate::exports::FILE) {
        return Some(Doc::Exports);
    }
    if rel == format!("{}/{}", crate::checkedx::DIR, crate::checkedx::FILE) || rel.strip_prefix(&format!("{}/", crate::checkedx::DIR)).is_some_and(|f| crate::checkedx::is_logs(f) || crate::checkedx::is_proof(f)) {
        return Some(Doc::Checked);
    }
    if rel == crate::backup::index_rel() {
        return Some(Doc::BackupIndex);
    }
    if rel == crate::places::machine_file() {
        return Some(Doc::MachineSealed);
    }
    None
}

pub const FIRST_WINDOW_FILE: &str = "first-window.json";

pub fn home_doc(rel: &str) -> Option<Doc> {
    use crate::home::Slot;
    let parts: Vec<&str> = rel.split('/').collect();
    let settings = Slot::Settings.as_str();
    match parts.as_slice() {
        [s, f] if *s == settings && *f == crate::settings::FILE => Some(Doc::Settings),
        [s, f] if *s == settings && *f == crate::queue::FILE => Some(Doc::Queue),
        [s, f] if *s == settings && *f == FIRST_WINDOW_FILE => Some(Doc::FirstWindow),
        [s, f] if *s == settings && *f == crate::lastread::ANCHORED_FILE => Some(Doc::LastAudit),
        [s, f] if *s == settings && *f == crate::restorex::FILE => Some(Doc::Unfetched),
        [s, f] if *s == settings && *f == crate::restorex::READ_ON => Some(Doc::Unfetched),
        [s, f] if *s == settings && *f == LABEL_FILE => Some(Doc::HomeLabel),
        [l, f] if *l == Slot::Ledger.as_str() && zikaron_store::layout::parse_entry_file(f).is_some() => Some(Doc::Entry),
        [g, f] if *g == Slot::GrantsHeld.as_str() && crate::lastread::is_cache(f) => Some(Doc::Verdict),
        [g, rest @ ..] if *g == Slot::GrantsHeld.as_str() => crate::vaultx::held_doc(&rest.join("/")),
        [k, t, f] if *k == Slot::Kits.as_str() && *t == crate::termsx::ROOM && f.starts_with("grant-") && f.ends_with(".json") => Some(Doc::TermsRecord),
        [k, t, d, _] if *k == Slot::Kits.as_str() && *t == crate::termsx::ROOM && d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()) => Some(Doc::TermsDoc),
        _ => None,
    }
}

pub(crate) fn walk(root: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) {
    let at = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    let Ok(listing) = std::fs::read_dir(&at) else { return };
    for e in listing.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        match std::fs::symlink_metadata(e.path()) {
            Ok(m) if m.is_dir() => walk(root, &r, out),
            Ok(m) if m.is_file() => out.push((e.path(), r)),
            _ => {}
        }
    }
}

fn walk_strict(root: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) -> Result<(), Fault> {
    let at = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    let listing = match std::fs::read_dir(&at) {
        Ok(l) => l,
        Err(e) if !rel.is_empty() && (e.kind() == std::io::ErrorKind::NotFound || zikaron_os::going(&at)) => return Ok(()),
        Err(e) => return Err(classify(&e, &at.display().to_string())),
    };
    for e in listing {
        let e = match e {
            Ok(e) => e,
            Err(x) if !rel.is_empty() && (x.kind() == std::io::ErrorKind::NotFound || zikaron_os::going(&at)) => return Ok(()),
            Err(x) => return Err(classify(&x, &at.display().to_string())),
        };
        let name = e.file_name().to_string_lossy().to_string();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        match std::fs::symlink_metadata(e.path()) {
            Ok(m) if m.is_dir() => walk_strict(root, &r, out)?,
            Ok(m) if m.is_file() => out.push((e.path(), r)),
            Ok(_) => {}
            Err(x) if x.kind() == std::io::ErrorKind::NotFound || zikaron_os::going(&e.path()) => {}
            Err(x) => return Err(classify(&x, &e.path().display().to_string())),
        }
    }
    Ok(())
}

pub fn homes() -> Result<Vec<(Whose, PathBuf)>, Fault> {
    homes_of(crate::register::read()?, false)
}

pub fn homes_on_record() -> Vec<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    if let Ok(h) = crate::home::where_is() {
        places.push(h);
    }
    if let Ok(m) = crate::machine::read() {
        places.extend(m.homes.iter().map(PathBuf::from));
    }
    let mut bases: Vec<PathBuf> = places.iter().filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
    if let Ok(m) = crate::home::machine_dir() {
        bases.push(m);
    }
    let is_id = |n: &str| n.len() == 40 && n.chars().all(|c| c.is_ascii_hexdigit());
    let mut out = places;
    for b in bases {
        let Ok(list) = std::fs::read_dir(&b) else { continue };
        for e in list.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !is_id(&name) || !e.path().is_dir() {
                continue;
            }
            for seat in crate::roles::Role::ALL {
                let h = e.path().join(seat.as_str());
                if h.is_dir() {
                    out.push(h);
                }
            }
        }
    }
    let mut uniq: Vec<PathBuf> = Vec::new();
    for p in out {
        if !uniq.iter().any(|x| crate::home::same_place(x, &p)) {
            uniq.push(p);
        }
    }
    uniq
}

fn homes_of(reg: Option<crate::identity::Registry>, strict: bool) -> Result<Vec<(Whose, PathBuf)>, Fault> {
    let mut out: Vec<(Whose, PathBuf)> = Vec::new();
    if let Some(reg) = &reg {
        for row in &reg.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    if out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
                        continue;
                    }
                    if h.is_dir() {
                        out.push((Whose::Seat { id: row.id.clone(), seat }, h));
                    } else if strict && !h.parent().map(Path::is_dir).unwrap_or(false) {
                        return Err(Fault::known(Known::HomeUnreachable, h.display().to_string()));
                    }
                }
            }
        }
        for (id, seat, home) in &reg.left {
            let h = PathBuf::from(home);
            if h.is_dir() && !out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
                out.push((Whose::Left { id: id.clone(), seat: *seat }, h));
            }
        }
    }
    if let Ok(h) = crate::home::where_is() {
        if h.is_dir() && !out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
            out.push((Whose::Loose, h));
        }
    }
    let remembered = match crate::machine::read() {
        Ok(m) => m.homes,
        Err(f) if strict => return Err(f),
        Err(_) => Vec::new(),
    };
    for (n, h) in remembered.iter().map(PathBuf::from).enumerate() {
        if out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
            continue;
        }
        if h.is_dir() {
            out.push((Whose::Kept { n }, h));
        } else if strict && !h.parent().map(Path::is_dir).unwrap_or(false) {
            return Err(Fault::known(Known::HomeUnreachable, h.display().to_string()));
        }
    }
    Ok(out)
}

pub fn machine_files() -> Result<Vec<Found>, Fault> {
    let m = crate::home::machine_dir()?;
    let mut all = Vec::new();
    walk(&m, "", &mut all);
    let mut out: Vec<Found> = all
        .into_iter()
        .filter_map(|(at, rel)| machine_doc(&rel).map(|doc| Found { at, rel, doc, whose: Whose::Machine }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

pub fn home_files(root: &Path, whose: &Whose) -> Vec<Found> {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    let mut out: Vec<Found> = all
        .into_iter()
        .filter_map(|(at, rel)| home_doc(&rel).map(|doc| Found { at, rel, doc, whose: whose.clone() }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

pub fn all_files() -> Result<Vec<Found>, Fault> {
    let m = crate::home::machine_dir()?;
    let mut mine = Vec::new();
    walk_strict(&m, "", &mut mine)?;
    let mut out: Vec<Found> = mine
        .into_iter()
        .filter_map(|(at, rel)| machine_doc(&rel).map(|doc| Found { at, rel, doc, whose: Whose::Machine }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    for (w, h) in homes_of(crate::register::read()?, true)? {
        let mut all = Vec::new();
        walk_strict(&h, "", &mut all)?;
        let mut here: Vec<Found> = all
            .into_iter()
            .filter_map(|(at, rel)| home_doc(&rel).map(|doc| Found { at, rel, doc, whose: w.clone() }))
            .collect();
        here.sort_by(|a, b| a.rel.cmp(&b.rel));
        out.extend(here);
    }
    Ok(out)
}

pub fn logical_rel(doc: Doc, rel: &str, plain: &[u8]) -> Result<String, Fault> {
    Ok(env::logical_rel(doc, rel, plain)?)
}

pub fn disk_rel(doc: Doc, logical: &str, nk: &crate::names::NameKey) -> Result<String, Fault> {
    Ok(env::disk_rel(doc, logical, nk)?)
}

pub fn home_under(nk: &crate::names::NameKey, id: &str, seat: crate::roles::Role, at: &Path) -> PathBuf {
    let seat_ok = at.file_name().map(|n| n == seat.as_str()).unwrap_or(false);
    let named = at.parent().and_then(|p| p.file_name()).map(|n| crate::names::is_home_name(&n.to_string_lossy())).unwrap_or(false);
    match (seat_ok && named, at.parent().and_then(Path::parent)) {
        (true, Some(base)) => base.join(crate::home::home_dir_name(nk, id)).join(seat.as_str()),
        _ => at.to_path_buf(),
    }
}

#[derive(Default, Debug)]
pub struct Restaged {
    pub to: Vec<PathBuf>,
    pub gone: Vec<PathBuf>,
    pub made: Vec<PathBuf>,
}

pub fn restage(old: &LocalKey, new: &LocalKey, nk: &crate::names::NameKey) -> Result<Restaged, Fault> {
    let files = {
        for (_, h) in homes_of(crate::register::read()?, true)? {
            ensure_label(&h)?;
        }
        all_files()?
    };
    let reg = crate::register::read()?;
    let mut homes: Vec<(PathBuf, PathBuf)> = Vec::new();
    if let Some(r) = &reg {
        for row in &r.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    homes.push((h.clone(), home_under(nk, &row.id, seat, &h)));
                }
            }
        }
        for (id, seat, h) in &r.left {
            let h = PathBuf::from(h);
            homes.push((h.clone(), home_under(nk, id, *seat, &h)));
        }
    }
    let moved_home = |p: &str| -> String {
        homes.iter().find(|(a, _)| crate::home::same_place(a, Path::new(p))).map(|(_, b)| b.display().to_string()).unwrap_or_else(|| p.to_string())
    };
    let mut out = Restaged::default();
    for (a, b) in &homes {
        if a != b && a.is_dir() && !b.exists() {
            let top = identity_dir_of(b).filter(|d| !d.exists()).unwrap_or_else(|| b.clone());
            if !out.made.contains(&top) {
                out.made.push(top);
                stage_plan(PLAN_MADE, &out.made)?;
            }
            crate::home::lay(b)?;
        }
    }
    for f in files {
        let raw = std::fs::read(&f.at).map_err(|e| classify(&e, &f.at.display().to_string()))?;
        let ex = expect_found(&f)?;
        let mut plain = open_with(old, &ex, &raw, &f.rel)?;
        let owner = ex.owner.clone().ok_or_else(|| Fault::from(Unread::Swapped.fault(f.doc, &f.rel)))?;
        let stage_next = |to: &Path, key: &LocalKey, doc: Doc, plain: &[u8]| ident_in(owner.clone(), doc, &f.rel, plain).and_then(|id| stage_next(to, key, &id, plain));
        if f.doc == Doc::Registry {
            let mut r = crate::identity::Registry::parse(&plain).map_err(|w| Fault::known(Known::IdentitiesShape, w))?;
            crate::identity::follow_machine(&mut r);
            for row in r.rows.iter_mut() {
                row.author_home = if row.author_home.is_empty() { String::new() } else { moved_home(&row.author_home) };
                row.grantee_home = if row.grantee_home.is_empty() { String::new() } else { moved_home(&row.grantee_home) };
            }
            for (_, _, h) in r.left.iter_mut() {
                *h = moved_home(h);
            }
            plain = r.to_bytes();
        }
        let root = root_of(&f);
        let new_root = homes.iter().find(|(a, _)| crate::home::same_place(a, &root)).map(|(_, b)| b.clone()).unwrap_or_else(|| root.clone());
        let to = new_root.join(disk_rel(f.doc, &logical_rel(f.doc, &f.rel, &plain)?, nk)?);
        if let Some(d) = to.parent() {
            if !d.exists() {
                let mut top = d.to_path_buf();
                while let Some(up) = top.parent() {
                    if up.exists() {
                        break;
                    }
                    top = up.to_path_buf();
                }
                if !out.made.contains(&top) {
                    out.made.push(top);
                    stage_plan(PLAN_MADE, &out.made)?;
                }
                std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
            }
        }
        let put = stage_next(&to, new, f.doc, &plain);
        zikaron_os::wipe(&mut plain);
        put?;
        if to != f.at {
            out.gone.push(f.at.clone());
        }
        out.to.push(to);
    }
    stage_plan(PLAN_MADE, &out.made)?;
    stage_plan_sealed(PLAN_GONE, new, &out.gone.iter().map(|p| p.display().to_string()).collect::<Vec<_>>())?;
    let moved: Vec<String> = homes.iter().filter(|(a, b)| a != b && a.is_dir()).map(|(a, b)| format!("{}\t{}", a.display(), b.display())).collect();
    stage_plan_sealed(PLAN_MOVED, new, &moved)?;
    Ok(out)
}

fn root_of(f: &Found) -> PathBuf {
    let mut root = f.at.clone();
    for _ in f.rel.split('/') {
        root.pop();
    }
    root
}

fn beside(at: &Path, suffix: &str) -> PathBuf {
    let mut s = at.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

pub const SET_ASIDE: &str = "set-aside";

pub fn set_aside_sealed() -> Result<(usize, Option<PathBuf>), Fault> {
    let mut roots: Vec<PathBuf> = vec![crate::home::machine_dir()?];
    if let Ok(h) = crate::home::where_is() {
        roots.push(h);
    }
    if let Ok(m) = crate::machine::read() {
        roots.extend(m.homes.iter().map(PathBuf::from));
    }
    set_aside_in(roots, None)
}

pub fn set_aside_unopenable(roots: Vec<PathBuf>, key: &LocalKey) -> Result<(usize, Option<PathBuf>), Fault> {
    set_aside_in(roots, Some(key))
}

fn opens_under(key: &LocalKey, bytes: &[u8]) -> bool {
    opens_under_key(key.bytes(), bytes)
}

fn set_aside_in(roots: Vec<PathBuf>, keep: Option<&LocalKey>) -> Result<(usize, Option<PathBuf>), Fault> {
    let m = crate::home::machine_dir()?;
    let live: Vec<PathBuf> = match keep {
        Some(_) => homes_of(crate::register::read().ok().flatten(), false).map(|h| h.into_iter().map(|(_, p)| p).collect()).unwrap_or_default(),
        None => Vec::new(),
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut sealed: Vec<PathBuf> = Vec::new();
    let mut seats: Vec<PathBuf> = Vec::new();
    for root in roots {
        if !root.is_dir() || seen.iter().any(|x| crate::home::same_place(x, &root)) {
            continue;
        }
        seen.push(root.clone());
        let mut all = Vec::new();
        walk(&root, "", &mut all);
        let here: Vec<PathBuf> = all
            .into_iter()
            .filter(|(_, rel)| !rel.split('/').any(|c| c.starts_with(SET_ASIDE)) && !is_staged(&rel))
            .filter(|(at, _)| match std::fs::read(at) {
                Ok(b) => is_sealed(&b) && keep.map(|k| !opens_under(k, &b)).unwrap_or(true),
                Err(_) => false,
            })
            .map(|(at, _)| at)
            .collect();
        for at in here {
            if !sealed.iter().any(|x| x == &at) {
                sealed.push(at);
            }
        }
        if identity_dir_of(&root).is_some() && !live.iter().any(|h| crate::home::same_place(h, &root)) {
            seats.push(root);
        }
    }
    let mut to: Option<PathBuf> = None;
    let mut n = 0usize;
    let mut put = |at: &Path, to: &mut Option<PathBuf>| -> Result<(), Fault> {
        if to.is_none() {
            let mut p = m.join(SET_ASIDE);
            let mut i = 2u32;
            while p.exists() {
                p = m.join(format!("{SET_ASIDE}-{i}"));
                i += 1;
            }
            std::fs::create_dir_all(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
            *to = Some(p);
        }
        n += 1;
        let dest = to.as_ref().map(|p| p.join(format!("{n}"))).unwrap_or_default();
        move_across(at, &dest)
    };
    for at in &sealed {
        put(at, &mut to)?;
    }
    for root in &seats {
        let mut rest = Vec::new();
        walk(root, "", &mut rest);
        if rest.iter().any(|(at, _)| std::fs::read(at).map(|b| is_sealed(&b)).unwrap_or(false)) {
            continue;
        }
        for (at, rel) in rest {
            if !is_staged(&rel) {
                put(&at, &mut to)?;
            }
        }
        prune_empty_under(root);
        if std::fs::remove_dir(root).is_ok() {
            if let Some(d) = root.parent() {
                let _ = std::fs::remove_dir(d);
            }
        }
    }
    Ok((sealed.len(), to))
}

pub fn plan_path(machine: &Path, name: &str) -> PathBuf {
    staged_path(&machine.join(name))
}

pub fn staged_path(at: &Path) -> PathBuf {
    beside(at, crate::keybox::NEXT)
}

fn put_plan(machine: &Path, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    let at = plan_path(machine, name);
    let file = at.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(machine, &file, bytes)
}

pub const GONE: &[u8] = b"zikaron-local/gone";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    BeforeCommit,
    AfterCommit,
}

static CUT: std::sync::OnceLock<Cut> = std::sync::OnceLock::new();

pub fn set_cut(c: Cut) -> bool {
    CUT.set(c).is_ok()
}

pub fn cut_point(c: Cut) -> Result<(), Fault> {
    if CUT.get() == Some(&c) {
        return Err(Fault::known(Known::RestorePartial, format!("cut {c:?}")));
    }
    Ok(())
}

pub fn is_cut(f: &Fault) -> bool {
    CUT.get().is_some() && f.which() == Some(Known::RestorePartial) && f.tail().starts_with("cut ")
}

pub fn drop_staged_everywhere() -> Result<(), Fault> {
    drop_staged_machine()?;
    for (_, h) in homes()? {
        let mut all = Vec::new();
        walk(&h, "", &mut all);
        for (at, rel) in all {
            if is_staged(&rel) {
                let _ = std::fs::remove_file(at);
            }
        }
    }
    Ok(())
}

pub fn drop_change() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    for d in read_plan(&m, PLAN_MADE) {
        if d.is_dir() {
            let _ = std::fs::remove_dir_all(&d);
        }
    }
    drop_staged_everywhere()
}

pub fn drop_staged_machine() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    let mut all = Vec::new();
    walk(&m, "", &mut all);
    for (at, rel) in all {
        if is_staged(&rel) {
            let _ = std::fs::remove_file(at);
        }
    }
    Ok(())
}

pub fn stage_next(at: &Path, key: &LocalKey, id: &Ident, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal_with(key, id, plain)?;
    let next = staged_path(at);
    let dir = next.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = next.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(&dir, &name, &sealed)
}

pub const PLAN_MADE: &str = "change-made-homes";
pub const PLAN_ASIDE: &str = "change-set-aside";
pub const PLAN_GONE: &str = "change-gone";
pub const PLAN_MOVED: &str = "change-moved-homes";

pub fn stage_plan(name: &str, paths: &[PathBuf]) -> Result<(), Fault> {
    let lines: String = paths.iter().map(|p| format!("{}\n", p.display())).collect();
    put_plan(&crate::home::machine_dir()?, name, lines.as_bytes())
}

pub fn stage_plan_sealed(name: &str, key: &LocalKey, lines: &[String]) -> Result<(), Fault> {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let sealed = seal_with(key, &Ident { owner: Owner::Machine, doc: Doc::Plan, logical: name.to_string() }, text.as_bytes())?;
    put_plan(&crate::home::machine_dir()?, name, &sealed)
}

fn read_plan_sealed(machine: &Path, name: &str, key: &LocalKey) -> Vec<String> {
    let at = plan_path(&machine, name);
    let Ok(raw) = std::fs::read(&at) else { return Vec::new() };
    match open_with(key, &Expect { owner: Some(Owner::Machine), doc: Doc::Plan, rel: Some(name.to_string()) }, &raw, name) {
        Ok(mut plain) => {
            let out = String::from_utf8_lossy(&plain).lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
            zikaron_os::wipe(&mut plain);
            out
        }
        Err(_) => Vec::new(),
    }
}

fn prune_up(from: &Path, stop: &Path) {
    let mut at = from.to_path_buf();
    while at.starts_with(stop) && at != stop {
        if std::fs::remove_dir(&at).is_err() {
            break;
        }
        match at.parent() {
            Some(p) => at = p.to_path_buf(),
            None => break,
        }
    }
}

fn prune_empty_under(root: &Path) {
    let Ok(list) = std::fs::read_dir(root) else { return };
    for e in list.flatten() {
        let p = e.path();
        if std::fs::symlink_metadata(&p).map(|m| m.is_dir()).unwrap_or(false) {
            prune_empty_under(&p);
            let _ = std::fs::remove_dir(&p);
        }
    }
}

fn identity_dir_of(home: &Path) -> Option<PathBuf> {
    let seat = home.file_name()?.to_string_lossy().to_string();
    let parent = home.parent()?;
    let named = crate::names::is_home_name(&parent.file_name()?.to_string_lossy());
    (named && crate::roles::Role::ALL.iter().any(|r| r.as_str() == seat)).then(|| parent.to_path_buf())
}

fn carry_home(old: &Path, new: &Path) -> usize {
    let mut failed = 0usize;
    let mut rest = Vec::new();
    walk(old, "", &mut rest);
    let mut clash: Vec<PathBuf> = Vec::new();
    for (at, rel) in rest {
        if is_staged(&rel) {
            continue;
        }
        let to = new.join(&rel);
        if to.exists() {
            clash.push(at);
            continue;
        }
        let moved = to.parent().map(|d| std::fs::create_dir_all(d).is_ok()).unwrap_or(true) && move_across(&at, &to).is_ok();
        if !moved {
            failed += 1;
        }
    }
    if !clash.is_empty() && aside_flat(&clash).is_err() {
        failed += clash.len();
    }
    prune_empty_under(old);
    match std::fs::remove_dir(old) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => failed += 1,
    }
    if let Some(d) = identity_dir_of(old) {
        let _ = std::fs::remove_dir(&d);
    }
    failed
}

fn move_across(from: &Path, to: &Path) -> Result<(), Fault> {
    let n = MOVES.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    if let Some(MoveFault::FailOnce(k)) = MOVE_FAULT.get() {
        if *k == n {
            return Err(classify(&std::io::Error::other("injected"), &to.display().to_string()));
        }
    }
    let renamed = if matches!(MOVE_FAULT.get(), Some(MoveFault::CrossDevice)) {
        Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices))
    } else {
        std::fs::rename(from, to)
    };
    match renamed {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let say = |e: std::io::Error| classify(&e, &to.display().to_string());
            let mut src = std::fs::File::open(from).map_err(say)?;
            let mut dst = std::fs::OpenOptions::new().write(true).create_new(true).open(to).map_err(say)?;
            std::io::copy(&mut src, &mut dst).map_err(say)?;
            dst.sync_all().map_err(say)?;
            std::fs::remove_file(from).map_err(|e| classify(&e, &from.display().to_string()))
        }
        Err(e) => Err(classify(&e, &to.display().to_string())),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveFault {
    CrossDevice,
    FailOnce(usize),
}

static MOVE_FAULT: std::sync::OnceLock<MoveFault> = std::sync::OnceLock::new();
static MOVES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn set_move_fault(f: MoveFault) -> bool {
    MOVE_FAULT.set(f).is_ok()
}

pub fn moves_so_far() -> usize {
    MOVES.load(std::sync::atomic::Ordering::SeqCst)
}

fn aside_flat(files: &[PathBuf]) -> Result<PathBuf, Fault> {
    let m = crate::home::machine_dir()?;
    let mut to = m.join(SET_ASIDE);
    let mut i = 2u32;
    while to.exists() {
        to = m.join(format!("{SET_ASIDE}-{i}"));
        i += 1;
    }
    std::fs::create_dir_all(&to).map_err(|e| classify(&e, &to.display().to_string()))?;
    for (n, at) in files.iter().enumerate() {
        if let Err(f) = move_across(at, &to.join(format!("{}", n + 1))) {
            let _ = std::fs::remove_dir(&to);
            return Err(f);
        }
    }
    Ok(to)
}

pub const ASIDE_HOMES: &str = "aside-homes";

pub const PLAN_SWAP: &str = "change-swap-home";

pub fn staged_home(root: &Path) -> PathBuf {
    staged_path(root)
}

pub fn stage_fresh_home(root: &Path) -> Result<crate::home::Home, Fault> {
    let staged = staged_home(root);
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(|e| classify(&e, &staged.display().to_string()))?;
    }
    let home = crate::home::Home::open_or_create(&staged)?;
    if let Ok(bytes) = std::fs::read(label_path(root)) {
        crate::home::put_at(&home.dir(crate::home::Slot::Settings), LABEL_FILE, &bytes)?;
    }
    carry_settings(root, &home)?;
    for slot in [crate::home::Slot::GrantsHeld, crate::home::Slot::Kits] {
        copy_tree(&root.join(slot.as_str()), &home.dir(slot))?;
    }
    crate::restorex::write(&home, &crate::restorex::State::Unfetched)?;
    Ok(home)
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), Fault> {
    let mut files = Vec::new();
    walk(from, "", &mut files);
    for (at, rel) in files {
        let dest = to.join(&rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
        }
        let say = |e: std::io::Error| classify(&e, &dest.display().to_string());
        if zikaron_os::is_owner_only(&at).map_err(|e| classify(&e, &at.display().to_string()))? {
            let mut o = zikaron_os::Options::new();
            o.write(true).create(true).truncate(true);
            zikaron_os::owner_only(&mut o);
            let mut out = o.open(&dest).map_err(say)?;
            let mut src = std::fs::File::open(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
            std::io::copy(&mut src, &mut out).map_err(say)?;
        } else {
            std::fs::copy(&at, &dest).map_err(say)?;
        }
        zikaron_os::sync_file(&dest).map_err(say)?;
    }
    Ok(())
}

pub fn swap_aside(root: &Path) -> Result<PathBuf, Fault> {
    let m = crate::home::machine_dir()?;
    let room = m.join(ASIDE_HOMES);
    std::fs::create_dir_all(&room).map_err(|e| classify(&e, &room.display().to_string()))?;
    let mut n = 1u32;
    while room.join(format!("{n}")).exists() || room.join(format!("{n}{PARTIAL}")).exists() {
        n += 1;
    }
    let to = room.join(format!("{n}"));
    put_plan(&m, PLAN_SWAP, format!("{}\t{}\n", root.display(), to.display()).as_bytes())?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    crate::machine::update(|machine| {
        machine.homes.push(to.display().to_string());
        machine.aside_at.push((to.display().to_string(), now));
    })?;
    cut_point(Cut::BeforeCommit)?;
    move_dir(root, &to)?;
    cut_point(Cut::AfterCommit)?;
    crate::home::rename_over(&staged_home(root), root)?;
    let _ = std::fs::remove_file(plan_path(&m, PLAN_SWAP));
    Ok(to)
}

const PARTIAL: &str = ".partial";

fn move_dir(from: &Path, to: &Path) -> Result<(), Fault> {
    let injected = matches!(MOVE_FAULT.get(), Some(MoveFault::CrossDevice));
    match if injected { Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices)) } else { std::fs::rename(from, to) } {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let partial = beside(to, PARTIAL);
            if partial.exists() {
                std::fs::remove_dir_all(&partial).map_err(|e| classify(&e, &partial.display().to_string()))?;
            }
            copy_tree(from, &partial)?;
            crate::home::rename_over(&partial, to)?;
            std::fs::remove_dir_all(from).map_err(|e| classify(&e, &from.display().to_string()))
        }
        Err(e) => Err(classify(&e, &to.display().to_string())),
    }
}

pub fn settle_swap() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    let plan = plan_path(&m, PLAN_SWAP);
    if let Ok(text) = std::fs::read_to_string(&plan) {
        if let Some((root, to)) = text.trim().split_once('\t') {
            let (root, to) = (PathBuf::from(root), PathBuf::from(to));
            let staged = staged_home(&root);
            if staged.is_dir() {
                if to.is_dir() {
                    if root.exists() {
                        std::fs::remove_dir_all(&root).map_err(|e| classify(&e, &root.display().to_string()))?;
                    }
                    crate::home::rename_over(&staged, &root)?;
                } else {
                    let _ = std::fs::remove_dir_all(beside(&to, PARTIAL));
                    let _ = std::fs::remove_dir_all(&staged);
                    crate::machine::update(|machine| {
                        machine.homes.retain(|h| Path::new(h) != to);
                        machine.aside_at.retain(|(h, _)| Path::new(h) != to);
                    })?;
                }
            }
        }
        let _ = std::fs::remove_file(&plan);
    }
    if let Ok(Some(reg)) = crate::register::read() {
        for row in &reg.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    let staged = staged_home(&h);
                    if staged.is_dir() && h.is_dir() {
                        let _ = std::fs::remove_dir_all(&staged);
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn carry_settings(from_root: &Path, into: &crate::home::Home) -> Result<(), Fault> {
    let from = from_root.join(crate::home::Slot::Settings.as_str()).join(crate::settings::FILE);
    match read(&from, Doc::Settings)? {
        Some(plain) => put(&into.dir(crate::home::Slot::Settings), crate::settings::FILE, Doc::Settings, &plain),
        None => Ok(()),
    }
}

fn home_places() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Ok(m) = crate::home::machine_dir() {
        out.push(m);
    }
    if let Some(p) = crate::home::where_is().ok().and_then(|h| h.parent().map(Path::to_path_buf)) {
        out.push(p);
    }
    if let Ok(m) = crate::machine::read() {
        out.extend(m.homes.iter().filter_map(|h| Path::new(h).parent().map(Path::to_path_buf)));
    }
    for d in known_identity_dirs() {
        if let Some(p) = d.parent() {
            out.push(p.to_path_buf());
        }
    }
    let mut uniq: Vec<PathBuf> = Vec::new();
    for p in out {
        if !uniq.iter().any(|x| crate::home::same_place(x, &p)) {
            uniq.push(p);
        }
    }
    uniq
}

fn known_identity_dirs() -> Vec<PathBuf> {
    let Ok(Some(reg)) = crate::register::read() else { return Vec::new() };
    let mut homes: Vec<PathBuf> = reg.rows.iter().flat_map(|r| crate::roles::Role::ALL.iter().filter_map(|s| r.home(*s)).collect::<Vec<_>>()).collect();
    homes.extend(reg.left.iter().map(|(_, _, h)| PathBuf::from(h)));
    homes.iter().filter_map(|h| identity_dir_of(h)).collect()
}

pub fn clear_strays() -> Result<(usize, Option<PathBuf>), Fault> {
    let known = known_identity_dirs();
    let seat_named = |p: &Path| crate::roles::Role::ALL.iter().any(|r| p.join(r.as_str()).is_dir());
    let mut aside: Vec<PathBuf> = Vec::new();
    let mut strays: Vec<PathBuf> = Vec::new();
    for place in home_places() {
        let Ok(list) = std::fs::read_dir(&place) else { continue };
        for e in list.flatten() {
            let d = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if !d.is_dir() || !crate::names::is_home_name(&name) || !seat_named(&d) || known.iter().any(|k| crate::home::same_place(k, &d)) {
                continue;
            }
            if strays.iter().any(|x| crate::home::same_place(x, &d)) {
                continue;
            }
            let mut files = Vec::new();
            walk(&d, "", &mut files);
            aside.extend(files.into_iter().filter(|(_, rel)| !is_staged(rel)).map(|(at, _)| at));
            strays.push(d);
        }
    }
    let to = if aside.is_empty() { None } else { Some(aside_flat(&aside)?) };
    for d in &strays {
        prune_empty_under(d);
        let _ = std::fs::remove_dir(d);
    }
    Ok((aside.len(), to))
}

fn read_plan(machine: &Path, name: &str) -> Vec<PathBuf> {
    std::fs::read_to_string(plan_path(&machine, name))
        .map(|t| t.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled {
    pub forward: usize,
    pub back: usize,
    pub held: usize,
}

fn settle_in(root: &Path, doc_of: &dyn Fn(&str) -> Option<Doc>, key: &LocalKey, back_all: bool, out: &mut Settled, kept: &mut Vec<PathBuf>) -> Result<(), Fault> {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    for (at, rel) in all {
        let Some(base) = unstaged(&rel) else { continue };
        let Some(doc) = doc_of(base) else { continue };
        let target = beside_strip(&at);
        let opened = if back_all { None } else { std::fs::read(&at).ok().and_then(|b| open_staged(key, doc, &b)).map(|p| (doc, p)) };
        match opened {
            Some((Doc::Machine, plain)) => {
                match crate::machine::write_plain_bytes(&plain) {
                    Ok(()) => {
                        let _ = std::fs::remove_file(&at);
                        out.forward += 1;
                    }
                    Err(f) => {
                        note_trouble(f);
                        kept.push(at.clone());
                        out.held += 1;
                    }
                }
            }
            Some((_, plain)) if plain == GONE => {
                let _ = std::fs::remove_file(&target);
                let _ = std::fs::remove_file(&at);
                out.forward += 1;
            }
            Some(_) => match crate::home::rename_over(&at, &target) {
                Ok(()) => out.forward += 1,
                Err(f) => {
                    note_trouble(f);
                    kept.push(at.clone());
                    out.held += 1;
                }
            },
            None => {
                let _ = std::fs::remove_file(&at);
                out.back += 1;
            }
        }
    }
    Ok(())
}

fn beside_strip(at: &Path) -> PathBuf {
    let s = at.as_os_str().to_string_lossy().to_string();
    PathBuf::from(unstaged(&s).unwrap_or(&s))
}

fn homes_stop(at: &Path, homes: &[(Whose, PathBuf)], machine: &Path) -> PathBuf {
    homes.iter().map(|(_, h)| h).find(|h| at.starts_with(h)).cloned().unwrap_or_else(|| {
        at.ancestors().find(|a| identity_dir_of(a).is_some()).map(Path::to_path_buf).unwrap_or_else(|| machine.to_path_buf())
    })
}

pub fn settle_pending() -> Result<Settled, Fault> {
    let key = crate::keybox::local_key()?;
    if let Err(f) = settle_swap() {
        note_trouble(f);
    }
    let m = crate::home::machine_dir()?;
    let never = crate::keybox::drop_staged()?;
    let mut out = Settled::default();
    let machine_doc_or = |rel: &str| -> Option<Doc> {
        if rel == crate::machine::FILE {
            Some(Doc::Machine)
        } else {
            machine_doc(rel)
        }
    };
    let mut kept: Vec<PathBuf> = Vec::new();
    let made = read_plan(&m, PLAN_MADE);
    let aside = read_plan(&m, PLAN_ASIDE);
    let gone = if never { Vec::new() } else { read_plan_sealed(&m, PLAN_GONE, &key) };
    let moved = if never { Vec::new() } else { read_plan_sealed(&m, PLAN_MOVED, &key) };
    if never {
        for d in &made {
            if d.is_dir() {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }
    settle_in(&m, &machine_doc_or, &key, never, &mut out, &mut kept)?;
    let homes = homes_of(crate::register::read()?, false)?;
    for (_, h) in &homes {
        settle_in(h, &home_doc, &key, never, &mut out, &mut kept)?;
    }
    if !never && (!gone.is_empty() || !moved.is_empty()) {
        let mut failed = 0usize;
        if out.held == 0 {
            for g in &gone {
                let at = PathBuf::from(g);
                match std::fs::remove_file(&at) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => failed += 1,
                }
                if let Some(d) = at.parent() {
                    let stop = homes_stop(&at, &homes, &m);
                    prune_up(d, &stop);
                }
            }
            for line in &moved {
                if let Some((a, b)) = line.split_once('\t') {
                    failed += carry_home(Path::new(a), Path::new(b));
                }
            }
        }
        if out.held > 0 || failed > 0 {
            if failed > 0 {
                note_trouble(Fault::known(Known::RestorePartial, format!("names · {failed}")));
            }
            for name in [PLAN_GONE, PLAN_MOVED] {
                kept.push(plan_path(&m, name));
            }
        }
    }
    let changed = !gone.is_empty() || !moved.is_empty() || !aside.is_empty();
    if !never && !aside.is_empty() {
        match set_aside_unopenable(aside, &key) {
            Ok((0, _)) => {}
            Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
            Err(f) => {
                note_trouble(f);
                kept.push(plan_path(&m, PLAN_ASIDE));
            }
        }
    }
    if !never && changed && kept.is_empty() {
        match clear_strays() {
            Ok((0, _)) => {}
            Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
            Err(f) => note_trouble(f),
        }
    }
    let mut left = Vec::new();
    walk(&m, "", &mut left);
    for (_, h) in &homes {
        walk(h, "", &mut left);
    }
    for (at, rel) in left {
        if is_staged(&rel) && !kept.contains(&at) && std::fs::remove_file(&at).is_ok() {
            out.back += 1;
        }
    }
    Ok(out)
}

static OPENED: std::sync::Mutex<Option<Opened>> = std::sync::Mutex::new(None);

#[derive(Clone, Debug, Default)]
pub struct Opened {
    pub settled: Settled,
    pub troubles: Vec<Fault>,
}

pub fn after_open() {
    let mut o = Opened::default();
    match settle_pending() {
        Ok(s) => o.settled = s,
        Err(f) => o.troubles.push(f),
    }
    if let Ok(mut g) = OPENED.lock() {
        let was = g.get_or_insert_with(Opened::default);
        was.settled = o.settled;
        was.troubles.extend(o.troubles);
    }
}

pub fn note_trouble(f: Fault) {
    if let Ok(mut g) = OPENED.lock() {
        g.get_or_insert_with(Opened::default).troubles.push(f);
    }
}

pub fn take_opened() -> Option<Opened> {
    OPENED.lock().ok().and_then(|mut g| g.take())
}
