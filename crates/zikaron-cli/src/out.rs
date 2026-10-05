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
//! exactly one canonical value and the table's exit code. An answer may carry one line for people
//! ([`Answer::told`]), which `emit` writes to stderr before stdout; it is not part of the contract, and stdout and
//! the exit code are the same with or without it. The tests also scan that stdout is written only in
//! this file and that `Exit::Misuse` appears only here and in `codes.rs`.
//!
//! Misuse (as in HARNESS) exits 2 with zero bytes on stdout, which is how downstream tells an answer from
//! misuse without parsing. The subject is named on the first stderr line as `<REASON> <subject>`, the subject
//! running to the end of the line (spaces included): the subject alone (the flag, the value, the path at
//! fault), never a sentence. What it means for people is the second line, worded in one table ([`Said`]);
//! later lines are not part of the contract.

use crate::codes::{Exit, Key, Reason};
use std::io::Write;
use zikaron::json::{self, Value};

/// One answer: an exit code and a value to print, and at most one line for people on stderr.
pub struct Answer {
    pub exit: Exit,
    pub value: Value,
    pub told: Option<String>,
}

impl Answer {
    /// The same answer with one line for people (stderr; never part of the contract).
    pub fn telling(self, line: String) -> Answer {
        Answer { told: Some(line), ..self }
    }
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
        told: None,
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
        told: None,
    }
}

/// Pass the value from below as the answer (audit report, grant check verdict, depth reading): no wrapping,
/// no renamed keys.
pub fn verbatim(exit: Exit, value: Value) -> Answer {
    Answer { exit, value, told: None }
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
    if let Some(line) = &a.told {
        eprintln!("{line}");
    }
    let code = a.exit.code();
    let bytes = json::canon_bytes(&a.value);
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(&bytes);
    let _ = out.flush();
    drop(out);
    std::process::exit(code as i32)
}

/// What a misuse says to people: the second stderr line. Closed; every word the command line says on stderr
/// is in this one table (today in Chinese; the subject on the first line is the machine's part).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Said {
    Usage,
    NotAVerb,
    NotAFlag,
    NeedsValue,
    NotThisVerbsFlag,
    Repeated,
    Missing,
    OneOf,
    NotInt,
    KeyNotHex,
    KeyOutOfRange,
    Unreadable,
    NotJson,
    NotLawJson,
    NotUtf8,
    Sealed,
    EndpointShape,
    EndpointScheme,
    NeedEndpoint,
    OneEndpoint,
    ChainIdNotInt,
    AdoptionShape,
    NotHex,
    Not32,
    Not20,
    RpcNotObject,
    FormWord,
    NeedHashOrCalldata,
    FileShape,
    ProofShape,
    EntryNotId,
    SeqPrevPair,
}

impl Said {
    pub const ALL: [Said; 32] = [
        Said::Usage,
        Said::NotAVerb,
        Said::NotAFlag,
        Said::NeedsValue,
        Said::NotThisVerbsFlag,
        Said::Repeated,
        Said::Missing,
        Said::OneOf,
        Said::NotInt,
        Said::KeyNotHex,
        Said::KeyOutOfRange,
        Said::Unreadable,
        Said::NotJson,
        Said::NotLawJson,
        Said::NotUtf8,
        Said::Sealed,
        Said::EndpointShape,
        Said::EndpointScheme,
        Said::NeedEndpoint,
        Said::OneEndpoint,
        Said::ChainIdNotInt,
        Said::AdoptionShape,
        Said::NotHex,
        Said::Not32,
        Said::Not20,
        Said::RpcNotObject,
        Said::FormWord,
        Said::NeedHashOrCalldata,
        Said::FileShape,
        Said::ProofShape,
        Said::EntryNotId,
        Said::SeqPrevPair,
    ];

    /// The words, one table.
    pub fn text(self) -> &'static str {
        match self {
            Said::Usage => crate::args::USAGE,
            Said::NotAVerb => "不是一个动词",
            Said::NotAFlag => "不是 --名字 的形",
            Said::NeedsValue => "这一旗要一个值",
            Said::NotThisVerbsFlag => "不在这个动词的旗单里",
            Said::Repeated => "给了不止一次",
            Said::Missing => "缺这一旗",
            Said::OneOf => "这几旗之中恰要一面",
            Said::NotInt => "不是一个整数",
            Said::KeyNotHex => "不是六十四位十六进制",
            Said::KeyOutOfRange => "不在曲线的范围里",
            Said::Unreadable => "读不出",
            Said::NotJson => "不是 JSON",
            Said::NotLawJson => "不是法 §3.5 收的 JSON",
            Said::NotUtf8 => "参数不是合法 UTF-8",
            Said::Sealed => crate::ledger::SEALED_SAID,
            Said::EndpointShape => "--endpoint 的形是 <链号>=<url>",
            Said::EndpointScheme => "端点只认 http:// 或 https:// 地址",
            Said::NeedEndpoint => "至少要一个 --endpoint <链号>=<url>",
            Said::OneEndpoint => "anchor 只往一条链上发,只收一个 --endpoint",
            Said::ChainIdNotInt => "链号不是十进制整数",
            Said::AdoptionShape => "导入元素要 {chainId, tx}",
            Said::NotHex => "不是十六进制",
            Said::Not32 => "不是三十二字节",
            Said::Not20 => "不是二十字节",
            Said::RpcNotObject => "录制的 rpc 不是对象",
            Said::FormWord => "--form 只认 registry 或 bare",
            Said::NeedHashOrCalldata => "至少要一个 --hash,或一段 --calldata",
            Said::FileShape => "--file 的形是 <包内路径>=<盘上的路>",
            Said::ProofShape => "--proof 的形是 <包内路径>=<tx>=<盘上的路>",
            Said::EntryNotId => "不是六十四位小写十六进制",
            Said::SeqPrevPair => "--seq 与 --prev 要么都给,要么都不给",
        }
    }
}

/// The line for people when the law refuses an entry's body (`E_BODY_FIELD`): which member, and which flags
/// give it.
pub fn body_line(member: &str, flags: &str) -> String {
    format!("{member}: 缺或不成形,由 {flags} 给")
}

/// Misuse: the subject alone on the first stderr line (`<REASON> <subject>`), what it means for people on the
/// second ([`Said`]), nothing on stdout, exit 2.
pub fn misuse(reason: Reason, subject: &str, said: Said) -> ! {
    eprintln!("{} {}", reason.as_str(), subject);
    eprintln!("{}", said.text());
    std::process::exit(Exit::Misuse.code() as i32)
}
