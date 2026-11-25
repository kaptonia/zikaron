use zikaron_glue::landing::{land_bytes, Trouble};
use zikaron_os::{pretend_unsupported, way};

fn scratch(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("zk-glue-levels-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("scratch");
    p
}

fn only(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).expect("list").flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    names
}

#[test]
fn a_file_lands_by_each_way_and_a_file_there_is_refused() {
    for (ways, what) in [(0, "hard link"), (way::LINK, "rename that never replaces"), (way::LINK | way::RENAME_NEW, "claim and rename")] {
        pretend_unsupported(ways);
        let d = scratch(&format!("w{ways}"));
        let out = d.join("out.json");
        land_bytes(&out, b"first").expect(what);
        assert_eq!(std::fs::read(&out).expect("landed"), b"first");
        assert!(matches!(land_bytes(&out, b"second"), Err(Trouble::Occupied(_))), "{what}");
        assert_eq!(std::fs::read(&out).expect("untouched"), b"first");
        let cut = d.join("cut.json");
        std::fs::write(&cut, b"").expect("a claim");
        assert!(matches!(land_bytes(&cut, b"mine"), Err(Trouble::Occupied(_))), "{what}");
        assert_eq!(std::fs::read(&cut).expect("left as it is"), b"");
        assert_eq!(only(&d), vec!["cut.json".to_string(), "out.json".to_string()], "{what}: no temporary file left");
        let _ = std::fs::remove_dir_all(&d);
    }
    pretend_unsupported(way::LINK | way::RENAME_NEW | way::CLAIM);
    let d = scratch("none");
    assert!(matches!(land_bytes(&d.join("out.json"), b"x"), Err(Trouble::Io(..))));
    assert!(only(&d).is_empty());
    pretend_unsupported(0);
    let _ = std::fs::remove_dir_all(&d);
}
