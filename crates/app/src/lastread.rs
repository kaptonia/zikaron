//! On-disk caches of the last audit and the last grant checks:
//!
//! 1. The `anchored` set from the self-audit report, in the home's `settings/` directory ([`ANCHORED_FILE`]);
//! 2. Each held grant's six-check verdict and verification time, at `grants-held/<id>.verdict.json`
//! ([`verdict_path`]).
//!
//! Without them, every startup would show all entries as "confirming" and all grants as "not verified" until
//! the background checks finish, which looks like failure. Startup hydrates from these caches first, the UI
//! shows "confirmed · verified hh:mm", and fresh results replace them as they arrive.
//!
//! They are caches, not proof. A cached anchoring shows as `Lamp::Remembered`, never as `Lamp::Anchored`,
//! which only the current session's audit report produces; anything that requires "anchored" (granting to
//! others, starting a relicense…) accepts only the current report. Staleness only turns the status dot gray
//! instead of green, never yellow (yellow means "in progress").
//!
//! Times are always passed in by the caller (Unix seconds): the app uses `Shell::clock` (default
//! [`now_secs`]); tests inject fixed times.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

/// The file name of the `anchored` set in `settings/`.
pub const ANCHORED_FILE: &str = "last-audit.json";

/// The suffix of verdict file names (`grants-held/<id>.verdict.json`).
pub const VERDICT_SUFFIX: &str = ".verdict.json";

/// A cache older than this (seconds) is stale: the status dot turns gray (never yellow). The background check
/// runs either way.
pub const STALE_SECS: u64 = 24 * 3600;

/// The cached `anchored` set: each entry id with its chain id and transaction, and the audit time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Anchored {
    pub rows: Vec<(String, (u64, String))>,
    pub at: u64,
    /// Whether that audit was complete (`auditx::whole`, recorded when saved). Files from older versions lack
    /// this field and read as not complete.
    pub whole: bool,
}

impl Anchored {
    /// Whether this entry is in the cached set.
    pub fn has(&self, id: &str) -> bool {
        self.rows.iter().any(|(x, _)| x.eq_ignore_ascii_case(id))
    }
}

/// A cached grant verdict: the kit crate's overall verdict, the six checks (raw token/state pairs) and the
/// verification time; plus the upstream ledger audit label and chain time (shown as "upstream" and
/// "remaining" on the vault detail card), and the block time the grant was anchored at in its issuer's ledger
/// (used by the vault's date filter after a restart, before any re-check). Files from older versions lack the
/// last three fields, which read as empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub verdict: String,
    pub checks: Vec<(String, String)>,
    pub at: u64,
    pub upstream_label: String,
    pub chain_now: Option<u64>,
    pub anchored_at: Option<u64>,
}

/// Whether a cache time is more than [`STALE_SECS`] old. A cache time in the future (clock set back) also
/// counts as stale.
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

/// Save the `anchored` set when a new audit arrives, overwriting the previous one. Written as canonical JSON.
pub fn save_anchored(home: &Home, rows: &[(String, (u64, String))], at: u64, whole: bool) -> Result<(), Fault> {
    let arr: Vec<Value> = rows
        .iter()
        .map(|(id, (chain, tx))| {
            Value::Obj(vec![
                ("chain".to_string(), Value::Int(*chain)),
                ("id".to_string(), Value::Str(id.clone())),
                ("tx".to_string(), Value::Str(tx.clone())),
            ])
        })
        .collect();
    let v = Value::Obj(vec![("anchored".to_string(), Value::Arr(arr)), ("at".to_string(), Value::Int(at)), ("whole".to_string(), Value::Bool(whole))]);
    crate::local::put(&home.dir(Slot::Settings), ANCHORED_FILE, crate::local::Doc::LastAudit, &json::canon_bytes(&v))
}

