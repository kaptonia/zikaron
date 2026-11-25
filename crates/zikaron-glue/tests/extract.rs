use std::path::{Path, PathBuf};
use zikaron::json::Value;
use zikaron_glue::extract::{self, Kept, Refused, Why};

const KEY: [u8; 32] = [
    0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04, 0x49, 0x66, 0xf0, 0x94, 0x53, 0x89, 0xdc, 0x9e, 0x86, 0xda, 0xe8, 0x8c, 0x7a, 0x84, 0x12,
    0xf4, 0x60, 0x3b, 0x6b, 0x78, 0x69, 0x0d,
];

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn link(to: &Path, at: &Path) -> bool {
    std::process::Command::new("ln").arg("-s").arg(to).arg(at).status().map(|s| s.success()).unwrap_or(false)
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-glue-extract-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    Scratch(p)
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

fn package(out: &Path, form: zikaron_glue::pack::Form, entries: Vec<Vec<u8>>, originals: &[(&str, Vec<(&str, Vec<u8>)>)]) {
    let mut b = zikaron_glue::pack::Bundle { entries, note: String::new(), ..Default::default() };
    for (name, files) in originals {
        match files.as_slice() {
            [("", bytes)] => {
                zikaron_glue::kitnames::put_original(name, bytes.clone(), &mut b).expect("kit-legal");
            }
            _ => {
                let inside: Vec<(String, Vec<u8>)> = files.iter().map(|(n, b)| (n.to_string(), b.clone())).collect();
                zikaron_glue::kitnames::put_directory(name, inside, &mut b).expect("kit-legal");
            }
        }
    }
    zikaron_glue::pack::export_as(out, b, form).expect("exports");
}

fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut names: Vec<PathBuf> = std::fs::read_dir(at).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default();
        names.sort();
        for p in names {
            let rel = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            let md = std::fs::symlink_metadata(&p).expect("reads");
            if md.is_dir() {
                out.push((format!("{rel}/"), Vec::new()));
                walk(root, &p, out);
            } else {
                out.push((rel, std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

#[test]
fn a_file_original_is_taken_out_under_its_own_name_from_either_form() {
    let _one = one_at_a_time();
    let w = scratch("file");
    let bytes = b"chapter one".to_vec();
    let ents = entries(&[hex(&bytes)]);
    for (tag, form) in [("dir", zikaron_glue::pack::Form::Folder), ("file", zikaron_glue::pack::Form::File)] {
        let kit = w.0.join(format!("k-{tag}"));
        package(&kit, form, ents.clone(), &[("第一章 手稿.pdf", vec![("", bytes.clone())])]);
        let to = w.0.join(format!("out-{tag}"));
        std::fs::create_dir_all(&to).expect("a folder");
        let (plan, landed) = extract::take(&kit, &to).unwrap_or_else(|r| panic!("{tag}: {r:?}"));
        assert_eq!(landed.names, vec!["第一章 手稿.pdf".to_string()], "{tag}");
        assert!(plan.left.is_empty(), "{tag}");
        assert_eq!(tree(&to), vec![("第一章 手稿.pdf".to_string(), bytes.clone())], "{tag}: nothing else in the folder");
    }
}

#[test]
fn a_directory_original_is_judged_by_its_recomputed_manifest_and_taken_out_whole() {
    let _one = one_at_a_time();
    let w = scratch("dir");
    let src = w.0.join("src").join("Sound Archive");
    std::fs::create_dir_all(src.join("Night Takes")).expect("a tree");
    std::fs::write(src.join("readme.txt"), b"read me").expect("w");
    std::fs::write(src.join("Night Takes").join("take 03.wav"), b"night").expect("w");
    let content = dir_content(&src);
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[content.clone()]), &[("Sound Archive", vec![("Night Takes/take 03.wav", b"night".to_vec()), ("readme.txt", b"read me".to_vec())])]);
    let rows = zikaron_glue::recordcheck::records(&zikaron_glue::read::kit_entries(&kit).expect("entries"), &zikaron_glue::recordcheck::originals_in_dir(&kit), None);
    assert_eq!(rows.iter().map(|r| r.original).collect::<Vec<_>>(), vec![zikaron_glue::recordcheck::Original::Match], "the directory matches its record");
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (plan, landed) = extract::take(&kit, &to).expect("taken");
    let top = "Sound Archive".to_string();
    assert_eq!((plan.take[0].kept, landed.names.clone()), (Kept::Dir, vec![top.clone()]));
    assert_eq!(dir_content(&to.join(&top)), content, "taken out, it recomputes to the content recorded");
    assert!(to.join(&top).join("Night Takes").join("take 03.wav").is_file(), "names inside restored segment by segment");
}

#[test]
fn a_name_already_there_refuses_the_whole_batch_and_writes_nothing() {
    let _one = one_at_a_time();
    let w = scratch("exists");
    let (a, b) = (b"one".to_vec(), b"two".to_vec());
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a), hex(&b)]), &[("a.txt", vec![("", a)]), ("b.txt", vec![("", b)])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    std::fs::write(to.join("b.txt"), b"mine").expect("w");
    let before = tree(&to);
    assert_eq!(extract::take(&kit, &to).err(), Some(Refused::Exists(vec!["b.txt".into()])));
    assert_eq!(tree(&to), before, "zero bytes written");
}

#[test]
fn a_link_in_the_folder_is_refused_and_nothing_lands_outside() {
    let _one = one_at_a_time();
    let w = scratch("link");
    let a = b"one".to_vec();
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a)]), &[("a.txt", vec![("", a)])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let outside = w.0.join("outside");
    std::fs::create_dir_all(&outside).expect("a folder");
    if !link(&outside.join("a.txt"), &to.join("a.txt")) {
        return;
    }
    assert!(matches!(extract::take(&kit, &to), Err(Refused::Link(p)) if p.ends_with("a.txt")));
    assert!(tree(&outside).is_empty(), "nothing outside the chosen folder");
    assert_eq!(tree(&to).len(), 1, "the link alone");
}

