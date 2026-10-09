//! The whole-machine backup: one file, sealed with a backup password, that restores everything.
//!
//! What it holds: the identity register and every identity's key (a recovery-word identity's 16 bytes of
//! entropy, an existing-key identity's 32-byte private key), every sealed local file of this machine opened
//! (ledger entries, held grants, settings, queues, indexes…) and the machine settings. It never holds the master
//! key, the passcode or the failure count: a restore makes a new master key.
//!
//! The envelope (shared by the family's files): the first line is the magic [`MAGIC`]; the second line is a plain
//! header (format version, app name, creation time, scrypt salt and parameters, nonce); the rest is the plain
//! package sealed with XChaCha20-Poly1305 under K = scrypt(backup password, salt), with the two header lines as
//! additional data. The plain package lives only in memory. A wrong password, a file of another app and a
//! format newer than this one are each refused by name ([`Known::BackupPassword`], [`Known::BackupNotOurs`],
//! [`Known::BackupTooNew`]); none of them touches the vault or its failure count.
//!
//! The file name is `zikaron-backup-YYYY-MM-DD.zikaron` ([`file_stem`], [`EXT`]), numbered if taken. It is
//! written without ever replacing an existing file (`home::land_numbered`, 0600), then read back and opened
//! before it counts.

use crate::fault::{classify, Fault, Known};
use crate::local::{Doc, Whose};
use crate::roles::Role;
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

/// The first line of every backup (including its trailing newline).
pub const MAGIC: &[u8] = b"zikaron-backup/1\n";
/// The app named in the header.
pub const APP: &str = "zikaron-desk";
/// The format version of the plain package this version writes and reads.
pub const FORMAT: u64 = 1;
/// The file's extension.
pub const EXT: &str = "zikaron";
/// The minimum backup password length in characters, shared by the whole-machine backup and key file
/// export.
pub const PASSWORD_MIN: usize = 8;
const NONCE: usize = 24;
const SALT: usize = 32;

/// The file name stem for a backup written at `now` (UTC date): `zikaron-backup-YYYY-MM-DD`.
pub fn file_stem(now: u64) -> String {
    let (y, m, d, _, _, _) = crate::when::civil(now);
    format!("zikaron-backup-{y:04}-{m:02}-{d:02}")
}

/// What a backup holds, counted (shown on the confirmation card and the toast).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub created: u64,
    pub identities: usize,
    /// Ledger entries across every seat home.
    pub entries: usize,
    /// Records (`history` entries) among them.
    pub records: usize,
    /// Held grants across every seat home.
    pub held: usize,
}

/// One identity's key in the package.
#[derive(Clone)]
struct KeyOf {
    id: String,
    /// "words" (16 bytes of entropy) or "key" (a 32-byte private key in `account`).
    kind: String,
    account: String,
    bytes: Vec<u8>,
}

/// One local file in the package.
#[derive(Clone)]
struct FileOf {
    whose: Whose,
    rel: String,
    doc: Doc,
    bytes: Vec<u8>,
}

/// The plain package (memory only).
struct Package {
    registry: Vec<u8>,
    primary: Option<(String, crate::keybox::PrimaryKind)>,
    keys: Vec<KeyOf>,
    machine: Vec<u8>,
    files: Vec<FileOf>,
    /// The read-only networks table's bytes (machine-wide); absent in backups made by older versions, and when
    /// this machine has none.
    read_nets: Option<Vec<u8>>,
}

impl Drop for Package {
    fn drop(&mut self) {
        for k in self.keys.iter_mut() {
            zikaron_ui::secret::wipe(&mut k.bytes);
        }
    }
}

/// Bytes that hold keys (the plain package, a primary identity's secret): wiped when dropped, on every path.
struct Wiped(Vec<u8>);

impl Drop for Wiped {
    fn drop(&mut self) {
        zikaron_ui::secret::wipe(&mut self.0);
    }
}

