use crate::envelope::{self as env, Doc, Expect, Owner};
use crate::keybox::LocalKey;
use crate::names::NameKey;
use crate::trouble::{Code, Trouble};
use std::path::Path;
use zikaron::json::Value;

pub mod member {
    pub const PROXY: &str = "proxy";
    pub const CLI_ANCHOR: &str = "cli_anchor";
    pub const CLI_ALONE: &str = "cli_alone";
    pub const REST_KEYS: &str = "rest_keys";
}

pub mod proxy {
    pub const SYSTEM: &str = "system";
    pub const NONE: &str = "none";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CliAnchor {
    #[default]
    Send,
    Queue,
}

impl CliAnchor {
    pub const ALL: [CliAnchor; 2] = [CliAnchor::Send, CliAnchor::Queue];

    pub fn as_str(self) -> &'static str {
        match self {
            CliAnchor::Send => "send",
            CliAnchor::Queue => "queue",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Send,
    Leave,
    Nothing,
}

pub fn turn(cli_anchor: CliAnchor, count: usize) -> Turn {
    match (cli_anchor, count) {
        (_, 0) => Turn::Nothing,
        (CliAnchor::Send, _) => Turn::Send,
        (CliAnchor::Queue, _) => Turn::Leave,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lasts {
    Secs(u64),
    Never,
}

pub const LASTS_MAX: u64 = 365 * 86_400;
pub const LASTS_MIN: u64 = 60;
pub const NEVER: &str = "never";

impl Lasts {
    pub fn ok(self) -> bool {
        match self {
            Lasts::Secs(n) => (LASTS_MIN..=LASTS_MAX).contains(&n),
            Lasts::Never => true,
        }
    }

    pub fn expires(self, issued: u64) -> Option<u64> {
        match self {
            Lasts::Secs(n) => Some(issued + n),
            Lasts::Never => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cli {
    pub proxy: Option<String>,
    pub cli_anchor: CliAnchor,
    pub cli_alone: Option<Lasts>,
    pub rest_keys: RestKeys,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct RestKeys(pub Vec<(String, String)>);

impl std::fmt::Debug for RestKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter().map(|(s, k)| (s.as_str(), k.len()))).finish()
    }
}

impl RestKeys {
    pub fn value(&self) -> Value {
        Value::Obj(self.0.iter().map(|(s, k)| (s.clone(), Value::Str(k.clone()))).collect())
    }

    pub fn of(&self, service: &str) -> Option<&str> {
        self.0.iter().find(|(s, _)| s.eq_ignore_ascii_case(service)).map(|(_, k)| k.as_str())
    }

    pub fn tail(&self, service: &str) -> Option<String> {
        let k: Vec<char> = self.of(service)?.chars().collect();
        Some(k[k.len().saturating_sub(4)..].iter().collect())
    }
}

pub fn take(into: &mut Cli, k: &str, v: &Value) -> Result<bool, Trouble> {
    match (k, v) {
        (member::PROXY, Value::Str(p)) => into.proxy = Some(p.clone()),
        (member::CLI_ANCHOR, Value::Str(w)) if CliAnchor::ALL.iter().any(|x| x.as_str() == w) => {
            into.cli_anchor = CliAnchor::ALL.into_iter().find(|x| x.as_str() == w).unwrap_or_default();
        }
        (member::CLI_ANCHOR, _) => return Err(Trouble::new(Code::MachineShape, member::CLI_ANCHOR)),
        (member::CLI_ALONE, Value::Int(n)) if Lasts::Secs(*n).ok() => into.cli_alone = Some(Lasts::Secs(*n)),
        (member::CLI_ALONE, Value::Str(w)) if w == NEVER => into.cli_alone = Some(Lasts::Never),
        (member::CLI_ALONE, _) => return Err(Trouble::new(Code::MachineShape, member::CLI_ALONE)),
        (member::REST_KEYS, Value::Obj(ms)) => {
            into.rest_keys.0 = ms
                .iter()
                .map(|(s, k)| match k {
                    Value::Str(k) if !s.is_empty() => Ok((s.clone(), k.clone())),
                    _ => Err(Trouble::new(Code::MachineShape, member::REST_KEYS)),
                })
                .collect::<Result<_, _>>()?;
        }
        (member::REST_KEYS, _) => return Err(Trouble::new(Code::MachineShape, member::REST_KEYS)),
        _ => return Ok(false),
    }
    Ok(true)
}

pub fn of_plain(bytes: &[u8], what: &str) -> Result<Cli, Trouble> {
    let v = zikaron::json::parse(bytes).map_err(|t| Trouble::new(Code::MachineShape, format!("{what}: {t:?}")))?;
    let Value::Obj(members) = v else { return Err(Trouble::new(Code::MachineShape, what.to_string())) };
    let mut out = Cli::default();
    for (k, val) in &members {
        take(&mut out, k, val)?;
    }
    Ok(out)
}

pub fn read(machine: &Path, account: &str, local: &LocalKey, names: &NameKey) -> Result<Cli, Trouble> {
    let rel = crate::places::machine_file_for(account);
    let at = machine.join(&rel);
    let bytes = match std::fs::read(&at) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Cli::default()),
        Err(e) => return Err(Trouble::io(&e, &at.display().to_string())),
    };
    let ex = Expect { owner: Some(Owner::Machine), doc: Doc::MachineSealed, rel: Some(rel.clone()) };
    let plain = env::open_with(local, &ex, &bytes, &at.display().to_string(), Some(names))?;
    of_plain(&plain, &rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_read_as_the_desktop_writes_them() {
        let read = |s: &str| of_plain(s.as_bytes(), "m");
        assert_eq!(read(r#"{"shape":"x"}"#).expect("defaults"), Cli::default());
        let all = read(r#"{"cli_alone":3600,"cli_anchor":"queue","proxy":"http://p:8080"}"#).expect("all three");
        assert_eq!(all, Cli { proxy: Some("http://p:8080".into()), cli_anchor: CliAnchor::Queue, cli_alone: Some(Lasts::Secs(3600)), rest_keys: RestKeys::default() });
        assert_eq!(read(r#"{"cli_alone":"never","cli_anchor":"send"}"#).expect("never").cli_alone, Some(Lasts::Never));
        assert_eq!(read(r#"{"proxy":7}"#).expect("not text").proxy, None, "a proxy that is not text is not taken here");
        for (bad, name) in [(r#"{"cli_anchor":"later"}"#, member::CLI_ANCHOR), (r#"{"cli_anchor":1}"#, member::CLI_ANCHOR), (r#"{"cli_alone":59}"#, member::CLI_ALONE), (r#"{"cli_alone":31536001}"#, member::CLI_ALONE), (r#"{"cli_alone":"always"}"#, member::CLI_ALONE)] {
            assert_eq!(read(bad), Err(Trouble::new(Code::MachineShape, name)), "{bad}");
        }
        assert_eq!(read(r#"{"cli_alone":60}"#).expect("the least").cli_alone, Some(Lasts::Secs(LASTS_MIN)));
        assert_eq!(read(r#"{"cli_alone":31536000}"#).expect("the most").cli_alone, Some(Lasts::Secs(LASTS_MAX)));
        assert_eq!(read("[]").map_err(|t| t.code), Err(Code::MachineShape));
    }

    #[test]
    fn the_switch_sends_what_there_is_or_leaves_it() {
        for (sw, n, want) in [
            (CliAnchor::Send, 0, Turn::Nothing),
            (CliAnchor::Queue, 0, Turn::Nothing),
            (CliAnchor::Send, 1, Turn::Send),
            (CliAnchor::Send, 9, Turn::Send),
            (CliAnchor::Queue, 1, Turn::Leave),
            (CliAnchor::Queue, 9, Turn::Leave),
        ] {
            assert_eq!(turn(sw, n), want, "{sw:?} {n}");
        }
    }
}
