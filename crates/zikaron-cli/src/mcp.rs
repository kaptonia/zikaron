//! `zikaron mcp`: the Model Context Protocol on standard input and output (CLI-SCHEMA §13), the command line's
//! other way of being asked. It is not a door and holds no right of its own. Each tool call is the verb it names,
//! run as this same program in a process of its own with the arguments laid out as a person would type them; the
//! answer is that process's answer, byte for byte, and its exit code. Which way a write goes (the command line's
//! credential, else the running desktop) is judged at each call as `--home` judges it (`desk::writes_alone`); the
//! master key is opened, if at all, in that one process, never in this one.
//!
//! Messages are read one per line and written one per line through `out::frame`; nothing listens on a port or a
//! local channel, and nothing is written to a file. Tool calls are done one at a time in the order they came;
//! `ping` and `initialize` are answered as they come.
//!
//! The tools are read off the verb table: the write tools are verbs the desktop does under `--home`
//! (`verbs::HOME`), each taking that row's flags and `home`; the read tools are verbs of the table taking their
//! row's flags less the ones withheld (`kit-verify` among them writes when it takes originals out, and its `home`,
//! which may be left out, goes the way a write's goes). A verb not named here is no tool.

use crate::args::Args;
use crate::codes::Reason;
use crate::out::{self, Said, Subject};
use crate::verbs;
use std::io::{BufRead, Read};
use std::sync::{mpsc, Arc, Mutex};
use zikaron::json::{self, Value};

/// The protocol versions spoken; the first is the one answered when the client asks for another.
const VERSIONS: [&str; 2] = ["2025-11-25", "2025-06-18"];

/// The verbs the desktop does under `--home` (`verbs::HOME`) that are no write tool: `init` (the home has its genesis
/// already) and `attest` (it asks for the passcode, on the desktop alone). Every other row of `HOME` is a write tool.
const NOT_WRITES: [&str; 2] = ["init", "attest"];

/// The read tools, each with the flags of its row it does not take.
const READS: [(&str, &[&str]); 9] = [
    ("show", &[]),
    ("audit", &["out"]),
    ("scan", &["endpoint", "fixture", "basis", "adoptions", "proxy"]),
    ("check-grant", &[]),
    ("chain-check", &[]),
    ("depth", &[]),
    ("badge", &["encode"]),
    ("kit-verify", &["ledger", "key"]),
    ("contract", &[]),
];

/// The tools of the read table that write: `kit-verify` takes the originals out into a folder (`originals-to`). Each is
/// marked as writing (not read-only) and, cancelled while it runs, is let finish and answered, as a write is. A verb
/// with a read tool (`kit-verify`) is offered once, as that tool, though the desktop does it under `--home` too: its
/// `home` goes the way a write's goes.
const WRITES_TOO: [&str; 1] = ["kit-verify"];

/// The write tools whose `home` may be left out: `kit-verify` verifies a plain package, and takes its originals out,
/// with no home (a package addressed to one key is opened through a home: the credential's, or the desktop's).
const HOME_OPTIONAL: [&str; 1] = ["kit-verify"];

/// A read tool's flag given as the home: `scan` reads with the home's chain settings and nodes (`--ledger
/// <home>/ledger`).
const AS_HOME: [(&str, &str); 1] = [("scan", "ledger")];

/// The parameter that names the home.
const HOME: &str = "home";

/// The parameters whose values are paths (any tool when the tool is empty); each is taken whole only. A `hop` is
/// one path or two joined by `=`.
const PATHS: [(&str, &str); 13] = [
    ("", "home"),
    ("", "ledger"),
    ("", "input"),
    ("", "fragment"),
    ("history", "file"),
    ("adopt", "anchors"),
    ("kit-export", "out"),
    ("kit-verify", "kit"),
    ("kit-verify", "originals-to"),
    ("show", "path"),
    ("check-grant", "grant"),
    ("chain-check", "hop"),
    ("badge", "decode"),
];

/// One tool: its name (the verb's), whether it writes, and its parameters (flags of the verb's row, in that order).
struct Tool {
    name: &'static str,
    write: bool,
    params: Vec<&'static str>,
}

