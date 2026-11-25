use crate::fault::{classify, Fault, Known};
use crate::local::{Doc, Whose};
use crate::roles::Role;
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

pub const MAGIC: &[u8] = b"zikaron-backup/1\n";
pub const APP: &str = "zikaron-desk";
pub const FORMAT: u64 = 1;
pub const EXT: &str = "zikaron";
pub const PASSWORD_MIN: usize = 8;
pub fn password_ok(password: &crate::secret::Secret, again: &crate::secret::Secret) -> Result<(), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    if password.overflowed() || again.overflowed() {
        return Err(Fault::known(Known::PasswordLong, crate::secret::CAP.to_string()));
    }
    if password != again {
        return Err(Fault::known(Known::PasswordsDiffer, String::new()));
    }
    if password.chars() < PASSWORD_MIN {
        return Err(Fault::known(Known::PasswordShort, password.chars().to_string()));
    }
    Ok(())
}

const NONCE: usize = 24;
const SALT: usize = 32;

pub fn file_stem(now: u64) -> String {
    let (y, m, d, _, _, _) = crate::when::civil(now);
    format!("zikaron-backup-{y:04}-{m:02}-{d:02}")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub created: u64,
    pub identities: usize,
    pub entries: usize,
    pub records: usize,
    pub held: usize,
}

