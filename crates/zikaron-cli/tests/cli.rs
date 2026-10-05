//! Checks of the command line: real binaries for behavior (exit codes, stdout bytes, trace marks) and source
//! scans for discipline (no third-party crates, no direct ledger writes, a closed flag list, law literals in
//! one place).
//!
//! Real runs use `CARGO_BIN_EXE_zikaron`, the binary cargo builds for this package, so the tests always drive
//! the product's own entry point.

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

/// A test's own temporary directory, removed when the test lets go of it (passing or failing), so no run
/// leaves anything in the temporary directory.
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
        .output()
        .expect("起不动 zikaron");
    Ran {
        code: o.status.code().unwrap_or(-1),
        out: o.stdout,
    }
}

const A_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

// Behavior: real binaries.

#[test]
fn a_misuse_writes_not_one_byte_to_stdout() {
    let w = scratch("misuse");
    // Misuse as in HARNESS: exit 2 and zero bytes on stdout. Whether there are bytes is how downstream tells
    // an answer from misuse without parsing.
    for args in [
        vec!["conjure"],
        vec!["keygen", "--seed", "1"],
        vec!["badge", "--decode", "/nowhere/at/all"],
        vec!["depth", "--ledger", "book"],
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
    assert_eq!(first, "E_UNREADABLE /nowhere/at/all");
}

#[test]
fn an_answer_carries_no_trailing_newline() {
    let w = scratch("newline");
    let r = run_in(&w, &["keygen"]);
    assert_eq!(r.code, 0);
    assert!(!r.out.is_empty());
    // One value per call; a newline is not a separator.
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
    // `mode` is a member the law requires. The shell does not judge it: with both flags missing the body has
    // no such member and the core returns `E_BODY_FIELD`.
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
    let g = run_in(
        &w,
        &[
            "grant", "--ledger", "b", "--key", A_KEY,
            "--grantee", "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc",
            "--work", &format!("0x{}", "c1".repeat(32)),
            "--terms", &format!("0x{}", "71".repeat(32)),
        ],
    );
    assert_eq!(g.code, 0);
    let id = String::from_utf8_lossy(&g.out)
        .split("\"entryId\":\"")
        .nth(1)
        .and_then(|x| x.split('"').next())
        .expect("答里没有 entryId")
        .to_string();
    let file = format!("b/{}.entry", id.trim_start_matches("0x"));
    // An unanchored grant: check four UNKNOWN, verdict PARTIAL. It exits 3, not 0: folded into green, a buyer
    // would take an unanchored grant as a green light.
    let c = run_in(&w, &["check-grant", "--grant", &file, "--ledger", "b"]);
    assert_eq!(c.code, 3, "PARTIAL 该有自己那一格");
    assert!(String::from_utf8_lossy(&c.out).contains("\"verdict\":\"PARTIAL\""));
}

#[test]
fn a_world_that_cannot_answer_is_not_a_world_that_answers_no() {
    let w = scratch("unanswered");
    // Law §9.4: a scan failure is the absence of an answer. No endpoint exits 4, not 1.
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
            "anchor", "--endpoint", &ep, "--key", A_KEY, "--form", "bare",
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
    // The trace marks show this run passed through the command line itself, the core (canonical form and
    // thirteen steps) and storage (append).
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
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "pack", "--note", ""]);
    assert_eq!(r.code, 0);
    assert!(String::from_utf8_lossy(&r.out).contains("\"state\":\"KIT_OK\""));
    assert!(w.join("pack/manifest.json").is_file());
    // The target already exists: nothing is overwritten and not one byte is written to that path.
    let before = std::fs::read(w.join("pack/manifest.json")).expect("读不出清单");
    let again = run_in(&w, &["kit-export", "--ledger", "b", "--out", "pack", "--note", "第二趟"]);
    assert_eq!(again.code, 1);
    assert_eq!(before, std::fs::read(w.join("pack/manifest.json")).expect("读不出清单"));
    // A half-laid staging area does not stay on disk.
    let leftovers: Vec<String> = std::fs::read_dir(&w)
        .expect("列不动")
        .filter_map(|x| x.ok())
        .map(|x| x.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("staging"))
        .collect();
    assert!(leftovers.is_empty(), "临时地没清干净:{leftovers:?}");
}

// Discipline: source scans.

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
        // Stop at the next section header: `[[bin]]` also has a `name =` that would read as a dependency.
        if line.starts_with('[') {
            break;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // No third-party crates: every dependency must be a path dependency.
        assert!(line.contains("path ="), "第三方依赖:{line}");
    }
}