impl std::ops::Deref for Wiped {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

/// The vault's slots being rebuilt: every secret wiped when dropped, on every path.
struct Slots(Vec<(String, Vec<u8>)>);

impl Drop for Slots {
    fn drop(&mut self) {
        for (_, b) in self.0.iter_mut() {
            zikaron_ui::secret::wipe(b);
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
        // A deleted identity's home does not travel (`collect` leaves it out); named for completeness.
        Whose::Left { id, seat } => vec![("whose", s("left")), ("id", s(id)), ("seat", s(seat.as_str()))],
    }
}

/// How the package encodes a byte share (a key, a file, the machine settings, the read-only networks table, the
/// register). Every byte share in the package goes through this single function, so the plaintext scan can
/// take the encoded form from here rather than from a copy.
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
            // Every path is checked: each segment must be one plain path component on every system ([`plain_member`])
            // and match the kind it claims, so a crafted package cannot place a file outside a home or as another kind.
            if !plain_member(&rel) {
                return Err(bad("file"));
            }
            // The register travels in its own field; a backup never holds its own index (a restore writes it).
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
    // The read-only networks table is optional (older backups have none); when present it must parse as the
    // table does, or the backup is refused by name before anything is staged.
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

/// A member path is `/`-joined segments, each exactly one plain path component on every system: not empty (so
/// no leading `/`), not ending in `.` or a space (which covers `.` and `..`, and the trailing dots and spaces
/// Windows strips, turning `.. ` into `..`), and with no other system's separator, drive or stream mark (`\`,
/// `:`) and no NUL. So no segment can be read as several steps, a drive or a parent, and no file can escape
/// the home it is restored into. Names this app writes always pass.
fn plain_member(rel: &str) -> bool {
    rel.split('/').all(|x| !x.is_empty() && !x.ends_with(['.', ' ']) && !x.contains(['\\', ':', '\0']))
}

// ───────────────────────── The envelope ─────────────────────────

fn nonce_and_salt() -> Result<(Vec<u8>, Vec<u8>), Fault> {
    Ok((crate::key::random(NONCE)?, crate::key::random(SALT)?))
}

/// The scrypt parameters a new backup is sealed with: always the standard level, deliberately not calibrated
/// to this machine, since a backup made on a fast machine must open on a slow one, where a higher level could
/// take minutes or run out of memory. The loop below never raises it: doubling N from the standard level
/// passes it immediately, so it returns the standard parameters after one timed derivation. The light
/// parameters `keybox::params` returns under the test hooks (off in normal builds) are used as is: tests
/// measure behavior, not time.
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
        // What this app writes stays within the standard level (the accepted reading bounds are wider so other
        // tools' files open; they are not a target).
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
    zikaron_ui::secret::wipe(&mut key);
    let ct = ct.ok_or_else(|| Fault::known(Known::BackupShape, "seal".to_string()))?;
    let mut out = aad;
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Reads a backup's header and opens its body. Another app's file, a newer format and a wrong password are
/// each refused by name. Nothing on this machine is read or changed.
fn open(bytes: &[u8], password: &str) -> Result<(u64, Wiped), Fault> {
    let not_ours = || Fault::known(Known::BackupNotOurs, String::new());
    // Another app's backup of the same family has the same shape with its own name in the magic and header.
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
    // Parameters out of the keystore bounds are refused before deriving (a file cannot make derivation use
    // terabytes of memory).
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
    zikaron_ui::secret::wipe(&mut key);
    let plain = plain.ok_or_else(|| Fault::known(Known::BackupPassword, String::new()))?;
    Ok((created, Wiped(plain)))
}

fn read_file(path: &Path) -> Result<Vec<u8>, Fault> {
    std::fs::read(path).map_err(|e| classify(&e, &path.display().to_string()))
}

// ───────────────────────── Export ─────────────────────────

/// Collect this machine's package (vault open: keys and local files are opened).
fn collect() -> Result<Package, Fault> {
    let reg = crate::register::read()?.unwrap_or_default();
    let mut keys = Vec::new();
    for row in &reg.rows {
        match row.kind() {
            crate::identity::Kind::Words => {
                let acct = crate::places::seed_slot(&row.id);
                let b = crate::keybox::get(&acct)?.ok_or_else(|| Fault::known(Known::KeychainMissing, acct.clone()))?;
                keys.push(KeyOf { id: row.id.clone(), kind: "words".into(), account: acct, bytes: b });
            }
            crate::identity::Kind::Existing => {
                for acct in row.accounts() {
                    let b = crate::keybox::get(&acct)?.ok_or_else(|| Fault::known(Known::KeychainMissing, acct.clone()))?;
                    keys.push(KeyOf { id: row.id.clone(), kind: "key".into(), account: acct, bytes: b });
                }
            }
        }
    }
    let mut files = Vec::new();
    let key = crate::keybox::local_key()?;
    // The register travels in its own field (its file name is this machine's, not the backup's). A deleted
    // identity's homes stay on this machine. Each file travels by its logical path (what it is), never by its
    // on-disk name (which depends on this vault's names key). The record of checked chain facts (`checkedx`)
    // stays here: an export carries no chain readings, and a restored machine asks again.
    for f in crate::local::all_files()?.into_iter().filter(|f| !matches!(f.doc, Doc::Registry | Doc::Checked | Doc::BackupIndex) && !matches!(f.whose, Whose::Left { .. })) {
        let raw = read_file(&f.at)?;
        let bytes = crate::local::open_with(&key, &crate::local::expect_found(&f)?, &raw, &f.rel)?;
        let rel = crate::local::logical_rel(f.doc, &f.rel, &bytes)?;
        files.push(FileOf { whose: f.whose, rel, doc: f.doc, bytes });
    }
    let machine = match std::fs::read(crate::machine::path()?) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(classify(&e, "machine.json")),
    };
    // The read-only networks table travels when this machine has a readable one (it is plain, like the machine
    // settings); an unreadable table stays behind, since the app already refuses it by name.
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
                if zikaron::entry::check(&f.bytes).map(|e| e.kind == zikaron::tokens::EntryType::History).unwrap_or(false) {
                    out.records += 1;
                }
            }
            Doc::Held => out.held += 1,
            _ => {}
        }
    }
    out
}

