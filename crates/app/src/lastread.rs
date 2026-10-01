//! What the last pass knew. Two caches on disk:
//!
//! 1. The `anchored` set from the self-audit report, saved with the report in the home's `settings/` room
//! ([`ANCHORED_FILE`]);
//! 2. Each held grant's six-check verdict and verification time, saved at `grants-held/<id>.verdict.json`
//! ([`verdict_path`]).
//!
//! Kept only in memory (`shell.audit`, `shell.cards`), both would be empty at startup, lights would all show
//! "confirming" and "not verified", and everyone opening the app would think anchoring failed and grants were
//! unchecked. So "what the last pass knew" gets a home on disk: startup and returning home hydrate from it
//! first, the face says "confirmed · verified hh:mm", the background still re-examines, and the new reading
//! replaces it on arrival.
//!
//! They are caches, not proof. What is read back never passes for this pass's audit: the anchor light's
//! "anchored" comes only from this pass's report (`ledgerx::Lamp::Anchored`), and the cache gives a different
//! light (`Lamp::Remembered`); every place that needs "anchored" to allow something (granting to others,
//! starting a relicense…) still accepts only this pass's report. Staleness only decides whether the face's
//! dot is gray or green, never yellow (yellow means "on its way"; the cache says "verified last time"). Not
//! anchored.
//!
//! Times are always given by the caller (seconds): the product takes them from the system clock (only through
//! `Shell::clock`, default [`now_secs`]); tests inject fixed times.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

/// The file name of the `anchored` set in `settings/`. One name, one home.
pub const ANCHORED_FILE: &str = "last-audit.json";

/// The suffix of verdict file names (`grants-held/<id>.verdict.json`). One name, one home.
pub const VERDICT_SUFFIX: &str = ".verdict.json";

/// A cache older than this is stale: the face's dot turns gray (never yellow), and the background still
/// re-examines.
pub const STALE_SECS: u64 = 24 * 3600;

/// The `anchored` set read back: which entries, each anchored in which transaction on which chain, and that
/// audit's time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Anchored {
    pub rows: Vec<(String, (u64, String))>,
    pub at: u64,
    /// Whether that pass's reading was whole (the `auditx::whole` decision, fixed when saved). Older files
    /// lack this cell and read as not whole.
    pub whole: bool,
}

impl Anchored {
    /// Whether this entry is in the last pass's set.
    pub fn has(&self, id: &str) -> bool {
        self.rows.iter().any(|(x, _)| x.eq_ignore_ascii_case(id))
    }
}

/// A verdict read back: the kit crate's overall verdict, the six checks (raw word pairs), the verification
/// time; plus that pass's upstream ledger audit label and chain time (the vault detail card's "upstream" and
/// "remaining" speak from them). Older files lack the last two cells, which read as empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub verdict: String,
    pub checks: Vec<(String, String)>,
    pub at: u64,
    pub upstream_label: String,
    pub chain_now: Option<u64>,
}

/// Whether stale: the cache time more than [`STALE_SECS`] before now is stale; now earlier than the cache
/// time (clock set back) also counts as stale.
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

/// Save the `anchored` set (when this pass's audit arrives; overwriting the previous one is intended). The
/// shape is a canonical value.
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

/// Read the `anchored` set. No file gives `None` ("never audited" is not an error); an unreadable shape is
/// refused by name (a broken cache is said plainly, never quietly used as an empty set).
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
    // The "whole" cell: older files lack it and read as not whole (a reading that cannot say whether it is
    // whole is not used to decide "absent").
    let whole = match &v {
        Value::Obj(m) => m.iter().any(|(k, x)| k == "whole" && *x == Value::Bool(true)),
        _ => false,
    };
    Ok(Some(Anchored { rows: out, at, whole }))
}

/// Whether this file is a verdict cache (by file name). Places that want "vault content" (bundle export's
/// `vaultx::held_rows`) skip it: the cache changes with this desk's re-checks and is not a received grant;
/// carried into a backup, restoring into the same re-checked home would collide with "exists with different
/// bytes".
pub fn is_cache(name: &str) -> bool {
    name.ends_with(VERDICT_SUFFIX)
}

/// Where a grant's verdict file lives. One name, one home.
pub fn verdict_path(home: &Home, id: &str) -> Result<std::path::PathBuf, Fault> {
    // The same keyed stem as the held grant it belongs to (`vaultx::held_stem`).
    Ok(home.dir(Slot::GrantsHeld).join(format!("{}{VERDICT_SUFFIX}", crate::vaultx::held_stem(id)?)))
}

/// Save a verdict (when a re-check arrives; overwriting the previous one is intended).
pub fn save_verdict(home: &Home, id: &str, v: &Verdict) -> Result<(), Fault> {
    let (verdict, checks, at) = (&v.verdict, &v.checks, v.at);
    let arr: Vec<Value> = checks
        .iter()
        .map(|(tok, st)| Value::Obj(vec![("state".to_string(), Value::Str(st.clone())), ("token".to_string(), Value::Str(tok.clone()))]))
        .collect();
    let mut m = vec![
        ("at".to_string(), Value::Int(at)),
    ];
    if let Some(n) = v.chain_now {
        m.push(("chain_now".to_string(), Value::Int(n)));
    }
    m.push(("checks".to_string(), Value::Arr(arr)));
    // The grant this verdict is for, inside the sealed file: its name on disk is keyed and says nothing.
    m.push(("grant".to_string(), Value::Str(grant_form(id))));
    m.push(("upstream_label".to_string(), Value::Str(v.upstream_label.clone())));
    m.push(("verdict".to_string(), Value::Str(verdict.to_string())));
    let v = Value::Obj(m);
    let p = verdict_path(home, id)?;
    let name = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    crate::local::put(&home.dir(Slot::GrantsHeld), &name, crate::local::Doc::Verdict, &json::canon_bytes(&v))
}

/// The one form a grant id takes inside a verdict cache and in what `load_verdicts` answers: `0x` and lower
/// case, as `Entry::id_hex` spells it (callers compare against that).
pub fn grant_form(id: &str) -> String {
    format!("0x{}", id.trim().trim_start_matches("0x").trim_start_matches("0X").to_ascii_lowercase())
}

/// A verdict's bytes naming its grant inside (an older cache named it only by its file name): as they are when
/// they already do.
pub fn with_grant(bytes: &[u8], id: &str) -> Result<Vec<u8>, Fault> {
    let v = json::parse(bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("verdict {t:?}")))?;
    let Value::Obj(mut m) = v else { return Err(Fault::known(Known::SettingsShape, "verdict".to_string())) };
    if m.iter().any(|(k, _)| k == "grant") {
        return Ok(bytes.to_vec());
    }
    m.push(("grant".to_string(), Value::Str(grant_form(id))));
    Ok(json::canon_bytes(&Value::Obj(m)))
}

/// Read a verdict. None when absent; an unreadable shape is refused by name.
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
    Ok(Some(Verdict { verdict, checks, at, upstream_label: str_of(&v, "upstream_label").unwrap_or_default(), chain_now: int_of(&v, "chain_now") }))
}

/// Read every verdict in the vault (for startup hydration): files under `grants-held/` ending in
/// [`VERDICT_SUFFIX`]. An unreadable one goes by name into a second list (a broken cache is said plainly)
/// without blocking the others.
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
        // The grant is named inside the file (its name on disk is keyed).
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

/// The system clock now (seconds). The default of `Shell::clock`; tests replace it with a fixed value.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
