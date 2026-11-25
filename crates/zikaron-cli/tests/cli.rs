use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");

fn src(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("读不出 {}:{e}", p.display()))
}

fn sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for item in std::fs::read_dir(&d).expect("列不动 src") {
            let e = item.expect("列不动");
            if e.path().is_dir() {
                stack.push(e.path());
            } else if e.path().extension().and_then(|x| x.to_str()) == Some("rs") {
                let name = e.file_name().to_string_lossy().into_owned();
                out.push((name, std::fs::read_to_string(e.path()).expect("读不出")));
            }
        }
    }
    out
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-cli-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("建不出草稿地");
    Scratch(p)
}

struct Ran {
    code: i32,
    out: Vec<u8>,
}

fn run_in(dir: &Path, args: &[&str]) -> Ran {
    let o = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env(zikaron_os::HOME_VAR, own_home())
        .output()
        .expect("起不动 zikaron");
    Ran {
        code: o.status.code().unwrap_or(-1),
        out: o.stdout,
    }
}

fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-home-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}


#[path = "support/old_grant.rs"]
mod old_grant;

const A_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

#[test]
fn a_misuse_writes_not_one_byte_to_stdout() {
    let w = scratch("misuse");
    for args in [
        vec!["conjure"],
        vec!["keygen", "--seed", "1"],
        vec!["badge", "--decode", "/nowhere/at/all"],
    ] {
        let r = run_in(&w, &args);
        assert_eq!(r.code, 2, "{args:?} 该退 2");
        assert!(r.out.is_empty(), "{args:?} 的 stdout 该是零字节");
    }
}

#[test]
fn a_misuse_names_its_subject_on_the_first_line_of_stderr() {
    let w = scratch("stderr");
    let o = Command::new(BIN)
        .args(["badge", "--decode", "/nowhere/at/all"])
        .current_dir(&w)
        .output()
        .expect("起不动");
    let err = String::from_utf8_lossy(&o.stderr);
    let first = err.lines().next().unwrap_or_default();
    assert_eq!(first, "E_UNREADABLE #3 (15 bytes)");
}

#[test]
fn an_answer_carries_no_trailing_newline() {
    let w = scratch("newline");
    let r = run_in(&w, &["keygen"]);
    assert_eq!(r.code, 0);
    assert!(!r.out.is_empty());
    assert_ne!(*r.out.last().expect("空答"), b'\n');
}

#[test]
fn an_answer_is_canonical_json_with_members_in_byte_order() {
    let w = scratch("canon");
    let r = run_in(&w, &["keygen"]);
    let text = String::from_utf8(r.out).expect("答不是 UTF-8");
    assert!(text.starts_with('{') && text.ends_with('}'));
    assert!(!text.contains(' '), "正典字节里没有空白");
    let keys: Vec<&str> = text
        .split(',')
        .filter_map(|part| part.split(':').next())
        .map(|k| k.trim_start_matches('{').trim_matches('"'))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    assert_eq!(keys, sorted, "成员该按字节序");
}

#[test]
fn the_law_refuses_what_the_shell_hands_it_and_the_token_comes_back_verbatim() {
    let w = scratch("token");
    let r = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(r.code, 0, "建档该绿");
    let r = run_in(&w, &["history", "--ledger", "b", "--key", A_KEY, "--content", &format!("0x{}", "aa".repeat(32))]);
    assert_eq!(r.code, 1);
    let text = String::from_utf8_lossy(&r.out).into_owned();
    assert!(text.contains("\"token\":\"E_BODY_FIELD\""), "实得 {text}");
    assert!(text.contains("\"reason\":\"E_ENTRY\""), "实得 {text}");
}

#[test]
fn partial_gets_its_own_exit_code_and_is_never_folded_into_green() {
    let w = scratch("partial");
    assert_eq!(
        run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code,
        0
    );
    let id = old_grant::old_grant(&w.0.join("b"), A_KEY, "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc", &format!("0x{}", "c1".repeat(32)), &format!("0x{}", "71".repeat(32)));
    let file = format!("b/{}.entry", id.trim_start_matches("0x"));
    let c = run_in(&w, &["check-grant", "--grant", &file, "--ledger", "b"]);
    assert_eq!(c.code, 3, "PARTIAL 该有自己那一格");
    assert!(String::from_utf8_lossy(&c.out).contains("\"verdict\":\"PARTIAL\""));
}

#[test]
fn a_grant_missing_a_member_is_told_by_its_shape_before_any_chain() {
    let w = scratch("grant-no-terms");
    assert_eq!(run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code, 0);
    let before = std::fs::read_dir(w.0.join("b")).expect("the ledger").count();
    let g = run_in(&w, &["grant", "--ledger", "b", "--key", A_KEY, "--grantee", "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc", "--work", &format!("0x{}", "c1".repeat(32))]);
    let said = String::from_utf8_lossy(&g.out).to_string();
    assert_eq!(g.code, 1, "{said}");
    assert!(said.contains("\"reason\":\"E_ENTRY\"") && said.contains("\"token\":\"E_BODY_FIELD\""), "{said}");
    assert_eq!(std::fs::read_dir(w.0.join("b")).expect("the ledger").count(), before, "nothing written");
}

#[test]
fn a_grant_with_no_node_to_check_on_is_refused_unwritten() {
    let w = scratch("grant-no-node");
    assert_eq!(run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code, 0);
    let before = std::fs::read_dir(w.0.join("b")).expect("the ledger").count();
    let g = run_in(&w, &["grant", "--ledger", "b", "--key", A_KEY, "--grantee", "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc", "--work", &format!("0x{}", "c1".repeat(32)), "--terms", &format!("0x{}", "71".repeat(32))]);
    let said = String::from_utf8_lossy(&g.out).to_string();
    assert_eq!(g.code, 4, "{said}");
    assert!(said.contains("\"reason\":\"E_ISSUE_REFUSED\"") && said.contains("\"token\":\"NO_ENDPOINT\""), "{said}");
    assert_eq!(std::fs::read_dir(w.0.join("b")).expect("the ledger").count(), before, "nothing written");
}

#[test]
fn a_world_that_cannot_answer_is_not_a_world_that_answers_no() {
    let w = scratch("unanswered");
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("绑不到端口");
        let p = l.local_addr().expect("读不出端口").port();
        drop(l);
        p
    };
    let ep = format!("31337=http://127.0.0.1:{port}");
    let r = run_in(
        &w,
        &[
            "anchor", "--endpoint", &ep, "--key", A_KEY, "--form", "registry", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3",
            "--hash", &format!("0x{}", "11".repeat(32)), "--wait-secs", "2",
        ],
    );
    assert_eq!(r.code, 4, "答不出该走 4");
}

#[test]
fn every_verb_the_shell_runs_marks_the_trace_and_the_bases_it_leans_on_mark_theirs() {
    let w = scratch("trace");
    let trace = w.join("trace.log");
    let o = Command::new(BIN)
        .args(["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"])
        .current_dir(&w)
        .env("ZIKARON_TRACE", &trace)
        .output()
        .expect("起不动");
    assert_eq!(o.status.code(), Some(0));
    let marks = std::fs::read_to_string(&trace).unwrap_or_default();
    for id in ["V1", "K1", "A1"] {
        assert!(marks.lines().any(|l| l.trim() == id), "痕迹里没有 {id}:{marks}");
    }
}