#[test]
fn the_ledger_is_never_touched_by_hand() {
    // Directories only through storage: the file that reads and writes the ledger (`ledger.rs`) has no
    // standard-library disk operation.
    let text = src("ledger.rs");
    for forbidden in ["std::fs::", "File::", "read_dir", "create_dir", "remove_"] {
        assert!(!text.contains(forbidden), "ledger.rs 里出现了 {forbidden}");
    }
}

#[test]
fn no_law_literal_lives_outside_the_bases() {
    // Law bytes come from the core and kit core constants (spec, type names, signing domains, KIT_OK /
    // BADGE_OK); none is written again here.
    let forbidden = [
        "\"zikaron/1\"",
        "\"zikaron.kit/1\"",
        "\"zikaron.fpm/1\"",
        "\"zikaron.ack/1\"",
        "\"zikaron/1-adoption\"",
        "\"KIT_OK\"",
        "\"BADGE_OK\"",
        "\"genesis\"",
        "\"revocation\"",
        "\"succession\"",
        "\"annotation\"",
        // The retraction type literal lives in `zikaron_glue::retraction` only.
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
    // Flag names are closed (`ALL_FLAGS` plus `MORE_FLAGS`): a verb asking for a flag outside them would
    // reject it as misuse at `close`.
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
fn the_twenty_one_verbs_are_twenty_one_and_each_one_answers() {
    let names = zikaron_cli::verbs::VERBS;
    assert_eq!(names.len(), 21);
    let text = src("verbs.rs");
    for v in names {
        assert!(text.contains(&format!("\"{v}\" =>")), "{v} 没有分发的落点");
    }
    let mut sorted = names.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 21, "二十一个名字里有重的");
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
    // The source never ends, so reading to end would hang; reading exactly the length is the only way.
    // The command line reads the system's source only through the operating-system crate, which reads exactly
    // the length (the source never ends, so reading to end would hang).
    assert!(text.contains("zikaron_os::fill_random("), "熵只经 zikaron_os 那一处取");
    assert!(!text.contains("fs::read("), "整档读一个无尽的档会挂住");
    // Unavailable is said: no second source and no makeshift fallback.
    assert!(!text.to_lowercase().contains("fallback"));
    let os = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../zikaron-os/src/lib.rs")).expect("zikaron-os");
    assert!(os.contains("imp::fill_random(buf)"), "取熵在 zikaron-os 里每个系统一份实现");
}

#[test]
fn the_cli_schema_and_the_key_table_agree_cell_by_cell() {
    // `CLI-SCHEMA.md` section 7 must match `codes::Key` cell by cell.
    use zikaron_cli::codes::Key;
    let doc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CLI-SCHEMA.md"),
    )
    .expect("读不出 CLI-SCHEMA.md");
    let section = doc
        .split("## 7 · Closed table of output keys")
        .nth(1)
        .expect("CLI-SCHEMA.md has no section 7")
        .trim_start()
        // Only the first paragraph of section 7: that is the key table itself.
        .split("\n\n")
        .next()
        .expect("第七节读不完");
    let mut listed: Vec<String> = section
        .split('`')
        .skip(1)
        .step_by(2)
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty() && x != "codes::Key")
        .collect();
    listed.sort();
    listed.dedup();
    let mut real: Vec<String> = Key::ALL.iter().map(|k| k.as_str().to_string()).collect();
    real.sort();
    assert_eq!(listed, real, "the key table in CLI-SCHEMA.md does not match codes::Key");
}

#[test]
fn the_cli_schema_carries_the_whole_exit_table_and_every_reason() {
    use zikaron_cli::codes::{Exit, Reason};
    let doc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CLI-SCHEMA.md"),
    )
    .expect("读不出 CLI-SCHEMA.md");
    for e in Exit::ALL {
        assert!(doc.contains(&format!("| {} |", e.code())), "CLI-SCHEMA.md lacks exit code {}", e.code());
    }
    for r in Reason::ALL {
        assert!(doc.contains(r.as_str()), "CLI-SCHEMA.md lacks refusal reason {}", r.as_str());
    }
    for v in zikaron_cli::verbs::VERBS {
        assert!(doc.contains(&format!("`{v}`")), "CLI-SCHEMA.md lacks verb {v}");
    }
}

