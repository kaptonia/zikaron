//! Library tests of the storage crate.
//!
//! End-to-end behavior (real processes, cut writes, concurrency, copy equivalence) is exercised through the
//! driver binary; these tests check the library's own decisions.

use std::path::{Path, PathBuf};
use zikaron_store::codes::{Code, Why};
use zikaron_store::layout::{self, EntryName, Kind};
use zikaron_store::ledger::{self, LedgerDir, Stored, ENTRY_MAX};

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

impl From<&Scratch> for PathBuf {
    fn from(s: &Scratch) -> PathBuf {
        s.0.clone()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "zikaron-store-test-{}-{}-{:?}",
        tag,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    Scratch(p)
}

fn name(hex_pair: &str) -> EntryName {
    EntryName::parse(&hex_pair.repeat(32)).unwrap()
}

fn file_of(dir: &Path, n: &EntryName) -> PathBuf {
    dir.join(layout::entry_file_name(n))
}

// Names and layout.

#[test]
fn an_entry_name_is_sixty_four_lowercase_hex_and_nothing_else() {
    assert!(EntryName::parse(&"ab".repeat(32)).is_some());
    assert!(EntryName::parse(&"AB".repeat(32)).is_none(), "大写不收");
    assert!(EntryName::parse(&"ab".repeat(31)).is_none(), "短的不收");
    assert!(EntryName::parse(&"ab".repeat(33)).is_none(), "长的不收");
    assert!(EntryName::parse(&format!("{}/x", "a".repeat(62))).is_none(), "带分隔不收");
}

#[test]
fn every_name_falls_into_exactly_one_of_three_classes() {
    assert_eq!(layout::classify(&format!("{}.entry", "a".repeat(64))), Kind::Entry);
    assert_eq!(layout::classify(".zks-tmp-0123456789abcdef"), Kind::Tmp);
    assert_eq!(layout::classify("README.txt"), Kind::Foreign);
    assert_eq!(layout::classify(".zks-tmp-0123456789abcde"), Kind::Foreign, "位数不对即外来");
    assert_eq!(layout::classify(&"a".repeat(64)), Kind::Foreign, "少后缀即外来");
}

#[test]
fn a_path_never_leaves_the_archive() {
    let root_buf = std::env::temp_dir().join("x");
    let root = root_buf.as_path();
    assert!(layout::path_of(root, "..").is_none());
    assert!(layout::path_of(root, "a/b").is_none());
    assert!(layout::path_of(root, "").is_none());
    assert!(layout::path_of(root, "ok.entry").is_some());
}

#[test]
fn only_a_regular_file_of_the_exact_tmp_shape_is_ours_to_sweep() {
    assert!(layout::is_tmp_file(".zks-tmp-0123456789abcdef", true));
    assert!(!layout::is_tmp_file(".zks-tmp-0123456789abcdef", false), "同名的目录不是我们的");
    assert!(!layout::is_tmp_file("tmp-0123456789abcdef", true));
}

// Append.

#[test]
fn an_append_lands_the_bytes_and_leaves_no_residue() {
    let d = scratch("append");
    let led = LedgerDir::open(&d).unwrap();
    let n = name("a1");
    assert_eq!(led.append(&n, b"alpha").unwrap(), Stored::Written);
    assert_eq!(std::fs::read(file_of(&d, &n)).unwrap(), b"alpha");
    let left: Vec<String> = std::fs::read_dir(&d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, vec![layout::entry_file_name(&n)], "临时档不留");
}

#[test]
fn the_same_bytes_twice_is_one_entry_and_no_second_write() {
    let d = scratch("idem");
    let led = LedgerDir::open(&d).unwrap();
    let n = name("a1");
    assert_eq!(led.append(&n, b"alpha").unwrap(), Stored::Written);
    assert_eq!(led.append(&n, b"alpha").unwrap(), Stored::AlreadyThere);
    assert_eq!(led.pile().unwrap().items.len(), 1);
}

#[test]
fn different_bytes_under_a_taken_name_are_refused_and_change_nothing() {
    let d = scratch("conflict");
    let led = LedgerDir::open(&d).unwrap();
    let n = name("a1");
    led.append(&n, b"alpha").unwrap();
    let before = std::fs::read(file_of(&d, &n)).unwrap();
    let t = led.append(&n, b"beta").unwrap_err();
    assert_eq!(t.code, Code::Conflict);
    assert_eq!(std::fs::read(file_of(&d, &n)).unwrap(), before, "一个字节也没动");
    let left = std::fs::read_dir(&d).unwrap().count();
    assert_eq!(left, 1, "被拒的那一次不留临时档");
}

#[test]
fn a_write_over_the_cap_is_refused_before_anything_is_written() {
    let d = scratch("cap");
    let led = LedgerDir::open(&d).unwrap();
    let n = name("a1");
    let t = led.append(&n, &vec![0u8; ENTRY_MAX + 1]).unwrap_err();
    assert_eq!(t.code, Code::TooLarge);
    assert_eq!(t.size, Some(ENTRY_MAX + 1));
    assert_eq!(t.cap, Some(ENTRY_MAX));
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0, "拒了就什么也没写");
    assert!(ledger::within_cap(ENTRY_MAX, ENTRY_MAX), "上限本身在限内");
    assert!(!ledger::within_cap(ENTRY_MAX + 1, ENTRY_MAX));
}

