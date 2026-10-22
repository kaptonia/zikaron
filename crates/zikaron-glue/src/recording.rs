//! The recording convention: how a `history` entry's `content` and `mode` are filled for bytes (`zikaron/1`
//! §6.2 requires both and leaves their meaning to the writer). Shared by the app and the CLI's `history --file`,
//! so the same file gives the same two members either way.
//!
//! `mark` is always [`FAMILY`]; `toolchain` is the sha256 of that literal's UTF-8 bytes. The toolchain names the
//! convention (which bytes are hashed, by which algorithm), not the program that hashed them.

use zikaron::cryptox;

/// The family literal: every `content` written under it is the sha256 of some bytes (a file's, a manifest's,
/// a commit object's).
pub const FAMILY: &str = "bytes-sha256/1";

/// The mode's `toolchain`: the sha256 of [`FAMILY`]'s UTF-8 bytes.
pub fn toolchain() -> [u8; 32] {
    cryptox::sha256(FAMILY.as_bytes())
}

/// A file's `content` under this convention: the sha256 of its bytes.
pub fn content_of(bytes: &[u8]) -> [u8; 32] {
    cryptox::sha256(bytes)
}

/// One row of a directory's manifest under this convention: a regular file's path from the directory and the
/// sha256 of its bytes in hex (`zikaron::hexfmt`).
pub type DirRow = (String, String);

/// The separator a directory's manifest spells its paths with, on every system: a convention of this program's
/// (neither law fixes how a directory is recorded), so the same tree gives the same content anywhere.
pub const SEPARATOR: &str = "/";

/// The separator directories signed on Windows were spelled with before [`SEPARATOR`] held everywhere. Only read:
/// a record of that spelling is still recognised ([`dir_contents`]), nothing is signed with it.
const OLD_WINDOWS_SEPARATOR: &str = "\\";

/// A path inside a directory as its manifest spells it: its names from the directory joined by [`SEPARATOR`]. The
/// one place a manifest path is spelled: walking a directory ([`dir_rows`]) and recomputing one from the names a
/// record package keeps ([`crate::extract`]) both join here, so both give the same rows.
pub fn dir_path(names: &[&str]) -> String {
    names.join(SEPARATOR)
}

/// A directory's rows: one per regular file under `root`, descended in name order; anything else (a symbolic
/// link, a device file) is left out and counted, never followed. A directory or file that cannot be read stops
/// the walk with the system's error and where it happened. Signing a directory and taking a directory out of a
/// record package both walk it here.
pub fn dir_rows(root: &std::path::Path) -> Result<(Vec<DirRow>, usize), (std::io::Error, std::path::PathBuf)> {
    let mut rows = Vec::new();
    let mut skipped = 0usize;
    walk(root, &mut Vec::new(), &mut rows, &mut skipped)?;
    Ok((rows, skipped))
}

/// One directory of the walk: `names` are its names from the root, each row's path its names joined by
/// [`dir_path`].
fn walk(at: &std::path::Path, names: &mut Vec<String>, out: &mut Vec<DirRow>, skipped: &mut usize) -> Result<(), (std::io::Error, std::path::PathBuf)> {
    let mut items: Vec<std::path::PathBuf> = std::fs::read_dir(at)
        .map_err(|e| (e, at.to_path_buf()))?
        .map(|e| e.map(|e| e.path()).map_err(|e| (e, at.to_path_buf())))
        .collect::<Result<_, _>>()?;
    items.sort();
    for p in items {
        let md = std::fs::symlink_metadata(&p).map_err(|e| (e, p.clone()))?;
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if md.is_dir() {
            names.push(name);
            let walked = walk(&p, names, out, skipped);
            names.pop();
            walked?;
        } else if md.is_file() {
            let b = std::fs::read(&p).map_err(|e| (e, p.clone()))?;
            let mut path: Vec<&str> = names.iter().map(|n| n.as_str()).collect();
            path.push(&name);
            out.push((dir_path(&path), zikaron::hexfmt::encode(&content_of(&b))));
        } else {
            *skipped += 1;
        }
    }
    Ok(())
}

/// The members of a directory's manifest.
mod member {
    pub const FILES: &str = "files";
    pub const PATH: &str = "path";
    pub const SHA256: &str = "sha256";
}

/// A directory's manifest: `{"files":[{"path","sha256"},…]}`, the rows sorted by path, as one canonical JSON value,
/// so the same tree gives the same value anywhere.
pub fn dir_manifest(rows: &[DirRow]) -> zikaron::json::Value {
    use zikaron::json::Value;
    let mut rows: Vec<&DirRow> = rows.iter().collect();
    rows.sort();
    let arr = rows
        .into_iter()
        .map(|(path, sha)| Value::Obj(vec![(member::PATH.to_string(), Value::Str(path.clone())), (member::SHA256.to_string(), Value::Str(sha.clone()))]))
        .collect();
    Value::Obj(vec![(member::FILES.to_string(), Value::Arr(arr))])
}

/// A directory's `content` under this convention: the sha256 of its manifest's canonical bytes, with those bytes.
pub fn dir_content(rows: &[DirRow]) -> ([u8; 32], Vec<u8>) {
    let bytes = zikaron::json::canon_bytes(&dir_manifest(rows));
    (content_of(&bytes), bytes)
}

/// The contents a directory's rows may have been recorded under, each with its manifest's bytes: the one signed
/// now ([`dir_content`]), then, when it differs, the one a directory signed on Windows before [`SEPARATOR`] held
/// everywhere got (each path's names joined by a backslash, the rows sorted again by that spelling). Reading a
/// record takes either; signing writes only the first. A name that itself holds a backslash (possible outside
/// Windows) reads in the old spelling as a folder and a name in it.
pub fn dir_contents(rows: &[DirRow]) -> Vec<([u8; 32], Vec<u8>)> {
    std::iter::once(dir_content(rows)).chain(old_dir_content(rows)).collect()
}

/// A directory's content in the spelling directories signed on Windows had before [`SEPARATOR`] held everywhere,
/// with its manifest's bytes; `None` when it is the content [`dir_content`] gives (no file below a folder).
pub fn old_dir_content(rows: &[DirRow]) -> Option<([u8; 32], Vec<u8>)> {
    let old: Vec<DirRow> = rows.iter().map(|(p, sha)| (p.split(SEPARATOR).collect::<Vec<_>>().join(OLD_WINDOWS_SEPARATOR), sha.clone())).collect();
    let old = dir_content(&old);
    (old.0 != dir_content(rows).0).then_some(old)
}

/// The rows a manifest ([`dir_manifest`]) lists, read back from it.
pub fn manifest_rows(manifest: &zikaron::json::Value) -> Vec<DirRow> {
    use zikaron::json::Value;
    let Some(Value::Arr(files)) = manifest.member(member::FILES) else { return Vec::new() };
    files
        .iter()
        .filter_map(|f| match (f.member(member::PATH), f.member(member::SHA256)) {
            (Some(Value::Str(p)), Some(Value::Str(s))) => Some((p.clone(), s.clone())),
            _ => None,
        })
        .collect()
}

/// Whether a directory's rows give `content` (hex) under either spelling ([`dir_contents`]): the one judgement
/// of a directory against its record, wherever a directory is checked again.
pub fn dir_holds(rows: &[DirRow], content: &str) -> bool {
    dir_contents(rows).iter().any(|(d, _)| zikaron::hexfmt::encode(d).eq_ignore_ascii_case(content.trim()))
}