/// How many ledger entries and held grants this machine has now. Counts file names only; nothing is opened.
pub fn count_now() -> Result<u64, Fault> {
    Ok(crate::local::all_files()?.iter().filter(|f| matches!(f.doc, Doc::Entry | Doc::Held)).count() as u64)
}

/// The folder in the machine directory recording which entries and held grants the last backup holds (by
/// identity, never by location: on-disk names change with the vault's names key), and its file. The index
/// names entries and held grants by id, so it is sealed like every local file that identifies someone
/// (`local`, kind [`Doc::BackupIndex`]): no backup carries it (a restore writes its own), and it is resealed
/// under a new master key with the rest.
pub const INDEX_DIR: &str = "backup";
pub const INDEX_FILE: &str = "index.json";
/// The plain index older versions wrote beside the machine settings. Never read and never sealed in place (no
/// migration): the older counting method is used until the next backup, whose sealed index replaces it (and
/// removes the plain file).
pub const PLAIN_INDEX_FILE: &str = "backup-index.json";
/// The index's shape.
pub const INDEX_FORM: &str = "zikaron-desk/backup-index/1";

/// The sealed index's `/`-separated path inside the machine directory (how `local::machine_doc` recognizes it).
pub fn index_rel() -> String {
    format!("{INDEX_DIR}/{INDEX_FILE}")
}

/// Where the sealed index lies.
pub fn index_path() -> Result<PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(INDEX_DIR).join(INDEX_FILE))
}