#[test]
fn the_kit_never_lands_unless_the_kit_law_says_it_is_a_kit() {
    let w = scratch("kit");
    assert_eq!(
        run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code,
        0
    );
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "pack", "--note", "", "--as", "folder"]);
    assert_eq!(r.code, 0);
    assert!(String::from_utf8_lossy(&r.out).contains("\"state\":\"KIT_OK\""));
    assert!(w.join("pack/manifest.json").is_file());
    let before = std::fs::read(w.join("pack/manifest.json")).expect("读不出清单");
    let again = run_in(&w, &["kit-export", "--ledger", "b", "--out", "pack", "--note", "第二趟"]);
    assert_eq!(again.code, 1);
    assert_eq!(before, std::fs::read(w.join("pack/manifest.json")).expect("读不出清单"));
    let leftovers: Vec<String> = std::fs::read_dir(&w)
        .expect("列不动")
        .filter_map(|x| x.ok())
        .map(|x| x.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("staging"))
        .collect();
    assert!(leftovers.is_empty(), "临时地没清干净:{leftovers:?}");
}

#[test]
fn the_crate_leans_on_nothing_but_the_four_bases() {
    let manifest = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .expect("读不出 Cargo.toml");
    let deps = manifest
        .split("[dependencies]")
        .nth(1)
        .expect("没有 [dependencies]");
    for line in deps.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        assert!(line.contains("path ="), "第三方依赖:{line}");
    }
}

#[test]
fn the_ledger_is_never_touched_by_hand() {
    let text = src("ledger.rs");
    for forbidden in ["std::fs::", "File::", "read_dir", "create_dir", "remove_"] {
        assert!(!text.contains(forbidden), "ledger.rs 里出现了 {forbidden}");
    }
}

#[test]
fn no_law_literal_lives_outside_the_bases() {
    let forbidden = [
        "\"zikaron/1\"",
        "\"zikaron.kit/1\"",
        "\"KIT_OK\"",
        "\"BADGE_OK\"",
        "\"genesis\"",
        "\"revocation\"",
        "\"succession\"",
        "\"annotation\"",
        "\"retraction\"",
    ];
    for (name, text) in sources() {
        for f in forbidden {
            assert!(!text.contains(f), "{name} 里写了法自己的字面 {f}");
        }
    }
}

#[test]
fn every_flag_a_verb_asks_for_lives_in_the_flag_table() {
    let text = src("verbs.rs");
    let mut asked: Vec<String> = Vec::new();
    for call in ["a.one(\"", "a.need(\"", "a.many(\"", "a.u64_of(\"", "a.key(\""] {
        let mut rest = text.as_str();
        while let Some(i) = rest.find(call) {
            rest = &rest[i + call.len()..];
            if let Some(j) = rest.find('"') {
                asked.push(rest[..j].to_string());
            }
        }
    }
    assert!(asked.len() > 30, "扫到的旗太少({}):扫法坏了", asked.len());
    for name in asked {
        assert!(
            zikaron_cli::verbs::is_known_flag(&name),
            "--{name} 不在旗名闭表里"
        );
    }
}

#[test]
fn the_verbs_are_counted_and_each_one_answers() {
    let names = zikaron_cli::verbs::VERBS;
    assert_eq!(names.len(), 32);
    let text = src("verbs_own.rs");
    for v in names {
        assert!(text.contains(&format!("\"{v}\" =>")), "{v} 没有分发的落点");
    }
    let mut sorted = names.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 32, "三十二个名字里有重的");
}

#[test]
fn the_exit_table_is_five_codes_and_they_are_all_different() {
    use zikaron_cli::codes::Exit;
    let codes: Vec<u8> = Exit::ALL.iter().map(|e| e.code()).collect();
    assert_eq!(codes, vec![0, 1, 2, 3, 4]);
    let mut uniq = codes.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), 5);
}

#[test]
fn the_output_key_table_and_the_reason_table_have_no_twin_spellings() {
    use zikaron_cli::codes::Reason;
    let mut reasons: Vec<&str> = Reason::ALL.iter().map(|r| r.as_str()).collect();
    let n = reasons.len();
    reasons.sort_unstable();
    reasons.dedup();
    assert_eq!(reasons.len(), n, "拒因表里有重名");
    for r in Reason::ALL {
        assert!(r.as_str().starts_with("E_"), "{} 不像一枚拒因", r.as_str());
    }
}

#[test]
fn the_entropy_well_is_read_by_length_and_has_no_fallback() {
    let text = src("entropy.rs");
    assert!(text.contains("zikaron_os::fill_random("), "熵只经 zikaron_os 那一处取");
    assert!(!text.contains("fs::read("), "整档读一个无尽的档会挂住");
    assert!(!text.to_lowercase().contains("fallback"));
    let os = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../zikaron-os/src/lib.rs")).expect("zikaron-os");
    assert!(os.contains("imp::fill_random(buf)"), "取熵在 zikaron-os 里每个系统一份实现");
}

#[test]
fn not_one_of_the_twenty_verbs_passes_without_marking_the_trace() {
    for verb in zikaron_cli::verbs::VERBS {
        let w = scratch(&format!("mark-{verb}"));
        let trace = w.join("trace.log");
        let _ = Command::new(BIN)
            .arg(verb)
            .current_dir(&w)
            .env("ZIKARON_TRACE", &trace)
            .output()
            .expect("起不动");
        let marks = std::fs::read_to_string(&trace).unwrap_or_default();
        assert!(
            marks.lines().any(|l| l.trim() == "V1"),
            "verb {verb} ran but the trace has no command-line mark"
        );
        let _ = std::fs::remove_dir_all(&w);
    }
}

#[test]
fn a_selector_that_matched_nothing_is_a_refusal_not_an_empty_green_bundle() {
    let w = scratch("empty-kit");
    assert_eq!(
        run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code,
        0
    );
    let bogus = format!("0x{}", "99".repeat(32));
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "pack", "--entry", &bogus, "--note", ""]);
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert!(!w.join("pack").exists());
    let g = run_in(&w, &["show", "--ledger", "b", "--entry", &{
        let listing = std::fs::read_dir(w.0.join("b")).expect("列不动");
        let name = listing
            .filter_map(|x| x.ok())
            .map(|x| x.file_name().to_string_lossy().into_owned())
            .find(|n| n.ends_with(".entry"))
            .expect("账本里一枚也没有");
        name.trim_end_matches(".entry").to_string()
    }]);
    assert_eq!(g.code, 0);
    let id = String::from_utf8_lossy(&g.out)
        .split("\"entryId\":\"")
        .nth(1)
        .and_then(|x| x.split('"').next())
        .expect("没有 entryId")
        .to_string();
    for form in [id.clone(), id.trim_start_matches("0x").to_string()] {
        let out_dir = format!("k-{}", &form[..6]);
        let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", &out_dir, "--entry", &form, "--note", ""]);
        assert_eq!(r.code, 0, "{form} 该选得中:{}", String::from_utf8_lossy(&r.out));
        assert!(String::from_utf8_lossy(&r.out).contains("\"entries\":1"));
    }
}

#[test]
fn anchor_refuses_to_quietly_drop_the_endpoints_it_will_not_use() {
    let w = scratch("one-chain");
    let r = run_in(
        &w,
        &[
            "anchor", "--endpoint", "1=http://127.0.0.1:1", "--endpoint", "2=http://127.0.0.1:2",
            "--key", A_KEY, "--form", "registry", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3", "--hash", &format!("0x{}", "11".repeat(32)),
        ],
    );
    assert_eq!(r.code, 2, "两条链的端点该是误用(同一条链的是它的节点表)");
    assert!(r.out.is_empty());
}

