//! Printing and exiting: stdout carries one canonical JSON value with no trailing newline; exit codes follow
//! the table.
//!
//! Canonical bytes come only from the core's `json::canon_bytes` (member order, integer spelling, string
//! escapes), so two implementations handing the same `Value` to it produce identical bytes.
//!
//! The process has exactly two exits and they exclude each other:
//!
//! - [`emit`] is the only place that writes stdout, and it never returns: it writes, flushes and exits.
//! Nothing runs after it, so "print, then refuse" is unreachable.
//! - [`misuse`] writes stderr only and exits.
//!
//! So every run ends either through `misuse` with exit 2 and zero bytes on stdout, or through `emit` with
//! exactly one canonical value and the table's exit code. The tests also scan that stdout is written only in
//! this file and that `Exit::Misuse` appears only here and in `codes.rs`.
//!
//! Misuse (as in HARNESS) exits 2 with zero bytes on stdout, which is how downstream tells an answer from
//! misuse without parsing. The subject is named on the first stderr line as `<REASON> <subject>`, the subject
//! running to the end of the line (spaces included); later lines are for people and not part of the contract.

use crate::codes::{Exit, Key, Reason};
use std::io::Write;
use zikaron::json::{self, Value};

/// One answer: an exit code and a value to print.
pub struct Answer {
    pub exit: Exit,
    pub value: Value,
}

fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect(),
    )
}

/// A string value (free bytes other than key names go through here).
pub fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

/// An affirmative answer: `ok` true, exit 0.
pub fn affirmed(members: Vec<(Key, Value)>) -> Answer {
    let mut ms = vec![(Key::Ok, Value::Bool(true))];
    ms.extend(members);
    Answer {
        exit: Exit::Affirmed,
        value: obj(ms),
    }
}

/// A negative answer: the subject fails. Exit 1, with a reason.
pub fn denied(reason: Reason, members: Vec<(Key, Value)>) -> Answer {
    at(Exit::Denied, reason, members)
}

/// A neither answer: PARTIAL / GAPS / UNAVAILABLE. Exit 3.
///
/// `ok` is false here because it asks whether the answer is affirmative. That does not make it red: red is 1,
/// this is 3, and neither folds into the other.
pub fn partial(reason: Reason, members: Vec<(Key, Value)>) -> Answer {
    at(Exit::Partial, reason, members)
}

/// No answer: unreachable endpoint, disagreeing readings, declined scan. Exit 4 (law §9.4).
pub fn unanswered(reason: Reason, members: Vec<(Key, Value)>) -> Answer {
    at(Exit::Unanswered, reason, members)
}

fn at(exit: Exit, reason: Reason, members: Vec<(Key, Value)>) -> Answer {
    let mut ms = vec![
        (Key::Ok, Value::Bool(false)),
        (Key::Reason, s(reason.as_str())),
    ];
    ms.extend(members);
    Answer {
        exit,
        value: obj(ms),
    }
}

/// Pass the value from below as the answer (audit report, grant check verdict, depth reading): no wrapping,
/// no renamed keys.
pub fn verbatim(exit: Exit, value: Value) -> Answer {
    Answer { exit, value }
}

/// Print an answer, flush, exit by the table. No trailing newline: one value per call, and a newline is not a
/// separator.
///
/// It never returns. That is what keeps stdout empty on misuse: after printing there is no next step, so no
/// path can reach [`misuse`]. `process::exit` runs no destructors and does not flush Rust's buffers, so this
/// flushes first.
///
/// Nothing in this crate may rely on `Drop` for cleanup: temporary places are cleared explicitly by `landing`
/// and `pack`, and a `Drop`-based cleanup on the way to `run` would be cut off silently here.
pub fn emit(a: Answer) -> ! {
    let code = a.exit.code();
    let bytes = json::canon_bytes(&a.value);
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(&bytes);
    let _ = out.flush();
    drop(out);
    std::process::exit(code as i32)
}

/// Misuse: the subject on the first stderr line, nothing on stdout, exit 2.
pub fn misuse(reason: Reason, subject: &str) -> ! {
    eprintln!("{} {}", reason.as_str(), subject);
    std::process::exit(Exit::Misuse.code() as i32)
}
