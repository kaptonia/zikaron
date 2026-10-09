//! Record bundle (kit) index.
//!
//! Bundles live in each identity's home (`<home>/kits/…`), and sibling apps need one place to find which
//! bundles were exported on this machine, and where. Homes may not store absolute paths (any copy of a home
//! is equivalent), so the index lives in the machine directory: `<machine directory>/kits/index.json`. Only
//! this app rewrites it, atomically (`home::put_at`: temporary name then rename, mode 0600). It is created
//! empty at startup when absent, and rewritten when the home moves.
//!
//! ─── Shape ───
//!
//! `{"form":"zikaron.kits-index/1","kits":[{"contents":[hex32…],"created":seconds,"id":hex32,
//! "link":"https://…"?,"note_md":"…","path":"/…","root":hex32}…]}`, canonical key order. Readers recognize
//! the file by the `form` literal, not by an integer version.
//!
//! ─── `link` ───
//!
//! By default `link` is the home's publish base (`settings.publish`) plus the bundle's path relative to the
//! `kits` directory. A hand-filled link overrides it and is recorded in [`OVERRIDES`] (read only by this app).
//! When the publish base changes, rows without an override are reassembled; overridden rows are kept.
//!
//! ─── `anchoredOn` ───
//!
//! Where the kit says it is anchored: the chain id, registry and start block of the home it was exported
//! from, written at export as the fixed last line of the manifest's `note_md` ([`AnchoredOn::line`]). The row
//! keeps it in a separate optional field, and the row's `note_md` is the author's text with that line removed
//! ([`split_note`]). Informational only; no check depends on it.

use crate::fault::{classify, Fault, Known};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};
use zikaron_glue::names::Field;

/// The `contents` and `note_md` key names, taken from the shared kit name table rather than copied here.
fn contents_key() -> &'static str {
    Field::Contents.as_str()
}
fn note_key() -> &'static str {
    Field::NoteMd.as_str()
}

/// The index's form literal.
pub const FORM: &str = "zikaron.kits-index/1";
/// The directory in the machine directory that holds the index.
pub const DIR: &str = "kits";
/// Index file name.
pub const FILE: &str = "index.json";
/// Hand-filled `link`s are recorded here (read only by this app, not by other index readers).
pub const OVERRIDES: &str = "links.json";

/// Every kind of export and where it lands. Two have fixed places under the machine directory (the kits index
/// file, and a directory of kit verification results); the other four go wherever the user picks. The path
/// builders (`path_in`, `verifiedx::path_of`) use the same constants.
pub const EXPORTS: [Export; 6] = [
    Export { name: "kitsIndex", place: Some(&[DIR, FILE]), room: false },
    Export { name: "kitVerifications", place: Some(&[DIR, crate::verifiedx::DIR]), room: true },
    Export { name: "kit", place: None, room: false },
    Export { name: "mirror", place: None, room: false },
    Export { name: "machineBackup", place: None, room: false },
    Export { name: "grantFile", place: None, room: false },
];

/// One row of [`EXPORTS`]: the export's name, its path under the machine directory (`None` when the user picks
/// the place), and whether that path is a directory of exports (`room`) rather than a single file.
#[derive(Clone, Copy, Debug)]
pub struct Export {
    pub name: &'static str,
    pub place: Option<&'static [&'static str]>,
    pub room: bool,
}

/// Where an exported kit says it is anchored: its home's chain, registry and start block at export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchoredOn {
    pub chain_id: u64,
    pub from_block: u64,
    /// The registry contract, `0x` and forty lowercase hex digits.
    pub registry: String,
}

/// The line's fixed head and the separator between its three parts (space, U+00B7, space).
const LINE_HEAD: &str = "anchored-on: eip155:";
const LINE_SEP: &str = " \u{b7} ";

impl AnchoredOn {
    /// The fixed last line of a kit's `note_md`, the same in every language:
    /// `anchored-on: eip155:<chain> · registry <address> · from <block>`.
    pub fn line(&self) -> String {
        format!("{LINE_HEAD}{}{LINE_SEP}registry {}{LINE_SEP}from {}", self.chain_id, self.registry, self.from_block)
    }