/// What a backup holds, per its index: each entry and held grant by kind and logical path, with the backup's
/// time (so an index left by another backup is never mistaken for this one's).
fn index_bytes(at: u64, held: &[(Doc, String)]) -> Vec<u8> {
    let mut items: Vec<String> = held.iter().filter(|(d, _)| matches!(d, Doc::Entry | Doc::Held)).map(|(d, l)| format!("{}:{l}", d.tag())).collect();
    items.sort();
    items.dedup();
    json::canon_bytes(&obj(vec![("at", Value::Int(at)), ("form", s(INDEX_FORM)), ("items", Value::Arr(items.into_iter().map(Value::Str).collect()))]))
}

/// Writes what the last backup holds, sealed, to the machine directory ([`index_bytes`]). An existing index
/// that cannot be read is never overwritten (`local::put` refuses by name and says how to proceed). Once the
/// sealed index is written, a plain index left by an older version is removed ([`drop_plain_index`]).
fn write_index(at: u64, held: &[(Doc, String)]) -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    let room = m.join(INDEX_DIR);
    std::fs::create_dir_all(&room).map_err(|e| classify(&e, &room.display().to_string()))?;
    crate::local::put(&room, INDEX_FILE, Doc::BackupIndex, &index_bytes(at, held))?;
    drop_plain_index(&m)
}

/// Removes the plain index an older version left beside the machine settings (none is not an error).
fn drop_plain_index(m: &Path) -> Result<(), Fault> {
    let plain = m.join(PLAIN_INDEX_FILE);
    match std::fs::remove_file(&plain) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(classify(&e, &plain.display().to_string())),
    }
}

/// The index of the backup made at `at`: `None` when there is none, it belongs to another backup, or it cannot
/// be read (sealed under another key, altered, truncated, another file; an older version's plain index is
/// never consulted); the older counting method is then used. While locked this is `LOCKED` (the index cannot
/// be opened, and its contents are not guessed).
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

/// The count the "behind last backup" measure starts from (`machine::backup_behind` subtracts what the backup
/// had). For a backup that recorded its contents: what it had plus the entries and held grants this machine
/// has that it lacks, so a deleted folder no longer offsets what was recorded since. For an older record, or
/// one whose index cannot be read: this machine's current count. Files are matched by logical path (read as
/// the backup reads it), whatever their on-disk name; reading needs the vault open.
pub fn measured(last: Option<&crate::machine::Backed>) -> Result<u64, Fault> {
    let index = match last {
        Some(b) if b.indexed => read_index(b.at)?.map(|i| (b.count, i)),
        _ => None,
    };
    let Some((had, index)) = index else { return count_now() };
    let held: std::collections::BTreeSet<(&'static str, String)> = index.into_iter().map(|(d, l)| (d.tag(), l)).collect();
    let key = crate::keybox::local_key()?;
    let mut behind = 0u64;
    // A deleted identity's homes never travel in a backup (`collect`), so they are not counted against it.
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

/// What an export wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    pub path: PathBuf,
    pub summary: Summary,
}

/// Writes a backup into `folder` (vault open). The name is `zikaron-backup-YYYY-MM-DD.zikaron`, numbered if
/// taken; an existing file is never replaced (0600). The file is then read back and opened with the same
/// password, and only then counts: its time, place and count are recorded in the machine settings.
pub fn export(folder: &Path, password: &str, now: u64) -> Result<Exported, Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    let p = collect()?;
    let summary = summary_of(&p, now);
    let package = Wiped(package_bytes(&p));
    let sealed = seal(&package, password, now)?;
    // Named and written in one step that never replaces an existing file (`home::land_numbered`), then read
    // back: the file on disk must open with the password to the same package. If either step fails, that is
    // recorded on this machine (`machine::backup_failed`) before it is reported.
    let landed = (|| -> Result<PathBuf, Fault> {
        let at = crate::home::land_numbered(folder, &file_stem(now), EXT, &sealed)?;
        let back = read_file(&at)?;
        let (_, again) = open(&back, password)?;
        if again.0 != package.0 {
            return Err(Fault::known(Known::BackupNotLanded, at.display().to_string()));
        }
        Ok(at)
    })();
    let at = match landed {
        Ok(at) => at,
        Err(f) => {
            let _ = crate::machine::update(|m| m.backup_failed = Some(now));
            return Err(f);
        }
    };
    // The file was written and read back, so clear the failure flag whatever the bookkeeping below reports.
    crate::machine::update(|m| m.backup_failed = None)?;
    // Unreadable settings are refused before the index is written.
    crate::machine::read()?;
    write_index(now, &p.files.iter().map(|f| (f.doc, f.rel.clone())).collect::<Vec<_>>())?;
    let count = count_now()?;
    crate::machine::update(|m| {
        m.backup = Some(crate::machine::Backed { at: now, path: at.display().to_string(), count, indexed: true });
        m.backup_failed = None;
    })?;
    Ok(Exported { path: at, summary })
}

