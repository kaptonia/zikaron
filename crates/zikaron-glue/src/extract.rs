//! Taking a record package's originals out into a folder the person chose. One judge and one way of landing,
//! called by the desktop's "take the originals out" and by the command line's `kit-verify --originals-to`.
//!
//! **Which originals.** A record (`history` entry) certifies its original's `content`. A file original is kept in
//! the package as one file (a `contents` row, kit law §7.3); a signed directory as one file per file of it, each
//! under the directory's name. What the package holds is judged against the record, never against a cache: a
//! file whose sha256 is the record's `content`, or a directory whose manifest, recomputed from the files kept under
//! its name with the names it was signed with (`crate::recording::dir_contents`: the spelling signing a directory
//! uses, or the one a directory signed on Windows had before), hashes to it. [`crate::recordcheck::records`] makes that judgement for the desktop's
//! verification, the command line's `kit-verify` and this module alike; an original that does not match and a
//! record without its original are not taken and are named.
//!
//! **Names.** An original lands under the name it had (`files/zikaron-names.json`, [`crate::kitnames`]) when that
//! name is one plain segment ([`plain_name`]), its package name otherwise, and the plan says so; a directory
//! whose names do not all hold lands whole under its package names. Every package path is a kit path (kit law
//! §7.2) or the package is refused by name.
//!
//! **Landing, all or nothing.** Nothing is written before every name is free in the chosen folder: a name already
//! there refuses the whole batch by name, zero bytes; a symbolic link there is refused by name and never followed.
//! The originals are laid out in a staging folder of our own inside the chosen folder, streamed from the package
//! read again, each one's fingerprint checked against its record, then moved into place one by one without ever
//! replacing. Any step failing takes back what was placed and clears the staging folder: the chosen folder is as
//! it was. Disk writes go through [`crate::landing`].

use std::path::{Path, PathBuf};

/// How a package keeps an original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    File,
    Dir,
}

/// One file of an original in the package: its package path (under `files/`), its names inside the original
/// (none for a file original), and the sha256 of its bytes when the original was judged (hex; for a file original,
/// its content).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub path: String,
    pub rel: Vec<String>,
    pub sha: String,
}

/// An original the package holds, as a candidate: its fingerprint, how it is kept, the name it had, its package
/// name, and its files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Original {
    /// The `contents` row's digest for a file; the recomputed directory digest for a directory (hex, lowercase).
    pub content: String,
    pub kept: Kept,
    /// The name it had: a file's original name (the last name of the path it was signed under, from the names
    /// table, else of its package path), a directory's top name (from the names table, else its package name).
    pub name: String,
    /// Its package name: a file's package path's last segment, a directory's top package segment.
    pub package_name: String,
    pub members: Vec<Member>,
    /// Whether every name it would land under holds ([`plain_name`]): the name and, for a directory, each name of
    /// each of its files.
    pub names_hold: bool,
}

impl Original {
    /// The name it lands under: the name it had when that holds, else its package name.
    pub fn landing_name(&self) -> &str {
        if self.names_hold {
            &self.name
        } else {
            &self.package_name
        }
    }

    /// A member's names inside the landed original: none for a file (the file is the original, landing under
    /// [`Original::landing_name`] whatever folder the package keeps it in); for a directory's file, as signed when
    /// the names hold, else its package names under the directory's top package name.
    fn landing_rel(&self, m: &Member) -> Vec<String> {
        match self.kept {
            Kept::File => Vec::new(),
            Kept::Dir if self.names_hold => m.rel.clone(),
            Kept::Dir => m.path.split('/').skip(1).map(|s| s.to_string()).collect(),
        }
    }
}

/// Whether `s` is one plain name to land a file or folder under on any system: not empty, not `.` or `..`, no
/// path separator, no control character, none of the characters some system will not take in a name
/// (`< > : " | ? *`), not ending in a dot or a space, not a name a system reserves for a device (`CON`, `PRN`,
/// `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, alone or before an extension, in any case), at most 255 bytes.
pub fn plain_name(s: &str) -> bool {
    if s.is_empty() || s == "." || s == ".." || s.len() > 255 {
        return false;
    }
    if s.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*')) {
        return false;
    }
    if s.ends_with('.') || s.ends_with(' ') {
        return false;
    }
    let stem = s.split('.').next().unwrap_or(s).trim_end().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    !device
}