#[derive(Clone)]
struct KeyOf {
    id: String,
    kind: String,
    account: String,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct FileOf {
    whose: Whose,
    rel: String,
    doc: Doc,
    bytes: Vec<u8>,
}

struct Package {
    registry: Vec<u8>,
    primary: Option<(String, crate::keybox::PrimaryKind)>,
    keys: Vec<KeyOf>,
    machine: Vec<u8>,
    files: Vec<FileOf>,
    read_nets: Option<Vec<u8>>,
}

impl Drop for Package {
    fn drop(&mut self) {
        for k in self.keys.iter_mut() {
            zikaron_os::wipe(&mut k.bytes);
        }
    }
}

struct Wiped(Vec<u8>);

impl Drop for Wiped {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}

impl std::ops::Deref for Wiped {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

struct Slots(Vec<(String, Vec<u8>)>);

impl Drop for Slots {
    fn drop(&mut self) {
        for (_, b) in self.0.iter_mut() {
            zikaron_os::wipe(b);
        }
    }
}

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    zikaron::hexfmt::decode(s)
}

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

fn obj(m: Vec<(&str, Value)>) -> Value {
    let mut m: Vec<(String, Value)> = m.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    m.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Obj(m)
}

fn text(v: &Value, k: &str) -> Option<String> {
    match v.member(k) {
        Some(Value::Str(x)) => Some(x.clone()),
        _ => None,
    }
}

fn whose_value(w: &Whose) -> Vec<(&'static str, Value)> {
    match w {
        Whose::Machine => vec![("whose", s("machine"))],
        Whose::Loose => vec![("whose", s("loose"))],
        Whose::Kept { n } => vec![("whose", s("kept")), ("n", Value::Int(*n as u64))],
        Whose::Seat { id, seat } => vec![("whose", s("seat")), ("id", s(id)), ("seat", s(seat.as_str()))],
        Whose::Left { id, seat } => vec![("whose", s("left")), ("id", s(id)), ("seat", s(seat.as_str()))],
    }
}

pub fn transcribe(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn package_bytes(p: &Package) -> Vec<u8> {
    let keys: Vec<Value> = p.keys.iter().map(|k| obj(vec![("account", s(&k.account)), ("hex", s(&transcribe(&k.bytes))), ("id", s(&k.id)), ("kind", s(&k.kind))])).collect();
    let files: Vec<Value> = p
        .files
        .iter()
        .map(|f| {
            let mut m = whose_value(&f.whose);
            m.push(("rel", s(&f.rel)));
            m.push(("doc", s(f.doc.tag())));
            m.push(("hex", s(&transcribe(&f.bytes))));
            obj(m)
        })
        .collect();
    let primary = match &p.primary {
        Some((id, k)) => obj(vec![("id", s(id)), ("kind", s(k.as_str()))]),
        None => Value::Null,
    };
    let mut members = vec![
        ("app", s(APP)),
        ("files", Value::Arr(files)),
        ("format", Value::Int(FORMAT)),
        ("keys", Value::Arr(keys)),
        ("machine", s(&transcribe(&p.machine))),
        ("primary", primary),
    ];
    if let Some(b) = &p.read_nets {
        members.push(("readNetworks", s(&transcribe(b))));
    }
    members.push(("registry", s(&transcribe(&p.registry))));
    json::canon_bytes(&obj(members))
}

fn package_of(bytes: &[u8]) -> Result<Package, Fault> {
    let bad = |w: &str| Fault::known(Known::BackupShape, w.to_string());
    let v = json::parse(bytes).map_err(|_| bad("package"))?;
    if text(&v, "app").as_deref() != Some(APP) {
        return Err(Fault::known(Known::BackupNotOurs, String::new()));
    }
    let registry = text(&v, "registry").and_then(|x| unhex(&x)).ok_or_else(|| bad("registry"))?;
    let machine = text(&v, "machine").and_then(|x| unhex(&x)).ok_or_else(|| bad("machine"))?;
    let primary = match v.member("primary") {
        Some(Value::Null) | None => None,
        Some(p) => {
            let id = text(p, "id").ok_or_else(|| bad("primary"))?;
            let k = match text(p, "kind").as_deref() {
                Some("words") => crate::keybox::PrimaryKind::Words,
                Some("key-file") => crate::keybox::PrimaryKind::KeyFile,
                _ => return Err(bad("primary")),
            };
            Some((id, k))
        }
    };
    let mut keys = Vec::new();
    if let Some(Value::Arr(a)) = v.member("keys") {
        for k in a {
            keys.push(KeyOf {
                id: text(k, "id").ok_or_else(|| bad("key"))?,
                kind: text(k, "kind").ok_or_else(|| bad("key"))?,
                account: text(k, "account").ok_or_else(|| bad("key"))?,
                bytes: text(k, "hex").and_then(|x| unhex(&x)).ok_or_else(|| bad("key"))?,
            });
        }
    }
    let mut files = Vec::new();
    if let Some(Value::Arr(a)) = v.member("files") {
        for f in a {
            let whose = match text(f, "whose").as_deref() {
                Some("machine") => Whose::Machine,
                Some("loose") => Whose::Loose,
                Some("kept") => match f.member("n") {
                    Some(Value::Int(n)) if *n < 1_000_000 => Whose::Kept { n: *n as usize },
                    _ => return Err(bad("file")),
                },
                Some("seat") => {
                    let id = text(f, "id").ok_or_else(|| bad("file"))?;
                    let seat = match text(f, "seat").as_deref() {
                        Some(x) if x == Role::Author.as_str() => Role::Author,
                        Some(x) if x == Role::Grantee.as_str() => Role::Grantee,
                        _ => return Err(bad("file")),
                    };
                    Whose::Seat { id, seat }
                }
                _ => return Err(bad("file")),
            };
            let rel = text(f, "rel").ok_or_else(|| bad("file"))?;
            if !plain_member(&rel) {
                return Err(bad("file"));
            }
            let doc = text(f, "doc").and_then(|t| Doc::from_tag(&t)).filter(|d| !matches!(d, Doc::Registry | Doc::BackupIndex)).ok_or_else(|| bad("file"))?;
            let fits = match whose {
                Whose::Machine => crate::local::machine_doc(&rel) == Some(doc),
                _ => crate::local::home_doc(&rel) == Some(doc),
            };
            if !fits {
                return Err(bad("file"));
            }
            let bytes = text(f, "hex").and_then(|x| unhex(&x)).ok_or_else(|| bad("file"))?;
            files.push(FileOf { whose, rel, doc, bytes });
        }
    }
    let read_nets = match v.member("readNetworks") {
        None => None,
        Some(x) => {
            let b = match x {
                Value::Str(t) => unhex(t).ok_or_else(|| bad("readNetworks"))?,
                _ => return Err(bad("readNetworks")),
            };
            crate::readnets::from_bytes(&b, "readNetworks").map_err(|_| bad("readNetworks"))?;
            Some(b)
        }
    };
    Ok(Package { registry, primary, keys, machine, files, read_nets })
}

fn plain_member(rel: &str) -> bool {
    rel.split('/').all(|x| !x.is_empty() && !x.ends_with(['.', ' ']) && !x.contains(['\\', ':', '\0']))
}

fn nonce_and_salt() -> Result<(Vec<u8>, Vec<u8>), Fault> {
    Ok((crate::key::random(NONCE)?, crate::key::random(SALT)?))
}

pub fn params() -> crate::keystore::Params {
    let p = crate::keybox::params();
    if p != crate::keystore::Params::standard() {
        return p;
    }
    let mut p = p;
    let mut out = [0u8; 32];
    loop {
        let t = std::time::Instant::now();
        let _ = crate::cryptx::scrypt(b"calibrate", b"zikaron-backup", p.n, p.r, p.p, &mut out);
        let next = crate::keystore::Params { n: p.n * 2, ..p };
        if t.elapsed() >= std::time::Duration::from_millis(500) || next.n > crate::keystore::Params::standard().n || !crate::keystore::in_range(next.n, next.r, next.p) {
            return p;
        }
        p = next;
    }
}

fn key_of(password: &str, salt: &[u8], k: crate::keystore::Params) -> Result<[u8; 32], Fault> {
    let mut out = [0u8; 32];
    if !crate::cryptx::scrypt(password.as_bytes(), salt, k.n, k.r, k.p, &mut out) {
        return Err(Fault::known(Known::BackupShape, format!("kdf n={} r={} p={}", k.n, k.r, k.p)));
    }
    Ok(out)
}

fn seal(package: &[u8], password: &str, created: u64) -> Result<Vec<u8>, Fault> {
    let k = params();
    let (n, salt) = nonce_and_salt()?;
    let header = json::canon_bytes(&obj(vec![
        ("app", s(APP)),
        ("created", Value::Int(created)),
        ("format", Value::Int(FORMAT)),
        ("kdf", obj(vec![("n", Value::Int(k.n as u64)), ("p", Value::Int(k.p as u64)), ("r", Value::Int(k.r as u64)), ("salt", s(&hex(&salt)))])),
        ("nonce", s(&hex(&n))),
    ]));
    let mut aad = MAGIC.to_vec();
    aad.extend_from_slice(&header);
    aad.push(b'\n');
    let mut key = key_of(password, &salt, k)?;
    let mut nn = [0u8; NONCE];
    nn.copy_from_slice(&n);
    let ct = crate::cryptx::xchacha_seal(&key, &nn, &aad, package);
    zikaron_os::wipe(&mut key);
    let ct = ct.ok_or_else(|| Fault::known(Known::BackupShape, "seal".to_string()))?;
    let mut out = aad;
    out.extend_from_slice(&ct);
    Ok(out)
}

fn open(bytes: &[u8], password: &str) -> Result<(u64, Wiped), Fault> {
    let not_ours = || Fault::known(Known::BackupNotOurs, String::new());
    if !bytes.starts_with(MAGIC) {
        return Err(not_ours());
    }
    let rest = &bytes[MAGIC.len()..];
    let nl = rest.iter().position(|b| *b == b'\n').ok_or_else(not_ours)?;
    let header = &rest[..nl];
    let h = json::parse(header).map_err(|_| not_ours())?;
    if text(&h, "app").as_deref() != Some(APP) {
        return Err(not_ours());
    }
    match h.member("format") {
        Some(Value::Int(f)) if *f == FORMAT => {}
        Some(Value::Int(f)) if *f > FORMAT => return Err(Fault::known(Known::BackupTooNew, f.to_string())),
        _ => return Err(Fault::known(Known::BackupShape, "format".to_string())),
    }
    let created = match h.member("created") {
        Some(Value::Int(t)) => *t,
        _ => return Err(Fault::known(Known::BackupShape, "created".to_string())),
    };
    let kdf = h.member("kdf").ok_or_else(|| Fault::known(Known::BackupShape, "kdf".to_string()))?;
    let num = |k: &str| match kdf.member(k) {
        Some(Value::Int(n)) => usize::try_from(*n).ok(),
        _ => None,
    };
    let (Some(n), Some(r), Some(p)) = (num("n"), num("r"), num("p")) else {
        return Err(Fault::known(Known::BackupShape, "kdf".to_string()));
    };
    if !crate::keystore::in_range(n, r, p) {
        return Err(Fault::known(Known::BackupShape, format!("kdf n={n} r={r} p={p}")));
    }
    let salt = text(kdf, "salt").and_then(|x| unhex(&x)).ok_or_else(|| Fault::known(Known::BackupShape, "salt".to_string()))?;
    let nonce = text(&h, "nonce").and_then(|x| unhex(&x)).filter(|x| x.len() == NONCE).ok_or_else(|| Fault::known(Known::BackupShape, "nonce".to_string()))?;
    let aad = &bytes[..MAGIC.len() + nl + 1];
    let body = &bytes[MAGIC.len() + nl + 1..];
    let mut key = key_of(password, &salt, crate::keystore::Params { n, r, p })?;
    let mut nn = [0u8; NONCE];
    nn.copy_from_slice(&nonce);
    let plain = crate::cryptx::xchacha_open(&key, &nn, aad, body);
    zikaron_os::wipe(&mut key);
    let plain = plain.ok_or_else(|| Fault::known(Known::BackupPassword, String::new()))?;
    Ok((created, Wiped(plain)))
}

fn read_file(path: &Path) -> Result<Vec<u8>, Fault> {
    std::fs::read(path).map_err(|e| classify(&e, &path.display().to_string()))
}

fn collect() -> Result<Package, Fault> {
    let reg = crate::register::read()?.unwrap_or_default();
    let mut keys = Vec::new();
    for row in &reg.rows {
        let mut have: Vec<String> = Vec::new();
        match row.kind() {
            crate::identity::Kind::Words => {
                let acct = crate::places::seed_slot(&row.id);
                let b = crate::keybox::get(&acct)?.ok_or_else(|| Fault::known(Known::KeychainMissing, acct.clone()))?;
                have.push(acct.clone());
                if let Ok(e) = <[u8; crate::family::ENTROPY_BYTES]>::try_from(b.as_slice()) {
                    have.extend(crate::key::seat_slots(&e).unwrap_or_default().into_iter().map(|(a, _)| a));
                }
                keys.push(KeyOf { id: row.id.clone(), kind: "words".into(), account: acct, bytes: b });
            }
            crate::identity::Kind::Existing => {}
        }
        let needed = row.needed_accounts();
        for acct in row.accounts().into_iter().filter(|a| !have.contains(a)) {
            match crate::keybox::get(&acct)? {
                Some(b) => keys.push(KeyOf { id: row.id.clone(), kind: "key".into(), account: acct, bytes: b }),
                None if needed.contains(&acct) => return Err(Fault::known(Known::KeychainMissing, acct.clone())),
                None => {}
            }
        }
    }
    let mut files = Vec::new();
    let key = crate::keybox::local_key()?;
    for f in crate::local::all_files()?.into_iter().filter(|f| !matches!(f.doc, Doc::Registry | Doc::Checked | Doc::BackupIndex | Doc::MachineSealed | Doc::Exports) && !matches!(f.whose, Whose::Left { .. })) {
        let raw = read_file(&f.at)?;
        let bytes = crate::local::open_with(&key, &crate::local::expect_found(&f)?, &raw, &f.rel)?;
        let rel = crate::local::logical_rel(f.doc, &f.rel, &bytes)?;
        files.push(FileOf { whose: f.whose, rel, doc: f.doc, bytes });
    }
    let machine = crate::machine::to_bytes(&crate::machine::read()?)?;
    let nets_at = crate::readnets::path_in(&crate::home::machine_dir()?);
    let read_nets = match std::fs::read(&nets_at) {
        Ok(b) if crate::readnets::from_bytes(&b, "").is_ok() => Some(b),
        Ok(_) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(classify(&e, &nets_at.display().to_string())),
    };
    Ok(Package { registry: reg.to_bytes(), primary: crate::keybox::primary()?, keys, machine, files, read_nets })
}

fn summary_of(p: &Package, created: u64) -> Summary {
    let reg = crate::identity::Registry::parse(&p.registry).unwrap_or_default();
    let mut out = Summary { created, identities: reg.rows.len(), ..Summary::default() };
    for f in &p.files {
        match f.doc {
            Doc::Entry => {
                out.entries += 1;
                if zikaron_glue::book::check(&f.bytes).map(|e| e.kind == zikaron_glue::book::Kind::History).unwrap_or(false) {
                    out.records += 1;
                }
            }
            Doc::Held => out.held += 1,
            _ => {}
        }
    }
    out
}

pub fn content_now() -> Result<(usize, usize), Fault> {
    let (mut entries, mut records) = (0usize, 0usize);
    for f in crate::local::all_files()?.into_iter().filter(|f| f.doc == Doc::Entry) {
        entries += 1;
        if let Some(b) = crate::local::read(&f.at, Doc::Entry)? {
            if zikaron_glue::book::check(&b).is_ok_and(|e| e.kind == zikaron_glue::book::Kind::History) {
                records += 1;
            }
        }
    }
    Ok((entries, records))
}

pub fn count_now() -> Result<u64, Fault> {
    Ok(crate::local::all_files()?.iter().filter(|f| matches!(f.doc, Doc::Entry | Doc::Held)).count() as u64)
}

pub const INDEX_DIR: &str = "backup";
pub const INDEX_FILE: &str = "index.json";
pub const INDEX_FORM: &str = "zikaron-desk/backup-index/1";

pub fn index_rel() -> String {
    format!("{INDEX_DIR}/{INDEX_FILE}")
}

pub fn index_path() -> Result<PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(INDEX_DIR).join(INDEX_FILE))
}

