use super::a8_send::{chain, node, PLENTY};
use super::{seated, vault_open};
use app::action::{apply, apply_settled, Action, Applied};
use app::fault::Known;
use app::task::{Done, Kind};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn base(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zk-r30b3-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("a scratch folder");
    d
}

fn boot() -> app::shell::Shell {
    let ctx = zikaron_ui::egui::Context::default();
    app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx))
}

fn author(at: &Path, note: &str) -> (app::shell::Shell, String, String, PathBuf) {
    vault_open();
    let mut shell = boot();
    seated(&mut shell, &at.join("author"));
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.join("author").display().to_string() }), Applied::Homed { .. }));
    let _ = apply_settled(&mut shell, Action::Genesis { statement: "r30b3".into() });
    let w = at.join("w.txt");
    std::fs::write(&w, b"the granted work").expect("a file");
    let record = match apply_settled(&mut shell, Action::RecordWork { note_md: note.into(), files: vec![w.display().to_string()], for_: None }) {
        Applied::RecordedBatch { ids, .. } => ids.last().cloned().unwrap_or_default(),
        other => panic!("record: {other:?}"),
    };
    let work = zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(b"the granted work"));
    let grant = earlier_grant(&mut shell, &format!("0x{}", "33".repeat(20)), &work, &format!("0x{}", "44".repeat(32)), None);
    (shell, grant, record, w)
}

fn earlier_grant(shell: &mut app::shell::Shell, grantee: &str, work: &str, terms: &str, upstream: Option<&str>) -> String {
    use zikaron::json::Value;
    let home = shell.home.as_ref().expect("the author's home");
    let (seq, prev) = app::ledgerx::head(home).expect("a head").expect("a ledger with a head");
    let me = shell.anchor.expect("the seat's key in office");
    let secret = [Some(app::places::key_slot(&me)), app::register::account_now().ok().flatten()]
        .into_iter()
        .flatten()
        .filter_map(|slot| app::key::load(Some(&slot)).ok().flatten())
        .find(|k| k.address() == Some(me))
        .expect("the seat's key");
    let root = app::auditx::root_of(&home.ledger().expect("a ledger").pile().expect("entries").items).expect("its root");
    let mut members = vec![
        ("grantee".to_string(), Value::Str(grantee.to_string())),
        ("ledger".to_string(), Value::Str(root)),
        ("terms".to_string(), Value::Str(terms.to_string())),
    ];
    if let Some(u) = upstream {
        members.push(("upstream".to_string(), Value::Str(u.trim().to_string())));
    }
    members.push(("work".to_string(), Value::Str(work.to_string())));
    let body = Value::Obj(members);
    let e = app::entryx::seal_under(zikaron_glue::book::Law::Two, &secret, zikaron_glue::book::Kind::Grant.as_str(), seq + 1, Some(&prev), body).expect("sealed");
    let name = zikaron_store::EntryName::parse(e.id.trim_start_matches("0x")).expect("an entry name");
    home.ledger().expect("a ledger").append(&name, &e.bytes).expect("landed");
    let _ = apply(shell, Action::QueueEntry { id: e.id.clone() });
    shell.stale_rows();
    shell.stale_grants();
    e.id
}

fn package(author: &app::shell::Shell, grant: &str, to: &Path) -> PathBuf {
    let home = author.home.as_ref().expect("the author's home");
    let items = home.ledger().expect("a ledger").pile().expect("its entries").items;
    let chain = app::badgex::chain_for(&app::badgex::pool(home).expect("a pool"), grant).expect("the chain");
    let (bytes, _) = app::grantfilex::build(app::grantfilex::parts(&chain, &items, &[], None).expect("parts")).expect("a package");
    std::fs::write(to, bytes).expect("written");
    to.to_path_buf()
}

fn grantee(at: &Path) -> app::shell::Shell {
    let mut shell = boot();
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.display().to_string() }), Applied::Homed { .. }));
    shell
}

fn kept_files(shell: &app::shell::Shell) -> usize {
    let dir = shell.home.as_ref().expect("a home").dir(app::home::Slot::GrantsHeld).join(app::grantfilex::KEPT);
    std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
}

fn kept_rank(shell: &app::shell::Shell, grant_bytes: &[u8], grant: &str) -> Option<(bool, usize)> {
    let e = zikaron_glue::book::check(grant_bytes).expect("a grant");
    let dir = shell.home.as_ref().expect("a home").dir(app::home::Slot::GrantsHeld).join(app::grantfilex::KEPT);
    app::supplyx::kept_copy(&dir, &e.id_hex(), &e.author, &mut Vec::new()).map(|s| app::supplyx::rank(&s.items, grant))
}