#[test]
fn a_failure_midway_leaves_the_folder_as_it_was() {
    let _one = one_at_a_time();
    let w = scratch("midway");
    let (a, b) = (b"one".to_vec(), b"two".to_vec());
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a), hex(&b)]), &[("a.txt", vec![("", a)]), ("b.txt", vec![("", b)])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    std::fs::write(to.join("kept.txt"), b"mine").expect("w");
    let before = tree(&to);
    extract::pretend_place_fails_after(Some(1));
    let got = extract::take(&kit, &to);
    extract::pretend_place_fails_after(None);
    assert!(matches!(got, Err(Refused::Landing(..))), "{got:?}");
    assert_eq!(tree(&to), before, "the one placed is taken back, the staging folder gone");
}

#[test]
fn a_record_without_its_original_is_left_and_named() {
    let _one = one_at_a_time();
    let w = scratch("left");
    let a = b"one".to_vec();
    let ents = entries(&[hex(&a), hex(b"never packed")]);
    let missing_id = zikaron::hexfmt::encode(&zikaron::entry::entry_id(&ents[2]));
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, ents, &[("a.txt", vec![("", a)])]);
    let plan = extract::plan_at(&kit).expect("a plan");
    assert_eq!(plan.take.iter().map(|o| o.landing_name().to_string()).collect::<Vec<_>>(), vec!["a.txt".to_string()]);
    assert_eq!(plan.left.iter().map(|l| (l.entry_id.clone(), l.why)).collect::<Vec<_>>(), vec![(missing_id, Why::Missing)]);
}