fn index_bytes(at: u64, held: &[(Doc, String)]) -> Vec<u8> {
    let mut items: Vec<String> = held.iter().filter(|(d, _)| matches!(d, Doc::Entry | Doc::Held)).map(|(d, l)| format!("{}:{l}", d.tag())).collect();
    items.sort();
    items.dedup();
    json::canon_bytes(&obj(vec![("at", Value::Int(at)), ("form", s(INDEX_FORM)), ("items", Value::Arr(items.into_iter().map(Value::Str).collect()))]))
}

fn write_index(at: u64, held: &[(Doc, String)]) -> Result<(), Fault> {
    let room = crate::home::machine_dir()?.join(INDEX_DIR);
    std::fs::create_dir_all(&room).map_err(|e| classify(&e, &room.display().to_string()))?;
    crate::local::put(&room, INDEX_FILE, Doc::BackupIndex, &index_bytes(at, held))
}

fn read_index(at: u64) -> Result<Option<Vec<(Doc, String)>>, Fault> {
    let Ok(at_path) = index_path() else { return Ok(None) };
    let bytes = match crate::local::read(&at_path, Doc::BackupIndex) {
        Ok(Some(b)) => b,
        Err(f) if f.which() == Some(Known::Locked) => return Err(f),
        Ok(None) | Err(_) => return Ok(None),
    };
    let Ok(v) = json::parse(&bytes) else { return Ok(None) };
    if text(&v, "form").as_deref() != Some(INDEX_FORM) || !matches!(v.member("at"), Some(Value::Int(n)) if *n == at) {
        return Ok(None);
    }
    let Some(Value::Arr(a)) = v.member("items") else { return Ok(None) };
    Ok(a.iter()
        .map(|x| match x {
            Value::Str(t) => t.split_once(':').and_then(|(d, l)| Doc::from_tag(d).map(|d| (d, l.to_string()))),
            _ => None,
        })
        .collect())
}