#[test]
fn a_write_cut_at_any_of_its_three_steps_leaves_a_legal_archive() {
    let d = scratch("segments");
    let led = LedgerDir::open(&d).unwrap();
    let n = name("a1");
    // Cut during the write: the temporary file exists, the strict read is unchanged.
    let staged = led.stage(&n, b"alpha").unwrap();
    let tmp = staged.tmp_path().to_path_buf();
    assert!(tmp.is_file());
    assert_eq!(led.pile().unwrap().items.len(), 0, "没落定的写不入堆");
    // Cut after the write: the entry exists, the temporary file too.
    let linked = staged.link().unwrap();
    assert_eq!(led.pile().unwrap().items, vec![b"alpha".to_vec()]);
    assert!(tmp.is_file(), "封口之前临时档还在");
    linked.seal().unwrap();
    assert!(!tmp.exists(), "封口之后残料清了");
}

#[test]
fn an_abandoned_write_leaves_the_archive_as_it_was() {
    let d = scratch("abandon");
    let led = LedgerDir::open(&d).unwrap();
    led.stage(&name("a1"), b"alpha").unwrap().abandon();
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0);
}

// Two reads.

#[test]
fn the_strict_read_is_the_pile_and_refuses_rather_than_shorten() {
    let d = scratch("strict");
    let led = LedgerDir::open(&d).unwrap();
    led.append(&name("a1"), b"alpha").unwrap();
    led.append(&name("b2"), b"beta").unwrap();
    assert_eq!(led.pile().unwrap().items, vec![b"alpha".to_vec(), b"beta".to_vec()]);
    std::fs::write(d.join("README.txt"), b"hello").unwrap();
    let t = led.pile().unwrap_err();
    assert_eq!(t.code, Code::Unaccounted);
    assert_eq!(t.names, vec!["README.txt".to_string()]);
}

// Unix only: the link is made the unix way (on Windows a link is another kind of file).
#[cfg(unix)]
#[test]
fn a_symlink_wearing_an_entry_name_is_not_an_entry() {
    let d = scratch("symlink");
    let led = LedgerDir::open(&d).unwrap();
    led.append(&name("a1"), b"alpha").unwrap();
    let stray = d.join(layout::entry_file_name(&name("b2")));
    std::os::unix::fs::symlink(d.join(layout::entry_file_name(&name("a1"))), &stray).unwrap();
    let t = led.pile().unwrap_err();
    assert_eq!(t.code, Code::Unaccounted, "跟着符号链走,拷一份就不等价了");
    let s = led.survey().unwrap();
    assert_eq!(s.items, vec![b"alpha".to_vec()]);
    assert!(s.skipped.iter().any(|k| k.why == Why::NotAFile));
}

