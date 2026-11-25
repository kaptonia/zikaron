use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    File,
    Dir,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub path: String,
    pub rel: Vec<String>,
    pub sha: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Original {
    pub content: String,
    pub kept: Kept,
    pub name: String,
    pub package_name: String,
    pub members: Vec<Member>,
    pub names_hold: bool,
    pub strict: bool,
}

impl Original {
    pub fn landing_name(&self) -> &str {
        if self.names_hold || self.strict {
            &self.name
        } else {
            &self.package_name
        }
    }

    fn landing_rel(&self, m: &Member) -> Vec<String> {
        match self.kept {
            Kept::File => Vec::new(),
            Kept::Dir if self.names_hold || self.strict => m.rel.clone(),
            Kept::Dir => m.path.split('/').skip(1).map(|s| s.to_string()).collect(),
        }
    }
}

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

pub fn one_name(s: &str) -> bool {
    if s.is_empty() || s == "." || s == ".." || s.contains('/') || s.contains('\0') {
        return false;
    }
    if cfg!(windows) {
        return plain_name(s);
    }
    true
}

pub fn originals_v2(manifest: &[u8], read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<(Original, Option<Vec<u8>>)> {
    use zikaron::json::Value;
    let Ok(v) = zikaron::json::parse(manifest) else { return Vec::new() };
    let rows: Vec<(String, Option<String>)> = match v.member(crate::names::Field::Files.as_str()) {
        Some(Value::Arr(a)) => a
            .iter()
            .filter_map(|r| {
                let path = r.member(crate::names::Field::Path.as_str()).and_then(Value::as_str)?.to_string();
                let name = r.member(crate::names::Field::NameMd.as_str()).and_then(Value::as_str).map(str::to_string);
                Some((path, name))
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut out: Vec<(Original, Option<Vec<u8>>)> = Vec::new();
    let contents = crate::recordcheck::contents_named(manifest);
    let dirs: Vec<String> = contents.iter().filter(|(_, p, _)| !rows.iter().any(|(r, _)| r == p)).map(|(_, p, _)| format!("{p}/")).collect();
    for (content, path, dir_name) in contents {
        let content = content.to_ascii_lowercase();
        let package_name = path.rsplit('/').next().unwrap_or(&path).to_string();
        if let Some((_, name_md)) = rows.iter().find(|(p, _)| *p == path) {
            let under = dirs.iter().any(|d| path.starts_with(d.as_str()));
            let name = match name_md {
                Some(n) if under => n.rsplit('/').next().unwrap_or(n).to_string(),
                Some(n) => n.clone(),
                None => package_name.clone(),
            };
            out.push((
                Original {
                    content: content.clone(),
                    kept: Kept::File,
                    names_hold: one_name(&name),
                    name,
                    package_name,
                    members: vec![Member { path: path.clone(), rel: Vec::new(), sha: content }],
                    strict: true,
                },
                read(&path),
            ));
            continue;
        }
        let pre = format!("{path}/");
        let mut members: Vec<Member> = Vec::new();
        let mut whole = true;
        for (p, name_md) in rows.iter().filter(|(p, _)| p.starts_with(&pre)) {
            let rel: Vec<String> = name_md.clone().unwrap_or_else(|| p[pre.len()..].to_string()).split('/').map(str::to_string).collect();
            match read(p) {
                Some(b) => members.push(Member { path: p.clone(), rel, sha: zikaron::hexfmt::encode(&crate::recording::content_of(&b)) }),
                None => whole = false,
            }
        }
        if !whole || members.is_empty() {
            continue;
        }
        let dir_rows: Vec<crate::recording::DirRow> = members.iter().map(|m| (m.rel.join("/"), m.sha.clone())).collect();
        let (digest, bytes) = crate::recording::dir_content(&dir_rows);
        let name = dir_name.unwrap_or_else(|| package_name.clone());
        let names_hold = one_name(&name) && members.iter().all(|m| m.rel.iter().all(|s| one_name(s)));
        out.push((
            Original { content: zikaron::hexfmt::encode(&digest), kept: Kept::Dir, name, package_name, members, names_hold, strict: true },
            Some(bytes),
        ));
    }
    out
}

pub fn originals(contents: &[(String, String)], names: &[(String, String)], read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<(Original, Option<Vec<u8>>)> {
    let original_of = |path: &str| names.iter().find(|(_, to)| to == path).map(|(from, _)| from.clone()).unwrap_or_else(|| path.to_string());
    let mut out: Vec<(Original, Option<Vec<u8>>)> = Vec::new();
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
                strict: false,
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
        let (digest, manifest) = crate::recording::dir_content(&rows);
        for (digest, manifest) in std::iter::once((digest, manifest)).chain(crate::recording::old_dir_content(&rows)) {
            out.push((
                Original { content: zikaron::hexfmt::encode(&digest), kept: Kept::Dir, name: top.clone(), package_name: package_top.clone(), members: members.clone(), names_hold, strict: false },
                Some(manifest),
            ));
        }
    }
    out
}

pub fn originals_in_dir(dir: &Path) -> Vec<(Original, Option<Vec<u8>>)> {
    let manifest = std::fs::read(dir.join(crate::names::MANIFEST)).unwrap_or_default();
    let files = dir.join(crate::names::FILES_DIR);
    if !kit1(&manifest) {
        return originals_v2(&manifest, &|p: &str| std::fs::read(files.join(p)).ok());
    }
    let contents = crate::recordcheck::contents_of(&manifest);
    let names = std::fs::read(files.join(crate::kitnames::NAMES_FILE)).map(|b| crate::kitnames::names_read(&b)).unwrap_or_default();
    originals(&contents, &names, &|p: &str| std::fs::read(files.join(p)).ok())
}

pub fn originals_in_pairs(pairs: &[(String, Vec<u8>)]) -> Vec<(Original, Option<Vec<u8>>)> {
    let manifest = pairs.iter().find(|(p, _)| p == crate::names::MANIFEST).map(|(_, b)| b.clone()).unwrap_or_default();
    let under = |p: &str| format!("{}/{}", crate::names::FILES_DIR, p);
    if !kit1(&manifest) {
        return originals_v2(&manifest, &|p: &str| pairs.iter().find(|(x, _)| *x == under(p)).map(|(_, b)| b.clone()));
    }
    let contents = crate::recordcheck::contents_of(&manifest);
    let names = pairs.iter().find(|(p, _)| *p == under(crate::kitnames::NAMES_FILE)).map(|(_, b)| crate::kitnames::names_read(b)).unwrap_or_default();
    originals(&contents, &names, &|p: &str| pairs.iter().find(|(x, _)| *x == under(p)).map(|(_, b)| b.clone()))
}

fn kit1(manifest: &[u8]) -> bool {
    zikaron_kit::v2::dispatch::manifest_law(Some(manifest)) == zikaron_kit::v2::dispatch::Law::Kit1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Mismatch,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Left {
    pub entry_id: String,
    pub content: String,
    pub why: Why,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub take: Vec<Original>,
    pub left: Vec<Left>,
}

impl Plan {
    pub fn files(&self) -> usize {
        self.take.iter().map(|o| o.members.len()).sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    Kit { verdict: String, subject: Option<String> },
    Unpack { code: String, subject: String },
    Path(String),
    Nothing,
    Clash(Vec<String>),
    Name(Vec<String>),
    NotAFolder(String),
    Exists(Vec<String>),
    Link(String),
    Changed(String),
    Landing(String, String),
}

pub mod code {
    pub const PATH: &str = "PATH";
    pub const NOTHING: &str = "NOTHING";
    pub const CLASH: &str = "CLASH";
    pub const NAME: &str = "NAME";
    pub const NOT_A_FOLDER: &str = "NOT_A_FOLDER";
    pub const EXISTS: &str = "EXISTS";
    pub const LINK: &str = "LINK";
    pub const CHANGED: &str = "CHANGED";
    pub const LANDING: &str = "LANDING";
    pub const ALL: [&str; 9] = [PATH, NOTHING, CLASH, NAME, NOT_A_FOLDER, EXISTS, LINK, CHANGED, LANDING];
}

pub mod words {
    pub const PATH: (&str, &str) = ("记录包里有不合记录包路径形制的路径,没有取出。", "A path in the record package is not of the package path form. Nothing was taken out.");
    pub const NOTHING: (&str, &str) = ("记录包里没有与记录对得上的原件,没有取出。", "The record package holds no original that matches its record. Nothing was taken out.");
    pub const CLASH: (&str, &str) = ("有两份原件要取成同一个名字,没有取出。", "Two originals would take the same name. Nothing was taken out.");
    pub const NAME: (&str, &str) = ("这台电脑建不成记录包里的这些名字,没有取出。", "This computer can't create these names from the record package. Nothing was taken out.");
    pub const NOT_A_FOLDER: (&str, &str) = ("所选的不是一个文件夹,没有取出。", "What was chosen is not a folder. Nothing was taken out.");
    pub const EXISTS: (&str, &str) = ("所选文件夹里已有同名的文件或文件夹,没有取出。", "The chosen folder already holds items with these names. Nothing was taken out.");
    pub const LINK: (&str, &str) = ("所选文件夹里这个名字是一个链接,没有取出。", "This name in the chosen folder is a link. Nothing was taken out.");
    pub const CHANGED: (&str, &str) = ("取出的原件与记录对不上,记录包可能在取出时变了,没有取出。", "An original taken out does not match its record; the package may have changed meanwhile. Nothing was taken out.");
    pub const LANDING: (&str, &str) = ("写盘失败,已撤回,没有取出。", "Writing to disk failed and was taken back. Nothing was taken out.");
}

impl Refused {
    pub fn code(&self) -> String {
        match self {
            Refused::Kit { verdict, .. } => verdict.clone(),
            Refused::Unpack { code, .. } => code.clone(),
            Refused::Path(_) => code::PATH.into(),
            Refused::Nothing => code::NOTHING.into(),
            Refused::Clash(_) => code::CLASH.into(),
            Refused::Name(_) => code::NAME.into(),
            Refused::NotAFolder(_) => code::NOT_A_FOLDER.into(),
            Refused::Exists(_) => code::EXISTS.into(),
            Refused::Link(_) => code::LINK.into(),
            Refused::Changed(_) => code::CHANGED.into(),
            Refused::Landing(..) => code::LANDING.into(),
        }
    }

    pub fn names(&self) -> &[String] {
        match self {
            Refused::Exists(n) | Refused::Clash(n) | Refused::Name(n) => n,
            _ => &[],
        }
    }

    pub fn detail(&self) -> Option<String> {
        match self {
            Refused::Kit { subject, .. } => subject.clone(),
            Refused::Unpack { subject, .. } => Some(subject.clone()),
            Refused::Path(p) | Refused::NotAFolder(p) | Refused::Link(p) | Refused::Changed(p) => Some(p.clone()),
            Refused::Landing(s, w) => Some(format!("{s}: {w}")),
            Refused::Nothing | Refused::Exists(_) | Refused::Clash(_) | Refused::Name(_) => None,
        }
    }

    pub fn words(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Refused::Kit { .. } | Refused::Unpack { .. } => None,
            Refused::Path(_) => Some(words::PATH),
            Refused::Nothing => Some(words::NOTHING),
            Refused::Clash(_) => Some(words::CLASH),
            Refused::Name(_) => Some(words::NAME),
            Refused::NotAFolder(_) => Some(words::NOT_A_FOLDER),
            Refused::Exists(_) => Some(words::EXISTS),
            Refused::Link(_) => Some(words::LINK),
            Refused::Changed(_) => Some(words::CHANGED),
            Refused::Landing(..) => Some(words::LANDING),
        }
    }
}

pub fn plan(dir: &Path) -> Result<Plan, Refused> {
    crate::seam_v2();
    if let Err((verdict, subject)) = crate::book::kit_verdict(dir) {
        return Err(Refused::Kit { verdict, subject });
    }
    let entries = crate::read::kit_entries(dir).map_err(|p| Refused::Kit { verdict: zikaron_kit::tokens::KitFailToken::Unreadable.as_str().to_string(), subject: Some(p) })?;
    let held = originals_in_dir(dir);
    plan_in(dir, &entries, &held)
}

pub fn plan_in(dir: &Path, entries: &[Vec<u8>], held: &[(Original, Option<Vec<u8>>)]) -> Result<Plan, Refused> {
    plan_of(entries, granted_in_dir(dir).as_ref(), held)
}

pub fn granted_in_dir(dir: &Path) -> Option<crate::grantfile::Granted> {
    if !dir.join(crate::names::FILES_DIR).join(crate::grantfile::CODE_FILE).is_file() {
        return None;
    }
    let pairs: Vec<(String, Vec<u8>)> = zikaron_kit::v2::kit::walk(dir).ok()?.into_iter().collect();
    crate::grantfile::granted(&pairs)
}

fn plan_of(entries: &[Vec<u8>], grant: Option<&crate::grantfile::Granted>, held: &[(Original, Option<Vec<u8>>)]) -> Result<Plan, Refused> {
    let pairs: Vec<(String, Option<Vec<u8>>)> = held.iter().map(|(o, b)| (o.content.clone(), b.clone())).collect();
    let mut rows: Vec<(String, String, crate::recordcheck::Original)> = crate::recordcheck::records(entries, &pairs, None).into_iter().map(|r| (r.id, r.content, r.original)).collect();
    if let Some(g) = grant.filter(|g| !rows.iter().any(|(_, c, _)| *c == g.work)) {
        rows.push((g.id.clone(), g.work.clone(), crate::recordcheck::original_of(&g.work, &pairs)));
    }
    let mut take: Vec<Original> = Vec::new();
    let mut left: Vec<Left> = Vec::new();
    for (id, content, original) in rows {
        match original {
            crate::recordcheck::Original::Match => {
                if take.iter().any(|o| o.content == content) {
                    continue;
                }
                let hit = candidates(held, &content).find(|(_, b)| b.as_ref().map(|x| zikaron::hexfmt::encode(&crate::recording::content_of(x)) == content).unwrap_or(false));
                if let Some((o, _)) = hit.filter(|(o, _)| !take.iter().any(|t| t.kept == Kept::Dir && o.kept == Kept::Dir && t.members == o.members)) {
                    take.push(o.clone());
                }
            }
            crate::recordcheck::Original::Mismatch => {
                let name = candidates(held, &content).find(|(o, _)| o.kept == Kept::File).map(|(o, _)| o.landing_name().to_string());
                left.push(Left { entry_id: id, content, why: Why::Mismatch, name });
            }
            crate::recordcheck::Original::Missing => left.push(Left { entry_id: id, content, why: Why::Missing, name: None }),
        }
    }
    let in_folders: Vec<String> = take.iter().filter(|o| o.kept == Kept::Dir).flat_map(|o| o.members.iter().map(|m| m.path.clone())).collect();
    take.retain(|o| o.kept == Kept::Dir || !o.members.iter().all(|m| in_folders.contains(&m.path)));
    for o in &take {
        if let Some(m) = o.members.iter().find(|m| !zikaron_kit::kitdir::is_kit_path(&m.path)) {
            return Err(Refused::Path(m.path.clone()));
        }
    }
    let mut unheld: Vec<String> = Vec::new();
    for o in take.iter().filter(|o| o.strict && !o.names_hold) {
        let names = std::iter::once(o.name.clone()).chain(o.members.iter().flat_map(|m| m.rel.clone()));
        for n in names.filter(|n| !one_name(n)) {
            if !unheld.contains(&n) {
                unheld.push(n);
            }
        }
    }
    if !unheld.is_empty() {
        return Err(Refused::Name(unheld));
    }
    take.sort_by(|a, b| a.landing_name().cmp(b.landing_name()));
    let mut clash: Vec<String> = Vec::new();
    for (i, o) in take.iter().enumerate() {
        let n = o.landing_name();
        if take[..i].iter().any(|p| p.landing_name().to_lowercase() == n.to_lowercase()) && !clash.iter().any(|c| c == n) {
            clash.push(n.to_string());
        }
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

fn candidates<'a>(held: &'a [(Original, Option<Vec<u8>>)], content: &'a str) -> impl Iterator<Item = &'a (Original, Option<Vec<u8>>)> {
    let deep = |o: &Original| o.kept == Kept::File && o.members.iter().any(|m| m.path.contains('/'));
    let of = move |o: &Original| o.content == content;
    held.iter().filter(move |(o, _)| of(o) && !deep(o)).chain(held.iter().filter(move |(o, _)| of(o) && deep(o)))
}

static FAIL_AFTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);

pub fn pretend_place_fails_after(n: Option<usize>) {
    FAIL_AFTER.store(n.unwrap_or(usize::MAX), std::sync::atomic::Ordering::SeqCst);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
    pub to: PathBuf,
    pub names: Vec<String>,
}

pub fn land(plan: &Plan, dir: &Path, to: &Path) -> Result<Landed, Refused> {
    crate::seam_v2();
    if plan.take.is_empty() {
        return Err(Refused::Nothing);
    }
    match std::fs::metadata(to) {
        Ok(m) if m.is_dir() => {}
        _ => return Err(Refused::NotAFolder(to.display().to_string())),
    }
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

fn free(to: &Path, name: &str) -> Result<Free, Refused> {
    match std::fs::symlink_metadata(to.join(name)) {
        Ok(m) if m.file_type().is_symlink() => Err(Refused::Link(to.join(name).display().to_string())),
        Ok(_) => Ok(Free::Taken),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Free::Yes),
        Err(e) if refuses_the_name(&e) => Err(Refused::Name(vec![name.to_string()])),
        Err(e) => Err(Refused::Landing(to.join(name).display().to_string(), e.to_string())),
    }
}

fn trouble(t: crate::landing::Trouble) -> Refused {
    let w = t.said().unwrap_or(t.code()).to_string();
    Refused::Landing(t.subject().to_string(), w)
}

fn lay_out(plan: &Plan, dir: &Path, staging: &Path) -> Result<(), Refused> {
    let files = dir.join(crate::names::FILES_DIR);
    let mut made = Made::default();
    for o in &plan.take {
        let top = staging.join(o.landing_name());
        for m in &o.members {
            let rel = o.landing_rel(m);
            let mut names: Vec<&str> = vec![o.landing_name()];
            names.extend(rel.iter().map(|x| x.as_str()));
            let Some((file, folders)) = names.split_last() else { continue };
            let at = made.folders(staging, folders)?.join(file);
            let mut src = std::fs::File::open(files.join(&m.path)).map_err(|e| Refused::Landing(m.path.clone(), e.to_string()))?;
            if o.strict {
                put_named(&at, file, &mut src)?;
                continue;
            }
            crate::landing::put_new_from(&at, &mut src).map_err(|t| match t {
                crate::landing::Trouble::Occupied(_) => Refused::Clash(vec![file.to_string()]),
                t => trouble(t),
            })?;
        }
        let holds = match o.kept {
            Kept::File => sha_at(&top)? == o.content,
            Kept::Dir if o.names_hold || o.strict => {
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

fn refuses_the_name(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::InvalidFilename | std::io::ErrorKind::InvalidInput)
}

#[derive(Default)]
struct Made(std::collections::BTreeSet<Vec<String>>);

impl Made {
    fn folders(&mut self, root: &Path, names: &[&str]) -> Result<PathBuf, Refused> {
        let mut at = root.to_path_buf();
        for (i, n) in names.iter().enumerate() {
            at.push(n);
            let spelled: Vec<String> = names[..=i].iter().map(|s| s.to_string()).collect();
            if self.0.contains(&spelled) {
                continue;
            }
            match std::fs::create_dir(&at) {
                Ok(()) => {
                    self.0.insert(spelled);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(Refused::Clash(vec![n.to_string()])),
                Err(e) if refuses_the_name(&e) => return Err(Refused::Name(vec![n.to_string()])),
                Err(e) => return Err(Refused::Landing(at.display().to_string(), e.to_string())),
            }
        }
        Ok(at)
    }
}

fn put_named(at: &Path, file: &str, body: &mut dyn std::io::Read) -> Result<(), Refused> {
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    let mut f = match o.open(at) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(Refused::Clash(vec![file.to_string()])),
        Err(e) if refuses_the_name(&e) => return Err(Refused::Name(vec![file.to_string()])),
        Err(e) => return Err(Refused::Landing(at.display().to_string(), e.to_string())),
    };
    std::io::copy(body, &mut f).map_err(|e| Refused::Landing(at.display().to_string(), e.to_string()))?;
    f.sync_all().map_err(|e| Refused::Landing(at.display().to_string(), e.to_string()))
}

fn sha_at(at: &Path) -> Result<String, Refused> {
    std::fs::read(at).map(|b| zikaron::hexfmt::encode(&crate::recording::content_of(&b))).map_err(|e| Refused::Landing(at.display().to_string(), e.to_string()))
}

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

pub fn plan_at(kit: &Path) -> Result<Plan, Refused> {
    let unpacked = if crate::container::is_container_at(kit) {
        Some(crate::read::unpack(kit).map_err(|b| Refused::Unpack { code: b.code().to_string(), subject: b.subject() })?)
    } else {
        None
    };
    plan(unpacked.as_ref().map(|u| u.dir.as_path()).unwrap_or(kit))
}