    /// Parse a line written by [`AnchoredOn::line`], exactly: decimal numbers without leading zeros (each within
    /// the canonical JSON integer limit, so the index row reads back) and a lowercase address. Anything else is
    /// `None`.
    pub fn of_line(line: &str) -> Option<AnchoredOn> {
        let rest = line.strip_prefix(LINE_HEAD)?;
        let (chain, rest) = rest.split_once(LINE_SEP)?;
        let (registry, from) = rest.strip_prefix("registry ")?.split_once(LINE_SEP)?;
        let from = from.strip_prefix("from ")?;
        let decimal = |t: &str| -> Option<u64> {
            let ok = !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) && (t == "0" || !t.starts_with('0'));
            t.parse::<u64>().ok().filter(|n| ok && *n <= json::MAX_INT)
        };
        let lower_hex = registry.len() == 42
            && registry.starts_with("0x")
            && registry[2..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !lower_hex {
            return None;
        }
        Some(AnchoredOn { chain_id: decimal(chain)?, from_block: decimal(from)?, registry: registry.to_string() })
    }

    fn value(&self) -> Value {
        Value::Obj(vec![
            ("chainId".to_string(), Value::Int(self.chain_id)),
            ("fromBlock".to_string(), Value::Int(self.from_block)),
            ("registry".to_string(), Value::Str(self.registry.clone())),
        ])
    }

    fn of_value(v: &Value) -> Option<AnchoredOn> {
        let int = |k: &str| match member(v, k) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let a = AnchoredOn { chain_id: int("chainId")?, from_block: int("fromBlock")?, registry: str_of(v, "registry")? };
        // Only values that round-trip through the line form are accepted.
        (AnchoredOn::of_line(&a.line()).as_ref() == Some(&a)).then_some(a)
    }
}

/// Split a manifest note into the author's text and the anchoring point, which only the last non-empty line
/// can carry, and only when it matches exactly. Lines split at `\n` (a preceding `\r` belongs to the break);
/// whitespace-only lines count as empty, so trailing blank lines neither hide the anchoring line nor survive
/// its removal. The inverse of [`note_with`].
pub fn split_note(note: &str) -> (String, Option<AnchoredOn>) {
    let mut at = note.len();
    for piece in note.split_inclusive('\n').collect::<Vec<_>>().into_iter().rev() {
        at -= piece.len();
        let line = piece.strip_suffix('\n').unwrap_or(piece);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        // The author's text is everything before this line, its own last line break left out.
        let before = &note[..at];
        let author = before.strip_suffix('\n').map(|b| b.strip_suffix('\r').unwrap_or(b)).unwrap_or(before);
        return match AnchoredOn::of_line(line) {
            Some(a) => (author.to_string(), Some(a)),
            None => (note.to_string(), None),
        };
    }
    (note.to_string(), None)
}

/// The note a kit carries: the author's text, then the anchoring line when one is given. An existing
/// anchoring line is removed first, so exporting again never doubles it.
pub fn note_with(note: &str, at: Option<&AnchoredOn>) -> String {
    let author = split_note(note).0;
    match at {
        None => author,
        Some(a) if author.is_empty() => a.line(),
        Some(a) => format!("{author}\n{}", a.line()),
    }
}

/// One index row (seven cells, and `anchoredOn` when the kit says where it is anchored).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The bundle's `kit_id`, lowercase with 0x.
    pub id: String,
    /// Export time, whole unix seconds.
    pub created: u64,
    /// This ledger's root (the genesis entry's id).
    pub root: String,
    /// The bundle directory's absolute path.
    pub path: String,
    /// Digests of the original files (the `contents` values already in the bundle's MANIFEST).
    pub contents: Vec<String>,
    /// The author's text: the manifest's `note_md` without its anchoring line.
    pub note_md: String,
    pub link: Option<String>,
    pub anchored_on: Option<AnchoredOn>,
}

/// Where the index file is.
pub fn path_in(machine: &Path) -> PathBuf {
    machine.join(DIR).join(FILE)
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).and_then(|(_, v)| match v {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).map(|(_, v)| v),
        _ => None,
    }
}