/// The originals a package holds, read from its `contents` rows (content, package path under `files/`), its
/// names table, and `read` (a package path to the file's bytes). A file original per `contents` row, its
/// content as the row lists it and its bytes as read; a directory original per top name under which files are
/// kept two or more names deep, its content recomputed from those files with the names they were signed with and
/// its fingerprint bytes being that manifest's canonical bytes (whose sha256 is the directory's content), once per
/// spelling its record may have (`crate::recording::dir_contents`). A directory with a file that does not read is
/// no candidate.
pub fn originals(contents: &[(String, String)], names: &[(String, String)], read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<(Original, Option<Vec<u8>>)> {
    let original_of = |path: &str| names.iter().find(|(_, to)| to == path).map(|(from, _)| from.clone()).unwrap_or_else(|| path.to_string());
    let mut out: Vec<(Original, Option<Vec<u8>>)> = Vec::new();
    // Directories: top name (as signed), top package name, its members (`None` once a file of it does not read).
    let mut dirs: Vec<(String, String, Option<Vec<Member>>)> = Vec::new();
    for (content, path) in contents {
        let had = original_of(path);
        let segs: Vec<&str> = had.split('/').collect();
        let package_segs: Vec<&str> = path.split('/').collect();
        let bytes = read(path);
        let sha = bytes.as_ref().map(|b| zikaron::hexfmt::encode(&crate::recording::content_of(b)));
        if segs.len() >= 2 && package_segs.len() >= 2 {
            let member = sha.clone().map(|sha| Member { path: path.clone(), rel: segs[1..].iter().map(|s| s.to_string()).collect(), sha });
            match dirs.iter_mut().find(|(top, _, _)| top == segs[0]) {
                Some((_, _, ms)) => match (ms.as_mut(), member) {
                    (Some(v), Some(m)) => v.push(m),
                    _ => *ms = None,
                },
                None => dirs.push((segs[0].to_string(), package_segs[0].to_string(), member.map(|m| vec![m]))),
            }
        }
        let last = package_segs.last().copied().unwrap_or(path.as_str()).to_string();
        // A file lands as itself, under one name: the last of the names it was signed under (a file kept inside a
        // folder had the folder's names before its own).
        let had_last = segs.last().copied().unwrap_or(had.as_str()).to_string();
        let content = content.to_ascii_lowercase();
        out.push((
            Original {
                content: content.clone(),
                kept: Kept::File,
                names_hold: plain_name(&had_last),
                name: had_last,
                package_name: last,
                members: vec![Member { path: path.clone(), rel: Vec::new(), sha: content }],
            },
            bytes,
        ));
    }
    for (top, package_top, members) in dirs {
        let Some(members) = members else { continue };
        let rows: Vec<crate::recording::DirRow> = members
            .iter()
            .map(|m| {
                let rel: Vec<&str> = m.rel.iter().map(|s| s.as_str()).collect();
                (crate::recording::dir_path(&rel), m.sha.clone())
            })
            .collect();
        let names_hold = plain_name(&top) && members.iter().all(|m| m.rel.iter().all(|s| plain_name(s)));
        // One candidate per spelling its record may have (`recording::dir_contents`): the record picks the one it holds.
        let (digest, manifest) = crate::recording::dir_content(&rows);
        for (digest, manifest) in std::iter::once((digest, manifest)).chain(crate::recording::old_dir_content(&rows)) {
            out.push((
                Original { content: zikaron::hexfmt::encode(&digest), kept: Kept::Dir, name: top.clone(), package_name: package_top.clone(), members: members.clone(), names_hold },
                Some(manifest),
            ));
        }
    }
    out
}

/// The originals a package folder holds ([`originals`] over its manifest, names table and files).
pub fn originals_in_dir(dir: &Path) -> Vec<(Original, Option<Vec<u8>>)> {
    let manifest = std::fs::read(dir.join(crate::names::MANIFEST)).unwrap_or_default();
    let contents = crate::recordcheck::contents_of(&manifest);
    let files = dir.join(crate::names::FILES_DIR);
    let names = std::fs::read(files.join(crate::kitnames::NAMES_FILE)).map(|b| crate::kitnames::names_read(&b)).unwrap_or_default();
    originals(&contents, &names, &|p: &str| std::fs::read(files.join(p)).ok())
}

/// The originals an enumeration holds (a grant file, a kit fetched from a publish address).
pub fn originals_in_pairs(pairs: &[(String, Vec<u8>)]) -> Vec<(Original, Option<Vec<u8>>)> {
    let manifest = pairs.iter().find(|(p, _)| p == crate::names::MANIFEST).map(|(_, b)| b.clone()).unwrap_or_default();
    let contents = crate::recordcheck::contents_of(&manifest);
    let under = |p: &str| format!("{}/{}", crate::names::FILES_DIR, p);
    let names = pairs.iter().find(|(p, _)| *p == under(crate::kitnames::NAMES_FILE)).map(|(_, b)| crate::kitnames::names_read(b)).unwrap_or_default();
    originals(&contents, &names, &|p: &str| pairs.iter().find(|(x, _)| *x == under(p)).map(|(_, b)| b.clone()))
}

/// Why a record's original is not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The package keeps a file for it whose bytes are not the original it records.
    Mismatch,
    /// The package keeps no original for it.
    Missing,
}

