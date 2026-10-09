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
//! running to the end of the line (spaces included): the subject alone, never a sentence, and never a word the
//! person typed ([`Subject`]: a flag of the closed table by its name, anything else by its place and length).
//! What it means for people is the second line, worded in one table ([`Said`]); later lines are not part of the
//! contract.

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

/// The language of the lines for people (stderr's second line, the line an answer tells): the system's, as the
/// environment's locale says it (the first of `LC_ALL`, `LC_MESSAGES`, `LANG`, `LANGUAGE` that is set and not
/// empty; `LANGUAGE`'s first entry): Chinese when it is `zh…`, English for any other, and English when none
/// is set (it renders on every console). What machines read (stdout, the exit code, the reason, the first
/// stderr line) is the same in both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    En,
}

/// [`Lang`] from any source of the four names (the environment, or a test's table).
pub fn lang_of(var: impl Fn(&str) -> Option<String>) -> Lang {
    let set = ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"].iter().find_map(|k| var(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()));
    match set {
        Some(v) if v.split(':').next().unwrap_or("").to_ascii_lowercase().starts_with("zh") => Lang::Zh,
        _ => Lang::En,
    }
}

/// This process's language for people, read once.
pub fn lang() -> Lang {
    static L: std::sync::OnceLock<Lang> = std::sync::OnceLock::new();
    *L.get_or_init(|| lang_of(|k| std::env::var(k).ok()))
}

/// Of two sentences given in both languages (a refusal the desktop words), the one in this process's language.
pub fn pick(zh: String, en: String) -> String {
    match lang() {
        Lang::Zh => zh,
        Lang::En => en,
    }
}

/// The words for people in this process's language.
pub fn say(zh: &'static str, en: &'static str) -> &'static str {
    match lang() {
        Lang::Zh => zh,
        Lang::En => en,
    }
}

/// What a node address that does not read is told, by why (`zikaron_net::read_address`): its scheme, its
/// port, or the rest of its shape.
pub fn address_said(url: &str) -> Said {
    match zikaron_anchor::rpc::read_address(url) {
        Err(zikaron_anchor::rpc::NotAnAddress::Port) => Said::EndpointPort,
        Err(zikaron_anchor::rpc::NotAnAddress::Scheme) | Ok(_) => Said::EndpointScheme,
        Err(_) => Said::EndpointAddress,
    }
}

/// What a misuse's first stderr line names (`<REASON> <subject>`). Closed, and nothing the person typed can be
/// in it: a name from the command line's own tables (the program, a flag of the closed flag table) is said as
/// it is; any other word (a value, a path, a word where a flag belongs, a flag not in the table, a verb not in
/// the table, an argument that is not text) is named by its place among the arguments (the verb is #1) and its
/// length, so a secret given in any spelling (`--key=…` included) never lands in a terminal or a log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// The program itself (no verb given).
    Program,
    /// Flags of the closed flag table (`verbs::flag_named`), said as `--name`, one or more.
    Flags(Vec<&'static str>),
    /// A word the person typed: its place and length, as `#3 (12 bytes)`.
    Placed { at: usize, bytes: usize },
    /// An `--endpoint` value whose half before the first `=` reads as a chain id: that chain id and the length
    /// after the `=`, as `#3 (1= + 29 bytes)` (a half that is not a chain id may be part of the address).
    Endpoint { at: usize, chain: u64, bytes: usize },
    /// A word that is not among the arguments (read from a file the person named): its length alone.
    Unplaced { bytes: usize },
}

impl Subject {
    /// One flag of the closed table.
    pub fn flag(name: &'static str) -> Subject {
        Subject::Flags(vec![name])
    }

    /// The first line's subject, spelled.
    pub fn spelled(&self) -> String {
        match self {
            Subject::Program => "zikaron".to_string(),
            Subject::Flags(names) => names.iter().map(|n| format!("--{n}")).collect::<Vec<_>>().join(" "),
            Subject::Placed { at, bytes } => format!("#{at} ({bytes} bytes)"),
            Subject::Endpoint { at, chain, bytes } => format!("#{at} ({chain}= + {bytes} bytes)"),
            Subject::Unplaced { bytes } => format!("({bytes} bytes)"),
        }
    }
}

/// This run's arguments, the verb first (kept by `args::Args::read` before anything is judged), so that a word
/// at fault is named by its place.
static ARGUMENTS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Keep this run's arguments (the verb first); a later reading of arguments in the same process replaces them.
pub fn keep_arguments(args: Vec<String>) {
    *ARGUMENTS.lock().unwrap_or_else(|e| e.into_inner()) = args;
}

