use zikaron_store::{Code, EntryName, LedgerDir, Why};

fn scratch(tag: &str) -> LedgerDir {
    let p = std::env::temp_dir().join(format!("zk-store-side-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    LedgerDir::open_or_create(&p).expect("a ledger")
}

fn entry_file(c: char) -> String {
    format!("{}.entry", c.to_string().repeat(64))
}

fn name(c: char) -> EntryName {
    EntryName::parse(&c.to_string().repeat(64)).expect("a name")
}

fn with_stray(tag: &str, stray: &str, dir: bool) -> (Result<Vec<Vec<u8>>, (Code, Vec<String>)>, Vec<(String, Why)>) {
    let l = scratch(tag);
    l.append(&name('a'), b"one").expect("an entry");
    let at = l.root().join(stray);
    if dir {
        std::fs::create_dir(&at).expect("a directory");
    } else {
        std::fs::write(&at, b"\x00\x05\x16\x07 system bytes").expect("a stray");
    }
    let strict = l.pile().map(|p| p.items).map_err(|t| (t.code, t.names));
    let lenient = l.survey().expect("the lenient read").skipped.into_iter().map(|s| (s.name, s.why)).collect();
    let _ = std::fs::remove_dir_all(l.root());
    (strict, lenient)
}

#[test]
fn the_systems_files_beside_this_crates_names_are_the_systems() {
    let entry = entry_file('a');
    let tmp = ".zks-tmp-0123456789abcdef";
    for (form, stray) in [
        ("an entry's sidecar", format!("._{entry}")),
        ("a temporary file's sidecar", format!("._{tmp}")),
        ("the folder view's file", ".DS_Store".to_string()),
    ] {
        let (strict, lenient) = with_stray(&format!("sys{}", stray.len()), &stray, false);
        assert_eq!(strict, Ok(vec![b"one".to_vec()]), "{form}: the strict read accounts for it and reads on");
        assert_eq!(lenient, vec![(stray.clone(), Why::SystemSide)], "{form}: the lenient read skips it by name, as the system's");
    }
}

#[test]
fn every_other_name_is_refused_as_before() {
    let entry = entry_file('a');
    for (form, stray, why) in [
        ("a sidecar of a name this crate never writes", "._x".to_string(), Why::ForeignName),
        ("a sidecar of a sidecar", format!("._._{entry}"), Why::ForeignName),
        ("a sidecar of an entry-like name in capitals", format!("._{}", entry.to_uppercase()), Why::ForeignName),
        ("the bare sidecar prefix", "._".to_string(), Why::ForeignName),
        ("the folder view's name in another case", ".ds_store".to_string(), Why::ForeignName),
        ("some other file", "notes.txt".to_string(), Why::ForeignName),
    ] {
        let (strict, lenient) = with_stray(&format!("other{}", stray.len()), &stray, false);
        assert_eq!(strict, Err((Code::Unaccounted, vec![stray.clone()])), "{form}: the whole read refused, named");
        assert_eq!(lenient, vec![(stray.clone(), why)], "{form}");
    }
}

#[test]
fn a_directory_under_a_system_name_is_not_the_systems() {
    let entry = entry_file('a');
    for stray in [".DS_Store".to_string(), format!("._{entry}")] {
        let (strict, lenient) = with_stray(&format!("dir{}", stray.len()), &stray, true);
        assert_eq!(strict, Err((Code::Unaccounted, vec![stray.clone()])), "{stray}: a directory is not what the system writes there");
        assert_eq!(lenient, vec![(stray.clone(), Why::NotAFile)], "{stray}");
    }
}
