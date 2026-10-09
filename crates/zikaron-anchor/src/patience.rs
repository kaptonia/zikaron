//! The patience table: what to do when a node's answer is a trouble that may pass if asked again a moment later.
//! One table, read by every question a read, a scan or a send asks before its broadcast; the broadcast itself
//! ([`crate::rpc::BROADCAST`]) is never in it (carried once, on a new connection).
//!
//! | class | when | asked again |
//! |---|---|---|
//! | rate limited | a 429, or the node's words carry a rate-limit marker and name no range | after 300 ms, then after 900 ms; still limited, the trouble is passed on (the caller moves to the next node or refuses by name) |
//! | cut before the answer | the connection broke before one byte of the answer came | once more at once, on a new connection, reads only: done by the transport (`zikaron_net`), so it never reaches this table |
//! | server error | an HTTP status from 500 to 599 | after 300 ms, once |
//! | timeout | the exchange's deadline passed | never (the deadline is spent) |
//! | anything else | a refusal about what was asked, a shape, a wrong chain… | never |
//!
//! Each class asks at most twice more, so a limited node is never asked faster or more often for being
//! limited; every pause is under a second, so patience never keeps a person waiting for minutes. Pauses can
//! be set for a test ([`set_waits`]: zero, so no wall clock is waited).

use crate::rpc::{self, Endpoint, Trouble};
use crate::wire::W;
use std::time::Duration;
use zikaron::json::Value;

/// What kind of trouble an answer was, for patience. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    RateLimited,
    CutBeforeAnswer,
    ServerError,
    Timeout,
    Other,
}

/// The rate-limited row's pauses.
pub const RATE_LIMITED: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(900)];
/// The server-error row's pause.
pub const SERVER_ERROR: [Duration; 1] = [Duration::from_millis(300)];

/// The pauses before each new ask, per class: the table above, the one place these numbers live.
pub const TABLE: [(Class, &[Duration]); 5] = [
    (Class::RateLimited, &RATE_LIMITED),
    (Class::CutBeforeAnswer, &[]),
    (Class::ServerError, &SERVER_ERROR),
    (Class::Timeout, &[]),
    (Class::Other, &[]),
];

/// Words in a node's refusal that say the range asked was too wide: such a refusal is about the range, never
/// waited out, even when it also reads as a limit being exceeded (`block range limit exceeded`).
const RANGE_WORDS: [&str; 3] = ["range", "results", "blocks"];

/// Which class a trouble is. A rate limit is a 429 or a node's words with a rate-limit marker
/// (`said::says_rate_limited`; the code alone is not enough) naming no range; a server error is a status from
/// 500 to 599; a timeout is the deadline passing (`rpc::is_late`). A cut before the answer is the transport's
/// and never comes here: what does come is its failure after its one more ask, which is not waited out.
pub fn class_of(t: &Trouble) -> Class {
    let range = match t {
        Trouble::Node(e) => {
            let low = e.to_lowercase();
            RANGE_WORDS.iter().any(|w| low.contains(w))
        }
        _ => false,
    };
    if crate::said::says_rate_limited(t) && !range {
        return Class::RateLimited;
    }
    match t {
        Trouble::Transport(e) if rpc::status_of(e).is_some_and(|s| (500..600).contains(&s)) => Class::ServerError,
        Trouble::Transport(e) if rpc::is_late(e) => Class::Timeout,
        _ => Class::Other,
    }
}

static WAITS: std::sync::Mutex<Option<Duration>> = std::sync::Mutex::new(None);

/// Every pause of the table taken as `d` from now on in this process (`None`: the table's own). Tests set zero,
/// so no wall clock is waited and the number of asks stays the table's.
pub fn set_waits(d: Option<Duration>) {
    *WAITS.lock().unwrap_or_else(|e| e.into_inner()) = d;
}

/// A pause as this process takes it ([`set_waits`]): the pause itself, or the one set for tests.
pub fn waited(d: Duration) -> Duration {
    WAITS.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or(d)
}

/// The pauses of a class, as this process takes them ([`set_waits`]).
pub fn pauses(c: Class) -> Vec<Duration> {
    let row = TABLE.iter().find(|(k, _)| *k == c).map(|(_, p)| p.to_vec()).unwrap_or_default();
    match *WAITS.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(d) => vec![d; row.len()],
        None => row,
    }
}

/// Ask one node one question with patience: asked again after each pause of its trouble's class, at most as
/// many times as the class's row has pauses (each class counted apart); the last trouble is passed on. A
/// broadcast is asked once, whatever comes back.
pub fn ask(ep: &mut dyn Endpoint, method: &str, params: &Value) -> Result<W, Trouble> {
    if method == rpc::BROADCAST {
        return ep.call(method, params);
    }
    let mut taken: Vec<(Class, usize)> = Vec::new();
    loop {
        let t = match ep.call(method, params) {
            Ok(w) => return Ok(w),
            Err(t) => t,
        };
        let c = class_of(&t);
        let n = taken.iter().find(|(k, _)| *k == c).map(|(_, n)| *n).unwrap_or(0);
        let Some(pause) = pauses(c).get(n).copied() else { return Err(t) };
        if !pause.is_zero() {
            std::thread::sleep(pause);
        }
        match taken.iter_mut().find(|(k, _)| *k == c) {
            Some((_, m)) => *m += 1,
            None => taken.push((c, 1)),
        }
    }
}
