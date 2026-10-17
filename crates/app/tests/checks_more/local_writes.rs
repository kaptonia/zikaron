//! This machine's own writes: one temporary shape for every write, a write that does not land leaves no
//! temporary file, and the writer sweeps what a cut write left when it opens the home (a reader never does).

use app::action::{apply, Action, Applied};

/// A scratch directory of this test's own.
fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("zk-local-writes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch");
    d
}

/// The temporary-shaped names in a directory.
fn temps(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| app::home::is_temp_name(n)).collect()).unwrap_or_default()
}

/// Every temporary name this product writes is the store's one shape, and the store reads a leftover one as a
/// temporary file (so a strict read of a ledger room passes over it instead of refusing the whole room).
#[test]
fn every_temporary_file_is_one_shape_the_store_reads_as_temporary() {
    let dir = scratch("shape");
    let landing = zikaron_glue::landing::staging_beside(&dir.join("kit")).expect("a landing's name");
    let landing = landing.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    for n in [landing.as_str(), &zikaron_store::layout::tmp_file_name("0123456789abcdef")] {
        assert!(app::home::is_temp_name(n), "{n}");
        assert_eq!(zikaron_store::layout::classify(n), zikaron_store::layout::Kind::Tmp, "{n}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A write that does not land (the place is a folder that cannot be replaced) is refused and takes its
/// temporary file with it; a write that lands leaves none either.
#[test]
fn a_write_that_does_not_land_leaves_no_temporary_file() {
    let dir = scratch("cut");
    std::fs::create_dir_all(dir.join("taken").join("inside")).expect("a folder in the way");
    assert!(app::home::put_at(&dir, "taken", b"x").is_err(), "refused");
    assert!(temps(&dir).is_empty(), "no temporary file left: {:?}", temps(&dir));
    app::home::put_at(&dir, "landed", b"y").expect("landed");
    assert_eq!(std::fs::read(dir.join("landed")).expect("read"), b"y");
    assert!(temps(&dir).is_empty(), "none after a write that landed");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The sweep removes temporary-shaped regular files at any depth and nothing else.
#[test]
fn the_sweep_takes_temporary_files_at_any_depth_and_nothing_else() {
    let dir = scratch("sweep");
    let deep = dir.join("ledger").join("deeper");
    std::fs::create_dir_all(&deep).expect("rooms");
    let t1 = zikaron_store::layout::tmp_file_name("0000000000000001");
    let t2 = zikaron_store::layout::tmp_file_name("0000000000000002");
    std::fs::write(dir.join(&t1), b"cut").expect("temp");
    std::fs::write(deep.join(&t2), b"cut").expect("temp");
    // A folder of the temporary shape (a tree landing's staging area) is not a regular file: kept.
    let t3 = zikaron_store::layout::tmp_file_name("0000000000000003");
    std::fs::create_dir_all(dir.join(&t3)).expect("a folder");
    for keep in ["settings.json", ".DS_Store", ".zks-tmp-short"] {
        std::fs::write(dir.join(keep), b"mine").expect("kept");
    }
    assert_eq!(app::home::sweep_temps(&dir), 2);
    assert!(!dir.join(&t1).exists() && !deep.join(&t2).exists(), "both temporary files went");
    assert!(dir.join(&t3).is_dir(), "a folder is never swept");
    for keep in ["settings.json", ".DS_Store", ".zks-tmp-short"] {
        assert!(dir.join(keep).exists(), "{keep} kept");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The writer opening a home sweeps a temporary file a cut write left in a room; a reader (another holds the
/// writer lock) leaves it: it may be the writer's write in flight.
#[test]
fn the_writer_sweeps_on_opening_and_a_reader_does_not() {
    if super::alone_in(module_path!(), "the_writer_sweeps_on_opening_and_a_reader_does_not") {
        return;
    }
    super::vault_open();
    let dir = scratch("open");
    let root = dir.join("home");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }), "laid and opened");
    let home = app::home::Home::open_or_create(root.display().to_string().as_str()).expect("the home");
    let cut = zikaron_store::layout::tmp_file_name("00000000000000aa");
    let ledger = home.dir(app::home::Slot::Ledger);
    // A reader: another handle holds the writer lock, so this opening reads only.
    drop(shell);
    std::fs::write(ledger.join(&cut), b"cut").expect("a cut write");
    let held = app::lock::take(&home).expect("another writer");
    let mut reader = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    assert!(matches!(apply(&mut reader, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }), "opened to read");
    assert!(ledger.join(&cut).exists(), "a reader sweeps nothing");
    drop(reader);
    drop(held);
    // The writer.
    let mut writer = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    assert!(matches!(apply(&mut writer, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }), "opened to write");
    assert!(!ledger.join(&cut).exists(), "the writer swept it");
    drop(writer);
    let _ = std::fs::remove_dir_all(&dir);
}