/// Opens a backup with its password and reports what it holds (nothing on this machine changes; works while
/// locked).
pub fn peek(path: &Path, password: &str) -> Result<Summary, Fault> {
    let (created, plain) = open(&read_file(path)?, password)?;
    let p = package_of(&plain)?;
    Ok(summary_of(&p, created))
}

/// This identity's ledger entries for one seat, from a backup (the source for fetching a restored identity's
/// ledger). The identity must be in the backup.
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

// ───────────────────────── Restore ─────────────────────────

/// A backed-up file's path on this disk under the new names key, and its bytes (an older verdict cache gets
/// its grant written inside, since its name no longer encodes it).
fn on_disk(f: &FileOf, nk: &crate::names::NameKey) -> Result<(String, Vec<u8>), Fault> {
    let bytes = if f.doc == Doc::Verdict {
        let id = f.rel.rsplit('/').next().and_then(|n| n.strip_suffix(crate::lastread::VERDICT_SUFFIX)).ok_or_else(|| Fault::known(Known::BackupShape, f.rel.clone()))?;
        crate::lastread::with_grant(&f.bytes, id)?
    } else {
        f.bytes.clone()
    };
    // The logical path comes from the bytes themselves (an older backup's paths were that machine's names).
    let logical = crate::local::logical_rel(f.doc, &f.rel, &bytes).map_err(|_| Fault::known(Known::BackupShape, f.rel.clone()))?;
    Ok((crate::local::disk_rel(f.doc, &logical, nk).map_err(|_| Fault::known(Known::BackupShape, f.rel.clone()))?, bytes))
}

/// Where a restore is started from; each seals the new vault its own way.
pub enum From<'a> {
    /// First run: the vault was made moments ago by this run's passcode and holds nothing else; its master key
    /// is kept.
    FirstRun,
    /// Settings: the passcode was just checked; the new vault is sealed with it under a new master key.
    Settings(&'a str),
    /// The locked card: a new passcode, a new master key, the failure count at zero.
    Locked(&'a str),
}

/// The result of a restore.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    pub summary: Summary,
}

/// A fresh place for a restored home: the usual place when it is free or empty, otherwise a numbered one
/// beside it (a home already on this machine is never written into: its files are sealed under the old key).
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

/// Where a restore puts the data folders the backed-up machine had previously opened (under the machine
/// directory).
pub const KEPT_HOMES: &str = "kept-homes";

/// Removes every staged file under one directory (a restore that did not take effect).
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

