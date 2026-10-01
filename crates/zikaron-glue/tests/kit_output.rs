//! Checks of kit output: real binaries for behavior, source scans for discipline.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_zkg");

fn src(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("读不出 {}:{e}", p.display()))
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
    let p = std::env::temp_dir().join(format!("zk-glue-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("建不出草稿地");
    Scratch(p)
}

struct Ran {
    code: i32,
    out: String,
}

fn run_in(dir: &Path, args: &[&str]) -> Ran {
    let o = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("起不动 zkg");
    Ran {
        code: o.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&o.stdout).into_owned(),
    }
}

/// A ready ledger: three entries, one grant and one revocation of it.
///
/// Building it through the command surface would start another package's binary, so the storage directory is
/// laid out through the public API of `zikaron-store` instead.
fn a_ledger(dir: &Path) -> (String, String) {
    use zikaron::json::Value;
    use zikaron_store::layout::EntryName;
    use zikaron_store::ledger::LedgerDir;
    const KEY: [u8; 32] = [
        0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04, 0x49, 0x66, 0xf0, 0x94, 0x53,
        0x89, 0xdc, 0x9e, 0x86, 0xda, 0xe8, 0x8c, 0x7a, 0x84, 0x12, 0xf4, 0x60, 0x3b, 0x6b, 0x78,
        0x69, 0x0d,
    ];
    let author = zikaron::hexfmt::encode(
        &zikaron::cryptox::address_of_privkey(&KEY).expect("私钥不成钥"),
    );
    let book = dir.join("book");
    let led = LedgerDir::open_or_create(&book).expect("开不出账本");
    let seal = |kind: &str, seq: u64, prev: Option<&str>, body: Value| -> Vec<u8> {
        let six = Value::Obj(vec![
            ("author".into(), Value::Str(author.clone())),
            ("body".into(), body),
            ("entryType".into(), Value::Str(kind.into())),
            (
                "prev".into(),
                match prev {
                    Some(p) => Value::Str(p.into()),
                    None => Value::Null,
                },
            ),
            ("seq".into(), Value::Int(seq)),
            ("spec".into(), Value::Str(zikaron::tokens::SPEC.into())),
        ]);
        let (_, digest) = zikaron::entry::presig_and_digest(
            &zikaron::entry::b6_bytes(&six),
            zikaron::tokens::Domain::Entry.as_str(),
        );
        let (r, s, v) = zikaron::cryptox::sign_digest(&KEY, &digest).expect("签不出");
        let mut raw = Vec::with_capacity(65);
        raw.extend_from_slice(&r);
        raw.extend_from_slice(&s);
        raw.push(v);
        let Value::Obj(mut ms) = six else { panic!("不是对象") };
        ms.push(("sig".into(), Value::Str(zikaron::hexfmt::encode(&raw))));
        zikaron::json::canon_bytes(&Value::Obj(ms))
    };
    let put = |bytes: &[u8]| {
        let id = zikaron::hexfmt::encode(&zikaron::entry::entry_id(bytes));
        let name = EntryName::parse(id.trim_start_matches("0x")).expect("名不合形");
        led.append(&name, bytes).expect("追加不上");
        id
    };
    let hex32 = |b: u8| format!("0x{}", std::iter::repeat(format!("{b:02x}")).take(32).collect::<String>());

    let g0 = seal(
        "genesis",
        0,
        None,
        Value::Obj(vec![("statement_md".into(), Value::Str("开端".into()))]),
    );
    let id0 = put(&g0);
    let g1 = seal(
        "grant",
        1,
        Some(&id0),
        Value::Obj(vec![
            ("grantee".into(), Value::Str("0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc".into())),
            ("terms".into(), Value::Str(hex32(0x71))),
            ("work".into(), Value::Str(hex32(0xc1))),
        ]),
    );
    let id1 = put(&g1);
    let g2 = seal(
        "revocation",
        2,
        Some(&id1),
        Value::Obj(vec![("grant".into(), Value::Str(id1.clone()))]),
    );
    put(&g2);
    (author, hex32(0xc1))
}

// Behavior.

