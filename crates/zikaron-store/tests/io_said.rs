//! A failing disk operation is `E_IO` carrying the system's own words (`Trouble::said`): which operation, on
//! which path, what the system said. Cases: the directory cannot be created (a file stands in its place);
//! the temporary file cannot be created (the directory is read-only); the directory cannot be synced after
//! the entry landed (the platform layer is made to fail it for that directory alone). Every other error code
//! carries no system words. (A read-only directory refuses new files on unix systems only.)

#![cfg(unix)]

use zikaron_store::{Code, EntryName, LedgerDir};

fn scratch(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("zk-store-io-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("scratch");
    p
}

fn name() -> EntryName {
    EntryName::parse(&"b".repeat(64)).expect("a name")
}

fn read_only(p: &std::path::Path, on: bool) {
    let mut perms = std::fs::metadata(p).expect("there").permissions();
    perms.set_readonly(on);
    std::fs::set_permissions(p, perms).expect("permissions");
}

#[test]
fn a_disk_failure_carries_the_systems_words() {
    let base = scratch("said");
    // The directory cannot be created: a file stands where its parent would be.
    std::fs::write(base.join("a-file"), b"x").expect("a file");
    let t = LedgerDir::open_or_create(base.join("a-file").join("ledger")).err().expect("refused");
    assert_eq!(t.code, Code::Io);
    let said = t.said.clone().unwrap_or_default();
    assert!(said.starts_with("create directory ") && said.contains("a-file"), "{said}");

    // The temporary file cannot be created: the directory is read and searched, not written.
    let ro = LedgerDir::open_or_create(base.join("read-only")).expect("a ledger");
    read_only(ro.root(), true);
    let t = ro.append(&name(), b"one").err();
    read_only(ro.root(), false);
    let t = t.expect("refused");
    assert_eq!(t.code, Code::Io);
    let said = t.said.clone().unwrap_or_default();
    assert!(said.starts_with("create ") && said.contains(&ro.root().display().to_string()) && said.len() > "create ".len() + ro.root().display().to_string().len() + 2, "{said}");

    // The entry lands and the directory cannot be synced.
    let ns = LedgerDir::open_or_create(base.join("no-sync")).expect("a ledger");
    zikaron_os::pretend_sync_fails_at(Some(ns.root().to_path_buf()));
    let t = ns.append(&name(), b"one").err();
    zikaron_os::pretend_sync_fails_at(None);
    let t = t.expect("refused");
    assert_eq!(t.code, Code::Io);
    let said = t.said.clone().unwrap_or_default();
    assert!(said.starts_with("directory sync ") && said.contains(&ns.root().display().to_string()), "{said}");

    // Any other code carries no words of the system.
    let fine = LedgerDir::open_or_create(base.join("fine")).expect("a ledger");
    fine.append(&name(), b"one").expect("lands");
    let other = fine.append(&name(), b"two").err().expect("the same name with other bytes is refused");
    assert_ne!(other.code, Code::Io);
    assert_eq!(other.said, None);
    let _ = std::fs::remove_dir_all(&base);
}