pub fn measured(last: Option<&crate::machine::Backed>) -> Result<u64, Fault> {
    let index = match last {
        Some(b) => read_index(b.at)?.map(|i| (b.count, i)),
        None => None,
    };
    let Some((had, index)) = index else { return count_now() };
    let held: std::collections::BTreeSet<(&'static str, String)> = index.into_iter().map(|(d, l)| (d.tag(), l)).collect();
    let key = crate::keybox::local_key()?;
    let mut behind = 0u64;
    for f in crate::local::all_files()?.into_iter().filter(|f| matches!(f.doc, Doc::Entry | Doc::Held) && !matches!(f.whose, Whose::Left { .. })) {
        let raw = read_file(&f.at)?;
        let bytes = crate::local::open_with(&key, &crate::local::expect_found(&f)?, &raw, &f.rel)?;
        let logical = crate::local::logical_rel(f.doc, &f.rel, &bytes)?;
        if !held.contains(&(f.doc.tag(), logical)) {
            behind += 1;
        }
    }
    Ok(had.saturating_add(behind))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    pub path: PathBuf,
    pub summary: Summary,
}

pub fn export(folder: &Path, password: &str, now: u64) -> Result<Exported, Fault> {
    export_staged(folder, password, now, |at| crate::task::stage_at(crate::task::Kind::Backup, at))
}

pub fn export_staged(folder: &Path, password: &str, now: u64, mut stage: impl FnMut(u8)) -> Result<Exported, Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    stage(0);
    let p = collect()?;
    let summary = summary_of(&p, now);
    let package = Wiped(package_bytes(&p));
    stage(1);
    let sealed = seal(&package, password, now)?;
    stage(2);
    let at = crate::home::land_numbered(folder, &file_stem(now), EXT, &sealed)?;
    stage(3);
    let back = read_file(&at)?;
    let (_, again) = open(&back, password)?;
    if again.0 != package.0 {
        return Err(Fault::known(Known::BackupNotLanded, at.display().to_string()));
    }
    crate::machine::read()?;
    write_index(now, &p.files.iter().map(|f| (f.doc, f.rel.clone())).collect::<Vec<_>>())?;
    let count = count_now()?;
    crate::machine::update(|m| {
        m.backup = Some(crate::machine::Backed { at: now, path: at.display().to_string(), count });
        m.backup_failed = None;
    })?;
    Ok(Exported { path: at, summary })
}

