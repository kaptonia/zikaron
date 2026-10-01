//! Disclosure kits: enumeration (kit law §7.1), kit paths (§7.2), manifest (§7.3) and verification (§7.4).
//! Proof file bytes are not read (§7.5); they are pinned by digest.

use crate::tokens::{self as t, KitFailToken, Rule};
use zikaron::tokens::Token;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use zikaron::entry;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::trace;

/// Verification outcome: counts and the invalid-entry table on success, the verdict and its subject on
/// failure.
#[derive(Clone, Debug)]
pub enum KitVerdict {
    Ok {
        entries: usize,
        files: usize,
        proofs: usize,
        invalid: Vec<(String, Token)>,
        kit_id: [u8; 32],
    },
    Fail {
        verdict: KitFailToken,
        subject: Option<String>,
    },
}

fn fail(verdict: KitFailToken, subject: &str) -> KitVerdict {
    KitVerdict::Fail {
        verdict,
        subject: Some(subject.to_string()),
    }
}

/// Kit law §7.2: a kit path is one or more segments joined by `/`, each of `a-z0-9._-`, 1 to 255 long, not
/// `.` or `..`, not starting with `-`; at most 1024 in all, not starting with `/`.
pub fn is_kit_path(p: &str) -> bool {
    let b = p.as_bytes();
    if b.is_empty() || b.len() > 1024 || b[0] == b'/' {
        return false;
    }
    for seg in p.split('/') {
        let s = seg.as_bytes();
        if s.is_empty() || s.len() > 255 {
            return false;
        }
        if seg == "." || seg == ".." {
            return false;
        }
        if s[0] == b'-' {
            return false;
        }
        if !s
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-'))
        {
            return false;
        }
    }
    true
}

