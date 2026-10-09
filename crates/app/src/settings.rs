//! Settings: role, size cap, chain endpoints and basis, and assorted local bookkeeping. The file holds no
//! path of the home itself, so any copy of a home is equivalent (see the `home` module header).
//!
//! Reading takes only the members it knows; any other member (written by an older or newer version) has no
//! effect and is written back unchanged (`Settings::extra`).

use crate::fault::{classify, Fault, Known};
use crate::home::{Home, Slot};
use crate::key::Address;
use crate::roles::Role;
use zikaron::json::{self, Value};

/// The settings file name.
pub const FILE: &str = "desk.json";

/// Default size cap in bytes. The cap can be changed but not removed.
pub const CAP_DEFAULT: u64 = 4 * 1024 * 1024 * 1024;

/// Where and when the last bundle was exported.
///
/// This is a destination the person chose, not a record of where this home is, so copying the home carries
/// it along unchanged. That keeps copies equivalent; writing the home's own path into the home would not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorRecord {
    pub path: String,
    pub at: u64,
}

/// A registered git repository: its path and the last anchored commit.
///
/// Like [`MirrorRecord`], this is content the person chose, not the home's location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRecord {
    pub path: String,
    /// The commit anchored last (forty bare hex digits); empty when never anchored.
    pub last_commit: String,
}

/// Default self-audit period in seconds. Zero means it runs only when the person clicks.
pub const AUDIT_EVERY_DEFAULT: u64 = 300;

#[derive(Clone, Debug)]
pub struct Settings {
    pub role: Role,
    pub cap_bytes: u64,
    /// Chain endpoints as `<chain id>=<url>`. Configuration, not a path, so copies stay equivalent.
    pub endpoints: Vec<String>,
    /// The last bundle export; `None` means never exported.
    pub mirror: Option<MirrorRecord>,
    /// The three basis fields (law §9.4): chain, registry, and the block to scan from. Empty means not
    /// configured: the self-audit clock cannot query the chain, the UI says so, and an offline pass is never
    /// shown as an online one.
    pub chain_id: Option<u64>,
    pub registry: Option<Address>,
    pub from_block: u64,
    /// Self-audit period in seconds.
    pub audit_every: u64,
    /// The registered git repository.
    pub repo: Option<RepoRecord>,
    /// The former local exclusivity list, kept read-only for older homes.
    ///
    /// Exclusivity is now carried by the issuance record written once at signing (`termsx`). This list is
    /// read but never changed; the UI marks its entries "no terms document", and the double-sale check still
    /// reads it (`legacy` in `grantx::table`).
    pub exclusive: Vec<String>,
    /// Address book, purely local: only what the person pasted in. This layer never discovers or enumerates
    /// addresses, so it never becomes a directory.
    pub book: Vec<String>,
    /// Where each vault grant's upstream bytes are stored (local bookkeeping, not part of any entry).
    pub upstreams: Vec<(String, String)>,
    /// Vault re-check period in seconds; zero means it never runs by itself.
    pub review_every: u64,
    /// Keys the sentinel has already alerted (`sentinelx::key_of`); each event alerts once.
    pub alarmed: Vec<String>,
    /// Interface language. `None` means never chosen (Chinese by default); written only once chosen.
    pub lang: Option<crate::lang::Lang>,
    /// Time zone for displayed moments; `None` reads as UTC.
    pub zone: Option<crate::when::Zone>,
    /// Members this version does not know (written by a newer version), kept and written back unchanged so
    /// the newer version still finds them after an older one has saved.
    pub extra: Vec<(String, Value)>,
    /// Auto anchor, off by default. On: estimate gas and show the confirmation card right after recording.
    /// Off: only add to the ledger and wait for the person to anchor by hand. Anchoring costs money and cannot
    /// be undone, so it is not spent on the person's behalf by default. Written only when on, so a file that
    /// never touched it stays byte-identical.
    pub auto_anchor: bool,
    /// Display only: the records and ledger pages hide what remains on this machine after a deletion (a
    /// deleted entry that was never published, and its local deletion). Off by default; nothing else reads it.
    pub hide_local_deletions: bool,
    /// Which choice this home's network came from: a row of the known deployments table, or "custom" taken
    /// from what its identity recorded (`None` when configured by hand or not configured). Cleared when the
    /// person changes the basis or nodes, so the UI shows the row's name only while it is accurate.
    pub network: Option<String>,
    /// Publish address: where the person puts record bundles on static hosting; `https://` only. The
    /// publish-address pointer in grant files uses it, and "check publication" fetches each file against it.
    /// Any other form reads as not configured.
    pub publish: Option<String>,
    /// The person's own names for held grants (`0x…` grant id, name), typed when adding a grant. Purely
    /// local: sealed with the rest of this file, never in any entry or export.
    pub grant_notes: Vec<(String, String)>,
    /// The person's own names for issuers (lower-case `0x…` address, name), typed when adding a grant; every
    /// grant from that issuer shows it. Purely local, as above.
    pub issuer_notes: Vec<(String, String)>,
}

