//! Lay out, self-verify and write a disclosure kit (kit law §7).
//!
//! A kit is laid out in a temporary directory beside the target and moved into place only after the kit core's
//! `verify_kit` returns KIT_OK. No partial or unverified kit ever appears on disk; on failure the temporary
//! directory is cleared and the caller's path is untouched. Writing first and checking afterwards would let
//! someone take the kit before the check.
//!
//! This module does not judge kit validity (manifest members, path character sets, table order): `verify_kit`
//! does. It lays out, computes digests (through the kit core's `doc::doc_id`, kit law §1) and asks
//! before writing.

use crate::landing;
use crate::names::{Field, Key, Slot, ENTRIES_DIR, ENTRY_SUFFIX, FILES_DIR, MANIFEST, PROOFS_DIR};
use crate::tidy::{self, Fate};
use std::path::Path;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron_kit::doc;
use zikaron_kit::kitdir::{self, KitVerdict};
use zikaron_kit::tokens::SPEC_KIT;

/// Everything a kit needs.
#[derive(Default)]
pub struct Bundle {
    /// Entry bytes (ids are computed by the kit core, never claimed by the caller).
    pub entries: Vec<Vec<u8>>,
    /// (kit path, bytes).
    pub files: Vec<(String, Vec<u8>)>,
    /// (kit path, tx, bytes). Proof kits are captured by the anchoring crate; this module only pins their
    /// bytes (kit law §7.5).
    pub proofs: Vec<(String, String, Vec<u8>)>,
    /// Which kit paths are also work contents (`contents`, kit law §7.3).
    pub contents: Vec<String>,
    pub root: Option<String>,
    pub note: String,
}

/// The outcome of writing a kit.
pub struct Landed {
    pub entries: usize,
    pub files: usize,
    pub proofs: usize,
    pub kit_id: String,
    /// Platform junk dropped before output (the count is reported).
    pub dropped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trouble {
    /// A disk operation failed: the subject, and the system's message (operation, path, OS text).
    Io(String, String),
    /// Something already exists at the target path; it is never overwritten.
    Occupied(String),
    /// A path fails kit law §7.2 and is not platform junk: refused, subject named.
    BadPath(String),
    /// Two things at the same kit path.
    Duplicate(String),
    /// Self-verification failed: the kit verdict and its subject.
    Refused(String, Option<String>),
}

impl From<landing::Trouble> for Trouble {
    fn from(t: landing::Trouble) -> Trouble {
        match t {
            landing::Trouble::Occupied(p) => Trouble::Occupied(p),
            landing::Trouble::Io(p, w) => Trouble::Io(p, w),
        }
    }
}

impl Trouble {
    pub fn code(&self) -> &'static str {
        match self {
            Trouble::Io(..) => "E_IO",
            Trouble::Occupied(_) => "E_OCCUPIED",
            Trouble::BadPath(_) => "E_BAD_PATH",
            Trouble::Duplicate(_) => "E_DUPLICATE_PATH",
            Trouble::Refused(_, _) => "E_KIT",
        }
    }

    /// The system's message for a failed disk operation (`None` for every other refusal).
    pub fn said(&self) -> Option<&str> {
        match self {
            Trouble::Io(_, w) => Some(w.as_str()),
            _ => None,
        }
    }

    /// The kit verdict; only a failed self-verification has one.
    pub fn verdict(&self) -> Option<&str> {
        match self {
            Trouble::Refused(v, _) => Some(v.as_str()),
            _ => None,
        }
    }

    /// The path involved; only disk failures have one.
    pub fn path(&self) -> Option<&str> {
        match self {
            Trouble::Io(p, _) | Trouble::Occupied(p) | Trouble::BadPath(p) | Trouble::Duplicate(p) => Some(p.as_str()),
            Trouble::Refused(_, _) => None,
        }
    }

    /// The subject of the refusal.
    pub fn subject(&self) -> String {
        match self {
            Trouble::Io(p, _) | Trouble::Occupied(p) | Trouble::BadPath(p) | Trouble::Duplicate(p) => p.clone(),
            Trouble::Refused(verdict, subject) => match subject {
                Some(x) => format!("{verdict}:{x}"),
                None => verdict.clone(),
            },
        }
    }
}

/// kit law §1: `doc_id(b) = sha256(b)`. Digests come only from the kit core.
fn digest(b: &[u8]) -> String {
    hexfmt::encode(&doc::doc_id(b))
}

