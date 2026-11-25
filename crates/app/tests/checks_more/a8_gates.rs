use app::action::{apply, apply_settled, Action, Applied};
use app::fault::Known;
use app::shell::Shell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use zikaron::json::Value;

const PIN: &str = "27618394";
const OTHER_PIN: &str = "81726354";
const PW: &str = "zikaron-a8-backup-probe";

fn light_vault() {
    assert!(app::keybox::set_light_kdf(None), "the light level set once, before any derivation");
    super::vault_open();
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zk-a8-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("a scratch folder");
    d
}

fn files(dir: &Path, skip: &[&str]) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(list) = std::fs::read_dir(at) else { return };
        for e in list.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).map(|r| r.display().to_string()).unwrap_or_default();
                out.push((rel, std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.retain(|(rel, _)| !skip.iter().any(|s| rel.ends_with(s)));
    out.sort();
    out
}

#[cfg(unix)]
fn shut(dir: &Path) -> std::fs::Permissions {
    let was = std::fs::metadata(dir).expect("metadata").permissions();
    let mut p = was.clone();
    p.set_readonly(true);
    std::fs::set_permissions(dir, p).expect("read-only");
    was
}

#[cfg(unix)]
fn reopen(dir: &Path, was: std::fs::Permissions) {
    std::fs::set_permissions(dir, was).expect("back");
}

fn edit(bytes: &[u8], path: &[&str], f: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v = zikaron::json::parse(bytes).ok().expect("the document reads");
    let mut at = &mut v;
    for k in path {
        at = match at {
            Value::Obj(m) => &mut m.iter_mut().find(|(n, _)| n == k).unwrap_or_else(|| panic!("no member {k}")).1,
            Value::Arr(a) => &mut a[k.parse::<usize>().expect("an index")],
            other => panic!("{k}: not a container: {other:?}"),
        };
    }
    f(at);
    zikaron::json::canon_bytes(&v)
}

fn flip(v: &mut Value) {
    let Value::Str(s) = v else { panic!("not text: {v:?}") };
    let last = s.pop().expect("not empty");
    s.push(if last == '0' { '1' } else { '0' });
}

fn member(bytes: &[u8], path: &[&str]) -> Value {
    let mut v = zikaron::json::parse(bytes).ok().expect("the document reads");
    for k in path {
        v = match v {
            Value::Obj(m) => m.into_iter().find(|(n, _)| n == k).unwrap_or_else(|| panic!("no member {k}")).1,
            other => panic!("{k}: not an object: {other:?}"),
        };
    }
    v
}

fn known(r: Result<impl Sized, app::fault::Fault>) -> Option<Known> {
    r.err().and_then(|f| f.which())
}

fn vault() -> Vec<u8> {
    std::fs::read(app::keybox::path().expect("the vault's place")).expect("the vault")
}

fn put_vault(b: &[u8]) {
    std::fs::write(app::keybox::path().expect("the vault's place"), b).expect("the vault written");
}

struct Ids {
    genesis: String,
    history: String,
    grant: String,
    probe: String,
    draft: app::grantx::Draft,
}

fn every(ids: &Ids) -> Vec<Action> {
    use app::secret::Secret;
    let s = String::new;
    vec![
        Action::Show(app::shell::Page::FirstRun),
        Action::SelfCheck,
        Action::Measure,
        Action::Quit,
        Action::SwitchRole,
        Action::ReadIdentities,
        Action::NewIdentity,
        Action::ConfirmIdentity { answers: Vec::new(), label: s(), network: s() },
        Action::DropFresh,
        Action::ImportIdentity { form: app::action::ImportForm::Words(Secret::default()), seat: app::roles::Role::Author, label: s(), network: s() },
        Action::SwitchIdentity { id: s() },
        Action::DeleteIdentity { id: s(), pin: Secret::default() },
        Action::NameIdentity { id: s(), label: s() },
        Action::SetPin { pin: Secret::default(), again: Secret::default() },
        Action::Unlock { pin: Secret::default() },
        Action::Reseal { pin: Secret::default() },
        Action::ResetEmptyKeybox,
        Action::SetMachineAside,
        Action::Lock,
        Action::ChangePin { old: Secret::default(), pin: Secret::default(), again: Secret::default() },
        Action::RecoverWords { words: Secret::default(), pin: Secret::default(), again: Secret::default() },
        Action::RecoverKeystore { path: s(), password: Secret::default(), pin: Secret::default(), again: Secret::default() },
        Action::RevealWords { pin: Secret::default() },
        Action::HideWords,
        Action::BackupKey { pin: Secret::default(), password: Secret::default(), again: Secret::default(), dir: s() },
        Action::SetAutoLock { on: true, secs: 900 },
        Action::SetPrimary { id: s(), pin: Secret::default() },
        Action::ExportBackup { pin: Secret::default(), password: Secret::default(), again: Secret::default(), dir: s() },
        Action::PeekBackup { path: s(), password: Secret::default() },
        Action::RestoreBackup { path: s(), password: Secret::default(), how: app::action::RestoreHow::FirstRun },
        Action::Resume,
        Action::CatchUp,
        Action::ChooseNetwork { name: s() },
        Action::OpenHome { root: s() },
        Action::ChangeHome { root: s() },
        Action::MigrateHome { to: s() },
        Action::SetCap { bytes: 1 },
        Action::ExportMirror { to: s() },
        Action::Reconcile,
        Action::RecheckAll,
        Action::ReadChain,
        Action::ReadChainTime,
        Action::SetEndpoints { specs: s() },
        Action::SetLogAsks { n: 7 },
        Action::SetRestMode { on: true },
        Action::AddRest { base: "https://api.etherscan.io/v2/api".into(), key: app::secret::Secret::of("a8-key") },
        Action::RemoveRest { base: "https://api.etherscan.io/v2/api".into() },
        Action::Genesis { statement: "a8 again".into() },
        Action::Adopt { dir: s() },
        Action::ReadLedger,
        Action::OpenEntry { id: s() },
        Action::Annotate { subject: ids.genesis.clone(), note_md: "a8".into() },
        Action::Retract { subject: ids.history.clone(), note_md: "a8".into() },
        Action::Audit,
        Action::SetBasis { chain: s(), registry: s(), from_block: s() },
        Action::SetAuditEvery { secs: 1 },
        Action::SetAutoAnchor { on: true },
        Action::SetHideLocalDeletions { on: true },
        Action::TakeContent { source: app::anchorx::Source::File, path: s() },
        Action::RecordWork { note_md: "a8".into(), files: Vec::new(), for_: None },
        Action::RegisterRepo { path: s() },
        Action::SetKitLink { path: s(), link: s() },
        Action::DropKitCopy { path: s() },
        Action::CheckRepo,
        Action::TakeDropped { path: s() },
        Action::EstimateGas { count: 1 },
        Action::SendBatch { count: 1 },
        Action::BumpFee { tx: format!("0x{}", "ef".repeat(32)), cap: 1 },
        Action::PickKit { from: s(), to: s(), ids: s() },
        Action::ExportKit { from: s(), to: s(), ids: s(), attach: s(), note: s(), out: s(), form: "folder".into(), recipient: String::new(), recipient_key: String::new(), originals: "none".into(), companion: true },
        Action::VetAttachments { paths: Vec::new() },
        Action::Peek { ask: app::peekx::Ask::Size { path: s() } },
        Action::DraftGrant { draft: Box::new(ids.draft.clone()), exclusive: false, terms_file: None },
        Action::QueueEntry { id: s() },
        Action::ReadGrants,
        Action::CheckClash { work: s(), from: s(), to: s() },
        Action::Revoke { grant: ids.grant.clone(), case: "a8".into() },
        Action::MigrateGrants,
        Action::ReadStory { grant: s() },
        Action::Upgrade,
        Action::UpgradeCost,
        Action::Rotate { step: app::action::RotateStep::Write },
        Action::MoveChain { chain: "31338".into(), registry: format!("0x{}", "12".repeat(20)), from_block: "0".into(), nodes: "http://127.0.0.1:9".into() },
        Action::ReadBook { address: s(), dir: s() },
        Action::FaceReread,
        Action::FaceBooks,
        Action::FaceHome { chain: 31337 },
        Action::FaceAccount { platform: "x".into(), handle: "a8".into(), note: s() },
        Action::FaceKey { address: format!("0x{}", "13".repeat(20)), role: "payout".into(), note: s(), consent: s(), sign_here: true, pin: Secret::default() },
        Action::FaceName { name: "a8".into() },
        Action::FaceRelease { subject: format!("0x{}", "ab".repeat(32)) },
        Action::FaceReseat { entry: format!("0x{}", "ab".repeat(32)) },
        Action::FaceConsent { identity: format!("0x{}", "13".repeat(20)), role: "payout".into(), pin: Secret::default() },
        Action::FaceReroot { to: format!("0x{}", "14".repeat(20)) },
        Action::FaceOrigin { from: format!("0x{}", "15".repeat(20)) },
        Action::FaceRerootMake { answers: Vec::new(), label: "a8".into(), chain: 31337 },
        Action::FaceRerootFee { go: false },
        Action::FaceJoined { root: format!("0x{}", "14".repeat(20)), to: format!("0x{}", "15".repeat(20)) },
        Action::RememberAddress { address: s() },
        Action::ForgetAddress { address: s() },
        Action::Diligence { address: s(), dir: s(), work: s(), from: s(), to: s() },
        Action::SaveSnapshot { to: s() },
        Action::VerifyWork { path: s(), work: s(), then_to: String::new() },
        Action::ExtractOriginals { kit: s(), to: s() },
        Action::CheckDelivery { path: s(), expect: s() },
        Action::CheckPayload { typed: s(), ledgers: s(), endpoints: s(), registry: s(), from_block: s(), now: s(), file: s(), terms: s() },
        Action::AuditCheck,
        Action::AuditOthers,
        Action::ImportGrant { typed: s() },
        Action::ImportGrantDir { dir: s() },
        Action::SetUpstream { grant: s(), dir: s() },
        Action::NoteHeld { grant: s(), note: s(), issuer_note: s() },
        Action::ReviewVault,
        Action::ListHeld,
        Action::SetReviewEvery { secs: s() },
        Action::SetLang { lang: app::lang::Lang::En },
        Action::SetZone { zone: app::when::Zone::Utc },
        Action::SetAppearance { appearance: s() },
        Action::ExportBadge { grant: s(), out: s() },
        Action::CopyGrantCode { grant: s() },
        Action::ExportGrantFile { id: s(), to: s(), original: String::new(), seal_to: String::new(), key: None },
        Action::SetPublish { url: s() },
        Action::CheckPublished { local: s() },
        Action::FetchLedger { from: s(), password: Secret::default() },
        Action::FetchAside { from: s(), password: Secret::default() },
        Action::CheckTail,
        Action::ViewOldData { root: s() },
        Action::LeaveOldData,
        Action::TakeWriter,
        Action::SaveReadNetwork { was: None, name: s(), chain: s(), registry: s(), from_block: s(), nodes: s() },
        Action::RemoveReadNetwork { chain: 1, registry: s() },
        Action::ReadReadNetwork { chain: 1, registry: s() },
        Action::ReadCliPath,
        Action::SetCliPath { on: false },
        Action::SetCliAnchor { to: app::machine::CliAnchor::Send },
        Action::SetCliDesktop { on: true },
        Action::SetCliAlone { lasts: app::machine::Lasts::Never, pin: String::new().into() },
        Action::CliAloneOff,
        Action::CliPassAdd { note: "a8".into(), pin: String::new().into() },
        Action::CliPassDrop { tag: "0000000000000000".into() },
        Action::CleanExports { yes: String::new() },
        Action::SendAsked { count: 1 },
        Action::SetProxy { choice: s() },
    ]
}

fn settle(shell: &mut Shell) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !shell.tasks.flying().is_empty() && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    shell.drain();
}

