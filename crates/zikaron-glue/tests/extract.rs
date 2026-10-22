//! Taking a record package's originals out (`zikaron_glue::extract`), on packages laid out by the crate's own
//! export: which originals go (a file by its bytes, a signed directory by its recomputed manifest), under which
//! names, and the chosen folder either holding all of them or left as it was.

use std::path::{Path, PathBuf};
use zikaron::json::Value;
use zikaron_glue::extract::{self, Kept, Refused, Why};

const KEY: [u8; 32] = [
    0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04, 0x49, 0x66, 0xf0, 0x94, 0x53, 0x89, 0xdc, 0x9e, 0x86, 0xda, 0xe8, 0x8c, 0x7a, 0x84, 0x12,
    0xf4, 0x60, 0x3b, 0x6b, 0x78, 0x69, 0x0d,
];

/// One test at a time: placing failing on purpose (`extract::pretend_place_fails_after`) holds for the whole process,
/// and the tests here run on threads of one process.
fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A per-test temporary directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A symbolic link at `at` to `to`, made by the system's own tool (the platform calls live in the platform crate
/// only); `false` where there is no such tool, and the test that needs one has nothing to stand on.
fn link(to: &Path, at: &Path) -> bool {
    std::process::Command::new("ln").arg("-s").arg(to).arg(at).status().map(|s| s.success()).unwrap_or(false)
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-glue-extract-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    Scratch(p)
}

/// A signed entry of this ledger.
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

/// A genesis and one record per content, in a line.
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

/// A directory's content as signing it computes it.
fn dir_content(dir: &Path) -> String {
    let (rows, _) = zikaron_glue::recording::dir_rows(dir).expect("walks");
    zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&rows).0)
}

/// A package at `out` (a folder, or one file) carrying `entries`, the originals given (each original's name and
/// its files, (name inside it, bytes); one file with no inside name for a file original) put in as the desktop puts
/// them (`kitnames::put_original`, the name table).
fn package(out: &Path, form: zikaron_glue::pack::Form, entries: Vec<Vec<u8>>, originals: &[(&str, Vec<(&str, Vec<u8>)>)]) {
    let mut b = zikaron_glue::pack::Bundle { entries, note: String::new(), ..Default::default() };
    let mut names = Vec::new();
    for (name, files) in originals {
        for (inner, bytes) in files {
            let path = if inner.is_empty() { name.to_string() } else { format!("{name}/{inner}") };
            zikaron_glue::kitnames::put_original(&path, bytes.clone(), &mut b, &mut names).expect("kit-legal");
        }
    }
    if !names.is_empty() {
        b.files.push((zikaron_glue::kitnames::NAMES_FILE.to_string(), zikaron_glue::kitnames::names_table(&names)));
    }
    zikaron_glue::pack::export_as(out, b, form).expect("exports");
}

/// What a folder holds, path by path with bytes (empty folders by name).
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

/// A file original under a name the kit cannot carry is taken out under that name again, its bytes the ones
/// recorded, from a folder package and from one file alike. Victim: whoever receives a package and wants the
/// original by the name it had, not `1-91-xxxx.pdf`.
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

/// A signed directory is kept file by file; it is judged by its manifest recomputed from those files under the
/// names they were signed with (the function signing a directory uses), so the record check calls it a match and
/// it is taken out whole, recomputing to the content recorded. Victim: whoever signed a folder and got "missing".
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
    assert_eq!((plan.take[0].kept, landed.names.clone()), (Kept::Dir, vec!["Sound Archive".to_string()]));
    assert_eq!(dir_content(&to.join("Sound Archive")), content, "taken out, it recomputes to the content recorded");
}

/// A name already in the chosen folder refuses the whole batch by name, and nothing is written. Victim: whoever
/// would have a file of the same name overwritten, or half the batch landed.
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

/// A link at a name in the chosen folder is refused by name and never followed: nothing lands where it points.
/// Victim: whoever is led to write outside the folder they chose.
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

/// Placing failing midway takes back what was placed and clears the staging folder: the chosen folder is as it
/// was. Victim: whoever is left with half a batch.
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

/// Only originals that match their records go; a record whose original the package lacks is left and named.
/// Victim: whoever would take a stray file for an original.
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

/// A package the kit law refuses gives nothing, judged when it is read again; a package holding no matching
/// original gives nothing, by name; a place that is not a folder is refused by name.
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

