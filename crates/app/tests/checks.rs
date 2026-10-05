//! The shell's self-check suite. Reads the bytes of the current tree: computed now, scanned now, or by
//! starting the real binary as a child process.

use std::path::{Path, PathBuf};
use std::process::Command;

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// One source file of the app by its module file name. A module split into a folder (`x/mod.rs`
/// and its children) reads as one text: `mod.rs` first, then the other files in name order.
fn read_src_file(name: &str) -> std::io::Result<String> {
    let p = src().join(name);
    if p.is_file() {
        return std::fs::read_to_string(&p);
    }
    let dir = p.with_extension("");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|x| x.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort_by_key(|f| (f.file_name().and_then(|n| n.to_str()) != Some("mod.rs"), f.clone()));
    let mut out = String::new();
    for f in files {
        out.push_str(&std::fs::read_to_string(&f)?);
        out.push('\n');
    }
    Ok(out)
}

/// What ships: each module of the lib (a folder module as one text, named `<folder>.rs`) and the
/// window binary.
fn shipped() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(src()).expect("src/ reads") {
        let p = e.expect("a directory entry").path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push((name, std::fs::read_to_string(&p).expect("a source file reads")));
        } else if p.is_dir() && p.join("mod.rs").is_file() {
            let file = format!("{name}.rs");
            let text = read_src_file(&file).expect("a module folder reads");
            out.push((file, text));
        }
    }
    out.push(("bin/app.rs".into(), read_src_file("bin/app.rs").expect("bin/app.rs reads")));
    out.sort();
    out
}

/// The vault these tests seal local data under (local data is sealed with a key from the vault's master key,
/// so reading or writing it needs the vault open): places set once for this test process (a temporary machine
/// directory, the product's own account name; the real machine directory is never touched), a passcode set, the
/// vault open.
fn vault_open() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let machine = std::env::temp_dir().join(format!("zk-checks-machine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&machine);
        // The directory lives as long as this test process: removed when the process exits, so no run leaves it.
        extern "C" fn tidy() {
            let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("zk-checks-machine-{}", std::process::id())));
        }
        extern "C" {
            fn atexit(f: extern "C" fn()) -> i32;
        }
        unsafe {
            atexit(tidy);
        }
        app::places::set(app::places::Places { key_account: app::places::ACCOUNT.to_string(), machine_dir: Some(machine), user_home: None });
        app::keybox::set_pin("27618394").expect("开得了这一趟的库");
    });
}

fn code_only(s: &str) -> String {
    s.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ───────────────────── Design lives in the widget library; the app keeps no private copy
// ─────────────────────

#[test]
fn the_app_keeps_no_private_copy_of_a_settled_constant() {
    for (name, text) in shipped() {
        let code = code_only(&text);
        for needle in ["Color32::from_rgb", "Color32::from_rgba", "CornerRadius::", "FontId::new"] {
            assert!(
                !code.contains(needle),
                "{name} contains {needle}: design values live in the kit crate and the app keeps no private copy"
            );
        }
    }
}

// ───────────────────── Third-party crates enter in one place only ─────────────────────

#[test]
fn the_egui_family_touches_the_shipped_app_in_one_module_only() {
    for (name, text) in shipped() {
        let code = code_only(&text);
        let touches = code.contains("eframe") || code.contains("egui::") || code.contains("use zikaron_ui::egui");
        if name == "window.rs" {
            assert!(touches, "window.rs 本该是那一处;它不碰 egui 就说明窗子搬家了");
        } else {
            assert!(!touches, "{name} 碰了 egui 一族:出货的这一份里只许 window.rs 碰");
        }
    }
}

// ───────────────────── No test-driver code ships ─────────────────────

#[test]
fn no_test_driver_code_ships_in_the_app() {
    let toml = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("the manifest reads");
    assert!(!toml.contains("app-drive") && !toml.contains("doorword"), "the app manifest names no test-driver binary and no test-driver dependency");
    // No test-driver literal appears in the shipped source.
    for (name, text) in shipped() {
        for needle in ["H0_PALETTE", "H1_BOOT", "emit-plan"] {
            assert!(!text.contains(needle), "{name} carries the test-driver literal {needle}");
        }
    }
}

#[test]
fn the_window_binary_refuses_command_line_arguments() {
    let out = Command::new(env!("CARGO_BIN_EXE_app"))
        .arg("palette")
        .output()
        .expect("起不动窗子那一枚");
    assert_eq!(out.status.code(), Some(2), "递参数进来该得一句具名的拒绝");
    assert!(out.stdout.is_empty(), "拒绝的时候 stdout 一个字节也不写");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.starts_with("E_ARGS "), "首行是理由与主语,现读:{said}");
}

// ───────────────────── The UI frame never blocks ─────────────────────

#[test]
fn the_frame_path_takes_what_is_there_and_never_waits() {
    let text = read_src_file("task.rs").expect("读不出 task.rs");
    let code = code_only(&text);
    for needle in [".recv()", "recv_timeout", "recv_deadline"] {
        assert!(!code.contains(needle), "task.rs 里有 {needle}:收信只许 try_recv");
    }
    // `join` may live only in shutdown: joining a handle in the frame is waiting.
    let shutdown = code.split("pub fn shutdown").nth(1).expect("没有 shutdown");
    let joins_total = code.matches(".join()").count();
    let joins_in_shutdown = shutdown.matches(".join()").count();
    assert_eq!(joins_total, joins_in_shutdown, "join 只许住 shutdown");
    assert_eq!(joins_in_shutdown, 1, "关门恰一处 join");
}

#[test]
fn exactly_one_place_starts_a_thread() {
    let mut n = 0;
    for (_, text) in shipped() {
        n += code_only(&text).matches("thread::spawn").count();
    }
    assert_eq!(n, 1, "起线程只许一处(它在单飞那一问之后)");
}

#[test]
fn nothing_reaches_the_task_pool_except_through_apply() {
    for (name, text) in shipped() {
        if name == "action.rs" || name == "task.rs" {
            continue;
        }
        assert!(
            !code_only(&text).contains("tasks.spawn("),
            "{name} 绕过了 action::apply:动作只有一处正主"
        );
    }
}

// ───────────────────── Trace channel ─────────────────────

#[test]
fn the_trace_channel_is_never_compiled_out() {
    let text = read_src_file("trace.rs").expect("读不出 trace.rs");
    assert!(
        !code_only(&text).contains("#[cfg("),
        "痕迹通道里有 cfg:发布构建照开,没有哪个开关能把它编掉"
    );
}

#[test]
fn a_mark_carries_a_feature_id_and_nothing_else() {
    let text = read_src_file("trace.rs").expect("读不出 trace.rs");
    assert!(
        code_only(&text).contains("pub fn mark(f: Feature)"),
        "记号只收闭型的功能号,收不下一句自由文本"
    );
    for f in app::feature::Feature::ALL {
        let id = f.id();
        assert!(!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric()), "功能号 {id:?} 不成形");
    }
}

// ───────────────────── Three-way errors ─────────────────────

#[test]
fn every_fault_carries_its_evidence_tail() {
    use app::fault::{classify, Class, Fault, Known};
    let a = Fault::known(Known::FileMissing, "原话");
    assert_eq!(a.tail(), "原话");
    let b = Fault::unknown("原话");
    assert_eq!(b.tail(), "原话");
    assert!(b.passthrough(), "未知那一支照原样透传,一个字不加");
    let e = std::io::Error::other("说不清");
    let c = classify(&e, "主语");
    assert_eq!(c.class(), Class::Unknown);
    assert_eq!(c.said(), c.tail(), "不猜:脸上那句就是底下那句");
}

#[test]
fn the_known_table_is_closed_and_each_member_has_a_sentence() {
    use app::fault::{translate, Known};
    let mut said: Vec<&str> = Vec::new();
    for k in Known::ALL {
        let s = translate(k);
        assert!(!s.is_empty(), "{} 没有人话", k.as_str());
        assert!(!said.contains(&s), "两枚已知错误共用一句人话:{s}");
        said.push(s);
    }
}

// ───────────────────── Actions and verbs ─────────────────────

/// Every legal action can point to its equivalent CLI verb (the other half of "the shell decides nothing").
///
/// Passing only non-legal actions would never make `if a.is_legal()` true: the assertion body would never
/// run, and deleting `verb()` entirely would stay green. An assertion that never runs is worse than none; it
/// gives false confidence. So exactly the nine legal actions are passed in, and each is first asserted to be
/// legal.
#[test]
fn every_legal_action_names_the_verb_it_equals() {
    use app::action::Action;
    let legal = [
        Action::MakeAnchorKey,
        Action::Genesis { statement: "s".into() },
        Action::Annotate { subject: "0x00".into(), note_md: "n".into() },
        Action::RecordWork { note_md: "n".into(), files: Vec::new(), for_: None },
        Action::SendBatch { count: 1 },
        Action::DraftGrant {
            draft: Box::new(app::grantx::Draft::default()),
            exclusive: false,
            terms_file: None,
        },
        Action::Revoke { grant: "0x00".into(), case: "c".into() },
        Action::AdoptAnchors { rows: "r".into(), attestor: String::new(), attestation: String::new() },
        Action::Succeed {
            to: "0x00".into(),
            kind: "handover".into(),
            effective: "0".into(),
            statement_md: "s".into(),
        },
    ];
    let mut asked = 0;
    for a in legal {
        assert!(a.is_legal(), "{a:?} 该是法律动作");
        assert!(a.verb().is_some(), "{a:?} is a legal action but names no equivalent CLI verb");
        asked += 1;
    }
    assert_eq!(asked, 9, "这一腿要真的问过九枚,不是一枚也没问");
    // The converse must hold too: moving the landing place and local preferences create no legal fact, so
    // they point to no verb.
    for a in [Action::SelfCheck, Action::Quit] {
        assert!(!a.is_legal(), "{a:?} 不该是法律动作");
    }
}

// ───────────────────── No disk in the frame ─────────────────────


/// No disk in the frame. "The UI frame never blocks" covers more than channels: walking a directory or asking
/// the ledger is a disk read, and reading the disk in the frame is waiting.
///
/// Carried by structure: these happen only in the background pass (`Action::Measure`), nowhere in the window
/// module.
#[test]
fn the_frame_never_touches_the_disk() {
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let code = code_only(&text);
    for needle in ["usage()", ".survey()", "read_dir", "std::fs::", "metadata("] {
        assert!(!code.contains(needle), "window.rs 里有 {needle}:读盘要走后台那一趟");
    }
    // The background pass itself must exist: the home-measuring kind is in the closed table.
    assert!(app::task::Kind::ALL.contains(&app::task::Kind::Archive));
}

// ═════════════════════ Identity and keys ═════════════════════

/// No system keychain interface anywhere in the product.
///
/// The key lives in the local key vault (`keybox`), and no path reads a private key from the keychain. The
/// product has no interface that reads or writes the system keychain, which makes this structural, and the
/// shipped slot `anchor` has no path to be touched. This scans the source of every package under `crates/`
/// file by file: no symbol of the keychain family may appear. The system file dialog has
/// nothing to do with the keychain and lives in the platform interface (`platform/`), one place.
#[test]
fn there_is_no_system_keychain_interface_anywhere() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.file_name().and_then(|x| x.to_str()) == Some("target") {
                continue;
            }
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") && p.components().any(|c| c.as_os_str() == "src") {
                // Scan only each package's source (`src/`); the suite's own pin tables do not count.
                out.push(p);
            }
        }
    }
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("crates 目录").to_path_buf();
    let mut files = Vec::new();
    walk(&crates, &mut files);
    let symbols = ["SecItem", "kSecClass", "kSecAttr", "SecKeychain", "CoreFoundation", "name = \"Security\"", "keychain::"];
    for p in &files {
        let code = code_only(&std::fs::read_to_string(p).expect("读得出源"));
        for sym in symbols {
            assert!(!code.contains(sym), "{} 里还有钥匙串接口那一族:{sym}", p.display());
        }
    }
    assert!(!src().join("keychain.rs").exists(), "钥匙串那一档该整个没了");
    // The phrase book and refusal words never mention the system keychain: the key lives in the local key
    // vault, and the words say so; someone told "no key in the keychain" would search the system keychain for
    // a key that never lived there. The code name column (such as `KEYCHAIN_MISSING`) is a stable identifier
    // and is not covered by this.
    let hits = |t: &str| t.contains("钥匙串") || t.to_lowercase().contains("keychain");
    for (k, zh, en) in app::lang::TABLE.iter() {
        assert!(!hits(zh) && !hits(en), "话册 {k:?} 的人话还说钥匙串");
    }
    for k in app::fault::Known::ALL {
        assert!(!hits(app::fault::translate(k)), "拒因 {} 的人话还说钥匙串", k.as_str());
    }
    // The file dialog stays in one place (one name, one home); FFI lives only in the platform interface.
    let platform = code_only(&read_src_file("platform.rs").expect("平台接口那一处"));
    assert_eq!(platform.matches("pub fn choose_path(").count(), 1, "选档框只此一处");
    for (name, text) in shipped() {
        if name == "platform.rs" {
            continue;
        }
        assert!(!code_only(&text).contains("extern \"C\""), "{name} 里有 FFI");
    }
}

/// The eight things the app needs from the operating system each go through the platform interface. Outside
/// `platform/` the shipped app names no platform crate, reads no `HOME`, no `/etc/localtime`, and no system
/// font directory; the widget library keeps the font rows per system in one table. The two ways the window
/// program stops without a window (misuse, a window that cannot be made) are said through the interface, never
/// written to standard error by hand.
#[test]
fn platform_capabilities_go_through_one_interface() {
    let platform = code_only(&read_src_file("platform.rs").expect("平台接口"));
    for f in ["pub fn choose_path(", "pub fn lock_now(", "pub fn lock_wait(", "pub fn home_dir(", "pub fn zone_rules(", "pub fn user_temp_dir(", "pub fn app_data_dir(", "pub fn say_without_window("] {
        assert_eq!(platform.matches(f).count(), 1, "接口里 {f} 恰一处");
    }
    assert!(platform.contains("compile_error!"), "没有实现的系统在编译时停下");
    let faces = code_only(&std::fs::read_to_string(src().join("window").join("faces.rs")).expect("窗那一处"));
    assert_eq!(faces.matches("crate::platform::say_without_window(").count(), 1, "没有窗时说话只经平台接口一处");
    assert!(!faces.contains("eprintln!"), "窗那一处不自写标准错误");
    let bin = code_only(&std::fs::read_to_string(src().join("bin").join("app.rs")).expect("出货那一枚"));
    assert!(bin.contains("refuse_arguments()") && !bin.contains("eprintln!"), "误用经窗那一处、再经平台接口说");
    for (name, text) in shipped() {
        if name == "platform.rs" {
            continue;
        }
        let code = code_only(&text);
        for w in ["rfd::", "objc_msgSend", "var_os(\"HOME\")", "\"/etc/localtime\"", "/System/Library", "confstr"] {
            assert!(!code.contains(w), "{name} 里有 {w}:平台能力只经 platform 那一处");
        }
    }
    let fonts = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/fonts.rs")).expect("字体那一处"));
    // Three rows of the font table: macOS's and Windows's (Chinese from the system), and every other system's
    // (all embedded); each system is named, never read off as "not Linux".
    assert_eq!(fonts.matches("pub const ROLES:").count(), 3, "字体表三份(macOS、Windows 各一份,其余系统一份)");
    assert!(fonts.contains("#[cfg(target_os = \"macos\")]\npub const ROLES:") && fonts.contains("#[cfg(target_os = \"windows\")]\npub const ROLES:"), "macOS 与 Windows 两份各点名");
    assert!(!fonts.contains("not(target_os = \"linux\")"), "不以「不是 Linux」当 macOS");
}

/// The signing API exposes only two domains (law §5.7).
///
/// Carried by structure: `sign.rs` has only two signing functions, each with its domain fixed in its body; no
/// domain literal is spelled, all come from the core's closed type. So "sign another domain" is not forbidden
/// but impossible: there is no function to call. The law has a closed set of four domains; kit law's two
/// (fpm, ack) are signed by the command line, and this desk has no face for them.
#[test]
fn the_anchor_key_signs_two_faces_and_no_more() {
    use app::sign::{domains, Face};
    // Two faces, two domains, one to one.
    assert_eq!(Face::ALL.len(), 2, "两张脸,不多不少");
    let text = read_src_file("sign.rs").expect("读不出 sign.rs");
    let code = code_only(&text);
    // One face, one function: each face has a function with its domain fixed in its body, and none takes a
    // domain-choosing parameter.
    for want in ["pub fn sign_entry(", "pub fn sign_adoption("] {
        assert!(code.contains(want), "少了 {want}");
    }
    for gone in ["pub fn sign_fpm(", "pub fn sign_ack("] {
        assert!(!code.contains(gone), "撤下的那一面又长回来了:{gone}");
    }
    assert_eq!(
        code.matches("seal(s, preimage,").count(),
        Face::ALL.len(),
        "搅拌那一处的调用点该与脸数一样多:一面一门"
    );
    // The domain cell's type is closed, not `&str`. Passing a domain string in has no form at compile time.
    assert!(
        code.contains("fn seal(s: &Secret, preimage: &[u8], domain: Law)"),
        "seal 的第三格要收闭型"
    );
    assert!(
        !code.contains("domain: &str"),
        "sign.rs 里还有一处收自由域字符串的口"
    );
    // No domain literal spelled: domain spellings may not appear in this file.
    for spelled in ["zikaron/1", "zikaron.fpm", "zikaron.ack", "personal_sign", "0x19", "7702"] {
        assert!(!code.contains(spelled), "sign.rs 里自拼了域字面 {spelled}");
    }
    // The recognized domains are exactly the two members of the core's closed type, faces and domains one to
    // one.
    let want = vec![
        zikaron::tokens::Domain::Entry.as_str(),
        zikaron::tokens::Domain::Adoption.as_str(),
    ];
    assert_eq!(domains(), want);
    let mut ds: Vec<&str> = Face::ALL.iter().map(|f| f.domain()).collect();
    let n = ds.len();
    ds.sort();
    ds.dedup();
    assert_eq!(ds.len(), n, "两张脸的域两两不同");
    // Names outside the faces are not recognized: this is the owner refusing a third name.
    assert!(Face::parse("zikaron/1").is_none(), "域的字面不是一张脸");
    assert!(Face::parse("kit").is_none() && Face::parse("ack").is_none(), "撤下的两张脸认不出");
    for f in Face::ALL {
        assert_eq!(Face::parse(f.as_str()), Some(f), "{} 认不回自己", f.as_str());
    }
}

/// The settings file holds no key material; BIP-39 generates only identity keys.
///
/// Carried by structure: settings has no place for private keys or words and does not touch derivation or
/// seeds; the word list crate and the exit that reads entropy as words each live in one or two places.
#[test]
fn settings_hold_no_key_material_and_words_stay_with_identity() {
    let text = read_src_file("settings.rs").expect("读不出 settings.rs");
    let code = code_only(&text);
    for field in ["privkey", "private_key", "secret", "Secret", "mnemonic"] {
        assert!(!code.contains(field), "设置那一档里出现了 {field}:设置不装密钥材料");
    }
    for w in ["cryptx::", "family::", "identity::", "phrase_of(", ".words()", "Secret::take("] {
        assert!(!code.contains(w), "设置那一档里出现了 {w}:设置不碰派生与种子");
    }
    // BIP-39 generates only identity keys: the word list crate lives only in the cryptx module; the exit
    // reading entropy as words is called only by the identity layer and cryptx itself, and the word copy
    // (`Fresh::words`) is taken only by the identity layer and the window (for display). The window touches
    // no disk (see the no-disk-in-the-frame test), and the identity layer writes only the register (zero key
    // material, checked by its unit tests), so plaintext words have no path to our own files.
    for (name, t) in shipped() {
        let c = code_only(&t);
        if name != "cryptx.rs" {
            for w in ["bip39", "BIP39"] {
                assert!(!c.contains(w), "{name} 里出现了 {w}:词表件只许住 cryptx");
            }
        }
        if c.contains("phrase_of(") {
            assert!(["cryptx.rs", "identity.rs"].contains(&name.as_str()), "{name} 把熵读成了词:只许身份那一层");
        }
        if c.contains(".words()") {
            assert!(["identity.rs", "window.rs"].contains(&name.as_str()), "{name} 拿了明文词:只许身份那一层与显示");
        }
    }
}

/// Showing a raw private key is one-time: the function takes the `Secret` away, and there is nothing left the
/// second time.
#[test]
fn the_bare_key_can_be_revealed_only_once() {
    let text = read_src_file("key.rs").expect("读不出 key.rs");
    assert!(
        code_only(&text).contains("pub fn reveal_once(slot: &mut Option<Secret>)"),
        "签名一改,「一次性」就不再由结构承载"
    );
    let s = app::key::Secret::take([9u8; 32]).expect("在阶内");
    let mut slot = Some(s);
    assert!(app::key::reveal_once(&mut slot).is_some(), "第一次给得出");
    assert!(slot.is_none(), "给过之后那一格空了");
    assert!(app::key::reveal_once(&mut slot).is_none(), "没有第二次");
}

