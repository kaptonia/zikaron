//! The alerts page lists only what needs attention, and the rail counts it: every row fine gives one sentence and
//! a count of none; the whole-machine backup never backed up or behind is not raised at all; one row that warns
//! is the only line and counts one. Read over the window's own alerts page with given watch rows.

use app::action::{apply, Applied};
use app::lang::{t, Key};
use app::nav::Page;
use app::shell::Shell;
use app::watchx::{Gap, Item, Light, Row, Say};
use zikaron_ui::egui;

fn a_home(tag: &str) -> Shell {
    super::vault_open();
    let ctx = egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let root = std::env::temp_dir().join(format!("zk-alerts-{tag}-{}", std::process::id())).join("home");
    let _ = std::fs::remove_dir_all(&root);
    super::seated(&mut shell, &root);
    assert!(matches!(apply(&mut shell, app::action::Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }));
    shell
}

fn row(item: Item, light: Light, gap: Option<Gap>) -> Row {
    Row { item, light, n: 1, detail: String::new(), gap, rings: Vec::new() }
}

/// Every row of the recorder's watch, fine.
fn all_fine() -> Vec<Row> {
    Item::AUTHOR.iter().map(|i| row(*i, Light::Ok, None)).collect()
}

fn read(tag: &str, rows: Vec<Row>) -> app::window::AlertsReading {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    app::window::probe_alerts(&ctx, a_home(tag), rows, 1180.0, 760.0).1
}

fn titles() -> Vec<&'static str> {
    [Key::ItemUnanchored, Key::ItemQueueBacklog, Key::ItemWindowExpiring, Key::WizBackupTitle, Key::ItemAuditLabel].map(t).to_vec()
}

#[test]
fn every_row_fine_says_one_sentence_and_counts_none() {
    if super::alone_in(module_path!(), "every_row_fine_says_one_sentence_and_counts_none") {
        return;
    }
    let r = read("fine", all_fine());
    assert_eq!(r.count, 0);
    assert_eq!(r.texts.iter().filter(|s| *s == t(Key::AlertsNone)).count(), 1, "{:?}", r.texts);
    assert!(!r.texts.iter().any(|s| titles().contains(&s.as_str())), "no item listed: {:?}", r.texts);
}

#[test]
fn the_backup_is_never_raised_never_backed_up_or_behind() {
    if super::alone_in(module_path!(), "the_backup_is_never_raised_never_backed_up_or_behind") {
        return;
    }
    for (say, light) in [(Say::BackupNever, Light::Warn), (Say::BackupBehind, Light::Warn), (Say::BackupFailed, Light::Bad)] {
        let mut rows = all_fine();
        for r in rows.iter_mut().filter(|r| r.item == Item::Backup) {
            *r = row(Item::Backup, light, Some(Gap { say, n: 3, secs: None, go: Page::Archive }));
        }
        let r = read("backup", rows);
        assert_eq!(r.count, 0, "{say:?}");
        assert!(r.texts.iter().any(|s| s == t(Key::AlertsNone)), "{say:?}: {:?}", r.texts);
        assert!(!r.texts.iter().any(|s| s == t(Key::WizBackupTitle)), "{say:?}: the backup is not on the page");
    }
}

#[test]
fn one_row_that_warns_is_the_only_line_and_counts_one() {
    if super::alone_in(module_path!(), "one_row_that_warns_is_the_only_line_and_counts_one") {
        return;
    }
    let mut rows = all_fine();
    for r in rows.iter_mut().filter(|r| r.item == Item::QueueBacklog) {
        *r = row(Item::QueueBacklog, Light::Warn, Some(Gap { say: Say::Queue, n: 2, secs: None, go: Page::Queue }));
    }
    let r = read("warn", rows);
    assert_eq!(r.count, 1);
    let shown: Vec<&str> = titles().into_iter().filter(|w| r.texts.iter().any(|s| s == w)).collect();
    assert_eq!(shown, vec![t(Key::ItemQueueBacklog)], "{:?}", r.texts);
    assert!(!r.texts.iter().any(|s| s == t(Key::AlertsNone)));
}

/// What rings is what the alerts page lists: the backup behind with a ring of its own rings nothing; a warning
/// row's ring rings once.
#[test]
fn only_what_needs_attention_rings() {
    use app::watchx::{to_ring, Episode, Ring};
    let ring = |subject: &str| vec![Ring { subject: subject.to_string(), episode: Episode::Until(1) }];
    let mut backup = row(Item::Backup, Light::Warn, Some(Gap { say: Say::BackupNever, n: 3, secs: None, go: Page::Archive }));
    backup.rings = ring("backup");
    assert!(to_ring(vec![backup.clone()]).is_empty(), "the backup never rings");
    let mut warn = row(Item::QueueBacklog, Light::Warn, Some(Gap { say: Say::Queue, n: 2, secs: None, go: Page::Queue }));
    warn.rings = ring("queue");
    let rung = to_ring(vec![backup, warn]);
    assert_eq!(rung.len(), 1, "one ring");
    assert_eq!(rung[0].item, Item::QueueBacklog, "the row that needs attention");
}