#[test]
fn what_cannot_be_taken_is_refused_by_name_and_writes_nothing() {
    let _one = one_at_a_time();
    let w = scratch("refused");
    let a = b"one".to_vec();
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a)]), &[("a.txt", vec![("", a.clone())])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    assert_eq!(extract::take(&kit, &w.0.join("k").join("manifest.json")).err(), Some(Refused::NotAFolder(w.0.join("k").join("manifest.json").display().to_string())));
    std::fs::write(kit.join("files").join("a.txt"), b"changed").expect("w");
    assert!(matches!(extract::take(&kit, &to), Err(Refused::Kit { .. })));
    let none = w.0.join("none");
    package(&none, zikaron_glue::pack::Form::Folder, entries(&[hex(b"other")]), &[("a.txt", vec![("", a)])]);
    assert_eq!(extract::take(&none, &to).err(), Some(Refused::Nothing));
    assert!(tree(&to).is_empty(), "zero bytes");
}

#[test]
fn a_kit2_name_is_restored_as_written_never_rewritten() {
    let _one = one_at_a_time();
    for bad in ["", ".", "..", "a/b", "a\\b", "CON", "con.txt", "Lpt1", "a:b", "x.", "x ", "tab\there"] {
        assert!(!extract::plain_name(bad), "{bad:?}");
    }
    for good in ["第一章 手稿.pdf", "a.txt", "COM10", "console.log", ".hidden"] {
        assert!(extract::plain_name(good), "{good:?}");
    }
    if cfg!(windows) {
        return;
    }
    let w = scratch("names");
    let a = b"device".to_vec();
    let signed = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("a:b.txt".to_string(), hex(b"inner"))]).0);
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a), signed]), &[("CON", vec![("", a.clone())]), ("Folder", vec![("a:b.txt", b"inner".to_vec())])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (plan, landed) = extract::take(&kit, &to).expect("taken");
    assert!(plan.take.iter().all(|o| o.strict && o.names_hold), "{plan:?}");
    assert_eq!(landed.names.len(), 2);
    assert_eq!(std::fs::read(to.join("CON")).expect("there"), a);
    let dir = plan.take.iter().find(|o| o.kept == Kept::Dir).expect("the folder");
    assert_eq!(std::fs::read(to.join(dir.landing_name()).join("a:b.txt")).expect("there"), b"inner".to_vec(), "its bytes the ones recorded");
}

#[test]
fn two_originals_under_one_name_in_any_case_are_refused() {
    let _one = one_at_a_time();
    let w = scratch("clash");
    let (a, b) = (b"one".to_vec(), b"two".to_vec());
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a), hex(&b)]), &[("Notes.txt", vec![("", a)]), ("notes.txt", vec![("", b)])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    assert!(matches!(extract::take(&kit, &to), Err(Refused::Clash(n)) if n.len() == 1));
    assert!(tree(&to).is_empty());
}

#[test]
fn an_original_changed_after_it_was_judged_is_not_placed() {
    let _one = one_at_a_time();
    let w = scratch("changed");
    let a = b"one".to_vec();
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a)]), &[("a.txt", vec![("", a)])]);
    let plan = extract::plan(&kit).expect("a plan");
    std::fs::write(kit.join("files").join("a.txt"), b"two").expect("w");
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    assert_eq!(extract::land(&plan, &kit, &to).err(), Some(Refused::Changed("a.txt".into())));
    assert!(tree(&to).is_empty(), "nothing placed, the staging folder gone");
}

#[test]
fn a_file_kept_inside_a_signed_folder_is_delivered_by_the_folder() {
    let _one = one_at_a_time();
    let w = scratch("nested-file");
    let src = w.0.join("src").join("Photos");
    std::fs::create_dir_all(&src).expect("a tree");
    std::fs::write(src.join("a.jpg"), b"photo a").expect("w");
    std::fs::write(src.join("b.jpg"), b"photo b").expect("w");
    let folder = dir_content(&src);
    let file = hex(b"photo a");
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[file.clone(), folder.clone()]), &[("Photos", vec![("a.jpg", b"photo a".to_vec()), ("b.jpg", b"photo b".to_vec())])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (plan, landed) = extract::take(&kit, &to).unwrap_or_else(|r| panic!("{r:?}"));
    assert!(plan.left.is_empty(), "both records' originals are delivered: {:?}", plan.left);
    let top = "Photos".to_string();
    assert_eq!(landed.names, vec![top.clone()], "the folder alone");
    assert_eq!(std::fs::read(to.join(&top).join("a.jpg")).expect("the file, inside"), b"photo a".to_vec());
    assert_eq!(dir_content(&to.join(&top)), folder, "the folder whole");
}

