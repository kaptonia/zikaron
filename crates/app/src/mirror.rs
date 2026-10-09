//! Mirror backups: export a bundle, and on import re-verify every entry through the core.
//!
//! A bundle is a directory with a manifest (`mirror.json`) and an `entries/` directory. The manifest records
//! each entry's name, digest and size; the name is the computed entry id (`entry_id`, law §2.1),
//! never copied.
//!
//! A bundle comes from elsewhere. A matching digest only shows it was not altered in transit, not that it is
//! a valid entry, and restore poisoning is the attack to defend against. So every entry goes through the
//! core's `entry::check` and its computed `entry_id` must equal the file name; if the digest, validity or id
//! check fails for any entry, the whole bundle is refused. Keeping the good entries and skipping the bad ones
//! would produce a ledger nobody can vouch for.
//!
//! After a restore, writing stays disabled until an anchor reconciliation (scan, assemble, core report)
//! returns `COMPLETE`. Any other label (UNAVAILABLE / GAPS / BROKEN_CHAIN) keeps writing disabled and is shown
//! as is, never rounded up to success.

use crate::fault::{classify, Fault, Known};
use crate::home::Home;
use zikaron::entry as k1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron_store::EntryName;

/// Bundle layout constants (manifest name, entries directory, kind and version), shared with the command line
/// through the glue crate.
pub use zikaron_glue::mirror::{ENTRIES, KIND, MANIFEST, VERSION};
/// The bundle directory name inside the chosen folder (see [`bundle_in`]).
pub const STEM: &str = "ZIKARON-backup";
/// Where the vault (`grants-held/`) goes inside a bundle.
pub const HELD: &str = "held";

/// The result of writing a bundle.
pub struct Made {
    pub root: std::path::PathBuf,
    pub entries: usize,
    pub bytes: u64,
    /// How many entries this export added (equal to `entries` for a new bundle).
    pub added: usize,
    /// Whether this export topped up an existing bundle rather than writing a new one.
    pub topped_up: bool,
}

/// One manifest row.
pub struct Row {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

/// A bundle's manifest.
pub struct Sheet {
    pub kind: String,
    pub version: u64,
    /// Which ledger this bundle belongs to (the genesis author; one ledger, one root, law §4.2). Empty
    /// in bundles from older versions.
    pub root: String,
    pub rows: Vec<Row>,
    /// Vault files (relative path, digest, size); empty in bundles from older versions.
    pub held: Vec<Row>,
    /// The owning identity's id, for bundles without a ledger. Bundles with a ledger are recognized by root and
    /// leave this empty; bundles from older versions also lack it and are still treated as the user's own.
    pub owner: String,
}

fn sha_hex(b: &[u8]) -> String {
    hexfmt::encode(&zikaron::cryptox::sha256(b)).trim_start_matches("0x").to_string()
}


/// Where a holder's bundle for one role goes inside a chosen folder: `<chosen>/ZIKARON-backup/<address>/<seat>`,
/// so different addresses or roles backed up to one folder get separate bundles. An empty or relative folder
/// is refused at once (`PATH_RELATIVE`) instead of writing into the current directory.
pub fn bundle_in(folder: &std::path::Path, holder: &str, seat: crate::roles::Role) -> Result<std::path::PathBuf, Fault> {
    crate::home::landing(&folder.to_string_lossy())?;
    let who = holder.trim().trim_start_matches("0x").to_ascii_lowercase();
    if who.is_empty() {
        return Err(Fault::known(Known::KeychainMissing, crate::lang::t(crate::lang::Key::TailBackupNoHolder).to_string()));
    }
    Ok(folder.join(STEM).join(who).join(seat.as_str()))
}

/// The chosen folder a bundle is in: three levels up in the current layout (`<seat>`, `<address>`,
/// `ZIKARON-backup`), one level up in the old one. Used to show the backup folder in settings.
pub fn folder_of(bundle: &std::path::Path) -> std::path::PathBuf {
    let up = |p: &std::path::Path| p.parent().map(|x| x.to_path_buf()).unwrap_or_default();
    let stem_at = up(&up(bundle));
    if stem_at.file_name().map(|n| n == STEM).unwrap_or(false) {
        up(&stem_at)
    } else {
        up(bundle)
    }
}


/// What is at a bundle path; this defines "occupied".
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

/// A bundle's root: from the manifest when present (current bundles always write it), otherwise computed from
/// the entries as for the ledger (`auditx::root_of`).
///
/// A bundle with no entries has no root (empty string): grantees often have no ledger but still need their
/// vault backed up.
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

/// Classify what is at `bundle`: missing or empty is `Empty`; a valid manifest with this ledger's root is
/// `Ours`; another root is `Other`; anything else (unreadable or malformed manifest) is `Junk`, so the UI can
/// say which.
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
        // Neither side has a root (no ledger), so compare owners: vault-only bundles of two people in one
        // folder look identical. Treating another's as ours would rewrite its manifest from this vault on
        // top-up, silently dropping the other person's files from the manifest (unrecoverable at restore).
        Ok(root) if root.is_empty() && our_root.is_empty() => match (sheet.owner.as_str(), our_owner()) {
            // Bundles from older versions lack `owner` and are still treated as ours.
            ("", _) => Belongs::Ours { entries: sheet.rows.len() },
            (had, Some(now)) if had.eq_ignore_ascii_case(&now) => Belongs::Ours { entries: sheet.rows.len() },
            // This is an identity, not a ledger root; the prefix makes that clear in the UI.
            (had, _) => Belongs::Other { root: format!("身份 {had}") },
        },
        Ok(root) if root.eq_ignore_ascii_case(our_root) => Belongs::Ours { entries: sheet.rows.len() },
        Ok(root) => Belongs::Other { root },
        Err(f) => Belongs::Junk { why: f.said().to_string() },
    }
}