fn row(members: Vec<(Field, Value)>) -> Value {
    Value::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect(),
    )
}

/// Clean paths before output: platform junk is dropped and named, anything else malformed is refused. Runs
/// before layout, so nothing that must not enter the kit touches the staging area either.
pub fn tidy(b: &mut Bundle) -> Result<Vec<String>, Trouble> {
    crate::seam_v2();
    let mut dropped: Vec<String> = Vec::new();
    for list in [&mut b.files] {
        let mut keep: Vec<(String, Vec<u8>)> = Vec::new();
        for (p, bytes) in list.drain(..) {
            match tidy::fate(&p) {
                Fate::Keep => keep.push((p, bytes)),
                Fate::Drop => dropped.push(p),
                Fate::Refuse => return Err(Trouble::BadPath(p)),
            }
        }
        *list = keep;
    }
    let mut keep_proofs: Vec<(String, String, Vec<u8>)> = Vec::new();
    for (p, tx, bytes) in b.proofs.drain(..) {
        match tidy::fate(&p) {
            Fate::Keep => keep_proofs.push((p, tx, bytes)),
            Fate::Drop => dropped.push(p),
            Fate::Refuse => return Err(Trouble::BadPath(p)),
        }
    }
    b.proofs = keep_proofs;
    // Dropped items are no longer `contents` candidates.
    b.contents.retain(|p| !dropped.contains(p));
    dropped.sort();
    Ok(dropped)
}

