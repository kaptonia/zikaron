use crate::envelope::{self as env, Doc, Expect, Ident, Owner, SETTINGS_DIR};
use crate::keybox::LocalKey;
use crate::names::NameKey;
use crate::trouble::{Code, Trouble};
use std::path::Path;
use zikaron::json::{self, Value};

pub const FILE: &str = "unfetched.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Unfetched,
    NewerElsewhere { missing: usize, lacks: Vec<String> },
}

impl State {
    pub fn lacking(lacks: &[String]) -> State {
        let mut l: Vec<String> = lacks.iter().map(|h| bare(h)).collect();
        l.sort();
        l.dedup();
        State::NewerElsewhere { missing: l.len(), lacks: l }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            State::Unfetched => word::UNFETCHED,
            State::NewerElsewhere { .. } => word::NEWER_ELSEWHERE,
        }
    }
}

pub mod member {
    pub const STATE: &str = "state";
    pub const MISSING: &str = "missing";
    pub const LACKS: &str = "lacks";
}

pub mod word {
    pub const UNFETCHED: &str = "unfetched";
    pub const NEWER_ELSEWHERE: &str = "newer-elsewhere";
}

fn bare(h: &str) -> String {
    h.trim().trim_start_matches("0x").to_ascii_lowercase()
}

pub fn render(s: &State) -> Vec<u8> {
    let mut m = vec![(member::STATE.to_string(), Value::Str(s.as_str().to_string()))];
    if let State::NewerElsewhere { missing, lacks } = s {
        m.insert(0, (member::MISSING.to_string(), Value::Int(*missing as u64)));
        if !lacks.is_empty() {
            m.insert(0, (member::LACKS.to_string(), Value::Arr(lacks.iter().map(|h| Value::Str(h.clone())).collect())));
        }
    }
    json::canon_bytes(&Value::Obj(m))
}

pub fn of_plain(bytes: &[u8], what: &str) -> Result<State, Trouble> {
    let bad = || Trouble::new(Code::SettingsShape, what.to_string());
    let v = json::parse(bytes).map_err(|_| bad())?;
    let Value::Obj(m) = &v else { return Err(bad()) };
    let state = m.iter().find(|(k, _)| k == member::STATE).and_then(|(_, v)| if let Value::Str(s) = v { Some(s.as_str()) } else { None });
    let missing = m.iter().find(|(k, _)| k == member::MISSING).and_then(|(_, v)| if let Value::Int(n) = v { Some(*n as usize) } else { None });
    let lacks: Option<Vec<String>> = match m.iter().find(|(k, _)| k == member::LACKS).map(|(_, v)| v) {
        None => Some(Vec::new()),
        Some(Value::Arr(a)) => a.iter().map(|x| x.as_str().map(bare).filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))).collect(),
        Some(_) => None,
    };
    match (state, missing, lacks) {
        (Some(word::UNFETCHED), _, _) => Ok(State::Unfetched),
        (Some(word::NEWER_ELSEWHERE), Some(n), Some(l)) if l.is_empty() || l.len() == n => Ok(State::NewerElsewhere { missing: n, lacks: l }),
        _ => Err(bad()),
    }
}

pub fn rel() -> String {
    format!("{SETTINGS_DIR}/{FILE}")
}

pub fn read(home: &Path, local: &LocalKey, names: &NameKey, number: &str) -> Result<Option<State>, Trouble> {
    let rel = rel();
    let at = home.join(&rel);
    let raw = match std::fs::read(&at) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Trouble::io(&e, &at.display().to_string())),
    };
    let ex = Expect { owner: Some(Owner::Home(number.to_string())), doc: Doc::Unfetched, rel: Some(rel.clone()) };
    let plain = env::open_with(local, &ex, &raw, &rel, Some(names))?;
    of_plain(&plain, &at.display().to_string()).map(Some)
}

pub fn write(home: &Path, local: &LocalKey, number: &str, s: &State) -> Result<(), Trouble> {
    let sealed = env::seal_checked(local, &Ident { owner: Owner::Home(number.to_string()), doc: Doc::Unfetched, logical: rel() }, &render(s))?;
    crate::land::put_at(&home.join(SETTINGS_DIR), FILE, &sealed)
}

pub fn clear(home: &Path) -> Result<(), Trouble> {
    let at = home.join(rel());
    match std::fs::remove_file(&at) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Trouble::io(&e, &at.display().to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mark_reads_back_as_written() {
        let h = |c: char| c.to_string().repeat(64);
        for s in [State::Unfetched, State::NewerElsewhere { missing: 0, lacks: Vec::new() }, State::NewerElsewhere { missing: 13, lacks: Vec::new() }, State::lacking(&[format!("0x{}", h('B')), h('a')])] {
            assert_eq!(of_plain(&render(&s), "m"), Ok(s.clone()));
        }
        assert_eq!(State::lacking(&[h('b'), format!("0x{}", h('B'))]), State::NewerElsewhere { missing: 1, lacks: vec![h('b')] }, "each once, bare and lowercase");
        let named = |missing: usize, lacks: &str| format!(r#"{{"lacks":{lacks},"missing":{missing},"state":"newer-elsewhere"}}"#);
        for bad in [named(2, &format!(r#"["{}"]"#, h('a'))), named(1, r#"["ab"]"#), named(1, r#""x""#)] {
            assert_eq!(of_plain(bad.as_bytes(), "m"), Err(Trouble::new(Code::SettingsShape, "m")), "{bad}");
        }
        for bad in [r#"{"state":"later"}"#, r#"{"state":"newer-elsewhere"}"#, r#"{"missing":1}"#, "[]", "x"] {
            assert_eq!(of_plain(bad.as_bytes(), "m"), Err(Trouble::new(Code::SettingsShape, "m")), "{bad}");
        }
    }
}