/// A record whose original is not taken: the record, its content, why, and the name of what the package keeps
/// for it (none when it keeps nothing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Left {
    pub entry_id: String,
    pub content: String,
    pub why: Why,
    pub name: Option<String>,
}

/// What taking the originals out of a package would do: the originals to take (one per content, in landing-name
/// order) and the records left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub take: Vec<Original>,
    pub left: Vec<Left>,
}

impl Plan {
    /// How many files the originals taken hold.
    pub fn files(&self) -> usize {
        self.take.iter().map(|o| o.members.len()).sum()
    }
}

/// Why the originals were not taken. Each says nothing was written in the chosen folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The package does not pass the kit law: its verdict and subject.
    Kit { verdict: String, subject: Option<String> },
    /// A single-file package does not read as one: the layout's code and subject.
    Unpack { code: String, subject: String },
    /// A path in the package is not a kit path (kit law §7.2).
    Path(String),
    /// The package holds no original matching its record.
    Nothing,
    /// Two originals would land under one name (names a case-folding file system takes as one included).
    Clash(Vec<String>),
    /// The chosen place is not a folder.
    NotAFolder(String),
    /// These names are already in the chosen folder.
    Exists(Vec<String>),
    /// A symbolic link stands at this name in the chosen folder.
    Link(String),
    /// An original laid out does not have the fingerprint its record certifies (the package changed meanwhile).
    Changed(String),
    /// A disk operation failed and what was placed was taken back: the subject and the system's words.
    Landing(String, String),
}

/// The closed table of refusal codes [`Refused::code`] gives (beside the kit law's verdicts and the layout's codes).
pub mod code {
    pub const PATH: &str = "PATH";
    pub const NOTHING: &str = "NOTHING";
    pub const CLASH: &str = "CLASH";
    pub const NOT_A_FOLDER: &str = "NOT_A_FOLDER";
    pub const EXISTS: &str = "EXISTS";
    pub const LINK: &str = "LINK";
    pub const CHANGED: &str = "CHANGED";
    pub const LANDING: &str = "LANDING";
    pub const ALL: [&str; 8] = [PATH, NOTHING, CLASH, NOT_A_FOLDER, EXISTS, LINK, CHANGED, LANDING];
}

