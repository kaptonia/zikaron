use crate::rpc::{self, Trouble};
use crate::wire;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    Funds,
    NonceUsed,
    Pending,
    Underpriced,
    GasTooLow,
    Reverted,
    RateLimited,
    NoMethod,
    Auth,
    WrongChain,
    Coded(String),
    Other(String),
}

impl Refusal {
    pub fn about_the_call(&self) -> bool {
        matches!(self, Refusal::Funds | Refusal::NonceUsed | Refusal::Pending | Refusal::Underpriced | Refusal::GasTooLow | Refusal::Reverted | Refusal::Coded(_))
    }
}

const MARKS: [(&[&str], fn() -> Refusal); 10] = [
    (&["insufficient funds", "insufficient balance"], || Refusal::Funds),
    (&["nonce too low", "nonce has already been used", "nonce is too low", "already been used"], || Refusal::NonceUsed),
    (&["already known", "known transaction", "already imported", "already exists", "alreadyknown"], || Refusal::Pending),
    (&["underpriced", "fee cap less than block base fee", "max fee per gas less than block base fee", "less than block base fee", "fee too low"], || Refusal::Underpriced),
    (&["intrinsic gas too low", "gas too low", "gas limit too low"], || Refusal::GasTooLow),
    (&["execution reverted", "reverted"], || Refusal::Reverted),
    (&["rate limit", "too many requests", "limit exceeded", "request limit", "capacity exceeded", "throttled"], || Refusal::RateLimited),
    (&["method not found", "does not exist/is not available", "not supported", "unsupported method", "method not available"], || Refusal::NoMethod),
    (&["unauthorized", "authentication", "api key", "project id", "forbidden", "access denied", "invalid key"], || Refusal::Auth),
    (&["chain id", "chainid", "wrong chain", "invalid chain"], || Refusal::WrongChain),
];

