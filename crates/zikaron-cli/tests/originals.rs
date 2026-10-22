//! `kit-verify --originals-to` on the real binary: a plain record package, one file or a folder, gives its
//! originals out with no home and no key; the folder either holds them all or is as it was.

use std::path::{Path, PathBuf};
use std::process::Command;
use zikaron::json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: [u8; 32] = [
    0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04, 0x49, 0x66, 0xf0, 0x94, 0x53, 0x89, 0xdc, 0x9e, 0x86, 0xda, 0xe8, 0x8c, 0x7a, 0x84, 0x12,
    0xf4, 0x60, 0x3b, 0x6b, 0x78, 0x69, 0x0d,
];

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-cli-originals-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join("home")).expect("a scratch place");
    Scratch(p)
}

/// One run in `dir` (its own user home): exit code, stdout, the first two lines of stderr.
fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env(zikaron_os::HOME_VAR, dir.join("home"))
        .env("LANG", "en_US.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .output()
        .expect("runs");
    let err = String::from_utf8_lossy(&o.stderr).lines().take(2).collect::<Vec<_>>().join("\n");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), err)
}

fn member(out: &str, k: &str) -> Value {
    zikaron::json::parse(out.as_bytes()).ok().and_then(|v| v.member(k).cloned()).unwrap_or(Value::Null)
}

fn texts(v: &Value) -> Vec<String> {
    match v {
        Value::Arr(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn seal(kind: &str, seq: u64, prev: Option<&str>, body: Value) -> Vec<u8> {
    let author = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&KEY).expect("a key"));
    let six = Value::Obj(vec![
        ("author".into(), Value::Str(author)),
        ("body".into(), body),
        ("entryType".into(), Value::Str(kind.into())),
        ("prev".into(), prev.map(|p| Value::Str(p.into())).unwrap_or(Value::Null)),
        ("seq".into(), Value::Int(seq)),
        ("spec".into(), Value::Str(zikaron::tokens::SPEC.into())),
    ]);
    let (_, digest) = zikaron::entry::presig_and_digest(&zikaron::entry::b6_bytes(&six), zikaron::tokens::Domain::Entry.as_str());
    let (r, s, v) = zikaron::cryptox::sign_digest(&KEY, &digest).expect("signs");
    let mut raw = r.to_vec();
    raw.extend_from_slice(&s);
    raw.push(v);
    let Value::Obj(mut ms) = six else { panic!("an object") };
    ms.push(("sig".into(), Value::Str(zikaron::hexfmt::encode(&raw))));
    zikaron::json::canon_bytes(&Value::Obj(ms))
}

fn entries(contents: &[String]) -> Vec<Vec<u8>> {
    let mut out = vec![seal("genesis", 0, None, Value::Obj(vec![("statement_md".into(), Value::Str("start".into()))]))];
    for (i, c) in contents.iter().enumerate() {
        let prev = zikaron::hexfmt::encode(&zikaron::entry::entry_id(out.last().expect("one")));
        let mode = Value::Obj(vec![
            ("mark".into(), Value::Str(zikaron_glue::recording::FAMILY.into())),
            ("toolchain".into(), Value::Str(zikaron::hexfmt::encode(&zikaron_glue::recording::toolchain()))),
        ]);
        out.push(seal("history", i as u64 + 1, Some(&prev), Value::Obj(vec![("content".into(), Value::Str(c.clone())), ("mode".into(), mode)])));
    }
    out
}

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(b))
}

fn dir_content(dir: &Path) -> String {
    let (rows, _) = zikaron_glue::recording::dir_rows(dir).expect("walks");
    zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&rows).0)
}