#[test]
fn files_of_one_name_in_two_signed_folders_go_with_their_folders() {
    let _one = one_at_a_time();
    let w = scratch("two-folders");
    let (one, two) = (w.0.join("src").join("One"), w.0.join("src").join("Two"));
    for (d, bytes) in [(&one, b"first".to_vec()), (&two, b"second".to_vec())] {
        std::fs::create_dir_all(d).expect("a tree");
        std::fs::write(d.join("note.txt"), bytes).expect("w");
    }
    let kit = w.0.join("k");
    let recs = entries(&[hex(b"first"), hex(b"second"), dir_content(&one), dir_content(&two)]);
    package(&kit, zikaron_glue::pack::Form::Folder, recs, &[("One", vec![("note.txt", b"first".to_vec())]), ("Two", vec![("note.txt", b"second".to_vec())])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (plan, landed) = extract::take(&kit, &to).unwrap_or_else(|r| panic!("{r:?}"));
    assert!(plan.left.is_empty(), "{:?}", plan.left);
    let (one, two) = ("One".to_string(), "Two".to_string());
    assert_eq!(landed.names, vec![one.clone(), two.clone()]);
    assert_eq!(std::fs::read(to.join(&one).join("note.txt")).expect("first"), b"first".to_vec());
    assert_eq!(std::fs::read(to.join(&two).join("note.txt")).expect("second"), b"second".to_vec());
}

#[test]
fn a_file_attached_alone_lands_under_its_own_name_before_a_folders_copy() {
    let _one = one_at_a_time();
    let w = scratch("alone-first");
    let bytes = b"the contract".to_vec();
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&bytes)]), &[("z.pdf", vec![("", bytes.clone())]), ("a-docs", vec![("final.pdf", bytes.clone())])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (_, landed) = extract::take(&kit, &to).unwrap_or_else(|r| panic!("{r:?}"));
    assert_eq!(landed.names, vec!["z.pdf".to_string()]);
    assert_eq!(std::fs::read(to.join("z.pdf")).expect("there"), bytes);
}

#[test]
fn a_file_inside_a_folder_lands_under_its_own_last_name() {
    let _one = one_at_a_time();
    let w = scratch("last-name");
    let bytes = b"chapter one".to_vec();
    let kit = w.0.join("k");
    let mut b = zikaron_glue::pack::Bundle { entries: entries(&[hex(&bytes)]), ..Default::default() };
    let path = format!("{}/{}", zikaron_glue::kitnames::kit_segment("手稿").expect("k"), zikaron_glue::kitnames::kit_segment("第一章 草稿.txt").expect("k"));
    b.names.push((path.clone(), "第一章 草稿.txt".to_string()));
    b.files.push((path.clone(), bytes.clone()));
    b.contents.push(path);
    zikaron_glue::pack::export_as(&kit, b, zikaron_glue::pack::Form::Folder).expect("exports");
    let plan = extract::plan(&kit).unwrap_or_else(|r| panic!("{r:?}"));
    let file = plan.take.iter().find(|o| o.kept == Kept::File).expect("the file");
    assert!(file.names_hold, "{file:?}");
    assert_eq!(file.landing_name(), "第一章 草稿.txt");
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let landed = extract::land(&plan, &kit, &to).unwrap_or_else(|r| panic!("{r:?}"));
    assert_eq!(landed.names, vec!["第一章 草稿.txt".to_string()]);
    assert_eq!(std::fs::read(to.join("第一章 草稿.txt")).expect("there"), bytes);
}

