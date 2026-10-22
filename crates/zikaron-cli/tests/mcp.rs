//! `zikaron mcp`: the real program spoken to as an AI app speaks to it, one message per line on its input, one
//! answer per line on its output. The handshake and its versions; the tools, read off the verb table, and none
//! that is withheld; each tool call answered by the verb's own answer, byte for byte as the same command typed in a
//! terminal, with its exit code; a write going the way the command line goes at that moment (through a stand-in
//! desktop, or with the credential), and both ways closed answered as the terminal answers; misuse, a relative
//! path and a number outside the law's integers refused by name; a cancelled read stopped and unanswered, a
//! cancelled write done and answered; the desktop's CLOSED said as it is. Unix only, as the stand-in desktop
//! listens on a local socket (`tests/desk.rs`).
#![cfg(unix)]

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use zikaron::json::Value;
use zikaron_glue::door::{Reply, Request};

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const EN: &str = "en_US.UTF-8";
const ZH: &str = "zh_CN.UTF-8";

/// A short private root: a user home with the pointer to a machine folder, and a data folder (as `tests/desk.rs`).
struct Bench {
    root: PathBuf,
    user: PathBuf,
    machine: PathBuf,
    home: PathBuf,
}

impl Bench {
    fn new(tag: &str) -> Bench {
        let base = if std::env::temp_dir().as_os_str().len() <= 40 { std::env::temp_dir() } else { PathBuf::from("/tmp") };
        let root = base.join(format!("zkm-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (user, machine, home) = (root.join("u"), root.join("m"), root.join("h"));
        for d in [&user, &machine, &home] {
            std::fs::create_dir_all(d).expect("dir");
        }
        let machine = std::fs::canonicalize(&machine).expect("machine");
        let home = std::fs::canonicalize(&home).expect("home");
        std::fs::write(user.join(zikaron_os::machine::POINTER), format!("{}\n", machine.display())).expect("pointer");
        Bench { root, user, machine, home }
    }

    fn command(&self, args: &[&str], lang: &str) -> Command {
        let mut c = Command::new(BIN);
        c.args(args).env(zikaron_os::HOME_VAR, &self.user).env_remove("LC_ALL").env_remove("LC_MESSAGES").env_remove("LANGUAGE").env("LANG", lang);
        c
    }

    /// The same command typed in a terminal: exit code, stdout, stderr.
    fn typed(&self, args: &[&str], lang: &str) -> (i32, Vec<u8>, String) {
        let out = self.command(args, lang).stdin(Stdio::null()).output().expect("runs");
        (out.status.code().unwrap_or(-1), out.stdout, String::from_utf8_lossy(&out.stderr).into_owned())
    }

    fn session(&self, lang: &str) -> Session {
        let mut child = self.command(&["mcp"], lang).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("mcp starts");
        let out = child.stdout.take().expect("stdout");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Session { child, lines: rx }
    }

    fn door(&self) -> PathBuf {
        zikaron_glue::door::place(&self.machine, &self.home)
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A running `zikaron mcp` and the lines it wrote.
struct Session {
    child: std::process::Child,
    lines: mpsc::Receiver<String>,
}

impl Session {
    fn send(&mut self, line: &str) {
        let input = self.child.stdin.as_mut().expect("stdin");
        input.write_all(line.as_bytes()).expect("writes");
        input.write_all(b"\n").expect("writes");
        input.flush().expect("flush");
    }

    /// The next line written, within `secs`, as the raw line and its value.
    fn next(&self, secs: u64) -> Option<(String, Value)> {
        let line = self.lines.recv_timeout(Duration::from_secs(secs)).ok()?;
        let v = zikaron::json::parse_tests_1_3(line.as_bytes()).ok().or_else(|| zikaron::json::parse_tests_1_3(line.replacen("\"code\":-", "\"code\":", 1).as_bytes()).ok())?;
        Some((line, v))
    }

    fn ask(&mut self, line: &str) -> Value {
        self.send(line);
        self.next(60).expect("an answer").1
    }

    fn call(&mut self, id: u64, tool: &str, args: &str) -> Value {
        self.ask(&format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#))
    }

    fn end(mut self) -> i32 {
        drop(self.child.stdin.take());
        self.child.wait().expect("ends").code().unwrap_or(-1)
    }
}

fn result(v: &Value) -> &Value {
    v.member("result").unwrap_or_else(|| panic!("no result: {v:?}"))
}

fn text_of(r: &Value) -> String {
    r.member("content").and_then(|c| c.as_arr()).and_then(|c| c.first()).and_then(|c| c.member("text")).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn exit_of(r: &Value) -> Option<u64> {
    r.member("structuredContent").and_then(|s| s.member("exit")).and_then(Value::as_int)
}

fn is_error(r: &Value) -> bool {
    r.member("isError") == Some(&Value::Bool(true))
}

/// A stand-in desktop answering each request with `answer`, reporting what came (as `tests/desk.rs`).
fn desktop(at: &Path, answer: impl Fn(&Request) -> Option<Vec<u8>> + Send + 'static) -> (zikaron_os::door::Closer, mpsc::Receiver<Request>) {
    let (l, closer) = zikaron_os::door::listen(at).expect("the door opens");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(Some(mut s)) = l.accept() {
            let Ok(bytes) = zikaron_glue::door::take(&mut s) else { continue };
            let Some(q) = Request::of_bytes(&bytes) else { continue };
            if let Some(r) = answer(&q) {
                let _ = zikaron_glue::door::put(&mut s, &r);
            }
            let _ = tx.send(q);
        }
    });
    (closer, rx)
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#;

/// The versions: either of the two asked is answered as asked, any other with the newer; the tools capability
/// alone, and the program's name and version.
#[test]
fn the_handshake_answers_one_of_two_versions() {
    let b = Bench::new("hello");
    for (asked, answered) in [("2025-11-25", "2025-11-25"), ("2025-06-18", "2025-06-18"), ("2024-11-05", "2025-11-25")] {
        let mut s = b.session(EN);
        let v = s.ask(&INIT.replace("2025-06-18", asked));
        let r = result(&v);
        assert_eq!(r.member("protocolVersion").and_then(Value::as_str), Some(answered));
        assert_eq!(zikaron::json::canon_bytes(r.member("capabilities").unwrap()), br#"{"tools":{"listChanged":false}}"#.to_vec());
        assert_eq!(r.member("serverInfo").and_then(|x| x.member("name")).and_then(Value::as_str), Some("zikaron"));
        // A notification is not answered; `ping` is.
        s.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        let pong = s.ask(r#"{"jsonrpc":"2.0","id":"p","method":"ping"}"#);
        assert_eq!(pong.member("id").and_then(Value::as_str), Some("p"), "the next line answers the ping: {pong:?}");
        assert_eq!(s.end(), 0, "the input closed, it ends");
    }
}

/// The tools are the verb table's: the eight writes with their `--home` row and `home`, the nine reads with their
/// row less what is withheld; nothing else, and not a withheld verb.
#[test]
fn the_tools_are_the_verb_tables_and_no_withheld_one_appears() {
    let b = Bench::new("list");
    let mut s = b.session(EN);
    let v = s.ask(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#);
    let tools = result(&v).member("tools").and_then(Value::as_arr).expect("tools").clone();
    // Members are in canonical order: the parameters are compared as sets.
    let got: Vec<(String, Vec<String>, bool)> = tools
        .iter()
        .map(|t| {
            let name = t.member("name").and_then(Value::as_str).unwrap_or_default().to_string();
            let Some(Value::Obj(props)) = t.member("inputSchema").and_then(|i| i.member("properties")) else { panic!("properties of {name}") };
            let ro = t.member("annotations").and_then(|a| a.member("readOnlyHint")) == Some(&Value::Bool(true));
            let mut ps: Vec<String> = props.iter().map(|(k, _)| k.clone()).collect();
            ps.sort();
            (name, ps, ro)
        })
        .collect();
    let want: Vec<(&str, &[&str], bool)> = vec![
        ("history", &["home", "file", "note"], false),
        ("grant", &["home", "grantee", "work", "terms", "history", "window-from", "window-to", "scope", "upstream"], false),
        ("revoke", &["home", "grant", "case"], false),
        ("adopt", &["home", "anchors", "attestor", "attestation"], false),
        ("annotate", &["home", "subject", "note"], false),
        ("retract", &["home", "subject", "note"], false),
        ("anchor", &["home"], false),
        // A package sealed to one recipient: an address or a key.
        ("kit-export", &["home", "entry", "note", "out", "as", "to", "to-key", "originals"], false),
        ("show", &["ledger", "entry", "path"], true),
        ("audit", &["ledger", "root", "input", "fragment", "unavailable"], true),
        ("scan", &["home"], true),
        ("check-grant", &["ledger", "root", "input", "fragment", "unavailable", "now", "grant"], true),
        ("chain-check", &["ledger", "root", "hop", "now", "input", "fragment", "unavailable"], true),
        ("depth", &["ledger", "root", "work", "input", "fragment", "unavailable"], true),
        ("badge", &["decode"], true),
        // `kit-verify` takes the originals out when given a place: a write; a sealed package opens through the home,
        // which may be left out.
        ("kit-verify", &["kit", "fragment", "unavailable", "originals-to", "home"], false),
        ("contract", &[], true),
    ];
    let want: Vec<(String, Vec<String>, bool)> = want
        .into_iter()
        .map(|(n, p, ro)| {
            let mut ps: Vec<String> = p.iter().map(|x| x.to_string()).collect();
            ps.sort();
            (n.to_string(), ps, ro)
        })
        .collect();
    assert_eq!(got, want);
    // Each one is a verb of the table; a verb not in the list is no tool.
    for (name, _, _) in &got {
        assert!(zikaron_cli::verbs::VERBS.contains(&name.as_str()), "{name} is a verb");
    }
    for withheld in ["keygen", "init", "attest", "succeed", "fpm-sign", "ack-sign", "hand-over", "take-over", "mcp"] {
        assert!(!got.iter().any(|(n, _, _)| n == withheld), "{withheld} is withheld");
        let e = s.call(9, withheld, "{}");
        assert!(e.member("error").is_some(), "{withheld}: {e:?}");
    }
    // `anchor` spends gas when sending is automatic: destructive and reaching outside.
    let anchor = tools.iter().find(|t| t.member("name").and_then(Value::as_str) == Some("anchor")).expect("anchor");
    let notes = anchor.member("annotations").expect("annotations");
    assert_eq!((notes.member("destructiveHint"), notes.member("openWorldHint")), (Some(&Value::Bool(true)), Some(&Value::Bool(true))));
    // The write tools need `home`, but `kit-verify`, which verifies a plain package with none.
    let required = |name: &str| tools.iter().find(|t| t.member("name").and_then(Value::as_str) == Some(name)).and_then(|t| t.member("inputSchema")).and_then(|i| i.member("required")).cloned();
    assert_eq!(required("kit-export"), Some(Value::Arr(vec![Value::Str("home".into())])));
    assert_eq!(required("kit-verify"), None);
    // A repeated flag is a list of texts.
    let kit = tools.iter().find(|t| t.member("name").and_then(Value::as_str) == Some("kit-export")).expect("kit-export");
    let entry = kit.member("inputSchema").and_then(|i| i.member("properties")).and_then(|p| p.member("entry")).expect("entry");
    assert_eq!(entry.member("type").and_then(Value::as_str), Some("array"));
}

/// Through the desktop (no credential on this machine): the call is the verb with `--home`, its answer the one
/// the terminal gets, byte for byte, with its exit code; the desktop is asked what the terminal would ask it.
#[test]
fn a_write_through_the_desktop_answers_as_the_terminal() {
    let b = Bench::new("door");
    let home = b.home.display().to_string();
    let wrote = Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 3 }.to_bytes();
    let (closer, rx) = desktop(&b.door(), move |_| Some(wrote.clone()));
    let (code, out, _) = b.typed(&["annotate", "--home", &home, "--subject", "s", "--note", "注记"], EN);
    let typed = rx.recv_timeout(Duration::from_secs(30)).expect("asked by the terminal");
    let mut s = b.session(EN);
    let r = s.call(1, "annotate", &format!(r#"{{"home":"{home}","subject":"s","note":"注记"}}"#));
    let asked = rx.recv_timeout(Duration::from_secs(30)).expect("asked through mcp");
    closer.close();
    let r = result(&r);
    assert_eq!(text_of(r).as_bytes(), out.as_slice(), "byte for byte");
    assert_eq!(exit_of(r), Some(code as u64));
    assert!(!is_error(r));
    assert_eq!((asked.verb, asked.args), (typed.verb, typed.args), "the desktop is asked the same");
    s.end();
}

/// With the credential on this machine the write goes `--ledger <home>/ledger`, judged at the call; a credential
/// that expired is answered as the terminal answers that command (same code, same sentence). Both ways closed is
/// answered as the terminal answers `--home` with no desktop.
#[test]
fn the_way_is_judged_at_each_call_and_answered_as_the_terminal() {
    let b = Bench::new("ways");
    let home = b.home.display().to_string();
    let mut s = b.session(ZH);
    // Neither: no credential, no desktop.
    let (code, out, _) = b.typed(&["retract", "--home", &home, "--subject", "s"], ZH);
    let r = s.call(1, "retract", &format!(r#"{{"home":"{home}","subject":"s"}}"#));
    let r = result(&r);
    assert_eq!((text_of(r).into_bytes(), exit_of(r)), (out, Some(code as u64)), "both closed: as typed");
    assert_eq!(code, 4);
    // The credential appears (expired), the same session now goes with it. An expired credential is removed by
    // the command that finds it, so it is issued again before the same command is typed.
    let issue = || zikaron_seal::credential::issue(&b.machine, "acct", &[7u8; 32], zikaron_seal::credential::Term { issued: 10, expires: Some(70) }).expect("issued");
    issue();
    std::fs::create_dir_all(zikaron_seal::envelope::label_path(&b.home).parent().unwrap()).expect("settings");
    std::fs::write(zikaron_seal::envelope::label_path(&b.home), b"{}").expect("a label: a home kept sealed");
    let r = s.call(2, "retract", &format!(r#"{{"home":"{home}","subject":"s"}}"#));
    let r = result(&r);
    issue();
    let ledger = b.home.join("ledger").display().to_string();
    let (code, out, _) = b.typed(&["retract", "--ledger", &ledger, "--subject", "s"], ZH);
    assert_eq!((text_of(r).into_bytes(), exit_of(r)), (out.clone(), Some(code as u64)), "with the credential: as typed");
    assert_ne!(code, 2, "an answer, not misuse: {}", String::from_utf8_lossy(&out));
    assert!(!is_error(r));
    s.end();
}

/// Misuse is the verb's (exit 2, its two stderr lines as the text, marked an error); a relative path, a parameter
/// not the tool's and a value of the wrong kind are refused before anything runs; a number outside the law's
/// integers is refused by where it stands.
#[test]
fn misuse_and_what_is_refused_by_name() {
    let b = Bench::new("misuse");
    let (closer, rx) = desktop(&b.door(), |_| Some(Reply::Closed.to_bytes()));
    let home = b.home.display().to_string();
    let mut s = b.session(EN);
    let r = s.call(1, "kit-export", &format!(r#"{{"home":"{home}"}}"#));
    let r = result(&r);
    let (_, _, err) = b.typed(&["kit-export", "--home", &home], EN);
    assert!(is_error(r));
    assert_eq!((exit_of(r), text_of(r)), (Some(2), err.trim_end().to_string()));
    for (args, first) in [
        (r#"{"home":"rel/h","subject":"s"}"#.to_string(), "E_ARGS --home"),
        (format!(r#"{{"home":"{home}","nope":"x"}}"#), "E_ARGS #2 (4 bytes)"),
        (format!(r#"{{"home":"{home}","subject":["s"]}}"#), "E_ARGS --subject"),
        (r#"{"subject":"s"}"#.to_string(), "E_ARGS --home"),
    ] {
        let r = s.call(2, "annotate", &args);
        let r = result(&r);
        assert!(is_error(r), "{args}");
        assert_eq!(text_of(r).lines().next(), Some(first), "{args}");
    }
    // The place originals are taken out to is a path, taken whole only: nothing runs for a relative one.
    let r = s.call(5, "kit-verify", &format!(r#"{{"kit":"{home}","originals-to":"rel/out"}}"#));
    let r = result(&r);
    assert!(is_error(r));
    assert_eq!(text_of(r).lines().next(), Some("E_ARGS --originals-to"));
    // Through the desktop (no credential here), `kit-verify` takes the flags of its `--home` row only: a chain reading
    // given beside `home` is refused by name before anything runs.
    let r = s.call(6, "kit-verify", &format!(r#"{{"home":"{home}","kit":"{home}","fragment":"{home}"}}"#));
    let r = result(&r);
    assert!(is_error(r));
    assert_eq!(text_of(r).lines().next(), Some("E_ARGS --fragment"));
    // Both ways to name a recipient: the verb's misuse, judged before the desktop is asked.
    let r = s.call(7, "kit-export", &format!(r#"{{"home":"{home}","out":"{home}/o.zkit","to":"0x{}","to-key":"0x02{}"}}"#, "11".repeat(20), "11".repeat(32)));
    let r = result(&r);
    assert!(is_error(r));
    assert_eq!((exit_of(r), text_of(r).lines().next()), (Some(2), Some("E_ARGS --to --to-key")));
    for (n, at) in [("-1", "params.arguments.n"), ("1.5", "params.arguments.n"), ("9007199254740992", "params.arguments.n")] {
        s.send(&format!(r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"annotate","arguments":{{"n":{n}}}}}}}"#));
        let (line, v) = s.next(30).expect("answered");
        assert!(line.contains("\"code\":-32600"), "{line}");
        assert_eq!(v.member("error").and_then(|e| e.member("data")).and_then(|d| d.member("at")).and_then(Value::as_str), Some(at), "{n}");
    }
    let (line, _) = {
        s.send(r#"{"jsonrpc":"2.0","id":4,"method":"resources/list"}"#);
        s.next(30).expect("answered")
    };
    assert!(line.contains("\"code\":-32601"), "{line}");
    s.end();
    closer.close();
    let asked: Vec<Request> = rx.try_iter().collect();
    assert_eq!(asked.len(), 0, "misuse and mcp's own refusals ask no desktop");
}

/// The desktop's CLOSED is said as it is (nothing retried); a write cancelled while it runs is done and answered;
/// a read cancelled while it runs is stopped and not answered, and the session goes on.
#[test]
fn closed_and_cancellations_are_said_as_they_are() {
    let b = Bench::new("cancel");
    let home = b.home.display().to_string();
    let (closer, rx) = desktop(&b.door(), |q| {
        if q.verb == "history" {
            std::thread::sleep(Duration::from_millis(800));
            return Some(Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 1 }.to_bytes());
        }
        Some(Reply::Closed.to_bytes())
    });
    let mut s = b.session(EN);
    let r = s.call(1, "annotate", &format!(r#"{{"home":"{home}","subject":"s"}}"#));
    let r = result(&r);
    assert!(!is_error(r));
    assert_eq!(exit_of(r), Some(4));
    assert!(text_of(r).contains("CLOSED"), "{}", text_of(r));
    let file = b.root.join("f.txt");
    std::fs::write(&file, b"x").expect("file");
    s.send(&format!(r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"history","arguments":{{"home":"{home}","file":"{}"}}}}}}"#, file.display()));
    std::thread::sleep(Duration::from_millis(200));
    s.send(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2}}"#);
    let (_, v) = s.next(30).expect("the write is answered");
    assert_eq!(v.member("id"), Some(&Value::Int(2)));
    assert_eq!(exit_of(result(&v)), Some(0));
    // A read that waits for ever (a named pipe no one writes): stopped, nothing answered, the next call goes on.
    let pipe = b.root.join("pipe");
    assert!(Command::new("mkfifo").arg(&pipe).status().expect("mkfifo").success());
    s.send(&format!(r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"show","arguments":{{"path":"{}"}}}}}}"#, pipe.display()));
    std::thread::sleep(Duration::from_millis(500));
    s.send(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":3}}"#);
    let after = s.call(4, "contract", "{}");
    assert_eq!(after.member("id"), Some(&Value::Int(4)), "the cancelled read is not answered: {after:?}");
    s.end();
    closer.close();
    let _ = rx.try_iter().count();
}

/// Each read tool answers as the same command typed in a terminal, byte for byte, with its exit code.
#[test]
fn each_read_tool_answers_as_the_terminal() {
    let b = Bench::new("reads");
    let none = b.root.join("none.json").display().to_string();
    let ledger = b.home.join("ledger").display().to_string();
    let cases: Vec<(&str, String, Vec<String>)> = vec![
        ("contract", "{}".into(), vec![]),
        ("show", format!(r#"{{"path":"{none}"}}"#), vec!["--path".into(), none.clone()]),
        ("audit", format!(r#"{{"ledger":"{ledger}"}}"#), vec!["--ledger".into(), ledger.clone()]),
        ("check-grant", format!(r#"{{"grant":"{none}"}}"#), vec!["--grant".into(), none.clone()]),
        ("chain-check", "{}".into(), vec![]),
        ("depth", format!(r#"{{"ledger":"{ledger}"}}"#), vec!["--ledger".into(), ledger.clone()]),
        ("badge", format!(r#"{{"decode":"{none}"}}"#), vec!["--decode".into(), none.clone()]),
        ("kit-verify", format!(r#"{{"kit":"{none}"}}"#), vec!["--kit".into(), none.clone()]),
    ];
    let mut s = b.session(EN);
    for (i, (tool, args, flags)) in cases.into_iter().enumerate() {
        let mut typed: Vec<&str> = vec![tool];
        typed.extend(flags.iter().map(String::as_str));
        let (code, out, err) = b.typed(&typed, EN);
        let r = s.call(i as u64 + 1, tool, &args);
        let r = result(&r);
        let text = if code == 2 { err.trim_end().to_string() } else { String::from_utf8_lossy(&out).into_owned() };
        assert_eq!((text_of(r), exit_of(r)), (text, Some(code as u64)), "{tool}");
        assert_eq!(is_error(r), code == 2, "{tool}");
    }
    s.end();
}

/// `mcp` takes no flag: given one it is misuse, zero bytes on stdout.
#[test]
fn mcp_given_a_flag_is_misuse() {
    let b = Bench::new("flag");
    let (code, out, err) = b.typed(&["mcp", "--home", "/x"], EN);
    assert_eq!((code, out.len()), (2, 0), "{err}");
}

/// The `mcp` module listens on nothing, writes no file and opens no key: it reaches the sealed layer only through
/// the one judgement of whether a credential is there.
#[test]
fn the_module_listens_on_nothing_writes_nothing_and_opens_no_key() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/mcp.rs")).expect("source");
    let code: String = src.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    for w in ["Listener", "bind(", "fs::write", "File::create", "OpenOptions", "keybox", "credential::open", "sealed::", "envelope::open", "read_label", "stdout()", "println!", "print!"] {
        assert!(!code.contains(w), "mcp.rs has {w}");
    }
    assert!(code.contains("crate::desk::writes_alone"), "the way is judged by the one judgement");
}

/// What a message is refused for, by where it stands: not JSON is `-32700`; a container past the law's depth, a
/// member given twice and a number with a sign are `-32600` with their place; an unknown tool is `-32602`; a response
/// sent to the server is not answered.
#[test]
fn messages_are_refused_by_what_and_where() {
    let b = Bench::new("refused");
    let mut s = b.session(EN);
    let line = |s: &mut Session, m: &str| {
        s.send(m);
        s.next(30).expect("answered").0
    };
    assert!(line(&mut s, "{not json").contains("\"code\":-32700"));
    let deep = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{}{}}}"#, "[".repeat(130), "]".repeat(130));
    let l = line(&mut s, &deep);
    assert!(l.contains("\"code\":-32600") && l.contains("E_DEPTH"), "{l}");
    let l = line(&mut s, r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"show","arguments":{"path":"/a","path":"/b"}}}"#);
    assert!(l.contains("\"at\":\"params.arguments.path\""), "{l}");
    for n in ["+1", ".5", "-1"] {
        let l = line(&mut s, &format!(r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"show","arguments":{{"n":{n}}}}}}}"#));
        assert!(l.contains("\"code\":-32600") && l.contains("\"at\":\"params.arguments.n\""), "{n}: {l}");
    }
    let l = line(&mut s, r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"keygen"}}"#);
    assert!(l.contains("\"code\":-32602"), "{l}");
    // A response is not answered: the next line answers the ping after it.
    s.send(r#"{"jsonrpc":"2.0","id":99,"result":{}}"#);
    let l = line(&mut s, r#"{"jsonrpc":"2.0","id":"p","method":"ping"}"#);
    assert!(l.contains("\"id\":\"p\""), "{l}");
    assert_eq!(s.end(), 0);
}

/// Parameters as the table says: a `hop`'s two sides each whole; a repeated flag given once per value in the
/// order given; a value carrying NUL refused before anything runs.
#[test]
fn parameters_are_laid_out_as_the_table_says() {
    let b = Bench::new("params");
    let home = b.home.display().to_string();
    let (closer, rx) = desktop(&b.door(), |_| Some(Reply::Kit { kit_id: "k".into(), path: "/o".into(), entries: 2, files: 0, dropped: vec![], originals: vec![], left_out: vec![] }.to_bytes()));
    let mut s = b.session(EN);
    let r = s.call(1, "chain-check", r#"{"hop":["/a.grant=rel/input.json"]}"#);
    assert_eq!(text_of(result(&r)).lines().next(), Some("E_ARGS --hop"));
    let (one, two) = ("ab".repeat(32), "cd".repeat(32));
    let r = s.call(2, "kit-export", &format!(r#"{{"home":"{home}","out":"/o","entry":["{one}","{two}"]}}"#));
    assert_eq!(exit_of(result(&r)), Some(0), "{r:?}");
    let q = rx.recv_timeout(Duration::from_secs(30)).expect("asked");
    let entries: Vec<&str> = q.args.iter().filter(|(k, _)| k == "entry").map(|(_, v)| v.as_str()).collect();
    assert_eq!(entries, vec![format!("0x{one}").as_str(), format!("0x{two}").as_str()], "once per value, in order");
    let r = s.call(3, "annotate", &format!(r#"{{"home":"{home}","note":"a\u0000b"}}"#));
    assert_eq!(text_of(result(&r)).lines().next(), Some("E_ARGS --note"));
    // A recipient's key goes to the desktop as given; `kit-verify` with `home` goes through it, its flags whole.
    let g = "0x0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    let r = s.call(4, "kit-export", &format!(r#"{{"home":"{home}","out":"/o2","to-key":"{g}"}}"#));
    assert_eq!(exit_of(result(&r)), Some(0), "{r:?}");
    let q = rx.recv_timeout(Duration::from_secs(30)).expect("asked");
    assert!(q.args.iter().any(|(k, v)| k == "to-key" && v == g), "{:?}", q.args);
    let _ = s.call(5, "kit-verify", &format!(r#"{{"home":"{home}","kit":"/k.zkit","originals-to":"/out"}}"#));
    let q = rx.recv_timeout(Duration::from_secs(30)).expect("asked");
    assert_eq!((q.verb.as_str(), q.args.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()), ("kit-verify", vec!["kit", "originals-to"]));
    s.end();
    closer.close();
}

/// A call not started is not done when cancelled, nor when the input closes: the call being done finishes and is
/// answered, the session ends 0, and the desktop is asked only for that one.
#[test]
fn a_call_not_started_is_not_done() {
    let b = Bench::new("notyet");
    let home = b.home.display().to_string();
    let (closer, rx) = desktop(&b.door(), |_| {
        std::thread::sleep(Duration::from_millis(700));
        Some(Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 1 }.to_bytes())
    });
    let mut s = b.session(EN);
    let file = b.root.join("f.txt");
    std::fs::write(&file, b"x").expect("file");
    s.send(&format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"history","arguments":{{"home":"{home}","file":"{}"}}}}}}"#, file.display()));
    // The first call is being done once the desktop has it (the desktop answers it 0.7 s later).
    let first = rx.recv_timeout(Duration::from_secs(30)).expect("the first call reached the desktop");
    assert_eq!(first.verb, "history");
    s.send(&format!(r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"annotate","arguments":{{"home":"{home}","note":"cancelled"}}}}}}"#));
    s.send(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2}}"#);
    s.send(&format!(r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"annotate","arguments":{{"home":"{home}","note":"after the input closes"}}}}}}"#));
    let lines: Vec<String> = {
        drop(s.child.stdin.take());
        let mut got = Vec::new();
        while let Some((l, _)) = s.next(30) {
            got.push(l);
        }
        got
    };
    assert_eq!(s.child.wait().expect("ends").code(), Some(0));
    closer.close();
    let asked: Vec<Request> = rx.try_iter().collect();
    assert!(asked.is_empty(), "only the call being done reached the desktop: {:?}", asked.iter().map(|q| q.verb.as_str()).collect::<Vec<_>>());
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("\"id\":1"), "{lines:?}");
}