/// A name that cannot name a file on some system (a device name, a reserved character, a trailing dot) is not
/// used: the original lands under its package name and the plan says so; a directory with one such name inside lands
/// whole under its package names, each file still the bytes recorded.
#[test]
fn a_name_that_cannot_name_a_file_gives_way_to_the_package_name() {
    let _one = one_at_a_time();
    for bad in ["", ".", "..", "a/b", "a\\b", "CON", "con.txt", "Lpt1", "a:b", "x.", "x ", "tab\there"] {
        assert!(!extract::plain_name(bad), "{bad:?}");
    }
    for good in ["第一章 手稿.pdf", "a.txt", "COM10", "console.log", ".hidden"] {
        assert!(extract::plain_name(good), "{good:?}");
    }
    let w = scratch("names");
    let a = b"device".to_vec();
    // A folder signed with a file named `a:b.txt` (a name some system will not take): its content as signing computes
    // it, from its one row.
    let signed = zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&[("a:b.txt".to_string(), hex(b"inner"))]).0);
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&a), signed]), &[("CON", vec![("", a.clone())]), ("Folder", vec![("a:b.txt", b"inner".to_vec())])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (plan, landed) = extract::take(&kit, &to).expect("taken");
    assert!(plan.take.iter().all(|o| !o.names_hold), "both give way: {plan:?}");
    assert_eq!(landed.names.len(), 2);
    let file = plan.take.iter().find(|o| o.kept == Kept::File).expect("the file");
    assert_eq!(std::fs::read(to.join(file.landing_name())).expect("there"), a);
    let dir = plan.take.iter().find(|o| o.kept == Kept::Dir).expect("the folder");
    let inner = &dir.members[0].path;
    assert_eq!(std::fs::read(to.join(inner)).expect("there"), b"inner".to_vec(), "under its package names, its bytes the ones recorded");
}

/// Two originals landing under one name in any case are refused by name before anything is written (a
/// case-folding file system takes them as one).
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

/// Each original is checked against its record after it is laid out and before anything is placed: a package whose
/// file changed after it was judged is refused by name and the folder is as it was. Victim: whoever would be handed
/// bytes that are not the original recorded.
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

/// A file signed alone that the package keeps only inside a signed folder it also carries (the folder attached, the
/// file not) is delivered by the folder: the folder is taken out whole, the file inside it with the bytes recorded,
/// and nothing is placed again beside it; its record is not left. Victim: whoever signed a file and later the folder
/// holding it, and sends the folder: a stray copy beside the folder, or (two such folders holding files of one name)
/// the whole take-out refused as a clash.
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
    assert_eq!(landed.names, vec!["Photos".to_string()], "the folder alone");
    assert_eq!(std::fs::read(to.join("Photos").join("a.jpg")).expect("the file, inside"), b"photo a".to_vec());
    assert_eq!(dir_content(&to.join("Photos")), folder, "the folder whole");
}

/// Two signed folders that each hold a file of one name, each file also signed alone, only the folders attached:
/// each file goes with its folder, no clash. Victim: whoever signed two such folders and their files: the whole
/// take-out refused.
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
    assert_eq!(landed.names, vec!["One".to_string(), "Two".to_string()]);
    assert_eq!(std::fs::read(to.join("One").join("note.txt")).expect("first"), b"first".to_vec());
    assert_eq!(std::fs::read(to.join("Two").join("note.txt")).expect("second"), b"second".to_vec());
}

/// The same bytes attached alone and also inside a folder attached beside them: the file lands under the name it was
/// signed with alone, not the folder's copy's. Victim: whoever attached `z.pdf` and a folder holding the same bytes as
/// `final.pdf`: the file taken out under the folder's name.
#[test]
fn a_file_attached_alone_lands_under_its_own_name_before_a_folders_copy() {
    let _one = one_at_a_time();
    let w = scratch("alone-first");
    let bytes = b"the contract".to_vec();
    let kit = w.0.join("k");
    // The folder itself is not signed: only the file is, and the package carries it twice, the folder's copy first in
    // the package's order (`a-docs/…` before `z.pdf`).
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&bytes)]), &[("z.pdf", vec![("", bytes.clone())]), ("a-docs", vec![("final.pdf", bytes.clone())])]);
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (_, landed) = extract::take(&kit, &to).unwrap_or_else(|r| panic!("{r:?}"));
    assert_eq!(landed.names, vec!["z.pdf".to_string()]);
    assert_eq!(std::fs::read(to.join("z.pdf")).expect("there"), bytes);
}