fn row_value(r: &Row) -> Value {
    let mut m = Vec::new();
    if let Some(a) = &r.anchored_on {
        m.push(("anchoredOn".to_string(), a.value()));
    }
    m.extend([
        (contents_key().to_string(), Value::Arr(r.contents.iter().map(|c| Value::Str(c.clone())).collect())),
        ("created".to_string(), Value::Int(r.created)),
        ("id".to_string(), Value::Str(r.id.clone())),
    ]);
    if let Some(l) = &r.link {
        m.push(("link".to_string(), Value::Str(l.clone())));
    }
    m.push((note_key().to_string(), Value::Str(r.note_md.clone())));
    m.push(("path".to_string(), Value::Str(r.path.clone())));
    m.push(("root".to_string(), Value::Str(r.root.clone())));
    Value::Obj(m)
}

/// The index's canonical bytes. Writing and reading share one source.
pub fn bytes_of(rows: &[Row]) -> Vec<u8> {
    json::canon_bytes(&Value::Obj(vec![
        ("form".to_string(), Value::Str(FORM.to_string())),
        ("kits".to_string(), Value::Arr(rows.iter().map(row_value).collect())),
    ]))
}

/// Read the index. No file gives `None`; a `form` other than [`FORM`] or a row with missing cells is an
/// error, never silently treated as empty.
pub fn read(machine: &Path) -> Result<Option<Vec<Row>>, Fault> {
    let p = path_in(machine);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    };
    let bad = || Fault::known(Known::SettingsShape, p.display().to_string());
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))?;
    if str_of(&v, "form").as_deref() != Some(FORM) {
        return Err(bad());
    }
    let Some(Value::Arr(kits)) = member(&v, "kits") else { return Err(bad()) };
    let mut out = Vec::new();
    for k in kits {
        let created = match member(k, "created") {
            Some(Value::Int(n)) => *n,
            _ => return Err(bad()),
        };
        let contents = match member(k, contents_key()) {
            Some(Value::Arr(a)) => a.iter().map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None }).collect::<Option<Vec<_>>>().ok_or_else(bad)?,
            _ => return Err(bad()),
        };
        let (Some(id), Some(root), Some(path), Some(note_md)) = (str_of(k, "id"), str_of(k, "root"), str_of(k, "path"), str_of(k, note_key())) else {
            return Err(bad());
        };
        let anchored_on = match member(k, "anchoredOn") {
            None => None,
            Some(a) => Some(AnchoredOn::of_value(a).ok_or_else(bad)?),
        };
        out.push(Row { id, created, root, path, contents, note_md, link: str_of(k, "link"), anchored_on });
    }
    Ok(Some(out))
}

/// Atomic rewrite (temporary name then rename, 0600, through `home::put_at`).
pub fn write(machine: &Path, rows: &[Row]) -> Result<(), Fault> {
    crate::home::put_at(&machine.join(DIR), FILE, &bytes_of(rows))
}

/// At startup: write an empty index when there is none. Returns whether it wrote.
pub fn ensure(machine: &Path) -> Result<bool, Fault> {
    if path_in(machine).exists() {
        return Ok(false);
    }
    write(machine, &[])?;
    Ok(true)
}

/// Hand-filled `link`s (bundle directory absolute path → address; the same bundle exported twice is two rows,
/// each with its own source). No file means empty.
pub fn overrides(machine: &Path) -> Result<Vec<(String, String)>, Fault> {
    let p = machine.join(DIR).join(OVERRIDES);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::KitLinks)? else { return Ok(Vec::new()) };
    match json::parse(&bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))? {
        Value::Obj(m) => m
            .into_iter()
            .map(|(k, v)| if let Value::Str(s) = v { Some((k, s)) } else { None })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Fault::known(Known::SettingsShape, p.display().to_string())),
        _ => Err(Fault::known(Known::SettingsShape, p.display().to_string())),
    }
}

fn write_overrides(machine: &Path, o: &[(String, String)]) -> Result<(), Fault> {
    let v = Value::Obj(o.iter().map(|(k, s)| (k.clone(), Value::Str(s.clone()))).collect());
    crate::local::put(&machine.join(DIR), OVERRIDES, crate::local::Doc::KitLinks, &json::canon_bytes(&v))
}