#[test]
fn a_bundle_that_the_kit_law_refuses_never_reaches_the_caller_s_path() {
    let w = scratch("kit-gate");
    a_ledger(&w);
    std::fs::write(w.join("a.bin"), "A").expect("写不下");
    std::fs::write(w.join("b.bin"), "B").expect("写不下");
    // Two different byte strings at one kit path: the manifest's `files` table is no longer strictly
    // increasing and the kit core refuses.
    let r = run_in(
        &w,
        &["pack", "--ledger", "book", "--out", "bundle", "--file", "same.bin=a.bin", "--file", "same.bin=b.bin", "--note", ""],
    );
    assert_eq!(r.code, 1, "自验不符该退 1:{}", r.out);
    assert!(!w.join("bundle").exists(), "验不过的包不该落地");
    // A half-laid staging area does not stay on disk.
    let leftovers: Vec<String> = std::fs::read_dir(&w)
        .expect("列不动")
        .filter_map(|x| x.ok())
        .map(|x| x.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("staging"))
        .collect();
    assert!(leftovers.is_empty(), "临时地没清干净:{leftovers:?}");
}

#[test]
fn the_revocation_of_a_chosen_grant_is_always_in_the_bundle() {
    let w = scratch("closure");
    let (_, work) = a_ledger(&w);
    // Choosing by work picks the grant; its revocation has no `work` member and the selector cannot see it,
    // so the closure rule pulls it in. Without it the recipient's checks would pass a revoked grant.
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "bundle", "--work", &work, "--note", ""]);
    assert_eq!(r.code, 0, "{}", r.out);
    // Three entries: the grant, the revocation pulled in, and the spine (genesis travels with the kit so the
    // verifier can tell whose ledger it is).
    assert!(r.out.contains("\"entries\":3"), "该有三枚条目:{}", r.out);
    assert!(
        r.out.contains("\"pulled\":[\"0x"),
        "闭合律该报出拉进来的那一枚:{}",
        r.out
    );
}

#[test]
fn platform_junk_is_dropped_by_name_and_a_bad_path_is_refused_by_name() {
    let w = scratch("tidy");
    a_ledger(&w);
    std::fs::create_dir_all(w.join("stuff")).expect("建不出");
    std::fs::write(w.join("stuff/.DS_Store"), "垃圾").expect("写不下");
    std::fs::write(w.join("stuff/art.bin"), "作品").expect("写不下");
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b1", "--dir", "art=stuff", "--note", ""]);
    assert_eq!(r.code, 0, "{}", r.out);
    // A dropped item is named: silently missing one differs from refusing one.
    assert!(r.out.contains("\"dropped\":[\"art/.DS_Store\"]"), "{}", r.out);
    std::fs::write(w.join("up.bin"), "x").expect("写不下");
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b2", "--file", "README.md=up.bin", "--note", ""]);
    assert_eq!(r.code, 1);
    assert!(r.out.contains("E_BAD_PATH") && r.out.contains("README.md"), "{}", r.out);
    assert!(!w.join("b2").exists());
}

