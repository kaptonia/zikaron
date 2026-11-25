use app::action::{apply, Action, Applied};
use std::path::{Path, PathBuf};

fn home_at(tag: &str, key: u8) -> (app::shell::Shell, PathBuf) {
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let at = std::env::temp_dir().join(format!("zk-one-rule-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    let s = app::key::Secret::take([key; 32]).expect("a key");
    app::register::change(|reg| app::identity::add_existing(reg, &s, app::roles::Role::Author, false, "")).expect("an identity");
    let (row, seat) = app::register::now_row_listed().expect("the register").expect("an identity");
    let old = row.home(seat).expect("a seated home");
    app::register::change_listed(|reg| Ok(app::identity::rehome(reg, &old, &at))).expect("the register follows");
    assert!(shell.refresh_anchor().expect("the key reads back"));
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.display().to_string() }), Applied::Homed { .. }));
    assert!(matches!(apply(&mut shell, Action::Genesis { statement: "start".into() }), Applied::Genesised { .. }));
    (shell, at)
}

fn count(at: &Path) -> usize {
    app::home::Home::bare(at.to_path_buf()).ledger().expect("ledger").pile().expect("pile").items.len()
}

#[test]
fn a_label_this_session_holds_is_no_write_gate() {
    if super::alone_in(module_path!(), "a_label_this_session_holds_is_no_write_gate") {
        return;
    }
    let (mut shell, at) = home_at("pen", 0x41);
    shell.reconciled = Some((zikaron::v2::tokens::Label::Gaps.as_str().to_string(), false, 1));
    shell.pen = app::auditx::Pen::Held;
    let before = count(&at);
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "after a gap".into() }), Applied::Annotated(_)), "written");
    assert_eq!(count(&at), before + 1);
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_ledger_broken_on_disk_takes_no_entry_before_any_audit() {
    if super::alone_in(module_path!(), "a_ledger_broken_on_disk_takes_no_entry_before_any_audit") {
        return;
    }
    let (mut shell, at) = home_at("fork", 0x42);
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "one".into() }), Applied::Annotated(_)));
    let home = app::home::Home::bare(at.clone());
    let pile = home.ledger().expect("ledger").pile().expect("pile").items;
    let g = pile.iter().find(|b| zikaron_glue::book::check(b).is_ok_and(|e| e.seq == 0)).expect("genesis").clone();
    let secret = app::key::Secret::take([0x42; 32]).expect("the key");
    let other = app::entryx::seal(&secret, zikaron_glue::book::Kind::Annotation.as_str(), 1, Some(&zikaron::hexfmt::encode(&zikaron_glue::book::entry_id(&g))), zikaron::json::Value::Obj(vec![("note_md".into(), zikaron::json::Value::Str("other".into()))])).expect("sealed");
    let name = zikaron_store::EntryName::parse(other.id.trim_start_matches("0x")).expect("a name");
    home.ledger().expect("ledger").append(&name, &other.bytes).expect("landed");
    assert!(shell.audit.is_none(), "no self-audit this session");
    let before = count(&at);
    match apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "after the fork".into() }) {
        Applied::Trouble(_) => {}
        other => panic!("refused: {other:?}"),
    }
    assert_eq!(count(&at), before, "not one entry written");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn anchor_through_the_door_with_nothing_to_go_awaits_the_batch_out() {
    if super::alone_in(module_path!(), "anchor_through_the_door_with_nothing_to_go_awaits_the_batch_out") {
        return;
    }
    let (mut shell, at) = home_at("await", 0x43);
    shell.machine.cli_anchor = zikaron_seal::machine::CliAnchor::Send;
    let home = app::home::Home::bare(at.clone());
    let ids: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    assert!(!ids.is_empty(), "the genesis is queued");
    let (_, q) = app::queue::amend(&home, |q| q.mark(&ids, app::queue::Step::Submitted { tx: format!("0x{}", "ab".repeat(32)), chain: q.chain, nonce: Some(0) })).expect("in flight");
    shell.queue = q;
    let req = zikaron_glue::door::Request { verb: "anchor".into(), home: at.display().to_string(), args: Vec::new() };
    assert!(matches!(app::door::turn_of(&shell, &req), Ok(app::door::Turn::Await)), "the batch out is awaited");
    let _ = std::fs::remove_dir_all(&at);
}