/// Each refusal's words, Chinese and English: the one place both the desktop and the command line take them.
pub mod words {
    pub const PATH: (&str, &str) = ("记录包里有不合记录包路径形制的路径,没有取出。", "A path in the record package is not of the package path form. Nothing was taken out.");
    pub const NOTHING: (&str, &str) = ("记录包里没有与记录对得上的原件,没有取出。", "The record package holds no original that matches its record. Nothing was taken out.");
    pub const CLASH: (&str, &str) = ("有两份原件要取成同一个名字,没有取出。", "Two originals would take the same name. Nothing was taken out.");
    pub const NOT_A_FOLDER: (&str, &str) = ("所选的不是一个文件夹,没有取出。", "What was chosen is not a folder. Nothing was taken out.");
    pub const EXISTS: (&str, &str) = ("所选文件夹里已有同名的文件或文件夹,没有取出。", "The chosen folder already holds items with these names. Nothing was taken out.");
    pub const LINK: (&str, &str) = ("所选文件夹里这个名字是一个链接,没有取出。", "This name in the chosen folder is a link. Nothing was taken out.");
    pub const CHANGED: (&str, &str) = ("取出的原件与记录对不上,记录包可能在取出时变了,没有取出。", "An original taken out does not match its record; the package may have changed meanwhile. Nothing was taken out.");
    pub const LANDING: (&str, &str) = ("写盘失败,已撤回,没有取出。", "Writing to disk failed and was taken back. Nothing was taken out.");
}

impl Refused {
    /// Its code: the kit law's verdict for a package that does not pass, the layout's code for a single file that
    /// does not read, else one of [`code`].
    pub fn code(&self) -> String {
        match self {
            Refused::Kit { verdict, .. } => verdict.clone(),
            Refused::Unpack { code, .. } => code.clone(),
            Refused::Path(_) => code::PATH.into(),
            Refused::Nothing => code::NOTHING.into(),
            Refused::Clash(_) => code::CLASH.into(),
            Refused::NotAFolder(_) => code::NOT_A_FOLDER.into(),
            Refused::Exists(_) => code::EXISTS.into(),
            Refused::Link(_) => code::LINK.into(),
            Refused::Changed(_) => code::CHANGED.into(),
            Refused::Landing(..) => code::LANDING.into(),
        }
    }

    /// The names it is about (names already there, names that clash), in order.
    pub fn names(&self) -> &[String] {
        match self {
            Refused::Exists(n) | Refused::Clash(n) => n,
            _ => &[],
        }
    }

    /// What it names besides: the path, the subject, the system's words.
    pub fn detail(&self) -> Option<String> {
        match self {
            Refused::Kit { subject, .. } => subject.clone(),
            Refused::Unpack { subject, .. } => Some(subject.clone()),
            Refused::Path(p) | Refused::NotAFolder(p) | Refused::Link(p) | Refused::Changed(p) => Some(p.clone()),
            Refused::Landing(s, w) => Some(format!("{s}: {w}")),
            Refused::Nothing | Refused::Exists(_) | Refused::Clash(_) => None,
        }
    }

    /// Its words (Chinese, English) when it is this module's own; `None` for the kit law's and the layout's, which
    /// their own readers word.
    pub fn words(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Refused::Kit { .. } | Refused::Unpack { .. } => None,
            Refused::Path(_) => Some(words::PATH),
            Refused::Nothing => Some(words::NOTHING),
            Refused::Clash(_) => Some(words::CLASH),
            Refused::NotAFolder(_) => Some(words::NOT_A_FOLDER),
            Refused::Exists(_) => Some(words::EXISTS),
            Refused::Link(_) => Some(words::LINK),
            Refused::Changed(_) => Some(words::CHANGED),
            Refused::Landing(..) => Some(words::LANDING),
        }
    }
}

