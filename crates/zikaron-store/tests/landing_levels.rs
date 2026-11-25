use std::sync::Mutex;
use zikaron_os::{pretend_unsupported, way};
use zikaron_store::{Code, EntryName, LedgerDir, Stored, Why};

static TURN: Mutex<()> = Mutex::new(());

fn scratch(tag: &str) -> LedgerDir {
    let p = std::env::temp_dir().join(format!("zk-store-levels-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    LedgerDir::open_or_create(&p).expect("a ledger")
}

fn name(c: char) -> EntryName {
    EntryName::parse(&c.to_string().repeat(64)).expect("a name")
}

const WAYS: [(u8, &str); 3] = [(0, "hard link"), (way::LINK, "rename that never replaces"), (way::LINK | way::RENAME_NEW, "claim and rename")];

#[test]
fn each_way_lands_and_never_replaces() {
    let _t = TURN.lock().unwrap_or_else(|e| e.into_inner());
    for (ways, what) in WAYS {
        pretend_unsupported(ways);
        let l = scratch(&format!("w{ways}"));
        assert_eq!(l.append(&name('a'), b"one").expect(what), Stored::Written, "{what}");
        assert_eq!(l.append(&name('a'), b"one").expect(what), Stored::AlreadyThere, "{what}: same bytes, idempotent");
        assert_eq!(l.append(&name('a'), b"two").map_err(|t| t.code).err(), Some(Code::Conflict), "{what}: other bytes, refused");
        assert_eq!(l.pile().expect("pile").items, vec![b"one".to_vec()], "{what}: untouched");
        assert_eq!(l.layout().expect("layout").tmp, 0, "{what}: no temporary file left");
        let _ = std::fs::remove_dir_all(l.root());
    }
    pretend_unsupported(way::LINK | way::RENAME_NEW | way::CLAIM);
    let l = scratch("none");
    assert_eq!(l.append(&name('a'), b"one").map_err(|t| t.code).err(), Some(Code::Io));
    let layout = l.layout().expect("layout");
    assert_eq!((layout.entries, layout.tmp), (0, 0));
    pretend_unsupported(0);
    let _ = std::fs::remove_dir_all(l.root());
}

#[test]
fn an_empty_file_under_an_entrys_name_is_a_write_in_flight_and_is_taken_over() {
    let _t = TURN.lock().unwrap_or_else(|e| e.into_inner());
    for (ways, what) in WAYS {
        pretend_unsupported(ways);
        let l = scratch(&format!("cut{ways}"));
        l.append(&name('b'), b"kept").expect("another entry");
        let at = l.root().join(format!("{}.entry", "a".repeat(64)));
        std::fs::write(&at, b"").expect("the claim");
        assert_eq!(l.pile().expect("the strict read still reads").items, vec![b"kept".to_vec()], "{what}");
        let survey = l.survey().expect("survey");
        assert_eq!(survey.items, vec![b"kept".to_vec()]);
        assert!(survey.skipped.iter().any(|s| s.why == Why::InFlight), "{what}: said as in flight");
        let layout = l.layout().expect("layout");
        assert_eq!((layout.entries, layout.tmp), (1, 1), "{what}");
        assert_eq!(l.append(&name('a'), b"whole").expect(what), Stored::Written, "{what}");
        assert_eq!(l.pile().expect("pile").items, vec![b"whole".to_vec(), b"kept".to_vec()], "{what}");
        assert_eq!(l.layout().expect("layout").tmp, 0);
        let other = l.root().join("late");
        std::fs::write(&other, b"whole").expect("the other writer's file");
        zikaron_os::replace(&other, &at).expect("its rename");
        assert_eq!(l.pile().expect("pile").items, vec![b"whole".to_vec(), b"kept".to_vec()]);
        let _ = std::fs::remove_dir_all(l.root());
    }
    pretend_unsupported(0);
}
