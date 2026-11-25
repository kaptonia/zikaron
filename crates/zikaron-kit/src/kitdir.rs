use crate::tokens::{self as t, KitFailToken, Rule};
use zikaron::tokens::Token;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use zikaron::entry;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::trace;

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

pub fn is_kit_path(p: &str) -> bool {
    kit_path_fault(p).is_none()
}

pub fn kit_path_fault(p: &str) -> Option<PathFault> {
    let b = p.as_bytes();
    if b.is_empty() {
        return Some(PathFault::Empty);
    }
    if b.len() > 1024 {
        return Some(PathFault::TooLong { len: b.len() });
    }
    if b[0] == b'/' {
        return Some(PathFault::LeadingSlash);
    }
    for (i, seg) in p.split('/').enumerate() {
        let segment = i + 1;
        let s = seg.as_bytes();
        if s.is_empty() {
            return Some(PathFault::EmptySegment { segment });
        }
        if s.len() > 255 {
            return Some(PathFault::SegmentTooLong { segment, len: s.len() });
        }
        if seg == "." || seg == ".." {
            return Some(PathFault::Dot { segment });
        }
        if s[0] == b'-' {
            return Some(PathFault::LeadingDash { segment });
        }
        if let Some(c) = seg.chars().find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))) {
            return Some(PathFault::Char { segment, c });
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathFault {
    Empty,
    TooLong { len: usize },
    LeadingSlash,
    EmptySegment { segment: usize },
    SegmentTooLong { segment: usize, len: usize },
    Dot { segment: usize },
    LeadingDash { segment: usize },
    Char { segment: usize, c: char },
}

impl PathFault {
    pub fn as_str(&self) -> &'static str {
        match self {
            PathFault::Empty => "EMPTY",
            PathFault::TooLong { .. } => "TOO_LONG",
            PathFault::LeadingSlash => "LEADING_SLASH",
            PathFault::EmptySegment { .. } => "EMPTY_SEGMENT",
            PathFault::SegmentTooLong { .. } => "SEGMENT_TOO_LONG",
            PathFault::Dot { .. } => "DOT_SEGMENT",
            PathFault::LeadingDash { .. } => "LEADING_DASH",
            PathFault::Char { .. } => "CHAR",
        }
    }

    pub fn evidence(&self) -> String {
        match self {
            PathFault::Empty | PathFault::LeadingSlash => String::new(),
            PathFault::TooLong { len } => format!("length={len} max=1024"),
            PathFault::EmptySegment { segment } | PathFault::Dot { segment } | PathFault::LeadingDash { segment } => format!("segment={segment}"),
            PathFault::SegmentTooLong { segment, len } => format!("segment={segment} length={len} max=255"),
            PathFault::Char { segment, c } => format!("segment={segment} char=U+{:04X}", *c as u32),
        }
    }
}

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
    let here = if prefix.is_empty() {
        ".".to_string()
    } else {
        prefix.trim_end_matches('/').to_string()
    };
    for name in &names {
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

    let entries_arr = v.member("entries").and_then(|x| x.as_arr()).ok_or(Rule::Entries)?;
    let mut entries: Vec<String> = Vec::with_capacity(entries_arr.len());
    for it in entries_arr {
        entries.push(hex32_of(Some(it)).ok_or(Rule::Entries)?);
    }
    for i in 1..entries.len() {
        if entries[i - 1].as_bytes() >= entries[i].as_bytes() {
            return Err(Rule::Entries);
        }
    }

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

pub fn verify_kit(dir: &Path) -> KitVerdict {
    trace::mark(t::K2);
    let mut pairs: Vec<(String, Vec<u8>)> = Vec::new();
    if let Err(path) = walk(dir, "", &mut pairs) {
        return fail(KitFailToken::Unreadable, &path);
    }
    verify_enumeration(&pairs)
}

pub fn verify_enumeration(pairs: &[(String, Vec<u8>)]) -> KitVerdict {
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

#[cfg(test)]
mod path_fault_tests {
    use super::*;

    #[test]
    fn each_part_of_the_kit_path_rule_is_named_with_where() {
        let long_seg = "a".repeat(256);
        let long_path = vec!["a"; 600].join("/");
        let cases: Vec<(&str, Option<PathFault>)> = vec![
            ("a/b.txt", None),
            ("f0.txt", None),
            ("x_y-z.9", None),
            ("", Some(PathFault::Empty)),
            (&long_path, Some(PathFault::TooLong { len: long_path.len() })),
            ("/a", Some(PathFault::LeadingSlash)),
            ("a//b", Some(PathFault::EmptySegment { segment: 2 })),
            ("a/", Some(PathFault::EmptySegment { segment: 2 })),
            (&long_seg, Some(PathFault::SegmentTooLong { segment: 1, len: 256 })),
            ("a/../b", Some(PathFault::Dot { segment: 2 })),
            (".", Some(PathFault::Dot { segment: 1 })),
            ("-a", Some(PathFault::LeadingDash { segment: 1 })),
            ("文件.txt", Some(PathFault::Char { segment: 1, c: '文' })),
            ("docs/my file.txt", Some(PathFault::Char { segment: 2, c: ' ' })),
            ("A.txt", Some(PathFault::Char { segment: 1, c: 'A' })),
            ("a=b", Some(PathFault::Char { segment: 1, c: '=' })),
        ];
        for (p, want) in cases {
            assert_eq!(kit_path_fault(p), want, "{p:?}");
            assert_eq!(is_kit_path(p), want.is_none(), "{p:?}");
        }
        assert!(is_kit_path(&"a".repeat(255)));
        let at_most = format!("{}/{}", "a".repeat(255), vec!["b"; 384].join("/"));
        assert_eq!(at_most.len(), 1023);
        assert!(is_kit_path(&at_most));
        assert_eq!(PathFault::Char { segment: 2, c: '文' }.evidence(), "segment=2 char=U+6587");
    }
}