/// The walk of kit law §7.1: each directory in name byte order; it stops at a symlink, an unlistable
/// directory, a file that cannot be read in full, anything neither file nor directory, or a name that is not
/// valid UTF-8.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    let listing = match std::fs::read_dir(dir) {
        Ok(l) => l,
        Err(_) => {
            return Err(if prefix.is_empty() {
                ".".to_string()
            } else {
                prefix.trim_end_matches('/').to_string()
            })
        }
    };
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for item in listing {
        match item {
            Ok(e) => names.push(e.file_name()),
            Err(_) => {
                return Err(if prefix.is_empty() {
                    ".".to_string()
                } else {
                    prefix.trim_end_matches('/').to_string()
                })
            }
        }
    }
    names.sort_by(|a, b| as_bytes(a).cmp(as_bytes(b)));
    // The path listed for this directory: the kit directory itself is `.`, others lose the trailing `/`.
    let here = if prefix.is_empty() {
        ".".to_string()
    } else {
        prefix.trim_end_matches('/').to_string()
    };
    for name in &names {
        // A name that is not valid UTF-8 cannot be spelled as a path, so the subject is the directory that
        // lists it (§7.1).
        let name_str = match name.to_str() {
            Some(x) => x,
            None => return Err(here),
        };
        let rel = format!("{}{}", prefix, name_str);
        let path = dir.join(name);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => return Err(rel),
        };
        if meta.file_type().is_symlink() {
            return Err(rel);
        }
        if meta.is_dir() {
            walk(&path, &format!("{}/", rel), out)?;
        } else if meta.is_file() {
            match std::fs::read(&path) {
                Ok(bytes) => out.push((rel, bytes)),
                Err(_) => return Err(rel),
            }
        } else {
            return Err(rel);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn as_bytes(s: &std::ffi::OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes()
}

#[cfg(not(unix))]
fn as_bytes(s: &std::ffi::OsStr) -> &[u8] {
    s.to_str().map(|x| x.as_bytes()).unwrap_or(&[])
}

struct KitManifest {
    entries: Vec<String>,
    files: Vec<(String, String, u64)>,
    proofs: Vec<(String, String)>,
}

const MANIFEST_KEYS: [&str; 7] = [
    "spec", "root", "entries", "files", "contents", "proofs", "note_md",
];

fn arr_of_obj(v: Option<&Value>, keys: &[&str]) -> Option<Vec<Vec<(String, Value)>>> {
    let a = v?.as_arr()?;
    let mut out = Vec::with_capacity(a.len());
    for it in a {
        let ms = match it {
            Value::Obj(ms) => ms,
            _ => return None,
        };
        if ms.len() != keys.len() || !keys.iter().all(|k| ms.iter().any(|(mk, _)| mk == k)) {
            return None;
        }
        out.push(ms.clone());
    }
    Some(out)
}

fn hex32_of(v: Option<&Value>) -> Option<String> {
    let x = v?.as_str()?;
    if hexfmt::is_hex32(x) {
        Some(x.to_string())
    } else {
        None
    }
}

/// Kit law §7.3: manifest rules in the written order; the first failure names its rule as subject.
fn parse_manifest(bytes: &[u8]) -> Result<KitManifest, Rule> {
    let v = json::accept(bytes).map_err(|_| Rule::Canonical)?;
    let members = match &v {
        Value::Obj(ms) => ms,
        _ => return Err(Rule::Members),
    };
    if members.len() != MANIFEST_KEYS.len()
        || !MANIFEST_KEYS
            .iter()
            .all(|k| members.iter().any(|(mk, _)| mk == k))
    {
        return Err(Rule::Members);
    }
    match v.member("spec") {
        Some(Value::Str(s)) if s == t::SPEC_KIT => {}
        _ => return Err(Rule::Spec),
    }
    match v.member("root") {
        Some(Value::Null) => {}
        Some(Value::Str(s)) if hexfmt::is_hex20(s) => {}
        _ => return Err(Rule::Root),
    }

    // entries: hex32 elements, distinct, in byte order.
    let entries_arr = v.member("entries").and_then(|x| x.as_arr()).ok_or(Rule::Entries)?;
    let mut entries: Vec<String> = Vec::with_capacity(entries_arr.len());
    for it in entries_arr {
        entries.push(hex32_of(Some(it)).ok_or(Rule::Entries)?);
    }
    // The order rule also enforces distinctness: neighbours not strictly increasing fail.
    for i in 1..entries.len() {
        if entries[i - 1].as_bytes() >= entries[i].as_bytes() {
            return Err(Rule::Entries);
        }
    }

    // files: {path, sha256, size}; path is a kit path, paths distinct, sorted by path.
    let files_ms = arr_of_obj(v.member("files"), &["path", "sha256", "size"]).ok_or(Rule::Files)?;
    let mut files: Vec<(String, String, u64)> = Vec::with_capacity(files_ms.len());
    for ms in &files_ms {
        let get = |k: &str| ms.iter().find(|(mk, _)| mk == k).map(|(_, x)| x);
        let path = match get("path") {
            Some(Value::Str(s)) if is_kit_path(s) => s.clone(),
            _ => return Err(Rule::Files),
        };
        let sha = hex32_of(get("sha256")).ok_or(Rule::Files)?;
        let size = match get("size") {
            Some(Value::Int(i)) => *i,
            _ => return Err(Rule::Files),
        };
        files.push((path, sha, size));
    }
    for i in 1..files.len() {
        if files[i - 1].0.as_bytes() >= files[i].0.as_bytes() {
            return Err(Rule::Files);
        }
    }

    // contents: {content, path}; path must be in files and its sha256 must equal content; rows distinct,
    // sorted by content then path.
    let contents_ms =
        arr_of_obj(v.member("contents"), &["content", "path"]).ok_or(Rule::Contents)?;
    let files_at: HashMap<&str, &String> =
        files.iter().map(|(p, s, _)| (p.as_str(), s)).collect();
    let mut contents: Vec<(String, String)> = Vec::with_capacity(contents_ms.len());
    for ms in &contents_ms {
        let get = |k: &str| ms.iter().find(|(mk, _)| mk == k).map(|(_, x)| x);
        let content = hex32_of(get("content")).ok_or(Rule::Contents)?;
        let path = match get("path") {
            Some(Value::Str(s)) => s.clone(),
            _ => return Err(Rule::Contents),
        };
        match files_at.get(path.as_str()) {
            Some(sha) if *sha == &content => {}
            _ => return Err(Rule::Contents),
        }
        contents.push((content, path));
    }
    for i in 1..contents.len() {
        let a = (contents[i - 1].0.as_bytes(), contents[i - 1].1.as_bytes());
        let b = (contents[i].0.as_bytes(), contents[i].1.as_bytes());
        if a >= b {
            return Err(Rule::Contents);
        }
    }

    // proofs: {path, sha256, tx}; path is a kit path, paths distinct, sorted by path.
    let proofs_ms =
        arr_of_obj(v.member("proofs"), &["path", "sha256", "tx"]).ok_or(Rule::Proofs)?;
    let mut proofs: Vec<(String, String)> = Vec::with_capacity(proofs_ms.len());
    for ms in &proofs_ms {
        let get = |k: &str| ms.iter().find(|(mk, _)| mk == k).map(|(_, x)| x);
        let path = match get("path") {
            Some(Value::Str(s)) if is_kit_path(s) => s.clone(),
            _ => return Err(Rule::Proofs),
        };
        let sha = hex32_of(get("sha256")).ok_or(Rule::Proofs)?;
        hex32_of(get("tx")).ok_or(Rule::Proofs)?;
        proofs.push((path, sha));
    }
    for i in 1..proofs.len() {
        if proofs[i - 1].0.as_bytes() >= proofs[i].0.as_bytes() {
            return Err(Rule::Proofs);
        }
    }

    match v.member("note_md") {
        Some(Value::Str(_)) => {}
        _ => return Err(Rule::NoteMd),
    }

    Ok(KitManifest {
        entries,
        files,
        proofs,
    })
}

/// Kit law §7.1 plus §7.4: verify one directory.
pub fn verify_kit(dir: &Path) -> KitVerdict {
    trace::mark(t::K2);
    let mut pairs: Vec<(String, Vec<u8>)> = Vec::new();
    if let Err(path) = walk(dir, "", &mut pairs) {
        return fail(KitFailToken::Unreadable, &path);
    }
    verify_enumeration(&pairs)
}

/// Kit law §7.4: verification over an enumeration, in the written order.
pub fn verify_enumeration(pairs: &[(String, Vec<u8>)]) -> KitVerdict {
    // Index the enumeration by path once; a linear scan per lookup would cost paths times manifest rows.
    let at: HashMap<&str, &Vec<u8>> = pairs.iter().map(|(p, b)| (p.as_str(), b)).collect();
    let manifest_bytes = match at.get("manifest.json") {
        Some(b) => (*b).clone(),
        None => {
            return KitVerdict::Fail {
                verdict: KitFailToken::ManifestAbsent,
                subject: None,
            }
        }
    };
    let m = match parse_manifest(&manifest_bytes) {
        Ok(m) => m,
        Err(rule) => return fail(KitFailToken::Manifest, rule.as_str()),
    };

    let mut named: HashSet<String> = HashSet::new();
    named.insert("manifest.json".to_string());

    for id in &m.entries {
        let path = format!("entries/{}.zk1", &id[2..]);
        named.insert(path.clone());
        match at.get(path.as_str()) {
            Some(b) if hexfmt::encode(&entry::entry_id(b)) == *id => {}
            _ => return fail(KitFailToken::EntryBytes, id),
        }
    }
    for (path, sha, size) in &m.files {
        let full = format!("files/{}", path);
        named.insert(full.clone());
        match at.get(full.as_str()) {
            Some(b) if hexfmt::encode(&entry::entry_id(b)) == *sha && b.len() as u64 == *size => {}
            _ => return fail(KitFailToken::File, path),
        }
    }
    for (path, sha) in &m.proofs {
        let full = format!("proofs/{}", path);
        named.insert(full.clone());
        match at.get(full.as_str()) {
            Some(b) if hexfmt::encode(&entry::entry_id(b)) == *sha => {}
            _ => return fail(KitFailToken::ProofBytes, path),
        }
    }

    let mut extra: Vec<&String> = pairs
        .iter()
        .map(|(p, _)| p)
        .filter(|p| !named.contains(p.as_str()))
        .collect();
    if !extra.is_empty() {
        extra.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        return fail(KitFailToken::Extra, extra[0]);
    }

    // Listed ids whose bytes fail `accept`, in entries order, each with its parent-law token (informational).
    let mut invalid: Vec<(String, Token)> = Vec::new();
    for id in &m.entries {
        let path = format!("entries/{}.zk1", &id[2..]);
        if let Some(b) = at.get(path.as_str()) {
            if let Err(tok) = entry::check(b) {
                invalid.push((id.clone(), tok));
            }
        }
    }

    KitVerdict::Ok {
        entries: m.entries.len(),
        files: m.files.len(),
        proofs: m.proofs.len(),
        invalid,
        kit_id: entry::entry_id(&manifest_bytes),
    }
}
