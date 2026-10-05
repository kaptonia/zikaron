//! Mirror and restore: export a bundle; on import re-verify every entry through the core; only a COMPLETE
//! anchor reconciliation releases the pen.
//!
//! A bundle is a directory with a manifest (`mirror.json`) and an `entries/` room. The manifest records each
//! entry's name, digest and size; the name is the entry id (`entry_id`, law §2.1), computed, never copied.
//!
//! A bundle comes from elsewhere. A matching digest only shows it was not altered in transit, not that it is
//! a valid entry, and restore poisoning is the attack to defend against. So every entry goes through the
//! core's `entry::check` (thirteen steps) and its `entry_id` is computed and compared with the file name; if
//! any of digest, law or identity fails, the whole bundle is refused. Taking the good entries and skipping
//! the bad ones would produce a ledger nobody can vouch for.
//!
//! After a restore the home holds the pen: writing is allowed again only after an anchor reconciliation
//! (scan, assemble, core report) returns `COMPLETE`. Any other label (UNAVAILABLE / GAPS / BROKEN_CHAIN)
//! keeps the pen held and is shown unchanged, never leaning toward green.

use crate::fault::{classify, Fault, Known};
use crate::home::Home;
use zikaron::entry as k1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron_store::EntryName;

/// The bundle's layout (manifest name, entries room, kind and version) is named once in the glue crate, where
/// the command line reads mirrors by the same names.
pub use zikaron_glue::mirror::{ENTRIES, KIND, MANIFEST, VERSION};
/// The name of the bundle inside a folder, defined once: first run, settings and the test hooks all call
/// [`bundle_in`].
pub const STEM: &str = "ZIKARON-backup";
/// Where the vault room goes inside a bundle (the mirror also covers `grants-held/`).
pub const HELD: &str = "held";

/// Reading after writing a bundle.
pub struct Made {
    pub root: std::path::PathBuf,
    pub entries: usize,
    pub bytes: u64,
    /// How many entries this pass added (a new bundle equals `entries`).
    pub added: usize,
    /// Whether this pass topped up an old bundle or wrote a new one.
    pub topped_up: bool,
}

/// One entry's reading in a bundle.
pub struct Row {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

/// A bundle's manifest.
pub struct Sheet {
    pub kind: String,
    pub version: u64,
    /// Which ledger this bundle belongs to (the genesis author; law §4.2: one ledger, one root). Empty in
    /// older bundles without this field.
    pub root: String,
    pub rows: Vec<Row>,
    /// Vault room files (relative path, digest, size); empty in older bundles.
    pub held: Vec<Row>,
    /// Whose bundle this is when it has no ledger side (the current identity's id). Bundles with a ledger are
    /// recognized by root, and this stays empty; older bundles without it are empty too and are still
    /// recognized (bundles written earlier are never read as someone else's).
    pub owner: String,
}

fn sha_hex(b: &[u8]) -> String {
    hexfmt::encode(&zikaron::cryptox::sha256(b)).trim_start_matches("0x").to_string()
}


/// Where one holder's seat bundle lands inside a folder: `<chosen>/ZIKARON-backup/<address>/<seat>`. The only
/// place this name is built (first run, settings and the test hooks take it from here). A second address or seat
/// backed up into the same folder gets its own bundle. Empty or relative is refused at once
/// (`PATH_RELATIVE`), instead of writing into the current directory and reporting success.
pub fn bundle_in(folder: &std::path::Path, holder: &str, seat: crate::roles::Role) -> Result<std::path::PathBuf, Fault> {
    crate::home::landing(&folder.to_string_lossy())?;
    let who = holder.trim().trim_start_matches("0x").to_ascii_lowercase();
    if who.is_empty() {
        return Err(Fault::known(Known::KeychainMissing, crate::lang::t(crate::lang::Key::TailBackupNoHolder).to_string()));
    }
    Ok(folder.join(STEM).join(who).join(seat.as_str()))
}

/// Which chosen folder a bundle belongs to: three levels up in the new layout (`<seat>`, `<address>`,
/// `ZIKARON-backup`), one level up in the old. The settings backup path shows the chosen folder by it (backup
/// and restore read the path the same way).
pub fn folder_of(bundle: &std::path::Path) -> std::path::PathBuf {
    let up = |p: &std::path::Path| p.parent().map(|x| x.to_path_buf()).unwrap_or_default();
    let stem_at = up(&up(bundle));
    if stem_at.file_name().map(|n| n == STEM).unwrap_or(false) {
        up(&stem_at)
    } else {
        up(bundle)
    }
}


/// What is at that place. The rule for "occupied" lives here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Belongs {
    /// Nothing there, or empty: write a new bundle.
    Empty,
    /// An earlier bundle of this ledger, with how many entries it has now.
    Ours { entries: usize },
    /// A bundle of another ledger (that ledger's root).
    Other { root: String },
    /// Something that is not a bundle (unreadable or malformed manifest, root unreadable).
    Junk { why: String },
}