#[test]
fn a_refused_kit_export_answers_in_the_shape_the_cli_schema_names() {
    let w = scratch("kit-shape");
    assert_eq!(
        run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code,
        0
    );
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "p1", "--root", "notahex", "--note", ""]);
    assert_eq!(r.code, 1);
    let t = String::from_utf8_lossy(&r.out).into_owned();
    assert!(t.contains("\"reason\":\"E_KIT\""), "{t}");
    assert!(t.contains("\"state\":\"E_KIT_"), "state 该载 kit 法的 verdict:{t}");
    assert_eq!(run_in(&w, &["kit-export", "--ledger", "b", "--out", "p2", "--note", ""]).code, 0);
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "p2", "--note", ""]);
    assert_eq!(r.code, 1);
    let t = String::from_utf8_lossy(&r.out).into_owned();
    assert!(t.contains("\"reason\":\"E_LEDGER\"") && t.contains("\"path\":\"p2\""), "{t}");
    let bogus = format!("0x{}", "99".repeat(32));
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "p3", "--entry", &bogus, "--note", ""]);
    assert_eq!(r.code, 1);
    let t = String::from_utf8_lossy(&r.out).into_owned();
    assert!(t.contains("\"reason\":\"E_ENTRY_ABSENT\"") && t.contains(&bogus), "{t}");
    let up = format!("0x{}", "AB".repeat(32));
    for verb in [
        vec!["kit-export", "--ledger", "b", "--out", "p4", "--entry", &up, "--note", ""],
        vec!["show", "--ledger", "b", "--entry", &up],
    ] {
        let r = run_in(&w, &verb);
        assert_eq!(r.code, 2, "{verb:?} 该是误用");
        assert!(r.out.is_empty());
    }
}

#[test]
fn landing_a_document_never_clobbers_and_never_leaves_a_short_one() {
    let w = scratch("landing");
    assert_eq!(run_in(&w, &["init", "--ledger", "book", "--key", A_KEY, "--statement", "开端"]).code, 0);
    let sign = |out: &str| -> Ran { run_in(&w, &["audit", "--ledger", "book", "--out", out]) };

    let r = sign("fpm.json");
    assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.out));
    let landed = std::fs::read(w.join("fpm.json")).expect("落下去的档读不出");
    let v = zikaron::json::parse(&landed).expect("落下去的不是一枚 JSON");
    for k in ["anchors", "basis", "pile", "root"] {
        assert!(v.member(k).is_some(), "落下去的输入少了 {k}");
    }

    let before = std::fs::read(w.join("fpm.json")).expect("读不出");
    let again = sign("fpm.json");
    assert_eq!(again.code, 1);
    let t = String::from_utf8_lossy(&again.out).into_owned();
    assert!(t.contains("E_OCCUPIED") && t.contains("fpm.json"), "{t}");
    assert_eq!(before, std::fs::read(w.join("fpm.json")).expect("读不出"), "原有的档被动了");

    #[cfg(unix)]
    {
    use std::os::unix::fs::PermissionsExt;
    let locked = w.join("locked");
    std::fs::create_dir_all(&locked).expect("建不出");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).expect("改不了权限");
    let r = sign("locked/fpm.json");
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert!(!locked.join("fpm.json").exists(), "落不下去却留下了一份");
    let said = String::from_utf8_lossy(&r.out).into_owned();
    assert!(said.contains("\"detail\":\"E_IO: ") && said.contains("locked"), "盘错的 detail 带码与系统原话:{said}");
    let leftovers: Vec<String> = std::fs::read_dir(&locked)
        .expect("列不动")
        .filter_map(|x| x.ok())
        .map(|x| x.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(leftovers.is_empty(), "临时地留在盘上:{leftovers:?}");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).ok();
    }
}

#[test]
fn the_shell_holds_no_bare_write_to_a_path_the_caller_gave() {
    for (name, text) in sources() {
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!code.contains("fs::write("), "{name} 里有一句裸 fs::write");
        assert!(!code.contains("fs::rename("), "{name} 里有一句裸 fs::rename");
        assert!(!code.contains("File::create("), "{name} 里有一句裸 File::create");
    }
}

#[test]
fn stdout_has_one_writer_and_that_writer_never_returns() {
    for (name, text) in sources() {
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        if name == "out.rs" {
            continue;
        }
        for w in ["println!", "print!", "stdout()", "process::exit"] {
            assert!(!code.contains(w), "{name} 里有一处 {w}:印字与退出只许住 out.rs");
        }
    }
    let out_rs = src("out.rs");
    assert!(out_rs.contains("pub fn emit(a: Answer) -> !"), "emit 不再是不返回的:两个出口就不互斥了");
    assert!(out_rs.contains("pub fn misuse(reason: Reason, subject: Subject, said: Said) -> !"));
    assert!(out_rs.contains("pub fn frame(v: &Value)") && out_rs.contains("pub fn finish() -> !"), "mcp 的分帧写口与收尾");
    assert_eq!(out_rs.matches("std::process::exit").count(), 3, "out.rs 里的退出口不是三处");
    for (name, text) in sources() {
        if name == "out.rs" || name == "mcp.rs" {
            continue;
        }
        assert!(!text.contains("out::frame") && !text.contains("out::finish"), "{name} 调了只许 mcp 调的写口");
    }
    for (name, text) in sources() {
        if name == "out.rs" || name == "codes.rs" {
            continue;
        }
        assert!(!text.contains("Exit::Misuse"), "{name} 里出现了 Exit::Misuse");
    }
}