#[test]
fn not_one_of_the_twenty_verbs_passes_without_marking_the_trace() {
    // Every verb emits its trace mark: each of the twenty-one runs once (most stop at missing-flag misuse, and
    // the mark is emitted at dispatch), and the trace must show it.
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
    // A named id that is not found must not exit 0 with KIT_OK: that would seal an empty kit.
    let bogus = format!("0x{}", "99".repeat(32));
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "pack", "--entry", &bogus, "--note", ""]);
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert!(!w.join("pack").exists());
    // Bare 64 hex and `0x`-prefixed are the same id in every verb.
    let g = run_in(&w, &["show", "--ledger", "b", "--entry", &{
        let listing = std::fs::read_dir(w.join("b")).expect("列不动");
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
fn a_document_the_law_refused_never_reaches_the_out_path() {
    let w = scratch("no-land");
    // An acknowledgement that does not pair with its manifest (the signer has no row): exit 1 and nothing at
    // the `--out` path.
    let rows = format!(
        "[{{\"recipient\":\"0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc\",\"variant\":\"0x{}\"}}]",
        "e1".repeat(32)
    );
    std::fs::write(w.join("rows.json"), rows).expect("写不下");
    let f = run_in(&w, &["fpm-sign", "--key", A_KEY, "--work", &format!("0x{}", "c1".repeat(32)), "--rows", "rows.json", "--note", "", "--out", "fpm.json"]);
    assert_eq!(f.code, 0);
    assert!(w.join("fpm.json").is_file(), "肯定的答该落盘");
    let r = run_in(&w, &["ack-sign", "--key", A_KEY, "--fpm-doc", "fpm.json", "--variant", &format!("0x{}", "e1".repeat(32)), "--note", "", "--out", "ack.json"]);
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert!(String::from_utf8_lossy(&r.out).contains("ACK_NO_ROW"));
    assert!(!w.join("ack.json").exists(), "配不上的那一份不该躺在盘上");
}

#[test]
fn anchor_refuses_to_quietly_drop_the_endpoints_it_will_not_use() {
    let w = scratch("one-chain");
    let r = run_in(
        &w,
        &[
            "anchor", "--endpoint", "1=http://127.0.0.1:1", "--endpoint", "2=http://127.0.0.1:2",
            "--key", A_KEY, "--form", "bare", "--hash", &format!("0x{}", "11".repeat(32)),
        ],
    );
    assert_eq!(r.code, 2, "多给端点该是误用");
    assert!(r.out.is_empty());
}

#[test]
fn a_refused_kit_export_answers_in_the_shape_the_cli_schema_names() {
    let w = scratch("kit-shape");
    assert_eq!(
        run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]).code,
        0
    );
    // When the kit core judges a kit invalid, `state` is its verdict, not a shell refusal.
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "p1", "--root", "notahex", "--note", ""]);
    assert_eq!(r.code, 1);
    let t = String::from_utf8_lossy(&r.out).into_owned();
    assert!(t.contains("\"reason\":\"E_KIT\""), "{t}");
    assert!(t.contains("\"state\":\"E_KIT_"), "state 该载 kit 法的 verdict:{t}");
    // A disk failure carries `path`.
    assert_eq!(run_in(&w, &["kit-export", "--ledger", "b", "--out", "p2", "--note", ""]).code, 0);
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "p2", "--note", ""]);
    assert_eq!(r.code, 1);
    let t = String::from_utf8_lossy(&r.out).into_owned();
    assert!(t.contains("\"reason\":\"E_LEDGER\"") && t.contains("\"path\":\"p2\""), "{t}");
    // A named id not found has its own refusal, not the empty-lineage one.
    let bogus = format!("0x{}", "99".repeat(32));
    let r = run_in(&w, &["kit-export", "--ledger", "b", "--out", "p3", "--entry", &bogus, "--note", ""]);
    assert_eq!(r.code, 1);
    let t = String::from_utf8_lossy(&r.out).into_owned();
    assert!(t.contains("\"reason\":\"E_ENTRY_ABSENT\"") && t.contains(&bogus), "{t}");
    // A malformed id is misuse, in both verbs.
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
    let rows = format!(
        "[{{\"recipient\":\"0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc\",\"variant\":\"0x{}\"}}]",
        "e1".repeat(32)
    );
    std::fs::write(w.join("rows.json"), rows).expect("写不下");
    let work = format!("0x{}", "c1".repeat(32));
    let sign = |out: &str| -> Ran {
        run_in(
            &w,
            &["fpm-sign", "--key", A_KEY, "--work", &work, "--rows", "rows.json", "--note", "", "--out", out],
        )
    };

    // Success: the landed bytes equal the answer's byte for byte.
    let r = sign("fpm.json");
    assert_eq!(r.code, 0, "{}", String::from_utf8_lossy(&r.out));
    let answered = String::from_utf8_lossy(&r.out)
        .split("\"doc\":\"")
        .nth(1)
        .and_then(|x| x.split("\",\"").next())
        .expect("答里没有 doc")
        .replace("\\\"", "\"");
    let landed = std::fs::read_to_string(w.join("fpm.json")).expect("落下去的档读不出");
    assert_eq!(landed, answered, "落下去的与答的不是同一份");

    // Refusal one: the path is taken; refused by name, and the existing bytes are unchanged.
    let before = std::fs::read(w.join("fpm.json")).expect("读不出");
    let again = sign("fpm.json");
    assert_eq!(again.code, 1);
    let t = String::from_utf8_lossy(&again.out).into_owned();
    assert!(t.contains("E_OCCUPIED") && t.contains("fpm.json"), "{t}");
    assert_eq!(before, std::fs::read(w.join("fpm.json")).expect("读不出"), "原有的档被动了");

    // Refusal two (unix only: the folder is made unwritable with unix permission bits): a landing that fails
    // leaves nothing at the target.
    #[cfg(unix)]
    {
    use std::os::unix::fs::PermissionsExt;
    let locked = w.join("locked");
    std::fs::create_dir_all(&locked).expect("建不出");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).expect("改不了权限");
    let r = sign("locked/fpm.json");
    assert_eq!(r.code, 1, "{}", String::from_utf8_lossy(&r.out));
    assert!(!locked.join("fpm.json").exists(), "落不下去却留下了一份");
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
    // One landing path (`zikaron_glue::landing`): a plain `fs::write` in the shell would be a second exit,
    // and a second exit is the one that overwrites.
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
    // "Misuse writes zero bytes to stdout" is held by control flow: `emit` is the only writer and never
    // returns, so nothing can print and then refuse. This scan states it once.
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
    // The writer never returns.
    assert!(out_rs.contains("pub fn emit(a: Answer) -> !"), "emit 不再是不返回的:两个出口就不互斥了");
    assert!(out_rs.contains("pub fn misuse(reason: Reason, subject: &str, said: Said) -> !"));
    // Exactly two exits.
    assert_eq!(out_rs.matches("std::process::exit").count(), 2, "out.rs 里的退出口不是两处");
    // `Exit::Misuse` appears only in out.rs and codes.rs, so no answer can claim to be misuse.
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
    // At least one misuse path per verb; all three private-key cases (malformed, out of range, missing).
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
        vec!["succeed", "--ledger", "b", "--key", bad_key],
        vec!["annotate", "--ledger", "b", "--key", A_KEY, "--seq", "1"],
        vec!["retract", "--ledger", "b", "--key", A_KEY, "--seq", "1"],
        vec!["retract", "--ledger", "b", "--key", A_KEY, "--subject", "0x12", "--prev", "0x12"],
        vec!["anchor", "--endpoint", "1=http://127.0.0.1:1", "--key", bad_key, "--form", "bare", "--hash", &h32],
        vec!["anchor", "--endpoint", "1=http://127.0.0.1:1", "--key", A_KEY, "--form", "gossip", "--hash", &h32],
        vec!["anchor", "--endpoint", "nonsense", "--key", A_KEY, "--form", "bare", "--hash", &h32],
        vec!["anchor", "--endpoint", "1=http://a", "--endpoint", "2=http://b", "--key", A_KEY, "--form", "bare", "--hash", &h32],
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
fn the_cli_schema_lists_every_flag_the_code_knows() {
    // Section 8 of `CLI-SCHEMA.md` (flag names) must match the code; one row off fails.
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CLI-SCHEMA.md"))
        .expect("读不出 CLI-SCHEMA.md");
    let section = doc.split("## 8 · Flags").nth(1).expect("CLI-SCHEMA.md has no section 8");
    let mut listed: Vec<String> = section
        .lines()
        .filter_map(|l| l.strip_prefix("| `--"))
        .filter_map(|l| l.split('`').next())
        .map(str::to_string)
        .collect();
    let n = listed.len();
    listed.sort();
    listed.dedup();
    assert_eq!(listed.len(), n, "§八 里有重名的旗");
    let mut real: Vec<String> = zikaron_cli::verbs::all_flag_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let m = real.len();
    real.sort();
    real.dedup();
    assert_eq!(real.len(), m, "码里的旗名闭表有重名");
    assert_eq!(listed, real, "§八 与旗名闭表对不上");
    assert_eq!(listed.len(), 53, "the flag count changed; update the flag count stated in CLI-SCHEMA.md");
}