/// No path writes a plaintext private key to disk: no file-writing place takes a `Secret`'s bytes.
#[test]
fn no_path_writes_a_plaintext_key_to_disk() {
    let text = read_src_file("key.rs").expect("读不出 key.rs");
    let code = code_only(&text);
    for w in ["fs::write", "File::create", "land_bytes"] {
        assert!(!code.contains(w), "key.rs 里有落盘的路 {w}:明文只许住内存与系统钥匙串");
    }
    // The plaintext accessor is private to its module: nowhere else in the crate can get the thirty-two
    // bytes.
    assert!(code.contains("\n    fn bytes(&self)"), "bytes() 要住 key.rs 自己一处");
    assert!(!code.contains("pub(crate) fn bytes"), "bytes() 不许对 crate 公开");
    assert!(!code.contains("pub fn bytes"), "bytes() 不许对外公开");
    // The exits are exactly these, and none hands out "plaintext that can be written to disk".
    for exit in [
        // The key-lending exit is visible only to the `sign` file (`key` hangs under `sign`, both files in
        // place): other files in the crate cannot even write the call, so "open another signing exit taking
        // free domain strings" does not compile.
        "pub(in crate::sign) fn with_sign_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R)",
        "pub(crate) fn ciphered(&self, key: &[u8; 16], iv: &[u8; 16])",
        // The anchoring exit likewise: lending is visible only to `sign`, and the lending closure runs the
        // send itself.
        "pub(in crate::sign) fn with_tx_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R)",
        "pub fn reveal_once(slot: &mut Option<Secret>)",
    ] {
        assert!(code.contains(exit), "少了这一处出口:{exit}");
    }
    // The path to disk goes only through `cryptx::aes128_ctr`: keystore receives ciphertext.
    let ks = code_only(&read_src_file("keystore.rs").expect("读不出 keystore.rs"));
    assert!(ks.contains("secret.ciphered("), "keystore 要走密文那一口");
    assert!(!ks.contains(".bytes()"), "keystore 里不许再摸明文字节");
    // Each of the two lending exits has exactly one call, and each call's neighbor is its owner.
    for (口, 住处, 邻居) in [
        // Sending uses the whole endpoint table: the key is lent only to the signing step (`send::sign_for`),
        // and submitting the same bytes endpoint by endpoint borrows no key.
        ("with_tx_key(", "sign.rs", "send::sign_for("),
        ("with_sign_key(", "sign.rs", "cryptox::sign_digest("),
    ] {
        let mut sites = 0usize;
        let mut beside = false;
        for name in [
            "action.rs", "window.rs", "keystore.rs", "entryx.rs", "sign.rs", "mirror.rs",
            "firstrun.rs", "key.rs", "ledgerx.rs", "anchorx.rs", "queue.rs", "auditx.rs",
        ] {
            let t = code_only(&read_src_file(name).expect("读不出源"));
            // The two in `key.rs` are definitions (`fn with_… <R>(&self`), not calls: subtract them when
            // counting calls.
            let defs = t.matches(&format!("fn {}", 口.trim_end_matches('('))).count();
            let n = t.matches(口).count().saturating_sub(defs);
            sites += n;
            if n > 0 {
                assert_eq!(name, 住处, "{口} 跑到 {name} 去了");
                beside = t.contains(邻居);
            }
        }
        assert_eq!(sites, 1, "{口} 恰一处调用,现读 {sites}");
        assert!(beside, "{口} 那一处的邻居该是 {邻居}");
    }
}

// ═════════════════════ Archive and single writer ═════════════════════

/// The product builds its own preconditions: the four subdirectories are laid out by `lay`, not set up by
/// hand.
#[test]
fn the_home_lays_its_own_four_rooms() {
    let dir = std::env::temp_dir().join(format!("zk-test-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let h = app::home::Home::open_or_create(&dir).expect("建家");
    assert!(h.missing().is_empty(), "缺:{:?}", h.missing());
    for s in app::home::Slot::ALL {
        assert!(h.dir(s).is_dir(), "{} 没铺出来", s.as_str());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The lock comes from the kernel, and the product has not one line that deletes lock files.
///
/// This is the structural support for "no deadlock after a kill": there is no stale-lock judgment because
/// stale locks do not exist.
#[test]
fn the_lock_is_the_kernels_and_nothing_deletes_it() {
    let text = read_src_file("lock.rs").expect("读不出 lock.rs");
    let code = code_only(&text);
    assert!(code.contains("crate::platform::lock_now("), "锁要向内核要(经平台接口)");
    assert!(code_only(&read_src_file("platform.rs").expect("平台接口")).contains("fn flock("), "平台那一处向内核要 flock");
    for w in ["remove_file", "unlink", "stale", "陈旧", "pid ==", "SystemTime"] {
        assert!(!code.contains(w), "lock.rs 里有 {w}:陈旧锁的判词一旦出现,就是那条渐近线");
    }
    // In the shipped build, nobody but lock.rs itself may touch the lock file.
    for (name, t) in shipped() {
        if name == "lock.rs" {
            continue;
        }
        assert!(
            !code_only(&t).contains("LOCK_FILE"),
            "{name} 动了锁档的名字:锁档只归 lock.rs"
        );
    }
}

/// Any copy is equivalent: the home does not record where the home itself is.
///
/// What breaks equivalence is writing the home's own path into the home; the mirror record holds a location
/// the person chose (settings content, which may contain a path). So this test asks only about that, and also
/// asks that apart from the mirror record there is no path anywhere.
#[test]
fn nothing_inside_the_home_records_where_the_home_is() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-test-copy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let h = app::home::Home::open_or_create(&dir).expect("建家");
    let mut s = app::settings::Settings::default();
    s.cap_bytes = 4321;
    s.registry = app::key::Address::parse("0x4444444444444444444444444444444444444444");
    s.mirror = Some(app::settings::MirrorRecord {
        path: "/somewhere/the/user/picked".into(),
        at: 1_700_000_000,
    });
    // The three tables must have content too: endpoints, remembered addresses and exclusive flags all live in
    // tables.
    s.endpoints = vec!["31337=http://127.0.0.1:8545".to_string()];
    s.book = vec!["0x1111111111111111111111111111111111111111".to_string()];
    s.exclusive = vec!["0x22".to_string()];
    s.write(&h).expect("写设置");
    // Settings are sealed local data: the document is what the one sealing entry point opens.
    let at = h.dir(app::home::Slot::Settings).join(app::settings::FILE);
    let bytes = app::local::read(&at, app::local::Doc::Settings).expect("读设置").expect("设置在");
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(
        !text.contains(&dir.display().to_string()),
        "设置档里写了它自己在哪:{text}"
    );
    // The check is the statement itself: no string cell in this file carries this home's path.
    //
    // Asking "is there a string with a slash" and exempting a `path` by field name would also exempt `repo`,
    // whose serialized key is also `path`; and walking only objects would never look at strings in the
    // `endpoints`, `book` and `exclusive` tables. Neither names nor positions decide, only the statement, so
    // this walks the whole document (arrays included).
    let v = zikaron::json::parse(&bytes).expect("设置是 JSON");
    let mine = dir.display().to_string();
    let mut seen = 0usize;
    let mut hits: Vec<String> = Vec::new();
    walk_strings(&v, &mut |_key: &str, val: &str| {
        seen += 1;
        if val.contains(&mine) {
            hits.push(val.to_string());
        }
    });
    assert!(hits.is_empty(), "设置档里有一格记着这处家在哪:{hits:?}");
    assert!(seen >= 4, "走档那一步只看了 {seen} 格,数组里的串没走到");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Walk every string cell in a JSON value, entering both objects and arrays.
///
/// If the array branch fell into a catch-all that does nothing for `Arr`, strings in tables would never be
/// seen, and assertions built on the walk would be empty.
fn walk_strings(v: &zikaron::json::Value, f: &mut impl FnMut(&str, &str)) {
    match v {
        zikaron::json::Value::Obj(m) => {
            for (k, x) in m {
                match x {
                    zikaron::json::Value::Str(s) => f(k, s),
                    other => walk_strings(other, f),
                }
            }
        }
        zikaron::json::Value::Arr(xs) => {
            for x in xs {
                match x {
                    zikaron::json::Value::Str(s) => f("", s),
                    other => walk_strings(other, f),
                }
            }
        }
        _ => {}
    }
}

/// No point on the first-run checklist asks about a directory the product never creates.
///
/// Asking about a directory the product never creates would make that point red forever, with an action
/// sentence pointing to no path that turns it green: a light that never turns on only poses as a check.
///
/// Carried by structure: (1) no such function exists; (2) computing the checklist and translating the mirror
/// slot touch no file system, so they cannot ask about any directory; (3) every directory name joined onto a
/// path in this source is among those the product creates.
#[test]
fn no_point_on_the_checklist_asks_about_a_directory_the_product_never_makes() {
    let mirror = read_src_file("mirror.rs").expect("读不出 mirror.rs");
    assert!(
        !code_only(&mirror).contains("pub fn slot("),
        "那一处问从不建的目录的函数该没有了"
    );
    let window = read_src_file("window.rs").expect("读不出 window.rs");
    for name in ["fn checklist(", "fn backup_point("] {
        let body = window
            .split(name)
            .nth(1)
            .unwrap_or_else(|| panic!("没有 {name}"))
            .split("\n    fn ")
            .next()
            .expect("函数体");
        for w in ["join(", "is_dir", "is_file", "exists(", "read_dir", "std::fs::"] {
            assert!(!body.contains(w), "{name} 里有 {w}:这两处一处盘也不许碰");
        }
    }
    // Directory names joined onto paths are among those the product creates.
    let allowed: Vec<String> = app::home::Slot::ALL
        .iter()
        .map(|s| s.as_str().to_string())
        .chain(
            [
                app::home::APP_DIR,
                app::home::POINTER,
                app::home::DEFAULT_HOME,
                app::settings::FILE,
                app::lock::LOCK_FILE,
                app::mirror::MANIFEST,
                app::mirror::ENTRIES,
                "Library",
                "Application Support",
            ]
            .iter()
            .map(|x| x.to_string()),
        )
        .collect();
    // Scan only the path-building files: `join` elsewhere joins strings into a sentence (`Vec::join`), not
    // paths.
    for name in ["home.rs", "mirror.rs", "firstrun.rs", "settings.rs", "lock.rs", "keystore.rs"] {
        let text = read_src_file(name).expect("读不出源");
        for piece in code_only(&text).split(".join(\"").skip(1) {
            let lit = piece.split('"').next().unwrap_or("");
            // A path name contains at least one letter or digit; pure whitespace or punctuation ones are
            // separators for joining strings (`Vec::join`), not paths. This cut is by the literal's shape,
            // not by guessing the caller.
            if !lit.chars().any(|c| c.is_alphanumeric()) {
                continue;
            }
            assert!(
                allowed.contains(&lit.to_string()),
                "{name} 把 {lit:?} 接到了路上,而产品不建它"
            );
        }
    }
}

/// Every legal action points to its equivalent CLI verb, and that name really is in the verb table.
#[test]
fn every_legal_action_names_a_verb_that_the_contract_table_carries() {
    use app::action::Action;
    use app::shell::Page;
    let schema = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CLI-SCHEMA.md"),
    )
    .expect("cannot read CLI-SCHEMA.md");
    let all = [
        Action::Show(Page::FirstRun),
        Action::SelfCheck,
        Action::Quit,
        Action::MakeAnchorKey,
        Action::SwitchRole,
        Action::OpenHome { root: "x".into() },
        Action::MigrateHome { to: "x".into() },
        Action::SetCap { bytes: 1 },
    ];
    let mut legal = 0;
    for a in &all {
        if a.is_legal() {
            legal += 1;
            let v = a.verb().unwrap_or_else(|| panic!("{a:?} 是法律动作却指不出动词"));
            assert!(
                schema.contains(&format!("| `{v}` |")),
                "{a:?} 指的动词 {v} 不在契约 6 的动词表里"
            );
        }
    }
    assert!(legal >= 1, "这一批该有法律动作了,现读 {legal} 条");
}

/// An empty locked-out vault has a way out, and the button for it asks the same question as the action.
///
/// The problem: a machine with a passcode set but no identity yet locks after five wrong attempts, while the
/// vault holds no recovery seal and no key slot, so neither gate path (recovery words, key file) can reopen
/// it, and the person has no way back. The design is one closed question, "does the vault hold anything to
/// lose" (`keybox::recoverable`): whether the gate card shows "reset key vault" asks it, and whether
/// `reset_empty` refuses asks it too. Both places ask the same question; otherwise the rule for one thing
/// would be scattered and written separately, giving "the button shows, and pressing it is refused".
#[test]
fn the_way_out_of_an_empty_locked_box_and_the_key_that_offers_it_ask_one_question() {
    let keybox = read_src_file("keybox.rs").expect("读不出 keybox.rs");
    let kcode = code_only(&keybox);
    // The refusal asks the question itself, not recounting slots and seals.
    assert!(
        kcode.contains("pub fn reset_empty()") && kcode.contains("if recoverable()?"),
        "`reset_empty` 要问 `recoverable()`,不许自己再拼一遍条件"
    );
    // It deletes the whole file, and always asks first.
    let body = kcode.split("pub fn reset_empty()").nth(1).unwrap_or("");
    let asked = body.find("recoverable()");
    let removed = body.find("remove_file");
    assert!(asked.is_some() && removed.is_some() && asked < removed, "`reset_empty` 要先问再删");
    // The window button's condition asks the same place (the shell cell is read by `reread_vault` and
    // `recoverable` in the same pass).
    let win = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert_eq!(
        win.matches("!self.shell.vault_recoverable").count(),
        1,
        "门上那一枚「重置密钥库」的条件该只有一处"
    );
    assert!(win.contains("let empty_out = locked_out && !self.shell.vault_recoverable;"), "那一枚键的条件写法变了");
    assert!(win.contains("Action::ResetEmptyKeybox"), "门上要有那一枚键");
    let shell_src = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert!(
        shell_src.contains("crate::keybox::recoverable().unwrap_or(true)"),
        "壳上那一格要现读 `recoverable`,读不成即当作「还有东西」(宁可不出那一枚键)"
    );
    // That action needs no key: with the vault locked, every key-needing action fails its gate, and this one
    // must get through.
    assert!(!app::action::Action::ResetEmptyKeybox.needs_key(), "重置空库那一条不该要钥");
}

/// Whether the gate is on screen is answered by the vault's four-state closed table, one owner.
///
/// Asking "is it open" (`!unlocked()`) while the gate draws only in the two locked states would leave "no
/// vault yet", neither open nor one of those two states, on a brand-new machine with the rail present, the
/// page blank, and neither wizard nor gate: the person could not get through first run. The rule is
/// gathered into `State::gate_up`, and each of the four states is checked directly, not the frame.
#[test]
fn whether_the_gate_covers_the_window_is_one_closed_table_over_the_four_vault_states() {
    use app::keybox::State;
    let all = [State::Absent, State::Locked { wrong: 0 }, State::Locked { wrong: 4 }, State::LockedOut, State::Open];
    for s in all {
        // Both questions are closed, each member answered now.
        let gate = s.gate_up();
        let keys = s.keys_ready();
        let want_gate = matches!(s, State::Locked { .. } | State::LockedOut);
        let want_keys = matches!(s, State::Open);
        assert_eq!(gate, want_gate, "{s:?}:门在不在答错了");
        assert_eq!(keys, want_keys, "{s:?}:钥拿不拿得到答错了");
        // With no vault the shell still draws: the wizard must be able to stand so the person can set a
        // passcode.
        if matches!(s, State::Absent) {
            assert!(!gate, "还没有库时门不该摆在屏上(否则首启走不出去)");
            assert!(!keys, "还没有库时钥也拿不到");
        }
    }
    // Every place in the window asking "is the gate up" asks here, not assembling its own condition (scanning
    // the source). Six places: whether the shell draws, whether the wizard shows, whether cards show,
    // whether the gate draws itself, whether "key vault locked" toasts, whether the backup sheets float above
    // the gate (restoring from the locked card).
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let code = code_only(&text);
    assert_eq!(
        code.matches("vault.gate_up()").count() + code.matches("state.gate_up()").count(),
        6,
        "问「门在不在」的该是六处,都过 `State::gate_up`(现读到的不是六处)"
    );
    // No other spelling of the same question is allowed elsewhere: using "is it open" as the draw condition
    // (the first-run form), and assembling the gate question from `matches!` on the four states.
    assert!(
        !code.contains("if !self.shell.unlocked() {"),
        "the window decides whether to draw from 'unlocked or not' again (the cause of the first-run bug)"
    );
    for wrong in [
        "State::Absent | crate::keybox::State::Open",
        "State::Locked { .. } | crate::keybox::State::LockedOut",
    ] {
        assert!(!code.contains(wrong), "窗里又自己拼门那一问了:{wrong}");
    }
    // Whether keys are available has one owner: the shell's exit answers the same as `State::keys_ready` in
    // every state (compared by behavior, not by reading source).
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    for s in all {
        shell.vault = app::shell::Vault::Read(s);
        assert_eq!(shell.unlocked(), s.keys_ready(), "{s:?}:`Shell::unlocked` 与 `State::keys_ready` 答得不一样");
    }
    // A damaged store (the shell's fifth member): the gate is up and no key is ready.
    shell.vault = app::shell::Vault::Damaged(app::fault::Fault::known(app::fault::Known::KeyboxShape, String::new()));
    assert!(shell.vault.gate_up() && !shell.unlocked(), "库坏了:门在、钥不可得");
}

/// The shipped build has no path to change the anchor key location.
///
/// No environment variable may choose it: one variable could quietly swap the key the user signs with. The
/// statement setting places is only for test hooks, and the window binary never calls it.
#[test]
fn the_shipped_binary_cannot_move_where_the_anchor_key_lives() {
    for (name, text) in shipped() {
        let code = code_only(&text);
        assert!(
            !code.contains("places::set"),
            "{name} calls places::set: only test code may set the landing places"
        );
        for gone in ["ZIKARON_KEY_ACCOUNT", "ZIKARON_MACHINE_DIR"] {
            assert!(!text.contains(gone), "{name} 里还留着环境变量 {gone}");
        }
        // The KDF level likewise. The light level (`n=2`) is a convenience for tests; in the shipped build it
        // would make an offline brute force of an eight-digit passcode a matter of seconds. The statement
        // setting it lives with the statement setting places: in test hooks, not in the window.
        // The vault's level lives in `keybox.rs`, so only its definition may appear in that file (counted
        // now).
        if name == "keybox.rs" {
            assert_eq!(
                code.matches("set_light_kdf").count(),
                1,
                "keybox.rs 里 set_light_kdf 只许有定义那一处"
            );
        } else {
            assert!(
                !code.contains("set_light_kdf"),
                "{name} calls set_light_kdf: only test code may select the light KDF setting"
            );
        }
    }
    // When never set, the product's set runs.
    assert_eq!(app::places::key_account(), app::places::ACCOUNT);
    // With the KDF level in the vault file, this test guards one more thing: the vault file's self-description
    // is written by the creating pass's `params()`, and the shipped build never sets the light level, so the
    // vaults it creates are always standard. Conversely, the section read back is checked against bounds, so
    // a tampered file can only use KDF levels within bounds, never one demanding terabytes of memory.
    let keybox = read_src_file("keybox.rs").expect("读不出 keybox.rs");
    let code = code_only(&keybox);
    assert!(code.contains("book.kdf = Some(params())"), "建库那一趟该把这一趟的取式档写进库档");
    assert!(code.contains("crate::keystore::in_range(n, r, pp)"), "库档读回来那一节要现验界");
    assert!(code.contains("fn kdf_of(b: &Book)"), "开档那一侧该有「照这一本库那一档」的一处正主");
    assert_eq!(app::keybox::params(), app::keystore::Params::standard(), "没摆过轻档时跑的就是标准档");
}

// ═════════════════════ Bilingual base ═════════════════════

/// Key set equality is carried by construction, and the machine checks what remains: each key has exactly one
/// row, neither language is empty, and no two keys share the same words in one language.
#[test]
fn the_two_languages_carry_the_same_key_set() {
    let bad = app::lang::trouble();
    assert!(bad.is_empty(), "键表不齐:{bad:?}");
    assert_eq!(app::lang::TABLE.len(), app::lang::Key::ALL.len());
    assert_eq!(app::lang::Lang::ALL.len(), 2);
    // Both sides really answer, and switching language really switches.
    for l in app::lang::Lang::ALL {
        app::lang::set(l);
        assert_eq!(app::lang::lang(), l);
        for k in app::lang::Key::ALL {
            assert!(!app::lang::t(k).is_empty(), "{k:?} 在 {} 下是空的", l.as_str());
        }
    }
    app::lang::set(app::lang::Lang::Zh);
}

/// The interface has no hard-coded sentence. "A new visible surface is done only when both languages are in"
/// is guarded by this: any CJK character in the window files means someone bypassed the key table.
#[test]
fn the_window_carries_no_sentence_of_its_own() {
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    for (i, l) in text.lines().enumerate() {
        if l.trim_start().starts_with("//") {
            continue;
        }
        // Only characters in literals: Chinese in code can only be in comments or literals, and comments are
        // skipped above.
        let mut in_str = false;
        let mut prev = ' ';
        for c in l.chars() {
            if c == '"' && prev != '\\' {
                in_str = !in_str;
            }
            if in_str && ('\u{4e00}'..='\u{9fff}').contains(&c) {
                panic!("window.rs:{} 里有写死的话:{}", i + 1, l.trim());
            }
            prev = c;
        }
    }
}

/// A sentence never reaches the screen with a slot left open: wherever the shipped code fills a sentence
/// named by its key with `fillN`, every `{k}` in both languages is below N, and a sentence taken whole with
/// `t` has no slot at all. A raw `{1}` on screen reads as a broken app.
#[test]
fn every_sentence_is_filled_with_as_many_values_as_it_names() {
    use app::lang::TABLE;
    let highest = |s: &str| -> Option<usize> {
        let mut top = None;
        let mut rest = s;
        while let Some(i) = rest.find('{') {
            let after = &rest[i + 1..];
            if let Some(j) = after.find('}') {
                if let Ok(n) = after[..j].parse::<usize>() {
                    top = Some(top.map_or(n, |t: usize| t.max(n)));
                }
            }
            rest = after;
        }
        top
    };
    let slots = |key: &str| -> Option<usize> {
        TABLE.iter().find(|(k, _, _)| format!("{k:?}") == key).map(|(_, zh, en)| highest(zh).max(highest(en)).map_or(0, |n| n + 1))
    };
    let ident = |s: &str| -> String { s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect() };
    let mut bad = Vec::new();
    for (file, text) in shipped() {
        let code = code_only(&text);
        for (given, head) in [(1usize, "fill1("), (2, "fill2("), (3, "fill3("), (0, "t(")] {
            for (at, _) in code.match_indices(head) {
                let before = code[..at].chars().next_back();
                if before.map(|c| c.is_ascii_alphanumeric() || c == '_').unwrap_or(false) {
                    continue;
                }
                let tail = code[at + head.len()..].trim_start();
                let Some(named) = tail.strip_prefix("Key::") else { continue };
                let key = ident(named);
                if head == "t(" && !named[key.len()..].trim_start().starts_with(')') {
                    continue;
                }
                if let Some(n) = slots(&key) {
                    if n > given {
                        bad.push(format!("{file}: {key} has {n} slots, filled with {given}"));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "sentences shown with open slots: {bad:?}");
}

/// One path for taking words: the window may have no second way to get sentences.
#[test]
fn there_is_one_way_to_fetch_a_sentence() {
    let text = read_src_file("lang.rs").expect("读不出 lang.rs");
    let code = code_only(&text);
    assert_eq!(code.matches("pub fn t(").count(), 1, "取词的路恰一条");
    assert!(code.contains("pub const TABLE"), "一行同时带两种话的那张表该在这里");
}

// ═════════════════════ Whole-machine backup and restore ═════════════════════

/// All or nothing: a restore stages its new vault beside the vault first, then everything else (the new homes
/// in fresh places, every machine file as `.zk-next`), and only then renames the vault, the one moment it takes
/// effect; the staged files settle after it. While the staged vault is on disk nothing took effect, so a cut
/// anywhere before the rename leaves the machine as it was (`local::settle_pending`).
///
/// Carried by the order of four calls in `backup::restore`: `stage_new` (the vault) → `stage_next` (machine
/// files) → `commit_new` (the rename) → `settle_pending` (the staged files take their places).
#[test]
fn a_restore_stages_everything_before_it_lands_anything() {
    let text = read_src_file("backup.rs").expect("读不出 backup.rs");
    // Cut up to the next top-level `pub fn`, so these assertions govern `restore` and nothing below it.
    let tail = text.split("pub fn restore(").nth(1).expect("没有 restore");
    let body = tail.split("\npub fn ").next().expect("函数体");
    let vault_at = body.find("stage_new(").expect("restore 里该先把新库摆在旁边");
    let stage_at = body.find("stage_next(").expect("restore 里该把机器档摆在旁边");
    let commit_at = body.find("commit_new(").expect("restore 里该有库改名那一下");
    let settle_at = body.find("settle_pending(").expect("restore 里该在改名之后落位");
    assert!(vault_at < stage_at, "新库先摆:它在盘上即什么都没生效");
    assert!(stage_at < commit_at, "摆齐了才改名:这一句由次序承载");
    assert!(commit_at < settle_at, "改名之后才落位");
    assert!(!body.contains("keybox::write_book"), "restore 里不许直写库档");
    assert!(!body.contains("write_pointer"), "恢复不改机器指针:无身份的那一处家就地换");
    let settle = read_src_file("local.rs").expect("读不出 local.rs");
    let body = settle.split("pub fn settle_pending(").nth(1).expect("没有 settle_pending");
    assert!(body.contains("let never = crate::keybox::drop_staged()?"), "while a new key store is still staged nothing is settled; that is the only condition checked");
}

/// Releasing the pen has one condition: the core audit's label is COMPLETE. The shell does not lean toward
/// green.
#[test]
fn the_pen_is_granted_only_by_the_cores_own_label() {
    let text = read_src_file("auditx.rs").expect("读不出 auditx.rs");
    let code = code_only(&text);
    assert_eq!(
        code.matches("label == complete()").count(),
        1,
        "接笔那一句该只有一处来路"
    );
    for w in ["GAPS", "UNAVAILABLE", "BROKEN_CHAIN"] {
        assert!(!code.contains(w), "auditx.rs 里出现了 {w}:别的标签一律原样端出去,不在这里分支");
    }
}

/// The single-source flag is given by the endpoint rule, not counted in this layer.
#[test]
fn the_single_source_column_comes_from_the_endpoint_law() {
    let text = read_src_file("chainx.rs").expect("读不出 chainx.rs");
    let code = code_only(&text);
    assert!(code.contains("single_source: r.single_source"), "单源那一栏要照抄端点法给的");
    assert!(!code.contains("sources == 1"), "别在这一层自己判单源");
}

// ═════════════════════ First run and adoption ═════════════════════

/// Zero orphan child processes, because none are started.
///
/// The shipped build has no `Command::new`, so "reap all children on exit" is achieved not by reaping but by
/// having nothing to orphan. Background work is threads in this process, joined one by one in `shutdown`.
#[test]
fn the_shipped_app_starts_no_child_process_at_all() {
    for (name, text) in shipped() {
        let code = code_only(&text);
        for w in ["Command::new", "std::process::Command"] {
            assert!(!code.contains(w), "{name} 里起了子进程:发布件一个也不许起");
        }
    }
}

/// Adoption leaves the foreign directory as it is and lands each entry, verified, as a sealed copy in this home's
/// ledger (local data is sealed; a hard link to a plain foreign file would put plain data inside the home).
/// Nothing in it moves or rewrites a byte of the foreign directory.
#[test]
fn adoption_links_and_never_quietly_copies() {
    let text = read_src_file("firstrun.rs").expect("读不出 firstrun.rs");
    let code = code_only(&text);
    assert!(code.contains("into.ledger()?"), "every adopted entry lands in the archive through the local data module's single entry point");
    assert!(code.contains("Known::NotAdoptable"), "外来目录过不了核即整份具名拒,一条也不落");
    for w in ["fs::copy", "fs::rename", "std::fs::write", "hard_link"] {
        assert!(!code.contains(w), "firstrun.rs 里有 {w}:收编不许搬动外来目录的字节,也不许把明文连进家里");
    }
}

/// Genesis is a legal action, and its equivalent verb is the CLI's `init`.
#[test]
fn genesis_is_a_legal_action_bound_to_the_init_verb() {
    use app::action::Action;
    let a = Action::Genesis { statement: "x".into() };
    assert!(a.is_legal());
    assert_eq!(a.verb(), Some("init"));
}

// ═════════════════════ Law words come from the base ═════════════════════

/// Those two law words come from the core's closed types.
///
/// This test keeps no roster of law words of its own (a second roster would drift apart from the core's and
/// kit crate's `tokens.rs`); it only pins that the owners of these two really are on the base side.
#[test]
fn the_two_law_words_now_come_from_the_core() {
    assert_eq!(app::auditx::complete(), zikaron::tokens::Label::Complete.as_str());
    for (name, needle) in [("auditx.rs", "COMPLETE"), ("entryx.rs", "genesis")] {
        let text = read_src_file(name).expect("读不出源");
        assert!(
            !code_only(&text).contains(&format!("\"{needle}\"")),
            "{name} 里又自己拼了 {needle:?}"
        );
    }
}

/// A laid-out home must really exist on disk. "No error returned" does not count: the rooms must survive
/// being asked again on the spot.
#[test]
fn opening_a_home_is_judged_by_what_is_on_the_disk() {
    // Behavior readings, not source text. The check: whether the rooms exist on disk, as this pass's result
    // states.
    let base = std::env::temp_dir().join(format!("zk-lay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);

    // 1 · Laid out: the four rooms really exist on disk, and this asks the file system, not whether the
    // previous statement reported no error.
    let good = base.join("good");
    let h = app::home::Home::open_or_create(&good).expect("铺得出来");
    for s in app::home::Slot::ALL {
        assert!(h.dir(s).is_dir(), "{} 没落在盘上", s.as_str());
    }
    assert!(h.missing().is_empty(), "缺:{:?}", h.missing());

    // 2 · A room's name is taken by a file: refused by name, the reason being "cannot create", with the
    // room's name in the evidence tail.
    let bad = base.join("bad");
    std::fs::create_dir_all(&bad).expect("建根");
    let taken = app::home::Slot::Kits;
    std::fs::write(bad.join(taken.as_str()), b"not a room").expect("占个名");
    let f = match app::home::Home::open_or_create(&bad) {
        Ok(_) => panic!("一间房的名字被占了,还说铺成了"),
        Err(f) => f,
    };
    assert!(
        f.said().starts_with("CANNOT_LAY"),
        "建不出来要具名拒,现读 {} · {}",
        f.said(),
        f.tail()
    );
    assert!(f.tail().contains(taken.as_str()), "证据尾要说出缺的是哪一间:{}", f.tail());

    // 3 · The home-opening path answers the same refusal (not the first downstream place that happens to read
    // the disk).
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    match app::action::apply(
        &mut shell,
        app::action::Action::OpenHome { root: bad.display().to_string() },
    ) {
        app::action::Applied::Trouble(f) => {
            assert!(f.said().starts_with("CANNOT_LAY"), "开家该报同一枚:{}", f.said())
        }
        other => panic!("铺不出来的一处家开成了:{other:?}"),
    }
    assert!(shell.home.is_none(), "开不成就不许把它记成开着的家");

    // 4 · The three refusals differ pairwise: "cannot create directory", "no home yet" and "a writer already
    // exists" are not one sentence.
    assert_ne!(
        app::fault::translate(app::fault::Known::CannotLay),
        app::fault::translate(app::fault::Known::ReadOnly)
    );
    assert_ne!(
        app::fault::translate(app::fault::Known::NoHome),
        app::fault::translate(app::fault::Known::ReadOnly)
    );
    let _ = std::fs::remove_dir_all(&base);
}

// ═════════════════════ Ledger and anchoring ═════════════════════

/// Universally: every legal action points to its equivalent CLI verb.
///
/// This test reads the verb table in `CLI-SCHEMA.md` §6 now: the name on the face must really be in that
/// table. It walks every constructor of `Action` (one missing from the closed table fails to compile),
/// so it says "every one", not "the ones I remember checking".
#[test]
fn every_legal_action_names_a_verb_from_the_contract_six_table() {
    use app::action::Action;
    use app::anchorx::Source;
    use app::shell::Page;
    let every: Vec<Action> = vec![
        Action::Show(Page::FirstRun),
        Action::SelfCheck,
        Action::Measure,
        Action::Quit,
        Action::MakeAnchorKey,
        Action::SwitchRole,
        Action::OpenHome { root: String::new() },
        Action::MigrateHome { to: String::new() },
        Action::SetCap { bytes: 1 },
        Action::ExportMirror { to: String::new() },
        Action::SetAutoLock { on: true, secs: 900 },
        Action::SetPrimary { id: String::new(), pin: app::secret::Secret::default() },
        Action::ExportBackup { pin: app::secret::Secret::default(), password: app::secret::Secret::default(), again: app::secret::Secret::default(), dir: String::new() },
        Action::PeekBackup { path: String::new(), password: app::secret::Secret::default() },
        Action::RestoreBackup { path: String::new(), password: app::secret::Secret::default(), how: app::action::RestoreHow::FirstRun },
        Action::Resume,
        Action::CatchUp,
        Action::FetchLedger { from: String::new(), password: app::secret::Secret::default() },
        Action::Reconcile,
        Action::ReadChain,
        Action::SetEndpoints { specs: String::new() },
        Action::Genesis { statement: String::new() },
        Action::Adopt { dir: String::new() },
        Action::ReadLedger,
        Action::OpenEntry { id: String::new() },
        Action::Annotate { subject: String::new(), note_md: String::new() },
        Action::Audit,
        Action::SetBasis { chain: String::new(), registry: String::new(), from_block: String::new() },
        Action::SetAuditEvery { secs: 1 },
        Action::TakeContent { source: Source::File, path: String::new() },
        Action::RecordWork { note_md: String::new(), files: Vec::new(), for_: None },
        Action::RegisterRepo { path: String::new() },
        Action::CheckRepo,
        Action::EstimateGas { count: 1 },
        Action::SendBatch { count: 1 },
        Action::TakeDropped { path: String::new() },
        Action::PickKit { from: String::new(), to: String::new(), ids: String::new() },
        Action::ExportKit {
            from: String::new(),
            to: String::new(),
            ids: String::new(),
            attach: String::new(),
            note: String::new(),
            out: String::new(),
        },
        Action::ReadDepth { work: String::new() },
        Action::DraftGrant {
            draft: Box::new(app::grantx::Draft::default()),
            exclusive: false,
            terms_file: None,
        },
        Action::ReadGrants,
        Action::CheckClash { work: String::new(), from: String::new(), to: String::new() },
        Action::WizardTick { step: String::new(), said: String::new() },
        Action::WizardReset,
        Action::Revoke { grant: String::new(), case: String::new() },
        Action::ReadStory { grant: String::new() },
        Action::VerifyAnchors { rows: String::new() },
        Action::ListKeyAnchors { address: String::new() },
        Action::ReadClaim { text: String::new() },
        Action::AttestFor { text: String::new(), pin: app::secret::Secret::default() },
        Action::Cosign { rows: String::new(), attestor: String::new(), attestation: String::new() },
        Action::AdoptAnchors {
            rows: String::new(),
            attestor: String::new(),
            attestation: String::new(),
        },
        Action::LookAtKey { to: String::new() },
        Action::Succeed {
            to: String::new(),
            kind: String::new(),
            effective: String::new(),
            statement_md: String::new(),
        },
        Action::ReadBook { address: String::new(), dir: String::new() },
        Action::RememberAddress { address: String::new() },
        Action::ForgetAddress { address: String::new() },
        Action::ExportGrantFile { id: String::new(), to: String::new() },
        Action::SetPublish { url: String::new() },
        Action::CheckPublished { local: String::new() },
    ];
    // The member count is not pinned as a literal: this table samples the closed type (all legal actions are
    // in it, checked one by one below), no larger than the member count the closed type itself answers
    // (`Action::NAMES`).
    assert!(!every.is_empty() && every.len() <= app::action::Action::NAMES.len(), "样本表比 `Action` 的闭表还多");
    // The verb table is read now from `CLI-SCHEMA.md`, not copied here.
    let schema = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("CLI-SCHEMA.md"),
    )
    .expect("读不出 CLI-SCHEMA.md");
    let verbs: Vec<String> = schema
        .lines()
        .filter_map(|l| l.trim().strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .map(|s| s.to_string())
        .collect();
    assert!(verbs.len() >= 20, "契约 6 的动词表读出来只有 {} 条", verbs.len());
    let mut legal = 0usize;
    for a in &every {
        if a.is_legal() {
            legal += 1;
            let v = a.verb().unwrap_or_else(|| panic!("{a:?} 是法律动作而指不出动词"));
            assert!(verbs.iter().any(|x| x == v), "{a:?} 说的 {v:?} 不在契约 6 的表里");
        }
        if let Some(v) = a.verb() {
            assert!(verbs.iter().any(|x| x == v), "{a:?} 说的 {v:?} 不在契约 6 的表里");
        }
    }
    assert!(legal >= 9, "法律动作只数出 {legal} 条");
    // The three added actions each name their verb.
    assert_eq!(Action::Annotate { subject: String::new(), note_md: String::new() }.verb(), Some("annotate"));
    assert_eq!(
        Action::RecordWork { note_md: String::new(), files: Vec::new(), for_: None }.verb(),
        Some("history")
    );
    assert_eq!(Action::SendBatch { count: 1 }.verb(), Some("anchor"));
    assert_eq!(
        Action::DraftGrant {
            draft: Box::new(app::grantx::Draft::default()),
            exclusive: false,
            terms_file: None,
        }
        .verb(),
        Some("grant")
    );
}

/// Every page can reach the navigation bar, and every page's name is in the bilingual table.
#[test]
fn every_page_has_a_name_in_both_languages_and_an_icon() {
    use app::shell::Page;
    assert_eq!(Page::ALL.len(), 29, "二十九页");
    let mut keys = Vec::new();
    for p in Page::ALL {
        let k = p.key();
        assert!(app::lang::TABLE.iter().any(|(x, _, _)| *x == k), "{p:?} 的名不在表里");
        let _ = p.icon();
        keys.push(format!("{k:?}"));
    }
    let n = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), n, "两页共用一个名");
    assert!(app::lang::trouble().is_empty(), "{:?}", app::lang::trouble());
}

/// The DEFLATE part inflates a real zlib stream.
///
/// git objects are compressed with zlib, and "the content hash git anchors is recomputable byte for byte"
/// depends entirely on this part. The sample is a stream with dynamic Huffman tables and back references
/// (repeated segments plus random segments); the sha256 of the inflated bytes matches byte for byte.
#[test]
fn the_inflater_reads_a_real_zlib_stream() {
    let z = zikaron::hexfmt::decode(&format!("0x{}", ZLIB_SAMPLE)).expect("样本是十六进制");
    let out = app::zlibx::inflate_zlib(&z, 1 << 20).expect("解得开");
    assert_eq!(out.len(), 970, "解出来的长度");
    assert_eq!(
        zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&out)),
        "0x35ccdc6558f3343f59c1268be8e0fae8ce118b07cea54ceb2f75823b68accfb2"
    );
    // Truncated streams and bad headers: nothing is guessed; it fails at once.
    assert!(app::zlibx::inflate_zlib(&z[..z.len() / 2], 1 << 20).is_none(), "截断的流该停");
    assert!(app::zlibx::inflate_zlib(&[0x00, 0x00, 0x00], 1 << 20).is_none(), "坏头该停");
    // The output cap works: exceeding it stops (a compression bomb has nowhere to go).
    assert!(app::zlibx::inflate_zlib(&z, 16).is_none(), "上限该拦住");
}

const ZLIB_SAMPLE: &str = "78daabcacc4e2ccacfd33754a81a650d02d652df5312aa06bb65738575ee5d53aed6bb29675f247f5ab250dc65ca354f9bb9312609fb0c15e432ffdd5af0eec5ce99f531359a33ffae7f3a59d5e65ac87adf5fd745d417acdbfcefa5b27ed72745f9794f261eddc8fd666b98f51fb9fcc94e75a74ffcd30c7d7ab6cfed4edf95ed87ca7cb5a27ccbcad97ec4b64d60f2bab677b183f4cb13a7cf9c34fd76563e5129eb61b0c53a2913065fe35dbc2a59077c1a37eefa6467fdf3ddd7eff3b53d4dd6b77f0de2cedce9cd3b43af7577e8b6a21545c955674bd2fe6ce3e3ebffd892bce1c9a65d9a0526251f52d6647c67f8ba41dbf658da97e87bab744ebd3dab1d18eec8e7fbceebd3667f672e7613f77bc9397c0d3953ab97b55c7396dffaeaba932fe7c358269f088f4fb6f2cbbe9bc9d627f68a1a3de75378b42cadf7f997ba96f4a76e57ed4e3c5aa85a7d3b3bb1a0243f2f335161940164000050688899";

/// The content hash git anchors is recomputable byte for byte.
///
/// The test builds its own repository (packed history plus loose commits on top, so both object stores are
/// read), has the product read HEAD, and has `git` compute independently: the sha256 of
/// `git cat-file commit HEAD`'s bytes, the commit name, the ancestor count and the subject, all four
/// compared. The product side starts no child process (another test in the suite checks that); the
/// independent side starts a real `git`.
#[test]
fn the_git_content_hash_is_what_git_itself_says() {
    let repo = std::env::temp_dir().join(format!("zk-git-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(&repo).expect("建目录");
    // An empty global config of this test's own (a file, on every system), so the user's config plays no part.
    let empty_config = repo.with_extension("gitconfig");
    std::fs::write(&empty_config, b"").expect("空设置档");
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &empty_config)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "")
            .output()
            .expect("起不动 git");
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "-q", "."]);
    for (i, body) in ["one", "two", "three"].iter().enumerate() {
        std::fs::write(repo.join("f.txt"), body.repeat(200)).expect("写");
        git(&["add", "f.txt"]);
        git(&["commit", "-q", "-m", &format!("commit {i}"), "-m", "second paragraph"]);
    }
    // The history so far goes into a pack (with deltas between the versions of f.txt).
    git(&["repack", "-a", "-d", "-q"]);
    for i in 3..5 {
        std::fs::write(repo.join(format!("g{i}.txt")), format!("loose {i}")).expect("写");
        git(&["add", "."]);
        git(&["commit", "-q", "-m", &format!("commit {i}")]);
    }
    let got = app::gitx::head_of(&repo).expect("读得出 HEAD");

    let bytes = Command::new("git")
        .args(["cat-file", "commit", "HEAD"])
        .current_dir(&repo)
        .output()
        .expect("起不动 git");
    assert!(bytes.status.success(), "git cat-file 没成");
    assert_eq!(
        zikaron::hexfmt::encode(&got.content),
        zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&bytes.stdout)),
        "content 与 git cat-file 的字节对不上"
    );
    assert_eq!(got.bytes, bytes.stdout.len(), "提交对象的长度对不上");

    let rev = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo)
        .output()
        .expect("起不动 git");
    assert_eq!(got.commit, String::from_utf8_lossy(&rev.stdout).trim(), "HEAD 指的那一枚对不上");

    let count = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(&repo)
        .output()
        .expect("起不动 git");
    assert_eq!(
        got.ancestors.to_string(),
        String::from_utf8_lossy(&count.stdout).trim(),
        "祖先数对不上"
    );

    let subj = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(&repo)
        .output()
        .expect("起不动 git");
    assert_eq!(got.subject, String::from_utf8_lossy(&subj.stdout).trim(), "提交摘要对不上");
    assert_eq!(got.ancestors, 5, "五枚提交");
    let _ = std::fs::remove_dir_all(&repo);
}