#[test]
fn not_one_path_that_exits_two_writes_a_byte_to_stdout() {
    let w = scratch("misuse-all");
    assert_eq!(
        run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code,
        0
    );
    let h32 = format!("0x{}", "aa".repeat(32));
    let h20 = format!("0x{}", "11".repeat(20));
    let bad_key = "0xzz";
    let out_of_range = format!("0x{}", "00".repeat(32));
    let cases: Vec<Vec<&str>> = vec![
        vec!["conjure"],
        vec!["keygen", "--seed", "1"],
        vec!["init", "--ledger", "b", "--key", bad_key, "--statement", "x"],
        vec!["init", "--ledger", "b", "--key", &out_of_range, "--statement", "x"],
        vec!["init", "--ledger", "b", "--statement", "x"],
        vec!["init", "--ledger", "b", "--key", A_KEY, "--key", A_KEY, "--statement", "x"],
        vec!["history", "--ledger", "b", "--key", bad_key],
        vec!["history", "--ledger", "b", "--key", A_KEY, "--seq", "1"],
        vec!["grant", "--ledger", "b", "--key", A_KEY, "--nope", "1"],
        vec!["revoke", "--ledger", "b", "--key", bad_key],
        vec!["adopt", "--ledger", "b", "--key", A_KEY, "--anchors", "/nowhere"],
        vec!["attest", "--key", bad_key, "--author", &h20, "--anchors", "x", "--prev", &h32],
        vec!["attest", "--key", A_KEY, "--author", &h20, "--anchors", "/nowhere", "--prev", &h32],
        vec!["annotate", "--ledger", "b", "--key", A_KEY, "--seq", "1"],
        vec!["retract", "--ledger", "b", "--key", A_KEY, "--seq", "1"],
        vec!["retract", "--ledger", "b", "--key", A_KEY, "--subject", "0x12", "--prev", "0x12"],
        vec!["anchor", "--endpoint", "1=http://127.0.0.1:1", "--key", bad_key, "--form", "registry", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3", "--hash", &h32],
        vec!["anchor", "--endpoint", "1=http://127.0.0.1:1", "--key", A_KEY, "--form", "gossip", "--hash", &h32],
        vec!["anchor", "--endpoint", "nonsense", "--key", A_KEY, "--form", "registry", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3", "--hash", &h32],
        vec!["anchor", "--endpoint", "1=http://a", "--endpoint", "2=http://b", "--key", A_KEY, "--form", "registry", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3", "--hash", &h32],
        vec!["scan", "--basis", "x"],
        vec!["scan", "--fixture", "/nowhere", "--endpoint", "1=http://127.0.0.1:1"],
        vec!["audit"],
        vec!["check-grant", "--ledger", "b"],
        vec!["chain-check", "--hop", "/nowhere"],
        vec!["depth", "--ledger", "b"],
        vec!["fpm-sign", "--key", bad_key, "--work", &h32, "--note", ""],
        vec!["fpm-sign", "--key", A_KEY, "--work", &h32, "--rows", "/nowhere", "--note", ""],
        vec!["ack-sign", "--key", bad_key, "--note", ""],
        vec!["ack-sign", "--key", A_KEY, "--fpm", &h32, "--fpm-doc", "/nowhere", "--note", ""],
        vec!["badge"],
        vec!["badge", "--encode", "/nowhere", "--decode", "/nowhere"],
        vec!["kit-export", "--ledger", "b"],
        vec!["kit-export", "--ledger", "b", "--out", "p", "--entry", "0xZZ", "--note", ""],
        vec!["show", "--ledger", "b"],
        vec!["show", "--ledger", "b", "--entry", "zz"],
        vec!["contract", "--ledger", "b"],
    ];
    let mut two = 0usize;
    for c in &cases {
        let r = run_in(&w, c);
        if r.code != 2 {
            continue;
        }
        two += 1;
        assert!(r.out.is_empty(), "{c:?} 退 2 而 stdout 印了 {} 字节", r.out.len());
    }
    assert!(two >= 30, "只走到 {two} 条误用路,表太瘦了");
}

#[test]
fn the_flag_names_the_code_knows_are_each_named_once() {
    let mut real: Vec<String> = zikaron_cli::verbs::all_flag_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let m = real.len();
    real.sort();
    real.dedup();
    assert_eq!(real.len(), m, "码里的旗名闭表有重名");
}

fn files_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap_or_default()))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn member_of(r: &Ran, key: &str) -> Option<zikaron::json::Value> {
    match zikaron::json::parse(&r.out).ok()? {
        zikaron::json::Value::Obj(m) => m.into_iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn reason_of(r: &Ran) -> String {
    match member_of(r, "reason") {
        Some(zikaron::json::Value::Str(x)) => x,
        _ => String::new(),
    }
}

#[test]
fn a_second_init_on_a_rooted_ledger_is_refused_and_not_one_byte_moves() {
    let w = scratch("init-twice");
    let first = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(first.code, 0, "空目录上建档该绿:{}", String::from_utf8_lossy(&first.out));
    let before = files_of(&w.join("b"));
    assert_eq!(before.len(), 1, "建档落一枚");
    let second = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "x"]);
    assert_eq!(second.code, 1, "二次创世该拒:{}", String::from_utf8_lossy(&second.out));
    assert_eq!(reason_of(&second), "E_ALREADY_ROOTED");
    assert!(member_of(&second, "written").is_none(), "拒的答里不该有 written");
    assert!(matches!(member_of(&second, "author"), Some(zikaron::json::Value::Str(ref a)) if a.starts_with("0x") && a.len() == 42), "拒因带根的作者");
    assert!(matches!(member_of(&second, "count"), Some(zikaron::json::Value::Int(1))), "目录里一件");
    assert_eq!(files_of(&w.join("b")), before, "拒之后目录的档名集与字节都该与拒之前相同");
    let same = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(same.code, 1);
    assert_eq!(reason_of(&same), "E_ALREADY_ROOTED");
    assert_eq!(files_of(&w.join("b")), before);
    let other = "0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba";
    let third = run_in(&w, &["init", "--ledger", "b", "--key", other, "--statement", "另一个开端"]);
    assert_eq!(third.code, 1);
    assert_eq!(reason_of(&third), "E_ALREADY_ROOTED");
    assert_eq!(files_of(&w.join("b")), before, "换钥再来也一份不落");
}

#[test]
fn init_on_a_ledger_holding_bytes_it_cannot_read_as_a_root_is_refused() {
    let w = scratch("init-stray");
    std::fs::create_dir_all(w.join("b")).expect("建目录");
    std::fs::write(w.join("b").join("notes.txt"), b"not an entry").expect("写散档");
    let before = files_of(&w.join("b"));
    let r = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(r.code, 1, "有读不成的散档该拒:{}", String::from_utf8_lossy(&r.out));
    assert_eq!(reason_of(&r), "E_NOT_EMPTY");
    assert!(matches!(member_of(&r, "names"), Some(zikaron::json::Value::Arr(ref n)) if n.len() == 1), "跳过的那一件具名");
    assert!(matches!(member_of(&r, "count"), Some(zikaron::json::Value::Int(1))));
    assert_eq!(files_of(&w.join("b")), before, "零条目落地");

    let w2 = scratch("init-torn");
    std::fs::create_dir_all(w2.join("b")).expect("建目录");
    std::fs::write(w2.join("b").join(format!("{}.entry", "ab".repeat(32))), b"{\"torn\":").expect("写坏条目档");
    let before2 = files_of(&w2.join("b"));
    let r2 = run_in(&w2, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(r2.code, 1, "名合条目而核拒的字节该拒:{}", String::from_utf8_lossy(&r2.out));
    assert_eq!(reason_of(&r2), "E_NOT_EMPTY");
    assert_eq!(files_of(&w2.join("b")), before2, "零条目落地");
}

#[test]
fn init_on_an_empty_directory_still_writes_the_root() {
    let w = scratch("init-empty");
    std::fs::create_dir_all(w.join("b")).expect("建空目录");
    let r = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(r.code, 0, "空目录该绿:{}", String::from_utf8_lossy(&r.out));
    assert!(String::from_utf8_lossy(&r.out).contains("\"written\":true"));
    assert_eq!(files_of(&w.join("b")).len(), 1);
    let r2 = run_in(&w, &["init", "--ledger", "c", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(r2.code, 0, "目录不在也该绿");
    assert_eq!(files_of(&w.join("c")).len(), 1);
}

#[test]
fn init_on_a_ledger_that_already_holds_two_roots_names_the_fork() {
    let w = scratch("init-fork");
    assert_eq!(run_in(&w, &["init", "--ledger", "one", "--key", A_KEY, "--statement", "开端"]).code, 0);
    assert_eq!(run_in(&w, &["init", "--ledger", "two", "--key", A_KEY, "--statement", "x"]).code, 0);
    std::fs::create_dir_all(w.join("b")).expect("建目录");
    for from in ["one", "two"] {
        for (name, bytes) in files_of(&w.join(from)) {
            std::fs::write(w.join("b").join(name), bytes).expect("拷条目档");
        }
    }
    let before = files_of(&w.join("b"));
    assert_eq!(before.len(), 2, "两枚创世");
    let r = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "第三句"]);
    assert_eq!(r.code, 1, "两个根的账本该拒:{}", String::from_utf8_lossy(&r.out));
    assert_eq!(reason_of(&r), "E_TIP_FORKED");
    assert!(matches!(member_of(&r, "count"), Some(zikaron::json::Value::Int(2))));
    assert!(member_of(&r, "author").is_none(), "几个根时不挑一位作者");
    assert_eq!(files_of(&w.join("b")), before, "零条目落地");
}

#[test]
fn a_write_by_hand_into_a_ledger_that_already_holds_two_roots_is_refused() {
    let w = scratch("write-fork");
    let other_key = format!("0x{}", "17".repeat(32));
    let one = run_in(&w, &["init", "--ledger", "one", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(one.code, 0);
    assert_eq!(run_in(&w, &["init", "--ledger", "two", "--key", &other_key, "--statement", "x"]).code, 0);
    std::fs::create_dir_all(w.join("b")).expect("建目录");
    for from in ["one", "two"] {
        for (name, bytes) in files_of(&w.join(from)) {
            std::fs::write(w.join("b").join(name), bytes).expect("拷条目档");
        }
    }
    let before = files_of(&w.join("b"));
    let h32 = format!("0x{}", "ab".repeat(32));
    let root = text_of(&one, "entryId");
    let r = run_in(&w, &["history", "--ledger", "b", "--key", A_KEY, "--seq", "1", "--prev", &root, "--content", &h32, "--mark", "v1", "--toolchain", &h32]);
    assert_eq!(r.code, 1, "两个根的账本该拒:{}", String::from_utf8_lossy(&r.out));
    assert_eq!(reason_of(&r), "E_TIP_FORKED");
    assert_eq!(files_of(&w.join("b")), before, "零条目落地");
    let author = files_of(&w.join("one")).into_iter().find_map(|(_, b)| zikaron_glue::book::check(&b).ok()).map(|e| e.author).expect("创世作者");
    let named = run_in(&w, &["history", "--ledger", "b", "--key", A_KEY, "--root", &author, "--content", &h32, "--mark", "v1", "--toolchain", &h32]);
    assert_eq!(named.code, 0, "点名根之后照写:{}", String::from_utf8_lossy(&named.out));
}

#[test]
fn one_genesis_stored_under_two_names_is_still_one_root() {
    let w = scratch("init-dup");
    assert_eq!(run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code, 0);
    let (name, bytes) = files_of(&w.join("b")).into_iter().next().expect("一枚");
    let other = format!("{}.entry", "cd".repeat(32));
    assert_ne!(name, other);
    std::fs::write(w.join("b").join(&other), &bytes).expect("拷成另一个名");
    let before = files_of(&w.join("b"));
    assert_eq!(before.len(), 2);
    let r = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "x"]);
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert_eq!(reason_of(&r), "E_ALREADY_ROOTED");
    assert!(matches!(member_of(&r, "count"), Some(zikaron::json::Value::Int(2))), "目录里两件");
    assert_eq!(files_of(&w.join("b")), before, "零条目落地");
}

fn text_of(r: &Ran, key: &str) -> String {
    match member_of(r, key) {
        Some(zikaron::json::Value::Str(x)) => x,
        _ => String::new(),
    }
}

fn a_book_with_one_work(w: &Path) -> (String, String) {
    let g = run_in(w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(g.code, 0);
    let h32 = format!("0x{}", "ab".repeat(32));
    let h = run_in(w, &["history", "--ledger", "b", "--key", A_KEY, "--content", &h32, "--mark", "v1", "--toolchain", &h32]);
    assert_eq!(h.code, 0, "{}", String::from_utf8_lossy(&h.out));
    (text_of(&g, "entryId"), text_of(&h, "entryId"))
}

#[test]
fn a_retraction_lands_in_the_convention_shape_and_the_audit_lists_it_without_moving_the_label() {
    let w = scratch("retract-green");
    let (_, work) = a_book_with_one_work(&w);
    let before = run_in(&w, &["audit", "--ledger", "b"]);
    let r = run_in(&w, &["retract", "--ledger", "b", "--key", A_KEY, "--subject", &work, "--note", "写错了"]);
    assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.out));
    let id = text_of(&r, "entryId");
    let shown = run_in(&w, &["show", "--ledger", "b", "--entry", id.trim_start_matches("0x")]);
    assert_eq!(text_of(&shown, "entryType"), zikaron_glue::retraction::ENTRY_TYPE);
    let after = run_in(&w, &["audit", "--ledger", "b"]);
    assert_eq!(after.code, before.code, "删掉一条存证不改审计的码");
    assert_eq!(member_of(&after, "label"), member_of(&before, "label"), "标签不因约定条目而变");
    let count = |r: &Ran| match member_of(r, "entries") {
        Some(zikaron::json::Value::Int(n)) => n,
        other => panic!("entries is a count: {other:?}"),
    };
    assert_eq!(count(&after), count(&before) + 1, "审计数得到这一条");
    let unknown = format!("{:?}", member_of(&after, "unknown_type"));
    assert!(!unknown.contains(id.trim_start_matches("0x")), "法内类型不入 unknown_type:{unknown}");
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn a_retraction_the_convention_forbids_is_refused_by_name_and_not_one_byte_moves() {
    use zikaron_glue::retraction::Invalid;
    let w = scratch("retract-red");
    let (genesis, work) = a_book_with_one_work(&w);
    assert_eq!(run_in(&w, &["retract", "--ledger", "b", "--key", A_KEY, "--subject", &work]).code, 0);
    let absent = format!("0x{}", "cd".repeat(32));
    let cases: Vec<(Vec<&str>, Invalid)> = vec![
        (vec!["retract", "--ledger", "b", "--key", A_KEY, "--subject", &work], Invalid::Repeated),
        (vec!["retract", "--ledger", "b", "--key", A_KEY, "--subject", &genesis], Invalid::NotAWork),
        (vec!["retract", "--ledger", "b", "--key", A_KEY, "--subject", &absent], Invalid::NotInLedger),
        (vec!["retract", "--ledger", "b", "--key", A_KEY, "--subject", "0x12"], Invalid::Shape),
        (vec!["retract", "--ledger", "b", "--key", A_KEY], Invalid::Shape),
    ];
    for (argv, why) in cases {
        let before = files_of(&w.join("b"));
        let r = run_in(&w, &argv);
        assert_eq!(r.code, 1, "{argv:?}");
        assert_eq!(reason_of(&r), "E_RETRACTION", "{argv:?}");
        assert_eq!(text_of(&r, "token"), why.token(), "{argv:?}");
        assert_eq!(files_of(&w.join("b")), before, "{argv:?} 拒了而账本动了");
    }
    let _ = std::fs::remove_dir_all(&w);
}

#[cfg(unix)]
#[test]
fn a_disk_failure_says_the_systems_words_in_detail() {
    use std::os::unix::fs::PermissionsExt;
    let w = scratch("p5-io");
    let locked = w.join("locked");
    std::fs::create_dir_all(&locked).expect("建不出");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).expect("改不了权限");
    let r = run_in(&w, &["init", "--ledger", "locked/b", "--key", A_KEY, "--statement", "开端"]);
    let said = String::from_utf8_lossy(&r.out).into_owned();
    assert_eq!(r.code, 1, "{said}");
    assert!(said.contains("\"reason\":\"E_LEDGER\"") && said.contains("\"detail\":\"E_IO: ") && said.contains("locked/b"), "{said}");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).expect("改不了权限");
    assert_eq!(run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code, 0);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).expect("改不了权限");
    let k = run_in(&w, &["kit-export", "--ledger", "b", "--out", "locked/pack", "--note", ""]);
    let said = String::from_utf8_lossy(&k.out).into_owned();
    assert_eq!(k.code, 1, "{said}");
    assert!(said.contains("\"detail\":\"E_IO: ") && said.contains("\"path\":"), "{said}");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).ok();
}