/// The kit law §7.3 manifest. Each table follows its own order; the kit core judges it.
pub fn manifest(b: &Bundle) -> Value {
    crate::seam_v2();
    let mut ids: Vec<String> = b.entries.iter().map(|x| digest(x)).collect();
    ids.sort_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
    ids.dedup();

    let mut files: Vec<(String, String, u64)> = b
        .files
        .iter()
        .map(|(p, x)| (p.clone(), digest(x), x.len() as u64))
        .collect();
    files.sort_by(|x, y| x.0.as_bytes().cmp(y.0.as_bytes()));

    let mut contents: Vec<(String, String)> = b
        .contents
        .iter()
        .filter_map(|p| files.iter().find(|(fp, _, _)| fp == p).map(|(fp, s, _)| (s.clone(), fp.clone())))
        .collect();
    contents.sort_by(|x, y| (x.0.as_bytes(), x.1.as_bytes()).cmp(&(y.0.as_bytes(), y.1.as_bytes())));
    contents.dedup();

    let mut proofs: Vec<(String, String, String)> = b
        .proofs
        .iter()
        .map(|(p, tx, x)| (p.clone(), digest(x), tx.clone()))
        .collect();
    proofs.sort_by(|x, y| x.0.as_bytes().cmp(y.0.as_bytes()));

    Value::Obj(vec![
        (
            Field::Entries.as_str().to_string(),
            Value::Arr(ids.into_iter().map(Value::Str).collect()),
        ),
        (
            Field::Files.as_str().to_string(),
            Value::Arr(
                files
                    .into_iter()
                    .map(|(p, s, n)| {
                        row(vec![
                            (Field::Path, Value::Str(p)),
                            (Field::Sha256, Value::Str(s)),
                            (Field::Size, Value::Int(n)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            Field::Contents.as_str().to_string(),
            Value::Arr(
                contents
                    .into_iter()
                    .map(|(c, p)| row(vec![(Field::Content, Value::Str(c)), (Field::Path, Value::Str(p))]))
                    .collect(),
            ),
        ),
        (Field::NoteMd.as_str().to_string(), Value::Str(b.note.clone())),
        (
            Field::Proofs.as_str().to_string(),
            Value::Arr(
                proofs
                    .into_iter()
                    .map(|(p, s, tx)| {
                        row(vec![
                            (Field::Path, Value::Str(p)),
                            (Field::Sha256, Value::Str(s)),
                            (Field::Tx, Value::Str(tx)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            Field::Root.as_str().to_string(),
            match &b.root {
                Some(r) => Value::Str(r.clone()),
                None => Value::Null,
            },
        ),
        (Field::Spec.as_str().to_string(), Value::Str(SPEC_KIT.to_string())),
    ])
}

/// The verification note included in every kit. It is itself a manifest row: a kit may hold nothing outside
/// the manifest (`E_KIT_EXTRA`, kit law §7.4).
pub fn verification_note(b: &Bundle) -> Vec<u8> {
    crate::seam_v2();
    let mut t = String::new();
    t.push_str("# 怎么核这一包\n\n");
    t.push_str("1. 逐档算 sha256,对 `manifest.json` 里 `files` 与 `proofs` 各行的 `sha256`;\n");
    t.push_str("2. `entries/` 下每一档的 sha256 即它的档名(去 `.zk1`),也即 `entries` 表里那一行;\n");
    t.push_str("3. 每一枚条目按 zikaron/1 §4.3 判十三步;\n");
    t.push_str("4. 清单外的档一件也不该有。\n\n");
    t.push_str(&format!(
        "本束:条目 {} 枚 · 档 {} 件 · 证明 {} 件。\n",
        b.entries.len(),
        b.files.len() + 1,
        b.proofs.len()
    ));
    t.push_str("\n出包时已由 zikaron.kit/1 的 `verify_kit` 自验,KIT_OK 才落盘(不变式 9)。\n");
    t.into_bytes()
}

/// Prepare before output: clean paths, add the verification note, check duplicate paths. Directory and
/// single-file kits both go through here, so they carry the same content.
fn prepare(b: &mut Bundle) -> Result<Vec<String>, Trouble> {
    let dropped = tidy(b)?;
    b.files.push((Slot::Verify.path().to_string(), verification_note(b)));
    // Refuse duplicate kit paths here by name. Otherwise a duplicate would surface as an opaque
    // `E_KIT_MANIFEST:files` (the table must be strictly increasing), or a caller's own `verify.md` would be
    // replaced by the generated one.
    duplicates(b)?;
    Ok(dropped)
}

/// A kit's enumeration (kit law §7.1): kit path to bytes. Kit paths are built only here; directory kits
/// lay it out on disk ([`export`]) and single-file kits pack it ([`crate::container`]). Order is kit-path byte
/// order (the §7.1 walk).
pub fn enumeration(b: &Bundle) -> Vec<(String, Vec<u8>)> {
    crate::seam_v2();
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for bytes in &b.entries {
        let id = digest(bytes);
        out.push((format!("{ENTRIES_DIR}/{}{}", id.trim_start_matches("0x"), ENTRY_SUFFIX), bytes.clone()));
    }
    for (p, bytes) in &b.files {
        out.push((format!("{FILES_DIR}/{p}"), bytes.clone()));
    }
    for (p, _, bytes) in &b.proofs {
        out.push((format!("{PROOFS_DIR}/{p}"), bytes.clone()));
    }
    out.push((MANIFEST.to_string(), json::canon_bytes(&manifest(b))));
    out.sort_by(|x, y| x.0.as_bytes().cmp(y.0.as_bytes()));
    out.dedup_by(|x, y| x.0 == y.0);
    out
}

/// Lay out and self-verify in memory. Returns the enumeration only on KIT_OK, so a failing kit leaves nothing
/// to write.
pub fn enumerate(mut b: Bundle) -> Result<(Vec<(String, Vec<u8>)>, Landed), Trouble> {
    crate::seam_v2();
    let dropped = prepare(&mut b)?;
    let pairs = enumeration(&b);
    match kitdir::verify_enumeration(&pairs) {
        KitVerdict::Ok { entries, files, proofs, kit_id, .. } => {
            Ok((pairs, Landed { entries, files, proofs, kit_id: hexfmt::encode(&kit_id), dropped }))
        }
        KitVerdict::Fail { verdict, subject } => Err(Trouble::Refused(verdict.as_str().to_string(), subject)),
    }
}

/// Lay out, self-verify and write to `out`, only on KIT_OK.
pub fn export(out: &Path, mut b: Bundle) -> Result<Landed, Trouble> {
    crate::seam_v2();
    if out.exists() {
        return Err(Trouble::Occupied(out.to_string_lossy().into_owned()));
    }
    let dropped = prepare(&mut b)?;

    let staging = landing::staging_beside(out)?;
    let _ = std::fs::remove_dir_all(&staging);
    // Once the staging area exists, every failure path clears it (single exit).
    let landed = lay_and_verify(&staging, &b);
    match landed {
        Ok((entries, files, proofs, kit_id)) => {
            // The only move into place (`landing::land_tree`): an existing target is refused; on failure the
            // temporary tree is cleared.
            if let Err(t) = landing::land_tree(out, &staging) {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(Trouble::from(t));
            }
            Ok(Landed {
                entries,
                files,
                proofs,
                kit_id,
                dropped,
            })
        }
        Err(t) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(t)
        }
    }
}

/// Two items at one kit path are refused, with that path as subject. Files and proofs are separate tables.
fn duplicates(b: &Bundle) -> Result<(), Trouble> {
    for list in [
        b.files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
        b.proofs.iter().map(|(p, _, _)| p.clone()).collect::<Vec<_>>(),
    ] {
        let mut seen = list.clone();
        seen.sort();
        for i in 1..seen.len() {
            if seen[i - 1] == seen[i] {
                return Err(Trouble::Duplicate(seen[i].clone()));
            }
        }
    }
    Ok(())
}

/// Lay out in the staging area and self-verify. The target is not touched here; only [`export`] moves the
/// kit into place.
fn lay_and_verify(staging: &Path, b: &Bundle) -> Result<(usize, usize, usize, String), Trouble> {
    mkdir(staging)?;
    for (rel, bytes) in enumeration(b) {
        put_under(staging, &rel, bytes.as_slice())?;
    }
    match kitdir::verify_kit(staging) {
        KitVerdict::Ok {
            entries,
            files,
            proofs,
            kit_id,
            ..
        } => Ok((entries, files, proofs, hexfmt::encode(&kit_id))),
        KitVerdict::Fail { verdict, subject } => Err(Trouble::Refused(verdict.as_str().to_string(), subject)),
    }
}

/// The JSON answer for one kit output.
pub fn answer(l: &Landed, out: &str) -> Value {
    Value::Obj(vec![
        (Key::Dropped.as_str().to_string(), Value::Arr(l.dropped.iter().map(|x| Value::Str(x.clone())).collect())),
        (Key::Entries.as_str().to_string(), Value::Int(l.entries as u64)),
        (Key::Files.as_str().to_string(), Value::Int(l.files as u64)),
        (Key::KitId.as_str().to_string(), Value::Str(l.kit_id.clone())),
        (Key::Ok.as_str().to_string(), Value::Bool(true)),
        (Key::Path.as_str().to_string(), Value::Str(out.to_string())),
        (Key::Proofs.as_str().to_string(), Value::Int(l.proofs as u64)),
        (Key::State.as_str().to_string(), Value::Str(zikaron_kit::tokens::KIT_OK.to_string())),
    ])
}

fn put_under(root: &Path, rel: &str, bytes: &[u8]) -> Result<(), Trouble> {
    let full = root.join(rel);
    if let Some(p) = full.parent() {
        mkdir(p)?;
    }
    write(&full, bytes)
}

fn mkdir(p: &Path) -> Result<(), Trouble> {
    landing::mkdir(p).map_err(Trouble::from)
}

/// Write an item into our own staging area (the move to the caller's path goes through `landing::land_tree`).
fn write(p: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    landing::put(p, bytes).map_err(Trouble::from)
}

/// Add a directory on disk to the kit (under `files/`), paths joined to the caller's prefix.
///
/// The walk follows kit law §7.1: each directory in name byte order, stopping with an error at a
/// symlink. Symlinks are not followed because they could pull outside content into the kit.
pub fn gather(root: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), Trouble> {
    crate::seam_v2();
    let read_dir = |e: std::io::Error| Trouble::Io(root.to_string_lossy().into_owned(), format!("read directory {}: {e}", root.display()));
    let listing = std::fs::read_dir(root).map_err(read_dir)?;
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for item in listing {
        let e = item.map_err(read_dir)?;
        names.push(e.file_name());
    }
    names.sort_by(|a, b| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));
    for name in names {
        let Some(name_str) = name.to_str() else {
            return Err(Trouble::BadPath(root.join(&name).to_string_lossy().into_owned()));
        };
        let rel = if prefix.is_empty() {
            name_str.to_string()
        } else {
            format!("{prefix}/{name_str}")
        };
        let path = root.join(&name);
        let meta = std::fs::symlink_metadata(&path).map_err(|e| Trouble::Io(rel.clone(), format!("read metadata {}: {e}", path.display())))?;
        if meta.file_type().is_symlink() {
            return Err(Trouble::BadPath(rel));
        }
        if meta.is_dir() {
            gather(&path, &rel, out)?;
        } else if meta.is_file() {
            let bytes = std::fs::read(&path).map_err(|e| Trouble::Io(rel.clone(), format!("read {}: {e}", path.display())))?;
            out.push((rel, bytes));
        } else {
            return Err(Trouble::BadPath(rel));
        }
    }
    Ok(())
}