fn spelled_tree(w: &Scratch) -> (PathBuf, Vec<zikaron_glue::recording::DirRow>) {
    let src = w.0.join("tree");
    std::fs::create_dir_all(src.join("sub").join("deeper")).expect("a tree");
    std::fs::write(src.join("a.txt"), b"a").expect("w");
    std::fs::write(src.join("sub").join("b.txt"), b"b").expect("w");
    std::fs::write(src.join("sub").join("deeper").join("c.txt"), b"c").expect("w");
    std::fs::write(src.join("sub0.txt"), b"zero").expect("w");
    let (rows, _) = zikaron_glue::recording::dir_rows(&src).expect("walks");
    (src, rows)
}

const SLASH_CONTENT: &str = "0x8fe7b9e3a4101514c152dc6f373e62419c5ae0612c1b74cae09bffc418433f0a";
const BACKSLASH_CONTENT: &str = "0x24374221625602543896b8d26a071b47a11cdb2ca555183dfd5eabde3d0dfe51";

#[test]
fn a_directorys_rows_are_spelled_with_a_slash_on_every_system() {
    let w = scratch("slash");
    let (_, rows) = spelled_tree(&w);
    let paths: Vec<&str> = rows.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, vec!["a.txt", "sub/b.txt", "sub/deeper/c.txt", "sub0.txt"]);
    let all = zikaron_glue::recording::dir_contents(&rows);
    let spelled: Vec<String> = all.iter().map(|(d, _)| zikaron::hexfmt::encode(d)).collect();
    assert_eq!(spelled, vec![SLASH_CONTENT.to_string(), BACKSLASH_CONTENT.to_string()], "signed now, then the old Windows spelling");
    assert_eq!(zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&rows).0), SLASH_CONTENT, "signing writes the slash spelling only");
    assert!(zikaron_glue::recording::dir_holds(&rows, BACKSLASH_CONTENT) && zikaron_glue::recording::dir_holds(&rows, SLASH_CONTENT));
    let flat = vec![("a.txt".to_string(), hex(b"a"))];
    assert_eq!(zikaron_glue::recording::dir_contents(&flat).len(), 1);
    assert!(zikaron_glue::recording::old_dir_content(&flat).is_none());
}

#[test]
fn a_kit2_package_does_not_join_an_old_spelling_record() {
    let _one = one_at_a_time();
    let w = scratch("backslash");
    let kit = w.0.join("k");
    let files = vec![("a.txt", b"a".to_vec()), ("sub/b.txt", b"b".to_vec()), ("sub/deeper/c.txt", b"c".to_vec()), ("sub0.txt", b"zero".to_vec())];
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[BACKSLASH_CONTENT.to_string()]), &[("tree", files)]);
    let rows = zikaron_glue::recordcheck::records(&zikaron_glue::read::kit_entries(&kit).expect("entries"), &zikaron_glue::recordcheck::originals_in_dir(&kit), None);
    assert_eq!(rows.iter().map(|r| r.original).collect::<Vec<_>>(), vec![zikaron_glue::recordcheck::Original::Missing], "joined by the row's value only");
    let plan = extract::plan(&kit).expect("a plan");
    assert!(plan.take.is_empty() && plan.left.len() == 1, "{plan:?}");
}

#[test]
fn a_directory_recorded_in_both_spellings_is_taken_out_once() {
    let _one = one_at_a_time();
    let w = scratch("both");
    let kit = w.0.join("k");
    let files = vec![("a.txt", b"a".to_vec()), ("sub/b.txt", b"b".to_vec()), ("sub/deeper/c.txt", b"c".to_vec()), ("sub0.txt", b"zero".to_vec())];
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[SLASH_CONTENT.to_string(), BACKSLASH_CONTENT.to_string()]), &[("tree", files)]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (plan, landed) = extract::take(&kit, &to).expect("taken");
    assert_eq!((plan.take.len(), landed.names.clone(), plan.left.len()), (1, vec!["tree".to_string()], 1));
}