/// A bundle's root: from the manifest when written there (new bundles always write it); older bundles compute
/// it entry by entry (the same source as the ledger, `auditx::root_of`).
///
/// A bundle with no entries has no root (empty string): the grantee side often has no ledger, and its vault
/// files still need backing up.
fn bundle_root(bundle: &std::path::Path, sheet: &Sheet) -> Result<String, Fault> {
    if !sheet.root.is_empty() {
        return Ok(sheet.root.clone());
    }
    if sheet.rows.is_empty() {
        return Ok(String::new());
    }
    let items: Vec<Vec<u8>> = verify(bundle)?.into_iter().map(|(_, b)| b).collect();
    crate::auditx::root_of(&items)
}

/// What is at this place: nothing or empty is an empty place; a readable manifest with this ledger's root is
/// an earlier bundle of this ledger; another root is another ledger's bundle; anything else (content with an
/// unreadable or malformed manifest) is junk. Each form is named, so the screen can say which.
pub fn belongs(bundle: &std::path::Path, our_root: &str) -> Belongs {
    if !bundle.exists() {
        return Belongs::Empty;
    }
    let empty = std::fs::read_dir(bundle).map(|mut d| d.next().is_none()).unwrap_or(false);
    if empty {
        return Belongs::Empty;
    }
    let sheet = match inspect(bundle) {
        Ok(s) => s,
        Err(f) => return Belongs::Junk { why: f.said().to_string() },
    };
    if let Some(why) = sheet.shape_trouble() {
        return Belongs::Junk { why };
    }
    match bundle_root(bundle, &sheet) {
        // Neither side has a root (no ledger yet), so ask whose it is: the grantee side backs up only the
        // vault, the bundle holds no ledger, and two people's bundles in one folder look identical. Treating
        // it as ours would rewrite the manifest from this machine's vault on top-up, and the other person's
        // files would stay on disk while disappearing from the manifest (unrecoverable at restore), silently.
        Ok(root) if root.is_empty() && our_root.is_empty() => match (sheet.owner.as_str(), our_owner()) {
            // Older bundles lack this field: recognized as ours as before (the new field does not turn
            // earlier bundles into someone else's).
            ("", _) => Belongs::Ours { entries: sheet.rows.len() },
            (had, Some(now)) if had.eq_ignore_ascii_case(&now) => Belongs::Ours { entries: sheet.rows.len() },
            // This recognizes whose bundle, not which ledger; the tail says so, so the screen does not read
            // an identity as a ledger root.
            (had, _) => Belongs::Other { root: format!("身份 {had}") },
        },
        Ok(root) if root.eq_ignore_ascii_case(our_root) => Belongs::Ours { entries: sheet.rows.len() },
        Ok(root) => Belongs::Other { root },
        Err(f) => Belongs::Junk { why: f.said().to_string() },
    }
}