/// The plan for a package folder: it must pass the kit law (judged again here, never taken from an earlier
/// verification), every package path of an original must be a kit path, and each record is judged by
/// [`crate::recordcheck::records`] against the originals the folder holds. Records whose original matches give
/// the originals to take, one per content; the others are left and named. Of the candidates for one content, a file
/// attached alone comes before one the package keeps inside a folder (it lands under the name it was signed with
/// alone); a file original that a folder being taken out already holds (its package path one of that folder's files)
/// is delivered by the folder and is not placed again beside it. Two originals that would land under one name (in
/// any case) are refused by name.
pub fn plan(dir: &Path) -> Result<Plan, Refused> {
    crate::seam_v2();
    if let zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } = zikaron_kit::kitdir::verify_kit(dir) {
        return Err(Refused::Kit { verdict: verdict.as_str().to_string(), subject });
    }
    let entries = crate::read::kit_entries(dir).map_err(|p| Refused::Kit { verdict: zikaron_kit::tokens::KitFailToken::Unreadable.as_str().to_string(), subject: Some(p) })?;
    let held = originals_in_dir(dir);
    plan_of(&entries, &held)
}

/// The plan for a package that passed the kit law, from its entries and the originals it holds ([`originals_in_dir`]),
/// as [`plan`] makes it: the desktop's verification, having read both already, lists what would be taken by it.
pub fn plan_of(entries: &[Vec<u8>], held: &[(Original, Option<Vec<u8>>)]) -> Result<Plan, Refused> {
    let pairs: Vec<(String, Option<Vec<u8>>)> = held.iter().map(|(o, b)| (o.content.clone(), b.clone())).collect();
    let rows = crate::recordcheck::records(entries, &pairs, None);
    let mut take: Vec<Original> = Vec::new();
    let mut left: Vec<Left> = Vec::new();
    for r in rows {
        match r.original {
            crate::recordcheck::Original::Match => {
                if take.iter().any(|o| o.content == r.content) {
                    continue;
                }
                // The candidate that matched: one whose fingerprint bytes hash to this record's content, a file
                // attached alone first.
                let hit = candidates(held, &r.content).find(|(_, b)| b.as_ref().map(|x| zikaron::hexfmt::encode(&crate::recording::content_of(x)) == r.content).unwrap_or(false));
                // A folder a record of its other spelling already takes out is taken out once.
                if let Some((o, _)) = hit.filter(|(o, _)| !take.iter().any(|t| t.kept == Kept::Dir && o.kept == Kept::Dir && t.members == o.members)) {
                    take.push(o.clone());
                }
            }
            crate::recordcheck::Original::Mismatch => {
                let name = candidates(held, &r.content).find(|(o, _)| o.kept == Kept::File).map(|(o, _)| o.landing_name().to_string());
                left.push(Left { entry_id: r.id, content: r.content, why: Why::Mismatch, name });
            }
            crate::recordcheck::Original::Missing => left.push(Left { entry_id: r.id, content: r.content, why: Why::Missing, name: None }),
        }
    }
    // A file a folder being taken out holds is delivered by the folder.
    let in_folders: Vec<String> = take.iter().filter(|o| o.kept == Kept::Dir).flat_map(|o| o.members.iter().map(|m| m.path.clone())).collect();
    take.retain(|o| o.kept == Kept::Dir || !o.members.iter().all(|m| in_folders.contains(&m.path)));
    for o in &take {
        if let Some(m) = o.members.iter().find(|m| !zikaron_kit::kitdir::is_kit_path(&m.path)) {
            return Err(Refused::Path(m.path.clone()));
        }
    }
    take.sort_by(|a, b| a.landing_name().cmp(b.landing_name()));
    let mut clash: Vec<String> = Vec::new();
    for (i, o) in take.iter().enumerate() {
        let n = o.landing_name();
        if take[..i].iter().any(|p| p.landing_name().to_lowercase() == n.to_lowercase()) && !clash.iter().any(|c| c == n) {
            clash.push(n.to_string());
        }
        // Within a directory, two files under one name in any case.
        if o.kept == Kept::Dir {
            let rels: Vec<String> = o.members.iter().map(|m| o.landing_rel(m).join("/").to_lowercase()).collect();
            if rels.iter().enumerate().any(|(k, r)| rels[..k].contains(r)) && !clash.iter().any(|c| c == n) {
                clash.push(n.to_string());
            }
        }
    }
    if !clash.is_empty() {
        return Err(Refused::Clash(clash));
    }
    Ok(Plan { take, left })
}