/// The tools, read off the verb table.
fn tools() -> Vec<Tool> {
    let mut all = Vec::new();
    for name in verbs::HOME.iter().map(|(v, _)| *v).filter(|v| !NOT_WRITES.contains(v) && !READS.iter().any(|(r, _)| r == v)) {
        let params = verbs::accepts(name, true).unwrap_or_default();
        all.push(Tool { name, write: true, params });
    }
    for (name, withheld) in READS {
        let params = verbs::accepts(name, false)
            .unwrap_or_default()
            .into_iter()
            .filter(|f| !withheld.contains(f))
            .map(|f| if AS_HOME.contains(&(name, f)) { HOME } else { f })
            .collect();
        all.push(Tool { name, write: WRITES_TOO.contains(&name), params });
    }
    all
}

/// The flag a tool's parameter is passed as.
fn flag_of(tool: &str, param: &'static str) -> &'static str {
    AS_HOME.iter().find(|(t, _)| *t == tool).filter(|_| param == HOME).map(|(_, f)| *f).unwrap_or(param)
}

fn repeated(tool: &str, param: &str) -> bool {
    verbs::REPEATED.contains(&(tool, param))
}

fn is_path(tool: &str, param: &str) -> bool {
    PATHS.iter().any(|(t, p)| *p == param && (t.is_empty() || *t == tool))
}

/// Each tool's one sentence, a closed constant in the system's language (`out::say`); never anything from a ledger.
fn told(tool: &str) -> &'static str {
    let (zh, en) = match tool {
        "history" => ("为一个文件在本家账本里记一条存证", "Record a file in this home's ledger"),
        "grant" => ("签发一份授权", "Issue a grant"),
        "revoke" => ("撤销一份授权", "Revoke a grant"),
        "adopt" => ("收编已在链上的存证", "Adopt anchors already on chain"),
        "annotate" => ("给一条记录添加注记", "Annotate a record"),
        "retract" => ("删除一条记录(写一条撤回)", "Retract a record (write a retraction)"),
        "kit-export" => ("把记录包导出到给定位置;给了接收方即加密给它", "Export a record kit to the given place; given a recipient, encrypt it to them"),
        "anchor" => (
            "上链:发送本家的待上链队列;自动发送时花这个身份的 gas",
            "Anchor: send this home's queue; with sending automatic it spends this identity's gas",
        ),
        "show" => ("读一条记录", "Show one entry"),
        "audit" => ("核验一本账本", "Audit a ledger"),
        "scan" => ("读链上的存证,用本家的链设置", "Read anchors on chain with this home's chain settings"),
        "check-grant" => ("核验一份授权", "Check a grant"),
        "chain-check" => ("核验一条转授权链", "Check a chain of grants"),
        "depth" => ("读一条记录的上链深度", "Read an entry's anchoring depth"),
        "badge" => ("解读一份授权码", "Decode a grant code"),
        "kit-verify" => (
            "核验一份记录包,加密的经本家打开;给了位置即把原件取到那里",
            "Verify a record kit, an encrypted one opened through this home; given a place, take its originals out there",
        ),
        "contract" => ("读命令行的契约:动词、旗与退出码", "Read the command line's contract: verbs, flags and exit codes"),
        _ => ("", ""),
    };
    out::say(zh, en)
}