/// The id of the identity on this machine now (`None` when unreadable: then only roots are compared, as
/// before).
fn our_owner() -> Option<String> {
    crate::register::now_row_listed().ok().flatten().map(|(row, _)| row.id)
}

/// This ledger's root (the genesis author). Empty with no root yet: the grantee side often has no ledger but
/// its vault still needs backing up; an empty root matches only bundles that also have none (see
/// [`belongs`]).
fn our_root(home: &Home) -> Result<String, Fault> {
    let survey = home
        .ledger()?
        .survey()?;
    if survey.items.is_empty() {
        return Ok(String::new());
    }
    crate::auditx::root_of(&survey.items)
}

/// Write a bundle, or top up an old one.
///
/// The location must be a full path: empty or relative is refused at once. An empty place gets a new bundle;
/// an earlier bundle of this ledger is compared entry by entry by digest, new entries are added and a new
/// manifest written; another ledger's bundle or junk is refused, saying which.
/// `_pass` is the exit gate's [`crate::exitgate::Pass`]: there is no way to this effect but through the gate.
pub fn export(_pass: &crate::exitgate::Pass, home: &Home, out: &std::path::Path, _now: u64) -> Result<Made, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::H4);
    crate::home::landing(&out.to_string_lossy())?;
    let root = our_root(home)?;
    // Top-up or new bundle depends on whether the place has a recognized manifest, not on how many entries it
    // lists (a vault-only backup always has zero entries, and that is still a top-up).
    let onto_old = match belongs(out, &root) {
        Belongs::Empty => false,
        Belongs::Ours { .. } => true,
        Belongs::Other { root } => return Err(Fault::known(Known::MirrorOther, root)),
        Belongs::Junk { why } => return Err(Fault::known(Known::Occupied, why)),
    };
    let ledger = home.ledger()?;
    let survey = ledger
        .survey()?;
    let dir = out.join(ENTRIES);
    std::fs::create_dir_all(&dir).map_err(|e| classify(&e, &dir.display().to_string()))?;
    let mut rows: Vec<Value> = Vec::new();
    let mut total = 0u64;
    let mut added = 0usize;
    // Storage returns entry bytes, not names; names are computed as the law §2.1 id.
    for bytes in &survey.items {
        let name = hexfmt::encode(&k1::entry_id(bytes));
        let name = name.trim_start_matches("0x").to_string();
        total += bytes.len() as u64;
        // Compare against the file on disk, not the old manifest row (as for the vault room). The manifest is
        // only a claim: a deleted file, a copy that missed files or a previous pass cut off before the
        // manifest was replaced all leave it unchanged. Skipping by it would produce a bundle with missing
        // files (found only at restore); and a file on disk that the manifest lacks would, with a direct
        // write, hit "already there" and be refused every time.
        let sha = sha_hex(bytes);
        let at = dir.join(&name);
        let have = std::fs::read(&at).map(|b| sha_hex(&b) == sha).unwrap_or(false);
        if !have {
            if at.exists() {
                std::fs::remove_file(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
            }
            zikaron_glue::landing::land_bytes(&at, bytes)
                .map_err(|t| Fault::of_landing(t))?;
            added += 1;
        }
        rows.push(Value::Obj(vec![
            ("bytes".into(), Value::Int(bytes.len() as u64)),
            ("name".into(), Value::Str(name)),
            ("sha256".into(), Value::Str(sha)),
        ]));
    }
    // The vault room travels too (bytes are credentials; losing the vault is losing the contracts). Each file
    // lands under `held/` by relative path with its digest in the manifest; a home without that room gives an
    // empty table and older bundles read as before. Vault files change (new grants, replaced files), so each
    // is compared by digest: matching files are not rewritten, differing ones are replaced.
    let mut held_rows: Vec<Value> = Vec::new();
    for (rel, bytes) in crate::vaultx::held_rows(home)? {
        let at = out.join(HELD).join(&rel);
        if let Some(d) = at.parent() {
            std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
        }
        total += bytes.len() as u64;
        let sha = sha_hex(&bytes);
        let same = std::fs::read(&at).map(|b| sha_hex(&b) == sha).unwrap_or(false);
        if !same {
            if at.exists() {
                std::fs::remove_file(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
            }
            zikaron_glue::landing::land_bytes(&at, &bytes)
                .map_err(|t| Fault::of_landing(t))?;
        }
        held_rows.push(Value::Obj(vec![
            ("bytes".into(), Value::Int(bytes.len() as u64)),
            ("path".into(), Value::Str(rel)),
            ("sha256".into(), Value::Str(sha)),
        ]));
    }
    // Bundles with a root are recognized by it and `owner` stays empty; without a root this identity is
    // recorded so two people's bundles in one place do not overwrite each other.
    let owner = if root.is_empty() { our_owner().unwrap_or_default() } else { String::new() };
    let mut sheet_fields = vec![
        ("entries".into(), Value::Arr(rows)),
        ("held".into(), Value::Arr(held_rows)),
        ("kind".into(), Value::Str(KIND.to_string())),
        ("root".into(), Value::Str(root)),
        ("version".into(), Value::Int(VERSION)),
    ];
    if !owner.is_empty() {
        sheet_fields.push(("owner".into(), Value::Str(owner)));
        sheet_fields.sort_by(|a: &(String, Value), b: &(String, Value)| a.0.cmp(&b.0));
    }
    let sheet = Value::Obj(sheet_fields);
    // Replace the manifest: the old one is overwritten (the manifest defines the bundle, and after a top-up
    // it describes the topped-up bundle).
    let manifest = out.join(MANIFEST);
    if manifest.exists() {
        std::fs::remove_file(&manifest).map_err(|e| classify(&e, &manifest.display().to_string()))?;
    }
    zikaron_glue::landing::land_bytes(&manifest, &json::canon_bytes(&sheet))
        .map_err(|t| Fault::of_landing(t))?;
    Ok(Made {
        root: out.to_path_buf(),
        entries: survey.items.len(),
        bytes: total,
        added,
        topped_up: onto_old,
    })
}

/// Read a bundle's manifest; unreadable is refused by name.
pub fn inspect(bundle: &std::path::Path) -> Result<Sheet, Fault> {
    let p = bundle.join(MANIFEST);
    let bytes = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
    // The manifest is read by the one reading the command line shares (`zikaron_glue::mirror::sheet`).
    let s = zikaron_glue::mirror::sheet(&bytes)
        .map_err(|t| Fault::known(Known::MirrorShape, crate::lang::filln(crate::lang::Key::Tail185, &[&(MANIFEST).to_string(), &format!("{:?}", t)])))?;
    let row = |r: zikaron_glue::mirror::Row| Row { name: r.name, sha256: r.sha256, bytes: r.bytes };
    Ok(Sheet {
        kind: s.kind,
        version: s.version,
        root: s.root,
        rows: s.rows.into_iter().map(row).collect(),
        held: s.held.into_iter().map(row).collect(),
        owner: s.owner,
    })
}

impl Sheet {
    /// Whether the manifest's shape is right, named row by row.
    pub fn shape_trouble(&self) -> Option<String> {
        if self.kind != KIND {
            return Some(crate::lang::filln(crate::lang::Key::Tail186, &[&format!("{:?}", self.kind), &(KIND).to_string()]));
        }
        if self.version != VERSION {
            return Some(crate::lang::filln(crate::lang::Key::Tail187, &[&(self.version).to_string(), &(VERSION).to_string()]));
        }
        for r in &self.rows {
            if EntryName::parse(&r.name).is_none() {
                return Some(crate::lang::filln(crate::lang::Key::Tail188, &[&format!("{:?}", r.name)]));
            }
            if r.sha256.len() != 64 {
                return Some(crate::lang::filln(crate::lang::Key::Tail189, &[&(r.name).to_string()]));
            }
        }
        for r in &self.held {
            // Relative paths that stay inside the room (`..`, absolute paths and empty segments are
            // malformed).
            if r.name.is_empty()
                || r.name.starts_with('/')
                || r.name.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..")
            {
                return Some(crate::lang::filln(crate::lang::Key::Tail190, &[&format!("{:?}", r.name)]));
            }
            if r.sha256.len() != 64 {
                return Some(crate::lang::filln(crate::lang::Key::Tail189, &[&(r.name).to_string()]));
            }
        }
        None
    }
}

/// Re-verify a bundle entry by entry. An entry counts only when digest, law (the core's thirteen steps) and
/// identity (computed `entry_id` equals the file name) all hold. Any failure refuses the whole bundle, naming
/// the entry and what failed.
pub fn verify(bundle: &std::path::Path) -> Result<Vec<(EntryName, Vec<u8>)>, Fault> {
    let sheet = inspect(bundle)?;
    if let Some(why) = sheet.shape_trouble() {
        return Err(Fault::known(Known::MirrorShape, why));
    }
    let dir = bundle.join(ENTRIES);
    let mut out = Vec::new();
    for r in &sheet.rows {
        let p = dir.join(&r.name);
        let bytes = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
        if bytes.len() as u64 != r.bytes {
            return Err(Fault::known(
                Known::MirrorEntry,
                crate::lang::filln(crate::lang::Key::Tail191, &[&(r.name).to_string(), &(bytes.len()).to_string(), &(r.bytes).to_string()]),
            ));
        }
        let got = sha_hex(&bytes);
        if got != r.sha256 {
            return Err(Fault::known(Known::MirrorEntry, crate::lang::filln(crate::lang::Key::Tail192, &[&(r.name).to_string()])));
        }
        // Through the core: the core runs the thirteen steps, and the refusal is the law's token, not a
        // sentence made up here.
        if let Err(token) = k1::check(&bytes) {
            return Err(Fault::mirror_entry(&r.name, token));
        }
        let id = hexfmt::encode(&k1::entry_id(&bytes));
        let id = id.trim_start_matches("0x");
        if id != r.name {
            return Err(Fault::known(
                Known::MirrorEntry,
                crate::lang::filln(crate::lang::Key::Tail194, &[&(r.name).to_string(), &(id).to_string()]),
            ));
        }
        let name = EntryName::parse(&r.name)
            .ok_or_else(|| Fault::known(Known::MirrorShape, crate::lang::filln(crate::lang::Key::Tail188, &[&(r.name).to_string()])))?;
        out.push((name, bytes));
    }
    Ok(out)
}

/// The state of the last bundle written now. Four states.
///
/// Where a mirror goes is the person's choice, so this answers from the last action's reading: whether one
/// was written, whether it is still there, whether it still verifies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mirrored {
    /// Never written.
    Never,
    /// Written, and nothing is at that place now.
    Gone { path: String },
    /// Written, still there, and it fails verification.
    Bad { path: String, why: String },
    /// Written, still there, verifies, with how many entries.
    At { path: String, at: u64, entries: usize },
}

/// Ask about the last bundle written. This reads the disk (verifying the whole bundle), so it runs in the
/// background, never in the frame.
pub fn status(rec: Option<&crate::settings::MirrorRecord>) -> Mirrored {
    let Some(r) = rec else {
        return Mirrored::Never;
    };
    let at = std::path::Path::new(&r.path);
    if !at.is_dir() {
        return Mirrored::Gone { path: r.path.clone() };
    }
    match verify(at) {
        Ok(rows) => Mirrored::At { path: r.path.clone(), at: r.at, entries: rows.len() },
        Err(f) => Mirrored::Bad { path: r.path.clone(), why: f.said().to_string() },
    }
}