pub const REVIEW_EVERY_DEFAULT: u64 = 600;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            role: Role::Author,
            cap_bytes: CAP_DEFAULT,
            endpoints: Vec::new(),
            mirror: None,
            chain_id: None,
            registry: None,
            from_block: 0,
            audit_every: AUDIT_EVERY_DEFAULT,
            repo: None,
            exclusive: Vec::new(),
            book: Vec::new(),
            upstreams: Vec::new(),
            review_every: REVIEW_EVERY_DEFAULT,
            alarmed: Vec::new(),
            lang: None,
            zone: None,
            auto_anchor: false,
            hide_local_deletions: false,
            network: None,
            publish: None,
            grant_notes: Vec::new(),
            issuer_notes: Vec::new(),
            extra: Vec::new(),
        }
    }
}

/// The members this version reads and writes; any other is kept as it came (`Settings::extra`).
const MEMBERS: [&str; 22] = [
    "alarmed", "auditEvery", "autoAnchor", "book", "capBytes", "chainId", "endpoints", "exclusive", "fromBlock", "grantNotes",
    "hideLocalDeletions", "issuerNotes", "lang", "mirror", "network", "publish", "registry", "repo", "reviewEvery", "role",
    "upstreams", "zone",
];

fn field<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(name, _)| name == k).map(|(_, x)| x),
        _ => None,
    }
}