// init: one ledger, one root.

/// Name and bytes of every file in a directory, by name; compared before and after a refusal.
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
    // Same key, another statement: a second seq 0 would give different bytes.
    let second = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "x"]);
    assert_eq!(second.code, 1, "二次创世该拒:{}", String::from_utf8_lossy(&second.out));
    assert_eq!(reason_of(&second), "E_ALREADY_ROOTED");
    assert!(member_of(&second, "written").is_none(), "拒的答里不该有 written");
    assert!(matches!(member_of(&second, "author"), Some(zikaron::json::Value::Str(ref a)) if a.starts_with("0x") && a.len() == 42), "拒因带根的作者");
    assert!(matches!(member_of(&second, "count"), Some(zikaron::json::Value::Int(1))), "目录里一件");
    assert_eq!(files_of(&w.join("b")), before, "拒之后目录的档名集与字节都该与拒之前相同");
    // Same key, same statement: identical bytes, still refused (one ledger, one root).
    let same = run_in(&w, &["init", "--ledger", "b", "--key", A_KEY, "--statement", "开端"]);
    assert_eq!(same.code, 1);
    assert_eq!(reason_of(&same), "E_ALREADY_ROOTED");
    assert_eq!(files_of(&w.join("b")), before);
    // Another key too: the root belongs to the ledger, not the key.
    let other = "0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba";
    let third = run_in(&w, &["init", "--ledger", "b", "--key", other, "--statement", "另一个开端"]);
    assert_eq!(third.code, 1);
    assert_eq!(reason_of(&third), "E_ALREADY_ROOTED");
    assert_eq!(files_of(&w.join("b")), before, "换钥再来也一份不落");
}