/// The current identity's id (`None` when unreadable; then only roots are compared).
fn our_owner() -> Option<String> {
    crate::register::now_row_listed().ok().flatten().map(|(row, _)| row.id)
}

/// This ledger's root (the genesis author), or empty when there is no ledger yet (common for grantees, whose
/// vault still needs backing up). An empty root matches only bundles that also have none (see [`belongs`]).
fn our_root(home: &Home) -> Result<String, Fault> {
    let survey = home
        .ledger()?
        .survey()?;
    if survey.items.is_empty() {
        return Ok(String::new());
    }
    crate::auditx::root_of(&survey.items)
}

/// Write a bundle, or top up an existing one.
///
/// `out` must be an absolute path. An empty location gets a new bundle; an earlier bundle of this ledger is
/// compared file by file by digest, missing entries are added and a new manifest is written; another ledger's
/// bundle or junk is refused with the reason. `_pass` ([`crate::exitgate::Pass`]) ensures this is only
/// reachable through the exit gate.
pub fn export(_pass: &crate::exitgate::Pass, home: &Home, out: &std::path::Path, _now: u64) -> Result<Made, Fault> {
    // Traced here so direct calls that bypass `apply` (tests, the CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::H4);
    crate::home::landing(&out.to_string_lossy())?;
    let root = our_root(home)?;
    // Top-up vs. new depends on a recognized manifest, not on the entry count (a vault-only backup always has
    // zero entries and is still a top-up).
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
    // The store returns entry bytes, not names; names are the computed entry ids.
    for bytes in &survey.items {
        let name = hexfmt::encode(&k1::entry_id(bytes));
        let name = name.trim_start_matches("0x").to_string();
        total += bytes.len() as u64;
        // Compare against the file on disk, not the old manifest, which may be stale (a deleted file, an
        // incomplete copy, an interrupted export). Trusting it could leave files missing (found only at
        // restore), and a file the manifest lacks would make every write fail with "already there".
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
    // The vault is included too (its files are credentials; losing it loses the grants). Each file goes under
    // `held/` by relative path, with its digest in the manifest; a home without a vault gives an empty table.
    // Vault files change (new grants, replaced files), so matching files are kept and differing ones replaced.
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
    // Bundles with a root are recognized by it and leave `owner` empty; without a root, record this identity
    // so two people's bundles in one folder never overwrite each other.
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
    // Replace the manifest so it describes the bundle as it now is.
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

/// Read a bundle's manifest; an unreadable manifest is an error.
pub fn inspect(bundle: &std::path::Path) -> Result<Sheet, Fault> {
    let p = bundle.join(MANIFEST);
    let bytes = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
    // Parsed by the same function the command line uses (`zikaron_glue::mirror::sheet`).
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
    /// The first shape problem in the manifest, if any, naming the offending row.
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
            // Must be a relative path that stays inside the vault directory (no `..`, `.`, empty segments
            // or absolute paths).
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

/// Re-verify a bundle entry by entry: size and digest match the manifest, the core's `entry::check` passes,
/// and the computed `entry_id` equals the file name. Any failure refuses the whole bundle, naming the entry
/// and the check that failed.
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
        // The core's check; its error token is reported as is.
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

/// The current state of the last bundle written.
///
/// The user chooses where mirrors go, so this checks the last recorded one: whether one was written, whether
/// it is still there, and whether it still verifies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mirrored {
    /// Never written.
    Never,
    /// Written, but nothing is at that path now.
    Gone { path: String },
    /// Written, still there, and it fails verification.
    Bad { path: String, why: String },
    /// Written, still there, verifies, with how many entries.
    At { path: String, at: u64, entries: usize },
}

/// Check the last bundle written. This verifies the whole bundle on disk, so it runs in the background, never
/// on the UI thread.
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
