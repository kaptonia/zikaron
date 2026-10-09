//! Content computation for the anchoring desk: each of the three sources computes its own content hash, plus
//! the mode mark (always the family literal).
//!
//! This layer makes no validity decisions. The `history` field format is checked by the core's
//! `entry::check`; this layer only computes a digest from disk, builds the mode mark object, and assembles the
//! body. When the shape is wrong, the core's refusal token is reported, not a message made up here.
//!
//! The mode mark has no field for a person to fill: `mark` is always the family literal [`FAMILY`], and
//! `toolchain` is always the sha256 of that literal's UTF-8 bytes. The format requires `mode`; only the choice
//! is removed, not the field.
//!
//! The three sources:
//!
//! 1. File: the sha256 of the file's bytes;
//! 2. Directory: each file's relative path and digest form a canonical manifest, and the sha256 of the
//! manifest's bytes is the content;
//! 3. git repository: the sha256 of the HEAD commit object's bytes (see [`crate::gitx`]), with the subject
//! and ancestor count as notes.
//!
//! Digests always come from the core's `cryptox::sha256` and canonical bytes from `json::canon_bytes`;
//! nothing is reimplemented here.

use crate::fault::{classify, Fault, Known};
use std::path::Path;
use zikaron::cryptox;
use zikaron::hexfmt;
use zikaron::json::Value;

/// The three content sources. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    File,
    Dir,
    Git,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::File, Source::Dir, Source::Git];

    pub fn as_str(self) -> &'static str {
        match self {
            Source::File => "file",
            Source::Dir => "dir",
            Source::Git => "git",
        }
    }
}

/// One computed content. `detail` is a note: git's commit subject, a directory's file count, or a file's byte
/// count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub source: Source,
    /// The 32-byte content digest.
    pub digest: [u8; 32],
    /// The path as given.
    pub subject: String,
    /// A human-readable note shown in the UI.
    pub detail: String,
    /// A single file's byte count (the "size" on the terms file card); directories and git repositories have
    /// none.
    pub size: Option<u64>,
}

impl Content {
    pub fn hex(&self) -> String {
        hexfmt::encode(&self.digest)
    }
}

/// A file's content: the sha256 of its bytes (`zikaron_glue::recording`, also used by the command line's
/// `history --file`). Signing and the record bundle attachment check (`kitx::Originals`) use the same function.
pub fn file_digest(bytes: &[u8]) -> [u8; 32] {
    zikaron_glue::recording::content_of(bytes)
}

/// A chosen file's size (a folder or an unreadable path has none); shown beside its name.
pub fn file_size(p: &Path) -> Option<u64> {
    std::fs::metadata(p).ok().filter(|m| m.is_file()).map(|m| m.len())
}

/// File: the sha256 of its bytes.
pub fn of_file(p: &Path) -> Result<Content, Fault> {
    // A folder is not a file: refused by name, so a batch that reaches a folder says why it stopped.
    if p.is_dir() {
        return Err(Fault::known(Known::ContentShape, crate::lang::filln(crate::lang::Key::TailNotAFile, &[&p.display().to_string()])));
    }
    let b = std::fs::read(p).map_err(|e| classify(&e, &p.display().to_string()))?;
    Ok(Content {
        source: Source::File,
        digest: file_digest(&b),
        subject: p.display().to_string(),
        detail: format!("{} bytes", b.len()),
        size: Some(b.len() as u64),
    })
}

/// A directory manifest: one `{path, sha256}` row per file, sorted by path.
///
/// Its shape is a canonical JSON value, so the manifest bytes are unique: the same tree gives the same bytes
/// anywhere. Only regular files are included; other kinds (symbolic links, device files) are left out and
/// counted, so skipping is reported rather than silent.
pub fn manifest(dir: &Path) -> Result<(Value, usize, usize), Fault> {
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut skipped = 0usize;
    walk(dir, dir, &mut rows, &mut skipped)?;
    if rows.is_empty() {
        return Err(Fault::known(Known::DirEmpty, dir.display().to_string()));
    }
    rows.sort();
    let n = rows.len();
    let arr = rows
        .into_iter()
        .map(|(path, sha)| {
            Value::Obj(vec![
                ("path".to_string(), Value::Str(path)),
                ("sha256".to_string(), Value::Str(sha)),
            ])
        })
        .collect();
    Ok((Value::Obj(vec![("files".to_string(), Value::Arr(arr))]), n, skipped))
}