/// A word the person gave, as a misuse names it: the place of the argument it is or is a `=`-piece of (or, for
/// a path read under a folder the person named, the place of that folder's argument) and that argument's
/// length; a word that is not among the arguments (it came from a file) by its length alone. Never the word.
pub fn typed(word: &str) -> Subject {
    let kept = ARGUMENTS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let args = kept.as_slice();
    // The argument itself, or one piece of it between `=` (`--file <kit path>=<disk path>`, `--hop <g>=<i>`).
    if let Some(i) = args.iter().position(|a| a == word || a.split('=').any(|piece| piece == word)) {
        return Subject::Placed { at: i + 1, bytes: args[i].len() };
    }
    let sep = |b: u8| b == b'/' || b == b'\\';
    let under = |a: &String| {
        !a.is_empty() && word.len() > a.len() && word.starts_with(a.as_str()) && (a.bytes().last().is_some_and(sep) || sep(word.as_bytes()[a.len()]))
    };
    match args.iter().enumerate().filter(|(_, a)| under(a)).max_by_key(|(_, a)| a.len()) {
        Some((i, a)) => Subject::Placed { at: i + 1, bytes: a.len() },
        None => Subject::Unplaced { bytes: word.len() },
    }
}

/// How an `--endpoint` value that does not read is named on the first stderr line: its address is never
/// echoed (it may carry a key), only its place and length. When the half before the first `=` reads as a chain
/// id (`rpc::endpoint_spec`'s reading), that chain id is said with the length after the `=`
/// ([`Subject::Endpoint`]).
pub fn unechoed_endpoint(at: usize, spec: &str) -> Subject {
    use zikaron_anchor::rpc::{endpoint_spec, NotAnEndpoint};
    let chain = match endpoint_spec(spec) {
        Ok((c, _)) => Some(c),
        // Read by the one reader already (else it would be `ChainNotInt`); only the address after it is empty.
        Err(NotAnEndpoint::NoAddress) => spec.split_once('=').and_then(|(c, _)| c.trim().parse::<u64>().ok()),
        Err(_) => None,
    };
    match (chain, spec.split_once('=')) {
        (Some(chain), Some((_, address))) => Subject::Endpoint { at, chain, bytes: address.len() },
        _ => Subject::Placed { at, bytes: spec.len() },
    }
}

/// What a misuse says to people: the second stderr line; and the one line an answer through the desktop
/// (`--home`) tells people ([`Answer::told`]). Closed; every word the command line says on stderr is in this one
/// table, in both languages ([`lang`]; the subject on the first line is the machine's part), except a refusal
/// the desktop words itself (it sends its sentence in both languages, [`pick`]).
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
    ProxyShape,
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
    RegistryOnlyWithRegistryForm,
    NotWithFixture,
    EndpointPort,
    EndpointAddress,
    PastIntCeiling,
    KeyFileOpen,
    NotWithHome,
    DesktopLocked,
    DesktopClosed,
    DoorPathLong,
    DoorNotYou,
    DoorBroken,
    NoMachine,
    OnDesktop,
    Queued,
}

impl Said {
    pub const ALL: [Said; 48] = [
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
        Said::ProxyShape,
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
        Said::RegistryOnlyWithRegistryForm,
        Said::NotWithFixture,
        Said::EndpointPort,
        Said::EndpointAddress,
        Said::PastIntCeiling,
        Said::KeyFileOpen,
        Said::NotWithHome,
        Said::DesktopLocked,
        Said::DesktopClosed,
        Said::DoorPathLong,
        Said::DoorNotYou,
        Said::DoorBroken,
        Said::NoMachine,
        Said::OnDesktop,
        Said::Queued,
    ];