fn rows_changed(kit: &Path, change: &dyn Fn(&str, &mut Vec<(String, Value)>)) {
    let at = kit.join("manifest.json");
    let mut v = zikaron::json::parse(&std::fs::read(&at).expect("a manifest")).expect("reads");
    let Value::Obj(ms) = &mut v else { panic!("an object") };
    let (_, Value::Arr(rows)) = ms.iter_mut().find(|(k, _)| k == "contents").expect("contents") else { panic!("an array") };
    for r in rows.iter_mut() {
        let Value::Obj(cells) = r else { panic!("an object") };
        let path = cells.iter().find(|(k, _)| k == "path").and_then(|(_, p)| p.as_str()).expect("a path").to_string();
        change(&path, cells);
    }
    std::fs::write(&at, zikaron::json::canon_bytes(&v)).expect("w");
}

fn manifest_rows(kit: &Path) -> Vec<(String, Option<String>)> {
    let m = std::fs::read(kit.join("manifest.json")).expect("a manifest");
    zikaron_glue::recordcheck::contents_named(&m).into_iter().map(|(_, p, n)| (p, n)).collect()
}

#[test]
fn a_directory_original_carries_its_name_in_its_row_and_lands_under_it() {
    let _one = one_at_a_time();
    let w = scratch("dirname");
    let inner = [("甲/第一.txt", b"one".to_vec())];
    let plain = [("two.txt", b"two".to_vec())];
    let c_named = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("甲/第一.txt".to_string(), hex(b"one"))]).0);
    let c_plain = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("two.txt".to_string(), hex(b"two"))]).0);
    let file = b"file".to_vec();
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[c_named, c_plain, hex(&file)]), &[("田野 笔记", inner.to_vec()), ("plain-folder", plain.to_vec()), ("報告.pdf", vec![("", file.clone())])]);
    let rows = manifest_rows(&kit);
    let named_path = zikaron_glue::kitnames::kit_segment("田野 笔记").expect("kit-legal");
    let file_path = zikaron_glue::kitnames::kit_segment("報告.pdf").expect("kit-legal");
    assert!(rows.contains(&(named_path, Some("田野 笔记".to_string()))), "{rows:?}");
    assert!(rows.contains(&("plain-folder".to_string(), None)), "a kit path that spells the name carries none: {rows:?}");
    assert!(rows.contains(&(file_path, None)), "a file original's name is its files row's: {rows:?}");
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    extract::take(&kit, &to).expect("taken");
    assert!(to.join("田野 笔记").join("甲").join("第一.txt").is_file());
    assert!(to.join("plain-folder").join("two.txt").is_file());
    assert_eq!(std::fs::read(to.join("報告.pdf")).expect("there"), file);
}

#[test]
fn a_row_without_a_name_lands_under_its_path_and_a_file_rows_name_is_not_read() {
    let _one = one_at_a_time();
    let w = scratch("noname");
    let c = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("x.txt".to_string(), hex(b"x"))]).0);
    let file = b"file".to_vec();
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[c, hex(&file)]), &[("Named Folder", vec![("x.txt", b"x".to_vec())]), ("報告.pdf", vec![("", file.clone())])]);
    let (dir_path, file_path) = (zikaron_glue::kitnames::kit_segment("Named Folder").expect("kit-legal"), zikaron_glue::kitnames::kit_segment("報告.pdf").expect("kit-legal"));
    let fp = file_path.clone();
    rows_changed(&kit, &|p, cells| {
        cells.retain(|(k, _)| k != "name_md");
        if p == fp {
            cells.push(("name_md".into(), Value::Str("别的名字.pdf".into())));
        }
    });
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    extract::take(&kit, &to).expect("taken");
    assert!(to.join(&dir_path).join("x.txt").is_file(), "the last segment of its path");
    assert!(to.join("報告.pdf").is_file() && !to.join("别的名字.pdf").exists() && !to.join(&file_path).exists());
}