fn ready(shell: &mut Shell, content: &app::anchorx::Content) {
    shell.content = Some(content.clone());
    shell.gas = Some((1, 21_000));
}

fn drive_read_only(shell: &mut Shell, mode: &str, root: &Path, ids: &Ids, content: &app::anchorx::Content) -> usize {
    let skip = [app::lock::LOCK_FILE];
    settle(shell);
    let before = files(root, &skip);
    let mut writes: Vec<Action> = every(ids).into_iter().filter(|a| a.writes_ledger()).collect();
    writes.push(Action::RecordWork { note_md: "a8".into(), files: vec![ids.probe.clone()], for_: None });
    let mut refused = 0;
    for a in writes {
        ready(shell, content);
        let name = a.name();
        let got = apply_settled(shell, a);
        let f = match got {
            Applied::Trouble(f) => f,
            Applied::RecordedBatch { ids, stopped: Some((0, _, f)), .. } if ids.is_empty() => f,
            other => panic!("{mode} {name}: not refused: {other:?}"),
        };
        assert_eq!(f.which(), Some(Known::ReadOnly), "{mode} {name}: refused as read-only, not {} · {}", f.said(), f.tail());
        refused += 1;
    }
    settle(shell);
    assert_eq!(files(root, &skip), before, "{mode}: not one byte of the home written");
    refused
}