#[test]
fn a_refused_body_is_said_in_the_systems_language_flags_and_all() {
    let w = scratch("bodyline");
    assert_eq!(run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "start"]).code, 0);
    let said = |lang: &str| -> (i32, String, String) {
        let o = Command::new(BIN)
            .args(["history", "--ledger", "b", "--key", A_KEY, "--content", &format!("0x{}", "aa".repeat(32))])
            .current_dir(&w)
            .env(zikaron_os::HOME_VAR, own_home())
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES")
            .env_remove("LANGUAGE")
            .env("LANG", lang)
            .output()
            .expect("runs");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or_default().to_string())
    };
    let (en_code, en_out, en) = said("en_US.UTF-8");
    let (zh_code, zh_out, zh) = said("zh_CN.UTF-8");
    assert_eq!((en_code, zh_code), (1, 1));
    assert_eq!(en_out, zh_out, "stdout is the same in either language");
    assert_eq!(en, "mode: missing or malformed; given by --mark and --toolchain (or --file)");
    assert!(!en.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "no Chinese on an English system: {en}");
    assert_eq!(zh, "mode: 缺或不成形,由 --mark 与 --toolchain(或 --file) 给");
}

#[test]
fn the_repeated_flags_are_the_ones_read_many_times() {
    let mut read: Vec<String> = Vec::new();
    for text in [src("verbs.rs"), src("desk.rs"), src("hopin.rs")] {
        for head in ["many(\"", "many_at(\""] {
            for piece in text.split(head).skip(1) {
                if let Some(name) = piece.split('"').next() {
                    read.push(name.to_string());
                }
            }
        }
    }
    read.sort();
    read.dedup();
    let mut table: Vec<String> = zikaron_cli::verbs::REPEATED.iter().map(|(_, f)| f.to_string()).collect();
    table.sort();
    table.dedup();
    assert_eq!(read, table, "flags read many times against the table of repeated flags");
}

