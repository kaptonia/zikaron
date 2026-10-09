//! What a node said: the one closed table both the app and the command line read a refusal by.
//!
//! A node's JSON-RPC error is read by its sentence markers first, then by its shape: a numeric `code` the
//! table names, a revert's `data`, any other numeric `code` (the node refused this call in words the table does
//! not know), or no numeric `code` at all (not the node's own refusal). An answer that was not the node's word
//! but carried an HTTP status (a gateway's page, a bare `429`) is read by that status: too many requests is
//! rate limiting, unauthorized and forbidden are credentials, any other is a refusal that carries its status.
//! Everything else the transport says is not a refusal.

use crate::rpc::{self, Trouble};
use crate::wire;

/// The node answered and refused. Closed; words the table does not know are [`Refusal::Coded`] when the node
/// gave them as its JSON-RPC error (a numeric `code`) and [`Refusal::Other`] otherwise, each with the original
/// words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The balance does not cover fee cap times gas.
    Funds,
    /// The nonce is used.
    NonceUsed,
    /// The same transaction is already pending.
    Pending,
    /// Underpriced (below the base fee, or a resend without enough increase).
    Underpriced,
    /// Gas too low.
    GasTooLow,
    /// The contract refused (execution reverted).
    Reverted,
    /// Rate limited.
    RateLimited,
    /// This node does not provide the method.
    NoMethod,
    /// Credentials required (key, project id).
    Auth,
    /// Wrong chain id.
    WrongChain,
    /// The node refused this call in its own JSON-RPC error, a numeric `code` with words the table does not
    /// otherwise name: the error as given.
    Coded(String),
    /// Not the node's own refusal of the call: an error without a numeric `code` (a gateway's words, a code
    /// written as text), or an answer refused at an HTTP status other than 429, 401 and 403: the words as given.
    Other(String),
}

impl Refusal {
    /// Whether the refusal is about the call itself, as opposed to this node's own state (rate limited, a
    /// method it lacks, credentials, a chain it does not serve) or words that are not the node's refusal at
    /// all. The node's JSON-RPC error with a numeric `code` the table does not name is the node refusing the
    /// call ([`Refusal::Coded`]). An estimate refused about the call would revert; one refused otherwise says
    /// nothing about the call.
    pub fn about_the_call(&self) -> bool {
        matches!(self, Refusal::Funds | Refusal::NonceUsed | Refusal::Pending | Refusal::Underpriced | Refusal::GasTooLow | Refusal::Reverted | Refusal::Coded(_))
    }
}

/// Markers in a JSON-RPC error sentence (compared in lowercase), one member per line; order decides: the
/// first match answers.
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

/// Read a node's error as a member: by its sentence markers first (codes differ between providers, and
/// sentences are more precise), then by its shape. A numeric `code` makes it the node's own JSON-RPC refusal:
/// -32601 method not provided, -32005 rate limited, 3 contract refused, a revert's `data` ([`reverted`]
/// reads it) contract refused, any other the node refusing the call ([`Refusal::Coded`]). Without a numeric
/// `code` (none, text, null, an error that is not an object) it is not the node's refusal of the call
/// ([`Refusal::Other`]). Both keep the original words.
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

/// A node's error read by its sentence markers alone (no code): `None` when no marker line matches.
fn marked(err: &str) -> Option<Refusal> {
    let w = wire::parse(err.as_bytes());
    let message = w.as_ref().and_then(|x| x.member("message")).and_then(|m| m.as_str()).unwrap_or(err).to_lowercase();
    MARKS.iter().find(|(marks, _)| marks.iter().any(|m| message.contains(m))).map(|(_, which)| which())
}

/// Whether a trouble says in so many words that the node is limiting its rate: a 429, or a node error whose
/// sentence carries a rate-limit marker. The rate-limit code alone does not say it (nodes give -32005 for
/// "too many results" and for other refusals too); a caller that waits a refusal out asks this.
pub fn says_rate_limited(t: &Trouble) -> bool {
    match t {
        Trouble::Node(e) => marked(e) == Some(Refusal::RateLimited),
        Trouble::Transport(e) => rpc::status_of(e) == Some(429),
        Trouble::NotServed(_) | Trouble::Contradiction(_) => false,
    }
}

/// Whether a trouble says the call itself is refused: a refusal about the call by the one table
/// ([`Refusal::about_the_call`]). The judgement the app and the command line both take at the gas estimate (a
/// call refused so would revert); the transport, a page at an HTTP status, an error without a numeric `code`
/// and a recording's hole say nothing about the call.
pub fn refuses_the_call(t: &Trouble) -> bool {
    refusal(t).is_some_and(|r| r.about_the_call())
}