/// The candidates for `content`, a file attached alone (its package path one name) before one kept inside a folder,
/// otherwise in the package's order.
fn candidates<'a>(held: &'a [(Original, Option<Vec<u8>>)], content: &'a str) -> impl Iterator<Item = &'a (Original, Option<Vec<u8>>)> {
    let deep = |o: &Original| o.kept == Kept::File && o.members.iter().any(|m| m.path.contains('/'));
    let of = move |o: &Original| o.content == content;
    held.iter().filter(move |(o, _)| of(o) && !deep(o)).chain(held.iter().filter(move |(o, _)| of(o) && deep(o)))
}

/// Whether placing is treated as failing after this many originals are in place ([`pretend_place_fails_after`]).
static FAIL_AFTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);

/// Treat placing as failing once `n` originals are in place (`None`: never), from now on in this process. Lets tests
/// reach the taking back of what was placed (the system refuses a move in place only on rare faults); nothing in
/// the product calls it.
pub fn pretend_place_fails_after(n: Option<usize>) {
    FAIL_AFTER.store(n.unwrap_or(usize::MAX), std::sync::atomic::Ordering::SeqCst);
}

/// What was taken out: where, and the names placed, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
    pub to: PathBuf,
    pub names: Vec<String>,
}

/// Take `plan`'s originals out of the package folder `dir` into the folder `to`, all or nothing (the module's
/// head says how). `dir`'s files are read again here.
pub fn land(plan: &Plan, dir: &Path, to: &Path) -> Result<Landed, Refused> {
    crate::seam_v2();
    if plan.take.is_empty() {
        return Err(Refused::Nothing);
    }
    match std::fs::metadata(to) {
        Ok(m) if m.is_dir() => {}
        _ => return Err(Refused::NotAFolder(to.display().to_string())),
    }
    // Every name free first: a link is refused by name, names already there all together.
    let mut there: Vec<String> = Vec::new();
    for o in &plan.take {
        let n = o.landing_name();
        match free(to, n)? {
            Free::Yes => {}
            Free::Taken => there.push(n.to_string()),
        }
    }
    if !there.is_empty() {
        return Err(Refused::Exists(there));
    }
    let staging = crate::landing::staging_beside(&to.join(code::LANDING)).map_err(trouble)?;
    let made = std::fs::create_dir(&staging).map_err(|e| Refused::Landing(staging.display().to_string(), e.to_string()));
    made?;
    let laid = lay_out(plan, dir, &staging);
    let mut placed: Vec<(PathBuf, Kept)> = Vec::new();
    let result = laid.and_then(|()| {
        for o in &plan.take {
            if placed.len() >= FAIL_AFTER.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(Refused::Landing(o.landing_name().to_string(), "placing refused (pretended)".into()));
            }
            let n = o.landing_name();
            match free(to, n)? {
                Free::Yes => {}
                Free::Taken => return Err(Refused::Exists(vec![n.to_string()])),
            }
            let (from, at) = (staging.join(n), to.join(n));
            match o.kept {
                Kept::File => crate::landing::place_file(&from, &at),
                Kept::Dir => crate::landing::land_tree(&at, &from),
            }
            .map_err(|t| match t {
                crate::landing::Trouble::Occupied(_) => Refused::Exists(vec![n.to_string()]),
                t => trouble(t),
            })?;
            placed.push((at, o.kept));
        }
        Ok(())
    });
    match result {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&staging);
            Ok(Landed { to: to.to_path_buf(), names: plan.take.iter().map(|o| o.landing_name().to_string()).collect() })
        }
        Err(r) => {
            for (at, kept) in placed.iter().rev() {
                crate::landing::take_back(at, *kept == Kept::Dir);
            }
            let _ = std::fs::remove_dir_all(&staging);
            Err(r)
        }
    }
}

enum Free {
    Yes,
    Taken,
}