fn an_old_ledger(dir: &Path, closed: bool) -> Vec<String> {
    use zikaron_glue::book::{self, Law};
    let key = zikaron::hexfmt::scalar32(A_KEY).expect("a key");
    let author = zikaron_cli::entry::address(&key).expect("an address");
    std::fs::create_dir_all(dir).expect("建目录");
    let o = |m: Vec<(&str, zikaron::json::Value)>| zikaron::json::Value::Obj(m.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    let st = |x: &str| zikaron::json::Value::Str(x.to_string());
    let h32 = |b: &str| format!("0x{}", b.repeat(32));
    let mut ids: Vec<String> = Vec::new();
    let mut lay = |kind: &str, body: zikaron::json::Value| {
        let seq = ids.len() as u64;
        let e = zikaron_cli::entry::seal_under(Law::One, &author, kind, seq, ids.last().map(String::as_str), body, &key).expect("sealed");
        std::fs::write(dir.join(format!("{}.entry", e.entry.id_hex().trim_start_matches("0x"))), &e.bytes).expect("laid");
        ids.push(e.entry.id_hex());
    };
    lay("genesis", o(vec![("statement_md", st("旧法的开端"))]));
    lay("history", o(vec![("content", st(&h32("0a"))), ("mode", o(vec![("mark", st("v1")), ("toolchain", st(&h32("0b")))]))]));
    if closed {
        lay("succession", book::closing_body(&author, 1, "迁至新法"));
    }
    ids
}

#[test]
fn an_old_ledger_given_as_an_input_is_read_by_that_law() {
    let w = scratch("old-input");
    let ids = an_old_ledger(&w.join("old"), false);
    let by_folder = run_in(&w, &["audit", "--ledger", "old", "--out", "in.json"]);
    let by_input = run_in(&w, &["audit", "--input", "in.json"]);
    let said = String::from_utf8_lossy(&by_input.out).to_string();
    assert_eq!(by_input.out, by_folder.out, "{said}");
    assert_eq!(by_input.code, by_folder.code, "{said}");
    assert!(said.contains(ids.last().expect("a tip")), "{said}");
    std::fs::write(w.join("bad.json"), r#"{"anchors":[],"basis":{"chains":[]},"evidence":[1]}"#).expect("w");
    let r = run_in(&w, &["audit", "--ledger", "old", "--fragment", "bad.json"]);
    let said = String::from_utf8_lossy(&r.out).to_string();
    assert_eq!(r.code, 1, "{said}");
    assert!(said.contains("NO_LABEL"), "{said}");
}

#[test]
fn an_original_asked_for_and_not_listed_is_named() {
    let w = scratch("gp-plain-original");
    let h32 = |b: &str| format!("0x{}", b.repeat(32));
    let (work, terms) = (h32("0a"), h32("0b"));
    assert_eq!(run_in(&w, &["init", "--ledger", "l", "--key", A_KEY, "--statement", "x"]).code, 0);
    let id = old_grant::old_grant(&w.0.join("l"), A_KEY, &format!("0x{}", "33".repeat(20)), &work, &terms);
    let asked = run_in(&w, &["grant-package", "--ledger", "l", "--grant", &id, "--out", "with", "--originals", "all"]);
    let said = String::from_utf8_lossy(&asked.out).to_string();
    assert_eq!(asked.code, 0, "{said}");
    assert!(said.contains(&format!("\"leftOut\":[\"{work}\"]")), "{said}");
    let none = run_in(&w, &["grant-package", "--ledger", "l", "--grant", &id, "--out", "without"]);
    let said = String::from_utf8_lossy(&none.out).to_string();
    assert!(said.contains("\"leftOut\":[]"), "not asked, nothing left out: {said}");
}

#[test]
fn a_strangers_new_genesis_leaves_an_old_ledger_old() {
    let w = scratch("old-stranger");
    let ids = an_old_ledger(&w.join("old"), false);
    let key = zikaron::hexfmt::scalar32(A_KEY).expect("a key");
    let root = zikaron_cli::entry::address(&key).expect("an address");
    let stranger = format!("0x{}", "7".repeat(64));
    for (dir, by) in [("stranger", stranger.as_str()), ("own", A_KEY)] {
        let r = run_in(&w, &["init", "--ledger", dir, "--key", by, "--statement", "x"]);
        assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.out));
    }
    let copy_in = |from: &str, to: &str| {
        std::fs::create_dir_all(w.join(to)).expect("a folder");
        for e in std::fs::read_dir(w.join(from)).expect("reads").filter_map(Result::ok) {
            std::fs::copy(e.path(), w.join(to).join(e.file_name())).expect("copied");
        }
    };
    copy_in("old", "old-and-stranger");
    copy_in("stranger", "old-and-stranger");
    let mixed = run_in(&w, &["audit", "--ledger", "old-and-stranger", "--root", &root]);
    let alone = run_in(&w, &["audit", "--ledger", "old", "--root", &root]);
    let said = String::from_utf8_lossy(&mixed.out).to_string();
    assert_eq!(mixed.code, alone.code, "{said}");
    assert!(said.contains(&format!("\"entries\":{}", ids.len())), "the old ledger's entries read: {said}");
    assert!(said.contains(&format!("\"label\":\"{}\"", label_of(&alone.out))), "{said}");
    copy_in("old", "old-and-own");
    copy_in("own", "old-and-own");
    let own = run_in(&w, &["audit", "--ledger", "old-and-own", "--root", &root]);
    let said = String::from_utf8_lossy(&own.out).to_string();
    assert!(said.contains("\"entries\":1"), "its own new genesis is the new law's: {said}");
}

fn label_of(report: &[u8]) -> String {
    let v = zikaron::json::parse(report).expect("a report");
    v.member("label").and_then(zikaron::json::Value::as_str).expect("a label").to_string()
}