/// A package carrying a file original (a name in Chinese characters) and a signed directory (`Field Notes`, two
/// files), as the desktop puts originals in.
fn package(w: &Path, out: &Path, form: zikaron_glue::pack::Form) -> (Vec<u8>, String) {
    let file = b"report bytes".to_vec();
    let src = w.join("src").join("Field Notes");
    std::fs::create_dir_all(src.join("Day 2")).expect("a tree");
    std::fs::write(src.join("day1.txt"), b"first day").expect("w");
    std::fs::write(src.join("Day 2").join("notes.txt"), b"second day").expect("w");
    let dir = dir_content(&src);
    let mut b = zikaron_glue::pack::Bundle { entries: entries(&[hex(&file), dir.clone()]), note: String::new(), ..Default::default() };
    let mut names = Vec::new();
    for (name, bytes) in [("報告.pdf", file.clone()), ("Field Notes/day1.txt", b"first day".to_vec()), ("Field Notes/Day 2/notes.txt", b"second day".to_vec())] {
        zikaron_glue::kitnames::put_original(name, bytes, &mut b, &mut names).expect("kit-legal");
    }
    b.files.push((zikaron_glue::kitnames::NAMES_FILE.to_string(), zikaron_glue::kitnames::names_table(&names)));
    zikaron_glue::pack::export_as(out, b, form).expect("exports");
    (file, dir)
}

fn listing(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned())).collect()).unwrap_or_default();
    v.sort();
    v
}

/// Both forms give out the file original under its own name with the recorded bytes and the directory whole,
/// recomputing to its record; the answer is the records' answer (no chain read: exit 3) with `taken` and `path`.
#[test]
fn both_forms_give_out_file_and_directory_originals() {
    let w = scratch("forms");
    for (tag, form) in [("file", zikaron_glue::pack::Form::File), ("folder", zikaron_glue::pack::Form::Folder)] {
        let kit = w.0.join(format!("k-{tag}"));
        let (file, dir) = package(&w.0, &kit, form);
        let to = w.0.join(format!("out-{tag}"));
        std::fs::create_dir_all(&to).expect("a folder");
        let (code, out, _) = run(&w.0, &["kit-verify", "--kit", &kit.display().to_string(), "--originals-to", &to.display().to_string()]);
        assert_eq!(code, 3, "{tag}: {out}");
        assert_eq!(texts(&member(&out, "taken")), vec!["Field Notes".to_string(), "報告.pdf".to_string()], "{tag}: {out}");
        assert_eq!(member(&out, "path").as_str(), Some(to.display().to_string().as_str()));
        assert!(out.matches("\"original\":\"match\"").count() == 2, "both originals match: {out}");
        assert_eq!(std::fs::read(to.join("報告.pdf")).expect("there"), file);
        assert_eq!(dir_content(&to.join("Field Notes")), dir, "{tag}");
        assert_eq!(listing(&to), vec!["Field Notes".to_string(), "報告.pdf".to_string()], "{tag}: nothing else");
    }
}