/// Whether `name` is free in `to`: nothing there; a symbolic link there is refused by name.
fn free(to: &Path, name: &str) -> Result<Free, Refused> {
    match std::fs::symlink_metadata(to.join(name)) {
        Ok(m) if m.file_type().is_symlink() => Err(Refused::Link(to.join(name).display().to_string())),
        Ok(_) => Ok(Free::Taken),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Free::Yes),
        Err(e) => Err(Refused::Landing(to.join(name).display().to_string(), e.to_string())),
    }
}

fn trouble(t: crate::landing::Trouble) -> Refused {
    let w = t.said().unwrap_or(t.code()).to_string();
    Refused::Landing(t.subject().to_string(), w)
}

/// Lay every original out under `staging`, streamed from the package's files read again, and check each one's
/// fingerprint against its record.
fn lay_out(plan: &Plan, dir: &Path, staging: &Path) -> Result<(), Refused> {
    let files = dir.join(crate::names::FILES_DIR);
    for o in &plan.take {
        let top = staging.join(o.landing_name());
        for m in &o.members {
            let at = o.landing_rel(m).iter().fold(top.clone(), |p, s| p.join(s));
            if let Some(p) = at.parent() {
                crate::landing::mkdir(p).map_err(trouble)?;
            }
            let mut src = std::fs::File::open(files.join(&m.path)).map_err(|e| Refused::Landing(m.path.clone(), e.to_string()))?;
            crate::landing::put_new_from(&at, &mut src).map_err(trouble)?;
        }
        // Each one checked against its record: a file by its bytes; a directory laid out under the names it was
        // signed with by its recomputed manifest; one laid out under its package names, file by file, against the
        // bytes its record's manifest was computed from.
        let holds = match o.kept {
            Kept::File => sha_at(&top)? == o.content,
            Kept::Dir if o.names_hold => {
                let (rows, skipped) = crate::recording::dir_rows(&top).map_err(|(e, at)| Refused::Landing(at.display().to_string(), e.to_string()))?;
                skipped == 0 && crate::recording::dir_holds(&rows, &o.content)
            }
            Kept::Dir => {
                let mut all = true;
                for m in &o.members {
                    all &= sha_at(&o.landing_rel(m).iter().fold(top.clone(), |p, s| p.join(s)))? == m.sha;
                }
                all
            }
        };
        if !holds {
            return Err(Refused::Changed(o.landing_name().to_string()));
        }
    }
    Ok(())
}

/// The sha256 of the file at `at`, in hex.
fn sha_at(at: &Path) -> Result<String, Refused> {
    std::fs::read(at).map(|b| zikaron::hexfmt::encode(&crate::recording::content_of(&b))).map_err(|e| Refused::Landing(at.display().to_string(), e.to_string()))
}

/// Take the originals out of the package at `kit` (a folder, or a single file read out anew into a folder of its
/// own and removed after) into `to`: the plan judged again on what is read now, then landed.
pub fn take(kit: &Path, to: &Path) -> Result<(Plan, Landed), Refused> {
    crate::seam_v2();
    let unpacked = if crate::container::is_container_at(kit) {
        Some(crate::read::unpack(kit).map_err(|b| Refused::Unpack { code: b.code().to_string(), subject: b.subject() })?)
    } else {
        None
    };
    let dir = unpacked.as_ref().map(|u| u.dir.as_path()).unwrap_or(kit);
    let p = plan(dir)?;
    let landed = land(&p, dir, to)?;
    Ok((p, landed))
}

/// The plan for the package at `kit` (a folder or a single file read out into a folder of its own and removed
/// after), without landing anything: what the desktop's sheet lists before "Take out".
pub fn plan_at(kit: &Path) -> Result<Plan, Refused> {
    let unpacked = if crate::container::is_container_at(kit) {
        Some(crate::read::unpack(kit).map_err(|b| Refused::Unpack { code: b.code().to_string(), subject: b.subject() })?)
    } else {
        None
    };
    plan(unpacked.as_ref().map(|u| u.dir.as_path()).unwrap_or(kit))
}
