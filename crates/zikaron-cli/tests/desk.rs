//! `--home`: the real CLI against a stand-in desktop in the test process. The stand-in listens on the desktop's
//! socket (`zikaron_glue::door::place`, in the machine folder found through the home variable and the pointer
//! file), reads one request and answers with each reply the desktop can give; stdout, exit code and the
//! human-readable line are checked in both languages. Misuse must be refused before any request, so the
//! stand-in counts connections. Each test has its own machine folder and home (unix only: the machine folder
//! sits at a short `/tmp` path because socket paths are length-limited, and paths are compared as unix
//! resolves them).
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use zikaron_glue::door::{Reply, Request};

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");

/// A short private root (local socket paths are length-limited): a user home with the pointer to a machine
/// folder, and a data folder.
struct Bench {
    root: PathBuf,
    user: PathBuf,
    machine: PathBuf,
    home: PathBuf,
}

impl Bench {
    fn new(tag: &str) -> Bench {
        let base = if std::env::temp_dir().as_os_str().len() <= 40 { std::env::temp_dir() } else { PathBuf::from("/tmp") };
        let root = base.join(format!("zkd-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (user, machine, home) = (root.join("u"), root.join("m"), root.join("h"));
        for d in [&user, &machine, &home] {
            std::fs::create_dir_all(d).expect("dir");
        }
        let machine = std::fs::canonicalize(&machine).expect("machine");
        std::fs::write(user.join(zikaron_os::machine::POINTER), format!("{}\n", machine.display())).expect("pointer");
        Bench { root, user, machine, home }
    }

    fn place(&self) -> PathBuf {
        zikaron_glue::door::place(&self.machine, &self.home)
    }

    /// Run the real command line with this bench's home (`zikaron_os::HOME_VAR`) and the given language.
    fn run(&self, args: &[&str], lang: &str) -> (i32, Vec<u8>, String) {
        let mut c = Command::new(BIN);
        c.args(args).env(zikaron_os::HOME_VAR, &self.user).env_remove("LC_ALL").env_remove("LC_MESSAGES").env_remove("LANGUAGE").env("LANG", lang);
        let out = zikaron_os::spawn(c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()))
            .expect("runs")
            .wait_with_output()
            .expect("waits");
        (out.status.code().unwrap_or(-1), out.stdout, String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A stand-in desktop: answers each request with `answer(request)` and reports what came. `None` hangs up
/// without a reply.
fn desktop(at: &Path, answer: impl Fn(&Request) -> Option<Vec<u8>> + Send + 'static) -> (zikaron_os::door::Closer, std::sync::mpsc::Receiver<Request>) {
    let (l, closer) = zikaron_os::door::listen(at).expect("the door opens");
    let (tx, rx) = std::sync::mpsc::channel();
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

fn json(b: &[u8]) -> zikaron::json::Value {
    zikaron::json::parse(b).unwrap_or_else(|_| panic!("not one value: {}", String::from_utf8_lossy(b)))
}

fn text(v: &zikaron::json::Value, k: &str) -> String {
    v.member(k).and_then(|x| x.as_str()).unwrap_or_default().to_string()
}

const EN: &str = "en_US.UTF-8";
const ZH: &str = "zh_CN.UTF-8";

/// No desktop listening for that home: exit 4, `E_DESKTOP` with `NOT_OPEN`, the message in the system's
/// language; nothing written anywhere.
#[test]
fn no_desktop_for_that_home_is_no_answer() {
    let b = Bench::new("none");
    for (lang, line) in [(EN, "The desktop is locked or not open"), (ZH, "桌面锁着或没有打开")] {
        let (code, out, err) = b.run(&["annotate", "--home", b.home.to_str().unwrap(), "--subject", "x"], lang);
        assert_eq!(code, 4, "{err}");
        let v = json(&out);
        assert_eq!((text(&v, "reason"), text(&v, "detail")), ("E_DESKTOP".into(), "NOT_OPEN".into()));
        assert_eq!(err.lines().next(), Some(line), "{lang}");
    }
    assert_eq!(std::fs::read_dir(&b.home).expect("home").count(), 0, "nothing written into the home");
}

/// Each reply the desktop can give, answered by the verb's own answer.
#[test]
fn each_reply_is_answered_by_the_verbs_own_answer() {
    let b = Bench::new("each");
    let replies: Vec<(&str, Vec<&str>, Reply, i32, &str)> = vec![
        ("annotate", vec!["--subject", "s", "--note", "n"], Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 7 }, 0, r#"{"entryId":"0xe","ledger":"/l","ok":true,"seq":7,"written":true}"#),
        ("anchor", vec![], Reply::Anchored { tx: "0xt".into(), block: 9 }, 0, r#"{"blockNumber":9,"ok":true,"tx":"0xt"}"#),
        ("anchor", vec![], Reply::Reverted { tx: "0xt".into(), status: 0 }, 1, r#"{"ok":false,"reason":"E_TX_STATUS","state":0,"tx":"0xt"}"#),
        ("anchor", vec![], Reply::NotYet { tx: "0xt".into(), waited: 60 }, 4, r#"{"count":60,"ok":false,"reason":"E_TX_NOT_YET","tx":"0xt"}"#),
        ("anchor", vec![], Reply::Unheard { tx: "0xt".into(), detail: "d".into() }, 4, r#"{"detail":"d","ok":false,"reason":"E_UNREACHABLE","tx":"0xt"}"#),
        ("anchor", vec![], Reply::Voided { tx: "0xt".into() }, 1, r#"{"ok":false,"reason":"E_TX_VOID","tx":"0xt"}"#),
        ("kit-export", vec!["--out", "/o"], Reply::Kit { kit_id: "k".into(), path: "/o".into(), entries: 2, files: 1, dropped: vec![] }, 0, r#"{"dropped":[],"entries":2,"files":1,"kitId":"k","ok":true,"path":"/o","proofs":0,"state":"KIT_OK"}"#),
        ("anchor", vec![], Reply::Queued { count: 3 }, 3, r#"{"count":3,"ok":false,"reason":"E_QUEUED"}"#),
        ("attest", vec![], Reply::OnDesktop, 1, r#"{"ok":false,"reason":"E_ON_DESKTOP"}"#),
        ("annotate", vec![], Reply::Refused { code: "READ_ONLY".into(), tail: "t".into(), network: false, zh: "只读".into(), en: "read only".into() }, 1, r#"{"detail":"t","ok":false,"reason":"E_DESKTOP_REFUSED","token":"READ_ONLY"}"#),
        ("anchor", vec![], Reply::Refused { code: "UNREACHABLE".into(), tail: "t".into(), network: true, zh: "连不上".into(), en: "unreachable".into() }, 4, r#"{"detail":"t","ok":false,"reason":"E_DESKTOP_REFUSED","token":"UNREACHABLE"}"#),
        ("annotate", vec![], Reply::Closed, 4, r#"{"detail":"CLOSED","ok":false,"reason":"E_DESKTOP"}"#),
    ];
    for (verb, flags, reply, code, want) in replies {
        let bytes = reply.to_bytes();
        let (closer, rx) = desktop(&b.place(), move |_| Some(bytes.clone()));
        let mut args = vec![verb, "--home", b.home.to_str().unwrap()];
        args.extend(flags);
        let (got, out, err) = b.run(&args, EN);
        closer.close();
        assert_eq!((got, String::from_utf8_lossy(&out).to_string()), (code, want.to_string()), "{reply:?}: {err}");
        let q = rx.recv().expect("the request came");
        assert_eq!(q.verb, verb);
        assert_eq!(q.home, std::fs::canonicalize(&b.home).expect("home").display().to_string(), "the home as the system resolves it");
        // Human-readable line: one per named answer; a refusal carries the desktop's own words.
        let line = err.lines().next().unwrap_or_default().to_string();
        match &reply {
            Reply::Queued { .. } => assert_eq!(line, "Queued; waiting for the user to send it from the desktop"),
            Reply::OnDesktop => assert_eq!(line, "This action must be done on the desktop"),
            Reply::Refused { en, .. } => assert_eq!(&line, en),
            Reply::Closed => assert_eq!(line, "The desktop locked, closed this data folder or quit before it answered"),
            _ => assert!(err.is_empty(), "{reply:?}: {err}"),
        }
    }
}

/// The request carries each flag the verb takes beside `--home`, normalized: integers within the ceiling,
/// entry ids in canonical form, file and output paths made absolute.
#[test]
fn the_request_carries_each_flag_as_read() {
    let b = Bench::new("args");
    let file = b.root.join("f.txt");
    std::fs::write(&file, b"bytes").expect("file");
    let anchors = b.root.join("anchors.json");
    std::fs::write(&anchors, br#"[ {"tx": "0xt", "chainId": 1, "payloadKind": "bare", "content": "0xc"} ]"#).expect("anchors");
    let wrote = Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 1 }.to_bytes();
    let cases: Vec<(Vec<String>, Vec<(&str, String)>)> = vec![
        (vec!["grant".into(), "--grantee".into(), "0xg".into(), "--window-from".into(), "+5".into(), "--window-to".into(), "9".into(), "--scope".into(), "s".into()], vec![("grantee", "0xg".into()), ("window-from", "5".into()), ("window-to", "9".into()), ("scope", "s".into())]),
        (vec!["history".into(), "--file".into(), file.display().to_string(), "--note".into(), "n".into()], vec![("file", std::fs::canonicalize(&file).unwrap().display().to_string()), ("note", "n".into())]),
        (vec!["kit-export".into(), "--entry".into(), "ab".repeat(32), "--entry".into(), format!("0x{}", "cd".repeat(32)), "--out".into(), "rel/out".into()], vec![("entry", format!("0x{}", "ab".repeat(32))), ("entry", format!("0x{}", "cd".repeat(32))), ("out", std::path::absolute("rel/out").unwrap().display().to_string())]),
        (vec!["succeed".into(), "--to".into(), "0xt".into(), "--effective".into(), "12".into()], vec![("to", "0xt".into()), ("effective", "12".into())]),
        // The anchors file is sent as the JSON it holds, in canonical bytes.
        (vec!["adopt".into(), "--anchors".into(), anchors.display().to_string(), "--attestor".into(), "0xa".into()], vec![("anchors", r#"[{"chainId":1,"content":"0xc","payloadKind":"bare","tx":"0xt"}]"#.into()), ("attestor", "0xa".into())]),
    ];
    for (args, want) in cases {
        let w = wrote.clone();
        let (closer, rx) = desktop(&b.place(), move |_| Some(w.clone()));
        let mut all: Vec<&str> = vec![args[0].as_str(), "--home", b.home.to_str().unwrap()];
        all.extend(args[1..].iter().map(String::as_str));
        let (code, _, err) = b.run(&all, EN);
        closer.close();
        assert_eq!(code, 0, "{args:?}: {err}");
        let q = rx.recv().expect("came");
        let got: Vec<(&str, String)> = q.args.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let mut want = want;
        let mut got = got;
        want.sort();
        got.sort();
        assert_eq!(got, want, "{args:?}");
    }
}

/// Misuse is refused before the desktop is asked: exit 2, nothing on stdout, no connection.
#[test]
fn misuse_is_judged_before_the_desktop_is_asked() {
    let b = Bench::new("misuse");
    let (closer, rx) = desktop(&b.place(), |_| Some(Reply::Closed.to_bytes()));
    let home = b.home.to_str().unwrap().to_string();
    let file = b.root.join("not-there.txt").display().to_string();
    let not_json = b.root.join("anchors.txt");
    std::fs::write(&not_json, b"[1,").expect("a file that is not JSON");
    let not_json = not_json.display().to_string();
    let cases: Vec<(Vec<&str>, &str, &str)> = vec![
        (vec!["annotate", "--home", "/nowhere/at/all"], "E_UNREADABLE #3 (15 bytes)", "cannot be read"),
        (vec!["annotate", "--home", &home, "--key", "x"], "E_ARGS --key", "not taken together with --home (the desktop uses its own identity and ledger)"),
        (vec!["annotate", "--home", &home, "--ledger", "x"], "E_ARGS --ledger", "not taken together with --home (the desktop uses its own identity and ledger)"),
        (vec!["history", "--home", &home], "E_ARGS --file", "this flag is missing"),
        (vec!["history", "--home", &home, "--file", &file], "", "cannot be read"),
        (vec!["history", "--home", &home, "--content", "x"], "E_ARGS --content", "not taken together with --home (the desktop uses its own identity and ledger)"),
        (vec!["kit-export", "--home", &home], "E_ARGS --out", "this flag is missing"),
        (vec!["kit-export", "--home", &home, "--out", "/o", "--entry", "zz"], "E_ARGS #7 (2 bytes)", "not 64 lowercase hex digits"),
        (vec!["grant", "--home", &home, "--window-from", "9007199254740992"], "E_ARGS --window-from", "past the whole-number ceiling 9007199254740991"),
        (vec!["keygen", "--home", &home], "E_ARGS --home", "not among this verb's flags"),
        (vec!["show", "--home", &home], "E_ARGS --home", "not among this verb's flags"),
        (vec!["adopt", "--home", &home, "--anchors", &file], "", "cannot be read"),
        (vec!["adopt", "--home", &home, "--anchors", &not_json], "", "not JSON the law takes (§3.5)"),
        (vec!["adopt", "--home", &home, "--key", "x"], "E_ARGS --key", "not taken together with --home (the desktop uses its own identity and ledger)"),
        (vec!["scan", "--home", &home], "E_ARGS --home", "not among this verb's flags"),
        (vec!["annotate", "--home", &home, "--home", &home], "E_ARGS --home", "given more than once"),
    ];
    for (args, first, second) in cases {
        let (code, out, err) = b.run(&args, EN);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(out.is_empty(), "{args:?}: misuse writes nothing to stdout");
        let mut lines = err.lines();
        let l1 = lines.next().unwrap_or_default();
        if !first.is_empty() {
            assert_eq!(l1, first, "{args:?}");
        } else {
            assert!(l1.starts_with("E_UNREADABLE "), "{args:?}: {err}");
        }
        assert_eq!(lines.next(), Some(second), "{args:?}");
    }
    closer.close();
    assert!(rx.try_recv().is_err(), "the desktop was never asked");
}

/// A desktop that hangs up or replies in a form this version cannot read gives `BROKEN`; a socket path too
/// long for the system gives `PATH_TOO_LONG`; an unreadable machine-folder pointer gives `NO_MACHINE`.
#[test]
fn a_door_that_breaks_or_cannot_be_reached_is_named() {
    let b = Bench::new("broken");
    for answer in [None, Some(b"{\"form\":\"zikaron-door/1\",\"reply\":\"later\"}".to_vec())] {
        let (closer, _rx) = desktop(&b.place(), move |_| answer.clone());
        let (code, out, err) = b.run(&["annotate", "--home", b.home.to_str().unwrap()], EN);
        closer.close();
        let v = json(&out);
        assert_eq!((code, text(&v, "reason"), text(&v, "detail")), (4, "E_DESKTOP".into(), "BROKEN".into()), "{err}");
        assert_eq!(err.lines().next(), Some("The desktop broke off, or answered in a form this version cannot read"));
    }
    // A machine folder so deep its socket path does not fit.
    let deep = b.root.join("d".repeat(120));
    std::fs::create_dir_all(&deep).expect("deep");
    std::fs::write(b.user.join(zikaron_os::machine::POINTER), format!("{}\n", deep.display())).expect("pointer");
    let (code, out, _) = b.run(&["annotate", "--home", b.home.to_str().unwrap()], EN);
    assert_eq!((code, text(&json(&out), "detail")), (4, "PATH_TOO_LONG".into()));
    // A pointer that does not read.
    std::fs::write(b.user.join(zikaron_os::machine::POINTER), b"relative\n").expect("pointer");
    let (code, out, err) = b.run(&["annotate", "--home", b.home.to_str().unwrap()], ZH);
    assert_eq!((code, text(&json(&out), "detail")), (4, "NO_MACHINE".into()));
    assert_eq!(err.lines().next(), Some("找不到本机数据所在的文件夹"));
}

/// Section 11 of `CLI-SCHEMA.md` lists the verbs done through the desktop and the flags each takes beside
/// `--home`, row by row as `verbs::HOME`.
#[test]
fn the_cli_schema_lists_the_verbs_done_through_the_desktop() {
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CLI-SCHEMA.md")).expect("CLI-SCHEMA.md");
    let section = doc.split("## 11 · `--home`").nth(1).expect("section 11");
    let table = section.split("| Verb | Flags beside `--home` |").nth(1).expect("the verb table");
    let rows: Vec<(String, Vec<String>)> = table
        .lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .map(|l| {
            let (verb, rest) = l.split_once('`').expect("verb");
            let flags = rest.split('|').nth(1).unwrap_or_default().split('`').skip(1).step_by(2).map(str::to_string).collect();
            (verb.to_string(), flags)
        })
        .collect();
    let real: Vec<(String, Vec<String>)> = zikaron_cli::verbs::HOME.iter().map(|(v, fl)| (v.to_string(), fl.iter().map(|f| f.to_string()).collect())).collect();
    assert_eq!(rows, real);
}