/// The canonical path in the plain form rows store (`home::plain_path`), so `/tmp` and `/private/tmp` compare
/// equal; the path unchanged when it does not exist.
fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).map(crate::home::plain_path).unwrap_or_else(|_| p.to_path_buf())
}

/// The bundle's path for assembling `link`: relative to the `kits` directory when inside it, otherwise the
/// bundle directory's name.
pub fn rel_of(kits_room: &Path, kit: &Path) -> String {
    let (room, kit) = (canon(kits_room), canon(kit));
    match kit.strip_prefix(&room) {
        Ok(r) if !r.as_os_str().is_empty() => r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/"),
        _ => kit.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
    }
}

/// The default `link`: the publish base (validated by `fetchx::base_of`, ending in `/`) plus the relative
/// path and a trailing `/`. `None` when no base is configured.
pub fn default_link(publish: Option<&str>, rel: &str) -> Option<String> {
    let base = crate::fetchx::base_of(publish?).ok()?;
    if rel.is_empty() {
        return None;
    }
    // Percent-encode each segment so spaces, non-ASCII and reserved characters never appear raw in the URL.
    let enc: Vec<String> = rel.split('/').map(pct).collect();
    Some(format!("{}{}/", base.as_str(), enc.join("/")))
}

/// Percent-encode one path segment (RFC 3986 unreserved characters unchanged, everything else `%XX` per
/// byte).
fn pct(seg: &str) -> String {
    seg.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// Read the bundle's manifest for the `contents` digests (as recorded, not recomputed) and the author's
/// `note_md` without its anchoring line.
pub fn manifest_facts(kit: &Path) -> Result<(Vec<String>, String), Fault> {
    manifest_row_facts(kit).map(|(c, n, _)| (c, n))
}

/// As [`manifest_facts`], plus the anchoring point from the note's last line.
pub fn manifest_row_facts(kit: &Path) -> Result<(Vec<String>, String, Option<AnchoredOn>), Fault> {
    let p = kit.join(zikaron_glue::names::MANIFEST);
    let bytes = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::NotAdoptable, format!("{}: {t:?}", p.display())))?;
    let contents = match member(&v, contents_key()) {
        Some(Value::Arr(a)) => a.iter().filter_map(|r| str_of(r, Field::Content.as_str())).collect(),
        _ => Vec::new(),
    };
    let (note, at) = split_note(&str_of(&v, note_key()).unwrap_or_default());
    Ok((contents, note, at))
}

/// Add a row after an export. Rows are keyed by bundle directory path: the same bundle exported to two places
/// is two rows, and exporting again to the same place replaces the row. `link` uses that path's override,
/// otherwise the default.
pub fn add(machine: &Path, home: &crate::home::Home, kit: &Path, id: &str, created: u64, publish: Option<&str>) -> Result<Row, Fault> {
    let (contents, note_md, anchored_on) = manifest_row_facts(kit)?;
    let root = crate::ledgerx::root_of(home)?;
    let mut rows = read(machine)?.unwrap_or_default();
    let o = overrides(machine)?;
    let path = crate::home::plain_path(kit.to_path_buf()).display().to_string();
    let link = o.iter().find(|(k, _)| *k == path).map(|(_, l)| l.clone()).or_else(|| default_link(publish, &rel_of(&home.dir(crate::home::Slot::Kits), kit)));
    let row = Row { id: id.to_string(), created, root, path, contents, note_md, link, anchored_on };
    rows.retain(|r| r.path != row.path);
    rows.push(row.clone());
    write(machine, &rows)?;
    Ok(row)
}