/// A directory manifest is a canonical value: the same tree gives the same string twice, and changing one
/// byte changes it.
#[test]
fn the_directory_manifest_is_canonical_and_sensitive() {
    let dir = std::env::temp_dir().join(format!("zk-man-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("b")).expect("建目录");
    std::fs::write(dir.join("a.txt"), b"one").expect("写");
    std::fs::write(dir.join("b").join("c.txt"), b"two").expect("写");
    let one = app::anchorx::of_dir(&dir).expect("算得出");
    let two = app::anchorx::of_dir(&dir).expect("算得出");
    assert_eq!(one.digest, two.digest, "两趟该同一串字");
    // The manifest passes the core's canonical byte rules: it is a value, not free text.
    let (doc, files, skipped) = app::anchorx::manifest(&dir).expect("摆得出清单");
    assert_eq!((files, skipped), (2, 0));
    let bytes = zikaron::json::canon_bytes(&doc);
    assert_eq!(zikaron::json::canon_bytes(&zikaron::json::parse(&bytes).expect("读得回")), bytes);
    std::fs::write(dir.join("a.txt"), b"ONE").expect("写");
    assert_ne!(app::anchorx::of_dir(&dir).expect("算得出").digest, one.digest, "动一个字节要换一串字");
    // Empty directory: the manifest cannot be computed, refused by name.
    let empty = dir.join("b2");
    std::fs::create_dir_all(&empty).expect("建目录");
    assert!(
        matches!(app::anchorx::of_dir(&empty), Err(f) if f.said().starts_with("DIR_EMPTY")),
        "空目录要具名拒"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The mode mark is always the family literal: `mark` is `bytes-sha256/1`, and `toolchain` is the sha256 of
/// that literal's UTF-8 bytes. A person can neither choose nor fill it.
#[test]
fn the_mode_is_the_family_literal_and_nothing_else() {
    let m = app::anchorx::mode();
    assert_eq!(m.mark, "bytes-sha256/1", "家族字面只此一员");
    assert_eq!(m.mark, app::anchorx::FAMILY);
    assert_eq!(m.toolchain, zikaron::cryptox::sha256(b"bytes-sha256/1"), "toolchain 取字面的 UTF-8 字节 sha256");
    // The body laid out is law §6.2's three cells, judged by the core's thirteen steps: this layer does not
    // check shape itself.
    let body = app::anchorx::history_body(&[7u8; 32], &m, "");
    let zikaron::json::Value::Obj(ms) = &body else { panic!("body 该是一个对象") };
    let names: Vec<&str> = ms.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(names, vec!["content", "mode"], "没有附言的时候恰两格");
    // The literal is assembled in one place: it may not appear in other product files.
    for f in std::fs::read_dir(src()).expect("读得出 src") {
        let p = f.expect("一档").path();
        if p.extension().map(|x| x == "rs").unwrap_or(false) && p.file_name().map(|n| n != "anchorx.rs").unwrap_or(false) {
            let code = code_only(&std::fs::read_to_string(&p).expect("读得出"));
            assert!(!code.contains("bytes-sha256/1"), "{} 里自拼了家族字面", p.display());
        }
    }
}

/// The queue has one way in and two ways out, each recording its reason (a successful receipt goes through
/// `anchored_out`, retraction through `drop_ids`), and what is written to disk is a canonical value.
#[test]
fn the_queue_has_one_way_in_and_one_way_out() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-queue-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("铺得出家");
    let a = format!("0x{}", "11".repeat(32));
    let b = format!("0x{}", "22".repeat(32));
    let mut q = app::queue::Queue::default();
    assert!(q.push(&a, 1).landed(), "第一次排得进去");
    assert!(!q.push(&a, 2).landed(), "同一枚只排一次");
    assert!(q.push(&b, 3).landed());
    assert_eq!(q.len(), 2);
    q.write(&home).expect("落得下");
    let back = app::queue::Queue::read(&home).expect("读得回");
    assert_eq!(back, q, "落盘往返逐条相同");
    assert_eq!(back.take_ids(1), vec![a.clone()], "合批挑的是队头那几枚");
    let mut c = back.clone();
    assert_eq!(c.drop_ids(&[a.clone()]), 1);
    assert!(!c.has(&a) && c.has(&b), "出队只走那一枚");
    // An unrecognized id refuses the whole batch; half is never sent.
    assert!(app::queue::Queue::hashes(&[a.clone(), "0x00".to_string()]).is_err());
    assert_eq!(app::queue::Queue::hashes(&[a, b]).expect("认得出").len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

// ═════════════════════ Grants and first window ═════════════════════

/// The depth reading and the grantee verifier share one implementation (kit crate), so both sides match byte
/// for byte.
///
/// This test computes both sides and compares: one through the product's `depthx::read`, the other calling the
/// kit crate's `reading::depth` directly. Equal canonical bytes mean one implementation; `depthx` computes no
/// number.
#[test]
fn the_depth_reading_comes_from_the_kit_core_itself() {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-depth-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("铺得出家");
    let secret = app::key::Secret::take([0x21u8; 32]).expect("在阶内");
    // A genesis plus one history entry, all through the product's own assembly.
    let g = app::entryx::genesis(&secret, "depth probe").expect("签得出");
    let ledger = home.ledger().expect("账本");
    let name = |id: &str| zikaron_store::EntryName::parse(id.trim_start_matches("0x")).expect("成名");
    ledger.append(&name(&g.id), &g.bytes).expect("落得下");
    let m = app::anchorx::mode();
    let work = [0x5au8; 32];
    let body = app::anchorx::history_body(&work, &m, "");
    let h = app::entryx::seal(&secret, "history", 1, Some(&g.id), body).expect("签得出");
    ledger.append(&name(&h.id), &h.bytes).expect("落得下");

    let pile = ledger.pile().expect("堆");
    let frag = app::auditx::empty_fragment();
    let work_hex = zikaron::hexfmt::encode(&work);
    let mine = app::depthx::read(&pile.items, &frag, &work_hex).expect("算得出");
    let outcome = app::auditx::outcome_of(&pile.items, &frag).expect("核出得来");
    let theirs = zikaron_kit::reading::depth(Some(&outcome), &work_hex);
    assert_eq!(
        zikaron::json::canon_bytes(&mine.value),
        zikaron::json::canon_bytes(&theirs),
        "两侧逐字节一致"
    );
    assert!(mine.found, "这本账里有这一枚记录");

    // This page produces no external report file (as kit law §9): this file has no path to disk.
    let code = code_only(&read_src_file("depthx.rs").expect("读不出 depthx.rs"));
    for w in ["fs::write", "File::create", "land_bytes", "fs::create_dir"] {
        assert!(!code.contains(w), "depthx.rs 里有落盘的路 {w}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The first-window checklist cannot skip steps, and can be left and resumed.
#[test]
fn the_first_window_checklist_refuses_a_skipped_step() {
    vault_open();
    use app::wizard::{Step, Wizard};
    assert_eq!(Step::ALL.len(), 2, "两步,不多不少");
    let mut w = Wizard::default();
    assert_eq!(w.next(), Some(Step::Terms));
    // Skipping a step is refused by name.
    assert!(matches!(
        w.tick(Step::Anchor, ""),
        Err(f) if f.said().starts_with("STEP_SKIPPED")
    ));
    assert_eq!(w.done(), 0, "拒了就一步也没走");
    for s in Step::ALL {
        w.tick(s, s.as_str()).unwrap_or_else(|f| panic!("{} 勾不上:{}", s.as_str(), f.said()));
    }
    assert!(w.next().is_none(), "两步走完");
    // Ticking after both are done is refused too.
    assert!(w.tick(Step::Terms, "").is_err());

    // Round trip to disk: the shape is a canonical value, read back identically.
    let dir = std::env::temp_dir().join(format!("zk-wiz-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("铺得出家");
    w.write(&home).expect("落得下");
    assert_eq!(Wizard::read(&home).expect("读得回"), w, "落盘往返逐条相同");
    // A file whose order was scrambled by hand cannot be read back.
    // The checklist is sealed local data: hand-written shapes go in through the same seal.
    let put = |b: &[u8]| app::local::put(&home.dir(app::home::Slot::Settings), app::wizard::FILE, app::local::Doc::FirstWindow, b).expect("写得下");
    put(br#"{"marks":[{"said":"","step":"anchor"}]}"#);
    assert!(Wizard::read(&home).is_err(), "乱了次序的清单不许被读成一份清单");
    // Lines in older files for removed steps (`bond`, `undertaking`): skipped, and the remaining lines
    // continue in today's order.
    put(br#"{"marks":[{"said":"a","step":"terms"},{"said":"b","step":"bond"},{"said":"c","step":"undertaking"}]}"#);
    let old = Wizard::read(&home).expect("留着撤下那几步的旧档照读得开");
    assert_eq!((old.done(), old.next()), (1, Some(Step::Anchor)), "旧档停在原处");
    assert!(Step::parse("undertaking").is_none(), "撤下的那一步认不出");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The double-sale gate fires only when all three hold: the same record, overlapping windows, and the
/// existing grant carrying the local exclusive flag.
///
/// The third is where the limit lies: terms are a hash, and a machine cannot read exclusivity from them (law
/// §6.3).
#[test]
fn the_double_sale_gate_needs_all_three() {
    use app::grantx::{conflicts, overlaps, Row};
    let row = |exclusive: bool, window: Option<(u64, u64)>| Row {
        id: format!("0x{}", "11".repeat(32)),
        seq: 1,
        grantee: format!("0x{}", "22".repeat(20)),
        work: format!("0x{}", "33".repeat(32)),
        terms: format!("0x{}", "44".repeat(32)),
        window,
        exclusive,
        exclusive_from: if exclusive { app::termsx::Exclusive::Signed } else { app::termsx::Exclusive::No },
        doc: None,
        doc_name: None,
        revoked: false,
    };
    let work = format!("0x{}", "33".repeat(32));
    let flagged = vec![row(true, Some((10, 20)))];
    let bare = vec![row(false, Some((10, 20)))];
    assert_eq!(conflicts(&flagged, &work, Some((15, 25))).len(), 1, "三样齐了即命中");
    assert_eq!(conflicts(&bare, &work, Some((15, 25))).len(), 0, "没有旗即不拦");
    assert_eq!(conflicts(&flagged, &work, Some((30, 40))).len(), 0, "窗口不相交即不拦");
    assert_eq!(
        conflicts(&flagged, &format!("0x{}", "99".repeat(32)), Some((15, 25))).len(),
        0,
        "换一枚记录即不拦"
    );
    let mut revoked = flagged.clone();
    revoked[0].revoked = true;
    assert_eq!(conflicts(&revoked, &work, Some((15, 25))).len(), 0, "已撤的不算");
    // A side without a window counts as "forever".
    assert!(overlaps(None, Some((1, 2))) && overlaps(Some((1, 2)), None));
    assert!(overlaps(Some((10, 20)), Some((20, 30))), "端点相接也算相交");
    assert!(!overlaps(Some((10, 20)), Some((21, 30))));

    // The badge needs a chain time; without it there is "no reading", never a guessed "within the window".
    let r = row(false, Some((10, 20)));
    assert_eq!(r.badge(None), app::grantx::Badge::Unknown);
    assert_eq!(r.badge(Some(15)), app::grantx::Badge::Live);
    assert_eq!(r.badge(Some(99)), app::grantx::Badge::Expired);
    let mut rr = r.clone();
    rr.revoked = true;
    assert_eq!(rr.badge(Some(15)), app::grantx::Badge::Revoked, "撤了压过在窗");
}

// ═════════════════════ From revocation to reading ═════════════════════

/// A revocation must revoke a grant in this ledger, and revoked overrides within-window (law §6.4).
#[test]
fn a_withdrawal_names_a_grant_this_ledger_published() {
    let g = format!("0x{}", "11".repeat(32));
    let c = format!("0x{}", "22".repeat(32));
    let names = |v: &zikaron::json::Value| -> Vec<String> {
        match v {
            zikaron::json::Value::Obj(m) => m.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        }
    };
    assert_eq!(names(&app::grantx::revocation_body(&g, &c).expect("摆得出")), vec!["case", "grant"]);
    assert_eq!(names(&app::grantx::revocation_body(&g, " ").expect("摆得出")), vec!["grant"]);
    assert!(app::grantx::revocation_body(" ", &c).is_err(), "grant 是必填的");

    // Revoked overrides within-window: a time inside the window still reads "revoked".
    let row = app::grantx::Row {
        id: g.clone(),
        seq: 1,
        grantee: format!("0x{}", "22".repeat(20)),
        work: format!("0x{}", "33".repeat(32)),
        terms: c.clone(),
        window: Some((10, 20)),
        exclusive: false,
        exclusive_from: app::termsx::Exclusive::No,
        doc: None,
        doc_name: None,
        revoked: true,
    };
    assert_eq!(row.badge(Some(15)), app::grantx::Badge::Revoked, "撤了压过在窗");
}

/// Co-signature verification cannot be bypassed, and a new head requires a new signature (law §6.6).
#[test]
fn a_cosigning_is_bound_to_the_head_it_was_written_against() {
    let rows = app::adoptx::rows_of(&format!(
        "31337 0x{} bare 0x{}\n",
        "11".repeat(32),
        "77".repeat(32)
    ))
    .expect("读得出");
    let s = app::key::Secret::take([0x31u8; 32]).expect("在阶内");
    let who = s.address().expect("有地址");
    let head = format!("0x{}", "aa".repeat(32));
    let pre = app::adoptx::preimage(&who, &rows, &head);
    let sig = app::sign::sign_adoption(&s, &pre).expect("签得出");
    app::adoptx::cosigned(&who, &rows, &head, &who.hex(), &sig).expect("验得过");
    // Another head: the same signature no longer matches.
    let other = format!("0x{}", "bb".repeat(32));
    assert!(
        matches!(
            app::adoptx::cosigned(&who, &rows, &other, &who.hex(), &sig),
            Err(f) if f.said().starts_with("COSIGN_REFUSED")
        ),
        "换头须重签"
    );
    // The preimage changes with the head and with the adopter (it binds exactly these three).
    assert_ne!(pre, app::adoptx::preimage(&who, &rows, &other));
    assert_ne!(pre, app::adoptx::preimage(&app::key::Address([9u8; 20]), &rows, &head));
    // A failed co-signature does not refuse the entry: a body without those two cells is still a legal body.
    let bare = app::adoptx::adoption_body(&rows, None);
    let with = app::adoptx::adoption_body(&rows, Some((&who.hex(), &sig)));
    let n = |v: &zikaron::json::Value| match v {
        zikaron::json::Value::Obj(m) => m.len(),
        _ => 0,
    };
    assert_eq!((n(&bare), n(&with)), (1, 3), "两格同在或同缺");
}

/// One key, one ledger: after succession this desk becomes read-only (law §7.3, §7.5).
#[test]
fn a_succession_hands_the_desk_over_and_closes_writing() {
    vault_open();
    use app::succeedx::{handed_over, succession_body, KIND_HANDOVER, KIND_ROTATION};
    assert!(succession_body("", KIND_HANDOVER, "1", "x").is_err());
    assert!(succession_body("0xaa", " ", "1", "x").is_err());
    assert!(succession_body("0xaa", KIND_ROTATION, "x", "x").is_err(), "生效时刻要一个数");
    assert!(succession_body("0xaa", KIND_ROTATION, "1", " ").is_err());

    // A real ledger: genesis plus one succession.
    let dir = std::env::temp_dir().join(format!("zk-succ-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("铺得出家");
    let secret = app::key::Secret::take([0x41u8; 32]).expect("在阶内");
    let mine = secret.address().expect("有地址");
    let ledger = home.ledger().expect("账本");
    let name = |id: &str| zikaron_store::EntryName::parse(id.trim_start_matches("0x")).expect("成名");
    let g = app::entryx::genesis(&secret, "succession probe").expect("签得出");
    ledger.append(&name(&g.id), &g.bytes).expect("落得下");
    let to = format!("0x{}", "88".repeat(20));
    let body = succession_body(&to, KIND_HANDOVER, "100", "交给她").expect("摆得出");
    let sc = app::entryx::seal(&secret, "succession", 1, Some(&g.id), body).expect("签得出");
    ledger.append(&name(&sc.id), &sc.bytes).expect("落得下");

    let pile = ledger.pile().expect("堆");
    assert_eq!(handed_over(&pile.items, Some(mine)).as_deref(), Some(to.as_str()));
    // A succession to oneself does not count as handing over.
    assert!(handed_over(&pile.items, app::key::Address::parse(&to)).is_none());
    // Without a local key the question cannot be asked yet.
    assert!(handed_over(&pile.items, None).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

// ═════════════════════ Genesis lands only in an empty ledger ═════════════════════

// ═════════════════════ Shell navigation · Settings ═════════════════════

/// The recorder rail has 6 items and the user rail 4 (delivery merged into the verify page), with groups and
/// item names as designed.
#[test]
fn the_two_seats_carry_the_prototype_rails() {
    use app::lang::{set, t, Lang};
    use app::nav::{count, rail};
    use app::roles::Role;
    set(Lang::Zh);
    assert_eq!(count(Role::Author), 6);
    assert_eq!(count(Role::Grantee), 4);
    let shape = |r: Role| -> Vec<(String, Vec<String>)> {
        rail(r)
            .iter()
            .map(|g| (g.title.map(t).unwrap_or("").to_string(), g.items.iter().map(|i| t(i.name).to_string()).collect()))
            .collect()
    };
    let want_author = vec![("", vec!["首页"]), ("记录", vec!["记录存证", "授权"]), ("查看", vec!["核验", "账本", "提醒"])];
    let want_grantee = vec![("", vec!["首页"]), ("授权", vec!["我的授权"]), ("查看", vec!["核验", "提醒"])];
    let own = |w: Vec<(&str, Vec<&str>)>| -> Vec<(String, Vec<String>)> {
        w.into_iter().map(|(g, i)| (g.to_string(), i.into_iter().map(str::to_string).collect())).collect()
    };
    assert_eq!(shape(Role::Author), own(want_author));
    assert_eq!(shape(Role::Grantee), own(want_grantee));
}

/// Every old page lands somewhere findable in both seats (a view on the rail, a settings section, or home for
/// a view exclusive to the other identity); the landing view always lights one rail item for this identity.
#[test]
fn every_old_page_lands_somewhere_findable_on_both_seats() {
    use app::nav::{home_of, lit, settle, Place};
    use app::roles::Role;
    for role in Role::ALL {
        for p in app::shell::Page::ALL {
            let raw = home_of(p, role);
            assert!(!matches!(raw, Place::Page(_) | Place::Home), "{p:?} 在 {role:?} 上没有译成视图或设置");
            match settle(Place::Page(p), role) {
                Place::Settings(_) | Place::SettingsHome | Place::Home => {}
                place @ Place::View(v, t) => {
                    // The user's relicense and the pending queue are entered from links; the rail lights
                    // nothing.
                    let relicense = v == app::nav::View::Grants && t == app::nav::tab::GRANTS_RELICENSE;
                    let pending = v == app::nav::View::Works && t == app::nav::tab::WORKS_PENDING;
                    assert!(relicense || pending || lit(place, role).is_some(), "{p:?} 在 {role:?} 上落到 {place:?},左栏不亮");
                }
                Place::Page(_) => panic!("{p:?} 落地后还是旧页"),
            }
        }
    }
}

#[test]
fn every_alias_name_resolves_to_a_place() {
    use app::nav::{alias_names, place_named};
    for n in alias_names() {
        assert!(place_named(&n).is_some(), "{n} 认不出");
    }
    assert_eq!(alias_names().len(), 3 + app::nav::View::ALL.len() + app::nav::Section::ALL.len());
    for old in ["Settings.seat", "Settings.chain", "Settings.cadence", "Settings.mirror"] {
        assert!(place_named(old).is_some(), "旧段名 {old} 该照认");
    }
    assert!(place_named("Settings.nowhere").is_none());
}

/// The "language" settings section reads and writes disk: saved to settings, read back after reopening;
/// without a home it is refused by name and the language does not switch.
#[test]
fn the_language_choice_is_written_to_the_home_and_read_back() {
    vault_open();
    use app::action::{apply, Action, Applied};
    use app::lang::{set, Lang};
    let dir = std::env::temp_dir().join(format!("zk-test-lang-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    set(Lang::Zh);
    match apply(&mut shell, Action::SetLang { lang: Lang::En }) {
        Applied::Trouble(f) => assert!(f.said().starts_with("NO_HOME"), "没有家该具名拒:{}", f.said()),
        other => panic!("没有家却换成了:{other:?}"),
    }
    assert_eq!(shell.settings.lang, None, "记不下来就不换");
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    shell.lock = Some(app::lock::take(&home).expect("取锁"));
    shell.home = Some(home);
    match apply(&mut shell, Action::SetLang { lang: Lang::En }) {
        Applied::Spoken(Lang::En) => {}
        other => panic!("该换成英文:{other:?}"),
    }
    let back = app::settings::Settings::read(shell.home.as_ref().unwrap()).expect("读回");
    assert_eq!(back.lang, Some(Lang::En), "盘上那一份记着英文");
    set(Lang::Zh);
    drop(shell);
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Hide entries deleted on this machine" is a per-home setting, off by default, saved to the home and read
/// back; it is display only: the records and ledger pages each leave out exactly the local deletion pair
/// (`Lamp::local`), and nothing else reads it.
#[test]
fn hiding_local_deletions_is_saved_to_the_home_and_only_filters_the_two_lists() {
    vault_open();
    use app::action::{apply, Action, Applied};
    assert!(!app::settings::Settings::default().hide_local_deletions, "默认不隐藏");
    let dir = std::env::temp_dir().join(format!("zk-test-hide-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let home = app::home::Home::open_or_create(&dir).expect("建家");
    shell.lock = Some(app::lock::take(&home).expect("取锁"));
    shell.home = Some(home);
    match apply(&mut shell, Action::SetHideLocalDeletions { on: true }) {
        Applied::HideLocalDeletions(true) => {}
        other => panic!("该存下隐藏:{other:?}"),
    }
    let back = app::settings::Settings::read(shell.home.as_ref().unwrap()).expect("读回");
    assert!(back.hide_local_deletions, "盘上那一份记着隐藏");
    match apply(&mut shell, Action::SetHideLocalDeletions { on: false }) {
        Applied::HideLocalDeletions(false) => {}
        other => panic!("该存下不隐藏:{other:?}"),
    }
    assert!(!app::settings::Settings::read(shell.home.as_ref().unwrap()).expect("读回").hide_local_deletions);
    drop(shell);
    let _ = std::fs::remove_dir_all(&dir);
    for (name, list) in [("works.rs", "let shown: Vec<&WorkLine>"), ("ledger.rs", "let shown: Vec<&crate::ledgerx::Row>")] {
        let text = std::fs::read_to_string(format!("{}/src/window/{name}", env!("CARGO_MANIFEST_DIR"))).expect("读源");
        let at = text.find(list).unwrap_or_else(|| panic!("{name} 没有列表"));
        let head = &text[at.saturating_sub(120)..at + 200];
        assert!(head.contains("settings.hide_local_deletions") && head.contains("lamp.local()"), "{name} 的列表按设置滤掉本机删除那一对");
    }
    let mut readers = 0;
    for entry in std::fs::read_dir(format!("{}/src", env!("CARGO_MANIFEST_DIR"))).unwrap().chain(std::fs::read_dir(format!("{}/src/window", env!("CARGO_MANIFEST_DIR"))).unwrap()) {
        let path = entry.unwrap().path();
        if path.extension().map(|e| e == "rs").unwrap_or(false) {
            readers += std::fs::read_to_string(&path).unwrap().matches("settings.hide_local_deletions").count();
        }
    }
    assert_eq!(readers, 3, "只有两个列表与设置页读它");
}

/// The "delete" reading convention sits on law §6.9's open types: the core fully verifies a `retraction`
/// entry per §4 and lists it under `UNKNOWN_TYPE`, with the same label as when a known type entry is at that
/// position; this desk reads "deleted" by the convention table, and invalid forms each read as invalid
/// without refusing the ledger.
#[test]
fn a_retraction_is_an_open_type_entry_the_core_accepts_and_this_desk_reads() {
    vault_open();
    use zikaron::tokens::{EntryType, Key};
    let name = |id: &str| zikaron_store::EntryName::parse(id.trim_start_matches("0x")).expect("成名");
    let secret = app::key::Secret::take([0x21u8; 32]).expect("在阶内");
    let m = app::anchorx::mode();
    // Two ledgers of the same shape: genesis, two records, and a fourth entry that is a delete in one and an
    // annotation in the other (control).
    let lay = |tag: &str, fourth: &dyn Fn(&str) -> (String, zikaron::json::Value)| {
        let dir = std::env::temp_dir().join(format!("zk-retract-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let home = app::home::Home::open_or_create(&dir).expect("铺得出家");
        let ledger = home.ledger().expect("账本");
        let g = app::entryx::genesis(&secret, "retraction probe").expect("签得出");
        ledger.append(&name(&g.id), &g.bytes).expect("落得下");
        let h1 = app::entryx::seal(&secret, "history", 1, Some(&g.id), app::anchorx::history_body(&[0x5a; 32], &m, "one")).expect("签得出");
        ledger.append(&name(&h1.id), &h1.bytes).expect("落得下");
        let h2 = app::entryx::seal(&secret, "history", 2, Some(&h1.id), app::anchorx::history_body(&[0x5b; 32], &m, "two")).expect("签得出");
        ledger.append(&name(&h2.id), &h2.bytes).expect("落得下");
        let (ty, body) = fourth(&h1.id);
        let x = app::entryx::seal(&secret, &ty, 3, Some(&h2.id), body).unwrap_or_else(|f| panic!("the core refused {ty}: {}", f.said()));
        ledger.append(&name(&x.id), &x.bytes).expect("落得下");
        (dir, home, g.id, h1.id, h2.id, x.id)
    };
    let (d1, home, g, h1, h2, r) = lay("r", &|s| (app::retractx::ENTRY_TYPE.to_string(), app::retractx::body(s, "写错了")));
    let (d2, control, ..) = lay("c", &|s| ("annotation".to_string(), app::anchorx::annotation_body(Some(s), "写错了")));

    let a = app::auditx::offline(&home).expect("核出得来");
    let b = app::auditx::offline(&control).expect("核出得来");
    let unknown = app::auditx::rows_of(&a.report, Key::UnknownType);
    assert_eq!(unknown.len(), 1, "删除条目列进 UNKNOWN_TYPE,恰一条");
    assert!(format!("{:?}", unknown[0]).contains(r.trim_start_matches("0x")), "列的就是那一条:{:?}", unknown[0]);
    assert!(app::auditx::rows_of(&b.report, Key::UnknownType).is_empty(), "对照账没有未知类型");
    assert_eq!(a.label, b.label, "标签不因开放类型变");
    assert!(!a.label.is_empty(), "有标签");

    let rows = app::ledgerx::table(&home, None, &[]).expect("读得成表").rows;
    let x = rows.iter().find(|x| x.id == r).expect("表里有删除条目");
    assert_eq!(x.kind, EntryType::Other);
    assert!(app::retractx::is_retraction(x));
    let reading = app::retractx::read(&rows);
    assert!(reading.is_deleted(&h1) && !reading.is_deleted(&h2));
    assert!(app::retractx::work_deleted(&rows, &zikaron::hexfmt::encode(&[0x5a; 32])));
    assert!(!app::retractx::work_deleted(&rows, &zikaron::hexfmt::encode(&[0x5b; 32])));

    // The invalid forms then land in the same ledger: repeated delete, deleting genesis, deleting another
    // ledger's entry, malformed. The core accepts them and the ledger is not refused.
    let ledger = home.ledger().expect("账本");
    let other = format!("0x{}", "ee".repeat(32));
    let bad = [
        app::retractx::body(&h1, ""),
        app::retractx::body(&g, ""),
        app::retractx::body(&other, ""),
        zikaron::json::Value::Obj(vec![(app::retractx::SUBJECT.to_string(), zikaron::json::Value::Str("0x12".into()))]),
    ];
    let mut prev = r.clone();
    let mut ids = Vec::new();
    for (i, body) in bad.into_iter().enumerate() {
        let e = app::entryx::seal(&secret, app::retractx::ENTRY_TYPE, 4 + i as u64, Some(&prev), body).unwrap_or_else(|f| panic!("the core refused form {i}: {}", f.said()));
        ledger.append(&name(&e.id), &e.bytes).expect("落得下");
        prev = e.id.clone();
        ids.push(e.id);
    }
    let rows = app::ledgerx::table(&home, None, &[]).expect("读得成表").rows;
    let reading = app::retractx::read(&rows);
    use app::retractx::Invalid;
    let why = |id: &str| reading.invalid.get(id).map(|x| x.1);
    assert_eq!(why(&ids[0]), Some(Invalid::Repeated));
    assert_eq!(why(&ids[1]), Some(Invalid::NotAWork));
    assert_eq!(why(&ids[2]), Some(Invalid::NotInLedger));
    assert_eq!(why(&ids[3]), Some(Invalid::Shape));
    assert_eq!(reading.count, 5);
    let after = app::auditx::offline(&home).expect("无效的删除也不拒账本");
    assert_eq!(app::auditx::rows_of(&after.report, Key::UnknownType).len(), 5);
    let _ = std::fs::remove_dir_all(&d1);
    let _ = std::fs::remove_dir_all(&d2);
}

/// The wizard's two gates. Without a passcode every wizard step lands on the passcode step (it is the
/// precondition for holding any key); with a passcode but no identity created with verification or imported,
/// the later steps land on the identity step. The gate lives in `Progress::gate`, and every window frame
/// passes it (step buttons, "start ledger…" and rerunning the wizard all go through here).
#[test]
fn the_wizard_never_stands_past_the_identity_step_without_an_identity() {
    use app::nav::{Progress, Step};
    // The passcode comes first. No passcode means no vault, and creating an identity needs keys, which need
    // an open vault: the reverse order would make first run impossible (the action layer answers "key vault
    // locked", with opening the vault a later step).
    let no_pin = Progress { key: true, pin: false, network: true, gas: true, genesis: true, backup: true };
    for s in Step::ALL {
        assert_eq!(no_pin.gate(s), Step::Pin, "没口令时去 {s:?} 应落在第一步(设口令)");
    }
    let none = Progress { key: false, pin: true, network: true, gas: true, genesis: true, backup: true };
    for s in Step::ALL {
        let want = if matches!(s, Step::Pin | Step::Key) { s } else { Step::Key };
        assert_eq!(none.gate(s), want, "没有身份时去 {s:?} 应落在 {want:?}");
    }
    // Neither present: the first step is setting the passcode.
    let bare = Progress { key: false, pin: false, network: true, gas: false, genesis: false, backup: false };
    for s in Step::ALL {
        assert_eq!(bare.gate(s), Step::Pin, "全新一台机器上去 {s:?} 应落在设口令");
    }
    assert_eq!(bare.first_open(), Some(Step::Pin), "全新一台机器上第一步是设口令");
    let with_key = Progress { key: true, pin: true, network: true, gas: false, genesis: false, backup: false };
    for s in Step::ALL {
        assert_eq!(with_key.gate(s), s, "有身份有口令时去 {s:?} 就是 {s:?}");
    }
    // The window really calls it, and before drawing this step (not merely in a function nobody reads).
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let at_gate = text.find("let step = pr.gate(want);").expect("向导弹层里要过 Progress::gate");
    let at_draw = text.find("full::cover(ctx, \"wizard\"").expect("the wizard's cover");
    assert!(at_gate < at_draw, "门要在画弹层之前过");
}

/// Required steps have neither "later" nor a skip button: passcode, identity and ledger creation are
/// required; only gas funding and backup location can be done later; a user does not necessarily need a
/// ledger, so ledger creation can wait for users. The rule is one table, `Step::deferrable`, asked by
/// both window buttons.
#[test]
fn only_gas_and_backup_may_be_postponed_in_the_wizard() {
    use app::nav::Step;
    use app::roles::Role;
    for role in [Role::Author, Role::Grantee] {
        assert!(!Step::Pin.deferrable(role), "设口令必做({role:?})");
        assert!(!Step::Key.deferrable(role), "创建身份必做({role:?})");
        assert!(Step::Gas.deferrable(role), "充值 gas 可后做({role:?})");
        assert!(Step::Backup.deferrable(role), "整机备份可后做({role:?})");
    }
    assert!(!Step::Genesis.deferrable(Role::Author), "记录者创建账本必做");
    assert!(Step::Genesis.deferrable(Role::Grantee), "使用方创建账本可后做");
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let code = code_only(&text);
    assert_eq!(code.matches("step.deferrable(role)").count(), 1, "「以后再说」那一枚键问 Step::deferrable,别处不另拼");
    assert!(!code.contains("matches!(step, Step::Key | Step::Pin)"), "不许再各自拼哪几步能跳");
    // Order: ledger creation is required and comes before gas funding; network sits after identity and before
    // ledger creation.
    assert_eq!(Step::ALL, [Step::Pin, Step::Key, Step::Network, Step::Genesis, Step::Gas, Step::Backup]);
    assert!(!Step::Network.deferrable(Role::Author) && !Step::Network.deferrable(Role::Grantee), "网络一步默认那一行已选中,不可后做");
}

// ═════════════════════ Vault details · One key, one seat ═════════════════════

/// Using a key passes the passcode gate, one statement covering three exits.
///
/// Exporting the key file, deleting the identity and showing recovery words each need a local passcode. If
/// each called `keybox::unlock` in its own code, "the passcode" would be scattered over three places: one
/// might forget to ask, or keep its own attempt counter, and five attempts would not be five. So actions
/// carrying a `pin` cell register in a closed table (`Action::pin_asked`), and `apply` performs the check
/// once.
///
/// This test scans: every `Action` variant with a `pin` cell must appear in `pin_asked`; the check happens
/// exactly once in `apply`, placed after "locked means refuse": a locked vault still refuses every
/// key-needing action per the closed table, and this gate is a second identification on an open vault.
#[test]
fn the_three_actions_that_use_a_key_ask_the_one_passcode_gate() {
    let text = read_src_file("action.rs").expect("读不出 action.rs");
    let code = code_only(&text);
    // Every member with a `pin` cell in the enumeration (read from the `Action` source now, not copied as a
    // list).
    let at = code.find("pub enum Action {").expect("Action 枚举");
    let body = &code[at..code[at..].find("\n}\n").map(|e| at + e).unwrap_or(code.len())];
    let mut carriers: Vec<String> = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        let Some((head, rest)) = line.split_once(" {") else { continue };
        if !rest.contains("pin") {
            continue;
        }
        let name = head.trim_end_matches(char::is_whitespace).to_string();
        if name.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
            carriers.push(name);
        }
    }
    assert!(carriers.len() >= 8, "带 pin 那一格的动作只数出 {} 员", carriers.len());
    let gate = code.find("pub fn pin_asked").expect("pin_asked 那一张闭表");
    let gate_body = &code[gate..code[gate..].find("\n    }\n").map(|e| gate + e).unwrap_or(code.len())];
    for name in &carriers {
        assert!(gate_body.contains(name.as_str()), "{name} 带着一格口令而没在 pin_asked 里报名");
    }
    // The three exits pass the gate; the five passcode gate exits themselves do not (asking the exit itself,
    // not a string).
    use app::action::Action as A;
    for a in [
        A::BackupKey { pin: "1".into(), password: String::new().into(), again: String::new().into(), dir: String::new() },
        A::DeleteIdentity { id: String::new(), pin: "2".into() },
        A::RevealWords { pin: "3".into() },
    ] {
        assert!(a.pin_asked().is_some(), "{a:?} 该过口令闸");
    }
    for a in [
        A::SetPin { pin: "1".into(), again: "1".into() },
        A::Unlock { pin: "1".into() },
        A::ChangePin { old: "1".into(), pin: "2".into(), again: "2".into() },
        A::RecoverWords { words: String::new().into(), pin: "1".into(), again: "1".into() },
        A::RecoverKeystore { path: String::new(), password: String::new().into(), pin: "1".into(), again: "1".into() },
    ] {
        assert!(a.pin_asked().is_none(), "{a:?} 是口令门自己,不过这一闸");
    }
    // Performed exactly once, after "locked means refuse": a locked vault still refuses per the closed table,
    // unchanged.
    assert_eq!(code.matches("a.pin_asked()").count(), 1, "过闸那一句只许一处");
    let run = code.find("a.pin_asked()").expect("执行那一句");
    let locked = code.find("a.needs_key() && !shell.unlocked()").expect("锁着即拒那一句");
    assert!(locked < run, "「锁着即拒」要摆在口令闸之前");
    // Export and delete are still in the key-needing table; showing words carries its own unlock question, so
    // it is not.
    assert!(A::BackupKey { pin: String::new().into(), password: String::new().into(), again: String::new().into(), dir: String::new() }.needs_key());
    assert!(A::DeleteIdentity { id: String::new(), pin: String::new().into() }.needs_key());
    assert!(!A::RevealWords { pin: String::new().into() }.needs_key());
}

/// Files are written in one place only, and permissions are set only in that statement.
///
/// With the key vault writing "temporary file, `sync_all`, `rename`" on its own beside `home::put_at`, the
/// two would set permissions separately (one 0600, the other 0644 by umask, readable byte for byte by other
/// accounts on the same machine). So the vault file also goes through that one place, which creates files
/// owner-only (`zikaron_os::owner_only`). Read back on disk: a file landed through it is owner-only.
#[test]
fn every_small_file_this_desk_lands_is_owner_only() {
    let home = code_only(&read_src_file("home.rs").expect("读不出 home.rs"));
    assert!(home.contains("zikaron_os::owner_only(&mut o)"), "落档那一句建临时档时即只本人可读写");
    assert!(home.contains("create_new(true)"), "临时名撞上了即具名拒,不去截断别人那一份");
    assert!(home.contains("open_owner_only(&tmp)") && home.contains("zikaron_os::replace(&tmp, &p)"), "临时档建时即只本人,替换进位");
    // The behavior, on this system: a file landed through the one write is owner-only on disk.
    let dir = std::env::temp_dir().join(format!("zk-owner-only-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    app::home::put_at(&dir, "probe.json", b"{}").expect("落得下");
    assert!(zikaron_os::is_owner_only(&dir.join("probe.json")).expect("读得回"), "落下的档只本人可读写");
    let _ = std::fs::remove_dir_all(&dir);
    // The vault side no longer writes files itself: `rename` and `File::create` appear nowhere in keybox.rs.
    let keybox = code_only(&read_src_file("keybox.rs").expect("读不出 keybox.rs"));
    for own in ["std::fs::rename", "File::create"] {
        assert!(!keybox.contains(own), "keybox.rs 里又自己落档了:{own}");
    }
    assert!(keybox.contains("crate::home::put_at(&dir, name, &bytes)"), "库档(与摆在旁边的新库)该经落档那一处");
    assert!(keybox.contains("crate::home::rename_over(&from, &to)"), "新库生效那一下改名也经家那一处");
}

/// One key, one seat: two seats with the same address cannot be constructed.
///
/// Writing one existing key into both seats (`author: a, grantee: a`) would make the user seat sign with the
/// recorder key, and readers of the frozen core could not tell the author's receipts from the grantee's
/// entries. The design is in the shape: seats and addresses are carried by a closed table
/// [`app::identity::Keys`]; `One` carries a single seat, `Both` has two addresses derived by two family paths
/// (necessarily different), and `Legacy` (the key at the original account base) has no path to create a new
/// one.
#[test]
fn one_key_sits_on_one_seat_and_the_old_shape_cannot_be_built() {
    use app::identity::{Keys, Registry, Row};
    use app::key::Address;
    use app::roles::Role;
    let a = Address([0x11; 20]);
    let g = Address([0x22; 20]);
    // One seat occupied: the other answers "no address, no home, no slot".
    let one = Row {
        id: a.hex(),
        keys: Keys::One { seat: Role::Grantee, addr: a },
        author_home: app::identity::UNSEATED.to_string(),
        grantee_home: "/x/g".to_string(),
        backed_words: false,
        backed_file: true,
        backup_at: app::identity::NO_BACKUP_AT.to_string(),
        label: app::identity::NO_LABEL.to_string(),
        created: app::identity::NO_CREATED.to_string(),
        network: None,
        custom: None,
    };
    assert_eq!(one.address(Role::Author), None);
    assert_eq!(one.address(Role::Grantee), Some(a));
    assert_eq!(one.home(Role::Author), None);
    assert_eq!(one.account(Role::Author), None);
    assert_eq!(one.accounts().len(), 1, "只占一席即只占一槽");
    assert_eq!(one.seats(), vec![Role::Grantee]);
    assert_eq!(one.first_seat(), Role::Grantee, "落在它占着的那一席上");
    // The two-seat branch: kind and slot shape are answered by the key's shape, with no separate cell.
    let both = Row { keys: Keys::Both { author: a, grantee: g }, author_home: "/x/a".into(), ..one.clone() };
    assert_eq!(both.kind(), app::identity::Kind::Words);
    assert_eq!(one.kind(), app::identity::Kind::Existing);
    assert_eq!(both.slot(), app::identity::Slot::Own);
    // Canonical byte round trip: an empty seat's address and home cells are UNSEATED, and the row reads back
    // the same.
    let reg = Registry { current: Some((one.id.clone(), Role::Grantee)), rows: vec![one.clone()], left: Vec::new() };
    assert_eq!(Registry::parse(&reg.to_bytes()).expect("读得回"), reg);
    // Rows written by older versions: the existing-key branch with both seats at one address (`author` and
    // `grantee` both that address), a home for each seat, and its own slot. Read back, it occupies only the
    // recorder seat, and the user seat is left empty.
    let before_this_change = format!(
        concat!(
            "{{\"current\":{{\"identity\":\"{id}\",\"seat\":\"author\"}},",
            "\"identities\":[{{\"author\":\"{id}\",\"backup\":{{\"file\":true,\"words\":false}},",
            "\"grantee\":\"{id}\",\"homes\":{{\"author\":\"/x/a\",\"grantee\":\"/x/g\"}},",
            "\"id\":\"{id}\",\"kind\":\"existing\",\"slot\":\"own\"}}],",
            "\"shape\":\"zikaron-desk/identities/1\"}}"
        ),
        id = a.hex()
    );
    let back = Registry::parse(before_this_change.as_bytes()).expect("旧形读得回");
    assert_eq!(back.rows.len(), 1);
    assert_eq!(back.rows[0].keys, Keys::One { seat: Role::Author, addr: a }, "两席同址那一行读回来只占记录者那一席");
    assert_eq!(back.rows[0].address(Role::Grantee), None, "使用方那一席就此留空");
    assert_eq!(back.rows[0].label, app::identity::NO_LABEL);
    assert_eq!(back.rows[0].created, app::identity::NO_CREATED);
    assert_eq!(back.rows[0].backup_at, app::identity::NO_BACKUP_AT);
    // Missing `label`, `created` and `backup.at` cells default by name without crashing.
    let stripped = String::from_utf8(reg.to_bytes()).expect("utf8")
        .replace("\"created\":\"\",", "")
        .replace("\"label\":\"\",", "")
        .replace("\"at\":\"\",", "");
    let old_row = Registry::parse(stripped.as_bytes()).expect("缺三栏也读得回");
    assert_eq!(old_row.rows[0].label, app::identity::NO_LABEL);
    assert_eq!(old_row.rows[0].created, app::identity::NO_CREATED);
    assert_eq!(old_row.rows[0].backup_at, app::identity::NO_BACKUP_AT);
    // The name cell carries no weight: changing it affects no check, only that cell's bytes.
    let named = Row { label: "工作室主号".into(), ..one.clone() };
    assert_eq!(named.kind(), one.kind());
    assert_eq!(named.accounts(), one.accounts());
    assert_eq!(named.first_seat(), one.first_seat());
}

/// The owner of "backed up" is the file on disk, not the previous step's exit code.
///
/// The flag only records "this was done once"; when the disk is full or read-only, the landing place was
/// swapped, or the written bytes do not match those in hand, and any of these returns no error, the register
/// gets a false flag, the person deletes the identity relying on it, and the key is lost forever. So the
/// landing step reads back and compares once after writing (shape and address), refusing by name as
/// `BACKUP_NOT_LANDED` on mismatch; afterwards "did this identity's backup really land" is answered by
/// `identity::backup_seen` reading the disk now, asked by the face.
#[test]
fn a_backup_counts_only_when_the_file_is_really_on_disk() {
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    // The read-back comes before "backed up" is handed out.
    let check = code.find("landed_check(&path, ks.address)?;").expect("落地之后该现读回来比一次");
    let done = code.find("Done::Keystore(crate::task::Keystore::BackedUp").expect("交出已备份那一句");
    assert!(check < done, "先读回来比过,再交出「已备份」");
    assert!(code.contains("Known::BackupNotLanded"), "对不上要具名拒");
    // Where the flag is recorded, the landing place is recorded with it (without it, "where is it" cannot be
    // asked).
    let shell = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert!(shell.contains("crate::identity::mark(reg, id, false, true, Some(path))"), "旗与落处一起记");
    // The reading exit only reads, changing no cell.
    let identity = code_only(&read_src_file("identity.rs").expect("读不出 identity.rs"));
    let at = identity.find("pub fn backup_seen").expect("读数口");
    let body = &identity[at..identity[at..].find("\n}\n").map(|e| at + e).unwrap_or(identity.len())];
    for writes in ["write(", "std::fs::write", "remove_file", "mark("] {
        assert!(!body.contains(writes), "读数口改了盘上的东西:{writes}");
    }
    // The window asks it, never assembling its own "does the file exist". Nothing is asked
    // in the frame: the identity card reads the cell saved when the register changed hands
    // (`Shell::seat_identities`), the delete card the cell saved when the layer opened.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert_eq!(window.matches("crate::identity::backup_seen").count(), 1, "脸上只许在开层那一刻问一处");
    assert!(window.contains("self.shell.backup_seen"), "身份卡读换手那一刻存下的那一格");
    assert_eq!(shell.matches("crate::identity::backup_seen").count(), 1, "换手那一处问一次");
}

/// The preconditions for deletion include the ledgers in every seat home this row occupies.
///
/// Looking only at the recorder seat's home would hurt someone who used both seats: after deleting the
/// identity the user seat's ledger (grants and receipts) could never be signed again, and a handover in it
/// would never have been considered; an existing key occupying only the user seat is worse, since its
/// handover can only be written in that seat's home and would never be found. So the closed seat table comes
/// from `Row::seats`, ledgers are read seat by seat, and no seat is hard-coded here.
#[test]
fn deleting_an_identity_looks_at_the_ledgers_of_every_seat_it_holds() {
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    let at = code.find("fn delete_identity").expect("删除那一段");
    let body = &code[at..code[at..].find("\n}\n").map(|e| at + e).unwrap_or(code.len())];
    assert!(body.contains("for s in row.seats()"), "账本该按这一行占着的每一席逐席读");
    assert!(!body.contains("row.home(crate::roles::Role::Author)"), "不许再写死记录者那一席");
    assert_eq!(body.matches("crate::local::Ledger::open(").count(), 1, "the ledger is opened in exactly one place (through the local data module's single entry point)");
    assert!(body.contains("crate::succeedx::handed_over"), "移交那一问照旧交给 succeedx");
}

// ═════════════════════ Seat × domain · Home ownership decided in one place ═════════════════════

/// Which seat can sign which domain is said in one place only.
///
/// If fetching the signing key asked only "does the current seat own the open home" and not "which domain is
/// this", the grantee seat could sign co-signatures, and chain readers could not tell what each seat signed.
/// Checking the seat once more inside signing would be forgotten again with the next domain. The
/// design: the domain-to-seat mapping lives only in the closed table `sign::seat_may`; every key user names
/// this use and passes it in, and key fetching allows it by the table or refuses by name as `SEAT_DOMAIN`.
#[test]
fn which_seat_signs_which_domain_is_said_in_exactly_one_place() {
    use app::roles::Role;
    use app::sign::{Face, Use};
    // The closed set of three is computed from the two faces: an extra domain adds a member here the same
    // day.
    assert_eq!(Use::all().len(), Face::ALL.len() + 1);
    // Six cells, cell by cell: this table is the owner of that rule.
    let want: [(Role, Use, bool); 6] = [
        (Role::Author, Use::Sign(Face::Entry), true),
        (Role::Author, Use::Sign(Face::Adoption), true),
        (Role::Author, Use::Anchor, true),
        (Role::Grantee, Use::Sign(Face::Entry), true),
        (Role::Grantee, Use::Sign(Face::Adoption), false),
        (Role::Grantee, Use::Anchor, true),
    ];
    for (seat, u, yes) in want {
        assert_eq!(app::sign::seat_may(seat, u), yes, "{seat:?} × {} 那一格答错了", u.as_str());
    }
    // A use available to only one seat can say which seat to go to (the refusal needs it).
    assert_eq!(app::sign::seat_for(Use::Sign(Face::Adoption)), Some(Role::Author));
    assert_eq!(app::sign::seat_for(Use::Sign(Face::Entry)), None);
    // Every place fetching the signing key names a use: four places, each with a `sign::Use`.
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    let calls: Vec<&str> = code.match_indices("= signing_key(shell").map(|(i, _)| &code[i..(i + 80).min(code.len())]).collect();
    assert_eq!(calls.len(), 4, "取签名钥那一口现数 {} 处", calls.len());
    for one in &calls {
        assert!(one.contains("crate::sign::Use::"), "有一处取签名钥没有具名递用处:{one}");
    }
    // Decided in one place: `seat_may` is asked only once in the action module, and no other action code
    // assembles its own seat check.
    assert_eq!(code.matches("crate::sign::seat_may(").count(), 1, "席位 × 域那一问只许在取钥那一处问");
    assert!(code.contains("Known::SeatDomain"), "拒要具名 SEAT_DOMAIN");
    for wrong in ["shell.settings.role == crate::roles::Role::Grantee &&", "role != crate::roles::Role::Author {"] {
        assert!(!code.contains(wrong), "动作段里又自己拼了一次席位判:{wrong}");
    }
    // The window side computes from the table, with no separate copy of the list.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains("crate::sign::seat_may(seat, *u)"), "脸上那一行该照闭表现算");
}

/// Opening a home is decided in one place, and no home-opening path bypasses it.
///
/// The victim: an author returning to their own home after switching seats or importing is judged a reader,
/// cannot write, and the face says nothing (as `lock.rs` notes: opening another descriptor in the same
/// process judges itself a reader). This test guards that: taking the writer lock and setting "the
/// open home" each appear in exactly two places in the product (opening and moving), and the re-entry reuse
/// check exists only in opening.
#[test]
fn opening_a_home_is_judged_in_one_place() {
    let mut takes = 0usize;
    let mut set_home = 0usize;
    let mut set_lock = 0usize;
    for (name, text) in shipped() {
        let code = code_only(&text);
        takes += code.matches("lock::take(").count();
        set_home += code.matches("shell.home = Some(").count();
        set_lock += code.matches("shell.lock = Some(").count();
        let _ = name;
    }
    assert_eq!(takes, 2, "取写者锁那一句在产品里现数 {takes} 处(该是开家与搬家两处)");
    assert_eq!(set_home, 2, "摆「正开着的那一处家」现数 {set_home} 处");
    assert_eq!(set_lock, 2, "摆那一把锁现数 {set_lock} 处");
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    // The re-entry reuse check exists only in opening, and comes before taking the lock.
    let at = code.find("fn open_home_at").expect("开家那一段");
    let body = &code[at..code[at..].find("\n}\n").map(|e| at + e).unwrap_or(code.len())];
    assert!(body.contains("crate::lock::holds_writer(shell.lock.as_ref(), &home)"), "复用那一判要在开家那一处");
    assert_eq!(code.matches("crate::lock::holds_writer(").count(), 1, "复用那一判只许一处");
    // Both home-opening paths go through here: the one the person chose (`open_home`) and entering an
    // identity's seat (`enter`).
    for caller in ["fn open_home(", "fn enter("] {
        let a = code.find(caller).unwrap_or_else(|| panic!("找不到 {caller}"));
        let b = &code[a..code[a..].find("\n}\n").map(|e| a + e).unwrap_or(code.len())];
        assert!(b.contains("open_home_at("), "{caller} 该走开家那一处");
    }
}

/// Key files are readable only by their owner; what is handed to the counterpart stays readable by others.
///
/// Without a "who can read it" cell, every file would follow the environment's umask (commonly 022, giving
/// 0644). Making everything 0600 would also tighten what should be handed to the counterpart (grant
/// documents, manifests and receipts, badges, disclosure kits, mirrors), which is another mistake. So the
/// caller names that cell per item ([`Readers`]), and the key file path names `Owner`.
#[test]
fn the_exported_key_file_is_for_its_owner_only_and_the_rest_are_not() {
    let code = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    // The key file path names Owner.
    assert!(
        code.contains("land_bytes_for(zikaron_glue::landing::Readers::Owner, &path, &ks.json)"),
        "导出密钥文件那一路该具名 Readers::Owner"
    );
    // No blanket rule: exactly this one place in the product writes with Owner.
    let mut owner = 0usize;
    for (_, text) in shipped() {
        owner += code_only(&text).matches("Readers::Owner").count();
    }
    assert_eq!(owner, 1, "具名 Owner 的落档现数 {owner} 处(只该是密钥文件那一路)");
    // The landing's permission is set in one statement, and the temporary file has it from creation (moving
    // into place uses hard_link, keeping the same inode's permission).
    let landing = std::fs::read_to_string(
        src().parent().expect("crates/app/src 的上一级").parent().expect("crates").join("zikaron-glue").join("src").join("landing.rs"),
    )
    .expect("读不出 landing.rs");
    assert!(landing.contains("zikaron_os::owner_only(&mut o)"), "只给本人那一路建临时地时即只本人可读写");
    assert_eq!(landing.matches("fn create_for(").count(), 1, "建临时地那一句只许一处");
    assert!(landing.contains("fn land_bytes_for("), "落档那一处该收「谁读得到」那一格");
}

// ═════════════════════ Vault lock, anchored record, landing place ═════════════════════

/// Reading, changing and writing the vault file all happen under one lock: writing the vault without the lock
/// has no form in the code.
///
/// "Record first, then try" (increment the attempt count on disk, then run the KDF) only narrows the window
/// to between two disk writes, and the window remains: N concurrent processes each read the same error count
/// and each write it back plus one, so five attempts become N times five (the victim: a machine used to
/// brute-force the passcode with many processes). So the whole vault section is under `flock`, and the entire
/// concurrency class disappears along with future instances; "writing the vault needs the lock first" is
/// carried by the type: `write_book`'s first argument is a reference to that lock, so without the lock the
/// call cannot be written.
#[test]
fn the_key_store_is_written_only_with_its_lock_in_hand() {
    let code = code_only(&read_src_file("keybox.rs").expect("读不出 keybox.rs"));
    // The writer takes a reference to the lock, and the lock is taken in one place only.
    assert!(code.contains("fn write_book(_held: &Held, b: &Book)"), "写库那一处该收手里那一把锁");
    assert_eq!(code.matches("fn lock_book()").count(), 1, "取库锁那一句只许一处");
    assert!(code.contains("crate::lock::grab_waiting("), "库档那一把要等得起(拿不到即等着)");
    // Every vault writer has taken the lock earlier in the same function: count the places for "write" and
    // "take lock".
    let writes = code.matches("write_book(&held").count() + code.matches("write_book(held,").count();
    assert_eq!(code.matches("write_book(").count(), writes + 1, "有一处写库没有把手里那一把递进去(+1 是定义那一处)");
    // Every exit that changes the vault starts by taking the lock (counted: the lock-taking places equal the
    // number of exits). Sixteen exits: set passcode, unlock, reseal at the lower bound, change passcode,
    // recover, settle the primary of an older vault, upgrade an older vault's names, record recovery seal,
    // remove recovery seal, place slot, remove slot, remove every slot, reset empty vault, stage a new vault,
    // commit it, drop a staged one.
    assert_eq!(code.matches("lock_book()?").count(), 16, "要改库的那几口各拿一次锁");
    // The vault lock and the home's writer lock are two locks: the vault lock's seat is in the machine
    // directory, named in one place by `places`.
    assert!(code.contains("crate::places::keybox_lock_file()"), "锁座那一份档的名由 places 一处给");
    let places = code_only(&read_src_file("places.rs").expect("读不出 places.rs"));
    assert!(places.contains("fn keybox_lock_file()"), "锁座那一份档的名住 places");
    assert!(!code.contains("crate::lock::take("), "库那一把不许借家的写者锁那一口");
    // The waiting and non-waiting exits each live in one place, with the words in lock.rs.
    let lock = code_only(&read_src_file("lock.rs").expect("读不出 lock.rs"));
    assert_eq!(lock.matches("pub fn grab(").count(), 1);
    assert_eq!(lock.matches("pub fn grab_waiting(").count(), 1);
}

/// "Was this entry anchored" is recorded by the queue file itself.
///
/// Otherwise only two things could answer: whether it is still queued (not after removal) and the last
/// self-audit report (silent if no audit ran or the report is stale). After a restart the queue would be
/// empty with no report, an anchored entry could be queued again, and the same entry would be anchored twice
/// on chain (the victim pays gas twice). So the receipt path records the fact on removal
/// (`Queue::anchored_out`) in the queue file, surviving restarts and home copies; and the one queueing entry
/// point asks it, so every queueing path gets it. Other removals (retraction) go through the remove-only exit
/// and record nothing: they state no chain fact (see the next rule).
#[test]
fn the_queue_remembers_what_it_anchored() {
    use app::queue::{Pushed, Queue};
    let mut q = Queue::default();
    let a = format!("0x{}", "11".repeat(32));
    let b = format!("0x{}", "22".repeat(32));
    assert_eq!(q.push(&a, 1), Pushed::Queued);
    assert_eq!(q.push(&a, 2), Pushed::InQueue, "同一枚只排一次");
    assert_eq!(q.push(&b, 3), Pushed::Queued);
    // The receipt path: removal and recording the fact happen together, so there is no frame where it was
    // removed but not recorded.
    assert_eq!(q.anchored_out(std::slice::from_ref(&a)), 1);
    assert!(q.anchored_here(&a));
    assert!(!q.anchored_here(&b));
    // The remove-only exit (used by retraction) records nothing: removed from the queue, and not in
    // "anchored".
    let mut r = q.clone();
    assert_eq!(r.drop_ids(std::slice::from_ref(&b)), 1);
    assert!(!r.anchored_here(&b), "撤回离队不记「锚成过」");
    assert_eq!(q.push(&a, 4), Pushed::Anchored, "锚成过的一枚再也排不进来");
    assert_eq!(q.len(), 1, "另一枚照旧在队里");
    // There is one queueing entry point, and its three forms each answer a sentence (closed table).
    let code = code_only(&read_src_file("queue.rs").expect("读不出 queue.rs"));
    assert_eq!(code.matches("pub fn push(").count(), 1, "入队只此一处");
    assert_eq!(code.matches("pub fn drop_ids(").count(), 1, "只出队的那一口只此一处");
    assert_eq!(code.matches("pub fn anchored_out(").count(), 1, "出队并记事实的那一口只此一处");
    let at = code.find("pub fn anchored_out(").expect("收据那一路的口");
    let body = &code[at..code[at..].find("\n    }\n").map(|e| at + e).unwrap_or(code.len())];
    assert!(body.contains("self.anchored.push("), "收据说成了那一路出队时记下那一枚");
    // The cell on disk: present in the canonical bytes; files written before this cell existed lack it and
    // read it as empty, not an error.
    let acted = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    assert!(acted.contains("crate::queue::Pushed::Anchored =>"), "落完顺手排那一处要照形出话");
    assert!(acted.contains("shell.queue.anchored_here(&id)"), "页上手排那一处也问同一格");
}

/// Choosing a landing place is decided in one place, and "why not the one you chose" is answered with it.
///
/// With three separate rules (`kit_target` / `migrate_target` / `file_target`), moving the home into a
/// non-empty folder would create a subdirectory without a word on the face (the victim: someone moving their
/// home into a folder that already has things, later unable to find their ledger there). So there is one
/// decision (`home::choose`), and its answer carries the closed table [`Why`] with three forms, spoken by the
/// face per form.
#[test]
fn where_a_thing_lands_is_judged_in_one_place_and_says_why() {
    use app::home::{choose, Kind, Why, HOME_STEM};
    let base = std::env::temp_dir().join(format!("zk-landing-{}", std::process::id()));
    let empty = base.join("empty");
    let full = base.join("full");
    std::fs::create_dir_all(&empty).expect("铺一处空的");
    std::fs::create_dir_all(full.join("something")).expect("铺一处不空的");
    // Moving the home: an empty folder is used; a non-empty one gets a new name, and it says why.
    let a = choose(&Kind::Home, &empty);
    assert_eq!((a.at.clone(), a.why), (empty.clone(), Why::AsPicked));
    let b = choose(&Kind::Home, &full);
    assert_eq!((b.at.clone(), b.why), (full.join(HOME_STEM), Why::FolderNotEmpty));
    assert!(b.free(), "新起的名此刻该空着");
    assert!(b.is_new_name());
    // Exporting a file: the first name is used when free; when taken, it is numbered and it says why.
    let d1 = choose(&Kind::File { stem: "snap".into(), ext: "json".into() }, &empty);
    assert_eq!(d1.why, Why::AsPicked);
    std::fs::write(&d1.at, b"x").expect("占住那一个名");
    let d2 = choose(&Kind::File { stem: "snap".into(), ext: "json".into() }, &empty);
    assert_eq!((d2.at.clone(), d2.why), (empty.join("snap-2.json"), Why::NameTaken));
    // Exporting a bundle: a file the person names is used.
    let named = base.join("pick-me.zip");
    let e = choose(&Kind::Bundle { stem: "kit".into() }, &named);
    assert_eq!((e.at.clone(), e.why), (named, Why::AsPicked));
    let _ = std::fs::remove_dir_all(&base);
    // None of those three names remains, and the landing question is asked in the product only through
    // `choose`.
    for (name, text) in shipped() {
        let code = code_only(&text);
        for gone in ["kit_target(", "migrate_target(", "file_target("] {
            assert!(!code.contains(gone), "{name} 里还留着落处那一族的旧成员:{gone}");
        }
    }
    let home = code_only(&read_src_file("home.rs").expect("读不出 home.rs"));
    assert_eq!(home.matches("pub fn choose(").count(), 1, "落处那一问只许一处判");
    assert_eq!(home.matches("fn numbered(").count(), 1, "往后编号那一句只许一处");
    // The face's sentence comes from `Why`, with nothing assembled on the window side.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains(".why.say()"), "搬家那一处要把「为什么」说出来");
    assert_eq!(window.matches("crate::home::choose(").count(), 4, "落处那几处调用各问同一句");
}

/// No disk in the frame: the added readings happen in the action layer or when a layer opens.
///
/// A code review once found four frame disk reads: the delete card read both seats' ledger directories
/// entirely and verified every signature each frame (a ledger of thousands of entries read sixty times per
/// second while the card was open, a freeze); the identity and delete cards each read the backup file each
/// frame; the import cell opened `/dev/urandom` each frame (drawing the three cells needs entropy); the move
/// row ran `read_dir` on the chosen place each frame. So the frame reads only cells on the shell and `Ux`,
/// and disk reads happen once in the action layer (`Shell::seat_identities`), when the layer opens
/// (`id_layer_open`), and when a directory is chosen.
#[test]
fn the_frame_body_asks_no_question_that_needs_the_disk() {
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    // Each of those four is scanned: how often the name appears in the window, and where it should be.
    assert!(!window.contains("crate::identity::backup_seen("), "备份档那一笔不许在帧里读(壳上那一格由动作层摆)");
    // The ledger reading is the action layer's (`action::seats_with_entries`), asked once when the layer opens;
    // the window opens no ledger itself.
    assert_eq!(window.matches("crate::local::Ledger::open(").count(), 0, "窗子不自己开账本目录");
    let at = window.find("fn id_layer_open").expect("开层那一处");
    let body = &window[at..window[at..].find("\n    }\n").map(|e| at + e).unwrap_or(window.len())];
    assert!(body.contains("crate::action::seats_with_entries"), "账本那一笔该在开层那一刻经动作层读");
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    let at = action.find("pub fn seats_with_entries(").expect("动作层那一口");
    let reader = &action[at..action[at..].find("\n}\n").map(|e| at + e).unwrap_or(action.len())];
    assert!(reader.contains("crate::local::Ledger::open("), "账本那一笔在动作层读");
    assert!(
        window.contains("self.ux.id_delete_backup = row.as_ref().map(crate::identity::backup_seen)"),
        "备份档那一笔该在开层那一刻读"
    );
    assert!(!window.contains("crate::identity::from_words(&self.ux.id_words"), "帧里不许为了「按不按得动」去开一次熵源");
    assert_eq!(window.matches("crate::home::choose(&crate::home::Kind::Home").count(), 1, "搬家那一处的落处只在选目录那一下现算");
    // The shell cell is set together by the exit that sets the identity table (one owner).
    let shell = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert_eq!(shell.matches("pub fn seat_identities(").count(), 1, "摆身份表那一口只许一处");
    assert!(shell.contains("self.backup_seen = reg"), "那一笔读数与身份表同一趟换");
    // That cell follows the identity table, not the ledger's source: when the home changes, it stays together
    // with `identities` (in `source_changed`'s closed table it is in the group that does not follow the
    // ledger's source).
    assert!(shell.contains("backup_seen: _,"), "备份档那一笔随身份表走,不随账的来源走");
    // The identity table is set through one exit only. The shell cell is a reading derived from the identity
    // table, so every writer of the identity table must go through `seat_identities`: in the whole product
    // the form "write to `identities`" may only appear inside that exit. (With `after_lock` writing its own
    // `.ok()`, the reading cell would stay at the answer from before the vault was locked.)
    for (name, text) in shipped() {
        let code = code_only(&text);
        let hits = code.matches(".identities = ").count();
        if name.ends_with("shell.rs") {
            assert_eq!(hits, 1, "{name} 里写身份表那一句只许一处(就在 seat_identities 里面)");
            let at = code.find("pub fn seat_identities(").expect("摆身份表那一口");
            let body = &code[at..code[at..].find("\n    }\n").map(|e| at + e).unwrap_or(code.len())];
            assert!(body.contains("self.identities = reg"), "那一句该在摆身份表那一口里面");
        } else {
            assert_eq!(hits, 0, "{name} 绕过了摆身份表那一口");
        }
    }
}

// ═════════════════════ The current row has one owner ═════════════════════

/// "Which row is current, which seat" is answered by two named exits; the product has no third hand-assembled
/// answer.
///
/// The rule is "take the current row, not the first row in the table". Without an owner, backup key slot
/// lookup would go through a `Registry::now` inside `account_now`, backup flag recording and word display
/// each through `view(seat)?.now()`, and signing key fetching, seat switching, home opening and mirror export
/// each through `read()?…now()`: seven places answering separately, one changed without the others knowing.
///
/// So the question is gathered into two exits, each named and readable: [`identity::now_row`] includes the
/// key at the account base (without a register, read as the current identity), and
/// [`register::now_row_listed`] accepts only a row in the register (without a register, none; it reads the
/// register, which is the archive's, so it lives there). They are
/// separate because the questions really differ: home ownership for signing, seat switching and home opening
/// need the latter, and reading the account base key as the current identity would pull machines without a
/// register into those gates (home opening would even rewrite the machine pointer). Every path points to its
/// owner.
#[test]
fn which_identity_is_current_is_asked_in_two_named_places() {
    let identity = code_only(&read_src_file("identity.rs").expect("读不出 identity.rs"));
    let register = code_only(&read_src_file("register.rs").expect("读不出 register.rs"));
    assert_eq!(identity.matches("pub fn now_row(").count(), 1, "含账名底那一枚那一口只许一处");
    assert_eq!(register.matches("pub fn now_row_listed(").count(), 1, "只认登记表那一口只许一处");
    assert_eq!(identity.matches("view.now()").count(), 1, "那一读只在 now_row 里面");
    assert_eq!(register.matches("read()?.and_then(|r| r.now()").count(), 1, "那一读只在 now_row_listed 里面");
    // The backup and word display paths ask the same exit.
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    for (what, at) in [("显词", "fn reveal_words"), ("备份", "fn backup_key")] {
        let i = action.find(at).unwrap_or_else(|| panic!("{what}那一段"));
        let body = &action[i..action[i..].find("\n}\n").map(|e| i + e).unwrap_or(action.len())];
        assert!(body.contains("crate::identity::now_row("), "{what}那一路要问那一口");
    }
    // Nowhere else in the product may take its own copy of the register and ask "who is current" (the window
    // and shell read the copy in hand, without disk).
    for (name, text) in shipped() {
        if name == "identity.rs" || name == "register.rs" || name == "window.rs" || name == "shell.rs" {
            continue;
        }
        let code = code_only(&text);
        assert!(!code.contains(".now()"), "{name} 绕过了那两口,自己问了一次「谁是当前」");
    }
}

// ═════════════════════ Removal records its reason ═════════════════════

/// "Anchored" is recorded only by the path that knows it.
///
/// The `anchored` cell is a statement about a chain fact. A single exit that recorded "anchored" for every
/// caller would, when retraction used it (a deleted record leaving the queue), record an entry that never
/// reached the chain as anchored; queueing it again later would answer "already on chain", and that cell of
/// the queue file would be false evidence.
///
/// So removal has two exits, each recording its reason. A successful receipt goes through
/// `Queue::anchored_out` (remove and record), called only by `settle`; other removals go through
/// `Queue::drop_ids` (remove only, touching not one byte of `anchored`). Queueing a deleted entry again
/// answers with the ledger fact ("this record is deleted"), never claiming a chain fact that did not happen.
#[test]
fn only_the_receipt_path_records_that_an_entry_was_anchored() {
    let queue = code_only(&read_src_file("queue.rs").expect("读不出 queue.rs"));
    assert_eq!(queue.matches("self.anchored.push(").count(), 1, "写「锚成过」那一句只许一处");
    let at = queue.find("pub fn anchored_out(").expect("收据那一路自己的口");
    let body = &queue[at..queue[at..].find("\n    }\n").map(|e| at + e).unwrap_or(queue.len())];
    assert!(body.contains("self.anchored.push("), "那一句住在收据那一路的口里");
    let at = queue.find("pub fn drop_ids(").expect("只出队那一口");
    let body = &queue[at..queue[at..].find("\n    }\n").map(|e| at + e).unwrap_or(queue.len())];
    assert!(!body.contains("anchored"), "只出队那一口一个字节也不碰 `anchored`");
    // The receipt exit is called in the product only by `settle`.
    let at = queue.find("pub fn settle(").expect("出队的唯一一处判");
    let body = &queue[at..queue[at..].find("\n}\n").map(|e| at + e).unwrap_or(queue.len())];
    assert!(body.contains("q.anchored_out(ids)"), "收据说成了那一路走记事实的那一口");
    for (name, text) in shipped() {
        if name == "queue.rs" {
            continue;
        }
        let code = code_only(&text);
        assert!(!code.contains(".anchored_out("), "{name} 不是收据那一路,不许记「锚成过」");
    }
    // Queueing a deleted entry again: refused by name, stating the ledger fact.
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    let at = action.find("fn queue_entry(").expect("入队那一扇门");
    let body = &action[at..action[at..].find("\n}\n").map(|e| at + e).unwrap_or(action.len())];
    assert!(body.contains("crate::retractx::read(&rows).is_deleted(&id)"), "被删了的那一条再入队要现读账问一次");
    assert!(body.contains("crate::lang::Key::Tail230"), "拒那一句说「此存证已删除」");
}

// ═════════════════════ Empty seats handled in two places ═════════════════════

/// Startup does not stop on an empty seat; buttons needing a home or key on an empty seat are decided in one
/// place.
///
/// Stopping on an empty seat at startup per the register would make the first-run wizard judge "create
/// identity" because this seat has no key, although the person has an identity; the data card's "change data
/// directory…" would still work on an empty seat and open a home for a seat without a key.
///
/// The design: (1) landing on a seat at startup follows the same rule as entering an identity and switching
/// identity (`Row::first_seat`); window startup goes only through `action::boot_home`, and the wizard's
/// preconditions are unchanged; (2) "is this seat empty" is decided only on the shell
/// (`Shell::seat_unseated`), asked by the identity card's and data card's buttons, with no second condition
/// in the window.
#[test]
fn boot_never_lands_on_an_empty_seat_and_the_empty_seat_is_judged_once() {
    let identity = code_only(&read_src_file("identity.rs").expect("读不出 identity.rs"));
    let at = identity.find("pub fn land_at_boot(").expect("开机落席那一口");
    let body = &identity[at..identity[at..].find("\n}\n").map(|e| at + e).unwrap_or(identity.len())];
    assert!(body.contains("row.first_seat()"), "开机落席与切换身份同一条规矩");
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    assert_eq!(action.matches("pub fn boot_home(").count(), 1, "开机那一趟只有一处");
    assert!(action.contains("crate::register::change_listed(crate::identity::land_at_boot)"), "开机先落席再开家");
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    // Window startup goes only through the action layer's `start`, which opens the home through `boot_home`.
    assert!(window.contains("crate::action::start(&mut shell)"), "窗子开机走那一处");
    let at = action.find("pub fn start(").expect("开机那一口");
    let start = &action[at..action[at..].find("\n}\n").map(|e| at + e).unwrap_or(action.len())];
    assert!(start.contains("boot_home(shell)"), "开机那一口经开家那一处");
    assert!(!window.contains("crate::identity::home_now("), "窗子不自己问开哪一处家");
    // Is this seat empty: decided once on the shell, with zero conditions written in the window.
    let shell = code_only(&read_src_file("shell.rs").expect("读不出 shell.rs"));
    assert_eq!(shell.matches("pub fn seat_unseated(").count(), 1, "一处判");
    assert!(!window.contains("r.address(seat).is_none()"), "窗子里不再自写「这一席空着」");
    assert!(window.contains("let unseated = self.shell.seat_unseated();"), "身份卡问那一口");
    assert!(window.contains("let seat_ok = !self.shell.seat_unseated();"), "数据卡问同一口");
    for key in ["t(Key::SetChangeHome), Role::Secondary, seat_ok)", "t(Key::DoMeasure), Role::Secondary, seat_ok, crate::task::Kind::Archive)"] {
        assert!(window.contains(key), "空席上这一键不可按:{key}");
    }
    // The wizard's preconditions are unchanged.
    let nav = code_only(&read_src_file("nav.rs").expect("读不出 nav.rs"));
    assert!(nav.contains("!self.key || !self.pin || (role == Role::Author && !self.genesis)"), "向导的前提不动");
}

// ═════════════════════ Passcode and vault hardening · anchoring words ═════════════════════

/// The body of a function in a source (from `fn <name>` to before the next top-level `fn ` or `pub fn `).
fn body_of<'a>(code: &'a str, head: &str) -> &'a str {
    let Some(at) = code.find(head) else { return "" };
    let rest = &code[at..];
    let end = rest[head.len()..]
        .find("\nfn ")
        .into_iter()
        .chain(rest[head.len()..].find("\npub fn "))
        .chain(rest[head.len()..].find("\n    fn "))
        .chain(rest[head.len()..].find("\n    pub fn "))
        .min()
        .map(|i| i + head.len())
        .unwrap_or(rest.len());
    &rest[..end]
}

/// "Which cell is secret" is answered by types: passcode, password, word and private key cells in the window,
/// and secret-carrying cells in actions, may only have the secret type (zeroed on drop, always masked in
/// debug output, never moved).
#[test]
fn every_secret_field_is_the_secret_type() {
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    let s = "crate::secret::Secret";
    for (field, ty) in [
        ("id_words", format!("[{s}; 12]")),
        ("id_hex", s.to_string()),
        ("id_ks_pw", s.to_string()),
        ("id_confirm", format!("[{s}; 3]")),
        ("id_pw", s.to_string()),
        ("id_pw2", s.to_string()),
        ("id_pin", s.to_string()),
        ("pin", s.to_string()),
        ("pin_again", s.to_string()),
        ("pin_old", s.to_string()),
        ("pin_words", format!("[{s}; 12]")),
        ("pin_ks_pw", s.to_string()),
    ] {
        assert!(window.contains(&format!("    {field}: {ty},")), "窗子那一格 `{field}` 的类型该是 {ty}");
    }
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    let enum_body = body_of(&action, "pub enum Action {");
    for line in enum_body.lines() {
        for secret in ["pin:", "again:", "old:", "words:", "password:"] {
            if line.contains(secret) {
                for part in line.split(',') {
                    if let Some(i) = part.find(secret) {
                        let ty = part[i + secret.len()..].trim().trim_end_matches('}').trim();
                        assert_eq!(ty, s, "动作里 `{}` 那一格的类型该是秘密型:{line}", secret.trim_end_matches(':'));
                    }
                }
            }
        }
    }
    assert!(action.contains("Words(crate::secret::Secret)") && action.contains("PrivateKey { key: crate::secret::Secret,"), "导入那几形的秘密也是秘密型");
    // The key file a primary import lands carries its password twice, each a secret.
    let keyfile = body_of(&action, "pub struct KeyFileOut {");
    assert!(keyfile.contains("pub password: crate::secret::Secret,") && keyfile.contains("pub again: crate::secret::Secret,"), "导入时那一份密钥文件的密码也是秘密型");
    // The widget library type's own three properties: zeroing, masked debug output, not moving (`secret.rs`'s
    // unit tests check the bytes).
    let lib = std::fs::read_to_string(src().join("..").join("..").join("zikaron-ui").join("src").join("secret.rs")).expect("读不出 secret.rs");
    assert!(lib.contains("impl Drop for Secret") && lib.contains("impl std::fmt::Debug for Secret"), "秘密型要自己抹零、自己遮调试输出");
}

/// The twelve cells are always masked, with the rule in the widget library component: the two call sites
/// write nothing and are masked, with no button showing plaintext; the gate's recovery path only asks whether
/// they are all filled and does not decode recovery words in the frame. "Back" clears all twelve cells.
#[test]
fn the_twelve_boxes_are_masked_in_the_kit_and_the_gate_does_not_parse_words_each_frame() {
    let ui = |f: &str| code_only(&std::fs::read_to_string(src().join("..").join("..").join("zikaron-ui").join("src").join(f)).expect("读不出件库"));
    let pin = ui("pin.rs");
    let grid = body_of(&pin, "pub fn words_grid_marked(");
    assert!(grid.contains("secret_numbered("), "the twelve cells are the masked numbered cell");
    assert!(!grid.contains("input::line") && !grid.contains("line_w("), "no plain cell among the twelve");
    let input = ui("input.rs");
    assert!(body_of(&input, "pub fn secret_numbered(").contains("secret_field("), "the numbered cell is the masked field");
    assert!(body_of(&input, "fn secret_field(").contains(".password(true)"), "遮住那一格恒遮");
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(!window.contains("identity::from_words("), "窗子里不许每帧解一次助记词(凑不凑得成交给动作层)");
    assert!(!window.contains("mask_grid(ui, \"gate-words\"") && !window.contains("IdShowWords"), "恢复与导入两处不设显示明文的键");
    let back = body_of(&window, "fn gate_back(");
    assert!(back.contains("pin_words.iter_mut()") && back.contains("w.clear()"), "恢复面板按「返回」清掉十二格");
}

/// Deleting an identity: the primary one is refused (another is made primary first); a secondary one takes any
/// recovery seal an older vault still holds for it along; the comment matches the code.
#[test]
fn deleting_an_identity_drops_its_recovery_seal_and_the_comment_says_so() {
    let text = read_src_file("identity.rs").expect("读不出 identity.rs");
    let del = body_of(&text, "pub fn delete(");
    assert!(code_only(del).contains("crate::keybox::drop_recovery(&row.id)?"), "删身份要撤它的恢复封");
    assert!(code_only(del).contains("Known::PrimaryDelete"), "主身份不直接删");
    assert!(del.contains("may still hold a seal for it: dropped with it"), "删身份那一处的注释要说旧库里那一道封随之撤掉");
    let kb = read_src_file("keybox.rs").expect("读不出 keybox.rs");
    assert!(kb.contains("outside what this file can wipe"), "keybox 档头讲抹零那一句要写明它管不到的那几样");
}

/// The post-landing test hook is set only by test code: in the shipped build this step is always empty.
#[test]
fn the_post_landing_test_hook_is_set_only_by_test_code() {
    for (name, text) in shipped() {
        let code = code_only(&text);
        if name == "action.rs" {
            assert_eq!(code.matches("set_landed_tamper").count(), 1, "action.rs 里 set_landed_tamper 只许有定义那一处");
        } else {
            assert!(!code.contains("set_landed_tamper"), "{name} calls set_landed_tamper: only test code may set the post-landing test hook");
        }
    }
}

/// Why the node refused is dispatched in one place: anchoring failures are no longer squashed into
/// `SEND_FAILED`; node refusal codes come only from `chainx::said_fault` (the pre-send balance gate has its
/// own place).
#[test]
fn what_the_node_said_is_dispatched_in_one_place() {
    let sign = code_only(&read_src_file("sign.rs").expect("读不出 sign.rs"));
    assert!(!sign.contains("Known::SendFailed"), "发锚那一路不许再把节点的话压成 SEND_FAILED");
    assert!(sign.contains("crate::chainx::said_fault(url, &t)"), "发锚那一路按节点说的话分派");
    for k in [
        "NonceUsed", "AlreadyPending", "Underpriced", "GasTooLow", "ContractRefused", "RateLimited", "MethodMissing",
        "NodeAuth", "WrongChain", "NodeRefused", "NodeTls", "NodeTimeout", "AnswerTooLong", "AnswerNotJson",
    ] {
        let uses: Vec<String> = shipped()
            .into_iter()
            .filter(|(n, _)| n != "fault.rs")
            .filter(|(_, t)| code_only(t).contains(&format!("Known::{k}")))
            .map(|(n, _)| n)
            .collect();
        assert_eq!(uses, vec!["chainx.rs".to_string()], "Known::{k} 只许由 chainx 那一处分派给出");
    }
    // "The transaction was sent but failed on chain" is kept only for the receipt status other than 1.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert_eq!(window.matches("Known::SendFailed").count(), 1, "SEND_FAILED 只在收据失败那一处说");
    assert!(app::action::receipt_failed(&app::action::receipt_state(0, 9)));
    assert!(!app::action::receipt_failed(&app::action::receipt_state(1, 9)));
    assert!(!app::action::receipt_failed("not yet in a block"));
}

/// Passcodes allow letters and digits, case-sensitive; all-identical is refused for any passcode, and
/// sequences and dates are judged only for all-digit ones.
#[test]
fn a_passcode_is_eight_ascii_letters_or_digits_and_patterns_are_judged_on_digits() {
    use app::keybox::{pin_trouble, PinTrouble};
    assert_eq!(pin_trouble("K7m2Xq9p"), None);
    assert_eq!(pin_trouble("Aaaaaaaa"), None);
    assert_eq!(pin_trouble("abcdefgh"), None);
    assert_eq!(pin_trouble("aaaaaaaa"), Some(PinTrouble::AllSame));
    assert_eq!(pin_trouble("K7m2Xq9!"), Some(PinTrouble::Shape));
    assert_eq!(pin_trouble("K7m2Xq9"), Some(PinTrouble::Shape));
    assert_eq!(pin_trouble("2761839\u{ff41}"), Some(PinTrouble::Shape));
    assert_eq!(pin_trouble("23456789"), Some(PinTrouble::Run));
    assert_eq!(pin_trouble("19991231"), Some(PinTrouble::DateLike));
    assert_eq!(pin_trouble("27618394"), None);
}

// ═════════════════════ Anchoring words · deletion ═════════════════════

/// Every legal token has plain words: twenty-six entry refusals and five scan refusals, each with a sentence
/// in both languages, all different; the face's plain half is the table's sentence, and the evidence tail is
/// still the token's name.
#[test]
fn every_legal_token_has_its_own_plain_sentence() {
    let row = |k: app::lang::Key| app::lang::TABLE.iter().find(|(x, _, _)| *x == k).map(|(_, zh, en)| (*zh, *en));
    let mut seen: Vec<app::lang::Key> = Vec::new();
    for t in zikaron::tokens::Token::ALL {
        let k = app::fault::entry_token_say(t);
        let (zh, en) = row(k).unwrap_or_else(|| panic!("{t:?} 的句不在表里"));
        assert!(!zh.is_empty() && !en.is_empty(), "{t:?} 缺一语");
        assert!(!seen.contains(&k), "{t:?} 与别的 token 共一句");
        seen.push(k);
        let f = app::fault::Fault::entry_refused(t);
        assert_eq!(f.human(), app::lang::t(k), "{t:?} 脸上那半句该是表里那一句");
        assert_eq!(f.tail(), format!("{t:?}"), "{t:?} 证据尾照旧是 token 的名");
        assert!(f.said().starts_with("ENTRY_REFUSED"), "{t:?} 码照旧");
    }
    use zikaron_anchor::scan::Refusal as R;
    let scans = [
        R::ChainIdMismatch { declared: 1, served: "0x2".into() },
        R::NoEndpoint(1),
        R::Unanswered { chain: 1, what: "x".into() },
        R::TxNotItself("0x".into()),
        R::Malformed("x".into()),
    ];
    let mut scan_keys: Vec<app::lang::Key> = Vec::new();
    for r in &scans {
        let k = app::fault::scan_refusal_say(r);
        let (zh, en) = row(k).unwrap_or_else(|| panic!("{} 的句不在表里", r.code()));
        assert!(!zh.is_empty() && !en.is_empty());
        assert!(!scan_keys.contains(&k), "{} 与别的拒因共一句", r.code());
        scan_keys.push(k);
        let f = app::fault::Fault::scan_refused(app::fault::Fault::scan_tail(r), std::slice::from_ref(r));
        assert_eq!(f.human(), app::lang::t(k));
        assert!(f.tail().starts_with(r.code()), "证据尾以拒因码起头");
    }
}

/// Masked cells do not open the input method: while a secret cell holds focus, input method events are
/// filtered, and the input method request is withdrawn after drawing; masking and input method handling live
/// in the same component, and call sites write nothing.
#[test]
fn a_masked_field_turns_the_input_method_off() {
    let input = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/input.rs")).expect("读不出 input.rs"));
    let body = body_of(&input, "fn secret_field(");
    assert!(body.contains("egui::Event::Ime(_)"), "握着焦点时输入法事件整枚不收");
    assert!(body.contains("o.ime = None"), "画完撤掉这一帧的输入法请求");
    assert_eq!(input.matches(".password(true)").count(), 1, "遮住的格只此一件");
}

/// Send or wait after queueing is answered by one cell: "auto anchor" is off by default; the main button's
/// text is chosen by one function.
#[test]
fn the_primary_key_reads_the_auto_anchor_setting_in_one_place() {
    assert!(!app::settings::Settings::default().auto_anchor, "自动上链默认关");
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    let pick = body_of(&window, "fn anchor_key(");
    assert!(pick.contains("settings.auto_anchor") && pick.contains("Key::AnchorNowKey") && pick.contains("Key::AddToLedgerKey"));
    assert!(window.matches("self.anchor_key()").count() >= 4, "存证与签发两页的主键与确认卡都读它");
    assert!(!window.contains("anchor_queue_only"), "表单里那一枚内存态单选撤了");
}

/// The queue file's six states are read back from disk; older files without state cells and block tables
/// still read as "queued".
#[test]
fn the_queue_file_keeps_each_step_and_reads_old_files() {
    vault_open();
    use app::queue::{Queue, Step};
    let dir = std::env::temp_dir().join(format!("zk-test-queue-steps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let h = app::home::Home::open_or_create(&dir).expect("建家");
    let id = |n: u8| format!("0x{}", format!("{n:02x}").repeat(32));
    let mut q = Queue::default();
    for n in 1..=4 {
        q.push(&id(n), n as u64);
    }
    q.mark(&[id(2)], Step::Submitted { tx: id(9), chain: 11155111 });
    q.mark(&[id(3)], Step::Reverted { tx: id(8), chain: 11155111 });
    q.mark(&[id(4)], Step::Refused { said: "INSUFFICIENT_FUNDS".into() });
    q.push(&id(5), 5);
    q.included_out(&[id(5)], &id(7), 11155111, 16);
    q.write(&h).expect("写");
    let back = Queue::read(&h).expect("读");
    assert_eq!(back.step_of(&id(1)), Some(&Step::Queued));
    assert_eq!(back.step_of(&id(2)), Some(&Step::Submitted { tx: id(9), chain: 11155111 }));
    assert_eq!(back.step_of(&id(3)), Some(&Step::Reverted { tx: id(8), chain: 11155111 }));
    assert_eq!(back.step_of(&id(4)), Some(&Step::Refused { said: "INSUFFICIENT_FUNDS".into() }));
    assert_eq!(back.block_of(&id(5)).map(|b| b.block), Some(16));
    assert!(back.anchored_here(&id(5)) && !back.has(&id(5)));
    assert_eq!(back.submitted(), vec![(id(9), 11155111, vec![id(2)])]);
    // Older file: only `queued` and `anchored`, with no state cell in rows.
    let old = format!("{{\"anchored\":[],\"queued\":[{{\"at\":1,\"id\":\"{}\"}}]}}", id(1));
    // An older version's shape, sealed as the queue file is now (the shape is what this reads, not the seal).
    app::local::put(&h.dir(app::home::Slot::Settings), app::queue::FILE, app::local::Doc::Queue, old.as_bytes()).expect("写旧档");
    let old = Queue::read(&h).expect("旧档读得回");
    assert_eq!(old.step_of(&id(1)), Some(&Step::Queued));
    assert!(old.blocks.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ═════════════════════ Landing and export · known deployments and new home inheritance ═════════════════════

/// Five landing failure forms each have plain words and a next step; the code is still `LANDING`, and the
/// tail is only the refusal code and subject.
#[test]
fn each_landing_failure_says_its_own_sentence() {
    let codes = ["E_IO", "E_OCCUPIED", "E_BAD_PATH", "E_DUPLICATE_PATH", "E_KIT"];
    let mut humans: Vec<String> = Vec::new();
    let mut nexts: Vec<String> = Vec::new();
    for c in codes {
        let f = app::fault::Fault::landing(c, "subject");
        assert!(f.said().starts_with("LANDING"), "{c} 码照旧");
        assert_eq!(f.tail(), format!("{c}: subject"), "{c} 尾只带码与主语");
        assert_ne!(f.human(), app::lang::t(app::lang::Key::FaultWhatLanding), "{c} 有自己那一句");
        humans.push(f.human().to_string());
        nexts.push(f.next().to_string());
    }
    let uniq = |v: &Vec<String>| {
        let mut x = v.clone();
        x.sort();
        x.dedup();
        x.len()
    };
    assert_eq!(uniq(&humans), codes.len());
    assert_eq!(uniq(&nexts), codes.len());
    let src = code_only(&read_src_file("kitx.rs").expect("读不出 kitx.rs"));
    assert!(!src.contains("Known::Landing"), "出包那一路的落盘失败只经 `Fault::landing` 一处");
}

/// Kit name transliteration happens in one place, with the kit crate's `is_kit_path` as the rule.
#[test]
fn kit_names_are_judged_by_the_kit_law_in_one_place() {
    let kitx = code_only(&read_src_file("kitx.rs").expect("读不出 kitx.rs"));
    assert!(body_of(&kitx, "pub fn kit_segment(").contains("kitdir::is_kit_path"));
    assert!(body_of(&kitx, "pub fn attach(").contains("kit_rel("), "附件逐件走转写那一处");
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains("crate::kitx::preview_names("), "附件列表预览与出包同一处转写");
}

/// The backup landing name is assembled in one place: `ZIKARON-backup/<address>/<seat>`; empty and relative
/// paths are refused.
#[test]
fn the_backup_bundle_path_is_built_in_one_place() {
    use app::roles::Role;
    // An absolute folder on this system (the temporary directory is one on every system).
    let x = std::env::temp_dir().join("x");
    let at = app::mirror::bundle_in(&x, "0xABCDEF0000000000000000000000000000000001", Role::Author).expect("拼得出");
    assert_eq!(at, x.join(app::mirror::STEM).join("abcdef0000000000000000000000000000000001").join(Role::Author.as_str()));
    assert_eq!(app::mirror::folder_of(&at), x.as_path());
    assert!(app::mirror::bundle_in(std::path::Path::new(""), "0x01", Role::Author).is_err());
    assert!(app::mirror::bundle_in(std::path::Path::new("rel"), "0x01", Role::Author).is_err());
    for name in shipped().into_iter().map(|(n, _)| n).filter(|n| n != "mirror.rs") {
        let t = code_only(&read_src_file(&name).unwrap_or_default());
        assert!(!t.contains("\"ZIKARON-backup\""), "{name} 自拼了备份名");
    }
}

/// Known deployments table: compiled into the product, chain id and contract always taken from the table; a
/// new home takes its basis through one entry point only.
#[test]
fn the_known_deployments_are_compiled_in_and_one_entry_point_fills_a_home() {
    let d = app::deploy::named(app::deploy::DEFAULT).expect("默认行在表里");
    assert_eq!(d.chain_id, 1);
    assert_eq!(d.registry.to_lowercase(), "0x36ea8a857a5fe813429d4d9947000c644a88809a");
    assert_eq!(d.from_block, 26_087_229);
    let t = app::deploy::named("sepolia").expect("测试网行在表里");
    assert_eq!((t.chain_id, t.registry.to_lowercase().as_str(), t.from_block), (11_155_111, "0xc29410b882c4c3b77e33659d2f06ac563e7b08a3", 11_715_660));
    assert!(d.nodes.iter().all(|u| u.starts_with("https://")) && d.nodes[0] != d.nodes[1]);
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    // A network (a known row, or one filled in by hand) enters a home through one entry point, at two call
    // sites: a writer opening a home without a network takes its identity's, and the wizard's network step
    // fills the current home; a known row is filled inside it.
    assert_eq!(action.matches("adopt_network(shell, ").count(), 2, "two call sites use the one entry point");
    assert_eq!(action.matches("adopt_deployment(shell, d)").count(), 1, "a known row fills a home inside the one entry point only");
    let machine = code_only(&read_src_file("machine.rs").expect("读不出 machine.rs"));
    assert!(!machine.contains("11155111") && !machine.contains("11_155_111"), "机器级设置档不留链号的第二份说法");
}

// ───────────────────── What the verifier should receive ─────────────────────

/// The audit input is resolved level by level with one owner: a closed table of four levels in fixed order;
/// the check page and vault re-check both take material through `supplyx::find`, and nowhere else writes
/// another path to fetch a ledger.
#[test]
fn the_audit_input_is_resolved_level_by_level_in_one_place() {
    use app::supplyx::Level;
    assert_eq!(Level::ALL, [Level::Local, Level::Vault, Level::Kit, Level::Remote], "the levels are tried in exactly this order");
    // Level name spelling is free and not pinned; only that each of the four levels has its own name.
    let mut names: Vec<&str> = Level::ALL.iter().map(|l| l.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Level::ALL.len(), "四级各有各的名");
    assert!(Level::Local < Level::Vault && Level::Vault < Level::Kit && Level::Kit < Level::Remote);
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    assert_eq!(action.matches("supplyx::find(").count(), 1, "动作这一层只有保管库复核自己调那一处;查验页经 checkx::run");
    let checkx = code_only(&read_src_file("checkx.rs").expect("读不出 checkx.rs"));
    assert_eq!(checkx.matches("supplyx::find(").count(), 1);
    // Ledger-fetching paths live only in `supplyx`: other modules may not open other homes by the register
    // themselves.
    for (name, text) in shipped() {
        // `firstrun.rs` opens this home's own ledger (first-run inventory), not other homes by the register.
        // `local.rs` is the one place every sealed ledger of this machine is opened through.
        if name == "supplyx.rs" || name == "firstrun.rs" || name == "local.rs" {
            continue;
        }
        assert!(!code_only(&text).contains("LedgerDir::open("), "{name} 自己去开别人的账本");
    }
}

/// Every gray light speaks from the result object: each missing thing has its own form, gray kept apart from
/// red; an answered state never has a gap, and the six gaps' action sentences all differ.
#[test]
fn every_grey_light_says_what_is_missing() {
    use app::checkx::Gap;
    use app::lang::{t, Key};
    let says = [
        (Gap::NoLedger, Key::NoteNoLedger),
        (Gap::LedgerRefused(String::new()), Key::NoteLedgerRefused),
        (Gap::NoNode, Key::NoteNoNode),
        (Gap::ChainUnread(String::new()), Key::NoteChainUnread),
        (Gap::NotYetAnchored, Key::NoteNotYetAnchored),
        (Gap::NoTime, Key::NoteNoTime),
    ];
    let mut seen: Vec<&str> = says.iter().map(|(_, k)| t(*k)).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), says.len(), "六种缺口六句话");
    assert!(says.iter().all(|(_, k)| !t(*k).is_empty()));
    // The check is on the result object: an answered state (PASS/FAIL) has no gap to speak of.
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains("crate::checkx::gap(&x,"), "脸上的缺口由 checkx 出,不在渲染层重算");
    let checkx = code_only(&read_src_file("checkx.rs").expect("读不出 checkx.rs"));
    assert!(checkx.contains("if state != State::Unknown.as_str()"), "只对未定那一态答");
}

/// Exclusivity is written once at signing and read-only afterwards: the product has no action flipping the
/// flag; only issuing writes it; the older list in the settings file is read-only.
#[test]
fn the_exclusive_flag_is_written_once_at_signing() {
    let action = code_only(&read_src_file("action.rs").expect("读不出 action.rs"));
    assert_eq!(action.matches("termsx::keep(").count(), 1, "落签发记录只在签发那一处");
    assert!(!action.contains("ToggleExclusive"), "翻旗那一枚动作撤了");
    for (name, text) in shipped() {
        let t = code_only(&text);
        assert!(!t.contains("s.exclusive.push"), "{name} 还在写那份旧名单");
        assert!(!t.contains("exclusive.retain"), "{name} 还在改那份旧名单");
    }
    // A three-state closed table: the record at signing, the older list (no terms document), none.
    let legacy = vec!["0xaa".to_string()];
    let rec = app::termsx::Record { grant: "0xbb".into(), terms: "0xcc".into(), exclusive: true, doc: None, name: None };
    assert_eq!(app::termsx::exclusive_of(&[rec.clone()], &legacy, "0xBB"), app::termsx::Exclusive::Signed);
    assert_eq!(app::termsx::exclusive_of(&[], &legacy, "0xAA"), app::termsx::Exclusive::Legacy);
    assert_eq!(app::termsx::exclusive_of(&[], &legacy, "0xdd"), app::termsx::Exclusive::No);
    // The document's in-kit path is assembled in one place and satisfies kit law §7.2.
    let rel = app::termsx::doc_rel("0xAB12", "授权条款.pdf").expect("拼得出");
    assert!(rel.starts_with(&format!("{}/ab12/", app::termsx::ROOM)) && zikaron_kit::kitdir::is_kit_path(&rel), "{rel}");
    for (name, text) in shipped() {
        if name == "termsx.rs" {
            continue;
        }
        assert!(!code_only(&text).contains("\"terms/\""), "{name} 自拼了文书的包内路");
    }
}

/// A grant file is an enumeration handed to the same kit verification: the container judges no hash itself;
/// directory kits and single-file bundles come from the same enumeration (`pack::enumeration`).
#[test]
fn a_grant_file_is_one_enumeration_judged_by_the_same_function() {
    let pairs = zikaron_glue::pack::enumerate(zikaron_glue::pack::Bundle {
        files: vec![("a.txt".into(), b"x".to_vec())],
        note: "n".into(),
        ..Default::default()
    })
    .map(|(p, _)| p)
    .ok()
    .expect("KIT_OK");
    let raw = zikaron_glue::container::encode(&pairs);
    assert!(zikaron_glue::container::is_container(&raw));
    let mut back = zikaron_glue::container::decode(&raw).ok().expect("拆得开");
    assert_eq!(back.first().map(|(p, _)| p.clone()), Some(zikaron_glue::names::MANIFEST.to_string()), "清单在先,读的一侧先拿到它");
    back.sort();
    let mut want = pairs.clone();
    want.sort();
    assert_eq!(back, want, "装了再拆即原样");
    // The reading lives in glue, the one the command line calls too; the app only maps its refusals.
    let grantfilex = code_only(&read_src_file("grantfilex.rs").expect("读不出 grantfilex.rs"));
    assert_eq!(grantfilex.matches("zikaron_glue::grantfile::open(").count(), 1, "应用只经那一处读法");
    assert_eq!(grantfilex.matches("verify_enumeration(").count(), 0, "应用不另起包验");
    let reading = code_only(&std::fs::read_to_string(src().join("../../zikaron-glue/src/grantfile.rs")).expect("读不出 grantfile.rs"));
    assert_eq!(reading.matches("verify_enumeration(").count(), 1, "包验只此一处");
    assert!(!reading.contains("sha256") && !reading.contains("doc_id"), "容器不自己比哈希");
    let pack = std::fs::read_to_string(src().join("../../zikaron-glue/src/pack.rs")).expect("读不出 pack.rs");
    assert_eq!(code_only(&pack).matches("fn enumeration(").count(), 1, "包内路怎么拼只住一处");
    assert!(code_only(&pack).contains("for (rel, bytes) in enumeration(b)"), "铺盘照同一份枚举");
}

/// Remote fetch uses only the same TLS client, https only: the product has no second path for fetching bytes;
/// the publish address does not accept http or other spellings; the product never calls the test-only trust
/// root exit.
#[test]
fn every_fetched_byte_goes_through_the_one_tls_client() {
    use app::fault::Known;
    assert_eq!(app::fetchx::base_of("http://x.example/k/").err().and_then(|f| f.which()), Some(Known::RemoteNotHttps));
    assert_eq!(app::fetchx::base_of("ftp://x.example/").err().and_then(|f| f.which()), Some(Known::RemoteNotHttps));
    assert_eq!(app::fetchx::base_of("https://x.example/k").expect("认得").at("manifest.json"), "https://x.example/k/manifest.json");
    let fetchx = code_only(&read_src_file("fetchx.rs").expect("读不出 fetchx.rs"));
    assert!(fetchx.contains("chainx::Https::new("), "远取复用节点问答那一枚 TLS 客户端");
    // The app opens no connection and builds no TLS of its own: both live in the one transport.
    for (name, text) in shipped() {
        let t = code_only(&text);
        assert!(!t.contains("TcpStream::connect"), "{name} 自己开了一条连接");
        assert!(!t.contains("rustls::"), "{name} 自己碰了 TLS");
        assert!(!t.contains("drive_trust_root"), "{name} calls the trust root reserved for the test driver");
    }
    // The one transport builds the TLS client configuration in one place.
    let net = code_only(&std::fs::read_to_string(src().join("../../zikaron-net/src/lib.rs")).expect("读不出 zikaron-net"));
    assert_eq!(net.matches("ClientConfig::builder").count(), 1, "TLS 配置只建一处");
    let settings = code_only(&read_src_file("settings.rs").expect("读不出 settings.rs"));
    assert!(settings.contains("fetchx::base_of(x)"), "设置档里读回来的发布地址照同一处认");
}

// ───────────────────── Widget library and pages ─────────────────────

/// Boolean state has only one component: the product source has no `ui.checkbox`; exclusivity and auto anchor
/// both use the widget library's switch (`toggle`); the settings page's "system notifications" row (no real
/// source, no consumer) is gone.
#[test]
fn every_boolean_is_the_one_toggle_from_the_kit() {
    for (name, text) in shipped() {
        let t = code_only(&text);
        assert!(!t.contains("ui.checkbox"), "{name} 还有一枚对勾控件");
        assert!(!t.contains(".checkbox("), "{name} 还有一枚对勾控件");
    }
    let window = code_only(&read_src_file("window.rs").expect("读不出 window.rs"));
    assert!(window.contains("toggle::toggle(ui, &mut self.typed.g_exclusive"), "the exclusive flag uses the kit's switch");
    assert!(window.contains("toggle::switch(ui, s.auto_anchor"), "auto put on chain uses the kit's switch");
    assert!(!window.contains("Key::NoticeOn"), "「系统通知 · 已开启」那一行撤了");
    let ui = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/toggle.rs")).expect("the switch file reads"));
    assert_eq!(ui.matches("pub fn toggle(").count(), 1, "one switch with a label");
    assert_eq!(ui.matches("pub fn switch(").count(), 1, "one bare switch");
}

/// Side by side only through the equal-cells component: the page layer never calls `ui.columns` (egui's
/// component gives columns a justified layout, and long strings in a column get stretched letter spacing
/// after wrapping); blocks side by side always go through the widget library's `grid::tiles`.
#[test]
fn side_by_side_goes_through_the_one_grid() {
    for (name, text) in shipped() {
        assert!(!code_only(&text).contains("ui.columns("), "{name} 还直接调 ui.columns");
    }
    let grid = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/grid.rs")).expect("读不出等分格件"));
    assert!(grid.contains("egui::Layout::top_down(egui::Align::Min)"), "格里的布局不两端对齐");
    assert!(!grid.contains("top_down_justified"), "格里的布局不两端对齐");
}

/// Two kinds of red button each in their place: the final-step button (solid dark red) appears only on
/// confirmation sheets; guide buttons (red text on white) may appear several per screen. Every place in the
/// product source taking the final-step token is a sheet's drawing function, and the list of them is the
/// list of confirmation sheets.
#[test]
fn the_solid_red_key_is_only_on_a_confirm_card() {
    let text = read_src_file("window.rs").expect("读不出 window.rs");
    let lines: Vec<&str> = text.lines().collect();
    let mut at: Vec<(usize, String)> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.contains("page::Pen::new()") {
            // Find the fn it belongs to, upwards.
            let owner = lines[..=i]
                .iter()
                .rev()
                .find(|x| ["fn ", "pub fn ", "pub(super) fn ", "pub(crate) fn "].iter().any(|v| x.trim_start().starts_with(v)))
                .map(|x| x.trim().to_string())
                .unwrap_or_default();
            at.push((i + 1, owner));
        }
    }
    let sheets = [
        "fn new_anchor_sheet(",
        "fn send_sheet(",
        "fn delete_record_sheet(",
        "fn grant_confirm_sheet(",
        "fn revoke_sheet(",
        "fn adopt_sheet(",
        "fn succeed_sheet(",
        "fn annotate_sheet(",
        "fn relicense_sheet(",
        "fn genesis_sheet(",
        "fn id_sheets(",
        "fn bk_sheets(",
        // Fetching found this home at odds with the fetched ledger: "fetch and replace".
        "fn conflict_sheet(",
    ];
    for (line, owner) in &at {
        assert!(sheets.iter().any(|s| owner.contains(s)), "window.rs:{line} takes the final-step token outside a confirmation sheet: {owner}");
    }
    for s in sheets {
        // The identity sheets hold two final steps (delete, set as primary); the backup sheets two (restore
        // from settings or at first run, replace on the locked card's confirm).
        let want = if s == "fn id_sheets(" || s == "fn bk_sheets(" { 2 } else { 1 };
        assert_eq!(at.iter().filter(|(_, o)| o.contains(s)).count(), want, "{s} holds exactly {want} final-step key(s)");
    }
    // The widget library side: final-step and guide buttons have separate tokens, and guide buttons have no
    // "at most one per page" limit.
    let page = code_only(&std::fs::read_to_string(src().join("../../zikaron-ui/src/page.rs")).expect("读不出 page.rs"));
    assert!(page.contains("pub struct Guide"), "引导键那一枚令牌在件库里");
    assert!(page.contains("pub fn commits_in"), "「确认卡之外零实心深红」那一条现算腿在件库里");
}

/// The embedded faces by system: on macOS and Windows only Latin and monospace ship with the crate (SIL OFL
/// 1.1) and the Chinese and heavy faces come from the system; on every other system all faces ship with it. Every embedded
/// face carries its licence and no path; a face taken from the system carries a path and no licence. A system
/// face missing on this machine (a build machine without the system's Chinese fonts) is not a failure of the
/// table; an embedded one cannot be missing.
#[test]
fn the_embedded_faces_carry_their_licence() {
    use zikaron_ui::fonts::{Place, Role, ROLES};
    let embedded: Vec<Role> = ROLES.iter().filter(|(_, _, _, p)| *p == Place::Embedded).map(|(r, _, _, _)| *r).collect();
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        assert_eq!(embedded, vec![Role::Latin, Role::Mono], "macOS 与 Windows 上内嵌只有拉丁与等宽两面");
    } else {
        assert_eq!(embedded.len(), Role::ALL.len(), "其余系统每一面都随二进制走");
    }
    let found = zikaron_ui::fonts::find();
    for (role, _, _, place) in ROLES {
        let Some(f) = found.face(role) else {
            assert_eq!(place, Place::System, "{} 是内嵌的,不会找不到", role.as_str());
            continue;
        };
        assert!(f.bytes > 0, "{} 那一面报得出字节数", role.as_str());
        match f.place {
            Place::Embedded => {
                assert_eq!(f.licence(), Some(zikaron_ui::fonts::OFL));
                assert!(f.path.is_none(), "内嵌那一面没有盘上的路");
            }
            Place::System => assert!(f.licence().is_none() && f.path.is_some(), "系统取那一面有路、不带许可"),
        }
    }
    let dir = src().join("../../zikaron-ui/fonts");
    for name in ["JetBrainsMono-Regular.ttf", "Inter-Regular.ttf", "OFL-JetBrainsMono.txt", "OFL-Inter.txt"] {
        assert!(dir.join(name).is_file(), "{name} 随仓");
    }
    for name in ["OFL-JetBrainsMono.txt", "OFL-Inter.txt"] {
        let text = std::fs::read_to_string(dir.join(name)).expect("读得出许可");
        assert!(text.contains("SIL OPEN FONT LICENSE"), "{name} 是 OFL 全文");
    }
}

/// The exit gate's closed table: every action answers whether it lets facts of this ledger leave the machine
/// (`Action::exit`), one arm each and no catch-all, so a new action does not compile until it is classified.
#[test]
fn every_action_answers_whether_it_is_an_exit() {
    let code = code_only(&read_src_file("action/mod.rs").expect("读不出 action/mod.rs"));
    let at = code.find("pub fn exit(&self) -> Option<crate::exitgate::Exit>").expect("归类那一口在");
    let body = &code[at..];
    let end = body.find("\n    }\n").expect("那一口的尾");
    let body = &body[..end];
    assert!(!body.contains("_ =>"), "出口归类不许有通配臂");
    assert_eq!(body.matches("=> Some(Exit::").count(), 5, "出口恰五员");
    assert_eq!(app::exitgate::Exit::ALL.len(), 5);
}

/// The five effects that let facts leave the machine each take the exit gate's `Pass`, and a `Pass` is made in
/// one place only: the gate's own `pass`, after it read the chain. So no effect is reachable but through the
/// gate (the type says so; the compiler holds it). The send asks its balance before the gate.
#[test]
fn every_exit_effect_takes_the_gates_pass() {
    let effects: [(&str, &str); 5] = [
        ("sign.rs", "pub fn anchor_send(\n    _pass: &crate::exitgate::Pass,"),
        ("kitx.rs", "pub fn export(\n    _pass: &crate::exitgate::Pass,"),
        ("badgex.rs", "pub fn export(_pass: &crate::exitgate::Pass,"),
        ("mirror.rs", "pub fn export(_pass: &crate::exitgate::Pass,"),
        ("grantfilex.rs", "pub fn export(_pass: &crate::exitgate::Pass,"),
    ];
    for (file, sig) in effects {
        let code = code_only(&read_src_file(file).unwrap_or_else(|_| panic!("读不出 {file}")));
        assert!(code.contains(sig), "{file}:出口那一处效果不收闸的令牌");
    }
    // The pass is built only by the gate: its fields are private and the one struct literal is in `pass`.
    let gate = code_only(&read_src_file("exitgate.rs").expect("exitgate"));
    assert!(gate.contains("pub struct Pass {\n    reading: Reading,\n    root: PathBuf,\n}"), "令牌的成员须是私有的");
    assert_eq!(gate.matches("Pass { reading").count(), 1, "令牌只在闸里造一处");
    for (name, text) in shipped().into_iter().filter(|(n, _)| n != "exitgate.rs") {
        assert!(!code_only(&text).contains("Pass { reading"), "{name} 自造了令牌");
    }
    let q = code_only(&read_src_file("action/queue.rs").expect("queue"));
    assert!(q.find("funds_gate(&b, &secret, fees)").unwrap_or(usize::MAX) < q.find("crate::exitgate::pass(&ask)").unwrap_or(0), "发交易:余额那一判在出口闸之前");
}

/// The put-on-chain sheet does not close before its broadcast lands (window layer, no window): pressed, it
/// stays open frame after frame while the anchoring task is held; the task let go (here it fails, which lands
/// too), the sheet closes on that landing. Judged by the landing, never by the clock.
#[test]
fn the_send_sheet_stays_open_until_the_broadcast_lands() {
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.gas = Some((1, 21_000));
    let (go, held) = std::sync::mpsc::channel::<()>();
    let started = shell.tasks.spawn(app::task::Kind::Anchor, move || {
        let _ = held.recv();
        Err(app::fault::Fault::known(app::fault::Known::NodeRefused, String::from("held")))
    });
    assert_eq!(started, app::task::Spawned::Started);
    let mut probe = app::window::SendProbe::pressed(shell, 1);
    for i in 0..5 {
        assert!(probe.frame(&ctx), "frame {i}: the sheet closed while the broadcast was still in flight");
    }
    go.send(()).expect("the task waits for this");
    // Wait for the worker itself to end (its outcome is sent before it ends), not for any time.
    while !probe.shell().tasks.finished_in_flight(app::task::Kind::Anchor) {
        std::thread::yield_now();
    }
    let closed = (0..5).any(|_| !probe.frame(&ctx));
    assert!(closed, "the broadcast landed and the sheet stayed open");
}

#[test]
fn the_tail_check_is_recorded_as_asked_before_it_is_asked() {
    // The window's clock polls the tail check every frame. Taking it records the asking for this ledger state
    // at once, so whatever answers (a refusal before the check starts included) is not asked, and said, again
    // on the next frame; the ledger moving makes it due again.
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let root = std::env::temp_dir().join(format!("zk-tail-latch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    shell.home = Some(app::home::Home::open_or_create(&root).expect("home"));
    shell.unfetched = Some(app::restorex::State::Unfetched);
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    shell.endpoints = vec![app::chainx::Endpoint::parse("31337=http://127.0.0.1:9").expect("endpoint")];
    assert!(shell.take_tail_due(), "due once");
    assert!(!shell.tail_due() && !shell.take_tail_due(), "not due again before anything changes");
    shell.book_mark += 1;
    assert!(shell.take_tail_due(), "the ledger moved: due again");
    let _ = std::fs::remove_dir_all(&root);
}