/// A file kept inside a folder the package carries, signed alone while the folder is not: it lands under the last name
/// it was signed with, which holds; its names hold judged on that name, not the whole path. Victim: whoever sends a
/// file of a folder whose name the kit cannot carry (a Chinese, capital or spaced name): the file taken out under
/// the package's transliterated name.
#[test]
fn a_file_inside_a_folder_lands_under_its_own_last_name() {
    let _one = one_at_a_time();
    let w = scratch("last-name");
    let bytes = b"chapter one".to_vec();
    let kit = w.0.join("k");
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[hex(&bytes)]), &[("手稿", vec![("第一章 草稿.txt", bytes.clone())])]);
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

/// A tree of four files, two of them below folders, one beside a folder whose name sorts between the two
/// spellings (`sub0.txt`): its rows as signing walks them.
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

/// The content a directory's rows give now (`/` between names, on every system) and in the spelling directories
/// signed on Windows had before (`\`), each worked out apart from the code under test: the sha256 of the canonical
/// manifest, rows sorted by their spelled path.
const SLASH_CONTENT: &str = "0x8fe7b9e3a4101514c152dc6f373e62419c5ae0612c1b74cae09bffc418433f0a";
const BACKSLASH_CONTENT: &str = "0x24374221625602543896b8d26a071b47a11cdb2ca555183dfd5eabde3d0dfe51";

/// A directory's manifest spells its paths with `/` on every system, so a tree signed on macOS or Linux keeps the
/// content it had, and the same tree signed on Windows now gets that content too; the old Windows spelling is
/// another content, reordered by its own spelling. Victim: whoever signs a folder on one system and verifies or takes
/// it out on another.
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
    // A folder with no folder inside spells alike either way: one content.
    let flat = vec![("a.txt".to_string(), hex(b"a"))];
    assert_eq!(zikaron_glue::recording::dir_contents(&flat).len(), 1);
    assert!(zikaron_glue::recording::old_dir_content(&flat).is_none());
}

/// A record of a directory signed on Windows in the old spelling matches that directory kept in a package, and the
/// directory is taken out whole and checked again against that record. Victim: whoever signed a folder on Windows
/// with an earlier version and verifies or takes it out on macOS or Linux, getting "missing".
#[test]
fn a_directory_recorded_in_the_old_windows_spelling_matches_and_is_taken_out() {
    let _one = one_at_a_time();
    let w = scratch("backslash");
    let (_, _) = spelled_tree(&w);
    let kit = w.0.join("k");
    let files = vec![("a.txt", b"a".to_vec()), ("sub/b.txt", b"b".to_vec()), ("sub/deeper/c.txt", b"c".to_vec()), ("sub0.txt", b"zero".to_vec())];
    package(&kit, zikaron_glue::pack::Form::Folder, entries(&[BACKSLASH_CONTENT.to_string()]), &[("tree", files)]);
    let rows = zikaron_glue::recordcheck::records(&zikaron_glue::read::kit_entries(&kit).expect("entries"), &zikaron_glue::recordcheck::originals_in_dir(&kit), None);
    assert_eq!(rows.iter().map(|r| r.original).collect::<Vec<_>>(), vec![zikaron_glue::recordcheck::Original::Match], "the old spelling's record matches");
    let to = w.0.join("out");
    std::fs::create_dir_all(&to).expect("a folder");
    let (plan, landed) = extract::take(&kit, &to).expect("taken");
    assert_eq!((plan.take.len(), plan.take[0].kept, landed.names.clone()), (1, Kept::Dir, vec!["tree".to_string()]));
    let (back, _) = zikaron_glue::recording::dir_rows(&to.join("tree")).expect("walks");
    assert!(zikaron_glue::recording::dir_holds(&back, BACKSLASH_CONTENT), "taken out, it holds the record");
}

/// The same folder recorded twice, once in each spelling, in one package: taken out once, not refused as two
/// originals under one name.
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
    assert_eq!((plan.take.len(), landed.names.clone(), plan.left.len()), (1, vec!["tree".to_string()], 0));
}