impl Settings {
    /// Read. A missing file returns the defaults: "never set" is not an error.
    pub fn read(home: &Home) -> Result<Settings, Fault> {
        let p = home.dir(Slot::Settings).join(FILE);
        // Sealed (`local::Doc::Settings`). A locked vault or a file that does not open is an error, not
        // defaults: reading defaults would let the next write overwrite the person's settings.
        let Some(bytes) = crate::local::read(&p, crate::local::Doc::Settings)? else {
            return Ok(Settings::default());
        };
        let v = json::parse(&bytes)
            .map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))?;
        let role = match field(&v, "role") {
            Some(Value::Str(s)) if s == Role::Grantee.as_str() => Role::Grantee,
            _ => Role::Author,
        };
        let cap = match field(&v, "capBytes") {
            Some(Value::Int(n)) if *n > 0 => *n,
            _ => CAP_DEFAULT,
        };
        let endpoints = match field(&v, "endpoints") {
            Some(Value::Arr(a)) => a
                .iter()
                .filter_map(|x| match x {
                    Value::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let addr = |k: &str| match field(&v, k) {
            Some(Value::Str(s)) => Address::parse(s),
            _ => None,
        };
        let mirror = match field(&v, "mirror") {
            Some(m) => {
                let path = match field(m, "path") {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                let at = match field(m, "at") {
                    Some(Value::Int(n)) => *n,
                    _ => 0,
                };
                if path.is_empty() { None } else { Some(MirrorRecord { path, at }) }
            }
            None => None,
        };
        let repo = match field(&v, "repo") {
            Some(r) => {
                let path = match field(r, "path") {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                let last_commit = match field(r, "lastCommit") {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                if path.is_empty() { None } else { Some(RepoRecord { path, last_commit }) }
            }
            None => None,
        };
        Ok(Settings {
            role,
            cap_bytes: cap,
            endpoints,
            mirror,
            chain_id: match field(&v, "chainId") {
                Some(Value::Int(n)) => Some(*n),
                _ => None,
            },
            registry: addr("registry"),
            from_block: match field(&v, "fromBlock") {
                Some(Value::Int(n)) => *n,
                _ => 0,
            },
            audit_every: match field(&v, "auditEvery") {
                Some(Value::Int(n)) => *n,
                _ => AUDIT_EVERY_DEFAULT,
            },
            repo,
            book: match field(&v, "book") {
                Some(Value::Arr(a)) => a
                    .iter()
                    .filter_map(|x| match x {
                        Value::Str(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            exclusive: match field(&v, "exclusive") {
                Some(Value::Arr(a)) => a
                    .iter()
                    .filter_map(|x| match x {
                        Value::Str(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            upstreams: match field(&v, "upstreams") {
                Some(Value::Arr(a)) => a
                    .iter()
                    .filter_map(|x| match (field(x, "grant"), field(x, "dir")) {
                        (Some(Value::Str(g)), Some(Value::Str(d))) => Some((g.clone(), d.clone())),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            review_every: match field(&v, "reviewEvery") {
                Some(Value::Int(n)) => *n,
                _ => REVIEW_EVERY_DEFAULT,
            },
            alarmed: match field(&v, "alarmed") {
                Some(Value::Arr(a)) => a
                    .iter()
                    .filter_map(|x| match x {
                        Value::Str(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            lang: match field(&v, "lang") {
                Some(Value::Str(x)) => crate::lang::Lang::ALL.iter().copied().find(|l| l.as_str() == x),
                _ => None,
            },
            zone: match field(&v, "zone") {
                Some(Value::Str(x)) => crate::when::Zone::parse(x),
                _ => None,
            },
            auto_anchor: matches!(field(&v, "autoAnchor"), Some(Value::Bool(true))),
            hide_local_deletions: matches!(field(&v, "hideLocalDeletions"), Some(Value::Bool(true))),
            network: match field(&v, "network") {
                Some(Value::Str(x)) if crate::deploy::named(x).is_some() => Some(x.clone()),
                _ => None,
            },
            publish: match field(&v, "publish") {
                Some(Value::Str(x)) => crate::fetchx::base_of(x).ok().map(|b| b.as_str().to_string()),
                _ => None,
            },
            grant_notes: pairs(&v, "grantNotes", "grant"),
            issuer_notes: pairs(&v, "issuerNotes", "issuer"),
            extra: match &v {
                Value::Obj(ms) => ms.iter().filter(|(k, _)| !MEMBERS.contains(&k.as_str())).cloned().collect(),
                _ => Vec::new(),
            },
        })
    }

    /// Write. Overwriting old settings is intended, so this writes directly (documents, not settings, must
    /// refuse overwriting).
    pub fn write(&self, home: &Home) -> Result<(), Fault> {
        let mut m = vec![
            ("auditEvery".to_string(), Value::Int(self.audit_every)),
            ("capBytes".to_string(), Value::Int(self.cap_bytes)),
            ("fromBlock".to_string(), Value::Int(self.from_block)),
            ("role".to_string(), Value::Str(self.role.as_str().to_string())),
        ];
        if let Some(c) = self.chain_id {
            m.push(("chainId".to_string(), Value::Int(c)));
        }
        if let Some(r) = self.registry {
            m.push(("registry".to_string(), Value::Str(r.hex())));
        }
        if let Some(n) = &self.network {
            m.push(("network".to_string(), Value::Str(n.clone())));
        }
        if let Some(u) = &self.publish {
            m.push(("publish".to_string(), Value::Str(u.clone())));
        }
        // Written only when there are any, so a settings file that never had a note stays byte-identical.
        for (key, what, rows) in [("grantNotes", "grant", &self.grant_notes), ("issuerNotes", "issuer", &self.issuer_notes)] {
            if !rows.is_empty() {
                let mut rows = rows.clone();
                rows.sort();
                m.push((
                    key.to_string(),
                    Value::Arr(
                        rows.into_iter()
                            .map(|(k, n)| Value::Obj(vec![(what.to_string(), Value::Str(k)), ("name".to_string(), Value::Str(n))]))
                            .collect(),
                    ),
                ));
            }
        }
        if !self.book.is_empty() {
            let mut ids = self.book.clone();
            ids.sort();
            ids.dedup();
            m.push(("book".to_string(), Value::Arr(ids.into_iter().map(Value::Str).collect())));
        }
        if !self.exclusive.is_empty() {
            let mut ids = self.exclusive.clone();
            ids.sort();
            ids.dedup();
            m.push((
                "exclusive".to_string(),
                Value::Arr(ids.into_iter().map(Value::Str).collect()),
            ));
        }
        if let Some(r) = &self.repo {
            m.push((
                "repo".to_string(),
                Value::Obj(vec![
                    ("lastCommit".to_string(), Value::Str(r.last_commit.clone())),
                    ("path".to_string(), Value::Str(r.path.clone())),
                ]),
            ));
        }
        if !self.endpoints.is_empty() {
            m.push((
                "endpoints".to_string(),
                Value::Arr(self.endpoints.iter().map(|x| Value::Str(x.clone())).collect()),
            ));
        }
        if let Some(mr) = &self.mirror {
            m.push((
                "mirror".to_string(),
                Value::Obj(vec![
                    ("at".to_string(), Value::Int(mr.at)),
                    ("path".to_string(), Value::Str(mr.path.clone())),
                ]),
            ));
        }
        if !self.upstreams.is_empty() {
            let mut rows = self.upstreams.clone();
            rows.sort();
            m.push((
                "upstreams".to_string(),
                Value::Arr(
                    rows.into_iter()
                        .map(|(g, d)| Value::Obj(vec![("dir".to_string(), Value::Str(d)), ("grant".to_string(), Value::Str(g))]))
                        .collect(),
                ),
            ));
        }
        if !self.alarmed.is_empty() {
            let mut ks = self.alarmed.clone();
            ks.sort();
            m.push(("alarmed".to_string(), Value::Arr(ks.into_iter().map(Value::Str).collect())));
        }
        if let Some(l) = self.lang {
            m.push(("lang".to_string(), Value::Str(l.as_str().to_string())));
        }
        if let Some(z) = self.zone {
            m.push(("zone".to_string(), Value::Str(z.as_str().to_string())));
        }
        if self.auto_anchor {
            m.push(("autoAnchor".to_string(), Value::Bool(true)));
        }
        if self.hide_local_deletions {
            m.push(("hideLocalDeletions".to_string(), Value::Bool(true)));
        }
        m.push(("reviewEvery".to_string(), Value::Int(self.review_every)));
        // Members this version does not know go back unchanged.
        for (k, v) in &self.extra {
            if !m.iter().any(|(n, _)| n == k) {
                m.push((k.clone(), v.clone()));
            }
        }
        let bytes = json::canon_bytes(&Value::Obj(m));
        // Every disk write goes through `local::put` (sealed, written aside, then renamed).
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Settings, &bytes)
    }

    /// Whether usage exceeds the cap, computed from the bytes on disk now. Returns (used, cap) when over.
    pub fn over_cap(&self, home: &Home) -> Result<Option<(u64, u64)>, Fault> {
        let used = home.usage()?;
        Ok(if used > self.cap_bytes { Some((used, self.cap_bytes)) } else { None })
    }
}

/// Move a whole home to another path.
///
/// The target must lie outside this home ([`outside_home`]) and be empty (nothing is overwritten); files are
/// copied one by one, then compared byte for byte. On failure the copy is left in place, the error is
/// returned, and the old home is not touched: after a failed move, the old home must still be the valid one.
pub fn migrate(from: &Home, to: &std::path::Path) -> Result<Home, Fault> {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::H3);
    outside_home(from.root(), to)?;
    if to.exists() && std::fs::read_dir(to).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(Fault::known(Known::Occupied, to.display().to_string()));
    }
    copy_tree(from.root(), to)?;
    let diffs = compare_tree(from.root(), to)?;
    if !diffs.is_empty() {
        return Err(Fault::known(Known::CopyMismatch, crate::lang::filln(crate::lang::Key::Tail210, &[&(diffs.len()).to_string(), &(diffs.join(" ")).to_string()])));
    }
    let home = Home::open(to)?;
    crate::home::write_pointer(to)?;
    Ok(home)
}

/// A move target must not be this home's root or lie under it: copying a tree into itself would copy the copy
/// again, one level deeper each time, until the path grew too long, leaving a half-nested tree in the old
/// home. This is decided from the two paths before any byte is written. Both are compared as real paths
/// (symbolic links resolved; for a target that does not exist yet, its deepest existing ancestor is resolved
/// and the rest appended). The reverse case (old root under the target) needs no check: such a target is not
/// empty.
pub fn outside_home(root: &std::path::Path, to: &std::path::Path) -> Result<(), Fault> {
    let root = std::fs::canonicalize(root).map_err(|e| classify(&e, &root.display().to_string()))?;
    let target = real_path(to)?;
    if target.starts_with(&root) {
        return Err(Fault::known(Known::InsideHome, format!("{} {}", to.display(), root.display())));
    }
    Ok(())
}

/// The real path of a place that may not exist yet: its deepest existing ancestor resolved, the rest appended
/// (a `..` in that rest steps back over a name that does not exist, so it cannot be a link).
fn real_path(p: &std::path::Path) -> Result<std::path::PathBuf, Fault> {
    use std::path::Component;
    let mut rest: Vec<Component> = Vec::new();
    let mut at = p;
    let base = loop {
        match std::fs::canonicalize(at) {
            Ok(b) => break b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(classify(&e, &at.display().to_string())),
        }
        let (Some(name), Some(up)) = (at.components().next_back(), at.parent()) else {
            return Err(Fault::known(Known::PathRelative, p.display().to_string()));
        };
        rest.push(name);
        at = up;
    };
    let mut out = base;
    for c in rest.into_iter().rev() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c.as_os_str()),
        }
    }
    Ok(out)
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> Result<(), Fault> {
    std::fs::create_dir_all(to).map_err(|e| classify(&e, &to.display().to_string()))?;
    for e in std::fs::read_dir(from).map_err(|e| classify(&e, &from.display().to_string()))? {
        let e = e.map_err(|e| classify(&e, &from.display().to_string()))?;
        let src = e.path();
        let dst = to.join(e.file_name());
        let md = std::fs::symlink_metadata(&src).map_err(|e| classify(&e, &src.display().to_string()))?;
        if md.is_dir() {
            copy_tree(&src, &dst)?;
        } else if md.is_file() {
            let bytes = std::fs::read(&src).map_err(|e| classify(&e, &src.display().to_string()))?;
            zikaron_glue::landing::land_bytes(&dst, &bytes)
                .map_err(|t| Fault::of_landing(t))?;
        }
        // Other shapes (symbolic links, device files) are not moved: a home should not contain them, and
        // moving them would lose their meaning.
    }
    Ok(())
}

fn compare_tree(a: &std::path::Path, b: &std::path::Path) -> Result<Vec<String>, Fault> {
    let mut bad = Vec::new();
    for e in std::fs::read_dir(a).map_err(|e| classify(&e, &a.display().to_string()))? {
        let e = e.map_err(|e| classify(&e, &a.display().to_string()))?;
        let src = e.path();
        let dst = b.join(e.file_name());
        let md = std::fs::symlink_metadata(&src).map_err(|e| classify(&e, &src.display().to_string()))?;
        if md.is_dir() {
            bad.extend(compare_tree(&src, &dst)?);
        } else if md.is_file() {
            let x = std::fs::read(&src).unwrap_or_default();
            let y = std::fs::read(&dst).unwrap_or_default();
            if x != y {
                bad.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    Ok(bad)
}

/// A list of `{<what>: key, name: text}` rows under `key`; malformed rows are skipped.
fn pairs(v: &Value, key: &str, what: &str) -> Vec<(String, String)> {
    match field(v, key) {
        Some(Value::Arr(a)) => a
            .iter()
            .filter_map(|x| match (field(x, what), field(x, "name")) {
                (Some(Value::Str(k)), Some(Value::Str(n))) => Some((k.clone(), n.clone())),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