/// Restores from a backup: identities, keys and local data are replaced by the backup's. Everything is staged
/// first (new homes written in fresh places, machine files staged beside theirs, the new vault staged beside
/// the vault); renaming the vault is the single moment it takes effect, and staged files settle after it. A
/// failure before that leaves the machine unchanged (the new homes are removed); an interruption after it is
/// settled on the next unlock (`local::settle_pending`).
pub fn restore(path: &Path, password: &str, from: From) -> Result<Restored, Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    let (created, plain) = open(&read_file(path)?, password)?;
    let p = package_of(&plain)?;
    let summary = summary_of(&p, created);
    // The backup's machine settings are validated before anything is staged: settings written by a newer
    // version, or malformed ones, are refused here rather than staged to fail at every unlock. The restored
    // settings record this backup as the last one (a backup cannot hold its own record).
    let mut machine = if p.machine.is_empty() { crate::machine::Machine::default() } else { crate::machine::from_bytes(&p.machine).map_err(|f| Fault::known(Known::BackupShape, format!("machine · {}", f.tail())))? };
    machine.backup = Some(crate::machine::Backed { at: created, path: path.display().to_string(), count: (summary.entries + summary.held) as u64, indexed: true });
    // The last-backup record is this backup: a failed attempt the backed-up machine had recorded
    // (`backup_failed`) is not carried over.
    machine.backup_failed = None;
    let mut reg = crate::identity::Registry::parse(&p.registry).map_err(|w| Fault::known(Known::BackupShape, w))?;
    // The homes the backed-up machine's deleted identities left are that machine's, not this one's.
    reg.left.clear();
    // The vault's slots: recovery-word identities derive both seat keys from their entropy again.
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
                zikaron_ui::secret::wipe(&mut e);
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
    // This machine's homes before the change (seat homes, the resolved home, folders it opened): whatever the
    // new key cannot open is moved aside once the change takes effect. The plain records and the layout find them
    // all (`local::homes_on_record`); the register adds any home it names elsewhere when it can be read. While
    // locked (from the lock card) it cannot be read, and never will be under the key being replaced, which is
    // expected; any other read failure is reported, never treated as an empty list.
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
    // Names on disk are keyed by the new vault's names key from here on.
    let nk = nb.name_key();
    // New homes in fresh places, written directly (nothing refers to them until the vault changes).
    let mut made: Vec<PathBuf> = Vec::new();
    let undo = |made: &[PathBuf]| {
        for d in made {
            let _ = std::fs::remove_dir_all(d);
        }
    };
    // The staged vault is written first: while it is staged the change has not taken effect, whatever else is
    // staged (`local::settle_pending`), so an interruption anywhere before the rename leaves this machine
    // unchanged.
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
                // Recorded in the plan before it is created: an interruption from here on removes it at the next
                // unlock.
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
        // The data folders the backed-up machine had opened come back in fresh places under this machine
        // directory, remembered as this machine's (they can be reopened from settings).
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
        // Each home keeps its label: the backup's, or (for a backup from an older version) a new one, written into
        // the home with the rest. Every file of a home is sealed as that home's.
        let label_of = |w: &Whose| -> Result<crate::local::Label, Fault> {
            match p.files.iter().find(|f| f.whose == *w && f.doc == Doc::HomeLabel).and_then(|f| crate::local::Label::parse(&f.bytes)) {
                Some(l) => Ok(l),
                None => crate::local::Label::new(match w {
                    Whose::Seat { id, seat } => Some((id.clone(), *seat)),
                    _ => None,
                }),
            }
        };
        let mut owners: Vec<(Whose, crate::local::Owner)> = Vec::new();
        for (w, root) in &homes {
            // A home the backup holds nothing for (a seat never opened) stays empty, without a label.
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
        // The home this machine resolves to without an identity stays where it is (the pointer is unchanged): the
        // backup's files for it are staged beside its own; its files the backup lacks stay sealed under the old key
        // and are moved aside once the change takes effect.
        let loose = crate::home::where_is()?;
        let is_seat = homes.iter().any(|(_, h)| crate::home::same_place(h, &loose));
        if !is_seat && p.files.iter().any(|f| f.whose == Whose::Loose) {
            // Its label is staged with its files: the backup's, or a new one (a backup with no files for it stages
            // nothing).
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
        // Machine files, staged beside theirs: the register (pointing at the new homes), the other sealed
        // machine files, and the machine settings.
        let m = crate::home::machine_dir()?;
        let machine_id = |doc: Doc, rel: &str| crate::local::Ident { owner: crate::local::Owner::Machine, doc, logical: rel.to_string() };
        crate::local::stage_next(&m.join(crate::places::registry_file()), &key, &machine_id(Doc::Registry, &crate::places::registry_file()), &reg.to_bytes())?;
        for f in p.files.iter().filter(|f| f.whose == Whose::Machine && f.doc != Doc::Registry) {
            crate::local::stage_next(&m.join(&f.rel), &key, &machine_id(f.doc, &f.rel), &f.bytes)?;
        }
        // The restored machine holds what this backup holds: its index is staged with the other machine files under
        // the new key, replacing this machine's index (sealed under the old key) in the same settle; an interrupted
        // settle completes it at the next unlock.
        let room = m.join(INDEX_DIR);
        std::fs::create_dir_all(&room).map_err(|e| classify(&e, &room.display().to_string()))?;
        let held: Vec<(Doc, String)> = p.files.iter().map(|f| (f.doc, f.rel.clone())).collect();
        crate::local::stage_next(&room.join(INDEX_FILE), &key, &machine_id(Doc::BackupIndex, &index_rel()), &index_bytes(created, &held))?;
        crate::local::stage_next(&m.join(crate::machine::FILE), &key, &machine_id(Doc::Machine, crate::machine::FILE), &crate::machine::to_bytes(&machine)?)?;
        // What this machine had that the backup lacks (former homes, the kept home's other files, machine files)
        // stays sealed under the old key and is planned to be moved aside intact once the change takes effect, never
        // deleted.
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
    // The change has taken effect; settling moves the staged files into place and carries out the plan (moving
    // aside what the new key cannot open). A staged file that cannot be placed now is placed at the next unlock,
    // and an interruption before this line leaves the whole pass to the next unlock.
    if let Err(f) = crate::local::settle_pending() {
        crate::local::note_trouble(f);
    }
    // The restored index was placed with the rest (staged above); remove any plain index an older version left.
    if let Err(f) = crate::home::machine_dir().and_then(|m| drop_plain_index(&m)) {
        crate::local::note_trouble(f);
    }
    // The read-only networks table goes back where the machine keeps it (after settling, which moves aside what
    // the new key cannot open; the table is plain). A backup without one leaves this machine's as it is.
    if let Some(b) = &p.read_nets {
        let landed = crate::home::machine_dir().and_then(|m| crate::readnets::from_bytes(b, "readNetworks").and_then(|nets| crate::readnets::write(&m, &nets)));
        if let Err(f) = landed {
            crate::local::note_trouble(f);
        }
    }
    Ok(Restored { summary })
}

#[cfg(test)]
mod tests {
    /// The package's read-only networks member is validated before anything is placed: a table travels and
    /// reads back byte for byte; none (an older backup) reads as none; one that is not hex, not text, or not a
    /// table refuses the whole backup by name.
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

    /// A backup never holds its own index (a restore writes the restored machine's): a package that carries a
    /// machine file of the index's kind at the index's place is refused whole, by name; the same package with a
    /// machine file of another kind at its own place reads.
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
        // The index's kind and place are the machine directory's one sealed index (`local::machine_doc`).
        assert_eq!(crate::local::machine_doc(&super::index_rel()), Some(Doc::BackupIndex));
        assert_eq!(crate::local::machine_doc(super::PLAIN_INDEX_FILE), None, "the plain file is never sealed in place");
    }

    /// Each segment is one plain component on every system: the forms another system would read as a parent,
    /// a drive, a stream or a second step are refused here, wherever this runs.
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