fn landed(shell: &mut app::shell::Shell, kind: Kind) -> Result<Done, app::fault::Fault> {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(o) = shell.drain().into_iter().find(|o| o.kind == kind) {
            return o.result;
        }
        assert!(Instant::now() < until, "{kind:?} never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_newer_package_replaces_the_copy_and_a_revocation_stays() {
    if super::alone_in(module_path!(), "a_newer_package_replaces_the_copy_and_a_revocation_stays") {
        return;
    }
    let at = base("a3");
    let (mut a, grant, _, _) = author(&at, "授权的那一件");
    let p1 = package(&a, &grant, &at.join("p1.zkgrant"));
    let mut g = grantee(&at.join("grantee"));
    assert!(!g.review_due(0.0, 0.0), "nothing due before");
    assert!(matches!(apply(&mut g, Action::ImportGrant { typed: p1.display().to_string() }), Applied::Held { .. }));
    assert!(g.review_due(0.0, 0.0), "what went in is re-checked at once");
    assert_eq!(kept_files(&g), 1);
    let held = app::vaultx::held(g.home.as_ref().expect("home")).expect("the vault");
    let bytes = held.iter().find(|h| h.id.eq_ignore_ascii_case(&grant)).expect("held").bytes.clone();
    assert_eq!(kept_rank(&g, &bytes, &grant), Some((false, 3)), "genesis, record, grant");
    let again = apply(&mut g, Action::ImportGrant { typed: p1.display().to_string() });
    assert!(matches!(&again, Applied::Trouble(f) if f.which() == Some(Known::AlreadyHeld)), "{again:?}");
    assert_eq!(kept_files(&g), 1, "a refused package is not kept");
    assert!(matches!(apply_settled(&mut a, Action::Revoke { grant: grant.clone(), case: String::new() }), Applied::Revoked { .. }));
    let p2 = package(&a, &grant, &at.join("p2.zkgrant"));
    let renewed = apply(&mut g, Action::ImportGrant { typed: p2.display().to_string() });
    assert!(matches!(&renewed, Applied::HeldRenewed { grant: x } if x.eq_ignore_ascii_case(&grant)), "{renewed:?}");
    assert_eq!(kept_rank(&g, &bytes, &grant), Some((true, 4)), "the copy read revokes the grant");
    let older = apply(&mut g, Action::ImportGrant { typed: p1.display().to_string() });
    assert!(matches!(&older, Applied::Trouble(f) if f.which() == Some(Known::AlreadyHeld)), "{older:?}");
    assert_eq!(kept_files(&g), 2);
    assert!(matches!(apply(&mut g, Action::ReviewVault), Applied::Started(Kind::Review)));
    assert!(!g.review_due(0.0, 0.0), "the change is being checked");
    let revoked = |d: Result<Done, app::fault::Fault>| match d {
        Ok(Done::Reviewed { cards, .. }) => cards.iter().any(|c| c.id.eq_ignore_ascii_case(&grant) && c.failed.iter().any(|x| x == zikaron_kit::tokens::Check::Revoked.as_str())),
        other => panic!("review: {other:?}"),
    };
    assert!(revoked(landed(&mut g, Kind::Review)), "revoked");
    assert!(matches!(apply(&mut g, Action::RecheckAll), Applied::FactsForgotten { .. }));
    assert_eq!(kept_rank(&g, &bytes, &grant), Some((true, 4)));
    assert!(matches!(apply(&mut g, Action::ReviewVault), Applied::Started(Kind::Review)));
    assert!(revoked(landed(&mut g, Kind::Review)), "revoked after checking everything again");

    let mut c = grantee(&at.join("by-code"));
    c.seat_identities(app::register::view().ok());
    let pool = app::badgex::pool(a.home.as_ref().expect("home")).expect("pool");
    let code = app::badgex::code_for(&pool, &grant).expect("a code");
    assert!(matches!(apply(&mut c, Action::ImportGrant { typed: code }), Applied::Held { .. }));
    assert_eq!(kept_files(&c), 0, "a code brings no package");
    assert!(matches!(apply(&mut c, Action::ReviewVault), Applied::Started(Kind::Review)));
    assert!(revoked(landed(&mut c, Kind::Review)), "revoked, read from the issuer's home on this machine");
    assert_eq!(kept_rank(&c, &bytes, &grant), Some((true, 4)), "the revocation is in the vault now");
    let shelf = app::supplyx::Shelf { kept: Some(c.home.as_ref().expect("home").dir(app::home::Slot::GrantsHeld).join(app::grantfilex::KEPT)), ..Default::default() };
    let found = app::supplyx::find(&shelf, app::supplyx::Want::Hop(&bytes), 0);
    let s = found.supply.expect("the vault gives the ledger");
    assert!(s.kept && s.level == app::supplyx::Level::Vault);
    let held = app::vaultx::held(c.home.as_ref().expect("home")).expect("vault").into_iter().find(|h| h.id.eq_ignore_ascii_case(&grant)).expect("held");
    let card = app::vaultx::review(&held, &s.items, &app::auditx::empty_fragment(), None, 0);
    assert!(card.failed.iter().any(|x| x == zikaron_kit::tokens::Check::Revoked.as_str()), "judged revoked from the vault alone: {:?}", card.failed);
    let second = earlier_grant(&mut a, &format!("0x{}", "35".repeat(20)), &zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(b"the granted work")), &format!("0x{}", "44".repeat(32)), None);
    let code2 = app::badgex::code_for(&app::badgex::pool(a.home.as_ref().expect("home")).expect("pool"), &second).expect("a code");
    assert!(matches!(apply(&mut c, Action::ReviewVault), Applied::Started(Kind::Review)));
    assert!(matches!(apply(&mut c, Action::ImportGrant { typed: code2 }), Applied::Held { .. }), "a second grant while the pass is out");
    assert!(!c.review_due(0.0, 0.0), "not while one is out");
    let _ = landed(&mut c, Kind::Review);
    assert!(c.review_due(0.0, 0.0), "the change waits for the next pass");
    assert_eq!(kept_files(&c), 1, "the revoking copy kept once");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_checked_package_newer_than_the_vault_is_read_as_itself() {
    if super::alone_in(module_path!(), "a_checked_package_newer_than_the_vault_is_read_as_itself") {
        return;
    }
    let at = base("a3check");
    let (mut a, grant, _, _) = author(&at, "第一件");
    let p1 = package(&a, &grant, &at.join("p1.zkgrant"));
    let mut g = grantee(&at.join("grantee"));
    assert!(matches!(apply(&mut g, Action::ImportGrant { typed: p1.display().to_string() }), Applied::Held { .. }));
    assert!(matches!(apply_settled(&mut a, Action::Revoke { grant: grant.clone(), case: String::new() }), Applied::Revoked { .. }));
    let p2 = package(&a, &grant, &at.join("p2.zkgrant"));
    let kept = Some(g.home.as_ref().expect("home").dir(app::home::Slot::GrantsHeld).join(app::grantfilex::KEPT));
    let read = |p: &Path| {
        let taken = app::payloadx::take_full(&p.display().to_string()).expect("taken");
        let (path, o) = taken.file.expect("a grant file");
        let shelf = app::supplyx::Shelf { kept: kept.clone(), carried: Some((path.display().to_string(), o.ledger)), ..Default::default() };
        let f = app::supplyx::find(&shelf, app::supplyx::Want::Hop(taken.hops.last().expect("a hop")), 0);
        let s = f.supply.expect("material");
        (s.level, app::supplyx::rank(&s.items, &grant))
    };
    assert_eq!(read(&p2), (app::supplyx::Level::Kit, (true, 4)), "the newer package is read as itself");
    assert_eq!(read(&p1), (app::supplyx::Level::Vault, (false, 3)), "the same as the vault's: the vault's copy");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_record_is_named_by_its_note_else_its_file() {
    if super::alone_in(module_path!(), "a_record_is_named_by_its_note_else_its_file") {
        return;
    }
    let at = base("c5");
    let (mut a, _, named, _) = author(&at, "第一章");
    let w2 = at.join("经桌面.txt");
    std::fs::write(&w2, b"recorded without a note").expect("a file");
    let bare = match apply_settled(&mut a, Action::RecordWork { note_md: String::new(), files: vec![w2.display().to_string()], for_: None }) {
        Applied::RecordedBatch { ids, .. } => ids.last().cloned().unwrap_or_default(),
        other => panic!("record: {other:?}"),
    };
    let home = a.home.as_ref().expect("home");
    let table = app::ledgerx::table_with(home, None, &app::queue::Queue::default()).expect("the table");
    let name = |id: &str| table.rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).and_then(|r| r.facts.record_name());
    assert_eq!(name(&named), Some("第一章".to_string()), "the name written with it stays first");
    assert_eq!(name(&bare), Some("经桌面.txt".to_string()), "no note: the file it was signed from");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_record_with_neither_note_nor_file_is_named_by_its_content_digest() {
    if super::alone_in(module_path!(), "a_record_with_neither_note_nor_file_is_named_by_its_content_digest") {
        return;
    }
    let at = base("y2");
    let (mut a, _, named, _) = author(&at, "第一章");
    let w = at.join("no-index.txt");
    std::fs::write(&w, b"recorded with no note and no file name").expect("a file");
    let bare = match apply_settled(&mut a, Action::RecordWork { note_md: String::new(), files: vec![w.display().to_string()], for_: None }) {
        Applied::Recorded { id, .. } => id,
        Applied::RecordedBatch { ids, .. } => ids.last().cloned().unwrap_or_default(),
        other => panic!("record: {other:?}"),
    };
    let index = app::recordsx::path_in(&app::home::machine_dir().expect("the machine directory"));
    let _ = std::fs::remove_file(&index);
    let home = a.home.as_ref().expect("home");
    let ledger = std::fs::read_dir(home.root().join(app::home::Slot::Ledger.as_str())).map(|d| d.count()).unwrap_or(0);
    let table = app::ledgerx::table_with(home, None, &app::queue::Queue::default()).expect("the table");
    let row = table.rows.iter().find(|r| r.id.eq_ignore_ascii_case(&bare)).expect("the record");
    let content = zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(b"recorded with no note and no file name"));
    assert_eq!((row.facts.note.clone(), row.facts.file.clone()), (None, None), "neither a note nor a file name");
    assert_eq!(row.work.as_deref(), Some(content.as_str()), "the fingerprint its details show");
    assert_eq!(row.facts.record_name(), Some(content[..10].to_string()), "named by the digest's first eight hex digits");
    let name = |id: &str| table.rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).and_then(|r| r.facts.record_name());
    assert_eq!(name(&named), Some("第一章".to_string()), "the name written with it stays first");
    assert_eq!(std::fs::read_dir(home.root().join(app::home::Slot::Ledger.as_str())).map(|d| d.count()).unwrap_or(0), ledger, "nothing written to the ledger for the name");
    assert!(!index.exists(), "nor to the index");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_forms_of_a_grant_package_never_share_a_name() {
    if super::alone_in(module_path!(), "the_forms_of_a_grant_package_never_share_a_name") {
        return;
    }
    let at = base("c11");
    let (a, grant, _, w) = author(&at, "第一件");
    let home = a.home.as_ref().expect("home");
    let n = node(|m, b| chain(m, b, PLENTY));
    let ask = app::exitgate::Ask { root: home.root().to_path_buf(), lanes: vec![app::exitgate::Lane { chain: 31337, registry: [0x11; 20], from_block: 0, eps: vec![app::chainx::Endpoint::at(31337, n.url.clone())] }], own: None };
    let pass = app::exitgate::pass(&ask).expect("the gate passes on an empty chain");
    let key = app::key::generate().expect("a key").public().expect("its public key");
    let out = at.join("out");
    std::fs::create_dir_all(&out).expect("a folder");
    let name = |x: &app::grantfilex::Exported| x.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let stem = app::grantfilex::stem(&grant);
    let ext = zikaron_glue::container::EXT;
    let plain = app::grantfilex::export(&pass, home, &grant, &out, app::grantfilex::Pick::No, None).expect("plain");
    let first = std::fs::read(&plain.path).expect("written");
    let original = app::grantfilex::export(&pass, home, &grant, &out, app::grantfilex::Pick::At(&w), None).expect("with the original");
    let sealed = app::grantfilex::export(&pass, home, &grant, &out, app::grantfilex::Pick::No, Some(&key)).expect("sealed");
    let both = app::grantfilex::export(&pass, home, &grant, &out, app::grantfilex::Pick::At(&w), Some(&key)).expect("both");
    let again = app::grantfilex::export(&pass, home, &grant, &out, app::grantfilex::Pick::No, None).expect("plain again");
    assert_eq!(
        [name(&plain), name(&original), name(&sealed), name(&both), name(&again)],
        [format!("{stem}.{ext}"), format!("{stem}-original.{ext}"), format!("{stem}-sealed.{ext}"), format!("{stem}-original-sealed.{ext}"), format!("{stem}-2.{ext}")]
    );
    assert_eq!(again.chosen.why, app::home::Why::NameTaken, "said as numbered");
    assert_eq!(std::fs::read(&plain.path).expect("still there"), first, "the first is not replaced");
    assert_eq!(std::fs::read_dir(&out).expect("the folder").count(), 5, "five files");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_rule_refusal_is_said_in_that_rules_words() {
    if super::alone_in(module_path!(), "a_rule_refusal_is_said_in_that_rules_words") {
        return;
    }
    let at = base("r268");
    let (mut a, _, record, _) = author(&at, "第一件");
    let absent = format!("0x{}", "ab".repeat(32));
    let said = |x: Applied| match x {
        Applied::Trouble(f) => (f.which(), f.human().to_string(), f.next(), f.rule().and_then(|r| r.token)),
        other => panic!("not refused: {other:?}"),
    };
    let t = |k| app::lang::t(k).to_string();
    use app::lang::Key as L;
    assert_eq!(said(apply(&mut a, Action::Revoke { grant: absent.clone(), case: String::new() })), (Some(Known::SubjectMissing), t(L::RevokeNotInLedger), t(L::RevokeNext), Some("E_REVOKE_NOT_IN_LEDGER")));
    assert_eq!(said(apply(&mut a, Action::Revoke { grant: record.clone(), case: String::new() })), (Some(Known::SubjectMissing), t(L::RevokeNotAGrant), t(L::RevokeNext), Some("E_REVOKE_NOT_A_GRANT")));
    assert_eq!(said(apply(&mut a, Action::Retract { subject: absent.clone(), note_md: String::new() })), (Some(Known::SubjectMissing), t(L::RetractNotInLedger), t(L::RetractNext), Some("E_RETRACT_NOT_IN_LEDGER")));
    assert_eq!(said(apply(&mut a, Action::Annotate { subject: absent.clone(), note_md: "注".into() })), (Some(Known::SubjectMissing), t(L::FaultWhatSubjectMissing), t(L::FaultNextSubjectMissing), Some("E_NAME_NOT_IN_LEDGER")));
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_grant_file_with_its_original_offers_it_out_on_the_check_page() {
    if super::alone_in(module_path!(), "a_grant_file_with_its_original_offers_it_out_on_the_check_page") {
        return;
    }
    let at = base("c14");
    let (a, grant, _, _) = author(&at, "第一件");
    let home = a.home.as_ref().expect("home");
    let items = home.ledger().expect("a ledger").pile().expect("entries").items;
    let chain = app::badgex::chain_for(&app::badgex::pool(home).expect("pool"), &grant).expect("chain");
    let write = |name: &str, original: bool| {
        let mut parts = app::grantfilex::parts(&chain, &items, &[], None).expect("parts");
        if original {
            parts.original = Some(zikaron_glue::grantfile::Original::File("w.txt".into(), b"the granted work".to_vec()));
        }
        let (bytes, _) = app::grantfilex::build(parts).expect("a package");
        let p = at.join(name);
        std::fs::write(&p, bytes).expect("written");
        p
    };
    let run = |p: &Path| {
        let hops = app::checkx::hops_of(&p.display().to_string()).expect("taken");
        app::checkx::run(hops, app::supplyx::Shelf::default(), Vec::new(), Err(app::fault::Fault::known(Known::NoRegistry, String::new())), Some(150), None)
    };
    let with = run(&write("with.zkgrant", true));
    let (path, plan) = with.takes.clone().expect("the original is offered");
    assert!(path.ends_with("with.zkgrant"));
    let plan = plan.expect("the package verifies").expect("a plan");
    assert_eq!(plan.take.iter().map(|o| o.landing_name()).collect::<Vec<_>>(), vec!["w.txt".to_string()], "the original, by its name");
    assert!(run(&write("without.zkgrant", false)).takes.is_none(), "no original, nothing offered");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn each_chain_a_record_is_on_is_shown_and_the_check_names_its_chains() {
    let id = format!("0x{}", "aa".repeat(32));
    let other = format!("0x{}", "bb".repeat(32));
    let report = zikaron::json::parse(
        format!(
            r#"{{"anchored":[{{"entry_id":"{id}","anchors":[{{"chainId":31338,"blockNumber":5,"blockTimestamp":900,"tx":"0x02"}},{{"chainId":11155111,"blockNumber":9,"blockTimestamp":100,"tx":"0x01"}},{{"chainId":11155111,"blockNumber":12,"blockTimestamp":200,"tx":"0x03"}}]}},{{"entry_id":"{other}","anchors":[{{"chainId":11155111,"blockNumber":9,"blockTimestamp":100,"tx":"0x01"}}]}}],"basis":{{"chains":[{{"chainId":11155111}},{{"chainId":31338}}]}}}}"#
        )
        .as_bytes(),
    )
    .expect("a report");
    assert_eq!(app::ledgerx::anchors_of(&report, &id), vec![(11155111, "0x01".to_string()), (31338, "0x02".to_string())], "each chain once, earliest first");
    assert_eq!(app::ledgerx::anchors_of(&report, &format!("0x{}", "cc".repeat(32))), Vec::new());
    assert_eq!(app::ledgerx::chains_checked(&report), vec![(11155111, 2), (31338, 1)], "the chains read, with what is on each");
}

fn probe_shell(ctx: &zikaron_ui::egui::Context) -> app::shell::Shell {
    vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(ctx))
}

#[test]
fn a_refused_verification_stays_in_the_conclusion() {
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.settings.role = app::roles::Role::Grantee;
    let f = app::fault::Fault::known(Known::AddressedNotRecipient, "/x/kit.zkit".to_string());
    let said = f.human().to_string();
    shell.failed.insert(Kind::Verify, f);
    let place = app::nav::Place::View(app::nav::View::Verify, app::nav::tab::VERIFY_WORK);
    let (mut shell, r) = app::window::probe_face(&ctx, shell, place, app::window::MIN_W, app::window::MIN_H);
    assert!(r.texts.iter().any(|t| t == &said), "the refusal is in the conclusion: {:?}", r.texts);
    shell.failed.remove(&Kind::Verify);
    let (_, r) = app::window::probe_face(&ctx, shell, place, app::window::MIN_W, app::window::MIN_H);
    assert!(!r.texts.iter().any(|t| t == &said), "a later pass clears it: {:?}", r.texts);
}

#[test]
fn the_export_page_shows_no_result_it_did_not_start() {
    if super::alone_in(module_path!(), "the_export_page_shows_no_result_it_did_not_start") {
        return;
    }
    let at = base("c12");
    let (mut shell, _, _, _) = author(&at, "第一件");
    let ctx = zikaron_ui::egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let _ = apply_settled(&mut shell, Action::ListHeld);
    let banner_head = app::lang::t(app::lang::Key::KitMadeBanner).split("{0}").next().unwrap_or("").to_string();
    shell.kit = Some(Done::Kit {
        root: PathBuf::from("/x/home"),
        publish: None,
        path: "/x/kits/kit.zkit".into(),
        kit_id: format!("0x{}", "11".repeat(32)),
        entries: 4,
        files: 3,
        pulled: Vec::new(),
        dropped: Vec::new(),
        originals: Vec::new(),
        left_out: Vec::new(),
        unreadable: Vec::new(),
        withdrawn: Vec::new(),
        facts: Default::default(),
    });
    let place = app::nav::Place::View(app::nav::View::Works, app::nav::tab::WORKS_KIT);
    let (_, r) = app::window::probe_face(&ctx, shell, place, app::window::MIN_W, app::window::MIN_H);
    assert!(!banner_head.is_empty() && !r.texts.iter().any(|t| t.starts_with(&banner_head)), "no result shown: {:?}", r.texts);
    assert!(r.texts.iter().any(|t| t == app::lang::t(app::lang::Key::U3KitKey)), "the export page was drawn: {:?}", r.texts);
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_sentences_say_what_the_product_does() {
    use app::lang::Key as L;
    let both = |k: L| {
        let (zh, en) = app::lang::both(k);
        [zh.to_string(), en.to_string()]
    };
    assert_eq!(both(L::GrantFileRow), ["授权文件".to_string(), "Grant file".to_string()]);
    let [zh, en] = both(L::SaidAdopted);
    assert!(zh.contains("本机数据目录") && zh.contains("原文件夹未动") && !zh.contains("原位"), "{zh}");
    assert!(en.contains("data folder") && en.contains("unchanged") && !en.contains("in place"), "{en}");
    for k in [L::CheckIssuerLedgerHint, L::NoteNoLedger] {
        let [zh, en] = both(k);
        assert!(!zh.contains("拖入") && !en.to_lowercase().contains("drop"), "{zh} / {en}");
        assert!(zh.contains("账本目录") && zh.contains("授权文件") && zh.contains("https"), "{zh}");
    }
}

#[test]
fn a_held_grants_issuer_is_named_by_its_address() {
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.settings.role = app::roles::Role::Grantee;
    let author = format!("0x{}", "ab12".repeat(10));
    shell.held = Some(vec![app::vaultx::Held {
        id: format!("0x{}", "cd".repeat(32)),
        bytes: Vec::new(),
        author: author.clone(),
        grantee: String::new(),
        work: format!("0x{}", "ef".repeat(32)),
        terms: String::new(),
        window: None,
        upstream: None,
        seq: 2,
    }]);
    let place = app::nav::Place::View(app::nav::View::MyGrants, 0);
    let (_, r) = app::window::probe_face(&ctx, shell, place, 1180.0, 760.0);
    let shown = zikaron_ui::width::elide_chars(&author, 13);
    assert!(r.texts.iter().any(|t| t.contains(&shown)), "the issuer by its address {shown}: {:?}", r.texts);
    assert!(!r.texts.iter().any(|t| t == app::lang::t(app::lang::Key::UnnamedIssuer)), "never unnamed with an address: {:?}", r.texts);
}

#[test]
fn a_folder_says_the_newer_packages_apart_from_what_was_added() {
    if super::alone_in(module_path!(), "a_folder_says_the_newer_packages_apart_from_what_was_added") {
        return;
    }
    let at = base("f274");
    let (mut a, grant, _, _) = author(&at, "授权的那一件");
    let p1 = package(&a, &grant, &at.join("p1.zkgrant"));
    let mut g = grantee(&at.join("grantee"));
    assert!(matches!(apply(&mut g, Action::ImportGrant { typed: p1.display().to_string() }), Applied::Held { .. }));
    assert!(matches!(apply_settled(&mut a, Action::Revoke { grant: grant.clone(), case: String::new() }), Applied::Revoked { .. }));
    let only = at.join("only-newer");
    std::fs::create_dir_all(&only).expect("a folder");
    let _ = package(&a, &grant, &only.join("p2.zkgrant"));
    let got = apply(&mut g, Action::ImportGrantDir { dir: only.display().to_string() });
    assert!(matches!(&got, Applied::Held { ids, renewed: 1, .. } if ids.is_empty()), "renewed, none added: {got:?}");
    assert_eq!(app::window::held_said(0, 1, 0), app::lang::fill1(app::lang::Key::SaidHeldRenewedN, "1"));
    assert_ne!(app::window::held_said(0, 1, 0), app::window::held_said(1, 0, 0), "never said as one added");
    let _ = apply_settled(&mut a, Action::Annotate { subject: String::new(), note_md: "更长".into() });
    let mixed = at.join("mixed");
    std::fs::create_dir_all(&mixed).expect("a folder");
    let _ = package(&a, &grant, &mixed.join("p3.zkgrant"));
    std::fs::write(mixed.join("not-a-grant.txt"), b"no grant").expect("a file");
    let got = apply(&mut g, Action::ImportGrantDir { dir: mixed.display().to_string() });
    assert!(matches!(&got, Applied::HeldPartly { ids, renewed: 1, refused: 1 } if ids.is_empty()), "{got:?}");
    assert_eq!(app::window::held_said(0, 1, 1), app::lang::fill3(app::lang::Key::SaidHeldRenewedPartly, "0", "1", "1"));
}

#[test]
fn a_grant_held_already_is_named_by_the_grant_the_import_is_for() {
    if super::alone_in(module_path!(), "a_grant_held_already_is_named_by_the_grant_the_import_is_for") {
        return;
    }
    let at = base("held-named");
    let bytes_of = |shell: &app::shell::Shell, id: &str| -> Vec<u8> {
        let items = shell.home.as_ref().expect("a home").ledger().expect("a ledger").pile().expect("its entries").items;
        items.into_iter().find(|b| zikaron_glue::book::check(b).is_ok_and(|e| e.id_hex().eq_ignore_ascii_case(id))).expect("the grant's bytes")
    };
    let (a, up, _, _) = author(&at.join("a"), "上游");
    let g1 = bytes_of(&a, &up);
    let (mut b, _, _, _) = author(&at.join("b"), "下游");
    let work = zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(b"the granted work"));
    let down = earlier_grant(&mut b, &format!("0x{}", "cc".repeat(20)), &work, &format!("0x{}", "55".repeat(32)), Some(&up));
    let g2 = bytes_of(&b, &down);
    let code = app::badgex::payload_text(&[g1, g2]).expect("a code");
    let mut g = grantee(&at.join("grantee"));
    assert!(matches!(apply(&mut g, Action::ImportGrant { typed: code.clone() }), Applied::Held { .. }), "taken in");
    let again = apply(&mut g, Action::ImportGrant { typed: code });
    match &again {
        Applied::Trouble(f) if f.which() == Some(Known::AlreadyHeld) => assert!(f.tail().eq_ignore_ascii_case(&down), "said of the sublicense: {f:?}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_newer_ledger_the_person_names_is_read_over_the_vaults_copy() {
    if super::alone_in(module_path!(), "a_newer_ledger_the_person_names_is_read_over_the_vaults_copy") {
        return;
    }
    let at = base("upstream-newer");
    let (mut a, grant, _, _) = author(&at, "授权的那一件");
    let p1 = package(&a, &grant, &at.join("p1.zkgrant"));
    let mut g = grantee(&at.join("grantee"));
    assert!(matches!(apply(&mut g, Action::ImportGrant { typed: p1.display().to_string() }), Applied::Held { .. }));
    assert!(matches!(apply_settled(&mut a, Action::Revoke { grant: grant.clone(), case: String::new() }), Applied::Revoked { .. }));
    let up = package(&a, &grant, &at.join("upstream.zkgrant"));
    assert!(matches!(apply(&mut g, Action::SetUpstream { grant: grant.clone(), dir: up.display().to_string() }), Applied::Upstream { .. }));
    drop(a);
    std::fs::rename(at.join("author"), at.join("author-gone")).expect("moved away");
    assert!(matches!(apply(&mut g, Action::ReviewVault), Applied::Started(Kind::Review)));
    match landed(&mut g, Kind::Review) {
        Ok(Done::Reviewed { cards, .. }) => {
            let c = cards.iter().find(|c| c.id.eq_ignore_ascii_case(&grant)).expect("the grant's card");
            assert!(c.failed.iter().any(|x| x == zikaron_kit::tokens::Check::Revoked.as_str()), "the recorded ledger's revocation is read: {c:?}");
        }
        other => panic!("review: {other:?}"),
    }
    let other = at.join("other");
    let (b, grant_b, _, _) = author(&other, "另一位");
    let pb = package(&b, &grant_b, &other.join("pb.zkgrant"));
    assert!(matches!(apply(&mut g, Action::ImportGrant { typed: pb.display().to_string() }), Applied::Held { .. }));
    let held = app::vaultx::held(g.home.as_ref().expect("home")).expect("the vault");
    let bytes = held.iter().find(|h| h.id.eq_ignore_ascii_case(&grant)).expect("held").bytes.clone();
    let e = zikaron_glue::book::check(&bytes).expect("a grant");
    let dir = g.home.as_ref().expect("a home").dir(app::home::Slot::GrantsHeld).join(app::grantfilex::KEPT);
    let mut misses = Vec::new();
    assert!(app::supplyx::kept_copy(&dir, &e.id_hex(), &e.author, &mut misses).is_some(), "this grant's copy");
    assert!(misses.is_empty(), "the other issuer's package is no miss: {misses:?}");
}

#[test]
fn a_verification_refused_on_the_spot_leaves_no_earlier_verdict() {
    if super::alone_in(module_path!(), "a_verification_refused_on_the_spot_leaves_no_earlier_verdict") {
        return;
    }
    let at = base("verify-refused");
    let (a, grant, _, _) = author(&at, "核验");
    let p1 = package(&a, &grant, &at.join("p1.zkgrant"));
    let mut g = grantee(&at.join("grantee"));
    assert!(matches!(apply(&mut g, Action::VerifyWork { path: p1.display().to_string(), work: String::new(), then_to: String::new() }), Applied::Started(Kind::Verify)));
    let _ = landed(&mut g, Kind::Verify);
    assert!(g.verified.is_some(), "a verdict for the first package");
    let gone = at.join("nowhere");
    let refused = apply(&mut g, Action::VerifyWork { path: gone.display().to_string(), work: String::new(), then_to: String::new() });
    assert!(matches!(refused, Applied::Trouble(_)), "refused on the spot: {refused:?}");
    assert!(g.verified.is_none(), "the first package's verdict is gone with the try");
}

#[test]
fn a_sealed_grant_package_opens_where_a_grant_is_taken() {
    if super::alone_in(module_path!(), "a_sealed_grant_package_opens_where_a_grant_is_taken") {
        return;
    }
    let at = base("b33-15");
    let (a, grant, _, _) = author(&at, "授权的那一件");
    let home = a.home.as_ref().expect("home");
    let n = node(|m, b| chain(m, b, PLENTY));
    let ask = app::exitgate::Ask { root: home.root().to_path_buf(), lanes: vec![app::exitgate::Lane { chain: 31337, registry: [0x11; 20], from_block: 0, eps: vec![app::chainx::Endpoint::at(31337, n.url.clone())] }], own: None };
    let pass = app::exitgate::pass(&ask).expect("the gate passes on an empty chain");
    let (keys, _) = app::addressedx::local_keys().expect("this machine's keys");
    let mine = keys.first().and_then(|k| k.public()).expect("an identity here");
    let out = at.join("out");
    std::fs::create_dir_all(&out).expect("a folder");
    let sealed = app::grantfilex::export(&pass, home, &grant, &out, app::grantfilex::Pick::No, Some(&mine)).expect("sealed").path;
    let hops = app::checkx::hops_of(&sealed.display().to_string()).expect("opened");
    assert_eq!(hops.form, app::payloadx::Form::GrantFile);
    let last = hops.hops.last().map(|b| zikaron_glue::book::check(b).expect("a grant").id_hex()).unwrap_or_default();
    assert!(last.eq_ignore_ascii_case(&grant), "the grant inside");
    let mut g = grantee(&at.join("grantee"));
    assert!(matches!(apply(&mut g, Action::ImportGrant { typed: sealed.display().to_string() }), Applied::Held { .. }));
    let kept = app::grantfilex::kept(g.home.as_ref().expect("home")).expect("the vault's packages");
    assert_eq!((kept_files(&g), kept.len()), (1, 1), "kept as the plain package");
    let stranger = app::key::generate().expect("a key").public().expect("its public key");
    let theirs = app::grantfilex::export(&pass, home, &grant, &out, app::grantfilex::Pick::No, Some(&stranger)).expect("sealed elsewhere").path;
    let refused = app::payloadx::take_full(&theirs.display().to_string()).err().expect("refused");
    assert_eq!(refused.which(), Some(Known::AddressedNotRecipient));
    let (zh, en) = app::lang::both(app::lang::Key::FaultWhatAddressedNotRecipient);
    assert_eq!((zh, en), ("这份加密包不是给本机身份的", "This encrypted package is not for an identity on this machine"));
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.settings.role = app::roles::Role::Grantee;
    let said = refused.human().to_string();
    shell.failed.insert(Kind::Check, refused);
    let place = app::nav::Place::View(app::nav::View::Verify, app::nav::tab::VERIFY_CHECK);
    let (_, r) = app::window::probe_face(&ctx, shell, place, app::window::MIN_W, app::window::MIN_H);
    assert!(r.texts.iter().any(|t| t == &said), "the refusal on the page: {:?}", r.texts);
    let _ = std::fs::remove_dir_all(&at);
}

fn anchor_of(id: &str, sender: &str, block: u64) -> zikaron::json::Value {
    use zikaron::json::Value;
    Value::Obj(vec![
        ("blockNumber".into(), Value::Int(block)),
        ("blockTimestamp".into(), Value::Int(1_000 + block)),
        ("chainId".into(), Value::Int(31337)),
        ("hash".into(), Value::Str(id.to_string())),
        ("sender".into(), Value::Str(sender.to_string())),
        ("tx".into(), Value::Str(format!("0x{block:064x}"))),
        ("verdict".into(), Value::Str("counted".into())),
    ])
}

#[test]
fn a_copy_behind_the_chain_says_the_issuers_ledger_has_updates() {
    if super::alone_in(module_path!(), "a_copy_behind_the_chain_says_the_issuers_ledger_has_updates") {
        return;
    }
    use zikaron::json::Value;
    let at = base("b33-17");
    let (mut a, grant, _, _) = author(&at, "授权的那一件");
    let pile = |a: &app::shell::Shell| a.home.as_ref().expect("home").ledger().expect("a ledger").pile().expect("its entries").items;
    let p1 = pile(&a);
    assert!(matches!(apply_settled(&mut a, Action::Revoke { grant: grant.clone(), case: String::new() }), Applied::Revoked { .. }));
    let p2 = pile(&a);
    let ids: Vec<String> = p2.iter().map(|b| zikaron_glue::book::check(b).expect("an entry").id_hex()).collect();
    let bytes = p1.iter().find(|b| zikaron_glue::book::check(b).is_ok_and(|e| e.id_hex().eq_ignore_ascii_case(&grant))).cloned().expect("the grant");
    let author_addr = zikaron_glue::book::check(&bytes).expect("a grant").author;
    let anchors: Vec<Value> = ids.iter().enumerate().map(|(i, id)| anchor_of(id, &author_addr, 10 + i as u64)).collect();
    let window = Value::Obj(vec![
        ("chainId".into(), Value::Int(31337)),
        ("fromBlock".into(), Value::Int(0)),
        ("registries".into(), Value::Arr(vec![Value::Str(format!("0x{}", "11".repeat(20)))])),
        ("senders".into(), Value::Arr(vec![Value::Str(author_addr.to_ascii_lowercase())])),
        ("toBlock".into(), Value::Int(100)),
    ]);
    let fragment = Value::Obj(vec![("anchors".into(), Value::Arr(anchors)), ("basis".into(), Value::Obj(vec![("chains".into(), Value::Arr(vec![window]))]))]);
    let e = zikaron_glue::book::check(&bytes).expect("a grant");
    let held = app::vaultx::Held { id: e.id_hex(), bytes: bytes.clone(), author: e.author.clone(), grantee: String::new(), work: String::new(), terms: String::new(), window: None, upstream: None, seq: e.seq };
    let mut old = app::vaultx::review(&held, &p1, &fragment, Some(2_000), ids.len());
    old.from = Some((app::supplyx::Level::Vault, String::new()));
    assert!(old.lacks, "the copy lacks entries the chain shows: {:?}", old.failed);
    assert_eq!(old.gaps(), vec![app::checkx::Gap::LedgerBehind], "what is lacking: {:?}", app::vaultx::states(&old.checks));
    let mut new = app::vaultx::review(&held, &p2, &fragment, Some(2_000), ids.len());
    new.from = Some((app::supplyx::Level::Vault, String::new()));
    assert!(!new.lacks && new.failed.iter().any(|x| x == zikaron_kit::tokens::Check::Revoked.as_str()), "the newer copy: revoked");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.settings.role = app::roles::Role::Grantee;
    shell.held = Some(vec![held]);
    shell.cards = Some((vec![old], None));
    let (_, r) = app::window::probe_face(&ctx, shell, app::nav::Place::View(app::nav::View::MyGrants, 0), 1180.0, 760.0);
    use app::lang::{t, Key as L};
    assert!(r.texts.iter().any(|x| x == t(L::GapLedgerBehind)), "wants the newer package: {:?}", r.texts);
    assert!(!r.texts.iter().any(|x| x == t(L::U3CheckSomeMissing) || x == t(L::NoteNotYetAnchored)), "not checks missing, not unanchored: {:?}", r.texts);
    let _ = std::fs::remove_dir_all(&at);
}
