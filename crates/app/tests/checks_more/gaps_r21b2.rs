//! Grant codes read the same whatever white space surrounds them; key files meet the backup floor; adoption
//! through the door; home layout refusal in one place; node channels kept apart by place; numbered file landing;
//! passcode hand-off; node addresses shown without their keys; keyed file naming.

/// A code with surrounding or embedded white space (CRLF, spaces, no-break space, BOM, zero-width space, word
/// joiner, soft hyphen) gives the same answer as the clean code, typed or in a file. The code decodes to a
/// non-entry, so the expected answer is the kit's refusal; white space read as part of the code would fail
/// differently. Runs alone because answers are localized and other tests switch the language.
#[test]
fn a_code_with_white_space_reads_as_the_clean_code_typed_or_in_a_file() {
    if super::alone_in(module_path!(), "a_code_with_white_space_reads_as_the_clean_code_typed_or_in_a_file") {
        return;
    }
    let clean = format!("{}{}", zikaron_kit::tokens::BADGE_PREFIX, zikaron_kit::b64::encode(b"not an entry, a few bytes long"));
    let half = zikaron_kit::tokens::BADGE_PREFIX.len() + (clean.len() - zikaron_kit::tokens::BADGE_PREFIX.len()) / 2;
    let dir = std::env::temp_dir().join(format!("zk-test-code-space-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let said = |typed: &str| app::payloadx::take_full(typed).map(|t| t.hops).map_err(|f| format!("{} {}", f.said(), f.tail()));
    let in_file = |name: &str, text: &str| {
        let p = dir.join(name);
        std::fs::write(&p, text.as_bytes()).expect("file");
        said(&p.display().to_string())
    };
    let want = said(&clean);
    assert!(want.as_ref().err().is_some_and(|e| e.starts_with("PAYLOAD_REFUSED")), "the clean code is the kit's to judge: {want:?}");
    assert_eq!(in_file("clean", &clean), want, "the clean code in a file");
    assert_eq!(app::vaultx::take(&dir.join("clean").display().to_string()).map_err(|f| format!("{} {}", f.said(), f.tail())), want, "the vault, the clean code");
    for (form, text) in [
        ("a line break after it", format!("{clean}\r\n")),
        ("a space before it", format!(" {clean}")),
        ("a space inside it", format!("{} {}", &clean[..half], &clean[half..])),
        ("a no-break space after it", format!("{clean}\u{a0}")),
        ("a byte-order mark before it", format!("\u{feff}{clean}")),
        ("a zero-width space inside it", format!("{}\u{200b}{}", &clean[..half], &clean[half..])),
        ("a word joiner and a soft hyphen inside it", format!("{}\u{2060}\u{ad}{}", &clean[..half], &clean[half..])),
    ] {
        assert_eq!(said(&text), want, "typed, {form}");
        assert_eq!(in_file(form, &text), want, "in a file, {form}");
        assert_eq!(app::vaultx::take(&dir.join(form).display().to_string()).map_err(|f| format!("{} {}", f.said(), f.tail())), want, "the vault, {form}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A key file backup is sealed at no less than `keystore::BACKUP_FLOOR` in memory and work cost, checked by
/// reading the parameters back from the file. Shipped builds use the same floor for the vault.
#[test]
fn a_key_file_written_is_never_below_the_backup_floor() {
    if super::alone_in(module_path!(), "a_key_file_written_is_never_below_the_backup_floor") {
        return;
    }
    use app::action::{apply, Action, Applied};
    super::vault_open();
    let floor = app::keystore::BACKUP_FLOOR;
    assert_eq!(app::keystore::Params::standard(), floor, "the level written is the floor");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_));
    let dir = std::env::temp_dir().join(format!("zk-r21b2-keyfile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let pw = "a backup password";
    // The passcode opens the vault first (its own task), then the key file is written in the background.
    let answered = app::action::apply_settled(&mut shell, Action::BackupKey { pin: "27618394".into(), password: pw.into(), again: pw.into(), dir: dir.display().to_string() });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while shell.tasks.in_flight(app::task::Kind::Keystore) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
    let files: Vec<_> = std::fs::read_dir(&dir).expect("the folder").flatten().map(|e| e.path()).collect();
    assert_eq!(files.len(), 1, "one key file landed: {files:?}, answered {answered:?}, {:?}", shell.faults);
    let s = app::keystore::shape(&std::fs::read(&files[0]).expect("the key file")).expect("a key file's shape");
    let (mem, work) = (|n: usize, r: usize| n * r, |n: usize, r: usize, p: usize| n * r * p);
    assert!(mem(s.n, s.r) >= mem(floor.n, floor.r) && work(s.n, s.r, s.p) >= work(floor.n, floor.r, floor.p), "n={} r={} p={} below the floor", s.n, s.r, s.p);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The refusal of an incomplete home layout (`Known::CannotLay`) is made only in `home::lay`, which opening a
/// home goes through.
#[test]
fn a_home_laid_incomplete_is_judged_in_one_place() {
    let makers: Vec<String> = super::shipped()
        .into_iter()
        .filter(|(name, _)| !name.ends_with("fault.rs"))
        .filter(|(_, text)| super::code_only(text).contains("Known::CannotLay"))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(makers, vec!["home.rs".to_string()], "one maker, the home layer (a folder module reads as one text)");
    let lay = super::code_only(&super::read_src_file("home.rs").expect("home.rs"));
    let at = lay.find("pub fn lay(").expect("lay");
    assert!(lay[at..].split("\n}\n").next().unwrap_or_default().contains("Known::CannotLay"), "inside lay");
}

/// An `adopt` door request becomes the adoption page's action: its anchors JSON is parsed into rows by the
/// page's own reader, with each malformed shape refused by name; attestor and attestation pass through as given
/// (a half co-signature is refused by the action, see `door_b15::a_cosignature_is_both_halves_or_none_judged_once`).
#[test]
fn an_adoption_through_the_door_is_the_adoption_pages_action() {
    let ctx = zikaron_ui::egui::Context::default();
    let shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let tx = format!("0x{}", "11".repeat(32));
    let content = format!("0x{}", "77".repeat(32));
    let turn = |anchors: Option<&str>| {
        let mut args = vec![("attestor".to_string(), "0xa".to_string()), ("attestation".to_string(), "0xs".to_string())];
        if let Some(a) = anchors {
            args.push(("anchors".to_string(), a.to_string()));
        }
        let q = zikaron_glue::door::Request { verb: "adopt".into(), home: "/h".into(), args };
        match app::door::turn_of(&shell, &q) {
            Ok(app::door::Turn::Act(app::action::Action::AdoptAnchors { rows, attestor, attestation })) => {
                assert_eq!((attestor.as_str(), attestation.as_str()), ("0xa", "0xs"), "as given");
                app::adoptx::rows_of(&rows).map(|r| r.len()).map_err(|f| f.which())
            }
            _ => panic!("adopt turns into the adoption page's action"),
        }
    };
    let good = format!(r#"{{"chainId":31337,"content":"{content}","payloadKind":"bare","tx":"{tx}"}}"#);
    assert_eq!(turn(Some(&format!("[{good},{good}]"))), Ok(2), "two rows, read as the page reads them");
    let shape = Err(Some(app::fault::Known::SettingsShape));
    assert_eq!(turn(Some(&format!(r#"[{good},{{"chainId":31337,"payloadKind":"bare","tx":"{tx}"}}]"#))), shape, "a member missing: refused by its line");
    assert_eq!(turn(Some(&format!(r#"[{{"chainId":"x","content":"{content}","payloadKind":"bare","tx":"{tx}"}}]"#))), shape, "a chain id not whole");
    assert_eq!(turn(Some(&good)), shape, "not an array: one line, refused");
    assert_eq!(turn(Some("[{}]")), shape, "an element with nothing in it");
    // An element must have exactly its four members, each a single word; nothing is reshaped.
    let tx_kind = format!("{tx} bare");
    assert_eq!(turn(Some(&format!(r#"[{{"chainId":31337,"content":"{content}","tx":"{tx_kind}"}}]"#))), shape, "white space in a value never fills the member missing beside it");
    // The escaped `\n` decodes to a real line break in the value.
    let two = format!("{content}\\n31337 {tx} bare {content}");
    assert_eq!(turn(Some(&format!(r#"[{{"chainId":31337,"content":"{two}","payloadKind":"bare","tx":"{tx}"}}]"#))), shape, "a line break in a value never makes two anchors of one");
    assert_eq!(turn(Some(&format!(r#"[{{"chainId":31337,"content":"{content}","note":"x","payloadKind":"bare","tx":"{tx}"}}]"#))), shape, "a member added is never dropped");
    assert_eq!(turn(Some(&format!(r#"[{{"chainId":"31337","content":"{content}","payloadKind":"bare","tx":"{tx}"}}]"#))), shape, "a chain id written as text");
    assert_eq!(turn(Some(&format!("[{good},{{\"chainId\":31337}}]"))), shape, "one bad element among good ones");
    let none = Err(Some(app::fault::Known::FieldMissing));
    assert_eq!(turn(Some("[]")), none, "no anchors at all");
    assert_eq!(turn(None), none, "no --anchors");
    assert!(app::door::verbs().iter().any(|(v, fl)| *v == "adopt" && *fl == ["anchors", "attestor", "attestation"]), "the door's table lists it");
}

/// Two nodes on one host with different paths display alike but are distinct places for per-place state,
/// over http and https.
#[test]
fn the_apps_node_channels_are_kept_apart_by_place() {
    for scheme in ["http", "https"] {
        let a = app::chainx::endpoint_at(&format!("{scheme}://node.example/eth/k1")).expect("a channel");
        let b = app::chainx::endpoint_at(&format!("{scheme}://node.example/polygon/k2")).expect("a channel");
        assert_eq!(a.name(), b.name(), "{scheme}: said alike");
        assert_ne!(a.place(), b.place(), "{scheme}: two places");
    }
}

/// `land_numbered` never replaces an existing entry: it uses the plain name if free, otherwise the next free
/// number (whether the name is held by a file, folder, read-only file or numbered files), leaving what was there
/// untouched. An unwritable folder is refused and nothing lands.
#[test]
fn a_named_landing_never_covers_what_is_there() {
    let d = std::env::temp_dir().join(format!("zk-test-land-numbered-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dir");
    let land = |dir: &std::path::Path, stem: &str| app::home::land_numbered(dir, stem, "zikaron", b"new bytes").map(|p| p.file_name().unwrap().to_string_lossy().to_string());
    assert_eq!(land(&d, "free").ok().as_deref(), Some("free.zikaron"), "free: the first name");
    std::fs::write(d.join("file.zikaron"), b"old").expect("a file");
    assert_eq!(land(&d, "file").ok().as_deref(), Some("file-2.zikaron"), "a file holds the name");
    assert_eq!(std::fs::read(d.join("file.zikaron")).expect("kept"), b"old", "untouched");
    std::fs::create_dir_all(d.join("folder.zikaron")).expect("a folder");
    assert_eq!(land(&d, "folder").ok().as_deref(), Some("folder-2.zikaron"), "a folder holds the name");
    let ro = d.join("ro.zikaron");
    std::fs::write(&ro, b"read only").expect("a file");
    let mut p = std::fs::metadata(&ro).expect("meta").permissions();
    p.set_readonly(true);
    std::fs::set_permissions(&ro, p).expect("read-only");
    assert_eq!(land(&d, "ro").ok().as_deref(), Some("ro-2.zikaron"), "a read-only file holds the name");
    assert_eq!(std::fs::read(&ro).expect("kept"), b"read only");
    for n in ["many.zikaron", "many-2.zikaron", "many-3.zikaron"] {
        std::fs::write(d.join(n), n.as_bytes()).expect("numbered");
    }
    assert_eq!(land(&d, "many").ok().as_deref(), Some("many-4.zikaron"), "several taken: the next free");
    assert_eq!(std::fs::read(d.join("many-2.zikaron")).expect("kept"), b"many-2.zikaron");
    // Unix only: a read-only folder bars new files there.
    #[cfg(unix)]
    {
        let shut = d.join("shut");
        std::fs::create_dir_all(&shut).expect("dir");
        let mut p = std::fs::metadata(&shut).expect("meta").permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&shut, p.clone()).expect("read-only folder");
        let refused = land(&shut, "x");
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        std::fs::set_permissions(&shut, p).expect("back");
        assert!(refused.is_err(), "a folder that cannot be written is refused: {refused:?}");
        assert_eq!(std::fs::read_dir(&shut).expect("dir").count(), 0, "nothing landed");
    }
    let mut back = std::fs::metadata(&ro).expect("meta").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    back.set_readonly(false);
    let _ = std::fs::set_permissions(&ro, back);
    let _ = std::fs::remove_dir_all(&d);
}

/// A backup whose same-day name is taken lands under the next number; the existing file is untouched and the
/// settings record the new path.
#[test]
fn a_backup_never_covers_one_of_the_same_name() {
    if super::alone_in(module_path!(), "a_backup_never_covers_one_of_the_same_name") {
        return;
    }
    super::vault_open();
    let out = std::env::temp_dir().join(format!("zk-test-backup-name-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("dir");
    let now = 1_800_000_000;
    let first = out.join(format!("{}.{}", app::backup::file_stem(now), app::backup::EXT));
    std::fs::write(&first, b"someone's earlier backup").expect("one there");
    let made = app::backup::export(&out, "a long backup password", now).expect("the export lands");
    assert_eq!(made.path, out.join(format!("{}-2.{}", app::backup::file_stem(now), app::backup::EXT)));
    assert_eq!(std::fs::read(&first).expect("kept"), b"someone's earlier backup", "untouched");
    assert_eq!(app::machine::read().expect("settings").backup.map(|b| b.path), Some(made.path.display().to_string()), "the record names the new one");
    let _ = std::fs::remove_dir_all(&out);
}

/// Handing a passcode to its task moves it out: every action carrying one hands it once, whole, leaving its own
/// cell empty, so no second copy lingers in memory after a failed or cancelled task. Actions without a passcode
/// hand nothing.
#[test]
fn a_passcode_handed_is_moved_out_and_handed_once() {
    use app::action::{Action, RestoreHow};
    let typed = || app::secret::Secret::from(String::from("q7w8e9r0"));
    let carrying = vec![
        Action::BackupKey { pin: typed(), password: typed(), again: typed(), dir: String::new() },
        Action::DeleteIdentity { id: String::new(), pin: typed() },
        Action::RevealWords { pin: typed() },
        Action::AttestFor { text: String::new(), pin: typed() },
        Action::ExportBackup { pin: typed(), password: typed(), again: typed(), dir: String::new() },
        Action::SetPin { pin: typed(), again: typed() },
        Action::Unlock { pin: typed() },
        Action::ChangePin { old: typed(), pin: typed(), again: typed() },
        Action::Reseal { pin: typed() },
        Action::RecoverWords { words: typed(), pin: typed(), again: typed() },
        Action::RecoverKeystore { path: String::new(), password: typed(), pin: typed(), again: typed() },
        Action::SetPrimary { id: String::new(), pin: typed() },
        Action::RestoreBackup { path: String::new(), password: typed(), how: RestoreHow::Settings { pin: typed() } },
        Action::RestoreBackup { path: String::new(), password: typed(), how: RestoreHow::Locked { pin: typed(), again: typed() } },
    ];
    for mut a in carrying {
        let handed = a.hand_pin().map(|p| p.expose().to_string());
        assert_eq!(handed.as_deref(), Some("q7w8e9r0"), "{}: handed whole", a.name());
        let left = a.pin_asked().or(a.gate_pin()).map(|p| p.expose().len());
        assert_eq!(left, Some(0), "{}: the action's own cell is empty once handed", a.name());
        assert!(a.hand_pin().is_none(), "{}: handing again gives nothing", a.name());
    }
    assert!(Action::Lock.hand_pin().is_none(), "an action with no passcode hands none");
}

/// A node address is displayed without any key in its path, query, fragment or user part; IPv6 keeps its
/// brackets, the default port is dropped, an unparseable address shows only its length, and addresses differing
/// only by key display alike. Applies to `Display`, `Debug` and the endpoint's `Debug`.
#[test]
fn a_node_address_is_said_without_its_key() {
    use app::chainx::{Endpoint, NodeAddr};
    let key = "S3CRETk3y";
    for (written, said) in [
        (format!("https://node.example/v3/{key}"), "https://node.example"),
        (format!("https://node.example:8443/rpc?apikey={key}"), "https://node.example:8443"),
        (format!("https://node.example/rpc#{key}"), "https://node.example"),
        ("http://[::1]:8545/x".to_string(), "http://[::1]:8545"),
        ("https://node.example:443/".to_string(), "https://node.example"),
    ] {
        let a = NodeAddr::new(written.as_str());
        assert_eq!(a.said(), said, "{said}");
        assert_eq!(a.to_string(), said);
        let e = Endpoint::at(1, written.as_str());
        for shown in [format!("{a:?}"), format!("{e:?}")] {
            assert!(!shown.contains(key), "{shown}");
        }
    }
    let user = NodeAddr::new(format!("https://user:{key}@node.example/"));
    assert!(!user.said().contains(key) && !user.said().contains("node.example"), "user part: not an address, said by length: {}", user.said());
    let unread = NodeAddr::new(format!("node.example/{key}"));
    assert_eq!(unread.said(), format!("({} bytes)", format!("node.example/{key}").len()));
    assert_eq!(NodeAddr::new("https://n.example/v3/aaaa").said(), NodeAddr::new("https://n.example/v3/bbbb").said(), "only the key differs: said alike");
}

/// The raw node address (which may carry a key) never leaves the crate: the field is private, and the raw
/// form, the place key (it includes the path) and the endpoint spelling are `pub(crate)`.
#[test]
fn a_node_address_gives_its_written_form_to_nobody_outside() {
    let code = super::code_only(&super::read_src_file("chainx.rs").expect("chainx.rs"));
    assert!(code.contains("pub struct NodeAddr(String);"), "the written form is a private field");
    for crate_only in ["pub(crate) fn for_transport(&self) -> &str", "pub(crate) fn place(&self) -> String", "pub(crate) fn spec(&self) -> String"] {
        assert!(code.contains(crate_only), "{crate_only}");
    }
    let at = code.find("impl NodeAddr {").expect("impl");
    let body = &code[at..at + code[at..].find("\n}\n").expect("end")];
    let public: Vec<&str> = body.lines().map(str::trim).filter(|l| l.starts_with("pub fn")).collect();
    assert_eq!(public, vec!["pub fn new(written: impl Into<String>) -> NodeAddr {", "pub fn said(&self) -> String {"], "only making one and saying it are public");
    assert!(code.contains("pub url: NodeAddr,"), "an endpoint's address is the type, not a string");
}

/// Every keyed document kind gets an on-disk name that differs from its logical name and contains no id; other
/// kinds keep their logical name; a keyed path of the wrong shape is refused rather than left revealing. A
/// keyed kind without a sample here fails the test, so new kinds are covered.
#[test]
fn every_keyed_kind_is_renamed_by_the_one_naming() {
    if super::alone_in(module_path!(), "every_keyed_kind_is_renamed_by_the_one_naming") {
        return;
    }
    use app::home::Slot;
    use app::local::{disk_rel, Doc};
    super::vault_open();
    let nk = app::names::key().expect("the names key");
    let id = "ab".repeat(32);
    let entry = zikaron_store::layout::ENTRY_SUFFIX;
    let sample = |d: Doc| -> Option<String> {
        Some(match d {
            Doc::Entry => format!("{}/{id}{entry}", Slot::Ledger.as_str()),
            Doc::Held => format!("{}/{id}{entry}", Slot::GrantsHeld.as_str()),
            Doc::Verdict => format!("{}/{id}{}", Slot::GrantsHeld.as_str(), app::lastread::VERDICT_SUFFIX),
            Doc::KeptGrant => format!("{}/{}/{id}.{}", Slot::GrantsHeld.as_str(), app::grantfilex::KEPT, zikaron_glue::container::EXT),
            Doc::TermsDoc => format!("{}/{}/{id}/{id}", Slot::Kits.as_str(), app::termsx::ROOM),
            Doc::TermsRecord => format!("{}/{}/grant-{id}.json", Slot::Kits.as_str(), app::termsx::ROOM),
            _ => return None,
        })
    };
    for d in Doc::ALL {
        match (d.keyed(), sample(d)) {
            (true, Some(logical)) => {
                let named = disk_rel(d, &logical, &nk).unwrap_or_else(|f| panic!("{}: {}", d.tag(), f.said()));
                assert!(named != logical && !named.contains(&id), "{}: {named}", d.tag());
                assert!(disk_rel(d, "a/b/c/d/e", &nk).is_err(), "{}: a path not its shape is refused", d.tag());
            }
            (true, None) => panic!("{}: a keyed kind with no sample here", d.tag()),
            (false, _) => assert_eq!(disk_rel(d, "settings/x.json", &nk).ok().as_deref(), Some("settings/x.json"), "{}: kept as is", d.tag()),
        }
    }
}

/// Both the out-of-date name check and the naming function read `Doc::keyed`; neither keeps its own list of kinds.
#[test]
fn which_kinds_are_renamed_is_judged_once() {
    let code = super::code_only(&super::read_src_file("local.rs").expect("local.rs"));
    let body = |head: &str| {
        let at = code.find(head).unwrap_or_else(|| panic!("{head}"));
        code[at..at + code[at..].find("\n}\n").expect("end")].to_string()
    };
    assert!(body("fn names_out_of_date(").contains(".keyed()"), "the out-of-date check reads the kinds table");
    let naming = body("pub fn disk_rel(");
    assert!(naming.contains("doc.keyed()") && !naming.contains("_ => logical.to_string()"), "naming asks the table first and lets no keyed kind through unnamed");
    assert_eq!(code.matches("Doc::Entry | Doc::Held | Doc::Verdict | Doc::KeptGrant | Doc::TermsDoc | Doc::TermsRecord").count(), 1, "the list is spelled once, in `keyed`");
}