fn walk(root: &Path, at: &Path, out: &mut Vec<(String, String)>, skipped: &mut usize) -> Result<(), Fault> {
    let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(at)
        .map_err(|e| classify(&e, &at.display().to_string()))?
        .map(|e| e.map(|e| e.path()).map_err(|e| classify(&e, &at.display().to_string())))
        .collect::<Result<_, _>>()?;
    names.sort();
    for p in names {
        let md = std::fs::symlink_metadata(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
        if md.is_dir() {
            walk(root, &p, out, skipped)?;
        } else if md.is_file() {
            let b = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
            let rel = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            out.push((rel, hexfmt::encode(&cryptox::sha256(&b))));
        } else {
            *skipped += 1;
        }
    }
    Ok(())
}

/// Directory: the sha256 of the manifest's canonical bytes.
pub fn of_dir(p: &Path) -> Result<Content, Fault> {
    let (doc, files, skipped) = manifest(p)?;
    let bytes = zikaron::json::canon_bytes(&doc);
    Ok(Content {
        source: Source::Dir,
        digest: cryptox::sha256(&bytes),
        subject: p.display().to_string(),
        detail: format!("{files} files · {skipped} skipped · manifest {} bytes", bytes.len()),

        size: None,
    })
}

/// git repository: the sha256 of the HEAD commit object's bytes, with the commit subject and ancestor count
/// as notes.
pub fn of_git(p: &Path) -> Result<Content, Fault> {
    let h = crate::gitx::head_of(p)?;
    Ok(Content {
        source: Source::Git,
        digest: h.content,
        subject: p.display().to_string(),
        detail: format!("{} · {} · {}", &h.commit[..12.min(h.commit.len())], h.ancestors, h.subject),

        size: None,
    })
}

/// Computes the content for a source. Shared by the window and tests.
pub fn of(source: Source, path: &Path) -> Result<Content, Fault> {
    match source {
        Source::File => of_file(path),
        Source::Dir => of_dir(path),
        Source::Git => of_git(path),
    }
}

/// The two fields of the mode mark (`mark` is a token, `toolchain` is hex32; both required).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mode {
    pub mark: String,
    pub toolchain: [u8; 32],
}

/// The family literal this desk emits. It is the only one: all three sources hash bytes (the file's, the
/// manifest's, the commit object's), so one literal describes how every `content` is computed. It is defined
/// in `zikaron_glue::recording`, which the command line also uses.
pub use zikaron_glue::recording::FAMILY;

/// Builds the mode mark. It takes no input: `mark` is always [`FAMILY`], and `toolchain` is always the sha256
/// of its UTF-8 bytes (`zikaron_glue::recording::toolchain`). Nobody chooses or fills it, so it can never be
/// missing.
pub fn mode() -> Mode {
    Mode { mark: FAMILY.to_string(), toolchain: zikaron_glue::recording::toolchain() }
}

/// The body of `history` (`content` and `mode` required, `note_md` optional).
///
/// The key names are copied from the format's field table, as in `entryx`; a typo is refused by the core at
/// once (`E_BODY_FIELD`), never silently accepted.
pub fn history_body(content: &[u8; 32], m: &Mode, note_md: &str) -> Value {
    let mut body = vec![
        ("content".to_string(), Value::Str(hexfmt::encode(content))),
        (
            "mode".to_string(),
            Value::Obj(vec![
                ("mark".to_string(), Value::Str(m.mark.clone())),
                ("toolchain".to_string(), Value::Str(hexfmt::encode(&m.toolchain))),
            ]),
        ),
    ];
    if !note_md.trim().is_empty() {
        body.push((crate::entryx::NOTE_MD.to_string(), Value::Str(note_md.to_string())));
    }
    Value::Obj(body)
}

/// "On whose behalf": the optional `for` member, an extra body member that is plain data. This desk writes it
/// into the body unchanged and never reads it; a sibling app uses it to cross-reference a record bundle's
/// `root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct For {
    pub app: String,
    /// The other party's identity address (hex20, with 0x).
    pub identity: String,
    pub reference: String,
    /// Optional: the other party's seat.
    pub seat: Option<String>,
}