fn o(members: Vec<(&str, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn st(x: &str) -> Value {
    Value::Str(x.to_string())
}

/// The tools as `tools/list` gives them: each parameter a string, a repeated one a list of strings; the write
/// tools need `home`; every result is the exit code and the verb's answer.
fn listed() -> Value {
    let text = o(vec![("type", st("string"))]);
    let texts = o(vec![("type", st("array")), ("items", text.clone())]);
    let items = tools()
        .into_iter()
        .map(|t| {
            let props = Value::Obj(t.params.iter().map(|p| (p.to_string(), if repeated(t.name, flag_of(t.name, p)) { texts.clone() } else { text.clone() })).collect());
            let mut input = vec![("type", st("object")), ("properties", props), ("additionalProperties", Value::Bool(false))];
            if t.write && t.params.contains(&HOME) && !HOME_OPTIONAL.contains(&t.name) {
                input.push(("required", Value::Arr(vec![st(HOME)])));
            }
            let output = o(vec![
                ("type", st("object")),
                ("properties", o(vec![("exit", o(vec![("type", st("integer"))])), ("answer", o(vec![]))])),
                ("required", Value::Arr(vec![st("exit"), st("answer")])),
            ]);
            let mut notes = vec![("readOnlyHint", Value::Bool(!t.write))];
            if t.name == "anchor" {
                notes.push(("destructiveHint", Value::Bool(true)));
                notes.push(("openWorldHint", Value::Bool(true)));
            }
            o(vec![
                ("name", st(t.name)),
                ("description", st(told(t.name))),
                ("inputSchema", o(input)),
                ("outputSchema", output),
                ("annotations", o(notes)),
            ])
        })
        .collect();
    o(vec![("tools", Value::Arr(items))])
}

/// A tool call's parameters refused here (no process is started): the two lines a misuse writes, the subject
/// first (`E_ARGS --home`), never a word the client gave as a value.
fn refusal(subject: Subject, said: Said) -> String {
    format!("{} {}\n{}", Reason::Args.as_str(), subject.spelled(), said.text())
}

/// The command a call stands for: the verb, then each parameter given as its flag (a repeated one once per value,
/// in the order given), the parameters in the tool's order. A write goes with the credential when this machine
/// writes alone (`--ledger <home>/ledger`), else through the desktop (`--home <home>`).
fn command(tool: &Tool, given: &[(String, Value)]) -> Result<Vec<String>, String> {
    for (i, (k, _)) in given.iter().enumerate() {
        if !tool.params.iter().any(|p| p == k) {
            return Err(refusal(Subject::Placed { at: i + 1, bytes: k.len() }, Said::McpNotAParameter));
        }
    }
    // Which way a write goes, judged once for the call: with the credential while this machine writes alone, else
    // through the desktop, which takes the flags of the verb's `--home` row and no other (one given is refused here
    // by name, before any process starts).
    let homed = tool.write && given.iter().any(|(k, _)| k == HOME);
    let alone = homed && alone();
    if homed && !alone {
        let through = verbs::accepts(tool.name, true).unwrap_or_default();
        if let Some((k, _)) = given.iter().find(|(k, _)| !through.iter().any(|f| f == k)) {
            let param = tool.params.iter().find(|p| *p == k).copied().unwrap_or(HOME);
            return Err(refusal(Subject::flag(param), Said::McpNotThroughDesktop));
        }
    }
    let mut argv = vec![tool.name.to_string()];
    for param in &tool.params {
        let Some((_, v)) = given.iter().find(|(k, _)| k == param) else {
            if tool.write && *param == HOME && !HOME_OPTIONAL.contains(&tool.name) {
                return Err(refusal(Subject::flag(HOME), Said::Missing));
            }
            continue;
        };
        let flag = flag_of(tool.name, param);
        let values: Vec<&str> = match (repeated(tool.name, flag), v) {
            (false, Value::Str(x)) => vec![x.as_str()],
            (true, Value::Arr(xs)) if xs.iter().all(|x| x.as_str().is_some()) => xs.iter().filter_map(Value::as_str).collect(),
            (false, _) => return Err(refusal(Subject::flag(param), Said::McpNotText)),
            (true, _) => return Err(refusal(Subject::flag(param), Said::McpNotTexts)),
        };
        // A value cannot carry a NUL into an argument (no system passes one): it is not text this takes.
        if values.iter().any(|x| x.contains('\0')) {
            return Err(refusal(Subject::flag(param), if repeated(tool.name, flag) { Said::McpNotTexts } else { Said::McpNotText }));
        }
        // A path whole; a `hop` is one path or two joined by `=`, each whole.
        let whole = |x: &str| std::path::Path::new(x).is_absolute();
        let paths_whole = |x: &str| if flag == "hop" { x.split('=').all(whole) } else { whole(x) };
        if is_path(tool.name, param) && !values.iter().all(|x| paths_whole(x)) {
            return Err(refusal(Subject::flag(param), Said::McpAbsolute));
        }
        for x in values {
            if *param == HOME {
                let ledger = std::path::Path::new(x).join(zikaron_seal::envelope::LEDGER_DIR).display().to_string();
                if !tool.write {
                    argv.extend([format!("--{flag}"), ledger]);
                } else if alone {
                    argv.extend(["--ledger".to_string(), ledger]);
                } else {
                    argv.extend(["--home".to_string(), x.to_string()]);
                }
            } else {
                argv.extend([format!("--{flag}"), x.to_string()]);
            }
        }
    }
    Ok(argv)
}

/// Whether this machine writes alone now: the command line's credential is in its folder (`desk::writes_alone`,
/// the one judgement). No machine folder is the desktop's way, whose answer then says so.
fn alone() -> bool {
    zikaron_os::machine::here().map(|(m, _)| crate::desk::writes_alone(&m)).unwrap_or(false)
}

/// One call in the queue.
struct Call {
    id: Value,
    tool: &'static str,
    args: Vec<(String, Value)>,
}

/// The calls queued and not started (by id), and the one being done.
#[derive(Default)]
struct Shared {
    waiting: Vec<Value>,
    running: Option<Running>,
}

/// The call being done: its id, whether it writes, its process once started, and whether a cancellation stopped it
/// (a read only).
struct Running {
    id: Value,
    write: bool,
    child: Option<std::process::Child>,
    stopped: bool,
}

/// Serve until the input closes: read a message per line, answer, then end (`out::finish`).
pub fn serve(_a: &Args) -> ! {
    let (calls, queue) = mpsc::channel::<Call>();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let worker = {
        let shared = shared.clone();
        std::thread::spawn(move || work(queue, shared))
    };
    for line in std::io::stdin().lock().split(b'\n') {
        let Ok(mut line) = line else { break };
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        take(&line, &calls, &shared);
    }
    // The input closed: the call being done finishes; the ones not started are not done.
    shared.lock().unwrap_or_else(|e| e.into_inner()).waiting.clear();
    drop(calls);
    let _ = worker.join();
    out::finish()
}

fn reply(id: &Value, result: Value) {
    out::frame(&o(vec![("jsonrpc", st("2.0")), ("id", id.clone()), ("result", result)]));
}

/// A JSON-RPC error: `code` is its magnitude (written negative, `out::frame`).
fn fail(id: &Value, code: u64, message: &str, data: Option<Value>) {
    let mut e = vec![("code", Value::Int(code)), ("message", st(message))];
    if let Some(d) = data {
        e.push(("data", d));
    }
    out::frame(&o(vec![("jsonrpc", st("2.0")), ("id", id.clone()), ("error", o(e))]));
}

const PARSE: u64 = 32700;
const REQUEST: u64 = 32600;
const METHOD: u64 = 32601;
const PARAMS: u64 = 32602;

/// One message read: answered at once, queued (a tool call) or taken note of (a notification).
fn take(line: &[u8], calls: &mpsc::Sender<Call>, shared: &Arc<Mutex<Shared>>) {
    use zikaron::tokens::CanonToken as T;
    let v = match json::parse_tests_1_3(line) {
        Ok(v) => v,
        Err(t @ (T::Utf8 | T::Json)) => return fail(&Value::Null, PARSE, "Parse error", Some(o(vec![("token", st(t.as_str()))]))),
        // A number outside the law's integers, a container too deep, a member twice: named by where it stands.
        Err(t) => return fail(&Value::Null, REQUEST, "Invalid request", Some(o(vec![("token", st(t.as_str())), ("at", st(&fault_at(line)))]))),
    };
    let id = v.member("id").cloned();
    if id.as_ref().is_some_and(|x| !matches!(x, Value::Int(_) | Value::Str(_))) {
        return fail(&Value::Null, REQUEST, "Invalid request", Some(o(vec![("at", st("id"))])));
    }
    let method = match (v.member("jsonrpc").and_then(Value::as_str), v.member("method").and_then(Value::as_str)) {
        (Some("2.0"), Some(m)) => m,
        // A response to nothing this asked: no answer.
        (_, None) if v.member("result").is_some() || v.member("error").is_some() => return,
        _ => return fail(id.as_ref().unwrap_or(&Value::Null), REQUEST, "Invalid request", None),
    };
    let params = v.member("params");
    let Some(id) = id else {
        if method == "notifications/cancelled" {
            if let Some(r) = params.and_then(|p| p.member("requestId")) {
                cancel(r, shared);
            }
        }
        return;
    };
    match method {
        "initialize" => {
            let asked = params.and_then(|p| p.member("protocolVersion")).and_then(Value::as_str);
            let version = VERSIONS.iter().find(|x| Some(**x) == asked).unwrap_or(&VERSIONS[0]);
            reply(
                &id,
                o(vec![
                    ("protocolVersion", st(version)),
                    ("capabilities", o(vec![("tools", o(vec![("listChanged", Value::Bool(false))]))])),
                    ("serverInfo", o(vec![("name", st("zikaron")), ("version", st(env!("CARGO_PKG_VERSION")))])),
                ]),
            );
        }
        "ping" => reply(&id, o(vec![])),
        "tools/list" => reply(&id, listed()),
        "tools/call" => {
            let name = params.and_then(|p| p.member("name")).and_then(Value::as_str);
            let Some(tool) = tools().into_iter().find(|t| Some(t.name) == name) else {
                return fail(&id, PARAMS, "Unknown tool", None);
            };
            let args = match params.and_then(|p| p.member("arguments")) {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Obj(ms)) => ms.clone(),
                Some(_) => return fail(&id, PARAMS, "Invalid params", Some(o(vec![("at", st("params.arguments"))]))),
            };
            shared.lock().unwrap_or_else(|e| e.into_inner()).waiting.push(id.clone());
            let _ = calls.send(Call { id, tool: tool.name, args });
        }
        _ => fail(&id, METHOD, "Method not found", None),
    }
}

/// A cancellation: a read tool's process is stopped and nothing is answered; a write's goes on to its end and its
/// answer is given (a send cut halfway cannot be undone); a call not started yet is not done.
fn cancel(id: &Value, shared: &Arc<Mutex<Shared>>) {
    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = s.waiting.iter().position(|x| x == id) {
        s.waiting.remove(i);
        return;
    }
    if let Some(r) = s.running.as_mut().filter(|r| r.id == *id && !r.write) {
        r.stopped = true;
        if let Some(c) = r.child.as_mut() {
            let _ = c.kill();
        }
    }
}

/// Do the queued calls, one at a time, in the order they came.
fn work(queue: mpsc::Receiver<Call>, shared: Arc<Mutex<Shared>>) {
    for call in queue {
        let tool = tools().into_iter().find(|t| t.name == call.tool).expect("a queued tool is in the table");
        {
            // Taken off the queue and made the call being done in one step: a cancellation finds it in one place or
            // the other, never between.
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            let Some(i) = s.waiting.iter().position(|x| *x == call.id) else { continue };
            s.waiting.remove(i);
            s.running = Some(Running { id: call.id.clone(), write: tool.write, child: None, stopped: false });
        }
        let argv = match command(&tool, &call.args) {
            Ok(a) => a,
            Err(text) => {
                shared.lock().unwrap_or_else(|e| e.into_inner()).running = None;
                reply(&call.id, result(text, Some(MISUSE), Value::Null, true));
                continue;
            }
        };
        // Stopped by a cancellation, nothing is answered.
        if let Some(r) = run(&argv, &shared) {
            reply(&call.id, r);
        }
    }
}

/// The exit code of misuse, which a refusal of this server's own is given as.
const MISUSE: u64 = 2;

/// A tool's result: the text, and whether it is an error; with an exit code of the command line's table, the
/// structured content (that code and the answer); without one, none.
fn result(text: String, exit: Option<u64>, answer: Value, error: bool) -> Value {
    let mut ms = vec![("content", Value::Arr(vec![o(vec![("type", st("text")), ("text", Value::Str(text))])])), ("isError", Value::Bool(error))];
    if let Some(code) = exit {
        ms.push(("structuredContent", o(vec![("exit", Value::Int(code)), ("answer", answer)])));
    }
    o(ms)
}

/// Run the verb as this program, its input empty and its environment this one's, without a deadline; the result
/// is its stdout, stderr and exit code as they came. `None` when a cancellation stopped it.
fn run(argv: &[String], shared: &Arc<Mutex<Shared>>) -> Option<Value> {
    let program = std::env::current_exe();
    let spawned = program.and_then(|p| {
        let mut c = std::process::Command::new(p);
        c.args(argv).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        zikaron_os::spawn(&mut c)
    });
    let mut child = match spawned {
        Ok(c) => c,
        Err(_) => {
            shared.lock().unwrap_or_else(|e| e.into_inner()).running = None;
            return Some(result(refusal(Subject::Program, Said::McpNoProgram), None, Value::Null, true));
        }
    };
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let stdout = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let stderr = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    {
        // A cancellation that came before the process was there stops it now.
        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
        let stop = s.running.as_ref().is_some_and(|r| r.stopped);
        if stop {
            let _ = child.kill();
        }
        if let Some(r) = s.running.as_mut() {
            r.child = Some(child);
        }
    }
    let status = loop {
        {
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(Running { child: Some(c), .. }) = s.running.as_mut() {
                if let Ok(Some(status)) = c.try_wait() {
                    break status;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let stopped = shared.lock().unwrap_or_else(|e| e.into_inner()).running.take().is_some_and(|r| r.stopped);
    let (out_bytes, err_bytes) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
    if stopped {
        return None;
    }
    let said = |b: &[u8]| String::from_utf8_lossy(b).trim_end().to_string();
    // An answer is one of the command line's exit codes; misuse writes nothing on stdout, so its two stderr lines
    // are the text, and it is marked an error. Any other end (a crash, a signal) is no answer: an error, its stderr as
    // the text, no structured content.
    let code = status.code().and_then(|c| u8::try_from(c).ok()).filter(|c| crate::codes::Exit::ALL.iter().any(|e| e.code() == *c));
    match code.map(u64::from) {
        Some(MISUSE) => Some(result(said(&err_bytes), Some(MISUSE), Value::Null, true)),
        Some(c) => Some(result(String::from_utf8_lossy(&out_bytes).into_owned(), Some(c), json::parse_tests_1_3(&out_bytes).unwrap_or(Value::Null), false)),
        None => Some(result(said(&err_bytes), None, Value::Null, true)),
    }
}

/// Where in a message the first number outside the law's integers (a sign, a fraction, an exponent, a leading
/// zero, past 2^53 − 1), the first container past the law's depth or the first member given twice stands, as member
/// names and places joined by `.` (`params.arguments.n`); the whole message as `.`.
fn fault_at(b: &[u8]) -> String {
    enum In {
        /// An object: the member being read, and the names read so far.
        Obj(Option<String>, Vec<String>),
        Arr(usize),
    }
    let path = |stack: &[In]| -> String {
        let parts: Vec<String> = stack
            .iter()
            .filter_map(|f| match f {
                In::Obj(k, _) => k.clone(),
                In::Arr(n) => Some(n.to_string()),
            })
            .collect();
        if parts.is_empty() { ".".to_string() } else { parts.join(".") }
    };
    let mut stack: Vec<In> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'{' | b'[' => {
                if stack.len() >= 128 {
                    return path(&stack);
                }
                stack.push(if b[i] == b'{' { In::Obj(None, Vec::new()) } else { In::Arr(0) });
                i += 1;
            }
            b'}' | b']' => {
                stack.pop();
                i += 1;
            }
            b',' => {
                match stack.last_mut() {
                    Some(In::Arr(n)) => *n += 1,
                    Some(In::Obj(k, _)) => *k = None,
                    None => {}
                }
                i += 1;
            }
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                let word = String::from_utf8_lossy(&b[start..i.min(b.len())]).into_owned();
                i += 1;
                let next = b[i.min(b.len())..].iter().find(|c| !c.is_ascii_whitespace());
                if let (Some(In::Obj(k, seen)), Some(b':')) = (stack.last_mut(), next) {
                    // A member given twice stands where it is given the second time.
                    let twice = seen.contains(&word);
                    seen.push(word.clone());
                    *k = Some(word);
                    if twice {
                        return path(&stack);
                    }
                }
            }
            b'-' | b'+' | b'.' | b'0'..=b'9' => {
                let start = i;
                while i < b.len() && matches!(b[i], b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E') {
                    i += 1;
                }
                let n = &b[start..i];
                let whole = n.iter().all(u8::is_ascii_digit) && (n == b"0" || n[0] != b'0');
                let within = std::str::from_utf8(n).ok().and_then(|x| x.parse::<u64>().ok()).is_some_and(|x| x <= json::MAX_INT);
                if !(whole && within) {
                    return path(&stack);
                }
            }
            _ => i += 1,
        }
    }
    ".".to_string()
}