#[test]
fn a_bundle_never_lands_on_a_path_that_already_holds_something() {
    let w = scratch("occupied");
    a_ledger(&w);
    assert_eq!(run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--note", ""]).code, 0);
    let before = std::fs::read(w.join("b/manifest.json")).expect("读不出清单");
    let again = run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--note", "第二趟"]);
    assert_eq!(again.code, 1);
    assert!(again.out.contains("E_OCCUPIED"));
    assert_eq!(before, std::fs::read(w.join("b/manifest.json")).expect("读不出清单"));
}

#[test]
fn every_part_of_a_tampered_bundle_is_refused_with_its_own_verdict() {
    let w = scratch("tamper");
    a_ledger(&w);
    assert_eq!(run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--note", ""]).code, 0);
    assert_eq!(run_in(&w, &["verify", "--kit", "b"]).code, 0);
    let copy = |tag: &str| {
        let to = w.join(tag);
        let mut stack = vec![(w.join("b"), to.clone())];
        while let Some((from, into)) = stack.pop() {
            std::fs::create_dir_all(&into).expect("建不出");
            for item in std::fs::read_dir(&from).expect("列不动") {
                let e = item.expect("列不动");
                if e.path().is_dir() {
                    stack.push((e.path(), into.join(e.file_name())));
                } else {
                    std::fs::copy(e.path(), into.join(e.file_name())).expect("拷不动");
                }
            }
        }
        to
    };
    let mut seen: Vec<String> = Vec::new();
    // Entry bytes.
    let d = copy("t1");
    let f = std::fs::read_dir(d.join("entries")).expect("列不动").next().expect("空的").expect("坏的").path();
    let mut b = std::fs::read(&f).expect("读不出");
    b.push(b'X');
    std::fs::write(&f, b).expect("写不下");
    // File bytes.
    let d2 = copy("t2");
    let vf = d2.join("files/verify.md");
    let mut b = std::fs::read(&vf).expect("读不出");
    b.push(b'X');
    std::fs::write(&vf, b).expect("写不下");
    // Manifest.
    let d3 = copy("t3");
    let mf = d3.join("manifest.json");
    let mut b = std::fs::read(&mf).expect("读不出");
    b.push(b' ');
    std::fs::write(&mf, b).expect("写不下");
    // One extra item.
    let d4 = copy("t4");
    std::fs::write(d4.join("extra.txt"), "清单里没有我").expect("写不下");

    for tag in ["t1", "t2", "t3", "t4"] {
        let r = run_in(&w, &["verify", "--kit", tag]);
        assert_eq!(r.code, 1, "{tag} 该被拒:{}", r.out);
        let state = r
            .out
            .split("\"state\":\"")
            .nth(1)
            .and_then(|x| x.split('"').next())
            .unwrap_or_default()
            .to_string();
        assert!(state.starts_with("E_KIT_"), "{tag} 的 verdict 不成形:{}", r.out);
        seen.push(state);
    }
    seen.sort();
    seen.dedup();
    // Each tampered part has its own refusal: four tamperings must not collapse into one sentence.
    assert_eq!(seen.len(), 4, "四处篡改只报出 {} 种拒因:{seen:?}", seen.len());
}

// Discipline: source scans.

#[test]
fn the_crate_leans_on_nothing_but_the_four_bases() {
    let manifest = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("读不出 Cargo.toml");
    let deps = manifest.split("[dependencies]").nth(1).expect("没有 [dependencies]");
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
fn every_kit_output_module_emits_its_own_mark() {
    // Every public module emits the kit output trace mark (this crate carries only that component).
    for name in ["pack.rs", "select.rs", "tidy.rs"] {
        let text = src(name);
        assert!(text.contains("seam_v2"), "{name} emits no kit output trace mark");
        assert!(!text.contains("seam_v3"), "{name} still carries a trace mark of another component");
    }
}

#[test]
fn no_kit_law_literal_lives_outside_the_bases() {
    // Kit law bytes come from the kit core's constants.
    for name in ["pack.rs", "select.rs", "tidy.rs", "names.rs"] {
        let text = src(name);
        for f in ["\"zikaron.kit/1\"", "\"KIT_OK\"", "\"GREEN\"", "\"PARTIAL\"", "\"FAIL\""] {
            assert!(!text.contains(f), "{name} 里写了法自己的字面 {f}");
        }
    }
}

#[test]
fn the_junk_list_is_a_closed_table_and_the_path_law_is_the_kit_s() {
    let text = src("tidy.rs");
    // Whether a path is a valid kit path is answered by the kit core; no second character-set check here.
    assert!(text.contains("kitdir::is_kit_path"));
    assert!(!text.contains("is_ascii_lowercase"), "别在这里重写一遍 kit 法 §7.2");
    // The junk list is closed; an entry needs a reason.
    assert!(text.contains("pub const JUNK: [&str; 6]"));
}

#[test]
fn a_write_that_fails_midway_leaves_nothing_behind() {
    let w = scratch("staging-leak");
    a_ledger(&w);
    std::fs::write(w.join("x"), "X").expect("写不下");
    std::fs::write(w.join("y"), "Y").expect("写不下");
    // `a` is a file while `a/b` needs `a` to be a directory: layout must fail halfway (the paths differ, so
    // the duplicate check passes). Wherever it fails, the staging area must not remain.
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--file", "a=x", "--file", "a/b=y", "--note", ""]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(!w.join("b").exists());
    let leftovers: Vec<String> = std::fs::read_dir(&w)
        .expect("列不动")
        .filter_map(|x| x.ok())
        .map(|x| x.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("staging"))
        .collect();
    assert!(leftovers.is_empty(), "铺了一半的临时地留在盘上:{leftovers:?}");
}

#[test]
fn two_things_may_not_share_one_path_in_the_bundle() {
    let w = scratch("reserved");
    a_ledger(&w);
    std::fs::write(w.join("mine.md"), "我自己的说明").expect("写不下");
    // A duplicate is reported by name, not as an opaque `E_KIT_MANIFEST:files`, and the caller's file is
    // never silently replaced. Both kinds are tried: against the generated note, and two of the caller's own.
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--file", "verify.md=mine.md", "--note", ""]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("E_DUPLICATE_PATH") && r.out.contains("verify.md"), "{}", r.out);
    assert!(!w.join("b").exists());
    std::fs::write(w.join("other.md"), "另一份").expect("写不下");
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b2", "--file", "same.md=mine.md", "--file", "same.md=other.md", "--note", ""]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("E_DUPLICATE_PATH") && r.out.contains("same.md"), "{}", r.out);
    assert!(!w.join("b2").exists());
}

#[test]
fn one_call_gives_one_answer_and_a_repeated_flag_is_misuse() {
    let w = scratch("one-answer");
    a_ledger(&w);
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--work", "0xaa", "--work", "0xbb", "--note", ""]);
    assert_eq!(r.code, 2, "同一面旗给两次该是误用:{}", r.out);
    assert!(r.out.is_empty(), "误用不该往 stdout 写:{}", r.out);
    // One call prints one value: stdout never has a second top-level `{`.
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b2", "--note", ""]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out.matches("\"ok\":").count(), 1, "一次调用印了不止一枚答:{}", r.out);
}

#[test]
fn a_flag_outside_a_verb_s_own_list_is_misuse() {
    let w = scratch("closed-flags");
    a_ledger(&w);
    // A verb outside the list fails loudly: `defend` was removed and is now misuse, where running and passing
    // with the name silently ignored would be wrong.
    let r = run_in(&w, &["defend", "--ledger", "book", "--out", "d"]);
    assert_eq!(r.code, 2, "{}", r.out);
    assert!(r.out.is_empty());
    // Likewise `await`, which is not in the verb list.
    let r = run_in(&w, &["await", "--root", "0x70997970c51812dc3a010c7d01b50e0d17dc79c8", "--grant", "x"]);
    assert_eq!(r.code, 2, "{}", r.out);
    assert!(r.out.is_empty());
    let r = run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--note", "", "--nope", "1"]);
    assert_eq!(r.code, 2);
    assert!(r.out.is_empty());
}

#[test]
fn every_write_in_this_crate_goes_through_the_one_landing() {
    // `std::fs::write`, `rename` and `File::create` appear only in `landing.rs`.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for item in std::fs::read_dir(&dir).expect("列不动 src") {
        let e = item.expect("列不动");
        if e.path().extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name == "landing.rs" {
            continue;
        }
        let text = std::fs::read_to_string(e.path()).expect("读不出");
        let code: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for f in ["fs::write(", "fs::rename(", "File::create("] {
            assert!(!code.contains(f), "{name} 里有一句裸 {f}");
        }
    }
}

#[test]
fn a_bundle_never_clobbers_and_a_failed_landing_leaves_no_half_tree() {
    let w = scratch("land-tree");
    a_ledger(&w);
    assert_eq!(run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--note", ""]).code, 0);
    let before = std::fs::read(w.join("b/manifest.json")).expect("读不出");
    let again = run_in(&w, &["pack", "--ledger", "book", "--out", "b", "--note", "第二趟"]);
    assert_eq!(again.code, 1);
    assert!(again.out.contains("E_OCCUPIED"));
    assert_eq!(before, std::fs::read(w.join("b/manifest.json")).expect("读不出"));
    let leftovers: Vec<String> = std::fs::read_dir(&w)
        .expect("列不动")
        .filter_map(|x| x.ok())
        .map(|x| x.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("staging") || n.contains("landing"))
        .collect();
    assert!(leftovers.is_empty(), "临时地留在盘上:{leftovers:?}");
}