#[test]
fn the_lenient_read_takes_any_directory_and_discloses_every_skip() {
    let d = scratch("lenient");
    std::fs::create_dir_all(d.join("nested")).unwrap();
    std::fs::write(d.join("README.txt"), b"hello").unwrap();
    std::fs::write(d.join(".zks-tmp-0123456789abcdef"), b"half").unwrap();
    let led = LedgerDir::open(&d).unwrap();
    led.append(&name("a1"), b"alpha").unwrap();
    let s = led.survey().unwrap();
    assert_eq!(s.items, vec![b"alpha".to_vec()]);
    let seen: Vec<(String, Why)> = s.skipped.iter().map(|k| (k.name.clone(), k.why)).collect();
    assert_eq!(
        seen,
        vec![
            (".zks-tmp-0123456789abcdef".to_string(), Why::InFlight),
            ("README.txt".to_string(), Why::ForeignName),
            ("nested".to_string(), Why::NotAFile),
        ]
    );
}

#[test]
fn an_oversize_file_is_refused_by_the_strict_read_and_disclosed_by_the_lenient_one() {
    let d = scratch("oversize");
    let n = name("a1");
    std::fs::write(d.join(layout::entry_file_name(&n)), vec![0u8; ENTRY_MAX + 1]).unwrap();
    let led = LedgerDir::open(&d).unwrap();
    let t = led.pile().unwrap_err();
    assert_eq!(t.code, Code::TooLarge);
    assert_eq!(t.size, Some(ENTRY_MAX + 1));
    assert_eq!(t.names, vec![layout::entry_file_name(&n)]);
    let s = led.survey().unwrap();
    assert!(s.items.is_empty());
    assert_eq!(s.skipped.iter().map(|k| k.why).collect::<Vec<_>>(), vec![Why::TooLarge]);
}

#[test]
fn a_named_file_is_read_as_it_lies_whatever_its_name() {
    let d = scratch("named");
    std::fs::write(d.join("README.txt"), b"hello").unwrap();
    let led = LedgerDir::open(&d).unwrap();
    assert_eq!(led.read_named("README.txt").unwrap(), b"hello");
    assert_eq!(led.read_named("absent").unwrap_err().code, Code::Absent);
    assert_eq!(led.read_named("../x").unwrap_err().code, Code::BadName);
}

// Unix only: a name that is not UTF-8 is made from raw bytes, the unix way.
#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_is_named_not_dropped() {
    use std::os::unix::ffi::OsStrExt;
    let d = scratch("nonutf8");
    let raw = std::ffi::OsStr::from_bytes(b"\xff\xfe");
    if std::fs::write(d.join(raw), b"x").is_err() {
        return;  // This file system cannot create such a name (APFS cannot).
    }
    let led = LedgerDir::open(&d).unwrap();
    assert_eq!(led.pile().unwrap_err().code, Code::Unaccounted);
    assert!(led.survey().unwrap().skipped.iter().any(|k| k.why == Why::NonUtf8Name));
}

// Archive and sweep.

#[test]
fn any_directory_is_a_legal_archive() {
    let d = scratch("any");
    std::fs::write(d.join("README.txt"), b"hello").unwrap();
    let led = LedgerDir::open(&d).unwrap();
    assert_eq!(led.layout().unwrap().foreign, 1);
    // Opening changed nothing: the directory still holds one file.
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
    assert_eq!(LedgerDir::open(d.join("README.txt")).unwrap_err().code, Code::NotADirectory);
    assert_eq!(LedgerDir::open(d.join("absent")).unwrap_err().code, Code::Absent);
}

