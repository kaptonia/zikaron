use crate::fault::{classify, Fault, Known};
use crate::home::{Home, Slot};
use crate::key::Address;
use crate::roles::Role;
use zikaron::json::{self, Value};

pub const FILE: &str = zikaron_seal::ground::SETTINGS_FILE;

fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

pub const CAP_DEFAULT: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorRecord {
    pub path: String,
    pub at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoRecord {
    pub path: String,
    pub last_commit: String,
}

pub const AUDIT_EVERY_DEFAULT: u64 = 300;

#[derive(Clone, Debug)]
pub struct Settings {
    pub role: Role,
    pub cap_bytes: u64,
    pub endpoints: Vec<String>,
    pub rest: Vec<String>,
    pub rest_on: bool,
    pub log_asks: Option<u64>,
    pub mirror: Option<MirrorRecord>,
    pub chain_id: Option<u64>,
    pub registry: Option<Address>,
    pub from_block: u64,
    pub earlier: Vec<zikaron_seal::ground::Earlier>,
    pub audit_every: u64,
    pub repo: Option<RepoRecord>,
    pub book: Vec<String>,
    pub upstreams: Vec<(String, String)>,
    pub review_every: u64,
    pub alarmed: Vec<String>,
    pub swept: Vec<String>,
    pub lang: Option<crate::lang::Lang>,
    pub zone: Option<crate::when::Zone>,
    pub extra: Vec<(String, Value)>,
    pub auto_anchor: bool,
    pub hide_local_deletions: bool,
    pub network: Option<String>,
    pub publish: Option<String>,
    pub grant_notes: Vec<(String, String)>,
    pub issuer_notes: Vec<(String, String)>,
}

pub const REVIEW_EVERY_DEFAULT: u64 = 600;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            role: Role::Author,
            cap_bytes: CAP_DEFAULT,
            endpoints: Vec::new(),
            rest: Vec::new(),
            rest_on: false,
            log_asks: None,
            mirror: None,
            chain_id: None,
            registry: None,
            from_block: 0,
            earlier: Vec::new(),
            audit_every: AUDIT_EVERY_DEFAULT,
            repo: None,
            book: Vec::new(),
            upstreams: Vec::new(),
            review_every: REVIEW_EVERY_DEFAULT,
            alarmed: Vec::new(),
            swept: Vec::new(),
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

const MEMBERS: [&str; 26] = [
    "alarmed", "auditEvery", "autoAnchor", "book", "capBytes", "chainId", "earlier", "endpoints", "fromBlock", "grantNotes",
    "hideLocalDeletions", "issuerNotes", "lang", "mirror", "network", "publish", "registry", "repo", zikaron_seal::ground::REST,
    zikaron_seal::ground::REST_ON, zikaron_seal::ground::LOG_ASKS, "reviewEvery", "role", "swept", "upstreams", "zone",
];

fn field<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(name, _)| name == k).map(|(_, x)| x),
        _ => None,
    }
}

impl Settings {
    pub fn read(home: &Home) -> Result<Settings, Fault> {
        let p = home.dir(Slot::Settings).join(FILE);
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
        let ground = zikaron_seal::ground::of_settings(&v);
        Ok(Settings {
            role,
            cap_bytes: cap,
            endpoints: ground.endpoints,
            rest: ground.rest,
            rest_on: ground.rest_on,
            log_asks: ground.log_asks.filter(|n| zikaron_anchor::action::asks_within(*n)),
            mirror,
            chain_id: ground.chain_id,
            registry: ground.registry.map(Address),
            from_block: ground.from_block,
            earlier: ground.earlier,
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
            swept: match field(&v, "swept") {
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

    pub fn write(&self, home: &Home) -> Result<(), Fault> {
        let _turn = turn();
        self.write_now(home)?;
        crate::reads::wrote(crate::reads::Copies::Settings);
        Ok(())
    }

    pub fn amend(home: &Home, f: impl FnOnce(&mut Settings)) -> Result<Settings, Fault> {
        let _turn = turn();
        let mut s = Settings::read(home)?;
        f(&mut s);
        s.write_now(home)?;
        crate::reads::wrote(crate::reads::Copies::Settings);
        Ok(s)
    }

    fn write_now(&self, home: &Home) -> Result<(), Fault> {
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
        if !self.earlier.is_empty() {
            m.push((zikaron_seal::ground::EARLIER.to_string(), Value::Arr(self.earlier.iter().map(|e| e.value()).collect())));
        }
        if let Some(n) = &self.network {
            m.push(("network".to_string(), Value::Str(n.clone())));
        }
        if let Some(u) = &self.publish {
            m.push(("publish".to_string(), Value::Str(u.clone())));
        }
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
        if !self.rest.is_empty() {
            m.push((zikaron_seal::ground::REST.to_string(), Value::Arr(self.rest.iter().map(|x| Value::Str(x.clone())).collect())));
        }
        if self.rest_on {
            m.push((zikaron_seal::ground::REST_ON.to_string(), Value::Bool(true)));
        }
        if let Some(n) = self.log_asks {
            m.push((zikaron_seal::ground::LOG_ASKS.to_string(), Value::Int(n)));
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
        if !self.swept.is_empty() {
            let mut ks = self.swept.clone();
            ks.sort();
            m.push(("swept".to_string(), Value::Arr(ks.into_iter().map(Value::Str).collect())));
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
        for (k, v) in &self.extra {
            if !m.iter().any(|(n, _)| n == k) {
                m.push((k.clone(), v.clone()));
            }
        }
        let bytes = json::canon_bytes(&Value::Obj(m));
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Settings, &bytes)
    }

    pub fn ground(&self) -> zikaron_seal::ground::Ground {
        zikaron_seal::ground::Ground {
            chain_id: self.chain_id,
            registry: self.registry.map(|r| r.0),
            from_block: self.from_block,
            endpoints: self.endpoints.clone(),
            earlier: self.earlier.clone(),
            publish: self.publish.clone(),
            rest: self.rest.clone(),
            rest_on: self.rest_on,
            log_asks: self.log_asks,
        }
    }

    pub fn log_asks(&self) -> u64 {
        self.log_asks.filter(|n| zikaron_anchor::action::asks_within(*n)).unwrap_or(zikaron_anchor::action::ASKS_DEFAULT)
    }

    pub fn over_cap(&self, home: &Home) -> Result<Option<(u64, u64)>, Fault> {
        let used = home.usage()?;
        Ok(if used > self.cap_bytes { Some((used, self.cap_bytes)) } else { None })
    }
}

pub fn migrate(from: &Home, to: &std::path::Path) -> Result<Home, Fault> {
    crate::trace::mark(crate::feature::Feature::H3);
    outside_home(from.root(), to)?;
    if to.exists() && std::fs::read_dir(to).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(Fault::known(Known::Occupied, to.display().to_string()));
    }
    copy_tree(from.root(), to)?;
    let diffs = compare_tree(from.root(), to)?;
    if !diffs.is_empty() {
        return Err(Fault::known(Known::CopyMismatch, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail210, &[&(diffs.len()).to_string(), &(diffs.join(" ")).to_string()]))));
    }
    let home = Home::open(to)?;
    crate::home::write_pointer(to)?;
    Ok(home)
}

pub fn outside_home(root: &std::path::Path, to: &std::path::Path) -> Result<(), Fault> {
    let root = std::fs::canonicalize(root).map_err(|e| classify(&e, &root.display().to_string()))?;
    let target = real_path(to)?;
    if target.starts_with(&root) {
        return Err(Fault::known(Known::InsideHome, format!("{} {}", to.display(), root.display())));
    }
    Ok(())
}

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