impl For {
    /// Builds from four fields: all empty means no `for`; otherwise `app`, `identity` and `ref` must all be
    /// present and `identity` must be hex20. A missing or malformed field is refused by name. Values are kept
    /// as typed (trimmed, case unchanged).
    pub fn from_fields(app: &str, identity: &str, reference: &str, seat: &str) -> Result<Option<For>, Fault> {
        let (app, identity, reference, seat) = (app.trim(), identity.trim(), reference.trim(), seat.trim());
        if app.is_empty() && identity.is_empty() && reference.is_empty() && seat.is_empty() {
            return Ok(None);
        }
        for (name, v) in [("app", app), ("identity", identity), ("ref", reference)] {
            if v.is_empty() {
                return Err(Fault::known(Known::FieldMissing, format!("for.{name}")));
            }
        }
        let hex = identity.strip_prefix("0x").unwrap_or("");
        if hex.len() != 40 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Fault::known(Known::AddressShape, format!("for.identity {identity}")));
        }
        Ok(Some(For {
            app: app.to_string(),
            identity: identity.to_string(),
            reference: reference.to_string(),
            seat: if seat.is_empty() { None } else { Some(seat.to_string()) },
        }))
    }

    /// The value of the `for` member in the body (canonical key order).
    pub fn value(&self) -> Value {
        let mut m = vec![
            ("app".to_string(), Value::Str(self.app.clone())),
            ("identity".to_string(), Value::Str(self.identity.clone())),
            ("ref".to_string(), Value::Str(self.reference.clone())),
        ];
        if let Some(s) = &self.seat {
            m.push(("seat".to_string(), Value::Str(s.clone())));
        }
        Value::Obj(m)
    }
}

/// The body of `history` with `for` added (only when present).
pub fn history_body_for(content: &[u8; 32], m: &Mode, note_md: &str, target: Option<&For>) -> Value {
    let mut body = history_body(content, m, note_md);
    if let (Value::Obj(members), Some(f)) = (&mut body, target) {
        members.push(("for".to_string(), f.value()));
    }
    body
}

/// The body of `annotation` (`subject` optional, `note_md` required).
pub fn annotation_body(subject: Option<&str>, note_md: &str) -> Value {
    let mut body = vec![(crate::entryx::NOTE_MD.to_string(), Value::Str(note_md.to_string()))];
    if let Some(s) = subject {
        body.push(("subject".to_string(), Value::Str(s.to_string())));
    }
    body.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Obj(body)
}

/// The state of each of the three pipeline steps. Closed, with every intermediate state explicit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Not reached yet.
    Waiting,
    /// This step succeeded.
    Done,
    /// This step failed.
    Failed,
}

impl Step {
    pub fn as_str(self) -> &'static str {
        match self {
            Step::Waiting => "waiting",
            Step::Done => "done",
            Step::Failed => "failed",
        }
    }
}

/// The three steps sign → record → queue. Each has its own state; they are never merged into one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flow {
    pub sign: Step,
    pub land: Step,
    pub queue: Step,
}

impl Default for Flow {
    fn default() -> Flow {
        Flow { sign: Step::Waiting, land: Step::Waiting, queue: Step::Waiting }
    }
}

impl Flow {
    /// The three steps' states, in order, for the UI to draw.
    pub fn steps(&self) -> [Step; 3] {
        [self.sign, self.land, self.queue]
    }
}

/// A passive indicator for a registered repository: the commit anchored last time, and how many commits have
/// been added since.
///
/// Computed once when the page opens, with no polling: the action calls this, not the UI. When it cannot be
/// computed it says so by name, never a guessed number.
pub struct Since {
    pub head: String,
    pub last: String,
    /// Commits added since the last anchoring. `None` when the last one is not on this lineage.
    pub grew: Option<usize>,
}

pub fn since(dir: &Path, last_commit: &str) -> Result<Since, Fault> {
    let repo = crate::gitx::Repo::open(dir)?;
    let head = repo.head()?;
    let all = repo.ancestor_set(&head)?;
    let head_hex = zikaron::hexfmt::encode(&head);
    let head_hex = head_hex.strip_prefix("0x").unwrap_or(&head_hex).to_string();
    let last = last_commit.trim().to_ascii_lowercase();
    // Check ancestry first, then subtract. Subtracting two reachable counts cannot tell whether the last
    // commit is on this lineage: if it is on another branch, the difference is meaningless (often exactly
    // zero) and the UI would wrongly say nothing changed. So membership in the reachable set is checked first.
    let grew = match crate::gitx::Repo::oid(&last) {
        Some(o) if all.contains(&o) => repo.ancestor_set(&o).ok().map(|n| all.len() - n.len()),
        // The last commit is not on HEAD's lineage (or not in this repository at all): that is not "zero
        // added"; there is nothing to compare.
        _ => None,
    };
    Ok(Since { head: head_hex, last, grew })
}