pub fn peek(path: &Path, password: &str) -> Result<Summary, Fault> {
    let (created, plain) = open(&read_file(path)?, password)?;
    let p = package_of(&plain)?;
    Ok(summary_of(&p, created))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Carried {
    pub whose: crate::local::Whose,
    pub doc: Doc,
    pub rel: String,
    pub sha256: [u8; 32],
}

pub fn carries(path: &Path, password: &str) -> Result<Vec<Carried>, Fault> {
    let (_, plain) = open(&read_file(path)?, password)?;
    let p = package_of(&plain)?;
    Ok(p.files.iter().map(|f| Carried { whose: f.whose.clone(), doc: f.doc, rel: f.rel.clone(), sha256: zikaron::cryptox::sha256(&f.bytes) }).collect())
}

pub fn ledger_of(path: &Path, password: &str, id: &str, seat: Role) -> Result<Vec<Vec<u8>>, Fault> {
    let (_, plain) = open(&read_file(path)?, password)?;
    let p = package_of(&plain)?;
    let reg = crate::identity::Registry::parse(&p.registry).map_err(|w| Fault::known(Known::BackupShape, w))?;
    if reg.find(id).is_none() {
        return Err(Fault::known(Known::BackupNoIdentity, id.to_string()));
    }
    Ok(p.files
        .iter()
        .filter(|f| f.doc == Doc::Entry && matches!(&f.whose, Whose::Seat { id: i, seat: s } if i.eq_ignore_ascii_case(id) && *s == seat))
        .map(|f| f.bytes.clone())
        .collect())
}

fn on_disk(f: &FileOf, nk: &crate::names::NameKey) -> Result<(String, Vec<u8>), Fault> {
    let bytes = f.bytes.clone();
    let logical = crate::local::logical_rel(f.doc, &f.rel, &bytes).map_err(|_| Fault::known(Known::BackupShape, f.rel.clone()))?;
    Ok((crate::local::disk_rel(f.doc, &logical, nk).map_err(|_| Fault::known(Known::BackupShape, f.rel.clone()))?, bytes))
}

pub enum From<'a> {
    FirstRun,
    Settings(&'a str),
    Locked(&'a str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    pub summary: Summary,
}

fn fresh_home(usual: &Path) -> PathBuf {
    let free = |p: &Path| !p.exists() || std::fs::read_dir(p).map(|mut d| d.next().is_none()).unwrap_or(false);
    if free(usual) {
        return usual.to_path_buf();
    }
    let mut n = 2u32;
    loop {
        let mut s = usual.as_os_str().to_owned();
        s.push(format!("-{n}"));
        let p = PathBuf::from(s);
        if free(&p) {
            return p;
        }
        n += 1;
    }
}

pub const KEPT_HOMES: &str = "kept-homes";

fn drop_staged_in(root: &Path) {
    fn walk(d: &Path) {
        let Ok(list) = std::fs::read_dir(d) else { return };
        for e in list.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p);
            } else if crate::local::is_staged(&p.to_string_lossy()) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    walk(root);
}

pub fn restore(path: &Path, password: &str, from: From) -> Result<Restored, Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    let (created, plain) = open(&read_file(path)?, password)?;
    let p = package_of(&plain)?;
    let summary = summary_of(&p, created);
    let mut machine = if p.machine.is_empty() { crate::machine::Machine::default() } else { crate::machine::from_bytes(&p.machine).map_err(|f| Fault::known(Known::BackupShape, format!("machine · {}", f.tail())))? };
    machine.backup = Some(crate::machine::Backed { at: created, path: path.display().to_string(), count: (summary.entries + summary.held) as u64 });
    machine.backup_failed = None;
    machine.cli_alone = None;
    let mut reg = crate::identity::Registry::parse(&p.registry).map_err(|w| Fault::known(Known::BackupShape, w))?;
    reg.left.clear();
    let mut slots = Slots(Vec::new());
    let mut primary_secret: Option<Wiped> = None;
    for k in &p.keys {
        match k.kind.as_str() {
            "words" => {
                if k.bytes.len() != crate::family::ENTROPY_BYTES {
                    return Err(Fault::known(Known::BackupShape, format!("key {}", k.id)));
                }
                let mut e = [0u8; crate::family::ENTROPY_BYTES];
                e.copy_from_slice(&k.bytes);
                slots.0.push((crate::places::seed_slot(&k.id), k.bytes.clone()));
                let seats = crate::key::seat_slots(&e);
                zikaron_os::wipe(&mut e);
                slots.0.extend(seats.ok_or_else(|| Fault::known(Known::BackupShape, format!("key {}", k.id)))?);
            }
            "key" => slots.0.push((k.account.clone(), k.bytes.clone())),
            _ => return Err(Fault::known(Known::BackupShape, format!("key {}", k.id))),
        }
        if p.primary.as_ref().map(|(id, _)| id.eq_ignore_ascii_case(&k.id)).unwrap_or(false) && primary_secret.is_none() {
            primary_secret = Some(Wiped(k.bytes.clone()));
        }
    }
    let pin = match from {
        From::FirstRun => crate::keybox::PinFor::KeepFresh,
        From::Settings(p) | From::Locked(p) => crate::keybox::PinFor::New(p),
    };
    let primary = match (&p.primary, &primary_secret) {
        (Some((id, kind)), Some(sec)) => Some((id.as_str(), *kind, &sec[..])),
        _ => None,
    };
    let mut former: Vec<PathBuf> = crate::local::homes_on_record();
    match crate::local::homes() {
        Ok(h) => former.extend(h.into_iter().map(|(_, p)| p)),
        Err(f) if f.which() == Some(Known::Locked) => {}
        Err(f) => crate::local::note_trouble(f),
    }
    let nb = crate::keybox::build_new(pin, primary, &slots.0)?;
    drop(slots);
    drop(primary_secret);
    let key = nb.local_key();
    let nk = nb.name_key();
    let mut made: Vec<PathBuf> = Vec::new();
    let undo = |made: &[PathBuf]| {
        for d in made {
            let _ = std::fs::remove_dir_all(d);
        }
    };
    let staged = (|| -> Result<(), Fault> {
        crate::keybox::stage_new(&nb)?;
        let mut homes: Vec<(Whose, PathBuf)> = Vec::new();
        for row in reg.rows.iter_mut() {
            for seat in Role::ALL {
                if row.address(seat).is_none() {
                    continue;
                }
                let usual = crate::home::identity_home_under(&nk, &row.id, seat)?;
                let at = fresh_home(&usual);
                made.push(at.clone());
                crate::local::stage_plan(crate::local::PLAN_MADE, &made)?;
                crate::home::Home::open_or_create(&at)?;
                match seat {
                    Role::Author => row.author_home = at.display().to_string(),
                    Role::Grantee => row.grantee_home = at.display().to_string(),
                }
                homes.push((Whose::Seat { id: row.id.clone(), seat }, at));
            }
        }
        let mut kept: Vec<usize> = p.files.iter().filter_map(|f| if let Whose::Kept { n } = f.whose { Some(n) } else { None }).collect();
        kept.sort_unstable();
        kept.dedup();
        machine.homes.clear();
        for n in kept {
            let at = fresh_home(&crate::home::machine_dir()?.join(KEPT_HOMES).join(format!("home-{}", n + 1)));
            made.push(at.clone());
            crate::local::stage_plan(crate::local::PLAN_MADE, &made)?;
            crate::home::Home::open_or_create(&at)?;
            machine.homes.push(at.display().to_string());
            homes.push((Whose::Kept { n }, at));
        }
        let label_of = |w: &Whose| -> Result<crate::local::Label, Fault> {
            p.files
                .iter()
                .find(|f| f.whose == *w && f.doc == Doc::HomeLabel)
                .and_then(|f| crate::local::Label::parse(&f.bytes))
                .ok_or_else(|| Fault::known(Known::BackupShape, crate::local::LABEL_FILE.to_string()))
        };
        let mut owners: Vec<(Whose, crate::local::Owner)> = Vec::new();
        for (w, root) in &homes {
            if !p.files.iter().any(|f| f.whose == *w) {
                continue;
            }
            let l = label_of(w)?;
            let at = crate::local::label_path(root);
            let dir = at.parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone());
            let name = at.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            crate::home::put_at(&dir, &name, &crate::local::seal_with(&key, &crate::local::label_ident(), &l.to_bytes())?)?;
            owners.push((w.clone(), crate::local::Owner::Home(l.number)));
        }
        for f in &p.files {
            if matches!(f.whose, Whose::Machine | Whose::Loose) || f.doc == Doc::HomeLabel {
                continue;
            }
            let Some((_, root)) = homes.iter().find(|(w, _)| *w == f.whose) else { continue };
            let Some((_, owner)) = owners.iter().find(|(w, _)| *w == f.whose) else { continue };
            let (rel, bytes) = on_disk(f, &nk)?;
            let at = root.join(&rel);
            let id = crate::local::ident_in(owner.clone(), f.doc, &rel, &bytes)?;
            if f.doc == Doc::Entry {
                let name = rel.rsplit('/').next().and_then(zikaron_store::layout::parse_entry_file).ok_or_else(|| Fault::known(Known::BackupShape, f.rel.clone()))?;
                let sealed = crate::local::seal_with(&key, &id, &bytes)?;
                zikaron_store::LedgerDir::open_or_create(root.join(crate::home::Slot::Ledger.as_str()))
                    .and_then(|l| l.append(&name, &sealed))
                    .map_err(|t| Fault::known(Known::Ledger, format!("{t:?}")))?;
            } else {
                let dir = at.parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone());
                let name = at.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                crate::home::put_at(&dir, &name, &crate::local::seal_with(&key, &id, &bytes)?)?;
            }
        }
        let loose = crate::home::where_is()?;
        let is_seat = homes.iter().any(|(_, h)| crate::home::same_place(h, &loose));
        if !is_seat && p.files.iter().any(|f| f.whose == Whose::Loose) {
            let l = label_of(&Whose::Loose)?;
            let owner = crate::local::Owner::Home(l.number.clone());
            let at = crate::local::label_path(&loose);
            if let Some(d) = at.parent() {
                std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
            }
            crate::local::stage_next(&at, &key, &crate::local::label_ident(), &l.to_bytes())?;
            for f in p.files.iter().filter(|f| f.whose == Whose::Loose && f.doc != Doc::HomeLabel) {
                let (rel, bytes) = on_disk(f, &nk)?;
                let at = loose.join(&rel);
                if let Some(d) = at.parent() {
                    std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
                }
                crate::local::stage_next(&at, &key, &crate::local::ident_in(owner.clone(), f.doc, &rel, &bytes)?, &bytes)?;
            }
        }
        let m = crate::home::machine_dir()?;
        let machine_id = |doc: Doc, rel: &str| crate::local::Ident { owner: crate::local::Owner::Machine, doc, logical: rel.to_string() };
        crate::local::stage_next(&m.join(crate::places::registry_file()), &key, &machine_id(Doc::Registry, &crate::places::registry_file()), &reg.to_bytes())?;
        for f in p.files.iter().filter(|f| f.whose == Whose::Machine && f.doc != Doc::Registry) {
            crate::local::stage_next(&m.join(&f.rel), &key, &machine_id(f.doc, &f.rel), &f.bytes)?;
        }
        let room = m.join(INDEX_DIR);
        std::fs::create_dir_all(&room).map_err(|e| classify(&e, &room.display().to_string()))?;
        let held: Vec<(Doc, String)> = p.files.iter().map(|f| (f.doc, f.rel.clone())).collect();
        crate::local::stage_next(&room.join(INDEX_FILE), &key, &machine_id(Doc::BackupIndex, &index_rel()), &index_bytes(created, &held))?;
        crate::local::stage_next(&m.join(crate::machine::FILE), &key, &machine_id(Doc::Machine, crate::machine::FILE), &crate::machine::plain_bytes(&machine)?)?;
        let sealed = crate::places::machine_file();
        crate::local::stage_next(&m.join(&sealed), &key, &machine_id(Doc::MachineSealed, &sealed), &crate::machine::sealed_bytes(&machine)?)?;
        let mut roots = former.clone();
        roots.push(m.clone());
        roots.push(loose.clone());
        crate::local::stage_plan(crate::local::PLAN_ASIDE, &roots)?;
        Ok(())
    })();
    let clean = |made: &[PathBuf]| {
        undo(made);
        let _ = crate::keybox::drop_staged();
        let _ = crate::local::drop_staged_everywhere();
        if let Ok(loose) = crate::home::where_is() {
            drop_staged_in(&loose);
        }
    };
    if let Err(f) = staged {
        clean(&made);
        return Err(f);
    }
    crate::local::cut_point(crate::local::Cut::BeforeCommit)?;
    if let Err(f) = crate::keybox::commit_new(nb) {
        clean(&made);
        return Err(f);
    }
    crate::local::cut_point(crate::local::Cut::AfterCommit)?;
    if let Err(f) = crate::local::settle_pending() {
        crate::local::note_trouble(f);
    }
    if let Some(b) = &p.read_nets {
        let landed = crate::home::machine_dir().and_then(|m| crate::readnets::from_bytes(b, "readNetworks").and_then(|nets| crate::readnets::write(&m, &nets)));
        if let Err(f) = landed {
            crate::local::note_trouble(f);
        }
    }
    Ok(Restored { summary })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shade {
    Green,
    Amber,
    Grey,
    Red,
}