#[test]
fn the_sweep_takes_only_our_own_exact_shape() {
    let d = scratch("sweep");
    std::fs::write(d.join(".zks-tmp-0123456789abcdef"), b"half").unwrap();
    std::fs::write(d.join(".zks-tmp-short"), b"not ours").unwrap();
    std::fs::write(d.join("README.txt"), b"hello").unwrap();
    std::fs::create_dir_all(d.join(".zks-tmp-fedcba9876543210")).unwrap();
    let led = LedgerDir::open(&d).unwrap();
    led.append(&name("a1"), b"alpha").unwrap();
    let s = led.sweep().unwrap();
    assert_eq!(s.swept, 1);
    assert_eq!(s.kept, vec![".zks-tmp-fedcba9876543210".to_string()], "同名的目录不砸");
    assert!(d.join(".zks-tmp-short").is_file(), "形不对的不动");
    assert!(d.join("README.txt").is_file(), "外来的不动");
    assert!(d.join(".zks-tmp-fedcba9876543210").is_dir());
    assert_eq!(led.read_named(&layout::entry_file_name(&name("a1"))).unwrap(), b"alpha");
}

#[test]
fn a_copy_of_an_archive_reads_the_same_bytes() {
    let d = scratch("copy-src");
    let led = LedgerDir::open(&d).unwrap();
    led.append(&name("a1"), b"alpha").unwrap();
    led.append(&name("b2"), b"beta").unwrap();
    let dst = scratch("copy-dst");
    for e in std::fs::read_dir(&d).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
    assert_eq!(LedgerDir::open(&dst).unwrap().pile().unwrap(), led.pile().unwrap());
}

#[test]
fn concurrent_appends_of_the_same_name_settle_on_one_answer() {
    let d = scratch("race");
    let n = name("a1");
    let mut hands = Vec::new();
    for i in 0..8 {
        let d = d.to_path_buf();
        let n = n.clone();
        hands.push(std::thread::spawn(move || {
            let led = LedgerDir::open(&d).unwrap();
            let bytes: Vec<u8> = if i % 2 == 0 { b"alpha".to_vec() } else { b"beta".to_vec() };
            led.append(&n, &bytes).map(|_| bytes)
        }));
    }
    let mut winners = 0;
    let mut refusals = 0;
    let mut settled: Option<Vec<u8>> = None;
    for h in hands {
        match h.join().unwrap() {
            Ok(b) => {
                winners += 1;
                settled = Some(b);
            }
            Err(t) => {
                assert_eq!(t.code, Code::Conflict);
                refusals += 1;
            }
        }
    }
    assert!(winners >= 1 && refusals >= 1, "两色都要见到:{winners} 收 {refusals} 拒");
    let led = LedgerDir::open(&d).unwrap();
    let pile = led.pile().unwrap();
    assert_eq!(pile.items.len(), 1, "只落一条,且没有残料");
    assert_eq!(pile.items[0], settled.unwrap());
}

// Unix only: the directory is made unlistable with unix permission bits.
#[cfg(unix)]
#[test]
fn a_directory_that_cannot_be_listed_is_named_by_every_reader() {
    use std::os::unix::fs::PermissionsExt;
    let d = scratch("unreadable");
    LedgerDir::open(&d).unwrap().append(&name("a1"), b"alpha").unwrap();
    std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o000)).unwrap();
    let led = LedgerDir::open(&d).unwrap();
    let (pile, survey, sweep, layout) = (led.pile(), led.survey(), led.sweep(), led.layout());
    std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
    if pile.is_ok() {
        return;  // A root-like identity can read it; the assertion does not hold there.
    }
    for code in [
        pile.err().map(|t| t.code),
        survey.err().map(|t| t.code),
        sweep.err().map(|t| t.code),
        layout.err().map(|t| t.code),
    ] {
        assert_eq!(code, Some(Code::Unreadable), "读不动的目录不许被读成空目录");
    }
}

#[test]
fn the_binary_holds_no_verb_that_deletes() {
    // The driver binary keeps no deleting verb; the test scans its source.
    let shell = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bin/zks.rs")).unwrap();
    for banned in ["remove_dir_all", "remove_file", "remove_dir"] {
        assert!(!shell.contains(banned), "壳里出现了删除路径:{banned}");
    }
    // The library sweep removes only this crate's exact temporary shape.
    let lib = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ledger.rs")).unwrap();
    assert!(lib.contains("remove_file"), "库侧的扫场还在");
}
