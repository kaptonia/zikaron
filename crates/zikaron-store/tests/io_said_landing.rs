//! Continues `io_said.rs`: a failing landing is `E_IO` naming the operation and the target path (each of the
//! three landing ways forced unsupported by the platform layer); a failing write of the temporary file is
//! `E_IO` naming `write` and the temporary path, and no temporary file is left. Its own test binary with a
//! single test, because the forced failures (and the file-size limit) are process-wide.

#![cfg(unix)]

use zikaron_store::{Code, EntryName, LedgerDir};

fn scratch(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("zk-store-io-land-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("scratch");
    p
}

fn name() -> EntryName {
    EntryName::parse(&"c".repeat(64)).expect("a name")
}

/// The files in a ledger's directory, by name.
fn names_in(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).expect("listed").map(|e| e.expect("entry").file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[repr(C)]
struct Rlimit {
    cur: u64,
    max: u64,
}

unsafe extern "C" {
    fn getrlimit(resource: i32, rlp: *mut Rlimit) -> i32;
    fn setrlimit(resource: i32, rlp: *const Rlimit) -> i32;
    fn signal(sig: i32, handler: usize) -> usize;
}

/// `RLIMIT_FSIZE` and `SIGXFSZ`: the same numbers on macOS and Linux.
const RLIMIT_FSIZE: i32 = 1;
const SIGXFSZ: i32 = 25;
const SIG_IGN: usize = 1;

/// Run `f` with no file of this process allowed to grow past zero bytes (a write past it fails with the
/// system's "file too large", the signal it would send ignored); the limit is put back after.
fn with_no_room<T>(f: impl FnOnce() -> T) -> T {
    let mut was = Rlimit { cur: 0, max: 0 };
    // SAFETY: plain system calls on a struct of the system's layout (two 64-bit counts on both systems).
    unsafe {
        assert_eq!(getrlimit(RLIMIT_FSIZE, &mut was), 0, "read the limit");
        signal(SIGXFSZ, SIG_IGN);
        assert_eq!(setrlimit(RLIMIT_FSIZE, &Rlimit { cur: 0, max: was.max }), 0, "lower the limit");
    }
    let got = f();
    // SAFETY: as above; the soft limit goes back to what it was, never above the hard one.
    unsafe {
        assert_eq!(setrlimit(RLIMIT_FSIZE, &was), 0, "the limit put back");
    }
    got
}

/// A landing that fails (every way unsupported) reports `E_IO` with each way's operation and the target
/// path; a failing write of the temporary file reports `E_IO` with `write` and the temporary path. Neither
/// leaves a file behind.
#[test]
fn a_landing_or_a_temporary_write_that_fails_says_the_operation_and_path() {
    let base = scratch("said");

    // The landing: none of the three ways supported.
    let l = LedgerDir::open_or_create(base.join("no-way")).expect("a ledger");
    zikaron_os::pretend_unsupported(zikaron_os::way::LINK | zikaron_os::way::RENAME_NEW | zikaron_os::way::CLAIM);
    let t = l.append(&name(), b"one").err();
    zikaron_os::pretend_unsupported(0);
    let t = t.expect("refused");
    assert_eq!(t.code, Code::Io);
    let said = t.said.clone().unwrap_or_default();
    let target = l.root().join(zikaron_store::layout::entry_file_name(&name())).display().to_string();
    for way in ["hard link ", "rename that never replaces ", "exclusive create "] {
        assert!(said.contains(way), "the operation {way:?} said: {said}");
    }
    assert!(said.contains(&target), "the target path said: {said}");
    assert!(names_in(l.root()).is_empty(), "nothing landed, the temporary file gone: {:?}", names_in(l.root()));

    // The landing by the third way: the name claimed, then the rename over the claim failing for the system's
    // own reason (the temporary file taken away before it): the rename's words, both paths, the claim gone.
    let l = LedgerDir::open_or_create(base.join("rename-fails")).expect("a ledger");
    let staged = l.stage(&name(), b"one").expect("staged");
    let tmp = staged.tmp_path().display().to_string();
    std::fs::remove_file(staged.tmp_path()).expect("the temporary file taken away");
    zikaron_os::pretend_unsupported(zikaron_os::way::LINK | zikaron_os::way::RENAME_NEW);
    let t = staged.link().err();
    zikaron_os::pretend_unsupported(0);
    let t = t.expect("refused");
    assert_eq!(t.code, Code::Io);
    let said = t.said.clone().unwrap_or_default();
    let target = l.root().join(zikaron_store::layout::entry_file_name(&name())).display().to_string();
    assert!(said.starts_with(&format!("replacing rename {tmp} \u{2192} {target}: ")), "{said}");
    assert!(names_in(l.root()).is_empty(), "the claim is not left: {:?}", names_in(l.root()));

    // The temporary file's bytes cannot be written: `write <tmp>` and the system's words, nothing left.
    let l = LedgerDir::open_or_create(base.join("no-room")).expect("a ledger");
    let t = with_no_room(|| l.append(&name(), b"one").err()).expect("refused");
    assert_eq!(t.code, Code::Io);
    let said = t.said.clone().unwrap_or_default();
    let root = l.root().display().to_string();
    assert!(said.starts_with(&format!("write {root}")) && said.contains(".zks-tmp-") && said.contains(": "), "{said}");
    assert!(names_in(l.root()).is_empty(), "the temporary file is not left: {:?}", names_in(l.root()));

    let _ = std::fs::remove_dir_all(&base);
}
