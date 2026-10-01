//! Settings: role, size cap, chain endpoints and basis, assorted bookkeeping. This file holds no absolute
//! path, which is how "any copy is equivalent" holds (see the `home` file header).
//!
//! Older files still open: reading picks only the keys it knows and ignores the rest. Extra cells written by
//! earlier versions still open, have no effect, and are not written again on the next write.

use crate::fault::{classify, Fault, Known};
use crate::home::{Home, Slot};
use crate::key::Address;
use crate::roles::Role;
use zikaron::json::{self, Value};

/// The settings file's name. One name, one home.
pub const FILE: &str = "desk.json";

/// Default size cap (bytes). Changeable, not removable: a missing cap is no cap.
pub const CAP_DEFAULT: u64 = 4 * 1024 * 1024 * 1024;

/// Where and when the last bundle was exported.
///
/// This is content the user chose, not a record of where this home is: it says where the mirror should go,
/// picked by the person. Copying the home elsewhere carries it along, still pointing where the person chose.
/// That does not conflict with "any copy is equivalent"; writing the home's own path into the home would (and
/// that stays zero).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorRecord {
    pub path: String,
    pub at: u64,
}

/// A registered git repository: its path, and which commit was anchored last.
///
/// As with [`MirrorRecord`]: content the person chose, not a record of where this home is, so copies stay
/// equivalent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRecord {
    pub path: String,
    /// The commit anchored last (forty bare hex digits); empty when never anchored.
    pub last_commit: String,
}

/// How often the self-audit clock runs (seconds). Zero means it never runs by itself (only when the person
/// clicks).
pub const AUDIT_EVERY_DEFAULT: u64 = 300;

#[derive(Clone, Debug)]
pub struct Settings {
    pub role: Role,
    pub cap_bytes: u64,
    /// Chain endpoints, spelled `<chain id>=<url>`. This is configuration, not a path, so copies stay
    /// equivalent.
    pub endpoints: Vec<String>,
    /// The last bundle export record. None means never exported.
    pub mirror: Option<MirrorRecord>,
    /// The three basis cells (law §9.4): which chain, which registry, which block to scan from. Empty means
    /// not configured: the self-audit clock then cannot ask the chain, the face says so, and an offline pass
    /// never passes for an online one.
    pub chain_id: Option<u64>,
    pub registry: Option<Address>,
    pub from_block: u64,
    /// Self-audit clock period (seconds).
    pub audit_every: u64,
    /// The registered git repository.
    pub repo: Option<RepoRecord>,
    /// The former local exclusive list (read-only historical bookkeeping).
    ///
    /// Exclusivity is now carried by the issuance record written once at signing (`termsx`); this list is
    /// read and never written, the face marks it "no terms document", and the double-sale gate still reads it
    /// (`legacy` in `grantx::table`).
    pub exclusive: Vec<String>,
    /// Address book, purely local. Only what the person pasted in; this layer neither discovers nor
    /// enumerates addresses, so it never becomes a directory.
    pub book: Vec<String>,
    /// Where each vault grant's upstream bytes are (local bookkeeping, not in the entry bytes).
    pub upstreams: Vec<(String, String)>,
    /// Vault periodic re-check period (seconds); zero means it never runs by itself.
    pub review_every: u64,
    /// Keys the sentinel has alerted (`<kind>:<grant>`); one alert per grant per kind.
    pub alarmed: Vec<String>,
    /// Which language the interface speaks. None means never chosen (then the default Chinese); the cell is
    /// written only once chosen.
    pub lang: Option<crate::lang::Lang>,
    /// The time zone moments are shown in; none chosen reads as UTC.
    pub zone: Option<crate::when::Zone>,
    /// Auto anchor (off by default): on means estimate gas and show the confirmation card right after
    /// recording; off means only add to the ledger and wait for the person to anchor by hand. Anchoring costs
    /// money and cannot be undone, so by default it is not spent for the person. The cell is written only
    /// when on, so a settings file that never touched it stays byte-identical.
    pub auto_anchor: bool,
    /// Which row of the known deployments table this home's basis came from (`None` when configured by the
    /// person or not yet configured). Cleared when the person changes the basis or nodes: the face line "from
    /// the network this machine chose" lights only when that is really so.
    pub network: Option<String>,
    /// Publish address: where the recorder puts record bundles on static hosting; `https://` only. The
    /// publish address pointer in grant files takes it; "check publication" fetches file by file against it.
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
            network: None,
            publish: None,
            grant_notes: Vec::new(),
            issuer_notes: Vec::new(),
        }
    }
}

fn field<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(name, _)| name == k).map(|(_, x)| x),
        _ => None,
    }
}

impl Settings {
    /// Read. No file returns the defaults: "never set" is not an error.
    pub fn read(home: &Home) -> Result<Settings, Fault> {
        let p = home.dir(Slot::Settings).join(FILE);
        // Sealed (`local::Doc::Settings`): no file returns the defaults; locked or not opening is refused by
        // name (reading it as defaults would let the next write overwrite the person's settings).
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
        })
    }

    /// Write. Overwriting the old settings is intended, so this writes directly; what must refuse overwriting
    /// is documents, not settings.
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
        m.push(("reviewEvery".to_string(), Value::Int(self.review_every)));
        let bytes = json::canon_bytes(&Value::Obj(m));
        // Writing to disk has one method (`local::put`: sealed, written aside, then renamed).
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Settings, &bytes)
    }

    /// Whether over the cap. Computed from the bytes on disk now, not from memory.
    pub fn over_cap(&self, home: &Home) -> Result<Option<(u64, u64)>, Fault> {
        let used = home.usage()?;
        Ok(if used > self.cap_bytes { Some((used, self.cap_bytes)) } else { None })
    }
}

/// Migration: move a whole home to another path.
///
/// Three steps: the target must be empty (no overwriting), copy file by file, then verify bytes file by file.
/// On failure the new copy is left in place and the error is named, and not one byte of the old is touched:
/// when a move fails, the copy still standing must be the old one.
pub fn migrate(from: &Home, to: &std::path::Path) -> Result<Home, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::H3);
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
                .map_err(|t| Fault::landing(t.code(), t.subject()))?;
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