/// A refusal read off an HTTP status: 429 rate limited, 401 and 403 credentials, any other the status in the
/// refusal's words.
fn refusal_at(status: u16, said: &str) -> Refusal {
    match status {
        429 => Refusal::RateLimited,
        401 | 403 => Refusal::Auth,
        _ => Refusal::Other(said.to_string()),
    }
}

/// What a trouble says as a refusal: a node's error by the table, an answer refused at its HTTP status by
/// the status; `None` for everything else (the transport, a malformed answer, a recording's hole).
pub fn refusal(t: &Trouble) -> Option<Refusal> {
    match t {
        Trouble::Node(e) => Some(refusal_of(e)),
        Trouble::Transport(e) => rpc::status_of(e).map(|s| refusal_at(s, e)),
        Trouble::NotServed(_) | Trouble::Contradiction(_) => None,
    }
}

/// What a contract said when it reverted, read from the node's error `data` by Solidity's error encoding.
/// Closed: the contract's sentence (`Error(string)`), a check the compiler put in (`Panic(uint256)`), an
/// error the contract declared (its selector and the bytes of its arguments), nothing at all (`0x`), or data
/// that is none of these. The member that carries the refusal ([`Refusal::Reverted`]) does not change; this is
/// only what the evidence says after it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reverted {
    /// `Error(string)`: the contract's own sentence (control characters escaped, at most [`REASON_CAP`]
    /// characters kept, the rest counted).
    Text(String),
    /// `Panic(uint256)`: the code, with its meaning when the compiler documents one.
    Panic(u64),
    /// An error the contract declared: its four-byte selector and how many bytes of arguments came with it.
    Custom([u8; 4], usize),
    /// `0x`: the contract reverted without saying why.
    Silent,
    /// Data that is not an error encoding: fewer than four bytes, or an `Error(string)` or `Panic(uint256)`
    /// whose body does not decode (its hex, as given).
    Unreadable(String),
}

/// The most characters of a contract's sentence the evidence keeps.
pub const REASON_CAP: usize = 256;

/// `Error(string)` and `Panic(uint256)`'s selectors.
const ERROR_STRING: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];
const PANIC: [u8; 4] = [0x4e, 0x48, 0x7b, 0x71];

/// What the revert in a node's error says: `None` when the error carries no `data` the reading knows (no
/// member, not text, not hex: then the node's own words are all there is). `data` is read as hex text, or as
/// an object's `data` member (nodes that nest it).
pub fn reverted(err: &str) -> Option<Reverted> {
    let w = wire::parse(err.as_bytes())?;
    let data = w.member("data")?;
    let text = data.as_str().or_else(|| data.member("data").and_then(|d| d.as_str()))?;
    let bytes = zikaron::hexfmt::decode(text.trim())?;
    Some(reverted_of(&bytes))
}

/// [`reverted`] over the data's bytes.
pub fn reverted_of(b: &[u8]) -> Reverted {
    let hex = || zikaron::hexfmt::encode(b);
    if b.is_empty() {
        return Reverted::Silent;
    }
    if b.len() < 4 {
        return Reverted::Unreadable(hex());
    }
    let (selector, body) = ([b[0], b[1], b[2], b[3]], &b[4..]);
    // One 32-byte word as a number, when it fits in 64 bits (the high 24 bytes zero).
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
    /// The words the evidence carries after a revert.
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

/// The panic codes the Solidity documentation names. Closed; any other code is said by number alone.
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

/// The pauses before asking a rate-limited node again (not whole seconds): the patience table's rate-limited
/// row (`patience::TABLE`), named here for the send loop that asks the same place again after each.
pub const RATE_BACKOFF: [std::time::Duration; 2] = crate::patience::RATE_LIMITED;

#[cfg(test)]
mod tests {
    use super::*;

    /// Each marker line answers its member; order decides between two lines; codes are read only after the
    /// markers; an answer refused at its status reads 429 as rate limited, 401 and 403 as credentials, any other as
    /// a refusal carrying the status; the transport's own failures are no refusal.
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
            // The first line that matches answers: a revert that mentions a rate limit is a revert.
            ("execution reverted: rate limit", Refusal::Reverted),
        ] {
            assert_eq!(refusal(&node(said, -32000)), Some(want), "{said}");
        }
        assert_eq!(refusal(&node("x", -32601)), Some(Refusal::NoMethod));
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
        for t in [Trouble::Transport(rpc::NOT_JSON.into()), rpc::shapeless("https://n.example"), rpc::late("u", std::time::Duration::from_secs(1)), Trouble::NotServed("k".into())] {
            assert_eq!(refusal(&t), None, "{t:?}");
        }
    }

    /// Which refusals are about the call itself: the six a node gives of the transaction and the node's coded
    /// refusal in words the table does not name; the other five are about the node or not its refusal.
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