#[test]
fn init_on_a_ledger_holding_bytes_it_cannot_read_as_a_root_is_refused() {
    // Case one: a stray file with a non-entry name (the store cannot read it; it is skipped).
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

    // Case two: an entry-named file with broken bytes (the store reads it into the pile; the core refuses
    // it). Counting only recognized seq 0 entries would miss this case and write a second root; asking "is it
    // empty" stops it.
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
    // An empty directory and a missing one both still get one genesis.
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
    // Two genesis entries, same key, different statements: each written into its own empty directory, then
    // both copied into one. Ids are counted, not authors, so two roots: `E_TIP_FORKED`.
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
    // The write gate audits under one root; a pile that already holds two (two keys each wrote a genesis)
    // cannot be judged, and the gate refuses what it cannot judge, `--seq` and `--prev` given by hand (no tip
    // asked) included.
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
    // Naming the root (`--root`) is how a writer picks its line in such a ledger: the gate audits under that
    // root, the same one the tip is asked under, and the write by the key that holds it lands.
    let author = files_of(&w.join("one")).into_iter().find_map(|(_, b)| zikaron::entry::check(&b).ok()).map(|e| e.author).expect("创世作者");
    let named = run_in(&w, &["history", "--ledger", "b", "--key", A_KEY, "--root", &author, "--content", &h32, "--mark", "v1", "--toolchain", &h32]);
    assert_eq!(named.code, 0, "点名根之后照写:{}", String::from_utf8_lossy(&named.out));
}

#[test]
fn one_genesis_stored_under_two_names_is_still_one_root() {
    // One genesis copied under two file names (identical bytes): one id, merged by the core, so still one
    // root: `E_ALREADY_ROOTED`, not `E_TIP_FORKED`.
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

// retract: the writing side of the retraction convention.

fn text_of(r: &Ran, key: &str) -> String {
    match member_of(r, key) {
        Some(zikaron::json::Value::Str(x)) => x,
        _ => String::new(),
    }
}

/// A ledger with a genesis and one history; returns (genesis id, history id).
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
    let unknown = format!("{:?}", member_of(&after, "unknown_type"));
    assert!(unknown.contains(id.trim_start_matches("0x")), "审计的 unknown_type 里列得出这一条:{unknown}");
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
