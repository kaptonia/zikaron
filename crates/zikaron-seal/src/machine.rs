//! What the sealed machine settings say about the command line: how its sending is handled (`cli_anchor`), the
//! proxy (`proxy`) and whether it writes alone with a credential (`cli_alone`). The file is the desktop's (the
//! sealed part of its machine settings, `places::machine_file_for`, sealed as `Doc::MachineSealed`); these three
//! members are read here only, by the desktop's settings and by the command line opening the same file with its
//! credential alike. Every other member is the desktop's own.

use crate::envelope::{self as env, Doc, Expect, Owner};
use crate::keybox::LocalKey;
use crate::names::NameKey;
use crate::trouble::{Code, Trouble};
use std::path::Path;
use zikaron::json::Value;

/// The three members' names.
pub mod member {
    pub const PROXY: &str = "proxy";
    pub const CLI_ANCHOR: &str = "cli_anchor";
    pub const CLI_ALONE: &str = "cli_alone";
}

/// The proxy member's two words besides an address: follow the system's settings, or go straight.
pub mod proxy {
    pub const SYSTEM: &str = "system";
    pub const NONE: &str = "none";
}

/// How the command line's sending is handled: send it as the send button would, or leave the entries queued for
/// the person to send from the desktop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CliAnchor {
    /// Send at once (the default, also when the member is absent).
    #[default]
    Send,
    /// Send nothing: show the request and leave the entries queued for the person.
    Queue,
}

impl CliAnchor {
    pub const ALL: [CliAnchor; 2] = [CliAnchor::Send, CliAnchor::Queue];

    /// The value written to the file.
    pub fn as_str(self) -> &'static str {
        match self {
            CliAnchor::Send => "send",
            CliAnchor::Queue => "queue",
        }
    }
}

/// What the command line's `anchor` does with a home's queue, by the switch: send the entries it can send, leave
/// them queued for the person (`E_QUEUED`), or, with none it can send, nothing to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Send,
    Leave,
    Nothing,
}

/// The one judgement of the switch, for both of the command line's roads (through the desktop's door and with its
/// credential): `count` entries that can be sent are sent when the switch says send and left queued when it says
/// queue; none is nothing to send, whatever it says.
pub fn turn(cli_anchor: CliAnchor, count: usize) -> Turn {
    match (cli_anchor, count) {
        (_, 0) => Turn::Nothing,
        (CliAnchor::Send, _) => Turn::Send,
        (CliAnchor::Queue, _) => Turn::Leave,
    }
}

/// How long a command-line credential lasts once issued: a number of seconds, or never expiring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lasts {
    Secs(u64),
    Never,
}

/// The longest a credential may last when it expires: a year.
pub const LASTS_MAX: u64 = 365 * 86_400;
/// The shortest: a minute.
pub const LASTS_MIN: u64 = 60;
/// The word written for a credential that never expires.
pub const NEVER: &str = "never";

impl Lasts {
    /// Whether this length may be chosen (between [`LASTS_MIN`] and [`LASTS_MAX`], or never).
    pub fn ok(self) -> bool {
        match self {
            Lasts::Secs(n) => (LASTS_MIN..=LASTS_MAX).contains(&n),
            Lasts::Never => true,
        }
    }

    /// When a credential issued at `issued` expires (`None`: never).
    pub fn expires(self, issued: u64) -> Option<u64> {
        match self {
            Lasts::Secs(n) => Some(issued + n),
            Lasts::Never => None,
        }
    }
}

/// The three as the file holds them: the proxy choice as written (`None`, never chosen), the handling of sending
/// (absent is [`CliAnchor::Send`]), writing alone (`None`, off).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cli {
    pub proxy: Option<String>,
    pub cli_anchor: CliAnchor,
    pub cli_alone: Option<Lasts>,
}

/// Read one member of the sealed settings into `into` when it is one of the three: `Ok(true)` taken, `Ok(false)`
/// not one of them (a proxy that is not text included: the desktop keeps it as written), and a member of the
/// three with a value this version does not write refused by its name (`MachineShape`).
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
        _ => return Ok(false),
    }
    Ok(true)
}

/// The three from the sealed settings' plain bytes: an object whose members of the three read ([`take`]); any
/// other member is not looked at. Bytes that are not an object are refused (`MachineShape`, `what` and the
/// trouble).
pub fn of_plain(bytes: &[u8], what: &str) -> Result<Cli, Trouble> {
    let v = zikaron::json::parse(bytes).map_err(|t| Trouble::new(Code::MachineShape, format!("{what}: {t:?}")))?;
    let Value::Obj(members) = v else { return Err(Trouble::new(Code::MachineShape, what.to_string())) };
    let mut out = Cli::default();
    for (k, val) in &members {
        take(&mut out, k, val)?;
    }
    Ok(out)
}

/// Open the sealed machine settings of `account` in the machine folder `machine` with the local key and the names
/// key the credential opened to, and read the three. No file is the defaults.
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

    /// One line per form of each of the three: read as set, absent as the default, another shape refused by the
    /// member's name (a proxy of another shape is not one of the three: left to the desktop as written).
    #[test]
    fn the_three_read_as_the_desktop_writes_them() {
        let read = |s: &str| of_plain(s.as_bytes(), "m");
        assert_eq!(read(r#"{"shape":"x"}"#).expect("defaults"), Cli::default());
        let all = read(r#"{"cli_alone":3600,"cli_anchor":"queue","proxy":"http://p:8080"}"#).expect("all three");
        assert_eq!(all, Cli { proxy: Some("http://p:8080".into()), cli_anchor: CliAnchor::Queue, cli_alone: Some(Lasts::Secs(3600)) });
        assert_eq!(read(r#"{"cli_alone":"never","cli_anchor":"send"}"#).expect("never").cli_alone, Some(Lasts::Never));
        assert_eq!(read(r#"{"proxy":7}"#).expect("not text").proxy, None, "a proxy that is not text is not taken here");
        for (bad, name) in [(r#"{"cli_anchor":"later"}"#, member::CLI_ANCHOR), (r#"{"cli_anchor":1}"#, member::CLI_ANCHOR), (r#"{"cli_alone":59}"#, member::CLI_ALONE), (r#"{"cli_alone":31536001}"#, member::CLI_ALONE), (r#"{"cli_alone":"always"}"#, member::CLI_ALONE)] {
            assert_eq!(read(bad), Err(Trouble::new(Code::MachineShape, name)), "{bad}");
        }
        assert_eq!(read(r#"{"cli_alone":60}"#).expect("the least").cli_alone, Some(Lasts::Secs(LASTS_MIN)));
        assert_eq!(read(r#"{"cli_alone":31536000}"#).expect("the most").cli_alone, Some(Lasts::Secs(LASTS_MAX)));
        assert_eq!(read("[]").map_err(|t| t.code), Err(Code::MachineShape));
    }

    /// The switch's one judgement, each switch by none, one and many entries to send.
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