/// A name already in the folder refuses the whole batch by name, zero bytes; a link there is refused and never
/// followed; a folder that cannot be written leaves it as it was. Each answer is `E_EXTRACT` with its token.
#[test]
fn what_stands_in_the_way_refuses_the_whole_batch_and_writes_nothing() {
    let w = scratch("refused");
    let kit = w.0.join("k.zkit");
    package(&w.0, &kit, zikaron_glue::pack::Form::File);
    let k = kit.display().to_string();
    // Same name.
    let to = w.0.join("same");
    std::fs::create_dir_all(&to).expect("a folder");
    std::fs::write(to.join("報告.pdf"), b"mine").expect("w");
    let (code, out, err) = run(&w.0, &["kit-verify", "--kit", &k, "--originals-to", &to.display().to_string()]);
    assert_eq!((code, member(&out, "reason").as_str(), member(&out, "token").as_str()), (1, Some("E_EXTRACT"), Some("EXISTS")), "{out}");
    assert_eq!(texts(&member(&out, "names")), vec!["報告.pdf".to_string()]);
    assert_eq!(err, zikaron_glue::extract::words::EXISTS.1, "the words from the one place");
    assert_eq!(listing(&to), vec!["報告.pdf".to_string()]);
    assert_eq!(std::fs::read(to.join("報告.pdf")).expect("there"), b"mine".to_vec(), "not written over");
    // A link, pointing outside.
    let to = w.0.join("link");
    let outside = w.0.join("outside");
    std::fs::create_dir_all(&to).expect("a folder");
    std::fs::create_dir_all(&outside).expect("a folder");
    // The link is made by the system's own tool (the platform calls live in the platform crate only).
    let linked = Command::new("ln").arg("-s").arg(outside.join("Field Notes")).arg(to.join("Field Notes")).status().map(|s| s.success()).unwrap_or(false);
    if linked {
        let (code, out, _) = run(&w.0, &["kit-verify", "--kit", &k, "--originals-to", &to.display().to_string()]);
        assert_eq!((code, member(&out, "token").as_str()), (1, Some("LINK")), "{out}");
        assert!(listing(&outside).is_empty(), "nothing outside the folder");
        assert_eq!(listing(&to), vec!["Field Notes".to_string()], "the link alone");
    }
    // A folder that cannot be written: the first write fails and nothing stays (a read-only folder keeps writes out on
    // unix systems; on Windows the attribute does not).
    if cfg!(unix) {
        let to = w.0.join("shut");
        std::fs::create_dir_all(&to).expect("a folder");
        let mut p = std::fs::metadata(&to).expect("there").permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&to, p.clone()).expect("shut");
        let (code, out, _) = run(&w.0, &["kit-verify", "--kit", &k, "--originals-to", &to.display().to_string()]);
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        std::fs::set_permissions(&to, p).expect("open");
        assert_eq!((code, member(&out, "token").as_str()), (1, Some("LANDING")), "{out}");
        assert!(listing(&to).is_empty(), "as it was");
    }
}

/// A package the kit law refuses gives nothing: a manifest naming a file by a path that is not a kit path
/// (uppercase, kit law §7.2) is refused as the law refuses it, before anything is written.
#[test]
fn a_package_the_kit_law_refuses_gives_nothing() {
    let w = scratch("law");
    let file = b"report bytes".to_vec();
    let ents = entries(&[hex(&file)]);
    let kit = w.0.join("bad");
    std::fs::create_dir_all(kit.join("entries")).expect("dirs");
    std::fs::create_dir_all(kit.join("files")).expect("dirs");
    let mut ids = Vec::new();
    for e in &ents {
        let id = zikaron::hexfmt::encode(&zikaron::entry::entry_id(e));
        std::fs::write(kit.join("entries").join(format!("{}.zk1", id.trim_start_matches("0x"))), e).expect("w");
        ids.push(id);
    }
    ids.sort();
    std::fs::write(kit.join("files").join("Report.pdf"), &file).expect("w");
    let sha = hex(&file);
    let manifest = Value::Obj(vec![
        ("contents".into(), Value::Arr(vec![Value::Obj(vec![("content".into(), Value::Str(sha.clone())), ("path".into(), Value::Str("Report.pdf".into()))])])),
        ("entries".into(), Value::Arr(ids.into_iter().map(Value::Str).collect())),
        ("files".into(), Value::Arr(vec![Value::Obj(vec![("path".into(), Value::Str("Report.pdf".into())), ("sha256".into(), Value::Str(sha)), ("size".into(), Value::Int(file.len() as u64))])])),
        ("note_md".into(), Value::Str(String::new())),
        ("proofs".into(), Value::Arr(Vec::new())),
        ("root".into(), Value::Null),
        ("spec".into(), Value::Str(zikaron_kit::tokens::SPEC_KIT.into())),
    ]);
    std::fs::write(kit.join("manifest.json"), zikaron::json::canon_bytes(&manifest)).expect("w");
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (code, out, _) = run(&w.0, &["kit-verify", "--kit", &kit.display().to_string(), "--originals-to", &to.display().to_string()]);
    assert_eq!((code, member(&out, "reason").as_str()), (1, Some("E_KIT")), "{out}");
    assert!(member(&out, "taken") == Value::Null && listing(&to).is_empty(), "zero bytes: {out}");
}