/// Read the `anchored` set. No file gives `None` ("never audited" is not an error); a malformed file is an
/// error, never silently treated as an empty set.
pub fn load_anchored(home: &Home) -> Result<Option<Anchored>, Fault> {
    let p = home.dir(Slot::Settings).join(ANCHORED_FILE);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::LastAudit)? else { return Ok(None) };
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))?;
    let (Some(rows), Some(at)) = (arr_of(&v, "anchored"), int_of(&v, "at")) else {
        return Err(Fault::known(Known::SettingsShape, p.display().to_string()));
    };
    let mut out = Vec::new();
    for r in rows {
        let (Some(id), Some(chain), Some(tx)) = (str_of(r, "id"), int_of(r, "chain"), str_of(r, "tx")) else {
            return Err(Fault::known(Known::SettingsShape, p.display().to_string()));
        };
        out.push((id, (chain, tx)));
    }
    // Older files lack "whole" and read as not complete, so they are never used to conclude an entry is absent.
    let whole = match &v {
        Value::Obj(m) => m.iter().any(|(k, x)| k == "whole" && *x == Value::Bool(true)),
        _ => false,
    };
    Ok(Some(Anchored { rows: out, at, whole }))
}

/// Whether this file is a verdict cache (by file name). Code that wants the vault's contents (such as
/// `vaultx::held_rows`) skips it: the cache changes with every re-check and is not a received grant, and in a
/// backup it would collide on restore with the re-checked home's copy ("exists with different bytes").
pub fn is_cache(name: &str) -> bool {
    name.ends_with(VERDICT_SUFFIX)
}

/// Where a grant's verdict file lives.
pub fn verdict_path(home: &Home, id: &str) -> Result<std::path::PathBuf, Fault> {
    // Same keyed stem as the held grant it belongs to (`vaultx::held_stem`).
    Ok(home.dir(Slot::GrantsHeld).join(format!("{}{VERDICT_SUFFIX}", crate::vaultx::held_stem(id)?)))
}

/// Save a verdict when a re-check arrives, overwriting the previous one.
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
    // Name the grant inside the sealed file, since the file name is a keyed hash that reveals nothing.
    m.push(("grant".to_string(), Value::Str(grant_form(id))));
    m.push(("upstream_label".to_string(), Value::Str(v.upstream_label.clone())));
    m.push(("verdict".to_string(), Value::Str(verdict.to_string())));
    let v = Value::Obj(m);
    let p = verdict_path(home, id)?;
    let name = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    crate::local::put(&home.dir(Slot::GrantsHeld), &name, crate::local::Doc::Verdict, &json::canon_bytes(&v))
}

/// Normal form of a grant id in verdict caches and `load_verdicts` results: `0x` and lowercase, as
/// `Entry::id_hex` writes it (callers compare against that).
pub fn grant_form(id: &str) -> String {
    format!("0x{}", id.trim().trim_start_matches("0x").trim_start_matches("0X").to_ascii_lowercase())
}

/// Normal form of an issuer address used as the key of the user's note about it (written by the vault, read
/// by the window): `0x` and lowercase, as [`grant_form`].
pub fn issuer_form(author: &str) -> String {
    grant_form(author)
}

/// Add the `grant` member to a verdict's bytes, for caches from older versions that named the grant only by
/// file name. Bytes that already have it are returned unchanged.
pub fn with_grant(bytes: &[u8], id: &str) -> Result<Vec<u8>, Fault> {
    let v = json::parse(bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("verdict {t:?}")))?;
    let Value::Obj(mut m) = v else { return Err(Fault::known(Known::SettingsShape, "verdict".to_string())) };
    if m.iter().any(|(k, _)| k == "grant") {
        return Ok(bytes.to_vec());
    }
    m.push(("grant".to_string(), Value::Str(grant_form(id))));
    Ok(json::canon_bytes(&Value::Obj(m)))
}

/// Read a verdict. `None` when absent; a malformed file is an error.
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
    Ok(Some(Verdict { verdict, checks, at, upstream_label: str_of(&v, "upstream_label").unwrap_or_default(), chain_now: int_of(&v, "chain_now"), anchored_at: int_of(&v, "anchored_at") }))
}

/// Read every verdict in the vault (for startup hydration): files under `grants-held/` ending in
/// [`VERDICT_SUFFIX`]. A malformed one goes into the error list without blocking the others.
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
        // The grant id is inside the file; the file name is keyed.
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

/// The system clock in Unix seconds. The default `Shell::clock`; tests replace it with a fixed value.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