#[test]
fn an_old_reading_of_an_old_ledger_is_read_by_that_law() {
    let w = scratch("old-reading");
    let ids = an_old_ledger(&w.join("old"), false);
    let key = zikaron::hexfmt::scalar32(A_KEY).expect("a key");
    let author = zikaron_cli::entry::address(&key).expect("an address");
    let (tip, registry) = (ids.last().expect("a tip").clone(), format!("0x{}", "12".repeat(20)));
    let reading = |extra: &str| {
        format!(
            r#"{{"anchors":[{{"blockNumber":5,"blockTimestamp":1000,"chainId":31337,"hash":"{tip}","sender":"{author}","tx":"0x{tx}","verdict":"counted"}}],"basis":{{"adoptionChains":[],"bareTx":[],"chains":[{{"chainId":31337,"fromBlock":0,"registries":["{registry}"],"senders":["{author}"],"toBlock":10}}]{extra}}},"evidence":[]}}"#,
            tx = "ab".repeat(32)
        )
    };
    std::fs::write(w.join("old.json"), reading("")).expect("w");
    let r = run_in(&w, &["audit", "--ledger", "old", "--fragment", "old.json"]);
    let said = String::from_utf8_lossy(&r.out).to_string();
    assert_eq!(r.code, 0, "{said}");
    assert!(said.contains("\"label\":\"COMPLETE\"") && said.contains(&tip), "{said}");
    std::fs::write(w.join("bad.json"), reading(r#","stray":[]"#)).expect("w");
    let r = run_in(&w, &["audit", "--ledger", "old", "--fragment", "bad.json"]);
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert!(String::from_utf8_lossy(&r.out).contains("NO_LABEL"), "{}", String::from_utf8_lossy(&r.out));
    assert_eq!(run_in(&w, &["init", "--ledger", "new", "--key", A_KEY, "--statement", "s"]).code, 0);
    let r = run_in(&w, &["audit", "--ledger", "new", "--fragment", "old.json"]);
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert!(String::from_utf8_lossy(&r.out).contains("NO_LABEL"), "{}", String::from_utf8_lossy(&r.out));
}

#[test]
fn an_old_ledger_takes_no_new_entry_until_it_is_moved_and_moves_once() {
    let w = scratch("migrate");
    let ids = an_old_ledger(&w.join("old"), false);
    let h32 = format!("0x{}", "ab".repeat(32));
    let before = files_of(&w.join("old"));
    let r = run_in(&w, &["history", "--ledger", "old", "--key", A_KEY, "--content", &h32, "--mark", "v1", "--toolchain", &h32]);
    assert_eq!((r.code, reason_of(&r)), (1, "E_NOT_MOVED".to_string()), "{}", String::from_utf8_lossy(&r.out));
    assert_eq!(files_of(&w.join("old")), before, "零条目落地");
    let m = run_in(&w, &["migrate", "--ledger", "old", "--key", A_KEY, "--effective", "1"]);
    assert_eq!(m.code, 0, "{}", String::from_utf8_lossy(&m.out));
    assert_eq!(files_of(&w.join("old")).len(), ids.len() + 2);
    let closing = text_of(&m, "closing");
    let continuing = text_of(&m, "continuing");
    let shown = run_in(&w, &["show", "--ledger", "old", "--entry", continuing.trim_start_matches("0x")]);
    let body = String::from_utf8_lossy(&shown.out).into_owned();
    assert!(body.contains(&closing) && body.contains("continues"), "the genesis continues the closing succession: {body}");
    let again = run_in(&w, &["migrate", "--ledger", "old", "--key", A_KEY]);
    assert_eq!((again.code, reason_of(&again)), (1, "E_ALREADY_MOVED".to_string()));
    let after = run_in(&w, &["history", "--ledger", "old", "--key", A_KEY, "--content", &h32, "--mark", "v1", "--toolchain", &h32]);
    assert_eq!(after.code, 0, "{}", String::from_utf8_lossy(&after.out));
    assert!(matches!(member_of(&after, "seq"), Some(zikaron::json::Value::Int(1))));
}

#[test]
fn a_move_cut_after_its_closing_succession_writes_the_genesis_alone() {
    let w = scratch("migrate-resume");
    let ids = an_old_ledger(&w.join("old"), true);
    let m = run_in(&w, &["migrate", "--ledger", "old", "--key", A_KEY, "--effective", "1"]);
    assert_eq!(m.code, 0, "{}", String::from_utf8_lossy(&m.out));
    assert_eq!(text_of(&m, "closing"), ids[2], "the closing succession already there is the one continued");
    assert_eq!(files_of(&w.join("old")).len(), ids.len() + 1, "one entry more: the genesis");
}

#[test]
fn an_old_archive_is_audited_by_the_law_it_was_written_in() {
    let w = scratch("audit-old-kit");
    let ids = an_old_ledger(&w.join("old"), true);
    let entries: Vec<Vec<u8>> = ids.iter().map(|id| std::fs::read(w.join("old").join(format!("{}.entry", id.trim_start_matches("0x")))).expect("an entry")).collect();
    let kit = zikaron_glue::pack::companion_bytes(&zikaron_glue::pack::Bundle { entries, ..Default::default() }).expect("a kit/1 package");
    std::fs::write(w.join("old.zkit"), kit).expect("the package");
    for place in ["old.zkit", "old"] {
        let r = run_in(&w, &["audit", "--ledger", place]);
        assert_eq!(r.code, 0, "{place}: {}", String::from_utf8_lossy(&r.out));
        assert!(matches!(member_of(&r, "entries"), Some(zikaron::json::Value::Int(3))), "{place}: three entries read: {}", String::from_utf8_lossy(&r.out));
        assert!(matches!(member_of(&r, "malformed"), Some(zikaron::json::Value::Arr(m)) if m.is_empty()), "{place}: none malformed");
        assert!(member_of(&r, "continues").is_none(), "{place}: the old law's report, which has no continuation");
        assert_eq!(text_of(&r, "label"), "COMPLETE");
    }
}

#[test]
fn init_on_a_continued_ledger_names_its_one_root() {
    let w = scratch("init-continued");
    an_old_ledger(&w.join("old"), false);
    let m = run_in(&w, &["migrate", "--ledger", "old", "--key", A_KEY, "--effective", "1"]);
    assert_eq!(m.code, 0, "{}", String::from_utf8_lossy(&m.out));
    let before = files_of(&w.join("old"));
    let r = run_in(&w, &["init", "--ledger", "old", "--key", A_KEY, "--statement", "再开一次"]);
    assert_eq!((r.code, reason_of(&r)), (1, "E_ALREADY_ROOTED".to_string()), "{}", String::from_utf8_lossy(&r.out));
    let author = zikaron_cli::entry::address(&zikaron::hexfmt::scalar32(A_KEY).expect("a key")).expect("an address");
    assert_eq!(text_of(&r, "author"), author);
    assert_eq!(files_of(&w.join("old")), before, "零条目落地");
    let other = "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a";
    assert_eq!(run_in(&w, &["init", "--ledger", "stranger", "--key", other, "--statement", "别人的开端"]).code, 0);
    for (name, bytes) in files_of(&w.join("stranger")) {
        std::fs::write(w.join("old").join(name), bytes).expect("拷条目档");
    }
    let before = files_of(&w.join("old"));
    let r = run_in(&w, &["init", "--ledger", "old", "--key", A_KEY, "--statement", "再开一次"]);
    assert_eq!((r.code, reason_of(&r)), (1, "E_TIP_FORKED".to_string()), "{}", String::from_utf8_lossy(&r.out));
    assert_eq!(files_of(&w.join("old")), before, "零条目落地");
}

#[test]
fn an_old_chain_is_checked_by_the_law_it_was_written_in() {
    use zikaron_glue::book::Law;
    let w = scratch("chain-old");
    let o = |m: Vec<(&str, zikaron::json::Value)>| zikaron::json::Value::Obj(m.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    let st = |x: &str| zikaron::json::Value::Str(x.to_string());
    let h32 = |b: &str| format!("0x{}", b.repeat(32));
    let lay = |dir: &Path, key: &str, entries: Vec<(&str, zikaron::json::Value)>| -> Vec<String> {
        let k = zikaron::hexfmt::scalar32(key).expect("a key");
        let author = zikaron_cli::entry::address(&k).expect("an address");
        std::fs::create_dir_all(dir).expect("建目录");
        let mut ids: Vec<String> = Vec::new();
        for (kind, body) in entries {
            let e = zikaron_cli::entry::seal_under(Law::One, &author, kind, ids.len() as u64, ids.last().map(String::as_str), body, &k).expect("sealed");
            std::fs::write(dir.join(format!("{}.entry", e.entry.id_hex().trim_start_matches("0x"))), &e.bytes).expect("laid");
            ids.push(e.entry.id_hex());
        }
        ids
    };
    let b_key = "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a";
    let b_addr = zikaron_cli::entry::address(&zikaron::hexfmt::scalar32(b_key).expect("a key")).expect("an address");
    let genesis = || ("genesis", o(vec![("statement_md", st("旧法的开端"))]));
    let a = lay(&w.join("a"), A_KEY, vec![genesis(), ("grant", o(vec![("grantee", st(&b_addr)), ("terms", st(&h32("71"))), ("work", st(&h32("44")))]))]);
    let b = lay(&w.join("b"), b_key, vec![genesis(), ("grant", o(vec![("grantee", st(&h32("cc")[..42])), ("terms", st(&h32("72"))), ("upstream", st(&a[1])), ("work", st(&h32("44")))]))]);
    let grant_file = |dir: &str, id: &str| {
        let p = w.join(format!("{dir}-grant.entry"));
        std::fs::copy(w.join(dir).join(format!("{}.entry", id.trim_start_matches("0x"))), &p).expect("the grant");
        p
    };
    let (ga, gb) = (grant_file("a", &a[1]), grant_file("b", &b[1]));
    std::fs::create_dir_all(w.join("a-revoked")).expect("建目录");
    for (name, bytes) in files_of(&w.join("a")) {
        std::fs::write(w.join("a-revoked").join(name), bytes).expect("拷条目档");
    }
    let k = zikaron::hexfmt::scalar32(A_KEY).expect("a key");
    let author = zikaron_cli::entry::address(&k).expect("an address");
    let rev = zikaron_cli::entry::seal_under(Law::One, &author, "revocation", 2, Some(&a[1]), o(vec![("grant", st(&a[1]))]), &k).expect("sealed");
    std::fs::write(w.join("a-revoked").join(format!("{}.entry", rev.entry.id_hex().trim_start_matches("0x"))), &rev.bytes).expect("laid");
    let input = |ledger: &str, name: &str| {
        let r = run_in(&w, &["audit", "--ledger", ledger, "--out", name]);
        assert!(w.join(name).is_file(), "{ledger}: {}", String::from_utf8_lossy(&r.out));
        name.to_string()
    };
    let (ia, ir, ib) = (input("a", "a.json"), input("a-revoked", "a-revoked.json"), input("b", "b.json"));
    let hop = |g: &Path, i: &str| format!("{}={}", g.display(), w.join(i).display());
    let r = run_in(&w, &["chain-check", "--hop", &hop(&ga, &ia), "--hop", &hop(&gb, &ib), "--now", "150"]);
    assert_ne!(text_of(&r, "verdict"), "FAIL", "{}", String::from_utf8_lossy(&r.out));
    let r = run_in(&w, &["chain-check", "--hop", &hop(&ga, &ir), "--hop", &hop(&gb, &ib), "--now", "150"]);
    assert_eq!((r.code, text_of(&r, "verdict")), (1, "FAIL".to_string()), "{}", String::from_utf8_lossy(&r.out));
    let failing = member_of(&r, "failing").expect("a failing point");
    assert_eq!(failing.member("kind").and_then(|x| x.as_str()), Some("hop"), "{}", String::from_utf8_lossy(&r.out));
    assert!(matches!(failing.member("index"), Some(zikaron::json::Value::Int(0))), "{}", String::from_utf8_lossy(&r.out));
}

#[test]
fn a_rest_keys_file_is_misuse_by_its_flag_and_its_key_is_never_said_back() {
    let w = scratch("rest-keys");
    let key = "SECRET-REST-KEY-5d1e";
    let dead = std::net::TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    let basis = w.0.join("basis.json");
    std::fs::write(&basis, format!("{{\"chains\":[{{\"chainId\":31337,\"fromBlock\":0,\"registries\":[\"0x{}\"],\"senders\":[\"0x{}\"],\"toBlock\":16}}]}}", "11".repeat(20), "22".repeat(20))).expect("a basis");
    let node = format!("31337=http://127.0.0.1:{dead}");
    let run = |keys: &Path| {
        let args = ["scan", "--endpoint", node.as_str(), "--basis", basis.to_str().expect("a path"), "--rest-keys", keys.to_str().expect("a path")];
        let o = Command::new(BIN).args(args).current_dir(&w.0).env(zikaron_os::HOME_VAR, own_home()).output().expect("起不动 zikaron");
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).to_string(), String::from_utf8_lossy(&o.stderr).to_string())
    };
    let bad = w.0.join("bad.keys");
    std::fs::write(&bad, format!("etherscan {key} more\n")).expect("a file");
    let (code, out, err) = run(&bad);
    assert_eq!(code, 2, "a line of another shape is misuse: {err}");
    assert!(out.is_empty(), "misuse writes nothing to stdout");
    assert!(err.lines().next().unwrap_or_default().contains("--rest-keys"), "named by its flag: {err}");
    assert!(!err.contains(key) && !err.contains("etherscan"), "the file's text is not said back: {err}");
    let good = w.0.join("good.keys");
    std::fs::write(&good, format!("# keys\n\netherscan {key}\n")).expect("a file");
    let (code, out, err) = run(&good);
    assert_ne!(code, 2, "a file that reads is taken: {err}");
    assert!(!out.contains(key) && !err.contains(key), "the key is never said back: {out} {err}");
}

#[test]
fn the_face_verbs_misuse_is_judged_before_anything_is_asked() {
    let w = scratch("face-misuse");
    let folder = w.join("plain");
    let r = run_in(&w, &["init", "--ledger", folder.to_str().unwrap_or_default(), "--key", A_KEY, "--statement", "s"]);
    assert_eq!(r.code, 0, "a plain folder");
    let f = folder.to_str().unwrap_or_default();
    let root = format!("0x{}", "11".repeat(20));
    for args in [
        vec!["face-table"],
        vec!["face-table", "--root", &root, "--ledger", f],
        vec!["face-table", "--root", "0x12"],
        vec!["face-table", "--root", &root],
        vec!["face-table", "--root", &root, "--endpoint", "999=http://127.0.0.1:9"],
        vec!["face-table", "--ledger", f],
        vec!["face-home", "--ledger", f, "--key", A_KEY, "--chain", "eleven", "--endpoint", "11155111=http://127.0.0.1:9"],
        vec!["face-home", "--ledger", f, "--key", A_KEY, "--chain", "9007199254740992", "--endpoint", "11155111=http://127.0.0.1:9"],
        vec!["face-name", "--ledger", f, "--key", A_KEY, "--name", "名字"],
        vec!["face-key", "--ledger", f, "--address", &root, "--role", "payout", "--endpoint", "11155111=http://127.0.0.1:9"],
    ] {
        let r = run_in(&w, &args);
        assert_eq!(r.code, 2, "{args:?} is misuse");
        assert!(r.out.is_empty(), "{args:?}: misuse writes nothing to stdout");
    }
}
