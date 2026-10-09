//! A mirror manifest's row names must be entry names, or the bundle is refused before anything is read.

use zikaron::json::Value;
use zikaron_glue::mirror::{entries, sheet, ReadTrouble, Row, ENTRIES, KIND, MANIFEST, VERSION};

/// A row name that is not an entry name is refused by name before any file is read; a path leaving the
/// entries folder (`..`, absolute, a separator) is never read.
#[test]
fn a_row_that_is_not_an_entry_name_is_refused_and_nothing_outside_is_read() {
    let dir = std::env::temp_dir().join(format!("zikaron-glue-mirror-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(ENTRIES)).expect("room");
    std::fs::write(dir.join("outside"), b"not yours").expect("outside");
    let good = "ab".repeat(32);
    std::fs::write(dir.join(ENTRIES).join(&good), b"entry").expect("entry");
    let lay = |name: &str| {
        let m = format!("{{\"entries\":[{{\"bytes\":5,\"name\":{},\"sha256\":\"\"}}],\"kind\":\"{KIND}\",\"version\":{VERSION}}}", zikaron::json::canon_bytes(&Value::Str(name.into())).iter().map(|b| *b as char).collect::<String>());
        std::fs::write(dir.join(MANIFEST), m).expect("manifest");
    };
    lay(&good);
    assert_eq!(entries(&dir).ok(), Some(vec![b"entry".to_vec()]));
    let abs = dir.join("outside").display().to_string();
    for bad in ["../outside", abs.as_str(), "a/b", "", &"AB".repeat(32), &"ab".repeat(31), &format!("{good}.entry")] {
        lay(bad);
        match entries(&dir) {
            Err(ReadTrouble::NotAnEntryName(n)) => assert_eq!(n, bad),
            other => panic!("{bad:?}: {:?}", other.map(|v| v.len())),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}


/// A manifest's absent members read as empty (row names are checked by `entries`, not here).
#[test]
fn a_manifests_absent_members_read_as_empty() {
    let s = sheet(b"{\"entries\":[{\"name\":\"x\"},{}]}").expect("JSON");
    assert_eq!((s.kind.as_str(), s.version, s.root.as_str(), s.owner.as_str(), s.held.len()), ("", 0, "", "", 0));
    assert_eq!(s.rows, vec![Row { name: "x".into(), sha256: String::new(), bytes: 0 }, Row { name: String::new(), sha256: String::new(), bytes: 0 }]);
    assert!(sheet(b"not json").is_err());
}