impl Shade {
    pub fn as_str(self) -> &'static str {
        match self {
            Shade::Green => "green",
            Shade::Amber => "amber",
            Shade::Grey => "grey",
            Shade::Red => "red",
        }
    }
}

pub fn backup_point(last: Option<&crate::machine::Backed>, now: Option<u64>, failed: Option<u64>) -> Shade {
    if failed.is_some() {
        return Shade::Red;
    }
    match (last, crate::machine::backup_behind(last, now)) {
        (None, _) => Shade::Grey,
        (Some(_), None) => Shade::Grey,
        (Some(_), Some(n)) if n > 0 => Shade::Amber,
        (Some(_), Some(_)) => Shade::Green,
    }
}

#[cfg(test)]
mod password_once {
    #[test]
    fn a_backup_password_is_judged_in_one_place() {
        use crate::secret::Secret;
        let named = |p: &str, a: &str| super::password_ok(&Secret::of(p), &Secret::of(a)).err().and_then(|f| f.which());
        assert_eq!(named("correct-horse", "correct-horse"), None);
        assert_eq!(named("correct-horse", "correct-hors"), Some(crate::fault::Known::PasswordsDiffer));
        assert_eq!(named("短短短短短短短", "短短短短短短短"), Some(crate::fault::Known::PasswordShort), "seven characters, however many bytes");
        assert_eq!(named("八个字的密码好了", "八个字的密码好了"), None, "eight characters");
        let long = "x".repeat(crate::secret::CAP + 1);
        assert_eq!(named(&long, &long), Some(crate::fault::Known::PasswordLong));
        let src = include_str!("action/identity.rs");
        assert_eq!(src.matches("crate::backup::password_ok(").count(), 2, "the backup's export and the key file's ask it");
        assert!(!src.contains("Known::PasswordShort"), "and judge it nowhere else");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_read_networks_member_is_judged_before_anything_is_placed() {
        let net = crate::readnets::cells("", "11155420", &format!("0x{}", "11".repeat(20)), "0", "https://read.example").expect("a row");
        let table = crate::readnets::bytes_of(&[net]);
        let package = |nets: Option<Vec<u8>>| super::Package { registry: b"{}".to_vec(), primary: None, keys: Vec::new(), machine: Vec::new(), files: Vec::new(), read_nets: nets };
        let read = |bytes: &[u8]| super::package_of(bytes).map(|p| p.read_nets.clone());
        assert_eq!(read(&super::package_bytes(&package(Some(table.clone())))).ok(), Some(Some(table.clone())));
        assert_eq!(read(&super::package_bytes(&package(None))).ok(), Some(None));
        let refused = |bytes: Vec<u8>| match read(&bytes) {
            Err(f) => f.said().starts_with("BACKUP_SHAPE") && f.tail().contains("readNetworks"),
            Ok(_) => false,
        };
        assert!(refused(super::package_bytes(&package(Some(b"not a table".to_vec())))), "not a table");
        let good = String::from_utf8(super::package_bytes(&package(Some(table.clone())))).expect("text");
        let hex = super::hex(&table);
        assert!(refused(good.replacen(&hex, "zz", 1).into_bytes()), "not hex");
        assert!(refused(good.replacen(&format!("\"{hex}\""), "7", 1).into_bytes()), "not text");
    }

    #[test]
    fn a_package_carrying_an_index_is_refused() {
        use crate::local::{Doc, Whose};
        let package = |doc: Doc, rel: String| super::Package {
            registry: b"{}".to_vec(),
            primary: None,
            keys: Vec::new(),
            machine: Vec::new(),
            files: vec![super::FileOf { whose: Whose::Machine, rel, doc, bytes: b"{}".to_vec() }],
            read_nets: None,
        };
        let index = super::package_bytes(&package(Doc::BackupIndex, super::index_rel()));
        match super::package_of(&index) {
            Err(f) => assert!(f.said().starts_with("BACKUP_SHAPE") && f.tail().contains("file"), "{f:?}"),
            Ok(_) => panic!("a package carrying an index was read"),
        }
        let records = format!("{}/{}", crate::recordsx::DIR, crate::recordsx::FILE);
        assert!(super::package_of(&super::package_bytes(&package(Doc::Records, records))).is_ok());
        assert_eq!(crate::local::machine_doc(&super::index_rel()), Some(Doc::BackupIndex));
    }

    #[test]
    fn a_member_is_plain_segments_on_every_system() {
        for ok in ["settings/settings.json", "ledger/0001.entry", "kits/terms/ab/name-1.json", "a.b/c..d/.e"] {
            assert!(super::plain_member(ok), "{ok}");
        }
        for bad in [
            "", "/etc/passwd", "a//b", "a/", "../x", "a/../b", "a/./b", ".", "a/...", "a/.. /b", "a/b. ", "a/x\\..\\..\\y", "a\\b", "C:/x", "a/C:x", "a/b:stream", "a/\\\\host\\share", "a/\0b",
        ] {
            assert!(!super::plain_member(bad), "{bad:?}");
        }
    }
}