    /// The words, one table: Chinese and English.
    pub fn words(self) -> (&'static str, &'static str) {
        match self {
            Said::Usage => (crate::args::USAGE, crate::args::USAGE),
            Said::NotAVerb => ("不是一个动词", "not a verb"),
            Said::NotAFlag => ("不是 --名字 的形", "not in the form --name"),
            Said::NeedsValue => ("这一旗要一个值", "this flag needs a value"),
            Said::NotThisVerbsFlag => ("不在这个动词的旗单里", "not among this verb's flags"),
            Said::Repeated => ("给了不止一次", "given more than once"),
            Said::Missing => ("缺这一旗", "this flag is missing"),
            Said::OneOf => ("这几旗之中恰要一面", "exactly one of these flags is wanted"),
            Said::NotInt => ("不是一个整数", "not an integer"),
            Said::KeyNotHex => ("不是六十四位十六进制", "not 64 hex digits"),
            Said::KeyOutOfRange => ("不在曲线的范围里", "not within the curve's range"),
            Said::Unreadable => ("读不出", "cannot be read"),
            Said::NotJson => ("不是 JSON", "not JSON"),
            Said::NotLawJson => ("不是法 §3.5 收的 JSON", "not JSON the law takes (§3.5)"),
            Said::NotUtf8 => ("参数不是合法 UTF-8", "an argument is not valid UTF-8"),
            Said::Sealed => ("已锁定:这是 ZIKARON Desk 封存的本机数据,命令行不读", "Locked: this is local data ZIKARON Desk keeps sealed; the command line does not read it"),
            Said::EndpointShape => ("--endpoint 的形是 <链号>=<url>", "--endpoint is <chain id>=<url>"),
            Said::EndpointScheme => ("端点只认 http:// 或 https:// 地址", "an endpoint is an http:// or https:// address"),
            Said::ProxyShape => ("--proxy 只认 system、none、http://主机:端口 或 socks5://主机:端口(不带用户名与口令)", "--proxy takes system, none, http://host:port or socks5://host:port (without a user name or password)"),
            Said::NeedEndpoint => ("至少要一个 --endpoint <链号>=<url>", "at least one --endpoint <chain id>=<url> is wanted"),
            Said::OneEndpoint => ("anchor 只往一条链上发,只收一个 --endpoint", "anchor sends to one chain and takes one --endpoint"),
            Said::ChainIdNotInt => ("链号不是十进制整数", "the chain id is not a decimal integer"),
            Said::AdoptionShape => ("导入元素要 {chainId, tx}", "an adoption is {chainId, tx}"),
            Said::NotHex => ("不是十六进制", "not hex"),
            Said::Not32 => ("不是三十二字节", "not 32 bytes"),
            Said::Not20 => ("不是二十字节", "not 20 bytes"),
            Said::RpcNotObject => ("录制的 rpc 不是对象", "the recording's rpc is not an object"),
            Said::FormWord => ("--form 只认 registry 或 bare", "--form takes registry or bare"),
            Said::NeedHashOrCalldata => ("至少要一个 --hash,或一段 --calldata", "at least one --hash, or a --calldata, is wanted"),
            Said::FileShape => ("--file 的形是 <包内路径>=<盘上的路>", "--file is <path in the bundle>=<path on disk>"),
            Said::ProofShape => ("--proof 的形是 <包内路径>=<tx>=<盘上的路>", "--proof is <path in the bundle>=<tx>=<path on disk>"),
            Said::EntryNotId => ("不是六十四位小写十六进制", "not 64 lowercase hex digits"),
            Said::SeqPrevPair => ("--seq 与 --prev 要么都给,要么都不给", "--seq and --prev go together or not at all"),
            Said::RegistryOnlyWithRegistryForm => ("--registry 只配 --form registry", "--registry goes with --form registry only"),
            Said::NotWithFixture => ("读录制那一路不用这一旗", "a run over recordings does not take this flag"),
            Said::EndpointPort => ("端点地址的端口要是 0 到 65535 之间的整数", "an endpoint's port must be a whole number from 0 to 65535"),
            Said::EndpointAddress => ("不是一处端点地址(主机缺、方括号里不是 IPv6 地址，或带了用户名或空白)", "not an endpoint address (no host, a bracket that is not an IPv6 address, or a user name or white space in it)"),
            Said::PastIntCeiling => ("过了整数上界 9007199254740991", "past the whole-number ceiling 9007199254740991"),
            Said::KeyFileOpen => ("这份密钥档别的账户也读得到(只许本人可读)", "other accounts can read this key file (it must be readable by its owner only)"),
            Said::NotWithHome => ("带 --home 时不收这一旗(桌面那一侧用它自己的身份与账本)", "not taken together with --home (the desktop uses its own identity and ledger)"),
            Said::DesktopLocked => ("桌面锁着或没有打开", "The desktop is locked or not open"),
            Said::DesktopClosed => ("桌面在答之前锁上、关了这处数据或退出了", "The desktop locked, closed this data folder or quit before it answered"),
            Said::DoorPathLong => ("本机数据所在文件夹的路径太长,连不到桌面", "The path of this machine's data folder is too long to reach the desktop"),
            Said::DoorNotYou => ("桌面那一端不是本机这位用户", "The desktop at the other end is another user's"),
            Said::DoorBroken => ("桌面答到一半断了,或答的形这一版读不出", "The desktop broke off, or answered in a form this version cannot read"),
            Said::NoMachine => ("找不到本机数据所在的文件夹", "This machine's data folder cannot be found"),
            Said::OnDesktop => ("此动作须在桌面上做", "This action must be done on the desktop"),
            Said::Queued => ("已在队列,待用户在桌面上发", "Queued; waiting for the user to send it from the desktop"),
        }
    }

    /// The words in this process's language ([`lang`]).
    pub fn text(self) -> &'static str {
        let (zh, en) = self.words();
        match lang() {
            Lang::Zh => zh,
            Lang::En => en,
        }
    }
}

/// The line for people when the law refuses an entry's body (`E_BODY_FIELD`): which member, and which flags
/// give it.
pub fn body_line(member: &str, flags: &str) -> String {
    match lang() {
        Lang::Zh => format!("{member}: 缺或不成形,由 {flags} 给"),
        Lang::En => format!("{member}: missing or malformed; given by {flags}"),
    }
}

/// Misuse: the subject alone on the first stderr line (`<REASON> <subject>`, [`Subject`]), what it means for
/// people on the second ([`Said`]), nothing on stdout, exit 2.
pub fn misuse(reason: Reason, subject: Subject, said: Said) -> ! {
    eprintln!("{} {}", reason.as_str(), subject.spelled());
    eprintln!("{}", said.text());
    std::process::exit(Exit::Misuse.code() as i32)
}
