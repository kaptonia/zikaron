use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

pub const ANCHORED_FILE: &str = zikaron_seal::lastaudit::FILE;

pub const VERDICT_SUFFIX: &str = zikaron_seal::envelope::VERDICT_SUFFIX;

pub const STALE_SECS: u64 = 24 * 3600;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Anchored {
    pub rows: Vec<(String, (u64, String))>,
    pub at: u64,
    pub whole: bool,
}

impl Anchored {
    pub fn has(&self, id: &str) -> bool {
        self.rows.iter().any(|(x, _)| x.eq_ignore_ascii_case(id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub verdict: String,
    pub checks: Vec<(String, String)>,
    pub at: u64,
    pub chain_now: Option<u64>,
    pub anchored_at: Option<u64>,
    pub undecided: Option<zikaron_glue::book::Undecided>,
}

pub fn of_card(c: &crate::vaultx::Card, at: u64, chain_now: Option<u64>) -> Verdict {
    Verdict { verdict: c.verdict.clone(), checks: crate::vaultx::states(&c.checks), at, chain_now, anchored_at: c.anchored_at, undecided: c.undecided }
}

fn undecided_of(code: &str) -> Option<zikaron_glue::book::Undecided> {
    use zikaron_glue::book::Undecided;
    [Undecided::OldLaw, Undecided::Continued].into_iter().find(|u| u.code() == code)
}

pub fn stale(at: u64, now: u64) -> bool {
    now < at || now - at > STALE_SECS
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

fn int_of(v: &Value, k: &str) -> Option<u64> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).and_then(|(_, v)| match v {
            Value::Int(n) => Some(*n),
            _ => None,
        }),
        _ => None,
    }
}

fn arr_of<'a>(v: &'a Value, k: &str) -> Option<&'a Vec<Value>> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).and_then(|(_, v)| match v {
            Value::Arr(a) => Some(a),
            _ => None,
        }),
        _ => None,
    }
}

pub fn save_anchored(home: &Home, rows: &[(String, (u64, String))], at: u64, whole: bool) -> Result<(), Fault> {
    let bytes = zikaron_seal::lastaudit::bytes(&zikaron_seal::lastaudit::Anchored { rows: rows.to_vec(), at, whole });
    crate::local::put(&home.dir(Slot::Settings), ANCHORED_FILE, crate::local::Doc::LastAudit, &bytes)
}

pub fn load_anchored(home: &Home) -> Result<Option<Anchored>, Fault> {
    let p = home.dir(Slot::Settings).join(ANCHORED_FILE);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::LastAudit)? else { return Ok(None) };
    let a = zikaron_seal::lastaudit::parse(&bytes).map_err(|w| Fault::known(Known::SettingsShape, format!("{}: {w}", p.display())))?;
    Ok(Some(Anchored { rows: a.rows, at: a.at, whole: a.whole }))
}

pub fn is_cache(name: &str) -> bool {
    name.ends_with(VERDICT_SUFFIX)
}

pub fn verdict_path(home: &Home, id: &str) -> Result<std::path::PathBuf, Fault> {
    Ok(home.dir(Slot::GrantsHeld).join(format!("{}{VERDICT_SUFFIX}", crate::vaultx::held_stem(id)?)))
}

pub fn save_verdict(home: &Home, id: &str, v: &Verdict) -> Result<(), Fault> {
    let (verdict, checks, at) = (&v.verdict, &v.checks, v.at);
    let arr: Vec<Value> = checks
        .iter()
        .map(|(tok, st)| Value::Obj(vec![("state".to_string(), Value::Str(st.clone())), ("token".to_string(), Value::Str(tok.clone()))]))
        .collect();
    let mut m = Vec::new();
    if let Some(n) = v.anchored_at {
        m.push(("anchored_at".to_string(), Value::Int(n)));
    }
    m.push(("at".to_string(), Value::Int(at)));
    if let Some(n) = v.chain_now {
        m.push(("chain_now".to_string(), Value::Int(n)));
    }
    m.push(("checks".to_string(), Value::Arr(arr)));
    if let Some(u) = v.undecided {
        m.push(("undecided".to_string(), Value::Str(u.code().to_string())));
    }
    m.push(("grant".to_string(), Value::Str(grant_form(id))));
    m.push(("verdict".to_string(), Value::Str(verdict.to_string())));
    let v = Value::Obj(m);
    let p = verdict_path(home, id)?;
    let name = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    crate::local::put(&home.dir(Slot::GrantsHeld), &name, crate::local::Doc::Verdict, &json::canon_bytes(&v))
}

pub fn grant_form(id: &str) -> String {
    format!("0x{}", id.trim().trim_start_matches("0x").trim_start_matches("0X").to_ascii_lowercase())
}

pub fn issuer_form(author: &str) -> String {
    grant_form(author)
}

pub fn load_verdict(home: &Home, id: &str) -> Result<Option<Verdict>, Fault> {
    let p = verdict_path(home, id)?;
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Verdict)? else { return Ok(None) };
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))?;
    let (Some(verdict), Some(at), Some(rows)) = (str_of(&v, "verdict"), int_of(&v, "at"), arr_of(&v, "checks")) else {
        return Err(Fault::known(Known::SettingsShape, p.display().to_string()));
    };
    let mut checks = Vec::new();
    for r in rows {
        let (Some(tok), Some(st)) = (str_of(r, "token"), str_of(r, "state")) else {
            return Err(Fault::known(Known::SettingsShape, p.display().to_string()));
        };
        checks.push((tok, st));
    }
    let undecided = match str_of(&v, "undecided") {
        None => None,
        Some(code) => Some(undecided_of(&code).ok_or_else(|| Fault::known(Known::SettingsShape, p.display().to_string()))?),
    };
    Ok(Some(Verdict { verdict, checks, at, chain_now: int_of(&v, "chain_now"), anchored_at: int_of(&v, "anchored_at"), undecided }))
}

pub fn load_verdicts(home: &Home) -> (Vec<(String, Verdict)>, Vec<Fault>) {
    let mut out = Vec::new();
    let mut bad = Vec::new();
    let Ok(listing) = std::fs::read_dir(home.dir(Slot::GrantsHeld)) else { return (out, bad) };
    let mut names: Vec<String> = listing.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    for n in names {
        if !n.ends_with(VERDICT_SUFFIX) {
            continue;
        }
        let p = home.dir(Slot::GrantsHeld).join(&n);
        let read = crate::local::read(&p, crate::local::Doc::Verdict).and_then(|b| {
            b.ok_or_else(|| Fault::known(Known::FileMissing, p.display().to_string())).and_then(|b| {
                json::parse(&b).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))
            })
        });
        match read.map(|v| str_of(&v, "grant")) {
            Ok(Some(id)) => match load_verdict(home, &id) {
                Ok(Some(v)) => out.push((grant_form(&id), v)),
                Ok(None) => {}
                Err(f) => bad.push(f),
            },
            Ok(None) => bad.push(Fault::known(Known::SettingsShape, format!("{}: grant", p.display()))),
            Err(f) => bad.push(f),
        }
    }
    (out, bad)
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
