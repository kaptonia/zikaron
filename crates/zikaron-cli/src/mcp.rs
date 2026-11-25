use crate::args::Args;
use crate::codes::Reason;
use crate::out::{self, Said, Subject};
use crate::verbs;
use std::io::{BufRead, Read};
use std::sync::{mpsc, Arc, Mutex};
use zikaron::json::{self, Value};

const VERSIONS: [&str; 2] = ["2025-11-25", "2025-06-18"];

const HOME: &str = "home";

pub struct Table {
    pub name: &'static str,
    pub not_writes: &'static [&'static str],
    pub reads: &'static [(&'static str, &'static [&'static str])],
    pub writes_too: &'static [&'static str],
    pub home_optional: &'static [&'static str],
    pub as_home: &'static [(&'static str, &'static str)],
    pub paths: &'static [(&'static str, &'static str)],
    pub joined: &'static [&'static str],
    pub sends: &'static [&'static str],
    pub door_reads: &'static [&'static str],
    pub alone_home: (&'static str, &'static str),
    pub alone_more: &'static [&'static str],
    pub told: fn(&str) -> &'static str,
}

struct Tool {
    name: &'static str,
    write: bool,
    through: bool,
    params: Vec<&'static str>,
    needs: Vec<(verbs::Need, Vec<&'static str>)>,
}

fn tools(t: &Table) -> Vec<Tool> {
    let mut all = Vec::new();
    for name in verbs::HOME.iter().map(|(v, _)| *v).filter(|v| !t.not_writes.contains(v) && !t.reads.iter().any(|(r, _)| r == v)) {
        let params = verbs::accepts(name, true).unwrap_or_default();
        let mut needs = needs_of(name, true, &params, &|f| f);
        if t.door_reads.contains(&name) {
            needs.extend(needs_of(name, false, &params, &|f| f).into_iter().filter(|(_, ps)| ps.as_slice() != [HOME]));
        }
        all.push(Tool { name, write: !t.door_reads.contains(&name), through: true, params, needs });
    }
    for &(name, withheld) in t.reads {
        let as_param = |f: &'static str| if t.as_home.contains(&(name, f)) { HOME } else { f };
        let params: Vec<&'static str> =
            verbs::accepts(name, false).unwrap_or_default().into_iter().filter(|f| !withheld.contains(f) && *f != "rest-keys").map(as_param).collect();
        let needs = needs_of(name, false, &params, &as_param);
        let write = t.writes_too.contains(&name);
        all.push(Tool { name, write, through: write, params, needs });
    }
    all
}

fn needs_of(name: &str, homed: bool, params: &[&'static str], as_param: &dyn Fn(&'static str) -> &'static str) -> Vec<(verbs::Need, Vec<&'static str>)> {
    verbs::required(name, homed)
        .iter()
        .map(|n| (*n, n.flags().into_iter().map(as_param).filter(|p| params.contains(p)).collect::<Vec<_>>()))
        .filter(|(_, ps)| !ps.is_empty())
        .collect()
}

fn required_members(tool: &Tool, home_needed: bool) -> Vec<(&'static str, Value)> {
    let mut names: Vec<&str> = Vec::new();
    if home_needed {
        names.push(HOME);
    }
    let mut groups: Vec<(&'static str, Value)> = Vec::new();
    for (need, ps) in &tool.needs {
        match (need, ps.as_slice()) {
            (_, [one]) => names.push(one),
            (verbs::Need::OneOf(_), many) => groups.push(("oneOf", branches(many))),
            (_, many) => groups.push(("anyOf", branches(many))),
        }
    }
    let mut out = Vec::new();
    if !names.is_empty() {
        out.push(("required", Value::Arr(names.into_iter().map(st).collect())));
    }
    if groups.len() > 1 {
        out.push(("allOf", Value::Arr(groups.into_iter().map(|(k, v)| o(vec![(k, v)])).collect())));
    } else {
        out.extend(groups);
    }
    out
}

fn branches(params: &[&'static str]) -> Value {
    Value::Arr(params.iter().map(|p| o(vec![("required", Value::Arr(vec![st(p)]))])).collect())
}

fn flag_of(t: &Table, tool: &str, param: &'static str) -> &'static str {
    t.as_home.iter().find(|(t, _)| *t == tool).filter(|_| param == HOME).map(|(_, f)| *f).unwrap_or(param)
}

fn repeated(tool: &str, param: &str) -> bool {
    verbs::REPEATED.contains(&(tool, param))
}

fn is_path(t: &Table, tool: &str, param: &str) -> bool {
    t.paths.iter().any(|(t, p)| *p == param && (t.is_empty() || *t == tool))
}

fn param_schema(t: &Table, tool: &str, param: &str, shape: &Value) -> Value {
    if !is_path(t, tool, param) {
        return shape.clone();
    }
    let said = if t.joined.contains(&param) {
        out::say("本机上的绝对路径,以 = 相连;给路径,不给内容", "Absolute paths on this machine joined by =; the paths, not their content")
    } else {
        out::say("本机上的绝对路径;给路径,不给内容", "An absolute path on this machine; the path, not its content")
    };
    match shape {
        Value::Obj(m) => Value::Obj(m.iter().cloned().chain(std::iter::once(("description".to_string(), st(said)))).collect()),
        other => other.clone(),
    }
}

fn o(members: Vec<(&str, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn st(x: &str) -> Value {
    Value::Str(x.to_string())
}

pub fn listed(t: &Table) -> Value {
    let text = o(vec![("type", st("string"))]);
    let texts = o(vec![("type", st("array")), ("items", text.clone())]);
    let items = tools(t)
        .into_iter()
        .map(|tool| {
            let props = Value::Obj(tool.params.iter().map(|p| (p.to_string(), param_schema(t, tool.name, p, if repeated(tool.name, flag_of(t, tool.name, p)) { &texts } else { &text }))).collect());
            let mut input = vec![("type", st("object")), ("properties", props), ("additionalProperties", Value::Bool(false))];
            let home_needed = tool.through && tool.params.contains(&HOME) && !t.home_optional.contains(&tool.name);
            input.extend(required_members(&tool, home_needed));
            let output = o(vec![
                ("type", st("object")),
                ("properties", o(vec![("exit", o(vec![("type", st("integer"))])), ("answer", o(vec![]))])),
                ("required", Value::Arr(vec![st("exit"), st("answer")])),
            ]);
            let mut notes = vec![("readOnlyHint", Value::Bool(!tool.write))];
            if t.sends.contains(&tool.name) {
                notes.push(("destructiveHint", Value::Bool(true)));
                notes.push(("openWorldHint", Value::Bool(true)));
            }
            o(vec![
                ("name", st(tool.name)),
                ("description", st((t.told)(tool.name))),
                ("inputSchema", o(input)),
                ("outputSchema", output),
                ("annotations", o(notes)),
            ])
        })
        .collect();
    o(vec![("tools", Value::Arr(items))])
}

fn refusal(subject: Subject, said: Said) -> String {
    format!("{} {}\n{}", Reason::Args.as_str(), subject.spelled(), said.text())
}

fn command(t: &Table, tool: &Tool, given: &[(String, Value)]) -> Result<Vec<String>, String> {
    let homed = tool.through && given.iter().any(|(k, _)| k == HOME);
    argv_with(t, tool, given, homed && alone())
}

pub fn argv_of(t: &Table, tool: &str, given: &[(String, Value)], alone: bool) -> Option<Result<Vec<String>, String>> {
    let tool = tools(t).into_iter().find(|x| x.name == tool)?;
    Some(argv_with(t, &tool, given, alone))
}

fn argv_with(t: &Table, tool: &Tool, given: &[(String, Value)], alone: bool) -> Result<Vec<String>, String> {
    for (i, (k, _)) in given.iter().enumerate() {
        if !tool.params.iter().any(|p| p == k) {
            return Err(refusal(Subject::Placed { at: i + 1, bytes: k.len() }, Said::McpNotAParameter));
        }
    }
    let homed = tool.through && given.iter().any(|(k, _)| k == HOME);
    let writes_entry = verbs::accepts(tool.name, false).is_some_and(|f| verbs::WRITE.iter().all(|w| f.contains(w))) || verbs::KEYED_TOO.contains(&tool.name);
    let keyed = homed && tool.write && writes_entry && KEYED.get().is_some();
    let alone = homed && alone && !keyed;
    if homed && !alone && !keyed {
        let through = verbs::accepts(tool.name, true).unwrap_or_default();
        if let Some((k, _)) = given.iter().find(|(k, _)| !through.iter().any(|f| f == k)) {
            let param = tool.params.iter().find(|p| *p == k).copied().unwrap_or(HOME);
            return Err(refusal(Subject::flag(param), Said::McpNotThroughDesktop));
        }
    }
    let mut argv = vec![tool.name.to_string()];
    let takes = |flag: &str| verbs::accepts(tool.name, false).unwrap_or_default().contains(&flag);
    if let Some(r) = REST_KEYS.get().filter(|_| !(homed && !alone && !keyed) && takes("rest-keys")) {
        argv.extend(["--rest-keys".to_string(), r.clone()]);
    }
    if let (true, Some(k)) = (keyed, KEYED.get()) {
        if takes("key") {
            argv.extend([format!("--{}", crate::args::KEY_FILE), k.key_file.clone()]);
        }
        if takes("endpoint") {
            for e in &k.endpoints {
                argv.extend(["--endpoint".to_string(), e.clone()]);
            }
        }
        if let (true, Some(p)) = (takes("proxy"), k.proxy.as_ref()) {
            argv.extend(["--proxy".to_string(), p.clone()]);
        }
    }
    for param in &tool.params {
        let Some((_, v)) = given.iter().find(|(k, _)| k == param) else {
            if tool.through && *param == HOME && !t.home_optional.contains(&tool.name) {
                return Err(refusal(Subject::flag(HOME), Said::Missing));
            }
            continue;
        };
        let flag = flag_of(t, tool.name, param);
        let values: Vec<&str> = match (repeated(tool.name, flag), v) {
            (false, Value::Str(x)) => vec![x.as_str()],
            (true, Value::Arr(xs)) if xs.iter().all(|x| x.as_str().is_some()) => xs.iter().filter_map(Value::as_str).collect(),
            (false, _) => return Err(refusal(Subject::flag(param), Said::McpNotText)),
            (true, _) => return Err(refusal(Subject::flag(param), Said::McpNotTexts)),
        };
        if values.iter().any(|x| x.contains('\0')) {
            return Err(refusal(Subject::flag(param), if repeated(tool.name, flag) { Said::McpNotTexts } else { Said::McpNotText }));
        }
        let whole = |x: &str| std::path::Path::new(x).is_absolute();
        let paths_whole = |x: &str| if t.joined.contains(param) { x.split('=').all(whole) } else { whole(x) };
        if is_path(t, tool.name, param) && !values.iter().all(|x| paths_whole(x)) {
            return Err(refusal(Subject::flag(param), Said::McpAbsolute));
        }
        for x in values {
            if *param == HOME {
                let ledger = std::path::Path::new(x).join(zikaron_seal::envelope::LEDGER_DIR).display().to_string();
                if !tool.through {
                    argv.extend([format!("--{flag}"), ledger]);
                } else if keyed {
                    argv.extend(["--ledger".to_string(), x.to_string()]);
                } else if alone {
                    let (flag, room) = t.alone_home;
                    let place = if room.is_empty() { x.to_string() } else { std::path::Path::new(x).join(room).display().to_string() };
                    argv.extend([format!("--{flag}"), place]);
                    argv.extend(t.alone_more.iter().map(|m| m.to_string()));
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

fn alone() -> bool {
    zikaron_os::machine::here().map(|(m, _)| crate::desk::writes_alone(&m)).unwrap_or(false)
}

struct Call {
    id: Value,
    tool: &'static str,
    args: Vec<(String, Value)>,
}

#[derive(Default)]
struct Shared {
    waiting: Vec<Value>,
    running: Option<Running>,
}

struct Running {
    id: Value,
    write: bool,
    child: Option<std::process::Child>,
    stopped: bool,
}

static REST_KEYS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

struct Keyed {
    key_file: String,
    endpoints: Vec<String>,
    proxy: Option<String>,
}

static KEYED: std::sync::OnceLock<Keyed> = std::sync::OnceLock::new();

pub fn serve(a: &Args, t: &'static Table) -> ! {
    if let Some(r) = a.one("rest-keys") {
        let _ = REST_KEYS.set(r);
    }
    match a.one(crate::args::KEY_FILE) {
        Some(k) => {
            let _ = KEYED.set(Keyed { key_file: k, endpoints: a.many("endpoint"), proxy: a.one("proxy") });
        }
        None if !a.many("endpoint").is_empty() || a.one("proxy").is_some() => {
            out::misuse(Reason::Args, Subject::flag(crate::args::KEY_FILE), Said::Missing)
        }
        None => {}
    }
    let (calls, queue) = mpsc::channel::<Call>();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let worker = {
        let shared = shared.clone();
        std::thread::spawn(move || work(t, queue, shared))
    };
    for line in std::io::stdin().lock().split(b'\n') {
        let Ok(mut line) = line else { break };
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        take(t, &line, &calls, &shared);
    }
    shared.lock().unwrap_or_else(|e| e.into_inner()).waiting.clear();
    drop(calls);
    let _ = worker.join();
    out::finish()
}

fn reply(id: &Value, result: Value) {
    out::frame(&o(vec![("jsonrpc", st("2.0")), ("id", id.clone()), ("result", result)]));
}

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

fn take(t: &Table, line: &[u8], calls: &mpsc::Sender<Call>, shared: &Arc<Mutex<Shared>>) {
    use zikaron::tokens::CanonToken as T;
    let v = match json::parse_tests_1_3(line) {
        Ok(v) => v,
        Err(t @ (T::Utf8 | T::Json)) => return fail(&Value::Null, PARSE, "Parse error", Some(o(vec![("token", st(t.as_str()))]))),
        Err(t) => return fail(&Value::Null, REQUEST, "Invalid request", Some(o(vec![("token", st(t.as_str())), ("at", st(&fault_at(line)))]))),
    };
    let id = v.member("id").cloned();
    if id.as_ref().is_some_and(|x| !matches!(x, Value::Int(_) | Value::Str(_))) {
        return fail(&Value::Null, REQUEST, "Invalid request", Some(o(vec![("at", st("id"))])));
    }
    let method = match (v.member("jsonrpc").and_then(Value::as_str), v.member("method").and_then(Value::as_str)) {
        (Some("2.0"), Some(m)) => m,
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
                    ("serverInfo", o(vec![("name", st(t.name)), ("version", st(env!("CARGO_PKG_VERSION")))])),
                ]),
            );
        }
        "ping" => reply(&id, o(vec![])),
        "tools/list" => reply(&id, listed(t)),
        "tools/call" => {
            let name = params.and_then(|p| p.member("name")).and_then(Value::as_str);
            let Some(tool) = tools(t).into_iter().find(|x| Some(x.name) == name) else {
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

fn work(t: &Table, queue: mpsc::Receiver<Call>, shared: Arc<Mutex<Shared>>) {
    for call in queue {
        let tool = tools(t).into_iter().find(|x| x.name == call.tool).expect("a queued tool is in the table");
        {
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            let Some(i) = s.waiting.iter().position(|x| *x == call.id) else { continue };
            s.waiting.remove(i);
            s.running = Some(Running { id: call.id.clone(), write: tool.write, child: None, stopped: false });
        }
        let argv = match command(t, &tool, &call.args) {
            Ok(a) => a,
            Err(text) => {
                shared.lock().unwrap_or_else(|e| e.into_inner()).running = None;
                reply(&call.id, result(text, Some(MISUSE), Value::Null, true));
                continue;
            }
        };
        if let Some(r) = run(&argv, &shared) {
            reply(&call.id, r);
        }
    }
}

const MISUSE: u64 = 2;

fn result(text: String, exit: Option<u64>, answer: Value, error: bool) -> Value {
    let mut ms = vec![("content", Value::Arr(vec![o(vec![("type", st("text")), ("text", Value::Str(text))])])), ("isError", Value::Bool(error))];
    if let Some(code) = exit {
        ms.push(("structuredContent", o(vec![("exit", Value::Int(code)), ("answer", answer)])));
    }
    o(ms)
}

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
    let code = status.code().and_then(|c| u8::try_from(c).ok()).filter(|c| crate::codes::Exit::ALL.iter().any(|e| e.code() == *c));
    match code.map(u64::from) {
        Some(MISUSE) => Some(result(said(&err_bytes), Some(MISUSE), Value::Null, true)),
        Some(c) => Some(result(String::from_utf8_lossy(&out_bytes).into_owned(), Some(c), json::parse_tests_1_3(&out_bytes).unwrap_or(Value::Null), false)),
        None => Some(result(said(&err_bytes), None, Value::Null, true)),
    }
}

pub fn fault_at(b: &[u8]) -> String {
    enum In {
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
