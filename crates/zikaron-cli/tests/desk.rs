#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use zikaron_glue::door::{Own, Reply, Request, Rule};

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");

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

#[test]
fn no_desktop_for_that_home_is_no_answer() {
    let b = Bench::new("none");
    for (lang, line) in [
        (EN, "The desktop is locked, not running, or has another data folder open (another role or identity); switch to this folder's identity and role on the desktop, or give --home the data folder in Settings > Local data"),
        (ZH, "桌面锁着、没有运行,或开着的不是这个数据目录(停在另一个角色或身份上);在桌面上切到它所属的身份与角色,或把 --home 改成「设置 > 本机数据」里的数据目录"),
    ] {
        let (code, out, err) = b.run(&["annotate", "--home", b.home.to_str().unwrap(), "--subject", "x"], lang);
        assert_eq!(code, 4, "{err}");
        let v = json(&out);
        assert_eq!((text(&v, "reason"), text(&v, "detail")), ("E_DESKTOP".into(), "NOT_OPEN".into()));
        assert_eq!(err.lines().next(), Some(line), "{lang}");
    }
    assert_eq!(std::fs::read_dir(&b.home).expect("home").count(), 0, "nothing written into the home");
}

#[test]
fn each_reply_is_answered_by_the_verbs_own_answer() {
    let b = Bench::new("each");
    let kit = b.home.display().to_string();
    let verified = |unanswered: Vec<String>| Reply::Verified {
        kit_id: "k".into(),
        entries: 1,
        records: vec![zikaron_glue::door::Record { entry_id: "0xe".into(), content: "0xc".into(), original: zikaron_glue::recordcheck::Original::Match, chain: zikaron_glue::recordcheck::OnChain::Anchored { first_at: 9 }, deleted: None }],
        taken: None,
        unanswered,
    };
    let replies: Vec<(&str, Vec<&str>, Reply, i32, &str)> = vec![
        ("annotate", vec!["--subject", "s", "--note", "n"], Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 7 }, 0, r#"{"entryId":"0xe","ledger":"/l","ok":true,"seq":7,"written":true}"#),
        ("anchor", vec![], Reply::Anchored { tx: "0xt".into(), block: 9 }, 0, r#"{"blockNumber":9,"ok":true,"tx":"0xt"}"#),
        ("anchor", vec![], Reply::Reverted { tx: "0xt".into(), status: 0 }, 1, r#"{"ok":false,"reason":"E_TX_STATUS","state":0,"tx":"0xt"}"#),
        ("anchor", vec![], Reply::NotYet { tx: "0xt".into(), waited: 60 }, 4, r#"{"count":60,"ok":false,"reason":"E_TX_NOT_YET","tx":"0xt"}"#),
        ("anchor", vec![], Reply::Unheard { tx: "0xt".into(), detail: "d".into() }, 4, r#"{"detail":"d","ok":false,"reason":"E_UNREACHABLE","tx":"0xt"}"#),
        ("anchor", vec![], Reply::Voided { tx: "0xt".into() }, 1, r#"{"ok":false,"reason":"E_TX_VOID","tx":"0xt"}"#),
        ("kit-export", vec!["--out", "/o"], Reply::Kit { kit_id: "k".into(), path: "/o".into(), entries: 2, files: 1, dropped: vec![], originals: vec!["a.txt".into()], left_out: vec!["/gone/b.txt".into()] }, 0, r#"{"dropped":[],"entries":2,"files":1,"kitId":"k","leftOut":["/gone/b.txt"],"ok":true,"originals":["a.txt"],"path":"/o","state":"KIT_OK"}"#),
        ("anchor", vec![], Reply::Queued { count: 3 }, 3, r#"{"count":3,"ok":false,"reason":"E_QUEUED"}"#),
        ("annotate", vec![], Reply::OnDesktop, 1, r#"{"ok":false,"reason":"E_ON_DESKTOP"}"#),
        ("annotate", vec![], Reply::Refused { code: "READ_ONLY".into(), tail: "t".into(), network: false, zh: "只读".into(), en: "read only".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_READ_ONLY","token":"READ_ONLY"}"#),
        ("init", vec![], Reply::Refused { code: "ALREADY_ROOTED".into(), tail: "t".into(), network: false, zh: "已有根".into(), en: "rooted".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_ALREADY_ROOTED"}"#),
        ("annotate", vec![], Reply::Refused { code: "ENTRY_REFUSED".into(), tail: "t".into(), network: false, zh: "条目被拒".into(), en: "entry refused".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_ENTRY"}"#),
        ("kit-export", vec!["--out", "/o"], Reply::Refused { code: "LANDING".into(), tail: "t".into(), network: false, zh: "落盘".into(), en: "landing".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_LEDGER"}"#),
        ("kit-verify", vec!["--kit", kit.as_str()], Reply::Refused { code: "EXTRACT_EXISTS".into(), tail: "t".into(), network: false, zh: "已有".into(), en: "exists".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_EXTRACT","token":"EXISTS"}"#),
        ("kit-export", vec!["--out", "/o"], Reply::Refused { code: "ADDRESS_CHAIN_UNREAD".into(), tail: "t".into(), network: true, zh: "未读".into(), en: "unread".into(), own: None }, 4, r#"{"detail":"t","ok":false,"reason":"E_ADDRESSED","token":"ADDRESS_CHAIN_UNREAD"}"#),
        ("kit-verify", vec!["--kit", kit.as_str()], Reply::Refused { code: "UNREACHABLE".into(), tail: "t".into(), network: true, zh: "连不上".into(), en: "unreachable".into(), own: None }, 4, r#"{"detail":"t","ok":false,"reason":"E_UNREACHABLE"}"#),
        ("annotate", vec![], Reply::Refused { code: "HANDED_OVER".into(), tail: "t".into(), network: false, zh: "已移交".into(), en: "handed over".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_HANDED_OVER"}"#),
        ("annotate", vec![], Reply::Refused { code: "DOOR_UNREAD".into(), tail: "t".into(), network: false, zh: "读不成".into(), en: "unread".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_DOOR_UNREAD"}"#),
        ("annotate", vec![], Reply::Refused { code: "NOT_A_CODE".into(), tail: "t".into(), network: false, zh: "无".into(), en: "none".into(), own: None }, 4, r#"{"detail":"BROKEN","ok":false,"reason":"E_DESKTOP"}"#),
        ("anchor", vec![], Reply::Refused { code: "UNREACHABLE".into(), tail: "t".into(), network: true, zh: "连不上".into(), en: "unreachable".into(), own: None }, 4, r#"{"detail":"t","ok":false,"reason":"E_SEND_REFUSED","token":"UNREACHABLE"}"#),
        ("anchor", vec![], Reply::Refused { code: "QUEUE_EMPTY".into(), tail: "t".into(), network: false, zh: "队列空".into(), en: "queue empty".into(), own: None }, 1, r#"{"detail":"t","ok":false,"reason":"E_SEND_REFUSED","token":"QUEUE_EMPTY"}"#),
        ("annotate", vec![], Reply::Refused { code: "BOOK_CODE".into(), tail: "11155111".into(), network: false, zh: "簿".into(), en: "book".into(), own: Some(Own::IssueRefused) }, 1, r#"{"detail":"11155111","ok":false,"reason":"E_ISSUE_REFUSED","token":"BOOK_CODE"}"#),
        ("annotate", vec![], Reply::Refused { code: "UNREACHABLE".into(), tail: "t".into(), network: true, zh: "连不上".into(), en: "unreachable".into(), own: Some(Own::IssueRefused) }, 4, r#"{"detail":"t","ok":false,"reason":"E_ISSUE_REFUSED","token":"UNREACHABLE"}"#),
        ("annotate", vec![], Reply::Refused { code: "NO_ENDPOINT".into(), tail: "11155111".into(), network: false, zh: "无节点".into(), en: "no node".into(), own: Some(Own::IssueRefused) }, 4, r#"{"detail":"11155111","ok":false,"reason":"E_ISSUE_REFUSED","token":"NO_ENDPOINT"}"#),
        ("annotate", vec![], Reply::Refused { code: "LEDGER_NOT_FETCHED".into(), tail: "t".into(), network: false, zh: "未取".into(), en: "not fetched".into(), own: Some(Own::ReadOnly) }, 1, r#"{"detail":"t","ok":false,"reason":"E_READ_ONLY","token":"LEDGER_NOT_FETCHED"}"#),
        ("annotate", vec![], Reply::Refused { code: "HANDED_OFF".into(), tail: "0xab".into(), network: false, zh: "已交接".into(), en: "handed off".into(), own: Some(Own::HandedOff) }, 1, r#"{"handoff":"0xab","ok":false,"reason":"E_HANDED_OFF"}"#),
        ("annotate", vec![], Reply::Refused { code: "INSUFFICIENT_FUNDS".into(), tail: "t".into(), network: false, zh: "余额不足".into(), en: "short".into(), own: Some(Own::SendRefused) }, 1, r#"{"detail":"t","ok":false,"reason":"E_SEND_REFUSED","token":"INSUFFICIENT_FUNDS"}"#),
        ("annotate", vec![], Reply::Noticed { entry_id: "0xe".into(), ledger: "/l".into(), tx: "0xt".into(), notice: zikaron_send::withdraw::NOT_INCLUDED.into(), withdrawn: false }, 4, r#"{"entryId":"0xe","notice":"not-included","ok":false,"reason":"E_NOT_NOTICED","tx":"0xt"}"#),
        ("init", vec![], Reply::Refused { code: "LEDGER_NOT_FETCHED".into(), tail: "t".into(), network: false, zh: "未取".into(), en: "not fetched".into(), own: Some(Own::ReadOnly) }, 1, r#"{"detail":"t","ok":false,"reason":"E_READ_ONLY","token":"LEDGER_NOT_FETCHED"}"#),
        ("anchor", vec![], Reply::OnChain { names: vec!["0xh".into()], tx: "0xt".into(), block: 5 }, 1, r#"{"blockNumber":5,"names":["0xh"],"ok":false,"reason":"E_ON_CHAIN","tx":"0xt"}"#),
        ("annotate", vec![], Reply::Closed, 4, r#"{"detail":"CLOSED","ok":false,"reason":"E_DESKTOP"}"#),
        ("annotate", vec![], Reply::RuleRefused { rule: Rule::Retraction, token: Some("E_RETRACT_REPEATED".into()), entry_id: None, names: None, zh: "已删过".into(), en: "already deleted".into() }, 1, r#"{"ok":false,"reason":"E_RETRACTION","token":"E_RETRACT_REPEATED"}"#),
        ("annotate", vec![], Reply::RuleRefused { rule: Rule::Retraction, token: Some("E_RETRACT_NOT_AUDITED".into()), entry_id: Some("0xs".into()), names: None, zh: "未核".into(), en: "not audited".into() }, 1, r#"{"entryId":"0xs","ok":false,"reason":"E_RETRACTION","token":"E_RETRACT_NOT_AUDITED"}"#),
        ("annotate", vec![], Reply::RuleRefused { rule: Rule::Withheld, token: Some("E_GRANT_WORK_DELETED".into()), entry_id: None, names: Some(Vec::new()), zh: "作品已删".into(), en: "work deleted".into() }, 1, r#"{"names":[],"ok":false,"reason":"E_GRANT_WITHHELD","token":"E_GRANT_WORK_DELETED"}"#),
        ("annotate", vec![], Reply::RuleRefused { rule: Rule::Revocation, token: Some("E_REVOKE_NOT_A_GRANT".into()), entry_id: Some("0xg".into()), names: None, zh: "不是授权".into(), en: "not a grant".into() }, 1, r#"{"entryId":"0xg","ok":false,"reason":"E_REVOCATION","token":"E_REVOKE_NOT_A_GRANT"}"#),
        ("annotate", vec![], Reply::RuleRefused { rule: Rule::Annotation, token: Some("E_NAME_NOT_IN_LEDGER".into()), entry_id: Some("0xs".into()), names: None, zh: "不在账本".into(), en: "not in the ledger".into() }, 1, r#"{"entryId":"0xs","ok":false,"reason":"E_ANNOTATION","token":"E_NAME_NOT_IN_LEDGER"}"#),
        ("annotate", vec![], Reply::RuleRefused { rule: Rule::WouldBreak, token: None, entry_id: Some("0xe".into()), names: Some(vec!["SEQ_GAP".into()]), zh: "会断链".into(), en: "would break".into() }, 1, r#"{"entryId":"0xe","names":["SEQ_GAP"],"ok":false,"reason":"E_WOULD_BREAK"}"#),
        ("kit-verify", vec!["--kit", kit.as_str()], verified(vec!["http://n1 E_UNANSWERED:chain 1 eth_getLogs range".into()]), 0, r#"{"entries":1,"kitId":"k","ok":true,"records":[{"anchored":"anchored","content":"0xc","entryId":"0xe","firstAt":9,"original":"match"}],"unanswered":["http://n1 E_UNANSWERED:chain 1 eth_getLogs range"]}"#),
        ("kit-verify", vec!["--kit", kit.as_str()], verified(Vec::new()), 0, r#"{"entries":1,"kitId":"k","ok":true,"records":[{"anchored":"anchored","content":"0xc","entryId":"0xe","firstAt":9,"original":"match"}]}"#),
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
        let line = err.lines().next().unwrap_or_default().to_string();
        match &reply {
            Reply::Queued { .. } => assert_eq!(line, "Queued; waiting for the user to send it from the desktop"),
            Reply::OnDesktop => assert_eq!(line, "This action must be done on the desktop"),
            Reply::OnChain { .. } => assert_eq!(line, "The chain already holds this anchor (put on chain elsewhere); nothing was sent"),
            Reply::Refused { own: Some(Own::IssueRefused), .. } => assert!(err.is_empty(), "{reply:?}: {err}"),
            Reply::Refused { own: Some(_), en, .. } => assert!(!line.is_empty() && &line != en, "{reply:?}: {err}"),
            Reply::Refused { code, .. } if code == "NOT_A_CODE" => assert!(!line.is_empty(), "{reply:?}: {err}"),
            Reply::Refused { en, .. } | Reply::RuleRefused { en, .. } => assert_eq!(&line, en),
            Reply::Noticed { .. } => {}
            Reply::Closed => assert_eq!(line, "The desktop locked, closed this data folder or quit before it answered"),
            _ => assert!(err.is_empty(), "{reply:?}: {err}"),
        }
    }
}

#[test]
fn the_output_through_the_desktop_is_the_same_in_either_language() {
    let b = Bench::new("tongue");
    let replies = [
        ("annotate", Reply::Refused { code: "READ_ONLY".into(), tail: "read-only evidence".into(), network: false, zh: "只读".into(), en: "read only".into(), own: None }),
        ("anchor", Reply::Refused { code: "UNREACHABLE".into(), tail: "transport failed".into(), network: true, zh: "连不上".into(), en: "unreachable".into(), own: None }),
        ("anchor", Reply::Unheard { tx: "0xt".into(), detail: "no node answered".into() }),
        ("annotate", Reply::RuleRefused { rule: Rule::Annotation, token: Some("E_NAME_NOT_IN_LEDGER".into()), entry_id: Some("0xs".into()), names: None, zh: "不在账本".into(), en: "not in the ledger".into() }),
    ];
    for (verb, reply) in replies {
        let mut seen = Vec::new();
        for lang in [EN, ZH] {
            let bytes = reply.to_bytes();
            let (closer, _rx) = desktop(&b.place(), move |_| Some(bytes.clone()));
            let (code, out, err) = b.run(&[verb, "--home", b.home.to_str().unwrap()], lang);
            closer.close();
            seen.push((code, out, err.lines().next().unwrap_or_default().to_string()));
        }
        assert_eq!((seen[0].0, &seen[0].1), (seen[1].0, &seen[1].1), "{verb}: the same output in either language");
        if !matches!(reply, Reply::Unheard { .. }) {
            assert_ne!(seen[0].2, seen[1].2, "{verb}: the line for people follows the language");
        }
    }
}

#[test]
fn the_request_carries_each_flag_as_read() {
    let b = Bench::new("args");
    let file = b.root.join("f.txt");
    std::fs::write(&file, b"bytes").expect("file");
    let wrote = Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 1 }.to_bytes();
    let cases: Vec<(Vec<String>, Vec<(&str, String)>)> = vec![
        (vec!["grant".into(), "--grantee".into(), "0xg".into(), "--window-from".into(), "+5".into(), "--window-to".into(), "9".into(), "--scope".into(), "s".into()], vec![("grantee", "0xg".into()), ("window-from", "5".into()), ("window-to", "9".into()), ("scope", "s".into())]),
        (vec!["history".into(), "--file".into(), file.display().to_string(), "--note".into(), "n".into()], vec![("file", std::fs::canonicalize(&file).unwrap().display().to_string()), ("note", "n".into())]),
        (vec!["kit-export".into(), "--entry".into(), "ab".repeat(32), "--entry".into(), format!("0x{}", "cd".repeat(32)), "--out".into(), "rel/out".into()], vec![("entry", format!("0x{}", "ab".repeat(32))), ("entry", format!("0x{}", "cd".repeat(32))), ("out", std::path::absolute("rel/out").unwrap().display().to_string())]),
        (vec!["kit-export".into(), "--out".into(), "/o".into(), "--originals".into(), "none".into()], vec![("out", "/o".into()), ("originals", "none".into())]),
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

#[test]
fn misuse_is_judged_before_the_desktop_is_asked() {
    let b = Bench::new("misuse");
    let (closer, rx) = desktop(&b.place(), |_| Some(Reply::Closed.to_bytes()));
    let home = b.home.to_str().unwrap().to_string();
    let file = b.root.join("not-there.txt").display().to_string();
    let plain = b.root.join("plain-ledger").display().to_string();
    let cases: Vec<(Vec<&str>, &str, &str)> = vec![
        (vec!["annotate", "--home", "/nowhere/at/all"], "E_UNREADABLE #3 (15 bytes)", "cannot be read"),
        (vec!["annotate", "--home", &home, "--key", "x"], "E_ARGS --key", "not taken together with --home (the desktop uses its own identity and ledger)"),
        (vec!["annotate", "--home", &home, "--ledger", "x"], "E_ARGS --ledger", "not taken together with --home (the desktop uses its own identity and ledger)"),
        (vec!["history", "--home", &home], "E_ARGS --file", "this flag is missing"),
        (vec!["history", "--home", &home, "--file", &file], "", "cannot be read"),
        (vec!["history", "--home", &home, "--content", "x"], "E_ARGS --content", "not taken together with --home (the desktop uses its own identity and ledger)"),
        (vec!["kit-export", "--home", &home], "E_ARGS --out", "this flag is missing"),
        (vec!["kit-export", "--home", &home, "--out", "/o", "--entry", "zz"], "E_ARGS #7 (2 bytes)", "not 64 lowercase hex digits"),
        (vec!["kit-export", "--home", &home, "--out", "/o", "--originals", "all"], "E_ARGS --originals", "--originals takes none"),
        (vec!["kit-export", "--ledger", &plain, "--out", "/o", "--originals", "none"], "E_ARGS --originals", "--originals applies only to a home the desktop keeps; give a plain ledger folder's originals one by one with --file"),
        (vec!["grant", "--home", &home, "--window-from", "9007199254740992"], "E_ARGS --window-from", "past the whole-number ceiling 9007199254740991"),
        (vec!["keygen", "--home", &home], "E_ARGS --home", "not among this verb's flags"),
        (vec!["show", "--home", &home], "E_ARGS --home", "not among this verb's flags"),
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
    let deep = b.root.join("d".repeat(120));
    std::fs::create_dir_all(&deep).expect("deep");
    std::fs::write(b.user.join(zikaron_os::machine::POINTER), format!("{}\n", deep.display())).expect("pointer");
    let (code, out, _) = b.run(&["annotate", "--home", b.home.to_str().unwrap()], EN);
    assert_eq!((code, text(&json(&out), "detail")), (4, "PATH_TOO_LONG".into()));
    std::fs::write(b.user.join(zikaron_os::machine::POINTER), b"../relative\n").expect("pointer");
    let (code, out, err) = b.run(&["annotate", "--home", b.home.to_str().unwrap()], ZH);
    assert_eq!((code, text(&json(&out), "detail")), (4, "NO_MACHINE".into()));
    assert_eq!(err.lines().next(), Some("找不到本机数据所在的文件夹"));
}

#[test]
fn migrate_through_the_desktop_is_done_on_the_desktop_only() {
    let b = Bench::new("migrate");
    let (closer, rx) = desktop(&b.place(), |_| Some(Reply::Wrote { entry_id: "0xe".into(), ledger: "/l".into(), seq: 1 }.to_bytes()));
    for (lang, line) in [(EN, "This action must be done on the desktop"), (ZH, "此动作须在桌面上做")] {
        let (code, out, err) = b.run(&["migrate", "--home", b.home.to_str().unwrap(), "--effective", "5"], lang);
        assert_eq!((code, String::from_utf8_lossy(&out).to_string()), (1, r#"{"ok":false,"reason":"E_ON_DESKTOP"}"#.to_string()), "{lang}: {err}");
        assert_eq!(err.lines().next(), Some(line), "{lang}");
    }
    closer.close();
    assert!(rx.try_recv().is_err(), "the desktop was asked nothing");
    assert_eq!(std::fs::read_dir(&b.home).expect("home").count(), 0, "nothing written into the home");
}

#[test]
fn a_blank_subject_is_refused_as_on_every_road_before_the_desktop_is_asked() {
    let b = Bench::new("blank-subject");
    let (code, out, err) = b.run(&["annotate", "--home", b.home.to_str().unwrap(), "--subject", "", "--note", "n"], EN);
    assert_eq!(code, 1, "{err}");
    let v = json(&out);
    assert_eq!((text(&v, "reason"), text(&v, "token")), ("E_ANNOTATION".into(), "E_NAME_SHAPE".into()), "{v:?}");
    assert_eq!(std::fs::read_dir(&b.home).expect("home").count(), 0, "nothing written into the home");
}