#[test]
fn every_ledger_write_is_refused_as_read_only_in_both_reader_modes() {
    if super::alone_in(module_path!(), "every_ledger_write_is_refused_as_read_only_in_both_reader_modes") {
        return;
    }
    light_vault();
    let base = scratch("single-writer");
    let root = base.join("home");
    let ctx = zikaron_ui::egui::Context::default();
    let boot = || Shell::boot(zikaron_ui::skin::dress(&ctx));
    let mut w = boot();
    super::seated(&mut w, &root);
    let opened = apply(&mut w, Action::OpenHome { root: root.display().to_string() });
    assert!(matches!(opened, Applied::Homed { mode: app::lock::Mode::Writer, .. }), "{opened:?}");
    let genesis = match apply_settled(&mut w, Action::Genesis { statement: "a8".into() }) {
        Applied::Genesised { id, .. } => id,
        other => panic!("genesis: {other:?}"),
    };
    let probe = base.join("probe.txt");
    std::fs::write(&probe, b"a8 single writer").expect("a file");
    let _ = apply_settled(&mut w, Action::TakeContent { source: app::anchorx::Source::File, path: probe.display().to_string() });
    let content = w.content.clone().expect("the content taken");
    let history = match apply_settled(&mut w, Action::RecordWork { note_md: "a8".into(), files: Vec::new(), for_: None }) {
        Applied::Recorded { id, .. } => id,
        other => panic!("record: {other:?}"),
    };
    let draft = app::grantx::Draft { grantee: format!("0x{}", "33".repeat(20)), work: content.hex(), terms: format!("0x{}", "44".repeat(32)), ..Default::default() };
    super::venue::in_venue(&mut w, &super::venue::venue());
    let grant = match apply_settled(&mut w, Action::DraftGrant { draft: Box::new(draft.clone()), exclusive: false, terms_file: None }) {
        Applied::Granted { id, .. } => id,
        other => panic!("grant: {other:?}"),
    };
    let ids = Ids { genesis, history, grant, probe: probe.display().to_string(), draft };
    let names: BTreeSet<&str> = every(&ids).iter().map(|a| a.name()).collect();
    assert_eq!(names, Action::NAMES.iter().copied().collect::<BTreeSet<&str>>(), "one sample of every member");
    assert_eq!(every(&ids).len(), Action::NAMES.len(), "one sample each");
    let set: Vec<&str> = every(&ids).iter().filter(|a| a.writes_ledger()).map(|a| a.name()).collect();
    assert!(set.len() >= 9, "the ledger-writing set: {set:?}");
    let mut r = boot();
    let opened = apply(&mut r, Action::OpenHome { root: root.display().to_string() });
    assert!(matches!(opened, Applied::Homed { mode: app::lock::Mode::Reader, .. }), "{opened:?}");
    settle(&mut w);
    let n = drive_read_only(&mut r, "reader", &root, &ids, &content);
    assert_eq!(n, set.len() + 1, "every member, and recording with files");
    drop(r);
    drop(w);
    let home = app::home::Home::open(&root).expect("the home");
    std::fs::write(app::lock::mark_path(&home), "0123456789abcdef0123456789abcdef\n").expect("another machine's mark");
    let mut o = boot();
    let opened = apply(&mut o, Action::OpenHome { root: root.display().to_string() });
    assert!(matches!(opened, Applied::Homed { mode: app::lock::Mode::OtherMachine, .. }), "{opened:?}");
    let n = drive_read_only(&mut o, "otherMachine", &root, &ids, &content);
    assert_eq!(n, set.len() + 1);
    drop(o);
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn the_wrong_count_is_written_before_the_derivation_and_none_runs_without_it() {
    if super::alone_in(module_path!(), "the_wrong_count_is_written_before_the_derivation_and_none_runs_without_it") {
        return;
    }
    light_vault();
    app::keybox::lock();
    let wrong = |b: &[u8]| member(b, &[app::keybox::member::WRONG]);
    let d = app::cryptx::derivations();
    assert_eq!(known(app::keybox::unlock(OTHER_PIN)), Some(Known::PinWrong));
    assert_eq!(app::cryptx::derivations(), d + 1, "one derivation");
    assert_eq!(wrong(&vault()), Value::Int(1), "the try is on disk");
    assert_eq!(app::keybox::tries_left().ok(), Some(4));
    let held = vault();
    let m = super::checks_machine();
    let was = shut(&m);
    let mut said = Vec::new();
    for (form, pin) in [("wrongPasscode", OTHER_PIN), ("rightPasscode", PIN)] {
        let d = app::cryptx::derivations();
        let got = app::keybox::unlock(pin);
        assert_eq!(app::cryptx::derivations(), d, "{form}: no derivation ran");
        let f = got.err().unwrap_or_else(|| panic!("{form}: refused"));
        assert_ne!(f.which(), Some(Known::PinWrong), "{form}: not judged: {}", f.said());
        assert!(f.tail().starts_with(&format!("{}:", m.join(app::places::keybox_file()).display())), "{form}: the count's write named: {}", f.tail());
        said.push(f.said().to_string());
    }
    reopen(&m, was);
    assert!(said.iter().all(|s| !s.is_empty()), "named: {said:?}");
    assert_eq!(vault(), held, "the vault as it was");
    assert_eq!(app::keybox::state().ok(), Some(app::keybox::State::Locked { wrong: 1 }), "not opened");
    app::keybox::unlock(PIN).expect("opens");
    assert_eq!(app::keybox::tries_left().ok(), Some(5));
}

#[test]
fn a_wrong_count_missing_or_not_an_integer_refuses_the_whole_vault() {
    if super::alone_in(module_path!(), "a_wrong_count_missing_or_not_an_integer_refuses_the_whole_vault") {
        return;
    }
    light_vault();
    app::keybox::lock();
    assert_eq!(known(app::keybox::unlock(OTHER_PIN)), Some(Known::PinWrong));
    let good = vault();
    let text = String::from_utf8(good.clone()).expect("the vault is text");
    let cell = format!("\"{}\":1", app::keybox::member::WRONG);
    assert_eq!(text.matches(&cell).count(), 1, "one count cell: {cell}");
    let absent = {
        let cut = text.replacen(&format!(",{cell}"), "", 1);
        if cut == text { text.replacen(&format!("{cell},"), "", 1) } else { cut }
    };
    assert_ne!(absent, text);
    let as_ = |v: &str| text.replacen(&cell, &format!("\"{}\":{v}", app::keybox::member::WRONG), 1);
    for (form, bytes, names_cell) in [
        ("absent", absent.clone(), true),
        ("text", as_("\"1\""), true),
        ("null", as_("null"), true),
        ("true", as_("true"), true),
        ("object", as_("{\"n\":1}"), true),
        ("negative", as_("-1"), false),
        ("fraction", as_("1.5"), false),
    ] {
        put_vault(bytes.as_bytes());
        for (what, got) in [("state", app::keybox::state().err()), ("triesLeft", app::keybox::tries_left().err())] {
            let f = got.unwrap_or_else(|| panic!("{form} {what}: refused, never read as zero"));
            assert_eq!(f.which(), Some(Known::KeyboxShape), "{form} {what}: {}", f.said());
            if names_cell {
                assert_eq!(f.tail(), app::keybox::member::WRONG, "{form} {what}: the cell named");
            }
        }
        let d = app::cryptx::derivations();
        assert_eq!(known(app::keybox::unlock(PIN)), Some(Known::KeyboxShape), "{form}: the right passcode refused too");
        assert_eq!(app::cryptx::derivations(), d, "{form}: no derivation");
        assert_eq!(std::fs::read(app::keybox::path().unwrap()).unwrap(), bytes.as_bytes(), "{form}: not rewritten");
    }
    put_vault(&good);
    assert_eq!(app::keybox::tries_left().ok(), Some(4), "the count as written");
}

#[test]
fn stored_parameters_below_the_factory_floor_are_refused_by_name_and_not_counted() {
    if super::alone_in(module_path!(), "stored_parameters_below_the_factory_floor_are_refused_by_name_and_not_counted") {
        return;
    }
    super::vault_open();
    let factory = app::keystore::Params { n: 262_144, r: 8, p: 1 };
    assert_eq!(app::keystore::Params::standard(), factory);
    assert_eq!(app::keybox::floor(), factory, "the floor is the factory level");
    app::keybox::lock();
    let good = vault();
    let kdf = |b: &[u8]| member(b, &[app::keybox::member::KDF]);
    assert_eq!(kdf(&good), Value::Obj(vec![("n".into(), Value::Int(262_144)), ("p".into(), Value::Int(1)), ("r".into(), Value::Int(8))]));
    for (form, n, r, p) in [("nHalved", 131_072u64, 8u64, 1u64), ("rHalved", 262_144, 4, 1), ("lightLevel", 4_096, 8, 6), ("probeLevel", 2, 8, 1)] {
        let low = edit(&good, &[app::keybox::member::KDF], |v| *v = Value::Obj(vec![("n".into(), Value::Int(n)), ("p".into(), Value::Int(p)), ("r".into(), Value::Int(r))]));
        put_vault(&low);
        for pin in [PIN, OTHER_PIN] {
            let d = app::cryptx::derivations();
            let f = app::keybox::unlock(pin).err().unwrap_or_else(|| panic!("{form}: refused"));
            assert_eq!(f.which(), Some(Known::KdfBelowFloor), "{form}: {}", f.said());
            assert_eq!(f.tail(), format!("n={n} r={r} p={p} < n=262144 r=8 p=1"), "{form}: both said");
            assert_eq!(app::cryptx::derivations(), d, "{form}: refused before the work");
        }
        assert_eq!(app::keybox::tries_left().ok(), Some(5), "{form}: not counted");
        assert_eq!(vault(), low, "{form}: not rewritten");
    }
    put_vault(&good);
    app::keybox::unlock(PIN).expect("the parameters as written open");
}

#[test]
fn each_seal_authenticates_its_parameters_and_salt() {
    if super::alone_in(module_path!(), "each_seal_authenticates_its_parameters_and_salt") {
        return;
    }
    light_vault();
    use app::keybox::member as m;
    let acct = format!("{}-a8-slot", app::places::key_account());
    app::keybox::put(&acct, &[7u8; 32]).expect("a slot");
    let words = [9u8; 16];
    let id = format!("0x{}", "5a".repeat(20));
    assert_eq!(app::keybox::add_recovery(&id, app::keybox::PrimaryKind::Words, &words).ok(), Some(true), "the primary's recovery seal");
    assert_eq!(app::keybox::get(&acct).ok().flatten(), Some(vec![7u8; 32]));
    assert_eq!(app::keybox::primary().ok().flatten().map(|(i, _)| i), Some(id.clone()));
    let good = vault();
    for (form, cell, to) in [("n", m::N, 4u64), ("r", m::R, 16), ("p", m::P, 2)] {
        put_vault(&edit(&good, &[m::KDF, cell], |v| *v = Value::Int(to)));
        assert_eq!(known(app::keybox::get(&acct)), Some(Known::KeyboxSlot), "{form}: the slot");
        assert_eq!(known(app::keybox::primary()), Some(Known::KeyboxSlot), "{form}: the sealed id");
    }
    put_vault(&edit(&good, &[m::SLOTS, "0", m::SALT], flip));
    assert_eq!(known(app::keybox::get(&acct)), Some(Known::KeyboxSlot), "the slot's salt");
    put_vault(&edit(&good, &[m::PRIMARY, m::ID, m::SALT], flip));
    assert_eq!(known(app::keybox::primary()), Some(Known::KeyboxSlot), "the sealed id's salt");
    put_vault(&good);
    assert_eq!(app::keybox::get(&acct).ok().flatten(), Some(vec![7u8; 32]), "as written");
    app::keybox::lock();
    let not_open = |form: &str| {
        assert_ne!(app::keybox::state().ok(), Some(app::keybox::State::Open), "{form}: not opened");
        assert_eq!(known(app::keybox::get(&acct)), Some(Known::Locked), "{form}: no master key in memory");
    };
    for (form, bytes) in [("pinParameters", edit(&good, &[m::KDF, m::N], |v| *v = Value::Int(4))), ("pinSalt", edit(&good, &[m::PIN, m::SALT], flip))] {
        put_vault(&bytes);
        let f = app::keybox::unlock(PIN).err().unwrap_or_else(|| panic!("{form}: refused"));
        assert_eq!(f.which(), Some(Known::PinWrong), "{form}: {}", f.said());
        not_open(form);
    }
    for (form, bytes) in [("recoveryParameters", edit(&good, &[m::KDF, m::N], |v| *v = Value::Int(4))), ("recoverySalt", edit(&good, &[m::RECOVERY, "0", m::SALT], flip))] {
        put_vault(&bytes);
        assert_eq!(known(app::keybox::recover(&words, PIN)), Some(Known::RecoveryNoMatch), "{form}");
        not_open(form);
        assert_eq!(vault(), bytes, "{form}: nothing written");
    }
    put_vault(&good);
    app::keybox::unlock(PIN).expect("the file as written opens");
    assert_eq!(app::keybox::get(&acct).ok().flatten(), Some(vec![7u8; 32]));
}

fn machine_with_a_ledger(base: &Path) -> Shell {
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    super::seated(&mut shell, &base.join("home"));
    answers!(apply(&mut shell, Action::OpenHome { root: base.join("home").display().to_string() }), Applied::Homed { .. });
    let made = apply_settled(&mut shell, Action::Genesis { statement: "a8 backup".into() });
    assert!(matches!(made, Applied::Genesised { .. }), "{made:?}");
    settle(&mut shell);
    shell
}

fn parts(bytes: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let magic = app::backup::MAGIC;
    assert!(bytes.starts_with(magic), "the first line is the magic");
    let rest = &bytes[magic.len()..];
    let nl = rest.iter().position(|b| *b == b'\n').expect("a second line");
    (magic.to_vec(), rest[..nl].to_vec(), rest[nl + 1..].to_vec())
}

fn joined(magic: &[u8], head: &[u8], body: &[u8]) -> Vec<u8> {
    let mut out = magic.to_vec();
    out.extend_from_slice(head);
    out.push(b'\n');
    out.extend_from_slice(body);
    out
}

#[test]
fn the_backup_envelope_is_magic_head_and_ciphertext_bound_to_both_lines() {
    if super::alone_in(module_path!(), "the_backup_envelope_is_magic_head_and_ciphertext_bound_to_both_lines") {
        return;
    }
    light_vault();
    let base = scratch("envelope");
    let _shell = machine_with_a_ledger(&base);
    let made = app::backup::export(&base.join("out"), PW, 1_800_000_000).expect("a backup");
    let bytes = std::fs::read(&made.path).expect("the file");
    let (magic, head, body) = parts(&bytes);
    assert_eq!(magic, app::backup::MAGIC);
    let h = zikaron::json::parse(&head).ok().expect("the head reads");
    assert_eq!(zikaron::json::canon_bytes(&h), head, "canonical");
    assert_eq!(member(&head, &["app"]), Value::Str(app::backup::APP.into()));
    assert_eq!(member(&head, &["format"]), Value::Int(app::backup::FORMAT));
    assert_eq!(member(&head, &["created"]), Value::Int(1_800_000_000));
    let num = |k: &str| match member(&head, &["kdf", k]) {
        Value::Int(n) => n as usize,
        other => panic!("{k}: {other:?}"),
    };
    let hex = |v: Value| match v {
        Value::Str(s) => zikaron::hexfmt::decode(&s).expect("hex"),
        other => panic!("{other:?}"),
    };
    let salt = hex(member(&head, &["kdf", "salt"]));
    let nonce: [u8; 24] = hex(member(&head, &["nonce"])).try_into().expect("a 24-byte nonce");
    assert!(!body.windows(b"registry".len()).any(|w| w == b"registry"), "the package is not in the clear");
    let mut key = [0u8; 32];
    assert!(app::cryptx::scrypt(PW.as_bytes(), &salt, num("n"), num("r"), num("p"), &mut key));
    let mut both = magic.clone();
    both.extend_from_slice(&head);
    both.push(b'\n');
    let plain = app::cryptx::xchacha_open(&key, &nonce, &both, &body).expect("opens with both lines as additional data");
    assert_eq!(member(&plain, &["app"]), Value::Str(app::backup::APP.into()), "the plain package");
    assert!(matches!(member(&plain, &["registry"]), Value::Str(_)));
    let mut head_line = head.clone();
    head_line.push(b'\n');
    assert!(app::cryptx::xchacha_open(&key, &nonce, &head_line, &body).is_none(), "the head line alone is not the additional data");
    assert!(app::cryptx::xchacha_open(&key, &nonce, &magic, &body).is_none(), "the magic alone is not the additional data");
    let created = b"1800000000";
    let at = head.windows(created.len()).position(|w| w == created).expect("the creation time in the head");
    let mut altered = head.clone();
    altered[at + created.len() - 1] = b'1';
    let path = base.join("altered-head.zikaron");
    std::fs::write(&path, joined(&magic, &altered, &body)).unwrap();
    assert_eq!(known(app::backup::peek(&path, PW)), Some(Known::BackupPassword), "a head byte changed");
    let mut magic2 = magic.clone();
    magic2[0] ^= 0x01;
    std::fs::write(&path, joined(&magic2, &head, &body)).unwrap();
    assert_eq!(known(app::backup::peek(&path, PW)), Some(Known::BackupNotOurs), "a magic byte changed");
    assert_eq!(app::backup::peek(&made.path, PW).ok(), Some(made.summary.clone()));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn every_backup_read_refusal_is_named_and_touches_no_vault_or_count() {
    if super::alone_in(module_path!(), "every_backup_read_refusal_is_named_and_touches_no_vault_or_count") {
        return;
    }
    light_vault();
    let base = scratch("refusals");
    let shell = machine_with_a_ledger(&base);
    drop(shell);
    let made = app::backup::export(&base.join("out"), PW, 1_800_000_000).expect("a backup");
    let good = std::fs::read(&made.path).expect("the file");
    let (magic, head, body) = parts(&good);
    assert_eq!(known(app::keybox::unlock(OTHER_PIN)), Some(Known::PinWrong));
    let head_with = |path: &[&str], v: Value| joined(&magic, &edit(&head, path, |x| *x = v), &body);
    let without_created = {
        let mut h = zikaron::json::parse(&head).ok().expect("the head");
        if let Value::Obj(m) = &mut h {
            m.retain(|(k, _)| k != "created");
        }
        joined(&magic, &zikaron::json::canon_bytes(&h), &body)
    };
    let forms: Vec<(&str, Vec<u8>, &str, Known)> = vec![
        ("anotherAppsMagic", joined(b"zikaron-other/1\n", &head, &body), PW, Known::BackupNotOurs),
        ("anotherAppInHead", head_with(&["app"], Value::Str("another-app".into())), PW, Known::BackupNotOurs),
        ("notABackup", b"hello\nworld\n".to_vec(), PW, Known::BackupNotOurs),
        ("newerFormat", head_with(&["format"], Value::Int(app::backup::FORMAT + 1)), PW, Known::BackupTooNew),
        ("parametersOutOfBounds", head_with(&["kdf", "n"], Value::Int(1 << 40)), PW, Known::BackupShape),
        ("shortNonce", head_with(&["nonce"], Value::Str("0x00".into())), PW, Known::BackupShape),
        ("noCreationTime", without_created, PW, Known::BackupShape),
        ("formatAsText", head_with(&["format"], Value::Str("1".into())), PW, Known::BackupShape),
        ("wrongPassword", good.clone(), "not-the-backup-password", Known::BackupPassword),
    ];
    let m = super::checks_machine();
    let before = files(&m, &[]);
    let vault_before = vault();
    for (form, bytes, pw, want) in forms {
        let path = base.join(format!("{form}.zikaron"));
        std::fs::write(&path, &bytes).unwrap();
        let mouths: [(&str, Option<Known>); 4] = [
            ("peek", known(app::backup::peek(&path, pw))),
            ("restoreFirstRun", known(app::backup::restore(&path, pw, app::backup::From::FirstRun))),
            ("restoreSettings", known(app::backup::restore(&path, pw, app::backup::From::Settings(PIN)))),
            ("restoreLocked", known(app::backup::restore(&path, pw, app::backup::From::Locked(PIN)))),
        ];
        for (mouth, said) in mouths {
            assert_eq!(said, Some(want), "{form} {mouth}");
        }
        assert_eq!(vault(), vault_before, "{form}: the vault as it was");
        assert_eq!(app::keybox::tries_left().ok(), Some(4), "{form}: the count as it was");
        assert_eq!(app::keybox::state().ok(), Some(app::keybox::State::Open), "{form}: still open");
    }
    assert_eq!(files(&m, &[]), before, "not one byte of the machine directory changed");
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn a_backup_counts_only_once_read_back_and_reopened() {
    if super::alone_in(module_path!(), "a_backup_counts_only_once_read_back_and_reopened") {
        return;
    }
    light_vault();
    let base = scratch("write");
    let _shell = machine_with_a_ledger(&base);
    let (t1, t2, t3) = (1_800_000_000u64, 1_800_000_100u64, 1_800_000_200u64);
    let made = app::backup::export(&base.join("out"), PW, t1).expect("a backup");
    assert!(made.summary.entries >= 1, "it holds the ledger: {:?}", made.summary);
    assert!(zikaron_os::is_owner_only(&made.path).expect("on disk"), "owner-only");
    assert_eq!(app::backup::peek(&made.path, PW).ok(), Some(made.summary.clone()), "reopens to what was answered");
    let record = |at: &str| app::machine::read().unwrap_or_else(|f| panic!("{at}: {}", f.said()));
    let m = record("landed");
    assert_eq!(m.backup.as_ref().map(|b| (b.at, b.path.clone())), Some((t1, made.path.display().to_string())));
    assert_eq!(m.backup_failed, None);
    let closed = base.join("closed");
    std::fs::create_dir_all(&closed).unwrap();
    let was = shut(&closed);
    let refused = app::backup::export(&closed, PW, t2);
    reopen(&closed, was);
    let f = refused.err().expect("refused");
    assert!(!f.said().is_empty(), "named");
    assert_eq!(std::fs::read_dir(&closed).unwrap().count(), 0, "no file left: {}", f.said());
    let m = record("not landed");
    assert_eq!(m.backup_failed, None, "the export itself records no failure");
    assert_eq!(m.backup.as_ref().map(|b| b.at), Some(t1), "the last backup kept");
    let next = app::backup::export(&closed, PW, t3).expect("lands");
    assert_eq!(app::backup::peek(&next.path, PW).ok(), Some(next.summary.clone()));
    let m = record("landed again");
    assert_eq!((m.backup.as_ref().map(|b| b.at), m.backup_failed), (Some(t3), None));
    let _ = std::fs::remove_dir_all(&base);
}