pub fn refusal_of(err: &str) -> Refusal {
    if let Some(r) = marked(err) {
        return r;
    }
    let w = wire::parse(err.as_bytes());
    let code = match w.as_ref().and_then(|x| x.member("code")).map(|c| &c.body) {
        Some(wire::Body::Num(n)) => n,
        _ => return Refusal::Other(err.to_string()),
    };
    match code.parse::<i64>().ok() {
        Some(-32601) => Refusal::NoMethod,
        Some(-32005) => Refusal::RateLimited,
        Some(3) => Refusal::Reverted,
        _ if reverted(err).is_some() => Refusal::Reverted,
        _ => Refusal::Coded(err.to_string()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LackSaid {
    Words,
    Code,
}

pub fn lack_said(err: &str) -> Option<LackSaid> {
    match marked(err) {
        Some(Refusal::NoMethod) => Some(LackSaid::Words),
        Some(_) => None,
        None => (refusal_of(err) == Refusal::NoMethod).then_some(LackSaid::Code),
    }
}

fn marked(err: &str) -> Option<Refusal> {
    let w = wire::parse(err.as_bytes());
    let message = w.as_ref().and_then(|x| x.member("message")).and_then(|m| m.as_str()).unwrap_or(err).to_lowercase();
    MARKS.iter().find(|(marks, _)| marks.iter().any(|m| message.contains(m))).map(|(_, which)| which())
}

pub fn says_rate_limited(t: &Trouble) -> bool {
    match t {
        Trouble::Node(e) => marked(e) == Some(Refusal::RateLimited),
        Trouble::Transport(_) => rpc::status_of(t) == Some(429),
        Trouble::NotServed(_) | Trouble::Contradiction(_) => false,
    }
}

pub fn refuses_the_call(t: &Trouble) -> bool {
    refusal(t).is_some_and(|r| r.about_the_call())
}

fn refusal_at(status: u16, said: &str) -> Refusal {
    match status {
        429 => Refusal::RateLimited,
        401 | 403 => Refusal::Auth,
        _ => Refusal::Other(said.to_string()),
    }
}

pub fn refusal(t: &Trouble) -> Option<Refusal> {
    match t {
        Trouble::Node(e) => Some(refusal_of(e)),
        Trouble::Transport(b) => rpc::status_of(t).map(|s| refusal_at(s, &b.evidence())),
        Trouble::NotServed(_) | Trouble::Contradiction(_) => None,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reverted {
    Text(String),
    Panic(u64),
    Custom([u8; 4], usize),
    Silent,
    Unreadable(String),
}

pub const REASON_CAP: usize = 256;

const ERROR_STRING: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];
const PANIC: [u8; 4] = [0x4e, 0x48, 0x7b, 0x71];

pub fn reverted(err: &str) -> Option<Reverted> {
    let w = wire::parse(err.as_bytes())?;
    let data = w.member("data")?;
    let text = data.as_str().or_else(|| data.member("data").and_then(|d| d.as_str()))?;
    let bytes = zikaron::hexfmt::decode(text.trim())?;
    Some(reverted_of(&bytes))
}

pub fn reverted_of(b: &[u8]) -> Reverted {
    let hex = || zikaron::hexfmt::encode(b);
    if b.is_empty() {
        return Reverted::Silent;
    }
    if b.len() < 4 {
        return Reverted::Unreadable(hex());
    }
    let (selector, body) = ([b[0], b[1], b[2], b[3]], &b[4..]);
    let word = |at: usize| -> Option<u64> {
        let w = body.get(at..at.checked_add(32)?)?;
        w[..24].iter().all(|x| *x == 0).then(|| u64::from_be_bytes(w[24..].try_into().unwrap_or([0; 8])))
    };
    match selector {
        ERROR_STRING => {
            let text = (|| {
                let offset = usize::try_from(word(0)?).ok()?;
                let len = usize::try_from(word(offset)?).ok()?;
                let start = offset.checked_add(32)?;
                body.get(start..start.checked_add(len)?)
            })();
            match text {
                Some(t) => {
                    let said = String::from_utf8_lossy(t);
                    let mut kept: String = said.chars().take(REASON_CAP).flat_map(|c| if c.is_control() { c.escape_default().collect::<Vec<_>>() } else { vec![c] }).collect();
                    let n = said.chars().count();
                    if n > REASON_CAP {
                        kept.push_str(&format!("\u{2026} ({} more characters)", n - REASON_CAP));
                    }
                    Reverted::Text(kept)
                }
                None => Reverted::Unreadable(hex()),
            }
        }
        PANIC if body.len() == 32 => match word(0) {
            Some(code) => Reverted::Panic(code),
            None => Reverted::Unreadable(hex()),
        },
        PANIC => Reverted::Unreadable(hex()),
        _ => Reverted::Custom(selector, body.len()),
    }
}

impl Reverted {
    pub fn evidence(&self) -> String {
        match self {
            Reverted::Text(t) => format!("reason: {t}"),
            Reverted::Panic(code) => match panic_meaning(*code) {
                Some(m) => format!("panic 0x{code:02x}: {m}"),
                None => format!("panic 0x{code:02x}"),
            },
            Reverted::Custom(sel, n) => format!("custom error {} with {n} bytes of arguments", zikaron::hexfmt::encode(sel)),
            Reverted::Silent => "no reason given".into(),
            Reverted::Unreadable(h) => format!("revert data not readable: {h}"),
        }
    }
}

fn panic_meaning(code: u64) -> Option<&'static str> {
    Some(match code {
        0x00 => "generic compiler panic",
        0x01 => "assertion failed",
        0x11 => "arithmetic overflow or underflow",
        0x12 => "division or modulo by zero",
        0x21 => "conversion to an enum out of range",
        0x22 => "storage byte array incorrectly encoded",
        0x31 => "pop on an empty array",
        0x32 => "array index out of bounds",
        0x41 => "too much memory allocated",
        0x51 => "call to a zero-initialized function",
        _ => return None,
    })
}

pub const RATE_BACKOFF: [std::time::Duration; 2] = crate::patience::RATE_LIMITED;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_node_said_is_one_closed_table() {
        let node = |m: &str, code: i64| Trouble::Node(format!("{{\"code\":{code},\"message\":\"{m}\"}}"));
        for (said, want) in [
            ("insufficient funds for gas * price + value", Refusal::Funds),
            ("nonce too low", Refusal::NonceUsed),
            ("already known", Refusal::Pending),
            ("replacement transaction underpriced", Refusal::Underpriced),
            ("intrinsic gas too low", Refusal::GasTooLow),
            ("execution reverted", Refusal::Reverted),
            ("daily request limit exceeded", Refusal::RateLimited),
            ("the method eth_x does not exist/is not available", Refusal::NoMethod),
            ("unauthorized: bad api key", Refusal::Auth),
            ("invalid chain id for signer", Refusal::WrongChain),
            ("execution reverted: rate limit", Refusal::Reverted),
        ] {
            assert_eq!(refusal(&node(said, -32000)), Some(want), "{said}");
        }
        assert_eq!(refusal(&node("x", -32601)), Some(Refusal::NoMethod));
        assert_eq!(lack_said(r#"{"code":-32601,"message":"Request contains invalid block params"}"#), Some(LackSaid::Code));
        assert_eq!(lack_said(r#"{"code":-32601,"message":"the method eth_x does not exist/is not available"}"#), Some(LackSaid::Words));
        assert_eq!(lack_said(r#"{"code":-32000,"message":"method not found"}"#), Some(LackSaid::Words));
        assert_eq!(lack_said(r#"{"code":-32005,"message":"rate limit exceeded"}"#), None);
        assert_eq!(refusal(&node("x", -32005)), Some(Refusal::RateLimited));
        assert_eq!(refusal(&node("x", 3)), Some(Refusal::Reverted));
        assert!(matches!(refusal(&node("exceeds block gas limit", -32000)), Some(Refusal::Coded(_))));
        assert!(matches!(refusal(&Trouble::Node("not json at all".into())), Some(Refusal::Other(_))));
        let at = |s: u16| refusal(&rpc::status("https://n.example", s));
        assert_eq!(at(429), Some(Refusal::RateLimited));
        assert_eq!(at(401), Some(Refusal::Auth));
        assert_eq!(at(403), Some(Refusal::Auth));
        for s in [400, 404, 428, 430, 500, 502, 503, 301] {
            match at(s) {
                Some(Refusal::Other(w)) => assert!(w.contains(&s.to_string()), "{w}"),
                other => panic!("{s}: {other:?}"),
            }
        }
        for t in [Trouble::Transport(rpc::Broke::NotJson), rpc::shapeless("https://n.example"), rpc::late("u", std::time::Duration::from_secs(1)), Trouble::NotServed("k".into())] {
            assert_eq!(refusal(&t), None, "{t:?}");
        }
    }

    #[test]
    fn a_refusal_about_the_call_is_told_from_one_about_the_node() {
        for r in [Refusal::Funds, Refusal::NonceUsed, Refusal::Pending, Refusal::Underpriced, Refusal::GasTooLow, Refusal::Reverted, Refusal::Coded(String::new())] {
            assert!(r.about_the_call(), "{r:?}");
        }
        for r in [Refusal::RateLimited, Refusal::NoMethod, Refusal::Auth, Refusal::WrongChain, Refusal::Other(String::new())] {
            assert!(!r.about_the_call(), "{r:?}");
        }
    }
}
