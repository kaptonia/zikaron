use app::action::{apply, apply_settled, Action, Applied};

fn home_with_queued(name: &str) -> (app::shell::Shell, std::path::PathBuf) {
    super::vault_open();
    let dir = super::a8_send::homes().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    super::seated(&mut shell, &dir.join("home"));
    answers!(apply(&mut shell, Action::OpenHome { root: dir.join("home").display().to_string() }), Applied::Homed { .. });
    let _ = apply_settled(&mut shell, Action::Genesis { statement: "r34b4".into() });
    shell.settings.chain_id = Some(31337);
    (shell, dir)
}

#[test]
fn the_queue_is_placed_off_the_ledger_once_a_generation() {
    if super::alone_in(module_path!(), "the_queue_is_placed_off_the_ledger_once_a_generation") {
        return;
    }
    let (mut shell, _dir) = home_with_queued("r34b4-517");
    shell.rows = None;
    let placed = |shell: &app::shell::Shell| shell.places_read.held().map(|p| p.len());
    let first = shell.next_lane_here(1);
    assert!(first.is_some(), "the queued genesis is placed off the ledger");
    assert_eq!(placed(&shell), Some(1), "read once: the genesis placed");
    let ledger = shell.home.as_ref().map(|h| h.dir(app::home::Slot::Ledger)).expect("a ledger folder");
    let aside = ledger.with_extension("aside");
    std::fs::rename(&ledger, &aside).expect("the ledger taken away");
    for _ in 0..3 {
        assert_eq!(shell.next_lane_here(1), first, "the same generation is placed by what was read once");
        assert_eq!(placed(&shell), Some(1), "not read again: the ledger is gone and the genesis still placed");
    }
    shell.book_changed();
    shell.count_here();
    let _ = shell.next_lane_here(1);
    assert_eq!(placed(&shell), Some(0), "a new generation reads the ledger anew (it is gone: nothing placed)");
    std::fs::rename(&aside, &ledger).expect("the ledger back");
    shell.book_changed();
    shell.count_here();
    assert_eq!(shell.next_lane_here(1), first);
    assert_eq!(placed(&shell), Some(1));
}

#[test]
fn a_face_entry_is_placed_with_the_table() {
    if super::alone_in(module_path!(), "a_face_entry_is_placed_with_the_table") {
        return;
    }
    let (mut shell, _dir) = home_with_queued("r34b4-517-face");
    let t = app::ledgerx::table(shell.home.as_ref().expect("a home"), None, &[]).expect("a table");
    shell.rows = Some((t.rows, 0));
    let id = shell.queue.items.first().map(|q| q.id.clone()).expect("a queued genesis");
    assert_eq!(shell.carried_on(&id), Some(None), "the genesis is placed by the table, carried on no chain");
    let row = shell.rows.as_ref().and_then(|(r, _)| r.first().cloned()).expect("a row");
    let face = app::ledgerx::Row { kind: zikaron_glue::book::Kind::Face, id: format!("0x{}", "ab".repeat(32)), ..row };
    if let Some((rows, _)) = shell.rows.as_mut() {
        rows.push(face.clone());
    }
    shell.seat_chains.insert(face.id.clone(), 11_155_111);
    assert_eq!(shell.carried_on(&face.id), Some(Some(11_155_111)));
    let place = app::queue::place_in(shell.rows.as_ref().map(|(r, _)| r.as_slice()), &shell.seat_chains)(&face.id).expect("placed");
    assert!(place.carried(11_155_111) && !place.carried(31_337), "carried on its seat chain alone");
}
