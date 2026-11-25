use crate::fault::{classify, Fault, Known};
use std::path::Path;
use zikaron::cryptox;
use zikaron::hexfmt;
use zikaron::json::Value;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub source: Source,
    pub digest: [u8; 32],
    pub subject: String,
    pub detail: String,
    pub size: Option<u64>,
}

impl Content {
    pub fn hex(&self) -> String {
        hexfmt::encode(&self.digest)
    }
}

pub fn file_digest(bytes: &[u8]) -> [u8; 32] {
    zikaron_glue::recording::content_of(bytes)
}

pub fn file_size(p: &Path) -> Option<u64> {
    std::fs::metadata(p).ok().filter(|m| m.is_file()).map(|m| m.len())
}

pub fn of_file(p: &Path) -> Result<Content, Fault> {
    if p.is_dir() {
        return Err(Fault::known(Known::ContentShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailNotAFile, &[&p.display().to_string()]))));
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

pub fn manifest(dir: &Path) -> Result<(Value, usize, usize), Fault> {
    let (rows, skipped) = zikaron_glue::recording::dir_rows(dir).map_err(|(e, at)| classify(&e, &at.display().to_string()))?;
    if rows.is_empty() {
        return Err(Fault::known(Known::DirEmpty, dir.display().to_string()));
    }
    let n = rows.len();
    Ok((zikaron_glue::recording::dir_manifest(&rows), n, skipped))
}

pub fn dir_digests(p: &Path) -> Result<Vec<[u8; 32]>, Fault> {
    let (doc, _, _) = manifest(p)?;
    Ok(zikaron_glue::recording::dir_contents(&zikaron_glue::recording::manifest_rows(&doc)).into_iter().map(|(d, _)| d).collect())
}

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

pub fn of(source: Source, path: &Path) -> Result<Content, Fault> {
    match source {
        Source::File => of_file(path),
        Source::Dir => of_dir(path),
        Source::Git => of_git(path),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mode {
    pub mark: String,
    pub toolchain: [u8; 32],
}

pub use zikaron_glue::recording::FAMILY;

pub fn mode() -> Mode {
    Mode { mark: FAMILY.to_string(), toolchain: zikaron_glue::recording::toolchain() }
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct For {
    pub app: String,
    pub identity: String,
    pub reference: String,
    pub seat: Option<String>,
}

impl For {
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

pub fn history_body_for(content: &[u8; 32], m: &Mode, note_md: &str, target: Option<&For>) -> Value {
    let mut body = history_body(content, m, note_md);
    if let (Value::Obj(members), Some(f)) = (&mut body, target) {
        members.push(("for".to_string(), f.value()));
    }
    body
}

pub fn annotation_body(subject: Option<&str>, note_md: &str) -> Value {
    let mut body = vec![(crate::entryx::NOTE_MD.to_string(), Value::Str(note_md.to_string()))];
    if let Some(s) = subject {
        body.push(("subject".to_string(), Value::Str(s.to_string())));
    }
    body.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Obj(body)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Waiting,
    Done,
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
    pub fn steps(&self) -> [Step; 3] {
        [self.sign, self.land, self.queue]
    }
}

pub struct Since {
    pub head: String,
    pub last: String,
    pub grew: Option<usize>,
}

pub fn since(dir: &Path, last_commit: &str) -> Result<Since, Fault> {
    let repo = crate::gitx::Repo::open(dir)?;
    let head = repo.head()?;
    let all = repo.ancestor_set(&head)?;
    let head_hex = zikaron::hexfmt::encode(&head);
    let head_hex = head_hex.strip_prefix("0x").unwrap_or(&head_hex).to_string();
    let last = last_commit.trim().to_ascii_lowercase();
    let grew = match crate::gitx::Repo::oid(&last) {
        Some(o) if all.contains(&o) => repo.ancestor_set(&o).ok().map(|n| all.len() - n.len()),
        _ => None,
    };
    Ok(Since { head: head_hex, last, grew })
}