#[test]
fn a_directory_name_that_is_no_one_name_is_refused_and_two_alike_clash() {
    let _one = one_at_a_time();
    if cfg!(windows) {
        return;
    }
    let w = scratch("badname");
    let c = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("x.txt".to_string(), hex(b"x"))]).0);
    for bad in ["", ".", "..", "../up", "a/b"] {
        let kit = w.0.join(format!("k{}", bad.len()));
        package(&kit, zikaron_glue::pack::Form::Folder, entries(&[c.clone()]), &[("Named Folder", vec![("x.txt", b"x".to_vec())])]);
        rows_changed(&kit, &|_, cells| {
            cells.retain(|(k, _)| k != "name_md");
            cells.push(("name_md".into(), Value::Str(bad.into())));
        });
        let to = w.0.join(format!("out{}", bad.len()));
        std::fs::create_dir_all(&to).expect("a folder");
        assert_eq!(extract::take(&kit, &to).err(), Some(extract::Refused::Name(vec![bad.to_string()])), "{bad:?}");
        assert_eq!(std::fs::read_dir(&to).expect("reads").count(), 0, "{bad:?}: nothing landed");
    }
    let long = "n".repeat(300);
    let kit = w.0.join("long");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[c.clone()]), &[("Named Folder", vec![("x.txt", b"x".to_vec())])]);
    rows_changed(&kit, &|_, cells| {
        cells.retain(|(k, _)| k != "name_md");
        cells.push(("name_md".into(), Value::Str(long.clone())));
    });
    let to = w.0.join("out-long");
    std::fs::create_dir_all(&to).expect("a folder");
    assert_eq!(extract::take(&kit, &to).err(), Some(extract::Refused::Name(vec![long.clone()])), "a name too long");
    assert_eq!(std::fs::read_dir(&to).expect("reads").count(), 0, "too long: nothing landed");
    let (c1, c2) = (c.clone(), zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("y.txt".to_string(), hex(b"y"))]).0));
    let kit = w.0.join("clash");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[c1, c2]), &[("Same Name", vec![("x.txt", b"x".to_vec())]), ("second", vec![("y.txt", b"y".to_vec())])]);
    rows_changed(&kit, &|p, cells| {
        if p == "second" {
            cells.push(("name_md".into(), Value::Str("SAME name".into())));
        }
    });
    let to = w.0.join("out-clash");
    std::fs::create_dir_all(&to).expect("a folder");
    assert!(matches!(extract::take(&kit, &to), Err(extract::Refused::Clash(_))));
    assert_eq!(std::fs::read_dir(&to).expect("reads").count(), 0);
}

#[test]
fn a_directory_name_that_is_not_text_fails_the_kit_law() {
    let _one = one_at_a_time();
    let w = scratch("nottext");
    let c = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("x.txt".to_string(), hex(b"x"))]).0);
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[c]), &[("Named Folder", vec![("x.txt", b"x".to_vec())])]);
    rows_changed(&kit, &|_, cells| {
        cells.retain(|(k, _)| k != "name_md");
        cells.push(("name_md".into(), Value::Int(7)));
    });
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    assert!(matches!(extract::take(&kit, &to), Err(extract::Refused::Kit { .. })));
}

#[test]
fn a_directory_original_delivers_what_was_put_under_it() {
    use zikaron_glue::kitnames::put_directory;
    use zikaron_glue::pack::{enumerate, Bundle, Trouble};
    let dir = |rel: &str, bytes: &[u8]| vec![(rel.to_string(), bytes.to_vec())];
    let mut two = Bundle::default();
    put_directory("src", dir("a.txt", b"first"), &mut two).expect("kit-legal");
    put_directory("src", dir("b.txt", b"second"), &mut two).expect("kit-legal");
    assert!(matches!(enumerate(two), Err(Trouble::Duplicate(p)) if p == "src"));
    let mut room = Bundle::default();
    put_directory("terms", dir("a.txt", b"first"), &mut room).expect("kit-legal");
    room.files.push(("terms/0x01/doc.txt".to_string(), b"terms".to_vec()));
    assert!(matches!(enumerate(room), Err(Trouble::Duplicate(p)) if p == "terms"));
    let mut one = Bundle::default();
    put_directory("src", dir("a.txt", b"first"), &mut one).expect("kit-legal");
    put_directory("other", dir("a.txt", b"first"), &mut one).expect("kit-legal");
    assert!(enumerate(one).is_ok(), "two folders of two names");
}