/// Set `link` by hand: a non-empty value is recorded as an override; an empty one removes the override and
/// restores the default from the home's publish base.
pub fn set_link(machine: &Path, path: &str, link: &str, root: &str, kits_room: &Path, publish: Option<&str>) -> Result<Option<String>, Fault> {
    let mut rows = read(machine)?.unwrap_or_default();
    // Only rows exported from this ledger (`root`) change: restoring the default needs this home's publish
    // base, and other homes' rows are edited from those homes.
    let Some(at) = rows.iter().position(|r| r.path == path && r.root.eq_ignore_ascii_case(root)) else {
        return Err(Fault::known(Known::SubjectMissing, path.to_string()));
    };
    let mut o = overrides(machine)?;
    o.retain(|(k, _)| k != path);
    let t = link.trim();
    let now = if t.is_empty() {
        default_link(publish, &rel_of(kits_room, Path::new(&rows[at].path)))
    } else {
        let u = crate::fetchx::base_of(t)?.as_str().to_string();
        o.push((path.to_string(), u.clone()));
        Some(u)
    };
    rows[at].link = now.clone();
    write_overrides(machine, &o)?;
    write(machine, &rows)?;
    Ok(now)
}

/// After the publish base changes: rows from this ledger (`root`) without an override get `link` rebuilt on
/// the new base, the same way as at add time. Returns how many rows changed.
pub fn relink(machine: &Path, root: &str, kits_room: &Path, publish: Option<&str>) -> Result<usize, Fault> {
    let Some(mut rows) = read(machine)? else { return Ok(0) };
    let o = overrides(machine)?;
    let mut n = 0;
    for r in rows.iter_mut() {
        if o.iter().any(|(k, _)| *k == r.path) || !r.root.eq_ignore_ascii_case(root) {
            continue;
        }
        let l = default_link(publish, &rel_of(kits_room, Path::new(&r.path)));
        if l != r.link {
            r.link = l;
            n += 1;
        }
    }
    if n > 0 {
        write(machine, &rows)?;
    }
    Ok(n)
}

/// Delete the local copy. The row is always removed; the bundle directory is deleted only when its manifest's
/// bundle id equals the row's `id`, so a directory that is gone or now holds another bundle is never touched.
/// The ledger is not affected. Returns the removed row and whether the directory was deleted.
pub fn drop_copy(machine: &Path, path: &str) -> Result<(Row, bool), Fault> {
    let mut rows = read(machine)?.unwrap_or_default();
    let Some(at) = rows.iter().position(|r| r.path == path) else {
        return Err(Fault::known(Known::SubjectMissing, path.to_string()));
    };
    let row = rows[at].clone();
    let dir = PathBuf::from(&row.path);
    let manifest = std::fs::read(dir.join(zikaron_glue::names::MANIFEST)).ok();
    let same = manifest.map(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(&b)).eq_ignore_ascii_case(&row.id)).unwrap_or(false);
    if same {
        std::fs::remove_dir_all(&dir).map_err(|e| classify(&e, &row.path))?;
    }
    rows.remove(at);
    let mut o = overrides(machine)?;
    if o.iter().any(|(k, _)| *k == row.path) {
        o.retain(|(k, _)| *k != row.path);
        write_overrides(machine, &o)?;
    }
    write(machine, &rows)?;
    Ok((row, same))
}

/// When the home moves: rows whose bundle path is under the old home are re-prefixed with the new home.
/// Returns how many rows changed.
pub fn rebase(machine: &Path, from: &Path, to: &Path) -> Result<usize, Fault> {
    let Some(mut rows) = read(machine)? else { return Ok(0) };
    let (from_c, to) = (canon(from), canon(to));
    let mut n = 0;
    let mut moved: Vec<(String, String)> = Vec::new();
    for r in rows.iter_mut() {
        let p = Path::new(&r.path);
        if let Ok(rest) = p.strip_prefix(&from_c).or_else(|_| p.strip_prefix(from)) {
            let new = to.join(rest).display().to_string();
            moved.push((r.path.clone(), new.clone()));
            r.path = new;
            n += 1;
        }
    }
    if n > 0 {
        // Overrides are keyed by bundle path, so they move with it.
        let mut o = overrides(machine)?;
        let mut touched = false;
        for (k, _) in o.iter_mut() {
            if let Some((_, new)) = moved.iter().find(|(old, _)| old == k) {
                *k = new.clone();
                touched = true;
            }
        }
        if touched {
            write_overrides(machine, &o)?;
        }
        write(machine, &rows)?;
    }
    Ok(n)
}