fn folds_case(at: &Path) -> bool {
    let probe = at.join("Case-Probe");
    std::fs::create_dir(&probe).expect("a probe");
    let folds = at.join("case-probe").is_dir();
    std::fs::remove_dir(&probe).expect("gone");
    folds
}

#[test]
fn folders_differing_in_case_clash_only_where_the_system_folds_them() {
    let _one = one_at_a_time();
    let w = scratch("case-folders");
    let files = vec![("Data/x.txt", b"x".to_vec()), ("data/y.txt", b"y".to_vec())];
    let rows: Vec<zikaron_glue::recording::DirRow> = files.iter().map(|(n, b)| (n.to_string(), hex(b))).collect();
    let content = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&rows).0);
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[content]), &[("Tree", files)]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    match extract::take(&kit, &to) {
        Err(Refused::Clash(names)) => {
            assert!(folds_case(&w.0), "a clash only where the system folds");
            assert_eq!(names, vec!["data".to_string()]);
            assert!(tree(&to).is_empty(), "nothing landed");
        }
        Ok(_) => {
            assert!(!folds_case(&w.0), "both land only where the system holds them apart");
            assert_eq!(std::fs::read(to.join("Tree/Data/x.txt")).expect("x"), b"x");
            assert_eq!(std::fs::read(to.join("Tree/data/y.txt")).expect("y"), b"y");
        }
        Err(other) => panic!("neither: {other:?}"),
    }
}

#[test]
fn each_record_carries_its_withdrawal_and_its_notes() {
    let (c1, c2) = (format!("0x{}", "11".repeat(32)), format!("0x{}", "22".repeat(32)));
    let mut items = entries(&[c1, c2]);
    let id = |b: &[u8]| zikaron::hexfmt::encode(&zikaron::entry::entry_id(b));
    let (g, h1, h2) = (id(&items[0]), id(&items[1]), id(&items[2]));
    let push = |items: &mut Vec<Vec<u8>>, kind: &str, body: Value| {
        let prev = id(items.last().expect("one"));
        let seq = items.len() as u64;
        items.push(seal(kind, seq, Some(&prev), body));
    };
    let subject = |s: &str| ("subject".to_string(), Value::Str(s.to_string()));
    push(&mut items, "retraction", Value::Obj(vec![subject(&h1)]));
    push(&mut items, "retraction", Value::Obj(vec![subject(&h1)]));
    push(&mut items, "retraction", Value::Obj(vec![subject(&g)]));
    push(&mut items, "annotation", Value::Obj(vec![("note_md".into(), Value::Str("看这一件".into())), subject(&h2)]));
    let rows = zikaron_glue::recordcheck::records(&items, &[], None);
    let first = rows.iter().find(|r| r.id == h1).expect("the first record");
    let second = rows.iter().find(|r| r.id == h2).expect("the second record");
    assert_eq!(first.deleted.as_ref().map(|d| (d.by.clone(), d.seq)), Some((id(&items[3]), 3)), "withdrawn by the first retraction");
    assert!(first.notes.is_empty());
    assert!(second.deleted.is_none(), "the misfired retractions withdraw nothing");
    assert_eq!(second.notes.iter().map(|n| (n.seq, n.note.as_str())).collect::<Vec<_>>(), [(6, "看这一件")]);
    assert_eq!(zikaron_seal_withdrawn(&items), vec![h1], "an export leaves out the same records' originals");
}

fn zikaron_seal_withdrawn(items: &[Vec<u8>]) -> Vec<String> {
    zikaron_glue::retraction::read_items(items).1.deleted.into_keys().collect()
}
